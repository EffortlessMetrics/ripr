use crate::app::OutputFormat;
use crate::cli::suggest::unknown_value;

pub(crate) fn parse_format(value: &str) -> Result<OutputFormat, String> {
    OutputFormat::parse_cli_name(value)
        .ok_or_else(|| unknown_value("format", value, &OutputFormat::accepted_cli_names()))
}
