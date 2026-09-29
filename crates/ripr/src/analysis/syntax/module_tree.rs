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

use ra_ap_syntax::{
    AstNode, SyntaxKind,
    ast::{self, HasAttrs, HasName},
};
use std::path::PathBuf;

use super::nesting::parse_clean_source_file;
use super::ra::{include_literal_path, path_target_from_attributes};
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
            ModulePathTarget::Literal(_) | ModulePathTarget::Unknown => scan.complete = false,
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
