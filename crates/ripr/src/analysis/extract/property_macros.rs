//! Deny-only lexical boundaries for unresolved property macros.
//!
//! Existing Rust literal/comment boundaries are borrowed. These ranges establish no macro
//! identity or execution semantics; they only prevent opaque tokens from
//! becoming executable calls/functions on parser and fallback paths.

use std::ops::Range;

pub(crate) struct OpaquePropertyMacro<'a> {
    pub(crate) name: &'a str,
    pub(crate) range: Range<usize>,
    pub(crate) body_start: usize,
}

pub(crate) fn may_have_property_macro(text: &str) -> bool {
    text.contains("prop_assert") || text.contains("proptest") || text.contains("quickcheck")
}

/// Find recognized leaf spellings without copying or reparsing source.
/// Unclosed or mismatched delimiters quarantine the remaining suffix.
pub(crate) fn opaque_property_macros(source: &str) -> Vec<OpaquePropertyMacro<'_>> {
    if !may_have_property_macro(source) {
        return Vec::new();
    }
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(end) = crate::analysis::syntax::non_code_token_end(source, cursor) {
            cursor = end;
            continue;
        }
        if !identifier_byte(bytes[cursor]) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && identifier_byte(bytes[cursor]) {
            cursor += 1;
        }
        let name = &source[start..cursor];
        if !matches!(
            name,
            "proptest" | "quickcheck" | "prop_assert" | "prop_assert_eq" | "prop_assert_ne"
        ) {
            continue;
        }
        let bang = skip_trivia(source, cursor);
        if bytes.get(bang) != Some(&b'!') {
            continue;
        }
        let open = skip_trivia(source, bang + 1);
        let Some(&delimiter) = bytes.get(open) else {
            continue;
        };
        if !matches!(delimiter, b'(' | b'[' | b'{') {
            continue;
        }
        // Match delimiters iteratively. A malformed or over-budget tree
        // remains opaque through EOF; it never exposes a later fake test.
        let mut stack = Vec::new();
        let mut end = bytes.len();
        let mut offset = open;
        while offset < bytes.len() {
            if let Some(after) = crate::analysis::syntax::non_code_token_end(source, offset) {
                offset = after;
                continue;
            }
            match bytes[offset] {
                b'(' => stack.push(b')'),
                b'[' => stack.push(b']'),
                b'{' => stack.push(b'}'),
                b')' | b']' | b'}' => {
                    if stack.pop() != Some(bytes[offset]) {
                        break;
                    }
                    if stack.is_empty() {
                        end = offset + 1;
                        break;
                    }
                }
                _ => {}
            }
            if stack.len() > 256 {
                break;
            }
            offset += 1;
        }
        result.push(OpaquePropertyMacro {
            name,
            range: start..end,
            body_start: open + 1,
        });
        cursor = end;
    }
    result
}

/// Borrow the source segments outside opaque token trees, preserving offsets.
pub(crate) fn outside_property_macros<'a>(
    text: &'a str,
    macros: &'a [OpaquePropertyMacro<'_>],
) -> impl Iterator<Item = (usize, &'a str)> {
    let mut start = 0;
    macros
        .iter()
        .map(move |item| item.range.clone())
        .chain(std::iter::once(text.len()..text.len()))
        .map(move |range| {
            let part = (start, &text[start..range.start]);
            start = range.end;
            part
        })
}

/// Identifier mentions are diagnostic only. Literal/comment text is excluded.
pub(crate) fn mentioned_identifiers(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut identifiers = std::collections::BTreeSet::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(end) = crate::analysis::syntax::non_code_token_end(text, cursor) {
            cursor = end;
            continue;
        }
        if !identifier_byte(bytes[cursor]) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && identifier_byte(bytes[cursor]) {
            cursor += 1;
        }
        identifiers.insert(text[start..cursor].to_string());
    }
    identifiers.into_iter().collect()
}

