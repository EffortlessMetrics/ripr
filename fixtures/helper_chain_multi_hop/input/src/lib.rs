pub fn first_char(input: &str) -> char {
    let head = input.chars().next().map_or('?', |c| c);
    head
}

pub fn boundary_char(input: &str) -> char {
    first_char(input)
}

pub fn classify(input: &str) -> &'static str {
    if boundary_char(input) == ' ' {
        "other"
    } else {
        "word"
    }
}
