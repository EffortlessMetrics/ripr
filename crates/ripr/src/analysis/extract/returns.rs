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
    fn extract_return_facts_tracks_common_return_shapes() {
        let body = r#"
fn parse(input: &str) -> Result<Option<i32>, Error> {
    if input.is_empty() { return Ok(None); }
    if input == "x" { return Err(Error::Invalid); }
    Some(7)
}
"#;

        assert_eq!(
            extract_return_facts(body, 10),
            vec![
                ReturnFact {
                    line: 12,
                    text: "if input.is_empty() { return Ok(None); }".to_string(),
                },
                ReturnFact {
                    line: 13,
                    text: "if input == \"x\" { return Err(Error::Invalid); }".to_string(),
                },
                ReturnFact {
                    line: 14,
                    text: "Some(7)".to_string(),
                },
            ]
        );
    }

    #[test]
    fn extract_return_facts_keeps_repeated_text_on_distinct_lines() {
        let body = "return Ok(1);\nreturn Ok(1);";

        assert_eq!(extract_return_facts(body, 5).len(), 2);
    }
}
