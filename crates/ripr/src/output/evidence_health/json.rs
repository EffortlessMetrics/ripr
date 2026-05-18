use serde_json::json;

use super::{EVIDENCE_HEALTH_SCHEMA_VERSION, EvidenceHealthReport};

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
