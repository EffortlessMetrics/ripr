use assertion_shaped_control_production_caller::validate_score;

#[test]
fn validate_score_accepts_in_range_values() {
    assert!(validate_score(50));
}
