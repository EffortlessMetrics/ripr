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
    fn extract_identifier_tokens_sorts_deduplicates_and_filters_assertion_noise() {
        let tokens = extract_identifier_tokens(
            "assert_eq!(apply_discount(total), expected_total); let total = Some(42);",
        );
        assert_eq!(tokens, vec!["apply_discount", "expected_total", "total"]);
    }

    #[test]
    fn extract_identifier_tokens_keeps_unicode_and_underscored_identifiers() {
        let tokens = extract_identifier_tokens("let café_total = price + 税率 + id;");
        assert_eq!(tokens, vec!["café_total", "price", "税率"]);
    }
}
