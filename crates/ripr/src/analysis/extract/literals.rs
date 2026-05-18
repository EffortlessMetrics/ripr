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
    fn extract_literals_sorts_and_deduplicates_values() {
        let body = "let retry = 3;\nlet copy = 3;\nlet penalty = -2;";

        assert_eq!(extract_literals(body), vec!["-2", "3"]);
    }

    #[test]
    fn extract_literal_facts_keep_line_context_and_skip_bare_minus() {
        let body = "let range = 10 - value;\nlet adjustment = -4;";

        assert_eq!(
            extract_literal_facts(body, 41),
            vec![
                LiteralFact {
                    line: 41,
                    value: "10".to_string(),
                },
                LiteralFact {
                    line: 42,
                    value: "-4".to_string(),
                },
            ]
        );
    }
}
