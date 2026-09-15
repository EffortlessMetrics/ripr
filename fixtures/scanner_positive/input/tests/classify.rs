use sc::classify;

#[test]
fn plain_word_is_word() {
    assert_eq!(classify("ab"), "word");
}

#[test]
fn trailing_space_is_blank() {
    assert_eq!(classify("ab "), "blank");
}
