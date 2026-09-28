use std::error::Error;

#[derive(Debug)]
pub enum ParseSummaryError {
    MalformedSource,
    ParserInit,
}

#[derive(Debug)]
pub enum OtherError {
    MalformedSource,
    Unrelated,
}

pub struct ParseSummary {
    pub node_count: usize,
}

pub fn try_parse_summary(raw: &str) -> Result<ParseSummary, ParseSummaryError> {
    if raw.contains('@') {
        return Err(ParseSummaryError::MalformedSource);
    }
    if raw.is_empty() {
        return Err(ParseSummaryError::ParserInit);
    }
    Ok(ParseSummary {
        node_count: raw.lines().count(),
    })
}

pub fn unrelated_check(raw: &str) -> Result<(), OtherError> {
    if raw.contains('@') {
        return Err(OtherError::MalformedSource);
    }
    Ok(())
}

pub fn parse_summary(raw: &str) -> Result<ParseSummary, Box<dyn Error>> {
    try_parse_summary(raw).map_err(|error| error.to_string().into())
}

impl std::fmt::Display for ParseSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for ParseSummaryError {}

impl std::fmt::Display for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for OtherError {}
