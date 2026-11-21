#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    TooShort,
    TooLong,
}

/// Validates an input string and returns a canonical form or an error.
/// Changed: was `ParseError::TooShort` for empty input, now `ParseError::TooLong`.
pub fn validate_or_default(input: &str) -> Result<&'static str, ParseError> {
    if input.is_empty() {
        return Err(ParseError::TooLong);
    }
    Ok("valid")
}
