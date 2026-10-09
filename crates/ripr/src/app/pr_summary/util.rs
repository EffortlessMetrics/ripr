use crate::output::markdown::code_span;
use serde_json::Value;

/// Walk a JSON value along the given path segments.
/// Returns `None` if any segment is missing or if `value` is `None`.
pub(super) fn value_path<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    let mut current = value?;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

/// The string at `key` as one code span, or `not_available`.
pub(super) fn string_field(value: Option<&Value>, key: &str) -> String {
    code_span(
        value
            .and_then(|value| value.get(key))
            .and_then(Value::as_str)
            .unwrap_or("not_available"),
    )
}

pub(super) fn summary_u64(value: Option<&Value>, key: &str) -> String {
    summary_field(value, key)
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

pub(super) fn summary_bool(value: Option<&Value>, key: &str) -> String {
    summary_field(value, key)
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

/// The summary string at `key` as one code span; `none` when null.
pub(super) fn summary_string_or_null(value: Option<&Value>, key: &str) -> String {
    let Some(value) = summary_field(value, key) else {
        return code_span("not_available");
    };
    if value.is_null() {
        code_span("none")
    } else {
        code_span(value.as_str().unwrap_or("invalid"))
    }
}

fn summary_field<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    value
        .and_then(|value| value.get("summary"))
        .and_then(|summary| summary.get(key))
}

pub(super) fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or(value).trim().to_string()
}
