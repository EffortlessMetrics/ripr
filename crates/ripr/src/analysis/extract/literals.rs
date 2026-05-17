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
        let literals = extract_literals("let a = 42;\nlet b = -7;\nlet c = 42;");

        assert_eq!(literals, vec!["-7".to_string(), "42".to_string()]);
    }

    #[test]
    fn extract_literal_facts_preserves_line_numbers_and_ignores_lone_minus() {
        let facts = extract_literal_facts("let delta = left - right;\nlet retry_after = -10;", 20);

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].line, 21);
        assert_eq!(facts[0].value, "-10");
    }

    #[test]
    fn extract_literal_facts_deduplicates_same_value_on_same_line_only() {
        let facts = extract_literal_facts("assert_eq!(value, 5 + 5);\nassert_eq!(other, 5);", 3);

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].line, 3);
        assert_eq!(facts[0].value, "5");
        assert_eq!(facts[1].line, 4);
        assert_eq!(facts[1].value, "5");
    }
}
