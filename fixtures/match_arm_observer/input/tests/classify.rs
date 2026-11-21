use match_arm_observer_fixture::{Status, classify};

#[test]
fn idle_arm_returns_zero() {
    assert_eq!(classify(Status::Idle), 0);
}
