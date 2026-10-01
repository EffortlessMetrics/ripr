use crate::cli::command::CliCommand;
use crate::cli::{CommandError, commands, help, help_json, rerun};

pub(super) fn execute(command: CliCommand) -> Result<(), CommandError> {
    match command {
        CliCommand::Help => {
            help::print_help();
            Ok(())
        }
        CliCommand::HelpAll => {
            help::print_help_all();
            Ok(())
        }
        CliCommand::HelpWorkflow(name) => {
            help::print_workflow(name.as_deref()).map_err(CommandError::from)
        }
        CliCommand::HelpJson => help_json::print_help_json().map_err(CommandError::from),
        CliCommand::Version => {
            println!("{}", crate::build_identity::version_line());
            Ok(())
        }
        CliCommand::Init(args) => commands::init(&args).map_err(CommandError::from),
        CliCommand::Config(args) => commands::config(&args).map_err(CommandError::from),
        CliCommand::Pilot(args) => commands::pilot(&args).map_err(CommandError::from),
        CliCommand::Outcome(args) => commands::outcome(&args).map_err(CommandError::from),
        CliCommand::EvidenceHealth(args) => {
            commands::evidence_health(&args).map_err(CommandError::from)
        }
        CliCommand::ReviewComments(args) => {
            commands::review_comments(&args).map_err(CommandError::from)
        }
        // Gate blocked decisions carry the Decision variant (exit code 3).
        CliCommand::Gate(args) => commands::gate(&args),
        CliCommand::Baseline(args) => commands::baseline(&args).map_err(CommandError::from),
        CliCommand::Zero(args) => commands::zero(&args).map_err(CommandError::from),
        CliCommand::Policy(args) => commands::policy(&args).map_err(CommandError::from),
        CliCommand::PrLedger(args) => commands::pr_ledger(&args).map_err(CommandError::from),
        CliCommand::PrComments(args) => commands::pr_comments(&args).map_err(CommandError::from),
        CliCommand::PrReview(args) => commands::pr_review(&args).map_err(CommandError::from),
        CliCommand::CoverageGrip(args) => {
            commands::coverage_grip(&args).map_err(CommandError::from)
        }
        CliCommand::AssistantLoop(args) => {
            commands::assistant_loop(&args).map_err(CommandError::from)
        }
        CliCommand::FirstPr(args) => commands::first_pr(&args).map_err(CommandError::from),
        CliCommand::FirstAction(args) => commands::first_action(&args).map_err(CommandError::from),
        CliCommand::Reports(args) => commands::reports(&args).map_err(CommandError::from),
        CliCommand::Calibrate(args) => commands::calibrate(&args).map_err(CommandError::from),
        CliCommand::Receipt(args) => commands::receipt(&args).map_err(CommandError::from),
        CliCommand::Feedback(args) => commands::feedback(&args).map_err(CommandError::from),
        // Agent typed refusals carry the Decision variant (exit code 3).
        CliCommand::Agent(args) => commands::agent(&args),
        CliCommand::Swarm(args) => commands::swarm(&args).map_err(CommandError::from),
        CliCommand::Diff(args) => commands::diff(&args).map_err(CommandError::from),
        CliCommand::Check(args) => commands::check(&args).map_err(CommandError::from),
        CliCommand::Explain(args) => commands::explain(&args).map_err(CommandError::from),
        CliCommand::Context(args) => commands::context(&args).map_err(CommandError::from),
        CliCommand::Doctor(args) => commands::doctor(&args).map_err(CommandError::from),
        CliCommand::Lsp(args) => commands::lsp(&args).map_err(CommandError::from),
        CliCommand::PrSummary(args) => commands::pr_summary(&args).map_err(CommandError::from),
        CliCommand::Annotations(args) => commands::annotations(&args).map_err(CommandError::from),
        CliCommand::PrEvidence(args) => commands::pr_evidence(&args).map_err(CommandError::from),
        CliCommand::ImpactedEvidence(args) => {
            commands::impacted_evidence(&args).map_err(CommandError::from)
        }
        CliCommand::RiprPlus(args) => commands::ripr_plus(&args).map_err(CommandError::from),
        CliCommand::Cache(args) => commands::cache(&args).map_err(CommandError::from),
        CliCommand::Rerun(args) => rerun::run(&args).map_err(CommandError::from),
        CliCommand::Mcp(args) => crate::mcp::run(&args).map_err(CommandError::from),
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
    fn execute_dispatches_rerun_parse_errors() {
        assert_eq!(
            execute(CliCommand::Rerun(Vec::new())),
            Err(CommandError::Failure(
                "rerun requires --changed-test <path> or --gap <canonical-gap-id> --gap-ledger <path>"
                    .to_string()
            ))
        );
    }

    #[test]
    fn execute_dispatches_subcommand_args_without_reparsing_argv() {
        assert_eq!(
            execute(CliCommand::Check(args(&["--format", "xml"]))),
            Err(CommandError::Failure(
                "unknown format \"xml\". Accepted: human, text, human-full, text-full, json, github, sarif, badge-json, badge-shields, badge-plus-json, badge-plus-shields, repo-badge-json, repo-badge-shields, repo-badge-plus-json, repo-badge-plus-shields, repo-seams-json, repo-seams-md, repo-exposure-json, repo-exposure-summary-json, repo-exposure-md, repo-sarif, agent-seam-packets-json."
                    .to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Doctor(args(&["--root"]))),
            Err(CommandError::Failure(
                "missing value for --root".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Init(args(&["--root"]))),
            Err(CommandError::Failure(
                "missing value for --root".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Config(args(&["validate", "--root"]))),
            Err(CommandError::Failure(
                "missing value for --root".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Policy(args(&["unknown"]))),
            Err(CommandError::Failure(
                "unknown policy subcommand \"unknown\"; expected `readiness`, `operations`, `history`, `promote`, `preview-promote`, `waiver-aging`, or `suppression-health`"
                    .to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Pilot(args(&["--max-seams", "0"]))),
            Err(CommandError::Failure(
                "--max-seams requires a positive integer; got \"0\"".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Outcome(args(&["--format", "xml"]))),
            Err(CommandError::Failure(
                "unknown outcome format \"xml\". Accepted: md, markdown, text, json.".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::EvidenceHealth(args(&["--root"]))),
            Err(CommandError::Failure(
                "missing value for --root".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::ReviewComments(args(&["--base"]))),
            Err(CommandError::Failure(
                "missing value for --base".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Gate(args(&["evaluate", "--mode", "strict"]))),
            Err(CommandError::Failure("unknown gate mode `strict`; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`".to_string()))
        );
        assert_eq!(
            execute(CliCommand::Baseline(args(&["create", "--from"]))),
            Err(CommandError::Failure(
                "missing value for --from".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Zero(args(&["status", "--delta"]))),
            Err(CommandError::Failure(
                "missing value for --delta".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::PrLedger(args(&["record", "--pr-number"]))),
            Err(CommandError::Failure(
                "missing value for --pr-number".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::PrComments(args(&["plan", "--mode"]))),
            Err(CommandError::Failure(
                "missing value for --mode".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::PrReview(args(&[
                "front-panel",
                "--first-action"
            ]))),
            Err(CommandError::Failure(
                "missing value for --first-action".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::CoverageGrip(args(&["frontier", "--ledger"]))),
            Err(CommandError::Failure(
                "missing value for --ledger".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::AssistantLoop(args(&["proof", "--pr-guidance"]))),
            Err(CommandError::Failure(
                "missing value for --pr-guidance".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::FirstPr(args(&["--gap-ledger"]))),
            Err(CommandError::Failure(
                "missing value for --gap-ledger".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::FirstAction(args(&["--pr-guidance"]))),
            Err(CommandError::Failure(
                "missing value for --pr-guidance".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Reports(args(&["index", "--reports-dir"]))),
            Err(CommandError::Failure(
                "missing value for --reports-dir".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Calibrate(args(&[
                "cargo-mutants",
                "--format",
                "xml"
            ]))),
            Err(CommandError::Failure(
                "unknown calibrate format \"xml\". Accepted: md, markdown, text, json.".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Receipt(args(&["unknown"]))),
            Err(CommandError::Failure(
                "unknown receipt subcommand \"unknown\"; expected `write` or `check`".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Agent(args(&["unknown"]))),
            Err(CommandError::Failure(
                "unknown agent subcommand \"unknown\"; expected `start`, `brief`, `packet`, `card`, `verify`, `verify-execute`, `receipt`, `status`, `review-summary`, or `repair`"
                    .to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Swarm(args(&["queue", "--top", "0"]))),
            Err(CommandError::Failure(
                "swarm queue --top requires a positive integer; got \"0\"".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Diff(args(&["--format", "xml"]))),
            Err(CommandError::Failure(
                "unknown diff format \"xml\"; expected `human`, `text`, `md`, `markdown`, or `json`".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Cache(Vec::new())),
            Err(CommandError::Failure(
                "cache requires subcommand `status` or `clear`".to_string()
            ))
        );
        assert_eq!(execute(CliCommand::Feedback(Vec::new())), Ok(()));
        assert_eq!(
            execute(CliCommand::Feedback(args(&["record"]))),
            Err(CommandError::Failure(
                "feedback record requires --snapshot; file and line are not a result identity"
                    .to_string()
            ))
        );
    }

    #[test]
    fn execute_dispatches_remaining_command_handlers() {
        assert_eq!(
            execute(CliCommand::Explain(Vec::new())),
            Err(CommandError::Failure(
                "missing finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Context(Vec::new())),
            Err(CommandError::Failure(
                "missing --at or --finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string()
            ))
        );
        assert_eq!(
            execute(CliCommand::Lsp(args(&["--bad"]))),
            Err(CommandError::Failure(
                "unknown lsp argument \"--bad\". Run `ripr lsp --help`.".to_string()
            ))
        );
    }
}
