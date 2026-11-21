use observation_verified_field_construction_fixture::default_config;

#[test]
fn config_has_three_retries() {
    let cfg = default_config();
    assert_eq!(cfg.retries, 3);
}
