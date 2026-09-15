use super::mask_comments_and_strings;
use crate::analysis::facts::CallFact;

pub(crate) fn extract_call_facts(body: &str, start_line: usize) -> Vec<CallFact> {
    // Comments (line and block, nested) and string contents are erased
    // before scanning so commented-out calls never become evidence.
    // Masking preserves newlines and byte layout, so line attribution
    // stays exact; the fact text keeps the original source line.
    let masked = mask_comments_and_strings(body);
    let mut calls = Vec::new();
    for (offset, (masked_line, original_line)) in masked.lines().zip(body.lines()).enumerate() {
        let scan_line = masked_line;
        let bytes = scan_line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'('
                && let Some((start, end)) = call_name_bounds_before_paren(scan_line, i)
            {
                let name = &scan_line[start..end];
                if is_call_name(name) {
                    calls.push(CallFact {
                        line: start_line + offset,
                        name: name.to_string(),
                        text: original_line.trim().to_string(),
                    });
                }
            }
            i += 1;
        }
    }
    calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    calls
}

fn call_name_bounds_before_paren(line: &str, paren_index: usize) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut end = paren_index;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    if bytes[end - 1] == b'>' {
        end = turbofish_start(line, end)?;
    }
    let mut start = end;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    (start < end).then_some((start, end))
}

fn turbofish_start(line: &str, end: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut i = end;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'>' => depth += 1,
            b'<' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return (i >= 2 && bytes[i - 1] == b':' && bytes[i - 2] == b':')
                        .then_some(i - 2);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_call_name(name: &str) -> bool {
    !matches!(
        name,
        "if" | "while"
            | "match"
            | "for"
            | "loop"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "assert_matches"
    )
}

#[cfg(test)]
fn call_names(calls: &[CallFact]) -> Vec<&str> {
    calls.iter().map(|call| call.name.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetimes_and_labels_do_not_mask_call_and_literal_facts() {
        // #3633 review (devin LYzZ + coderabbit LtlJ): nearby lifetime and
        // label apostrophes used to pair with char-literal openings and
        // erase live calls and numbers between them. Both extractors keep
        // the evidence.
        let body = "fn f<'a>() { x(7); 'b'; }\nfn g() { 'lbl: loop { keep(2); } }\n";

        let calls = extract_call_facts(body, 1);
        let call_names: Vec<&str> = calls.iter().map(|call| call.name.as_str()).collect();
        assert!(
            call_names.contains(&"x") && call_names.contains(&"keep"),
            "{call_names:?}"
        );

        let literals = super::super::extract_literal_facts(body, 1);
        let literal_values: Vec<&str> = literals
            .iter()
            .map(|literal| literal.value.as_str())
            .collect();
        assert!(
            literal_values.contains(&"7") && literal_values.contains(&"2"),
            "{literal_values:?}"
        );
    }

    #[test]
    fn given_control_flow_and_assertion_like_calls_when_extracting_then_skips_non_function_names() {
        let calls = extract_call_facts(
            r#"if(condition) {}
while(condition) {}
assert_eq(actual, expected);
real_call(1);
"#,
            10,
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].line, 13);
        assert_eq!(calls[0].name, "real_call");
        assert_eq!(calls[0].text, "real_call(1);");
    }

    #[test]
    fn call_extraction_ignores_comment_and_string_mentions() {
        let calls = extract_call_facts(
            r#"// fake_call()
let note = "device_labels(";
real_call(1);
"#,
            20,
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].line, 22);
        assert_eq!(calls[0].name, "real_call");
        assert_eq!(calls[0].text, "real_call(1);");
    }

    #[test]
    fn call_extraction_recognizes_turbofish_function_calls() {
        let calls = extract_call_facts(
            r#"
let rendered = render_pipeline::<String>("alpha");
let value = TypeName::new::<usize>();
let parsed = crate::parser::parse::<u64>("42");
let spaced = render_pipeline ("beta");
"#,
            30,
        );

        assert_eq!(
            call_names(&calls),
            vec!["render_pipeline", "new", "parse", "render_pipeline"]
        );
        assert_eq!(calls[0].line, 31);
        assert_eq!(
            calls[0].text,
            "let rendered = render_pipeline::<String>(\"alpha\");"
        );
        assert_eq!(calls[3].line, 34);
        assert_eq!(calls[3].text, "let spaced = render_pipeline (\"beta\");");
    }
}
