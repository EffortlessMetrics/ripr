use super::FirstUsefulActionReport;
use crate::output::first_pr::{ProofPathLabels, REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP};

/// Verify and receipt labels for a first-useful-action report (#3906).
///
/// The bullet labels come from the shared selector, so the manual pair names
/// its prerequisites wherever it is listed. The section headings keep their
/// title-case form.
struct ProofPathSections {
    labels: ProofPathLabels,
    verify_heading: &'static str,
    receipt_heading: &'static str,
}

/// The step that writes `analysis-outcome.json` beside the verify file; the
/// receipt is incomplete without it (#4304).
const ANALYSIS_OUTCOME_HEADING: &str = "Analysis Outcome For The Receipt";
const ANALYSIS_OUTCOME_LABEL: &str = "Analysis outcome for the receipt";

fn proof_path_sections(report: &FirstUsefulActionReport) -> ProofPathSections {
    let repair_start = report.commands.repair.is_some();
    let (verify_heading, receipt_heading) = if repair_start {
        (
            "Manual Verify Without A Repair Attempt",
            "Manual Receipt Without A Repair Attempt",
        )
    } else {
        ("Verify After The Test Edit", "Receipt After Verify")
    };
    ProofPathSections {
        labels: ProofPathLabels::for_repair_start(repair_start),
        verify_heading,
        receipt_heading,
    }
}

pub(crate) fn render_first_useful_action_markdown(report: &FirstUsefulActionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR First Useful Action\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!("Audience: {}\n", report.audience));
    out.push_str(&format!("Action: {}\n\n", report.action_kind));
    out.push_str("## Next\n\n");
    out.push_str(&format!("{}\n\n", with_period(&report.title)));

    if should_render_one_screen_recommendation(report) {
        render_one_screen_recommendation_markdown(report, &mut out);
    }

    if !report.why_first.is_empty() {
        out.push_str("## Why First\n\n");
        for reason in &report.why_first {
            push_wrapped_bullet(&mut out, reason);
        }
        out.push('\n');
    }

    if matches!(
        report.action_kind.as_str(),
        "write_focused_test" | "revise_focused_test"
    ) && let Some(target) = &report.target
    {
        out.push_str("## Where\n\n");
        out.push_str(&format!(
            "- File: `{}`\n",
            str_or(target.file.as_deref(), "unknown")
        ));
        out.push_str(&format!(
            "- Related test: `{}`\n",
            str_or(target.related_test.as_deref(), "unknown")
        ));
        out.push_str(&format!(
            "- Suggested test: `{}`\n\n",
            str_or(target.suggested_test_name.as_deref(), "unknown")
        ));
    }

    let sections = proof_path_sections(report);
    if let Some(repair) = &report.commands.repair {
        out.push_str("## Start Repair\n\n");
        out.push_str(&format!("`{repair}`\n\n"));
        out.push_str(&format!(
            "{REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n\n"
        ));
    }

    if let Some(outcome) = &report.commands.analysis_outcome {
        out.push_str(&format!("## {ANALYSIS_OUTCOME_HEADING}\n\n"));
        out.push_str(&format!("`{outcome}`\n\n"));
    }

    if let Some(verify) = &report.commands.verify {
        out.push_str(&format!("## {}\n\n", sections.verify_heading));
        out.push_str(&format!("`{verify}`\n\n"));
    }

    if let Some(receipt) = &report.commands.receipt {
        out.push_str(&format!("## {}\n\n", sections.receipt_heading));
        out.push_str(&format!("`{receipt}`\n\n"));
    }

    // Routes that cannot name one producing command (stale evidence, an
    // incomplete receipt) hand off to `agent status`, which names the command
    // for each missing workflow artifact. Show it so the Markdown reader is not
    // left without a next step.
    if let Some(status) = &report.commands.status {
        out.push_str("## Check Workflow Status\n\n");
        out.push_str(&format!("`{status}`\n\n"));
    }

    if report.status != "actionable"
        && report.status != "unchanged_after_attempt"
        && let Some(fallback) = &report.fallback
    {
        out.push_str("## Fallback\n\n");
        if let Some(missing) = &fallback.missing {
            out.push_str("Missing required artifact:\n");
            out.push_str(&format!("`{missing}`\n\n"));
        } else if let Some(summary) = &fallback.summary {
            push_wrapped_paragraph(&mut out, summary);
            out.push('\n');
        }
    }

    if !report.limits.is_empty() {
        out.push_str("## Limits\n\n");
        for limit in &report.limits {
            push_wrapped_bullet(&mut out, limit);
        }
    }

    out
}

fn should_render_one_screen_recommendation(report: &FirstUsefulActionReport) -> bool {
    report.selected.is_some()
        || matches!(
            report.action_kind.as_str(),
            "write_focused_test" | "revise_focused_test" | "generate_missing_artifact"
        )
}

