//! Module-tree edges one Rust file declares (#4435).
//!
//! The diff seeding authority needs to know whether a changed file is part of
//! any Cargo target's module tree. This producer reports, for one file, the
//! out-of-line edges rustc would follow from it: `mod name;` (default
//! resolution, including declarations nested in inline modules), top-level
//! `#[path = "..."] mod name;`, and literal `include!("...")`.
//!
//! Anything that could add an edge this scan cannot resolve makes the scan
//! incomplete instead of silently dropping it:
//!
//! - a parse error, or a source over the nesting budget;
//! - a `#[path]` that is not one plain string literal, or is introduced
//!   through `cfg_attr`;
//! - a `#[path]` on an inline module, or on an out-of-line declaration
//!   nested in one (the directory rules there differ and are not modeled);
//! - an out-of-line `mod name;` outside the item tree (inside a function);
//! - a `mod` keyword or an `include` inside a macro token tree (`cfg_if!`,
//!   `macro_rules!`), where edges only exist after expansion;
//! - any other item-position macro call (a dependency's macro can expand to
//!   `mod name;`), except std's `thread_local!` and `compile_error!`;
//! - any macro call inside a body other than a std macro that cannot emit
//!   items: a statement macro, or a block expression a macro returns, can
//!   declare `#[path = "..."] mod name;` inside a function;
//! - a non-literal `include!`, including the generated-code shape that
//!   names `OUT_DIR` (build output can declare `#[path]` modules).
//!
//! An incomplete scan never proves a file unreachable; the consumer keeps its
//! layout rule for that package.
//!
//! A top-level `mod name;` whose `#[path]` is unresolved still reports the
//! literal targets it spells (`cfg_attr(unix, path = "unix.rs")`), so the
//! consumer can name a changed file that only such a declaration reaches.

use ra_ap_syntax::{
    AstNode, SyntaxKind,
    ast::{self, HasAttrs, HasName},
};
use std::path::PathBuf;

use super::nesting::parse_clean_source_file;
use super::ra::{include_literal_path, parse_rust_string_literal, path_target_from_attributes};
use crate::analysis::facts::ModulePathTarget;

/// Std item-position macros whose expansion cannot declare a module.
/// Compared after any `std::`/`core::` prefix is dropped.
const ITEM_MACROS_WITHOUT_MODULES: [&str; 2] = ["thread_local", "compile_error"];

/// Std macros that expand to an expression or statement with no item in it,
/// so their calls inside a body cannot declare a module. Compared after any
/// `std::`/`core::`/`alloc::` prefix is dropped. A dependency macro that
/// shadows one of these names is not modeled.
const BODY_MACROS_WITHOUT_MODULES: [&str; 35] = [
    "assert",
    "assert_eq",
    "assert_matches",
    "assert_ne",
    "cfg",
    "column",
    "compile_error",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "include_bytes",
    "include_str",
    "line",
    "matches",
    "module_path",
    "option_env",
    "panic",
    "print",
    "println",
    "stringify",
    "thread_local",
    "todo",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln",
];

/// One out-of-line module-tree edge declared by a Rust file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RustModuleTreeEdge {
    /// `mod name;` with default resolution. `inline` names the enclosing
    /// inline modules, outermost first (`mod a { mod b; }` → `["a"]`).
    Default { inline: Vec<String>, name: String },
    /// Top-level `#[path = "..."] mod name;`, relative to the declaring
    /// file's directory.
    Path(PathBuf),
    /// `include!("...")`, relative to the including file's directory.
    Include(PathBuf),
    /// Out-of-line `mod name;` whose `#[path]` target is unresolved (a
    /// `cfg_attr` path, duplicate or non-literal `#[path]`). `candidates`
    /// are the literal `path = "..."` targets its attributes spell, relative
    /// to the declaring file's directory; empty when there are none, or when
    /// the declaration sits in an inline module. `default_applies` is true
    /// when every path is conditional (`cfg_attr`): one that does not apply
    /// leaves default resolution of `name`. `line` is the `mod` token's
    /// 1-based line. Always accompanies an incomplete scan.
    UnresolvedPath {
        inline: Vec<String>,
        name: String,
        line: usize,
        candidates: Vec<PathBuf>,
        default_applies: bool,
    },
}

/// The module-tree edges of one file and whether the scan saw every edge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RustModuleTreeScan {
    pub(crate) edges: Vec<RustModuleTreeEdge>,
    pub(crate) complete: bool,
}

