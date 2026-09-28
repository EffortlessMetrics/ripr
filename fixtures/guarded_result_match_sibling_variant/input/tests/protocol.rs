use guarded_result_match_sibling_variant::{ParseError, expect_ready};

#[test]
fn pins_the_sibling_variant_of_the_changed_owner() {
    match expect_ready("busy", 12) {
        Ok(value) => assert_eq!(value, 12),
        Err(ParseError::UnexpectedEof) => panic!("short payload must reject as eof"),
    }
}
