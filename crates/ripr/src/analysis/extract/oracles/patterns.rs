use super::arguments::{
    comparable_expression, custom_assertion_arguments, equality_assertion_arguments,
};

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

pub(super) fn is_exact_value_assertion(line: &str) -> bool {
    line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
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