/// Scans one Rust source text for its module-tree edges.
pub(crate) fn rust_module_tree_scan(text: &str) -> RustModuleTreeScan {
    let Some(parse) = parse_clean_source_file(text) else {
        return RustModuleTreeScan::default();
    };
    let tree = parse.tree();
    let mut scan = RustModuleTreeScan {
        edges: Vec::new(),
        complete: true,
    };
    for module in tree.syntax().descendants().filter_map(ast::Module::cast) {
        let attributes = module
            .attrs()
            .map(|attr| attr.syntax().text().to_string())
            .collect::<Vec<_>>();
        let path_target = path_target_from_attributes(&attributes);
        if module.item_list().is_some() {
            if path_target != ModulePathTarget::Default {
                scan.complete = false;
            }
            continue;
        }
        let Some(name) = module.name() else {
            scan.complete = false;
            continue;
        };
        let Some(inline) = enclosing_inline_modules(&module) else {
            scan.complete = false;
            continue;
        };
        match path_target {
            ModulePathTarget::Default => scan.edges.push(RustModuleTreeEdge::Default {
                inline,
                // `mod r#type;` resolves to `type.rs`.
                name: name.text().trim_start_matches("r#").to_string(),
            }),
            ModulePathTarget::Literal(path) if inline.is_empty() => {
                scan.edges
                    .push(RustModuleTreeEdge::Path(PathBuf::from(path)));
            }
            ModulePathTarget::Literal(_) => scan.complete = false,
            ModulePathTarget::Unknown => {
                scan.complete = false;
                let (spelled, always_applies) = spelled_path_literals(&module);
                let candidates = if inline.is_empty() {
                    spelled
                } else {
                    Vec::new()
                };
                let offset: usize = module
                    .mod_token()
                    .map(|token| token.text_range().start().into())
                    .unwrap_or_else(|| module.syntax().text_range().start().into());
                let line = text
                    .get(..offset)
                    .map_or(1, |prefix| prefix.matches('\n').count() + 1);
                // A plain `#[path]`, or `cfg_attr` paths under `p` and
                // `not(p)`, always apply; otherwise a `cfg_attr` path may not,
                // leaving default resolution. A non-literal `#[path]` spells
                // nothing but still always applies.
                let default_applies = !always_applies
                    && !module.attrs().any(|attr| {
                        attr.path()
                            .is_some_and(|path| path.syntax().text() == "path")
                    });
                scan.edges.push(RustModuleTreeEdge::UnresolvedPath {
                    inline,
                    name: name.text().trim_start_matches("r#").to_string(),
                    line,
                    candidates,
                    default_applies,
                });
            }
        }
    }
    for macro_call in tree.syntax().descendants().filter_map(ast::MacroCall::cast) {
        let Some(path) = macro_call.path() else {
            continue;
        };
        let callee = path.syntax().text().to_string();
        let callee = callee.trim_start_matches("::");
        let callee = callee
            .strip_prefix("std::")
            .or_else(|| callee.strip_prefix("core::"))
            .or_else(|| callee.strip_prefix("alloc::"))
            .unwrap_or(callee);
        if callee != "include" {
            // A macro from any crate can expand to `mod x;` without
            // spelling `mod` here, so its expansion is unknown. Std macros
            // that cannot declare modules are the only exception.
            let item_position = macro_call.syntax().parent().is_some_and(|parent| {
                matches!(
                    parent.kind(),
                    SyntaxKind::SOURCE_FILE | SyntaxKind::ITEM_LIST
                )
            });
            let known_without_modules = if item_position {
                ITEM_MACROS_WITHOUT_MODULES.contains(&callee)
            } else {
                // Inside a body any other macro can expand to
                // `#[path = "x.rs"] mod x;`, directly as a statement or in
                // a block expression it returns.
                BODY_MACROS_WITHOUT_MODULES.contains(&callee)
            };
            // Arguments stay an unparsed token tree, so a call nested in
            // them (`println!("{}", dep::with_module!())`) is checked here.
            if !known_without_modules
                || macro_call
                    .token_tree()
                    .is_some_and(|tokens| nests_unknown_macro_call(&tokens))
            {
                scan.complete = false;
            }
            continue;
        }
        // The node spans the call's attributes too; the expression starts at
        // the macro path.
        let start: usize = path.syntax().text_range().start().into();
        let end: usize = macro_call.syntax().text_range().end().into();
        let expression = text.get(start..end).unwrap_or_default();
        match include_literal_path(expression) {
            Some(target) => scan.edges.push(RustModuleTreeEdge::Include(target)),
            // Includes build output (`OUT_DIR`) or a computed path. Generated
            // code can declare `#[path]` modules, so either is unknown.
            None => scan.complete = false,
        }
    }
    // Declarations and includes inside a token tree only exist after macro
    // expansion (`cfg_if! { ... include!("unix.rs"); }`).
    if tree.syntax().descendants_with_tokens().any(|element| {
        let in_token_tree = element
            .parent()
            .is_some_and(|parent| parent.kind() == SyntaxKind::TOKEN_TREE);
        in_token_tree
            && (element.kind() == SyntaxKind::MOD_KW
                || element.as_token().is_some_and(|token| {
                    token.kind() == SyntaxKind::IDENT && token.text() == "include"
                }))
    }) {
        scan.complete = false;
    }
    scan
}

