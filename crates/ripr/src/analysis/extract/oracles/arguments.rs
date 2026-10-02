use crate::analysis::extract::mask_comments_and_strings;

pub(crate) fn equality_assertion_arguments(line: &str) -> Option<Vec<String>> {
    ["assert_eq!", "assert_ne!"]
        .iter()
        .find_map(|macro_name| macro_invocation_arguments(line, macro_name))
}

/// The semantic operands of known assertion macros, with their macro shape
/// retained for the canonical oracle classifier. Everything after the condition
/// (or the two comparison/pattern operands) is diagnostic formatting, including
/// non-string expressions. Unknown helpers keep their existing fallback path.
pub(crate) fn assertion_oracle_text(line: &str) -> Option<String> {
    let masked = mask_comments_and_strings(line);
    [
        ("assert!", 1),
        ("debug_assert!", 1),
        ("ensure!", 1),
        ("assert_eq!", 2),
        ("assert_ne!", 2),
        ("debug_assert_eq!", 2),
        ("debug_assert_ne!", 2),
        ("assert_matches!", 2),
        ("debug_assert_matches!", 2),
    ]
    .into_iter()
    .filter_map(|(name, count)| {
        let (offset, arguments) = macro_invocation_arguments_at(line, &masked, name)?;
        let operands = arguments.into_iter().take(count).collect::<Vec<_>>();
        // Keep separators outside any trailing line comment in an operand.
        // Trimming an operand must never hide the comma or closing delimiter.
        Some((offset, format!("{name}({}\n)", operands.join("\n, "))))
    })
    // `assert!(matches!(...))` must use the outer assertion's condition;
    // a nested assertion in its diagnostic never becomes the observer.
    .min_by_key(|(offset, _)| *offset)
    .map(|(_, text)| text)
}

pub(super) fn custom_assertion_arguments(line: &str) -> Option<Vec<String>> {
    let masked = mask_comments_and_strings(line);
    let open = masked.find('(')?;
    delimited_contents_at(line, open).map(|contents| split_top_level_commas(&contents))
}

pub(super) fn ensure_assertion_arguments(line: &str) -> Option<Vec<String>> {
    macro_invocation_arguments(line, "ensure!")
}

fn macro_invocation_arguments(line: &str, macro_name: &str) -> Option<Vec<String>> {
    let masked = mask_comments_and_strings(line);
    macro_invocation_arguments_at(line, &masked, macro_name).map(|(_, arguments)| arguments)
}

fn macro_invocation_arguments_at(
    line: &str,
    masked: &str,
    macro_name: &str,
) -> Option<(usize, Vec<String>)> {
    masked.match_indices(macro_name).find_map(|(index, _)| {
        let prefix_ok = index == 0
            || !masked[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        let suffix_start = index + macro_name.len();
        let (offset, opening) = masked[suffix_start..]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())?;
        if !prefix_ok || !matches!(opening, '(' | '[' | '{') {
            return None;
        }
        let open = suffix_start + offset;
        delimited_contents_at(line, open).map(|contents| (index, split_top_level_commas(&contents)))
    })
}

fn delimited_contents_at(text: &str, open_index: usize) -> Option<String> {
    let masked = mask_comments_and_strings(text);
    let open = masked.as_bytes().get(open_index).copied()?;
    if !matches!(open, b'(' | b'[' | b'{') {
        return None;
    }
    let mut stack = Vec::new();
    for (offset, ch) in masked[open_index..].char_indices() {
        match ch {
            '(' | '[' | '{' => stack.push(ch),
            ')' if stack.pop() != Some('(') => return None,
            ']' if stack.pop() != Some('[') => return None,
            '}' if stack.pop() != Some('{') => return None,
            _ => {}
        }
        if stack.is_empty() {
            return Some(text[open_index + 1..open_index + offset].to_string());
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<String> {
    let masked = mask_comments_and_strings(text);
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut generic_depth = 0usize;
    for (index, ch) in masked.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '<' if depth == 0
                && (generic_depth > 0
                    || masked[start..index].trim().is_empty()
                    || masked[..index].trim_end().ends_with("::")) =>
            {
                generic_depth += 1;
            }
            '>' if depth == 0 && !masked[..index].ends_with('-') => {
                generic_depth = generic_depth.saturating_sub(1);
            }
            ',' if depth == 0 && generic_depth == 0 => {
                args.push(text[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        args.push(tail.to_string());
    }
    args
}

pub(super) fn comparable_expression(expression: &str) -> String {
    expression
        .split_whitespace()
        .collect::<String>()
        .trim_start_matches('&')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{assertion_oracle_text, equality_assertion_arguments};

    #[test]
    fn oracle_operands_exclude_all_diagnostic_arguments() {
        for text in [
            r#"assert_eq!(rdr.len(), 10, "read error");"#,
            r##"assert_eq!(rdr.len(), 10, r#"read "error", ) ] }"#, read_error);"##,
            r#"assert_eq!(rdr.len(), 10, "read \"error\", )", read_error);"#,
            r#"assert_eq![rdr.len(), 10, "{}", read_error];"#,
            r#"assert_eq!{rdr.len(), 10, "{}", read_error};"#,
            r#"assert_eq!(rdr.len(), 10, "assert!(Err(ReadError::Closed))");"#,
        ] {
            assert_eq!(
                assertion_oracle_text(text).as_deref(),
                Some("assert_eq!(rdr.len()\n, 10\n)"),
                "{text}"
            );
        }
        assert_eq!(
            assertion_oracle_text("assert_eq!(read::<A, B>(rdr).unwrap_err(), Other, error)")
                .as_deref(),
            Some("assert_eq!(read::<A, B>(rdr).unwrap_err()\n, Other\n)")
        );
        assert_eq!(
            assertion_oracle_text("assert!(a < b, error)").as_deref(),
            Some("assert!(a < b\n)")
        );
        assert_eq!(
            assertion_oracle_text(
                r#"assert!(matches!(result, Err(ReadError::Closed)), "{}", diagnostic);"#
            )
            .as_deref(),
            Some("assert!(matches!(result, Err(ReadError::Closed))\n)")
        );
    }

    #[test]
    fn argument_boundaries_ignore_literal_and_comment_syntax() {
        let text =
            r##"assert_eq!(r#"one "quoted", ) two"#, "three, four", /* ), Err(_) */ "message");"##;
        assert_eq!(
            equality_assertion_arguments(text),
            Some(vec![
                r##"r#"one "quoted", ) two"#"##.to_string(),
                r#""three, four""#.to_string(),
                r#"/* ), Err(_) */ "message""#.to_string()
            ])
        );
        assert_eq!(assertion_oracle_text(r#"helper("assert_eq!(a, b)")"#), None);
        assert_eq!(
            assertion_oracle_text("my_assert_eq!(rdr, read_error)"),
            None
        );
        assert_eq!(
            assertion_oracle_text("/* assert_eq!(a, b) */ assert!(ready, error)"),
            Some("assert!(ready\n)".to_string())
        );
    }
}
