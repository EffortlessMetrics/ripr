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

use crate::app::annotations::ANNOTATIONS_HELP;
use crate::app::impacted_evidence::IMPACTED_EVIDENCE_HELP;
use crate::app::pr_evidence::PR_EVIDENCE_HELP;
use crate::app::pr_summary::PR_SUMMARY_HELP;
use crate::app::ripr_plus::PLUS_HELP;
use crate::cli::commands::{
    CACHE_CLEAR_HELP, CACHE_STATUS_HELP, FEEDBACK_EXPORT_HELP, FEEDBACK_RECORD_HELP,
    RECEIPT_CHECK_HELP, RECEIPT_WRITE_HELP,
};
use crate::output::first_pr::FIRST_PR_HELP;

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
    "agent card",
    "agent packet",
    "agent repair",
    "agent receipt",
    "agent review-summary",
    "agent start",
    "agent status",
    "agent verify",
    "agent verify-execute",
    "annotations",
    "assistant-loop health",
    "assistant-loop proof",
    "baseline create",
    "baseline diff",
    "baseline update",
    "cache clear",
    "cache status",
    "calibrate cargo-mutants",
    "check",
    "config validate",
    "context",
    "coverage-grip frontier",
    "diff",
    "doctor",
    "evidence-health",
    "explain",
    "feedback export",
    "feedback record",
    "first-action",
    "first-pr",
    "gate",
    "impacted-evidence",
    "init",
    "lsp",
    "mcp",
    "outcome",
    "pilot",
    "plus",
    "policy history",
    "policy operations",
    "policy preview-promote",
    "policy promote",
    "policy readiness",
    "policy suppression-health",
    "policy waiver-aging",
    "pr-comments plan",
    "pr-evidence",
    "pr-ledger record",
    "pr-review front-panel",
    "pr-summary",
    "receipt check",
    "receipt write",
    "reports ci-summary",
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
        "agent card" => AGENT_CARD_HELP,
        "agent brief" => AGENT_BRIEF_HELP,
        "agent packet" => AGENT_PACKET_HELP,
        "agent repair" => AGENT_REPAIR_HELP,
        "agent receipt" => AGENT_RECEIPT_HELP,
        "agent review-summary" => AGENT_REVIEW_SUMMARY_HELP,
        "agent start" => AGENT_START_HELP,
        "agent status" => AGENT_STATUS_HELP,
        "agent verify" => AGENT_VERIFY_HELP,
        "agent verify-execute" => AGENT_VERIFY_EXECUTE_HELP,
        "annotations" => ANNOTATIONS_HELP,
        "assistant-loop health" | "assistant-loop proof" => ASSISTANT_LOOP_HELP,
        "baseline create" | "baseline diff" | "baseline update" => BASELINE_HELP,
        "cache clear" => CACHE_CLEAR_HELP,
        "cache status" => CACHE_STATUS_HELP,
        "calibrate cargo-mutants" => CALIBRATE_HELP,
        "check" => CHECK_HELP,
        "config validate" => CONFIG_HELP,
        "context" => CONTEXT_HELP,
        "coverage-grip frontier" => COVERAGE_GRIP_HELP,
        "diff" => DIFF_HELP,
        "doctor" => DOCTOR_HELP,
        "evidence-health" => EVIDENCE_HEALTH_HELP,
        "explain" => EXPLAIN_HELP,
        "feedback export" => FEEDBACK_EXPORT_HELP,
        "feedback record" => FEEDBACK_RECORD_HELP,
        "first-action" => FIRST_ACTION_HELP,
        "first-pr" => FIRST_PR_HELP,
        "gate" => GATE_HELP,
        "impacted-evidence" => IMPACTED_EVIDENCE_HELP,
        "init" => INIT_HELP,
        "lsp" => LSP_HELP,
        "mcp" => crate::mcp::MCP_HELP,
        "outcome" => OUTCOME_HELP,
        "pilot" => PILOT_HELP,
        "plus" => PLUS_HELP,
        "policy history"
        | "policy operations"
        | "policy preview-promote"
        | "policy promote"
        | "policy readiness"
        | "policy suppression-health"
        | "policy waiver-aging" => POLICY_HELP,
        "pr-comments plan" => PR_COMMENTS_HELP,
        "pr-evidence" => PR_EVIDENCE_HELP,
        "pr-ledger record" => PR_LEDGER_HELP,
        "pr-review front-panel" => PR_REVIEW_HELP,
        "pr-summary" => PR_SUMMARY_HELP,
        "receipt check" => RECEIPT_CHECK_HELP,
        "receipt write" => RECEIPT_WRITE_HELP,
        "reports ci-summary"
        | "reports gap-ledger"
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

