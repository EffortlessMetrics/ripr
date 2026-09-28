use match_arm_blind_fixture::reason;

#[test]
fn some_arm_returns_incremented_value() {
    assert_eq!(reason(Some(5)), 6);
}
