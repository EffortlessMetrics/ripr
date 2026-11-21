use crate::analysis::facts::ReturnFact;

pub(crate) fn extract_return_facts(body: &str, start_line: usize) -> Vec<ReturnFact> {
    let mut returns = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("return ")
            || trimmed.contains(" return ")
            || trimmed.contains("Ok(")
            || trimmed.contains("Err(")
            || trimmed.contains("Some(")
            || trimmed.contains("None")
        {
            returns.push(ReturnFact {
                line: start_line + offset,
                text: trimmed.to_string(),
            });
        }
    }
    returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    returns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_return_facts_captures_result_option_and_explicit_returns() {
        let body =
            "if missing { return Err(ConfigError::Missing); }\nOk(Some(value))\nlet x = None;";

        let facts = extract_return_facts(body, 10);

        let lines_and_text = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            lines_and_text,
            vec![
                (10, "if missing { return Err(ConfigError::Missing); }"),
                (11, "Ok(Some(value))"),
                (12, "let x = None;"),
            ]
        );
    }

    #[test]
    fn extract_return_facts_sorts_and_deduplicates_duplicate_lines() {
        let facts = extract_return_facts("Ok(value)\nOk(value)", 3);

        let lines_and_text = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(lines_and_text, vec![(3, "Ok(value)"), (4, "Ok(value)")]);
    }

    #[test]
    fn extract_return_facts_ignores_substrings_without_return_signal() {
        let facts = extract_return_facts("let ok_value = status;\nlet none_count = 0;", 1);

        assert!(facts.is_empty());
    }
}
