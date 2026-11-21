use unwrap_err_variant_single_line_fixture::{ParseError, validate};

#[test]
fn too_long_name_rejected_with_exact_variant() { let err = validate("aaaaaaaaaaaa").unwrap_err(); assert_eq!(err, ParseError::TooLong(12)); }
