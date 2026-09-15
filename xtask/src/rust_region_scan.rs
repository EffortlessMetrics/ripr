//! Parser-backed production-text regions for the Rust source-role
//! authority gate (#3631).
//!
//! The gate scans the production text of every Rust file under
//! `crates/ripr/src`: the whole file minus the interiors of its top-level
//! `#[cfg(test)]` items, so test fixtures and assertions that manipulate
//! source text as data never reach the role-authority pattern scans. This
//! module derives those regions from a real syntax tree
//! (`ra_ap_syntax`, edition 2024) instead of the retired hand-written
//! depth-tracking scanner, which eliminates by construction the scanner's
//! documented edge cases (#3631) plus the fragility family they belong
//! to: under-scans past `else if` initializer chains, over-caught
//! multi-line attributes and doc comments preceding a gated item, braced
//! const-generic arguments confusing depth-0 item detection, and lexer
//! overruns on escaped character literals that silently leaked whole
//! cfg-test modules into production scans (found by the #3631
//! differential run; the retired scanner failed to exempt even its own
//! test module in `xtask/src/main.rs`).
//!
//! Fail-closed direction: a file that does not parse cleanly is scanned
//! verbatim (its cfg-test interiors included) and the fallback is
//! disclosed to the caller, so an unparseable file can only over-report
//! violations — never silently skip them. The retired scanner is not
//! kept as the fallback: an unreachable 600-line lexer with no live
//! callers would be exactly the false-confidence surface this rework
//! removes, and the verbatim fallback's over-catch direction is the safe
//! one for a policy gate.

use ra_ap_syntax::ast::{self, HasAttrs};
use ra_ap_syntax::{AstNode, Edition, SourceFile};

/// Production-region result for one Rust source file.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProductionRegions {
    /// Production text: everything except the interiors of top-level
    /// test-required items (including their attached doc comments and
    /// attribute run), with a newline seam after each removed region so
    /// patterns cannot splice across it.
    pub(crate) text: String,
    /// True when the file did not parse cleanly and `text` is the
    /// verbatim file instead of the parsed complement. Callers disclose
    /// this in the gate report.
    pub(crate) used_verbatim_fallback: bool,
}

/// Production text of one Rust source file, region-aware: parse the file
/// with the real grammar, exempt each top-level item whose attribute run
/// carries `#[cfg(test)]` (the item's full node range, which starts at
/// its first attached doc comment or attribute), and return the
/// complement. Plain-substring pattern matching over the returned text
/// keeps producer exemptions and allowed-site inventories behaving
/// exactly as before.
pub(crate) fn production_text_regions(source: &str) -> ProductionRegions {
    let parse = SourceFile::parse(source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        // Over-catch direction: without a tree the interiors of test
        // items stay in the scan, which can only over-report violations.
        return ProductionRegions {
            text: source.to_string(),
            used_verbatim_fallback: true,
        };
    }
    let mut exempt: Vec<(usize, usize)> = Vec::new();
    for item in parse.tree().syntax().children().filter_map(ast::Item::cast) {
        let gated = item
            .attrs()
            .any(|attr| attribute_text_requires_test(&attr.syntax().text().to_string()));
        if !gated {
            continue;
        }
        let range = item.syntax().text_range();
        exempt.push((usize::from(range.start()), usize::from(range.end())));
    }
    exempt.sort_unstable();

    let mut text = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end) in exempt {
        if start < cursor || end > source.len() {
            continue;
        }
        text.push_str(&source[cursor..start]);
        // Newline seam: patterns cannot splice across a removed region
        // (the retired scanner kept the same guarantee).
        text.push('\n');
        cursor = end;
    }
    text.push_str(&source[cursor..]);
    ProductionRegions {
        text,
        used_verbatim_fallback: false,
    }
}

/// True when `attr_text` is the exact test gate the retired scanner
/// honored: `#[cfg(test)]`, with comments and whitespace allowed between
/// the tokens (`#[cfg(/* test-only */ test)]` is a gated item). The
/// inner-attribute envelope `#![cfg(test)]` classifies the same way as in
/// the product authority, though it is unreachable from the region walk:
/// inner attributes attach to the source file, never to a top-level item.
/// The wider cfg-predicate families (`all`/`any`/`not`, `cfg_attr`) stay
/// in the product authority `analysis::facts::cfg_predicates`; re-hosting
/// only the honored spelling keeps the exemption inventory identical to
/// the gate's prior behavior (a wider predicate would exempt item
/// interiors the gate has always scanned).
fn attribute_text_requires_test(attr_text: &str) -> bool {
    let mut tokens = significant_tokens(attr_text);
    // Normalize `#![...]` to the outer shape before the spelling table.
    if tokens.get(1).map(String::as_str) == Some("!") {
        tokens.remove(1);
    }
    tokens == CFG_TEST_SPELLING
}

