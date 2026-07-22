//! Evidence-quality cluster: the Lane 1 evidence quality scorecard (input
//! artifacts, summary, maturity rows, recommended repairs, recent deltas,
//! unknowns, JSON/markdown renderers) and the evidence-quality trend report
//! (previous-artifact comparison, metric trends, static-only class trends,
//! movement front, JSON/markdown renderers), plus the report-local
//! `audit_*`, `finding_alignment_*`, `scorecard_*`, and `trend_*` helper
//! atoms that sit physically inside this region.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` where `main.rs` re-exports them so
//! existing call sites (`reports/repo.rs` and `tests.rs`) compile unchanged.

use super::*;

const EVIDENCE_QUALITY_SCORECARD_SCHEMA_VERSION: &str = "0.1";
const EVIDENCE_QUALITY_SCORECARD_REPAIR_LIMIT: usize = 5;
const EVIDENCE_QUALITY_SCORECARD_WORK_QUEUE_REPAIR_LIMIT: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityScorecardInput {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) schema_version: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityScorecardInputs {
    pub(crate) lane1_evidence_audit: EvidenceQualityScorecardInput,
    pub(crate) evidence_health: EvidenceQualityScorecardInput,
    pub(crate) previous_scorecard: EvidenceQualityScorecardInput,
    pub(crate) capability_matrix: EvidenceQualityScorecardInput,
    pub(crate) capabilities: EvidenceQualityScorecardInput,
    pub(crate) traceability: EvidenceQualityScorecardInput,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EvidenceQualityScorecardSummary {
    pub(crate) raw_headline_gaps: usize,
    pub(crate) canonical_gap_groups_total: usize,
    pub(crate) duplicate_looking_groups_total: usize,
    pub(crate) missing_discriminators_total: usize,
    pub(crate) static_limitations_total: usize,
    pub(crate) related_tests_total: usize,
    pub(crate) low_or_opaque_top_related_tests: usize,
    pub(crate) calibrated_records: usize,
    pub(crate) uncalibrated_records: usize,
    pub(crate) evidence_records_total: usize,
    pub(crate) evidence_records_missing: usize,
    pub(crate) top_repair_count: usize,
    pub(crate) recent_delta_available: bool,
    pub(crate) finding_alignment_raw_findings_total: usize,
    pub(crate) finding_alignment_raw_signals_total: usize,
    pub(crate) finding_alignment_canonical_items_total: usize,
    pub(crate) finding_alignment_aligned_raw_findings_total: usize,
    pub(crate) finding_alignment_unaligned_raw_findings_total: usize,
    pub(crate) finding_alignment_duplicate_groups_total: usize,
    pub(crate) finding_alignment_actionable_items_total: usize,
    pub(crate) finding_alignment_actionable_unresolved_canonical_gaps: usize,
    pub(crate) finding_alignment_already_observed_total: usize,
    pub(crate) finding_alignment_internal_only_total: usize,
    pub(crate) finding_alignment_internal_no_action_total: usize,
    pub(crate) finding_alignment_static_limitation_total: usize,
    pub(crate) finding_alignment_unknown_total: usize,
    pub(crate) finding_alignment_calibrated_supported_total: usize,
    pub(crate) finding_alignment_uncalibrated_total: usize,
    pub(crate) finding_alignment_visibility_unknown_total: usize,
    pub(crate) finding_alignment_presentation_text_actionable_total: usize,
    pub(crate) finding_alignment_static_unknown_without_named_limitation: usize,
    pub(crate) finding_alignment_canonical_items_without_repair_route: usize,
    pub(crate) finding_alignment_canonical_items_without_verify_command: usize,
    pub(crate) finding_alignment_actionable_gap_packet_public_projection_eligible_packets: usize,
    pub(crate) finding_alignment_actionable_gap_packet_public_projection_excluded_packets: usize,
    pub(crate) presentation_text_total: usize,
    pub(crate) presentation_text_user_visible: usize,
    pub(crate) presentation_text_observed: usize,
    pub(crate) presentation_text_unobserved: usize,
    pub(crate) presentation_text_internal_only: usize,
    pub(crate) presentation_text_visibility_unknown: usize,
    pub(crate) presentation_text_observer_unknown: usize,
    pub(crate) presentation_text_duplicate_groups: usize,
    pub(crate) presentation_text_actionable_snapshot: usize,
    pub(crate) presentation_text_no_action: usize,
    pub(crate) presentation_text_static_limitations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityMaturityRow {
    pub(crate) class: String,
    pub(crate) status: String,
    pub(crate) proof_source: String,
    pub(crate) known_limits: String,
    pub(crate) recommended_next_repair: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityRepair {
    pub(crate) slice: String,
    pub(crate) priority: usize,
    pub(crate) evidence_class: String,
    pub(crate) risk_kind: String,
    pub(crate) signal_count: usize,
    pub(crate) why: String,
    pub(crate) expected_impact: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityDelta {
    pub(crate) metric: String,
    pub(crate) before: usize,
    pub(crate) after: usize,
    pub(crate) delta: isize,
    pub(crate) direction: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityDeltas {
    pub(crate) available: bool,
    pub(crate) source: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) deltas: Vec<EvidenceQualityDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityUnknown {
    pub(crate) kind: String,
    pub(crate) summary: String,
    pub(crate) next_repair: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceQualityScorecardReport {
    pub(crate) generated_at: String,
    pub(crate) root: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) inputs: EvidenceQualityScorecardInputs,
    pub(crate) summary: EvidenceQualityScorecardSummary,
    pub(crate) maturity_by_class: Vec<EvidenceQualityMaturityRow>,
    pub(crate) canonical_gap_groups: Value,
    pub(crate) duplicate_looking_groups: Value,
    pub(crate) static_limitation_categories: Value,
    pub(crate) missing_discriminator_classes: Value,
    pub(crate) related_test_confidence: Value,
    pub(crate) oracle_semantics_distribution: Value,
    pub(crate) movement_availability: Value,
    pub(crate) calibration_coverage: Value,
    pub(crate) actionable_gap_top_lists: Value,
    pub(crate) actionable_gap_packet_public_projection: Value,
    pub(crate) evidence_class_work_queue: Value,
    pub(crate) language_aware_placement_route_quality: Value,
    pub(crate) cross_language_oracle_route_quality: Value,
    pub(crate) recommended_repairs: Vec<EvidenceQualityRepair>,
    pub(crate) recent_audit_deltas: EvidenceQualityDeltas,
    pub(crate) unknowns: Vec<EvidenceQualityUnknown>,
}

/// Build the Lane 1 evidence quality scorecard from current audit artifacts.
/// This report is advisory and does not change analyzer behavior, gate policy,
/// PR projection, editor output, or runtime execution.
pub(crate) fn evidence_quality_scorecard_report_impl() -> Result<(), String> {
    ensure_reports_dir()?;
    let scorecard_path = reports_dir().join("evidence-quality-scorecard.json");
    let previous_scorecard = scorecard_optional_json(&scorecard_path)?;

    let audit_path = reports_dir().join("lane1-evidence-audit.json");
    let evidence_health_path = reports_dir().join("evidence-health.json");
    if !audit_path.exists()
        && let Err(err) = lane1_evidence_audit_report_impl()
    {
        return write_limited_evidence_quality_scorecard_for_audit_regeneration_failure(
            &audit_path,
            &evidence_health_path,
            &scorecard_path,
            previous_scorecard.as_ref(),
            &err,
        );
    }
    let audit = read_json_value(&audit_path).map_err(|err| {
        format!("evidence-quality-scorecard requires lane1-evidence-audit.json; {err}")
    })?;

    let evidence_health = scorecard_optional_json(&evidence_health_path)?;
    let inputs = evidence_quality_scorecard_inputs(
        &audit_path,
        &evidence_health_path,
        &scorecard_path,
        previous_scorecard.as_ref(),
    )?;
    let report = evidence_quality_scorecard_from_values(
        evidence_quality_scorecard_generated_at()?,
        inputs,
        &audit,
        evidence_health.as_ref(),
        previous_scorecard.as_ref(),
    )?;

    write_report(
        "evidence-quality-scorecard.json",
        &evidence_quality_scorecard_json(&report)?,
    )?;
    write_report(
        "evidence-quality-scorecard.md",
        &evidence_quality_scorecard_markdown(&report),
    )
}

fn write_limited_evidence_quality_scorecard_for_audit_regeneration_failure(
    audit_path: &Path,
    evidence_health_path: &Path,
    scorecard_path: &Path,
    previous_scorecard: Option<&Value>,
    error: &str,
) -> Result<(), String> {
    let audit_report = evidence_quality_scorecard_audit_regeneration_failure_report(error);
    let audit_json = lane1_evidence_audit_json(&audit_report)?;
    write_report("lane1-evidence-audit.json", &audit_json)?;
    write_report(
        "lane1-evidence-audit.md",
        &lane1_evidence_audit_markdown(&audit_report),
    )?;
    write_report(
        "actionable-gaps.json",
        &lane1_actionable_gap_packets_json(&audit_report)?,
    )?;
    write_report(
        "actionable-gaps.md",
        &lane1_actionable_gap_packets_markdown(&audit_report),
    )?;
    let repo_exposure_path = reports_dir().join("lane1-evidence-audit.repo-exposure.json");
    if repo_exposure_path.exists() {
        fs::remove_file(&repo_exposure_path).map_err(|err| {
            format!(
                "failed to remove incomplete Lane 1 repo exposure input {}: {err}",
                repo_exposure_path.display()
            )
        })?;
    }

    let audit: Value = serde_json::from_str(&audit_json).map_err(|err| {
        format!("failed to parse limited scorecard audit-regeneration JSON: {err}")
    })?;
    let evidence_health = scorecard_optional_json(evidence_health_path)?;
    let inputs = evidence_quality_scorecard_inputs(
        audit_path,
        evidence_health_path,
        scorecard_path,
        previous_scorecard,
    )?;
    let report = evidence_quality_scorecard_from_values(
        evidence_quality_scorecard_generated_at()?,
        inputs,
        &audit,
        evidence_health.as_ref(),
        previous_scorecard,
    )?;

    write_report(
        "evidence-quality-scorecard.json",
        &evidence_quality_scorecard_json(&report)?,
    )?;
    write_report(
        "evidence-quality-scorecard.md",
        &evidence_quality_scorecard_markdown(&report),
    )
}

fn evidence_quality_scorecard_audit_regeneration_failure_report(
    error: &str,
) -> Lane1EvidenceAuditReport {
    let mut report = Lane1EvidenceAuditBuilder::default().finish(".".to_string(), None);
    let summary = evidence_quality_scorecard_error_summary(error);
    report
        .run_limitations
        .push(Lane1EvidenceAuditRunLimitation {
            category: EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED.to_string(),
            phase: "scorecard_missing_audit_regeneration".to_string(),
            input: "lane1-evidence-audit.json".to_string(),
            observed_seams: None,
            cache_limit: None,
            summary: format!(
                "Evidence-quality scorecard could not regenerate the required Lane 1 audit: {summary}. No user test debt is claimed from this limited scorecard."
            ),
            repair_route:
                "rerun cargo xtask lane1-evidence-audit with bounded diagnostics before scorecard generation"
                    .to_string(),
            timeout_ms: None,
            duration_ms: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            exit_code: None,
            stdout_bytes: None,
            stderr_bytes: None,
            latency_trace_tail: Vec::new(),
        });
    report.static_limitation_category_counts.insert(
        EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED.to_string(),
        1,
    );
    report.summary.static_limitations_total += 1;
    report.static_limitation_repair_route_counts.insert(
        "report/evidence-quality-scorecard-bounded-diagnostics".to_string(),
        1,
    );
    report
}

#[cfg(test)]
pub(crate) fn evidence_quality_scorecard_audit_regeneration_failure_audit(
    error: &str,
) -> Result<Value, String> {
    let report = evidence_quality_scorecard_audit_regeneration_failure_report(error);
    let json = lane1_evidence_audit_json(&report)?;
    serde_json::from_str(&json)
        .map_err(|err| format!("failed to build limited scorecard audit-regeneration JSON: {err}"))
}

fn evidence_quality_scorecard_error_summary(error: &str) -> String {
    const LIMIT: usize = 360;
    let mut summary = error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown error")
        .to_string();
    if summary.len() > LIMIT {
        summary.truncate(LIMIT);
        summary.push_str("...");
    }
    summary
}

fn evidence_quality_scorecard_inputs(
    audit_path: &Path,
    evidence_health_path: &Path,
    previous_scorecard_path: &Path,
    previous_scorecard: Option<&Value>,
) -> Result<EvidenceQualityScorecardInputs, String> {
    Ok(EvidenceQualityScorecardInputs {
        lane1_evidence_audit: scorecard_input_artifact(
            audit_path,
            "loaded",
            None,
            "required Lane 1 evidence-quality audit input",
        )?,
        evidence_health: scorecard_input_artifact(
            evidence_health_path,
            "optional",
            None,
            "optional durable evidence-health audit fields",
        )?,
        previous_scorecard: scorecard_input_artifact(
            previous_scorecard_path,
            if previous_scorecard.is_some() {
                "loaded"
            } else {
                "missing"
            },
            previous_scorecard,
            "optional previous scorecard for recent deltas",
        )?,
        capability_matrix: scorecard_input_artifact(
            Path::new("docs/CAPABILITY_MATRIX.md"),
            "loaded",
            None,
            "class-scoped capability maturity vocabulary",
        )?,
        capabilities: scorecard_input_artifact(
            Path::new("metrics/capabilities.toml"),
            "loaded",
            None,
            "machine-readable capability maturity metadata",
        )?,
        traceability: scorecard_input_artifact(
            Path::new(".ripr/traceability.toml"),
            "loaded",
            None,
            "spec/test/code/output/metric linkage",
        )?,
    })
}

fn scorecard_input_artifact(
    path: &Path,
    present_status: &str,
    value: Option<&Value>,
    note: &str,
) -> Result<EvidenceQualityScorecardInput, String> {
    if !path.exists() {
        return Ok(EvidenceQualityScorecardInput {
            path: normalize_path(path),
            status: "missing".to_string(),
            schema_version: None,
            sha256: None,
            note: Some(note.to_string()),
        });
    }
    Ok(EvidenceQualityScorecardInput {
        path: normalize_path(path),
        status: present_status.to_string(),
        schema_version: value.and_then(|value| audit_string(value, &["schema_version"])),
        sha256: Some(sha256_file(path)?),
        note: Some(note.to_string()),
    })
}

fn scorecard_optional_json(path: &Path) -> Result<Option<Value>, String> {
    if path.exists() {
        read_json_value(path).map(Some)
    } else {
        Ok(None)
    }
}

fn evidence_quality_scorecard_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

pub(crate) fn generated_at_unix_ms() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

pub(crate) fn evidence_quality_scorecard_from_values(
    generated_at: String,
    inputs: EvidenceQualityScorecardInputs,
    audit: &Value,
    evidence_health: Option<&Value>,
    previous_scorecard: Option<&Value>,
) -> Result<EvidenceQualityScorecardReport, String> {
    let root = audit_string(audit, &["inputs", "root"]).unwrap_or_else(|| ".".to_string());
    let mut summary = evidence_quality_scorecard_summary(audit);
    let maturity_by_class = evidence_quality_maturity_rows(&summary, audit);
    let mut recommended_repairs = evidence_quality_recommended_repairs(&summary, audit);
    recommended_repairs.truncate(EVIDENCE_QUALITY_SCORECARD_REPAIR_LIMIT);
    summary.top_repair_count = recommended_repairs.len();
    let recent_audit_deltas =
        evidence_quality_recent_deltas(&summary, previous_scorecard, &inputs.previous_scorecard);
    summary.recent_delta_available = recent_audit_deltas.available;
    let unknowns = evidence_quality_unknowns(&summary, audit, evidence_health, &inputs);
    let runtime_status = evidence_quality_scorecard_runtime_status(audit, evidence_health);
    let calibration_coverage = evidence_quality_calibration_coverage(&summary, audit);

    Ok(EvidenceQualityScorecardReport {
        generated_at,
        root,
        runtime_status,
        inputs,
        summary,
        maturity_by_class,
        canonical_gap_groups: scorecard_value_or_default(
            audit,
            &["canonical_gap_groups"],
            serde_json::json!({"total": 0, "largest": []}),
        ),
        duplicate_looking_groups: scorecard_value_or_default(
            audit,
            &["duplicate_looking_groups"],
            serde_json::json!([]),
        ),
        static_limitation_categories: scorecard_value_or_default(
            audit,
            &["static_limitations"],
            serde_json::json!({
                "by_reason": [],
                "by_stage": {},
                "by_category": {},
                "repair_routes": {}
            }),
        ),
        missing_discriminator_classes: scorecard_value_or_default(
            audit,
            &["missing_discriminator_classes"],
            serde_json::json!({"by_reason": [], "by_flow_sink": {}, "by_value": []}),
        ),
        related_test_confidence: scorecard_value_or_default(
            audit,
            &["related_test_ranking"],
            serde_json::json!({}),
        ),
        oracle_semantics_distribution: scorecard_value_or_default(
            audit,
            &["oracle_semantics_distribution"],
            serde_json::json!({}),
        ),
        movement_availability: scorecard_value_or_default(
            audit,
            &["movement_availability"],
            serde_json::json!({}),
        ),
        calibration_coverage,
        actionable_gap_top_lists: scorecard_value_or_default(
            audit,
            &["finding_alignment", "actionable_gap_top_lists"],
            serde_json::json!({
                "top_actionable_gap_classes": [],
                "top_actionable_files": [],
                "top_repair_kinds": [],
                "top_missing_discriminator_kinds": [],
                "top_static_limitation_reasons": [],
                "top_verify_command_unknowns": [],
                "top_repair_route_unknowns": [],
            }),
        ),
        actionable_gap_packet_public_projection: scorecard_value_or_default(
            audit,
            &[
                "finding_alignment",
                "actionable_gap_packet_public_projection",
            ],
            serde_json::json!({
                "scope": "emitted_actionable_gap_packets",
                "public_projection_eligible_packets": 0,
                "public_projection_excluded_packets": 0,
                "projection_exclusion_reasons": [],
            }),
        ),
        evidence_class_work_queue: scorecard_value_or_default(
            audit,
            &["finding_alignment", "coverage", "evidence_class_work_queue"],
            serde_json::json!([]),
        ),
        language_aware_placement_route_quality:
            evidence_quality_language_aware_placement_route_quality(audit),
        cross_language_oracle_route_quality: cross_language_oracle_route_quality_report_value(),
        recommended_repairs,
        recent_audit_deltas,
        unknowns,
    })
}

fn evidence_quality_scorecard_runtime_status(
    audit: &Value,
    evidence_health: Option<&Value>,
) -> Lane1RuntimeStatus {
    if let Some(status) = lane1_runtime_status_from_report_value(audit)
        && status.state != "full"
    {
        return status;
    }
    if let Some(evidence_health) = evidence_health
        && let Some(status) = lane1_runtime_status_from_report_value(evidence_health)
        && status.state != "full"
    {
        return status;
    }
    lane1_runtime_status_full()
}

fn evidence_quality_scorecard_summary(audit: &Value) -> EvidenceQualityScorecardSummary {
    EvidenceQualityScorecardSummary {
        raw_headline_gaps: audit_usize(audit, &["summary", "raw_headline_gaps"]).unwrap_or(0),
        canonical_gap_groups_total: audit_usize(audit, &["summary", "canonical_gap_groups_total"])
            .or_else(|| audit_usize(audit, &["canonical_gap_groups", "total"]))
            .unwrap_or(0),
        duplicate_looking_groups_total: audit_usize(
            audit,
            &["summary", "duplicate_looking_groups_total"],
        )
        .unwrap_or(0),
        missing_discriminators_total: audit_usize(
            audit,
            &["summary", "missing_discriminators_total"],
        )
        .unwrap_or(0),
        static_limitations_total: evidence_quality_scorecard_static_limitations_total(audit),
        related_tests_total: audit_usize(audit, &["summary", "related_tests_total"]).unwrap_or(0),
        low_or_opaque_top_related_tests: audit_usize(
            audit,
            &["summary", "low_or_opaque_top_related_tests"],
        )
        .unwrap_or(0),
        calibrated_records: audit_usize(audit, &["summary", "calibrated_records"]).unwrap_or(0),
        uncalibrated_records: audit_usize(audit, &["summary", "uncalibrated_records"]).unwrap_or(0),
        evidence_records_total: audit_usize(audit, &["summary", "evidence_records_total"])
            .unwrap_or(0),
        evidence_records_missing: audit_usize(audit, &["summary", "evidence_records_missing"])
            .unwrap_or(0),
        top_repair_count: 0,
        recent_delta_available: false,
        finding_alignment_raw_findings_total: finding_alignment_summary_usize(
            audit,
            "raw_signals",
            "finding_alignment_raw_findings_total",
        )
        .unwrap_or(0),
        finding_alignment_raw_signals_total: finding_alignment_summary_usize(
            audit,
            "raw_signals",
            "finding_alignment_raw_signals_total",
        )
        .unwrap_or(0),
        finding_alignment_canonical_items_total: finding_alignment_summary_usize(
            audit,
            "canonical_items",
            "finding_alignment_canonical_items_total",
        )
        .unwrap_or(0),
        finding_alignment_aligned_raw_findings_total: finding_alignment_summary_usize(
            audit,
            "aligned_raw_findings",
            "finding_alignment_aligned_raw_findings_total",
        )
        .unwrap_or(0),
        finding_alignment_unaligned_raw_findings_total: finding_alignment_summary_usize(
            audit,
            "unaligned_raw_findings",
            "finding_alignment_unaligned_raw_findings_total",
        )
        .unwrap_or(0),
        finding_alignment_duplicate_groups_total: finding_alignment_summary_usize(
            audit,
            "duplicate_groups_total",
            "finding_alignment_duplicate_groups_total",
        )
        .unwrap_or(0),
        finding_alignment_actionable_items_total: finding_alignment_summary_usize(
            audit,
            "actionable_gaps",
            "finding_alignment_actionable_items_total",
        )
        .unwrap_or(0),
        finding_alignment_actionable_unresolved_canonical_gaps: finding_alignment_summary_usize(
            audit,
            "actionable_gaps",
            "finding_alignment_actionable_unresolved_canonical_gaps",
        )
        .unwrap_or(0),
        finding_alignment_already_observed_total: finding_alignment_summary_usize(
            audit,
            "already_observed",
            "finding_alignment_already_observed_total",
        )
        .unwrap_or(0),
        finding_alignment_internal_only_total: finding_alignment_summary_usize(
            audit,
            "internal_no_action",
            "finding_alignment_internal_only_total",
        )
        .unwrap_or(0),
        finding_alignment_internal_no_action_total: finding_alignment_summary_usize(
            audit,
            "internal_no_action",
            "finding_alignment_internal_no_action_total",
        )
        .unwrap_or(0),
        finding_alignment_static_limitation_total: finding_alignment_summary_usize(
            audit,
            "static_limitations",
            "finding_alignment_static_limitation_total",
        )
        .unwrap_or(0),
        finding_alignment_unknown_total: finding_alignment_summary_usize(
            audit,
            "unknown",
            "finding_alignment_unknown_total",
        )
        .unwrap_or(0),
        finding_alignment_calibrated_supported_total: finding_alignment_summary_usize(
            audit,
            "calibrated_supported",
            "finding_alignment_calibrated_supported_total",
        )
        .unwrap_or(0),
        finding_alignment_uncalibrated_total: finding_alignment_summary_usize(
            audit,
            "uncalibrated",
            "finding_alignment_uncalibrated_total",
        )
        .unwrap_or(0),
        finding_alignment_visibility_unknown_total: finding_alignment_summary_usize(
            audit,
            "presentation_text_visibility_unknown",
            "finding_alignment_visibility_unknown_total",
        )
        .unwrap_or(0),
        finding_alignment_presentation_text_actionable_total: finding_alignment_summary_usize(
            audit,
            "presentation_text_actionable_output_repairs",
            "finding_alignment_presentation_text_actionable_total",
        )
        .unwrap_or_else(|| {
            finding_alignment_summary_usize(
                audit,
                "presentation_text_actionable_snapshot",
                "finding_alignment_presentation_text_actionable_total",
            )
            .unwrap_or(0)
        }),
        finding_alignment_static_unknown_without_named_limitation:
            finding_alignment_coverage_usize(audit, "static_unknown_without_named_limitation")
                .unwrap_or(0),
        finding_alignment_canonical_items_without_repair_route: finding_alignment_coverage_usize(
            audit,
            "canonical_items_without_repair_route",
        )
        .unwrap_or(0),
        finding_alignment_canonical_items_without_verify_command: finding_alignment_coverage_usize(
            audit,
            "canonical_items_without_verify_command",
        )
        .unwrap_or(0),
        finding_alignment_actionable_gap_packet_public_projection_eligible_packets:
            finding_alignment_actionable_gap_packet_public_projection_usize(
                audit,
                "public_projection_eligible_packets",
            )
            .unwrap_or(0),
        finding_alignment_actionable_gap_packet_public_projection_excluded_packets:
            finding_alignment_actionable_gap_packet_public_projection_usize(
                audit,
                "public_projection_excluded_packets",
            )
            .unwrap_or(0),
        presentation_text_total: presentation_text_summary_usize(audit, "presentation_text_total")
            .unwrap_or(0),
        presentation_text_user_visible: presentation_text_summary_usize(
            audit,
            "presentation_text_user_visible",
        )
        .unwrap_or(0),
        presentation_text_observed: presentation_text_summary_usize(
            audit,
            "presentation_text_observed",
        )
        .unwrap_or(0),
        presentation_text_unobserved: presentation_text_summary_usize(
            audit,
            "presentation_text_unobserved",
        )
        .unwrap_or(0),
        presentation_text_internal_only: presentation_text_summary_usize(
            audit,
            "presentation_text_internal_only",
        )
        .unwrap_or(0),
        presentation_text_visibility_unknown: presentation_text_summary_usize(
            audit,
            "presentation_text_visibility_unknown",
        )
        .unwrap_or(0),
        presentation_text_observer_unknown: presentation_text_summary_usize(
            audit,
            "presentation_text_observer_unknown",
        )
        .unwrap_or(0),
        presentation_text_duplicate_groups: presentation_text_summary_usize(
            audit,
            "presentation_text_duplicate_groups",
        )
        .unwrap_or(0),
        presentation_text_actionable_snapshot: presentation_text_summary_usize(
            audit,
            "presentation_text_actionable_snapshot",
        )
        .or_else(|| {
            presentation_text_summary_usize(audit, "presentation_text_actionable_output_repairs")
        })
        .unwrap_or(0),
        presentation_text_no_action: presentation_text_summary_usize(
            audit,
            "presentation_text_no_action",
        )
        .unwrap_or(0),
        presentation_text_static_limitations: presentation_text_summary_usize(
            audit,
            "presentation_text_static_limitations",
        )
        .unwrap_or(0),
    }
}

fn evidence_quality_scorecard_static_limitations_total(audit: &Value) -> usize {
    let summary_total = audit_usize(audit, &["summary", "static_limitations_total"]).unwrap_or(0);
    let category_total = audit_get(audit, &["static_limitations", "by_category"])
        .and_then(Value::as_object)
        .map(|counts| {
            counts
                .values()
                .filter_map(Value::as_u64)
                .map(|count| count as usize)
                .sum::<usize>()
        })
        .unwrap_or(0);
    summary_total.max(category_total)
}

fn evidence_quality_language_aware_placement_route_quality(audit: &Value) -> Value {
    let target_unresolved_signals = scorecard_count_at(
        audit,
        &[
            "static_limitations",
            "by_category",
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY,
        ],
    );
    let target_route_signals = scorecard_count_at(
        audit,
        &[
            "static_limitations",
            "repair_routes",
            CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE,
        ],
    );
    let oracle_unresolved_signals = scorecard_count_at(
        audit,
        &[
            "static_limitations",
            "by_category",
            CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY,
        ],
    );
    let oracle_route_signals = scorecard_count_at(
        audit,
        &[
            "static_limitations",
            "repair_routes",
            CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE,
        ],
    );
    let navigation_only_external_target_packets =
        scorecard_navigation_only_external_target_packet_count(audit, false);
    let navigation_only_external_target_public_promotions =
        scorecard_navigation_only_external_target_packet_count(audit, true);
    let projection_exclusions = audit_count_rows_map(
        audit,
        &[
            "finding_alignment",
            "actionable_gap_packet_public_projection",
            "projection_exclusion_reasons",
        ],
    );
    let cross_language_projection_exclusions = projection_exclusions
        .get(CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY)
        .copied()
        .unwrap_or_default();
    let total_language_aware_signals = target_unresolved_signals
        + target_route_signals
        + oracle_unresolved_signals
        + oracle_route_signals
        + navigation_only_external_target_packets;
    let status = if navigation_only_external_target_packets > 0 {
        "navigation_only_context_visible"
    } else if total_language_aware_signals > 0 {
        "static_limitation_route_visible"
    } else {
        "no_current_signal"
    };
    let navigation_only_external_target_status = if navigation_only_external_target_packets > 0 {
        "explicit_navigation_context_only"
    } else if target_unresolved_signals > 0 || target_route_signals > 0 {
        "target_unresolved_no_navigation_target_in_audit"
    } else {
        "not_applicable"
    };

    serde_json::json!({
        "status": status,
        "repair_route": CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE,
        "cross_language_target_unresolved_signals": target_unresolved_signals,
        "cross_language_test_target_inference_route_signals": target_route_signals,
        "cross_language_oracle_visibility_unresolved_signals": oracle_unresolved_signals,
        "cross_language_oracle_visibility_route_signals": oracle_route_signals,
        "cross_language_projection_exclusions": cross_language_projection_exclusions,
        "navigation_only_external_target_packets": navigation_only_external_target_packets,
        "navigation_only_external_target_public_promotions":
            navigation_only_external_target_public_promotions,
        "navigation_only_external_target_status": navigation_only_external_target_status,
        "calibration_boundary": "static placement limitations and navigation-only targets are not runtime calibration or public repair packets",
        "non_claims": [
            "not a public repair packet",
            "not swarm-ready work",
            "do not infer external test targets without explicit observer evidence",
            "do not promote navigation-only context into verify, receipt, or allowed edit surface",
            "do not claim full cross-language oracle proof"
        ],
    })
}

fn scorecard_navigation_only_external_target_packet_count(
    audit: &Value,
    public_only: bool,
) -> usize {
    [
        &["finding_alignment", "actionable_gap_packets"][..],
        &["actionable_gap_packets"][..],
        &["packets"][..],
    ]
    .iter()
    .flat_map(|path| audit_array(audit, path))
    .filter(|packet| audit_get(packet, &["navigation_only_target"]).is_some())
    .filter(|packet| {
        !public_only || audit_bool(packet, &["public_projection_eligible"]).unwrap_or(false)
    })
    .count()
}

fn finding_alignment_summary_usize(
    value: &Value,
    source_key: &str,
    scorecard_key: &str,
) -> Option<usize> {
    audit_usize_dynamic(value, &["finding_alignment", "summary"], source_key)
        .or_else(|| audit_usize_dynamic(value, &["summary"], scorecard_key))
}

fn finding_alignment_coverage_usize(value: &Value, key: &str) -> Option<usize> {
    audit_usize_dynamic(value, &["finding_alignment", "coverage"], key)
}

fn finding_alignment_actionable_gap_packet_public_projection_usize(
    value: &Value,
    key: &str,
) -> Option<usize> {
    audit_usize_dynamic(
        value,
        &[
            "finding_alignment",
            "actionable_gap_packet_public_projection",
        ],
        key,
    )
}

fn presentation_text_summary_usize(value: &Value, key: &str) -> Option<usize> {
    audit_usize_dynamic(value, &["finding_alignment", "summary"], key)
        .or_else(|| audit_usize_dynamic(value, &["summary"], key))
}

fn audit_usize_dynamic(value: &Value, path: &[&str], key: &str) -> Option<usize> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.get(key)?.as_u64().map(|count| count as usize)
}

pub(crate) fn finding_alignment_raw_to_canonical_ratio(
    summary: &EvidenceQualityScorecardSummary,
) -> Option<f64> {
    if summary.finding_alignment_canonical_items_total == 0 {
        return None;
    }
    Some(
        summary.finding_alignment_raw_signals_total as f64
            / summary.finding_alignment_canonical_items_total as f64,
    )
}

pub(crate) fn audit_evidence_class_work_queue(
    rows: &[Lane1EvidenceAuditAlignmentClassCoverage],
) -> Vec<Lane1EvidenceClassWorkItem> {
    let mut work_items = rows
        .iter()
        .filter_map(audit_evidence_class_work_item)
        .collect::<Vec<_>>();
    work_items.sort_by(|left, right| {
        right
            .work_score
            .cmp(&left.work_score)
            .then_with(|| right.actionable_items.cmp(&left.actionable_items))
            .then_with(|| {
                right
                    .static_limitation_items
                    .cmp(&left.static_limitation_items)
            })
            .then_with(|| left.evidence_class.cmp(&right.evidence_class))
    });
    work_items.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    work_items
}

fn audit_evidence_class_work_item(
    row: &Lane1EvidenceAuditAlignmentClassCoverage,
) -> Option<Lane1EvidenceClassWorkItem> {
    let duplicate_raw_signals = row.raw_findings.saturating_sub(row.canonical_items);
    let work_score = row.unaligned_raw_findings * 8
        + row.actionable_items * 10
        + row.static_limitation_items * 6
        + row.unknown_items * 6
        + duplicate_raw_signals * 2;
    if work_score == 0 {
        return None;
    }
    let dominant_signal = audit_evidence_class_dominant_signal(row, duplicate_raw_signals);
    let dominant_static_limitation_category = audit_top_count(&row.static_limitation_categories);
    let dominant_static_limitation_repair_route =
        audit_top_count(&row.static_limitation_repair_routes).map(|(label, _count)| label);
    let next_repair = if dominant_signal == "static_limitations" {
        dominant_static_limitation_repair_route
            .clone()
            .unwrap_or_else(|| audit_evidence_class_next_repair(dominant_signal).to_string())
    } else {
        audit_evidence_class_next_repair(dominant_signal).to_string()
    };
    Some(Lane1EvidenceClassWorkItem {
        evidence_class: row.evidence_class.clone(),
        work_score,
        dominant_signal: dominant_signal.to_string(),
        dominant_static_limitation_category: dominant_static_limitation_category
            .as_ref()
            .map(|(category, _count)| category.clone()),
        dominant_static_limitation_category_count: dominant_static_limitation_category
            .as_ref()
            .map(|(_category, count)| *count)
            .unwrap_or(0),
        dominant_static_limitation_repair_route,
        raw_findings: row.raw_findings,
        canonical_items: row.canonical_items,
        duplicate_raw_signals,
        actionable_items: row.actionable_items,
        static_limitation_items: row.static_limitation_items,
        unknown_items: row.unknown_items,
        unaligned_raw_findings: row.unaligned_raw_findings,
        next_repair,
    })
}

fn audit_top_count(counts: &BTreeMap<String, usize>) -> Option<(String, usize)> {
    let mut rows = counts
        .iter()
        .map(|(label, count)| (label.clone(), *count))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.into_iter().next()
}

fn audit_evidence_class_dominant_signal(
    row: &Lane1EvidenceAuditAlignmentClassCoverage,
    duplicate_raw_signals: usize,
) -> &'static str {
    [
        ("unaligned_raw_findings", row.unaligned_raw_findings * 8),
        ("actionable_canonical_gaps", row.actionable_items * 10),
        ("static_limitations", row.static_limitation_items * 6),
        ("unknown_items", row.unknown_items * 6),
        ("duplicate_raw_signals", duplicate_raw_signals * 2),
    ]
    .into_iter()
    .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(left.0)))
    .map(|(signal, _)| signal)
    .unwrap_or("none")
}

fn audit_evidence_class_next_repair(dominant_signal: &str) -> &'static str {
    match dominant_signal {
        "unaligned_raw_findings" => "analysis/finding-alignment-coverage",
        "actionable_canonical_gaps" => "dogfood/actionable-gap-repair-loop",
        "static_limitations" => "analysis/static-limitation-taxonomy",
        "unknown_items" => "analysis/named-limitations",
        "duplicate_raw_signals" => "analysis/canonical-grouping-audit",
        _ => "report/evidence-class-work-queue",
    }
}

