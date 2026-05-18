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
        let values = extract_literals("let a = 42;\nlet b = -7;\nlet c = 42;");

        assert_eq!(values, vec!["-7".to_string(), "42".to_string()]);
    }

    #[test]
    fn extract_literal_facts_preserves_source_lines_and_ignores_bare_minus() {
        let facts = extract_literal_facts("let a = -;\nlet b = -12;\nlet c = 3 + 3;", 40);

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].line, 41);
        assert_eq!(facts[0].value, "-12");
        assert_eq!(facts[1].line, 42);
        assert_eq!(facts[1].value, "3");
    }

    #[test]
    fn extract_literal_facts_deduplicates_same_value_on_same_line_only() {
        let facts = extract_literal_facts("let a = 9 + 9;\nlet b = 9;", 1);

        let values_by_line = facts
            .iter()
            .map(|fact| (fact.line, fact.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(values_by_line, vec![(1, "9"), (2, "9")]);
    }
}
