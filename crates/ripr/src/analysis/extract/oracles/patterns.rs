use super::arguments::{
    comparable_expression, custom_assertion_arguments, equality_assertion_arguments,
};
use crate::analysis::classify::{error_constructor_call_paths, rust_string_literals};

pub(super) fn is_snapshot_assertion(line: &str) -> bool {
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

pub(super) fn is_exact_error_variant_assertion(line: &str) -> bool {
    (line.contains("assert_matches!") || line.contains("matches!") || line.contains("assert_eq!"))
        && line.contains("Err(")
        && !line.contains("Err(_")
}

/// Returns true when `line` is an assertion on a variable known to hold an
/// unwrap_err result and pins a specific error result.
///
/// This recognizes the two-line pattern:
/// ```text
/// let err = f(-1).unwrap_err();
/// assert_eq!(err, MyError::Negative);          // ← this line
/// assert!(matches!(err, MyError::Negative));   // ← or this line
/// assert_eq!(err, MyError::new("negative"));   // ← or constructor equality
/// ```
/// `bound_error_vars` is the set of variable names bound by `.unwrap_err()`
/// or `.expect_err(...)` earlier in the same test body.
pub(crate) fn is_unwrap_err_bound_error_assertion(
    line: &str,
    bound_error_vars: &std::collections::BTreeSet<String>,
) -> bool {
    if bound_error_vars.is_empty() {
        return false;
    }
    // The line must be an assertion macro invocation.
    let is_assert = line.contains("assert_eq!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
        || line.contains("assert!");
    if !is_assert {
        return false;
    }
    if is_bound_error_equality_assertion(line, bound_error_vars) {
        return true;
    }
    // Must name at least one enum variant (SomeThing::Variant pattern with uppercase last component).
    if !contains_named_enum_variant(line) {
        return false;
    }
    // Must reference one of the known unwrap_err-bound variable names as a token.
    bound_error_vars
        .iter()
        .any(|var| line_references_variable(line, var))
}

fn is_bound_error_equality_assertion(
    line: &str,
    bound_error_vars: &std::collections::BTreeSet<String>,
) -> bool {
    let Some(args) = equality_assertion_arguments(line) else {
        return false;
    };
    let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
        return false;
    };
    let left = comparable_expression(left);
    let right = comparable_expression(right);
    bound_error_vars.iter().any(|var| {
        let var = comparable_expression(var);
        (left == var && expression_pins_specific_error(&right))
            || (right == var && expression_pins_specific_error(&left))
    })
}

fn expression_pins_specific_error(expression: &str) -> bool {
    contains_named_enum_variant(expression)
        || contains_error_constructor_call(expression)
        || contains_error_payload_literal(expression)
}

fn contains_error_constructor_call(expression: &str) -> bool {
    !error_constructor_call_paths(expression).is_empty()
}

fn contains_error_payload_literal(expression: &str) -> bool {
    rust_string_literals(expression)
        .iter()
        .any(|literal| literal_has_fixed_payload_text(literal))
}

fn literal_has_fixed_payload_text(literal: &str) -> bool {
    let mut fixed = String::new();
    let mut chars = literal.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    let _ = chars.next();
                    fixed.push('{');
                    continue;
                }
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    let _ = chars.next();
                    fixed.push('}');
                }
            }
            _ => fixed.push(ch),
        }
    }
    fixed.chars().any(|ch| ch.is_alphanumeric())
}

/// Returns true when the line contains a path-qualified enum variant:
/// at least one token of the form `Foo::Bar` where `Bar` starts with an
/// uppercase letter.
pub(crate) fn contains_named_enum_variant(line: &str) -> bool {
    // Split on non-identifier chars, find tokens containing "::" with an
    // uppercase final component.
    for token in line.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        if !token.contains("::") {
            continue;
        }
        if let Some(last) = token.rsplit("::").next()
            && last
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            return true;
        }
    }
    false
}

/// Returns true when `line` references `var` as a standalone identifier token
/// (not as a substring of a longer identifier).
fn line_references_variable(line: &str, var: &str) -> bool {
    line.match_indices(var).any(|(idx, _)| {
        let before_ok = idx == 0
            || !line[..idx]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let after_start = idx + var.len();
        let after_ok = after_start >= line.len()
            || !line[after_start..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        before_ok && after_ok
    })
}

pub(super) fn is_broad_error_assertion(line: &str) -> bool {
    line.contains("is_err") || line.contains("Err(_)")
}

pub(super) fn is_whole_object_equality_assertion(line: &str) -> bool {
    (line.contains("assert_eq!") || line.contains("assert_ne!")) && line.contains('{')
}

pub(super) fn is_duplicative_equality_assertion(line: &str) -> bool {
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

pub(super) fn is_duplicative_comparison(condition: &str) -> bool {
    let Some((operator, width)) = top_level_comparison_operator(condition) else {
        return false;
    };
    let left = condition[..operator].trim();
    let right = condition[operator + width..].trim();
    !left.is_empty()
        && !right.is_empty()
        && comparable_expression(left) == comparable_expression(right)
}

pub(super) fn is_exact_value_assertion(line: &str) -> bool {
    line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
}

pub(super) fn contains_exact_comparison(condition: &str) -> bool {
    let mut chars = condition.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
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
            '=' | '!' if matches!(chars.peek(), Some('=')) => return true,
            _ => {}
        }
    }
    false
}

fn top_level_comparison_operator(condition: &str) -> Option<(usize, usize)> {
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = condition.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
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
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '=' | '!'
                if paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0
                    && matches!(chars.peek(), Some((_, '='))) =>
            {
                return Some((index, 2));
            }
            _ => {}
        }
    }
    None
}