/// The exact token sequence of the honored spelling.
const CFG_TEST_SPELLING: [&str; 7] = ["#", "[", "cfg", "(", "test", ")", "]"];

/// Splits attribute text into significant tokens: identifier runs and
/// single punctuation characters, with whitespace and (nestable) comments
/// skipped. Built for `cfg(...)` predicate bodies, which contain no
/// string literals, so no literal handling is needed.
fn significant_tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            let mut depth = 1usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        if chars[i].is_alphanumeric() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
            continue;
        }
        tokens.push(chars[i].to_string());
        i += 1;
    }
    tokens
}

#[cfg(test)]
mod production_text_regions_tests {
    use super::production_text_regions;

    fn scanned(text: &str) -> String {
        let regions = production_text_regions(text);
        assert!(
            !regions.used_verbatim_fallback,
            "test fixture must parse cleanly"
        );
        regions.text
    }

    #[test]
    fn production_only_file_is_returned_verbatim() {
        let text = "pub fn a(path: &str) -> bool {\n    path.contains(\"/tests/\")\n}\n";
        assert_eq!(scanned(text), text);
    }

    #[test]
    fn production_after_a_leading_test_module_is_scanned() {
        // The #3534 blind-spot shape: the gated item sits first, so the
        // earlier first-item truncation hid everything below it.
        let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        let _ = path.contains(\"/tests/\");\n    }\n}\n\npub fn later(path: &str) -> bool {\n    path.contains(\"/tests/\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(scanned.contains("path.contains(\"/tests/\")"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn production_before_a_test_module_stays_scanned() {
        let text = "pub fn early(path: &str) -> bool {\n    path.ends_with(\"_test.rs\")\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() {\n        let _ = path.ends_with(\"_test.rs\");\n    }\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn early"));
        assert!(scanned.contains("path.ends_with(\"_test.rs\")"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn gated_declaration_exempts_only_the_declaration() {
        // `analysis/mod.rs` shape: a gated `mod name;` declaration with
        // production module declarations after it.
        let text = "mod facts;\n#[cfg(test)]\nmod source_role_corpus;\nmod summary;\npub fn rest(path: &str) -> bool {\n    path.contains(\"/tests/\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("mod facts;"));
        assert!(scanned.contains("mod summary;"));
        assert!(scanned.contains("path.contains(\"/tests/\")"));
        assert!(!scanned.contains("source_role_corpus"));
    }

    #[test]
    fn gated_free_function_is_exempt_in_full() {
        // The gated helper's body is the only `/tests/` occurrence, so a
        // leak of that body would surface as the denied spelling here.
        let text = "pub fn early() -> bool { true }\n\n#[cfg(test)]\nfn gated() {\n    let secret = path.contains(\"/tests/\");\n}\n\npub fn last(path: &str) -> bool {\n    path.ends_with(\"_test.rs\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn early"));
        assert!(scanned.contains("pub fn last"));
        assert!(scanned.contains("path.ends_with(\"_test.rs\")"));
        assert!(!scanned.contains("fn gated"));
        assert!(!scanned.contains("/tests/"));
    }

    #[test]
    fn gated_function_signature_parens_do_not_end_the_item() {
        // The item range runs to the body's closing brace, so the body
        // stays exempt regardless of brackets in the signature.
        let text = "#[cfg(test)]\nfn gated(x: [u8; 2]) {\n    let secret = path.contains(\"/tests/\");\n}\n\npub fn last() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn last"));
        assert!(!scanned.contains("fn gated"));
        assert!(!scanned.contains("/tests/"));
    }

    #[test]
    fn cfg_test_inside_a_production_array_stays_in_production_text() {
        // Only top-level gated items are exempt; an attributed element
        // inside a module-level array initializer is not a top-level item
        // and its text stays in the scan.
        let text = "pub const GATED: [&str; 1] = &[\n    #[cfg(test)]\n    \"inside\",\n];\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("inside"));
        assert!(scanned.contains("pub fn later"));
    }