/// Call-shaped identifiers outside comments/literals. This is deny-only input:
/// its names cannot create a call fact, relation, or assertion oracle.
pub(crate) fn lexical_call_names(text: &str) -> std::collections::BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut names = std::collections::BTreeSet::new();
    let mut cursor = 0;
    let mut previous_word = "";
    let mut brace_depth = 0usize;
    let mut enum_depth = None;
    let mut pending_enum = false;
    while cursor < bytes.len() {
        if let Some(end) = crate::analysis::syntax::non_code_token_end(text, cursor) {
            cursor = end;
            continue;
        }
        if !identifier_byte(bytes[cursor]) {
            if matches!(bytes[cursor], b';' | b'=' | b'{' | b'}') {
                previous_word = "";
            }
            match bytes[cursor] {
                b'{' => {
                    brace_depth += 1;
                    if pending_enum {
                        enum_depth = Some(brace_depth);
                        pending_enum = false;
                    }
                }
                b'}' => {
                    if enum_depth == Some(brace_depth) {
                        enum_depth = None;
                    }
                    brace_depth = brace_depth.saturating_sub(1);
                }
                _ => {}
            }
            cursor += 1;
            continue;
        }
        let mut start = cursor;
        while cursor < bytes.len() && identifier_byte(bytes[cursor]) {
            cursor += 1;
        }
        if &text[start..cursor] == "r" && bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
            start = cursor;
            while cursor < bytes.len() && identifier_byte(bytes[cursor]) {
                cursor += 1;
            }
        }
        let name = &text[start..cursor];
        // Declaration names and tuple-pattern binders are not independent
        // execution. This changes refusal-set subtraction only, never the
        // general call producer or ordinary function admission.
        let declaration_name = matches!(
            previous_word,
            "fn" | "struct" | "enum" | "type" | "trait" | "let"
        );
        let declaration_keyword =
            matches!(name, "fn" | "struct" | "enum" | "type" | "trait" | "pub");
        if !declaration_name
            && !declaration_keyword
            && enum_depth.is_none()
            && text[cursor..].trim_start().starts_with('(')
        {
            names.insert(name.to_string());
        }
        if name == "enum" && enum_depth.is_none() {
            pending_enum = true;
        }
        previous_word = name;
    }
    names
}

/// Per-test refusal set. A genuine outside-span occurrence defeats refusal.
pub(crate) fn property_only_call_names(text: &str) -> std::collections::BTreeSet<String> {
    let macros = opaque_property_macros(text);
    if macros.is_empty() {
        return std::collections::BTreeSet::new();
    }
    let mut inside = std::collections::BTreeSet::new();
    for item in &macros {
        inside.extend(lexical_call_names(&text[item.range.clone()]));
    }
    for (_, outside) in outside_property_macros(text, &macros) {
        for name in lexical_call_names(outside) {
            inside.remove(&name);
        }
    }
    inside
}

