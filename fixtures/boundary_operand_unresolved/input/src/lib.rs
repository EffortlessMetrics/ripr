pub fn quote_body(rest: &str, open: char, close: char) -> Option<&str> {
    let start = open.len_utf8();
    let end = rest.rfind(close)?;
    if end == start {
        Some("")
    } else if end > start {
        Some(&rest[start..end])
    } else {
        None
    }
}
