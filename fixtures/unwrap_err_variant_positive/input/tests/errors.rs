use unwrap_err_variant_positive_fixture::{CalcError, compute};

#[test]
fn negative_input_rejects_with_negative_error() {
    let err = compute(-1).unwrap_err();
    assert_eq!(err, CalcError::Negative);
}
