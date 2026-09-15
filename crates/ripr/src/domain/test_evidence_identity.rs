//! Canonical missing-discriminator identity for portable test evidence.
//!
//! Discriminator values are Rust-shaped expressions. Formatting whitespace
//! may move without changing the behavior, while whitespace inside literals is
//! behavior. Keep that distinction local to the producer instead of asking
//! renderers or cross-tool consumers to reconstruct it.

/// Normalize, order, and deduplicate producer-owned discriminator identities.
pub(super) fn canonicalize_missing_discriminators(values: &[String]) -> Vec<String> {
    let mut canonical = values
        .iter()
        .map(|value| normalize_discriminator_text(value))
        .collect::<Vec<_>>();
    canonical.retain(|value| !value.is_empty());
    canonical.sort();
    canonical.dedup();
    canonical
}

/// Encode arbitrary discriminator text without delimiter collisions.
///
/// Each sorted item is byte-length-prefixed. Values containing `;`, `:`, or
/// the fingerprint's own marker remain uniquely decodable.
pub(super) fn encode_missing_discriminators(values: &[String]) -> String {
    let mut encoded = String::new();
    for value in values {
        encoded.push_str(&value.len().to_string());
        encoded.push(':');
        encoded.push_str(value);
    }
    encoded
}

/// Normalize formatting whitespace without rewriting Rust literal contents.
///
/// Whitespace between tokens is formatting; whitespace inside normal, byte,
/// C, raw, or character literals is behavior. Lifetimes are not treated as
/// character literals unless the following token has a character-literal
/// shape.
fn normalize_discriminator_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut pending_space = false;

    while let Some(character) = chars.next() {
        if character.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;

        match character {
            '"' => {
                normalized.push(character);
                copy_quoted_literal(&mut chars, &mut normalized, character);
            }
            '\'' if starts_character_literal(chars.clone()) => {
                normalized.push(character);
                copy_quoted_literal(&mut chars, &mut normalized, character);
            }
            'r' => {
                normalized.push(character);
                if let Some(hash_count) = raw_string_hash_count(chars.clone()) {
                    copy_raw_string(&mut chars, &mut normalized, hash_count);
                }
            }
            _ => normalized.push(character),
        }
    }

    normalized
}

fn copy_quoted_literal(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
    delimiter: char,
) {
    let mut escaped = false;
    for character in chars.by_ref() {
        output.push(character);
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == delimiter {
            break;
        }
    }
}

fn starts_character_literal(mut chars: std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let Some(first) = chars.next() else {
        return false;
    };

    match first {
        '\\' => {
            let Some(escape) = chars.next() else {
                return false;
            };
            match escape {
                'x' => {
                    let Some(first_hex) = chars.next() else {
                        return false;
                    };
                    let Some(second_hex) = chars.next() else {
                        return false;
                    };
                    first_hex.is_ascii_hexdigit()
                        && second_hex.is_ascii_hexdigit()
                        && chars.next() == Some('\'')
                }
                'u' => {
                    if chars.next() != Some('{') {
                        return false;
                    }
                    let mut digits = 0usize;
                    loop {
                        let Some(character) = chars.next() else {
                            return false;
                        };
                        if character == '}' {
                            break;
                        }
                        if !character.is_ascii_hexdigit() && character != '_' {
                            return false;
                        }
                        if character != '_' {
                            digits += 1;
                        }
                    }
                    digits > 0 && chars.next() == Some('\'')
                }
                '\n' | '\r' => false,
                _ => chars.next() == Some('\''),
            }
        }
        '\n' | '\r' | '\'' => false,
        _ => chars.next() == Some('\''),
    }
}

fn raw_string_hash_count(mut chars: std::iter::Peekable<std::str::Chars<'_>>) -> Option<usize> {
    let mut hash_count = 0usize;
    while chars.peek() == Some(&'#') {
        let _ = chars.next();
        hash_count += 1;
    }
    (chars.next() == Some('"')).then_some(hash_count)
}

fn copy_raw_string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
    hash_count: usize,
) {
    for _ in 0..hash_count {
        if let Some(hash) = chars.next() {
            output.push(hash);
        }
    }
    if let Some(opening_quote) = chars.next() {
        output.push(opening_quote);
    }

    while let Some(character) = chars.next() {
        output.push(character);
        if character != '"' || !raw_string_closes(chars.clone(), hash_count) {
            continue;
        }
        for _ in 0..hash_count {
            if let Some(hash) = chars.next() {
                output.push(hash);
            }
        }
        break;
    }
}

fn raw_string_closes(
    mut chars: std::iter::Peekable<std::str::Chars<'_>>,
    hash_count: usize,
) -> bool {
    (0..hash_count).all(|_| chars.next() == Some('#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_whitespace_is_canonicalized_and_deduplicated() {
        let canonical = canonicalize_missing_discriminators(&[
            " amount   == threshold ".to_string(),
            "amount == threshold".to_string(),
            "   ".to_string(),
        ]);

        assert_eq!(canonical, vec!["amount == threshold".to_string()]);
    }

    #[test]
    fn normal_string_literal_whitespace_remains_significant() {
        let canonical = canonicalize_missing_discriminators(&[
            r#"actual == "a b""#.to_string(),
            r#"actual == "a  b""#.to_string(),
        ]);

        assert_eq!(canonical.len(), 2);
        assert_ne!(canonical[0], canonical[1]);
    }

    #[test]
    fn raw_and_character_literal_whitespace_remains_significant() {
        assert_ne!(
            normalize_discriminator_text(r##"actual == r#"a b"#"##),
            normalize_discriminator_text(r##"actual == r#"a  b"#"##)
        );
        assert_eq!(
            normalize_discriminator_text("actual   ==   ' '"),
            "actual == ' '"
        );
    }

    #[test]
    fn lifetime_quote_does_not_capture_following_formatting_whitespace() {
        assert_eq!(
            normalize_discriminator_text("value::< 'a   >()"),
            "value::< 'a >()"
        );
    }

    #[test]
    fn length_prefix_encoding_distinguishes_delimiter_partitions() {
        let left = encode_missing_discriminators(&["a;b".to_string(), "c".to_string()]);
        let right = encode_missing_discriminators(&["a".to_string(), "b;c".to_string()]);

        assert_ne!(left, right);
    }
}
