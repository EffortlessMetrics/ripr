use crate::analysis::facts::CallFact;

pub(crate) fn extract_call_facts(body: &str, start_line: usize) -> Vec<CallFact> {
    let mut calls = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'(' {
                let mut j = i;
                while j > 0 && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_') {
                    j -= 1;
                }
                if j < i {
                    let name = &line[j..i];
                    if is_call_name(name) {
                        calls.push(CallFact {
                            line: start_line + offset,
                            name: name.to_string(),
                            text: line.trim().to_string(),
                        });
                    }
                }
            }
            i += 1;
        }
    }
    calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    calls
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
mod tests {
    use super::*;

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
}