fn evidence_quality_maturity_rows(
    summary: &EvidenceQualityScorecardSummary,
    audit: &Value,
) -> Vec<EvidenceQualityMaturityRow> {
    let unknown_oracle_count = scorecard_count_at(
        audit,
        &[
            "oracle_semantics_distribution",
            "oracle_kind_counts",
            "unknown",
        ],
    ) + scorecard_count_at(
        audit,
        &[
            "oracle_semantics_distribution",
            "oracle_strength_counts",
            "unknown",
        ],
    );
    vec![
        scorecard_maturity_row(
            "evidence_record_contract",
            if summary.evidence_records_missing == 0 && summary.evidence_records_total > 0 {
                "fixture_backed"
            } else {
                "static_only"
            },
            "RIPR-SPEC-0021, evidence-record fixture corpus, Lane 1 audit",
            if summary.evidence_records_missing == 0 {
                "All audited seams carried evidence_record in the current artifact."
            } else {
                "Some audited seams are missing evidence_record."
            },
            "report/evidence-quality-scorecard",
        ),
        scorecard_maturity_row(
            "canonical_gap_identity",
            if summary.canonical_gap_groups_total > 0 {
                "fixture_backed"
            } else {
                "static_only"
            },
            "RIPR-SPEC-0033, canonical gap unit tests, Lane 1 audit groups",
            if summary.duplicate_looking_groups_total > 0 {
                "Duplicate-looking groups still need audit-driven review before another grouping change."
            } else {
                "Current audit reports no duplicate-looking canonical groups."
            },
            "fixtures/evidence-quality-benchmark-corpus",
        ),
        scorecard_maturity_row(
            "related_test_ranking",
            if summary.low_or_opaque_top_related_tests > 0 {
                "static_only"
            } else if summary.related_tests_total > 0 {
                "fixture_backed"
            } else {
                "uncalibrated"
            },
            "RIPR-SPEC-0029, Lane 1 audit related-test confidence distribution",
            if summary.low_or_opaque_top_related_tests > 0 {
                "Top related-test choices include low-confidence or opaque rankings."
            } else {
                "Current audit has no low-confidence top related-test signal."
            },
            "analysis/related-test-ranking-audit-fixes",
        ),
        scorecard_maturity_row(
            "oracle_semantics",
            if unknown_oracle_count > 0 {
                "static_only"
            } else {
                "fixture_backed"
            },
            "RIPR-SPEC-0030, oracle-semantics fixture scope, Lane 1 audit distribution",
            if unknown_oracle_count > 0 {
                "Unknown oracle kinds or strengths remain in the current audit."
            } else {
                "Current audit has no unknown oracle kind or strength buckets."
            },
            "analysis/oracle-semantics-audit-fixes",
        ),
        scorecard_maturity_row(
            "movement_identity",
            if scorecard_count_at(
                audit,
                &["movement_availability", "records_with_canonical_gap_id"],
            ) > 0
            {
                "fixture_backed"
            } else {
                "static_only"
            },
            "targeted-test outcome, assistant proof, baseline/ledger/gate identity consumers",
            "Movement identity is static evidence; it does not imply runtime calibration.",
            "report/evidence-quality-trend",
        ),
        scorecard_maturity_row(
            "runtime_calibration",
            if summary.calibrated_records > 0 {
                "imported_runtime_calibrated"
            } else {
                "uncalibrated"
            },
            "runtime-fixtures-v2 imported calibration labels when present",
            if summary.calibrated_records > 0 {
                "Calibration remains class-scoped to imported checked fixture outcomes."
            } else {
                "Current audit has no imported runtime calibration records."
            },
            "calibration/runtime-fixtures-v3",
        ),
        scorecard_maturity_row(
            "static_limitation_taxonomy",
            if summary.static_limitations_total > 0 {
                "static_only"
            } else {
                "fixture_backed"
            },
            "Lane 1 audit static limitation reason and stage distributions",
            if summary.static_limitations_total > 0 {
                "Static limitations remain analyzer limits, not user test gaps."
            } else {
                "Current audit reports no static limitations."
            },
            "analysis/static-limitation-taxonomy",
        ),
    ]
}

