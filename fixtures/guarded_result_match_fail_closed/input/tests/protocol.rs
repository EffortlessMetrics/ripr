use guarded_result_match::{ParseError, expect_response, other_helper};

#[test]
fn direct_owner_call_keeps_the_file_related() {
    let response = expect_response("42|workspace/refresh", "42").unwrap();
    assert_eq!(response.id, "42");
}

#[test]
fn wrong_owner_guard_never_credits_the_seam() {
    match other_helper("42") {
        Ok(response) => assert_eq!(response.id, "42"),
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

#[test]
fn variable_binding_scrutinee_keeps_existing_weaker_meaning() {
    let result = expect_response("42|workspace/refresh", "42");
    match result {
        Ok(response) => assert_eq!(response.id, "42"),
        Err(_) => panic!("failed"),
    }
}

#[test]
fn shadowed_callee_never_credits_the_seam() {
    let expect_response = other_helper;
    match expect_response("42") {
        Ok(response) => assert_eq!(response.id, "42"),
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn message_only_error_predicate_never_pins() {
    match expect_response("42|workspace/refresh", "42") {
        Ok(response) => assert_eq!(response.id, "42"),
        Err(error) => {
            if !error.to_string().contains("ParseError::InvalidData") {
                panic!("bad error: {error}");
            }
        }
    }
}

#[test]
fn swallowed_error_arm_never_credits() {
    match expect_response("42|workspace/refresh", "42") {
        Ok(response) => assert_eq!(response.id, "42"),
        Err(error) => {
            let _typed = error.downcast_ref::<ParseError>();
        }
    }
}
