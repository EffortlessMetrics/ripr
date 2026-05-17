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
    fn extract_identifier_tokens_filters_rust_oracle_noise_and_sorts_unique_tokens() {
        let tokens = extract_identifier_tokens(
            "assert_eq!(quote.total, quote.discount_total); assert!(result.is_ok());",
        );

        assert_eq!(tokens, vec!["discount_total", "quote", "result", "total"]);
    }

    #[test]
    fn extract_identifier_tokens_keeps_unicode_alphanumeric_tokens() {
        let tokens = extract_identifier_tokens("assert_eq!(café_total, заказ_total);");

        assert_eq!(tokens, vec!["café_total", "заказ_total"]);
    }
}
