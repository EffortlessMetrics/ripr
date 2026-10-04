//! Package re-export identity for Python owners.
//!
//! Real Python test suites rarely import a function from the file that defines
//! it. `humanize` tests write `import humanize` and call
//! `humanize.naturaldelta(...)`; `more-itertools` tests write
//! `import more_itertools as mi` and call `mi.one(...)`. Both names reach the
//! owner only because the package `__init__.py` re-exports it
//! (`from .time import naturaldelta`, `from .more import *`). Without that
//! link every changed line in such a package reads `no_static_path` although
//! the suite reaches and kills the mutant.
//!
//! This module computes the dotted package paths under which an owner is
//! re-exported, from the `__init__.py` module imports already parsed into the
//! workspace facts. It is bounded and fails closed:
//!
//! - only `__init__.py` files re-export, and only through `from M import ...`
//!   where `M` is the owner's module (or an earlier re-exporting package);
//! - an explicit re-export must keep the owner's name (`import x as y`
//!   renames are not followed);
//! - a star re-export never carries a `_private` name, and when the source
//!   module binds `__all__` at top level, the name must be listed in it (a
//!   binding that is not a literal list/tuple of strings fails closed);
//! - methods and module owners are never re-exported;
//! - the chain stops after [`MAX_REEXPORT_HOPS`] packages.
//!
//! The result only widens *which module path* identifies the owner. The
//! relation still requires the test to call the owner's name through that
//! path, so an unrelated name imported from the same package stays unrelated.

use super::related_tests::owner_module_paths;
use super::source_facts::parse_module_result;
use super::{PythonImport, PythonOwner};
use crate::domain::OwnerKind;
use rustpython_parser::ast::{self, Expr, Mod, Stmt};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Maximum number of package `__init__.py` hops followed from the owner module.
const MAX_REEXPORT_HOPS: usize = 3;

/// One package `__init__.py` and the module imports it declares.
struct PackageExporter<'a> {
    file: &'a Path,
    module_paths: Vec<String>,
    imports: &'a [PythonImport],
    /// The initializer's parsed statements; `None` when its source is
    /// unreadable or unparsable, which fails every re-export closed.
    body: Option<Vec<Stmt>>,
}

impl PackageExporter<'_> {
    /// Whether the initializer binds `name` other than through a top-level
    /// import (an assignment, `del`, loop or `with` target, or any binding in
    /// a conditional block). Python keeps the last binding and this reader
    /// does not order them, so any such binding fails closed.
    fn rebinds(&self, name: &str) -> bool {
        self.body
            .as_ref()
            .is_none_or(|body| body.iter().any(|stmt| stmt_binds(stmt, name, false)))
    }
}

/// Collects the package exporters (`__init__.py` module owners) from the
/// workspace owners.
fn package_exporters<'o, 's>(
    owners: &'o [PythonOwner],
    source_of: &impl Fn(&Path) -> Option<&'s str>,
) -> Vec<PackageExporter<'o>> {
    owners
        .iter()
        .filter(|owner| owner.is_module_owner() && is_package_init(&owner.file))
        .map(|owner| PackageExporter {
            file: &owner.file,
            module_paths: owner_module_paths(&owner.file),
            imports: &owner.imports,
            body: source_of(&owner.file).and_then(|source| {
                match parse_module_result(&owner.file, source) {
                    Ok(Mod::Module(module)) => Some(module.body),
                    _ => None,
                }
            }),
        })
        .collect()
}

/// Sets `reexport_modules` on every owner, taking the package exporters from
/// the same owner set (diff mode reads the whole workspace up front).
///
/// `source_of` returns the source text of a workspace file; it is used only to
/// honor a declared `__all__` on a star re-export.
pub(super) fn apply_package_reexports<'s>(
    owners: &mut [PythonOwner],
    source_of: impl Fn(&Path) -> Option<&'s str>,
) {
    let init_owners: Vec<PythonOwner> = owners
        .iter()
        .filter(|owner| owner.is_module_owner() && is_package_init(&owner.file))
        .cloned()
        .collect();
    let definitions = TopLevelDefinitions::from_owners(owners.iter());
    apply_with(owners, &init_owners, &definitions, source_of);
}

