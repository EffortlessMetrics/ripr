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
        let body = "let high = 42;\nlet low = -7;\nlet repeated = 42;";

        assert_eq!(extract_literals(body), ["-7", "42"]);
    }

    #[test]
    fn extract_literal_facts_preserves_line_numbers_and_ignores_bare_hyphens() {
        let body = "let range = 1 - 2;\nlet negative = -9;";

        let facts = extract_literal_facts(body, 40);

        assert_eq!(facts.len(), 3);
        assert_eq!(facts[0].line, 40);
        assert_eq!(facts[0].value, "1");
        assert_eq!(facts[1].line, 40);
        assert_eq!(facts[1].value, "2");
        assert_eq!(facts[2].line, 41);
        assert_eq!(facts[2].value, "-9");
    }
}
