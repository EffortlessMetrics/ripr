pub fn pick(kind: &str) -> &'static str {
    "alpha"
}

pub fn label_computed(kind: &str) -> &'static str {
    match kind {
        "word" => pick(kind),
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

pub fn label_guard(kind: &str) -> &'static str {
    match kind {
        "word" if kind.len() > 2 => "alpha",
        _ => "other",
    }
}

pub fn classify_guard(input: &str) -> &'static str {
    let final_label = label_guard(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}

pub fn label_bare(kind: &str) -> &'static str {
    match kind {
        word => "alpha",
        _ => "other",
    }
}

pub fn classify_bare(input: &str) -> &'static str {
    let final_label = label_bare(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}

pub fn tag_char(c: char) -> &'static str {
    match c {
        'a' => "alpha",
        _ => "other",
    }
}

pub fn classify_char(c: char) -> &'static str {
    let final_label = tag_char(c);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}
