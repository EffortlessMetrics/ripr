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
    fn extract_literals_returns_sorted_unique_values() {
        let body = "let low = -3;\nlet high = 42;\nlet again = 42;";

        let literals = extract_literals(body);

        assert_eq!(literals, vec!["-3".to_string(), "42".to_string()]);
    }

    #[test]
    fn extract_literal_facts_preserves_source_lines_and_ignores_bare_minus() {
        let body = "let range = 1..=3;\nlet value = -7;\nlet dash = a - b;";

        let facts = extract_literal_facts(body, 10);

        let line_values = facts
            .iter()
            .map(|fact| (fact.line, fact.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(line_values, vec![(10, "1"), (10, "3"), (11, "-7")]);
    }
}
