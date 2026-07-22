pub fn validate_score(value: i32) -> bool {
    check_score_invariants(value);
    value <= 100
}

fn check_score_invariants(value: i32) {
    let clamped = if value >= 0 { value } else { 0 };
    assert!(clamped <= 100, "score above maximum");
    assert_eq!(clamped, value);
}