fn identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn skip_trivia(text: &str, mut cursor: usize) -> usize {
    let bytes = text.as_bytes();
    loop {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let tail = &text[cursor..];
        if !(tail.starts_with("//") || tail.starts_with("/*")) {
            return cursor;
        }
        if let Some(end) = crate::analysis::syntax::non_code_token_end(text, cursor) {
            cursor = end;
        } else {
            return cursor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_spans_respect_literals_comments_and_function_lookalikes() {
        let source = r####"// proptest! { fn phantom() {} }
/* quickcheck! { /* nested */ fn phantom() {} } */
let text = r###"prop_assert_eq!(owner(1), [)])"###;
let escaped = "proptest! { \" }";
let character = '}';
fn proptest(x: u32) -> u32 { x }
let value = prop_assert_eq(owner(2));
other::prop_assert_eq /* between */ ! /* open */ (owner([1, { 2 }]), r#") }"#);
owner(3);
"####;
        let macros = opaque_property_macros(source);
        assert_eq!(macros.len(), 1);
        assert_eq!(macros[0].name, "prop_assert_eq");
        assert!(source[macros[0].range.clone()].ends_with("r#\") }\"#)"));
        assert!(!property_only_call_names(source).contains("owner"));
        let mentions = mentioned_identifiers(&source[macros[0].range.clone()]);
        assert!(mentions.contains(&"owner".to_string()));
        assert!(!mentions.contains(&"phantom".to_string()));
    }

    #[test]
    fn property_only_call_refusal_subtracts_independent_outside_calls() {
        assert!(property_only_call_names("prop_assert_eq!(owner(1), 1);").contains("owner"));
        assert!(
            !property_only_call_names("prop_assert_eq!(owner(1), 1); assert_eq!(owner(2), 2);")
                .contains("owner")
        );
        assert!(
            property_only_call_names(
                "prop_assert_eq!(owner(1), 1); // owner(2)\nlet s = \"owner(3)\";"
            )
            .contains("owner")
        );
        assert!(
            !property_only_call_names("fn prop_assert_eq(x: u32) {} prop_assert_eq(owner(1));")
                .contains("owner")
        );
    }

    #[test]
    fn declarations_do_not_cancel_property_only_call_refusal() {
        for outside in [
            "fn owner() {}",
            "fn /* declaration */ owner() {}",
            "fn r#owner() {}",
            "struct owner(i32);",
            "struct r#owner(i32);",
            "type Callback = fn(i32);",
            "enum Choice { owner(i32), Other }",
            "let owner(value) = opaque;",
        ] {
            let body = format!("{outside} prop_assert_eq!(owner(100),90);");
            assert!(
                property_only_call_names(&body).contains("owner"),
                "{outside}"
            );
        }
        assert!(!property_only_call_names("fn owner() { prop_assert_eq!(super::owner(100),90); assert_eq!(super::owner(100),90); }").contains("owner"));
    }

    #[test]
    fn malformed_property_delimiters_keep_the_remaining_suffix_opaque() {
        for source in [
            "proptest! { ([)\n#[test] fn phantom() { owner(1); }",
            "quickcheck! {\n#[test] fn phantom() { owner(1); }",
        ] {
            let macros = opaque_property_macros(source);
            assert_eq!(macros.len(), 1);
            assert_eq!(macros[0].range.end, source.len());
            assert!(
                outside_property_macros(source, &macros).all(|(_, part)| !part.contains("phantom"))
            );
        }
        let deep = format!(
            "proptest! {{ {}owner(1){} }} fn after() {{}}",
            "(".repeat(257),
            ")".repeat(257)
        );
        assert_eq!(opaque_property_macros(&deep)[0].range.end, deep.len());
    }

    #[test]
    fn property_calls_do_not_survive_crlf_or_multiline_arguments() {
        let body =
            "fn boundary() {\r\n prop_assert_eq!(\r\n hidden(1),\r\n 1); visible(2);\r\n}\r\n";
        let calls = crate::analysis::extract::extract_call_facts(body, 1);
        assert!(!calls.iter().any(|call| call.name == "hidden"));
        assert!(
            calls
                .iter()
                .any(|call| call.name == "visible" && call.line == 4)
        );
    }

    #[test]
    fn retained_call_text_cannot_reborrow_discarded_same_line_arguments() {
        for body in [
            "fn boundary() { prop_assert_eq!(owner(100,100),90); assert_eq!(owner(90,100),90); }",
            "fn boundary() { assert_eq!(owner(90,100),90); prop_assert_eq!(owner(100,100),90); }",
            r##"fn boundary() { let note = r#"é [( ]"#; /* λ */ prop_assert_eq!(owner(100,100),90); assert_eq!(owner(90,100),90); }"##,
        ] {
            let calls = crate::analysis::extract::extract_call_facts(body, 1);
            assert!(calls.iter().any(|call| call.name == "owner"));
            for owner in calls.iter().filter(|call| call.name == "owner") {
                assert!(!owner.text.contains("owner(100,100)"));
                assert!(owner.text.contains("owner(90,100)"));
                assert_eq!(owner.text.len(), body.trim().len());
                assert_eq!(
                    owner.text.find("owner(90,100)"),
                    body.trim().find("owner(90,100)")
                );
            }
        }
    }

    #[test]
    fn fallback_property_tokens_do_not_create_functions_or_oracles() {
        let source = "invalid rust;\nproptest! {\n#[test]\nfn phantom() { assert_eq!(owner(1),1); }\n}\n#[test]\nfn ordinary() { proptest! { assert_eq!(hidden(1),1); } assert_eq!(visible(1),1); }\n";
        let facts = crate::analysis::syntax::lexical::summarize_file_lexically(
            std::path::PathBuf::from("tests/opaque.rs"),
            source.to_string(),
        );
        assert!(facts.used_lexical_fallback);
        assert_eq!(facts.tests.len(), 1);
        assert_eq!(facts.tests[0].name, "ordinary");
        assert_eq!(facts.functions.len(), 1);
        assert!(
            facts.tests[0]
                .assertions
                .iter()
                .all(|oracle| !oracle.text.contains("hidden"))
        );
        assert!(
            facts.tests[0]
                .assertions
                .iter()
                .any(|oracle| oracle.text.contains("visible"))
        );
        assert!(
            facts
                .calls
                .iter()
                .all(|call| call.name != "hidden" && call.name != "owner")
        );
    }
}
