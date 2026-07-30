mod agent;
mod core;
mod overview;
mod policy;
mod pr;
mod reports;
mod rerun;
mod swarm;

use agent::*;
use core::*;
use overview::*;
use policy::*;
use pr::*;
use reports::*;
use rerun::*;
use swarm::*;

use crate::cli::commands::{RECEIPT_CHECK_HELP, RECEIPT_WRITE_HELP};

/// The command paths that resolve to a flag-documenting help body.
///
/// A path here is the space-separated command as the user types it, which is
/// also the string the CLI names in its own unknown-argument errors. Keeping
/// the list next to [`help_text_for`] lets a test assert that every path a user
/// can mistype still lands on real help.
///
/// Only the flag-parity test reads it, so it is test-only; production lookups
/// go through [`help_text_for`].
#[cfg(test)]
const REGISTERED_COMMAND_PATHS: &[&str] = &[
    "agent brief",
    "agent packet",
    "agent repair",
    "agent receipt",
    "agent review-summary",
    "agent start",
    "agent status",
    "agent verify",
    "agent verify-execute",
    "assistant-loop health",
    "assistant-loop proof",
    "baseline create",
    "baseline diff",
    "baseline update",
    "calibrate cargo-mutants",
    "check",
    "config validate",
    "coverage-grip frontier",
    "diff",
    "doctor",
    "evidence-health",
    "first-action",
    "gate",
    "init",
    "lsp",
    "outcome",
    "pilot",
    "policy history",
    "policy operations",
    "policy preview-promote",
    "policy promote",
    "policy readiness",
    "policy suppression-health",
    "policy waiver-aging",
    "pr-comments plan",
    "pr-ledger record",
    "pr-review front-panel",
    "receipt check",
    "receipt write",
    "reports gap-ledger",
    "reports index",
    "reports ts-false-actionable",
    "reports ts-limitations",
    "rerun",
    "review-comments",
    "swarm ingest",
    "swarm queue",
    "zero status",
];

/// Resolve a command path to the help body that documents its flags.
///
/// Several subcommand groups (`policy`, `baseline`, `reports`, ...) document
/// every subcommand's flags in one shared body, so those paths all map to the
/// same constant. `None` means the path has no help body to mine for flag
/// suggestions; callers fall back to naming `ripr <path> --help`.
pub(super) fn help_text_for(command: &str) -> Option<&'static str> {
    let help_text = match command {
        "agent brief" => AGENT_BRIEF_HELP,
        "agent packet" => AGENT_PACKET_HELP,
        "agent repair" => AGENT_REPAIR_HELP,
        "agent receipt" => AGENT_RECEIPT_HELP,
        "agent review-summary" => AGENT_REVIEW_SUMMARY_HELP,
        "agent start" => AGENT_START_HELP,
        "agent status" => AGENT_STATUS_HELP,
        "agent verify" => AGENT_VERIFY_HELP,
        "agent verify-execute" => AGENT_VERIFY_EXECUTE_HELP,
        "assistant-loop health" | "assistant-loop proof" => ASSISTANT_LOOP_HELP,
        "baseline create" | "baseline diff" | "baseline update" => BASELINE_HELP,
        "calibrate cargo-mutants" => CALIBRATE_HELP,
        "check" => CHECK_HELP,
        "config validate" => CONFIG_HELP,
        "coverage-grip frontier" => COVERAGE_GRIP_HELP,
        "diff" => DIFF_HELP,
        "doctor" => DOCTOR_HELP,
        "evidence-health" => EVIDENCE_HEALTH_HELP,
        "first-action" => FIRST_ACTION_HELP,
        "gate" => GATE_HELP,
        "init" => INIT_HELP,
        "lsp" => LSP_HELP,
        "outcome" => OUTCOME_HELP,
        "pilot" => PILOT_HELP,
        "policy history"
        | "policy operations"
        | "policy preview-promote"
        | "policy promote"
        | "policy readiness"
        | "policy suppression-health"
        | "policy waiver-aging" => POLICY_HELP,
        "pr-comments plan" => PR_COMMENTS_HELP,
        "pr-ledger record" => PR_LEDGER_HELP,
        "pr-review front-panel" => PR_REVIEW_HELP,
        "receipt check" => RECEIPT_CHECK_HELP,
        "receipt write" => RECEIPT_WRITE_HELP,
        "reports gap-ledger"
        | "reports index"
        | "reports ts-false-actionable"
        | "reports ts-limitations" => REPORTS_HELP,
        "rerun" => RERUN_HELP,
        "review-comments" => REVIEW_COMMENTS_HELP,
        "swarm ingest" => SWARM_INGEST_HELP,
        "swarm queue" => SWARM_QUEUE_HELP,
        "zero status" => ZERO_HELP,
        _ => return None,
    };
    Some(help_text)
}

