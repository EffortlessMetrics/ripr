//! Tests for the no-panic-family policy module, extracted from
//! `xtask/src/tests.rs` as the second behavior-preserving decomposition
//! slice of #2119. Test bodies moved verbatim; shared harness helpers
//! (`temp_dir`, `write`, `with_temp_cwd`) are reused from `crate::tests`.

use std::fs;

use super::{
    PanicAllowEntry, PanicAllowEntryV2, PanicAllowEntryVersioned, PanicFamilyLastSeen,
    PanicFamilySelector, SemanticPanicFinding, build_no_panic_allowlist_proposals,
    collect_panic_findings, collect_semantic_panic_findings, evaluate_semantic_no_panic_policy,
    forbidden_panic_patterns, no_panic_toml_string, panic_family_from_pattern,
    parse_no_panic_allowlist_toml, parse_no_panic_allowlist_toml_v2, parse_string_value,
    render_no_panic_allowlist_proposals_markdown, render_no_panic_allowlist_proposals_toml,
    semantic_selector_matches,
};
use crate::tests::{temp_dir, with_temp_cwd, write};

fn semantic_panic_finding(
    line: usize,
    container: &str,
    receiver_fingerprint: Option<&str>,
) -> SemanticPanicFinding {
    SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "method_call".to_string(),
        line,
        column: Some(5),
        container: Some(container.to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: receiver_fingerprint.map(str::to_string),
        snippet_fingerprint: "value.unwrap()".to_string(),
    }
}

fn semantic_panic_entry(
    id: &str,
    container: &str,
    receiver_fingerprint: Option<&str>,
    last_seen_line: Option<usize>,
) -> PanicAllowEntryVersioned {
    PanicAllowEntryVersioned::V2(PanicAllowEntryV2 {
        id: Some(id.to_string()),
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        classification: Some("test_only".to_string()),
        owner: Some("test-infra".to_string()),
        explanation: "Test helper".to_string(),
        expires: Some("2026-12-31".to_string()),
        selector: Some(PanicFamilySelector {
            kind: "method_call".to_string(),
            container: Some(container.to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: receiver_fingerprint.map(str::to_string),
            text_contains: None,
            snippet: None,
        }),
        last_seen: last_seen_line.map(|line| PanicFamilyLastSeen {
            line,
            column: Some(5),
        }),
        count: None,
    })
}

// ============================================================================
// Panic allowlist TOML tests
// ============================================================================

#[test]
fn validate_panic_allow_entry_v2_rejects_unfilled_todo_placeholders() -> Result<(), String> {
    // #2090: a pasted --propose entry with TODO-* markers must fail the
    // gate, not pass as reviewed policy.
    let base = PanicAllowEntryV2 {
        id: Some("panic-0099".to_string()),
        path: "crates/ripr/src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        classification: Some("test_only".to_string()),
        owner: Some("core/analysis".to_string()),
        explanation: "reviewed reason".to_string(),
        expires: Some("2026-12-01".to_string()),
        selector: None,
        last_seen: None,
        count: None,
    };
    let ok = super::validate_panic_allow_entry_v2(&base, "policy/test.toml", 1, "0.3");
    assert!(ok.is_ok(), "reviewed entry must validate: {ok:?}");
    for (field, entry) in [
        (
            "id",
            PanicAllowEntryV2 {
                id: Some("TODO-review-id".to_string()),
                ..base.clone()
            },
        ),
        (
            "owner",
            PanicAllowEntryV2 {
                owner: Some("TODO-owner".to_string()),
                ..base.clone()
            },
        ),
        (
            "expires",
            PanicAllowEntryV2 {
                expires: Some("TODO-expiry".to_string()),
                ..base.clone()
            },
        ),
    ] {
        match super::validate_panic_allow_entry_v2(&entry, "policy/test.toml", 1, "0.3") {
            Err(message) => assert!(
                message.contains("placeholder"),
                "{field} rejection should name the placeholder: {message}"
            ),
            Ok(()) => {
                return Err(format!("{field} placeholder must be rejected"));
            }
        }
    }
    Ok(())
}

#[test]
fn parse_no_panic_allowlist_toml_parses_valid_entries() {
    with_temp_cwd("parse_valid", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
classification = "test_only"
explanation = "Test helper"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "src/lib.rs");
        assert_eq!(entries[0].line, 42);
        assert_eq!(entries[0].column, Some(10));
        assert_eq!(entries[0].family, "unwrap");
        assert_eq!(entries[0].classification, Some("test_only".to_string()));
        assert_eq!(entries[0].explanation, "Test helper");
    });
}

#[test]
fn parse_no_panic_allowlist_toml_requires_path() {
    with_temp_cwd("missing_path", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
line = 42
family = "unwrap"
explanation = "Missing path"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required field: path"));
    });
}

#[test]
fn parse_no_panic_allowlist_toml_requires_line() {
    with_temp_cwd("missing_line", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
explanation = "Missing line"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required field: line"));
    });
}

#[test]
fn parse_no_panic_allowlist_toml_requires_family() {
    with_temp_cwd("missing_family", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
explanation = "Missing family"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("missing required field: family")
        );
    });
}

