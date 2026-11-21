use observation_unverified_field_construction_fixture::default_config;

#[test]
fn config_exists() {
    let cfg = default_config();
    assert!(cfg.timeout_secs > 0);
}