fn scorecard_maturity_row(
    class: &str,
    status: &str,
    proof_source: &str,
    known_limits: &str,
    recommended_next_repair: &str,
) -> EvidenceQualityMaturityRow {
    EvidenceQualityMaturityRow {
        class: class.to_string(),
        status: status.to_string(),
        proof_source: proof_source.to_string(),
        known_limits: known_limits.to_string(),
        recommended_next_repair: recommended_next_repair.to_string(),
    }
}

fn evidence_quality_recommended_repairs(
    summary: &EvidenceQualityScorecardSummary,
    audit: &Value,
) -> Vec<EvidenceQualityRepair> {
    let mut repairs = Vec::new();
    scorecard_push_work_queue_repairs(&mut repairs, audit);
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "analysis/related-test-ranking-audit-fixes",
            priority: 100,
            evidence_class: "related_test_ranking",
            risk_kind: "low_or_opaque_top_related_tests",
            signal_count: summary.low_or_opaque_top_related_tests,
            why: "Top related-test choices include low-confidence or opaque evidence.",
            expected_impact: "Improve first-useful-action task quality and agent packet reliability without changing gate behavior.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "analysis/predicate-boundary-repair-routes",
            priority: 95,
            evidence_class: "predicate_boundary",
            risk_kind: "finding_alignment_canonical_items_without_repair_route",
            signal_count: summary.finding_alignment_canonical_items_without_repair_route,
            why: "Actionable canonical gaps must carry concrete repair routes before humans or agents can safely treat them as work.",
            expected_impact: "Close the actionability loop for predicate-boundary gaps by naming the assertion repair route.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "analysis/static-limitation-taxonomy",
            priority: 90,
            evidence_class: "static_limitations",
            risk_kind: "static_limitations_total",
            signal_count: summary.static_limitations_total,
            why: "Static limitations need repairable categories before analyzer confidence can move.",
            expected_impact: "Separate analyzer limits from user test gaps and expose next repair routes.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "analysis/oracle-semantics-audit-fixes",
            priority: 85,
            evidence_class: "oracle_semantics",
            risk_kind: "unknown_oracle_semantics",
            signal_count: scorecard_count_at(
                audit,
                &[
                    "oracle_semantics_distribution",
                    "oracle_kind_counts",
                    "unknown",
                ],
            ) + scorecard_count_at(
                audit,
                &[
                    "oracle_semantics_distribution",
                    "oracle_strength_counts",
                    "unknown",
                ],
            ),
            why: "Opaque oracle semantics block stronger evidence claims.",
            expected_impact: "Make what the oracle observes and misses explicit for supported shapes.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "calibration/runtime-fixtures-v3",
            priority: 80,
            evidence_class: "runtime_calibration",
            risk_kind: "uncalibrated_records",
            signal_count: summary.uncalibrated_records,
            why: "Records without imported runtime outcomes cannot make calibrated claims.",
            expected_impact: "Expand class-scoped calibration only where checked runtime fixtures support it.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "fixtures/evidence-quality-benchmark-corpus",
            priority: 70,
            evidence_class: "canonical_gap_identity",
            risk_kind: "duplicate_looking_groups_total",
            signal_count: summary.duplicate_looking_groups_total,
            why: "Duplicate-looking canonical groups should be fixture-pinned before another identity refinement.",
            expected_impact: "Prevent raw count chasing and preserve must-not-claim guards for grouping changes.",
        },
    );
    scorecard_push_repair(
        &mut repairs,
        ScorecardRepairSpec {
            slice: "fixtures/evidence-quality-benchmark-corpus",
            priority: 65,
            evidence_class: "missing_discriminators",
            risk_kind: "missing_discriminators_total",
            signal_count: summary.missing_discriminators_total,
            why: "Missing discriminator classes need positive and negative fixtures before heuristic expansion.",
            expected_impact: "Keep analyzer changes audit-driven and fixture-first.",
        },
    );
    repairs.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| right.signal_count.cmp(&left.signal_count))
            .then_with(|| left.slice.cmp(&right.slice))
    });
    repairs
}

fn scorecard_push_work_queue_repairs(repairs: &mut Vec<EvidenceQualityRepair>, audit: &Value) {
    let Some(rows) = audit_get(
        audit,
        &["finding_alignment", "coverage", "evidence_class_work_queue"],
    )
    .and_then(Value::as_array) else {
        return;
    };

    for (idx, row) in rows
        .iter()
        .take(EVIDENCE_QUALITY_SCORECARD_WORK_QUEUE_REPAIR_LIMIT)
        .enumerate()
    {
        let Some(evidence_class) = row.get("evidence_class").and_then(Value::as_str) else {
            continue;
        };
        let Some(dominant_signal) = row.get("dominant_signal").and_then(Value::as_str) else {
            continue;
        };
        let Some(slice) = row.get("next_repair").and_then(Value::as_str) else {
            continue;
        };
        if slice.is_empty() {
            continue;
        }
        let signal_count = scorecard_work_queue_signal_count(row, dominant_signal);
        if signal_count == 0 {
            continue;
        }
        let (risk_kind, why, expected_impact) =
            scorecard_work_queue_repair_text(row, evidence_class, dominant_signal);
        scorecard_push_repair(
            repairs,
            ScorecardRepairSpec {
                slice,
                priority: 120usize.saturating_sub(idx),
                evidence_class,
                risk_kind: &risk_kind,
                signal_count,
                why: &why,
                expected_impact: &expected_impact,
            },
        );
    }
}

fn scorecard_work_queue_signal_count(row: &Value, dominant_signal: &str) -> usize {
    match dominant_signal {
        "unaligned_raw_findings" => scorecard_row_count(row, "unaligned_raw_findings"),
        "actionable_canonical_gaps" => scorecard_row_count(row, "actionable_items"),
        "static_limitations" => {
            let category_count =
                scorecard_row_count(row, "dominant_static_limitation_category_count");
            if category_count > 0 {
                category_count
            } else {
                scorecard_row_count(row, "static_limitation_items")
            }
        }
        "unknown_items" => scorecard_row_count(row, "unknown_items"),
        "duplicate_raw_signals" => scorecard_row_count(row, "duplicate_raw_signals"),
        _ => scorecard_row_count(row, "work_score"),
    }
}

