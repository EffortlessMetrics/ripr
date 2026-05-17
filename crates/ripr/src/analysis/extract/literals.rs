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
    use super::{extract_literal_facts, extract_literals};

    #[test]
    fn extract_literals_returns_sorted_unique_values() {
        let literals = extract_literals("let a = 20;\nlet b = -5;\nlet c = 20;\n");

        assert_eq!(literals, vec!["-5", "20"]);
    }

    #[test]
    fn extract_literal_facts_preserves_source_lines_and_ignores_lone_minus() {
        let facts = extract_literal_facts("let gap = end - start;\nlet x = -12;\nlet y = 7;\n", 41);
        let simplified = facts
            .iter()
            .map(|fact| (fact.line, fact.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(simplified, vec![(42, "-12"), (43, "7")]);
    }
}
