use sc::classify_bound;
use sc::classify_computed;
use sc::classify_bare;
use sc::classify_trim;

#[test]
fn bound_long_word_is_word() {
    assert_eq!(classify_bound("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), "word");
}

#[test]
fn bound_long_trailing_space_is_blank() {
    assert_eq!(classify_bound("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa "), "blank");
}

#[test]
fn trimmed_word_is_word() {
    assert_eq!(classify_trim("ab"), "word");
}

#[test]
fn trimmed_trailing_space_is_word() {
    assert_eq!(classify_trim("ab "), "word");
}

#[test]
fn computed_word_is_word() {
    assert_eq!(classify_computed("ab"), "word");
}

#[test]
fn computed_space_is_blank() {
    assert_eq!(classify_computed(" "), "blank");
}

#[test]
fn bare_word_is_word() {
    assert_eq!(classify_bare("ab"), "word");
}

#[test]
fn bare_space_is_blank() {
    assert_eq!(classify_bare(" "), "blank");
}