    #[test]
    fn production_between_two_gated_items_is_scanned() {
        let text = "#[cfg(test)]\nmod a { fn ta() {} }\n\npub fn mid() -> bool { true }\n\n#[cfg(test)]\nmod b { fn tb() {} }\n\npub fn last() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn mid"));
        assert!(scanned.contains("pub fn last"));
        assert!(!scanned.contains("fn ta"));
        assert!(!scanned.contains("fn tb"));
    }

    #[test]
    fn nested_braces_inside_a_test_module_stay_exempt() {
        let text = "#[cfg(test)]\nmod tests {\n    mod inner {\n        fn t() {\n            if x {\n                let _ = path.starts_with(\"tests\");\n            }\n        }\n    }\n}\n\npub fn later(path: &str) -> bool {\n    path.starts_with(\"tests\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(scanned.contains("path.starts_with(\"tests\")"));
        assert!(!scanned.contains("fn t()"));
        assert!(!scanned.contains("mod inner"));
    }

    #[test]
    fn brace_strings_inside_a_test_module_do_not_end_the_exemption() {
        let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        let braces = \"}{{{\";\n    }\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn brace_strings_in_production_do_not_shift_depth() {
        let text = "pub fn render() -> &'static str {\n    \"}{\"\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = \"x\"; }\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn render"));
        assert!(scanned.contains("pub fn later"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn raw_strings_with_hashes_keep_depth() {
        let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        let raw = r#\" { } \"#;\n        let raw2 = r\" } \";\n    }\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn byte_and_c_raw_strings_with_hashes_keep_depth() {
        let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        let a = br#\" { } \"#;\n        let b = cr#\" { } \"#;\n        let c = br#\" q \" z { \"#;\n        let d = b\"{ }\";\n        let e = c\"{ }\";\n    }\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn char_literals_and_lifetimes_keep_depth() {
        let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        let _ = ('{', '}');\n    }\n}\n\npub fn later(path: &'static str) -> char {\n    '{'\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(scanned.contains("path: &'static str"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn cfg_test_spelling_in_comments_or_strings_does_not_exempt() {
        let text = "pub fn doc() -> bool {\n    // a line mentioning #[cfg(test)]\n    /* block #[cfg(test)] comment */\n    let s = \"#[cfg(test)]\";\n    true\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn doc"));
        assert!(scanned.contains("pub fn later"));
    }

    #[test]
    fn indented_cfg_test_items_stay_in_production_text() {
        // Only top-level gated items are exempt; a nested cfg(test) item
        // inside a production function body stays in the scanned text,
        // matching the earlier first-item boundary policy.
        let text =
            "pub fn wrapper() {\n    #[cfg(test)]\n    mod inner {\n        fn t() {}\n    }\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("mod inner"));
    }

    #[test]
    fn gated_items_with_extra_attributes_stay_exempt() {
        let text = "pub fn early() -> bool { true }\n\n#[cfg(test)]\n#[expect(clippy::duck)]\nmod tests {\n    fn t() {\n        let _ = path.contains(\"/tests/\");\n    }\n}\n\npub fn last(path: &str) -> bool {\n    path.contains(\"/tests/\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn early"));
        assert!(scanned.contains("pub fn last"));
        assert!(!scanned.contains("fn t()"));
    }

    #[test]
    fn test_module_body_strings_do_not_leak_into_production_text() {
        // Repo-shaped case: test code legitimately quotes test-path
        // strings; those must stay inside the exempt region.
        let text = "pub fn early() -> bool { true }\n\n#[cfg(test)]\nmod tests {\n    fn t() {\n        assert!(fixture.ends_with(\"_test.rs\"));\n        assert!(fixture.starts_with(\"tests\"));\n    }\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn early"));
        assert!(!scanned.contains("_test.rs"));
        assert!(!scanned.contains("starts_with(\"tests"));
    }

    #[test]
    fn preceding_doc_attribute_is_removed_with_the_gated_item() {
        // A doc attribute carrying a denied pattern before a gated module
        // belongs to the exempt item, not production text.
        let text = "#[doc = r#\"path.contains(\"/tests/\")\"# ]
#[cfg(test)]
mod tests {
}

pub fn later() {}
";
        let scanned = scanned(text);
        assert!(!scanned.contains("/tests/"), "{scanned}");
        assert!(scanned.contains("pub fn later()"));
    }

    #[test]
    fn multiline_attribute_and_block_doc_are_exempt_with_the_item() {
        // #3631 edge case 2: the retired scanner trimmed attribute/doc
        // runs line by line, so multi-line attributes and block docs
        // preceding a gated item leaked into production scans. The item
        // node starts at its first attached doc comment, so the parser
        // exempts the whole attached run.
        let text = "/** block doc\n * with path.contains(\"/tests/\") inside */\n#[doc = \"multi\nline with contains(\\\"/tests/\\\") too\"]\n#[cfg(test)]\nmod tests {\n}\n\npub fn later() {}\n";
        let scanned = scanned(text);
        assert!(!scanned.contains("/tests/"), "{scanned}");
        assert!(scanned.contains("pub fn later()"));
    }

    #[test]
    fn inner_file_doc_stays_in_production_text() {
        // `//!` docs attach to the source file, not to an item; they stay
        // in production text exactly as before.
        let text = "//! crate docs mentioning #[cfg(test)]\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("crate docs"));
        assert!(scanned.contains("pub fn later"));
    }

    #[test]
    fn escaped_char_literal_before_a_gated_item_does_not_leak_the_module() {
        // Found by the #3631 differential run: the retired scanner's
        // char-literal ender overshot `'\\'`, swallowed the following
        // source, and leaked whole cfg-test modules into production scans.
        let text = "pub fn normalize(id: &str) -> String {\n    id.replace('\\\\', \"/\")\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn normalize"));
        assert!(!scanned.contains("fn t()"));
        assert!(scanned.contains("pub fn later"));
    }

    #[test]
    fn else_if_initializer_chain_does_not_bail_to_end_of_file() {
        // #3631 edge case 1: the retired scanner bailed to end-of-file on
        // `else if` chains in a gated initializer, under-scanning every
        // later production region in the file. The item node ends at its
        // own `;`.
        let text = "#[cfg(test)]\nconst X: &str = if true {\n    \"\"\n} else if false {\n    r#\"path.contains(\"/tests/\")\"#\n} else {\n    \"\"\n};\n\npub fn later() -> bool { true }\n";
        let scanned = scanned(text);
        assert!(!scanned.contains("/tests/"), "{scanned}");
        assert!(scanned.contains("pub fn later"));
        assert!(!scanned.contains("const X"), "{scanned}");
    }

    #[test]
    fn braced_const_generic_default_does_not_confuse_item_detection() {
        // #3631 edge case 3: brace depth inside generic argument lists
        // (`= { 1 + 1 }`) confused the retired depth-0 item detection.
        let text = "#[cfg(test)]\nstruct Wrapper<const N: usize = { 1 + 1 }> {\n    value: [u8; N],\n}\n\npub fn later(path: &str) -> bool {\n    path.contains(\"/tests/\")\n}\n";
        let scanned = scanned(text);
        assert!(scanned.contains("pub fn later"));
        assert!(scanned.contains("path.contains(\"/tests/\")"));
        assert!(!scanned.contains("Wrapper"), "{scanned}");
    }

    #[test]
    fn newline_seam_prevents_pattern_splicing_across_a_removed_item() {
        // Same guarantee the retired scanner carried: after a removed
        // region the output carries a newline seam, so text cannot join
        // directly across it even when the source packs items onto one
        // line.
        let text = "fn a() {}#[cfg(test)] mod tests {}fn b() {}\n";
        let scanned = scanned(text);
        assert_eq!(scanned, "fn a() {}\nfn b() {}\n");
    }

    #[test]
    fn unparseable_file_falls_back_to_verbatim_scan() {
        let text = "pub fn broken( {\n#[cfg(test)]\nmod tests {\n";
        let regions = production_text_regions(text);
        assert!(regions.used_verbatim_fallback);
        assert_eq!(regions.text, text);
    }
}

#[cfg(test)]
mod cfg_test_spelling_tests {
    use super::attribute_text_requires_test;

    #[test]
    fn honored_spellings_require_test() {
        for spelling in [
            "#[cfg(test)]",
            "#[cfg( test )]",
            "#[\ncfg(\n test )\n]",
            "#[cfg(/* test-only */ test)]",
            "#![cfg(test)]",
        ] {
            assert!(
                attribute_text_requires_test(spelling),
                "{spelling} must gate on test"
            );
        }
    }

    #[test]
    fn lookalike_spellings_do_not_require_test() {
        for spelling in [
            "#[cfg(test_support)]",
            "#[cfg(tests)]",
            "#[cfg(all(test))]",
            "#[cfg(unix)]",
            "#[cfg(feature = \"test\")]",
            "#[cfg_attr(test, allow(dead_code))]",
            "#[allow(test)]",
            "#[expect(clippy::duck)]",
            "#[doc = \"cfg(test)\"]",
            "/// cfg(test)",
            "#[cfg]",
            "#[cfg(test",
        ] {
            assert!(
                !attribute_text_requires_test(spelling),
                "{spelling} must not gate on test"
            );
        }
    }
}