fn scorecard_row_count(row: &Value, key: &str) -> usize {
    row.get(key)
        .and_then(Value::as_u64)
        .map(|count| count as usize)
        .unwrap_or(0)
}

fn scorecard_work_queue_repair_text(
    row: &Value,
    evidence_class: &str,
    dominant_signal: &str,
) -> (String, String, String) {
    if dominant_signal == "static_limitations" {
        let category = row
            .get("dominant_static_limitation_category")
            .and_then(Value::as_str)
            .unwrap_or("static_limitation");
        return (
            format!("static_limitations:{category}"),
            format!(
                "Audit work queue ranks `{evidence_class}` because `{category}` dominates its named static limitations."
            ),
            "Use the named repair route fixture-first; do not count the limitation as user test debt until the analyzer support is proven." // ripr-allow: static-language: verbatim move from allowlisted main.rs (#2119 slice 6); rewording this output string is a separate output-contract change
                .to_string(),
        );
    }

    (
        dominant_signal.to_string(),
        format!(
            "Audit work queue ranks `{evidence_class}` because `{dominant_signal}` is the dominant live signal."
        ),
        "Use the audit-derived class row to choose the next fixture-backed analyzer or counting repair."
            .to_string(),
    )
}

struct ScorecardRepairSpec<'a> {
    slice: &'a str,
    priority: usize,
    evidence_class: &'a str,
    risk_kind: &'a str,
    signal_count: usize,
    why: &'a str,
    expected_impact: &'a str,
}

fn scorecard_push_repair(repairs: &mut Vec<EvidenceQualityRepair>, spec: ScorecardRepairSpec<'_>) {
    if spec.signal_count == 0 {
        return;
    }
    repairs.push(EvidenceQualityRepair {
        slice: spec.slice.to_string(),
        priority: spec.priority,
        evidence_class: spec.evidence_class.to_string(),
        risk_kind: spec.risk_kind.to_string(),
        signal_count: spec.signal_count,
        why: spec.why.to_string(),
        expected_impact: spec.expected_impact.to_string(),
    });
}

fn evidence_quality_recent_deltas(
    current: &EvidenceQualityScorecardSummary,
    previous_scorecard: Option<&Value>,
    previous_input: &EvidenceQualityScorecardInput,
) -> EvidenceQualityDeltas {
    let Some(previous) = previous_scorecard else {
        return EvidenceQualityDeltas {
            available: false,
            source: None,
            reason: Some("no previous scorecard artifact was available".to_string()),
            deltas: Vec::new(),
        };
    };
    let mut deltas = Vec::new();
    for (metric, after) in [
        (
            "duplicate_looking_groups_total",
            current.duplicate_looking_groups_total,
        ),
        (
            "missing_discriminators_total",
            current.missing_discriminators_total,
        ),
        ("static_limitations_total", current.static_limitations_total),
        (
            "low_or_opaque_top_related_tests",
            current.low_or_opaque_top_related_tests,
        ),
        ("uncalibrated_records", current.uncalibrated_records),
        ("calibrated_records", current.calibrated_records),
        (
            "finding_alignment_duplicate_groups_total",
            current.finding_alignment_duplicate_groups_total,
        ),
        (
            "finding_alignment_actionable_items_total",
            current.finding_alignment_actionable_items_total,
        ),
        (
            "finding_alignment_canonical_items_without_repair_route",
            current.finding_alignment_canonical_items_without_repair_route,
        ),
        (
            "finding_alignment_canonical_items_without_verify_command",
            current.finding_alignment_canonical_items_without_verify_command,
        ),
        (
            "finding_alignment_static_unknown_without_named_limitation",
            current.finding_alignment_static_unknown_without_named_limitation,
        ),
        (
            "finding_alignment_static_limitation_total",
            current.finding_alignment_static_limitation_total,
        ),
        (
            "finding_alignment_calibrated_supported_total",
            current.finding_alignment_calibrated_supported_total,
        ),
        (
            "finding_alignment_uncalibrated_total",
            current.finding_alignment_uncalibrated_total,
        ),
        (
            "presentation_text_visibility_unknown",
            current.presentation_text_visibility_unknown,
        ),
        (
            "presentation_text_actionable_snapshot",
            current.presentation_text_actionable_snapshot,
        ),
        (
            "presentation_text_static_limitations",
            current.presentation_text_static_limitations,
        ),
    ] {
        let Some(before) = audit_usize(previous, &["summary", metric]) else {
            continue;
        };
        deltas.push(EvidenceQualityDelta {
            metric: metric.to_string(),
            before,
            after,
            delta: after as isize - before as isize,
            direction: scorecard_delta_direction(metric, before, after),
        });
    }
    if deltas.is_empty() {
        EvidenceQualityDeltas {
            available: false,
            source: Some(previous_input.path.clone()),
            reason: Some(
                "previous scorecard did not contain comparable summary metrics".to_string(),
            ),
            deltas,
        }
    } else {
        EvidenceQualityDeltas {
            available: true,
            source: Some(previous_input.path.clone()),
            reason: None,
            deltas,
        }
    }
}

fn scorecard_delta_direction(metric: &str, before: usize, after: usize) -> String {
    if before == after {
        return "unchanged".to_string();
    }
    let improved = if metric == "calibrated_records"
        || metric == "finding_alignment_calibrated_supported_total"
    {
        after > before
    } else {
        after < before
    };
    if improved {
        "improved".to_string()
    } else {
        "worse".to_string()
    }
}

fn evidence_quality_unknowns(
    summary: &EvidenceQualityScorecardSummary,
    audit: &Value,
    evidence_health: Option<&Value>,
    inputs: &EvidenceQualityScorecardInputs,
) -> Vec<EvidenceQualityUnknown> {
    let mut unknowns = Vec::new();
    if lane1_audit_has_completeness_limitation(audit) {
        scorecard_push_unknown(
            &mut unknowns,
            "lane1_evidence_audit_limited",
            "Lane 1 evidence audit reported a bounded run limitation, so zero or partial counts from that artifact must not be treated as complete repo truth.",
            Some("report/lane1-audit-bounded-diagnostics"),
        );
    }
    if report_has_run_limitation_category(
        audit,
        EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED,
    ) {
        scorecard_push_unknown(
            &mut unknowns,
            EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED,
            "Evidence-quality scorecard could not regenerate the required Lane 1 audit, so this scorecard is a bounded diagnostic instead of complete repo truth.",
            Some("report/evidence-quality-scorecard-bounded-diagnostics"),
        );
    }
    if evidence_health.is_none() {
        scorecard_push_unknown(
            &mut unknowns,
            "evidence_health_unavailable",
            "Evidence-health JSON was not available, so durable health-only audit fields are not joined.",
            Some("report/evidence-health-audit-fields"),
        );
    } else if evidence_health.is_some_and(report_has_run_limitations) {
        scorecard_push_unknown(
            &mut unknowns,
            "evidence_health_limited",
            "Evidence-health JSON reported a bounded run limitation, so health-only fields are diagnostic rather than complete repo truth.",
            Some("report/evidence-health-bounded-diagnostics"),
        );
    }
    if inputs.previous_scorecard.status == "missing" {
        scorecard_push_unknown(
            &mut unknowns,
            "recent_delta_unavailable",
            "No previous scorecard artifact was available for before/after delta reporting.",
            Some("report/evidence-quality-trend"),
        );
    }
    if summary.uncalibrated_records > 0 {
        scorecard_push_unknown(
            &mut unknowns,
            "runtime_calibration_missing",
            "Some evidence records do not have imported runtime calibration data.",
            Some("calibration/runtime-fixtures-v3"),
        );
    }
    if summary.static_limitations_total > 0 {
        scorecard_push_unknown(
            &mut unknowns,
            "static_limitations_present",
            "Static limitations remain analyzer limits and should not be treated as user test gaps.",
            Some("analysis/static-limitation-taxonomy"),
        );
    }
    if summary.low_or_opaque_top_related_tests > 0 {
        scorecard_push_unknown(
            &mut unknowns,
            "related_test_low_confidence",
            "Some canonical groups have low-confidence or opaque top related-test choices.",
            Some("analysis/related-test-ranking-audit-fixes"),
        );
    }
    let unknown_oracle = scorecard_count_at(
        audit,
        &[
            "oracle_semantics_distribution",
            "oracle_kind_counts",
            "unknown",
        ],
    ) + scorecard_count_at(
        audit,
        &[
            "oracle_semantics_distribution",
            "oracle_strength_counts",
            "unknown",
        ],
    );
    if unknown_oracle > 0 {
        scorecard_push_unknown(
            &mut unknowns,
            "oracle_semantics_opaque",
            "Unknown oracle kind or strength buckets remain in the current audit.",
            Some("analysis/oracle-semantics-audit-fixes"),
        );
    }
    if summary.raw_headline_gaps > 0 && summary.canonical_gap_groups_total == 0 {
        scorecard_push_unknown(
            &mut unknowns,
            "canonical_gap_identity_missing",
            "Headline gaps exist without canonical group identity in the scorecard input.",
            Some("fixtures/evidence-quality-benchmark-corpus"),
        );
    }
    if !finding_alignment_summary_available(audit) {
        scorecard_push_unknown(
            &mut unknowns,
            "finding_alignment_unavailable",
            "No finding_alignment summary was available in the scorecard input, so raw-to-canonical and presentation-text counts are reported as zero instead of inferred from raw gaps.",
            Some("report/presentation-text-scorecard-trend-fields"),
        );
    }
    unknowns
}

pub(crate) fn report_has_run_limitations(value: &Value) -> bool {
    value
        .get("run_limitations")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn lane1_audit_has_completeness_limitation(value: &Value) -> bool {
    [
        "lane1_repo_exposure_timeout",
        "lane1_repo_exposure_incomplete",
        "lane1_repo_exposure_runner_error",
        "lane1_repo_exposure_sampled",
        "lane1_repo_exposure_large_cache_preflight_skip",
        EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED,
    ]
    .iter()
    .any(|category| report_has_run_limitation_category(value, category))
}

fn report_has_run_limitation_category(value: &Value, category: &str) -> bool {
    value
        .get("run_limitations")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("category").and_then(Value::as_str) == Some(category))
        })
}

fn finding_alignment_summary_available(value: &Value) -> bool {
    audit_get(value, &["finding_alignment", "summary"]).is_some()
        || audit_get(value, &["summary"])
            .and_then(|summary| summary.get("finding_alignment_raw_signals_total"))
            .is_some()
}

fn scorecard_push_unknown(
    unknowns: &mut Vec<EvidenceQualityUnknown>,
    kind: &str,
    summary: &str,
    next_repair: Option<&str>,
) {
    unknowns.push(EvidenceQualityUnknown {
        kind: kind.to_string(),
        summary: summary.to_string(),
        next_repair: next_repair.map(str::to_string),
    });
}

fn evidence_quality_calibration_coverage(
    summary: &EvidenceQualityScorecardSummary,
    audit: &Value,
) -> Value {
    let runtime_scope = if summary.calibrated_records > 0 {
        "imported_runtime_calibrated"
    } else {
        "uncalibrated"
    };
    serde_json::json!({
        "availability_counts": scorecard_value_or_default(
            audit,
            &["calibration_availability", "availability_counts"],
            serde_json::json!({}),
        ),
        "confidence_counts": scorecard_value_or_default(
            audit,
            &["calibration_availability", "confidence_counts"],
            serde_json::json!({}),
        ),
        "agreement_counts": scorecard_value_or_default(
            audit,
            &["calibration_availability", "agreement_counts"],
            serde_json::json!({}),
        ),
        "by_evidence_class": scorecard_value_or_default(
            audit,
            &["calibration_availability", "runtime_confidence_by_class"],
            scorecard_value_or_default(
                audit,
                &["finding_alignment", "runtime_confidence_by_class"],
                serde_json::json!([]),
            ),
        ),
        "calibrated_records": summary.calibrated_records,
        "uncalibrated_records": summary.uncalibrated_records,
        "runtime_scope": runtime_scope,
    })
}

fn scorecard_value_or_default(value: &Value, path: &[&str], default: Value) -> Value {
    audit_get(value, path).cloned().unwrap_or(default)
}

fn scorecard_count_at(value: &Value, path: &[&str]) -> usize {
    audit_usize(value, path).unwrap_or(0)
}

