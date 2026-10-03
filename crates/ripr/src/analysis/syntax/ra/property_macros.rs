//! Property-macro syntax is a limitation witness, never executable evidence.
//!
//! The name alone cannot establish a macro's expansion: local no-op macros
//! can use the same spelling as proptest and QuickCheck. Keep their bodies
//! opaque and collect only lexical identifier mentions from the existing
//! parse. No file-sized overlay, second parse, functions or tests are created.

use super::LineIndex;
use crate::analysis::facts::UnresolvedPropertyMacroFact;
use ra_ap_syntax::{AstNode, SourceFile, SyntaxKind, ast};
use std::collections::BTreeSet;

pub(super) fn unresolved_property_macros(
    source: &SourceFile,
    lines: &LineIndex,
) -> Vec<UnresolvedPropertyMacroFact> {
    source
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter_map(|call| {
            // A template inside another opaque token tree is not an invocation.
            if call
                .syntax()
                .ancestors()
                .skip(1)
                .any(|node| node.kind() == SyntaxKind::TOKEN_TREE)
            {
                return None;
            }
            let path = call.path()?;
            let leaf = path.segment()?.name_ref()?;
            if !matches!(
                leaf.text(),
                "proptest" | "quickcheck" | "prop_assert" | "prop_assert_eq" | "prop_assert_ne"
            ) {
                return None;
            }
            let name = path
                .syntax()
                .text()
                .to_string()
                .replace([' ', '\t', '\n'], "");
            let tree = call.token_tree()?;
            let line = lines.line(call.syntax().text_range().start());
            // Read identifier tokens already owned by the first parse. Do not
            // clone the token-tree source or construct affirmative CallFacts:
            // repeated calls on one long line would duplicate that whole line.
            let mentioned_identifiers = tree
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| token.kind() == SyntaxKind::IDENT)
                .map(|token| token.text().trim_start_matches("r#").to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            Some(UnresolvedPropertyMacroFact {
                name,
                line,
                mentioned_identifiers,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{is_assertion_macro, is_assertion_macro_leaf, summarize_file_with_parser};
    use std::path::Path;

    #[test]
    fn property_macro_names_do_not_grant_assertion_authority() {
        for name in ["prop_assert", "prop_assert_eq", "prop_assert_ne"] {
            assert!(!is_assertion_macro(name));
            assert!(!is_assertion_macro_leaf(name));
            assert!(!is_assertion_macro(&format!("proptest::{name}")));
        }
        assert!(is_assertion_macro("assert_eq"));
        assert!(!is_assertion_macro("other::assert_eq"));
    }

    #[test]
    fn opaque_property_blocks_keep_mentions_without_inventing_tests() -> Result<(), String> {
        for macro_name in ["proptest", "quickcheck", "other::proptest"] {
            for attribute in [
                "#[test]",
                "#[doc = \"quickcheck\"]",
                "#[cfg(feature = \"quickcheck\")]",
            ] {
                let source = format!(
                    "fn gate(x: u32) -> bool {{ x > 10 }}\n{macro_name}! {{\n {attribute}\n fn hidden(x: u32) -> bool {{ gate(x) }}\n}}\n"
                );
                let facts = summarize_file_with_parser(Path::new("src/lib.rs"), &source)?;
                assert_eq!(facts.functions.len(), 1);
                assert_eq!(facts.functions[0].name, "gate");
                assert!(facts.tests.is_empty());
                assert_eq!(facts.unresolved_property_macros.len(), 1);
                let witness = &facts.unresolved_property_macros[0];
                assert_eq!(witness.name, macro_name);
                assert_eq!(witness.line, 2);
                assert!(
                    witness
                        .mentioned_identifiers
                        .iter()
                        .any(|call| call == "gate")
                );
            }
        }
        Ok(())
    }

    #[test]
    fn opaque_mentions_ignore_comments_strings_and_identifier_substrings() -> Result<(), String> {
        let source = r#"proptest! {
            #[test] fn hidden() {
                let sample = "gate(10)"; // gate(20)
                /* gate(30) */
                other::gate_suffix(40);
            }
        }"#;
        let facts = summarize_file_with_parser(Path::new("src/lib.rs"), source)?;
        assert_eq!(facts.unresolved_property_macros.len(), 1);
        let identifiers = &facts.unresolved_property_macros[0].mentioned_identifiers;
        assert!(!identifiers.iter().any(|name| name == "gate"));
        assert!(identifiers.iter().any(|name| name == "gate_suffix"));
        assert!(facts.tests.is_empty());
        Ok(())
    }

    #[test]
    fn ordinary_files_and_macro_lookalikes_have_no_property_witness() -> Result<(), String> {
        for source in [
            "#[test] fn ordinary() { assert_eq!(1, 1); }",
            "// proptest! { fn fake() { gate(1); } }\nfn gate(x:u32)->u32{x}",
            "const TEXT: &str = \"quickcheck! { fn fake() { gate(1); } }\";",
            "my_proptest! { #[test] fn fake() { gate(1); } }",
            "macro_rules! wrapper { () => { proptest! { #[test] fn fake() { gate(1); } } } }",
        ] {
            let facts = summarize_file_with_parser(Path::new("src/lib.rs"), source)?;
            assert!(facts.unresolved_property_macros.is_empty());
        }
        Ok(())
    }
}