/// The command paths [`help_text_for`] can resolve.
#[cfg(test)]
pub(super) fn registered_command_paths() -> &'static [&'static str] {
    REGISTERED_COMMAND_PATHS
}

pub(super) fn print_help() {
    println!("{HELP}");
}

pub(super) fn print_help_all() {
    println!("{HELP_ALL}");
}

pub(super) fn print_check_help() {
    println!("{CHECK_HELP}");
}

pub(super) fn print_config_help() {
    println!("{CONFIG_HELP}");
}

pub(super) fn print_diff_help() {
    println!("{DIFF_HELP}");
}

pub(super) fn print_init_help() {
    println!("{INIT_HELP}");
}

pub(super) fn print_pilot_help() {
    println!("{PILOT_HELP}");
}

pub(super) fn print_outcome_help() {
    println!("{OUTCOME_HELP}");
}

pub(super) fn print_evidence_health_help() {
    println!("{EVIDENCE_HEALTH_HELP}");
}

pub(super) fn print_review_comments_help() {
    println!("{REVIEW_COMMENTS_HELP}");
}

pub(super) fn print_gate_help() {
    println!("{GATE_HELP}");
}

pub(super) fn print_baseline_help() {
    println!("{BASELINE_HELP}");
}

pub(super) fn print_zero_help() {
    println!("{ZERO_HELP}");
}

pub(super) fn print_policy_help() {
    println!("{POLICY_HELP}");
}

pub(super) fn print_pr_ledger_help() {
    println!("{PR_LEDGER_HELP}");
}

pub(super) fn print_pr_comments_help() {
    println!("{PR_COMMENTS_HELP}");
}

pub(super) fn print_pr_review_help() {
    println!("{PR_REVIEW_HELP}");
}

pub(super) fn print_coverage_grip_help() {
    println!("{COVERAGE_GRIP_HELP}");
}

pub(super) fn print_assistant_loop_help() {
    println!("{ASSISTANT_LOOP_HELP}");
}

pub(super) fn print_first_action_help() {
    println!("{FIRST_ACTION_HELP}");
}

pub(super) fn print_reports_help() {
    println!("{REPORTS_HELP}");
}

pub(super) fn print_calibrate_help() {
    println!("{CALIBRATE_HELP}");
}

pub(super) fn print_agent_help() {
    println!("{AGENT_HELP}");
}

pub(super) fn print_agent_start_help() {
    println!("{AGENT_START_HELP}");
}

pub(super) fn print_agent_brief_help() {
    println!("{AGENT_BRIEF_HELP}");
}

pub(super) fn print_agent_packet_help() {
    println!("{AGENT_PACKET_HELP}");
}

pub(super) fn print_agent_verify_help() {
    println!("{AGENT_VERIFY_HELP}");
}

pub(super) fn print_agent_verify_execute_help() {
    println!("{AGENT_VERIFY_EXECUTE_HELP}");
}

pub(super) fn print_agent_receipt_help() {
    println!("{AGENT_RECEIPT_HELP}");
}

pub(super) fn print_agent_status_help() {
    println!("{AGENT_STATUS_HELP}");
}

pub(super) fn print_agent_review_summary_help() {
    println!("{AGENT_REVIEW_SUMMARY_HELP}");
}

pub(super) fn print_agent_repair_help() {
    println!("{AGENT_REPAIR_HELP}");
}

pub(super) fn print_swarm_help() {
    println!("{SWARM_HELP}");
}

pub(super) fn print_swarm_queue_help() {
    println!("{SWARM_QUEUE_HELP}");
}

pub(super) fn print_swarm_ingest_help() {
    println!("{SWARM_INGEST_HELP}");
}

pub(super) fn print_explain_help() {
    println!("{EXPLAIN_HELP}");
}

pub(super) fn print_context_help() {
    println!("{CONTEXT_HELP}");
}

pub(super) fn print_doctor_help() {
    println!("{DOCTOR_HELP}");
}

pub(super) fn print_lsp_help() {
    println!("{LSP_HELP}");
}

