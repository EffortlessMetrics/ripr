use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Response {
    pub id: String,
    pub method: String,
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

fn build_response(id: &str, method: &str) -> Result<Response, ParseError> {
    if method.is_empty() {
        return Err(ParseError::BadMethod {
            method: method.to_string(),
        });
    }
    Ok(Response {
        id: id.to_string(),
        method: method.to_string(),
    })
}

pub fn other_helper(raw: &str) -> Result<Response, Box<dyn Error>> {
    Ok(build_response(raw, "fallback")?)
}

pub fn expect_response(
    raw: &str,
    expected_id: &str,
) -> Result<Response, Box<dyn Error>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Box::new(ParseError::InvalidData {
            reason: format!("empty frame for {expected_id}"),
        }));
    }
    let (id, method) = trimmed
        .split_once('|')
        .ok_or(ParseError::InvalidData {
            reason: "missing separator".to_string(),
        })?;
    if id != expected_id {
        return Err(Box::new(ParseError::InvalidData {
            reason: format!("id mismatch {id}"),
        }));
    }
    Ok(build_response(id, method)?)
}
