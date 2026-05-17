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
    fn extract_return_facts_keeps_line_order_and_repeated_text_on_distinct_lines() {
        let facts = extract_return_facts(
            "if ok { return Ok(42); }\nlet value = Some(1);\nlet value = Some(1);\nNone",
            20,
        );

        let actual = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                (20, "if ok { return Ok(42); }"),
                (21, "let value = Some(1);"),
                (22, "let value = Some(1);"),
                (23, "None"),
            ]
        );
    }

    #[test]
    fn extract_return_facts_ignores_lines_without_return_shapes() {
        let facts = extract_return_facts(
            "let token = missing_value;\nlet error = no_error_here;\nlet option = something;",
            1,
        );

        assert_eq!(facts, Vec::new());
    }
}
