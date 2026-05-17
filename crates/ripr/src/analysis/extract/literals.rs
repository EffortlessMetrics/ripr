use crate::analysis::facts::LiteralFact;

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
        for ch in line.chars() {
            if ch.is_ascii_digit() || (ch == '-' && current.is_empty()) {
                current.push(ch);
            } else if !current.is_empty() {
                if current != "-" {
                    literals.push(LiteralFact {
                        line: start_line + offset,
                        value: current.clone(),
                    });
                }
                current.clear();
            }
        }
        if !current.is_empty() && current != "-" {
            literals.push(LiteralFact {
                line: start_line + offset,
                value: current,
            });
        }
    }
    literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
    literals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_values_are_sorted_and_deduplicated() {
        let body = r#"
let high = 20;
let low = -5;
let repeated = 20;
"#;

        assert_eq!(extract_literals(body), vec!["-5", "20"]);
    }

    #[test]
    fn literal_facts_keep_source_lines_and_remove_same_line_duplicates() {
        let body = "let pair = (7, 7);
let next = 7;
let negative = -3;
let dash = left - right;";

        let facts = extract_literal_facts(body, 40);

        assert_eq!(
            facts,
            vec![
                LiteralFact {
                    line: 40,
                    value: "7".to_string(),
                },
                LiteralFact {
                    line: 41,
                    value: "7".to_string(),
                },
                LiteralFact {
                    line: 42,
                    value: "-3".to_string(),
                },
            ]
        );
    }
}
