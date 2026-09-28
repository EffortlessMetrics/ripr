//! Pre-parse bracket budget for the Python preview adapter (#4109).
//!
//! `rustpython_parser` is recursive. A discovered file with deep bracket
//! nesting overflows the process stack before any finding can be emitted,
//! including when that file is not in the diff. This scanner refuses the
//! file first.
//!
//! Brackets inside comments are ignored. Brackets inside string literals are
//! counted: that over-declines a literal full of brackets (fail-closed) and
//! still sees f-string expressions, which the parser recurses into. A `#`
//! inside a string must not be treated as a comment, or the rest of the line
//! — including real nesting — would be skipped.

pub(in crate::analysis::language::python) const MAX_PYTHON_PARSE_NESTING_DEPTH: usize = 128;

pub(in crate::analysis::language::python) fn nesting_budget_reason(source: &str) -> Option<String> {
    if bracket_depth_exceeds(source, MAX_PYTHON_PARSE_NESTING_DEPTH) {
        Some(format!(
            "parse_budget: nesting depth exceeded {MAX_PYTHON_PARSE_NESTING_DEPTH}"
        ))
    } else {
        None
    }
}

enum ScanMode {
    Code,
    Comment,
    String {
        quote: u8,
        triple: bool,
        raw: bool,
        escaped: bool,
    },
}

fn bracket_depth_exceeds(source: &str, budget: usize) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0usize;
    let mut mode = ScanMode::Code;
    let mut depth = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match mode {
            ScanMode::Comment => {
                if byte == b'\n' {
                    mode = ScanMode::Code;
                }
                index += 1;
            }
            ScanMode::String {
                quote,
                triple,
                raw,
                escaped,
            } => {
                if note_bracket(byte, &mut depth, budget) {
                    return true;
                }
                if !raw && escaped {
                    mode = ScanMode::String {
                        quote,
                        triple,
                        raw,
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if !raw && byte == b'\\' {
                    mode = ScanMode::String {
                        quote,
                        triple,
                        raw,
                        escaped: true,
                    };
                    index += 1;
                    continue;
                }
                if byte == quote && raw_quote_can_terminate(bytes, index, raw) {
                    if triple {
                        if index + 2 < bytes.len()
                            && bytes[index + 1] == quote
                            && bytes[index + 2] == quote
                        {
                            mode = ScanMode::Code;
                            index += 3;
                            continue;
                        }
                    } else {
                        mode = ScanMode::Code;
                        index += 1;
                        continue;
                    }
                }
                index += 1;
            }
            ScanMode::Code => {
                if byte == b'#' {
                    mode = ScanMode::Comment;
                    index += 1;
                    continue;
                }
                if byte == b'\'' || byte == b'"' {
                    let triple = index + 2 < bytes.len()
                        && bytes[index + 1] == byte
                        && bytes[index + 2] == byte;
                    mode = ScanMode::String {
                        quote: byte,
                        triple,
                        raw: prefix_is_raw(bytes, index),
                        escaped: false,
                    };
                    index += if triple { 3 } else { 1 };
                    continue;
                }
                if note_bracket(byte, &mut depth, budget) {
                    return true;
                }
                index += 1;
            }
        }
    }
    false
}

fn note_bracket(byte: u8, depth: &mut usize, budget: usize) -> bool {
    match byte {
        b'(' | b'[' | b'{' => {
            *depth += 1;
            *depth > budget
        }
        b')' | b']' | b'}' => {
            *depth = depth.saturating_sub(1);
            false
        }
        _ => false,
    }
}

/// Raw strings keep a quote that is preceded by an odd number of backslashes.
/// Non-raw escapes are consumed before this check, so they always terminate.
fn raw_quote_can_terminate(bytes: &[u8], index: usize, raw: bool) -> bool {
    if !raw {
        return true;
    }
    let mut slashes = 0usize;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slashes += 1;
        cursor -= 1;
    }
    slashes.is_multiple_of(2)
}

fn prefix_is_raw(bytes: &[u8], quote_index: usize) -> bool {
    let mut start = quote_index;
    while start > 0
        && matches!(
            bytes[start - 1],
            b'r' | b'R' | b'u' | b'U' | b'b' | b'B' | b'f' | b'F'
        )
    {
        start -= 1;
    }
    if start > 0 {
        let previous = bytes[start - 1];
        if previous.is_ascii_alphanumeric() || previous == b'_' || !previous.is_ascii() {
            return false;
        }
    }
    bytes[start..quote_index]
        .iter()
        .any(|byte| matches!(byte, b'r' | b'R'))
}

#[cfg(test)]
mod tests {
    use super::{MAX_PYTHON_PARSE_NESTING_DEPTH, nesting_budget_reason};

    fn nested(depth: usize) -> String {
        let mut source = String::from("x = ");
        source.push_str(&"(".repeat(depth));
        source.push('1');
        source.push_str(&")".repeat(depth));
        source.push('\n');
        source
    }

    #[test]
    fn budget_allows_depth_128_and_refuses_129() -> Result<(), String> {
        if nesting_budget_reason(&nested(MAX_PYTHON_PARSE_NESTING_DEPTH)).is_some() {
            return Err(
                "depth 128 is under the abort threshold and must stay parseable".to_string(),
            );
        }
        let reason = nesting_budget_reason(&nested(MAX_PYTHON_PARSE_NESTING_DEPTH + 1))
            .ok_or_else(|| "depth 129 must trip the parse budget".to_string())?;
        if reason
            != format!("parse_budget: nesting depth exceeded {MAX_PYTHON_PARSE_NESTING_DEPTH}")
        {
            return Err(format!("unexpected budget reason: {reason}"));
        }
        Ok(())
    }

    #[test]
    fn flat_calls_and_comment_nesting_do_not_trip() -> Result<(), String> {
        let flat = format!("x = {}\n", "()".repeat(400));
        if nesting_budget_reason(&flat).is_some() {
            return Err("sequential calls are depth 1 and must not trip".to_string());
        }
        let comment = format!("# {}\nx = 1\n", "(".repeat(400));
        if nesting_budget_reason(&comment).is_some() {
            return Err("brackets inside a comment are not parser nesting".to_string());
        }
        Ok(())
    }

    #[test]
    fn string_hash_does_not_hide_following_code() -> Result<(), String> {
        let mut source = String::from("x = \"foo # \" + ");
        source.push_str(&nested(MAX_PYTHON_PARSE_NESTING_DEPTH + 1));
        if nesting_budget_reason(&source).is_none() {
            return Err("a # inside a string must not comment out the following call".to_string());
        }
        let mut raw = String::from("x = r\"foo \\\" # \" + ");
        raw.push_str(&nested(MAX_PYTHON_PARSE_NESTING_DEPTH + 1));
        if nesting_budget_reason(&raw).is_none() {
            return Err("a raw string must not swallow the following call".to_string());
        }
        Ok(())
    }

    #[test]
    fn string_literals_fail_closed_when_they_themselves_are_deep() -> Result<(), String> {
        let literal = format!(
            "x = \"{}\"\n",
            "(".repeat(MAX_PYTHON_PARSE_NESTING_DEPTH + 1)
        );
        if nesting_budget_reason(&literal).is_none() {
            return Err(
                "deep brackets inside a string are declined rather than handed to the parser"
                    .to_string(),
            );
        }
        Ok(())
    }
}
