fn parse_value<T>(value: &str, flag: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse::<T>()
        .map_err(|err| format!("invalid {flag}: {err}"))
}

fn require_positive<T>(parsed: T, flag: &str) -> Result<T, String>
where
    T: Eq + From<u8>,
{
    if parsed == T::from(0) {
        return Err(format!("invalid {flag}: expected a positive integer"));
    }
    Ok(parsed)
}

pub(super) fn parse_positive_usize(value: &str, flag: &str) -> Result<usize, String> {
    let parsed = parse_value::<usize>(value, flag)?;
    require_positive(parsed, flag)
}

pub(super) fn parse_positive_u64(value: &str, flag: &str) -> Result<u64, String> {
    let parsed = parse_value::<u64>(value, flag)?;
    require_positive(parsed, flag)
}