#[test]
fn parse_no_panic_allowlist_toml_requires_explanation() {
    with_temp_cwd("missing_explanation", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
family = "unwrap"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("missing required field: explanation")
        );
    });
}

#[test]
fn parse_no_panic_allowlist_toml_rejects_unknown_fields() {
    with_temp_cwd("unknown_field", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
family = "unwrap"
explanation = "Test"
unknown_field = "value"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("unknown field 'unknown_field'")
        );
    });
}

#[test]
fn parse_no_panic_allowlist_toml_rejects_duplicate_locations() {
    with_temp_cwd("duplicate", |root| {
        let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "First entry"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "Duplicate entry"
"#;
        write(&root.join("allowlist.toml"), toml_content);

        let result = parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("duplicate allowlist entry"));
    });
}

#[test]
fn panic_family_from_pattern_matches_all_families() {
    assert_eq!(panic_family_from_pattern("unwrap("), "unwrap");
    assert_eq!(panic_family_from_pattern("expect("), "expect");
    assert_eq!(panic_family_from_pattern("panic!"), "panic_macro");
    assert_eq!(panic_family_from_pattern("todo!"), "todo");
    assert_eq!(panic_family_from_pattern("unimplemented!"), "unimplemented");
    assert_eq!(panic_family_from_pattern("unreachable!"), "unreachable");
}

#[test]
fn collect_panic_findings_finds_exact_locations() {
    with_temp_cwd("collect_findings", |root| {
        let rs_file = root.join("lib.rs");
        write(
            &rs_file,
            "fn test() {\n    let x = some_fn().unwrap();\n    let y = other().expect(\"msg\");\n}\n",
        );

        let patterns = vec!["unwrap(".to_string(), "expect(".to_string()];
        let findings = collect_panic_findings(root, &patterns).unwrap();

        // Should find unwrap( on line 2 and expect( on line 3
        assert!(findings.iter().any(|f| f.line == 2 && f.family == "unwrap"));
        assert!(findings.iter().any(|f| f.line == 3 && f.family == "expect"));
    });
}

#[test]
fn parse_string_value_preserves_hashes_and_unescapes_quotes_inside_values() -> Result<(), String> {
    let parsed = parse_string_value(
        "\"fs::write( root.join(\\\"src/lib.rs\\\"), r#\\\" body \\\"#, )\" # trailing comment",
        "allowlist.toml",
        1,
    )?;
    assert_eq!(
        parsed,
        "fs::write( root.join(\"src/lib.rs\"), r#\" body \"#, )"
    );
    Ok(())
}

// ============================================================================
// v0.2 semantic selector tests
// ============================================================================

