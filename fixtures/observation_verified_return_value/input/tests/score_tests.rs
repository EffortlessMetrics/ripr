use observation_verified_return_value_fixture::{SCALE_FACTOR, compute_score};

#[test]
fn score_uses_scale_factor() {
    assert_eq!(compute_score(3), 3 * SCALE_FACTOR);
}