/// Sets `reexport_modules` on `owners` from separately loaded package
/// `__init__.py` module owners (repo mode loads one production file at a time,
/// so top-level definitions come from `owners`, the initializers, and
/// `definition_owners`: the owners of the modules the initializers
/// star-import).
pub(super) fn apply_package_reexports_from<'s>(
    owners: &mut [PythonOwner],
    init_owners: &[PythonOwner],
    definition_owners: &[PythonOwner],
    source_of: impl Fn(&Path) -> Option<&'s str>,
) {
    let definitions =
        TopLevelDefinitions::from_owners(owners.iter().chain(init_owners).chain(definition_owners));
    apply_with(owners, init_owners, &definitions, source_of);
}

fn apply_with<'s>(
    owners: &mut [PythonOwner],
    init_owners: &[PythonOwner],
    definitions: &TopLevelDefinitions,
    source_of: impl Fn(&Path) -> Option<&'s str>,
) {
    let exporters = package_exporters(init_owners, &source_of);
    let mut star_exports = StarExports {
        source_of,
        declared: HashMap::new(),
    };
    for owner in owners.iter_mut() {
        owner.reexport_modules =
            package_reexport_modules(owner, &exporters, definitions, &mut star_exports);
    }
}

/// Module paths of every file defining a top-level function or class, by name.
struct TopLevelDefinitions {
    by_name: HashMap<String, Vec<Vec<String>>>,
}

impl TopLevelDefinitions {
    fn from_owners<'a>(owners: impl Iterator<Item = &'a PythonOwner>) -> Self {
        let mut by_name: HashMap<String, Vec<Vec<String>>> = HashMap::new();
        for owner in owners {
            if owner.is_module_owner() || owner.qualified_name != owner.name {
                continue;
            }
            by_name
                .entry(owner.name.clone())
                .or_default()
                .push(owner_module_paths(&owner.file));
        }
        Self { by_name }
    }

    /// Whether a module named by any of `module` defines `name` at top level.
    fn defines(&self, module: &str, name: &str) -> bool {
        self.by_name
            .get(name)
            .is_some_and(|files| files.iter().any(|paths| paths.iter().any(|p| p == module)))
    }
}