#[test]
fn v0_2_method_call_selector_allows_line_movement() -> Result<(), String> {
    let selector = PanicFamilySelector {
        kind: "method_call".to_string(),
        container: Some("my_test_fn".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    let finding_at_10 = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "method_call".to_string(),
        line: 10,
        column: Some(5),
        container: Some("my_test_fn".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "x.unwrap()".to_string(),
    };
    let finding_at_25 = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "method_call".to_string(),
        line: 25,
        column: Some(12),
        container: Some("my_test_fn".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "y.unwrap()".to_string(),
    };
    if !semantic_selector_matches(&selector, &finding_at_10) {
        return Err("selector should match finding at line 10".to_string());
    }
    if !semantic_selector_matches(&selector, &finding_at_25) {
        return Err("selector should match finding at line 25 (line movement allowed)".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_receiver_fingerprint_disambiguates_same_container_calls() -> Result<(), String> {
    let selector = PanicFamilySelector {
        kind: "method_call".to_string(),
        container: Some("my_test_fn".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: Some("left_side()".to_string()),
        text_contains: None,
        snippet: None,
    };
    let matching = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "method_call".to_string(),
        line: 10,
        column: Some(5),
        container: Some("my_test_fn".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: Some("left_side()".to_string()),
        snippet_fingerprint: "left_side().unwrap()".to_string(),
    };
    let different_receiver = SemanticPanicFinding {
        receiver_fingerprint: Some("right_side()".to_string()),
        snippet_fingerprint: "right_side().unwrap()".to_string(),
        ..matching.clone()
    };
    if !semantic_selector_matches(&selector, &matching) {
        return Err("receiver fingerprint should match identical receiver".to_string());
    }
    if semantic_selector_matches(&selector, &different_receiver) {
        return Err("receiver fingerprint should reject a different receiver".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_macro_call_selector_matches_exact_macro() -> Result<(), String> {
    let selector = PanicFamilySelector {
        kind: "macro_call".to_string(),
        container: Some("test_fn".to_string()),
        callee: Some("panic!".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    let finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "panic_macro".to_string(),
        kind: "macro_call".to_string(),
        line: 5,
        column: Some(9),
        container: Some("test_fn".to_string()),
        callee: Some("panic!".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "panic!(\"msg\")".to_string(),
    };
    if !semantic_selector_matches(&selector, &finding) {
        return Err("macro_call selector should match panic! finding".to_string());
    }
    let wrong_callee = SemanticPanicFinding {
        callee: Some("todo!".to_string()),
        family: "todo".to_string(),
        snippet_fingerprint: "todo!(\"msg\")".to_string(),
        ..finding.clone()
    };
    if semantic_selector_matches(&selector, &wrong_callee) {
        return Err("macro_call selector should not match different callee".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_call_selector_matches_exact_free_function() -> Result<(), String> {
    let selector = PanicFamilySelector {
        kind: "call".to_string(),
        container: Some("helper".to_string()),
        callee: Some("panic".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    let finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "panic_macro".to_string(),
        kind: "call".to_string(),
        line: 3,
        column: Some(5),
        container: Some("helper".to_string()),
        callee: Some("panic".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "panic(\"msg\")".to_string(),
    };
    if !semantic_selector_matches(&selector, &finding) {
        return Err("call selector should match finding".to_string());
    }
    let wrong_kind = SemanticPanicFinding {
        kind: "method_call".to_string(),
        ..finding.clone()
    };
    if semantic_selector_matches(&selector, &wrong_kind) {
        return Err("call selector should not match method_call finding".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_string_literal_selector_requires_text_contains() -> Result<(), String> {
    let selector_with_text = PanicFamilySelector {
        kind: "string_literal".to_string(),
        container: None,
        callee: None,
        receiver_fingerprint: None,
        text_contains: Some("error".to_string()),
        snippet: None,
    };
    let finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "panic_macro".to_string(),
        kind: "string_literal".to_string(),
        line: 10,
        column: Some(5),
        container: None,
        callee: None,
        receiver_fingerprint: None,
        snippet_fingerprint: "panic!(\"error happened\")".to_string(),
    };
    if !semantic_selector_matches(&selector_with_text, &finding) {
        return Err("string_literal selector with text_contains should match".to_string());
    }
    let selector_no_text = PanicFamilySelector {
        kind: "string_literal".to_string(),
        container: None,
        callee: None,
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    if semantic_selector_matches(&selector_no_text, &finding) {
        return Err("string_literal selector without text_contains should not match".to_string());
    }
    let finding_no_match = SemanticPanicFinding {
        snippet_fingerprint: "panic!(\"other\")".to_string(),
        ..finding.clone()
    };
    if semantic_selector_matches(&selector_with_text, &finding_no_match) {
        return Err(
            "string_literal selector should not match when text_contains is absent from snippet"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn v0_2_selector_kind_mismatch_rejects() -> Result<(), String> {
    let selector = PanicFamilySelector {
        kind: "method_call".to_string(),
        container: None,
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    let macro_finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "panic_macro".to_string(),
        kind: "macro_call".to_string(),
        line: 5,
        column: Some(9),
        container: None,
        callee: Some("panic!".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "panic!(\"msg\")".to_string(),
    };
    if semantic_selector_matches(&selector, &macro_finding) {
        return Err("method_call selector should reject macro_call finding".to_string());
    }
    let invalid_selector = PanicFamilySelector {
        kind: "invalid".to_string(),
        container: None,
        callee: None,
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    if semantic_selector_matches(&invalid_selector, &macro_finding) {
        return Err("invalid selector kind should reject all findings".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_rejects_unknown_selector_kind() -> Result<(), String> {
    with_temp_cwd("reject_unknown_kind", |root| {
        let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
explanation = "Bad kind"

[allow.selector]
kind = "foo"
callee = "unwrap"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let toml_path = root
            .join("allowlist.toml")
            .to_str()
            .ok_or("non-UTF-8 path")?
            .to_string();
        let result = parse_no_panic_allowlist_toml_v2(&toml_path);
        let err = result
            .err()
            .ok_or("expected parse error for unknown selector kind")?;
        if !err.contains("invalid selector kind 'foo'") {
            return Err(format!("unexpected error message: {err}"));
        }
        if !err.contains("method_call, macro_call, call, string_literal") {
            return Err(format!("error should list supported kinds, got: {err}"));
        }
        Ok(())
    })
}

#[test]
fn v0_2_call_selector_handles_associated_function_form() -> Result<(), String> {
    with_temp_cwd("associated_fn", |root| {
        // Option::unwrap(x) is a CallExpr, callee should be just "unwrap"
        let code = "fn demo() { Option::unwrap(some_opt) }\n";
        write(&root.join("lib.rs"), code);
        let patterns = vec!["unwrap(".to_string()];
        let findings = collect_semantic_panic_findings(root, &patterns)
            .map_err(|e| format!("collect failed: {e}"))?;
        if findings.is_empty() {
            return Err("expected to find Option::unwrap call".to_string());
        }
        let f = &findings[0];
        if f.kind != "call" {
            return Err(format!(
                "Option::unwrap(x) should be kind=call, got kind={}",
                f.kind
            ));
        }
        if f.callee.as_deref() != Some("unwrap") {
            return Err(format!("callee should be 'unwrap', got {:?}", f.callee));
        }
        if f.family != "unwrap" {
            return Err(format!("family should be 'unwrap', got {}", f.family));
        }
        Ok(())
    })
}

#[test]
fn v0_2_call_selector_does_not_match_substring_helper_name() -> Result<(), String> {
    with_temp_cwd("no_substring_match", |root| {
        // A function named `panic_family_from_pattern` should NOT match
        // the panic-family patterns since its base callee name is
        // `panic_family_from_pattern`, not `panic`.
        let code = "fn demo() { panic_family_from_pattern(\"x\") }\n";
        write(&root.join("lib.rs"), code);
        let patterns = vec!["panic!".to_string()];
        let findings = collect_semantic_panic_findings(root, &patterns)
            .map_err(|e| format!("collect failed: {e}"))?;
        if !findings.is_empty() {
            return Err(format!(
                "panic_family_from_pattern should not match as a panic-family call, got {} findings",
                findings.len()
            ));
        }
        Ok(())
    })
}

#[test]
fn v0_2_string_literal_still_requires_text_contains() -> Result<(), String> {
    with_temp_cwd("string_literal_text_contains", |root| {
        let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Needs text_contains"

[allow.selector]
kind = "string_literal"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let toml_path = root
            .join("allowlist.toml")
            .to_str()
            .ok_or("non-UTF-8 path")?
            .to_string();
        let result = parse_no_panic_allowlist_toml_v2(&toml_path);
        let err = result
            .err()
            .ok_or("expected parse error for string_literal without text_contains")?;
        if !err.contains("string_literal selector requires text_contains") {
            return Err(format!("unexpected error message: {err}"));
        }
        Ok(())
    })
}

#[test]
fn v0_2_kind_mismatch_reports_actionable_error() -> Result<(), String> {
    // A method_call selector must not match a call-type finding
    let selector = PanicFamilySelector {
        kind: "method_call".to_string(),
        container: Some("demo".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    // Option::unwrap(x) produces kind=call, not method_call
    let call_finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "call".to_string(),
        line: 3,
        column: Some(5),
        container: Some("demo".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "Option::unwrap(x)".to_string(),
    };
    if semantic_selector_matches(&selector, &call_finding) {
        return Err(
            "method_call selector must not match a call-type finding (Option::unwrap)".to_string(),
        );
    }
    // Conversely, a method_call finding must not match a call selector
    let call_selector = PanicFamilySelector {
        kind: "call".to_string(),
        container: Some("demo".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        text_contains: None,
        snippet: None,
    };
    let method_finding = SemanticPanicFinding {
        path: "src/lib.rs".to_string(),
        family: "unwrap".to_string(),
        kind: "method_call".to_string(),
        line: 5,
        column: Some(8),
        container: Some("demo".to_string()),
        callee: Some("unwrap".to_string()),
        receiver_fingerprint: None,
        snippet_fingerprint: "x.unwrap()".to_string(),
    };
    if semantic_selector_matches(&call_selector, &method_finding) {
        return Err("call selector must not match a method_call finding (.unwrap())".to_string());
    }
    Ok(())
}

#[test]
fn v0_2_last_seen_drift_is_advisory_not_failure() -> Result<(), String> {
    with_temp_cwd("last_seen_drift", |root| {
        let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper"

[allow.selector]
kind = "method_call"
container = "my_fn"
callee = "unwrap"

[allow.last_seen]
line = 10
column = 5
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let entries =
            parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap())
                .map_err(|e| format!("parse failed: {e}"))?;

        let entry_count = entries.len();
        if entry_count != 1 {
            return Err(format!("expected 1 entry, got {entry_count}"));
        }

        match &entries[0] {
            PanicAllowEntryVersioned::V2(v2) => {
                let selector = v2.selector.as_ref().ok_or("missing selector")?;
                let finding = SemanticPanicFinding {
                    path: "src/lib.rs".to_string(),
                    family: "unwrap".to_string(),
                    kind: "method_call".to_string(),
                    line: 20,
                    column: Some(8),
                    container: Some("my_fn".to_string()),
                    callee: Some("unwrap".to_string()),
                    receiver_fingerprint: None,
                    snippet_fingerprint: "x.unwrap()".to_string(),
                };
                if !semantic_selector_matches(selector, &finding) {
                    return Err("selector should match finding at different line".to_string());
                }
                let ls = v2.last_seen.as_ref().ok_or("missing last_seen")?;
                if ls.line != 10 {
                    return Err(format!("expected last_seen.line 10, got {}", ls.line));
                }
            }
            _ => return Err("expected V2 entry".to_string()),
        }
        Ok(())
    })
}

#[test]
fn no_panic_policy_reports_allowed_drift_and_unallowed_buckets() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "other_fn", None),
    ];
    let entries = vec![semantic_panic_entry(
        "panic-0001",
        "my_fn",
        Some("left()"),
        Some(10),
    )];

    let report = evaluate_semantic_no_panic_policy(&findings, &entries);
    if report.allowed_findings.len() != 1 {
        return Err(format!(
            "expected one allowed finding, got {:?}",
            report.allowed_findings
        ));
    }
    if report.advisory_drift.len() != 1 {
        return Err(format!(
            "expected one advisory drift entry, got {:?}",
            report.advisory_drift
        ));
    }
    if report.unallowed_findings.len() != 1 {
        return Err(format!(
            "expected one unallowed finding, got {:?}",
            report.unallowed_findings
        ));
    }
    if !report
        .violations
        .iter()
        .any(|violation| violation.contains("unallowed panic-family"))
    {
        return Err(format!(
            "expected unallowed violation, got {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_rejects_ambiguous_selector_matches() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
    ];
    let entries = vec![semantic_panic_entry("panic-0001", "my_fn", None, None)];

    let report = evaluate_semantic_no_panic_policy(&findings, &entries);
    if !report
        .warnings
        .iter()
        .any(|warning| warning.contains("ambiguous semantic allowlist entry"))
    {
        return Err(format!(
            "expected ambiguity warning, got {:?}",
            report.warnings
        ));
    }
    if !report
        .violations
        .iter()
        .any(|violation| violation.contains("ambiguous semantic allowlist entry"))
    {
        return Err(format!(
            "expected ambiguity violation, got {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_rejects_duplicate_semantic_identities() -> Result<(), String> {
    let findings = vec![semantic_panic_finding(20, "my_fn", Some("left()"))];
    let entries = vec![
        semantic_panic_entry("panic-0001", "my_fn", Some("left()"), None),
        semantic_panic_entry("panic-0002", "my_fn", Some("left()"), None),
    ];

    let report = evaluate_semantic_no_panic_policy(&findings, &entries);
    if !report
        .warnings
        .iter()
        .any(|warning| warning.contains("duplicate semantic allowlist identity"))
    {
        return Err(format!(
            "expected duplicate identity warning, got {:?}",
            report.warnings
        ));
    }
    if !report
        .violations
        .iter()
        .any(|violation| violation.contains("duplicate semantic"))
    {
        return Err(format!(
            "expected duplicate identity violation, got {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_proposals_convert_v0_1_entries_to_semantic_selectors() -> Result<(), String> {
    let findings = vec![semantic_panic_finding(20, "my_fn", Some("left()"))];
    let entries = vec![PanicAllowEntryVersioned::V1(PanicAllowEntry {
        path: "src/lib.rs".to_string(),
        line: 20,
        column: Some(5),
        family: "unwrap".to_string(),
        classification: Some("test_only".to_string()),
        explanation: "Legacy test helper".to_string(),
    })];

    let proposals = build_no_panic_allowlist_proposals(&findings, &entries);
    if proposals.len() != 1 {
        return Err(format!("expected one proposal, got {proposals:?}"));
    }
    let proposal = &proposals[0];
    if !proposal.replaces_v1_entry {
        return Err("expected proposal to replace a v0.1 entry".to_string());
    }
    if proposal.old_coordinates.as_deref() != Some("20:5") {
        return Err(format!(
            "unexpected old coordinates: {:?}",
            proposal.old_coordinates
        ));
    }
    if proposal.container.as_deref() != Some("my_fn")
        || proposal.callee.as_deref() != Some("unwrap")
        || proposal.receiver_fingerprint.as_deref() != Some("left()")
    {
        return Err(format!("unexpected selector proposal: {proposal:?}"));
    }
    let markdown = render_no_panic_allowlist_proposals_markdown(&proposals);
    if !markdown.contains("Legacy test helper") || !markdown.contains("Replaces v0.1 entry") {
        return Err(format!("unexpected markdown proposal: {markdown}"));
    }
    let toml = render_no_panic_allowlist_proposals_toml(&proposals);
    if !toml.contains("receiver_fingerprint = \"left()\"")
        || !toml.contains("status = \"proposal\"")
    {
        return Err(format!("unexpected TOML proposal: {toml}"));
    }
    Ok(())
}

#[test]
fn no_panic_proposals_keep_drifted_v0_1_entries_review_only() -> Result<(), String> {
    let findings = vec![semantic_panic_finding(30, "my_fn", Some("left()"))];
    let entries = vec![PanicAllowEntryVersioned::V1(PanicAllowEntry {
        path: "src/lib.rs".to_string(),
        line: 20,
        column: Some(5),
        family: "unwrap".to_string(),
        classification: Some("test_only".to_string()),
        explanation: "Legacy test helper".to_string(),
    })];

    let proposals = build_no_panic_allowlist_proposals(&findings, &entries);
    if proposals.len() != 1 {
        return Err(format!("expected one proposal, got {proposals:?}"));
    }
    let proposal = &proposals[0];
    if proposal.confidence != "review" {
        return Err(format!("expected review confidence, got {proposal:?}"));
    }
    if !proposal
        .warnings
        .iter()
        .any(|warning| warning.contains("v0.1 coordinates did not match a current finding"))
    {
        return Err(format!(
            "expected drift warning on proposal, got {proposal:?}"
        ));
    }
    Ok(())
}

#[test]
fn no_panic_proposals_include_single_match_semantic_entries() -> Result<(), String> {
    let findings = vec![semantic_panic_finding(20, "my_fn", Some("left()"))];
    let entries = vec![semantic_panic_entry(
        "panic-0001",
        "my_fn",
        Some("left()"),
        Some(20),
    )];

    let proposals = build_no_panic_allowlist_proposals(&findings, &entries);
    if proposals.len() != 1 {
        return Err(format!("expected one proposal, got {proposals:?}"));
    }
    let proposal = &proposals[0];
    if proposal.replaces_v1_entry {
        return Err(format!(
            "semantic entry proposal should not replace v0.1 entry: {proposal:?}"
        ));
    }
    if proposal.confidence != "high" || !proposal.warnings.is_empty() {
        return Err(format!(
            "single-match semantic proposal should be high confidence: {proposal:?}"
        ));
    }
    Ok(())
}

#[test]
fn no_panic_proposals_split_ambiguous_semantic_selectors() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
    ];
    let entries = vec![semantic_panic_entry("panic-0001", "my_fn", None, None)];

    let proposals = build_no_panic_allowlist_proposals(&findings, &entries);
    if proposals.len() != 2 {
        return Err(format!("expected two proposals, got {proposals:?}"));
    }
    if !proposals.iter().all(|proposal| {
        proposal
            .warnings
            .iter()
            .any(|warning| warning.contains("existing selector matches 2 current findings"))
            && proposal.warnings.iter().any(|warning| {
                warning.contains("proposal adds receiver_fingerprint to disambiguate")
            })
    }) {
        return Err(format!(
            "expected ambiguity warnings on proposals, got {proposals:?}"
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_counted_entry_allows_exact_match() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
    ];
    let mut entry = semantic_panic_entry("panic-0001", "my_fn", None, None);
    if let PanicAllowEntryVersioned::V2(ref mut v2) = entry {
        v2.count = Some(2);
    }

    let report = evaluate_semantic_no_panic_policy(&findings, &[entry]);
    if !report.violations.is_empty() {
        return Err(format!(
            "count=2 entry matching 2 findings should have no violations, got: {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_counted_entry_too_few_is_advisory_drift() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
    ];
    let mut entry = semantic_panic_entry("panic-0001", "my_fn", None, None);
    if let PanicAllowEntryVersioned::V2(ref mut v2) = entry {
        v2.count = Some(3);
    }

    let report = evaluate_semantic_no_panic_policy(&findings, &[entry]);
    if !report.violations.is_empty() {
        return Err(format!(
            "expected entry match count {{3}} with actual {{2}} should be advisory drift, not a violation; got: {:?}",
            report.violations
        ));
    }
    if !report
        .advisory_drift
        .iter()
        .any(|d| d.contains("stale-count drift"))
    {
        return Err(format!(
            "expected stale-count drift advisory when entry match count shrank, got: {:?}",
            report.advisory_drift
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_counted_entry_too_many_is_count_exceeded() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
        semantic_panic_finding(40, "my_fn", Some("third()")),
    ];
    let mut entry = semantic_panic_entry("panic-0001", "my_fn", None, None);
    if let PanicAllowEntryVersioned::V2(ref mut v2) = entry {
        v2.count = Some(2);
    }

    let report = evaluate_semantic_no_panic_policy(&findings, &[entry]);
    if !report
        .violations
        .iter()
        .any(|v| v.contains("count exceeded"))
    {
        return Err(format!(
            "expected entry match count {{2}} with actual {{3}} should produce 'count exceeded' violation, got: {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_default_count_one_multi_match_remains_ambiguous() -> Result<(), String> {
    let findings = vec![
        semantic_panic_finding(20, "my_fn", Some("left()")),
        semantic_panic_finding(30, "my_fn", Some("right()")),
    ];
    // No explicit count; defaults to 1.
    let entry = semantic_panic_entry("panic-0001", "my_fn", None, None);

    let report = evaluate_semantic_no_panic_policy(&findings, &[entry]);
    if !report
        .violations
        .iter()
        .any(|v| v.contains("ambiguous semantic allowlist entry"))
    {
        return Err(format!(
            "default entry match count {{1}} with actual {{2}} should preserve 'ambiguous semantic allowlist entry' wording, got: {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn no_panic_policy_snippet_narrows_to_matching_finding() -> Result<(), String> {
    let findings = vec![
        SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 10,
            column: Some(5),
            container: Some("my_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "SystemTime::now().duration_since(UNIX_EPOCH).unwrap()"
                .to_string(),
        },
        SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 20,
            column: Some(5),
            container: Some("my_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "some_result.unwrap()".to_string(),
        },
    ];
    let mut entry = semantic_panic_entry("panic-0001", "my_fn", None, None);
    if let PanicAllowEntryVersioned::V2(ref mut v2) = entry
        && let Some(ref mut sel) = v2.selector
    {
        sel.snippet = Some("UNIX_EPOCH".to_string());
    }

    let report = evaluate_semantic_no_panic_policy(&findings, &[entry]);
    // The second finding (some_result.unwrap()) should be unallowed.
    if !report
        .unallowed_findings
        .iter()
        .any(|f| f.contains("src/lib.rs:20"))
    {
        return Err(format!(
            "finding at line 20 should be unallowed when snippet only matches line 10, got: {:?}",
            report.unallowed_findings
        ));
    }
    if report
        .violations
        .iter()
        .any(|v| v.contains("src/lib.rs:10"))
    {
        return Err(format!(
            "finding at line 10 should be allowed by snippet, but got violation: {:?}",
            report.violations
        ));
    }
    Ok(())
}

#[test]
fn parse_no_panic_allowlist_toml_parses_snippet_and_count() -> Result<(), String> {
    with_temp_cwd("snippet_count_parsing", |root| {
        let toml_content = r#"schema_version = "0.3"

[[allow]]
id = "panic-0001"
path = "xtask/src/main.rs"
family = "unwrap"
classification = "test_only"
owner = "test-infra"
explanation = "Exactly two unwrap calls in setup helper"
expires = "2026-12-31"
count = 2

[allow.selector]
kind = "method_call"
container = "setup_helper"
callee = "unwrap"
snippet = "duration_since(UNIX_EPOCH)"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let toml_path = root
            .join("allowlist.toml")
            .to_str()
            .ok_or("non-UTF-8 path")?
            .to_string();
        let entries = parse_no_panic_allowlist_toml_v2(&toml_path)
            .map_err(|e| format!("parse failed: {e}"))?;
        if entries.len() != 1 {
            return Err(format!("expected 1 entry, got {}", entries.len()));
        }
        let PanicAllowEntryVersioned::V2(ref v2) = entries[0] else {
            return Err("expected V2 entry".to_string());
        };
        if v2.count != Some(2) {
            return Err(format!("expected count=2, got {:?}", v2.count));
        }
        let sel = v2.selector.as_ref().ok_or("expected selector")?;
        if sel.snippet.as_deref() != Some("duration_since(UNIX_EPOCH)") {
            return Err(format!(
                "expected snippet=duration_since(UNIX_EPOCH), got {:?}",
                sel.snippet
            ));
        }
        Ok(())
    })
}

#[test]
fn no_panic_toml_string_escapes_basic_string_control_characters() -> Result<(), String> {
    let escaped =
        no_panic_toml_string("quote\" slash\\ back\u{08} tab\t line\n form\u{0c} cr\r del\u{7f}");
    if escaped != "quote\\\" slash\\\\ back\\b tab\\t line\\n form\\f cr\\r del\\u007F" {
        return Err(format!("unexpected TOML escaping: {escaped}"));
    }
    Ok(())
}

#[test]
fn v0_3_governed_entries_parse_with_semantic_selectors() -> Result<(), String> {
    with_temp_cwd("v03_governed_entry", |root| {
        let toml_content = r#"schema_version = "0.3"
policy = "no-panic-allowlist"
owner = "core/policy"
status = "canonical"

[[allow]]
id = "panic-0001"
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
owner = "core/tests"
explanation = "Test helper"
expires = "2026-12-31"

[allow.selector]
kind = "method_call"
container = "my_fn"
callee = "unwrap"

[allow.last_seen]
line = 10
column = 5
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let allowlist_path = root.join("allowlist.toml");
        let allowlist_path = allowlist_path.to_str().ok_or("non-UTF-8 allowlist path")?;
        let entries = parse_no_panic_allowlist_toml_v2(allowlist_path)
            .map_err(|err| format!("parse failed: {err}"))?;

        match &entries[0] {
            PanicAllowEntryVersioned::V2(entry) => {
                if entry.id.as_deref() != Some("panic-0001") {
                    return Err(format!("unexpected id: {:?}", entry.id));
                }
                if entry.owner.as_deref() != Some("core/tests") {
                    return Err(format!("unexpected owner: {:?}", entry.owner));
                }
                if entry.expires.as_deref() != Some("2026-12-31") {
                    return Err(format!("unexpected expires: {:?}", entry.expires));
                }
                if entry.selector.is_none() {
                    return Err("expected semantic selector".to_string());
                }
            }
            PanicAllowEntryVersioned::V1(_) => {
                return Err("schema 0.3 entry must parse as semantic V2".to_string());
            }
        }
        Ok(())
    })
}

#[test]
fn v0_3_requires_governed_fields() -> Result<(), String> {
    with_temp_cwd("v03_requires_owner", |root| {
        let toml_content = r#"schema_version = "0.3"
policy = "no-panic-allowlist"
owner = "core/policy"
status = "canonical"

[[allow]]
id = "panic-0001"
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper"
expires = "2026-12-31"

[allow.selector]
kind = "method_call"
container = "my_fn"
callee = "unwrap"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let allowlist_path = root.join("allowlist.toml");
        let allowlist_path = allowlist_path.to_str().ok_or("non-UTF-8 allowlist path")?;
        let result = parse_no_panic_allowlist_toml_v2(allowlist_path);
        let err = result
            .err()
            .ok_or("expected parse error for missing schema 0.3 owner")?;
        if !err.contains("missing required field: owner") {
            return Err(format!("unexpected error message: {err}"));
        }
        Ok(())
    })
}

#[test]
fn v0_3_call_selectors_require_container_and_callee() -> Result<(), String> {
    with_temp_cwd("v03_requires_selector_specificity", |root| {
        let toml_content = r#"schema_version = "0.3"
policy = "no-panic-allowlist"
owner = "core/policy"
status = "canonical"

[[allow]]
id = "panic-0001"
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
owner = "core/tests"
explanation = "Test helper"
expires = "2026-12-31"

[allow.selector]
kind = "method_call"
callee = "unwrap"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let allowlist_path = root.join("allowlist.toml");
        let allowlist_path = allowlist_path.to_str().ok_or("non-UTF-8 allowlist path")?;
        let result = parse_no_panic_allowlist_toml_v2(allowlist_path);
        let err = result
            .err()
            .ok_or("expected parse error for missing selector container")?;
        if !err.contains("method_call selector requires container") {
            return Err(format!("unexpected error message: {err}"));
        }
        Ok(())
    })
}

#[test]
fn v0_3_call_selectors_reject_synthetic_container() -> Result<(), String> {
    with_temp_cwd("v03_rejects_synthetic_container", |root| {
        let toml_content = r#"schema_version = "0.3"
policy = "no-panic-allowlist"
owner = "core/policy"
status = "canonical"

[[allow]]
id = "panic-0001"
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
owner = "core/tests"
explanation = "Test helper"
expires = "2026-12-31"

[allow.selector]
kind = "method_call"
container = "closure_12345"
callee = "unwrap"
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let allowlist_path = root.join("allowlist.toml");
        let allowlist_path = allowlist_path.to_str().ok_or("non-UTF-8 allowlist path")?;
        let result = parse_no_panic_allowlist_toml_v2(allowlist_path);
        let err = result
            .err()
            .ok_or("expected parse error for synthetic selector container")?;
        if !err.contains("uses unstable synthetic container") {
            return Err(format!("unexpected error message: {err}"));
        }
        Ok(())
    })
}

#[test]
fn v0_1_entries_still_match_by_line_and_column() -> Result<(), String> {
    with_temp_cwd("v01_in_v02_file", |root| {
        let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "Legacy entry."
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let entries =
            parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap())
                .map_err(|e| format!("parse failed: {e}"))?;

        match &entries[0] {
            PanicAllowEntryVersioned::V1(v1) => {
                if v1.line != 42 || v1.column != Some(10) || v1.family != "unwrap" {
                    return Err(format!(
                        "v0.1 entry mismatch: line={} col={:?} family={}",
                        v1.line, v1.column, v1.family
                    ));
                }
            }
            _ => return Err("expected V1 entry".to_string()),
        }
        Ok(())
    })
}

#[test]
fn v0_2_missing_selector_and_missing_coordinates_fails_clearly() -> Result<(), String> {
    with_temp_cwd("missing_both", |root| {
        let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
explanation = "Entry with neither selector nor line."
"#;
        write(&root.join("allowlist.toml"), toml_content);
        let result =
            parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap());
        let err = result
            .err()
            .ok_or("expected parse error for entry with neither selector nor line")?;
        if !err.contains("either a [allow.selector] or line number") {
            return Err(format!("unexpected error message: {err}"));
        }
        Ok(())
    })
}

#[test]
fn semantic_extractor_avoids_substring_false_positive_function_names() -> Result<(), String> {
    with_temp_cwd("substring_fp", |root| {
        // Code that contains "panic" in function/variable names but not as actual panic calls
        let code = r#"
fn panic_family_from_pattern() -> &'static str {
    "panic!"
}

fn has_unwrap_in_name() -> bool {
    true
}
"#;
        write(&root.join("lib.rs"), code);
        let patterns = forbidden_panic_patterns();
        let findings = collect_semantic_panic_findings(root, &patterns)
            .map_err(|e| format!("collect failed: {e}"))?;
        // Should find NO panic-family calls since these are just function names and strings
        if !findings.is_empty() {
            let lines: Vec<String> = findings
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}:{} kind={}",
                        f.path,
                        f.line,
                        f.column.unwrap_or(0),
                        f.kind
                    )
                })
                .collect();
            return Err(format!("expected no findings, got: {:?}", lines));
        }
        Ok(())
    })
}

#[test]
fn semantic_extractor_uses_byte_offsets_for_utf8_line_column() -> Result<(), String> {
    // Verify that line_and_column_for_node handles UTF-8 correctly
    let code = "fn test() {\n    let x = \"héllo\".unwrap();\n}\n";
    let patterns = vec!["unwrap(".to_string()];
    let root = temp_dir("utf8_offsets");
    write(&root.join("lib.rs"), code);
    let findings = collect_semantic_panic_findings(&root, &patterns)
        .map_err(|e| format!("collect failed: {e}"))?;
    let _ = fs::remove_dir_all(&root);

    if findings.is_empty() {
        return Err("expected to find unwrap call".to_string());
    }
    let f = &findings[0];
    if f.line != 2 {
        return Err(format!("expected line 2, got {}", f.line));
    }
    if f.family != "unwrap" {
        return Err(format!("expected family unwrap, got {}", f.family));
    }
    if f.kind != "method_call" {
        return Err(format!("expected kind method_call, got {}", f.kind));
    }
    Ok(())
}
