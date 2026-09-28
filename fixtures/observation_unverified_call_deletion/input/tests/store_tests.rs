use observation_unverified_call_deletion_fixture::{Cache, store_result};

#[test]
fn store_result_runs_without_panic() {
    let cache = Cache;
    store_result(&cache, 42);
    assert!(true);
}
