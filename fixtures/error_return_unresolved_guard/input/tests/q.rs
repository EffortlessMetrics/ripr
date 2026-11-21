use error_return_unresolved_guard::{cancelled_error, run_slow};

#[test]
fn cancelled_return_is_exact() {
    let err = run_slow(true).expect_err("must cancel");
    assert_eq!(err, cancelled_error());
    assert_eq!(err.message, "operation cancelled");
}
