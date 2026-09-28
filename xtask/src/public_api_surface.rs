//! Transitive public-API surface collection for `check-public-api` (#3052).
//!
//! The previous collector matched two line prefixes in
//! `crates/ripr/src/lib.rs`, so every `pub` item reachable through an
//! allowlisted `pub mod` was invisible to the gate. This module walks the
//! crate's module tree with `ra_ap_syntax` — the same parser and
//! parse-and-walk shape already used by `no_panic::collect_semantic_panic_findings`
//! — and records module-level items whose visibility is a bare `pub`.
//!
//! Scope boundary, stated so the gate does not overclaim again: this collects
//! module-level items only. Public struct fields, enum variants, trait items,
//! and associated functions in `impl` blocks are semver-relevant but are not
//! collected here.
//!
//! Visibility boundary: a module is nameable only when a bare `pub` chain
//! reaches it from the crate root. A non-`pub` module is still walked, because
//! `#[macro_export]` binds a macro at the crate root whatever the declaring
//! module's visibility; nothing else in such a module is recorded.
//!
//! Configuration boundary: `cfg` predicates are evaluated with `test = false`
//! and every other option unknown, so an item is dropped only when no non-test
//! build can compile it.
//!
//! Resolution boundary: this is a syntax walk, not a name resolution pass. A
//! `pub use` records the name it binds in its own module, which is the
//! nameable public path, without resolving what that name refers to. A glob
//! re-export cannot be expanded and is recorded as a glob entry so it must be
//! recorded deliberately rather than passing silently.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ra_ap_syntax::ast::{self, HasAttrs, HasName, HasVisibility};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxNode};

use crate::read_text_lossy;

/// Collect the public API surface reachable from `root_file`, naming every
/// entry under `crate_name`.
///
/// Entries are `<kind> <path>` (for example `pub mod ripr::domain`,
/// `pub const ripr::output::start_here_state::START_HERE_CLEAN`), sorted and
/// deduplicated. Anything the walk cannot resolve is an error rather than a
/// silent omission.
pub(crate) fn public_api_surface(
    root_file: &Path,
    crate_name: &str,
) -> Result<Vec<String>, String> {
    let mut collector = Collector::default();
    collector.module_file(root_file, crate_name, Scope::Nameable)?;
    Ok(collector.entries.into_iter().collect())
}

/// What a module contributes to the surface.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// The module is nameable from outside the crate, so every bare-`pub`
    /// module-level item in it is public API.
    Nameable,
    /// The module is not nameable from outside the crate. Its ordinary items
    /// are invisible, but `#[macro_export]` still binds at the crate root, so
    /// the walk continues for macros alone.
    MacroExportsOnly,
}

#[derive(Default)]
struct Collector {
    entries: BTreeSet<String>,
    /// Completed work, keyed by file *and* module path: one file reached under
    /// two `#[path]` module declarations is nameable under both paths, so the
    /// file alone is not the unit of work.
    visited: BTreeSet<(PathBuf, String)>,
    /// Files whose walk is in progress, so a module tree that names itself
    /// terminates instead of growing a module path forever.
    active: BTreeSet<PathBuf>,
}

impl Collector {
    fn module_file(&mut self, file: &Path, module_path: &str, scope: Scope) -> Result<(), String> {
        let key = visit_key(file);
        let visit = (key.clone(), format!("{}{module_path}", scope_tag(scope)));
        if self.visited.contains(&visit) {
            return Ok(());
        }
        // Check the cycle guard before recording completed work. Inserting into
        // `visited` first meant a walk refused as cyclic still left its pair
        // behind, so a later non-cyclic path reaching the same file at the same
        // module path returned early and recorded nothing — a silent omission
        // in a gate that must fail closed.
        if !self.active.insert(key.clone()) {
            return Ok(());
        }
        let result = self.walk_module_file(file, module_path, scope);
        self.active.remove(&key);
        self.visited.insert(visit);
        result
    }

    fn walk_module_file(
        &mut self,
        file: &Path,
        module_path: &str,
        scope: Scope,
    ) -> Result<(), String> {
        let text = read_text_lossy(file)?;
        let parse = SourceFile::parse(&text, Edition::Edition2024);
        if !parse.errors().is_empty() {
            return Err(format!(
                "{}: the file does not parse, so its public surface cannot be collected",
                file.display()
            ));
        }
        let dirs = ModuleDirs::for_file(file)?;
        self.items(parse.tree().syntax(), &dirs, module_path, file, scope)
    }

