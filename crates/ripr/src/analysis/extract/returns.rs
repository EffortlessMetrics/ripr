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
    use super::extract_return_facts;

    #[test]
    fn extracts_explicit_result_option_and_none_return_shapes() {
        let body = r#"fn classify(value: i32) -> Option<Result<i32, Error>> {
    if value < 0 { return None; }
    if value == 0 { Some(Ok(0)) } else { Some(Err(Error::Bad)) }
}"#;

        let facts = extract_return_facts(body, 20);
        let simplified = facts
            .iter()
            .map(|fact| (fact.line, fact.text.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            simplified,
            vec![
                (21, "if value < 0 { return None; }"),
                (
                    22,
                    "if value == 0 { Some(Ok(0)) } else { Some(Err(Error::Bad)) }"
                )
            ]
        );
    }

    #[test]
    fn deduplicates_same_return_text_on_same_line_but_keeps_later_lines() {
        let body = "return Ok(1);\nreturn Ok(1);\n";

        let facts = extract_return_facts(body, 7);

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].line, 7);
        assert_eq!(facts[1].line, 8);
    }
}
