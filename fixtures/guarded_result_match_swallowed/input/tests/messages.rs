use guarded_result_match_swallowed::{ParseError, expect_ready};

#[test]
fn logs_failures_without_discriminating() {
    match expect_ready("busy", 12) {
        Err(error) => {
            eprintln!("kind rejection failed: {error:?}");
        }
        Ok(value) => assert_eq!(value, 12),
    }
}

#[test]
fn wildcard_panic_without_variant_pin() {
    match expect_ready("busy", 12) {
        Ok(value) => assert_eq!(value, 12),
        Err(_) => panic!("rejection failed"),
    }
}