/// Whether a macro argument token tree spells a call (`name!(`, `name![`,
/// `name!{`) to a macro outside [`BODY_MACROS_WITHOUT_MODULES`].
fn nests_unknown_macro_call(tokens: &ast::TokenTree) -> bool {
    let tokens = tokens
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect::<Vec<_>>();
    tokens.windows(3).any(|window| {
        window[0].kind() == SyntaxKind::IDENT
            && window[1].kind() == SyntaxKind::BANG
            && matches!(
                window[2].kind(),
                SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY
            )
            && !BODY_MACROS_WITHOUT_MODULES.contains(&window[0].text())
    })
}

/// Every `path = "<literal>"` a declaration's attributes introduce: a plain
/// `#[path = ..]`, and the attributes a `cfg_attr` introduces after its
/// predicate, nested `cfg_attr` included. A predicate's own `key = "value"`
/// pairs are skipped, even one whose key is `path`.
///
/// The second value is true when some introduced path always applies: a
/// plain `#[path]`, or two top-level `cfg_attr` paths whose predicates are
/// `p` and `not(p)`. Default resolution is then never the module's file.
fn spelled_path_literals(module: &ast::Module) -> (Vec<PathBuf>, bool) {
    let mut candidates = Vec::new();
    let mut plain_path = false;
    let mut predicates = Vec::new();
    for attr in module.attrs() {
        let tokens = attr
            .syntax()
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia())
            .map(|token| (token.kind(), token.text().to_string()))
            .collect::<Vec<_>>();
        // `#[ .. ]` or `#![ .. ]`: the attribute body sits inside the brackets.
        let Some(open) = tokens
            .iter()
            .position(|(kind, _)| *kind == SyntaxKind::L_BRACK)
        else {
            continue;
        };
        let body = tokens
            .get(open + 1..tokens.len().saturating_sub(1))
            .unwrap_or_default();
        if !introduced_path_literals(body, &mut candidates) {
            continue;
        }
        match cfg_attr_arguments(body) {
            None => plain_path = true,
            // Only a predicate that alone gates a path directly can pair
            // with its negation; a nested `cfg_attr` adds a condition.
            Some(arguments) => {
                let direct = arguments.iter().skip(1).any(|argument| {
                    cfg_attr_arguments(argument).is_none()
                        && introduced_path_literals(argument, &mut Vec::new())
                });
                if let Some(predicate) = arguments.first().filter(|_| direct) {
                    predicates.push(
                        predicate
                            .iter()
                            .map(|(_, text)| text.as_str())
                            .collect::<String>(),
                    );
                }
            }
        }
    }
    let complementary = predicates
        .iter()
        .any(|predicate| predicates.contains(&format!("not({predicate})")));
    (candidates, plain_path || complementary)
}