    fn items(
        &mut self,
        node: &SyntaxNode,
        dirs: &ModuleDirs,
        module_path: &str,
        owner: &Path,
        scope: Scope,
    ) -> Result<(), String> {
        for child in node.children() {
            let Some(item) = ast::Item::cast(child) else {
                continue;
            };
            if excluded_from_non_test_builds(&item) {
                continue;
            }
            // `#[macro_export]` puts the macro at the crate root regardless of
            // where it is declared, so neither its own visibility nor its
            // module's applies.
            if let ast::Item::MacroRules(macro_rules) = &item
                && has_macro_export(macro_rules)
            {
                let name = item_name(macro_rules.name());
                self.entries
                    .insert(format!("pub macro {}::{name}", crate_root(module_path)));
                continue;
            }
            if let ast::Item::Module(module) = &item {
                self.module(module, dirs, module_path, owner, scope)?;
                continue;
            }
            if scope == Scope::MacroExportsOnly {
                continue;
            }
            self.nameable_item(&item, module_path);
        }
        Ok(())
    }

    fn nameable_item(&mut self, item: &ast::Item, module_path: &str) {
        match item {
            ast::Item::Use(use_item) if is_bare_pub(use_item) => {
                self.uses(use_item, module_path);
            }
            ast::Item::Fn(item) => self.named(item, "fn", module_path, item.name()),
            ast::Item::Struct(item) => self.named(item, "struct", module_path, item.name()),
            ast::Item::Enum(item) => self.named(item, "enum", module_path, item.name()),
            ast::Item::Union(item) => self.named(item, "union", module_path, item.name()),
            ast::Item::Trait(item) => self.named(item, "trait", module_path, item.name()),
            ast::Item::TypeAlias(item) => self.named(item, "type", module_path, item.name()),
            ast::Item::Const(item) => self.named(item, "const", module_path, item.name()),
            ast::Item::Static(item) => self.named(item, "static", module_path, item.name()),
            ast::Item::ExternCrate(item) if is_bare_pub(item) => {
                let name = match item.rename().and_then(|rename| rename.name()) {
                    Some(name) => name.text().to_string(),
                    None => match item.name_ref() {
                        Some(name_ref) => name_ref.text().to_string(),
                        None => "<unnamed>".to_string(),
                    },
                };
                self.entries
                    .insert(format!("pub extern crate {module_path}::{name}"));
            }
            ast::Item::MacroDef(item) => self.named(item, "macro", module_path, item.name()),
            // Items that are not bare `pub`, plus `impl` associated items,
            // `extern` blocks, and macro invocations, which are outside the
            // stated module-level scope of this collector.
            _ => {}
        }
    }

    fn module(
        &mut self,
        module: &ast::Module,
        dirs: &ModuleDirs,
        module_path: &str,
        owner: &Path,
        scope: Scope,
    ) -> Result<(), String> {
        // A module is nameable only if it is bare `pub` inside an already
        // nameable module. Items declared below a non-`pub` module are not
        // nameable from outside the crate; anything re-exported out of one is
        // recorded at the `pub use` that binds the name. The walk continues
        // regardless, because `#[macro_export]` ignores module visibility.
        let child_scope = if scope == Scope::Nameable && is_bare_pub(module) {
            Scope::Nameable
        } else {
            Scope::MacroExportsOnly
        };
        let name = item_name(module.name());
        let child_path = format!("{module_path}::{name}");
        if child_scope == Scope::Nameable {
            self.entries.insert(format!("pub mod {child_path}"));
        }

        if let Some(list) = module.item_list() {
            return self.items(
                list.syntax(),
                &dirs.inside_inline_module(&name),
                &child_path,
                owner,
                child_scope,
            );
        }
        let declaration = if is_bare_pub(module) {
            "pub mod"
        } else {
            "mod"
        };
        let file = resolve_module_file(
            dirs,
            &name,
            path_attribute(module).as_deref(),
            owner,
            declaration,
        )?;
        self.module_file(&file, &child_path, child_scope)
    }

    fn uses(&mut self, use_item: &ast::Use, module_path: &str) {
        let Some(tree) = use_item.use_tree() else {
            return;
        };
        let mut leaves = Vec::new();
        expand_use_tree(&tree, &[], &mut leaves);
        for leaf in leaves {
            match leaf {
                UseLeaf::Name(name) => {
                    self.entries
                        .insert(format!("pub use {module_path}::{name}"));
                }
                // A syntax walk cannot enumerate what a glob re-exports.
                // Recording the glob keeps it visible to the gate instead of
                // silently widening the surface.
                UseLeaf::Glob => {
                    self.entries.insert(format!("pub use {module_path}::*"));
                }
            }
        }
    }

