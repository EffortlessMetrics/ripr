use super::super::rust_index::LiteralFact;

pub(crate) fn extract_identifier_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| is_interesting_token(s))
        .map(String::from)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
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

pub(crate) fn extract_literals(body: &str) -> Vec<String> {
    let mut literals = extract_literal_facts(body, 1)
        .into_iter()
        .map(|literal| literal.value)
        .collect::<Vec<_>>();
    literals.sort();
    literals.dedup();
    literals
}

pub(crate) fn extract_literal_facts(body: &str, start_line: usize) -> Vec<LiteralFact> {
    let mut literals = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let mut current = String::new();
        let mut handle_token = |token: &str| {
            if !token.is_empty() && token != "-" {
                literals.push(LiteralFact {
                    line: start_line + offset,
                    value: token.to_string(),
                });
            }
        };
        for ch in line.chars() {
            if ch.is_ascii_digit() || (ch == '-' && current.is_empty()) {
                current.push(ch);
            } else if !current.is_empty() {
                handle_token(&current);
                current.clear();
            }
        }
        handle_token(&current);
    }
    literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
    literals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_helpers_are_callable() {
        let tokens = extract_identifier_tokens("let result = check_value()");
        assert!(!tokens.is_empty());
        assert!(tokens.contains(&"result".to_string()));
        assert!(tokens.contains(&"check_value".to_string()));

        let literals = extract_literals("let x = 42;\nlet y = 100;");
        assert!(literals.contains(&"42".to_string()));
        assert!(literals.contains(&"100".to_string()));
    }
}