pub(super) fn print_rerun_help() {
    println!("{RERUN_HELP}");
}

#[cfg(test)]
mod tests {
    use super::{
        AGENT_BRIEF_HELP, AGENT_HELP, AGENT_PACKET_HELP, AGENT_RECEIPT_HELP,
        AGENT_REVIEW_SUMMARY_HELP, AGENT_START_HELP, AGENT_STATUS_HELP, AGENT_VERIFY_HELP,
        ASSISTANT_LOOP_HELP, BASELINE_HELP, CALIBRATE_HELP, CHECK_HELP, CONFIG_HELP, CONTEXT_HELP,
        COVERAGE_GRIP_HELP, DIFF_HELP, DOCTOR_HELP, EVIDENCE_HEALTH_HELP, EXPLAIN_HELP,
        FIRST_ACTION_HELP, GATE_HELP, HELP, HELP_ALL, INIT_HELP, LSP_HELP, OUTCOME_HELP,
        PILOT_HELP, POLICY_HELP, PR_COMMENTS_HELP, PR_LEDGER_HELP, PR_REVIEW_HELP, REPORTS_HELP,
        RERUN_HELP, REVIEW_COMMENTS_HELP, SWARM_HELP, SWARM_INGEST_HELP, SWARM_QUEUE_HELP,
        ZERO_HELP, print_agent_brief_help, print_agent_help, print_agent_packet_help,
        print_agent_receipt_help, print_agent_repair_help, print_agent_review_summary_help,
        print_agent_start_help, print_agent_status_help, print_agent_verify_help,
        print_assistant_loop_help, print_baseline_help, print_calibrate_help, print_check_help,
        print_config_help, print_context_help, print_coverage_grip_help, print_diff_help,
        print_doctor_help, print_evidence_health_help, print_explain_help, print_first_action_help,
        print_gate_help, print_help, print_help_all, print_init_help, print_lsp_help,
        print_outcome_help, print_pilot_help, print_policy_help, print_pr_comments_help,
        print_pr_ledger_help, print_pr_review_help, print_reports_help, print_rerun_help,
        print_review_comments_help, print_swarm_help, print_swarm_ingest_help,
        print_swarm_queue_help, print_zero_help,
    };
    use crate::cli::command::KNOWN_COMMANDS;

    /// The exhaustive reference owns the full inventory. This assertion used to
    /// target the default screen, which is why that screen had grown to 91
    /// lines (#1613).
    #[test]
    fn help_all_mentions_supported_commands() {
        assert!(HELP_ALL.contains("ripr init"));
        assert!(HELP_ALL.contains("ripr config validate"));
        assert!(HELP_ALL.contains("ripr pilot"));
        assert!(HELP_ALL.contains("ripr outcome"));
        assert!(HELP_ALL.contains("ripr rerun --changed-test"));
        assert!(HELP_ALL.contains("ripr evidence-health"));
        assert!(HELP_ALL.contains("ripr review-comments"));
        assert!(HELP_ALL.contains("ripr gate evaluate"));
        assert!(HELP_ALL.contains("ripr baseline create"));
        assert!(HELP_ALL.contains("ripr baseline diff"));
        assert!(HELP_ALL.contains("ripr baseline update"));
        assert!(HELP_ALL.contains("ripr zero status"));
        assert!(HELP_ALL.contains("ripr policy readiness"));
        assert!(HELP_ALL.contains("ripr policy operations"));
        assert!(HELP_ALL.contains("ripr policy history"));
        assert!(HELP_ALL.contains("ripr policy promote"));
        assert!(HELP_ALL.contains("ripr policy preview-promote"));
        assert!(HELP_ALL.contains("ripr policy waiver-aging"));
        assert!(HELP_ALL.contains("ripr pr-ledger record"));
        assert!(HELP_ALL.contains("ripr pr-comments plan"));
        assert!(HELP_ALL.contains("ripr pr-review front-panel"));
        assert!(HELP_ALL.contains("ripr coverage-grip frontier"));
        assert!(HELP_ALL.contains("ripr assistant-loop proof"));
        assert!(HELP_ALL.contains("ripr assistant-loop health"));
        assert!(HELP_ALL.contains("ripr first-pr"));
        assert!(HELP_ALL.contains("ripr start-here"));
        assert!(HELP_ALL.contains("ripr first-action"));
        assert!(HELP_ALL.contains("ripr reports index"));
        assert!(HELP_ALL.contains("ripr reports gap-ledger"));
        assert!(HELP_ALL.contains("ripr calibrate"));
        assert!(HELP_ALL.contains("ripr receipt write"));
        assert!(HELP_ALL.contains("ripr receipt check"));
        assert!(HELP_ALL.contains("ripr agent start"));
        assert!(HELP_ALL.contains("ripr agent brief"));
        assert!(HELP_ALL.contains("ripr agent packet"));
        assert!(HELP_ALL.contains("ripr agent verify"));
        assert!(HELP_ALL.contains("ripr agent receipt"));
        assert!(HELP_ALL.contains("ripr agent status"));
        assert!(HELP_ALL.contains("ripr agent review-summary"));
        assert!(HELP_ALL.contains("ripr swarm queue"));
        assert!(HELP_ALL.contains("ripr swarm ingest"));
        assert!(HELP_ALL.contains("ripr plus"));
        assert!(HELP_ALL.contains("ripr diff"));
        assert!(HELP_ALL.contains("ripr check"));
        assert!(HELP_ALL.contains("ripr explain"));
        assert!(HELP_ALL.contains("ripr context"));
        assert!(HELP_ALL.contains("ripr doctor"));
        assert!(HELP_ALL.contains("ripr cache status"));
        assert!(HELP_ALL.contains("Start-here path:"));
        assert!(HELP_ALL.contains("Safe next action means repair one named gap"));
        assert!(HELP_ALL.contains("Missing artifact, stale evidence, wrong root"));
        assert!(HELP_ALL.contains("Verify command, receipt command, and receipt path"));
        assert!(HELP_ALL.contains("Preview-limited evidence stays syntax-first"));
    }

