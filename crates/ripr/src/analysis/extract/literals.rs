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
        let literals = extract_literals("let high = 10;\nlet low = 2;\nlet repeat = 10;");
        assert_eq!(literals, vec!["10", "2"]);
    }

    #[test]
    fn extract_literal_facts_tracks_source_lines_and_negative_numbers() {
        let facts = extract_literal_facts(
            "let floor = -5;\nlet separator = value - other;\nlet ceiling = 10;",
            41,
        );
        let rendered = facts
            .into_iter()
            .map(|fact| (fact.line, fact.value))
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![(41, "-5".to_string()), (43, "10".to_string())]
        );
    }

    #[test]
    fn extract_literal_facts_deduplicates_same_value_on_same_line_only() {
        let facts = extract_literal_facts("let a = 7 + 7;\nlet b = 7;", 10);
        let rendered = facts
            .into_iter()
            .map(|fact| (fact.line, fact.value))
            .collect::<Vec<_>>();
        assert_eq!(rendered, vec![(10, "7".to_string()), (11, "7".to_string())]);
    }
}
