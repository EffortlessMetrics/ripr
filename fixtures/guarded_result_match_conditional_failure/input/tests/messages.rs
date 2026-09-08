use guarded_result_match_conditional_failure::{
    Config, ParseError, diagnostics_enabled, expect_ready,
};

#[test]
fn conditional_panic_swallows_the_error() {
    let config = Config;
    match expect_ready("busy", 12) {
        Ok(value) => assert_eq!(value, 12),
        Err(ParseError::InvalidData) => {
            if diagnostics_enabled() {
                panic!("debug");
            }
        }
    }
}

#[test]
fn unrelated_unwrap_is_not_a_failure_action() {
    let config = Config;
    match expect_ready("busy", 12) {
        Ok(value) => assert_eq!(value, 12),
        Err(ParseError::InvalidData) => {
            config.unwrap();
        }
    }
}

#[test]
fn nested_closure_panic_is_not_a_failure_action() {
    match expect_ready("busy", 12) {
        Ok(value) => assert_eq!(value, 12),
        Err(ParseError::InvalidData) => {
            let loud = || panic!("x");
            loud();
        }
    }
}
