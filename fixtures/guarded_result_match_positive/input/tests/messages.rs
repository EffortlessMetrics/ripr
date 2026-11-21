use guarded_result_match_positive::{ParseError, expect_ready};

#[test]
fn rejects_unready_kind_with_exact_error_routing() {
    match expect_ready("busy", 12) {
        Err(error) if error == ParseError::InvalidData => {}
        result => panic!("fixture accepted an unready kind: {result:?}"),
    }
}

#[test]
fn rejects_short_payload_with_variant_pattern() {
    match expect_ready("ready", 0) {
        Ok(value) => assert_eq!(value, 0),
        Err(ParseError::UnexpectedEof) => panic!("short payload must reject as eof"),
    }
}
