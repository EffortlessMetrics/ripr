mod other;

pub fn is_word_start(input: &str) -> bool {
    let prev = input.chars().next().map_or(false, |c| c == ' ');
    prev
}

pub fn classify(input: &str) -> &'static str {
    if is_word_start(input.trim()) {
        "other"
    } else {
        "word"
    }
}

pub fn ambiguous(input: &str) -> &'static str {
    if other::is_word_start(input.trim()) {
        "other"
    } else {
        "word"
    }
}
