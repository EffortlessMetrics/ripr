use error_path_sibling_oracle_fake_clean_fixture::validate_or_default;

// Covers the happy path: valid input returns "valid".
// Does NOT observe the error variant at all — only a sibling exact-value
// oracle crediting the non-error return value exists. This test cannot
// discriminate between ParseError::TooShort and ParseError::TooLong.
#[test]
fn valid_input_returns_valid() {
    assert_eq!(validate_or_default("hello"), Ok("valid"));
}
