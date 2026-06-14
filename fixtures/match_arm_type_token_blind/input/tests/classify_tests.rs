use match_arm_type_token_blind_fixture::{Mode, classify};

#[test]
fn warm_arm_returns_one() {
    assert_eq!(classify(Mode::Warm), 1);
}
