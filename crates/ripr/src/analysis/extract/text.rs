pub(crate) fn extract_identifier_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if is_interesting_token(&current) {
                tokens.push(current.clone());
            }
            current.clear();
        }
    }
    if is_interesting_token(&current) {
        tokens.push(current);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn is_interesting_token(token: &str) -> bool {
    token.len() > 2
        && !matches!(
            token,
            "assert"
                | "assert_eq"
                | "assert_ne"
                | "assert_matches"
                | "let"
                | "mut"
                | "true"
                | "false"
                | "Some"
                | "None"
                | "Ok"
                | "Err"
                | "unwrap"
                | "expect"
                | "is_ok"
                | "is_err"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_identifier_tokens_returns_sorted_unique_interesting_tokens() {
        let tokens = extract_identifier_tokens(
            "assert_eq!(invoice.total_cents(), expected_total_cents); let total = total;",
        );

        assert_eq!(
            tokens,
            vec![
                "expected_total_cents".to_string(),
                "invoice".to_string(),
                "total".to_string(),
                "total_cents".to_string()
            ]
        );
    }

    #[test]
    fn extract_identifier_tokens_filters_short_and_builtin_assertion_words() {
        let tokens = extract_identifier_tokens("assert!(Ok(x).is_ok()); let id = true;");

        assert!(tokens.is_empty());
    }
}
