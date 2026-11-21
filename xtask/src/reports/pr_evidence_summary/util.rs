use serde_json::Value;

pub(super) fn string_field(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(md_escape)
        .unwrap_or_else(|| "not_available".to_string())
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

pub(super) fn summary_string_or_null(value: Option<&Value>, key: &str) -> String {
    let Some(value) = summary_field(value, key) else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "none".to_string()
    } else {
        value
            .as_str()
            .map(md_escape)
            .unwrap_or_else(|| "invalid".to_string())
    }
}

fn summary_field<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    value
        .and_then(|value| value.get("summary"))
        .and_then(|summary| summary.get(key))
}

pub(super) fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

pub(super) fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or(value).trim().to_string()
}
