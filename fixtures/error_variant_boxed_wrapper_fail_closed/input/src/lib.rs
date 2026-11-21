use std::error::Error;

#[derive(Debug)]
pub enum ParseSummaryError {
    MalformedSource,
    ParserInit,
}

#[derive(Debug)]
pub enum ChecksumError {
    BadChecksum,
    MalformedPayload,
}

#[derive(Debug)]
pub enum RenderError {
    BadHex,
    MissingTheme,
}

pub fn try_checksum_summary(payload: &str) -> Result<usize, ChecksumError> {
    if payload.ends_with('?') {
        return Err(ChecksumError::BadChecksum);
    }
    Ok(payload.len())
}

pub fn checksum_summary(payload: &str) -> Result<usize, Box<dyn Error>> {
    try_checksum_summary(payload).map_err(Into::into)
}

pub fn try_render_summary(raw: &str) -> Result<String, RenderError> {
    if raw.starts_with('z') {
        return Err(RenderError::BadHex);
    }
    if raw.contains('%') {
        return Err(RenderError::MissingTheme);
    }
    Ok(raw.to_ascii_uppercase())
}

pub fn render_summary(raw: &str) -> Result<String, Box<dyn Error>> {
    try_render_summary(raw).map_err(Into::into)
}

macro_rules! fixture_error {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{self:?}")
            }
        }

        impl Error for $name {}
    };
}

fixture_error!(ChecksumError);
fixture_error!(RenderError);
fixture_error!(ParseSummaryError);
