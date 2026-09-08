#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidData,
    UnexpectedEof,
}

pub fn diagnostics_enabled() -> bool {
    false
}

pub struct Config;

impl Config {
    pub fn unwrap(&self) -> u8 {
        0
    }
}

pub fn expect_ready(kind: &str, len: usize) -> Result<usize, ParseError> {
    if kind != "ready" {
        return Err(ParseError::InvalidData);
    }
    Ok(len)
}
