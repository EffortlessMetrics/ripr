use std::collections::BTreeMap;

use super::{EVIDENCE_HEALTH_SCHEMA_VERSION, EvidenceHealthReport};

pub(crate) fn render_evidence_health_markdown(report: &EvidenceHealthReport) -> String {
    let mut out = String::new();
    push_header(&mut out, report);
    push_summary(&mut out, report);
    push_grip_classes(&mut out, report);
    push_missing_discriminators(&mut out, report);
    push_oracle_strength(&mut out, report);
    push_related_test_confidence(&mut out, report);
    push_evidence_quality(&mut out, report);
    push_largest_canonical_gap_groups(&mut out, report);
    push_actionability(&mut out, report);
    push_static_limitation_distribution(&mut out, report);
    push_calibration_coverage(&mut out, report);
    push_evidence_quality_risks(&mut out, report);
    push_calibration_availability(&mut out, report);
    push_top_static_limitations(&mut out, report);
    out
}

fn push_header(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("# RIPR evidence health report\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    push_metric(out, "Schema", EVIDENCE_HEALTH_SCHEMA_VERSION);
    push_metric(out, "Status", "advisory");
    push_metric(out, "Root", report.root.as_str());
    push_metric(
        out,
        "Calibration",
        report
            .calibration
            .source
            .as_deref()
            .unwrap_or("not provided"),
    );
    out.push('\n');
}

fn push_summary(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(out, "Seams", report.metrics.seams_total);
    push_count(
        out,
        "Headline-eligible seams",
        report.metrics.headline_eligible_total,
    );
    push_count(
        out,
        "Weakly gripped seams",
        report.metrics.weakly_gripped_total,
    );
    push_count(out, "Ungripped seams", report.metrics.ungripped_total);
    push_count(
        out,
        "Missing discriminators",
        report.metrics.missing_discriminators_total,
    );
    push_count(out, "Observed values", report.metrics.observed_values_total);
    push_count(out, "Related tests", report.metrics.related_tests_total);
    push_count(
        out,
        "Opaque oracle classifications",
        report.metrics.opaque_oracle_count,
    );
    out.push('\n');
}

fn push_grip_classes(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Grip Classes\n\n");
    push_counts_table(out, "Grip class", &report.metrics.grip_class_counts);
}

fn push_missing_discriminators(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Missing Discriminators\n\n");
    if report.metrics.missing_discriminator_counts.is_empty() {
        out.push_str("No missing discriminators were reported.\n\n");
    } else {
        push_counts_table_limited(
            out,
            "Missing discriminator",
            &report.metrics.missing_discriminator_counts,
            25,
        );
    }
}

fn push_oracle_strength(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Oracle Strength\n\n");
    push_counts_table(
        out,
        "Oracle strength",
        &report.metrics.oracle_strength_counts,
    );
}

fn push_related_test_confidence(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Related Test Confidence\n\n");
    push_counts_table(
        out,
        "Relation confidence",
        &report.metrics.related_test_confidence_counts,
    );
}

fn push_evidence_quality(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Evidence Quality\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(
        out,
        "Canonical gap groups",
        report.evidence_quality.canonical_gap_groups_total,
    );
    push_count(
        out,
        "Duplicate-looking groups",
        report.evidence_quality.duplicate_looking_groups_total,
    );
    push_count(
        out,
        "Records with canonical gap identity",
        report
            .evidence_quality
            .movement_availability
            .records_with_canonical_gap_id,
    );
    push_count(
        out,
        "Records with complete evidence path",
        report
            .evidence_quality
            .movement_availability
            .records_with_complete_evidence_path,
    );
    push_count(
        out,
        "Records with verify command",
        report
            .evidence_quality
            .movement_availability
            .records_with_verify_command,
    );
    out.push('\n');
}

fn push_largest_canonical_gap_groups(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Largest Canonical Gap Groups\n\n");
    if report.evidence_quality.largest_canonical_groups.is_empty() {
        out.push_str("No canonical gap groups were reported.\n\n");
        return;
    }

    out.push_str("| Group | Count | Reported size | Owner | Seam kind | Flow sink | Missing discriminator | Assertion shape | Example seam | File |\n");
    out.push_str("| --- | ---: | ---: | --- | --- | --- | --- | --- | --- | --- |\n");
    for group in &report.evidence_quality.largest_canonical_groups {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            group.canonical_gap_id,
            group.count,
            group
                .reported_group_size
                .map_or_else(|| "n/a".to_string(), |size| size.to_string()),
            group.owner,
            group.seam_kind,
            group.flow_sink,
            group.missing_discriminator,
            group.assertion_shape,
            group.example_seam_id,
            group.example_file
        ));
    }
    out.push('\n');
}

