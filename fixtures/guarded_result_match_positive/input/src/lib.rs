#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidData,
    UnexpectedEof,
}

pub fn expect_ready(kind: &str, len: usize) -> Result<usize, ParseError> {
    if kind != "ready" {
        return Err(ParseError::InvalidData);
    }
    if len == 0 {
        return Err(ParseError::UnexpectedEof);
    }
    Ok(len)
}
