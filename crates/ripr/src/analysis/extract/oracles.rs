use crate::analysis::facts::OracleFact;
use crate::domain::{OracleKind, OracleStrength};

use super::text::extract_identifier_tokens;

pub(crate) fn extract_assertions(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if is_assertion_line(trimmed) {
            let classification = classify_assertion(trimmed);
            out.push(OracleFact {
                line: start_line + offset,
                text: trimmed.to_string(),
                kind: classification.kind,
                strength: classification.strength,
                observed_tokens: extract_identifier_tokens(trimmed),
            });
        }
    }
    out
}

fn is_assertion_line(line: &str) -> bool {
    line.contains("assert!")
        || line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
        || is_snapshot_assertion(line)
        || is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || line.contains("expect_")
        || line.contains(".expect(")
        || line.contains(".unwrap(")
        || line.contains("should_panic")
}

pub(crate) fn extract_line_scanned_oracles(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if !is_line_scanned_oracle(trimmed) {
            continue;
        }
        let classification = classify_assertion(trimmed);
        out.push(OracleFact {
            line: start_line + offset,
            text: trimmed.to_string(),
            kind: classification.kind,
            strength: classification.strength,
            observed_tokens: extract_identifier_tokens(trimmed),
        });
    }
    out
}

fn is_line_scanned_oracle(line: &str) -> bool {
    is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || is_mock_expectation_line(line)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OracleClassification {
    pub(crate) kind: OracleKind,
    pub(crate) strength: OracleStrength,
}

pub(crate) fn classify_assertion(line: &str) -> OracleClassification {
    if is_exact_error_variant_assertion(line) {
        OracleClassification {
            kind: OracleKind::ExactErrorVariant,
            strength: OracleStrength::Strong,
        }
    } else if is_broad_error_assertion(line) {
        OracleClassification {
            kind: OracleKind::BroadError,
            strength: OracleStrength::Weak,
        }
    } else if is_duplicative_equality_assertion(line) {
        OracleClassification {
            kind: OracleKind::RelationalCheck,
            strength: OracleStrength::Weak,
        }
    } else if is_whole_object_equality_assertion(line) {
        OracleClassification {
            kind: OracleKind::WholeObjectEquality,
            strength: OracleStrength::Strong,
        }
    } else if is_exact_value_assertion(line) {
        OracleClassification {
            kind: OracleKind::ExactValue,
            strength: OracleStrength::Strong,
        }
    } else if is_snapshot_assertion(line) {
        OracleClassification {
            kind: OracleKind::Snapshot,
            strength: OracleStrength::Medium,
        }
    } else if line.contains(".unwrap(")
        || line.contains(".expect(")
        || line.contains("is_ok")
        || line.contains("is_some")
        || line.contains("is_none")
    {
        OracleClassification {
            kind: OracleKind::SmokeOnly,
            strength: OracleStrength::Smoke,
        }
    } else if is_mock_expectation_line(line) || is_side_effect_observer_assertion(line) {
        OracleClassification {
            kind: OracleKind::MockExpectation,
            strength: OracleStrength::Medium,
        }
    } else if is_clear_exact_custom_assertion_helper(line) {
        OracleClassification {
            kind: OracleKind::ExactValue,
            strength: OracleStrength::Strong,
        }
    } else if is_custom_assertion_helper(line) {
        OracleClassification {
            kind: OracleKind::Unknown,
            strength: OracleStrength::Unknown,
        }
    } else if line.contains("> 0")
        || line.contains("<")
        || line.contains(">")
        || line.contains("is_empty")
        || line.contains("contains")
        || line.contains("assert!")
    {
        OracleClassification {
            kind: OracleKind::RelationalCheck,
            strength: OracleStrength::Weak,
        }
    } else {
        OracleClassification {
            kind: OracleKind::Unknown,
            strength: OracleStrength::Unknown,
        }
    }
}

fn is_snapshot_assertion(line: &str) -> bool {
    let expect_test_comparison = (line.contains("expect![[") || line.contains("expect_file!["))
        && (line.contains(".assert_eq(")
            || line.contains(".assert_debug_eq(")
            || line.contains(".assert_json_eq("));
    let known_snapshot_macros = [
        "assert_snapshot!",
        "assert_yaml_snapshot!",
        "assert_json_snapshot!",
        "assert_debug_snapshot!",
        "assert_display_snapshot!",
        "assert_csv_snapshot!",
        "assert_ron_snapshot!",
        "assert_toml_snapshot!",
        "assert_compact_debug_snapshot!",
        "assert_compact_json_snapshot!",
        "assert_binary_snapshot!",
    ];
    known_snapshot_macros
        .iter()
        .any(|macro_name| contains_macro_invocation(line, macro_name))
        || expect_test_comparison
}

pub(crate) fn contains_macro_invocation(line: &str, macro_name: &str) -> bool {
    line.match_indices(macro_name).any(|(index, _)| {
        let prefix_ok = index == 0
            || !line[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let suffix_start = index + macro_name.len();
        let suffix_ok = line[suffix_start..]
            .trim_start()
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, '(' | '[' | '{'));
        prefix_ok && suffix_ok
    })
}