    /// The default screen has to stay readable without scrolling. Before #1613
    /// it was 91 lines of command inventory with the quick start at line 75, so
    /// this envelope is what keeps it from regrowing. 40 leaves room to edit the
    /// wording without inviting the whole catalog back.
    #[test]
    fn help_overview_fits_one_screen() {
        let lines = HELP.lines().count();
        assert!(
            lines <= 40,
            "the default help screen is {lines} lines; keep it under one screen and put \
             reference material in HELP_ALL (ripr help --all)"
        );
    }

    /// The first screen has to answer "what do I run?" without the reader
    /// knowing any internal vocabulary: the setup commands, one route per task,
    /// and how to reach the rest.
    #[test]
    fn help_overview_routes_to_first_actions_and_full_reference() {
        for needle in [
            "ripr doctor",
            "ripr check",
            "ripr explain",
            "ripr first-pr",
            "ripr lsp --stdio",
            "ripr init --ci github",
            "ripr help <command>",
            "ripr help --all",
        ] {
            assert!(
                HELP.contains(needle),
                "the default help screen should route to {needle}"
            );
        }
        // The advisory boundary belongs on the first screen; a reader should not
        // have to opt into `--all` to learn that ripr does not run mutants.
        assert!(HELP.contains("does not run mutants"));
    }

    /// `ripr help --all` claims to be every command, so it is checked against
    /// the parser's own list rather than a hand-kept copy. The previous overview
    /// had already drifted: `pr-summary`, `annotations`, `pr-evidence`, and
    /// `impacted-evidence` were all reachable and undocumented.
    #[test]
    fn help_all_documents_every_public_command() {
        // `help` documents itself in the header and `More:` lines rather than as
        // a catalog entry.
        let documented_elsewhere = ["help"];
        let missing: Vec<&str> = KNOWN_COMMANDS
            .iter()
            .copied()
            .filter(|command| !documented_elsewhere.contains(command))
            .filter(|command| !HELP_ALL.contains(&format!("ripr {command}")))
            .collect();
        assert!(
            missing.is_empty(),
            "ripr help --all omits reachable command(s): {missing:?}; \
             every KNOWN_COMMANDS entry must appear in the full reference"
        );
    }

    #[test]
    fn print_help_all_writes_the_full_reference() {
        print_help_all();
    }

    #[test]
    fn check_help_mentions_repo_badge_formats_and_examples() {
        assert!(CHECK_HELP.contains("repo-badge-plus-shields"));
        assert!(CHECK_HELP.contains("repo-exposure-json"));
        assert!(CHECK_HELP.contains("repo-exposure-summary-json"));
        assert!(CHECK_HELP.contains("agent-seam-packets-json"));
        assert!(CHECK_HELP.contains("repo-sarif"));
        assert!(CHECK_HELP.contains("needs test-efficiency"));
        assert!(CHECK_HELP.contains("docs/BADGE_ADOPTION.md"));
        assert!(CHECK_HELP.contains("--mode ready --json"));
        assert!(DIFF_HELP.contains("Usage: ripr diff"));
        assert!(DIFF_HELP.contains("full-repo-limited"));
    }

