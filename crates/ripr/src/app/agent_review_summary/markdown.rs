use super::types::AgentReviewSummaryReport;
use crate::output::markdown::{COMMAND_SHELL_DISCLOSURE, PowershellForm, powershell_form};

/// What a missing agent receipt means before any repair (#3906, N5).
///
/// A receipt is written only after a focused test edit, by the repair's
/// after phase (or a manual verify and receipt), so its absence before a
/// repair is the expected state, not a loop step to run now. The generated
/// CI summary prints the same text; it holds no single quote so the workflow
/// can carry it in a single-quoted shell string.
pub(crate) const NO_RECEIPT_BEFORE_REPAIR: &str = "No agent receipt yet. None is expected before a repair: the `--attempt ... --phase after` command writes it after the focused test edit.";

pub(crate) fn render_agent_review_summary_markdown(report: &AgentReviewSummaryReport) -> String {
    let mut rendered = String::new();
    rendered.push_str("# RIPR Agent Review Summary\n\n");
    rendered.push_str(&format!("Status: {}\n", report.status));
    match &report.target_seam {
        Some(seam) => rendered.push_str(&format!("Target seam: {}\n", seam.seam_id)),
        None => rendered.push_str("Target seam: unknown\n"),
    }
    rendered.push_str(&format!("Movement: {}\n", report.static_movement.state));
    if report.static_movement.state == "missing_artifact" {
        rendered.push_str(&format!("Receipt: {NO_RECEIPT_BEFORE_REPAIR}\n"));
    }
    match &report.analysis_outcome {
        Some(outcome) => rendered.push_str(&format!(
            "Analysis outcome: {} ({})\n",
            outcome.kind.as_str(),
            if outcome.kind.is_complete() {
                "complete"
            } else {
                "incomplete"
            }
        )),
        None => rendered.push_str("Analysis outcome: unavailable (incomplete)\n"),
    }
    if let Some(before) = &report.static_movement.before_class {
        let after = report
            .static_movement
            .after_class
            .as_deref()
            .unwrap_or("unknown");
        rendered.push_str(&format!("Static class: {before} -> {after}\n"));
    }
    if let Some(artifact) = &report.static_movement.evidence_artifact {
        rendered.push_str(&format!("Evidence artifact: {artifact}\n"));
    }
    rendered.push('\n');
    rendered.push_str("## Reviewer Focus\n\n");
    rendered.push_str(&format!("{}\n\n", report.reviewer_summary.headline));
    rendered.push_str(&format!(
        "What changed: {}\n",
        report.reviewer_summary.what_changed
    ));
    rendered.push_str(&format!("Evidence: {}\n", report.reviewer_summary.evidence));
    rendered.push_str(&format!(
        "Remaining: {}\n",
        report.reviewer_summary.remaining
    ));
    if !report.reviewer_summary.reviewer_should_inspect.is_empty() {
        rendered.push_str("\nInspect:\n");
        for item in &report.reviewer_summary.reviewer_should_inspect {
            rendered.push_str(&format!("- {item}\n"));
        }
    }
    if let Some(next_command) = &report.next_command {
        rendered.push_str("\nNext command:\n\n");
        if next_command.runs_after_test_edit() {
            rendered.push_str(&format!(
                "{}\n\n",
                crate::app::agent_status::AFTER_TEST_EDIT_NOTE
            ));
        }
        rendered.push_str(COMMAND_SHELL_DISCLOSURE);
        rendered.push_str("```bash\n");
        rendered.push_str(&next_command.command);
        rendered.push_str("\n```\n");
        match powershell_form(&next_command.command) {
            PowershellForm::Translated(line) => {
                rendered.push_str("\n```powershell\n");
                rendered.push_str(&line);
                rendered.push_str("\n```\n");
            }
            PowershellForm::SameAsBash => {}
            PowershellForm::Unavailable => rendered.push_str(&format!(
                "{}: `{}`\n",
                crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE,
                next_command.command
            )),
        }
    }
    rendered.push_str("\n## Limits\n\n");
    rendered.push_str("- Static artifact relationship only.\n");
    rendered.push_str("- No runtime mutation execution.\n");
    rendered.push_str("- No automatic source edits.\n");
    rendered.push_str("- No generated tests.\n");
    rendered
}
