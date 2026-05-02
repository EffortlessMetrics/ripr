use crate::app::{Mode, OutputFormat};

pub(super) fn parse_mode(value: &str) -> Result<Mode, String> {
    match value {
        "instant" => Ok(Mode::Instant),
        "draft" => Ok(Mode::Draft),
        "fast" => Ok(Mode::Fast),
        "deep" => Ok(Mode::Deep),
        "ready" => Ok(Mode::Ready),
        _ => Err(format!("unknown mode {value:?}")),
    }
}

pub(super) fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "human" | "text" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        "github" => Ok(OutputFormat::Github),
        _ => Err(format!("unknown format {value:?}")),
    }
}

pub(super) fn expect_value<'a>(
    args: &'a [String],
    idx: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(idx)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("missing value for {flag}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_modes() {
        assert_eq!(parse_mode("instant"), Ok(Mode::Instant));
        assert_eq!(parse_mode("draft"), Ok(Mode::Draft));
        assert_eq!(parse_mode("fast"), Ok(Mode::Fast));
        assert_eq!(parse_mode("deep"), Ok(Mode::Deep));
        assert_eq!(parse_mode("ready"), Ok(Mode::Ready));
        assert_eq!(parse_mode("slow"), Err("unknown mode \"slow\"".to_string()));
    }

    #[test]
    fn parses_output_formats() {
        assert_eq!(parse_format("human"), Ok(OutputFormat::Human));
        assert_eq!(parse_format("text"), Ok(OutputFormat::Human));
        assert_eq!(parse_format("json"), Ok(OutputFormat::Json));
        assert_eq!(parse_format("github"), Ok(OutputFormat::Github));
        assert_eq!(
            parse_format("xml"),
            Err("unknown format \"xml\"".to_string())
        );
    }

    #[test]
    fn expects_value_at_index() {
        let values = args(&["--diff", "sample.diff"]);
        assert_eq!(expect_value(&values, 1, "--diff"), Ok("sample.diff"));
        assert_eq!(
            expect_value(&values, 2, "--diff"),
            Err("missing value for --diff".to_string())
        );
    }
}
