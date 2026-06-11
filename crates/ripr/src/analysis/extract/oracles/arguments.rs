pub(super) fn equality_assertion_arguments(line: &str) -> Option<Vec<String>> {
    ["assert_eq!", "assert_ne!"]
        .iter()
        .find_map(|macro_name| macro_invocation_arguments(line, macro_name))
}

pub(super) fn custom_assertion_arguments(line: &str) -> Option<Vec<String>> {
    let open = line.find('(')?;
    delimited_contents_at(line, open).map(|contents| split_top_level_commas(&contents))
}

fn macro_invocation_arguments(line: &str, macro_name: &str) -> Option<Vec<String>> {
    line.match_indices(macro_name)
        .filter_map(|(index, _)| {
            let prefix_ok = index == 0
                || !line[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
            let suffix_start = index + macro_name.len();
            let open_offset = line[suffix_start..]
                .char_indices()
                .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some((offset, ch)))?;
            if !prefix_ok || open_offset.1 != '(' {
                return None;
            }
            let open = suffix_start + open_offset.0;
            delimited_contents_at(line, open).map(|contents| split_top_level_commas(&contents))
        })
        .next()
}

fn delimited_contents_at(text: &str, open_index: usize) -> Option<String> {
    let open = text.as_bytes().get(open_index).copied()?;
    if open != b'(' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut content_start = None;
    for (offset, ch) in text[open_index..].char_indices() {
        let index = open_index + offset;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => {
                depth += 1;
                if depth == 1 {
                    content_start = Some(index + ch.len_utf8());
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let start = content_start?;
                    return Some(text[start..index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
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
