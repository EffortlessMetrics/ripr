pub fn label_of(kind: &str) -> &'static str {
    match kind {
        "word" => label_of("text"),
        "text" => "beta",
        _ => "alpha",
    }
}

pub fn classify(input: &str) -> &'static str {
    let final_label = label_of(input);
    if final_label == "alpha" {
        "word"
    } else {
        "blank"
    }
}
