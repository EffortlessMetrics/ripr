use unwrap_err_generic_is_err_fixture::compute;

// Binds the error via unwrap_err() but only makes a generic string assertion
// — does NOT pin the specific CalcError::Negative variant.
#[test]
fn negative_input_returns_some_error() {
    let err = compute(-1).unwrap_err();
    assert!(err.to_string().contains("error"));
}
