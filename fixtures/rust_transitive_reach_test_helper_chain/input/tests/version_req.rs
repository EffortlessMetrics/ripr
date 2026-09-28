use rust_transitive_reach_test_helper_chain_fixture::accepts_boundary;

#[track_caller]
fn assert_public_api_accepts(values: &[u64]) {
    for value in values {
        assert!(accepts_boundary(*value));
    }
}

#[track_caller]
fn assert_public_api_rejects(values: &[u64]) {
    for value in values {
        assert!(!accepts_boundary(*value));
    }
}

#[test]
fn greater_than_patch_examples() {
    assert_public_api_accepts(&[4, 5]);
    assert_public_api_rejects(&[3, 2]);
}
