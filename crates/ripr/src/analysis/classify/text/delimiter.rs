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

#[cfg(test)]
mod tests {
    use super::*;

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
}