/// `ripr help workflow [name]`: bounded, non-executing workflow guidance
/// (RIPR-SPEC-0189). The render is pure static text from the typed workflow
/// catalog; it performs no repository discovery, analysis, compilation, test,
/// child-process, network, mutation, or product-artifact work.
pub(super) fn print_workflow(name: Option<&str>) -> Result<(), String> {
    let rendered = match name {
        Some(name) => crate::cli::workflow_catalog::render_workflow(name)?,
        None => crate::cli::workflow_catalog::render_workflow_listing(),
    };
    println!("{rendered}");
    Ok(())
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

pub(super) fn print_agent_card_help() {
    println!("{AGENT_CARD_HELP}");
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

/// Test-only access to the two rendered human discovery surfaces, for the
/// projection-agreement checks in `command_metadata` (issue #4823).
#[cfg(test)]
pub(crate) fn discovery_surfaces() -> (&'static str, &'static str) {
    (overview::HELP, overview::HELP_ALL)
}

#[cfg(test)]
mod tests {
    #[test]
    fn review_comments_help_discloses_cooperative_budget_boundary() -> Result<(), String> {
        let text = super::help_text_for("review-comments").ok_or("missing review-comments help")?;
        for required in [
            "default 120000ms",
            "safe boundaries",
            "Non-preemptible operations can overrun",
            "outer orchestration wrapper for a hard process bound",
        ] {
            if !text.contains(required) {
                return Err(format!("review-comments help omitted {required:?}"));
            }
        }
        Ok(())
    }
    use super::{
        AGENT_BRIEF_HELP, AGENT_CARD_HELP, AGENT_HELP, AGENT_PACKET_HELP, AGENT_RECEIPT_HELP,
        AGENT_REVIEW_SUMMARY_HELP, AGENT_START_HELP, AGENT_STATUS_HELP, AGENT_VERIFY_HELP,
        ANNOTATIONS_HELP, ASSISTANT_LOOP_HELP, BASELINE_HELP, CACHE_CLEAR_HELP, CACHE_STATUS_HELP,
        CALIBRATE_HELP, CHECK_HELP, CONFIG_HELP, CONTEXT_HELP, COVERAGE_GRIP_HELP, DIFF_HELP,
        DOCTOR_HELP, EVIDENCE_HEALTH_HELP, EXPLAIN_HELP, FEEDBACK_EXPORT_HELP,
        FEEDBACK_RECORD_HELP, FIRST_ACTION_HELP, FIRST_PR_HELP, GATE_HELP, HELP, HELP_ALL,
        IMPACTED_EVIDENCE_HELP, INIT_HELP, LSP_HELP, OUTCOME_HELP, PILOT_HELP, PLUS_HELP,
        POLICY_HELP, PR_COMMENTS_HELP, PR_EVIDENCE_HELP, PR_LEDGER_HELP, PR_REVIEW_HELP,
        PR_SUMMARY_HELP, REPORTS_HELP, RERUN_HELP, REVIEW_COMMENTS_HELP, SWARM_HELP,
        SWARM_INGEST_HELP, SWARM_QUEUE_HELP, ZERO_HELP, print_agent_brief_help,
        print_agent_card_help, print_agent_help, print_agent_packet_help, print_agent_receipt_help,
        print_agent_repair_help, print_agent_review_summary_help, print_agent_start_help,
        print_agent_status_help, print_agent_verify_help, print_assistant_loop_help,
        print_baseline_help, print_calibrate_help, print_check_help, print_config_help,
        print_context_help, print_coverage_grip_help, print_diff_help, print_doctor_help,
        print_evidence_health_help, print_explain_help, print_first_action_help, print_gate_help,
        print_help, print_help_all, print_init_help, print_lsp_help, print_outcome_help,
        print_pilot_help, print_policy_help, print_pr_comments_help, print_pr_ledger_help,
        print_pr_review_help, print_reports_help, print_rerun_help, print_review_comments_help,
        print_swarm_help, print_swarm_ingest_help, print_swarm_queue_help, print_zero_help,
    };
    use crate::cli::command::known_commands;

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
        assert!(HELP_ALL.contains("ripr feedback record"));
        assert!(HELP_ALL.contains("ripr feedback export"));
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
        assert!(HELP_ALL.contains("ripr cache clear"));
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
        let missing: Vec<&str> = known_commands()
            .into_iter()
            .filter(|command| !documented_elsewhere.contains(command))
            .filter(|command| !HELP_ALL.contains(&format!("ripr {command}")))
            .collect();
        assert!(
            missing.is_empty(),
            "ripr help --all omits reachable command(s): {missing:?}; \
             every catalog-derived known command must appear in the full reference"
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
        assert!(CHECK_HELP.contains("walks up to a Cargo.toml containing [workspace]"));
        assert!(CHECK_HELP.contains("needs test-efficiency"));
        assert!(CHECK_HELP.contains("docs/BADGE_ADOPTION.md"));
        assert!(CHECK_HELP.contains("--mode ready --json"));
        assert!(CHECK_HELP.contains("--quiet"));
        assert!(CHECK_HELP.contains("ripr progress:"));
        assert!(CHECK_HELP.contains("percentage or ETA"));
        assert!(CHECK_HELP.contains("does not mean analysis is faster"));
        assert!(CHECK_HELP.contains("json, sarif, github"));
        assert!(!CHECK_HELP.contains("json, sarif, markdown"));
        assert!(DIFF_HELP.contains("Usage: ripr diff"));
        assert!(DIFF_HELP.contains("full-repo-limited"));
    }

    #[test]
    fn check_help_names_the_latency_trace_env_and_mode_cost_class() {
        // #4946(d): every repo-exposure env var a check user can set is
        // documented on the same "Environment variables:" surface; this one
        // was the last omission.
        assert!(CHECK_HELP.contains("RIPR_REPO_EXPOSURE_LATENCY_TRACE"));
        assert!(CHECK_HELP.contains("emits diagnostic phase/cache"));
        assert!(CHECK_HELP.contains("Presence enables tracing"));
        // #4946(c): the --mode entry orients on the cost class, not only the
        // index scope, so a caller can predict the wall-clock difference.
        assert!(CHECK_HELP.contains("Cost class: whole-workspace modes"));
        assert!(CHECK_HELP.contains("order of magnitude longer"));
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
        assert!(CONFIG_HELP.contains("ancestor-aware loader"));
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
        assert!(PR_LEDGER_HELP.contains("--out-jsonl"));
        assert!(POLICY_HELP.contains("--out-jsonl"));
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
        assert!(AGENT_HELP.starts_with("Create a bounded repair transaction for a coding agent"));
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
        assert!(
            AGENT_CARD_HELP.starts_with("Hand off one seam as the compact default repair card")
        );
        assert!(AGENT_CARD_HELP.contains("Usage: ripr agent card"));
        assert!(AGENT_CARD_HELP.contains("repair_card.v1"));
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
        // The compose commands take whatever base the caller has; this screen
        // no longer asserts `origin/main`, which does not exist in a repository
        // whose default branch is not `main`.
        assert!(DOCTOR_HELP.contains("ripr start-here --root . --base <ref> --head HEAD"));
        assert!(!DOCTOR_HELP.contains("--base origin/main"));
        assert!(DOCTOR_HELP.contains("safe next action means repair one named gap"));
        assert!(DOCTOR_HELP.contains("missing artifact, stale evidence, wrong root"));
        assert!(DOCTOR_HELP.contains("verify command, receipt command, and receipt path"));
        assert!(LSP_HELP.starts_with("Start the experimental ripr LSP server"));
        assert!(LSP_HELP.contains("--stdio"));
        assert!(LSP_HELP.contains("--version"));
        assert!(LSP_HELP.contains("ripr.collectContext           one object"));
        assert!(LSP_HELP.contains("InvalidParams"));
        // Pin: cache status/clear help lives beside the parser and is imported
        // here. Drop CACHE_STATUS_HELP / CACHE_CLEAR_HELP from this test module
        // import list and this test fails to compile.
        assert_eq!(
            super::help_text_for("cache status"),
            Some(CACHE_STATUS_HELP)
        );
        assert_eq!(super::help_text_for("cache clear"), Some(CACHE_CLEAR_HELP));
        assert!(
            CACHE_STATUS_HELP
                .lines()
                .any(|line| line.trim_start().starts_with("--json")),
            "cache status help must document --json on an option-list line: {CACHE_STATUS_HELP}"
        );
        assert!(
            CACHE_CLEAR_HELP
                .lines()
                .any(|line| line.trim_start().starts_with("--dry-run")),
            "cache clear help must document --dry-run on an option-list line: {CACHE_CLEAR_HELP}"
        );
        assert!(
            CACHE_CLEAR_HELP
                .lines()
                .any(|line| line.trim_start().starts_with("--force")),
            "cache clear help must document --force on an option-list line: {CACHE_CLEAR_HELP}"
        );
        // Pin: these six public commands live beside their parsers and are
        // imported here so unknown-flag suggestions mine the same body --help
        // prints. Drop a constant from this import list and this test fails
        // to compile.
        assert_eq!(super::help_text_for("first-pr"), Some(FIRST_PR_HELP));
        assert_eq!(super::help_text_for("pr-summary"), Some(PR_SUMMARY_HELP));
        assert_eq!(super::help_text_for("annotations"), Some(ANNOTATIONS_HELP));
        assert_eq!(super::help_text_for("pr-evidence"), Some(PR_EVIDENCE_HELP));
        assert_eq!(
            super::help_text_for("impacted-evidence"),
            Some(IMPACTED_EVIDENCE_HELP)
        );
        assert_eq!(super::help_text_for("plus"), Some(PLUS_HELP));
        assert_eq!(
            super::help_text_for("feedback record"),
            Some(FEEDBACK_RECORD_HELP)
        );
        assert_eq!(
            super::help_text_for("feedback export"),
            Some(FEEDBACK_EXPORT_HELP)
        );
        for (name, help_text, flag) in [
            ("first-pr", FIRST_PR_HELP, "--gap-ledger"),
            ("pr-summary", PR_SUMMARY_HELP, "--baseline"),
            ("annotations", ANNOTATIONS_HELP, "--comments"),
            ("pr-evidence", PR_EVIDENCE_HELP, "--head"),
            ("impacted-evidence", IMPACTED_EVIDENCE_HELP, "--pr-evidence"),
            ("plus", PLUS_HELP, "--repo-exposure-summary"),
        ] {
            assert!(
                help_text
                    .lines()
                    .any(|line| line.trim_start().starts_with(flag)),
                "{name} help must document {flag} on an option-list line: {help_text}"
            );
        }
    }

    /// The Usage synopsis names every Record option, so a flag the parser
    /// accepts is never documented only below the fold (#4391).
    #[test]
    fn pr_ledger_usage_names_every_record_option() {
        let usage = PR_LEDGER_HELP
            .lines()
            .find(|line| line.starts_with("Usage: ripr pr-ledger record"))
            .unwrap_or_default();
        let options: Vec<&str> = PR_LEDGER_HELP
            .split("Record options:")
            .nth(1)
            .unwrap_or_default()
            .lines()
            .skip(1)
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| line.split_whitespace().next())
            .filter(|flag| flag.starts_with("--"))
            .collect();
        assert!(
            !options.is_empty(),
            "Record options section must list flags"
        );
        for flag in options {
            assert!(
                usage.contains(&format!("{flag} ")) || usage.ends_with(flag),
                "Usage line omits {flag}: {usage}"
            );
        }
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
        print_agent_card_help();
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

    /// #5009: the global `-v`/`--verbose` flag is extracted before every
    /// command parser, so the per-command parity gate below cannot see it.
    /// Pin its disclosure against the extraction owner itself: every
    /// spelling the single stripping pass accepts must appear on the
    /// `ripr help --all` reference, which also discloses what verbose mode
    /// adds and the any-position contract. A new global spelling therefore
    /// cannot land without a help disclosure in the same PR.
    #[test]
    fn global_verbose_spellings_are_documented_on_help_all() -> Result<(), String> {
        let parse_rs = include_str!("parse.rs");
        let (skeleton, literals) = scan_rust_source(parse_rs);
        let (body_start, body_end) = function_body_span(&skeleton, "extract_global_verbose")
            .ok_or_else(|| "extract_global_verbose body not found in parse.rs".to_string())?;
        let mut accepted: Vec<&str> = Vec::new();
        for (at, literal) in &literals {
            if *at >= body_start && *at < body_end && !accepted.contains(&literal.as_str()) {
                accepted.push(literal.as_str());
            }
        }
        if accepted != ["--verbose", "-v"] {
            return Err(format!(
                "extract_global_verbose accepts {accepted:?}; update the owner and this \
                 gate together with the help --all disclosure"
            ));
        }
        for spelling in accepted {
            if !HELP_ALL.contains(spelling) {
                return Err(format!(
                    "help --all omits the global flag spelling {spelling:?}"
                ));
            }
        }
        for required in ["any position", "stderr"] {
            if !HELP_ALL.contains(required) {
                return Err(format!(
                    "help --all global-flags entry omits the disclosure {required:?}"
                ));
            }
        }
        Ok(())
    }

    // ── flag/help parity gate (#2342, revived by #4317) ──────────────────────

    /// `include_str!` of every file that owns a command's argv parsing. Paths
    /// resolve relative to this file's directory (`src/cli`), so the parity
    /// gate reads the exact parser text that ships in this crate and cannot
    /// drift away from it.
    const AGENT_PARSER_RS: &str = include_str!("agent.rs");
    const CLI_COMMANDS_RS: &str = include_str!("commands.rs");
    const CHECK_PARSER_RS: &str = include_str!("commands/check.rs");
    const CI_SUMMARY_PARSER_RS: &str = include_str!("commands/ci_summary.rs");
    const REVIEW_COMMENTS_PARSER_RS: &str = include_str!("commands/review_comments.rs");
    const CONTEXT_PARSER_RS: &str = include_str!("commands/context.rs");
    const CONFIG_PARSER_RS: &str = include_str!("commands/config.rs");
    const DOCTOR_PARSER_RS: &str = include_str!("commands/doctor.rs");
    const GATE_PARSER_RS: &str = include_str!("commands/gate.rs");
    const INIT_PARSER_RS: &str = include_str!("commands/init.rs");
    const PILOT_PARSER_RS: &str = include_str!("commands/pilot.rs");
    const BASELINE_PARSER_RS: &str = include_str!("commands/baseline.rs");
    const CACHE_PARSER_RS: &str = include_str!("commands/cache.rs");
    const RECEIPT_PARSER_RS: &str = include_str!("commands/receipt.rs");
    const FEEDBACK_PARSER_RS: &str = include_str!("commands/feedback.rs");
    const POLICY_PARSE_RS: &str = include_str!("commands/policy/parse.rs");
    const SWARM_QUEUE_PARSER_RS: &str = include_str!("commands/swarm/queue.rs");
    const SWARM_INGEST_PARSER_RS: &str = include_str!("commands/swarm/ingest.rs");
    const RERUN_PARSER_RS: &str = include_str!("rerun.rs");
    const MCP_PARSER_RS: &str = include_str!("../mcp/mod.rs");
    const PR_SUMMARY_PARSER_RS: &str = include_str!("../app/pr_summary/mod.rs");
    const ANNOTATIONS_PARSER_RS: &str = include_str!("../app/annotations.rs");
    const PR_EVIDENCE_PARSER_RS: &str = include_str!("../app/pr_evidence.rs");
    const IMPACTED_EVIDENCE_PARSER_RS: &str = include_str!("../app/impacted_evidence.rs");
    const RIPR_PLUS_PARSER_RS: &str = include_str!("../app/ripr_plus.rs");
    const FIRST_PR_PARSER_RS: &str = include_str!("../output/first_pr/options.rs");

    /// Each registered command path and the function(s) in its parser source
    /// that accept argv flags.
    ///
    /// The accepted set is the flag-shaped string literals (`--name`) inside
    /// those function bodies, so a new parse arm joins the parity comparison
    /// with no second edit, and a renamed or deleted function fails the gate
    /// loudly instead of silently skipping the command. Commands whose flags
    /// are parsed behind a dispatcher keep the dispatcher's function only
    /// when it accepts flags of its own (`doctor`, `lsp`); pure subcommand
    /// routers add nothing.
    const PARSER_SOURCES: &[(&str, &str, &[&str])] = &[
        (
            "agent brief",
            AGENT_PARSER_RS,
            &["parse_agent_brief_options"],
        ),
        (
            "agent packet",
            AGENT_PARSER_RS,
            &["parse_agent_packet_options"],
        ),
        ("agent card", AGENT_PARSER_RS, &["parse_agent_card_options"]),
        (
            "agent repair",
            AGENT_PARSER_RS,
            &["parse_agent_repair_command"],
        ),
        (
            "agent receipt",
            AGENT_PARSER_RS,
            &["parse_agent_receipt_options"],
        ),
        (
            "agent review-summary",
            AGENT_PARSER_RS,
            &["parse_agent_review_summary_options"],
        ),
        (
            "agent start",
            AGENT_PARSER_RS,
            &["parse_agent_start_options"],
        ),
        (
            "agent status",
            AGENT_PARSER_RS,
            &["parse_agent_status_options"],
        ),
        (
            "agent verify",
            AGENT_PARSER_RS,
            &["parse_agent_verify_options"],
        ),
        (
            "agent verify-execute",
            AGENT_PARSER_RS,
            &["parse_agent_verify_execute_options"],
        ),
        (
            "annotations",
            ANNOTATIONS_PARSER_RS,
            &["run_annotations", "parse_options"],
        ),
        (
            "assistant-loop health",
            CLI_COMMANDS_RS,
            &["parse_assistant_loop_health_options"],
        ),
        (
            "assistant-loop proof",
            CLI_COMMANDS_RS,
            &["parse_assistant_loop_proof_options"],
        ),
        (
            "baseline create",
            BASELINE_PARSER_RS,
            // The create flags are matched in the parse-state's apply_arg
            // method, not in the free parse function.
            &["parse_baseline_create_options", "apply_arg"],
        ),
        (
            "baseline diff",
            BASELINE_PARSER_RS,
            &["parse_baseline_diff_options"],
        ),
        (
            "baseline update",
            BASELINE_PARSER_RS,
            &["parse_baseline_update_options"],
        ),
        ("cache clear", CACHE_PARSER_RS, &["parse_clear_args"]),
        ("cache status", CACHE_PARSER_RS, &["parse_status_args"]),
        (
            "calibrate cargo-mutants",
            CLI_COMMANDS_RS,
            &["parse_calibrate_cargo_mutants_options"],
        ),
        ("check", CHECK_PARSER_RS, &["check"]),
        (
            "config validate",
            CONFIG_PARSER_RS,
            &["config", "parse_validate_root"],
        ),
        ("context", CONTEXT_PARSER_RS, &["context"]),
        (
            "coverage-grip frontier",
            CLI_COMMANDS_RS,
            &["parse_coverage_grip_frontier_options"],
        ),
        ("diff", CLI_COMMANDS_RS, &["parse_diff_options"]),
        ("doctor", DOCTOR_PARSER_RS, &["doctor"]),
        (
            "evidence-health",
            CLI_COMMANDS_RS,
            &["parse_evidence_health_options"],
        ),
        ("explain", CLI_COMMANDS_RS, &["explain"]),
        (
            "feedback export",
            FEEDBACK_PARSER_RS,
            &["parse_export_options"],
        ),
        (
            "feedback record",
            FEEDBACK_PARSER_RS,
            &["parse_record_options"],
        ),
        (
            "first-action",
            CLI_COMMANDS_RS,
            &["parse_first_action_options"],
        ),
        ("first-pr", FIRST_PR_PARSER_RS, &["parse_options"]),
        ("gate", GATE_PARSER_RS, &["parse_gate_options"]),
        (
            "impacted-evidence",
            IMPACTED_EVIDENCE_PARSER_RS,
            &["run_impacted_evidence", "parse_options"],
        ),
        ("init", INIT_PARSER_RS, &["parse_init_options"]),
        ("lsp", CLI_COMMANDS_RS, &["lsp"]),
        ("mcp", MCP_PARSER_RS, &["run"]),
        ("outcome", CLI_COMMANDS_RS, &["parse_outcome_options"]),
        ("pilot", PILOT_PARSER_RS, &["parse_pilot_options"]),
        (
            "plus",
            RIPR_PLUS_PARSER_RS,
            &["run_ripr_plus", "parse_options"],
        ),
        (
            "policy history",
            POLICY_PARSE_RS,
            &["parse_policy_history_options"],
        ),
        (
            "policy operations",
            POLICY_PARSE_RS,
            &["parse_policy_operations_options"],
        ),
        (
            "policy preview-promote",
            POLICY_PARSE_RS,
            &["parse_policy_preview_promotion_options"],
        ),
        (
            "policy promote",
            POLICY_PARSE_RS,
            &["parse_policy_promotion_options"],
        ),
        (
            "policy readiness",
            POLICY_PARSE_RS,
            &["parse_policy_readiness_options"],
        ),
        (
            "policy suppression-health",
            POLICY_PARSE_RS,
            &["parse_policy_suppression_health_options"],
        ),
        (
            "policy waiver-aging",
            POLICY_PARSE_RS,
            &["parse_policy_waiver_aging_options"],
        ),
        (
            "pr-comments plan",
            CLI_COMMANDS_RS,
            &["parse_pr_comments_plan_options"],
        ),
        (
            "pr-evidence",
            PR_EVIDENCE_PARSER_RS,
            &["run_pr_evidence", "parse_options"],
        ),
        (
            "pr-ledger record",
            CLI_COMMANDS_RS,
            &["parse_pr_evidence_ledger_options"],
        ),
        (
            "pr-review front-panel",
            CLI_COMMANDS_RS,
            &["parse_pr_review_front_panel_options"],
        ),
        (
            "pr-summary",
            PR_SUMMARY_PARSER_RS,
            &["run_pr_summary", "parse_options"],
        ),
        (
            "receipt check",
            RECEIPT_PARSER_RS,
            &["parse_receipt_check_options"],
        ),
        (
            "receipt write",
            RECEIPT_PARSER_RS,
            &["parse_receipt_write_options"],
        ),
        (
            "reports ci-summary",
            CI_SUMMARY_PARSER_RS,
            &["parse_ci_summary_options"],
        ),
        (
            "reports gap-ledger",
            CLI_COMMANDS_RS,
            &["parse_gap_decision_ledger_options"],
        ),
        (
            "reports index",
            CLI_COMMANDS_RS,
            &["parse_report_packet_index_options"],
        ),
        (
            "reports ts-false-actionable",
            CLI_COMMANDS_RS,
            &["parse_typescript_false_actionable_options"],
        ),
        (
            "reports ts-limitations",
            CLI_COMMANDS_RS,
            &["parse_typescript_limitations_options"],
        ),
        ("rerun", RERUN_PARSER_RS, &["parse_options"]),
        (
            "review-comments",
            REVIEW_COMMENTS_PARSER_RS,
            &["parse_review_comments_options"],
        ),
        ("swarm ingest", SWARM_INGEST_PARSER_RS, &["parse_options"]),
        ("swarm queue", SWARM_QUEUE_PARSER_RS, &["parse_options"]),
        (
            "zero status",
            CLI_COMMANDS_RS,
            &["parse_ripr_zero_status_options"],
        ),
    ];

    /// Extract the flags a help body documents (#2342).
    ///
    /// A flag counts as documented when it opens a line (the Options list) or
    /// appears inside the `Usage:` block. Prose and examples do not document
    /// a flag: #4317 showed that a mention inside another option's essay
    /// (`--perl-facts` inside CHECK_HELP's `--write-artifact` entry) leaves
    /// the flag undiscoverable and unsuggestible, so the miner reads only
    /// the two surfaces a reader scans for accepted syntax. This is the same
    /// documented-surface definition `suggest.rs` mines for suggestion
    /// candidates (Options lines plus the command's own usage lines, scoped
    /// per sibling there), so a flag this gate counts as documented is
    /// suggestible on a typo with no second edit.
    fn extract_flags(help: &str) -> Vec<String> {
        let mut flags: Vec<String> = Vec::new();
        let mut in_usage_block = false;
        for line in help.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("Usage:") {
                in_usage_block = true;
                scan_usage_flag_tokens(line, &mut flags);
                continue;
            }
            // Wrapped usage blocks list one `ripr <command>` line per
            // subcommand until the first blank line.
            if in_usage_block && trimmed.starts_with("ripr ") {
                scan_usage_flag_tokens(line, &mut flags);
                continue;
            }
            in_usage_block = false;
            // Options definitions are indented; a column-start `--` line is
            // prose, which known_flags in suggest.rs also skips. The two
            // miners must agree or the "documented here means suggestible
            // there" promise above breaks.
            if !line.starts_with(' ') {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("--") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if !name.is_empty() {
                    let flag = format!("--{name}");
                    if !flags.contains(&flag) {
                        flags.push(flag);
                    }
                }
            }
        }
        flags
    }

    /// `--flag` tokens inside a `Usage:` line: `--` preceded by the line
    /// start, whitespace, `[`, `(`, or `|`, so bracketed and alternation
    /// forms like `[--base REV|--diff PATH]` are covered.
    fn scan_usage_flag_tokens(line: &str, flags: &mut Vec<String>) {
        let bytes = line.as_bytes();
        let mut index = 0usize;
        while index + 1 < bytes.len() {
            if bytes[index] != b'-' || bytes[index + 1] != b'-' {
                index += 1;
                continue;
            }
            let previous_ok =
                index == 0 || matches!(bytes[index - 1], b' ' | b'\t' | b'[' | b'(' | b'|');
            if !previous_ok {
                index += 1;
                continue;
            }
            let mut end = index + 2;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-' || bytes[end] == b'_')
            {
                end += 1;
            }
            let token = &line[index..end];
            if token.len() > 2 && !flags.iter().any(|known| known == token) {
                flags.push(token.to_string());
            }
            index = end;
        }
    }

    /// A string literal shaped exactly like a parser match arm: `--` plus
    /// flag-name characters, nothing else. Prose fragments that merely start
    /// with `--` (`--worktree cannot be combined with --diff`) carry spaces
    /// and are filtered out here.
    fn flag_shaped_literal(literal: &str) -> bool {
        match literal.strip_prefix("--") {
            Some(name) => {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            }
            None => false,
        }
    }

    /// One pass over Rust source producing `(skeleton, literals)`.
    ///
    /// The skeleton blanks comment bodies and string-literal contents while
    /// keeping the byte layout, so code searches and brace matching see only
    /// code; `literals` carries each string literal's byte offset and decoded
    /// content. Raw strings (`r#"…"#`) and raw identifiers (`r#type`) are
    /// told apart from plain identifiers starting with `r`.
    fn scan_rust_source(source: &str) -> (String, Vec<(usize, String)>) {
        let bytes = source.as_bytes();
        let mut skeleton = vec![b' '; bytes.len()];
        let mut literals: Vec<(usize, String)> = Vec::new();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            let raw_candidate = bytes[i] == b'r'
                && i + 1 < bytes.len()
                && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#');
            if raw_candidate {
                let mut hashes = 0usize;
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] == b'#' {
                    hashes += 1;
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'"' {
                    let content_start = j + 1;
                    let mut k = content_start;
                    let mut close: Option<(usize, usize)> = None;
                    while k < bytes.len() {
                        if bytes[k] == b'"' {
                            let mut end = k + 1;
                            let mut seen = 0usize;
                            while end < bytes.len() && bytes[end] == b'#' && seen < hashes {
                                seen += 1;
                                end += 1;
                            }
                            if seen == hashes {
                                close = Some((k, end));
                                break;
                            }
                        }
                        k += 1;
                    }
                    if let Some((content_end, after)) = close {
                        literals.push((
                            content_start,
                            source[content_start..content_end].to_string(),
                        ));
                        i = after;
                        continue;
                    }
                }
                // Not a raw string after all (`r#type`-style identifier), or
                // the raw string never terminates: treat the `r` as code.
                skeleton[i] = b'r';
                i += 1;
                continue;
            }
            if bytes[i] == b'"' {
                let content_start = i + 1;
                let mut k = content_start;
                while k < bytes.len() {
                    if bytes[k] == b'\\' {
                        k += 2;
                        continue;
                    }
                    if bytes[k] == b'"' {
                        break;
                    }
                    k += 1;
                }
                let mut content_end = k.min(bytes.len());
                while content_end > content_start && !source.is_char_boundary(content_end) {
                    content_end += 1;
                }
                literals.push((
                    content_start,
                    source[content_start..content_end].to_string(),
                ));
                i = (content_end + 1).min(bytes.len());
                continue;
            }
            skeleton[i] = bytes[i];
            i += 1;
        }
        (String::from_utf8_lossy(&skeleton).into_owned(), literals)
    }

    /// Byte range of a named function's body in the blanked skeleton.
    ///
    /// Word boundaries on both sides keep `parse_agent_verify` from matching
    /// inside `parse_agent_verify_execute_options`.
    fn function_body_span(skeleton: &str, function: &str) -> Option<(usize, usize)> {
        let needle = format!("fn {function}");
        let skeleton_bytes = skeleton.as_bytes();
        let mut search_from = 0usize;
        while let Some(relative) = skeleton[search_from..].find(&needle) {
            let start = search_from + relative;
            let before_ok = start == 0 || {
                let previous = skeleton_bytes[start - 1];
                !(previous.is_ascii_alphanumeric() || previous == b'_')
            };
            let after = start + needle.len();
            let after_ok = after >= skeleton_bytes.len() || {
                let next = skeleton_bytes[after];
                !(next.is_ascii_alphanumeric() || next == b'_')
            };
            if !before_ok || !after_ok {
                search_from = start + needle.len();
                continue;
            }
            let open = start + skeleton[start..].find('{')?;
            let mut depth = 0usize;
            for (offset, byte) in skeleton_bytes[open..].iter().enumerate() {
                if *byte == b'{' {
                    depth += 1;
                } else if *byte == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some((open, open + offset));
                    }
                }
            }
            return None;
        }
        None
    }

    /// The flags one command's parser accepts: flag-shaped string literals
    /// in the listed function bodies of its parser source.
    ///
    /// Honest boundary of this net: it has no notion of match-arm scrutinee
    /// position. It collects every string literal in a listed body whose
    /// decoded content is exactly `--` plus name characters, wherever that
    /// literal sits — an argv match arm, an `expect_value(args, i, "--flag")`
    /// value label, or any other position, including inside a nested helper
    /// as long as the helper's body lies within a listed function's span. A
    /// literal in prose position survives only when the whole literal is
    /// flag-shaped: `"--flag"` counts, but a format string like
    /// `"invalid --flag: {err}"` carries spaces and braces and does not.
    ///
    /// The practical decay vector is that a literal which outlives its parse
    /// arm — kept alive by a diagnostic, an error label, or a helper — keeps
    /// this direction of the gate green for one more edit. The second layer
    /// that catches the actual parser change is the per-command argv tests,
    /// which drive the real parsers and fail once an arm stops matching:
    /// `*_requires_values_for_value_flags`, `*_rejects_unknown_argument`, and
    /// `*_suggests_the_nearest_flag_for_a_typo` in `commands/context.rs`,
    /// `commands/check.rs`, `commands/doctor.rs`, `commands/pilot.rs`,
    /// `commands/config.rs`, `commands/receipt.rs`, `commands/review_comments.rs`,
    /// `commands.rs`, and
    /// `agent.rs`. The suggestion scoping tests in `suggest.rs` pin which of
    /// those flags belong to which sibling of a shared help body. Tightening
    /// this scanner to scrutinee position without a real Rust parser would
    /// risk silently dropping genuine arms — a false "documented but accepted
    /// by no parser" drift — so the imprecision is disclosed instead of
    /// pretended away.
    ///
    /// `None` means the table lookup or a function-body span failed to
    /// resolve; the parity test asserts both cases with the offending name
    /// before it calls this, so `None` never reaches the comparison.
    fn parser_accepted_flags(command: &str) -> Option<Vec<String>> {
        let (_, source, functions) = PARSER_SOURCES
            .iter()
            .find(|(known, _, _)| *known == command)?;
        let (skeleton, literals) = scan_rust_source(source);
        let mut flags: Vec<String> = Vec::new();
        for function in functions.iter() {
            let (body_start, body_end) = function_body_span(&skeleton, function)?;
            for (at, literal) in &literals {
                if *at >= body_start
                    && *at < body_end
                    && flag_shaped_literal(literal)
                    && !flags.contains(literal)
                {
                    flags.push(literal.clone());
                }
            }
        }
        Some(flags)
    }

    /// The revived #2342 gate (#4317): for every command that ships a help
    /// body, the flags its parser accepts and the flags its help documents
    /// must agree in both directions. A parsed-but-undocumented flag is
    /// invisible to readers and unsuggestible on typos (the `--perl-facts`
    /// and `--finding` gaps in #4317); a documented-but-unparsed flag is
    /// advice the CLI refuses. Siblings that share one help body (policy,
    /// reports, baseline, assistant-loop) are compared at body level — one
    /// documented surface, one union of accepted flags — while the
    /// per-sibling split inside shared bodies is pinned by the suggestion
    /// scoping tests in `suggest.rs`.
    #[test]
    fn every_parsed_flag_agrees_with_its_help_documentation() {
        for command in super::registered_command_paths() {
            assert!(
                PARSER_SOURCES.iter().any(|(known, _, _)| known == command),
                "no parser source registered for {command:?}; every documented \
                 command needs a PARSER_SOURCES entry"
            );
        }
        for (command, _, _) in PARSER_SOURCES {
            assert!(
                super::registered_command_paths().contains(command),
                "PARSER_SOURCES names {command:?}, which REGISTERED_COMMAND_PATHS does not"
            );
        }
        // Every listed function must resolve in its source. A rename without
        // a table update would otherwise silently drop the command from the
        // comparison below.
        for (command, source, functions) in PARSER_SOURCES {
            let (skeleton, _) = scan_rust_source(source);
            for function in functions.iter() {
                assert!(
                    function_body_span(&skeleton, function).is_some(),
                    "parser source for {command:?} has no function {function:?}; \
                     update PARSER_SOURCES after the rename"
                );
            }
        }

        let mut drift: Vec<String> = Vec::new();
        let mut groups: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
        for command in super::registered_command_paths() {
            match super::help_text_for(command) {
                Some(body) => match groups
                    .iter_mut()
                    .find(|(known, _)| std::ptr::eq(*known, body))
                {
                    Some((_, group)) => group.push(command),
                    None => groups.push((body, vec![command])),
                },
                None => drift.push(format!(
                    "registered path {command:?} resolves to no help body"
                )),
            }
        }

        // (command, flag, reason) pairs allowed on one side only. Every
        // entry must carry the reason it cannot drift into a user-facing
        // gap.
        let exceptions: &[(&str, &str, &str)] = &[
            // Hidden read-only aliases of `--mutants-json` and
            // `--repo-exposure-json`, kept undocumented on purpose: the help
            // body shows the canonical names, and listing both spellings
            // would present one surface as two.
            (
                "calibrate cargo-mutants",
                "--cargo-mutants-json",
                "hidden alias of --mutants-json",
            ),
            (
                "calibrate cargo-mutants",
                "--input",
                "hidden alias of --mutants-json",
            ),
            (
                "calibrate cargo-mutants",
                "--static-json",
                "hidden alias of --repo-exposure-json",
            ),
        ];

        for (body, commands) in &groups {
            let documented = extract_flags(body);
            let mut accepted: Vec<String> = Vec::new();
            for command in commands {
                // The resolution sweep above guarantees the lookup succeeds.
                if let Some(flags) = parser_accepted_flags(command) {
                    for flag in flags {
                        if !accepted.contains(&flag) {
                            accepted.push(flag);
                        }
                    }
                }
            }
            let excepted = |command: &str, flag: &str| {
                // `--help` is accepted by every command's dispatch but is
                // deliberately documented nowhere: help bodies route readers
                // to it with the `Run `ripr <command> --help`` pointer
                // instead of an Options entry.
                flag == "--help"
                    || exceptions
                        .iter()
                        .any(|(owner, name, _)| *owner == command && *name == flag)
            };
            let undocumented: Vec<String> = accepted
                .iter()
                .filter(|flag| !documented.contains(flag))
                .filter(|flag| !commands.iter().any(|command| excepted(command, flag)))
                .cloned()
                .collect();
            let unaccepted: Vec<String> = documented
                .iter()
                .filter(|flag| !accepted.contains(flag))
                .filter(|flag| !commands.iter().any(|command| excepted(command, flag)))
                .cloned()
                .collect();
            if !undocumented.is_empty() {
                drift.push(format!(
                    "parsed but absent from help for {commands:?}: {undocumented:?}; \
                     add the Options entry the flag is missing"
                ));
            }
            if !unaccepted.is_empty() {
                drift.push(format!(
                    "documented but accepted by no parser for {commands:?}: {unaccepted:?}; \
                     drop the stale entry or restore the parse arm"
                ));
            }
        }
        assert!(
            drift.is_empty(),
            "flag/help parity drift across {} help bodies:\n{}",
            groups.len(),
            drift.join("\n")
        );
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
                &["--base", "--diff", "--mode", "--json", "--quiet"],
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
            ("doctor", DOCTOR_HELP, &["--root", "--json", "--profile"]),
            ("config validate", CONFIG_HELP, &["--root"]),
            (
                "pilot",
                PILOT_HELP,
                &["--root", "--out", "--mode", "--max-seams", "--quiet"],
            ),
        ];

        for (cmd, help, expected_flags) in commands {
            for flag in *expected_flags {
                assert!(help.contains(flag), "{cmd} help should contain {flag}");
            }
        }
    }
}
