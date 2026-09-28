use guarded_result_match::{ParseError, expect_response};

#[test]
fn validates_ready_response() {
    let mut cursor = std::io::Cursor::new("ready:payload");
    match expect_response(&mut cursor, "ready") {
        Ok(response) => {
            assert_eq!(response.id, "ready");
            assert_eq!(response.body, "payload");
        }
        Err(error) => {
            if !matches!(
                error.downcast_ref::<ParseError>(),
                Some(ParseError::InvalidData { .. })
            ) {
                panic!("unexpected error variant: {error}");
            }
        }
    }
}