/// The top-level comma-separated arguments of `cfg_attr(..)`, predicate
/// first, or `None` when `body` is not a `cfg_attr`.
fn cfg_attr_arguments(body: &[(SyntaxKind, String)]) -> Option<Vec<&[(SyntaxKind, String)]>> {
    // The parser tokenizes `cfg_attr` as a contextual keyword.
    let [
        (name_kind, name),
        (SyntaxKind::L_PAREN, _),
        inner @ ..,
        (SyntaxKind::R_PAREN, _),
    ] = body
    else {
        return None;
    };
    if name != "cfg_attr" || !matches!(name_kind, SyntaxKind::IDENT | SyntaxKind::CFG_ATTR_KW) {
        return None;
    }
    let mut depth = 0usize;
    let mut segment_start = 0usize;
    let mut arguments = Vec::new();
    for (position, (kind, _)) in inner.iter().enumerate() {
        match kind {
            SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY => depth += 1,
            SyntaxKind::R_PAREN | SyntaxKind::R_BRACK | SyntaxKind::R_CURLY => {
                depth = depth.saturating_sub(1);
            }
            SyntaxKind::COMMA if depth == 0 => {
                arguments.push(inner.get(segment_start..position).unwrap_or_default());
                segment_start = position + 1;
            }
            _ => {}
        }
    }
    arguments.push(inner.get(segment_start..).unwrap_or_default());
    Some(arguments)
}

/// Collects the `path` literal of one attribute body (`path = ".."`, or
/// what `cfg_attr(<predicate>, <attr>, ..)` introduces, recursively), and
/// says whether it introduced any.
fn introduced_path_literals(body: &[(SyntaxKind, String)], candidates: &mut Vec<PathBuf>) -> bool {
    if let [
        (SyntaxKind::IDENT, key),
        (SyntaxKind::EQ, _),
        (SyntaxKind::STRING, value),
    ] = body
        && key == "path"
        && let Some(literal) = parse_rust_string_literal(value)
    {
        if !candidates.contains(&PathBuf::from(&literal)) {
            candidates.push(PathBuf::from(literal));
        }
        return true;
    }
    let Some(arguments) = cfg_attr_arguments(body) else {
        return false;
    };
    // The predicate introduces nothing; every argument after it is visited.
    let mut introduced = false;
    for argument in arguments.into_iter().skip(1) {
        introduced |= introduced_path_literals(argument, candidates);
    }
    introduced
}

