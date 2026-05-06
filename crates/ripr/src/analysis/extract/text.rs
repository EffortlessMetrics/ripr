use ra_ap_syntax::TextSize;

pub(crate) fn text_size_to_usize(offset: TextSize) -> usize {
    let value: u32 = offset.into();
    value as usize
}

pub(crate) fn slice_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let end = text_size_to_usize(end);
    text.get(start..end).unwrap_or("").to_string()
}

pub(crate) fn slice_macro_call_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let mut end = text_size_to_usize(end);
    let bytes = text.as_bytes();
    let mut cursor = end;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'\n' {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b';') {
        end = cursor + 1;
    }
    text.get(start..end).unwrap_or("").trim().to_string()
}

pub(crate) fn is_predicate_operator(operator: &str) -> bool {
    matches!(
        operator,
        "==" | "!=" | "<=" | ">=" | "<" | ">" | "&&" | "||"
    )
}

pub(crate) fn has_effect_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        ".save(",
        ".publish(",
        ".send(",
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".emit(",
        ".increment(",
        "metrics.",
        "log::",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(crate) fn is_effect_call_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "save"
            | "publish"
            | "send"
            | "write"
            | "insert"
            | "push"
            | "remove"
            | "delete"
            | "emit"
            | "increment"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_helpers_are_callable() {
        let text = "fn test() { assert!(x > 5); }";
        let start = TextSize::from(0u32);
        let end = TextSize::from(9u32);

        let sliced = slice_text(text, start, end);
        assert_eq!(sliced, "fn test()");

        assert!(is_predicate_operator(">"));
        assert!(!is_predicate_operator("+"));

        assert!(has_effect_text("obj.save(data)"));
        assert!(!has_effect_text("obj.read(data)"));

        assert!(is_effect_call_name("save"));
        assert!(!is_effect_call_name("read"));
    }
}
