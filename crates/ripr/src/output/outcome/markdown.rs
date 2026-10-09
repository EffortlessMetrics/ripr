use super::{
    SEAM_GRIP_CLASS_ORDER, TargetedTestOutcomeMovement, TargetedTestOutcomeReport,
    TargetedTestOutcomeSeam, review, targeted_test_outcome_gap_summary,
};
use crate::output::markdown::{code_span, inline_prose};
use std::collections::BTreeMap;

pub(crate) fn render_targeted_test_outcome_md(report: &TargetedTestOutcomeReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr targeted-test outcome report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str("Inputs:\n");
    out.push_str(&format!("- before: {}\n", code_span(&report.before_path)));
    out.push_str(&format!("- after: {}\n", code_span(&report.after_path)));
    out.push_str(&repository_heads_line(report));
    out.push('\n');

    out.push_str("## Summary\n\n");
    out.push_str("| Bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!("| moved | {} |\n", report.moved.len()));
    out.push_str(&format!("| unchanged | {} |\n", report.unchanged.len()));
    out.push_str(&format!("| regressed | {} |\n", report.regressed.len()));
    out.push_str(&format!("| new | {} |\n", report.new.len()));
    out.push_str(&format!("| removed | {} |\n", report.removed.len()));

    push_targeted_outcome_gap_summary_md(&mut out, report);

    out.push_str("\n## Grip Counts\n\n");
    out.push_str("| Class | Before | After |\n| --- | ---: | ---: |\n");
    for class in std::iter::once("seams_total").chain(SEAM_GRIP_CLASS_ORDER.iter().copied()) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            class,
            count_for_class(&report.before_counts, class),
            count_for_class(&report.after_counts, class)
        ));
    }

    push_targeted_outcome_movements_md(&mut out, "Moved", &report.moved);
    push_targeted_outcome_movements_md(&mut out, "Unchanged", &report.unchanged);
    push_targeted_outcome_movements_md(&mut out, "Regressed", &report.regressed);
    push_targeted_outcome_seams_md(&mut out, "New", &report.new);
    push_targeted_outcome_seams_md(&mut out, "Removed", &report.removed);
    push_targeted_outcome_review_receipt_md(&mut out, report);
    out.push_str(
        "\nThis report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.\n",
    );
    out
}

fn count_for_class(counts: &BTreeMap<String, usize>, class: &str) -> usize {
    match counts.get(class) {
        Some(count) => *count,
        None => 0,
    }
}

/// The Inputs head line (#6031): what repository head each snapshot
/// reports, and what that means for attributing the movement. Matching,
/// mismatching, and head-less pairs each get the sentence that is true of
/// them — the markdown receipt must not leave the mismatch on stderr only.
fn repository_heads_line(report: &TargetedTestOutcomeReport) -> String {
    match (&report.heads.before_repository_head, &report.heads.after_repository_head) {
        (Some(before), Some(after)) if before == after => format!(
            "- repository heads: both snapshots report {}\n",
            code_span(before)
        ),
        (Some(before), Some(after)) => format!(
            "- repository heads: before {} / after {} — the pair spans different heads, so reported movement may include changes other than the one being measured\n",
            code_span(before),
            code_span(after)
        ),
        (Some(only), None) => format!(
            "- repository heads: the before snapshot reports {}, the after snapshot does not carry a head SHA, so the receipt cannot confirm the pair came from the same repository\n",
            code_span(only)
        ),
        (None, Some(only)) => format!(
            "- repository heads: the after snapshot reports {}, the before snapshot does not carry a head SHA, so the receipt cannot confirm the pair came from the same repository\n",
            code_span(only)
        ),
        (None, None) => "- repository heads: neither snapshot carries a head SHA, so the receipt cannot confirm the pair came from the same repository\n".to_string(),
    }
}

fn push_targeted_outcome_movements_md(
    out: &mut String,
    title: &str,
    movements: &[TargetedTestOutcomeMovement],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if movements.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for movement in movements {
        out.push_str(&format!(
            "- {} {}:{} {} -> {} ({}; gap {})\n",
            code_span(&movement.seam_id),
            inline_prose(&movement.file),
            movement.line,
            movement.before,
            movement.after,
            movement.direction,
            movement.gap_movement
        ));
        for delta in &movement.evidence_delta {
            out.push_str(&format!("  - {}\n", inline_prose(delta)));
        }
        if movement.evidence_delta.is_empty()
            && let Some(reason) = &movement.no_movement_reason
        {
            out.push_str(&format!("  - no movement: {}\n", inline_prose(reason)));
        }
    }
}

fn push_targeted_outcome_review_receipt_md(out: &mut String, report: &TargetedTestOutcomeReport) {
    out.push_str("\n## Review Receipt\n\n");
    let gap_summary = [review::targeted_test_outcome_gap_summary_sentence(report)];
    push_review_receipt_list_md(out, "Gap movement summary", &gap_summary);
    push_review_receipt_list_md(out, "What changed?", &review::review_what_changed(report));
    push_review_receipt_list_md(
        out,
        "What RIPR flagged before?",
        &review::review_ripr_flagged_before(report),
    );
    push_review_receipt_list_md(
        out,
        "What focused proof changed?",
        &review::review_focused_proof_added(report),
    );
    push_review_receipt_list_md(
        out,
        "What moved after verification?",
        &review::review_movement_after_verification(report),
    );
    push_review_receipt_list_md(
        out,
        "What remains weak or unknown?",
        &review::review_remaining_weak_or_unknown(report),
    );
    push_review_receipt_list_md(
        out,
        "Reviewer should inspect",
        &review::review_should_inspect(report),
    );
    push_review_receipt_list_md(
        out,
        "Reviewer may believe",
        &review::reviewer_may_believe(report),
    );
    push_review_receipt_list_md(
        out,
        "Reviewer should not believe",
        &review::reviewer_should_not_believe(),
    );
}

fn push_targeted_outcome_gap_summary_md(out: &mut String, report: &TargetedTestOutcomeReport) {
    let summary = targeted_test_outcome_gap_summary(report);
    out.push_str("\n## Gap Movement\n\n");
    out.push_str("| Movement | Count |\n| --- | ---: |\n");
    out.push_str(&format!("| closed | {} |\n", summary.closed));
    out.push_str(&format!("| opened | {} |\n", summary.opened));
    out.push_str(&format!("| strengthened | {} |\n", summary.strengthened));
    out.push_str(&format!("| weakened | {} |\n", summary.weakened));
    out.push_str(&format!("| unchanged | {} |\n", summary.unchanged));
    out.push_str(&format!("| new | {} |\n", summary.new));
    out.push_str(&format!("| removed | {} |\n", summary.removed));
    out.push_str(&format!("| changed | {} |\n", summary.changed));
}

fn push_review_receipt_list_md(out: &mut String, title: &str, items: &[String]) {
    out.push_str(&format!("### {title}\n\n"));
    for item in items {
        out.push_str(&format!("- {}\n", inline_prose(item)));
    }
    out.push('\n');
}

fn push_targeted_outcome_seams_md(
    out: &mut String,
    title: &str,
    seams: &[TargetedTestOutcomeSeam],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if seams.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for seam in seams {
        out.push_str(&format!(
            "- {} {}:{} {} ({})\n",
            code_span(&seam.seam_id),
            inline_prose(&seam.file),
            seam.line,
            seam.grip_class,
            seam.seam_kind
        ));
    }
}