pub(crate) fn evidence_quality_scorecard_json(
    report: &EvidenceQualityScorecardReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": EVIDENCE_QUALITY_SCORECARD_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "evidence-quality-scorecard",
        "generated_at": report.generated_at,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "scope": {
            "kind": "repo",
            "root": report.root,
        },
        "inputs": {
            "lane1_evidence_audit": scorecard_input_json(&report.inputs.lane1_evidence_audit),
            "evidence_health": scorecard_input_json(&report.inputs.evidence_health),
            "previous_scorecard": scorecard_input_json(&report.inputs.previous_scorecard),
            "capability_matrix": scorecard_input_json(&report.inputs.capability_matrix),
            "capabilities": scorecard_input_json(&report.inputs.capabilities),
            "traceability": scorecard_input_json(&report.inputs.traceability),
        },
        "headline": evidence_quality_scorecard_headline_json(&report.summary),
        "summary": evidence_quality_scorecard_summary_json(&report.summary),
        "maturity_by_class": report.maturity_by_class.iter().map(|row| {
            serde_json::json!({
                "class": row.class,
                "status": row.status,
                "proof_source": row.proof_source,
                "known_limits": row.known_limits,
                "recommended_next_repair": row.recommended_next_repair,
            })
        }).collect::<Vec<_>>(),
        "canonical_gap_groups": report.canonical_gap_groups,
        "duplicate_looking_groups": report.duplicate_looking_groups,
        "static_limitation_categories": report.static_limitation_categories,
        "missing_discriminator_classes": report.missing_discriminator_classes,
        "related_test_confidence": report.related_test_confidence,
        "oracle_semantics_distribution": report.oracle_semantics_distribution,
        "movement_availability": report.movement_availability,
        "calibration_coverage": report.calibration_coverage,
        "actionable_gap_top_lists": report.actionable_gap_top_lists,
        "actionable_gap_packet_public_projection": report.actionable_gap_packet_public_projection,
        "evidence_class_work_queue": report.evidence_class_work_queue,
        "language_aware_placement_route_quality": report.language_aware_placement_route_quality,
        "cross_language_oracle_route_quality": report.cross_language_oracle_route_quality,
        "recommended_repairs": report.recommended_repairs.iter().map(|repair| {
            serde_json::json!({
                "slice": repair.slice,
                "priority": repair.priority,
                "evidence_class": repair.evidence_class,
                "risk_kind": repair.risk_kind,
                "signal_count": repair.signal_count,
                "why": repair.why,
                "expected_impact": repair.expected_impact,
            })
        }).collect::<Vec<_>>(),
        "recent_audit_deltas": {
            "available": report.recent_audit_deltas.available,
            "source": report.recent_audit_deltas.source,
            "reason": report.recent_audit_deltas.reason,
            "deltas": report.recent_audit_deltas.deltas.iter().map(|delta| {
                serde_json::json!({
                    "metric": delta.metric,
                    "before": delta.before,
                    "after": delta.after,
                    "delta": delta.delta,
                    "direction": delta.direction,
                })
            }).collect::<Vec<_>>(),
        },
        "unknowns": report.unknowns.iter().map(|unknown| {
            serde_json::json!({
                "kind": unknown.kind,
                "summary": unknown.summary,
                "next_repair": unknown.next_repair,
            })
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render evidence quality scorecard JSON: {err}"))
}

fn evidence_quality_scorecard_headline_json(summary: &EvidenceQualityScorecardSummary) -> Value {
    serde_json::json!({
        "primary_metric": "finding_alignment_actionable_unresolved_canonical_gaps",
        "primary_count": summary.finding_alignment_actionable_unresolved_canonical_gaps,
        "counting_model": "actionable_canonical_gaps",
        "raw_signals": summary.finding_alignment_raw_signals_total,
        "canonical_items": summary.finding_alignment_canonical_items_total,
        "already_observed": summary.finding_alignment_already_observed_total,
        "internal_no_action": summary.finding_alignment_internal_no_action_total,
        "static_limitations": summary.static_limitations_total,
        "unknown": summary.finding_alignment_unknown_total,
        "raw_to_canonical_ratio": finding_alignment_raw_to_canonical_ratio(summary),
        "note": "Raw findings are diagnostic; actionable canonical gaps are the user-facing repair count."
    })
}

fn evidence_quality_scorecard_summary_json(summary: &EvidenceQualityScorecardSummary) -> Value {
    let mut object = serde_json::Map::new();
    scorecard_summary_insert_usize(&mut object, "raw_headline_gaps", summary.raw_headline_gaps);
    scorecard_summary_insert_usize(
        &mut object,
        "canonical_gap_groups_total",
        summary.canonical_gap_groups_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "duplicate_looking_groups_total",
        summary.duplicate_looking_groups_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "missing_discriminators_total",
        summary.missing_discriminators_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "static_limitations_total",
        summary.static_limitations_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "related_tests_total",
        summary.related_tests_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "low_or_opaque_top_related_tests",
        summary.low_or_opaque_top_related_tests,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "calibrated_records",
        summary.calibrated_records,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "uncalibrated_records",
        summary.uncalibrated_records,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "evidence_records_total",
        summary.evidence_records_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "evidence_records_missing",
        summary.evidence_records_missing,
    );
    scorecard_summary_insert_usize(&mut object, "top_repair_count", summary.top_repair_count);
    object.insert(
        "recent_delta_available".to_string(),
        serde_json::json!(summary.recent_delta_available),
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_raw_findings_total",
        summary.finding_alignment_raw_findings_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_raw_signals_total",
        summary.finding_alignment_raw_signals_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_canonical_items_total",
        summary.finding_alignment_canonical_items_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_aligned_raw_findings_total",
        summary.finding_alignment_aligned_raw_findings_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_unaligned_raw_findings_total",
        summary.finding_alignment_unaligned_raw_findings_total,
    );
    object.insert(
        "finding_alignment_raw_to_canonical_ratio".to_string(),
        finding_alignment_raw_to_canonical_ratio(summary)
            .map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_duplicate_groups_total",
        summary.finding_alignment_duplicate_groups_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_actionable_items_total",
        summary.finding_alignment_actionable_items_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_actionable_unresolved_canonical_gaps",
        summary.finding_alignment_actionable_unresolved_canonical_gaps,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_already_observed_total",
        summary.finding_alignment_already_observed_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_internal_only_total",
        summary.finding_alignment_internal_only_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_internal_no_action_total",
        summary.finding_alignment_internal_no_action_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_static_limitation_total",
        summary.finding_alignment_static_limitation_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_unknown_total",
        summary.finding_alignment_unknown_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_calibrated_supported_total",
        summary.finding_alignment_calibrated_supported_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_uncalibrated_total",
        summary.finding_alignment_uncalibrated_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_visibility_unknown_total",
        summary.finding_alignment_visibility_unknown_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_presentation_text_actionable_total",
        summary.finding_alignment_presentation_text_actionable_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_static_unknown_without_named_limitation",
        summary.finding_alignment_static_unknown_without_named_limitation,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_canonical_items_without_repair_route",
        summary.finding_alignment_canonical_items_without_repair_route,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_canonical_items_without_verify_command",
        summary.finding_alignment_canonical_items_without_verify_command,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_actionable_gap_packet_public_projection_eligible_packets",
        summary.finding_alignment_actionable_gap_packet_public_projection_eligible_packets,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "finding_alignment_actionable_gap_packet_public_projection_excluded_packets",
        summary.finding_alignment_actionable_gap_packet_public_projection_excluded_packets,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_total",
        summary.presentation_text_total,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_user_visible",
        summary.presentation_text_user_visible,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_observed",
        summary.presentation_text_observed,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_unobserved",
        summary.presentation_text_unobserved,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_internal_only",
        summary.presentation_text_internal_only,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_visibility_unknown",
        summary.presentation_text_visibility_unknown,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_observer_unknown",
        summary.presentation_text_observer_unknown,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_duplicate_groups",
        summary.presentation_text_duplicate_groups,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_actionable_snapshot",
        summary.presentation_text_actionable_snapshot,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_no_action",
        summary.presentation_text_no_action,
    );
    scorecard_summary_insert_usize(
        &mut object,
        "presentation_text_static_limitations",
        summary.presentation_text_static_limitations,
    );
    Value::Object(object)
}

fn scorecard_summary_insert_usize(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: usize,
) {
    object.insert(key.to_string(), serde_json::json!(value));
}

fn scorecard_input_json(input: &EvidenceQualityScorecardInput) -> Value {
    serde_json::json!({
        "path": input.path,
        "status": input.status,
        "schema_version": input.schema_version,
        "sha256": input.sha256,
        "note": input.note,
    })
}

fn scorecard_push_top_count_table(out: &mut String, heading: &str, value: &Value, key: &str) {
    let rows = value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if rows.is_empty() {
        out.push_str(&format!(
            "No {} counts were reported.\n\n",
            heading.to_lowercase()
        ));
        return;
    }
    out.push_str(&format!("| {heading} | Count |\n"));
    out.push_str("| --- | ---: |\n");
    for row in rows {
        let label = audit_string(row, &["label"]).unwrap_or_else(|| "unknown".to_string());
        let count = audit_usize(row, &["count"]).unwrap_or(0);
        out.push_str(&format!(
            "| {} | {} |\n",
            audit_markdown_cell(&label),
            count
        ));
    }
    out.push('\n');
}

fn scorecard_push_language_aware_placement_route_quality(out: &mut String, value: &Value) {
    out.push_str("## Language-Aware Placement Route Quality\n\n");
    out.push_str("This section summarizes cross-language placement and navigation-only target evidence as analyzer route-quality context. It does not create public repair packets, verify commands, receipt commands, or allowed edit surfaces.\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        out,
        "Cross-language target unresolved signals",
        audit_usize(value, &["cross_language_target_unresolved_signals"]).unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Cross-language target-inference route signals",
        audit_usize(
            value,
            &["cross_language_test_target_inference_route_signals"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Cross-language oracle visibility unresolved signals",
        audit_usize(
            value,
            &["cross_language_oracle_visibility_unresolved_signals"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Navigation-only external target packets",
        audit_usize(value, &["navigation_only_external_target_packets"]).unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Navigation-only public promotions",
        audit_usize(
            value,
            &["navigation_only_external_target_public_promotions"],
        )
        .unwrap_or_default(),
    );
    out.push('\n');
    out.push_str(&format!(
        "- status: `{}`\n",
        audit_markdown_cell(
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- repair_route: `{}`\n",
        audit_markdown_cell(
            value
                .get("repair_route")
                .and_then(Value::as_str)
                .unwrap_or(CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE)
        )
    ));
    out.push_str(&format!(
        "- navigation_only_external_target_status: `{}`\n",
        audit_markdown_cell(
            value
                .get("navigation_only_external_target_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str("- calibration_boundary: static placement limitations and navigation-only targets are not runtime calibration or public repair packets.\n\n");
}

fn scorecard_push_evidence_class_work_queue_table(out: &mut String, value: &Value) {
    let rows = value.as_array().map_or(&[][..], Vec::as_slice);
    if rows.is_empty() {
        out.push_str("No evidence-class work queue rows were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | Work score | Dominant signal | Static category | Static route | Actionable | Limitations | Unknown | Unaligned | Duplicate raw | Next repair |\n");
    out.push_str("| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for row in rows.iter().take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT) {
        let evidence_class =
            audit_string(row, &["evidence_class"]).unwrap_or_else(|| "unknown".to_string());
        let dominant_signal =
            audit_string(row, &["dominant_signal"]).unwrap_or_else(|| "unknown".to_string());
        let static_category = audit_string(row, &["dominant_static_limitation_category"])
            .unwrap_or_else(|| "n/a".to_string());
        let static_route = audit_string(row, &["dominant_static_limitation_repair_route"])
            .unwrap_or_else(|| "n/a".to_string());
        let next_repair =
            audit_string(row, &["next_repair"]).unwrap_or_else(|| "unknown".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&evidence_class),
            audit_usize(row, &["work_score"]).unwrap_or(0),
            audit_markdown_cell(&dominant_signal),
            audit_markdown_cell(&static_category),
            audit_markdown_cell(&static_route),
            audit_usize(row, &["actionable_items"]).unwrap_or(0),
            audit_usize(row, &["static_limitation_items"]).unwrap_or(0),
            audit_usize(row, &["unknown_items"]).unwrap_or(0),
            audit_usize(row, &["unaligned_raw_findings"]).unwrap_or(0),
            audit_usize(row, &["duplicate_raw_signals"]).unwrap_or(0),
            audit_markdown_cell(&next_repair),
        ));
    }
    out.push('\n');
}

fn scorecard_push_runtime_confidence_by_class_table(out: &mut String, value: &Value) {
    let rows = value
        .get("by_evidence_class")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if rows.is_empty() {
        out.push_str("No runtime confidence by-class rows were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | Canonical | Calibrated supported | Fixture-backed | Static-only | Unknown confidence | Uncalibrated | Actionable | Limitations |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows.iter().take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(
                &audit_string(row, &["evidence_class"]).unwrap_or_else(|| "unknown".to_string())
            ),
            audit_usize(row, &["canonical_items"]).unwrap_or(0),
            audit_usize(row, &["calibrated_supported"]).unwrap_or(0),
            audit_usize(row, &["fixture_backed"]).unwrap_or(0),
            audit_usize(row, &["static_only"]).unwrap_or(0),
            audit_usize(row, &["unknown_confidence"]).unwrap_or(0),
            audit_usize(row, &["uncalibrated"]).unwrap_or(0),
            audit_usize(row, &["actionable_items"]).unwrap_or(0),
            audit_usize(row, &["static_limitation_items"]).unwrap_or(0),
        ));
    }
    out.push('\n');
}

pub(crate) fn evidence_quality_scorecard_markdown(
    report: &EvidenceQualityScorecardReport,
) -> String {
    let mut out = String::new();
    out.push_str("# Lane 1 evidence quality scorecard\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str("This repo-local scorecard summarizes Lane 1 evidence quality from existing evidence artifacts. It does not change analyzer behavior, gate policy, PR or CI projection, editor output, source files, generated tests, provider calls, or runtime execution.\n\n");

    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Actionable canonical gaps",
        report
            .summary
            .finding_alignment_actionable_unresolved_canonical_gaps,
    );
    audit_push_count(
        &mut out,
        "Canonical evidence items",
        report.summary.finding_alignment_canonical_items_total,
    );
    audit_push_count(
        &mut out,
        "Raw alignment signals",
        report.summary.finding_alignment_raw_signals_total,
    );
    audit_push_count(
        &mut out,
        "Raw headline gaps",
        report.summary.raw_headline_gaps,
    );
    audit_push_count(
        &mut out,
        "Canonical gap groups",
        report.summary.canonical_gap_groups_total,
    );
    audit_push_count(
        &mut out,
        "Duplicate-looking groups",
        report.summary.duplicate_looking_groups_total,
    );
    audit_push_count(
        &mut out,
        "Missing discriminators",
        report.summary.missing_discriminators_total,
    );
    audit_push_count(
        &mut out,
        "Static limitations",
        report.summary.static_limitations_total,
    );
    audit_push_count(
        &mut out,
        "Low or opaque top related tests",
        report.summary.low_or_opaque_top_related_tests,
    );
    audit_push_count(
        &mut out,
        "Uncalibrated records",
        report.summary.uncalibrated_records,
    );
    out.push('\n');

    out.push_str("## Finding Alignment And Presentation Text\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Raw alignment signals",
        report.summary.finding_alignment_raw_signals_total,
    );
    audit_push_count(
        &mut out,
        "Canonical alignment items",
        report.summary.finding_alignment_canonical_items_total,
    );
    audit_push_count(
        &mut out,
        "Aligned raw findings",
        report.summary.finding_alignment_aligned_raw_findings_total,
    );
    audit_push_count(
        &mut out,
        "Unaligned raw findings",
        report
            .summary
            .finding_alignment_unaligned_raw_findings_total,
    );
    audit_push_count(
        &mut out,
        "Alignment duplicate groups",
        report.summary.finding_alignment_duplicate_groups_total,
    );
    audit_push_count(
        &mut out,
        "Actionable canonical items",
        report.summary.finding_alignment_actionable_items_total,
    );
    audit_push_count(
        &mut out,
        "Already observed items",
        report.summary.finding_alignment_already_observed_total,
    );
    audit_push_count(
        &mut out,
        "Internal no-action items",
        report.summary.finding_alignment_internal_no_action_total,
    );
    audit_push_count(
        &mut out,
        "Alignment static limitations",
        report.summary.finding_alignment_static_limitation_total,
    );
    audit_push_count(
        &mut out,
        "Alignment calibrated-supported items",
        report.summary.finding_alignment_calibrated_supported_total,
    );
    audit_push_count(
        &mut out,
        "Alignment uncalibrated items",
        report.summary.finding_alignment_uncalibrated_total,
    );
    audit_push_count(
        &mut out,
        "Static unknown without named limitation",
        report
            .summary
            .finding_alignment_static_unknown_without_named_limitation,
    );
    audit_push_count(
        &mut out,
        "Canonical items without repair route",
        report
            .summary
            .finding_alignment_canonical_items_without_repair_route,
    );
    audit_push_count(
        &mut out,
        "Canonical items without verify command",
        report
            .summary
            .finding_alignment_canonical_items_without_verify_command,
    );
    audit_push_count(
        &mut out,
        "Presentation text items",
        report.summary.presentation_text_total,
    );
    audit_push_count(
        &mut out,
        "Presentation text user-visible",
        report.summary.presentation_text_user_visible,
    );
    audit_push_count(
        &mut out,
        "Presentation text observed",
        report.summary.presentation_text_observed,
    );
    audit_push_count(
        &mut out,
        "Presentation text unobserved",
        report.summary.presentation_text_unobserved,
    );
    audit_push_count(
        &mut out,
        "Presentation text internal-only",
        report.summary.presentation_text_internal_only,
    );
    audit_push_count(
        &mut out,
        "Presentation text visibility unknown",
        report.summary.presentation_text_visibility_unknown,
    );
    audit_push_count(
        &mut out,
        "Presentation text static limitations",
        report.summary.presentation_text_static_limitations,
    );
    if let Some(ratio) = finding_alignment_raw_to_canonical_ratio(&report.summary) {
        out.push_str(&format!("| Raw-to-canonical ratio | {:.2} |\n", ratio));
    } else {
        out.push_str("| Raw-to-canonical ratio | n/a |\n");
    }
    out.push('\n');

    out.push_str("## Actionable Canonical Gap Top Lists\n\n");
    scorecard_push_top_count_table(
        &mut out,
        "Actionable gap class",
        &report.actionable_gap_top_lists,
        "top_actionable_gap_classes",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Actionable file",
        &report.actionable_gap_top_lists,
        "top_actionable_files",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Repair kind",
        &report.actionable_gap_top_lists,
        "top_repair_kinds",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Missing discriminator kind",
        &report.actionable_gap_top_lists,
        "top_missing_discriminator_kinds",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Static limitation reason",
        &report.actionable_gap_top_lists,
        "top_static_limitation_reasons",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Verify command unknown class",
        &report.actionable_gap_top_lists,
        "top_verify_command_unknowns",
    );
    scorecard_push_top_count_table(
        &mut out,
        "Repair route unknown class",
        &report.actionable_gap_top_lists,
        "top_repair_route_unknowns",
    );

    out.push_str("## Actionable Gap Packet Public Projection Readiness\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Public projection eligible packets",
        report
            .summary
            .finding_alignment_actionable_gap_packet_public_projection_eligible_packets,
    );
    audit_push_count(
        &mut out,
        "Public projection excluded packets",
        report
            .summary
            .finding_alignment_actionable_gap_packet_public_projection_excluded_packets,
    );
    out.push('\n');
    scorecard_push_top_count_table(
        &mut out,
        "Projection exclusion reason",
        &report.actionable_gap_packet_public_projection,
        "projection_exclusion_reasons",
    );

    scorecard_push_language_aware_placement_route_quality(
        &mut out,
        &report.language_aware_placement_route_quality,
    );
    cross_language_oracle_route_quality_push_markdown(
        &mut out,
        &report.cross_language_oracle_route_quality,
    );

    out.push_str("## Evidence Class Work Queue\n\n");
    scorecard_push_evidence_class_work_queue_table(&mut out, &report.evidence_class_work_queue);

    out.push_str("## Maturity By Class\n\n");
    out.push_str("| Class | Status | Proof source | Known limits | Next repair |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for row in &report.maturity_by_class {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.class),
            audit_markdown_cell(&row.status),
            audit_markdown_cell(&row.proof_source),
            audit_markdown_cell(&row.known_limits),
            audit_markdown_cell(&row.recommended_next_repair),
        ));
    }
    out.push('\n');

    out.push_str("## Top Evidence-Quality Risks\n\n");
    if report.recommended_repairs.is_empty() {
        out.push_str("No scored evidence-quality repair risks were reported.\n\n");
    } else {
        out.push_str("| Repair slice | Evidence class | Risk | Signals | Expected impact |\n");
        out.push_str("| --- | --- | --- | ---: | --- |\n");
        for repair in &report.recommended_repairs {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                audit_markdown_cell(&repair.slice),
                audit_markdown_cell(&repair.evidence_class),
                audit_markdown_cell(&repair.risk_kind),
                repair.signal_count,
                audit_markdown_cell(&repair.expected_impact),
            ));
        }
        out.push('\n');
    }

    out.push_str("## Recommended Lane 1 Repairs\n\n");
    if report.recommended_repairs.is_empty() {
        out.push_str("No recommended repairs were emitted.\n\n");
    } else {
        for repair in &report.recommended_repairs {
            out.push_str(&format!(
                "- `{}`: {} ({} signals).\n",
                repair.slice, repair.why, repair.signal_count
            ));
        }
        out.push('\n');
    }

    out.push_str("## Duplicate-Looking And Canonical Group Signals\n\n");
    audit_push_count(
        &mut out,
        "Canonical gap groups",
        report.summary.canonical_gap_groups_total,
    );
    audit_push_count(
        &mut out,
        "Duplicate-looking groups",
        report.summary.duplicate_looking_groups_total,
    );
    out.push('\n');

    out.push_str("## Static Limitations And Missing Discriminators\n\n");
    audit_push_count(
        &mut out,
        "Static limitations",
        report.summary.static_limitations_total,
    );
    audit_push_count(
        &mut out,
        "Missing discriminators",
        report.summary.missing_discriminators_total,
    );
    out.push('\n');
    audit_push_value_counts_table_limited(
        &mut out,
        "Static limitation category",
        &report.static_limitation_categories,
        &["by_category"],
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_value_counts_table_limited(
        &mut out,
        "Static limitation repair route",
        &report.static_limitation_categories,
        &["repair_routes"],
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Related-Test And Oracle Distributions\n\n");
    audit_push_count(
        &mut out,
        "Related tests",
        report.summary.related_tests_total,
    );
    audit_push_count(
        &mut out,
        "Low or opaque top related tests",
        report.summary.low_or_opaque_top_related_tests,
    );
    out.push('\n');

    out.push_str("## Movement And Calibration Coverage\n\n");
    audit_push_count(
        &mut out,
        "Calibrated records",
        report.summary.calibrated_records,
    );
    audit_push_count(
        &mut out,
        "Uncalibrated records",
        report.summary.uncalibrated_records,
    );
    out.push('\n');
    out.push_str("### Runtime Confidence By Evidence Class\n\n");
    scorecard_push_runtime_confidence_by_class_table(&mut out, &report.calibration_coverage);

    out.push_str("## Recent Deltas\n\n");
    if report.recent_audit_deltas.available {
        out.push_str("| Metric | Before | After | Delta | Direction |\n");
        out.push_str("| --- | ---: | ---: | ---: | --- |\n");
        for delta in &report.recent_audit_deltas.deltas {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                audit_markdown_cell(&delta.metric),
                delta.before,
                delta.after,
                delta.delta,
                audit_markdown_cell(&delta.direction),
            ));
        }
        out.push('\n');
    } else {
        out.push_str(&format!(
            "{}\n\n",
            audit_markdown_cell(
                report
                    .recent_audit_deltas
                    .reason
                    .as_deref()
                    .unwrap_or("recent deltas unavailable")
            )
        ));
    }

    out.push_str("## Unknowns And Unavailable Inputs\n\n");
    if report.unknowns.is_empty() {
        out.push_str("No scorecard unknowns were reported.\n");
    } else {
        out.push_str("| Kind | Summary | Next repair |\n");
        out.push_str("| --- | --- | --- |\n");
        for unknown in &report.unknowns {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                audit_markdown_cell(&unknown.kind),
                audit_markdown_cell(&unknown.summary),
                audit_markdown_cell(unknown.next_repair.as_deref().unwrap_or("n/a")),
            ));
        }
    }
    out
}

const EVIDENCE_QUALITY_TREND_SCHEMA_VERSION: &str = "0.1";
const EVIDENCE_QUALITY_TREND_CATEGORY_LIMIT: usize = 8;
pub(crate) const EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE: &str =
    "evidence_quality_trend_previous_artifact_unavailable";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EvidenceQualityTrendArgs {
    current: Option<PathBuf>,
    previous: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityTrendInputs {
    pub(crate) current_scorecard: EvidenceQualityScorecardInput,
    pub(crate) previous_artifact: EvidenceQualityScorecardInput,
    pub(crate) capability_matrix: EvidenceQualityScorecardInput,
    pub(crate) traceability: EvidenceQualityScorecardInput,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EvidenceQualityTrendSummary {
    pub(crate) status: String,
    pub(crate) compared_metrics: usize,
    pub(crate) improved_metrics: usize,
    pub(crate) regressed_metrics: usize,
    pub(crate) unchanged_metrics: usize,
    pub(crate) unknown_metrics: usize,
    pub(crate) no_history: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityTrendMetric {
    pub(crate) metric: String,
    pub(crate) label: String,
    pub(crate) before: Option<usize>,
    pub(crate) after: Option<usize>,
    pub(crate) delta: Option<isize>,
    pub(crate) direction: String,
    pub(crate) interpretation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityTrendUnknown {
    pub(crate) kind: String,
    pub(crate) summary: String,
    pub(crate) next_repair: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceQualityTrendReport {
    pub(crate) generated_at: String,
    pub(crate) root: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) inputs: EvidenceQualityTrendInputs,
    pub(crate) summary: EvidenceQualityTrendSummary,
    pub(crate) metric_trends: Vec<EvidenceQualityTrendMetric>,
    pub(crate) static_limitation_category_trends: Vec<EvidenceQualityTrendMetric>,
    pub(crate) runtime_confidence_static_only_class_trends: Vec<EvidenceQualityTrendMetric>,
    pub(crate) unknowns: Vec<EvidenceQualityTrendUnknown>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceQualityTrendMovementFront {
    pub(crate) current_actionable_count: Option<usize>,
    pub(crate) actionable_delta_since_prior_refresh: Option<isize>,
    pub(crate) resolved: Option<usize>,
    pub(crate) improved: Option<usize>,
    pub(crate) unchanged_after_attempt: Option<usize>,
    pub(crate) missing_receipts: Option<usize>,
    pub(crate) orphaned_receipts: Option<usize>,
    pub(crate) top_blocked_reason: String,
}

struct EvidenceQualityTrendMetricSpec<'a> {
    metric: &'a str,
    label: &'a str,
    lower_is_better: bool,
    current_path: &'a [&'a str],
    previous_path: &'a [&'a str],
}

/// Build a repo-local Lane 1 evidence-quality trend report.
/// The trend is advisory and compares existing scorecard or audit snapshots;
/// it does not change analyzer behavior, gate policy, CI projection, editor
/// output, source files, generated tests, provider calls, or runtime execution.
pub(crate) fn evidence_quality_trend_report_impl(args: &[String]) -> Result<(), String> {
    ensure_reports_dir()?;
    let args = parse_evidence_quality_trend_args(args)?;
    let explicit_current = args.current.is_some();
    let current_path = args
        .current
        .unwrap_or_else(|| reports_dir().join("evidence-quality-scorecard.json"));
    if !current_path.exists() {
        if explicit_current {
            return Err(format!(
                "current evidence-quality scorecard not found: {}",
                current_path.display()
            ));
        }
        evidence_quality_scorecard_report_impl()?;
    }
    let current = read_json_value(&current_path).map_err(|err| {
        format!(
            "evidence-quality-trend requires a current scorecard at {}; {err}",
            current_path.display()
        )
    })?;

    let explicit_previous = args.previous.is_some();
    let previous_path = args
        .previous
        .or_else(evidence_quality_trend_default_previous_path)
        .unwrap_or_else(|| reports_dir().join("evidence-quality-scorecard.previous.json"));
    let previous = if previous_path.exists() {
        match read_json_value(&previous_path) {
            Ok(previous) => Some(previous),
            Err(err) => {
                return write_limited_evidence_quality_trend_for_previous_input_failure(
                    &current_path,
                    &current,
                    &previous_path,
                    "malformed",
                    &format!(
                        "failed to read previous evidence-quality artifact at {}; {err}",
                        previous_path.display()
                    ),
                );
            }
        }
    } else if explicit_previous {
        return write_limited_evidence_quality_trend_for_previous_input_failure(
            &current_path,
            &current,
            &previous_path,
            "missing",
            &format!(
                "previous evidence-quality artifact not found: {}",
                previous_path.display()
            ),
        );
    } else {
        None
    };

    let inputs =
        evidence_quality_trend_inputs(&current_path, &current, &previous_path, previous.as_ref())?;
    let report = evidence_quality_trend_from_values(
        evidence_quality_scorecard_generated_at()?,
        inputs,
        &current,
        previous.as_ref(),
    )?;

    write_report(
        "evidence-quality-trend.json",
        &evidence_quality_trend_json(&report)?,
    )?;
    write_report(
        "evidence-quality-trend.md",
        &evidence_quality_trend_markdown(&report),
    )
}

fn write_limited_evidence_quality_trend_for_previous_input_failure(
    current_path: &Path,
    current: &Value,
    previous_path: &Path,
    previous_status: &str,
    error: &str,
) -> Result<(), String> {
    let inputs = EvidenceQualityTrendInputs {
        current_scorecard: scorecard_input_artifact(
            current_path,
            "loaded",
            Some(current),
            "current evidence-quality scorecard",
        )?,
        previous_artifact: scorecard_input_artifact(
            previous_path,
            previous_status,
            None,
            "optional previous scorecard or audit snapshot unavailable; movement is diagnostic only",
        )?,
        capability_matrix: scorecard_input_artifact(
            Path::new("docs/CAPABILITY_MATRIX.md"),
            "loaded",
            None,
            "class-scoped capability maturity vocabulary",
        )?,
        traceability: scorecard_input_artifact(
            Path::new(".ripr/traceability.toml"),
            "loaded",
            None,
            "spec/test/code/output/metric linkage",
        )?,
    };
    let previous_artifact_path = inputs.previous_artifact.path.clone();
    let mut report = evidence_quality_trend_from_values(
        evidence_quality_scorecard_generated_at()?,
        inputs,
        current,
        None,
    )?;
    trend_push_unknown(
        &mut report.unknowns,
        EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE,
        &format!(
            "Evidence-quality trend could not load the requested previous artifact: {}. No movement or badge-readiness delta claim is made from this limited trend.",
            evidence_quality_scorecard_error_summary(error)
        ),
        Some("report/evidence-quality-trend"),
    );
    report.runtime_status = lane1_runtime_status_limited_input(
        "previous_artifact_input",
        "evidence-quality-previous-artifact",
        Some(&previous_artifact_path),
        EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE,
        "supply a readable previous scorecard or audit snapshot, or omit --previous when movement history is optional",
        true,
    );
    write_report(
        "evidence-quality-trend.json",
        &evidence_quality_trend_json(&report)?,
    )?;
    write_report(
        "evidence-quality-trend.md",
        &evidence_quality_trend_markdown(&report),
    )
}

fn parse_evidence_quality_trend_args(args: &[String]) -> Result<EvidenceQualityTrendArgs, String> {
    let mut parsed = EvidenceQualityTrendArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--current" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("evidence-quality-trend --current requires a path".to_string());
                };
                parsed.current = Some(PathBuf::from(value));
                index += 2;
            }
            "--previous" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("evidence-quality-trend --previous requires a path".to_string());
                };
                parsed.previous = Some(PathBuf::from(value));
                index += 2;
            }
            "--help" | "-h" => {
                return Err(
                    "usage: cargo xtask evidence-quality-trend [--current <path>] [--previous <path>]"
                        .to_string(),
                );
            }
            other => {
                return Err(format!(
                    "unknown evidence-quality-trend argument `{other}`; expected --current or --previous"
                ));
            }
        }
    }
    Ok(parsed)
}