    fn named<T: HasVisibility>(
        &mut self,
        item: &T,
        kind: &str,
        module_path: &str,
        name: Option<ast::Name>,
    ) {
        if !is_bare_pub(item) {
            return;
        }
        self.entries
            .insert(format!("pub {kind} {module_path}::{}", item_name(name)));
    }
}

fn scope_tag(scope: Scope) -> &'static str {
    match scope {
        Scope::Nameable => "",
        // A file first walked for macros alone must still be walked in full if
        // a nameable path to it appears later.
        Scope::MacroExportsOnly => "macro-only ",
    }
}

/// The identity of a file for visit bookkeeping. `#[path]` attributes compose
/// relative segments, so two spellings can name one file; canonicalizing keeps
/// the cycle guard and the visited set keyed by the file itself. A path that
/// cannot be canonicalized keeps its literal spelling and is still bounded by
/// the module tree.
fn visit_key(file: &Path) -> PathBuf {
    fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf())
}

enum UseLeaf {
    Name(String),
    Glob,
}

fn expand_use_tree(tree: &ast::UseTree, prefix: &[String], out: &mut Vec<UseLeaf>) {
    let mut segments = prefix.to_vec();
    if let Some(path) = tree.path() {
        for segment in path.segments() {
            segments.push(segment.syntax().text().to_string());
        }
    }
    if let Some(list) = tree.use_tree_list() {
        for child in list.use_trees() {
            expand_use_tree(&child, &segments, out);
        }
        return;
    }
    if tree.star_token().is_some() {
        out.push(UseLeaf::Glob);
        return;
    }
    if let Some(rename) = tree.rename() {
        let name = match rename.name() {
            Some(name) => name.text().to_string(),
            None => "_".to_string(),
        };
        out.push(UseLeaf::Name(name));
        return;
    }
    let mut reversed = segments.iter().rev();
    let Some(last) = reversed.next() else {
        return;
    };
    if last == "self" {
        // `use foo::bar::{self}` binds `bar`, not `self`.
        if let Some(parent) = reversed.next() {
            out.push(UseLeaf::Name(parent.clone()));
        }
        return;
    }
    out.push(UseLeaf::Name(last.clone()));
}

/// A bare `pub` is the only visibility that reaches outside the crate.
/// `pub(crate)`, `pub(super)`, `pub(self)`, and `pub(in path)` all carry a
/// `VisibilityInner` and are rejected.
fn is_bare_pub<T: HasVisibility>(item: &T) -> bool {
    match item.visibility() {
        Some(visibility) => visibility.visibility_inner().is_none(),
        None => false,
    }
}

fn item_name(name: Option<ast::Name>) -> String {
    match name {
        Some(name) => name.text().to_string(),
        // Fail visible rather than silent: an unnamed item still surfaces as an
        // entry the allowlist must account for.
        None => "<unnamed>".to_string(),
    }
}

fn crate_root(module_path: &str) -> &str {
    match module_path.split_once("::") {
        Some((root, _)) => root,
        None => module_path,
    }
}

/// Whether no non-test build can compile this item.
///
/// The baseline surface is what a normal `cargo build` exports, so each `cfg`
/// predicate is evaluated with `test = false` and every other option left
/// unknown; the item is dropped only when the predicate is false under every
/// assignment of those unknowns. `#[cfg(any(test, feature = "x"))]` therefore
/// survives — a feature-enabled build exports it — while
/// `#[cfg(all(test, not(feature = "x")))]` does not. Stacked `#[cfg]`
/// attributes are conjunctive, so one definitely-false predicate is enough.
fn excluded_from_non_test_builds<T: HasAttrs>(item: &T) -> bool {
    item.attrs().any(|attr| match attr.meta() {
        Some(ast::Meta::CfgMeta(cfg)) => cfg
            .cfg_predicate()
            .is_some_and(|predicate| evaluate_cfg(&predicate) == Truth::False),
        _ => false,
    })
}

/// Three-valued because most `cfg` options (features, targets) are neither
/// fixed nor decidable here; only a decided `False` may drop an item.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

fn evaluate_cfg(predicate: &ast::CfgPredicate) -> Truth {
    match predicate {
        ast::CfgPredicate::CfgAtom(atom) => evaluate_cfg_atom(atom),
        ast::CfgPredicate::CfgComposite(composite) => evaluate_cfg_composite(composite),
    }
}

fn evaluate_cfg_atom(atom: &ast::CfgAtom) -> Truth {
    if atom.true_token().is_some() {
        return Truth::True;
    }
    if atom.false_token().is_some() {
        return Truth::False;
    }
    // `test` is the one option this walk fixes. A `key = "value"` atom is never
    // it, so `#[cfg(feature = "test")]` stays unknown rather than being read as
    // the `test` option.
    let is_bare_test = atom.eq_token().is_none()
        && atom
            .ident_token()
            .is_some_and(|token| token.text() == "test");
    if is_bare_test {
        Truth::False
    } else {
        Truth::Unknown
    }
}

