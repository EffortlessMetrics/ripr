use crate::app::OutputFormat;

pub(crate) fn parse_format(value: &str) -> Result<OutputFormat, String> {
    OutputFormat::parse_cli_name(value).ok_or_else(|| format!("unknown format {value:?}"))
}
