use crate::cli::command::CliCommand;
use crate::cli::{commands, help};

pub(super) fn execute(command: CliCommand) -> Result<(), String> {
    match command {
        CliCommand::Help => {
            help::print_help();
            Ok(())
        }
        CliCommand::Version => {
            println!("ripr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliCommand::Init(args) => commands::init(&args),
        CliCommand::Pilot(args) => commands::pilot(&args),
        CliCommand::Outcome(args) => commands::outcome(&args),
        CliCommand::EvidenceHealth(args) => commands::evidence_health(&args),
        CliCommand::ReviewComments(args) => commands::review_comments(&args),
        CliCommand::Gate(args) => commands::gate(&args),
        CliCommand::Baseline(args) => commands::baseline(&args),
        CliCommand::Zero(args) => commands::zero(&args),
        CliCommand::PrLedger(args) => commands::pr_ledger(&args),
        CliCommand::CoverageGrip(args) => commands::coverage_grip(&args),
        CliCommand::AssistantLoop(args) => commands::assistant_loop(&args),
        CliCommand::FirstAction(args) => commands::first_action(&args),
        CliCommand::Calibrate(args) => commands::calibrate(&args),
        CliCommand::Agent(args) => commands::agent(&args),
        CliCommand::Check(args) => commands::check(&args),
        CliCommand::Explain(args) => commands::explain(&args),
        CliCommand::Context(args) => commands::context(&args),
        CliCommand::Doctor(args) => commands::doctor(&args),
        CliCommand::Lsp(args) => commands::lsp(&args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn execute_handles_top_level_help_and_version() {
        assert_eq!(execute(CliCommand::Help), Ok(()));
        assert_eq!(execute(CliCommand::Version), Ok(()));
    }

    #[test]
    fn execute_dispatches_subcommand_args_without_reparsing_argv() {
        assert_eq!(
            execute(CliCommand::Check(args(&["--format", "xml"]))),
            Err("unknown format \"xml\"".to_string())
        );
        assert_eq!(
            execute(CliCommand::Doctor(args(&["--root"]))),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            execute(CliCommand::Init(args(&["--root"]))),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            execute(CliCommand::Pilot(args(&["--max-seams", "0"]))),
            Err("invalid --max-seams: expected a positive integer".to_string())
        );
        assert_eq!(
            execute(CliCommand::Outcome(args(&["--format", "xml"]))),
            Err("unknown outcome format \"xml\"".to_string())
        );
        assert_eq!(
            execute(CliCommand::EvidenceHealth(args(&["--root"]))),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            execute(CliCommand::ReviewComments(args(&["--base"]))),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            execute(CliCommand::Gate(args(&["evaluate", "--mode", "strict"]))),
            Err("unknown gate mode `strict`".to_string())
        );
        assert_eq!(
            execute(CliCommand::Baseline(args(&["create", "--from"]))),
            Err("missing value for --from".to_string())
        );
        assert_eq!(
            execute(CliCommand::Zero(args(&["status", "--delta"]))),
            Err("missing value for --delta".to_string())
        );
        assert_eq!(
            execute(CliCommand::PrLedger(args(&["record", "--pr-number"]))),
            Err("missing value for --pr-number".to_string())
        );
        assert_eq!(
            execute(CliCommand::CoverageGrip(args(&["frontier", "--ledger"]))),
            Err("missing value for --ledger".to_string())
        );
        assert_eq!(
            execute(CliCommand::AssistantLoop(args(&["proof", "--pr-guidance"]))),
            Err("missing value for --pr-guidance".to_string())
        );
        assert_eq!(
            execute(CliCommand::FirstAction(args(&["--pr-guidance"]))),
            Err("missing value for --pr-guidance".to_string())
        );
        assert_eq!(
            execute(CliCommand::Calibrate(args(&[
                "cargo-mutants",
                "--format",
                "xml"
            ]))),
            Err("unknown calibrate format \"xml\"".to_string())
        );
        assert_eq!(
            execute(CliCommand::Agent(args(&["unknown"]))),
            Err(
                "unknown agent subcommand \"unknown\"; expected `start`, `brief`, `packet`, `verify`, `receipt`, `status`, or `review-summary`"
                    .to_string()
            )
        );
    }

    #[test]
    fn execute_dispatches_remaining_command_handlers() {
        assert_eq!(
            execute(CliCommand::Explain(Vec::new())),
            Err("missing finding selector".to_string())
        );
        assert_eq!(
            execute(CliCommand::Context(Vec::new())),
            Err("missing --at or --finding selector".to_string())
        );
        assert_eq!(
            execute(CliCommand::Lsp(args(&["--bad"]))),
            Err("unknown lsp argument \"--bad\"".to_string())
        );
    }
}
