use match_arm_comment_literal_no_promotion::route;

#[test]
fn only_the_unchanged_sibling_is_observed() {
    assert_eq!(route("focused-test"), "proof");
}
