mod agent;
mod core;
mod overview;
mod policy;
mod pr;
mod reports;
mod swarm;

use agent::*;
use core::*;
use overview::*;
use policy::*;
use pr::*;
use reports::*;
use swarm::*;

pub(super) fn print_help() {
    println!("{HELP}");
}

pub(super) fn print_check_help() {
    println!("{CHECK_HELP}");
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

pub(super) fn print_agent_receipt_help() {
    println!("{AGENT_RECEIPT_HELP}");
}

pub(super) fn print_agent_status_help() {
    println!("{AGENT_STATUS_HELP}");
}

pub(super) fn print_agent_review_summary_help() {
    println!("{AGENT_REVIEW_SUMMARY_HELP}");
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

#[cfg(test)]
mod tests {
    use super::{
        AGENT_BRIEF_HELP, AGENT_HELP, AGENT_PACKET_HELP, AGENT_RECEIPT_HELP,
        AGENT_REVIEW_SUMMARY_HELP, AGENT_START_HELP, AGENT_STATUS_HELP, AGENT_VERIFY_HELP,
        ASSISTANT_LOOP_HELP, BASELINE_HELP, CALIBRATE_HELP, CHECK_HELP, CONTEXT_HELP,
        COVERAGE_GRIP_HELP, DOCTOR_HELP, EVIDENCE_HEALTH_HELP, EXPLAIN_HELP, FIRST_ACTION_HELP,
        GATE_HELP, HELP, INIT_HELP, LSP_HELP, OUTCOME_HELP, PILOT_HELP, POLICY_HELP,
        PR_COMMENTS_HELP, PR_LEDGER_HELP, PR_REVIEW_HELP, REPORTS_HELP, REVIEW_COMMENTS_HELP,
        SWARM_HELP, SWARM_INGEST_HELP, SWARM_QUEUE_HELP, ZERO_HELP, print_agent_brief_help,
        print_agent_help, print_agent_packet_help, print_agent_receipt_help,
        print_agent_review_summary_help, print_agent_start_help, print_agent_status_help,
        print_agent_verify_help, print_assistant_loop_help, print_baseline_help,
        print_calibrate_help, print_check_help, print_context_help, print_coverage_grip_help,
        print_doctor_help, print_evidence_health_help, print_explain_help, print_first_action_help,
        print_gate_help, print_help, print_init_help, print_lsp_help, print_outcome_help,
        print_pilot_help, print_policy_help, print_pr_comments_help, print_pr_ledger_help,
        print_pr_review_help, print_reports_help, print_review_comments_help, print_swarm_help,
        print_swarm_ingest_help, print_swarm_queue_help, print_zero_help,
    };

    #[test]
    fn top_level_help_mentions_supported_commands() {
        assert!(HELP.contains("ripr init"));
        assert!(HELP.contains("ripr pilot"));
        assert!(HELP.contains("ripr outcome"));
        assert!(HELP.contains("ripr evidence-health"));
        assert!(HELP.contains("ripr review-comments"));
        assert!(HELP.contains("ripr gate evaluate"));
        assert!(HELP.contains("ripr baseline create"));
        assert!(HELP.contains("ripr baseline diff"));
        assert!(HELP.contains("ripr baseline update"));
        assert!(HELP.contains("ripr zero status"));
        assert!(HELP.contains("ripr policy readiness"));
        assert!(HELP.contains("ripr policy operations"));
        assert!(HELP.contains("ripr policy history"));
        assert!(HELP.contains("ripr policy promote"));
        assert!(HELP.contains("ripr policy preview-promote"));
        assert!(HELP.contains("ripr policy waiver-aging"));
        assert!(HELP.contains("ripr pr-ledger record"));
        assert!(HELP.contains("ripr pr-comments plan"));
        assert!(HELP.contains("ripr pr-review front-panel"));
        assert!(HELP.contains("ripr coverage-grip frontier"));
        assert!(HELP.contains("ripr assistant-loop proof"));
        assert!(HELP.contains("ripr assistant-loop health"));
        assert!(HELP.contains("ripr first-pr"));
        assert!(HELP.contains("ripr start-here"));
        assert!(HELP.contains("ripr first-action"));
        assert!(HELP.contains("ripr reports index"));
        assert!(HELP.contains("ripr reports gap-ledger"));
        assert!(HELP.contains("ripr calibrate"));
        assert!(HELP.contains("ripr agent start"));
        assert!(HELP.contains("ripr agent brief"));
        assert!(HELP.contains("ripr agent packet"));
        assert!(HELP.contains("ripr agent verify"));
        assert!(HELP.contains("ripr agent receipt"));
        assert!(HELP.contains("ripr agent status"));
        assert!(HELP.contains("ripr agent review-summary"));
        assert!(HELP.contains("ripr swarm queue"));
        assert!(HELP.contains("ripr swarm ingest"));
        assert!(HELP.contains("ripr check"));
        assert!(HELP.contains("ripr explain"));
        assert!(HELP.contains("ripr context"));
        assert!(HELP.contains("ripr doctor"));
        assert!(HELP.contains("Start-here path:"));
        assert!(HELP.contains("Safe next action means repair one named gap"));
        assert!(HELP.contains("Missing artifact, stale evidence, wrong root"));
        assert!(HELP.contains("Verify command, receipt command, and receipt path"));
        assert!(HELP.contains("Preview-limited evidence stays syntax-first"));
    }

    #[test]
    fn check_help_mentions_repo_badge_formats_and_examples() {
        assert!(CHECK_HELP.contains("repo-badge-plus-shields"));
        assert!(CHECK_HELP.contains("repo-exposure-json"));
        assert!(CHECK_HELP.contains("agent-seam-packets-json"));
        assert!(CHECK_HELP.contains("repo-sarif"));
        assert!(CHECK_HELP.contains("test-efficiency-report"));
        assert!(CHECK_HELP.contains("--mode ready --json"));
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
        assert!(PILOT_HELP.starts_with("Find the top test gap in this repo"));
        assert!(PILOT_HELP.contains("Usage: ripr pilot"));
        assert!(PILOT_HELP.contains("pilot-summary.json"));
        assert!(PILOT_HELP.contains("--timeout-ms MS"));
        assert!(OUTCOME_HELP.starts_with("Compare before/after static evidence"));
        assert!(OUTCOME_HELP.contains("Usage: ripr outcome"));
        assert!(OUTCOME_HELP.contains("--before PATH"));
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
        print_pilot_help();
        print_outcome_help();
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
        print_swarm_help();
        print_swarm_queue_help();
        print_swarm_ingest_help();
        print_check_help();
        print_explain_help();
        print_context_help();
        print_doctor_help();
        print_lsp_help();
    }
}