/// The dotted package paths that re-export `owner` under its own name.
fn package_reexport_modules<'s, F: Fn(&Path) -> Option<&'s str>>(
    owner: &PythonOwner,
    exporters: &[PackageExporter<'_>],
    definitions: &TopLevelDefinitions,
    star_exports: &mut StarExports<'s, F>,
) -> Vec<String> {
    if owner.is_module_owner()
        || matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        )
        || owner.qualified_name != owner.name
        || owner.name.is_empty()
    {
        return Vec::new();
    }
    let name = owner.name.as_str();
    let mut frontier: Vec<(Vec<String>, PathBuf)> =
        vec![(owner_module_paths(&owner.file), owner.file.clone())];
    let mut visited: Vec<&Path> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for _ in 0..MAX_REEXPORT_HOPS {
        let mut next: Vec<(Vec<String>, PathBuf)> = Vec::new();
        for exporter in exporters {
            if visited.contains(&exporter.file) || exporter.file == owner.file.as_path() {
                continue;
            }
            let reexports = frontier.iter().any(|(paths, source_file)| {
                exporter.imports.iter().any(|import| {
                    paths.contains(&import.source_module)
                        && imports_name(import, name, source_file, star_exports)
                })
            });
            if reexports && !binds_name_elsewhere(exporter, name, &frontier, definitions) {
                visited.push(exporter.file);
                for path in &exporter.module_paths {
                    if !out.contains(path) {
                        out.push(path.clone());
                    }
                }
                next.push((exporter.module_paths.clone(), exporter.file.to_path_buf()));
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    out
}

/// Whether the initializer may bind `name` to something other than the
/// frontier's definition: an import of another object under that name
/// (`from .b import f` next to `from .a import f`, `from .b import g as f`,
/// `import f`), a star import from another module that defines `name`, its
/// own top-level definition, or any other binding such as `f = ...`. Python
/// keeps only the last binding; this reader does not order bindings, so any
/// second binding fails closed.
fn binds_name_elsewhere(
    exporter: &PackageExporter<'_>,
    name: &str,
    frontier: &[(Vec<String>, PathBuf)],
    definitions: &TopLevelDefinitions,
) -> bool {
    let in_frontier = |module: &str| {
        frontier
            .iter()
            .any(|(paths, _)| paths.iter().any(|p| p == module))
    };
    let conflicting_import = exporter.imports.iter().any(|import| {
        if import.imported == "*" {
            return !in_frontier(&import.source_module)
                && definitions.defines(&import.source_module, name);
        }
        import.alias == name && (import.imported != name || !in_frontier(&import.source_module))
    });
    conflicting_import
        || exporter.rebinds(name)
        || exporter
            .module_paths
            .iter()
            .any(|module| definitions.defines(module, name))
}

fn imports_name<'s, F: Fn(&Path) -> Option<&'s str>>(
    import: &PythonImport,
    name: &str,
    source_file: &Path,
    star_exports: &mut StarExports<'s, F>,
) -> bool {
    if import.imported == name {
        return import.alias == name;
    }
    import.imported == "*" && !name.starts_with('_') && star_exports.exports(source_file, name)
}

/// What a module's `__all__` says about its star export.
#[derive(Clone, Debug, PartialEq, Eq)]
enum AllDeclaration {
    /// No top-level `__all__` binding: every public name is exported.
    Absent,
    /// A literal list/tuple of string names (including `+=` extensions).
    Names(Vec<String>),
    /// `__all__` is built in a way this reader does not evaluate, or the
    /// source is unreadable or unparsable: fail closed.
    Unresolved,
}

/// Star-export answers per source module, parsing each module once.
struct StarExports<'s, F: Fn(&Path) -> Option<&'s str>> {
    source_of: F,
    declared: HashMap<PathBuf, AllDeclaration>,
}

impl<'s, F: Fn(&Path) -> Option<&'s str>> StarExports<'s, F> {
    fn exports(&mut self, file: &Path, name: &str) -> bool {
        let source_of = &self.source_of;
        let declaration =
            self.declared
                .entry(file.to_path_buf())
                .or_insert_with(|| match source_of(file) {
                    Some(source) => all_declaration(file, source),
                    None => AllDeclaration::Unresolved,
                });
        match declaration {
            AllDeclaration::Absent => true,
            AllDeclaration::Names(names) => names.iter().any(|listed| listed == name),
            AllDeclaration::Unresolved => false,
        }
    }
}

/// Reads the module's top-level `__all__` from the parsed AST. A mention in
/// a comment or docstring is not a declaration; any binding other than a
/// literal list/tuple of strings (or a `+=` of one) is unresolved.
fn all_declaration(file: &Path, source: &str) -> AllDeclaration {
    let Ok(Mod::Module(module)) = parse_module_result(file, source) else {
        return AllDeclaration::Unresolved;
    };
    let mut declaration = AllDeclaration::Absent;
    for stmt in &module.body {
        let (value, extends) = match stmt {
            Stmt::Assign(assign) if assign.targets.iter().any(is_all_name) => {
                (Some(assign.value.as_ref()), false)
            }
            Stmt::AnnAssign(assign) if is_all_name(&assign.target) => {
                (assign.value.as_deref(), false)
            }
            Stmt::AugAssign(assign) if is_all_name(&assign.target) => {
                if !matches!(assign.op, ast::Operator::Add) {
                    return AllDeclaration::Unresolved;
                }
                (Some(assign.value.as_ref()), true)
            }
            Stmt::Expr(expr) if mutates_all(&expr.value) => return AllDeclaration::Unresolved,
            // An import, `del`, loop target or conditional binding replaces
            // or removes the literal list in a way this reader does not track.
            other if stmt_binds(other, "__all__", true) => return AllDeclaration::Unresolved,
            _ => continue,
        };
        let Some(names) = value.and_then(literal_string_names) else {
            return AllDeclaration::Unresolved;
        };
        declaration = match (declaration, extends) {
            (AllDeclaration::Names(mut existing), true) => {
                existing.extend(names);
                AllDeclaration::Names(existing)
            }
            (AllDeclaration::Absent, true) => return AllDeclaration::Unresolved,
            _ => AllDeclaration::Names(names),
        };
    }
    declaration
}

/// Whether `stmt` binds `name`. Compound statements are searched, since a
/// branch or loop body at module level still binds a module name; function
/// and class bodies are not. Top-level imports count only when `imports` is
/// set (the initializer reader already weighs its own top-level imports);
/// imports inside a block always count.
fn stmt_binds(stmt: &Stmt, name: &str, imports: bool) -> bool {
    let target = |expr: &Expr| target_binds(expr, name);
    let block = |body: &[Stmt]| body.iter().any(|stmt| stmt_binds(stmt, name, true));
    match stmt {
        Stmt::Assign(s) => s.targets.iter().any(target),
        Stmt::AnnAssign(s) => target(&s.target),
        Stmt::AugAssign(s) => target(&s.target),
        Stmt::Delete(s) => s.targets.iter().any(target),
        Stmt::FunctionDef(s) => s.name.as_str() == name,
        Stmt::AsyncFunctionDef(s) => s.name.as_str() == name,
        Stmt::ClassDef(s) => s.name.as_str() == name,
        Stmt::Import(s) => imports && s.names.iter().any(|alias| import_binding(alias) == name),
        Stmt::ImportFrom(s) => {
            imports
                && s.names.iter().any(|alias| {
                    alias.name.as_str() != "*"
                        && alias.asname.as_ref().unwrap_or(&alias.name).as_str() == name
                })
        }
        Stmt::For(s) => target(&s.target) || block(&s.body) || block(&s.orelse),
        Stmt::AsyncFor(s) => target(&s.target) || block(&s.body) || block(&s.orelse),
        Stmt::While(s) => block(&s.body) || block(&s.orelse),
        Stmt::If(s) => block(&s.body) || block(&s.orelse),
        Stmt::With(s) => {
            s.items
                .iter()
                .any(|item| item.optional_vars.as_deref().is_some_and(target))
                || block(&s.body)
        }
        Stmt::AsyncWith(s) => {
            s.items
                .iter()
                .any(|item| item.optional_vars.as_deref().is_some_and(target))
                || block(&s.body)
        }
        Stmt::Try(s) => {
            block(&s.body)
                || handlers_bind(&s.handlers, name)
                || block(&s.orelse)
                || block(&s.finalbody)
        }
        Stmt::TryStar(s) => {
            block(&s.body)
                || handlers_bind(&s.handlers, name)
                || block(&s.orelse)
                || block(&s.finalbody)
        }
        Stmt::Match(s) => s.cases.iter().any(|case| block(&case.body)),
        _ => false,
    }
}

fn handlers_bind(handlers: &[ast::ExceptHandler], name: &str) -> bool {
    handlers.iter().any(|handler| {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        handler
            .name
            .as_ref()
            .is_some_and(|bound| bound.as_str() == name)
            || handler.body.iter().any(|stmt| stmt_binds(stmt, name, true))
    })
}

/// The name `import a.b` or `import a.b as c` binds (`a` or `c`).
fn import_binding(alias: &ast::Alias) -> &str {
    match &alias.asname {
        Some(asname) => asname.as_str(),
        None => alias.name.as_str().split('.').next().unwrap_or_default(),
    }
}

fn target_binds(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Name(bound) => bound.id.as_str() == name,
        Expr::Tuple(tuple) => tuple.elts.iter().any(|elt| target_binds(elt, name)),
        Expr::List(list) => list.elts.iter().any(|elt| target_binds(elt, name)),
        Expr::Starred(starred) => target_binds(&starred.value, name),
        _ => false,
    }
}

fn is_all_name(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(name) if name.id.as_str() == "__all__")
}

/// `__all__.extend(...)`, `__all__.append(...)` and similar calls.
fn mutates_all(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    matches!(call.func.as_ref(), Expr::Attribute(attribute) if is_all_name(&attribute.value))
}

fn literal_string_names(expr: &Expr) -> Option<Vec<String>> {
    let elements = match expr {
        Expr::List(list) => &list.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        _ => return None,
    };
    elements
        .iter()
        .map(|element| match element {
            Expr::Constant(constant) => match &constant.value {
                ast::Constant::Str(value) => Some(value.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn is_package_init(file: &Path) -> bool {
    file.file_name().and_then(|name| name.to_str()) == Some("__init__.py")
}