fn evidence_quality_trend_default_previous_path() -> Option<PathBuf> {
    [
        reports_dir().join("evidence-quality-scorecard.previous.json"),
        reports_dir().join("lane1-evidence-audit.previous.json"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn evidence_quality_trend_inputs(
    current_path: &Path,
    current: &Value,
    previous_path: &Path,
    previous: Option<&Value>,
) -> Result<EvidenceQualityTrendInputs, String> {
    Ok(EvidenceQualityTrendInputs {
        current_scorecard: scorecard_input_artifact(
            current_path,
            "loaded",
            Some(current),
            "current evidence-quality scorecard",
        )?,
        previous_artifact: scorecard_input_artifact(
            previous_path,
            if previous.is_some() {
                "loaded"
            } else {
                "missing"
            },
            previous,
            "optional previous scorecard or audit snapshot for trend comparison",
        )?,
        capability_matrix: scorecard_input_artifact(
            Path::new("docs/CAPABILITY_MATRIX.md"),
            "loaded",
            None,
            "class-scoped capability maturity vocabulary",
        )?,
        traceability: scorecard_input_artifact(
            Path::new(".ripr/traceability.toml"),
            "loaded",
            None,
            "spec/test/code/output/metric linkage",
        )?,
    })
}

pub(crate) fn evidence_quality_trend_from_values(
    generated_at: String,
    inputs: EvidenceQualityTrendInputs,
    current: &Value,
    previous: Option<&Value>,
) -> Result<EvidenceQualityTrendReport, String> {
    let root = audit_string(current, &["scope", "root"])
        .or_else(|| audit_string(current, &["inputs", "root"]))
        .unwrap_or_else(|| ".".to_string());
    let current_limited_kinds = evidence_quality_current_scorecard_limited_kinds(current);
    let mut metric_trends = evidence_quality_metric_trends(current, previous);
    let mut static_limitation_category_trends =
        evidence_quality_static_limitation_category_trends(current, previous);
    let mut runtime_confidence_static_only_class_trends =
        evidence_quality_runtime_confidence_static_only_class_trends(current, previous);
    if !current_limited_kinds.is_empty() {
        evidence_quality_mark_limited_current_trends_unknown(
            &mut metric_trends,
            &current_limited_kinds,
        );
        evidence_quality_mark_limited_current_trends_unknown(
            &mut static_limitation_category_trends,
            &current_limited_kinds,
        );
        evidence_quality_mark_limited_current_trends_unknown(
            &mut runtime_confidence_static_only_class_trends,
            &current_limited_kinds,
        );
    }
    let summary = evidence_quality_trend_summary(previous, &metric_trends);
    let unknowns =
        evidence_quality_trend_unknowns(previous, &metric_trends, &current_limited_kinds);
    let runtime_status = evidence_quality_trend_runtime_status(current, &inputs, &unknowns);

    Ok(EvidenceQualityTrendReport {
        generated_at,
        root,
        runtime_status,
        inputs,
        summary,
        metric_trends,
        static_limitation_category_trends,
        runtime_confidence_static_only_class_trends,
        unknowns,
    })
}

fn evidence_quality_trend_runtime_status(
    current: &Value,
    inputs: &EvidenceQualityTrendInputs,
    unknowns: &[EvidenceQualityTrendUnknown],
) -> Lane1RuntimeStatus {
    if let Some(status) = lane1_runtime_status_from_report_value(current)
        && status.state != "full"
    {
        return lane1_runtime_status_with_input_path(
            status,
            "current_scorecard_input",
            &inputs.current_scorecard.path,
        );
    }
    if unknowns
        .iter()
        .any(|unknown| unknown.kind == "current_scorecard_limited")
    {
        return lane1_runtime_status_limited_input(
            "current_scorecard_input",
            "evidence-quality-scorecard",
            Some(&inputs.current_scorecard.path),
            "current_scorecard_limited",
            "rerun Lane 1 audit, evidence-health, and scorecard after resolving limited input diagnostics",
            false,
        );
    }
    if unknowns
        .iter()
        .any(|unknown| unknown.kind == EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE)
    {
        return lane1_runtime_status_limited_input(
            "previous_artifact_input",
            "evidence-quality-previous-artifact",
            Some(&inputs.previous_artifact.path),
            EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE,
            "supply a readable previous scorecard or audit snapshot, or omit --previous when movement history is optional",
            true,
        );
    }
    lane1_runtime_status_full()
}

fn evidence_quality_current_scorecard_limited_kinds(current: &Value) -> Vec<String> {
    audit_get(current, &["unknowns"])
        .and_then(Value::as_array)
        .map(|unknowns| {
            unknowns
                .iter()
                .filter_map(|unknown| unknown.get("kind").and_then(Value::as_str))
                .filter(|kind| evidence_quality_current_scorecard_limited_kind(kind))
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn evidence_quality_current_scorecard_limited_kind(kind: &str) -> bool {
    matches!(
        kind,
        "lane1_evidence_audit_limited"
            | "evidence_health_limited"
            | "evidence_quality_scorecard_audit_regeneration_failed"
    )
}

fn evidence_quality_mark_limited_current_trends_unknown(
    trends: &mut [EvidenceQualityTrendMetric],
    limited_kinds: &[String],
) {
    let interpretation = evidence_quality_limited_current_interpretation(limited_kinds);
    for trend in trends {
        trend.delta = None;
        trend.direction = "unknown".to_string();
        trend.interpretation = interpretation.clone();
    }
}

fn evidence_quality_limited_current_interpretation(limited_kinds: &[String]) -> String {
    format!(
        "Current scorecard has limited input diagnostics ({}), so trend direction is not claimed.",
        limited_kinds.join(", ")
    )
}

fn evidence_quality_metric_trends(
    current: &Value,
    previous: Option<&Value>,
) -> Vec<EvidenceQualityTrendMetric> {
    let mut trends = [
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_actionable_unresolved_canonical_gaps",
            label: "Actionable canonical gaps",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_actionable_unresolved_canonical_gaps",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_actionable_unresolved_canonical_gaps",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "raw_headline_gaps",
            label: "Raw headline gaps",
            lower_is_better: true,
            current_path: &["summary", "raw_headline_gaps"],
            previous_path: &["summary", "raw_headline_gaps"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "duplicate_looking_groups_total",
            label: "Duplicate-looking groups",
            lower_is_better: true,
            current_path: &["summary", "duplicate_looking_groups_total"],
            previous_path: &["summary", "duplicate_looking_groups_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "missing_discriminators_total",
            label: "Missing discriminators",
            lower_is_better: true,
            current_path: &["summary", "missing_discriminators_total"],
            previous_path: &["summary", "missing_discriminators_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "static_limitations_total",
            label: "Static limitations",
            lower_is_better: true,
            current_path: &["summary", "static_limitations_total"],
            previous_path: &["summary", "static_limitations_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "low_or_opaque_top_related_tests",
            label: "Low or opaque top related tests",
            lower_is_better: true,
            current_path: &["summary", "low_or_opaque_top_related_tests"],
            previous_path: &["summary", "low_or_opaque_top_related_tests"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "oracle_unknown_count",
            label: "Unknown oracle classifications",
            lower_is_better: true,
            current_path: &[],
            previous_path: &[],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "uncalibrated_records",
            label: "Uncalibrated records",
            lower_is_better: true,
            current_path: &["summary", "uncalibrated_records"],
            previous_path: &["summary", "uncalibrated_records"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "calibrated_records",
            label: "Calibrated records",
            lower_is_better: false,
            current_path: &["summary", "calibrated_records"],
            previous_path: &["summary", "calibrated_records"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "evidence_records_missing",
            label: "Evidence records missing",
            lower_is_better: true,
            current_path: &["summary", "evidence_records_missing"],
            previous_path: &["summary", "evidence_records_missing"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_raw_signals_total",
            label: "Finding-alignment raw signals",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_raw_signals_total"],
            previous_path: &["summary", "finding_alignment_raw_signals_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_canonical_items_total",
            label: "Finding-alignment canonical items",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_canonical_items_total"],
            previous_path: &["summary", "finding_alignment_canonical_items_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_duplicate_groups_total",
            label: "Finding-alignment duplicate groups",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_duplicate_groups_total"],
            previous_path: &["summary", "finding_alignment_duplicate_groups_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_actionable_items_total",
            label: "Finding-alignment actionable items",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_actionable_items_total"],
            previous_path: &["summary", "finding_alignment_actionable_items_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_canonical_items_without_repair_route",
            label: "Finding-alignment canonical items without repair route",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_canonical_items_without_repair_route",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_canonical_items_without_repair_route",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_canonical_items_without_verify_command",
            label: "Finding-alignment canonical items without verify command",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_canonical_items_without_verify_command",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_canonical_items_without_verify_command",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_static_unknown_without_named_limitation",
            label: "Finding-alignment static unknown without named limitation",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_static_unknown_without_named_limitation",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_static_unknown_without_named_limitation",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_actionable_gap_packet_public_projection_eligible_packets",
            label: "Actionable gap public-projection eligible packets",
            lower_is_better: false,
            current_path: &[
                "summary",
                "finding_alignment_actionable_gap_packet_public_projection_eligible_packets",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_actionable_gap_packet_public_projection_eligible_packets",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_actionable_gap_packet_public_projection_excluded_packets",
            label: "Actionable gap public-projection excluded packets",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_actionable_gap_packet_public_projection_excluded_packets",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_actionable_gap_packet_public_projection_excluded_packets",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_already_observed_total",
            label: "Finding-alignment already observed items",
            lower_is_better: false,
            current_path: &["summary", "finding_alignment_already_observed_total"],
            previous_path: &["summary", "finding_alignment_already_observed_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_internal_no_action_total",
            label: "Finding-alignment internal no-action items",
            lower_is_better: false,
            current_path: &["summary", "finding_alignment_internal_no_action_total"],
            previous_path: &["summary", "finding_alignment_internal_no_action_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_static_limitation_total",
            label: "Finding-alignment static limitations",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_static_limitation_total"],
            previous_path: &["summary", "finding_alignment_static_limitation_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_calibrated_supported_total",
            label: "Finding-alignment calibrated-supported items",
            lower_is_better: false,
            current_path: &["summary", "finding_alignment_calibrated_supported_total"],
            previous_path: &["summary", "finding_alignment_calibrated_supported_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_uncalibrated_total",
            label: "Finding-alignment uncalibrated items",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_uncalibrated_total"],
            previous_path: &["summary", "finding_alignment_uncalibrated_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_visibility_unknown_total",
            label: "Finding-alignment visibility unknown",
            lower_is_better: true,
            current_path: &["summary", "finding_alignment_visibility_unknown_total"],
            previous_path: &["summary", "finding_alignment_visibility_unknown_total"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "finding_alignment_presentation_text_actionable_total",
            label: "Presentation text actionable items",
            lower_is_better: true,
            current_path: &[
                "summary",
                "finding_alignment_presentation_text_actionable_total",
            ],
            previous_path: &[
                "summary",
                "finding_alignment_presentation_text_actionable_total",
            ],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "presentation_text_visibility_unknown",
            label: "Presentation text visibility unknown",
            lower_is_better: true,
            current_path: &["summary", "presentation_text_visibility_unknown"],
            previous_path: &["summary", "presentation_text_visibility_unknown"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "presentation_text_static_limitations",
            label: "Presentation text static limitations",
            lower_is_better: true,
            current_path: &["summary", "presentation_text_static_limitations"],
            previous_path: &["summary", "presentation_text_static_limitations"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "presentation_text_observed",
            label: "Presentation text observed",
            lower_is_better: false,
            current_path: &["summary", "presentation_text_observed"],
            previous_path: &["summary", "presentation_text_observed"],
        },
        EvidenceQualityTrendMetricSpec {
            metric: "presentation_text_no_action",
            label: "Presentation text no-action",
            lower_is_better: false,
            current_path: &["summary", "presentation_text_no_action"],
            previous_path: &["summary", "presentation_text_no_action"],
        },
    ]
    .into_iter()
    .map(|spec| evidence_quality_metric_trend(current, previous, spec))
    .collect::<Vec<_>>();
    trends.sort_by(|left, right| {
        evidence_quality_trend_metric_order(&left.metric)
            .cmp(&evidence_quality_trend_metric_order(&right.metric))
            .then_with(|| left.metric.cmp(&right.metric))
    });
    trends
}

fn evidence_quality_trend_metric_order(metric: &str) -> u8 {
    match metric {
        "finding_alignment_actionable_unresolved_canonical_gaps" => 0,
        _ => 1,
    }
}

fn evidence_quality_metric_trend(
    current: &Value,
    previous: Option<&Value>,
    spec: EvidenceQualityTrendMetricSpec<'_>,
) -> EvidenceQualityTrendMetric {
    let after = if spec.metric == "oracle_unknown_count" {
        Some(evidence_quality_oracle_unknown_count(current))
    } else {
        audit_usize(current, spec.current_path)
    };
    let before = previous.and_then(|previous| {
        if spec.metric == "oracle_unknown_count" {
            Some(evidence_quality_oracle_unknown_count(previous))
        } else {
            audit_usize(previous, spec.previous_path)
        }
    });
    let (delta, direction) = evidence_quality_trend_direction(before, after, spec.lower_is_better);
    let interpretation = evidence_quality_metric_interpretation(spec.metric, &direction);
    EvidenceQualityTrendMetric {
        metric: spec.metric.to_string(),
        label: spec.label.to_string(),
        before,
        after,
        delta,
        direction,
        interpretation,
    }
}

fn evidence_quality_trend_direction(
    before: Option<usize>,
    after: Option<usize>,
    lower_is_better: bool,
) -> (Option<isize>, String) {
    let (Some(before), Some(after)) = (before, after) else {
        return (None, "unknown".to_string());
    };
    let delta = after as isize - before as isize;
    if delta == 0 {
        return (Some(delta), "unchanged".to_string());
    }
    let improved = if lower_is_better {
        after < before
    } else {
        after > before
    };
    if improved {
        (Some(delta), "improvement".to_string())
    } else {
        (Some(delta), "regression".to_string())
    }
}

fn evidence_quality_metric_interpretation(metric: &str, direction: &str) -> String {
    if direction == "unknown" {
        return "No comparable previous value was available.".to_string();
    }
    match metric {
        "calibrated_records" => {
            "Higher calibrated record counts show broader checked imported-runtime coverage."
        }
        "oracle_unknown_count" => {
            "Lower unknown oracle counts show sharper fixture-backed oracle semantics."
        }
        "low_or_opaque_top_related_tests" => {
            "Lower low or opaque top related-test counts improve first-useful-action reliability."
        }
        "duplicate_looking_groups_total" => {
            "Lower duplicate-looking group counts reduce canonical-gap overcount risk."
        }
        "uncalibrated_records" => {
            "Lower uncalibrated record counts indicate more evidence classes have runtime context."
        }
        "static_limitations_total" => {
            "Lower static limitation counts indicate fewer analyzer limits in the current evidence."
        }
        "evidence_records_missing" => {
            "Lower missing evidence_record counts protect the shared evidence spine."
        }
        "raw_headline_gaps" | "missing_discriminators_total" => {
            "Lower counts may be useful but require class-scoped fixture context before promotion."
        }
        "finding_alignment_raw_signals_total" | "finding_alignment_canonical_items_total" => {
            "Alignment volume is diagnostic; interpret direction with the raw-to-canonical ratio and class mix."
        }
        "finding_alignment_duplicate_groups_total" => {
            "Lower duplicate group counts mean fewer raw findings are surfacing as duplicate actions."
        }
        "finding_alignment_actionable_items_total"
        | "finding_alignment_presentation_text_actionable_total" => {
            "Lower unresolved actionable item counts are useful only when already-observed, internal-only, or limitation counts explain the movement."
        }
        "finding_alignment_already_observed_total" | "presentation_text_observed" => {
            "Higher observed counts mean more canonical items are recognized as already gripped."
        }
        "finding_alignment_calibrated_supported_total" => {
            "Higher calibrated-supported counts show more canonical items have checked runtime context."
        }
        "finding_alignment_uncalibrated_total" => {
            "Lower uncalibrated counts indicate more canonical items have class-scoped runtime context."
        }
        "finding_alignment_internal_no_action_total" | "presentation_text_no_action" => {
            "Higher no-action counts mean more raw signals are classified as non-user debt."
        }
        "finding_alignment_static_limitation_total"
        | "finding_alignment_visibility_unknown_total"
        | "presentation_text_visibility_unknown"
        | "presentation_text_static_limitations" => {
            "Lower limitation counts mean fewer analyzer-unknown items remain for this evidence class."
        }
        "finding_alignment_actionable_gap_packet_public_projection_eligible_packets" => {
            "Higher eligible packet counts mean more actionable canonical gaps have the evidence needed for future public projection readiness."
        }
        "finding_alignment_actionable_gap_packet_public_projection_excluded_packets" => {
            "Lower excluded packet counts mean fewer actionable canonical gap packets are missing projection-readiness prerequisites."
        }
        _ => "Trend is advisory and does not redefine RIPR scores.",
    }
    .to_string()
}

fn evidence_quality_oracle_unknown_count(value: &Value) -> usize {
    scorecard_count_at(
        value,
        &[
            "oracle_semantics_distribution",
            "oracle_kind_counts",
            "unknown",
        ],
    ) + scorecard_count_at(
        value,
        &[
            "oracle_semantics_distribution",
            "oracle_strength_counts",
            "unknown",
        ],
    )
}

fn evidence_quality_static_limitation_category_trends(
    current: &Value,
    previous: Option<&Value>,
) -> Vec<EvidenceQualityTrendMetric> {
    let current_counts = evidence_quality_static_category_counts(current);
    let previous_counts = previous
        .map(evidence_quality_static_category_counts)
        .unwrap_or_default();
    let categories = current_counts
        .keys()
        .chain(previous_counts.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut rows = categories
        .iter()
        .map(|category| {
            let after = current_counts.get(category).copied();
            let before = previous_counts.get(category).copied();
            let (delta, direction) = evidence_quality_trend_direction(before, after, true);
            EvidenceQualityTrendMetric {
                metric: format!("static_limitation_category:{category}"),
                label: category.clone(),
                before,
                after,
                delta,
                direction,
                interpretation:
                    "Lower category counts indicate fewer analyzer limitations of this class."
                        .to_string(),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .after
            .unwrap_or(0)
            .cmp(&left.after.unwrap_or(0))
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(EVIDENCE_QUALITY_TREND_CATEGORY_LIMIT);
    rows
}

fn evidence_quality_static_category_counts(value: &Value) -> BTreeMap<String, usize> {
    [
        &["static_limitation_categories", "by_category"][..],
        &["static_limitations", "by_category"][..],
    ]
    .into_iter()
    .find_map(|path| audit_get(value, path).and_then(Value::as_object))
    .map(|object| {
        object
            .iter()
            .filter_map(|(key, value)| value.as_u64().map(|count| (key.clone(), count as usize)))
            .collect::<BTreeMap<_, _>>()
    })
    .unwrap_or_default()
}

fn evidence_quality_runtime_confidence_static_only_class_trends(
    current: &Value,
    previous: Option<&Value>,
) -> Vec<EvidenceQualityTrendMetric> {
    let current_counts = evidence_quality_runtime_confidence_static_only_class_counts(current);
    let previous_counts = previous
        .map(evidence_quality_runtime_confidence_static_only_class_counts)
        .unwrap_or_default();
    let classes = current_counts
        .keys()
        .chain(previous_counts.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut rows = classes
        .iter()
        .map(|class| {
            let after = current_counts.get(class).copied();
            let before = previous_counts.get(class).copied();
            let (delta, direction) = evidence_quality_trend_direction(before, after, true);
            EvidenceQualityTrendMetric {
                metric: format!("runtime_confidence_static_only_class:{class}"),
                label: class.clone(),
                before,
                after,
                delta,
                direction,
                interpretation:
                    "Lower static-only counts mean more canonical items in this class have runtime context."
                        .to_string(),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .after
            .unwrap_or(0)
            .cmp(&left.after.unwrap_or(0))
            .then_with(|| right.before.unwrap_or(0).cmp(&left.before.unwrap_or(0)))
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(EVIDENCE_QUALITY_TREND_CATEGORY_LIMIT);
    rows
}

fn evidence_quality_runtime_confidence_static_only_class_counts(
    value: &Value,
) -> BTreeMap<String, usize> {
    [
        &["calibration_coverage", "by_evidence_class"][..],
        &["calibration_availability", "runtime_confidence_by_class"][..],
        &["finding_alignment", "runtime_confidence_by_class"][..],
    ]
    .into_iter()
    .find_map(|path| audit_get(value, path).and_then(Value::as_array))
    .map(|rows| {
        rows.iter()
            .filter_map(|row| {
                let class = audit_string(row, &["evidence_class"])?;
                let count = audit_usize(row, &["static_only"]).unwrap_or(0);
                (count > 0).then_some((class, count))
            })
            .collect::<BTreeMap<_, _>>()
    })
    .unwrap_or_default()
}

fn evidence_quality_trend_summary(
    previous: Option<&Value>,
    metric_trends: &[EvidenceQualityTrendMetric],
) -> EvidenceQualityTrendSummary {
    if previous.is_none() {
        return EvidenceQualityTrendSummary {
            status: "unknown".to_string(),
            unknown_metrics: metric_trends.len(),
            no_history: true,
            ..EvidenceQualityTrendSummary::default()
        };
    }
    let improved_metrics = metric_trends
        .iter()
        .filter(|trend| trend.direction == "improvement")
        .count();
    let regressed_metrics = metric_trends
        .iter()
        .filter(|trend| trend.direction == "regression")
        .count();
    let unchanged_metrics = metric_trends
        .iter()
        .filter(|trend| trend.direction == "unchanged")
        .count();
    let unknown_metrics = metric_trends
        .iter()
        .filter(|trend| trend.direction == "unknown")
        .count();
    let compared_metrics = metric_trends.len().saturating_sub(unknown_metrics);
    let status = if compared_metrics == 0 {
        "unknown"
    } else if regressed_metrics > 0 && improved_metrics > 0 {
        "mixed"
    } else if regressed_metrics > 0 {
        "regression"
    } else if improved_metrics > 0 {
        "improvement"
    } else {
        "unchanged"
    };
    EvidenceQualityTrendSummary {
        status: status.to_string(),
        compared_metrics,
        improved_metrics,
        regressed_metrics,
        unchanged_metrics,
        unknown_metrics,
        no_history: false,
    }
}

fn evidence_quality_trend_unknowns(
    previous: Option<&Value>,
    metric_trends: &[EvidenceQualityTrendMetric],
    current_limited_kinds: &[String],
) -> Vec<EvidenceQualityTrendUnknown> {
    let mut unknowns = Vec::new();
    if previous.is_none() {
        trend_push_unknown(
            &mut unknowns,
            "trend_history_unavailable",
            "No previous scorecard or audit snapshot was available, so the report cannot claim improvement or regression.",
            Some("report/evidence-quality-trend"),
        );
    }
    if !current_limited_kinds.is_empty() {
        trend_push_unknown(
            &mut unknowns,
            "current_scorecard_limited",
            &format!(
                "Current scorecard includes limited input diagnostics ({}), so the trend cannot claim improvement or regression.",
                current_limited_kinds.join(", ")
            ),
            Some(
                "rerun Lane 1 audit, evidence-health, and scorecard after resolving limited input diagnostics",
            ),
        );
    }
    for trend in metric_trends
        .iter()
        .filter(|trend| trend.direction == "unknown" && trend.after.is_none())
    {
        trend_push_unknown(
            &mut unknowns,
            "current_metric_missing",
            &format!("Current scorecard is missing metric `{}`.", trend.metric),
            Some("report/evidence-quality-scorecard"),
        );
    }
    unknowns
}

fn trend_push_unknown(
    unknowns: &mut Vec<EvidenceQualityTrendUnknown>,
    kind: &str,
    summary: &str,
    next_repair: Option<&str>,
) {
    unknowns.push(EvidenceQualityTrendUnknown {
        kind: kind.to_string(),
        summary: summary.to_string(),
        next_repair: next_repair.map(str::to_string),
    });
}

pub(crate) fn evidence_quality_trend_json(
    report: &EvidenceQualityTrendReport,
) -> Result<String, String> {
    let movement_front = evidence_quality_trend_movement_front(report);
    let value = serde_json::json!({
        "schema_version": EVIDENCE_QUALITY_TREND_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "evidence-quality-trend",
        "generated_at": report.generated_at,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "scope": {
            "kind": "repo",
            "root": report.root,
        },
        "inputs": {
            "current_scorecard": scorecard_input_json(&report.inputs.current_scorecard),
            "previous_artifact": scorecard_input_json(&report.inputs.previous_artifact),
            "capability_matrix": scorecard_input_json(&report.inputs.capability_matrix),
            "traceability": scorecard_input_json(&report.inputs.traceability),
        },
        "summary": {
            "status": report.summary.status,
            "compared_metrics": report.summary.compared_metrics,
            "improved_metrics": report.summary.improved_metrics,
            "regressed_metrics": report.summary.regressed_metrics,
            "unchanged_metrics": report.summary.unchanged_metrics,
            "unknown_metrics": report.summary.unknown_metrics,
            "no_history": report.summary.no_history,
        },
        "movement_front": evidence_quality_trend_movement_front_json(&movement_front),
        "metric_trends": evidence_quality_trend_metrics_json(&report.metric_trends),
        "static_limitation_category_trends": evidence_quality_trend_metrics_json(&report.static_limitation_category_trends),
        "runtime_confidence_static_only_class_trends": evidence_quality_trend_metrics_json(&report.runtime_confidence_static_only_class_trends),
        "unknowns": report.unknowns.iter().map(|unknown| {
            serde_json::json!({
                "kind": unknown.kind,
                "summary": unknown.summary,
                "next_repair": unknown.next_repair,
            })
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render evidence quality trend JSON: {err}"))
}

fn evidence_quality_trend_movement_front(
    report: &EvidenceQualityTrendReport,
) -> EvidenceQualityTrendMovementFront {
    let actionable = report
        .metric_trends
        .iter()
        .find(|trend| trend.metric == "finding_alignment_actionable_unresolved_canonical_gaps");
    EvidenceQualityTrendMovementFront {
        current_actionable_count: actionable.and_then(|trend| trend.after),
        actionable_delta_since_prior_refresh: actionable.and_then(|trend| trend.delta),
        resolved: None,
        improved: None,
        unchanged_after_attempt: None,
        missing_receipts: None,
        orphaned_receipts: None,
        top_blocked_reason: evidence_quality_trend_top_blocked_reason(report),
    }
}

fn evidence_quality_trend_top_blocked_reason(report: &EvidenceQualityTrendReport) -> String {
    if let Some(unknown) = report
        .unknowns
        .iter()
        .find(|unknown| unknown.kind == "current_scorecard_limited")
        .or_else(|| {
            report
                .unknowns
                .iter()
                .find(|unknown| unknown.kind == "trend_history_unavailable")
        })
    {
        return unknown.kind.clone();
    }
    if let Some(regressed) = report
        .metric_trends
        .iter()
        .find(|trend| trend.direction == "regression")
    {
        return format!("metric_regression:{}", regressed.metric);
    }
    "receipt_linked_outcomes_unavailable".to_string()
}

fn evidence_quality_trend_movement_front_json(front: &EvidenceQualityTrendMovementFront) -> Value {
    serde_json::json!({
        "current_actionable_count": front.current_actionable_count,
        "actionable_delta_since_prior_refresh": front.actionable_delta_since_prior_refresh,
        "resolved": front.resolved,
        "improved": front.improved,
        "unchanged_after_attempt": front.unchanged_after_attempt,
        "missing_receipts": front.missing_receipts,
        "orphaned_receipts": front.orphaned_receipts,
        "top_blocked_reason": front.top_blocked_reason,
        "receipt_linked_movement_source": "unavailable_in_evidence_quality_trend",
        "next_receipt_linked_command": "cargo xtask actionable-gap-outcomes",
    })
}

fn evidence_quality_trend_metrics_json(metrics: &[EvidenceQualityTrendMetric]) -> Vec<Value> {
    metrics
        .iter()
        .map(|trend| {
            serde_json::json!({
                "metric": trend.metric,
                "label": trend.label,
                "before": trend.before,
                "after": trend.after,
                "delta": trend.delta,
                "direction": trend.direction,
                "interpretation": trend.interpretation,
            })
        })
        .collect()
}

fn trend_optional_usize_cell(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn trend_optional_isize_cell(value: Option<isize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

pub(crate) fn evidence_quality_trend_markdown(report: &EvidenceQualityTrendReport) -> String {
    let movement_front = evidence_quality_trend_movement_front(report);
    let mut out = String::new();
    out.push_str("# Lane 1 evidence quality trend\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str("This repo-local trend compares existing Lane 1 scorecard or audit snapshots. It does not change analyzer behavior, gate policy, PR or CI projection, editor output, source files, generated tests, provider calls, score definitions, or runtime execution.\n\n");

    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Movement Since Prior Refresh\n\n");
    out.push_str("This front section reports scorecard movement for actionable counts. Receipt-linked resolved, improved, unchanged, missing-receipt, and orphaned-receipt movement belongs to `cargo xtask actionable-gap-outcomes`; this trend report does not infer those states from scorecard deltas.\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| --- | ---: |\n");
    out.push_str(&format!(
        "| Current actionable count | {} |\n",
        trend_optional_usize_cell(movement_front.current_actionable_count)
    ));
    out.push_str(&format!(
        "| Delta since prior refresh | {} |\n",
        trend_optional_isize_cell(movement_front.actionable_delta_since_prior_refresh)
    ));
    out.push_str("| Resolved | unavailable |\n");
    out.push_str("| Improved | unavailable |\n");
    out.push_str("| Unchanged after attempt | unavailable |\n");
    out.push_str("| Missing receipts | unavailable |\n");
    out.push_str("| Orphaned receipts | unavailable |\n");
    out.push_str(&format!(
        "| Top blocked reason | {} |\n\n",
        audit_markdown_cell(&movement_front.top_blocked_reason)
    ));

    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| --- | ---: |\n");
    out.push_str(&format!("| Status | {} |\n", report.summary.status));
    audit_push_count(
        &mut out,
        "Compared metrics",
        report.summary.compared_metrics,
    );
    audit_push_count(
        &mut out,
        "Improved metrics",
        report.summary.improved_metrics,
    );
    audit_push_count(
        &mut out,
        "Regressed metrics",
        report.summary.regressed_metrics,
    );
    audit_push_count(
        &mut out,
        "Unchanged metrics",
        report.summary.unchanged_metrics,
    );
    audit_push_count(&mut out, "Unknown metrics", report.summary.unknown_metrics);
    out.push('\n');

    out.push_str("## Metric Trends\n\n");
    trend_push_metric_table(&mut out, &report.metric_trends);

    out.push_str("## Static Limitation Category Trends\n\n");
    if report.static_limitation_category_trends.is_empty() {
        out.push_str("No static limitation category trend rows were reported.\n\n");
    } else {
        trend_push_metric_table(&mut out, &report.static_limitation_category_trends);
    }

    out.push_str("## Runtime Confidence Static-Only Class Trends\n\n");
    if report
        .runtime_confidence_static_only_class_trends
        .is_empty()
    {
        out.push_str("No runtime confidence static-only class trend rows were reported.\n\n");
    } else {
        trend_push_metric_table(
            &mut out,
            &report.runtime_confidence_static_only_class_trends,
        );
    }

    out.push_str("## Unknowns\n\n");
    if report.unknowns.is_empty() {
        out.push_str("No trend unknowns were reported.\n");
    } else {
        out.push_str("| Kind | Summary | Next repair |\n");
        out.push_str("| --- | --- | --- |\n");
        for unknown in &report.unknowns {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                audit_markdown_cell(&unknown.kind),
                audit_markdown_cell(&unknown.summary),
                audit_markdown_cell(unknown.next_repair.as_deref().unwrap_or("n/a")),
            ));
        }
    }
    out
}

fn trend_push_metric_table(out: &mut String, metrics: &[EvidenceQualityTrendMetric]) {
    if metrics.is_empty() {
        out.push_str("No metric trends were reported.\n\n");
        return;
    }
    out.push_str("| Metric | Before | After | Delta | Direction | Interpretation |\n");
    out.push_str("| --- | ---: | ---: | ---: | --- | --- |\n");
    for trend in metrics {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&trend.label),
            trend
                .before
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            trend
                .after
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            trend
                .delta
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            audit_markdown_cell(&trend.direction),
            audit_markdown_cell(&trend.interpretation),
        ));
    }
    out.push('\n');
}
