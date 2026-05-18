use crate::app::{Mode, OutputFormat};
use crate::cli::command::CliCommand;

pub(super) fn parse_args(args: Vec<String>) -> Result<CliCommand, String> {
    let command = args.get(1).map(|s| s.as_str());
    let command_args = args.get(2..).map_or_else(Vec::new, <[String]>::to_vec);
    CliCommand::from_parts(command, command_args)
}

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
    OutputFormat::parse_cli_name(value).ok_or_else(|| format!("unknown format {value:?}"))
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
    fn parse_args_returns_top_level_command_shape() {
        assert_eq!(parse_args(args(&["ripr"])), Ok(CliCommand::Help));
        assert_eq!(
            parse_args(args(&["ripr", "--version"])),
            Ok(CliCommand::Version)
        );
        assert_eq!(
            parse_args(args(&["ripr", "check", "--format", "json"])),
            Ok(CliCommand::Check(args(&["--format", "json"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "pilot", "--max-seams", "3"])),
            Ok(CliCommand::Pilot(args(&["--max-seams", "3"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "outcome", "--format", "json"])),
            Ok(CliCommand::Outcome(args(&["--format", "json"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "review-comments", "--base", "main"])),
            Ok(CliCommand::ReviewComments(args(&["--base", "main"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "gate", "evaluate"])),
            Ok(CliCommand::Gate(args(&["evaluate"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "pr-ledger", "record"])),
            Ok(CliCommand::PrLedger(args(&["record"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "pr-comments", "plan"])),
            Ok(CliCommand::PrComments(args(&["plan"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "coverage-grip", "frontier"])),
            Ok(CliCommand::CoverageGrip(args(&["frontier"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "assistant-loop", "proof"])),
            Ok(CliCommand::AssistantLoop(args(&["proof"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "first-action", "--root", "."])),
            Ok(CliCommand::FirstAction(args(&["--root", "."])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "reports", "index"])),
            Ok(CliCommand::Reports(args(&["index"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "calibrate", "cargo-mutants"])),
            Ok(CliCommand::Calibrate(args(&["cargo-mutants"])))
        );
        assert_eq!(
            parse_args(args(&["ripr", "agent", "brief", "--json"])),
            Ok(CliCommand::Agent(args(&["brief", "--json"])))
        );
    }

    #[test]
    fn parse_args_preserves_unknown_command_error() {
        assert_eq!(
            parse_args(args(&["ripr", "unknown"])),
            Err("unknown command \"unknown\". Run `ripr --help`.".to_string())
        );
    }

    struct ModeScenario {
        given_mode: &'static str,
        then_result: Result<Mode, String>,
    }

    #[test]
    fn given_mode_strings_when_parse_mode_then_returns_expected_result() {
        let scenarios = [
            ModeScenario {
                given_mode: "instant",
                then_result: Ok(Mode::Instant),
            },
            ModeScenario {
                given_mode: "draft",
                then_result: Ok(Mode::Draft),
            },
            ModeScenario {
                given_mode: "fast",
                then_result: Ok(Mode::Fast),
            },
            ModeScenario {
                given_mode: "deep",
                then_result: Ok(Mode::Deep),
            },
            ModeScenario {
                given_mode: "ready",
                then_result: Ok(Mode::Ready),
            },
            ModeScenario {
                given_mode: "slow",
                then_result: Err("unknown mode \"slow\"".to_string()),
            },
        ];

        for scenario in scenarios {
            let actual = parse_mode(scenario.given_mode);
            assert_eq!(
                actual, scenario.then_result,
                "mode scenario failed for given={:?}",
                scenario.given_mode
            );
        }
    }

    #[test]
    fn given_format_strings_when_parse_format_then_returns_expected_result() {
        assert_eq!(parse_format("human"), Ok(OutputFormat::Human));
        assert_eq!(parse_format("text"), Ok(OutputFormat::Human));
        assert_eq!(parse_format("json"), Ok(OutputFormat::Json));
        assert_eq!(
            parse_format("agent-seam-packets-json"),
            Ok(OutputFormat::AgentSeamPacketsJson)
        );
        assert_eq!(
            parse_format("xml"),
            Err("unknown format \"xml\"".to_string())
        );
    }

    #[test]
    fn given_args_and_index_when_expect_value_then_returns_value_or_missing_error() {
        let values = args(&["--diff", "sample.diff"]);

        let when_value_is_present = expect_value(&values, 1, "--diff");
        assert_eq!(when_value_is_present, Ok("sample.diff"));

        let when_value_is_missing = expect_value(&values, 2, "--diff");
        assert_eq!(
            when_value_is_missing,
            Err("missing value for --diff".to_string())
        );
    }
}
