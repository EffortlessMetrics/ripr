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
    fn extract_return_facts_captures_explicit_and_result_option_returns() {
        let facts = extract_return_facts(
            "if invalid { return Err(Error::Invalid); }\nOk(total)\nSome(total)\nNone",
            21,
        );
        let rendered = facts
            .into_iter()
            .map(|fact| (fact.line, fact.text))
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                (21, "if invalid { return Err(Error::Invalid); }".to_string()),
                (22, "Ok(total)".to_string()),
                (23, "Some(total)".to_string()),
                (24, "None".to_string()),
            ]
        );
    }

    #[test]
    fn extract_return_facts_ignores_non_return_mentions() {
        let facts = extract_return_facts(
            "let return_value = total;\nlet token = \"Ok\";\nlet none_value = option;",
            3,
        );
        assert_eq!(facts, Vec::<ReturnFact>::new());
    }
}
