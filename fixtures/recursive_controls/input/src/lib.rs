mod other;

pub fn label_cycle(kind: &str) -> &'static str {
    match kind {
        "word" => "alpha",
        _ => label_cycle(kind),
    }
}

pub fn classify_cycle(input: &str) -> &'static str {
    let final_label = label_cycle(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}

pub fn label_deep(kind: &str) -> &'static str {
    match kind {
        "a" => "alpha",
        "b" => label_deep("a"),
        "c" => label_deep("b"),
        "d" => label_deep("c"),
        _ => "other",
    }
}

pub fn classify_deep(input: &str) -> &'static str {
    let final_label = label_deep(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}

pub fn fix(kind: &str) -> &str {
    "text"
}

pub fn label_computed(kind: &str) -> &'static str {
    match kind {
        "word" => label_computed(fix(kind)),
        _ => "other",
    }
}

pub fn classify_computed(input: &str) -> &'static str {
    let final_label = label_computed(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}

pub fn resolve(kind: &str) -> &'static str {
    "alpha"
}

pub fn label_amb(kind: &str) -> &'static str {
    match kind {
        "word" => resolve("x"),
        _ => "other",
    }
}

pub fn classify_ambiguous(input: &str) -> &'static str {
    let final_label = label_amb(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}
