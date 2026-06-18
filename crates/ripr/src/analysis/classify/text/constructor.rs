#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::analysis) struct ErrorConstructorPayload {
    pub(in crate::analysis) path: String,
    pub(in crate::analysis) string_literals: Vec<String>,
}

pub(in crate::analysis) fn error_constructor_call_paths(text: &str) -> Vec<String> {
    let mut paths = error_constructor_payloads(text)
        .into_iter()
        .map(|payload| payload.path)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub(in crate::analysis) fn error_constructor_payloads(text: &str) -> Vec<ErrorConstructorPayload> {
    let mut payloads = constructor_open_indices(text)
        .into_iter()
        .filter_map(|open| {
            let path = constructor_path_before_open(text, open)?;
            let arguments = delimited_contents_at(text, open)?;
            Some(ErrorConstructorPayload {
                path,
                string_literals: string_literals(&arguments),
            })
        })
        .collect::<Vec<_>>();
    payloads.sort();
    payloads.dedup();
    payloads
}

fn constructor_open_indices(text: &str) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if ch == '*'
                && let Some((_, '/')) = chars.peek().copied()
            {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }
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
            '/' => match chars.peek().copied() {
                Some((_, '/')) => {
                    let _ = chars.next();
                    in_line_comment = true;
                }
                Some((_, '*')) => {
                    let _ = chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
            '(' => indices.push(index),
            _ => {}
        }
    }
    indices
}

fn constructor_path_before_open(text: &str, open: usize) -> Option<String> {
    let before = text.get(..open)?;
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_rust_path_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let path = before[start..].trim();
    is_error_constructor_path(path).then(|| path.to_string())
}

fn delimited_contents_at(text: &str, open: usize) -> Option<String> {
    if text.as_bytes().get(open).copied()? != b'(' {
        return None;
    }
    let mut depth = 0i32;
    let mut content_start = None;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = text[open..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let index = open + offset;
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if ch == '*'
                && let Some((_, '/')) = chars.peek().copied()
            {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }
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
            '/' => match chars.peek().copied() {
                Some((_, '/')) => {
                    let _ = chars.next();
                    in_line_comment = true;
                }
                Some((_, '*')) => {
                    let _ = chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
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

fn is_error_constructor_path(path: &str) -> bool {
    let Some((owner, method)) = path.rsplit_once("::") else {
        return false;
    };
    let Some(owner_tail) = owner.rsplit("::").next() else {
        return false;
    };
    let owner_is_type = owner_tail
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());
    let method_is_constructor = method
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase());
    owner_is_type && method_is_constructor
}

fn is_rust_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'
}

fn string_literals(text: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut start = None;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if ch == '*'
                && let Some((_, '/')) = chars.peek().copied()
            {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if let Some(content_start) = start {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                let literal = text[content_start..index].to_string();
                if !literal.is_empty() {
                    literals.push(literal);
                }
                start = None;
            }
            continue;
        }
        match ch {
            '"' => start = Some(index + ch.len_utf8()),
            '/' => match chars.peek().copied() {
                Some((_, '/')) => {
                    let _ = chars.next();
                    in_line_comment = true;
                }
                Some((_, '*')) => {
                    let _ = chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
            _ => {}
        }
    }
    literals.sort();
    literals.dedup();
    literals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_payloads_read_literals_inside_matching_constructor_only() -> Result<(), String> {
        let payloads = error_constructor_payloads(
            r#"assert_eq!(err, CargoAllowError::new(format!("duplicate allow id `{}`", id)), "message outside");"#,
        );
        if payloads.len() != 1 {
            return Err(format!(
                "expected one constructor payload, got {payloads:?}"
            ));
        }
        let payload = &payloads[0];
        if payload.path != "CargoAllowError::new" {
            return Err(format!("unexpected constructor path: {}", payload.path));
        }
        if payload.string_literals != vec!["duplicate allow id `{}`".to_string()] {
            return Err(format!(
                "unexpected constructor literals: {:?}",
                payload.string_literals
            ));
        }
        Ok(())
    }

    #[test]
    fn constructor_payloads_ignore_comments() -> Result<(), String> {
        let payloads = error_constructor_payloads(
            r#"assert_eq!(err, CargoAllowError::new("real" /* "block comment" */)); // OtherError::new("line comment")"#,
        );
        if payloads.len() != 1 {
            return Err(format!(
                "comments should not add constructor calls: {payloads:?}"
            ));
        }
        if payloads[0].string_literals != vec!["real".to_string()] {
            return Err(format!(
                "comments should not add string literals: {:?}",
                payloads[0].string_literals
            ));
        }
        Ok(())
    }
}