fn push_actionability(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Actionability\n\n");
    push_counts_table(
        out,
        "Actionability class",
        &report.evidence_quality.actionability_class_counts,
    );
}

fn push_static_limitation_distribution(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Static Limitation Distribution\n\n");
    push_counts_table(
        out,
        "Static limitation stage",
        &report.evidence_quality.static_limitation_stage_counts,
    );
    push_counts_table_limited(
        out,
        "Static limitation reason",
        &report.evidence_quality.static_limitation_reason_counts,
        15,
    );
}

fn push_calibration_coverage(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Evidence-Record Calibration Coverage\n\n");
    push_counts_table(
        out,
        "Calibration availability",
        &report.evidence_quality.calibration_availability_counts,
    );
}

fn push_evidence_quality_risks(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Top Evidence Quality Risks\n\n");
    if report
        .evidence_quality
        .top_evidence_quality_risks
        .is_empty()
    {
        out.push_str("No evidence-quality risks were reported.\n\n");
        return;
    }

    out.push_str("| Risk | Count | Summary |\n");
    out.push_str("| --- | ---: | --- |\n");
    for risk in &report.evidence_quality.top_evidence_quality_risks {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            risk.kind, risk.count, risk.summary
        ));
    }
    out.push('\n');
}

fn push_calibration_availability(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Calibration Availability\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(
        out,
        "Matched calibration rows",
        report.calibration.matched_total,
    );
    push_count(
        out,
        "Static rows without runtime context",
        report.calibration.static_without_runtime_total,
    );
    push_count(
        out,
        "Runtime rows without static seam",
        report.calibration.runtime_without_static_total,
    );
    push_count(
        out,
        "Ambiguous file-line joins",
        report.calibration.ambiguous_file_line_total,
    );
    push_count(
        out,
        "Unmatched runtime rows",
        report.calibration.unmatched_runtime_total,
    );
    out.push('\n');
}

fn push_top_static_limitations(out: &mut String, report: &EvidenceHealthReport) {
    out.push_str("## Top Static Limitations\n\n");
    if report.top_static_limitations.is_empty() {
        out.push_str("No static limitations were reported.\n");
        return;
    }

    out.push_str("### Categories\n\n");
    push_counts_table(
        out,
        "Category",
        &report.evidence_quality.static_limitation_category_counts,
    );
    out.push('\n');

    out.push_str("### Largest Limitation Signals\n\n");
    out.push_str("| Limitation | Count | Example seam | Summary |\n");
    out.push_str("| --- | ---: | --- | --- |\n");
    for limitation in &report.top_static_limitations {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            limitation.kind,
            limitation.count,
            limitation.example_seam_id.as_deref().unwrap_or("n/a"),
            limitation.summary
        ));
    }
}

fn push_metric(out: &mut String, name: &str, value: &str) {
    out.push_str(&format!("| {name} | {value} |\n"));
}

fn push_count(out: &mut String, name: &str, count: usize) {
    out.push_str(&format!("| {name} | {count} |\n"));
}

fn push_counts_table(out: &mut String, heading: &str, counts: &BTreeMap<String, usize>) {
    out.push_str(&format!("| {heading} | Count |\n"));
    out.push_str("| --- | ---: |\n");
    for (key, count) in counts {
        out.push_str(&format!("| {key} | {count} |\n"));
    }
    out.push('\n');
}

fn push_counts_table_limited(
    out: &mut String,
    heading: &str,
    counts: &BTreeMap<String, usize>,
    limit: usize,
) {
    out.push_str(&format!("| {heading} | Count |\n"));
    out.push_str("| --- | ---: |\n");
    let mut rows = counts.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    for (key, count) in rows.iter().take(limit) {
        out.push_str(&format!("| {key} | {count} |\n"));
    }
    if rows.len() > limit {
        out.push_str(&format!(
            "\n_{} additional {} rows omitted from Markdown; JSON contains the full count map._\n",
            rows.len() - limit,
            heading.to_ascii_lowercase()
        ));
    }
    out.push('\n');
}
