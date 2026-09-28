use hm::classify;

#[test]
fn leading_space_classifies_other() {
    assert_eq!(classify(" x"), "other");
}
