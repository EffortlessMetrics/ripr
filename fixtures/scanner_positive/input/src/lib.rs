pub fn scan_state(input: &str) -> &'static str {
    let mut state = "text";
    for symbol in input.chars() {
        state = match (state, symbol) {
            ("text", ' ') => "text",
            ("text", _) => "word",
            ("word", ' ') => "text",
            ("word", _) => "word",
        };
    }
    state
}

pub fn classify(input: &str) -> &'static str {
    let final_state = scan_state(input);
    if final_state == "word" {
        "word"
    } else {
        "blank"
    }
}
