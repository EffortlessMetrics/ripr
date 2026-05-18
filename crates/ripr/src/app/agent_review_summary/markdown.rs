use super::types::AgentReviewSummaryReport;

pub(crate) fn render_agent_review_summary_markdown(report: &AgentReviewSummaryReport) -> String {
    let mut rendered = String::new();
    rendered.push_str("# RIPR Agent Review Summary\n\n");
    rendered.push_str(&format!("Status: {}\n", report.status));
    match &report.target_seam {
        Some(seam) => rendered.push_str(&format!("Target seam: {}\n", seam.seam_id)),
        None => rendered.push_str("Target seam: unknown\n"),
    }
    rendered.push_str(&format!("Movement: {}\n", report.static_movement.state));
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
        rendered.push_str("\nNext command:\n");
        rendered.push_str("```bash\n");
        rendered.push_str(&next_command.command);
        rendered.push_str("\n```\n");
    }
    rendered.push_str("\n## Limits\n\n");
    rendered.push_str("- Static artifact relationship only.\n");
    rendered.push_str("- No runtime mutation execution.\n");
    rendered.push_str("- No automatic source edits.\n");
    rendered.push_str("- No generated tests.\n");
    rendered
}
