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
pub enum LengthError {
    TooLong,
    EmptyPayload,
}

#[derive(Debug)]
pub enum RenderError {
    BadHex,
    MissingTheme,
}

#[derive(Debug)]
pub enum GlyphError {
    MalformedGlyph,
    MissingGlyph,
}

#[derive(Debug)]
pub enum ThemeError {
    DuplicateTheme,
    MissingPalette,
}

pub struct ParseSummary {
    pub node_count: usize,
}

pub fn try_parse_summary(raw: &str) -> Result<ParseSummary, ParseSummaryError> {
    if raw.contains('@') {
        return Err(ParseSummaryError::MalformedSource);
    }
    if raw.is_empty() {
        return Err(ParseSummaryError::MalformedSource);
    }
    Ok(ParseSummary {
        node_count: raw.lines().count(),
    })
}

pub fn parse_summary(raw: &str) -> Result<ParseSummary, Box<dyn Error>> {
    try_parse_summary(raw).map_err(Into::into)
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

pub fn try_length_summary(raw: &str) -> Result<usize, LengthError> {
    if raw.len() > 40 {
        return Err(LengthError::TooLong);
    }
    if raw.is_empty() {
        return Err(LengthError::EmptyPayload);
    }
    Ok(raw.len())
}

pub fn length_summary(raw: &str) -> Result<usize, Box<dyn Error>> {
    try_length_summary(raw).map_err(Into::into)
}

pub fn try_glyph_summary(raw: &str) -> Result<char, GlyphError> {
    if raw.starts_with('#') {
        return Err(GlyphError::MalformedGlyph);
    }
    match raw.chars().next() {
        Some(glyph) => Ok(glyph),
        None => Err(GlyphError::MissingGlyph),
    }
}

pub fn glyph_summary(raw: &str) -> Result<char, Box<dyn Error>> {
    try_glyph_summary(raw).map_err(|error| error.to_string().into())
}

pub fn try_theme_summary(raw: &str) -> Result<String, ThemeError> {
    if raw.starts_with('&') {
        return Err(ThemeError::DuplicateTheme);
    }
    if raw.contains('^') {
        return Err(ThemeError::MissingPalette);
    }
    Ok(raw.to_ascii_lowercase())
}

pub fn theme_summary(raw: &str) -> Result<String, Box<dyn Error>> {
    try_theme_summary(raw).map_err(Into::into)
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

fixture_error!(ParseSummaryError);
fixture_error!(ChecksumError);
fixture_error!(LengthError);
fixture_error!(RenderError);
fixture_error!(GlyphError);
fixture_error!(ThemeError);
