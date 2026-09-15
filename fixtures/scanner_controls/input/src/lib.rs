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

pub fn classify_bound(input: &str) -> &'static str {
    let final_state = scan_state(input);
    if final_state == "word" {
        "word"
    } else {
        "blank"
    }
}

pub fn scan_trim(input: &str) -> &'static str {
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

pub fn classify_trim(input: &str) -> &'static str {
    let final_state = scan_trim(input.trim());
    if final_state == "word" {
        "word"
    } else {
        "blank"
    }
}

pub fn next_for(input: &str) -> &'static str {
    "word"
}

pub fn scan_computed(input: &str) -> &'static str {
    let mut state = "text";
    for symbol in input.chars() {
        state = match (state, symbol) {
            ("text", ' ') => "text",
            ("text", _) => next_for(input),
            ("word", ' ') => "text",
            ("word", _) => "word",
        };
    }
    state
}

pub fn classify_computed(input: &str) -> &'static str {
    let final_state = scan_computed(input);
    if final_state == "word" {
        "word"
    } else {
        "blank"
    }
}

pub fn scan_bare(input: &'static str) -> &'static str {
    let mut state = "text";
    for symbol in input.chars() {
        state = match (state, symbol) {
            ("text", 'x') => input,
            ("text", ' ') => "text",
            ("text", _) => "word",
            ("word", ' ') => "text",
            ("word", _) => "word",
        };
    }
    state
}

pub fn classify_bare(input: &'static str) -> &'static str {
    let final_state = scan_bare(input);
    if final_state == "word" {
        "word"
    } else {
        "blank"
    }
}
