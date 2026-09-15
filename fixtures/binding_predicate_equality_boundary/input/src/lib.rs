pub fn split_after(input: &str, delim: char) -> &str {
    let end = input.rfind(delim).map_or(1, |idx| idx);
    let start = delim.len_utf8();
    if end == start {
        &input[..end]
    } else {
        input
    }
}