fn render_one_screen_recommendation_markdown(report: &FirstUsefulActionReport, out: &mut String) {
    // F60-12: `why` explains the selection; it is not the changed behavior.
    // Name the changed expression only when the selected evidence names one,
    // and give `why` its own line.
    let changed_behavior = match report
        .selected
        .as_ref()
        .and_then(|selected| selected.changed_behavior.as_deref())
        .map(str::trim)
        .filter(|expression| !expression.is_empty())
    {
        Some(expression) => crate::output::markdown::code_span(expression),
        None => "not named by the selected evidence".to_string(),
    };
    let evidence_strength = report
        .selected
        .as_ref()
        .and_then(|selected| {
            selected
                .current_evidence_strength
                .as_deref()
                .or(selected.classification.as_deref())
        })
        .unwrap_or(report.status.as_str());
    let missing_discriminator = report
        .selected
        .as_ref()
        .and_then(|selected| selected.missing_discriminator.as_deref())
        .or_else(|| {
            report
                .target
                .as_ref()
                .and_then(|target| target.suggested_assertion.as_deref())
        })
        .unwrap_or("missing discriminator unavailable");
    let focused_proof_intent = report
        .target
        .as_ref()
        .and_then(|target| target.suggested_assertion.as_deref())
        .unwrap_or(report.title.as_str());
    let verify_command = report.commands.verify.as_deref().unwrap_or("not_available");
    let receipt_command = report
        .commands
        .receipt
        .as_deref()
        .unwrap_or("not_available");
    let artifacts = one_screen_artifacts(report);

    out.push_str("## One-Screen Recommendation\n\n");
    out.push_str(&format!("- Changed behavior: {changed_behavior}\n"));
    if !report.why.trim().is_empty() {
        out.push_str(&format!("- Why: {}\n", with_period(report.why.trim())));
    }
    out.push_str(&format!(
        "- Current evidence strength: `{evidence_strength}`\n"
    ));
    out.push_str(&format!(
        "- Missing discriminator: {missing_discriminator}\n"
    ));
    out.push_str(&format!("- Focused proof intent: {focused_proof_intent}\n"));
    // #3906: a carried repair start leads the command lines; its after phase
    // runs verify and writes the receipt, so the low-level verify and
    // receipt commands become the manual alternative.
    let sections = proof_path_sections(report);
    if let Some(repair) = &report.commands.repair {
        out.push_str(&format!("- Repair start: `{repair}`\n"));
        out.push_str(&format!(
            "- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n"
        ));
    }
    if let Some(outcome) = &report.commands.analysis_outcome {
        out.push_str(&format!("- {ANALYSIS_OUTCOME_LABEL}: `{outcome}`\n"));
    }
    out.push_str(&format!(
        "- {}: `{verify_command}`\n",
        sections.labels.verify
    ));
    out.push_str(&format!(
        "- {}: `{receipt_command}`\n",
        sections.labels.receipt
    ));
    if !artifacts.is_empty() {
        let joined = artifacts
            .into_iter()
            .map(|artifact| format!("`{artifact}`"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- Artifacts: {joined}\n"));
    }
    out.push_str(
        "- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.\n\n",
    );
}

fn one_screen_artifacts(report: &FirstUsefulActionReport) -> Vec<&str> {
    let mut artifacts = Vec::new();
    if let Some(selected) = report.selected.as_ref() {
        push_unique_str(&mut artifacts, selected.source_artifact.as_str());
    }
    if let Some(path) = report.evidence.pr_guidance.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.assistant_proof.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.gap_ledger.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.ledger.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.receipt.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    artifacts
}

fn push_unique_str<'a>(items: &mut Vec<&'a str>, value: &'a str) {
    if !items.contains(&value) {
        items.push(value);
    }
}

pub(super) fn with_period(value: &str) -> String {
    if value.ends_with('.') {
        value.to_string()
    } else {
        format!("{value}.")
    }
}

pub(super) fn str_or<'a>(value: Option<&'a str>, fallback: &'static str) -> &'a str {
    match value {
        Some(value) => value,
        None => fallback,
    }
}

fn push_wrapped_bullet(out: &mut String, text: &str) {
    push_wrapped(out, "- ", "  ", &with_period(text), 79);
}

pub(super) fn push_wrapped_paragraph(out: &mut String, text: &str) {
    push_wrapped(out, "", "", &with_period(text), 79);
}

fn push_wrapped(
    out: &mut String,
    first_prefix: &str,
    continuation_prefix: &str,
    text: &str,
    width: usize,
) {
    let mut line = String::from(first_prefix);
    let mut first_word = true;
    for word in text.split_whitespace() {
        let separator = if first_word { "" } else { " " };
        if !first_word && line.len() + separator.len() + word.len() > width {
            out.push_str(&line);
            out.push('\n');
            line.clear();
            line.push_str(continuation_prefix);
            line.push_str(word);
        } else {
            line.push_str(separator);
            line.push_str(word);
        }
        first_word = false;
    }
    out.push_str(&line);

    out.push('\n');
}
