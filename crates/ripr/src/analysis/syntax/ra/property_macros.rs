//! Overlay reparse of `proptest!` / `quickcheck!` token trees (#4789).
//!
//! The outer Rust grammar keeps a macro call's body as an opaque token tree,
//! so inner `fn` items, owner calls, and `prop_assert*` oracles never become
//! facts. This module copies only those inner bytes into a same-length
//! overlay (every other byte is a space, newlines stay) so a second parse
//! sees real items at the original offsets and lines.
//!
//! Proptest strategy parameter lists (`x in 0u32..100`) are not Rust and
//! cause the grammar to drop the function body; they carry no test facts, so
//! the overlay blanks them. A `proptest!` fn is a test only when it spells
//! `#[test]`. Every `quickcheck!` fn is a test.

use ra_ap_syntax::{AstNode, Edition, Parse, SourceFile, SyntaxKind, ast};

use super::{function_module_segments, is_cfg_test_module_member, text_size_to_usize};

/// Which property-test block produced an overlay function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropertyMacroKind {
    Proptest,
    Quickcheck,
}

/// Provenance for one overlay function: the originating block and the
/// surrounding module path from the *outer* parse (the overlay itself is
/// top-level items, so it cannot recover `mod tests { ... }` ancestry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PropertyMacroOrigin {
    pub(super) kind: PropertyMacroKind,
    pub(super) module_segments: Vec<String>,
    pub(super) inside_cfg_test: bool,
    inner_start: usize,
    inner_end: usize,
}

impl PropertyMacroKind {
    fn treats_fn_as_test(self, spelled_test_attribute: bool) -> bool {
        match self {
            Self::Quickcheck => true,
            Self::Proptest => spelled_test_attribute,
        }
    }
}

pub(super) fn executable_test_attribute(
    origin: Option<&PropertyMacroOrigin>,
    spelled_test_attribute: bool,
) -> bool {
    match origin {
        Some(origin) => origin.kind.treats_fn_as_test(spelled_test_attribute),
        None => spelled_test_attribute,
    }
}

pub(super) fn record_quickcheck_attr(
    origin: Option<&PropertyMacroOrigin>,
    attrs: &mut Vec<String>,
) {
    if !origin.is_some_and(|origin| origin.kind == PropertyMacroKind::Quickcheck) {
        return;
    }
    let already = attrs.iter().any(|attr| {
        attr.chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
            .contains("quickcheck")
    });
    if !already {
        attrs.push("#[quickcheck]".to_string());
    }
}

/// Same-length overlay parse of property-macro token trees.
pub(super) struct PropertyMacroOverlay {
    parse: Parse<SourceFile>,
    origins: Vec<PropertyMacroOrigin>,
}

impl PropertyMacroOverlay {
    /// Build an overlay from the outer parse of `text`. An empty overlay
    /// (no property-macro blocks) still parses, and [`Self::functions`]
    /// yields nothing.
    pub(super) fn from_source(source: &SourceFile, text: &str) -> Self {
        let origins = collect_origins(source);
        let overlay = build_overlay(text, &origins);
        let parse = SourceFile::parse(&overlay, Edition::CURRENT);
        Self { parse, origins }
    }

    /// Inner `fn` items that recovered a body after param-list blanking.
    pub(super) fn functions(&self) -> impl Iterator<Item = (ast::Fn, &PropertyMacroOrigin)> {
        let origins = &self.origins;
        self.parse
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .filter_map(move |function| {
                function.body()?;
                let start = text_size_to_usize(function.syntax().text_range().start());
                let origin = origins
                    .iter()
                    .find(|origin| start >= origin.inner_start && start < origin.inner_end)?;
                Some((function, origin))
            })
    }
}

fn collect_origins(source: &SourceFile) -> Vec<PropertyMacroOrigin> {
    let mut origins = Vec::new();
    for macro_call in source
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        if nested_in_fn_or_token_tree(&macro_call) {
            continue;
        }
        let Some(kind) = property_macro_kind(&macro_call) else {
            continue;
        };
        let Some(tree) = macro_call.token_tree() else {
            continue;
        };
        let range = tree.syntax().text_range();
        let start = text_size_to_usize(range.start());
        let end = text_size_to_usize(range.end());
        if end.saturating_sub(start) < 2 {
            continue;
        }
        origins.push(PropertyMacroOrigin {
            kind,
            module_segments: function_module_segments(macro_call.syntax()),
            inside_cfg_test: is_cfg_test_module_member(macro_call.syntax()),
            inner_start: start + 1,
            inner_end: end - 1,
        });
    }
    origins
}

