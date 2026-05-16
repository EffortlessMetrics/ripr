use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::{CanonicalGapIdentity, canonical_gap_identities};
use crate::analysis::seams::SeamGripClass;
use crate::domain::{OracleKind, OracleStrength, StageState};
use crate::output::evidence_record::{EvidenceRecord, evidence_record_for};

pub(crate) const EVIDENCE_HEALTH_SCHEMA_VERSION: &str = "0.1";
const EVIDENCE_HEALTH_TOP_GROUP_LIMIT: usize = 10;
const EVIDENCE_HEALTH_TOP_RISK_LIMIT: usize = 5;

const STAGE_LABELS: &[&str] = &["yes", "weak", "no", "unknown", "opaque", "not_applicable"];
const ORACLE_STRENGTH_LABELS: &[&str] = &["strong", "medium", "weak", "smoke", "none", "unknown"];
const ORACLE_KIND_LABELS: &[&str] = &[
    "exact_value",
    "exact_error_variant",
    "whole_object_equality",
    "snapshot",
    "relational_check",
    "broad_error",
    "smoke_only",
    "mock_expectation",
    "unknown",
];
const RELATION_CONFIDENCE_LABELS: &[&str] = &["high", "medium", "low", "opaque"];
const VALUE_CONTEXT_LABELS: &[&str] = &[
    "function_argument",
    "assertion_argument",
    "builder_method",
    "table_row",
    "enum_variant",
    "return_value",
    "unknown",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceHealthReport {
    root: String,
    metrics: EvidenceHealthMetrics,
    evidence_quality: EvidenceHealthQuality,
    calibration: EvidenceHealthCalibration,
    top_static_limitations: Vec<StaticLimitation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceHealthMetrics {
    seams_total: usize,
    headline_eligible_total: usize,
    weakly_gripped_total: usize,
    ungripped_total: usize,
    grip_class_counts: BTreeMap<String, usize>,
    stage_state_counts: BTreeMap<String, BTreeMap<String, usize>>,
    unknown_stage_counts: BTreeMap<String, usize>,
    unknown_stop_reason_counts: BTreeMap<String, usize>,
    missing_discriminators_total: usize,
    seams_with_missing_discriminators: usize,
    missing_discriminator_counts: BTreeMap<String, usize>,
    observed_values_total: usize,
    seams_with_observed_values: usize,
    observed_value_context_counts: BTreeMap<String, usize>,
    related_tests_total: usize,
    seams_with_related_tests: usize,
    related_test_confidence_counts: BTreeMap<String, usize>,
    oracle_strength_counts: BTreeMap<String, usize>,
    oracle_kind_counts: BTreeMap<String, usize>,
    opaque_oracle_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceHealthQuality {
    canonical_gap_groups_total: usize,
    duplicate_looking_groups_total: usize,
    largest_canonical_groups: Vec<EvidenceHealthCanonicalGroup>,
    actionability_class_counts: BTreeMap<String, usize>,
    static_limitation_stage_counts: BTreeMap<String, usize>,
    static_limitation_reason_counts: BTreeMap<String, usize>,
    static_limitation_category_counts: BTreeMap<String, usize>,
    calibration_availability_counts: BTreeMap<String, usize>,
    movement_availability: EvidenceHealthMovementAvailability,
    top_evidence_quality_risks: Vec<EvidenceHealthRisk>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EvidenceHealthMovementAvailability {
    records_with_seam_id: usize,
    records_with_canonical_gap_id: usize,
    records_with_complete_evidence_path: usize,
    records_with_recommendation: usize,
    records_with_verify_command: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceHealthCanonicalGroup {
    canonical_gap_id: String,
    count: usize,
    reported_group_size: Option<usize>,
    owner: String,
    seam_kind: String,
    flow_sink: String,
    missing_discriminator: String,
    assertion_shape: String,
    example_seam_id: String,
    example_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceHealthCanonicalGroupCounter {
    count: usize,
    reported_group_size: Option<usize>,
    owner: String,
    seam_kind: String,
    flow_sink: String,
    missing_discriminator: String,
    assertion_shape: String,
    example_seam_id: String,
    example_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceHealthRisk {
    kind: String,
    count: usize,
    summary: String,
}

#[derive(Debug, Default)]
struct EvidenceHealthQualityCounters {
    canonical_group_counts: BTreeMap<String, EvidenceHealthCanonicalGroupCounter>,
    actionability_class_counts: BTreeMap<String, usize>,
    static_limitation_stage_counts: BTreeMap<String, usize>,
    static_limitation_reason_counts: BTreeMap<String, usize>,
    static_limitation_category_counts: BTreeMap<String, usize>,
    calibration_availability_counts: BTreeMap<String, usize>,
    movement_availability: EvidenceHealthMovementAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceHealthCalibration {
    status: String,
    source: Option<String>,
    matched_total: usize,
    static_without_runtime_total: usize,
    runtime_without_static_total: usize,
    ambiguous_file_line_total: usize,
    unmatched_runtime_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticLimitationCounter {
    count: usize,
    summary: String,
    example_seam_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticLimitation {
    kind: String,
    count: usize,
    summary: String,
    example_seam_id: Option<String>,
}

impl EvidenceHealthCalibration {
    pub(crate) fn not_provided() -> Self {
        Self {
            status: "not_provided".to_string(),
            source: None,
            matched_total: 0,
            static_without_runtime_total: 0,
            runtime_without_static_total: 0,
            ambiguous_file_line_total: 0,
            unmatched_runtime_total: 0,
        }
    }

    pub(crate) fn from_json(source: String, contents: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(contents)
            .map_err(|err| format!("failed to parse mutation calibration context: {err}"))?;
        Ok(Self {
            status: "loaded".to_string(),
            source: Some(source),
            matched_total: usize_field(&value, &["summary", "matched_total"]),
            static_without_runtime_total: usize_field(
                &value,
                &["summary", "static_without_runtime_total"],
            ),
            runtime_without_static_total: value
                .get("missed_runtime_signals")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            ambiguous_file_line_total: usize_field(
                &value,
                &["summary", "ambiguous_file_line_total"],
            ),
            unmatched_runtime_total: usize_field(&value, &["summary", "unmatched_mutants_total"]),
        })
    }
}

pub(crate) fn build_evidence_health_report(
    classified: &[ClassifiedSeam],
    root: String,
    calibration: EvidenceHealthCalibration,
) -> EvidenceHealthReport {
    let mut grip_class_counts = grip_class_counts();
    let mut stage_state_counts = stage_state_counts();
    let mut unknown_stage_counts = stage_counts();
    let mut unknown_stop_reason_counts = unknown_stop_reason_counts();
    let mut missing_discriminator_counts = BTreeMap::new();
    let mut observed_value_context_counts = labeled_counts(VALUE_CONTEXT_LABELS);
    let mut related_test_confidence_counts = labeled_counts(RELATION_CONFIDENCE_LABELS);
    let mut oracle_strength_counts = labeled_counts(ORACLE_STRENGTH_LABELS);
    let mut oracle_kind_counts = labeled_counts(ORACLE_KIND_LABELS);
    let mut limitations: BTreeMap<String, StaticLimitationCounter> = BTreeMap::new();
    let canonical_gaps = canonical_gap_identities(classified);
    let mut quality_counters = EvidenceHealthQualityCounters::default();

    let mut headline_eligible_total = 0;
    let mut missing_discriminators_total = 0;
    let mut seams_with_missing_discriminators = 0;
    let mut observed_values_total = 0;
    let mut seams_with_observed_values = 0;
    let mut related_tests_total = 0;
    let mut seams_with_related_tests = 0;
    let mut opaque_oracle_count = 0;

    for entry in classified {
        let canonical_gap = canonical_gaps.get(entry.seam.id());
        let record = evidence_record_for(entry, canonical_gap);
        count_evidence_record_quality(&record, canonical_gap, &mut quality_counters);

        increment(&mut grip_class_counts, entry.class.as_str());
        if entry.class.is_headline_eligible() {
            headline_eligible_total += 1;
        }
        increment_unknown_stop_reason(&mut unknown_stop_reason_counts, entry.class);

        count_stage(
            &mut stage_state_counts,
            &mut unknown_stage_counts,
            &mut limitations,
            "reach",
            &entry.evidence.reach.state,
            entry.seam.id().as_str(),
        );
        count_stage(
            &mut stage_state_counts,
            &mut unknown_stage_counts,
            &mut limitations,
            "activate",
            &entry.evidence.activate.state,
            entry.seam.id().as_str(),
        );
        count_stage(
            &mut stage_state_counts,
            &mut unknown_stage_counts,
            &mut limitations,
            "propagate",
            &entry.evidence.propagate.state,
            entry.seam.id().as_str(),
        );
        count_stage(
            &mut stage_state_counts,
            &mut unknown_stage_counts,
            &mut limitations,
            "observe",
            &entry.evidence.observe.state,
            entry.seam.id().as_str(),
        );
        count_stage(
            &mut stage_state_counts,
            &mut unknown_stage_counts,
            &mut limitations,
            "discriminate",
            &entry.evidence.discriminate.state,
            entry.seam.id().as_str(),
        );

        if entry.evidence.related_tests.is_empty() {
            increment_limitation(
                &mut limitations,
                "no_related_tests",
                "No related test was associated with the seam.",
                entry.seam.id().as_str(),
            );
        } else {
            seams_with_related_tests += 1;
        }

        if !entry.evidence.observed_values.is_empty() {
            seams_with_observed_values += 1;
        }
        observed_values_total += entry.evidence.observed_values.len();
        for value in &entry.evidence.observed_values {
            increment(&mut observed_value_context_counts, value.context.as_str());
        }

        if !entry.evidence.missing_discriminators.is_empty() {
            seams_with_missing_discriminators += 1;
            increment_limitation(
                &mut limitations,
                "missing_discriminator",
                "At least one discriminator remains missing for the seam.",
                entry.seam.id().as_str(),
            );
        }
        missing_discriminators_total += entry.evidence.missing_discriminators.len();
        for missing in &entry.evidence.missing_discriminators {
            increment(&mut missing_discriminator_counts, missing.value.as_str());
        }

        related_tests_total += entry.evidence.related_tests.len();
        for related in &entry.evidence.related_tests {
            increment(
                &mut related_test_confidence_counts,
                related.relation_confidence.as_str(),
            );
            increment(
                &mut oracle_strength_counts,
                related.oracle_strength.as_str(),
            );
            increment(&mut oracle_kind_counts, related.oracle_kind.as_str());
            if related.oracle_kind == OracleKind::Unknown
                || related.oracle_strength == OracleStrength::Unknown
            {
                opaque_oracle_count += 1;
                increment_limitation(
                    &mut limitations,
                    "opaque_oracle",
                    "A related test contains an assertion shape ripr cannot classify.",
                    entry.seam.id().as_str(),
                );
            }
        }
    }

    let metrics = EvidenceHealthMetrics {
        seams_total: classified.len(),
        headline_eligible_total,
        weakly_gripped_total: count_for(&grip_class_counts, "weakly_gripped"),
        ungripped_total: count_for(&grip_class_counts, "ungripped"),
        grip_class_counts,
        stage_state_counts,
        unknown_stage_counts,
        unknown_stop_reason_counts,
        missing_discriminators_total,
        seams_with_missing_discriminators,
        missing_discriminator_counts,
        observed_values_total,
        seams_with_observed_values,
        observed_value_context_counts,
        related_tests_total,
        seams_with_related_tests,
        related_test_confidence_counts,
        oracle_strength_counts,
        oracle_kind_counts,
        opaque_oracle_count,
    };
    let evidence_quality = evidence_quality_from_counts(quality_counters, &metrics);

    EvidenceHealthReport {
        root,
        metrics,
        evidence_quality,
        calibration,
        top_static_limitations: top_limitations(limitations),
    }
}

pub(crate) fn render_evidence_health_json(report: &EvidenceHealthReport) -> Result<String, String> {
    let value = json!({
        "schema_version": EVIDENCE_HEALTH_SCHEMA_VERSION,
        "tool": "ripr",
        "scope": "repo",
        "status": "advisory",
        "inputs": {
            "root": report.root,
            "mutation_calibration": report.calibration.source,
        },
        "metrics": {
            "seams_total": report.metrics.seams_total,
            "headline_eligible_total": report.metrics.headline_eligible_total,
            "weakly_gripped_total": report.metrics.weakly_gripped_total,
            "ungripped_total": report.metrics.ungripped_total,
            "grip_class_counts": report.metrics.grip_class_counts,
            "stage_state_counts": report.metrics.stage_state_counts,
            "unknown_stage_counts": report.metrics.unknown_stage_counts,
            "unknown_stop_reason_counts": report.metrics.unknown_stop_reason_counts,
            "missing_discriminators_total": report.metrics.missing_discriminators_total,
            "seams_with_missing_discriminators": report.metrics.seams_with_missing_discriminators,
            "missing_discriminator_counts": report.metrics.missing_discriminator_counts,
            "observed_values_total": report.metrics.observed_values_total,
            "seams_with_observed_values": report.metrics.seams_with_observed_values,
            "observed_value_context_counts": report.metrics.observed_value_context_counts,
            "related_tests_total": report.metrics.related_tests_total,
            "seams_with_related_tests": report.metrics.seams_with_related_tests,
            "related_test_confidence_counts": report.metrics.related_test_confidence_counts,
            "oracle_strength_counts": report.metrics.oracle_strength_counts,
            "oracle_kind_counts": report.metrics.oracle_kind_counts,
            "opaque_oracle_count": report.metrics.opaque_oracle_count,
        },
        "evidence_quality": {
            "canonical_gap_groups_total": report.evidence_quality.canonical_gap_groups_total,
            "duplicate_looking_groups_total": report.evidence_quality.duplicate_looking_groups_total,
            "largest_canonical_groups": report.evidence_quality.largest_canonical_groups.iter().map(|group| {
                json!({
                    "canonical_gap_id": group.canonical_gap_id,
                    "count": group.count,
                    "reported_group_size": group.reported_group_size,
                    "owner": group.owner,
                    "seam_kind": group.seam_kind,
                    "flow_sink": group.flow_sink,
                    "missing_discriminator": group.missing_discriminator,
                    "assertion_shape": group.assertion_shape,
                    "example_seam_id": group.example_seam_id,
                    "example_file": group.example_file,
                })
            }).collect::<Vec<_>>(),
            "actionability_class_counts": report.evidence_quality.actionability_class_counts,
            "static_limitation_stage_counts": report.evidence_quality.static_limitation_stage_counts,
            "static_limitation_reason_counts": report.evidence_quality.static_limitation_reason_counts,
            "static_limitation_category_counts": report.evidence_quality.static_limitation_category_counts,
            "calibration_availability_counts": report.evidence_quality.calibration_availability_counts,
            "movement_availability": {
                "records_with_seam_id": report.evidence_quality.movement_availability.records_with_seam_id,
                "records_with_canonical_gap_id": report.evidence_quality.movement_availability.records_with_canonical_gap_id,
                "records_with_complete_evidence_path": report.evidence_quality.movement_availability.records_with_complete_evidence_path,
                "records_with_recommendation": report.evidence_quality.movement_availability.records_with_recommendation,
                "records_with_verify_command": report.evidence_quality.movement_availability.records_with_verify_command,
            },
            "top_evidence_quality_risks": report.evidence_quality.top_evidence_quality_risks.iter().map(|risk| {
                json!({
                    "kind": risk.kind,
                    "count": risk.count,
                    "summary": risk.summary,
                })
            }).collect::<Vec<_>>(),
        },
        "calibration": {
            "status": report.calibration.status,
            "source": report.calibration.source,
            "matched_total": report.calibration.matched_total,
            "static_without_runtime_total": report.calibration.static_without_runtime_total,
            "runtime_without_static_total": report.calibration.runtime_without_static_total,
            "ambiguous_file_line_total": report.calibration.ambiguous_file_line_total,
            "unmatched_runtime_total": report.calibration.unmatched_runtime_total,
        },
        "top_static_limitations": report.top_static_limitations.iter().map(|limitation| {
            json!({
                "kind": limitation.kind,
                "count": limitation.count,
                "summary": limitation.summary,
                "example_seam_id": limitation.example_seam_id,
            })
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render evidence health JSON: {err}"))
}

pub(crate) fn render_evidence_health_markdown(report: &EvidenceHealthReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR evidence health report\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    push_metric(&mut out, "Schema", EVIDENCE_HEALTH_SCHEMA_VERSION);
    push_metric(&mut out, "Status", "advisory");
    push_metric(&mut out, "Root", report.root.as_str());
    push_metric(
        &mut out,
        "Calibration",
        report
            .calibration
            .source
            .as_deref()
            .unwrap_or("not provided"),
    );
    out.push('\n');

    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(&mut out, "Seams", report.metrics.seams_total);
    push_count(
        &mut out,
        "Headline-eligible seams",
        report.metrics.headline_eligible_total,
    );
    push_count(
        &mut out,
        "Weakly gripped seams",
        report.metrics.weakly_gripped_total,
    );
    push_count(&mut out, "Ungripped seams", report.metrics.ungripped_total);
    push_count(
        &mut out,
        "Missing discriminators",
        report.metrics.missing_discriminators_total,
    );
    push_count(
        &mut out,
        "Observed values",
        report.metrics.observed_values_total,
    );
    push_count(
        &mut out,
        "Related tests",
        report.metrics.related_tests_total,
    );
    push_count(
        &mut out,
        "Opaque oracle classifications",
        report.metrics.opaque_oracle_count,
    );
    out.push('\n');

    out.push_str("## Grip Classes\n\n");
    push_counts_table(&mut out, "Grip class", &report.metrics.grip_class_counts);

    out.push_str("## Missing Discriminators\n\n");
    if report.metrics.missing_discriminator_counts.is_empty() {
        out.push_str("No missing discriminators were reported.\n\n");
    } else {
        push_counts_table_limited(
            &mut out,
            "Missing discriminator",
            &report.metrics.missing_discriminator_counts,
            25,
        );
    }

    out.push_str("## Oracle Strength\n\n");
    push_counts_table(
        &mut out,
        "Oracle strength",
        &report.metrics.oracle_strength_counts,
    );

    out.push_str("## Related Test Confidence\n\n");
    push_counts_table(
        &mut out,
        "Relation confidence",
        &report.metrics.related_test_confidence_counts,
    );

    out.push_str("## Evidence Quality\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(
        &mut out,
        "Canonical gap groups",
        report.evidence_quality.canonical_gap_groups_total,
    );
    push_count(
        &mut out,
        "Duplicate-looking groups",
        report.evidence_quality.duplicate_looking_groups_total,
    );
    push_count(
        &mut out,
        "Records with canonical gap identity",
        report
            .evidence_quality
            .movement_availability
            .records_with_canonical_gap_id,
    );
    push_count(
        &mut out,
        "Records with complete evidence path",
        report
            .evidence_quality
            .movement_availability
            .records_with_complete_evidence_path,
    );
    push_count(
        &mut out,
        "Records with verify command",
        report
            .evidence_quality
            .movement_availability
            .records_with_verify_command,
    );
    out.push('\n');

    out.push_str("## Largest Canonical Gap Groups\n\n");
    if report.evidence_quality.largest_canonical_groups.is_empty() {
        out.push_str("No canonical gap groups were reported.\n\n");
    } else {
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

    out.push_str("## Actionability\n\n");
    push_counts_table(
        &mut out,
        "Actionability class",
        &report.evidence_quality.actionability_class_counts,
    );

    out.push_str("## Static Limitation Distribution\n\n");
    push_counts_table(
        &mut out,
        "Static limitation stage",
        &report.evidence_quality.static_limitation_stage_counts,
    );
    push_counts_table_limited(
        &mut out,
        "Static limitation reason",
        &report.evidence_quality.static_limitation_reason_counts,
        15,
    );

    out.push_str("## Evidence-Record Calibration Coverage\n\n");
    push_counts_table(
        &mut out,
        "Calibration availability",
        &report.evidence_quality.calibration_availability_counts,
    );

    out.push_str("## Top Evidence Quality Risks\n\n");
    if report
        .evidence_quality
        .top_evidence_quality_risks
        .is_empty()
    {
        out.push_str("No evidence-quality risks were reported.\n\n");
    } else {
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

    out.push_str("## Calibration Availability\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    push_count(
        &mut out,
        "Matched calibration rows",
        report.calibration.matched_total,
    );
    push_count(
        &mut out,
        "Static rows without runtime context",
        report.calibration.static_without_runtime_total,
    );
    push_count(
        &mut out,
        "Runtime rows without static seam",
        report.calibration.runtime_without_static_total,
    );
    push_count(
        &mut out,
        "Ambiguous file-line joins",
        report.calibration.ambiguous_file_line_total,
    );
    push_count(
        &mut out,
        "Unmatched runtime rows",
        report.calibration.unmatched_runtime_total,
    );
    out.push('\n');

    out.push_str("## Top Static Limitations\n\n");
    if report.top_static_limitations.is_empty() {
        out.push_str("No static limitations were reported.\n");
        return out;
    }
    out.push_str("### Categories\n\n");
    push_counts_table(
        &mut out,
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
    out
}

fn grip_class_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for class in SeamGripClass::ALL {
        counts.insert(class.as_str().to_string(), 0);
    }
    counts
}

fn stage_state_counts() -> BTreeMap<String, BTreeMap<String, usize>> {
    let mut counts = BTreeMap::new();
    for stage in ["reach", "activate", "propagate", "observe", "discriminate"] {
        counts.insert(stage.to_string(), labeled_counts(STAGE_LABELS));
    }
    counts
}

fn stage_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for stage in ["reach", "activate", "propagate", "observe", "discriminate"] {
        counts.insert(stage.to_string(), 0);
    }
    counts
}

fn unknown_stop_reason_counts() -> BTreeMap<String, usize> {
    labeled_counts(&[
        "activation_unknown",
        "propagation_unknown",
        "observation_unknown",
        "discrimination_unknown",
        "opaque",
    ])
}

fn labeled_counts(labels: &[&str]) -> BTreeMap<String, usize> {
    labels
        .iter()
        .map(|label| ((*label).to_string(), 0))
        .collect()
}

fn count_stage(
    stage_state_counts: &mut BTreeMap<String, BTreeMap<String, usize>>,
    unknown_stage_counts: &mut BTreeMap<String, usize>,
    limitations: &mut BTreeMap<String, StaticLimitationCounter>,
    stage: &str,
    state: &StageState,
    seam_id: &str,
) {
    if let Some(counts) = stage_state_counts.get_mut(stage) {
        increment(counts, state.as_str());
    }
    if matches!(state, StageState::Unknown | StageState::Opaque) {
        increment(unknown_stage_counts, stage);
        let kind = format!("{stage}_unknown");
        let summary = format!("The {stage} stage is unknown or opaque for at least one seam.");
        increment_limitation(limitations, &kind, &summary, seam_id);
    }
}

fn increment_unknown_stop_reason(counts: &mut BTreeMap<String, usize>, class: SeamGripClass) {
    let key = match class {
        SeamGripClass::ActivationUnknown => Some("activation_unknown"),
        SeamGripClass::PropagationUnknown => Some("propagation_unknown"),
        SeamGripClass::ObservationUnknown => Some("observation_unknown"),
        SeamGripClass::DiscriminationUnknown => Some("discrimination_unknown"),
        SeamGripClass::Opaque => Some("opaque"),
        _ => None,
    };
    if let Some(key) = key {
        increment(counts, key);
    }
}

fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    let entry = map.entry(key.to_string()).or_insert(0);
    *entry += 1;
}

fn increment_limitation(
    limitations: &mut BTreeMap<String, StaticLimitationCounter>,
    kind: &str,
    summary: &str,
    seam_id: &str,
) {
    let entry = limitations
        .entry(kind.to_string())
        .or_insert_with(|| StaticLimitationCounter {
            count: 0,
            summary: summary.to_string(),
            example_seam_id: None,
        });
    entry.count += 1;
    if entry.example_seam_id.is_none() {
        entry.example_seam_id = Some(seam_id.to_string());
    }
}

fn top_limitations(
    limitations: BTreeMap<String, StaticLimitationCounter>,
) -> Vec<StaticLimitation> {
    let mut rows: Vec<_> = limitations
        .into_iter()
        .map(|(kind, counter)| StaticLimitation {
            kind,
            count: counter.count,
            summary: counter.summary,
            example_seam_id: counter.example_seam_id,
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    rows.truncate(10);
    rows
}

fn count_for(counts: &BTreeMap<String, usize>, key: &str) -> usize {
    counts.get(key).copied().unwrap_or(0)
}

fn count_evidence_record_quality(
    record: &EvidenceRecord,
    canonical_gap: Option<&CanonicalGapIdentity>,
    counters: &mut EvidenceHealthQualityCounters,
) {
    if let Some(gap) = canonical_gap {
        let entry = counters
            .canonical_group_counts
            .entry(gap.id.clone())
            .or_insert_with(|| EvidenceHealthCanonicalGroupCounter {
                count: 0,
                reported_group_size: record.canonical_gap_group_size,
                owner: gap.owner.clone(),
                seam_kind: gap.seam_kind.clone(),
                flow_sink: gap.flow_sink.clone(),
                missing_discriminator: gap.missing_discriminator.clone(),
                assertion_shape: gap.assertion_shape.clone(),
                example_seam_id: record.seam_id.clone(),
                example_file: record.location.file.clone(),
            });
        entry.count += 1;
        entry.reported_group_size = record.canonical_gap_group_size;
    }

    increment(
        &mut counters.actionability_class_counts,
        &record.actionability.class,
    );
    increment(
        &mut counters.calibration_availability_counts,
        &record.calibration.availability,
    );

    if !record.seam_id.is_empty() {
        counters.movement_availability.records_with_seam_id += 1;
    }
    if record.canonical_gap_id.is_some() {
        counters.movement_availability.records_with_canonical_gap_id += 1;
    }
    if evidence_path_is_complete(record) {
        counters
            .movement_availability
            .records_with_complete_evidence_path += 1;
    }
    if !record.recommendation.action.is_empty() {
        counters.movement_availability.records_with_recommendation += 1;
    }
    if record.recommendation.verify_command.is_some() {
        counters.movement_availability.records_with_verify_command += 1;
    }

    for limitation in &record.static_limitations {
        increment(
            &mut counters.static_limitation_stage_counts,
            &limitation.stage,
        );
        increment(
            &mut counters.static_limitation_reason_counts,
            &limitation.reason,
        );
        increment(
            &mut counters.static_limitation_category_counts,
            &limitation.category,
        );
    }
}

fn evidence_path_is_complete(record: &EvidenceRecord) -> bool {
    [
        &record.evidence_path.reach,
        &record.evidence_path.activate,
        &record.evidence_path.propagate,
        &record.evidence_path.observe,
        &record.evidence_path.discriminate,
    ]
    .into_iter()
    .all(|stage| !stage.state.is_empty() && !stage.confidence.is_empty())
}

fn evidence_quality_from_counts(
    counters: EvidenceHealthQualityCounters,
    metrics: &EvidenceHealthMetrics,
) -> EvidenceHealthQuality {
    let mut groups = counters
        .canonical_group_counts
        .into_iter()
        .map(|(canonical_gap_id, counter)| EvidenceHealthCanonicalGroup {
            canonical_gap_id,
            count: counter.count,
            reported_group_size: counter.reported_group_size,
            owner: counter.owner,
            seam_kind: counter.seam_kind,
            flow_sink: counter.flow_sink,
            missing_discriminator: counter.missing_discriminator,
            assertion_shape: counter.assertion_shape,
            example_seam_id: counter.example_seam_id,
            example_file: counter.example_file,
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.canonical_gap_id.cmp(&right.canonical_gap_id))
    });
    let canonical_gap_groups_total = groups.len();
    let duplicate_looking_groups_total = groups.iter().filter(|group| group.count > 1).count();
    groups.truncate(EVIDENCE_HEALTH_TOP_GROUP_LIMIT);

    let static_limitations_total = counters.static_limitation_reason_counts.values().sum();
    let uncalibrated_records = count_for(&counters.calibration_availability_counts, "not_imported");
    let low_or_opaque_related_tests = count_for(&metrics.related_test_confidence_counts, "low")
        + count_for(&metrics.related_test_confidence_counts, "opaque");
    let mut risks = Vec::new();
    push_risk(
        &mut risks,
        "static_limitations",
        static_limitations_total,
        "Evidence records still contain static limitations.",
    );
    push_risk(
        &mut risks,
        "missing_discriminators",
        metrics.missing_discriminators_total,
        "Headline evidence still lacks concrete discriminator coverage.",
    );
    push_risk(
        &mut risks,
        "uncalibrated_records",
        uncalibrated_records,
        "Evidence records do not have imported runtime calibration data.",
    );
    push_risk(
        &mut risks,
        "duplicate_canonical_groups",
        duplicate_looking_groups_total,
        "Canonical gap groups still contain more than one raw seam.",
    );
    push_risk(
        &mut risks,
        "low_or_opaque_related_tests",
        low_or_opaque_related_tests,
        "Related-test evidence includes low-confidence or opaque rankings.",
    );
    risks.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    risks.truncate(EVIDENCE_HEALTH_TOP_RISK_LIMIT);

    EvidenceHealthQuality {
        canonical_gap_groups_total,
        duplicate_looking_groups_total,
        largest_canonical_groups: groups,
        actionability_class_counts: counters.actionability_class_counts,
        static_limitation_stage_counts: counters.static_limitation_stage_counts,
        static_limitation_reason_counts: counters.static_limitation_reason_counts,
        static_limitation_category_counts: counters.static_limitation_category_counts,
        calibration_availability_counts: counters.calibration_availability_counts,
        movement_availability: counters.movement_availability,
        top_evidence_quality_risks: risks,
    }
}

fn push_risk(risks: &mut Vec<EvidenceHealthRisk>, kind: &str, count: usize, summary: &str) {
    if count == 0 {
        return;
    }
    risks.push(EvidenceHealthRisk {
        kind: kind.to_string(),
        count,
        summary: summary.to_string(),
    });
}

fn usize_field(value: &Value, path: &[&str]) -> usize {
    let mut current = value;
    for segment in path {
        match current.get(*segment) {
            Some(next) => current = next,
            None => return 0,
        }
    }
    current.as_u64().map_or(0, |number| number as usize)
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, StageEvidence, StageState, ValueContext, ValueFact,
    };

    #[test]
    fn evidence_health_counts_core_metrics() -> Result<(), String> {
        let report = build_evidence_health_report(
            &[
                weak_boundary_seam(),
                weak_boundary_seam_at_line(121),
                opaque_call_seam(),
            ],
            ".".to_string(),
            EvidenceHealthCalibration::not_provided(),
        );
        let json = render_evidence_health_json(&report)?;
        let value: Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;

        assert_eq!(value["schema_version"], Value::from("0.1"));
        assert_eq!(value["metrics"]["seams_total"], Value::from(3));
        assert_eq!(value["metrics"]["weakly_gripped_total"], Value::from(2));
        assert_eq!(value["metrics"]["ungripped_total"], Value::from(1));
        assert_eq!(
            value["metrics"]["missing_discriminators_total"],
            Value::from(2)
        );
        assert_eq!(value["metrics"]["observed_values_total"], Value::from(2));
        assert_eq!(value["metrics"]["related_tests_total"], Value::from(3));
        assert_eq!(value["metrics"]["opaque_oracle_count"], Value::from(1));
        assert_eq!(
            value["metrics"]["related_test_confidence_counts"]["high"],
            Value::from(2)
        );
        assert_eq!(
            value["metrics"]["oracle_strength_counts"]["unknown"],
            Value::from(1)
        );
        assert_eq!(
            value["evidence_quality"]["canonical_gap_groups_total"],
            Value::from(2)
        );
        assert_eq!(
            value["evidence_quality"]["duplicate_looking_groups_total"],
            Value::from(1)
        );
        assert_eq!(
            value["evidence_quality"]["largest_canonical_groups"][0]["count"],
            Value::from(2)
        );
        assert_eq!(
            value["evidence_quality"]["movement_availability"]["records_with_canonical_gap_id"],
            Value::from(3)
        );
        assert_eq!(
            value["evidence_quality"]["actionability_class_counts"]["actionable_related_test_extension"],
            Value::from(2)
        );
        assert_eq!(
            value["evidence_quality"]["calibration_availability_counts"]["not_imported"],
            Value::from(3)
        );
        assert_eq!(
            value["evidence_quality"]["static_limitation_category_counts"]["activation_static_unknown"],
            Value::from(1)
        );
        Ok(())
    }

    #[test]
    fn evidence_health_markdown_names_calibration_and_limitations() -> Result<(), String> {
        let report = build_evidence_health_report(
            &[weak_boundary_seam(), opaque_call_seam()],
            ".".to_string(),
            EvidenceHealthCalibration::from_json(
                "target/ripr/reports/mutation-calibration.json".to_string(),
                r#"{
                  "summary": {
                    "matched_total": 2,
                    "static_without_runtime_total": 1,
                    "ambiguous_file_line_total": 1,
                    "unmatched_mutants_total": 3
                  },
                  "missed_runtime_signals": [{"id": "runtime-only"}]
                }"#,
            )?,
        );
        let markdown = render_evidence_health_markdown(&report);

        assert!(markdown.contains("RIPR evidence health report"));
        assert!(markdown.contains("Evidence Quality"));
        assert!(markdown.contains("Largest Canonical Gap Groups"));
        assert!(markdown.contains("Evidence-Record Calibration Coverage"));
        assert!(markdown.contains("Top Evidence Quality Risks"));
        assert!(markdown.contains("Matched calibration rows"));
        assert!(markdown.contains("missing_discriminator"));
        assert!(markdown.contains("Categories"));
        assert!(markdown.contains("activation_static_unknown"));
        assert!(markdown.contains("Runtime rows without static seam"));
        Ok(())
    }

    fn weak_boundary_seam() -> ClassifiedSeam {
        weak_boundary_seam_at_line(120)
    }

    fn weak_boundary_seam_at_line(line: usize) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discount",
            SeamKind::PredicateBoundary,
            line,
            42,
            "amount >= threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount == threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: "discounts_large_orders".to_string(),
                    file: PathBuf::from("tests/pricing.rs"),
                    line: 12,
                    oracle_kind: OracleKind::ExactValue,
                    oracle_strength: OracleStrength::Strong,
                    evidence_summary: "asserts returned discount".to_string(),
                    relation_reason: RelationReason::DirectOwnerCall,
                    relation_confidence: RelationConfidence::High,
                }],
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "direct call"),
                activate: StageEvidence::new(StageState::Yes, Confidence::High, "value observed"),
                propagate: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "return reaches assertion",
                ),
                observe: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "assertion is nearby",
                ),
                discriminate: StageEvidence::new(
                    StageState::No,
                    Confidence::Unknown,
                    "boundary value missing",
                ),
                observed_values: vec![ValueFact {
                    line: 12,
                    text: "discounted_total(150)".to_string(),
                    value: "150".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount == threshold".to_string(),
                    reason: "equality boundary not observed".to_string(),
                    flow_sink: None,
                }],
            },
            class: SeamGripClass::WeaklyGripped,
        }
    }

    fn opaque_call_seam() -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discount",
            SeamKind::CallPresence,
            180,
            55,
            "apply_discount(amount)",
            RequiredDiscriminator::CallSite {
                target: "apply_discount".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: "discounts_smoke".to_string(),
                    file: PathBuf::from("tests/pricing.rs"),
                    line: 22,
                    oracle_kind: OracleKind::Unknown,
                    oracle_strength: OracleStrength::Unknown,
                    evidence_summary: "helper assertion not classified".to_string(),
                    relation_reason: RelationReason::SameTestFile,
                    relation_confidence: RelationConfidence::Opaque,
                }],
                reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "same file"),
                activate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Unknown,
                    "call target not activated",
                ),
                propagate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Unknown,
                    "propagation unknown",
                ),
                observe: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Unknown,
                    "oracle unknown",
                ),
                discriminate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Unknown,
                    "discriminator unknown",
                ),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
            class: SeamGripClass::Ungripped,
        }
    }
}
