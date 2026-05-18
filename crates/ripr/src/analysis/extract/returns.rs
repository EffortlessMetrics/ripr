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
    fn extract_return_facts_finds_explicit_and_result_like_returns() {
        let body = "let value = compute();\nreturn value;\nOk(value)\nErr(Error::Missing)";

        let facts = extract_return_facts(body, 20);

        let lines_and_text = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            lines_and_text,
            vec![
                (21, "return value;"),
                (22, "Ok(value)"),
                (23, "Err(Error::Missing)")
            ]
        );
    }

    #[test]
    fn extract_return_facts_sorts_and_deduplicates_by_line_and_text() {
        let body = "Some(value)\nSome(value)\nNone";

        let facts = extract_return_facts(body, 1);

        let lines_and_text = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            lines_and_text,
            vec![(1, "Some(value)"), (2, "Some(value)"), (3, "None")]
        );
    }
}
