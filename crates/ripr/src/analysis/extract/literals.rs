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
    fn extract_literal_facts_preserves_source_lines_and_deduplicates_per_line() {
        let facts = extract_literal_facts("let a = 42;\nlet b = -7 + 42;\nlet c = -;", 10);

        let actual = facts
            .iter()
            .map(|fact| (fact.line, fact.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(actual, vec![(10, "42"), (11, "-7"), (11, "42")]);
    }

    #[test]
    fn extract_literals_returns_sorted_unique_values_across_body() {
        let literals = extract_literals("let a = 3;\nlet b = -1;\nlet c = 3;");

        assert_eq!(literals, vec!["-1", "3"]);
    }
}
