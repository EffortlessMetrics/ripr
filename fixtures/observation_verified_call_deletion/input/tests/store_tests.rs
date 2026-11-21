use observation_verified_call_deletion_fixture::{Cache, store_result};

#[test]
fn store_result_inserts_result_key_with_value() {
    let mut cache = Cache::new();
    store_result(&mut cache, 42);
    assert_eq!(cache.inserted, vec!["result_key=42".to_string()]);
}
