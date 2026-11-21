use hc::classify;

#[test]
fn leading_space_classifies_other() {
    assert_eq!(classify(" x"), "other");
}

#[test]
fn letter_classifies_word() {
    assert_eq!(classify("hello"), "word");
}
