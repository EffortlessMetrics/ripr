#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    TooLong(usize),
}

pub fn validate(name: &str) -> Result<String, ParseError> {
    if name.is_empty() {
        return Err(ParseError::Empty);
    }
    if name.len() > 8 {
        return Err(ParseError::TooLong(name.len()));
    }
    Ok(name.to_string())
}