pub(super) fn is_mock_expectation_line(line: &str) -> bool {
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

pub(super) fn is_side_effect_observer_assertion(line: &str) -> bool {
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

pub(super) fn is_custom_assertion_helper(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.contains('!')
        && (trimmed.starts_with("assert_")
            || trimmed.contains("::assert_")
            || trimmed.contains(".assert_"))
        && trimmed.contains('(')
}

pub(super) fn is_clear_exact_custom_assertion_helper(line: &str) -> bool {
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

#[cfg(test)]
mod spec_0106_tests {
    use super::*;
    use std::collections::BTreeSet;

    fn vars(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // Control 1 (POSITIVE): unwrap_err binding + named variant → upgrade fires.
    #[test]
    fn is_unwrap_err_bound_error_assertion_upgrades_named_variant() {
        let bound = vars(&["err"]);
        assert!(
            is_unwrap_err_bound_error_assertion("assert_eq!(err, CalcError::Negative);", &bound),
            "exact variant assertion on bound var must be recognized"
        );
        assert!(
            is_unwrap_err_bound_error_assertion(
                "assert!(matches!(err, CalcError::Negative));",
                &bound
            ),
            "matches! variant assertion on bound var must be recognized"
        );
    }

    #[test]
    fn is_unwrap_err_bound_error_assertion_upgrades_constructor_payload_equality()
    -> Result<(), String> {
        let bound = vars(&["err"]);
        if !is_unwrap_err_bound_error_assertion(
            r#"assert_eq!(err, CargoAllowError::new(format!("duplicate allow id `{}`", id)));"#,
            &bound,
        ) {
            return Err(
                "exact constructor-payload equality on bound error must be recognized".to_string(),
            );
        }
        if is_unwrap_err_bound_error_assertion("assert_eq!(err, expected_error);", &bound) {
            return Err(
                "opaque expected variables must not be promoted to exact error variants"
                    .to_string(),
            );
        }
        if is_unwrap_err_bound_error_assertion(
            r#"assert_ne!(err, CargoAllowError::new(format!("duplicate allow id `{}`", id)));"#,
            &bound,
        ) {
            return Err("negative constructor-payload assertions must not be promoted".to_string());
        }
        if is_unwrap_err_bound_error_assertion("assert_ne!(err, CalcError::Negative);", &bound) {
            return Err("negative enum-variant assertions must not be promoted".to_string());
        }
        Ok(())
    }

    // Control 3 (GENERIC): generic assertion without variant token → no upgrade.
    #[test]
    fn is_unwrap_err_bound_error_assertion_rejects_placeholder_only_string_payload()
    -> Result<(), String> {
        let bound = vars(&["err"]);
        let cases = [
            (
                "placeholder-only",
                r#"assert_eq!(err, format!("{}", id));"#,
                false,
            ),
            (
                "escaped-placeholder-only",
                r#"assert_eq!(err, format!("{{}} {}", id));"#,
                false,
            ),
            (
                "fixed-text",
                r#"assert_eq!(err, format!("duplicate allow id `{}`", id));"#,
                true,
            ),
            (
                "escaped-fixed-text",
                r#"assert_eq!(err, format!("{{duplicate}} {}", id));"#,
                true,
            ),
        ];
        for (label, assertion, expected) in cases {
            let actual = is_unwrap_err_bound_error_assertion(assertion, &bound);
            if actual != expected {
                return Err(format!("{label}: expected {expected}, got {actual}"));
            }
        }
        Ok(())
    }

    #[test]
    fn generic_assertion_on_bound_var_not_upgraded() {
        let bound = vars(&["err"]);
        assert!(
            !is_unwrap_err_bound_error_assertion(
                "assert!(err.to_string().contains(\"error\"));",
                &bound
            ),
            "generic string-contains assertion must not be treated as ExactErrorVariant"
        );
        assert!(
            !is_unwrap_err_bound_error_assertion("assert!(result.is_err());", &bound),
            "is_err assertion must not be treated as ExactErrorVariant"
        );
    }

    // Empty bound_vars guard — must return false without accessing bound_vars.
    #[test]
    fn empty_bound_vars_returns_false() {
        let empty = vars(&[]);
        assert!(
            !is_unwrap_err_bound_error_assertion("assert_eq!(err, CalcError::Negative);", &empty),
            "empty bound_vars must short-circuit to false"
        );
    }

    // Variable must be referenced as a token (not a substring).
    #[test]
    fn variable_must_be_token_not_substring() {
        let bound = vars(&["err"]);
        // "cerr" contains "err" as a substring — must not match.
        assert!(
            !is_unwrap_err_bound_error_assertion("assert_eq!(cerr, CalcError::Negative);", &bound),
            "substring match of variable must not fire"
        );
    }

    #[test]
    fn contains_named_enum_variant_recognizes_qualified_variant() {
        assert!(
            contains_named_enum_variant("assert_eq!(err, CalcError::Negative);"),
            "CalcError::Negative must be recognized"
        );
        assert!(
            !contains_named_enum_variant("assert!(err.to_string().contains(\"error\"));"),
            "no qualified variant — must return false"
        );
    }

    #[test]
    fn exact_comparison_ignores_string_contents_and_escapes() -> Result<(), String> {
        for (condition, expected) in [
            ("state == TerminalState::Pass", true),
            ("state != TerminalState::Pending", true),
            (r#"label.contains("==")"#, false),
            (r#"label.contains("escaped \"!=\" text")"#, false),
            ("ready = true", false),
        ] {
            let actual = contains_exact_comparison(condition);
            if actual != expected {
                return Err(format!(
                    "comparison classification mismatch for {condition}: {actual}"
                ));
            }
        }
        Ok(())
    }
}