fn nested_in_fn_or_token_tree(macro_call: &ast::MacroCall) -> bool {
    macro_call.syntax().ancestors().skip(1).any(|ancestor| {
        ast::Fn::can_cast(ancestor.kind()) || ancestor.kind() == SyntaxKind::TOKEN_TREE
    })
}

fn property_macro_kind(macro_call: &ast::MacroCall) -> Option<PropertyMacroKind> {
    let path = macro_call.path()?;
    let compact = path
        .syntax()
        .text()
        .to_string()
        .replace([' ', '\t', '\n'], "");
    let leaf = compact.rsplit("::").next().unwrap_or(compact.as_str());
    match leaf {
        "proptest" => Some(PropertyMacroKind::Proptest),
        "quickcheck" => Some(PropertyMacroKind::Quickcheck),
        _ => None,
    }
}

fn build_overlay(text: &str, origins: &[PropertyMacroOrigin]) -> String {
    let src = text.as_bytes();
    let mut overlay = src.to_vec();
    for byte in &mut overlay {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
    for origin in origins {
        if origin.inner_end > overlay.len() || origin.inner_start > origin.inner_end {
            continue;
        }
        overlay[origin.inner_start..origin.inner_end]
            .copy_from_slice(&src[origin.inner_start..origin.inner_end]);
        if origin.kind == PropertyMacroKind::Proptest {
            blank_proptest_param_lists(&mut overlay, text, origin.inner_start, origin.inner_end);
        }
    }
    String::from_utf8(overlay).unwrap_or_else(|_| {
        text.bytes()
            .map(|byte| if byte == b'\n' { '\n' } else { ' ' })
            .collect()
    })
}

/// Blank `fn name(x in strategy)` interiors so the Rust grammar keeps the
/// body. Newlines stay so line numbers remain real.
fn blank_proptest_param_lists(
    overlay: &mut [u8],
    text: &str,
    inner_start: usize,
    inner_end: usize,
) {
    let Some(inner) = text.get(inner_start..inner_end) else {
        return;
    };
    // Parse the inner text as a source file so we can walk recovered `fn`
    // nodes. Offsets in this parse are inner-relative; strategy params may
    // still make this parse dirty, so we also walk tokens for `in`.
    let parse = SourceFile::parse(inner, Edition::CURRENT);
    let mut interiors = Vec::new();
    token_param_interiors(parse.tree().syntax(), &mut interiors);
    for (rel_start, rel_end) in interiors {
        let start = inner_start.saturating_add(rel_start);
        let end = inner_start.saturating_add(rel_end);
        if start >= end || end > overlay.len() {
            continue;
        }
        for byte in &mut overlay[start..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
}

fn token_param_interiors(node: &ra_ap_syntax::SyntaxNode, interiors: &mut Vec<(usize, usize)>) {
    let tokens = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WHITESPACE
                    | SyntaxKind::COMMENT
                    | SyntaxKind::STRING
                    | SyntaxKind::BYTE_STRING
                    | SyntaxKind::C_STRING
                    | SyntaxKind::CHAR
                    | SyntaxKind::BYTE
            )
        })
        .collect::<Vec<_>>();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].text() != "fn" {
            index += 1;
            continue;
        }
        index += 1;
        if index >= tokens.len() {
            break;
        }
        // Function name.
        index += 1;
        if index < tokens.len() && tokens[index].text() == "<" {
            skip_balanced(&tokens, &mut index, "<", ">");
        }
        if index >= tokens.len() || tokens[index].text() != "(" {
            continue;
        }
        let open = tokens[index].text_range();
        let Some(close_index) = matching_close(&tokens, index, "(", ")") else {
            index += 1;
            continue;
        };
        let saw_in = tokens[index + 1..close_index]
            .iter()
            .any(|token| token.text() == "in");
        if saw_in {
            interiors.push((
                text_size_to_usize(open.end()),
                text_size_to_usize(tokens[close_index].text_range().start()),
            ));
        }
        index = close_index + 1;
    }
}

fn skip_balanced(tokens: &[ra_ap_syntax::SyntaxToken], index: &mut usize, open: &str, close: &str) {
    let mut depth = 0usize;
    while *index < tokens.len() {
        if tokens[*index].text() == open {
            depth = depth.saturating_add(1);
        } else if tokens[*index].text() == close {
            depth = depth.saturating_sub(1);
            *index += 1;
            if depth == 0 {
                return;
            }
            continue;
        }
        *index += 1;
    }
}