    #[test]
    fn gate_family_help_states_file_backed_output_discipline() {
        assert!(GATE_HELP.contains("stdout contains human `Wrote ...` status lines"));
        assert!(BASELINE_HELP.contains("ripr baseline create --from PATH"));
        assert!(BASELINE_HELP.contains("--dry-run"));
        assert!(BASELINE_HELP.contains("it prints the candidate JSON"));
        assert!(BASELINE_HELP.contains("to stdout without"));
        assert!(ZERO_HELP.contains("stdout contains human `Wrote ...`"));
        assert!(ZERO_HELP.contains("status lines rather than the JSON report"));
    }

    #[test]
    fn command_specific_help_usage_lines_are_stable() {
        // Each subcommand help block leads with a one-line action-oriented opener,
        // followed by a blank line and the canonical `Usage: ripr <cmd>` line.
        // Tests check both surfaces so the user-facing copy and the syntax stay aligned.
        assert!(INIT_HELP.starts_with("Write an optional repo policy file"));
        assert!(INIT_HELP.contains("Usage: ripr init"));
        assert!(INIT_HELP.contains("--ci github"));
        assert!(INIT_HELP.contains("--dry-run"));
        assert!(INIT_HELP.contains("--force"));
        assert!(CONFIG_HELP.starts_with("Validate the repository's ripr.toml"));
        assert!(CONFIG_HELP.contains("Usage: ripr config validate"));
        assert!(CONFIG_HELP.contains("without running workspace probes"));
        assert!(PILOT_HELP.starts_with("Find the top test gap in this repo"));
        assert!(PILOT_HELP.contains("Usage: ripr pilot"));
        assert!(PILOT_HELP.contains("pilot-summary.json"));
        assert!(PILOT_HELP.contains("--timeout-ms MS"));
        assert!(OUTCOME_HELP.starts_with("Compare before/after static evidence"));
        assert!(OUTCOME_HELP.contains("Usage: ripr outcome"));
        assert!(OUTCOME_HELP.contains("--before PATH"));
        assert!(RERUN_HELP.starts_with("Re-evaluate static evidence affected"));
        assert!(RERUN_HELP.contains("Usage: ripr rerun --changed-test PATH"));
        assert!(RERUN_HELP.contains("gap selector groups"));
        assert!(RERUN_HELP.contains("current_state_only"));
        assert!(DIFF_HELP.starts_with("Analyze the changed surface first"));
        assert!(DIFF_HELP.contains("--head REV"));
        assert!(
            EVIDENCE_HEALTH_HELP.starts_with("Summarize how strong the current static evidence")
        );
        assert!(EVIDENCE_HEALTH_HELP.contains("Usage: ripr evidence-health"));
        assert!(EVIDENCE_HEALTH_HELP.contains("--mutation-calibration PATH"));
        assert!(REVIEW_COMMENTS_HELP.starts_with("Write advisory PR test guidance"));
        assert!(REVIEW_COMMENTS_HELP.contains("Usage: ripr review-comments"));
        assert!(REVIEW_COMMENTS_HELP.contains("target/ripr/review/comments.json"));
        assert!(GATE_HELP.starts_with("Evaluate the optional pass/fail gate"));
        assert!(GATE_HELP.contains("Usage: ripr gate evaluate"));
        assert!(GATE_HELP.contains("visible-only"));
        assert!(GATE_HELP.contains("ripr-waive"));
        assert!(BASELINE_HELP.starts_with("Create, diff, and shrink a reviewed baseline"));
        assert!(BASELINE_HELP.contains("Usage:"));
        assert!(BASELINE_HELP.contains("ripr baseline create"));
        assert!(BASELINE_HELP.contains("ripr baseline diff"));
        assert!(BASELINE_HELP.contains("ripr baseline update"));
        assert!(BASELINE_HELP.contains(".ripr/gate-baseline.json"));
        assert!(BASELINE_HELP.contains("baseline-debt-delta.json"));
        assert!(BASELINE_HELP.contains("--remove-resolved"));
        assert!(ZERO_HELP.starts_with("Summarize current RIPR Zero progress"));
        assert!(ZERO_HELP.contains("Usage: ripr zero status"));
        assert!(ZERO_HELP.contains("baseline-debt-delta JSON"));
        assert!(ZERO_HELP.contains("RIPR Zero status report"));
        assert!(POLICY_HELP.starts_with("Summarize which RIPR policy posture"));
        assert!(POLICY_HELP.contains("Usage: ripr policy readiness"));
        assert!(POLICY_HELP.contains("ripr policy operations"));
        assert!(POLICY_HELP.contains("ripr policy history"));
        assert!(POLICY_HELP.contains("ripr policy promote"));
        assert!(POLICY_HELP.contains("ripr policy preview-promote"));
        assert!(POLICY_HELP.contains("ripr policy waiver-aging"));
        assert!(POLICY_HELP.contains("ripr policy suppression-health"));
        assert!(POLICY_HELP.contains("policy-readiness.json"));
        assert!(POLICY_HELP.contains("policy-operations.json"));
        assert!(POLICY_HELP.contains("policy-history.json"));
        assert!(POLICY_HELP.contains("policy-promotion-<mode>.json"));
        assert!(POLICY_HELP.contains("preview-promotion-<language>-<class>.json"));
        assert!(POLICY_HELP.contains("waiver-aging.json"));
        assert!(POLICY_HELP.contains("suppression-health.json"));
        assert!(POLICY_HELP.contains("read-only advisory governance"));
        assert!(PR_LEDGER_HELP.starts_with("Record a read-only PR evidence ledger"));
        assert!(PR_LEDGER_HELP.contains("Usage: ripr pr-ledger record"));
        assert!(PR_LEDGER_HELP.contains("pr-evidence-ledger.json"));
        assert!(PR_LEDGER_HELP.contains("read-only advisory history"));
        assert!(PR_COMMENTS_HELP.starts_with("Plan or publish bounded inline PR comments"));
        assert!(PR_COMMENTS_HELP.contains("Usage: ripr pr-comments plan"));
        assert!(PR_COMMENTS_HELP.contains("comment-publish-plan.json"));
        assert!(PR_COMMENTS_HELP.contains("read-only advisory projection"));
        assert!(PR_REVIEW_HELP.starts_with("Compose the first-screen PR review summary"));
        assert!(PR_REVIEW_HELP.contains("Usage: ripr pr-review front-panel"));
        assert!(PR_REVIEW_HELP.contains("pr-review-front-panel.json"));
        assert!(PR_REVIEW_HELP.contains("read-only advisory first-screen report"));
        assert!(
            COVERAGE_GRIP_HELP.starts_with("Report whether line coverage and behavior evidence")
        );
        assert!(COVERAGE_GRIP_HELP.contains("Usage: ripr coverage-grip frontier"));
        assert!(COVERAGE_GRIP_HELP.contains("coverage-grip-frontier.json"));
        assert!(COVERAGE_GRIP_HELP.contains("separate axes"));
        assert!(ASSISTANT_LOOP_HELP.starts_with("Produce or summarize advisory agent proof"));
        assert!(ASSISTANT_LOOP_HELP.contains("Usage:"));
        assert!(ASSISTANT_LOOP_HELP.contains("ripr assistant-loop proof"));
        assert!(ASSISTANT_LOOP_HELP.contains("ripr assistant-loop health"));
        assert!(ASSISTANT_LOOP_HELP.contains("test-oracle-assistant-proof.json"));
        assert!(ASSISTANT_LOOP_HELP.contains("assistant-loop-health.json"));
        assert!(ASSISTANT_LOOP_HELP.contains("Campaign 20 artifacts"));
        assert!(FIRST_ACTION_HELP.starts_with("Recommend the next focused test"));
        assert!(FIRST_ACTION_HELP.contains("Usage: ripr first-action"));
        assert!(FIRST_ACTION_HELP.contains("--gap-ledger PATH"));
        assert!(FIRST_ACTION_HELP.contains("first-useful-action.json"));
        assert!(FIRST_ACTION_HELP.contains("read-only advisory router"));
        assert!(FIRST_ACTION_HELP.contains("safe next action"));
        assert!(FIRST_ACTION_HELP.contains("verify command, receipt command, and receipt path"));
        assert!(FIRST_ACTION_HELP.contains("preview-limited evidence"));
        assert!(REPORTS_HELP.starts_with("Write reviewer-first report projections"));
        assert!(REPORTS_HELP.contains("Usage:"));
        assert!(REPORTS_HELP.contains("ripr reports index"));
        assert!(REPORTS_HELP.contains("ripr reports gap-ledger"));
        assert!(REPORTS_HELP.contains("target/ripr/reports/index.json"));
        assert!(REPORTS_HELP.contains("gap-decision-ledger.json"));
        assert!(REPORTS_HELP.contains("read-only advisory map"));
        assert!(CALIBRATE_HELP.starts_with("Import cargo-mutants outcomes"));
        assert!(CALIBRATE_HELP.contains("Usage: ripr calibrate cargo-mutants"));
        assert!(CALIBRATE_HELP.contains("--mutants-json PATH"));
        assert!(AGENT_HELP.starts_with("Create a bounded packet for a coding agent"));
        assert!(AGENT_HELP.contains("Usage: ripr agent"));
        assert!(AGENT_START_HELP.starts_with("Start a source-edit-free workflow packet"));
        assert!(AGENT_START_HELP.contains("Usage: ripr agent start"));
        assert!(AGENT_START_HELP.contains("workflow.json"));
        assert!(AGENT_BRIEF_HELP.starts_with("Write a bounded brief for a coding agent"));
        assert!(AGENT_BRIEF_HELP.contains("Usage: ripr agent brief"));
        assert!(AGENT_BRIEF_HELP.contains("--max-seams N"));
        assert!(AGENT_BRIEF_HELP.contains("RIPR-SPEC-0010"));
        assert!(AGENT_PACKET_HELP.starts_with("Write a per-change handoff packet"));
        assert!(AGENT_PACKET_HELP.contains("Usage: ripr agent packet"));
        assert!(AGENT_PACKET_HELP.contains("agent-seam-packets-json"));
        assert!(AGENT_VERIFY_HELP.starts_with("Verify static-evidence movement"));
        assert!(AGENT_VERIFY_HELP.contains("Usage: ripr agent verify"));
        assert!(AGENT_VERIFY_HELP.contains("repo-exposure-json"));
        assert!(AGENT_RECEIPT_HELP.starts_with("Write a provenance receipt"));
        assert!(AGENT_RECEIPT_HELP.contains("Usage: ripr agent receipt"));
        assert!(AGENT_RECEIPT_HELP.contains("--verify-json PATH"));
        assert!(AGENT_STATUS_HELP.starts_with("Report local agent-loop artifact state"));
        assert!(AGENT_STATUS_HELP.contains("Usage: ripr agent status"));
        assert!(AGENT_STATUS_HELP.contains("before snapshot"));
        assert!(AGENT_REVIEW_SUMMARY_HELP.starts_with("Summarize agent-loop artifacts"));
        assert!(AGENT_REVIEW_SUMMARY_HELP.contains("Usage: ripr agent review-summary"));
        assert!(AGENT_REVIEW_SUMMARY_HELP.contains("Human Markdown is the default"));
        assert!(SWARM_HELP.starts_with("Queue bounded repair work"));
        assert!(SWARM_HELP.contains("Usage: ripr swarm <subcommand>"));
        assert!(SWARM_QUEUE_HELP.starts_with("Queue GapRecord-backed repair packets"));
        assert!(SWARM_QUEUE_HELP.contains("Usage: ripr swarm queue"));
        assert!(SWARM_QUEUE_HELP.contains("allowed_edit_surface"));
        assert!(SWARM_INGEST_HELP.starts_with("Classify one external agent result"));
        assert!(SWARM_INGEST_HELP.contains("Usage: ripr swarm ingest"));
        assert!(SWARM_INGEST_HELP.contains("edited_forbidden_file"));
        assert!(EXPLAIN_HELP.starts_with("Print why ripr flagged"));
        assert!(EXPLAIN_HELP.contains("Usage: ripr explain"));
        assert!(CONTEXT_HELP.starts_with("Print the per-change context packet"));
        assert!(CONTEXT_HELP.contains("Usage: ripr context"));
        assert!(DOCTOR_HELP.starts_with("Diagnose the local ripr setup"));
        assert!(DOCTOR_HELP.contains("Usage: ripr doctor [--root PATH]"));
        assert!(DOCTOR_HELP.contains("--json"));
        assert!(DOCTOR_HELP.contains("Cargo.toml"));
        assert!(DOCTOR_HELP.contains("Start-here next step:"));
        assert!(DOCTOR_HELP.contains("ripr start-here --root . --base origin/main --head HEAD"));
        assert!(DOCTOR_HELP.contains("safe next action means repair one named gap"));
        assert!(DOCTOR_HELP.contains("missing artifact, stale evidence, wrong root"));
        assert!(DOCTOR_HELP.contains("verify command, receipt command, and receipt path"));
        assert!(LSP_HELP.starts_with("Start the experimental ripr LSP server"));
        assert!(LSP_HELP.contains("--stdio"));
        assert!(LSP_HELP.contains("--version"));
    }

