use sc::classify;

#[test]
fn word_label_is_blank() {
    assert_eq!(classify("word"), "blank");
}

#[test]
fn other_label_is_word() {
    assert_eq!(classify("zz"), "word");
}
