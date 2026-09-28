use rust_field_construction_token_coincidence_fixture::{default_config, fallback};

#[test]
fn config_timeout_and_fallback_retries() {
    let cfg = default_config();
    assert_eq!(cfg.timeout_secs, 30);
    let fb = fallback();
    assert_eq!(fb.retries, 3);
}
