use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Response {
    pub id: String,
    pub body: String,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    InvalidData { reason: String },
    BadMethod { method: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidData { reason } => write!(f, "invalid data: {reason}"),
            ParseError::BadMethod { method } => write!(f, "bad method: {method}"),
        }
    }
}

impl Error for ParseError {}

fn build_response(id: &str, body: &str) -> Result<Response, ParseError> {
    Ok(Response {
        id: id.to_string(),
        body: body.to_string(),
    })
}

pub fn expect_response(
    reader: &mut impl std::io::Read,
    expected_id: &str,
) -> Result<Response, Box<dyn Error>> {
    use std::io::Read;
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    let trimmed = text.trim();
    if trimmed != expected_id {
        return Err(Box::new(ParseError::InvalidData {
            reason: format!("id mismatch {trimmed}"),
        }));
    }
    Ok(build_response(expected_id, trimmed)?)
}
