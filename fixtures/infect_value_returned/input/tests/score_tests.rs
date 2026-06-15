use infect_value_returned_fixture::{MULTIPLIER, score};

#[test]
fn score_uses_multiplier() {
    assert_eq!(score(10), 10 * MULTIPLIER / 10 + 1);
}
