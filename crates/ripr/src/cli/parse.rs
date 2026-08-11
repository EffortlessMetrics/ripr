mod format;
mod mode;
mod value;

use crate::cli::command::CliCommand;

pub(crate) use format::parse_format;
pub(crate) use mode::parse_mode;
pub(crate) use value::expect_value;

/// Whether argv requests the package version before a top-level command.
///
/// Only leading flags participate. A command-local contract such as
/// `ripr lsp --version` must continue to reach that command.
#[doc(hidden)]
pub fn top_level_version_requested(args: &[String]) -> bool {
    args.iter()
        .skip(1)
        .take_while(|arg| arg.starts_with('-'))
        .any(|arg| matches!(arg.as_str(), "--version" | "-V"))
}

pub(super) fn parse_args(args: Vec<String>) -> Result<CliCommand, String> {
    // Version is a process-level identity query. Resolve it before dispatch so
    // an output-looking flag (or a help-looking flag) cannot turn a version
    // request into analysis/help output. The CLI dispatch handles `--verbose`
    // separately; keeping this precedence in the parser makes the same rule
    // hold for library callers and installed-binary invocations.
    if top_level_version_requested(&args) {
        return Ok(CliCommand::Version);
    }

    let command = args.get(1).map(|s| s.as_str());
    let command_args = args.get(2..).map_or_else(Vec::new, <[String]>::to_vec);
    CliCommand::from_parts(command, command_args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, OutputFormat};

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
            parse_args(args(&[
                "ripr",
                "rerun",
                "--changed-test",
                "tests/pricing.rs"
            ])),
            Ok(CliCommand::Rerun(args(&[
                "--changed-test",
                "tests/pricing.rs"
            ])))
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
        assert_eq!(
            parse_args(args(&["ripr", "swarm", "queue", "--language", "python"])),
            Ok(CliCommand::Swarm(args(&["queue", "--language", "python"])))
        );
        assert_eq!(
            parse_args(args(&[
                "ripr",
                "swarm",
                "ingest",
                "--result",
                "agent-result.json"
            ])),
            Ok(CliCommand::Swarm(args(&[
                "ingest",
                "--result",
                "agent-result.json"
            ])))
        );
    }

    #[test]
    fn top_level_version_predicate_stops_at_command_boundary() {
        for argv in [
            args(&["ripr", "--version"]),
            args(&["ripr", "--help", "--version"]),
            args(&["ripr", "-v", "--version"]),
            args(&["ripr", "--version", "-v"]),
        ] {
            assert!(
                top_level_version_requested(&argv),
                "leading version flags should be top-level: {argv:?}"
            );
        }
        for argv in [
            args(&["ripr", "check", "--version"]),
            args(&["ripr", "lsp", "--version"]),
        ] {
            assert!(
                !top_level_version_requested(&argv),
                "command-local version flags must not be intercepted: {argv:?}"
            );
        }
    }

    #[test]
    fn version_precedes_help_and_output_looking_flags() {
        for argv in [
            args(&["ripr", "--version", "--json"]),
            args(&["ripr", "--json", "--version"]),
            args(&["ripr", "--version", "--help"]),
            args(&["ripr", "--verbose", "--version"]),
        ] {
            assert_eq!(
                parse_args(argv),
                Ok(CliCommand::Version),
                "version must be resolved before other top-level-looking flags"
            );
        }

        assert_eq!(
            parse_args(args(&["ripr", "lsp", "--version"])),
            Ok(CliCommand::Lsp(args(&["--version"]))),
            "command-local LSP version remains distinct from top-level version"
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
                then_result: Err(
                    "unknown mode \"slow\"; expected `instant`, `draft`, `fast`, `deep`, or `ready`"
                        .to_string(),
                ),
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
            parse_format("repo-exposure-summary-json"),
            Ok(OutputFormat::RepoExposureSummaryJson)
        );
        assert_eq!(
            parse_format("xml"),
            Err(
                "unknown format \"xml\"; see `ripr check --help` for the accepted formats"
                    .to_string()
            )
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