fn evaluate_cfg_composite(composite: &ast::CfgComposite) -> Truth {
    let operands: Vec<Truth> = composite
        .cfg_predicates()
        .map(|predicate| evaluate_cfg(&predicate))
        .collect();
    let keyword = composite.keyword();
    match keyword.as_ref().map(|token| token.text()) {
        Some("not") => match operands.as_slice() {
            [inner] => negate(*inner),
            // A malformed `not` decides nothing rather than dropping the item.
            _ => Truth::Unknown,
        },
        // `all()` is true and `any()` is false on an empty operand list, which
        // is what the empty folds below produce.
        Some("all") => {
            if operands.contains(&Truth::False) {
                Truth::False
            } else if operands.contains(&Truth::Unknown) {
                Truth::Unknown
            } else {
                Truth::True
            }
        }
        Some("any") => {
            if operands.contains(&Truth::True) {
                Truth::True
            } else if operands.contains(&Truth::Unknown) {
                Truth::Unknown
            } else {
                Truth::False
            }
        }
        // An unrecognized combinator must not decide the item's fate.
        _ => Truth::Unknown,
    }
}

fn negate(value: Truth) -> Truth {
    match value {
        Truth::True => Truth::False,
        Truth::False => Truth::True,
        Truth::Unknown => Truth::Unknown,
    }
}

fn has_macro_export(item: &ast::MacroRules) -> bool {
    item.attrs().any(|attr| match attr.meta() {
        Some(ast::Meta::PathMeta(meta)) => meta
            .path()
            .is_some_and(|path| path.syntax().text().to_string().trim() == "macro_export"),
        _ => false,
    })
}

fn path_attribute(module: &ast::Module) -> Option<String> {
    for attr in module.attrs() {
        let Some(ast::Meta::KeyValueMeta(meta)) = attr.meta() else {
            continue;
        };
        let is_path = meta
            .path()
            .is_some_and(|path| path.syntax().text().to_string().trim() == "path");
        if !is_path {
            continue;
        }
        let value = meta.expr()?.syntax().text().to_string();
        return Some(value.trim().trim_matches('"').to_string());
    }
    None
}

/// The two base directories a `mod` declaration resolves against, which differ
/// outside `lib.rs`/`main.rs`/`mod.rs`.
///
/// A name-resolved `mod foo;` in `bar.rs` looks in `bar/`, but a
/// `#[path = "..."]` on a module declared at the top level of `bar.rs` is
/// relative to the directory holding `bar.rs` itself. Inside an inline
/// `mod block { ... }` both bases become the inline module's directory.
struct ModuleDirs {
    /// Where a name-resolved child module file is looked up.
    children: PathBuf,
    /// What a `#[path]` attribute is relative to.
    path_attribute: PathBuf,
}

impl ModuleDirs {
    fn for_file(file: &Path) -> Result<Self, String> {
        let parent = file
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", file.display()))?;
        let stem = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("{} has no readable file stem", file.display()))?;
        let children = if matches!(stem, "lib" | "main" | "mod") {
            parent.to_path_buf()
        } else {
            parent.join(stem)
        };
        Ok(Self {
            children,
            path_attribute: parent.to_path_buf(),
        })
    }

    fn inside_inline_module(&self, name: &str) -> Self {
        let nested = self.children.join(name);
        Self {
            children: nested.clone(),
            path_attribute: nested,
        }
    }
}

fn resolve_module_file(
    dirs: &ModuleDirs,
    name: &str,
    path_attribute: Option<&str>,
    owner: &Path,
    declaration: &str,
) -> Result<PathBuf, String> {
    let dir = &dirs.children;
    if let Some(relative) = path_attribute {
        let candidate = dirs.path_attribute.join(relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(format!(
            "{}: `{declaration} {name};` has #[path = \"{relative}\"] which does not resolve to {}",
            owner.display(),
            candidate.display()
        ));
    }
    let flat = dir.join(format!("{name}.rs"));
    if flat.is_file() {
        return Ok(flat);
    }
    let nested = dir.join(name).join("mod.rs");
    if nested.is_file() {
        return Ok(nested);
    }
    Err(format!(
        "{}: `{declaration} {name};` resolves to neither {} nor {}, so its public surface cannot be collected",
        owner.display(),
        flat.display(),
        nested.display()
    ))
}

#[cfg(test)]
mod tests;
