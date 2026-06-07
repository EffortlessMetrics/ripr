use super::{TargetedTestOutcomeReport, targeted_test_outcome_gap_summary};

pub(super) fn review_what_changed(report: &TargetedTestOutcomeReport) -> Vec<String> {
    vec![
        format!(
            "Compared before snapshot {} with after snapshot {}.",
            report.before_path, report.after_path
        ),
        format!(
            "Static seam movement: {} moved, {} unchanged, {} regressed, {} new, {} removed.",
            report.moved.len(),
            report.unchanged.len(),
            report.regressed.len(),
            report.new.len(),
            report.removed.len()
        ),
    ]
}

pub(super) fn review_ripr_flagged_before(report: &TargetedTestOutcomeReport) -> Vec<String> {
    let mut items = Vec::new();
    for movement in report
        .moved
        .iter()
        .chain(report.unchanged.iter())
        .chain(report.regressed.iter())
    {
        if review_attention_class(&movement.before) {
            items.push(format!(
                "{} before {} at {}:{}.",
                movement.before, movement.seam_kind, movement.file, movement.line
            ));
        }
    }
    for seam in &report.removed {
        if review_attention_class(&seam.grip_class) {
            items.push(format!(
                "{} before {} at {}:{} later disappeared from the after snapshot.",
                seam.grip_class, seam.seam_kind, seam.file, seam.line
            ));
        }
    }
    review_limit_or_default(
        items,
        "No before-snapshot weak or unknown seams were present in the compared artifacts.",
    )
}

pub(super) fn review_focused_proof_added(report: &TargetedTestOutcomeReport) -> Vec<String> {
    let mut items = Vec::new();
    for movement in report
        .moved
        .iter()
        .chain(report.unchanged.iter())
        .chain(report.regressed.iter())
    {
        let proof_deltas = movement
            .evidence_delta
            .iter()
            .filter(|delta| positive_proof_delta(delta))
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        if proof_deltas.is_empty() {
            continue;
        }
        items.push(format!(
            "{} at {}:{} shows static evidence movement for focused proof outside RIPR: {}.",
            movement.seam_kind,
            movement.file,
            movement.line,
            proof_deltas.join("; ")
        ));
    }
    review_limit_or_default(
        items,
        "No focused proof signal from a test or output proof outside RIPR was visible in the rendered static snapshots.",
    )
}

pub(super) fn review_movement_after_verification(
    report: &TargetedTestOutcomeReport,
) -> Vec<String> {
    let mut items = Vec::new();
    let improved = report
        .moved
        .iter()
        .filter(|movement| movement.direction == "improved")
        .count();
    let changed = report
        .moved
        .iter()
        .filter(|movement| movement.direction != "improved")
        .count();
    items.push(format!(
        "{} improved, {} changed without ranking higher, {} regressed, {} unchanged.",
        improved,
        changed,
        report.regressed.len(),
        report.unchanged.len()
    ));
    items.push(targeted_test_outcome_gap_summary_sentence(report));
    for movement in report.moved.iter().chain(report.regressed.iter()).take(4) {
        items.push(format!(
            "{} at {}:{} moved {} -> {} ({}).",
            movement.seam_kind,
            movement.file,
            movement.line,
            movement.before,
            movement.after,
            movement.direction
        ));
    }
    let unchanged_with_delta = report
        .unchanged
        .iter()
        .filter(|movement| !movement.evidence_delta.is_empty())
        .take(3)
        .map(|movement| {
            format!(
                "{} at {}:{} kept {} but evidence changed: {}.",
                movement.seam_kind,
                movement.file,
                movement.line,
                movement.after,
                movement.evidence_delta.join("; ")
            )
        });
    items.extend(unchanged_with_delta);
    items
}

pub(super) fn targeted_test_outcome_gap_summary_sentence(
    report: &TargetedTestOutcomeReport,
) -> String {
    let summary = targeted_test_outcome_gap_summary(report);
    format!(
        "Gap movement: {} closed, {} opened, {} strengthened, {} weakened, {} unchanged, {} new, {} removed, {} changed.",
        summary.closed,
        summary.opened,
        summary.strengthened,
        summary.weakened,
        summary.unchanged,
        summary.new,
        summary.removed,
        summary.changed
    )
}

pub(super) fn review_remaining_weak_or_unknown(report: &TargetedTestOutcomeReport) -> Vec<String> {
    let mut items = Vec::new();
    for movement in report
        .moved
        .iter()
        .chain(report.unchanged.iter())
        .chain(report.regressed.iter())
    {
        if review_attention_class(&movement.after) {
            items.push(format!(
                "{} remains {} at {}:{}.",
                movement.seam_kind, movement.after, movement.file, movement.line
            ));
        }
    }
    for seam in &report.new {
        if review_attention_class(&seam.grip_class) {
            items.push(format!(
                "New {} is {} at {}:{}.",
                seam.seam_kind, seam.grip_class, seam.file, seam.line
            ));
        }
    }
    review_limit_or_default(
        items,
        "No weak or unknown after-snapshot seams were present in the compared artifacts.",
    )
}

pub(super) fn review_should_inspect(report: &TargetedTestOutcomeReport) -> Vec<String> {
    vec![
        format!(
            "Open the compared artifacts: {} and {}.",
            report.before_path, report.after_path
        ),
        "Inspect the focused test or output proof corresponding to each listed evidence delta."
            .to_string(),
        "Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete."
            .to_string(),
    ]
}

pub(super) fn reviewer_may_believe(report: &TargetedTestOutcomeReport) -> Vec<String> {
    let mut items = vec![format!(
        "RIPR compared only the listed static snapshots: {} and {}.",
        report.before_path, report.after_path
    )];
    let has_focused_proof_signal = report
        .moved
        .iter()
        .chain(report.unchanged.iter())
        .chain(report.regressed.iter())
        .any(|movement| {
            movement
                .evidence_delta
                .iter()
                .any(|delta| positive_proof_delta(delta))
        });
    if has_focused_proof_signal {
        items.push(
            "The listed focused-proof signals are static evidence visible after a test or output proof changed outside RIPR."
                .to_string(),
        );
    } else {
        items.push(
            "No focused-proof signal was visible; this receipt only records before/after static movement."
                .to_string(),
        );
    }
    items.push(
        "The movement and remaining-weak sections define the static claim boundary for this receipt."
            .to_string(),
    );
    items
}

pub(super) fn reviewer_should_not_believe() -> Vec<String> {
    vec![
        "Runtime mutation result.".to_string(),
        "Coverage adequacy.".to_string(),
        "General correctness.".to_string(),
        "Merge approval.".to_string(),
        "That RIPR edited source or generated tests.".to_string(),
    ]
}

pub(super) fn review_attention_class(class: &str) -> bool {
    !matches!(class, "strongly_gripped" | "intentional" | "suppressed")
}

fn positive_proof_delta(delta: &str) -> bool {
    delta.contains("missing discriminator no longer reported")
        || delta.contains("new observed value")
        || delta.contains("stronger related oracle visible")
        || delta.contains("related test count increased")
        || delta.contains("evidence moved from missing to yes")
        || delta.contains("evidence moved from weak to yes")
}

fn review_limit_or_default(mut items: Vec<String>, fallback: &str) -> Vec<String> {
    if items.is_empty() {
        return vec![fallback.to_string()];
    }
    items.truncate(5);
    items
}
