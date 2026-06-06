mod arguments;
mod classify;
mod patterns;
mod scan;

pub(crate) use classify::classify_assertion;
#[cfg(test)]
pub(crate) use patterns::contains_macro_invocation;
pub(crate) use scan::{extract_assertions, extract_line_scanned_oracles};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{OracleKind, OracleStrength};

    fn assert_classification(line: &str, kind: OracleKind, strength: OracleStrength) {
        let classification = classify_assertion(line);
        assert_eq!(classification.kind, kind, "unexpected kind for {line}");
        assert_eq!(
            classification.strength, strength,
            "unexpected strength for {line}"
        );
    }

    #[test]
    fn classify_assertion_distinguishes_error_specificity() {
        assert_classification(
            "assert_matches!(result, Err(ParseError::InvalidDigit));",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        );
        assert_classification(
            "assert!(result.is_err());",
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
        assert_classification(
            "assert_matches!(result, Err(_));",
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
    }

    #[test]
    fn classify_assertion_downgrades_duplicative_equality_even_with_nested_commas() {
        assert_classification(
            r#"assert_eq!(render(vec!["a,b", nested(1, 2)]), render(vec!["a,b", nested(1, 2)]));"#,
            OracleKind::RelationalCheck,
            OracleStrength::Weak,
        );
        assert_classification(
            "assert_eq!(actual.total, expected.total);",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
    }

    #[test]
    fn classify_assertion_recognizes_snapshot_and_custom_exact_helpers() {
        assert_classification(
            "insta::assert_json_snapshot!(payload);",
            OracleKind::Snapshot,
            OracleStrength::Medium,
        );
        assert_classification(
            "assert_payload_eq(actual, expected_with_commas(\"a,b\"));",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        assert_classification(
            "helpers::assert_equal(actual, expected);",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        assert_classification(
            "checker.assert_matches(expected_pattern);",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
    }

    #[test]
    fn extract_assertions_preserves_line_numbers_and_observed_tokens() {
        let facts = extract_assertions(
            r#"
let value = compute();
assert_eq!(value, expected_count);
assert!(events.contains(&Event::Saved));
"#,
            40,
        );

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].line, 42);
        assert_eq!(facts[0].kind, OracleKind::ExactValue);
        assert!(facts[0].observed_tokens.contains(&"value".to_string()));
        assert!(
            facts[0]
                .observed_tokens
                .contains(&"expected_count".to_string())
        );
        assert_eq!(facts[1].line, 43);
        assert_eq!(facts[1].kind, OracleKind::MockExpectation);
        assert_eq!(facts[1].strength, OracleStrength::Medium);
    }

    #[test]
    fn extract_line_scanned_oracles_captures_helpers_without_general_asserts() {
        let facts = extract_line_scanned_oracles(
            r#"
assert_eq!(actual, expected);
mock_writer.verify();
assert_payload_eq(actual, expected);
expect_metric_recorded(counter);
"#,
            7,
        );

        let lines: Vec<usize> = facts.iter().map(|fact| fact.line).collect();
        assert_eq!(lines, vec![9, 10, 11]);
        assert_eq!(facts[0].kind, OracleKind::MockExpectation);
        assert_eq!(facts[1].kind, OracleKind::ExactValue);
        assert_eq!(facts[2].kind, OracleKind::MockExpectation);
    }

    #[test]
    fn contains_macro_invocation_requires_token_boundaries_and_invocation_suffix() {
        assert!(contains_macro_invocation(
            "assert_snapshot!{payload}",
            "assert_snapshot!"
        ));
        assert!(contains_macro_invocation(
            "insta::assert_snapshot![payload]",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "my_assert_snapshot!(payload)",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_snapshot_helper(payload)",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_snapshot!",
            "assert_snapshot!"
        ));
    }

    #[test]
    fn extract_assertions_keeps_line_numbers_and_observed_tokens() {
        let body = "let value = compute();\nassert_eq!(value, expected_total);\nvalue.unwrap();";

        let assertions = extract_assertions(body, 20);

        assert_eq!(assertions.len(), 2);
        assert_eq!(assertions[0].line, 21);
        assert_eq!(assertions[0].kind, OracleKind::ExactValue);
        assert_eq!(assertions[0].strength, OracleStrength::Strong);
        assert_eq!(
            assertions[0].observed_tokens,
            vec!["expected_total", "value"]
        );
        assert_eq!(assertions[1].line, 22);
        assert_eq!(assertions[1].kind, OracleKind::SmokeOnly);
        assert_eq!(assertions[1].strength, OracleStrength::Smoke);
    }

    #[test]
    fn classify_assertion_distinguishes_exact_and_broad_error_oracles() {
        let exact = classify_assertion("assert_matches!(result, Err(ConfigError::MissingPort))");
        let broad = classify_assertion("assert!(result.is_err())");

        assert_eq!(exact.kind, OracleKind::ExactErrorVariant);
        assert_eq!(exact.strength, OracleStrength::Strong);
        assert_eq!(broad.kind, OracleKind::BroadError);
        assert_eq!(broad.strength, OracleStrength::Weak);
    }

    #[test]
    fn classify_assertion_marks_duplicative_equality_as_weak() {
        let classification = classify_assertion("assert_eq!(&actual, actual)");

        assert_eq!(classification.kind, OracleKind::RelationalCheck);
        assert_eq!(classification.strength, OracleStrength::Weak);
    }

    #[test]
    fn classify_assertion_handles_nested_commas_in_custom_exact_helpers() {
        let classification = classify_assertion(
            "crate::helpers::assert_config_matches(actual, Config { ports: [80, 443] })",
        );

        assert_eq!(classification.kind, OracleKind::ExactValue);
        assert_eq!(classification.strength, OracleStrength::Strong);
    }

    #[test]
    fn contains_macro_invocation_requires_identifier_boundary_and_delimiter() {
        assert!(contains_macro_invocation(
            "insta::assert_json_snapshot!({\"ok\": true})",
            "assert_json_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "my_assert_json_snapshot!({\"ok\": true})",
            "assert_json_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_json_snapshot_helper({\"ok\": true})",
            "assert_json_snapshot!"
        ));
    }

    #[test]
    fn line_scanned_oracles_include_mock_expectations_without_macro_asserts() {
        let body = "mock_service.expect_publish().times(1);\nlet value = 1;";

        let oracles = extract_line_scanned_oracles(body, 7);

        assert_eq!(oracles.len(), 1);
        assert_eq!(oracles[0].line, 7);
        assert_eq!(oracles[0].kind, OracleKind::MockExpectation);
        assert_eq!(oracles[0].strength, OracleStrength::Medium);
    }
}
