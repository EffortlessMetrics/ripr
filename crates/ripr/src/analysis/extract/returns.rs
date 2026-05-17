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
    fn return_facts_capture_explicit_and_wrapper_returns_with_lines() {
        let body = r#"
let setup = 1;
if ready { return Ok(value); }
Err(Error::Denied)
Some(fallback)
None
"#;

        let facts = extract_return_facts(body, 10);

        assert_eq!(
            facts,
            vec![
                ReturnFact {
                    line: 12,
                    text: "if ready { return Ok(value); }".to_string(),
                },
                ReturnFact {
                    line: 13,
                    text: "Err(Error::Denied)".to_string(),
                },
                ReturnFact {
                    line: 14,
                    text: "Some(fallback)".to_string(),
                },
                ReturnFact {
                    line: 15,
                    text: "None".to_string(),
                },
            ]
        );
    }

    #[test]
    fn return_facts_keep_broad_none_matches_and_are_stable() {
        let body = r#"let value = compute();
let note = "None of this returns";
return value;
return value;"#;

        let facts = extract_return_facts(body, 3);

        assert_eq!(
            facts,
            vec![
                ReturnFact {
                    line: 4,
                    text: "let note = \"None of this returns\";".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "return value;".to_string(),
                },
                ReturnFact {
                    line: 6,
                    text: "return value;".to_string(),
                },
            ]
        );
    }
}