fn matching_close(
    tokens: &[ra_ap_syntax::SyntaxToken],
    open_index: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, token) in tokens[open_index..].iter().enumerate() {
        if token.text() == open {
            depth = depth.saturating_add(1);
        } else if token.text() == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(open_index + offset);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::summarize_file_with_parser;
    use super::super::{is_assertion_macro, is_assertion_macro_leaf};
    use crate::analysis::facts::{FileFacts, FunctionFact, FunctionSourceRole, TestFact};
    use crate::domain::OracleStrength;
    use std::path::Path;

    fn facts(source: &str) -> Result<FileFacts, String> {
        summarize_file_with_parser(Path::new("src/lib.rs"), source)
    }

    fn test_names(source: &str) -> Result<Vec<String>, String> {
        Ok(facts(source)?
            .tests
            .iter()
            .map(|test| test.name.clone())
            .collect())
    }

    fn function_names(source: &str) -> Result<Vec<String>, String> {
        Ok(facts(source)?
            .functions
            .iter()
            .map(|function| function.name.clone())
            .collect())
    }

    fn named_test<'a>(file: &'a FileFacts, name: &str) -> Result<&'a TestFact, String> {
        file.tests
            .iter()
            .find(|test| test.name == name)
            .ok_or_else(|| format!("missing test {name}"))
    }

    fn named_function<'a>(file: &'a FileFacts, name: &str) -> Result<&'a FunctionFact, String> {
        file.functions
            .iter()
            .find(|function| function.name == name)
            .ok_or_else(|| format!("missing function {name}"))
    }

    #[test]
    fn assertion_macro_authority_includes_prop_assert_family() {
        for name in ["prop_assert", "prop_assert_eq", "prop_assert_ne"] {
            assert!(is_assertion_macro(name), "{name}");
            assert!(is_assertion_macro_leaf(name), "{name}");
            assert!(
                is_assertion_macro(&format!("proptest::{name}")),
                "qualified {name}"
            );
        }
        assert!(is_assertion_macro("assert_eq"));
        assert!(!is_assertion_macro("proptest"));
        assert!(!is_assertion_macro_leaf("snapshot_helper"));
    }

    #[test]
    fn proptest_test_fn_keeps_real_lines_owner_call_and_prop_assert_eq_oracle() -> Result<(), String>
    {
        let source = "pub fn gate(x: u32) -> bool {\n    x > 10\n}\n\nproptest! {\n    #[test]\n    fn gate_threshold(x in 0u32..100) {\n        prop_assert_eq!(gate(x), x > 10);\n    }\n}\n";
        let file = facts(source)?;
        let test = named_test(&file, "gate_threshold")?;
        if test.start_line != 7 {
            return Err(format!("fn keyword line must stay real: {test:?}"));
        }
        if !test.calls.iter().any(|call| call.name == "gate") {
            return Err(format!("owner call must be indexed: {:?}", test.calls));
        }
        if !test.assertions.iter().any(|oracle| {
            oracle.text.contains("prop_assert_eq!") && oracle.strength == OracleStrength::Strong
        }) {
            return Err(format!(
                "prop_assert_eq! must be a parser oracle: {:?}",
                test.assertions
            ));
        }
        if !test.attrs.iter().any(|attr| attr.contains("#[test]")) {
            return Err(format!(
                "#[test] must remain on the test fact: {:?}",
                test.attrs
            ));
        }
        Ok(())
    }

    #[test]
    fn unmarked_proptest_fn_is_a_function_fact_not_a_test() -> Result<(), String> {
        let source = "pub fn gate(x: u32) -> bool { x > 10 }\n\nproptest! {\n    fn unmarked(x in 0u32..100) {\n        prop_assert!(gate(x));\n    }\n}\n";
        let file = facts(source)?;
        if file.tests.iter().any(|test| test.name == "unmarked") {
            return Err(format!(
                "unmarked proptest fn must not be a test: {:?}",
                file.tests
            ));
        }
        let helper = named_function(&file, "unmarked")?;
        if helper.source_role != FunctionSourceRole::Production {
            return Err(format!("unmarked role {:?}", helper.source_role));
        }
        if !helper.calls.iter().any(|call| call.name == "gate") {
            return Err(format!(
                "unmarked fn still records the owner call: {:?}",
                helper.calls
            ));
        }
        Ok(())
    }

    #[test]
    fn quickcheck_fn_is_a_test_with_recorded_quickcheck_attr() -> Result<(), String> {
        let source = "pub fn gate(x: u32) -> bool { x > 10 }\n\nquickcheck! {\n    fn qc_gate(x: u32) -> bool {\n        gate(x) == (x > 10)\n    }\n}\n";
        let file = facts(source)?;
        let test = named_test(&file, "qc_gate")?;
        if !test.attrs.iter().any(|attr| attr.contains("quickcheck")) {
            return Err(format!("#[quickcheck] must be recorded: {:?}", test.attrs));
        }
        if !test.calls.iter().any(|call| call.name == "gate") {
            return Err(format!(
                "quickcheck body must keep the owner call: {:?}",
                test.calls
            ));
        }
        if test.start_line != 4 {
            return Err(format!("quickcheck line {}", test.start_line));
        }
        Ok(())
    }

    #[test]
    fn spelled_quickcheck_attr_is_not_duplicated() -> Result<(), String> {
        let source = "quickcheck! {\n    #[quickcheck]\n    fn already_marked(x: u32) -> bool { x == x }\n}\n";
        let file = facts(source)?;
        let test = named_test(&file, "already_marked")?;
        let count = test
            .attrs
            .iter()
            .filter(|attr| {
                attr.chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect::<String>()
                    .contains("quickcheck")
            })
            .count();
        if count != 1 {
            return Err(format!(
                "spelled #[quickcheck] must not be duplicated: {:?}",
                test.attrs
            ));
        }
        Ok(())
    }

    #[test]
    fn quickcheck_fn_with_prop_assert_eq_keeps_the_oracle() -> Result<(), String> {
        let source = "pub fn gate(x: u32) -> bool { x > 10 }\n\nquickcheck! {\n    fn qc_assert(x: u32) {\n        prop_assert_eq!(gate(x), x > 10);\n    }\n}\n";
        let file = facts(source)?;
        let test = named_test(&file, "qc_assert")?;
        if !test
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("prop_assert_eq!"))
        {
            return Err(format!("{:?}", test.assertions));
        }
        Ok(())
    }

    #[test]
    fn mixed_proptest_block_indexes_only_the_marked_test() -> Result<(), String> {
        let source = "proptest! {\n    #[test]\n    fn marked(x in 0u32..4) { prop_assert_eq!(gate(x), x > 10); }\n    fn helper(x in 0u32..4) { let _ = gate(x); }\n}\nfn gate(x: u32) -> bool { x > 10 }\n";
        if test_names(source)? != vec!["marked".to_string()] {
            return Err(format!("marked tests: {:?}", test_names(source)?));
        }
        let names = function_names(source)?;
        if !names.contains(&"marked".to_string())
            || !names.contains(&"helper".to_string())
            || !names.contains(&"gate".to_string())
        {
            return Err(format!("function set {names:?}"));
        }
        Ok(())
    }

    #[test]
    fn lookalike_and_commented_macros_are_not_expanded() -> Result<(), String> {
        let source = r#"
fn owner() {}
// proptest! { #[test] fn from_comment(x in 0u32..1) { prop_assert_eq!(owner(), true); } }
const SAMPLE: &str = "proptest! { #[test] fn from_string(x in 0u32..1) { prop_assert_eq!(owner(), true); } }";
my_proptest! {
    #[test]
    fn from_lookalike(x in 0u32..1) {
        prop_assert_eq!(owner(), true);
    }
}
"#;
        let names = test_names(source)?;
        if !names.is_empty() {
            return Err(format!(
                "comments, strings, and lookalike macros must not mint tests: {names:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn nested_macro_inside_a_function_body_stays_invisible() -> Result<(), String> {
        let source = "fn outer() {\n    proptest! {\n        #[test]\n        fn inner(x in 0u32..1) { prop_assert_eq!(x, x); }\n    }\n}\n";
        let names = test_names(source)?;
        if !names.is_empty() {
            return Err(format!(
                "fn-nested proptest! must not be expanded: {names:?}"
            ));
        }
        if function_names(source)? != vec!["outer".to_string()] {
            return Err(format!("outer only: {:?}", function_names(source)?));
        }
        Ok(())
    }

    #[test]
    fn empty_and_paren_forms_do_not_crash() -> Result<(), String> {
        let empty = facts("proptest! {}\nquickcheck! {}\n")?;
        if !empty.tests.is_empty() {
            return Err(format!("empty macros minted tests: {:?}", empty.tests));
        }
        let paren = facts(
            "pub fn gate(x: u32) -> bool { x > 10 }\nproptest!(\n    #[test]\n    fn paren_case(x in 0u32..8) {\n        prop_assert_ne!(gate(x), x < 10);\n    }\n);\n",
        )?;
        let test = named_test(&paren, "paren_case")?;
        if !test.calls.iter().any(|call| call.name == "gate") {
            return Err("paren-form owner call missing".to_string());
        }
        if !test
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("prop_assert_ne!"))
        {
            return Err("paren-form prop_assert_ne missing".to_string());
        }
        Ok(())
    }

    #[test]
    fn nested_strategy_generics_still_recover_the_body() -> Result<(), String> {
        let source = "proptest! {\n    #[test]\n    fn nested_strategy(xs in any::<Vec<(u32, u32)>>()) {\n        prop_assert!(gate(xs.len() as u32) || xs.is_empty());\n    }\n}\nfn gate(x: u32) -> bool { x > 10 }\n";
        let file = facts(source)?;
        let test = named_test(&file, "nested_strategy")?;
        if !test
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("prop_assert!"))
        {
            return Err(format!("{:?}", test.assertions));
        }
        if !test.calls.iter().any(|call| call.name == "gate") {
            return Err("nested strategy lost the owner call".to_string());
        }
        Ok(())
    }

    #[test]
    fn qualified_paths_and_cfg_test_module_keep_identity() -> Result<(), String> {
        let source = "#[cfg(test)]\nmod tests {\n    proptest::proptest! {\n        #[test]\n        fn in_mod(x in 0u32..3) { prop_assert_eq!(crate::gate(x), x > 10); }\n    }\n    quickcheck::quickcheck! {\n        fn qc_in_mod(x: u32) -> bool { crate::gate(x) }\n    }\n}\npub fn gate(x: u32) -> bool { x > 10 }\n";
        let file = facts(source)?;
        let proptest = named_function(&file, "in_mod")?;
        if !proptest.id.0.contains("tests::in_mod") {
            return Err(format!(
                "overlay must keep outer module identity: {}",
                proptest.id.0
            ));
        }
        let quickcheck = named_function(&file, "qc_in_mod")?;
        if quickcheck.source_role != FunctionSourceRole::TestAttribute {
            return Err(format!("quickcheck role {:?}", quickcheck.source_role));
        }
        if !quickcheck.id.0.contains("tests::qc_in_mod") {
            return Err(format!("quickcheck id {}", quickcheck.id.0));
        }
        Ok(())
    }

    #[test]
    fn unmarked_proptest_fn_inside_cfg_test_keeps_helper_role() -> Result<(), String> {
        let source = "#[cfg(test)]\nmod tests {\n    proptest! {\n        fn helper(x in 0u32..2) { let _ = crate::gate(x); }\n    }\n}\npub fn gate(x: u32) -> bool { x > 10 }\n";
        let file = facts(source)?;
        let helper = named_function(&file, "helper")?;
        if helper.source_role != FunctionSourceRole::CfgTestModule {
            return Err(format!("cfg(test) helper role {:?}", helper.source_role));
        }
        if file.tests.iter().any(|test| test.name == "helper") {
            return Err("cfg(test) unmarked helper must not be a test".to_string());
        }
        Ok(())
    }

    #[test]
    fn oracle_without_owner_call_does_not_invent_a_gate_call() -> Result<(), String> {
        let source = "pub fn gate(x: u32) -> bool { x > 10 }\n\nproptest! {\n    #[test]\n    fn constants_only(x in 0u32..1) {\n        prop_assert_eq!(1u32, 1u32);\n    }\n}\n";
        let file = facts(source)?;
        let test = named_test(&file, "constants_only")?;
        if test.calls.iter().any(|call| call.name == "gate") {
            return Err(format!(
                "no owner call in the block must not mint one: {:?}",
                test.calls
            ));
        }
        if !test
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("prop_assert_eq!"))
        {
            return Err("constants-only oracle missing".to_string());
        }
        Ok(())
    }

    #[test]
    fn ordinary_outer_tests_are_not_doubled() -> Result<(), String> {
        let source = "#[test]\nfn ordinary() { assert_eq!(gate(11), true); }\nfn gate(x: u32) -> bool { x > 10 }\n";
        if test_names(source)? != vec!["ordinary".to_string()] {
            return Err(format!("ordinary tests: {:?}", test_names(source)?));
        }
        Ok(())
    }
}
