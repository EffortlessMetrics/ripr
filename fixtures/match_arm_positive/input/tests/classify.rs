use sc::classify;

#[test]
fn word_label_is_word() {
    assert_eq!(classify("word"), "word");
}

#[test]
fn text_label_is_blank() {
    assert_eq!(classify("text"), "blank");
}
