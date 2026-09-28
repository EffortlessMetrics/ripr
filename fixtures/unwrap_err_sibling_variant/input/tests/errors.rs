use unwrap_err_sibling_variant_fixture::{CalcError, compute};

// Only tests the Negative error — does NOT pin TooLarge.
#[test]
fn negative_input_rejects_with_negative_error() {
    let err = compute(-1).unwrap_err();
    assert_eq!(err, CalcError::Negative);
}