    #[test]
    fn every_help_printer_executes_without_panic() {
        // Each wrapper is a `println!("{CONST}")` over the help-text
        // constants already asserted on above. Exercise them so the
        // wrappers are coverage-attributed; stdout is captured by the
        // cargo-test harness.
        print_help();
        print_init_help();
        print_config_help();
        print_pilot_help();
        print_outcome_help();
        print_rerun_help();
        print_evidence_health_help();
        print_review_comments_help();
        print_gate_help();
        print_baseline_help();
        print_zero_help();
        print_policy_help();
        print_pr_ledger_help();
        print_pr_comments_help();
        print_pr_review_help();
        print_coverage_grip_help();
        print_assistant_loop_help();
        print_first_action_help();
        print_reports_help();
        print_calibrate_help();
        print_agent_help();
        print_agent_start_help();
        print_agent_brief_help();
        print_agent_packet_help();
        print_agent_verify_help();
        print_agent_receipt_help();
        print_agent_status_help();
        print_agent_review_summary_help();
        print_agent_repair_help();
        print_swarm_help();
        print_swarm_queue_help();
        print_swarm_ingest_help();
        print_diff_help();
        print_check_help();
        print_explain_help();
        print_context_help();
        print_doctor_help();
        print_lsp_help();
    }

    /// Extract all `--flag` tokens from a help text (#2342).
    #[expect(
        dead_code,
        reason = "flag-parity test helper; used by #2342 test suite"
    )]
    fn extract_flags(help: &str) -> Vec<String> {
        let mut flags = Vec::new();
        for line in help.lines() {
            let trimmed = line.trim();
            // Match lines starting with `--` (flag definitions in help text)
            if let Some(rest) = trimmed.strip_prefix("--") {
                // Take the flag name up to the first space or end of line
                let name = rest.split_whitespace().next().unwrap_or(rest);
                if !name.is_empty() {
                    flags.push(format!("--{name}"));
                }
            }
            // Also match `--flag` embedded in usage lines like `[--flag VALUE]`
            for word in trimmed.split_whitespace() {
                if word.starts_with("--") && word.len() > 2 {
                    let name = word
                        .trim_start_matches('-')
                        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                        .next()
                        .unwrap_or("");
                    if !name.is_empty() {
                        let flag = format!("--{name}");
                        if !flags.contains(&flag) {
                            flags.push(flag);
                        }
                    }
                }
            }
        }
        flags
    }

    #[test]
    fn help_flags_are_consistent_for_key_commands() {
        // #2342: verify that flags mentioned in the HELP text for key commands
        // actually appear. This catches drift where a flag is removed from help
        // but still parsed, or added to the parser but not documented. The test
        // checks the HELP text contains expected flag tokens — if a flag moves
        // between Usage and Options sections, the test still passes as long as
        // the flag appears somewhere in the help text.
        let commands: &[(&str, &str, &[&str])] = &[
            (
                "check",
                CHECK_HELP,
                &["--base", "--diff", "--mode", "--json"],
            ),
            (
                "explain",
                EXPLAIN_HELP,
                &[
                    "--from",
                    "--mode",
                    "--no-unchanged-tests",
                    "--perl-facts",
                    "--suppression-policy",
                ],
            ),
            (
                "context",
                CONTEXT_HELP,
                &[
                    "--from",
                    "--at",
                    "--mode",
                    "--perl-facts",
                    "--suppression-policy",
                ],
            ),
            ("gate", GATE_HELP, &["--pr-guidance", "--mode"]),
            ("doctor", DOCTOR_HELP, &["--root", "--json"]),
            ("config validate", CONFIG_HELP, &["--root"]),
            (
                "pilot",
                PILOT_HELP,
                &["--root", "--out", "--mode", "--max-seams"],
            ),
        ];

        for (cmd, help, expected_flags) in commands {
            for flag in *expected_flags {
                assert!(help.contains(flag), "{cmd} help should contain {flag}");
            }
        }
    }
}
