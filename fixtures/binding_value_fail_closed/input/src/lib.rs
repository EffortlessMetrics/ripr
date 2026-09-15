pub fn via_map_or_else(input: &str, delim: char) -> &str {
    let end = input.rfind(delim).map_or_else(|| 1, |idx| idx);
    let start = delim.len_utf8();
    if end == start {
        &input[..end]
    } else {
        input
    }
}

pub fn via_shifted_closure(input: &str, delim: char) -> &str {
    let end = input.rfind(delim).map_or(0, |idx| idx + 1);
    let start = delim.len_utf8();
    if end == start {
        &input[..end]
    } else {
        input
    }
}

pub fn via_dynamic_needle(input: &str, delim: char) -> bool {
    let needle = delim.to_string();
    let end = input.rfind(needle.as_str()).map_or(0, |idx| idx);
    let start = delim.len_utf8();
    end == start
}
