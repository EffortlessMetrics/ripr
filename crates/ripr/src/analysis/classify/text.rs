pub(in crate::analysis) fn exact_error_variant(text: &str) -> Option<String> {
    let start = text.find("Err(")?;
    let inner = delimited_contents_at(text, start + "Err".len())?;
    enum_variant_values(&inner).into_iter().next()
}

pub(in crate::analysis) fn delimited_contents_at(text: &str, open_index: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.get(open_index) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in text.char_indices().skip_while(|(idx, _)| *idx < open_index) {
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
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let start = open_index + 1;
                    return text.get(start..idx).map(ToString::to_string);
                }
            }
            _ => {}
        }
    }
    None
}

pub(in crate::analysis) fn enum_variant_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    for token in text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        if !token.contains("::") {
            continue;
        }
        let Some(last) = token.rsplit("::").next() else {
            continue;
        };
        if last
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            values.push(token.to_string());
        }
    }
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_error_variant_reads_first_variant_inside_result_error() {
        assert_eq!(
            exact_error_variant("return Err(AuthError::RevokedToken);").as_deref(),
            Some("AuthError::RevokedToken")
        );
    }

    #[test]
    fn exact_error_variant_returns_none_without_result_error() {
        assert_eq!(exact_error_variant("return Ok(value);"), None);
    }

    #[test]
    fn delimited_contents_at_handles_nested_calls_and_strings() {
        let text = r#"score(Ok("a)b"), other(1, 2))"#;

        let contents = delimited_contents_at(text, "score".len());

        assert_eq!(contents.as_deref(), Some(r#"Ok("a)b"), other(1, 2)"#));
    }

    #[test]
    fn delimited_contents_at_returns_none_for_non_delimiter_or_unclosed_text() {
        assert_eq!(delimited_contents_at("score(value)", 0), None);
        assert_eq!(delimited_contents_at("score(value", "score".len()), None);
    }

    #[test]
    fn enum_variant_values_returns_sorted_unique_variants() {
        let values = enum_variant_values(
            "Err(AuthError::RevokedToken) Err(AuthError::ExpiredToken) AuthError::RevokedToken",
        );

        assert_eq!(
            values,
            vec![
                "AuthError::ExpiredToken".to_string(),
                "AuthError::RevokedToken".to_string()
            ]
        );
    }

    #[test]
    fn enum_variant_values_ignores_lowercase_and_unqualified_tokens() {
        assert_eq!(
            enum_variant_values("err(auth_error::revoked) Revoked"),
            Vec::<String>::new()
        );
    }
}
