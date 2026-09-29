#[derive(Debug, PartialEq)]
pub enum ParseError {
    ParserInit,
    MalformedSource,
}

pub fn try_parse(raw: &str) -> Result<usize, ParseError> {
    if raw.contains('@') {
        return Err(ParseError::MalformedSource);
    }
    Ok(raw.len())
}