fn is_exact_error_variant_assertion(line: &str) -> bool {
    (line.contains("assert_matches!") || line.contains("matches!") || line.contains("assert_eq!"))
        && line.contains("Err(")
        && !line.contains("Err(_")
}

fn is_broad_error_assertion(line: &str) -> bool {
    line.contains("is_err") || line.contains("Err(_)")
}

fn is_whole_object_equality_assertion(line: &str) -> bool {
    (line.contains("assert_eq!") || line.contains("assert_ne!")) && line.contains('{')
}

fn is_duplicative_equality_assertion(line: &str) -> bool {
    let Some(args) = equality_assertion_arguments(line) else {
        return false;
    };
    let Some(left) = args.first() else {
        return false;
    };
    let Some(right) = args.get(1) else {
        return false;
    };
    comparable_expression(left) == comparable_expression(right)
}

fn equality_assertion_arguments(line: &str) -> Option<Vec<String>> {
    ["assert_eq!", "assert_ne!"]
        .iter()
        .find_map(|macro_name| macro_invocation_arguments(line, macro_name))
}

fn is_exact_value_assertion(line: &str) -> bool {
    line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
}

fn is_mock_expectation_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let has_expectation_call = lower.contains("expect_") && lower.contains('(');
    let has_mock_verification_call = lower.contains("mock")
        && [
            ".assert_",
            ".checkpoint(",
            ".times(",
            ".verify(",
            "assert_expectations(",
        ]
        .iter()
        .any(|token| lower.contains(token));
    has_expectation_call || has_mock_verification_call
}

fn is_side_effect_observer_assertion(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let has_observer_token = [
        "event",
        "emitted",
        "published",
        "sent",
        "saved",
        "persist",
        "state",
        "stored",
        "metric",
        "counter",
        "recorded",
    ]
    .iter()
    .any(|token| lower.contains(token));
    has_observer_token && (lower.contains("assert") || lower.contains("expect"))
}

fn is_custom_assertion_helper(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.contains('!')
        && (trimmed.starts_with("assert_")
            || trimmed.contains("::assert_")
            || trimmed.contains(".assert_"))
        && trimmed.contains('(')
}

fn is_clear_exact_custom_assertion_helper(line: &str) -> bool {
    if !is_custom_assertion_helper(line) {
        return false;
    }
    let Some(name) = custom_assertion_helper_name(line) else {
        return false;
    };
    let Some(arguments) = custom_assertion_arguments(line) else {
        return false;
    };
    let argument_count_supports_exact = if line.contains(".assert_") {
        !arguments.is_empty()
    } else {
        arguments.len() >= 2
    };
    argument_count_supports_exact
        && (name.contains("_eq")
            || name.contains("_equal")
            || name.contains("_matches")
            || name.ends_with("eq")
            || name.ends_with("equal")
            || name.ends_with("matches"))
}

fn custom_assertion_helper_name(line: &str) -> Option<String> {
    let before_args = line.split_once('(')?.0.trim();
    let name = before_args
        .rsplit([':', '.'])
        .find(|part| !part.is_empty())?
        .trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_ascii_lowercase())
    }
}

