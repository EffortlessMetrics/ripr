pub fn is_word_start(input: &str) -> bool {
    let prev = input.chars().next().map_or(false, |c| c == ' ');
    prev
}
