use observation_unverified_return_value_fixture::compute_score;

#[test]
fn result_is_positive() {
    assert!(compute_score(3) > 0);
}
