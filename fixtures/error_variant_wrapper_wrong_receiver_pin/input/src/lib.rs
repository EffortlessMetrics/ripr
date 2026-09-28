use std::error::Error;

#[derive(Debug)]
pub enum ParseSummaryError {
    MalformedSource,
    ParserInit,
}

pub struct ParseSummary {
    pub node_count: usize,
}

pub fn try_parse_summary(raw: &str) -> Result<ParseSummary, ParseSummaryError> {
    if raw.contains('@') {
        return Err(ParseSummaryError::MalformedSource);
    }
    Ok(ParseSummary {
        node_count: raw.lines().count(),
    })
}

pub struct ParserA;
pub struct ParserB;

impl ParserA {
    pub fn parse_summary(&self, raw: &str) -> Result<ParseSummary, Box<dyn Error>> {
        try_parse_summary(raw).map_err(|error| error.to_string().into())
    }
}

impl ParserB {
    pub fn parse_summary(&self, raw: &str) -> Result<ParseSummary, ParseSummaryError> {
        if raw.contains('@') {
            return Err(ParseSummaryError::MalformedSource);
        }
        Ok(ParseSummary { node_count: 0 })
    }
}

impl std::fmt::Display for ParseSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for ParseSummaryError {}