fn custom_assertion_arguments(line: &str) -> Option<Vec<String>> {
    let open = line.find('(')?;
    delimited_contents_at(line, open).map(|contents| split_top_level_commas(&contents))
}

fn macro_invocation_arguments(line: &str, macro_name: &str) -> Option<Vec<String>> {
    line.match_indices(macro_name)
        .filter_map(|(index, _)| {
            let prefix_ok = index == 0
                || !line[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
            let suffix_start = index + macro_name.len();
            let open_offset = line[suffix_start..]
                .char_indices()
                .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some((offset, ch)))?;
            if !prefix_ok || open_offset.1 != '(' {
                return None;
            }
            let open = suffix_start + open_offset.0;
            delimited_contents_at(line, open).map(|contents| split_top_level_commas(&contents))
        })
        .next()
}

fn delimited_contents_at(text: &str, open_index: usize) -> Option<String> {
    let open = text.as_bytes().get(open_index).copied()?;
    if open != b'(' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut content_start = None;
    for (offset, ch) in text[open_index..].char_indices() {
        let index = open_index + offset;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => {
                depth += 1;
                if depth == 1 {
                    content_start = Some(index + ch.len_utf8());
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let start = content_start?;
                    return Some(text[start..index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                args.push(text[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        args.push(tail.to_string());
    }
    args
}

fn comparable_expression(expression: &str) -> String {
    expression
        .split_whitespace()
        .collect::<String>()
        .trim_start_matches('&')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_assertion_distinguishes_exact_error_from_broad_error() {
        let exact = classify_assertion("assert_matches!(result, Err(ConfigError::MissingKey(_)))");
        assert_eq!(exact.kind, OracleKind::ExactErrorVariant);
        assert_eq!(exact.strength, OracleStrength::Strong);

        let broad = classify_assertion("assert!(result.is_err())");
        assert_eq!(broad.kind, OracleKind::BroadError);
        assert_eq!(broad.strength, OracleStrength::Weak);
    }

    #[test]
    fn classify_assertion_downgrades_duplicative_equality() {
        let classification = classify_assertion("assert_eq!(actual.total(), actual.total())");

        assert_eq!(classification.kind, OracleKind::RelationalCheck);
        assert_eq!(classification.strength, OracleStrength::Weak);
    }

    #[test]
    fn classify_assertion_recognizes_exact_custom_helper_names() {
        let free_function = classify_assertion("assert_price_eq(actual.total(), 42)");
        assert_eq!(free_function.kind, OracleKind::ExactValue);
        assert_eq!(free_function.strength, OracleStrength::Strong);

        let method_helper = classify_assertion("response.assert_matches(Status::Accepted)");
        assert_eq!(method_helper.kind, OracleKind::ExactValue);
        assert_eq!(method_helper.strength, OracleStrength::Strong);
    }

    #[test]
    fn extract_assertions_preserves_source_lines_and_observed_tokens() {
        let body = r#"let result = apply_discount(order);
assert_eq!(result.total_cents(), 4200);
let smoke = result.receipt().expect("receipt should exist");
"#;

        let assertions = extract_assertions(body, 10);

        assert_eq!(assertions.len(), 2);
        assert_eq!(assertions[0].line, 11);
        assert_eq!(assertions[0].kind, OracleKind::ExactValue);
        assert_eq!(assertions[0].strength, OracleStrength::Strong);
        assert!(
            assertions[0]
                .observed_tokens
                .contains(&"result".to_string())
        );
        assert!(
            assertions[0]
                .observed_tokens
                .contains(&"total_cents".to_string())
        );
        assert_eq!(assertions[1].line, 12);
        assert_eq!(assertions[1].kind, OracleKind::SmokeOnly);
        assert_eq!(assertions[1].strength, OracleStrength::Smoke);
    }

    #[test]
    fn contains_macro_invocation_rejects_identifier_substrings() {
        assert!(contains_macro_invocation(
            "assert_snapshot!(value)",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "my_assert_snapshot!(value)",
            "assert_snapshot!"
        ));
    }
}
