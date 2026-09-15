pub fn label(kind: &str) -> &'static str {
    match kind {
        "word" => "alpha",
        "text" => "beta",
        _ => "other",
    }
}

pub fn classify(input: &str) -> &'static str {
    let final_label = label(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}
