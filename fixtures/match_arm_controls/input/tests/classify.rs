use sc::{classify_bare, classify_char, classify_computed, classify_guard};

#[test]
fn computed_label_is_word() {
    assert_eq!(classify_computed("word"), "word");
}

#[test]
fn guarded_label_is_word() {
    assert_eq!(classify_guard("word"), "word");
}

#[test]
fn bare_label_is_word() {
    assert_eq!(classify_bare("word"), "word");
}

#[test]
fn char_tag_is_word() {
    assert_eq!(classify_char('a'), "word");
}
