use serde_json::Value;

pub(crate) fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(crate) fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn path_value_returns_nested_value_or_none() {
        let value = json!({ "a": { "b": 1 } });

        assert_eq!(path_value(&value, &["a", "b"]), Some(&json!(1)));
        assert!(path_value(&value, &["a", "missing"]).is_none());
        assert!(path_value(&value, &["missing"]).is_none());
    }

    #[test]
    fn string_path_returns_owned_string_only_for_string_values() {
        let value = json!({ "a": { "b": "c" }, "count": 1 });

        assert_eq!(string_path(&value, &["a", "b"]), Some("c".to_string()));
        assert!(string_path(&value, &["count"]).is_none());
    }
}
