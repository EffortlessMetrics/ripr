use ct::{ambiguous, classify};

#[test]
fn classify_leading_space_is_other() {
    assert_eq!(classify(" x"), "other");
}

#[test]
fn ambiguous_leading_space_is_other() {
    assert_eq!(ambiguous(" x"), "other");
}
