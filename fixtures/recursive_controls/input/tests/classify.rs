use sc::{classify_ambiguous, classify_computed, classify_cycle, classify_deep};

#[test]
fn cycle_label_is_blank() {
    assert_eq!(classify_cycle("zz"), "blank");
}

#[test]
fn deep_label_is_word() {
    assert_eq!(classify_deep("d"), "word");
}

#[test]
fn computed_label_is_blank() {
    assert_eq!(classify_computed("word"), "blank");
}

#[test]
fn ambiguous_label_is_word() {
    assert_eq!(classify_ambiguous("word"), "word");
}