/// The inline modules enclosing an out-of-line declaration, outermost first,
/// or `None` when the declaration sits outside the item tree (a function body
/// or other block).
fn enclosing_inline_modules(module: &ast::Module) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for ancestor in module.syntax().ancestors().skip(1) {
        match ancestor.kind() {
            SyntaxKind::SOURCE_FILE => {
                names.reverse();
                return Some(names);
            }
            SyntaxKind::ITEM_LIST => {}
            SyntaxKind::MODULE => {
                let inline = ast::Module::cast(ancestor)?;
                names.push(inline.name()?.text().trim_start_matches("r#").to_string());
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default(inline: &[&str], name: &str) -> RustModuleTreeEdge {
        RustModuleTreeEdge::Default {
            inline: inline.iter().map(|segment| segment.to_string()).collect(),
            name: name.to_string(),
        }
    }

    #[test]
    fn scan_reports_default_path_include_and_nested_inline_edges() {
        let scan = rust_module_tree_scan(
            "mod plain;\n\
             #[cfg(test)]\nmod tests;\n\
             #[path = \"odd/place.rs\"]\nmod placed;\n\
             pub mod outer { pub mod inner { mod leaf; } }\n\
             mod inline_only { fn f() {} }\n\
             include!(\"fragment.rs\");\n",
        );
        assert!(scan.complete, "{scan:?}");
        assert_eq!(
            scan.edges,
            vec![
                default(&[], "plain"),
                default(&[], "tests"),
                RustModuleTreeEdge::Path(PathBuf::from("odd/place.rs")),
                default(&["outer", "inner"], "leaf"),
                RustModuleTreeEdge::Include(PathBuf::from("fragment.rs")),
            ]
        );
    }

    #[test]
    fn scan_is_incomplete_for_edges_it_cannot_resolve() {
        for source in [
            "#[path = concat!(\"a\", \".rs\")]\nmod dynamic;\n",
            "#[cfg_attr(unix, path = \"unix.rs\")]\nmod platform;\n",
            "#[path = \"inline\"]\nmod inline { mod child; }\n",
            "mod outer { #[path = \"x.rs\"] mod child; }\n",
            "fn f() { mod hidden; }\n",
            "cfg_if::cfg_if! { if #[cfg(unix)] { mod unix; } }\n",
            "macro_rules! declare { ($name:ident) => { mod $name; }; }\n",
            "include!(concat!(\"frag\", \".rs\"));\n",
            "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
            "decl::declare_mod!(generated);\n",
            "cfg_if::cfg_if! { if #[cfg(unix)] { include!(\"unix.rs\"); } }\n",
            "mod outer { lazy_static::lazy_static! { static ref X: u8 = 1; } }\n",
            "fn f() { decl::declare_path_mod!(generated); }\n",
            "fn f() -> u8 { decl::with_module!(generated) }\n",
            "fn f() { let _ = preinclude!(\"x.rs\"); }\n",
            "fn f() { println!(\"{}\", decl::with_module!(generated)); }\n",
            "mod broken\n",
        ] {
            assert!(
                !rust_module_tree_scan(source).complete,
                "`{source}` must not scan as complete"
            );
        }
        // Over the nesting budget the parse is refused and the scan stays incomplete.
        let deep = format!(
            "mod plain;\nfn f() {}{}\n",
            "{".repeat(300),
            "}".repeat(300)
        );
        assert!(!rust_module_tree_scan(&deep).complete);
    }

    fn unresolved(
        line: usize,
        name: &str,
        candidates: &[&str],
        default_applies: bool,
    ) -> RustModuleTreeEdge {
        RustModuleTreeEdge::UnresolvedPath {
            inline: Vec::new(),
            name: name.to_string(),
            line,
            candidates: candidates.iter().map(PathBuf::from).collect(),
            default_applies,
        }
    }

    #[test]
    fn unresolved_path_declarations_report_the_targets_they_spell() {
        let scan = rust_module_tree_scan(
            "#[cfg_attr(all(unix, feature = \"fast\"), path = \"unix.rs\")]\n\
             #[cfg_attr(windows, cfg_attr(target_env = \"msvc\", path = r\"win/msvc.rs\"))]\n\
             mod sys;\n\
             #[path = \"a.rs\"]\n#[path = \"b.rs\"]\nmod twice;\n\
             #[path = concat!(\"dyn\", \".rs\")]\nmod r#dynamic;\n\
             mod outer { #[cfg_attr(unix, path = \"nested.rs\")] mod inner; }\n\
             #[cfg_attr(path = \"predicate.rs\", path = \"chosen.rs\")] mod keyed;\n\
             #[cfg_attr(unix, path = \"u.rs\")]\n#[cfg_attr(not(unix), path = \"w.rs\")]\nmod either;\n\
             #[cfg_attr(unix, cfg_attr(x, path = \"n.rs\"))]\n#[cfg_attr(not(unix), path = \"m.rs\")]\nmod nested_pair;\n",
        );
        assert!(!scan.complete, "{scan:?}");
        assert_eq!(
            scan.edges,
            vec![
                // A cfg predicate's own `feature = "..."` is not a target.
                unresolved(3, "sys", &["unix.rs", "win/msvc.rs"], true),
                // A plain `#[path]` always applies: no default resolution.
                unresolved(6, "twice", &["a.rs", "b.rs"], false),
                unresolved(8, "dynamic", &[], false),
                // Inside an inline module the directory rules differ, so no
                // spelled target is reported; default resolution still is.
                RustModuleTreeEdge::UnresolvedPath {
                    inline: vec!["outer".to_string()],
                    name: "inner".to_string(),
                    line: 9,
                    candidates: Vec::new(),
                    default_applies: true,
                },
                // A custom cfg whose key is `path` is predicate, not target.
                unresolved(10, "keyed", &["chosen.rs"], true),
                // `unix` and `not(unix)` cover every target: no default.
                unresolved(13, "either", &["u.rs", "w.rs"], false),
                // Under `unix` without `x` neither path applies.
                unresolved(16, "nested_pair", &["n.rs", "m.rs"], true),
            ]
        );
    }

    #[test]
    fn scan_ignores_non_item_text_and_resolves_raw_identifiers() {
        let scan = rust_module_tree_scan(
            "// mod commented;\n\
             const TEXT: &str = \"mod quoted;\";\n\
             fn f() { println!(\"in a body\"); assert_eq!(1, 1); let _ = std::vec![1]; assert!(!(1 != 2), \"{}\", format!(\"x\")); }\n\
             thread_local! { static COUNT: u8 = 0; }\n\
             mod r#type;\n\
             mod r#async { mod r#match; }\n\
             std::include!(\"frag.rs\");\n",
        );
        assert!(scan.complete, "{scan:?}");
        assert_eq!(
            scan.edges,
            vec![
                default(&[], "type"),
                default(&["async"], "match"),
                RustModuleTreeEdge::Include(PathBuf::from("frag.rs")),
            ]
        );
    }
}
