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
        let mut cursor = 0;
        while cursor < line.len() {
            if let Some((literal, next_cursor)) = numeric_literal_at(line, cursor) {
                literals.push(LiteralFact {
                    line: start_line + offset,
                    value: literal,
                });
                cursor = next_cursor;
            } else if let Some(ch) = line[cursor..].chars().next() {
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
    literals
}

fn numeric_literal_at(line: &str, cursor: usize) -> Option<(String, usize)> {
    let rest = line.get(cursor..)?;
    let first = rest.chars().next()?;
    let negative = first == '-';
    if negative && !is_unary_minus_context(line, cursor) {
        return None;
    }

    let mut current = cursor + if negative { first.len_utf8() } else { 0 };
    let digit = line.get(current..)?.chars().next()?;
    if !digit.is_ascii_digit() || is_identifier_tail_before(line, cursor) {
        return None;
    }

    let numeric_start = cursor;
    if digit == '0' {
        let prefix_cursor = current + digit.len_utf8();
        if let Some((radix, prefix_len)) = radix_prefix(line, prefix_cursor) {
            let digit_start = prefix_cursor + prefix_len;
            let digit_end = consume_digits_and_underscores(line, digit_start, radix);
            if contains_digit_for_radix(&line[digit_start..digit_end], radix) {
                let numeric_end = digit_end;
                let next_cursor = consume_literal_suffix(line, digit_end);
                let value = canonical_radix_literal(&line[numeric_start..numeric_end]);
                return Some((value, next_cursor));
            }
            return None;
        }
    }

    current = consume_digits_and_underscores(line, current, 10);

    if line
        .get(current..)
        .is_some_and(|suffix| suffix.starts_with('.'))
        && line
            .get(current + 1..)
            .and_then(|suffix| suffix.chars().next())
            .is_some_and(|ch| ch.is_ascii_digit())
    {
        current += 1;
        current = consume_digits_and_underscores(line, current, 10);
    }

    if line
        .get(current..)
        .and_then(|suffix| suffix.chars().next())
        .is_some_and(|ch| matches!(ch, 'e' | 'E'))
    {
        let exponent_start = current;
        let mut exponent_cursor = current + 1;
        if line
            .get(exponent_cursor..)
            .and_then(|suffix| suffix.chars().next())
            .is_some_and(|ch| matches!(ch, '+' | '-'))
        {
            exponent_cursor += 1;
        }
        let exponent_digits = consume_digits_and_underscores(line, exponent_cursor, 10);
        if contains_digit_for_radix(&line[exponent_cursor..exponent_digits], 10) {
            current = exponent_digits;
        } else {
            current = exponent_start;
        }
    }

    let numeric_end = current;
    let next_cursor = consume_literal_suffix(line, current);
    let value = canonical_decimal_literal(&line[numeric_start..numeric_end]);
    Some((value, next_cursor))
}

fn consume_digits_and_underscores(line: &str, mut cursor: usize, radix: u32) -> usize {
    while let Some(ch) = line.get(cursor..).and_then(|suffix| suffix.chars().next()) {
        if ch == '_' || ch.is_digit(radix) {
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    cursor
}

fn consume_literal_suffix(line: &str, mut cursor: usize) -> usize {
    while let Some(ch) = line.get(cursor..).and_then(|suffix| suffix.chars().next()) {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    cursor
}

fn contains_digit_for_radix(value: &str, radix: u32) -> bool {
    value.chars().any(|ch| ch.is_digit(radix))
}

fn radix_prefix(line: &str, cursor: usize) -> Option<(u32, usize)> {
    let suffix = line.get(cursor..)?;
    if suffix.starts_with('x') || suffix.starts_with('X') {
        Some((16, 1))
    } else if suffix.starts_with('o') || suffix.starts_with('O') {
        Some((8, 1))
    } else if suffix.starts_with('b') || suffix.starts_with('B') {
        Some((2, 1))
    } else {
        None
    }
}

fn canonical_decimal_literal(raw: &str) -> String {
    raw.chars().filter(|ch| *ch != '_').collect()
}

fn canonical_radix_literal(raw: &str) -> String {
    raw.chars()
        .filter(|ch| *ch != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_unary_minus_context(line: &str, cursor: usize) -> bool {
    line[..cursor]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .is_none_or(|ch| {
            matches!(
                ch,
                '(' | '['
                    | '{'
                    | ','
                    | '='
                    | '!'
                    | '<'
                    | '>'
                    | '&'
                    | '|'
                    | '+'
                    | '-'
                    | '*'
                    | '/'
                    | '%'
                    | ':'
                    | ';'
            )
        })
}

fn is_identifier_tail_before(line: &str, cursor: usize) -> bool {
    line[..cursor]
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
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

    #[test]
    fn extract_literals_ignores_malformed_radix_prefix_without_digits() {
        let values = extract_literals("let bad = 0b_cnt + 0x_ + 0o_mode;");

        assert!(values.is_empty());
    }

    #[test]
    fn extract_literals_handles_radix_prefixes_suffixes_and_case() {
        let values =
            extract_literals("let flags = 0B1010usize + 0O77_u8 + 0Xff_i32 + 0b_1010_u16;");

        assert_eq!(values, vec!["0b1010", "0o77", "0xff"]);
    }

    #[test]
    fn extract_literals_handles_decimal_float_exponents_and_suffixes() {
        let values = extract_literals("let ratio = 1_000.50f64 + 2e+3_f32 + 4E-2 + 9e_name;");

        assert_eq!(values, vec!["1000.50", "2e+3", "4E-2", "9"]);
    }

    #[test]
    fn extract_literals_respects_identifier_and_unary_boundaries() {
        let values =
            extract_literals("let value9 = total - discount + foo_42 + (-7) + [ -8, amount*-9 ];");

        assert_eq!(values, vec!["-7", "-8", "-9"]);
    }

    #[test]
    fn extract_literals_ignores_invalid_radix_without_later_digit_leakage() {
        let values = extract_literals("let bad = 0b_2 + 0x_g1 + 0o_8;");

        assert!(values.is_empty());
    }
}
