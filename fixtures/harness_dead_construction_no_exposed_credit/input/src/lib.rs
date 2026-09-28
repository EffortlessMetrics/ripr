/// Parses a raw limit string, falling back to zero.
pub fn parse_limit(raw: &str) -> u16 {
    raw.parse().unwrap_or(0)
}

/// Reports whether the parsed limit exceeds the default.
pub fn limit_exceeds_default(raw: &str) -> bool {
    parse_limit(raw) > 0
}
