//! Evidence-audit cluster: the Lane 1 evidence audit (builder, runtime
//! status, static-limitation backlog, repo-exposure capture, JSON/markdown
//! renderers) and the actionable-gap-outcomes report (packet identity,
//! movement detection, projection exclusions, outcome counts, JSON/markdown
//! renderers), plus the report-local `audit_*` JSON and markdown atoms that
//! sit physically inside this region.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported from `main.rs` so
//! existing call sites (`dispatch.rs`, `reports/repo.rs`, `ripr_swarm.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

const LANE1_EVIDENCE_AUDIT_SCHEMA_VERSION: &str = "0.1";
pub(crate) const LANE1_EVIDENCE_AUDIT_TOP_LIMIT: usize = 10;
const LANE1_STATIC_LIMITATION_BACKLOG_PACKET_LIMIT: usize = LANE1_EVIDENCE_AUDIT_TOP_LIMIT * 2;
const LANE1_EVIDENCE_AUDIT_DUPLICATE_LIMIT: usize = 25;
const LANE1_ACTIONABLE_GAP_PACKET_LIMIT: usize = 25;
const LANE1_EVIDENCE_AUDIT_TRACE_TAIL_LIMIT: usize = 12;
const LANE1_EVIDENCE_AUDIT_TIMEOUT_ENV: &str = "RIPR_LANE1_EVIDENCE_AUDIT_TIMEOUT_MS";
pub(crate) const LANE1_EVIDENCE_AUDIT_DEFAULT_TIMEOUT_MS: u64 = 240_000;
const LANE1_EVIDENCE_AUDIT_CACHE_MAX_GB_ENV: &str = "RIPR_LANE1_EVIDENCE_AUDIT_MAX_CACHE_GB";
const LANE1_EVIDENCE_AUDIT_DEFAULT_CACHE_MAX_GB: u64 = 20;
const LANE1_BYTES_PER_GB: u64 = 1024 * 1024 * 1024;
const LANE1_EVIDENCE_AUDIT_SAMPLE_SEAMS_ENV: &str = "RIPR_LANE1_EVIDENCE_AUDIT_SAMPLE_SEAMS";
const LANE1_EVIDENCE_AUDIT_SAMPLE_SEAM_LIMIT: usize = 5_000;
const REPO_EXPOSURE_SEAM_LIMIT_ENV: &str = "RIPR_REPO_EXPOSURE_SEAM_LIMIT";
pub(crate) const CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY: &str =
    "cross_language_target_unresolved";
pub(crate) const CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE: &str =
    "analysis/cross-language-test-target-inference";
pub(crate) const CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY: &str =
    "cross_language_oracle_visibility_unresolved";
pub(crate) const CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE: &str =
    "analysis/cross-language-oracle-visibility";
pub(crate) const EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED: &str =
    "evidence_quality_scorecard_audit_regeneration_failed";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditReport {
    pub(crate) root: String,
    pub(crate) repo_exposure_schema_version: Option<String>,
    pub(crate) repo_exposure_generation: Option<Lane1EvidenceAuditRepoExposureGeneration>,
    pub(crate) run_limitations: Vec<Lane1EvidenceAuditRunLimitation>,
    pub(crate) summary: Lane1EvidenceAuditSummary,
    pub(crate) finding_alignment: Lane1EvidenceAuditFindingAlignmentSummary,
    pub(crate) alignment_coverage_by_class: Vec<Lane1EvidenceAuditAlignmentClassCoverage>,
    pub(crate) unaligned_raw_findings_by_class: BTreeMap<String, usize>,
    pub(crate) top_unaligned_examples: Vec<Lane1EvidenceAuditUnalignedExample>,
    pub(crate) same_line_duplicate_groups: Vec<Lane1EvidenceAuditSameLineDuplicateGroup>,
    pub(crate) evidence_class_work_queue: Vec<Lane1EvidenceClassWorkItem>,
    pub(crate) static_unknown_without_named_limitation: usize,
    pub(crate) canonical_items_without_repair_route: usize,
    pub(crate) canonical_items_without_verify_command: usize,
    pub(crate) actionable_gap_top_lists: Lane1EvidenceAuditActionableGapTopLists,
    pub(crate) actionable_gap_packets: Vec<Lane1ActionableGapPacket>,
    pub(crate) runtime_confidence_by_class: Vec<Lane1EvidenceAuditRuntimeConfidenceClassCoverage>,
    pub(crate) largest_canonical_groups: Vec<Lane1EvidenceAuditGroup>,
    pub(crate) duplicate_looking_groups: Vec<Lane1EvidenceAuditGroup>,
    pub(crate) missing_discriminator_reason_counts: BTreeMap<String, usize>,
    pub(crate) missing_discriminator_flow_sink_counts: BTreeMap<String, usize>,
    pub(crate) missing_discriminator_value_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_reason_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_stage_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_category_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_repair_route_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_backlog_packets: Vec<Lane1StaticLimitationBacklogPacket>,
    pub(crate) oracle_semantics_counts: BTreeMap<String, usize>,
    pub(crate) oracle_kind_counts: BTreeMap<String, usize>,
    pub(crate) oracle_strength_counts: BTreeMap<String, usize>,
    pub(crate) related_test_confidence_counts: BTreeMap<String, usize>,
    pub(crate) top_related_test_confidence_counts: BTreeMap<String, usize>,
    pub(crate) top_related_test_reason_counts: BTreeMap<String, usize>,
    pub(crate) movement_availability: Lane1EvidenceAuditMovement,
    pub(crate) calibration_availability_counts: BTreeMap<String, usize>,
    pub(crate) calibration_confidence_counts: BTreeMap<String, usize>,
    pub(crate) calibration_agreement_counts: BTreeMap<String, usize>,
    pub(crate) evidence_record_field_health: Vec<Lane1EvidenceAuditFieldHealth>,
    pub(crate) top_files_by_unresolved_evidence_debt: Vec<Lane1EvidenceAuditFileDebt>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditActionableGapTopLists {
    pub(crate) top_actionable_gap_classes: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_actionable_files: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_repair_kinds: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_missing_discriminator_kinds: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_static_limitation_reasons: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_verify_command_unknowns: Vec<Lane1EvidenceAuditTopCount>,
    pub(crate) top_repair_route_unknowns: Vec<Lane1EvidenceAuditTopCount>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditTopCount {
    pub(crate) label: String,
    pub(crate) count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1StaticLimitationBacklogPacket {
    pub(crate) packet_id: String,
    pub(crate) limitation_category: String,
    pub(crate) limitation_subroute: String,
    pub(crate) repair_route: String,
    pub(crate) signal_count: usize,
    pub(crate) sample_canonical_gap_ids: Vec<String>,
    pub(crate) sample_sources: Vec<Lane1StaticLimitationBacklogSample>,
    pub(crate) dominant_evidence_class: String,
    pub(crate) why_not_actionable: String,
    pub(crate) unlock_condition: String,
    pub(crate) non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1StaticLimitationBacklogSample {
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) evidence_class: String,
    pub(crate) source_file: String,
    pub(crate) line: Option<usize>,
    pub(crate) expression: Option<String>,
    pub(crate) limitation_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1StaticLimitationBacklogPacketBuilder {
    pub(crate) limitation_category: String,
    pub(crate) limitation_subroute: String,
    pub(crate) signal_count: usize,
    pub(crate) repair_route_counts: BTreeMap<String, usize>,
    pub(crate) evidence_class_counts: BTreeMap<String, usize>,
    pub(crate) sample_canonical_gap_ids: BTreeSet<String>,
    pub(crate) sample_sources: Vec<Lane1StaticLimitationBacklogSample>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1ActionableGapPacket {
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) gap_state: String,
    pub(crate) actionability: String,
    pub(crate) source_file: String,
    pub(crate) primary_anchor: Value,
    pub(crate) repair_kind: String,
    pub(crate) target_test_type: String,
    pub(crate) assertion_shape: String,
    pub(crate) repair_route: Option<Value>,
    pub(crate) target_test_shape: String,
    pub(crate) recommended_repair: String,
    pub(crate) why: String,
    pub(crate) related_test_or_observer: Option<Value>,
    pub(crate) candidate_value_or_observer: Option<String>,
    pub(crate) missing_discriminators: Vec<Value>,
    pub(crate) verify_command: String,
    pub(crate) repair_route_source: String,
    pub(crate) verify_command_source: String,
    pub(crate) receipt_command: Option<String>,
    pub(crate) receipt_command_or_path: Option<String>,
    pub(crate) receipt_source: String,
    pub(crate) public_projection_eligible: bool,
    pub(crate) projection_exclusion_reasons: Vec<String>,
    pub(crate) raw_evidence_refs: Vec<Value>,
    pub(crate) raw_findings: Vec<Value>,
    pub(crate) raw_findings_supporting_only: bool,
    pub(crate) static_limitations: Vec<Value>,
    pub(crate) confidence_basis: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) allowed_edit_surface: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditRepoExposureGeneration {
    pub(crate) command: String,
    pub(crate) timeout_ms: u128,
    pub(crate) status: String,
    pub(crate) failure_reason: Option<String>,
    pub(crate) duration_ms: u128,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) latency_trace_events_total: usize,
    pub(crate) latency_trace_tail: Vec<RepoExposureLatencyTrace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Lane1EvidenceAuditRepoExposureOutcome {
    Complete(Lane1EvidenceAuditRepoExposureGeneration),
    TimedOut(Lane1EvidenceAuditRepoExposureGeneration),
    FailedIncomplete(Lane1EvidenceAuditRepoExposureGeneration),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditRunLimitation {
    pub(crate) category: String,
    pub(crate) phase: String,
    pub(crate) input: String,
    pub(crate) observed_seams: Option<usize>,
    pub(crate) cache_limit: Option<usize>,
    pub(crate) summary: String,
    pub(crate) repair_route: String,
    pub(crate) timeout_ms: Option<u128>,
    pub(crate) duration_ms: Option<u128>,
    pub(crate) command: Option<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout_bytes: Option<usize>,
    pub(crate) stderr_bytes: Option<usize>,
    pub(crate) latency_trace_tail: Vec<RepoExposureLatencyTrace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1RuntimeStatus {
    pub(crate) state: String,
    pub(crate) phase: Option<String>,
    pub(crate) duration_ms: Option<u128>,
    pub(crate) limit_ms: Option<u128>,
    pub(crate) input_kind: Option<String>,
    pub(crate) input_path: Option<String>,
    pub(crate) limitation_category: Option<String>,
    pub(crate) repair_route: Option<String>,
    pub(crate) downstream_consumable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditSummary {
    pub(crate) seams_total: usize,
    pub(crate) raw_headline_gaps: usize,
    pub(crate) evidence_records_total: usize,
    pub(crate) evidence_records_missing: usize,
    pub(crate) canonical_gap_groups_total: usize,
    pub(crate) duplicate_looking_groups_total: usize,
    pub(crate) headline_without_canonical_gap_id: usize,
    pub(crate) missing_discriminators_total: usize,
    pub(crate) static_limitations_total: usize,
    pub(crate) related_tests_total: usize,
    pub(crate) seams_without_related_tests: usize,
    pub(crate) low_or_opaque_top_related_tests: usize,
    pub(crate) calibrated_records: usize,
    pub(crate) uncalibrated_records: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditFindingAlignmentSummary {
    pub(crate) raw_findings_total: usize,
    pub(crate) raw_signals_total: usize,
    pub(crate) canonical_items_total: usize,
    pub(crate) aligned_raw_findings_total: usize,
    pub(crate) unaligned_raw_findings_total: usize,
    pub(crate) duplicate_groups_total: usize,
    pub(crate) actionable_items_total: usize,
    pub(crate) actionable_unresolved_canonical_gaps: usize,
    pub(crate) already_observed_total: usize,
    pub(crate) internal_only_total: usize,
    pub(crate) internal_no_action_total: usize,
    pub(crate) static_limitation_total: usize,
    pub(crate) unknown_total: usize,
    pub(crate) calibrated_supported_total: usize,
    pub(crate) uncalibrated_total: usize,
    pub(crate) visibility_unknown_total: usize,
    pub(crate) presentation_text_actionable_total: usize,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditAlignmentClassCoverage {
    pub(crate) evidence_class: String,
    pub(crate) raw_findings: usize,
    pub(crate) canonical_items: usize,
    pub(crate) aligned_raw_findings: usize,
    pub(crate) unaligned_raw_findings: usize,
    pub(crate) actionable_items: usize,
    pub(crate) already_observed_items: usize,
    pub(crate) internal_no_action_items: usize,
    pub(crate) static_limitation_items: usize,
    pub(crate) unknown_items: usize,
    pub(crate) static_limitation_categories: BTreeMap<String, usize>,
    pub(crate) static_limitation_repair_routes: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceClassWorkItem {
    pub(crate) evidence_class: String,
    pub(crate) work_score: usize,
    pub(crate) dominant_signal: String,
    pub(crate) dominant_static_limitation_category: Option<String>,
    pub(crate) dominant_static_limitation_category_count: usize,
    pub(crate) dominant_static_limitation_repair_route: Option<String>,
    pub(crate) raw_findings: usize,
    pub(crate) canonical_items: usize,
    pub(crate) duplicate_raw_signals: usize,
    pub(crate) actionable_items: usize,
    pub(crate) static_limitation_items: usize,
    pub(crate) unknown_items: usize,
    pub(crate) unaligned_raw_findings: usize,
    pub(crate) next_repair: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditRuntimeConfidenceClassCoverage {
    pub(crate) evidence_class: String,
    pub(crate) canonical_items: usize,
    pub(crate) calibrated_supported: usize,
    pub(crate) fixture_backed: usize,
    pub(crate) static_only: usize,
    pub(crate) unknown_confidence: usize,
    pub(crate) uncalibrated: usize,
    pub(crate) actionable_items: usize,
    pub(crate) static_limitation_items: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditUnalignedExample {
    pub(crate) evidence_class: String,
    pub(crate) file: String,
    pub(crate) line: Option<usize>,
    pub(crate) kind: String,
    pub(crate) expression: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditSameLineDuplicateGroup {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) raw_findings: usize,
    pub(crate) evidence_classes: Vec<String>,
    pub(crate) kinds: Vec<String>,
    pub(crate) example_expression: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditSameLineDuplicateBuilder {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) raw_findings: usize,
    pub(crate) evidence_classes: BTreeSet<String>,
    pub(crate) kinds: BTreeSet<String>,
    pub(crate) example_expression: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditMovement {
    pub(crate) records_with_seam_id: usize,
    pub(crate) records_with_canonical_gap_id: usize,
    pub(crate) records_with_complete_evidence_path: usize,
    pub(crate) records_with_recommendation: usize,
    pub(crate) records_with_verify_command: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditGroup {
    pub(crate) key: String,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) count: usize,
    pub(crate) reported_group_size: Option<usize>,
    pub(crate) owner: Option<String>,
    pub(crate) seam_kind: Option<String>,
    pub(crate) flow_sink: Option<String>,
    pub(crate) missing_discriminator: Option<String>,
    pub(crate) assertion_shape: Option<String>,
    pub(crate) example_seam_id: Option<String>,
    pub(crate) example_file: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditFieldHealth {
    pub(crate) field: String,
    pub(crate) present: usize,
    pub(crate) missing: usize,
    pub(crate) null: usize,
    pub(crate) empty: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1EvidenceAuditFileDebt {
    pub(crate) file: String,
    pub(crate) debt_score: usize,
    pub(crate) headline_gaps: usize,
    pub(crate) missing_discriminators: usize,
    pub(crate) static_limitations: usize,
    pub(crate) unknown_stage_records: usize,
    pub(crate) no_related_tests: usize,
    pub(crate) low_or_opaque_top_related_tests: usize,
    pub(crate) missing_evidence_records: usize,
}

/// Generate the Lane 1 evidence quality audit from the current repo exposure
/// data and write `target/ripr/reports/lane1-evidence-audit.{json,md}`.
pub(crate) fn lane1_evidence_audit_report_impl() -> Result<(), String> {
    ensure_reports_dir()?;
    let report = if let Some(limitation) =
        lane1_repo_exposure_large_cache_preflight_limitation(Path::new("."))?
    {
        lane1_evidence_audit_limited_report_from_run_limitation(".", limitation)
    } else {
        let repo_exposure_path = reports_dir().join("lane1-evidence-audit.repo-exposure.json");
        match write_lane1_evidence_audit_repo_exposure(&repo_exposure_path)? {
            Lane1EvidenceAuditRepoExposureOutcome::Complete(repo_exposure_generation) => {
                let report = lane1_evidence_audit_report_from_complete_repo_exposure(
                    ".",
                    &repo_exposure_path,
                    repo_exposure_generation,
                );
                if let Err(err) = fs::remove_file(&repo_exposure_path) {
                    eprintln!(
                        "warning: failed to remove temporary Lane 1 repo exposure input {}: {err}",
                        repo_exposure_path.display()
                    );
                }
                report
            }
            Lane1EvidenceAuditRepoExposureOutcome::TimedOut(repo_exposure_generation) => {
                lane1_evidence_audit_limited_report(".", repo_exposure_generation)
            }
            Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(repo_exposure_generation) => {
                lane1_evidence_audit_limited_report(".", repo_exposure_generation)
            }
        }
    };
    write_report(
        "lane1-evidence-audit.json",
        &lane1_evidence_audit_json(&report)?,
    )?;
    write_report(
        "lane1-evidence-audit.md",
        &lane1_evidence_audit_markdown(&report),
    )?;
    write_report(
        "actionable-gaps.json",
        &lane1_actionable_gap_packets_json(&report)?,
    )?;
    write_report(
        "actionable-gaps.md",
        &lane1_actionable_gap_packets_markdown(&report),
    )
}

pub(crate) fn lane1_evidence_audit_limited_report_from_run_limitation(
    root: &str,
    limitation: Lane1EvidenceAuditRunLimitation,
) -> Lane1EvidenceAuditReport {
    let mut report = Lane1EvidenceAuditBuilder::default().finish(root.to_string(), None);
    lane1_evidence_audit_record_run_limitation(&mut report, limitation);
    report
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lane1CacheFootprint {
    pub(crate) bytes: u64,
    pub(crate) files: usize,
}

fn lane1_repo_exposure_large_cache_preflight_limitation(
    root: &Path,
) -> Result<Option<Lane1EvidenceAuditRunLimitation>, String> {
    let Some(max_bytes) = lane1_evidence_audit_cache_max_bytes() else {
        return Ok(None);
    };
    lane1_repo_exposure_large_cache_preflight_limitation_for_root(root, max_bytes)
}

pub(crate) fn lane1_repo_exposure_large_cache_preflight_limitation_for_root(
    root: &Path,
    max_bytes: u64,
) -> Result<Option<Lane1EvidenceAuditRunLimitation>, String> {
    let cache_root = lane1_repo_exposure_cache_root(root);
    let Some(footprint) = lane1_cache_footprint(&cache_root)? else {
        return Ok(None);
    };
    if footprint.bytes <= max_bytes {
        return Ok(None);
    }
    let max_gb = max_bytes.div_ceil(LANE1_BYTES_PER_GB).max(1);
    let repair_route = format!(
        "run cargo xtask cache report && cargo xtask cache gc --dry-run --max-size-gb {max_gb} --ttl-days 14, then review and rerun without --dry-run before lane1-evidence-audit"
    );
    Ok(Some(Lane1EvidenceAuditRunLimitation {
        category: "lane1_repo_exposure_large_cache_preflight_skip".to_string(),
        phase: "repo_seam_facts_cache".to_string(),
        input: normalize_report_path(&cache_root.display().to_string()),
        observed_seams: None,
        cache_limit: None,
        summary: format!(
            "Lane 1 repo-exposure generation was skipped because target/ripr/cache contains {} bytes across {} files, above the configured {} byte cache budget. No user test debt is claimed from this limited artifact.",
            footprint.bytes, footprint.files, max_bytes
        ),
        repair_route,
        timeout_ms: None,
        duration_ms: None,
        command: Some("cargo xtask lane1-evidence-audit".to_string()),
        exit_code: None,
        stdout_bytes: None,
        stderr_bytes: None,
        latency_trace_tail: Vec::new(),
    }))
}

fn lane1_evidence_audit_cache_max_bytes() -> Option<u64> {
    let max_gb = std::env::var(LANE1_EVIDENCE_AUDIT_CACHE_MAX_GB_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(LANE1_EVIDENCE_AUDIT_DEFAULT_CACHE_MAX_GB);
    if max_gb == 0 {
        None
    } else {
        Some(max_gb.saturating_mul(LANE1_BYTES_PER_GB))
    }
}

fn lane1_repo_exposure_cache_root(root: &Path) -> PathBuf {
    let cache_root = Path::new("target").join("ripr").join("cache");
    if root == Path::new(".") {
        cache_root
    } else {
        root.join(cache_root)
    }
}

fn lane1_cache_footprint(cache_root: &Path) -> Result<Option<Lane1CacheFootprint>, String> {
    if !cache_root.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(cache_root)
        .map_err(|err| format!("failed to inspect {}: {err}", cache_root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", cache_root.display()));
    }
    let mut footprint = Lane1CacheFootprint::default();
    let mut stack = vec![cache_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|err| format!("failed to read cache directory {}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to read cache directory entry in {}: {err}",
                    dir.display()
                )
            })?;
            let file_type = entry.file_type().map_err(|err| {
                format!(
                    "failed to inspect cache entry {}: {err}",
                    entry.path().display()
                )
            })?;
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                let metadata = entry.metadata().map_err(|err| {
                    format!(
                        "failed to inspect cache file {}: {err}",
                        entry.path().display()
                    )
                })?;
                footprint.bytes = footprint.bytes.saturating_add(metadata.len());
                footprint.files += 1;
            }
        }
    }
    Ok(Some(footprint))
}

pub(crate) fn lane1_evidence_audit_report_from_complete_repo_exposure(
    root: &str,
    path: &Path,
    generation: Lane1EvidenceAuditRepoExposureGeneration,
) -> Lane1EvidenceAuditReport {
    match lane1_evidence_audit_from_repo_exposure_file(root, path) {
        Ok(mut report) => {
            if let Some(limitation) = lane1_repo_exposure_cache_store_limitation(&generation) {
                lane1_evidence_audit_record_run_limitation(&mut report, limitation);
            }
            if let Some(limitation) = lane1_repo_exposure_sample_limit_limitation(&generation) {
                lane1_evidence_audit_record_run_limitation(&mut report, limitation);
            }
            report.repo_exposure_generation = Some(generation);
            report
        }
        Err(err) => {
            eprintln!(
                "warning: failed to parse captured Lane 1 repo exposure {} after status {}: {err}",
                path.display(),
                generation.status
            );
            let status = if generation.status == "pass" {
                "pass_incomplete"
            } else {
                "complete_parse_failed"
            };
            lane1_evidence_audit_limited_report(
                root,
                Lane1EvidenceAuditRepoExposureGeneration {
                    status: status.to_string(),
                    failure_reason: Some(err),
                    ..generation
                },
            )
        }
    }
}

fn write_lane1_evidence_audit_repo_exposure(
    path: &Path,
) -> Result<Lane1EvidenceAuditRepoExposureOutcome, String> {
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = ripr_debug_binary();
    let timeout = Duration::from_millis(lane1_evidence_audit_timeout_ms());
    write_lane1_evidence_audit_repo_exposure_with_runner(
        path,
        &binary,
        timeout,
        lane1_evidence_audit_run_repo_exposure,
    )
}

pub(crate) fn lane1_evidence_audit_repo_exposure_args() -> Vec<String> {
    vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--mode".to_string(),
        "instant".to_string(),
        "--format".to_string(),
        "repo-exposure-json".to_string(),
    ]
}

fn lane1_evidence_audit_timeout_ms() -> u64 {
    lane1_evidence_audit_timeout_ms_from_env(std::env::var(LANE1_EVIDENCE_AUDIT_TIMEOUT_ENV).ok())
}

pub(crate) fn lane1_evidence_audit_timeout_ms_from_env(value: Option<String>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(LANE1_EVIDENCE_AUDIT_DEFAULT_TIMEOUT_MS)
}

fn lane1_evidence_audit_sample_seam_limit() -> Option<usize> {
    match std::env::var(LANE1_EVIDENCE_AUDIT_SAMPLE_SEAMS_ENV) {
        Ok(value) if value.trim() == "0" => None,
        Ok(value) => parse_lane1_evidence_audit_sample_seam_limit(&value)
            .or(Some(LANE1_EVIDENCE_AUDIT_SAMPLE_SEAM_LIMIT)),
        Err(_) => Some(LANE1_EVIDENCE_AUDIT_SAMPLE_SEAM_LIMIT),
    }
}

pub(crate) fn parse_lane1_evidence_audit_sample_seam_limit(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|limit| (limit > 0).then_some(limit))
}

fn lane1_evidence_audit_run_repo_exposure(
    binary: &Path,
    args: &[String],
    path: &Path,
    timeout: Duration,
) -> Result<TimedFileOutput, String> {
    let binary_text = binary.display().to_string();
    let seam_limit = lane1_evidence_audit_sample_seam_limit().map(|limit| limit.to_string());
    let mut envs = vec![(REPO_EXPOSURE_LATENCY_TRACE_ENV, "1")];
    if let Some(seam_limit) = seam_limit.as_deref() {
        envs.push((REPO_EXPOSURE_SEAM_LIMIT_ENV, seam_limit));
    }
    capture_stdout_to_file_with_timeout(
        &binary_text,
        args,
        &envs,
        path,
        timeout,
        "Lane 1 evidence audit repo exposure",
    )
}

pub(crate) fn write_lane1_evidence_audit_repo_exposure_with_runner<F>(
    path: &Path,
    binary: &Path,
    timeout: Duration,
    mut run_repo_exposure: F,
) -> Result<Lane1EvidenceAuditRepoExposureOutcome, String>
where
    F: FnMut(&Path, &[String], &Path, Duration) -> Result<TimedFileOutput, String>,
{
    let args = lane1_evidence_audit_repo_exposure_args();
    let started = Instant::now();
    let output = match run_repo_exposure(binary, &args, path, timeout) {
        Ok(output) => output,
        Err(err) => {
            let _ = fs::remove_file(path);
            return Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                lane1_evidence_audit_repo_exposure_runner_error(
                    binary,
                    &args,
                    timeout,
                    started.elapsed(),
                    err,
                ),
            ));
        }
    };
    if output.timed_out {
        let diagnostics =
            lane1_evidence_audit_repo_exposure_generation(binary, &args, timeout, &output);
        return match lane1_repo_exposure_file_looks_complete(path) {
            Ok(true) => {
                eprintln!(
                    "warning: repo exposure generation hit the timeout after writing complete JSON; continuing from {}",
                    path.display()
                );
                Ok(Lane1EvidenceAuditRepoExposureOutcome::Complete(
                    Lane1EvidenceAuditRepoExposureGeneration {
                        status: "timeout_complete".to_string(),
                        ..diagnostics
                    },
                ))
            }
            Ok(false) => {
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::TimedOut(diagnostics))
            }
            Err(inspect_err) => {
                eprintln!(
                    "warning: failed to inspect captured repo exposure {} after timeout: {inspect_err}",
                    path.display()
                );
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::TimedOut(
                    Lane1EvidenceAuditRepoExposureGeneration {
                        failure_reason: Some(inspect_err),
                        ..diagnostics
                    },
                ))
            }
        };
    }

    let diagnostics =
        lane1_evidence_audit_repo_exposure_generation(binary, &args, timeout, &output);
    match output.status {
        Some(status) if status.success() => match lane1_repo_exposure_file_looks_complete(path) {
            Ok(true) => Ok(Lane1EvidenceAuditRepoExposureOutcome::Complete(diagnostics)),
            Ok(false) => {
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                    Lane1EvidenceAuditRepoExposureGeneration {
                        status: "pass_incomplete".to_string(),
                        failure_reason: Some(
                            "repo exposure exited successfully but captured JSON was incomplete"
                                .to_string(),
                        ),
                        ..diagnostics
                    },
                ))
            }
            Err(inspect_err) => {
                eprintln!(
                    "warning: failed to inspect captured repo exposure {} after {status}: {inspect_err}",
                    path.display()
                );
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                    Lane1EvidenceAuditRepoExposureGeneration {
                        status: "pass_incomplete".to_string(),
                        failure_reason: Some(inspect_err),
                        ..diagnostics
                    },
                ))
            }
        },
        Some(status) => match lane1_repo_exposure_file_looks_complete(path) {
            Ok(true) => {
                eprintln!(
                    "warning: repo exposure generation returned non-zero status; continuing because {} contains a complete repo-exposure JSON document",
                    path.display()
                );
                Ok(Lane1EvidenceAuditRepoExposureOutcome::Complete(
                    Lane1EvidenceAuditRepoExposureGeneration {
                        status: "nonzero_complete".to_string(),
                        ..diagnostics
                    },
                ))
            }
            Ok(false) => {
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                    diagnostics,
                ))
            }
            Err(inspect_err) => {
                eprintln!(
                    "warning: failed to inspect captured repo exposure {} after {status}: {inspect_err}",
                    path.display()
                );
                let _ = fs::remove_file(path);
                Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                    diagnostics,
                ))
            }
        },
        None => {
            let _ = fs::remove_file(path);
            Ok(Lane1EvidenceAuditRepoExposureOutcome::FailedIncomplete(
                Lane1EvidenceAuditRepoExposureGeneration {
                    status: "missing_exit_status".to_string(),
                    ..diagnostics
                },
            ))
        }
    }
}

pub(crate) fn lane1_evidence_audit_limited_report(
    root: &str,
    generation: Lane1EvidenceAuditRepoExposureGeneration,
) -> Lane1EvidenceAuditReport {
    let mut report = Lane1EvidenceAuditBuilder::default().finish(root.to_string(), None);
    report.repo_exposure_generation = Some(generation.clone());
    lane1_evidence_audit_record_run_limitation(
        &mut report,
        lane1_limited_repo_exposure_run_limitation(&generation),
    );
    report
}

fn lane1_limited_repo_exposure_run_limitation(
    generation: &Lane1EvidenceAuditRepoExposureGeneration,
) -> Lane1EvidenceAuditRunLimitation {
    let limitation = lane1_limited_repo_exposure_limitation(generation);
    Lane1EvidenceAuditRunLimitation {
        category: limitation.category.to_string(),
        phase: "repo_exposure_generation".to_string(),
        input: "repo-exposure-json".to_string(),
        observed_seams: None,
        cache_limit: None,
        summary: limitation.summary.to_string(),
        repair_route: limitation.repair_route.to_string(),
        timeout_ms: Some(generation.timeout_ms),
        duration_ms: Some(generation.duration_ms),
        command: Some(generation.command.clone()),
        exit_code: generation.exit_code,
        stdout_bytes: Some(generation.stdout_bytes),
        stderr_bytes: Some(generation.stderr_bytes),
        latency_trace_tail: generation.latency_trace_tail.clone(),
    }
}

fn lane1_repo_exposure_sample_limit_limitation(
    generation: &Lane1EvidenceAuditRepoExposureGeneration,
) -> Option<Lane1EvidenceAuditRunLimitation> {
    let trace = generation
        .latency_trace_tail
        .iter()
        .rev()
        .find(|trace| trace.phase == "repo_exposure_seam_limit")?;
    Some(Lane1EvidenceAuditRunLimitation {
        category: "lane1_repo_exposure_sampled".to_string(),
        phase: "repo_exposure_generation".to_string(),
        input: format!("repo-exposure-json:{}", trace.status),
        observed_seams: None,
        cache_limit: None,
        summary: format!(
            "Lane 1 repo-exposure analyzed the bounded seam sample recorded as {}; counts are useful partial evidence and must not be treated as full-repo debt totals.",
            trace.status
        ),
        repair_route:
            "use the sampled work queue for the next analyzer narrowing slice; set RIPR_LANE1_EVIDENCE_AUDIT_SAMPLE_SEAMS=0 only when full-repo counts are required and the run can be allowed to take longer"
                .to_string(),
        timeout_ms: Some(generation.timeout_ms),
        duration_ms: Some(generation.duration_ms),
        command: Some(generation.command.clone()),
        exit_code: generation.exit_code,
        stdout_bytes: Some(generation.stdout_bytes),
        stderr_bytes: Some(generation.stderr_bytes),
        latency_trace_tail: generation.latency_trace_tail.clone(),
    })
}

fn lane1_evidence_audit_record_run_limitation(
    report: &mut Lane1EvidenceAuditReport,
    limitation: Lane1EvidenceAuditRunLimitation,
) {
    let repair_route = "report/lane1-audit-bounded-diagnostics";
    report.summary.static_limitations_total += 1;
    audit_increment(
        &mut report.static_limitation_category_counts,
        &limitation.category,
    );
    audit_increment(
        &mut report.static_limitation_repair_route_counts,
        repair_route,
    );
    report.run_limitations.push(limitation);
}

pub(crate) fn lane1_runtime_status_full() -> Lane1RuntimeStatus {
    Lane1RuntimeStatus {
        state: "full".to_string(),
        phase: None,
        duration_ms: None,
        limit_ms: None,
        input_kind: None,
        input_path: None,
        limitation_category: None,
        repair_route: None,
        downstream_consumable: true,
    }
}

fn lane1_runtime_status_for_report(
    limitations: &[Lane1EvidenceAuditRunLimitation],
) -> Lane1RuntimeStatus {
    limitations
        .iter()
        .map(lane1_runtime_status_from_run_limitation)
        .min_by_key(|status| lane1_runtime_status_priority(&status.state))
        .unwrap_or_else(lane1_runtime_status_full)
}

fn lane1_runtime_status_from_run_limitation(
    limitation: &Lane1EvidenceAuditRunLimitation,
) -> Lane1RuntimeStatus {
    Lane1RuntimeStatus {
        state: lane1_runtime_state_for_limitation_category(&limitation.category).to_string(),
        phase: Some(limitation.phase.clone()),
        duration_ms: limitation.duration_ms,
        limit_ms: limitation.timeout_ms,
        input_kind: Some(lane1_runtime_input_kind(limitation)),
        input_path: lane1_runtime_input_path(limitation),
        limitation_category: Some(limitation.category.clone()),
        repair_route: Some(limitation.repair_route.clone()),
        downstream_consumable: lane1_runtime_limitation_downstream_consumable(&limitation.category),
    }
}

fn lane1_runtime_status_from_limitation_value(limitation: &Value) -> Option<Lane1RuntimeStatus> {
    let category = audit_string(limitation, &["category"])?;
    Some(Lane1RuntimeStatus {
        state: lane1_runtime_state_for_limitation_category(&category).to_string(),
        phase: audit_string(limitation, &["phase"]),
        duration_ms: audit_u128(limitation, &["duration_ms"]),
        limit_ms: audit_u128(limitation, &["limit_ms"])
            .or_else(|| audit_u128(limitation, &["timeout_ms"])),
        input_kind: audit_string(limitation, &["input_kind"])
            .or_else(|| audit_string(limitation, &["input"]))
            .map(|input| lane1_runtime_input_kind_from_parts(&category, None, &input)),
        input_path: audit_string(limitation, &["input_path"]),
        limitation_category: Some(category.clone()),
        repair_route: audit_string(limitation, &["repair_route"]),
        downstream_consumable: audit_bool(limitation, &["downstream_consumable"])
            .unwrap_or_else(|| lane1_runtime_limitation_downstream_consumable(&category)),
    })
}

pub(crate) fn lane1_runtime_status_from_report_value(value: &Value) -> Option<Lane1RuntimeStatus> {
    if let Some(status) = value
        .get("run_limitations")
        .and_then(Value::as_array)
        .and_then(|limitations| {
            limitations
                .iter()
                .filter_map(lane1_runtime_status_from_limitation_value)
                .min_by_key(|status| lane1_runtime_status_priority(&status.state))
        })
        && status.state != "full"
    {
        return Some(status);
    }
    if let Some(status) = audit_get(value, &["runtime_status"])
        .and_then(lane1_runtime_status_from_runtime_status_value)
    {
        return Some(status);
    }
    None
}

fn lane1_runtime_status_from_runtime_status_value(value: &Value) -> Option<Lane1RuntimeStatus> {
    let state = audit_string(value, &["state"])?;
    Some(Lane1RuntimeStatus {
        downstream_consumable: audit_bool(value, &["downstream_consumable"])
            .unwrap_or(state == "full"),
        state,
        phase: audit_string(value, &["phase"]),
        duration_ms: audit_u128(value, &["duration_ms"]),
        limit_ms: audit_u128(value, &["limit_ms"]),
        input_kind: audit_string(value, &["input_kind"]),
        input_path: audit_string(value, &["input_path"]),
        limitation_category: audit_string(value, &["limitation_category"]),
        repair_route: audit_string(value, &["repair_route"]),
    })
}

pub(crate) fn lane1_runtime_status_with_input_path(
    mut status: Lane1RuntimeStatus,
    phase: &str,
    input_path: &str,
) -> Lane1RuntimeStatus {
    if status.phase.is_none() {
        status.phase = Some(phase.to_string());
    }
    if status.input_path.is_none() {
        status.input_path = Some(input_path.to_string());
    }
    status
}

pub(crate) fn lane1_runtime_status_limited_input(
    phase: &str,
    input_kind: &str,
    input_path: Option<&str>,
    category: &str,
    repair_route: &str,
    downstream_consumable: bool,
) -> Lane1RuntimeStatus {
    Lane1RuntimeStatus {
        state: lane1_runtime_state_for_limitation_category(category).to_string(),
        phase: Some(phase.to_string()),
        duration_ms: None,
        limit_ms: None,
        input_kind: Some(input_kind.to_string()),
        input_path: input_path.map(str::to_string),
        limitation_category: Some(category.to_string()),
        repair_route: Some(repair_route.to_string()),
        downstream_consumable,
    }
}

fn lane1_runtime_state_for_limitation_category(category: &str) -> &'static str {
    match category {
        "lane1_repo_exposure_timeout" | "evidence_health_timeout" => "limited_timeout",
        "lane1_repo_exposure_runner_error" | "evidence_health_runner_error" => {
            "limited_runner_failure"
        }
        "lane1_repo_exposure_cache_store_skipped_large_entry"
        | "lane1_repo_exposure_large_cache_preflight_skip" => "limited_large_cache_skip",
        "limited_stale_input" => "limited_stale_input",
        "lane1_repo_exposure_sampled" => "limited_sampled_input",
        "lane1_repo_exposure_incomplete"
        | EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED
        | EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE
        | "swarm_plan_input_unavailable"
        | "actionable_gap_outcomes_input_unavailable" => "limited_incomplete_input",
        _ => "limited_incomplete_input",
    }
}

fn lane1_runtime_limitation_downstream_consumable(category: &str) -> bool {
    matches!(
        category,
        "lane1_repo_exposure_cache_store_skipped_large_entry"
    )
}

pub(crate) fn lane1_runtime_status_priority(state: &str) -> u8 {
    match state {
        "limited_timeout" => 0,
        "limited_runner_failure" => 1,
        "limited_incomplete_input" => 2,
        "limited_sampled_input" => 3,
        "limited_stale_input" => 4,
        "limited_large_cache_skip" => 5,
        "full" => 9,
        _ => 8,
    }
}

fn lane1_runtime_input_kind(limitation: &Lane1EvidenceAuditRunLimitation) -> String {
    lane1_runtime_input_kind_from_parts(
        &limitation.category,
        Some(&limitation.phase),
        &limitation.input,
    )
}

fn lane1_runtime_input_kind_from_parts(category: &str, phase: Option<&str>, input: &str) -> String {
    if matches!(
        category,
        "lane1_repo_exposure_cache_store_skipped_large_entry"
            | "lane1_repo_exposure_large_cache_preflight_skip"
    ) {
        return "repo-seam-facts-cache".to_string();
    }
    if phase == Some("repo_exposure_generation") || input.starts_with("repo-exposure-json") {
        return "repo-exposure-json".to_string();
    }
    input.to_string()
}

pub(crate) fn lane1_runtime_status_json(status: &Lane1RuntimeStatus) -> Value {
    serde_json::json!({
        "state": status.state,
        "phase": status.phase,
        "duration_ms": status.duration_ms,
        "limit_ms": status.limit_ms,
        "input_kind": status.input_kind,
        "input_path": status.input_path,
        "limitation_category": status.limitation_category,
        "repair_route": status.repair_route,
        "downstream_consumable": status.downstream_consumable,
    })
}

pub(crate) fn lane1_runtime_status_push_markdown(
    out: &mut String,
    runtime_status: &Lane1RuntimeStatus,
) {
    out.push_str("## Runtime Status\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| State | `{}` |\n",
        audit_markdown_cell(&runtime_status.state)
    ));
    out.push_str(&format!(
        "| Phase | `{}` |\n",
        audit_markdown_cell(runtime_status.phase.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Duration ms | `{}` |\n",
        runtime_status
            .duration_ms
            .map(|value| value.to_string())
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "| Limit ms | `{}` |\n",
        runtime_status
            .limit_ms
            .map(|value| value.to_string())
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "| Input kind | `{}` |\n",
        audit_markdown_cell(runtime_status.input_kind.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Input path | `{}` |\n",
        audit_markdown_cell(runtime_status.input_path.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Limitation category | `{}` |\n",
        audit_markdown_cell(runtime_status.limitation_category.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Repair route | {} |\n",
        audit_markdown_cell(runtime_status.repair_route.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Downstream consumable | `{}` |\n\n",
        runtime_status.downstream_consumable
    ));
}

fn lane1_runtime_input_path(limitation: &Lane1EvidenceAuditRunLimitation) -> Option<String> {
    if limitation.category == "lane1_repo_exposure_large_cache_preflight_skip" {
        Some(limitation.input.clone())
    } else {
        None
    }
}

fn audit_u128(value: &Value, path: &[&str]) -> Option<u128> {
    audit_get(value, path)
        .and_then(Value::as_u64)
        .map(u128::from)
}

pub(crate) fn lane1_repo_exposure_cache_store_limitation(
    generation: &Lane1EvidenceAuditRepoExposureGeneration,
) -> Option<Lane1EvidenceAuditRunLimitation> {
    let trace = generation.latency_trace_tail.iter().rev().find(|trace| {
        trace.phase == "cache_store"
            && trace
                .status
                .starts_with("ignored_skipped_large_entry_seams_")
    })?;
    let parsed = lane1_parse_cache_store_skip_status(&trace.status);
    let input = match parsed {
        Some((observed_seams, cache_limit)) => {
            format!("classified_seams_{observed_seams}_limit_{cache_limit}")
        }
        None => trace
            .status
            .strip_prefix("ignored_skipped_large_entry_seams_")
            .map(|suffix| format!("classified_seams_{suffix}"))
            .unwrap_or_else(|| trace.status.clone()),
    };
    let summary = match parsed {
        Some((observed_seams, cache_limit)) => format!(
            "Lane 1 repo-exposure skipped the full classified seam cache store because the observed classified seam count ({observed_seams}) exceeded the configured full-cache store limit ({cache_limit}); evidence was still emitted, but later full audit runs may cold-compute until the full cache path is narrowed or configured for this repo."
        ),
        None => "Lane 1 repo-exposure skipped the full classified seam cache store because the classified seam count exceeded the bounded full-cache store limit; evidence was still emitted, but later full audit runs may cold-compute until the full cache path is narrowed or configured for this repo.".to_string(),
    };
    let repair_route = match parsed {
        Some((observed_seams, _)) => format!(
            "run cargo xtask cache report, then set RIPR_REPO_SEAM_CACHE_LIMIT={observed_seams} or higher only when disk and time budget allow; otherwise keep the run limited until cache sharding or payload narrowing lands"
        ),
        None => "run cargo xtask cache report, then configure RIPR_REPO_SEAM_CACHE_LIMIT only when disk and time budget allow; otherwise keep the run limited until cache sharding or payload narrowing lands".to_string(),
    };

    Some(Lane1EvidenceAuditRunLimitation {
        category: "lane1_repo_exposure_cache_store_skipped_large_entry".to_string(),
        phase: "repo_exposure_cache_store".to_string(),
        input,
        observed_seams: parsed.map(|(observed_seams, _)| observed_seams),
        cache_limit: parsed.map(|(_, cache_limit)| cache_limit),
        summary,
        repair_route,
        timeout_ms: Some(generation.timeout_ms),
        duration_ms: Some(generation.duration_ms),
        command: Some(generation.command.clone()),
        exit_code: generation.exit_code,
        stdout_bytes: Some(generation.stdout_bytes),
        stderr_bytes: Some(generation.stderr_bytes),
        latency_trace_tail: generation.latency_trace_tail.clone(),
    })
}

pub(crate) fn lane1_parse_cache_store_skip_status(status: &str) -> Option<(usize, usize)> {
    let suffix = status.strip_prefix("ignored_skipped_large_entry_seams_")?;
    let (observed, limit) = suffix.split_once("_limit_")?;
    Some((observed.parse().ok()?, limit.parse().ok()?))
}

pub(crate) struct Lane1LimitedRepoExposureLimitation {
    pub(crate) category: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) repair_route: &'static str,
}

fn lane1_limited_repo_exposure_limitation(
    generation: &Lane1EvidenceAuditRepoExposureGeneration,
) -> Lane1LimitedRepoExposureLimitation {
    if generation.status == "timeout" {
        return Lane1LimitedRepoExposureLimitation {
            category: "lane1_repo_exposure_timeout",
            summary: "Lane 1 repo-exposure generation exceeded its bounded runtime; partial repo-exposure JSON was discarded and no user test debt is claimed from this limited artifact.",
            repair_route: "inspect repo-exposure latency trace, increase RIPR_LANE1_EVIDENCE_AUDIT_TIMEOUT_MS for slower machines, or add fixture-backed analyzer narrowing for the slow phase",
        };
    }
    if generation.status == "runner_error" {
        return Lane1LimitedRepoExposureLimitation {
            category: "lane1_repo_exposure_runner_error",
            summary: "Lane 1 repo-exposure generation could not be started or captured; no partial repo-exposure JSON was accepted and no user test debt is claimed from this limited artifact.",
            repair_route: "inspect repo-exposure command availability, report directory permissions, captured failure_reason, and runner environment; rerun lane1-evidence-audit after fixing the invocation or capture path",
        };
    }
    Lane1LimitedRepoExposureLimitation {
        category: "lane1_repo_exposure_incomplete",
        summary: "Lane 1 repo-exposure generation ended before producing complete repo-exposure JSON; partial repo-exposure JSON was discarded and no user test debt is claimed from this limited artifact.",
        repair_route: "inspect repo-exposure exit status, stderr, and latency trace; rerun lane1-evidence-audit; or add fixture-backed analyzer narrowing for the failing phase",
    }
}

fn lane1_evidence_audit_repo_exposure_runner_error(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    duration: Duration,
    err: String,
) -> Lane1EvidenceAuditRepoExposureGeneration {
    Lane1EvidenceAuditRepoExposureGeneration {
        command: format!("{} {}", binary.display(), args.join(" ")),
        timeout_ms: timeout.as_millis(),
        status: "runner_error".to_string(),
        failure_reason: Some(err),
        duration_ms: duration.as_millis(),
        exit_code: None,
        stdout_bytes: 0,
        stderr_bytes: 0,
        latency_trace_events_total: 0,
        latency_trace_tail: Vec::new(),
    }
}

fn lane1_evidence_audit_repo_exposure_generation(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    output: &TimedFileOutput,
) -> Lane1EvidenceAuditRepoExposureGeneration {
    let trace = repo_exposure_latency_trace(&output.stderr);
    let trace_tail_start = trace
        .len()
        .saturating_sub(LANE1_EVIDENCE_AUDIT_TRACE_TAIL_LIMIT);
    let mut latency_trace_tail = trace[trace_tail_start..].to_vec();
    for sampled in trace
        .iter()
        .filter(|trace| trace.phase == "repo_exposure_seam_limit")
    {
        if !latency_trace_tail
            .iter()
            .any(|tail| tail.phase == sampled.phase && tail.status == sampled.status)
        {
            latency_trace_tail.insert(0, sampled.clone());
        }
    }
    Lane1EvidenceAuditRepoExposureGeneration {
        command: format!("{} {}", binary.display(), args.join(" ")),
        timeout_ms: timeout.as_millis(),
        status: if output.timed_out {
            "timeout".to_string()
        } else if output.status.is_some_and(|status| status.success()) {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        failure_reason: None,
        duration_ms: output.duration.as_millis(),
        exit_code: output.status.and_then(|status| status.code()),
        stdout_bytes: output.stdout_bytes,
        stderr_bytes: output.stderr.len(),
        latency_trace_events_total: trace.len(),
        latency_trace_tail,
    }
}

#[cfg(test)]
pub(crate) fn lane1_evidence_audit_timeout_error(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    output: &TimedFileOutput,
) -> String {
    let timeout_ms = timeout.as_millis();
    let mut message = format!(
        "{} {} timed out after {timeout_ms} ms while generating Lane 1 repo exposure; no partial repo-exposure JSON was accepted.",
        binary.display(),
        args.join(" ")
    );
    let trace = repo_exposure_latency_trace(&output.stderr);
    if !trace.is_empty() {
        message.push_str("\nlast latency trace:");
        let start = trace.len().saturating_sub(8);
        for entry in &trace[start..] {
            message.push_str(&format!(
                "\n- phase={} status={} duration_ms={}",
                entry.phase, entry.status, entry.duration_ms
            ));
        }
    } else if !output.stderr.trim().is_empty() {
        message.push_str("\nstderr tail:");
        for line in output
            .stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            message.push('\n');
            message.push_str(line);
        }
    }
    message
}

pub(crate) fn lane1_repo_exposure_file_looks_complete(path: &Path) -> Result<bool, String> {
    let file = fs::File::open(path).map_err(|err| {
        format!(
            "failed to open captured repo exposure {}: {err}",
            path.display()
        )
    })?;
    let value: Value = match serde_json::from_reader(BufReader::new(file)) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    Ok(value.get("seams").and_then(Value::as_array).is_some())
}

fn lane1_evidence_audit_from_repo_exposure_file(
    root: &str,
    path: &Path,
) -> Result<Lane1EvidenceAuditReport, String> {
    let file = fs::File::open(path).map_err(|err| {
        format!(
            "failed to open temporary Lane 1 repo exposure input {}: {err}",
            path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut builder = Lane1EvidenceAuditBuilder::new(root);
    let mut schema_version = None;
    let mut saw_seams_array = false;

    for line in reader.lines() {
        let line = line.map_err(|err| {
            format!(
                "failed to read temporary Lane 1 repo exposure input {}: {err}",
                path.display()
            )
        })?;
        if schema_version.is_none() {
            schema_version = audit_schema_version_from_line(&line);
        }
        if line.trim_start().starts_with("\"seams\":") {
            saw_seams_array = true;
        }
        let Some(record_json) = audit_evidence_record_json_from_line(&line) else {
            continue;
        };
        let record = serde_json::from_str::<Value>(record_json).map_err(|err| {
            format!(
                "failed to parse evidence_record from {}: {err}",
                path.display()
            )
        })?;
        let seam = serde_json::json!({ "evidence_record": record });
        builder.ingest_seam(&seam);
    }

    if !saw_seams_array {
        return Err("repo exposure JSON is missing `seams` array".to_string());
    }

    Ok(builder.finish(root.to_string(), schema_version))
}

#[cfg(test)]
pub(crate) fn lane1_evidence_audit_from_repo_exposure(
    root: &str,
    repo_exposure_json: &str,
) -> Result<Lane1EvidenceAuditReport, String> {
    let value: Value = serde_json::from_str(repo_exposure_json)
        .map_err(|err| format!("failed to parse repo exposure JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "repo exposure JSON is missing `seams` array".to_string())?;

    let mut builder = Lane1EvidenceAuditBuilder::new(root);
    for seam in seams {
        builder.ingest_seam(seam);
    }

    Ok(builder.finish(root.to_string(), audit_string(&value, &["schema_version"])))
}

#[derive(Default)]
pub(crate) struct Lane1EvidenceAuditBuilder {
    pub(crate) workspace_root: Option<PathBuf>,
    pub(crate) summary: Lane1EvidenceAuditSummary,
    pub(crate) finding_alignment: Lane1EvidenceAuditFindingAlignmentSummary,
    pub(crate) movement: Lane1EvidenceAuditMovement,
    pub(crate) alignment_class_coverage: BTreeMap<String, Lane1EvidenceAuditAlignmentClassCoverage>,
    pub(crate) unaligned_raw_findings_by_class: BTreeMap<String, usize>,
    pub(crate) unaligned_examples: Vec<Lane1EvidenceAuditUnalignedExample>,
    pub(crate) same_line_raw_findings: BTreeMap<String, Lane1EvidenceAuditSameLineDuplicateBuilder>,
    pub(crate) static_unknown_without_named_limitation: usize,
    pub(crate) canonical_items_without_repair_route: usize,
    pub(crate) canonical_items_without_verify_command: usize,
    pub(crate) actionable_gap_class_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_file_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_repair_kind_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_missing_discriminator_kind_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_static_limitation_reason_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_verify_command_unknown_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_repair_route_unknown_counts: BTreeMap<String, usize>,
    pub(crate) actionable_gap_packets: BTreeMap<String, Lane1ActionableGapPacket>,
    pub(crate) runtime_confidence_by_class:
        BTreeMap<String, Lane1EvidenceAuditRuntimeConfidenceClassCoverage>,
    pub(crate) canonical_groups: BTreeMap<String, Lane1EvidenceAuditGroup>,
    pub(crate) duplicate_groups: BTreeMap<String, Lane1EvidenceAuditGroup>,
    pub(crate) field_health: BTreeMap<String, Lane1EvidenceAuditFieldHealth>,
    pub(crate) file_debt: BTreeMap<String, Lane1EvidenceAuditFileDebt>,
    pub(crate) missing_reason_counts: BTreeMap<String, usize>,
    pub(crate) missing_flow_sink_counts: BTreeMap<String, usize>,
    pub(crate) missing_value_counts: BTreeMap<String, usize>,
    pub(crate) static_reason_counts: BTreeMap<String, usize>,
    pub(crate) static_stage_counts: BTreeMap<String, usize>,
    pub(crate) static_category_counts: BTreeMap<String, usize>,
    pub(crate) static_repair_route_counts: BTreeMap<String, usize>,
    pub(crate) static_limitation_backlog_packet_builders:
        BTreeMap<String, Lane1StaticLimitationBacklogPacketBuilder>,
    pub(crate) oracle_semantics_counts: BTreeMap<String, usize>,
    pub(crate) oracle_kind_counts: BTreeMap<String, usize>,
    pub(crate) oracle_strength_counts: BTreeMap<String, usize>,
    pub(crate) related_confidence_counts: BTreeMap<String, usize>,
    pub(crate) top_related_confidence_counts: BTreeMap<String, usize>,
    pub(crate) top_related_reason_counts: BTreeMap<String, usize>,
    pub(crate) calibration_availability_counts: BTreeMap<String, usize>,
    pub(crate) calibration_confidence_counts: BTreeMap<String, usize>,
    pub(crate) calibration_agreement_counts: BTreeMap<String, usize>,
}

impl Lane1EvidenceAuditBuilder {
    pub(crate) fn new(root: &str) -> Self {
        Self {
            workspace_root: Some(PathBuf::from(root)),
            ..Self::default()
        }
    }

    fn ingest_seam(&mut self, seam: &Value) {
        self.summary.seams_total += 1;
        let record = seam
            .get("evidence_record")
            .filter(|value| value.is_object());
        let headline = record
            .and_then(|record| audit_bool(record, &["headline_eligible"]))
            .or_else(|| audit_bool(seam, &["headline_eligible"]))
            .unwrap_or(false);
        if headline {
            self.summary.raw_headline_gaps += 1;
        }

        let file = record
            .and_then(|record| audit_string(record, &["location", "file"]))
            .or_else(|| audit_string(seam, &["file"]))
            .unwrap_or_else(|| "unknown".to_string());

        let Some(record) = record else {
            self.summary.evidence_records_missing += 1;
            let debt = audit_file_debt(&mut self.file_debt, &file);
            debt.debt_score += 1;
            debt.missing_evidence_records += 1;
            return;
        };

        self.summary.evidence_records_total += 1;
        audit_evidence_record_field_health(record, &mut self.field_health);
        audit_ingest_finding_alignment(record, &mut self.finding_alignment);
        self.ingest_alignment_coverage(record, &file);

        let seam_id = audit_string(record, &["seam_id"]);
        let canonical_gap_id = audit_string(record, &["canonical_gap_id"]);
        let owner = audit_string(record, &["owner"]);
        let seam_kind = audit_string(record, &["seam_kind"]);
        let assertion_shape = audit_string(record, &["recommendation", "assertion_shape", "kind"]);
        let group_size = audit_usize(record, &["canonical_gap_group_size"]);

        if seam_id.is_some() {
            self.movement.records_with_seam_id += 1;
        }
        if canonical_gap_id.is_some() {
            self.movement.records_with_canonical_gap_id += 1;
        } else if headline {
            self.summary.headline_without_canonical_gap_id += 1;
        }
        if audit_evidence_path_complete(record) {
            self.movement.records_with_complete_evidence_path += 1;
        }
        if audit_string(record, &["recommendation", "action"]).is_some() {
            self.movement.records_with_recommendation += 1;
        }
        if audit_string(record, &["recommendation", "verify_command"]).is_some() {
            self.movement.records_with_verify_command += 1;
        }

        let debt = audit_file_debt(&mut self.file_debt, &file);
        if headline {
            debt.debt_score += 1;
            debt.headline_gaps += 1;
        }

        let missing = audit_array(record, &["missing_discriminators"]);
        self.summary.missing_discriminators_total += missing.len();
        debt.debt_score += missing.len();
        debt.missing_discriminators += missing.len();
        let missing_signature = audit_missing_discriminator_signature(missing);
        let flow_sink = missing.iter().find_map(|missing| {
            audit_string(missing, &["flow_sink", "kind"])
                .or_else(|| audit_string(missing, &["flow_sink"]))
        });
        for missing in missing {
            let reason =
                audit_string(missing, &["reason"]).unwrap_or_else(|| "missing_reason".to_string());
            let value =
                audit_string(missing, &["value"]).unwrap_or_else(|| "missing_value".to_string());
            let sink = audit_string(missing, &["flow_sink", "kind"])
                .or_else(|| audit_string(missing, &["flow_sink"]))
                .unwrap_or_else(|| "no_flow_sink".to_string());
            audit_increment(&mut self.missing_reason_counts, &reason);
            audit_increment(&mut self.missing_value_counts, &value);
            audit_increment(&mut self.missing_flow_sink_counts, &sink);
        }

        let static_limitations = audit_array(record, &["static_limitations"]);
        let canonical_item =
            audit_get(record, &["canonical_item"]).filter(|value| value.is_object());
        let evidence_class = audit_alignment_evidence_class(record, canonical_item);
        let sample_canonical_gap_id = canonical_item
            .and_then(|item| audit_non_empty_string(item, &["canonical_gap_id"]))
            .or_else(|| canonical_gap_id.clone());
        let raw_findings = audit_array(record, &["raw_findings"]);
        let sample_line = audit_usize(record, &["location", "line"]).or_else(|| {
            raw_findings
                .first()
                .and_then(|raw| audit_usize(raw, &["line"]))
        });
        let sample_expression = raw_findings
            .first()
            .and_then(|raw| audit_non_empty_string(raw, &["expression"]));
        let mut static_limitation_backlog_samples = Vec::new();
        self.summary.static_limitations_total += static_limitations.len();
        debt.debt_score += static_limitations.len();
        debt.static_limitations += static_limitations.len();
        for limitation in static_limitations {
            let reason = audit_string(limitation, &["reason"])
                .unwrap_or_else(|| "missing_reason".to_string());
            let stage =
                audit_string(limitation, &["stage"]).unwrap_or_else(|| "missing_stage".to_string());
            let state =
                audit_string(limitation, &["state"]).unwrap_or_else(|| "missing_state".to_string());
            let category = audit_string(limitation, &["category"])
                .unwrap_or_else(|| static_limitation_category(&stage, &state, &reason).to_string());
            let base_repair_route = audit_string(limitation, &["repair_route"])
                .unwrap_or_else(|| static_limitation_repair_route(&category).to_string());
            let subroute = static_limitation_subroute(
                record,
                limitation,
                &category,
                &base_repair_route,
                &evidence_class,
            );
            let repair_route = static_limitation_repair_route_for_subroute(
                &category,
                &subroute,
                &base_repair_route,
            )
            .to_string();
            audit_increment(&mut self.static_reason_counts, &reason);
            audit_increment(&mut self.static_stage_counts, &stage);
            audit_increment(&mut self.static_category_counts, &category);
            audit_increment(&mut self.static_repair_route_counts, &repair_route);
            static_limitation_backlog_samples.push((category, subroute, repair_route, reason));
        }

        let unknown_stage_count = audit_unknown_stage_count(record);
        debt.debt_score += unknown_stage_count;
        debt.unknown_stage_records += unknown_stage_count;

        let related_tests = audit_array(record, &["related_tests"]);
        let related_tests_total =
            audit_usize(record, &["related_tests_total"]).unwrap_or(related_tests.len());
        self.summary.related_tests_total += related_tests_total;
        if related_tests_total == 0 {
            self.summary.seams_without_related_tests += 1;
            debt.debt_score += 1;
            debt.no_related_tests += 1;
        }
        if let Some(top_related) = related_tests.first() {
            let confidence = audit_string(top_related, &["relation_confidence"])
                .unwrap_or_else(|| "missing".to_string());
            let reason = audit_string(top_related, &["relation_reason"])
                .unwrap_or_else(|| "missing".to_string());
            audit_increment(&mut self.top_related_confidence_counts, &confidence);
            audit_increment(&mut self.top_related_reason_counts, &reason);
            if matches!(confidence.as_str(), "low" | "opaque") {
                self.summary.low_or_opaque_top_related_tests += 1;
                debt.debt_score += 1;
                debt.low_or_opaque_top_related_tests += 1;
            }
        }
        for related in related_tests {
            let confidence = audit_string(related, &["relation_confidence"])
                .unwrap_or_else(|| "missing".to_string());
            let oracle_kind =
                audit_string(related, &["oracle_kind"]).unwrap_or_else(|| "missing".to_string());
            let oracle_strength = audit_string(related, &["oracle_strength"])
                .unwrap_or_else(|| "missing".to_string());
            audit_increment(&mut self.related_confidence_counts, &confidence);
            audit_increment(&mut self.oracle_kind_counts, &oracle_kind);
            audit_increment(&mut self.oracle_strength_counts, &oracle_strength);
            audit_increment(
                &mut self.oracle_semantics_counts,
                &audit_oracle_semantics_key(related),
            );
        }

        for (category, subroute, repair_route, reason) in static_limitation_backlog_samples {
            self.ingest_static_limitation_backlog_packet_sample(
                &category,
                &subroute,
                &repair_route,
                Lane1StaticLimitationBacklogSample {
                    canonical_gap_id: sample_canonical_gap_id.clone(),
                    evidence_class: evidence_class.clone(),
                    source_file: file.clone(),
                    line: sample_line,
                    expression: sample_expression.clone(),
                    limitation_reason: Some(reason),
                },
            );
        }

        let availability = audit_string(record, &["calibration", "availability"])
            .unwrap_or_else(|| "missing".to_string());
        let confidence = audit_string(record, &["calibration", "confidence"])
            .unwrap_or_else(|| "missing".to_string());
        let agreement = audit_string(record, &["calibration", "agreement"])
            .unwrap_or_else(|| "missing".to_string());
        audit_increment(&mut self.calibration_availability_counts, &availability);
        audit_increment(&mut self.calibration_confidence_counts, &confidence);
        audit_increment(&mut self.calibration_agreement_counts, &agreement);
        if availability == "not_imported" || availability == "missing" {
            self.summary.uncalibrated_records += 1;
        } else {
            self.summary.calibrated_records += 1;
        }

        if headline {
            if let Some(id) = canonical_gap_id.as_ref() {
                audit_upsert_group(
                    &mut self.canonical_groups,
                    Lane1EvidenceAuditGroup {
                        key: format!("canonical:{id}"),
                        canonical_gap_id: canonical_gap_id.clone(),
                        count: 0,
                        reported_group_size: group_size,
                        owner: owner.clone(),
                        seam_kind: seam_kind.clone(),
                        flow_sink: flow_sink.clone(),
                        missing_discriminator: missing_signature.clone(),
                        assertion_shape: assertion_shape.clone(),
                        example_seam_id: seam_id.clone(),
                        example_file: Some(file.clone()),
                    },
                );
            }
            let duplicate_key = if let Some(id) = canonical_gap_id.as_ref() {
                format!("canonical:{id}")
            } else {
                format!(
                    "fallback:{}|{}|{}|{}|{}",
                    owner.as_deref().unwrap_or("missing_owner"),
                    seam_kind.as_deref().unwrap_or("missing_kind"),
                    flow_sink.as_deref().unwrap_or("missing_flow_sink"),
                    missing_signature
                        .as_deref()
                        .unwrap_or("missing_discriminator"),
                    assertion_shape
                        .as_deref()
                        .unwrap_or("missing_assertion_shape")
                )
            };
            audit_upsert_group(
                &mut self.duplicate_groups,
                Lane1EvidenceAuditGroup {
                    key: duplicate_key,
                    canonical_gap_id: canonical_gap_id.clone(),
                    count: 0,
                    reported_group_size: group_size,
                    owner,
                    seam_kind,
                    flow_sink,
                    missing_discriminator: missing_signature,
                    assertion_shape,
                    example_seam_id: seam_id,
                    example_file: Some(file),
                },
            );
        }
    }

    fn ingest_alignment_coverage(&mut self, record: &Value, file: &str) {
        let raw_findings = audit_array(record, &["raw_findings"]);
        let canonical_item =
            audit_get(record, &["canonical_item"]).filter(|value| value.is_object());
        let evidence_class = audit_alignment_evidence_class(record, canonical_item);
        let raw_signal_count = canonical_item
            .and_then(|item| audit_usize(item, &["raw_group_size"]))
            .unwrap_or(raw_findings.len())
            .max(raw_findings.len())
            .max(1);
        let static_limitation_rows = audit_static_limitation_category_rows(record);
        for raw in raw_findings {
            self.ingest_same_line_raw_finding(record, raw, &evidence_class);
        }

        let Some(canonical_item) = canonical_item else {
            {
                let coverage = self
                    .alignment_class_coverage
                    .entry(evidence_class.clone())
                    .or_insert_with(|| Lane1EvidenceAuditAlignmentClassCoverage {
                        evidence_class: evidence_class.clone(),
                        ..Lane1EvidenceAuditAlignmentClassCoverage::default()
                    });
                coverage.raw_findings += raw_signal_count;
                coverage.unaligned_raw_findings += raw_signal_count;
                audit_ingest_static_limitation_class_coverage(
                    coverage,
                    static_limitation_rows
                        .iter()
                        .map(|(category, repair_route)| (category.as_str(), repair_route.as_str())),
                );
            }
            *self
                .unaligned_raw_findings_by_class
                .entry(evidence_class.clone())
                .or_insert(0) += raw_signal_count;
            let example = audit_unaligned_example(record, raw_findings, &evidence_class);
            if self.unaligned_examples.len() < LANE1_EVIDENCE_AUDIT_TOP_LIMIT {
                self.unaligned_examples.push(example);
            }
            return;
        };

        let item_kind = audit_string(canonical_item, &["canonical_item_kind"]).unwrap_or_default();
        let gap_state = audit_string(canonical_item, &["gap_state"]).unwrap_or_default();
        let actionability = audit_string(canonical_item, &["actionability"]).unwrap_or_default();
        {
            let coverage = self
                .alignment_class_coverage
                .entry(evidence_class.clone())
                .or_insert_with(|| Lane1EvidenceAuditAlignmentClassCoverage {
                    evidence_class: evidence_class.clone(),
                    ..Lane1EvidenceAuditAlignmentClassCoverage::default()
                });
            coverage.raw_findings += raw_signal_count;
            coverage.canonical_items += 1;
            coverage.aligned_raw_findings += raw_signal_count;
            audit_ingest_static_limitation_class_coverage(
                coverage,
                static_limitation_rows
                    .iter()
                    .map(|(category, repair_route)| (category.as_str(), repair_route.as_str())),
            );
            if gap_state == "actionable" {
                coverage.actionable_items += 1;
            }
            if item_kind == "observed" || gap_state == "already_observed" {
                coverage.already_observed_items += 1;
            }
            if item_kind == "no_action" || gap_state == "internal_only" {
                coverage.internal_no_action_items += 1;
            }
            if item_kind == "limitation" || gap_state == "static_limitation" {
                coverage.static_limitation_items += 1;
            }
            if gap_state == "unknown" {
                coverage.unknown_items += 1;
            }
        }
        let actionable = audit_is_actionable_canonical_item(&item_kind, &gap_state);
        if actionable {
            self.ingest_actionable_gap_top_lists(record, canonical_item, &evidence_class, file);
            self.ingest_actionable_gap_packet(record, canonical_item, &evidence_class, file);
        }
        self.ingest_runtime_confidence_by_class(canonical_item, &evidence_class, actionable);
        if actionable && !audit_has_structured_repair_route(canonical_item) {
            self.canonical_items_without_repair_route += 1;
            audit_increment(
                &mut self.actionable_gap_repair_route_unknown_counts,
                &evidence_class,
            );
        }
        if actionable && audit_verify_command_is_missing(canonical_item) {
            self.canonical_items_without_verify_command += 1;
            audit_increment(
                &mut self.actionable_gap_verify_command_unknown_counts,
                &evidence_class,
            );
        }
        if audit_has_static_unknown_signal(record, canonical_item, &gap_state, &actionability)
            && !audit_has_named_static_limitation(record, canonical_item)
        {
            self.static_unknown_without_named_limitation += 1;
        }
    }

    pub(crate) fn ingest_static_limitation_backlog_packet_sample(
        &mut self,
        category: &str,
        subroute: &str,
        repair_route: &str,
        sample: Lane1StaticLimitationBacklogSample,
    ) {
        let entry = self
            .static_limitation_backlog_packet_builders
            .entry(format!("{category}|{subroute}|{repair_route}"))
            .or_insert_with(|| Lane1StaticLimitationBacklogPacketBuilder {
                limitation_category: category.to_string(),
                limitation_subroute: subroute.to_string(),
                ..Lane1StaticLimitationBacklogPacketBuilder::default()
            });
        entry.signal_count += 1;
        audit_increment(&mut entry.repair_route_counts, repair_route);
        audit_increment(&mut entry.evidence_class_counts, &sample.evidence_class);
        if let Some(canonical_gap_id) = sample.canonical_gap_id.as_deref()
            && entry.sample_canonical_gap_ids.len() < 3
        {
            entry
                .sample_canonical_gap_ids
                .insert(canonical_gap_id.to_string());
        }
        if entry.sample_sources.len() < 3 {
            entry.sample_sources.push(sample);
        }
    }

    fn ingest_runtime_confidence_by_class(
        &mut self,
        canonical_item: &Value,
        evidence_class: &str,
        actionable: bool,
    ) {
        let row = self
            .runtime_confidence_by_class
            .entry(evidence_class.to_string())
            .or_insert_with(|| Lane1EvidenceAuditRuntimeConfidenceClassCoverage {
                evidence_class: evidence_class.to_string(),
                ..Lane1EvidenceAuditRuntimeConfidenceClassCoverage::default()
            });
        row.canonical_items += 1;
        if actionable {
            row.actionable_items += 1;
        }
        let item_kind = audit_string(canonical_item, &["canonical_item_kind"]).unwrap_or_default();
        let gap_state = audit_string(canonical_item, &["gap_state"]).unwrap_or_default();
        if item_kind == "limitation" || gap_state == "static_limitation" {
            row.static_limitation_items += 1;
        }

        let confidence_basis =
            audit_string(canonical_item, &["confidence", "basis"]).unwrap_or_default();
        match confidence_basis.as_str() {
            "calibrated" | "runtime_calibrated" => {
                row.calibrated_supported += 1;
            }
            "fixture_backed" => {
                row.fixture_backed += 1;
                row.uncalibrated += 1;
            }
            "static_only" => {
                row.static_only += 1;
                row.uncalibrated += 1;
            }
            _ => {
                row.unknown_confidence += 1;
                row.uncalibrated += 1;
            }
        }
    }

    fn ingest_actionable_gap_top_lists(
        &mut self,
        record: &Value,
        canonical_item: &Value,
        evidence_class: &str,
        file: &str,
    ) {
        audit_increment(&mut self.actionable_gap_class_counts, evidence_class);
        audit_increment(&mut self.actionable_gap_file_counts, file);
        if let Some(repair_kind) =
            audit_non_empty_string(canonical_item, &["repair_route", "repair_kind"])
                .filter(|value| !audit_guidance_field_is_missing(value))
        {
            audit_increment(&mut self.actionable_gap_repair_kind_counts, &repair_kind);
        }
        for missing in audit_array(record, &["missing_discriminators"]) {
            let kind = audit_non_empty_string(missing, &["flow_sink", "kind"])
                .or_else(|| audit_non_empty_string(missing, &["flow_sink"]))
                .or_else(|| audit_non_empty_string(missing, &["reason"]))
                .unwrap_or_else(|| "missing_discriminator_kind_unknown".to_string());
            audit_increment(
                &mut self.actionable_gap_missing_discriminator_kind_counts,
                &kind,
            );
        }
        for limitation in audit_array(record, &["static_limitations"]) {
            let reason = audit_string(limitation, &["reason"])
                .unwrap_or_else(|| "missing_reason".to_string());
            audit_increment(
                &mut self.actionable_gap_static_limitation_reason_counts,
                &reason,
            );
        }
    }

    fn ingest_actionable_gap_packet(
        &mut self,
        record: &Value,
        canonical_item: &Value,
        evidence_class: &str,
        file: &str,
    ) {
        let stable_canonical_gap_id = audit_non_empty_string(canonical_item, &["canonical_gap_id"])
            .or_else(|| audit_non_empty_string(record, &["canonical_gap_id"]));
        let has_stable_canonical_gap_id = stable_canonical_gap_id.is_some();
        let canonical_gap_id = stable_canonical_gap_id
            .or_else(|| audit_non_empty_string(record, &["seam_id"]))
            .unwrap_or_else(|| format!("actionable-gap::{file}"));
        if self.actionable_gap_packets.contains_key(&canonical_gap_id) {
            return;
        }

        let (repair_kind, target_test_type, assertion_shape, repair_route_source, repair_route) =
            audit_actionable_gap_repair_route_fields(record, canonical_item);
        let target_test_shape =
            audit_actionable_gap_target_test_shape(&target_test_type, &assertion_shape);
        let (verify_command, verify_command_source) =
            audit_actionable_gap_verify_command_with_source(record, canonical_item);
        let (receipt_command_or_path, receipt_source) =
            audit_actionable_gap_receipt_command_or_path(record, canonical_item);
        let receipt_command = receipt_command_or_path
            .clone()
            .filter(|_| receipt_source.ends_with(".receipt_command"));
        let related_test_or_observer =
            audit_actionable_gap_related_test_or_observer(record, canonical_item);
        let candidate_value_or_observer =
            audit_actionable_gap_candidate_value_or_observer(record, canonical_item);
        let raw_evidence_refs = audit_json_array_owned(canonical_item, &["raw_evidence_refs"])
            .or_else(|| audit_json_array_owned(canonical_item, &["raw_findings"]))
            .or_else(|| audit_json_array_owned(record, &["raw_evidence_refs"]))
            .or_else(|| audit_json_array_owned(record, &["raw_findings"]))
            .unwrap_or_default();
        let static_limitations = audit_json_array_owned(canonical_item, &["static_limitations"])
            .or_else(|| audit_json_array_owned(record, &["static_limitations"]))
            .unwrap_or_default();
        let cross_language_target_unresolved = audit_static_limitations_has_category(
            &static_limitations,
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY,
        );
        let typed_related_target_available = !cross_language_target_unresolved
            && (related_test_or_observer
                .as_ref()
                .and_then(ripr_swarm_plan_related_target_file)
                .is_some()
                || candidate_value_or_observer
                    .as_ref()
                    .and_then(|candidate| ripr_swarm_attempt_related_target_file(candidate))
                    .is_some());
        let confidence_basis = audit_non_empty_string(canonical_item, &["confidence", "basis"])
            .or_else(|| audit_non_empty_string(record, &["calibration", "confidence"]))
            .unwrap_or_else(|| "unknown".to_string());
        let must_not_change = audit_string_array(canonical_item, &["must_not_change"])
            .or_else(|| audit_string_array(record, &["must_not_change"]))
            .unwrap_or_else(default_actionable_gap_packet_must_not_change);
        let allowed_edit_surface = audit_actionable_gap_allowed_edit_surface(
            self.workspace_root.as_deref(),
            record,
            canonical_item,
            &related_test_or_observer,
            cross_language_target_unresolved,
        );
        let gap_state = audit_non_empty_string(canonical_item, &["gap_state"])
            .unwrap_or_else(|| "gap_state_unknown".to_string());
        let actionability = audit_non_empty_string(canonical_item, &["actionability"])
            .or_else(|| audit_non_empty_string(record, &["actionability", "class"]))
            .unwrap_or_else(|| "actionability_unknown".to_string());
        let projection_exclusion_reasons =
            audit_actionable_gap_projection_exclusion_reasons(AuditActionableGapProjectionInput {
                canonical_gap_id: &canonical_gap_id,
                gap_state: &gap_state,
                actionability: &actionability,
                repair_kind: &repair_kind,
                target_test_type: &target_test_type,
                assertion_shape: &assertion_shape,
                target_test_shape: &target_test_shape,
                repair_route_present: repair_route.is_some(),
                repair_route_source: &repair_route_source,
                verify_command: &verify_command,
                verify_command_source: &verify_command_source,
                receipt_command_or_path: receipt_command_or_path.as_deref(),
                receipt_source: &receipt_source,
                typed_related_target_available,
                has_stable_canonical_gap_id,
                confidence_basis: &confidence_basis,
                must_not_change_count: must_not_change.len(),
                allowed_edit_surface_count: allowed_edit_surface.len(),
                raw_evidence_refs_count: audit_structured_raw_evidence_refs_count(
                    &raw_evidence_refs,
                ),
                static_limitations_count: static_limitations.len(),
                cross_language_target_unresolved,
            });
        let public_projection_eligible = projection_exclusion_reasons.is_empty();

        self.actionable_gap_packets.insert(
            canonical_gap_id.clone(),
            Lane1ActionableGapPacket {
                canonical_gap_id,
                evidence_class: evidence_class.to_string(),
                gap_state,
                actionability,
                source_file: file.to_string(),
                primary_anchor: audit_actionable_gap_primary_anchor(record, canonical_item, file),
                repair_kind,
                target_test_type,
                assertion_shape,
                repair_route,
                target_test_shape,
                recommended_repair: audit_non_empty_string(canonical_item, &["recommended_repair"])
                    .or_else(|| audit_non_empty_string(record, &["recommendation", "reason"]))
                    .unwrap_or_else(|| "recommended_repair_unknown".to_string()),
                why: audit_non_empty_string(canonical_item, &["why"])
                    .unwrap_or_else(|| "why_unknown".to_string()),
                related_test_or_observer,
                candidate_value_or_observer,
                missing_discriminators: audit_actionable_gap_missing_discriminators(
                    record,
                    canonical_item,
                ),
                verify_command,
                repair_route_source,
                verify_command_source,
                receipt_command,
                receipt_command_or_path,
                receipt_source,
                public_projection_eligible,
                projection_exclusion_reasons,
                raw_evidence_refs: raw_evidence_refs.clone(),
                raw_findings: raw_evidence_refs,
                raw_findings_supporting_only: true,
                static_limitations,
                confidence_basis,
                must_not_change,
                allowed_edit_surface,
            },
        );
    }

    fn ingest_same_line_raw_finding(&mut self, record: &Value, raw: &Value, evidence_class: &str) {
        let Some(line) =
            audit_usize(raw, &["line"]).or_else(|| audit_usize(record, &["location", "line"]))
        else {
            return;
        };
        let file = audit_string(raw, &["file"])
            .or_else(|| audit_string(record, &["location", "file"]))
            .unwrap_or_else(|| "unknown".to_string());
        let key = format!("{file}:{line}");
        let entry = self.same_line_raw_findings.entry(key).or_insert_with(|| {
            Lane1EvidenceAuditSameLineDuplicateBuilder {
                file,
                line,
                raw_findings: 0,
                evidence_classes: BTreeSet::new(),
                kinds: BTreeSet::new(),
                example_expression: audit_string(raw, &["expression"]),
            }
        });
        entry.raw_findings += 1;
        entry.evidence_classes.insert(evidence_class.to_string());
        entry.kinds.insert(
            audit_string(raw, &["kind"])
                .or_else(|| audit_string(raw, &["probe_kind"]))
                .unwrap_or_else(|| "unknown".to_string()),
        );
        if entry.example_expression.is_none() {
            entry.example_expression = audit_string(raw, &["expression"]);
        }
    }

    pub(crate) fn finish(
        mut self,
        root: String,
        repo_exposure_schema_version: Option<String>,
    ) -> Lane1EvidenceAuditReport {
        let mut largest_canonical_groups =
            audit_sorted_groups(self.canonical_groups.into_values().collect());
        self.summary.canonical_gap_groups_total = largest_canonical_groups.len();
        largest_canonical_groups.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);

        let mut duplicate_looking_groups = audit_sorted_groups(
            self.duplicate_groups
                .into_values()
                .filter(|group| {
                    group.count > 1 || group.reported_group_size.is_some_and(|size| size > 1)
                })
                .collect(),
        );
        self.summary.duplicate_looking_groups_total = duplicate_looking_groups.len();
        duplicate_looking_groups.truncate(LANE1_EVIDENCE_AUDIT_DUPLICATE_LIMIT);

        let mut field_health = self.field_health.into_values().collect::<Vec<_>>();
        field_health.sort_by(|left, right| left.field.cmp(&right.field));

        let mut file_debt = self.file_debt.into_values().collect::<Vec<_>>();
        file_debt.sort_by(|left, right| {
            right
                .debt_score
                .cmp(&left.debt_score)
                .then_with(|| left.file.cmp(&right.file))
        });
        file_debt.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);

        let mut alignment_coverage_by_class = self
            .alignment_class_coverage
            .into_values()
            .collect::<Vec<_>>();
        alignment_coverage_by_class.sort_by(|left, right| {
            right
                .raw_findings
                .cmp(&left.raw_findings)
                .then_with(|| left.evidence_class.cmp(&right.evidence_class))
        });
        let evidence_class_work_queue =
            audit_evidence_class_work_queue(&alignment_coverage_by_class);

        let mut top_unaligned_examples = self.unaligned_examples;
        top_unaligned_examples.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);

        let mut same_line_duplicate_groups = self
            .same_line_raw_findings
            .into_values()
            .filter(|group| group.raw_findings > 1)
            .map(|group| Lane1EvidenceAuditSameLineDuplicateGroup {
                file: group.file,
                line: group.line,
                raw_findings: group.raw_findings,
                evidence_classes: group.evidence_classes.into_iter().collect(),
                kinds: group.kinds.into_iter().collect(),
                example_expression: group.example_expression,
            })
            .collect::<Vec<_>>();
        same_line_duplicate_groups.sort_by(|left, right| {
            right
                .raw_findings
                .cmp(&left.raw_findings)
                .then_with(|| left.file.cmp(&right.file))
                .then_with(|| left.line.cmp(&right.line))
        });
        same_line_duplicate_groups.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
        let mut all_static_limitation_backlog_packets = self
            .static_limitation_backlog_packet_builders
            .into_values()
            .map(lane1_static_limitation_backlog_packet_from_builder)
            .collect::<Vec<_>>();
        all_static_limitation_backlog_packets.sort_by(|left, right| {
            right
                .signal_count
                .cmp(&left.signal_count)
                .then_with(|| left.limitation_category.cmp(&right.limitation_category))
                .then_with(|| left.repair_route.cmp(&right.repair_route))
        });
        let top_static_limitation_repair_routes =
            audit_top_counts(self.static_repair_route_counts.clone());
        let static_limitation_backlog_packets = lane1_select_static_limitation_backlog_packets(
            &all_static_limitation_backlog_packets,
            &top_static_limitation_repair_routes,
        );

        let actionable_gap_top_lists = Lane1EvidenceAuditActionableGapTopLists {
            top_actionable_gap_classes: audit_top_counts(self.actionable_gap_class_counts),
            top_actionable_files: audit_top_counts(self.actionable_gap_file_counts),
            top_repair_kinds: audit_top_counts(self.actionable_gap_repair_kind_counts),
            top_missing_discriminator_kinds: audit_top_counts(
                self.actionable_gap_missing_discriminator_kind_counts,
            ),
            top_static_limitation_reasons: audit_top_counts(
                self.actionable_gap_static_limitation_reason_counts,
            ),
            top_verify_command_unknowns: audit_top_counts(
                self.actionable_gap_verify_command_unknown_counts,
            ),
            top_repair_route_unknowns: audit_top_counts(
                self.actionable_gap_repair_route_unknown_counts,
            ),
        };
        let mut actionable_gap_packets = self
            .actionable_gap_packets
            .into_values()
            .collect::<Vec<_>>();
        actionable_gap_packets.sort_by(|left, right| {
            left.evidence_class
                .cmp(&right.evidence_class)
                .then_with(|| left.source_file.cmp(&right.source_file))
                .then_with(|| left.canonical_gap_id.cmp(&right.canonical_gap_id))
        });
        actionable_gap_packets.truncate(LANE1_ACTIONABLE_GAP_PACKET_LIMIT);
        let mut runtime_confidence_by_class = self
            .runtime_confidence_by_class
            .into_values()
            .collect::<Vec<_>>();
        runtime_confidence_by_class.sort_by(|left, right| {
            right
                .uncalibrated
                .cmp(&left.uncalibrated)
                .then_with(|| right.canonical_items.cmp(&left.canonical_items))
                .then_with(|| left.evidence_class.cmp(&right.evidence_class))
        });

        Lane1EvidenceAuditReport {
            root,
            repo_exposure_schema_version,
            repo_exposure_generation: None,
            run_limitations: Vec::new(),
            summary: self.summary,
            finding_alignment: self.finding_alignment,
            alignment_coverage_by_class,
            unaligned_raw_findings_by_class: self.unaligned_raw_findings_by_class,
            top_unaligned_examples,
            same_line_duplicate_groups,
            evidence_class_work_queue,
            static_unknown_without_named_limitation: self.static_unknown_without_named_limitation,
            canonical_items_without_repair_route: self.canonical_items_without_repair_route,
            canonical_items_without_verify_command: self.canonical_items_without_verify_command,
            actionable_gap_top_lists,
            actionable_gap_packets,
            runtime_confidence_by_class,
            largest_canonical_groups,
            duplicate_looking_groups,
            missing_discriminator_reason_counts: self.missing_reason_counts,
            missing_discriminator_flow_sink_counts: self.missing_flow_sink_counts,
            missing_discriminator_value_counts: self.missing_value_counts,
            static_limitation_reason_counts: self.static_reason_counts,
            static_limitation_stage_counts: self.static_stage_counts,
            static_limitation_category_counts: self.static_category_counts,
            static_limitation_repair_route_counts: self.static_repair_route_counts,
            static_limitation_backlog_packets,
            oracle_semantics_counts: self.oracle_semantics_counts,
            oracle_kind_counts: self.oracle_kind_counts,
            oracle_strength_counts: self.oracle_strength_counts,
            related_test_confidence_counts: self.related_confidence_counts,
            top_related_test_confidence_counts: self.top_related_confidence_counts,
            top_related_test_reason_counts: self.top_related_reason_counts,
            movement_availability: self.movement,
            calibration_availability_counts: self.calibration_availability_counts,
            calibration_confidence_counts: self.calibration_confidence_counts,
            calibration_agreement_counts: self.calibration_agreement_counts,
            evidence_record_field_health: field_health,
            top_files_by_unresolved_evidence_debt: file_debt,
        }
    }
}

fn audit_schema_version_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let value = trimmed
        .strip_prefix("\"schema_version\":")?
        .trim()
        .trim_end_matches(',');
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
}

fn audit_evidence_record_json_from_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let value = trimmed.strip_prefix("\"evidence_record\":")?.trim();
    Some(value.trim_end_matches(',').trim())
}

pub(crate) fn lane1_evidence_audit_json(
    report: &Lane1EvidenceAuditReport,
) -> Result<String, String> {
    let runtime_status = lane1_runtime_status_for_report(&report.run_limitations);
    let value = serde_json::json!({
        "schema_version": LANE1_EVIDENCE_AUDIT_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "lane1-evidence-audit",
        "scope": "repo",
        "status": "advisory",
        "run_status": runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&runtime_status),
        "inputs": {
            "root": report.root,
            "source": "repo-exposure-json",
            "repo_exposure_mode": "instant",
            "repo_exposure_schema_version": report.repo_exposure_schema_version,
            "repo_exposure_generation": report
                .repo_exposure_generation
                .as_ref()
                .map(lane1_evidence_audit_repo_exposure_generation_json),
        },
        "run_limitations": report
            .run_limitations
            .iter()
            .map(lane1_evidence_audit_run_limitation_json)
            .collect::<Vec<_>>(),
        "static_limitation_backlog": lane1_static_limitation_backlog_json(report),
        "summary": {
            "seams_total": report.summary.seams_total,
            "raw_headline_gaps": report.summary.raw_headline_gaps,
            "evidence_records_total": report.summary.evidence_records_total,
            "evidence_records_missing": report.summary.evidence_records_missing,
            "canonical_gap_groups_total": report.summary.canonical_gap_groups_total,
            "duplicate_looking_groups_total": report.summary.duplicate_looking_groups_total,
            "headline_without_canonical_gap_id": report.summary.headline_without_canonical_gap_id,
            "missing_discriminators_total": report.summary.missing_discriminators_total,
            "static_limitations_total": report.summary.static_limitations_total,
            "related_tests_total": report.summary.related_tests_total,
            "seams_without_related_tests": report.summary.seams_without_related_tests,
            "low_or_opaque_top_related_tests": report.summary.low_or_opaque_top_related_tests,
            "calibrated_records": report.summary.calibrated_records,
            "uncalibrated_records": report.summary.uncalibrated_records,
        },
        "finding_alignment": {
            "source": "evidence_record.canonical_item",
            "summary": audit_finding_alignment_summary_json(&report.finding_alignment),
            "coverage": {
                "alignment_coverage_by_class": report
                    .alignment_coverage_by_class
                    .iter()
                    .map(audit_alignment_class_coverage_json)
                    .collect::<Vec<_>>(),
                "unaligned_raw_findings_by_class": report.unaligned_raw_findings_by_class,
                "top_unaligned_examples": report
                    .top_unaligned_examples
                    .iter()
                    .map(audit_unaligned_example_json)
                    .collect::<Vec<_>>(),
                "same_line_duplicate_groups": report
                    .same_line_duplicate_groups
                    .iter()
                    .map(audit_same_line_duplicate_group_json)
                    .collect::<Vec<_>>(),
                "evidence_class_work_queue": report
                    .evidence_class_work_queue
                    .iter()
                    .map(audit_evidence_class_work_item_json)
                    .collect::<Vec<_>>(),
                "static_unknown_without_named_limitation": report.static_unknown_without_named_limitation,
                "canonical_items_without_repair_route": report.canonical_items_without_repair_route,
                "canonical_items_without_verify_command": report.canonical_items_without_verify_command,
            },
            "actionable_gap_top_lists": audit_actionable_gap_top_lists_json(
                &report.actionable_gap_top_lists
            ),
            "actionable_gap_packets": audit_actionable_gap_packets_json(
                &report.actionable_gap_packets
            ),
            "actionable_gap_packet_public_projection": audit_actionable_gap_packet_public_projection_json(
                &report.actionable_gap_packets
            ),
            "runtime_confidence_by_class": audit_runtime_confidence_by_class_json(
                &report.runtime_confidence_by_class
            ),
        },
        "canonical_gap_groups": {
            "total": report.summary.canonical_gap_groups_total,
            "largest": report.largest_canonical_groups.iter().map(audit_group_json).collect::<Vec<_>>(),
        },
        "duplicate_looking_groups": report
            .duplicate_looking_groups
            .iter()
            .map(audit_group_json)
            .collect::<Vec<_>>(),
        "missing_discriminator_classes": {
            "by_reason": audit_count_rows_json(&report.missing_discriminator_reason_counts),
            "by_flow_sink": report.missing_discriminator_flow_sink_counts,
            "by_value": audit_count_rows_json(&report.missing_discriminator_value_counts),
        },
        "static_limitations": {
            "by_reason": audit_count_rows_json(&report.static_limitation_reason_counts),
            "by_stage": report.static_limitation_stage_counts,
            "by_category": report.static_limitation_category_counts,
            "repair_routes": report.static_limitation_repair_route_counts,
        },
        "oracle_semantics_distribution": {
            "by_semantics": audit_count_rows_json(&report.oracle_semantics_counts),
            "oracle_kind_counts": report.oracle_kind_counts,
            "oracle_strength_counts": report.oracle_strength_counts,
        },
        "related_test_ranking": {
            "all_confidence_counts": report.related_test_confidence_counts,
            "top_confidence_counts": report.top_related_test_confidence_counts,
            "top_relation_reason_counts": report.top_related_test_reason_counts,
            "seams_without_related_tests": report.summary.seams_without_related_tests,
            "low_or_opaque_top_related_tests": report.summary.low_or_opaque_top_related_tests,
        },
        "movement_availability": {
            "records_with_seam_id": report.movement_availability.records_with_seam_id,
            "records_with_canonical_gap_id": report.movement_availability.records_with_canonical_gap_id,
            "records_with_complete_evidence_path": report.movement_availability.records_with_complete_evidence_path,
            "records_with_recommendation": report.movement_availability.records_with_recommendation,
            "records_with_verify_command": report.movement_availability.records_with_verify_command,
        },
        "calibration_availability": {
            "availability_counts": report.calibration_availability_counts,
            "confidence_counts": report.calibration_confidence_counts,
            "agreement_counts": report.calibration_agreement_counts,
            "calibrated_records": report.summary.calibrated_records,
            "uncalibrated_records": report.summary.uncalibrated_records,
            "runtime_confidence_by_class": audit_runtime_confidence_by_class_json(
                &report.runtime_confidence_by_class
            ),
        },
        "evidence_record_field_health": report
            .evidence_record_field_health
            .iter()
            .map(audit_field_health_json)
            .collect::<Vec<_>>(),
        "top_files_by_unresolved_evidence_debt": report
            .top_files_by_unresolved_evidence_debt
            .iter()
            .map(audit_file_debt_json)
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render Lane 1 evidence audit JSON: {err}"))
}

pub(crate) fn lane1_evidence_audit_markdown(report: &Lane1EvidenceAuditReport) -> String {
    let runtime_status = lane1_runtime_status_for_report(&report.run_limitations);
    let mut out = String::new();
    out.push_str("# Lane 1 evidence quality audit\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(&format!("Run status: `{}`\n\n", runtime_status.state));
    out.push_str("This repo-local report summarizes evidence quality from `seams[].evidence_record`. It does not change analyzer behavior, gate policy, PR projection, LSP UX, or runtime execution.\n\n");

    out.push_str("## Runtime Status\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| State | `{}` |\n",
        audit_markdown_cell(&runtime_status.state)
    ));
    out.push_str(&format!(
        "| Downstream consumable | `{}` |\n",
        runtime_status.downstream_consumable
    ));
    if let Some(category) = &runtime_status.limitation_category {
        out.push_str(&format!(
            "| Limitation category | `{}` |\n",
            audit_markdown_cell(category)
        ));
    }
    if let Some(phase) = &runtime_status.phase {
        out.push_str(&format!("| Phase | `{}` |\n", audit_markdown_cell(phase)));
    }
    if let Some(input_kind) = &runtime_status.input_kind {
        out.push_str(&format!(
            "| Input kind | `{}` |\n",
            audit_markdown_cell(input_kind)
        ));
    }
    if let Some(input_path) = &runtime_status.input_path {
        out.push_str(&format!(
            "| Input path | `{}` |\n",
            audit_markdown_cell(input_path)
        ));
    }
    if let Some(duration_ms) = runtime_status.duration_ms {
        out.push_str(&format!("| Duration | {} ms |\n", duration_ms));
    }
    if let Some(limit_ms) = runtime_status.limit_ms {
        out.push_str(&format!("| Limit | {} ms |\n", limit_ms));
    }
    if let Some(repair_route) = &runtime_status.repair_route {
        out.push_str(&format!(
            "| Repair route | {} |\n",
            audit_markdown_cell(repair_route)
        ));
    }
    out.push('\n');

    out.push_str("## Repo Exposure Generation\n\n");
    if let Some(generation) = &report.repo_exposure_generation {
        out.push_str("| Field | Value |\n");
        out.push_str("| --- | --- |\n");
        out.push_str(&format!(
            "| Status | `{}` |\n",
            audit_markdown_cell(&generation.status)
        ));
        if let Some(reason) = &generation.failure_reason {
            out.push_str(&format!(
                "| Failure reason | {} |\n",
                audit_markdown_cell(reason)
            ));
        }
        out.push_str(&format!("| Duration | {} ms |\n", generation.duration_ms));
        out.push_str(&format!("| Timeout | {} ms |\n", generation.timeout_ms));
        let exit = generation
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!("| Exit code | {} |\n", exit));
        out.push_str(&format!("| Stdout bytes | {} |\n", generation.stdout_bytes));
        out.push_str(&format!("| Stderr bytes | {} |\n", generation.stderr_bytes));
        out.push_str(&format!(
            "| Latency trace events | {} |\n",
            generation.latency_trace_events_total
        ));
        out.push('\n');
        if generation.latency_trace_tail.is_empty() {
            out.push_str("No repo-exposure latency trace lines were captured.\n\n");
        } else {
            out.push_str("Last repo-exposure latency trace events:\n\n");
            out.push_str("| Phase | Status | Duration |\n");
            out.push_str("| --- | --- | ---: |\n");
            for trace in &generation.latency_trace_tail {
                out.push_str(&format!(
                    "| `{}` | `{}` | {} ms |\n",
                    audit_markdown_cell(&trace.phase),
                    audit_markdown_cell(&trace.status),
                    trace.duration_ms
                ));
            }
            out.push('\n');
        }
    } else {
        out.push_str("No repo-exposure generation diagnostics were attached. This usually means the audit was built from an in-memory fixture instead of the live repo-exposure subprocess.\n\n");
    }

    if !report.run_limitations.is_empty() {
        out.push_str("## Run Limitations\n\n");
        out.push_str(
            "| Category | Phase | Input | Observed seams | Cache limit | Repair route |\n",
        );
        out.push_str("| --- | --- | --- | ---: | ---: | --- |\n");
        for limitation in &report.run_limitations {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} | {} |\n",
                audit_markdown_cell(&limitation.category),
                audit_markdown_cell(&limitation.phase),
                audit_markdown_cell(&limitation.input),
                limitation
                    .observed_seams
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                limitation
                    .cache_limit
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                audit_markdown_cell(&limitation.repair_route)
            ));
        }
        out.push('\n');
        for limitation in &report.run_limitations {
            if limitation.latency_trace_tail.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "Last latency trace for `{}`:\n\n",
                audit_markdown_cell(&limitation.category)
            ));
            out.push_str("| Phase | Status | Duration |\n");
            out.push_str("| --- | --- | ---: |\n");
            for trace in &limitation.latency_trace_tail {
                out.push_str(&format!(
                    "| `{}` | `{}` | {} ms |\n",
                    audit_markdown_cell(&trace.phase),
                    audit_markdown_cell(&trace.status),
                    trace.duration_ms
                ));
            }
            out.push('\n');
        }
    }

    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(&mut out, "Seams", report.summary.seams_total);
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
        "Seams without related tests",
        report.summary.seams_without_related_tests,
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

    out.push_str("## Finding Alignment\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Raw alignment signals",
        report.finding_alignment.raw_signals_total,
    );
    audit_push_count(
        &mut out,
        "Canonical alignment items",
        report.finding_alignment.canonical_items_total,
    );
    audit_push_count(
        &mut out,
        "Aligned raw findings",
        report.finding_alignment.aligned_raw_findings_total,
    );
    audit_push_count(
        &mut out,
        "Unaligned raw findings",
        report.finding_alignment.unaligned_raw_findings_total,
    );
    if let Some(ratio) = audit_finding_alignment_raw_to_canonical_ratio(&report.finding_alignment) {
        out.push_str(&format!("| Raw-to-canonical ratio | {ratio:.2} |\n"));
    } else {
        out.push_str("| Raw-to-canonical ratio | n/a |\n");
    }
    audit_push_count(
        &mut out,
        "Actionable canonical items",
        report.finding_alignment.actionable_items_total,
    );
    audit_push_count(
        &mut out,
        "Already observed items",
        report.finding_alignment.already_observed_total,
    );
    audit_push_count(
        &mut out,
        "Internal no-action items",
        report.finding_alignment.internal_no_action_total,
    );
    audit_push_count(
        &mut out,
        "Alignment static limitations",
        report.finding_alignment.static_limitation_total,
    );
    audit_push_count(
        &mut out,
        "Alignment uncalibrated items",
        report.finding_alignment.uncalibrated_total,
    );
    audit_push_count(
        &mut out,
        "Presentation text items",
        report.finding_alignment.presentation_text_total,
    );
    out.push('\n');

    out.push_str("## Finding Alignment Coverage\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Static unknown without named limitation",
        report.static_unknown_without_named_limitation,
    );
    audit_push_count(
        &mut out,
        "Canonical items without repair route",
        report.canonical_items_without_repair_route,
    );
    audit_push_count(
        &mut out,
        "Canonical items without verify command",
        report.canonical_items_without_verify_command,
    );
    out.push('\n');
    audit_push_alignment_class_coverage_table(&mut out, &report.alignment_coverage_by_class);
    audit_push_evidence_class_work_queue_table(&mut out, &report.evidence_class_work_queue);
    audit_push_runtime_confidence_by_class_table(&mut out, &report.runtime_confidence_by_class);
    audit_push_counts_table_limited(
        &mut out,
        "Unaligned raw finding class",
        &report.unaligned_raw_findings_by_class,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_unaligned_examples_table(&mut out, &report.top_unaligned_examples);
    audit_push_same_line_duplicate_table(&mut out, &report.same_line_duplicate_groups);

    out.push_str("## Actionable Canonical Gap Top Lists\n\n");
    audit_push_top_count_table(
        &mut out,
        "Actionable gap class",
        &report.actionable_gap_top_lists.top_actionable_gap_classes,
    );
    audit_push_top_count_table(
        &mut out,
        "Actionable file",
        &report.actionable_gap_top_lists.top_actionable_files,
    );
    audit_push_top_count_table(
        &mut out,
        "Repair kind",
        &report.actionable_gap_top_lists.top_repair_kinds,
    );
    audit_push_top_count_table(
        &mut out,
        "Missing discriminator kind",
        &report
            .actionable_gap_top_lists
            .top_missing_discriminator_kinds,
    );
    audit_push_top_count_table(
        &mut out,
        "Static limitation reason",
        &report
            .actionable_gap_top_lists
            .top_static_limitation_reasons,
    );
    audit_push_top_count_table(
        &mut out,
        "Verify command unknown class",
        &report.actionable_gap_top_lists.top_verify_command_unknowns,
    );
    audit_push_top_count_table(
        &mut out,
        "Repair route unknown class",
        &report.actionable_gap_top_lists.top_repair_route_unknowns,
    );

    out.push_str("## Largest Canonical Gap Groups\n\n");
    audit_push_group_table(&mut out, &report.largest_canonical_groups);

    out.push_str("## Duplicate-Looking Groups\n\n");
    audit_push_group_table(&mut out, &report.duplicate_looking_groups);

    out.push_str("## Missing Discriminator Classes\n\n");
    audit_push_counts_table_limited(
        &mut out,
        "Reason",
        &report.missing_discriminator_reason_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Flow sink",
        &report.missing_discriminator_flow_sink_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Static Limitations\n\n");
    audit_push_counts_table_limited(
        &mut out,
        "Category",
        &report.static_limitation_category_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Repair route",
        &report.static_limitation_repair_route_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Reason",
        &report.static_limitation_reason_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Stage",
        &report.static_limitation_stage_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Oracle Semantics\n\n");
    audit_push_counts_table_limited(
        &mut out,
        "Oracle semantics",
        &report.oracle_semantics_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Oracle kind",
        &report.oracle_kind_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Oracle strength",
        &report.oracle_strength_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Related-Test Ranking\n\n");
    audit_push_counts_table_limited(
        &mut out,
        "Top relation confidence",
        &report.top_related_test_confidence_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Top relation reason",
        &report.top_related_test_reason_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Movement Availability\n\n");
    out.push_str("| Field | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Records with seam ID",
        report.movement_availability.records_with_seam_id,
    );
    audit_push_count(
        &mut out,
        "Records with canonical gap ID",
        report.movement_availability.records_with_canonical_gap_id,
    );
    audit_push_count(
        &mut out,
        "Records with complete evidence path",
        report
            .movement_availability
            .records_with_complete_evidence_path,
    );
    audit_push_count(
        &mut out,
        "Records with recommendation",
        report.movement_availability.records_with_recommendation,
    );
    audit_push_count(
        &mut out,
        "Records with verify command",
        report.movement_availability.records_with_verify_command,
    );
    out.push('\n');

    out.push_str("## Calibration Availability\n\n");
    audit_push_counts_table_limited(
        &mut out,
        "Availability",
        &report.calibration_availability_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    audit_push_counts_table_limited(
        &mut out,
        "Agreement",
        &report.calibration_agreement_counts,
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );

    out.push_str("## Evidence Record Field Health\n\n");
    out.push_str("| Field | Present | Missing | Null | Empty |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: |\n");
    for field in &report.evidence_record_field_health {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&field.field),
            field.present,
            field.missing,
            field.null,
            field.empty
        ));
    }
    out.push('\n');

    out.push_str("## Top Files By Unresolved Evidence Debt\n\n");
    if report.top_files_by_unresolved_evidence_debt.is_empty() {
        out.push_str("No unresolved evidence debt was found.\n");
        return out;
    }
    out.push_str("| File | Debt | Headline gaps | Missing discriminators | Static limitations | Unknown stages | No related tests | Low/opaque top related | Missing records |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in &report.top_files_by_unresolved_evidence_debt {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.file),
            row.debt_score,
            row.headline_gaps,
            row.missing_discriminators,
            row.static_limitations,
            row.unknown_stage_records,
            row.no_related_tests,
            row.low_or_opaque_top_related_tests,
            row.missing_evidence_records
        ));
    }
    out
}

fn audit_group_json(group: &Lane1EvidenceAuditGroup) -> Value {
    serde_json::json!({
        "key": group.key,
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
}

fn audit_field_health_json(field: &Lane1EvidenceAuditFieldHealth) -> Value {
    serde_json::json!({
        "field": field.field,
        "present": field.present,
        "missing": field.missing,
        "null": field.null,
        "empty": field.empty,
    })
}

fn audit_file_debt_json(row: &Lane1EvidenceAuditFileDebt) -> Value {
    serde_json::json!({
        "file": row.file,
        "debt_score": row.debt_score,
        "headline_gaps": row.headline_gaps,
        "missing_discriminators": row.missing_discriminators,
        "static_limitations": row.static_limitations,
        "unknown_stage_records": row.unknown_stage_records,
        "no_related_tests": row.no_related_tests,
        "low_or_opaque_top_related_tests": row.low_or_opaque_top_related_tests,
        "missing_evidence_records": row.missing_evidence_records,
    })
}

fn audit_actionable_gap_top_lists_json(
    top_lists: &Lane1EvidenceAuditActionableGapTopLists,
) -> Value {
    serde_json::json!({
        "top_actionable_gap_classes": audit_top_counts_json(&top_lists.top_actionable_gap_classes),
        "top_actionable_files": audit_top_counts_json(&top_lists.top_actionable_files),
        "top_repair_kinds": audit_top_counts_json(&top_lists.top_repair_kinds),
        "top_missing_discriminator_kinds": audit_top_counts_json(
            &top_lists.top_missing_discriminator_kinds
        ),
        "top_static_limitation_reasons": audit_top_counts_json(
            &top_lists.top_static_limitation_reasons
        ),
        "top_verify_command_unknowns": audit_top_counts_json(
            &top_lists.top_verify_command_unknowns
        ),
        "top_repair_route_unknowns": audit_top_counts_json(
            &top_lists.top_repair_route_unknowns
        ),
    })
}

fn audit_actionable_gap_packets_json(packets: &[Lane1ActionableGapPacket]) -> Vec<Value> {
    packets
        .iter()
        .map(audit_actionable_gap_packet_json)
        .collect()
}

fn audit_actionable_gap_packet_json(packet: &Lane1ActionableGapPacket) -> Value {
    serde_json::json!({
        "canonical_gap_id": packet.canonical_gap_id,
        "evidence_class": packet.evidence_class,
        "gap_state": packet.gap_state,
        "actionability": packet.actionability,
        "source_file": packet.source_file,
        "primary_anchor": packet.primary_anchor,
        "repair_kind": packet.repair_kind,
        "target_test_type": packet.target_test_type,
        "assertion_shape": packet.assertion_shape,
        "repair_route": packet.repair_route,
        "target_test_shape": packet.target_test_shape,
        "recommended_repair": packet.recommended_repair,
        "why": packet.why,
        "related_test_or_observer": packet.related_test_or_observer,
        "candidate_value_or_observer": packet.candidate_value_or_observer,
        "missing_discriminators": packet.missing_discriminators,
        "verify_command": packet.verify_command,
        "repair_route_source": packet.repair_route_source,
        "verify_command_source": packet.verify_command_source,
        "receipt_command": packet.receipt_command,
        "receipt_command_or_path": packet.receipt_command_or_path,
        "receipt_source": packet.receipt_source,
        "public_projection_eligible": packet.public_projection_eligible,
        "projection_exclusion_reasons": packet.projection_exclusion_reasons,
        "raw_evidence_refs": packet.raw_evidence_refs,
        "raw_findings": packet.raw_findings,
        "raw_findings_supporting_only": packet.raw_findings_supporting_only,
        "static_limitations": packet.static_limitations,
        "confidence": {
            "basis": packet.confidence_basis,
        },
        "confidence_basis": packet.confidence_basis,
        "must_not_change": packet.must_not_change,
        "allowed_edit_surface": packet.allowed_edit_surface,
    })
}

fn audit_actionable_gap_packet_public_projection_eligible_count(
    packets: &[Lane1ActionableGapPacket],
) -> usize {
    packets
        .iter()
        .filter(|packet| packet.public_projection_eligible)
        .count()
}

fn audit_actionable_gap_packet_projection_exclusion_counts(
    packets: &[Lane1ActionableGapPacket],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for packet in packets {
        for reason in &packet.projection_exclusion_reasons {
            audit_increment(&mut counts, reason);
        }
    }
    counts
}

fn audit_actionable_gap_packet_public_projection_json(
    packets: &[Lane1ActionableGapPacket],
) -> Value {
    let eligible = audit_actionable_gap_packet_public_projection_eligible_count(packets);
    let excluded = packets.len().saturating_sub(eligible);
    serde_json::json!({
        "scope": "emitted_actionable_gap_packets",
        "public_projection_eligible_packets": eligible,
        "public_projection_excluded_packets": excluded,
        "projection_exclusion_reasons": audit_count_rows_json(
            &audit_actionable_gap_packet_projection_exclusion_counts(packets)
        ),
    })
}

pub(crate) fn lane1_actionable_gap_packets_json(
    report: &Lane1EvidenceAuditReport,
) -> Result<String, String> {
    let runtime_status = lane1_runtime_status_for_report(&report.run_limitations);
    let public_projection_eligible_packets =
        audit_actionable_gap_packet_public_projection_eligible_count(
            &report.actionable_gap_packets,
        );
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "actionable-gaps",
        "scope": "repo",
        "status": "advisory",
        "run_status": runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&runtime_status),
        "source_report": "target/ripr/reports/lane1-evidence-audit.json",
        "source": "evidence_record.canonical_item",
        "packet_limit": LANE1_ACTIONABLE_GAP_PACKET_LIMIT,
        "summary": {
            "raw_signals": report.finding_alignment.raw_signals_total,
            "canonical_items": report.finding_alignment.canonical_items_total,
            "actionable_gaps": report.finding_alignment.actionable_items_total,
            "already_observed": report.finding_alignment.already_observed_total,
            "internal_no_action": report.finding_alignment.internal_no_action_total,
            "static_limitations": report.finding_alignment.static_limitation_total,
            "packets_emitted": report.actionable_gap_packets.len(),
            "public_projection_eligible_packets": public_projection_eligible_packets,
            "public_projection_excluded_packets": report
                .actionable_gap_packets
                .len()
                .saturating_sub(public_projection_eligible_packets),
            "projection_exclusion_reasons": audit_count_rows_json(
                &audit_actionable_gap_packet_projection_exclusion_counts(
                    &report.actionable_gap_packets
                )
            ),
            "raw_to_canonical_ratio": audit_finding_alignment_raw_to_canonical_ratio(
                &report.finding_alignment
            ),
            "repair_route_unknowns": report.canonical_items_without_repair_route,
            "verify_command_unknowns": report.canonical_items_without_verify_command,
        },
        "run_limitations": report
            .run_limitations
            .iter()
            .map(lane1_evidence_audit_run_limitation_json)
            .collect::<Vec<_>>(),
        "static_limitation_backlog": lane1_static_limitation_backlog_json(report),
        "packets": audit_actionable_gap_packets_json(&report.actionable_gap_packets),
        "must_not_infer": [
            "raw findings are supporting evidence, not user work",
            "do not infer actionability from raw static class",
            "do not treat named static limitations as user test debt",
            "do not claim mutation execution or runtime proof from this packet"
        ],
    });
    serde_json::to_string_pretty(&value).map_err(|err| err.to_string())
}

pub(crate) fn lane1_static_limitation_backlog_json(report: &Lane1EvidenceAuditReport) -> Value {
    let top_categories = audit_top_counts(report.static_limitation_category_counts.clone())
        .iter()
        .map(|row| {
            let repair_route = lane1_static_limitation_category_repair_route(
                &row.label,
                &report.static_limitation_backlog_packets,
            );
            serde_json::json!({
                "category": row.label,
                "count": row.count,
                "repair_route": repair_route,
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "source": "lane1-evidence-audit.static_limitations",
        "top_categories": top_categories,
        "top_subroutes": lane1_static_limitation_backlog_top_subroutes(
            &report.static_limitation_backlog_packets
        ),
        "top_repair_routes": audit_top_counts(
            lane1_static_limitation_backlog_repair_route_counts(
                &report.static_limitation_backlog_packets,
            )
        )
            .iter()
            .map(|row| {
                serde_json::json!({
                    "repair_route": row.label,
                    "count": row.count,
                })
            })
            .collect::<Vec<_>>(),
        "limitation_backlog_packets": lane1_static_limitation_backlog_packets_json(
            &report.static_limitation_backlog_packets
        ),
    })
}

fn lane1_static_limitation_backlog_repair_route_counts(
    packets: &[Lane1StaticLimitationBacklogPacket],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for packet in packets {
        *counts.entry(packet.repair_route.clone()).or_insert(0) += packet.signal_count;
    }
    counts
}

fn lane1_static_limitation_category_repair_route(
    category: &str,
    packets: &[Lane1StaticLimitationBacklogPacket],
) -> String {
    let mut repair_route_counts = BTreeMap::new();
    for packet in packets
        .iter()
        .filter(|packet| packet.limitation_category == category)
    {
        *repair_route_counts
            .entry(packet.repair_route.clone())
            .or_insert(0) += packet.signal_count;
    }

    audit_top_counts(repair_route_counts)
        .first()
        .map(|row| row.label.clone())
        .unwrap_or_else(|| static_limitation_repair_route(category).to_string())
}

pub(crate) fn static_limitation_repair_route_for_subroute<'a>(
    category: &str,
    subroute: &str,
    fallback: &'a str,
) -> &'a str {
    if category == "activation_owner_call_absent_same_file_only"
        && subroute == "same_file_only_call_presence_receiver_method_missing_owner_call"
    {
        "analysis/same-file-receiver-method-owner-call-tracing"
    } else {
        fallback
    }
}

fn lane1_static_limitation_backlog_packet_from_builder(
    builder: Lane1StaticLimitationBacklogPacketBuilder,
) -> Lane1StaticLimitationBacklogPacket {
    let repair_route = audit_top_counts(builder.repair_route_counts)
        .first()
        .map(|row| row.label.clone())
        .unwrap_or_else(|| {
            static_limitation_repair_route(&builder.limitation_category).to_string()
        });
    let dominant_evidence_class = audit_top_counts(builder.evidence_class_counts)
        .first()
        .map(|row| row.label.clone())
        .unwrap_or_else(|| "unknown".to_string());
    Lane1StaticLimitationBacklogPacket {
        packet_id: static_limitation_backlog_packet_id(
            &builder.limitation_category,
            &builder.limitation_subroute,
            &repair_route,
        ),
        limitation_category: builder.limitation_category.clone(),
        limitation_subroute: builder.limitation_subroute.clone(),
        repair_route: repair_route.clone(),
        signal_count: builder.signal_count,
        sample_canonical_gap_ids: builder.sample_canonical_gap_ids.into_iter().collect(),
        sample_sources: builder.sample_sources,
        dominant_evidence_class,
        why_not_actionable: static_limitation_why_not_actionable(&builder.limitation_category)
            .to_string(),
        unlock_condition: static_limitation_unlock_condition(
            &builder.limitation_category,
            &builder.limitation_subroute,
            &repair_route,
        )
        .to_string(),
        non_claims: static_limitation_backlog_packet_non_claims(
            &builder.limitation_category,
            Some(&builder.limitation_subroute),
        ),
    }
}

fn lane1_select_static_limitation_backlog_packets(
    packets: &[Lane1StaticLimitationBacklogPacket],
    top_repair_routes: &[Lane1EvidenceAuditTopCount],
) -> Vec<Lane1StaticLimitationBacklogPacket> {
    let mut selected = Vec::new();
    let mut selected_packet_ids = BTreeSet::new();

    for packet in packets.iter().take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT) {
        if selected_packet_ids.insert(packet.packet_id.clone()) {
            selected.push(packet.clone());
        }
    }

    for route in top_repair_routes
        .iter()
        .take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT)
    {
        if selected
            .iter()
            .any(|packet| packet.repair_route == route.label)
        {
            continue;
        }
        let Some(packet) = packets
            .iter()
            .find(|packet| packet.repair_route == route.label)
        else {
            continue;
        };
        if selected_packet_ids.insert(packet.packet_id.clone()) {
            selected.push(packet.clone());
        }
    }

    selected.sort_by(|left, right| {
        right
            .signal_count
            .cmp(&left.signal_count)
            .then_with(|| left.limitation_category.cmp(&right.limitation_category))
            .then_with(|| left.repair_route.cmp(&right.repair_route))
    });
    selected.truncate(LANE1_STATIC_LIMITATION_BACKLOG_PACKET_LIMIT);
    selected
}

fn static_limitation_backlog_packet_id(
    category: &str,
    subroute: &str,
    repair_route: &str,
) -> String {
    if subroute == category {
        format!("limitation:{}:{}", category, audit_slug(repair_route))
    } else {
        format!(
            "limitation:{}:{}:{}",
            category,
            subroute,
            audit_slug(repair_route)
        )
    }
}

fn lane1_static_limitation_backlog_top_subroutes(
    packets: &[Lane1StaticLimitationBacklogPacket],
) -> Vec<Value> {
    let mut rows = packets
        .iter()
        .map(|packet| {
            serde_json::json!({
                "category": packet.limitation_category,
                "subroute": packet.limitation_subroute,
                "count": packet.signal_count,
                "repair_route": packet.repair_route,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        audit_usize(right, &["count"])
            .unwrap_or_default()
            .cmp(&audit_usize(left, &["count"]).unwrap_or_default())
            .then_with(|| {
                audit_non_empty_string(left, &["category"])
                    .unwrap_or_default()
                    .cmp(&audit_non_empty_string(right, &["category"]).unwrap_or_default())
            })
            .then_with(|| {
                audit_non_empty_string(left, &["subroute"])
                    .unwrap_or_default()
                    .cmp(&audit_non_empty_string(right, &["subroute"]).unwrap_or_default())
            })
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    rows
}

fn lane1_static_limitation_backlog_packets_json(
    packets: &[Lane1StaticLimitationBacklogPacket],
) -> Vec<Value> {
    packets
        .iter()
        .map(|packet| {
            serde_json::json!({
                "packet_id": packet.packet_id,
                "limitation_category": packet.limitation_category,
                "limitation_subroute": packet.limitation_subroute,
                "repair_route": packet.repair_route,
                "signal_count": packet.signal_count,
                "sample_canonical_gap_ids": packet.sample_canonical_gap_ids,
                "sample_sources": packet
                    .sample_sources
                    .iter()
                    .map(lane1_static_limitation_backlog_sample_json)
                    .collect::<Vec<_>>(),
                "dominant_evidence_class": packet.dominant_evidence_class,
                "why_not_actionable": packet.why_not_actionable,
                "unlock_condition": packet.unlock_condition,
                "non_claims": packet.non_claims,
            })
        })
        .collect()
}

pub(crate) fn lane1_static_limitation_backlog_sample_json(
    sample: &Lane1StaticLimitationBacklogSample,
) -> Value {
    serde_json::json!({
        "canonical_gap_id": sample.canonical_gap_id,
        "evidence_class": sample.evidence_class,
        "source_file": sample.source_file,
        "line": sample.line,
        "expression": sample.expression,
        "limitation_reason": sample.limitation_reason,
    })
}

pub(crate) fn static_limitation_subroute(
    record: &Value,
    limitation: &Value,
    category: &str,
    _repair_route: &str,
    evidence_class: &str,
) -> String {
    if let Some(explicit) = audit_non_empty_string(limitation, &["limitation_subroute"])
        .or_else(|| audit_non_empty_string(limitation, &["subroute"]))
    {
        return audit_identifier_slug(&explicit);
    }

    match category {
        "activation_owner_call_absent_call_presence_target_affinity" => {
            call_presence_target_affinity_subroute(record)
        }
        "activation_owner_call_absent_assertion_target_affinity" => {
            let class = audit_identifier_slug(evidence_class);
            format!("assertion_target_affinity_{class}_missing_owner_call")
        }
        "activation_owner_call_absent_affinity_only" => {
            related_test_affinity_subroute(record, evidence_class)
        }
        "activation_owner_call_absent_same_file_only" => {
            same_file_owner_call_subroute(record, evidence_class)
        }
        _ => audit_identifier_slug(category),
    }
}

fn call_presence_target_affinity_subroute(record: &Value) -> String {
    let Some(expression) = audit_static_limitation_example_expression(record) else {
        return "call_presence_target_affinity_missing_owner_call".to_string();
    };
    if call_presence_expression_is_method_chain(&expression) {
        "call_presence_target_affinity_method_chain_missing_owner_call".to_string()
    } else if call_presence_expression_is_associated_call(&expression) {
        "call_presence_target_affinity_associated_call_missing_owner_call".to_string()
    } else if expression.contains('(') {
        "call_presence_target_affinity_function_call_missing_owner_call".to_string()
    } else {
        "call_presence_target_affinity_missing_owner_call".to_string()
    }
}

fn related_test_affinity_subroute(record: &Value, evidence_class: &str) -> String {
    let relation = audit_dominant_related_test_reason(record)
        .map(|reason| audit_identifier_slug(&reason))
        .unwrap_or_else(|| "related_test_affinity".to_string());
    if audit_identifier_slug(evidence_class) != "call_presence" {
        return format!("{relation}_missing_owner_call");
    }
    let Some(expression) = audit_static_limitation_example_expression(record) else {
        return format!("{relation}_missing_owner_call");
    };
    if call_presence_expression_is_method_chain(&expression) {
        format!("{relation}_call_presence_method_chain_missing_owner_call")
    } else if call_presence_expression_is_associated_call(&expression) {
        format!("{relation}_call_presence_associated_call_missing_owner_call")
    } else if expression.contains('(') {
        format!("{relation}_call_presence_function_call_missing_owner_call")
    } else {
        format!("{relation}_missing_owner_call")
    }
}

fn same_file_owner_call_subroute(record: &Value, evidence_class: &str) -> String {
    if audit_identifier_slug(evidence_class) != "call_presence" {
        let class = audit_identifier_slug(evidence_class);
        return format!("same_file_only_{class}_missing_owner_call");
    }
    let Some(expression) = audit_static_limitation_example_expression(record) else {
        return "same_file_only_missing_owner_call".to_string();
    };
    if call_presence_expression_is_method_chain(&expression) {
        "same_file_only_call_presence_receiver_method_missing_owner_call".to_string()
    } else if call_presence_expression_is_associated_call(&expression) {
        "same_file_only_call_presence_associated_call_missing_owner_call".to_string()
    } else if expression.contains('(') {
        "same_file_only_call_presence_function_call_missing_owner_call".to_string()
    } else {
        "same_file_only_missing_owner_call".to_string()
    }
}

fn audit_static_limitation_example_expression(record: &Value) -> Option<String> {
    audit_array(record, &["raw_findings"])
        .first()
        .and_then(|raw| audit_non_empty_string(raw, &["expression"]))
        .or_else(|| {
            audit_array(record, &["canonical_item", "raw_findings"])
                .first()
                .and_then(|raw| audit_non_empty_string(raw, &["expression"]))
        })
}

fn call_presence_expression_is_method_chain(expression: &str) -> bool {
    let Some(dot_index) = expression.find('.') else {
        return false;
    };
    let Some(paren_index) = expression.find('(') else {
        return false;
    };
    dot_index < paren_index
}

fn call_presence_expression_is_associated_call(expression: &str) -> bool {
    expression.find("::").is_some_and(|path_index| {
        expression
            .find('(')
            .is_some_and(|paren_index| path_index < paren_index)
    })
}

pub(crate) fn audit_identifier_slug(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let trimmed = slug.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn audit_dominant_related_test_reason(record: &Value) -> Option<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for related in audit_array(record, &["related_tests"]) {
        let Some(reason) = audit_non_empty_string(related, &["relation_reason"]) else {
            continue;
        };
        *counts.entry(reason).or_insert(0) += 1;
    }
    audit_top_counts(counts)
        .first()
        .map(|row| row.label.clone())
}

pub(crate) fn static_limitation_why_not_actionable(category: &str) -> &'static str {
    match category {
        "activation_boundary_input_unresolved" => {
            "activation inputs cannot yet be mapped to a safe concrete test value"
        }
        "activation_owner_call_absent_assertion_target_affinity" => {
            "assertion-target affinity is not enough to prove the owner is exercised"
        }
        "activation_owner_call_absent_call_presence_target_affinity" => {
            "call-presence target affinity is not enough to prove the owner is exercised"
        }
        "activation_owner_call_absent_affinity_only" => {
            "related-test affinity is not enough to prove the owner is exercised"
        }
        "activation_owner_call_absent_same_file_only" => {
            "same-file proximity is not enough to prove the owner is exercised"
        }
        "observer_target_unknown" => {
            "the analyzer cannot name a bounded observer target for a safe repair packet"
        }
        CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY => {
            "binding, FFI, or external-language target placement is unresolved, so RIPR cannot choose a safe test edit surface"
        }
        CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY => {
            "external-language oracle visibility is unresolved, so static evidence cannot prove the external assertion path"
        }
        _ => "static evidence is insufficient to provide a bounded repair packet",
    }
}

pub(crate) fn static_limitation_unlock_condition(
    category: &str,
    subroute: &str,
    repair_route: &str,
) -> String {
    if category == "activation_owner_call_absent_same_file_only"
        && subroute == "same_file_only_call_presence_receiver_method_missing_owner_call"
    {
        return format!(
            "implement `{repair_route}` by establishing whether the receiver method call is reached through a bounded direct or helper owner-call path; keep the limitation non-actionable until owner activation is observed"
        );
    }
    if category == "activation_owner_call_absent_affinity_only"
        && subroute.starts_with("same_test_file_call_presence_")
    {
        return format!(
            "implement `{repair_route}` by tracing same-file related-test calls through a bounded production call graph; keep the limitation non-actionable until a direct or helper owner call is demonstrated"
        );
    }
    if category == "activation_owner_call_absent_same_file_only"
        && subroute.starts_with("same_file_only_call_presence_")
    {
        return format!(
            "implement `{repair_route}` by tracing same-file related-test calls through a bounded production call graph; keep the limitation non-actionable until a direct or helper owner call is demonstrated"
        );
    }

    match category {
        "activation_boundary_input_unresolved" => {
            format!(
                "implement `{repair_route}` so local, member-access, iterator, or computed operands can be resolved before candidate values are recommended"
            )
        }
        "activation_owner_call_absent_assertion_target_affinity"
        | "activation_owner_call_absent_call_presence_target_affinity"
        | "activation_owner_call_absent_affinity_only"
        | "activation_owner_call_absent_same_file_only" => {
            format!(
                "implement `{repair_route}` so related tests can be tied to direct or helper owner calls"
            )
        }
        "observer_target_unknown" => {
            format!(
                "implement `{repair_route}` so the observer target and assertion surface are explicit"
            )
        }
        CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY => {
            format!(
                "implement `{repair_route}` with explicit binding or external observer target evidence before any repair packet names an edit surface"
            )
        }
        CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY => {
            format!(
                "implement `{repair_route}` so the Rust seam, binding edge, external callsite, and external oracle can be named before actionability is considered"
            )
        }
        _ => format!(
            "implement `{repair_route}` and preserve the item as non-actionable until the packet contract is complete"
        ),
    }
}

pub(crate) fn static_limitation_backlog_packet_non_claims(
    category: &str,
    subroute: Option<&str>,
) -> Vec<String> {
    let mut claims = vec![
        "not a public repair packet".to_string(),
        "not swarm-ready work".to_string(),
        "do not edit tests from this backlog item alone".to_string(),
        "do not invent exact candidate values".to_string(),
    ];
    if category == "activation_boundary_input_unresolved" {
        claims.push("do not invent exact boundary candidate values".to_string());
    }
    if category == "activation_owner_call_absent_assertion_target_affinity" {
        claims.push("do not treat assertion-target affinity as activation evidence".to_string());
    }
    if category == "activation_owner_call_absent_call_presence_target_affinity" {
        claims
            .push("do not treat call-target assertion affinity as activation evidence".to_string());
    }
    if category == "activation_owner_call_absent_affinity_only"
        && subroute.is_some_and(|subroute| subroute.starts_with("same_test_file_call_presence_"))
    {
        claims.push("do not treat same-file affinity as owner-call evidence".to_string());
    }
    if category == "activation_owner_call_absent_same_file_only"
        && subroute.is_some_and(|subroute| subroute.starts_with("same_file_only_call_presence_"))
    {
        claims.push("do not treat same-file proximity as owner-call evidence".to_string());
    }
    if category == "activation_owner_call_absent_same_file_only"
        && subroute.is_some_and(|subroute| subroute.contains("receiver_method"))
    {
        claims.push("do not treat receiver method text as owner-call evidence".to_string());
    }
    if category == CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY {
        claims.push("do not infer TypeScript, Python, or other external test targets".to_string());
        claims.push("do not suggest unrelated Rust test files".to_string());
        claims
            .push("do not promote navigation-only target evidence into repair action".to_string());
    }
    if category == CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY {
        claims.push("do not claim full cross-language oracle proof".to_string());
        claims.push(
            "do not count external tests as gripped until the oracle edge is explicit".to_string(),
        );
    }
    claims
}

pub(crate) fn lane1_actionable_gap_packets_markdown(report: &Lane1EvidenceAuditReport) -> String {
    let runtime_status = lane1_runtime_status_for_report(&report.run_limitations);
    let mut out = String::new();
    out.push_str("# Actionable Canonical Gap Packets\n\n");
    out.push_str(&format!("Run status: `{}`\n\n", runtime_status.state));
    out.push_str("Advisory Lane 1 packets derived from `evidence_record.canonical_item`.\n\n");
    out.push_str("Raw findings remain supporting evidence; packets are bounded work items for humans and agents.\n\n");
    out.push_str("## Runtime Status\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| State | `{}` |\n",
        audit_markdown_cell(&runtime_status.state)
    ));
    out.push_str(&format!(
        "| Phase | `{}` |\n",
        audit_markdown_cell(runtime_status.phase.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Duration ms | `{}` |\n",
        runtime_status
            .duration_ms
            .map(|value| value.to_string())
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "| Limit ms | `{}` |\n",
        runtime_status
            .limit_ms
            .map(|value| value.to_string())
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "| Input kind | `{}` |\n",
        audit_markdown_cell(runtime_status.input_kind.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Input path | `{}` |\n",
        audit_markdown_cell(runtime_status.input_path.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Limitation category | `{}` |\n",
        audit_markdown_cell(runtime_status.limitation_category.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Repair route | {} |\n",
        audit_markdown_cell(runtime_status.repair_route.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| Downstream consumable | `{}` |\n\n",
        runtime_status.downstream_consumable
    ));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        &mut out,
        "Raw alignment signals",
        report.finding_alignment.raw_signals_total,
    );
    audit_push_count(
        &mut out,
        "Canonical alignment items",
        report.finding_alignment.canonical_items_total,
    );
    audit_push_count(
        &mut out,
        "Actionable canonical gaps",
        report.finding_alignment.actionable_items_total,
    );
    audit_push_count(
        &mut out,
        "Already observed",
        report.finding_alignment.already_observed_total,
    );
    audit_push_count(
        &mut out,
        "Internal/no-action",
        report.finding_alignment.internal_no_action_total,
    );
    audit_push_count(
        &mut out,
        "Static limitations",
        report.finding_alignment.static_limitation_total,
    );
    audit_push_count(
        &mut out,
        "Packets emitted",
        report.actionable_gap_packets.len(),
    );
    let public_projection_eligible_packets =
        audit_actionable_gap_packet_public_projection_eligible_count(
            &report.actionable_gap_packets,
        );
    audit_push_count(
        &mut out,
        "Public projection eligible packets",
        public_projection_eligible_packets,
    );
    audit_push_count(
        &mut out,
        "Public projection excluded packets",
        report
            .actionable_gap_packets
            .len()
            .saturating_sub(public_projection_eligible_packets),
    );
    audit_push_count(
        &mut out,
        "Repair route unknowns",
        report.canonical_items_without_repair_route,
    );
    audit_push_count(
        &mut out,
        "Verify command unknowns",
        report.canonical_items_without_verify_command,
    );
    out.push('\n');
    audit_push_counts_table_limited(
        &mut out,
        "Projection exclusion reason",
        &audit_actionable_gap_packet_projection_exclusion_counts(&report.actionable_gap_packets),
        LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    );
    let static_limitation_backlog = lane1_static_limitation_backlog_json(report);
    ripr_swarm_push_static_limitation_backlog_markdown(&mut out, &static_limitation_backlog);

    if !report.run_limitations.is_empty() {
        out.push_str("## Run Limitations\n\n");
        out.push_str("| Category | Phase | Observed seams | Cache limit | Repair route |\n");
        out.push_str("| --- | --- | ---: | ---: | --- |\n");
        for limitation in &report.run_limitations {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} | {} |\n",
                audit_markdown_cell(&limitation.category),
                audit_markdown_cell(&limitation.phase),
                limitation
                    .observed_seams
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                limitation
                    .cache_limit
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                audit_markdown_cell(&limitation.repair_route)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Packets\n\n");
    if report.actionable_gap_packets.is_empty() {
        out.push_str("No actionable canonical gap packets were emitted.\n");
        return out;
    }

    for packet in &report.actionable_gap_packets {
        out.push_str(&format!(
            "### `{}`\n\n",
            audit_markdown_cell(&packet.canonical_gap_id)
        ));
        out.push_str("| Field | Value |\n");
        out.push_str("| --- | --- |\n");
        out.push_str(&format!(
            "| Evidence class | `{}` |\n",
            audit_markdown_cell(&packet.evidence_class)
        ));
        out.push_str(&format!(
            "| Source file | `{}` |\n",
            audit_markdown_cell(&packet.source_file)
        ));
        out.push_str(&format!(
            "| Repair kind | `{}` |\n",
            audit_markdown_cell(&packet.repair_kind)
        ));
        out.push_str(&format!(
            "| Target test type | `{}` |\n",
            audit_markdown_cell(&packet.target_test_type)
        ));
        out.push_str(&format!(
            "| Recommended repair | {} |\n",
            audit_markdown_cell(&packet.recommended_repair)
        ));
        out.push_str(&format!(
            "| Verify command | `{}` |\n",
            audit_markdown_cell(&packet.verify_command)
        ));
        out.push_str(&format!(
            "| Public projection | {} |\n",
            if packet.public_projection_eligible {
                "eligible".to_string()
            } else {
                format!(
                    "excluded: {}",
                    audit_markdown_cell(&packet.projection_exclusion_reasons.join(", "))
                )
            }
        ));
        out.push_str(&format!(
            "| Repair route source | `{}` |\n",
            audit_markdown_cell(&packet.repair_route_source)
        ));
        out.push_str(&format!(
            "| Verify command source | `{}` |\n",
            audit_markdown_cell(&packet.verify_command_source)
        ));
        out.push_str(&format!(
            "| Receipt command/path | {} |\n",
            audit_markdown_cell(
                packet
                    .receipt_command_or_path
                    .as_deref()
                    .unwrap_or("missing")
            )
        ));
        out.push_str(&format!(
            "| Receipt source | `{}` |\n",
            audit_markdown_cell(&packet.receipt_source)
        ));
        out.push_str(&format!(
            "| Missing discriminators | {} |\n",
            audit_markdown_cell(&audit_actionable_gap_missing_discriminator_summary(
                &packet.missing_discriminators
            ))
        ));
        out.push_str(&format!(
            "| Raw findings | {} supporting finding(s) |\n",
            packet.raw_findings.len()
        ));
        out.push('\n');
    }
    out
}

pub(crate) fn audit_slug(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn actionable_gap_outcome_state_counts_from_entries(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for state in [
        "not_attempted",
        "attempted_no_receipt",
        "receipt_present",
        "evidence_improved",
        "evidence_unchanged",
        "evidence_regressed",
        "resolved",
        "unknown",
    ] {
        counts.insert(state.to_string(), 0);
    }
    for attempt in attempts {
        audit_increment(&mut counts, &attempt.outcome);
    }
    counts
}

pub(crate) fn actionable_gap_outcomes_missing_verify_result_count(outcomes: &Value) -> usize {
    audit_array(outcomes, &["outcomes"])
        .iter()
        .filter(|outcome| {
            let outcome_state = audit_non_empty_string(outcome, &["outcome_state"])
                .unwrap_or_else(|| "unknown".to_string());
            outcome_state != "not_attempted"
                && audit_non_empty_string(outcome, &["verify_result"])
                    .as_deref()
                    .is_none_or(ripr_swarm_plan_field_missing)
        })
        .count()
}

pub(crate) fn actionable_gap_outcomes_report_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_actionable_gap_outcomes_args(args)?;
    if !parsed.actionable_gaps.exists() {
        return Err(format!(
            "actionable-gap-outcomes requires `{}`; run `cargo xtask lane1-evidence-audit` first or pass `--actionable-gaps <path>`",
            normalize_path(&parsed.actionable_gaps)
        ));
    }

    let packets = read_json_value(&parsed.actionable_gaps)?;
    let receipt = match parsed.agent_receipt.as_ref() {
        Some(path) => Some(read_json_value(path)?),
        None => None,
    };
    let targeted = match parsed.targeted_test_outcome.as_ref() {
        Some(path) => Some(read_json_value(path)?),
        None => None,
    };
    let report = actionable_gap_outcomes_report_from_values(
        &packets,
        receipt.as_ref(),
        targeted.as_ref(),
        normalize_path(&parsed.actionable_gaps),
        parsed
            .agent_receipt
            .as_ref()
            .map(|path| normalize_path(path)),
        parsed
            .targeted_test_outcome
            .as_ref()
            .map(|path| normalize_path(path)),
    )?;

    write_report(
        "actionable-gap-outcomes.json",
        &actionable_gap_outcomes_json(&report)?,
    )?;
    write_report(
        "actionable-gap-outcomes.md",
        &actionable_gap_outcomes_markdown(&report),
    )
}

pub(crate) fn parse_actionable_gap_outcomes_args(
    args: &[String],
) -> Result<ActionableGapOutcomesArgs, String> {
    let mut actionable_gaps = PathBuf::from("target/ripr/reports/actionable-gaps.json");
    let mut agent_receipt: Option<PathBuf> = None;
    let mut targeted_test_outcome: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--actionable-gaps" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        actionable_gap_outcomes_usage()
                    ));
                };
                actionable_gaps = PathBuf::from(path);
            }
            "--agent-receipt" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        actionable_gap_outcomes_usage()
                    ));
                };
                agent_receipt = Some(PathBuf::from(path));
            }
            "--targeted-test-outcome" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        actionable_gap_outcomes_usage()
                    ));
                };
                targeted_test_outcome = Some(PathBuf::from(path));
            }
            "--help" | "-h" => return Err(actionable_gap_outcomes_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown actionable-gap-outcomes option `{flag}`\n{}",
                    actionable_gap_outcomes_usage()
                ));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument `{other}`\n{}",
                    actionable_gap_outcomes_usage()
                ));
            }
        }
        index += 1;
    }

    if agent_receipt.is_none() {
        agent_receipt = first_existing_path(&[
            "target/ripr/reports/agent-receipt.json",
            "target/ripr/workflow/agent-receipt.json",
            "target/ripr/agent/agent-receipt.json",
            "target/ripr/receipts/agent-receipt.json",
        ]);
    }
    if targeted_test_outcome.is_none() {
        targeted_test_outcome =
            first_existing_path(&["target/ripr/reports/targeted-test-outcome.json"]);
    }

    Ok(ActionableGapOutcomesArgs {
        actionable_gaps,
        agent_receipt,
        targeted_test_outcome,
    })
}

fn first_existing_path(paths: &[&str]) -> Option<PathBuf> {
    paths.iter().map(PathBuf::from).find(|path| path.exists())
}

fn actionable_gap_outcomes_usage() -> String {
    "usage: cargo xtask actionable-gap-outcomes [--actionable-gaps <path>] [--agent-receipt <path>] [--targeted-test-outcome <path>]"
        .to_string()
}

pub(crate) fn actionable_gap_outcomes_report_from_values(
    packets: &Value,
    agent_receipt: Option<&Value>,
    targeted_test_outcome: Option<&Value>,
    actionable_gaps_path: String,
    agent_receipt_path: Option<String>,
    targeted_test_outcome_path: Option<String>,
) -> Result<ActionableGapOutcomesReport, String> {
    let packet_items = audit_get(packets, &["packets"])
        .and_then(Value::as_array)
        .ok_or_else(|| "actionable-gaps JSON is missing `packets` array".to_string())?;
    let receipt_values = actionable_gap_receipt_values(agent_receipt);
    let orphaned_receipts = actionable_gap_orphaned_receipts(&receipt_values, packet_items);
    let outcomes = packet_items
        .iter()
        .map(|packet| {
            actionable_gap_outcome_from_packet(
                packet,
                &receipt_values,
                targeted_test_outcome,
                agent_receipt_path.as_deref(),
                targeted_test_outcome_path.as_deref(),
                agent_receipt.is_some(),
                !orphaned_receipts.is_empty(),
            )
        })
        .collect::<Vec<_>>();

    Ok(ActionableGapOutcomesReport {
        actionable_gaps_path,
        agent_receipt_path,
        targeted_test_outcome_path,
        packets_total: packet_items.len(),
        outcomes,
        orphaned_receipts,
    })
}

fn actionable_gap_receipt_values(receipt: Option<&Value>) -> Vec<&Value> {
    let Some(receipt) = receipt else {
        return Vec::new();
    };
    if let Some(receipts) = receipt.as_array() {
        return receipts.iter().collect();
    }
    if let Some(receipts) = audit_get(receipt, &["receipts"]).and_then(Value::as_array) {
        return receipts.iter().collect();
    }
    vec![receipt]
}

fn actionable_gap_orphaned_receipts(
    receipts: &[&Value],
    packets: &[Value],
) -> Vec<ActionableGapOrphanedReceipt> {
    receipts
        .iter()
        .enumerate()
        .filter(|(_, receipt)| !actionable_gap_receipt_matches_any_packet(receipt, packets))
        .map(|(index, receipt)| actionable_gap_orphaned_receipt_from_value(index, receipt))
        .collect()
}

fn actionable_gap_receipt_matches_any_packet(receipt: &Value, packets: &[Value]) -> bool {
    packets.iter().any(|packet| {
        let candidates = actionable_gap_id_candidates(packet);
        actionable_gap_receipt_matches_packet(receipt, packet, &candidates)
    })
}

fn actionable_gap_orphaned_receipt_from_value(
    index: usize,
    receipt: &Value,
) -> ActionableGapOrphanedReceipt {
    let seam_id = actionable_gap_receipt_seam_id(receipt);
    let receipt_id = seam_id
        .as_ref()
        .map(|seam_id| format!("receipt:{seam_id}"))
        .unwrap_or_else(|| format!("receipt:index-{index}"));
    ActionableGapOrphanedReceipt {
        receipt_id,
        seam_id,
        source_file: audit_non_empty_string(receipt, &["seam", "file"]),
        line: audit_usize(receipt, &["seam", "line"]),
        movement_direction: audit_non_empty_string(receipt, &["seam", "change"])
            .or_else(|| audit_non_empty_string(receipt, &["provenance", "movement"]))
            .or_else(|| audit_non_empty_string(receipt, &["summary", "next_action", "kind"])),
        reason: "Receipt artifact did not match any current actionable canonical gap packet."
            .to_string(),
    }
}

fn actionable_gap_outcome_from_packet(
    packet: &Value,
    receipts: &[&Value],
    targeted_test_outcome: Option<&Value>,
    agent_receipt_path: Option<&str>,
    targeted_test_outcome_path: Option<&str>,
    receipt_input_present: bool,
    orphaned_receipt_present: bool,
) -> ActionableGapOutcome {
    let canonical_gap_id = audit_non_empty_string(packet, &["canonical_gap_id"])
        .unwrap_or_else(|| "canonical_gap_id_unknown".to_string());
    let id_candidates = actionable_gap_id_candidates(packet);
    let receipt = receipts
        .iter()
        .copied()
        .find(|receipt| actionable_gap_receipt_matches_packet(receipt, packet, &id_candidates));
    let targeted_movement = targeted_test_outcome
        .and_then(|outcome| actionable_gap_targeted_movement(outcome, packet, &id_candidates));
    let receipt_movement = receipt.and_then(actionable_gap_receipt_movement);
    let targeted_verify_result =
        targeted_test_outcome.and_then(actionable_gap_targeted_verify_result);
    let receipt_verify_result = receipt.and_then(actionable_gap_receipt_verify_result);
    let receipt_timestamp = receipt.and_then(actionable_gap_receipt_timestamp);
    let receipt_state = if let Some(movement) = receipt_movement.as_ref() {
        receipt_lifecycle_state_from_movement(movement.direction.as_deref())
    } else if receipt.is_some() {
        RECEIPT_FOUND.to_string()
    } else if receipt_input_present && orphaned_receipt_present && targeted_movement.is_none() {
        RECEIPT_GAP_MISMATCH.to_string()
    } else if receipt_input_present {
        RECEIPT_MISSING.to_string()
    } else {
        RECEIPT_NOT_APPLICABLE.to_string()
    };
    let movement = targeted_movement.or(receipt_movement);
    let outcome_state = match movement.as_ref() {
        Some(movement) if receipt.is_none() && movement.source == "targeted_test_outcome" => {
            "attempted_no_receipt".to_string()
        }
        Some(movement) => movement.outcome_state.clone(),
        None if receipt.is_some() => "receipt_present".to_string(),
        None => "not_attempted".to_string(),
    };
    let reason = match movement.as_ref() {
        Some(movement) => movement.reason.clone(),
        None if receipt.is_some() => {
            "Receipt artifact matched this packet, but no evidence movement bucket was available."
                .to_string()
        }
        None => "No receipt or targeted-test outcome artifact matched this packet.".to_string(),
    };
    let receipt_command = audit_non_empty_string(packet, &["receipt_command"])
        .or_else(|| audit_non_empty_string(packet, &["receipt_command_or_path"]));
    let verify_result = receipt_verify_result.or_else(|| {
        if movement
            .as_ref()
            .is_some_and(|movement| movement.source == "targeted_test_outcome")
        {
            targeted_verify_result
        } else {
            None
        }
    });
    let timestamp = receipt_timestamp.or_else(|| {
        movement
            .as_ref()
            .and_then(|movement| movement.timestamp.clone())
    });
    let attempt_instance = actionable_gap_outcome_attempt_instance(
        timestamp.as_deref(),
        receipt.is_some(),
        agent_receipt_path,
        movement.as_ref().map(|movement| movement.source.as_str()),
        targeted_test_outcome_path,
    );

    ActionableGapOutcome {
        canonical_gap_id,
        evidence_class: audit_non_empty_string(packet, &["evidence_class"])
            .unwrap_or_else(|| "evidence_class_unknown".to_string()),
        repair_kind: audit_non_empty_string(packet, &["repair_kind"])
            .unwrap_or_else(|| "repair_kind_unknown".to_string()),
        source_file: audit_non_empty_string(packet, &["source_file"])
            .or_else(|| audit_non_empty_string(packet, &["primary_anchor", "file"]))
            .unwrap_or_else(|| "source_file_unknown".to_string()),
        verify_command: audit_non_empty_string(packet, &["verify_command"])
            .unwrap_or_else(|| "verify_command_unknown".to_string()),
        verify_result,
        receipt_command_or_path: audit_non_empty_string(packet, &["receipt_command_or_path"])
            .or_else(|| receipt_command.clone()),
        receipt_command,
        receipt_state,
        outcome_state,
        timestamp,
        attempt_instance,
        seam_id: movement
            .as_ref()
            .and_then(|movement| movement.seam_id.clone()),
        before: movement
            .as_ref()
            .and_then(|movement| movement.before.clone()),
        after: movement
            .as_ref()
            .and_then(|movement| movement.after.clone()),
        movement_source: movement.as_ref().map(|movement| movement.source.clone()),
        movement_direction: movement
            .as_ref()
            .and_then(|movement| movement.direction.clone()),
        evidence_delta: movement
            .as_ref()
            .map(|movement| movement.evidence_delta.clone())
            .unwrap_or_default(),
        reason,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapMovement {
    pub(crate) source: String,
    pub(crate) outcome_state: String,
    pub(crate) seam_id: Option<String>,
    pub(crate) before: Option<String>,
    pub(crate) after: Option<String>,
    pub(crate) direction: Option<String>,
    pub(crate) timestamp: Option<String>,
    pub(crate) evidence_delta: Vec<String>,
    pub(crate) reason: String,
}

pub(crate) fn actionable_gap_id_candidates(packet: &Value) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    if let Some(id) = audit_non_empty_string(packet, &["packet_id"]) {
        actionable_gap_push_id_candidate(&mut candidates, &id);
    }
    if let Some(id) = audit_non_empty_string(packet, &["canonical_gap_id"]) {
        actionable_gap_push_id_candidate(&mut candidates, &id);
    }
    if let Some(id) = audit_non_empty_string(packet, &["seam_id"]) {
        actionable_gap_push_id_candidate(&mut candidates, &id);
    }
    candidates
}

pub(crate) fn actionable_gap_push_id_candidate(candidates: &mut BTreeSet<String>, id: &str) {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return;
    }
    candidates.insert(trimmed.to_string());
    for prefix in ["gap:", "canonical_gap:", "canonical_item:"] {
        if let Some(stripped) = trimmed.strip_prefix(prefix)
            && !stripped.trim().is_empty()
        {
            candidates.insert(stripped.trim().to_string());
        }
    }
}

fn actionable_gap_receipt_matches_packet(
    receipt: &Value,
    packet: &Value,
    candidates: &BTreeSet<String>,
) -> bool {
    actionable_gap_receipt_seam_id(receipt).is_some_and(|id| candidates.contains(&id))
        || actionable_gap_anchor_matches(
            packet,
            audit_non_empty_string(receipt, &["seam", "file"]).as_deref(),
            audit_usize(receipt, &["seam", "line"]),
        )
}

fn actionable_gap_receipt_seam_id(receipt: &Value) -> Option<String> {
    audit_non_empty_string(receipt, &["seam", "seam_id"])
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "seam_id"]))
}

fn actionable_gap_anchor_matches(packet: &Value, file: Option<&str>, line: Option<usize>) -> bool {
    let Some(file) = file else {
        return false;
    };
    let Some(line) = line else {
        return false;
    };
    let packet_file = audit_non_empty_string(packet, &["primary_anchor", "file"])
        .or_else(|| audit_non_empty_string(packet, &["source_file"]));
    let packet_line = audit_usize(packet, &["primary_anchor", "line"]);
    packet_file.as_deref().is_some_and(|packet_file| {
        normalize_report_path(packet_file) == normalize_report_path(file)
    }) && packet_line == Some(line)
}

fn actionable_gap_targeted_movement(
    targeted: &Value,
    packet: &Value,
    candidates: &BTreeSet<String>,
) -> Option<ActionableGapMovement> {
    let timestamp = actionable_gap_targeted_timestamp(targeted);
    for bucket in ["moved", "unchanged", "regressed"] {
        for item in audit_array(targeted, &[bucket]) {
            if actionable_gap_targeted_item_matches_packet(item, packet, candidates) {
                let direction = audit_non_empty_string(item, &["direction"])
                    .unwrap_or_else(|| bucket.trim_end_matches('d').to_string());
                let outcome_state = actionable_gap_outcome_state_for_direction(&direction, bucket);
                return Some(ActionableGapMovement {
                    source: "targeted_test_outcome".to_string(),
                    outcome_state,
                    seam_id: audit_non_empty_string(item, &["seam_id"]),
                    before: audit_non_empty_string(item, &["before"]),
                    after: audit_non_empty_string(item, &["after"]),
                    direction: Some(direction),
                    timestamp: timestamp.clone(),
                    evidence_delta: audit_string_array(item, &["evidence_delta"])
                        .unwrap_or_default(),
                    reason: format!("Matched targeted-test outcome `{bucket}` bucket."),
                });
            }
        }
    }
    for item in audit_array(targeted, &["removed"]) {
        if actionable_gap_targeted_item_matches_packet(item, packet, candidates) {
            return Some(ActionableGapMovement {
                source: "targeted_test_outcome".to_string(),
                outcome_state: "resolved".to_string(),
                seam_id: audit_non_empty_string(item, &["seam_id"]),
                before: audit_non_empty_string(item, &["grip_class"]),
                after: None,
                direction: Some("resolved".to_string()),
                timestamp: timestamp.clone(),
                evidence_delta: Vec::new(),
                reason: "Matched targeted-test outcome `removed` bucket.".to_string(),
            });
        }
    }
    for item in audit_array(targeted, &["new"]) {
        if actionable_gap_targeted_item_matches_packet(item, packet, candidates) {
            return Some(ActionableGapMovement {
                source: "targeted_test_outcome".to_string(),
                outcome_state: "unknown".to_string(),
                seam_id: audit_non_empty_string(item, &["seam_id"]),
                before: None,
                after: audit_non_empty_string(item, &["grip_class"]),
                direction: Some("new".to_string()),
                timestamp: timestamp.clone(),
                evidence_delta: Vec::new(),
                reason: "Matched targeted-test outcome `new` bucket; this is not repair progress."
                    .to_string(),
            });
        }
    }
    None
}

fn actionable_gap_targeted_item_matches_packet(
    item: &Value,
    packet: &Value,
    candidates: &BTreeSet<String>,
) -> bool {
    audit_non_empty_string(item, &["seam_id"]).is_some_and(|id| candidates.contains(&id))
        || actionable_gap_anchor_matches(
            packet,
            audit_non_empty_string(item, &["file"]).as_deref(),
            audit_usize(item, &["line"]),
        )
}

fn actionable_gap_receipt_movement(receipt: &Value) -> Option<ActionableGapMovement> {
    let seam_id = actionable_gap_receipt_seam_id(receipt);
    let direction = audit_non_empty_string(receipt, &["seam", "change"])
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "movement"]))
        .or_else(|| audit_non_empty_string(receipt, &["summary", "next_action", "kind"]))?;
    let outcome_state = match direction.as_str() {
        "new_gap" => "receipt_present".to_string(),
        _ => actionable_gap_outcome_state_for_direction(&direction, "receipt"),
    };
    Some(ActionableGapMovement {
        source: "agent_receipt".to_string(),
        outcome_state,
        seam_id,
        before: audit_non_empty_string(receipt, &["seam", "before"])
            .or_else(|| audit_non_empty_string(receipt, &["provenance", "before_class"])),
        after: audit_non_empty_string(receipt, &["seam", "after"])
            .or_else(|| audit_non_empty_string(receipt, &["provenance", "after_class"])),
        direction: Some(direction),
        timestamp: actionable_gap_receipt_timestamp(receipt),
        evidence_delta: audit_string_array(receipt, &["seam", "evidence_delta"])
            .unwrap_or_default(),
        reason: "Matched agent receipt artifact.".to_string(),
    })
}

fn actionable_gap_receipt_timestamp(receipt: &Value) -> Option<String> {
    audit_non_empty_string(receipt, &["timestamp"])
        .or_else(|| audit_non_empty_string(receipt, &["generated_at"]))
        .or_else(|| audit_non_empty_string(receipt, &["recorded_at"]))
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "timestamp"]))
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "generated_at"]))
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "recorded_at"]))
}

fn actionable_gap_receipt_verify_result(receipt: &Value) -> Option<String> {
    audit_non_empty_string(receipt, &["verify_result"])
        .or_else(|| audit_non_empty_string(receipt, &["verification", "result"]))
        .or_else(|| audit_non_empty_string(receipt, &["verification", "status"]))
        .or_else(|| audit_non_empty_string(receipt, &["summary", "verify_result"]))
        .or_else(|| audit_non_empty_string(receipt, &["summary", "verification_result"]))
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "verify_result"]))
        .or_else(|| audit_non_empty_string(receipt, &["provenance", "verification_result"]))
}

fn actionable_gap_targeted_timestamp(targeted: &Value) -> Option<String> {
    audit_non_empty_string(targeted, &["timestamp"])
        .or_else(|| audit_non_empty_string(targeted, &["generated_at"]))
        .or_else(|| audit_non_empty_string(targeted, &["provenance", "timestamp"]))
        .or_else(|| audit_non_empty_string(targeted, &["provenance", "generated_at"]))
}

fn actionable_gap_targeted_verify_result(targeted: &Value) -> Option<String> {
    audit_non_empty_string(targeted, &["verify_result"])
        .or_else(|| audit_non_empty_string(targeted, &["verification", "result"]))
        .or_else(|| audit_non_empty_string(targeted, &["verification", "status"]))
        .or_else(|| audit_non_empty_string(targeted, &["summary", "verify_result"]))
        .or_else(|| audit_non_empty_string(targeted, &["summary", "verification_result"]))
        .or_else(|| audit_non_empty_string(targeted, &["provenance", "verify_result"]))
        .or_else(|| audit_non_empty_string(targeted, &["provenance", "verification_result"]))
}

fn actionable_gap_outcome_attempt_instance(
    timestamp: Option<&str>,
    receipt_present: bool,
    agent_receipt_path: Option<&str>,
    movement_source: Option<&str>,
    targeted_test_outcome_path: Option<&str>,
) -> Option<String> {
    if let Some(timestamp) = timestamp {
        return Some(format!("timestamp:{timestamp}"));
    }
    if receipt_present && let Some(path) = agent_receipt_path {
        return Some(format!("receipt_path:{path}"));
    }
    if movement_source == Some("targeted_test_outcome") {
        return targeted_test_outcome_path.map(|path| format!("targeted_test_outcome_path:{path}"));
    }
    None
}

fn actionable_gap_outcome_state_for_direction(direction: &str, bucket: &str) -> String {
    match direction {
        "improved" => "evidence_improved",
        "unchanged" => "evidence_unchanged",
        "regressed" => "evidence_regressed",
        "resolved" | "removed" => "resolved",
        "changed" if bucket == "moved" => "unknown",
        "changed" => "receipt_present",
        _ if bucket == "unchanged" => "evidence_unchanged",
        _ if bucket == "regressed" => "evidence_regressed",
        _ => "unknown",
    }
    .to_string()
}

pub(crate) fn actionable_gap_outcomes_json(
    report: &ActionableGapOutcomesReport,
) -> Result<String, String> {
    let state_counts = actionable_gap_outcome_state_counts(&report.outcomes);
    let movement_front = actionable_gap_outcomes_movement_front(report, &state_counts);
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "actionable-gap-outcomes",
        "scope": "repo",
        "status": "advisory",
        "source": "actionable-gaps plus optional receipt and targeted-test outcome artifacts",
        "inputs": {
            "actionable_gaps": report.actionable_gaps_path,
            "agent_receipt": report.agent_receipt_path,
            "targeted_test_outcome": report.targeted_test_outcome_path,
        },
        "summary": {
            "packets_total": report.packets_total,
            "outcomes_total": report.outcomes.len(),
            "not_attempted": state_counts.get("not_attempted").copied().unwrap_or(0),
            "attempted_no_receipt": state_counts.get("attempted_no_receipt").copied().unwrap_or(0),
            "receipt_present": state_counts.get("receipt_present").copied().unwrap_or(0),
            "evidence_improved": state_counts.get("evidence_improved").copied().unwrap_or(0),
            "evidence_unchanged": state_counts.get("evidence_unchanged").copied().unwrap_or(0),
            "evidence_regressed": state_counts.get("evidence_regressed").copied().unwrap_or(0),
            "resolved": state_counts.get("resolved").copied().unwrap_or(0),
            "unknown": state_counts.get("unknown").copied().unwrap_or(0),
            "receipts_present": report.outcomes.iter().filter(|outcome| receipt_lifecycle_state_is_present(&outcome.receipt_state)).count(),
            "receipts_missing_after_input": report.outcomes.iter().filter(|outcome| outcome.receipt_state == RECEIPT_MISSING).count(),
            "orphaned_receipts": report.orphaned_receipts.len(),
        },
        "movement_front": actionable_gap_outcomes_movement_front_json(&movement_front),
        "outcomes": report.outcomes.iter().map(actionable_gap_outcome_json).collect::<Vec<_>>(),
        "orphaned_receipts": report.orphaned_receipts.iter().map(actionable_gap_orphaned_receipt_json).collect::<Vec<_>>(),
        "must_not_infer": [
            "outcome reports join existing artifacts; they do not execute repairs",
            "raw findings remain supporting evidence, not user work",
            "targeted-test outcomes are static evidence movement, not mutation proof",
            "missing receipts do not imply a repair failed",
            "orphaned receipts do not create new actionable gaps"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render actionable-gap outcomes JSON: {err}"))
}

fn actionable_gap_outcomes_movement_front(
    report: &ActionableGapOutcomesReport,
    state_counts: &BTreeMap<String, usize>,
) -> ActionableGapOutcomeMovementFront {
    let resolved = state_counts.get("resolved").copied().unwrap_or(0);
    let improved = state_counts.get("evidence_improved").copied().unwrap_or(0);
    let unchanged_after_attempt = state_counts.get("evidence_unchanged").copied().unwrap_or(0);
    let missing_receipts = report
        .outcomes
        .iter()
        .filter(|outcome| outcome.receipt_state == RECEIPT_MISSING)
        .count();
    let orphaned_receipts = report.orphaned_receipts.len();
    ActionableGapOutcomeMovementFront {
        current_actionable_count: report.packets_total,
        receipt_linked_actionable_delta: -(resolved as i64),
        resolved,
        improved,
        unchanged_after_attempt,
        missing_receipts,
        orphaned_receipts,
        top_blocked_reason: actionable_gap_outcomes_top_blocked_reason(
            state_counts,
            missing_receipts,
            orphaned_receipts,
        ),
    }
}

fn actionable_gap_outcomes_top_blocked_reason(
    state_counts: &BTreeMap<String, usize>,
    missing_receipts: usize,
    orphaned_receipts: usize,
) -> String {
    for (reason, count) in [
        (
            "evidence_regressed",
            state_counts.get("evidence_regressed").copied().unwrap_or(0),
        ),
        ("missing_receipts", missing_receipts),
        ("orphaned_receipts", orphaned_receipts),
        (
            "attempted_no_receipt",
            state_counts
                .get("attempted_no_receipt")
                .copied()
                .unwrap_or(0),
        ),
        (
            "unchanged_after_attempt",
            state_counts.get("evidence_unchanged").copied().unwrap_or(0),
        ),
        (
            "not_attempted",
            state_counts.get("not_attempted").copied().unwrap_or(0),
        ),
    ] {
        if count > 0 {
            return reason.to_string();
        }
    }
    "none".to_string()
}

fn actionable_gap_outcomes_movement_front_json(front: &ActionableGapOutcomeMovementFront) -> Value {
    serde_json::json!({
        "current_actionable_count": front.current_actionable_count,
        "receipt_linked_actionable_delta": front.receipt_linked_actionable_delta,
        "resolved": front.resolved,
        "improved": front.improved,
        "unchanged_after_attempt": front.unchanged_after_attempt,
        "missing_receipts": front.missing_receipts,
        "orphaned_receipts": front.orphaned_receipts,
        "top_blocked_reason": front.top_blocked_reason,
    })
}

fn actionable_gap_outcome_json(outcome: &ActionableGapOutcome) -> Value {
    serde_json::json!({
        "canonical_gap_id": outcome.canonical_gap_id,
        "evidence_class": outcome.evidence_class,
        "repair_kind": outcome.repair_kind,
        "source_file": outcome.source_file,
        "verify_command": outcome.verify_command,
        "verify_result": outcome.verify_result,
        "receipt_command": outcome.receipt_command,
        "receipt_command_or_path": outcome.receipt_command_or_path,
        "receipt_state": outcome.receipt_state,
        "outcome_state": outcome.outcome_state,
        "timestamp": outcome.timestamp,
        "attempt_instance": outcome.attempt_instance,
        "seam_id": outcome.seam_id,
        "before": outcome.before,
        "after": outcome.after,
        "movement_source": outcome.movement_source,
        "movement_direction": outcome.movement_direction,
        "evidence_delta": outcome.evidence_delta,
        "reason": outcome.reason,
    })
}

fn actionable_gap_orphaned_receipt_json(receipt: &ActionableGapOrphanedReceipt) -> Value {
    serde_json::json!({
        "receipt_id": receipt.receipt_id,
        "seam_id": receipt.seam_id,
        "source_file": receipt.source_file,
        "line": receipt.line,
        "movement_direction": receipt.movement_direction,
        "reason": receipt.reason,
    })
}

fn actionable_gap_outcome_state_counts(
    outcomes: &[ActionableGapOutcome],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for state in [
        "not_attempted",
        "attempted_no_receipt",
        "receipt_present",
        "evidence_improved",
        "evidence_unchanged",
        "evidence_regressed",
        "resolved",
        "unknown",
    ] {
        counts.insert(state.to_string(), 0);
    }
    for outcome in outcomes {
        audit_increment(&mut counts, &outcome.outcome_state);
    }
    counts
}

pub(crate) fn actionable_gap_outcomes_markdown(report: &ActionableGapOutcomesReport) -> String {
    let mut out = String::new();
    let state_counts = actionable_gap_outcome_state_counts(&report.outcomes);
    let movement_front = actionable_gap_outcomes_movement_front(report, &state_counts);
    out.push_str("# Actionable Gap Outcomes\n\n");
    out.push_str("Advisory Lane 1 join from actionable-gap packets to optional receipt and targeted-test outcome artifacts.\n\n");
    out.push_str("## Movement Since Prior Refresh\n\n");
    out.push_str("This front section is receipt-linked static movement. It does not claim runtime adequacy, mutation proof, policy eligibility, gate passage, or merge readiness.\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| --- | ---: |\n");
    out.push_str(&format!(
        "| Current actionable count | {} |\n",
        movement_front.current_actionable_count
    ));
    out.push_str(&format!(
        "| Receipt-linked actionable delta | {} |\n",
        movement_front.receipt_linked_actionable_delta
    ));
    out.push_str(&format!("| Resolved | {} |\n", movement_front.resolved));
    out.push_str(&format!("| Improved | {} |\n", movement_front.improved));
    out.push_str(&format!(
        "| Unchanged after attempt | {} |\n",
        movement_front.unchanged_after_attempt
    ));
    out.push_str(&format!(
        "| Missing receipts | {} |\n",
        movement_front.missing_receipts
    ));
    out.push_str(&format!(
        "| Orphaned receipts | {} |\n",
        movement_front.orphaned_receipts
    ));
    out.push_str(&format!(
        "| Top blocked reason | {} |\n\n",
        audit_markdown_cell(&movement_front.top_blocked_reason)
    ));
    out.push_str("## Inputs\n\n");
    out.push_str(&format!(
        "- actionable gaps: `{}`\n",
        audit_markdown_cell(&report.actionable_gaps_path)
    ));
    out.push_str(&format!(
        "- agent receipt: `{}`\n",
        audit_markdown_cell(
            report
                .agent_receipt_path
                .as_deref()
                .unwrap_or("not provided")
        )
    ));
    out.push_str(&format!(
        "- targeted-test outcome: `{}`\n\n",
        audit_markdown_cell(
            report
                .targeted_test_outcome_path
                .as_deref()
                .unwrap_or("not provided")
        )
    ));

    out.push_str("## Summary\n\n");
    out.push_str("| State | Count |\n");
    out.push_str("| --- | ---: |\n");
    for state in [
        "not_attempted",
        "attempted_no_receipt",
        "receipt_present",
        "evidence_improved",
        "evidence_unchanged",
        "evidence_regressed",
        "resolved",
        "unknown",
    ] {
        audit_push_count(
            &mut out,
            state,
            state_counts.get(state).copied().unwrap_or(0),
        );
    }
    audit_push_count(
        &mut out,
        "orphaned_receipts",
        report.orphaned_receipts.len(),
    );
    out.push('\n');

    out.push_str("## Outcomes\n\n");
    if report.outcomes.is_empty() {
        out.push_str("No actionable-gap packets were present.\n");
    } else {
        for outcome in &report.outcomes {
            out.push_str(&format!(
                "### `{}`\n\n",
                audit_markdown_cell(&outcome.canonical_gap_id)
            ));
            out.push_str("| Field | Value |\n");
            out.push_str("| --- | --- |\n");
            out.push_str(&format!(
                "| Outcome state | `{}` |\n",
                audit_markdown_cell(&outcome.outcome_state)
            ));
            out.push_str(&format!(
                "| Evidence class | `{}` |\n",
                audit_markdown_cell(&outcome.evidence_class)
            ));
            out.push_str(&format!(
                "| Repair kind | `{}` |\n",
                audit_markdown_cell(&outcome.repair_kind)
            ));
            out.push_str(&format!(
                "| Verify command | `{}` |\n",
                audit_markdown_cell(&outcome.verify_command)
            ));
            out.push_str(&format!(
                "| Receipt state | `{}` |\n",
                audit_markdown_cell(&outcome.receipt_state)
            ));
            out.push_str(&format!(
                "| Movement source | {} |\n",
                audit_markdown_cell(outcome.movement_source.as_deref().unwrap_or("none"))
            ));
            out.push_str(&format!(
                "| Movement | {} |\n",
                audit_markdown_cell(
                    outcome
                        .movement_direction
                        .as_deref()
                        .unwrap_or("no movement artifact")
                )
            ));
            out.push_str(&format!(
                "| Reason | {} |\n\n",
                audit_markdown_cell(&outcome.reason)
            ));
        }
    }
    if !report.orphaned_receipts.is_empty() {
        out.push_str("## Orphaned Receipts\n\n");
        out.push_str("Receipts in the input that did not match any current actionable canonical gap packet remain visible here. They do not create new actionable gaps.\n\n");
        out.push_str("| Receipt | Seam | Location | Movement | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for receipt in &report.orphaned_receipts {
            let location = match (&receipt.source_file, receipt.line) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.clone(),
                (None, Some(line)) => format!("line {line}"),
                (None, None) => "unknown".to_string(),
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                audit_markdown_cell(&receipt.receipt_id),
                audit_markdown_cell(receipt.seam_id.as_deref().unwrap_or("unknown")),
                audit_markdown_cell(&location),
                audit_markdown_cell(receipt.movement_direction.as_deref().unwrap_or("unknown")),
                audit_markdown_cell(&receipt.reason)
            ));
        }
        out.push('\n');
    }
    out.push_str("This report is advisory and does not run repairs, generate tests, execute mutation testing, or change public badge semantics.\n");
    out
}

fn audit_top_counts_json(rows: &[Lane1EvidenceAuditTopCount]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "label": row.label,
                "count": row.count,
            })
        })
        .collect()
}

pub(crate) fn audit_count_rows_json(counts: &BTreeMap<String, usize>) -> Vec<Value> {
    counts
        .iter()
        .map(|(label, count)| {
            serde_json::json!({
                "label": label,
                "count": count,
            })
        })
        .collect()
}

pub(crate) fn audit_count_rows_map(value: &Value, path: &[&str]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in audit_array(value, path) {
        let Some(label) = audit_non_empty_string(row, &["label"]) else {
            continue;
        };
        let count = audit_usize(row, &["count"]).unwrap_or_default();
        if count > 0 {
            counts.insert(label, count);
        }
    }
    counts
}

fn audit_finding_alignment_summary_json(
    summary: &Lane1EvidenceAuditFindingAlignmentSummary,
) -> Value {
    let mut object = serde_json::Map::new();
    audit_insert_usize(&mut object, "raw_findings", summary.raw_findings_total);
    audit_insert_usize(&mut object, "raw_signals", summary.raw_signals_total);
    audit_insert_usize(
        &mut object,
        "canonical_items",
        summary.canonical_items_total,
    );
    audit_insert_usize(
        &mut object,
        "aligned_raw_findings",
        summary.aligned_raw_findings_total,
    );
    audit_insert_usize(
        &mut object,
        "unaligned_raw_findings",
        summary.unaligned_raw_findings_total,
    );
    object.insert(
        "raw_to_canonical_ratio".to_string(),
        audit_finding_alignment_raw_to_canonical_ratio(summary)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    audit_insert_usize(
        &mut object,
        "duplicate_groups_total",
        summary.duplicate_groups_total,
    );
    audit_insert_usize(
        &mut object,
        "actionable_gaps",
        summary.actionable_items_total,
    );
    audit_insert_usize(
        &mut object,
        "already_observed",
        summary.already_observed_total,
    );
    audit_insert_usize(
        &mut object,
        "internal_no_action",
        summary.internal_no_action_total,
    );
    audit_insert_usize(
        &mut object,
        "static_limitations",
        summary.static_limitation_total,
    );
    audit_insert_usize(&mut object, "unknown", summary.unknown_total);
    audit_insert_usize(
        &mut object,
        "calibrated_supported",
        summary.calibrated_supported_total,
    );
    audit_insert_usize(&mut object, "uncalibrated", summary.uncalibrated_total);
    audit_insert_usize(
        &mut object,
        "presentation_text_total",
        summary.presentation_text_total,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_user_visible",
        summary.presentation_text_user_visible,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_observed",
        summary.presentation_text_observed,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_unobserved",
        summary.presentation_text_unobserved,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_internal_only",
        summary.presentation_text_internal_only,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_visibility_unknown",
        summary.presentation_text_visibility_unknown,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_observer_unknown",
        summary.presentation_text_observer_unknown,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_duplicate_groups",
        summary.presentation_text_duplicate_groups,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_actionable_snapshot",
        summary.presentation_text_actionable_snapshot,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_actionable_output_repairs",
        summary.presentation_text_actionable_total,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_no_action",
        summary.presentation_text_no_action,
    );
    audit_insert_usize(
        &mut object,
        "presentation_text_static_limitations",
        summary.presentation_text_static_limitations,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_raw_findings_total",
        summary.raw_findings_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_raw_signals_total",
        summary.raw_signals_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_canonical_items_total",
        summary.canonical_items_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_aligned_raw_findings_total",
        summary.aligned_raw_findings_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_unaligned_raw_findings_total",
        summary.unaligned_raw_findings_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_duplicate_groups_total",
        summary.duplicate_groups_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_actionable_items_total",
        summary.actionable_items_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_actionable_unresolved_canonical_gaps",
        summary.actionable_unresolved_canonical_gaps,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_already_observed_total",
        summary.already_observed_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_internal_only_total",
        summary.internal_only_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_internal_no_action_total",
        summary.internal_no_action_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_static_limitation_total",
        summary.static_limitation_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_unknown_total",
        summary.unknown_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_calibrated_supported_total",
        summary.calibrated_supported_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_uncalibrated_total",
        summary.uncalibrated_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_visibility_unknown_total",
        summary.visibility_unknown_total,
    );
    audit_insert_usize(
        &mut object,
        "finding_alignment_presentation_text_actionable_total",
        summary.presentation_text_actionable_total,
    );
    Value::Object(object)
}

fn lane1_evidence_audit_repo_exposure_generation_json(
    generation: &Lane1EvidenceAuditRepoExposureGeneration,
) -> Value {
    serde_json::json!({
        "command": normalize_report_path(&generation.command),
        "timeout_ms": generation.timeout_ms,
        "status": generation.status,
        "failure_reason": generation.failure_reason,
        "duration_ms": generation.duration_ms,
        "exit_code": generation.exit_code,
        "stdout_bytes": generation.stdout_bytes,
        "stderr_bytes": generation.stderr_bytes,
        "latency_trace_events_total": generation.latency_trace_events_total,
        "latency_trace_tail": generation
            .latency_trace_tail
            .iter()
            .map(lane1_evidence_audit_latency_trace_json)
            .collect::<Vec<_>>(),
    })
}

fn lane1_evidence_audit_run_limitation_json(limitation: &Lane1EvidenceAuditRunLimitation) -> Value {
    let runtime_status = lane1_runtime_status_from_run_limitation(limitation);
    serde_json::json!({
        "category": limitation.category,
        "run_status": runtime_status.state.clone(),
        "phase": limitation.phase,
        "input": limitation.input,
        "input_kind": runtime_status.input_kind,
        "input_path": runtime_status.input_path,
        "observed_seams": limitation.observed_seams,
        "cache_limit": limitation.cache_limit,
        "summary": limitation.summary,
        "repair_route": limitation.repair_route,
        "timeout_ms": limitation.timeout_ms,
        "limit_ms": runtime_status.limit_ms,
        "duration_ms": limitation.duration_ms,
        "downstream_consumable": runtime_status.downstream_consumable,
        "command": limitation.command.as_ref().map(|command| normalize_report_path(command)),
        "exit_code": limitation.exit_code,
        "stdout_bytes": limitation.stdout_bytes,
        "stderr_bytes": limitation.stderr_bytes,
        "latency_trace_tail": limitation
            .latency_trace_tail
            .iter()
            .map(lane1_evidence_audit_latency_trace_json)
            .collect::<Vec<_>>(),
    })
}

fn lane1_evidence_audit_latency_trace_json(trace: &RepoExposureLatencyTrace) -> Value {
    repo_exposure_latency_trace_json(trace)
}

pub(crate) fn repo_exposure_latency_trace_json(trace: &RepoExposureLatencyTrace) -> Value {
    serde_json::json!({
        "phase": trace.phase,
        "status": trace.status,
        "duration_ms": trace.duration_ms,
    })
}

fn audit_alignment_class_coverage_json(row: &Lane1EvidenceAuditAlignmentClassCoverage) -> Value {
    serde_json::json!({
        "evidence_class": row.evidence_class,
        "raw_findings": row.raw_findings,
        "canonical_items": row.canonical_items,
        "aligned_raw_findings": row.aligned_raw_findings,
        "unaligned_raw_findings": row.unaligned_raw_findings,
        "actionable_items": row.actionable_items,
        "already_observed_items": row.already_observed_items,
        "internal_no_action_items": row.internal_no_action_items,
        "static_limitation_items": row.static_limitation_items,
        "unknown_items": row.unknown_items,
        "static_limitation_categories": row.static_limitation_categories,
        "static_limitation_repair_routes": row.static_limitation_repair_routes,
    })
}

fn audit_evidence_class_work_item_json(row: &Lane1EvidenceClassWorkItem) -> Value {
    serde_json::json!({
        "evidence_class": row.evidence_class,
        "work_score": row.work_score,
        "dominant_signal": row.dominant_signal,
        "dominant_static_limitation_category": row.dominant_static_limitation_category,
        "dominant_static_limitation_category_count": row.dominant_static_limitation_category_count,
        "dominant_static_limitation_repair_route": row.dominant_static_limitation_repair_route,
        "raw_findings": row.raw_findings,
        "canonical_items": row.canonical_items,
        "duplicate_raw_signals": row.duplicate_raw_signals,
        "actionable_items": row.actionable_items,
        "static_limitation_items": row.static_limitation_items,
        "unknown_items": row.unknown_items,
        "unaligned_raw_findings": row.unaligned_raw_findings,
        "next_repair": row.next_repair,
    })
}

fn audit_runtime_confidence_by_class_json(
    rows: &[Lane1EvidenceAuditRuntimeConfidenceClassCoverage],
) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "evidence_class": row.evidence_class,
                "canonical_items": row.canonical_items,
                "calibrated_supported": row.calibrated_supported,
                "fixture_backed": row.fixture_backed,
                "static_only": row.static_only,
                "unknown_confidence": row.unknown_confidence,
                "uncalibrated": row.uncalibrated,
                "actionable_items": row.actionable_items,
                "static_limitation_items": row.static_limitation_items,
            })
        })
        .collect()
}

fn audit_unaligned_example_json(example: &Lane1EvidenceAuditUnalignedExample) -> Value {
    serde_json::json!({
        "evidence_class": example.evidence_class,
        "file": example.file,
        "line": example.line,
        "kind": example.kind,
        "expression": example.expression,
        "reason": example.reason,
    })
}

fn audit_same_line_duplicate_group_json(group: &Lane1EvidenceAuditSameLineDuplicateGroup) -> Value {
    serde_json::json!({
        "file": group.file,
        "line": group.line,
        "raw_findings": group.raw_findings,
        "evidence_classes": group.evidence_classes,
        "kinds": group.kinds,
        "example_expression": group.example_expression,
    })
}

fn audit_finding_alignment_raw_to_canonical_ratio(
    summary: &Lane1EvidenceAuditFindingAlignmentSummary,
) -> Option<f64> {
    if summary.canonical_items_total == 0 {
        return None;
    }
    Some(summary.raw_signals_total as f64 / summary.canonical_items_total as f64)
}

fn audit_insert_usize(object: &mut serde_json::Map<String, Value>, key: &str, value: usize) {
    object.insert(key.to_string(), Value::from(value));
}

pub(crate) fn audit_get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

pub(crate) fn audit_string(value: &Value, path: &[&str]) -> Option<String> {
    audit_get(value, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn audit_non_empty_string(value: &Value, path: &[&str]) -> Option<String> {
    audit_string(value, path).filter(|text| !text.trim().is_empty())
}

fn audit_json_array_owned(value: &Value, path: &[&str]) -> Option<Vec<Value>> {
    audit_get(value, path).and_then(|value| value.as_array().cloned())
}

pub(crate) fn audit_string_array(value: &Value, path: &[&str]) -> Option<Vec<String>> {
    Some(
        audit_get(value, path)?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}

fn audit_actionable_gap_primary_anchor(
    record: &Value,
    canonical_item: &Value,
    file: &str,
) -> Value {
    if let Some(anchor) = audit_get(canonical_item, &["primary_anchor"]) {
        return anchor.clone();
    }
    let line = audit_usize(record, &["location", "line"]).or_else(|| {
        audit_get(canonical_item, &["raw_findings"])
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| audit_usize(item, &["line"]))
    });
    serde_json::json!({
        "file": audit_non_empty_string(record, &["location", "file"])
            .unwrap_or_else(|| file.to_string()),
        "line": line,
    })
}

fn audit_actionable_gap_repair_route_fields(
    record: &Value,
    canonical_item: &Value,
) -> (String, String, String, String, Option<Value>) {
    let repair_kind = audit_non_empty_string(canonical_item, &["repair_route", "repair_kind"])
        .filter(|value| !audit_guidance_field_is_missing(value))
        .unwrap_or_else(|| "repair_route_unknown".to_string());
    let target_test_type =
        audit_non_empty_string(canonical_item, &["repair_route", "target_test_type"])
            .filter(|value| !audit_guidance_field_is_missing(value))
            .unwrap_or_else(|| "target_test_type_unknown".to_string());
    let canonical_assertion_shape =
        audit_non_empty_string(canonical_item, &["repair_route", "suggested_assertion"])
            .or_else(|| {
                audit_non_empty_string(canonical_item, &["repair_route", "assertion_shape"])
            })
            .filter(|value| !audit_guidance_field_is_missing(value));
    let assertion_shape = canonical_assertion_shape
        .clone()
        .or_else(|| {
            audit_non_empty_string(record, &["recommendation", "assertion_shape", "kind"])
                .filter(|value| !audit_guidance_field_is_missing(value))
        })
        .unwrap_or_else(|| "assertion_shape_unknown".to_string());
    let source = if repair_kind != "repair_route_unknown"
        && target_test_type != "target_test_type_unknown"
        && canonical_assertion_shape.is_some()
    {
        "canonical_item.repair_route"
    } else {
        "missing"
    };
    let repair_route = if source == "canonical_item.repair_route" {
        Some(serde_json::json!({
            "repair_kind": repair_kind.clone(),
            "target_test_type": target_test_type.clone(),
            "assertion_shape": assertion_shape.clone(),
        }))
    } else {
        None
    };
    (
        repair_kind,
        target_test_type,
        assertion_shape,
        source.to_string(),
        repair_route,
    )
}

pub(crate) fn audit_actionable_gap_target_test_shape(
    target_test_type: &str,
    assertion_shape: &str,
) -> String {
    if audit_guidance_field_is_missing(target_test_type)
        || audit_guidance_field_is_missing(assertion_shape)
        || target_test_type == "target_test_type_unknown"
        || assertion_shape == "assertion_shape_unknown"
    {
        "target_test_shape_unknown".to_string()
    } else {
        format!("{target_test_type}: {assertion_shape}")
    }
}

pub(crate) fn audit_actionable_gap_verify_command_with_source(
    record: &Value,
    canonical_item: &Value,
) -> (String, String) {
    let mut unbounded_command = None;
    if let Some(command) = audit_non_empty_string(canonical_item, &["verify_command"])
        .filter(|value| !audit_guidance_field_is_missing(value))
        .filter(|value| value.trim() != "verify_command_unknown")
    {
        if audit_verify_command_is_unbounded_repo_exposure_snapshot_compare(&command) {
            unbounded_command = Some((command, "canonical_item.verify_command".to_string()));
        } else {
            return (command, "canonical_item.verify_command".to_string());
        }
    }

    if let Some(command) = audit_non_empty_string(record, &["recommendation", "verify_command"])
        .filter(|value| !audit_guidance_field_is_missing(value))
        .filter(|value| value.trim() != "verify_command_unknown")
    {
        if audit_verify_command_is_unbounded_repo_exposure_snapshot_compare(&command) {
            unbounded_command = unbounded_command.or_else(|| {
                Some((
                    command,
                    "evidence_record.recommendation.verify_command".to_string(),
                ))
            });
        } else {
            return (
                command,
                "evidence_record.recommendation.verify_command".to_string(),
            );
        }
    }

    if let Some(command) = audit_actionable_gap_bounded_verify_command_from_related_target(
        audit_actionable_gap_related_test_or_observer(record, canonical_item).as_ref(),
    ) {
        return (command, "related_test_or_observer.name".to_string());
    }

    if let Some(command) = unbounded_command {
        return command;
    }
    ("verify_command_unknown".to_string(), "missing".to_string())
}

fn audit_actionable_gap_bounded_verify_command_from_related_target(
    related_test_or_observer: Option<&Value>,
) -> Option<String> {
    let related = related_test_or_observer?;
    let file = ripr_swarm_plan_related_target_file(related)?;
    let package = audit_actionable_gap_cargo_package_from_target_file(&file)?;
    let test_name = audit_actionable_gap_related_target_name(related)?;
    Some(format!("cargo test -p {package} {test_name}"))
}

fn audit_actionable_gap_cargo_package_from_target_file(file: &str) -> Option<String> {
    let normalized = file.trim().replace('\\', "/");
    let package = if let Some(rest) = normalized.strip_prefix("crates/") {
        rest.split('/').next()
    } else if normalized.starts_with("xtask/") {
        Some("xtask")
    } else {
        None
    }?;
    audit_actionable_gap_safe_cargo_token(package)
}

fn audit_actionable_gap_related_target_name(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => object
            .get("name")
            .and_then(Value::as_str)
            .and_then(audit_actionable_gap_safe_cargo_test_filter),
        Value::Array(values) => values
            .iter()
            .find_map(audit_actionable_gap_related_target_name),
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

fn audit_actionable_gap_safe_cargo_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
    .then(|| trimmed.to_string())
}

fn audit_actionable_gap_safe_cargo_test_filter(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '-')))
    .then(|| trimmed.to_string())
}

fn audit_actionable_gap_receipt_command_or_path(
    record: &Value,
    canonical_item: &Value,
) -> (Option<String>, String) {
    let candidates = [
        (
            canonical_item,
            &["receipt_command"][..],
            "canonical_item.receipt_command",
        ),
        (
            canonical_item,
            &["receipt_path"][..],
            "canonical_item.receipt_path",
        ),
        (
            canonical_item,
            &["receipt_state_path"][..],
            "canonical_item.receipt_state_path",
        ),
        (
            canonical_item,
            &["receipt", "path"][..],
            "canonical_item.receipt.path",
        ),
        (
            record,
            &["receipt_command"][..],
            "evidence_record.receipt_command",
        ),
        (
            record,
            &["receipt_path"][..],
            "evidence_record.receipt_path",
        ),
        (
            record,
            &["receipt_state_path"][..],
            "evidence_record.receipt_state_path",
        ),
        (
            record,
            &["receipt", "path"][..],
            "evidence_record.receipt.path",
        ),
    ];

    for (value, path, source) in candidates {
        if let Some(command_or_path) = audit_non_empty_string(value, path)
            .filter(|value| !audit_guidance_field_is_missing(value))
        {
            return (Some(command_or_path), source.to_string());
        }
    }

    (None, "missing".to_string())
}

pub(crate) struct AuditActionableGapProjectionInput<'a> {
    pub(crate) canonical_gap_id: &'a str,
    pub(crate) gap_state: &'a str,
    pub(crate) actionability: &'a str,
    pub(crate) repair_kind: &'a str,
    pub(crate) target_test_type: &'a str,
    pub(crate) assertion_shape: &'a str,
    pub(crate) target_test_shape: &'a str,
    pub(crate) repair_route_present: bool,
    pub(crate) repair_route_source: &'a str,
    pub(crate) verify_command: &'a str,
    pub(crate) verify_command_source: &'a str,
    pub(crate) receipt_command_or_path: Option<&'a str>,
    pub(crate) receipt_source: &'a str,
    pub(crate) typed_related_target_available: bool,
    pub(crate) has_stable_canonical_gap_id: bool,
    pub(crate) confidence_basis: &'a str,
    pub(crate) must_not_change_count: usize,
    pub(crate) allowed_edit_surface_count: usize,
    pub(crate) raw_evidence_refs_count: usize,
    pub(crate) static_limitations_count: usize,
    pub(crate) cross_language_target_unresolved: bool,
}

pub(crate) fn audit_actionable_gap_projection_exclusion_reasons(
    input: AuditActionableGapProjectionInput<'_>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if !input.has_stable_canonical_gap_id {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_canonical_gap_id");
    }
    if input.canonical_gap_id.trim().is_empty() {
        audit_push_projection_exclusion_reason(&mut reasons, "malformed");
    }
    match input.gap_state.trim() {
        "actionable" => {}
        "unresolved" | "static_limitation" | "advisory" => {
            audit_push_projection_exclusion_reason(&mut reasons, "not_actionable_gap_state");
        }
        "already_observed" | "observed" | "resolved" => {
            audit_push_projection_exclusion_reason(&mut reasons, "already_observed");
        }
        "internal_only" | "no_action" => {
            audit_push_projection_exclusion_reason(&mut reasons, "no_action");
        }
        "" | "gap_state_unknown" | "unknown" => {
            audit_push_projection_exclusion_reason(&mut reasons, "malformed");
        }
        _ => {
            audit_push_projection_exclusion_reason(&mut reasons, "malformed");
        }
    }
    let actionability = input.actionability.trim();
    if audit_guidance_field_is_missing(actionability)
        || matches!(actionability, "actionability_unknown" | "unknown")
    {
        audit_push_projection_exclusion_reason(&mut reasons, "malformed");
    } else if actionability.contains("suppressed") {
        audit_push_projection_exclusion_reason(&mut reasons, "suppressed");
    } else if actionability.contains("intentional") {
        audit_push_projection_exclusion_reason(&mut reasons, "intentional");
    } else if actionability.contains("already_observed") || actionability.contains("observed") {
        audit_push_projection_exclusion_reason(&mut reasons, "already_observed");
    } else if actionability.contains("no_action") || actionability.contains("internal") {
        audit_push_projection_exclusion_reason(&mut reasons, "no_action");
    }
    if !input.repair_route_present
        || input.repair_route_source != "canonical_item.repair_route"
        || input.target_test_type == "target_test_type_unknown"
        || input.assertion_shape == "assertion_shape_unknown"
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_repair_route");
    }
    if audit_guidance_field_is_missing(input.repair_kind)
        || input.repair_kind == "repair_route_unknown"
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_repair_kind");
    }
    if input.target_test_shape == "target_test_shape_unknown"
        || audit_guidance_field_is_missing(input.target_test_shape)
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_target_test_shape");
    }
    if !matches!(
        input.verify_command_source,
        "canonical_item.verify_command" | "related_test_or_observer.name"
    ) || audit_guidance_field_is_missing(input.verify_command)
        || input.verify_command.trim() == "verify_command_unknown"
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_verify_command");
    } else if audit_verify_command_is_unbounded_repo_exposure_snapshot_compare(input.verify_command)
    {
        audit_push_projection_exclusion_reason(&mut reasons, "unbounded_verify_command");
    }
    if input
        .receipt_command_or_path
        .is_none_or(audit_guidance_field_is_missing)
        || !input.receipt_source.ends_with(".receipt_command")
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_receipt_command");
    }
    if !input.typed_related_target_available {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_related_test_or_observer");
    }
    if audit_guidance_field_is_missing(input.confidence_basis)
        || matches!(
            input.confidence_basis.trim(),
            "unknown" | "confidence_basis_unknown"
        )
    {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_confidence");
    }
    if input.must_not_change_count == 0 {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_must_not_change");
    }
    if input.allowed_edit_surface_count == 0 {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_allowed_edit_surface");
    }
    if input.cross_language_target_unresolved {
        audit_push_projection_exclusion_reason(
            &mut reasons,
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY,
        );
    }
    if input.raw_evidence_refs_count == 0 {
        audit_push_projection_exclusion_reason(&mut reasons, "missing_raw_evidence_refs");
    }
    if input.static_limitations_count > 0 {
        audit_push_projection_exclusion_reason(&mut reasons, "static_limitation_present");
    }
    reasons
}

pub(crate) fn audit_structured_raw_evidence_refs_count(values: &[Value]) -> usize {
    values
        .iter()
        .filter(|value| audit_raw_evidence_ref_is_structured(value))
        .count()
}

fn audit_raw_evidence_ref_is_structured(value: &Value) -> bool {
    let has_anchor = audit_non_empty_string(value, &["file"])
        .or_else(|| audit_non_empty_string(value, &["path"]))
        .or_else(|| audit_non_empty_string(value, &["source_file"]))
        .is_some();
    let has_identity = audit_non_empty_string(value, &["kind"])
        .or_else(|| audit_non_empty_string(value, &["source_id"]))
        .or_else(|| audit_non_empty_string(value, &["evidence_record_ref"]))
        .or_else(|| audit_non_empty_string(value, &["canonical_gap_id"]))
        .is_some();
    has_anchor && has_identity
}

pub(crate) fn audit_verify_command_is_unbounded_repo_exposure_snapshot_compare(
    command: &str,
) -> bool {
    let normalized = command.trim().replace('\\', "/");
    normalized.contains("ripr agent verify")
        && normalized.contains("--before")
        && normalized.contains("--after")
        && normalized.contains("repo-exposure.json")
}

pub(crate) fn audit_push_projection_exclusion_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.to_string());
    }
}

fn audit_static_limitations_has_category(values: &[Value], category: &str) -> bool {
    values.iter().any(|value| {
        value.as_str().is_some_and(|value| value == category)
            || audit_non_empty_string(value, &["category"]).is_some_and(|value| value == category)
    })
}

fn audit_actionable_gap_related_test_or_observer(
    record: &Value,
    canonical_item: &Value,
) -> Option<Value> {
    audit_get(canonical_item, &["related_test"])
        .or_else(|| audit_get(canonical_item, &["related_test_or_observer"]))
        .or_else(|| audit_get(canonical_item, &["observer"]))
        .or_else(|| audit_get(record, &["recommendation", "recommended_test"]))
        .cloned()
}

fn audit_actionable_gap_allowed_edit_surface(
    workspace_root: Option<&Path>,
    record: &Value,
    canonical_item: &Value,
    related_test_or_observer: &Option<Value>,
    cross_language_target_unresolved: bool,
) -> Vec<String> {
    if cross_language_target_unresolved {
        return Vec::new();
    }

    let mut values = audit_string_array(canonical_item, &["allowed_edit_surface"])
        .or_else(|| audit_string_array(record, &["allowed_edit_surface"]))
        .unwrap_or_default();
    if values.is_empty()
        && let Some(target) = related_test_or_observer
            .as_ref()
            .and_then(ripr_swarm_plan_related_target_file)
    {
        values.push(target);
    }
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            audit_existing_workspace_file_token(workspace_root, &value).or_else(|| {
                workspace_root
                    .is_none()
                    .then(|| ripr_swarm_attempt_workspace_relative_file_token(&value))?
            })
        })
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn audit_existing_workspace_file_token(
    workspace_root: Option<&Path>,
    value: &str,
) -> Option<String> {
    let token = ripr_swarm_attempt_workspace_relative_file_token(value)?;
    let root = workspace_root?;
    root.join(&token).is_file().then_some(token)
}

fn audit_actionable_gap_candidate_value_or_observer(
    record: &Value,
    canonical_item: &Value,
) -> Option<String> {
    audit_non_empty_string(canonical_item, &["candidate_value_or_observer"])
        .or_else(|| audit_non_empty_string(canonical_item, &["observer"]))
        .or_else(|| {
            audit_array(record, &["recommendation", "candidate_values"])
                .iter()
                .find_map(|candidate| {
                    audit_non_empty_string(candidate, &["value"])
                        .or_else(|| audit_non_empty_string(candidate, &["observer"]))
                        .or_else(|| audit_non_empty_string(candidate, &["reason"]))
                })
        })
        .or_else(|| {
            audit_array(record, &["missing_discriminators"])
                .iter()
                .find_map(|missing| {
                    audit_non_empty_string(missing, &["value"])
                        .or_else(|| audit_non_empty_string(missing, &["flow_sink", "kind"]))
                        .or_else(|| audit_non_empty_string(missing, &["reason"]))
                })
        })
}

fn audit_actionable_gap_missing_discriminators(
    record: &Value,
    canonical_item: &Value,
) -> Vec<Value> {
    audit_json_array_owned(canonical_item, &["missing_discriminators"])
        .or_else(|| audit_json_array_owned(record, &["missing_discriminators"]))
        .unwrap_or_default()
}

fn audit_actionable_gap_missing_discriminator_summary(discriminators: &[Value]) -> String {
    if discriminators.is_empty() {
        return "none".to_string();
    }

    discriminators
        .iter()
        .take(3)
        .map(|discriminator| {
            let value = audit_non_empty_string(discriminator, &["value"])
                .unwrap_or_else(|| "missing_discriminator_value_unknown".to_string());
            let reason = audit_non_empty_string(discriminator, &["reason"]);
            match reason {
                Some(reason) => format!("{value} ({reason})"),
                None => value,
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn default_actionable_gap_packet_must_not_change() -> Vec<String> {
    vec![
        "Do not infer actionability from raw static class.".to_string(),
        "Do not treat raw findings as independent user work.".to_string(),
        "Do not claim mutation execution or runtime proof from this packet.".to_string(),
    ]
}

fn audit_is_actionable_canonical_item(_item_kind: &str, gap_state: &str) -> bool {
    gap_state == "actionable"
}

fn audit_has_structured_repair_route(canonical_item: &Value) -> bool {
    let Some(repair_route) = audit_get(canonical_item, &["repair_route"]) else {
        return false;
    };
    repair_route.is_object()
        && audit_non_empty_string(repair_route, &["repair_kind"])
            .is_some_and(|field| !audit_guidance_field_is_missing(&field))
        && audit_non_empty_string(repair_route, &["target_test_type"])
            .is_some_and(|field| !audit_guidance_field_is_missing(&field))
        && audit_non_empty_string(repair_route, &["suggested_assertion"])
            .is_some_and(|field| !audit_guidance_field_is_missing(&field))
}

pub(crate) fn audit_verify_command_is_missing(canonical_item: &Value) -> bool {
    audit_non_empty_string(canonical_item, &["verify_command"]).is_none_or(|command| {
        audit_guidance_field_is_missing(&command) || command.trim() == "verify_command_unknown"
    })
}

pub(crate) fn audit_guidance_field_is_missing(value: &str) -> bool {
    matches!(value.trim(), "" | "unknown" | "none" | "no_action")
}

pub(crate) fn audit_bool(value: &Value, path: &[&str]) -> Option<bool> {
    audit_get(value, path).and_then(Value::as_bool)
}

pub(crate) fn audit_usize(value: &Value, path: &[&str]) -> Option<usize> {
    audit_get(value, path)
        .and_then(Value::as_u64)
        .map(|number| number as usize)
}

pub(crate) fn audit_array<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    audit_get(value, path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(crate) fn audit_increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    let entry = counts.entry(key.to_string()).or_insert(0);
    *entry += 1;
}

fn audit_top_counts(counts: BTreeMap<String, usize>) -> Vec<Lane1EvidenceAuditTopCount> {
    let mut rows = counts
        .into_iter()
        .map(|(label, count)| Lane1EvidenceAuditTopCount { label, count })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    rows
}

fn audit_alignment_evidence_class(record: &Value, canonical_item: Option<&Value>) -> String {
    canonical_item
        .and_then(|item| audit_non_empty_string(item, &["evidence_class"]))
        .or_else(|| {
            audit_get(record, &["presentation_text"])
                .filter(|value| value.is_object())
                .map(|_| "presentation_text".to_string())
        })
        .or_else(|| audit_non_empty_string(record, &["seam_kind"]))
        .or_else(|| {
            audit_array(record, &["raw_findings"])
                .first()
                .and_then(|raw| {
                    audit_non_empty_string(raw, &["probe_kind"])
                        .or_else(|| audit_non_empty_string(raw, &["kind"]))
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn audit_unaligned_example(
    record: &Value,
    raw_findings: &[Value],
    evidence_class: &str,
) -> Lane1EvidenceAuditUnalignedExample {
    let raw = raw_findings.first();
    Lane1EvidenceAuditUnalignedExample {
        evidence_class: evidence_class.to_string(),
        file: raw
            .and_then(|raw| audit_non_empty_string(raw, &["file"]))
            .or_else(|| audit_non_empty_string(record, &["location", "file"]))
            .unwrap_or_else(|| "unknown".to_string()),
        line: raw
            .and_then(|raw| audit_usize(raw, &["line"]))
            .or_else(|| audit_usize(record, &["location", "line"])),
        kind: raw
            .and_then(|raw| audit_non_empty_string(raw, &["kind"]))
            .or_else(|| raw.and_then(|raw| audit_non_empty_string(raw, &["probe_kind"])))
            .unwrap_or_else(|| "unknown".to_string()),
        expression: raw
            .and_then(|raw| audit_non_empty_string(raw, &["expression"]))
            .unwrap_or_else(|| "unknown".to_string()),
        reason: "missing canonical_item".to_string(),
    }
}

fn audit_has_static_unknown_signal(
    record: &Value,
    canonical_item: &Value,
    gap_state: &str,
    actionability: &str,
) -> bool {
    gap_state == "static_limitation"
        || actionability.contains("static_limitation")
        || audit_array(record, &["raw_findings"]).iter().any(|raw| {
            audit_string(raw, &["kind"]).as_deref() == Some("static_unknown")
                || audit_string(raw, &["probe_kind"]).as_deref() == Some("static_unknown")
        })
        || audit_string(canonical_item, &["evidence_class"]).as_deref() == Some("static_unknown")
}

fn audit_has_named_static_limitation(record: &Value, canonical_item: &Value) -> bool {
    audit_array(record, &["static_limitations"])
        .iter()
        .chain(audit_array(canonical_item, &["static_limitations"]).iter())
        .any(|limitation| {
            let Some(category) = audit_non_empty_string(limitation, &["category"]) else {
                return false;
            };
            let Some(repair_route) = audit_non_empty_string(limitation, &["repair_route"]) else {
                return false;
            };
            audit_static_limitation_category_is_named(&category)
                && audit_static_limitation_repair_route_is_named(&repair_route)
        })
}

fn audit_static_limitation_category_rows(record: &Value) -> Vec<(String, String)> {
    audit_array(record, &["static_limitations"])
        .iter()
        .map(|limitation| {
            let reason = audit_string(limitation, &["reason"])
                .unwrap_or_else(|| "missing_reason".to_string());
            let stage =
                audit_string(limitation, &["stage"]).unwrap_or_else(|| "missing_stage".to_string());
            let state =
                audit_string(limitation, &["state"]).unwrap_or_else(|| "missing_state".to_string());
            let category = audit_string(limitation, &["category"])
                .unwrap_or_else(|| static_limitation_category(&stage, &state, &reason).to_string());
            let repair_route = audit_string(limitation, &["repair_route"])
                .unwrap_or_else(|| static_limitation_repair_route(&category).to_string());
            (category, repair_route)
        })
        .collect()
}

fn audit_ingest_static_limitation_class_coverage<'a>(
    coverage: &mut Lane1EvidenceAuditAlignmentClassCoverage,
    rows: impl Iterator<Item = (&'a str, &'a str)>,
) {
    for (category, repair_route) in rows {
        audit_increment(&mut coverage.static_limitation_categories, category);
        audit_increment(&mut coverage.static_limitation_repair_routes, repair_route);
    }
}

fn audit_static_limitation_category_is_named(category: &str) -> bool {
    !matches!(category.trim(), "" | "static_unknown" | "unknown")
}

fn audit_static_limitation_repair_route_is_named(repair_route: &str) -> bool {
    !matches!(repair_route.trim(), "" | "unknown")
}

fn audit_ingest_finding_alignment(
    record: &Value,
    summary: &mut Lane1EvidenceAuditFindingAlignmentSummary,
) {
    let raw_findings = audit_array(record, &["raw_findings"]);
    let Some(canonical_item) =
        audit_get(record, &["canonical_item"]).filter(|value| value.is_object())
    else {
        summary.raw_findings_total += raw_findings.len();
        summary.raw_signals_total += raw_findings.len();
        summary.unaligned_raw_findings_total += raw_findings.len();
        return;
    };

    let raw_group_size = audit_usize(canonical_item, &["raw_group_size"]).unwrap_or(0);
    let raw_signal_count = raw_group_size.max(raw_findings.len()).max(1);
    summary.raw_findings_total += raw_signal_count;
    summary.raw_signals_total += raw_signal_count;
    summary.canonical_items_total += 1;
    summary.aligned_raw_findings_total += raw_signal_count;
    if raw_signal_count > 1 {
        summary.duplicate_groups_total += 1;
    }

    let item_kind = audit_string(canonical_item, &["canonical_item_kind"]).unwrap_or_default();
    let gap_state = audit_string(canonical_item, &["gap_state"]).unwrap_or_default();
    let actionability = audit_string(canonical_item, &["actionability"]).unwrap_or_default();
    let evidence_class = audit_string(canonical_item, &["evidence_class"]).unwrap_or_default();
    let confidence_basis =
        audit_string(canonical_item, &["confidence", "basis"]).unwrap_or_default();

    if item_kind == "gap" || gap_state == "actionable" {
        summary.actionable_items_total += 1;
        summary.actionable_unresolved_canonical_gaps += 1;
    }
    if item_kind == "observed" || gap_state == "already_observed" {
        summary.already_observed_total += 1;
    }
    if item_kind == "no_action" || gap_state == "internal_only" {
        summary.internal_only_total += 1;
    }
    if item_kind == "no_action" || actionability == "no_action_internal" {
        summary.internal_no_action_total += 1;
    }
    if item_kind == "limitation" || gap_state == "static_limitation" {
        summary.static_limitation_total += 1;
    }
    if gap_state == "unknown" {
        summary.unknown_total += 1;
    }
    if confidence_basis == "calibrated" || confidence_basis == "runtime_calibrated" {
        summary.calibrated_supported_total += 1;
    } else {
        summary.uncalibrated_total += 1;
    }

    let alignment = AuditCanonicalAlignment {
        evidence_class: &evidence_class,
        item_kind: &item_kind,
        gap_state: &gap_state,
        actionability: &actionability,
        raw_signal_count,
    };
    audit_ingest_presentation_text_alignment(record, canonical_item, &alignment, summary);
}

pub(crate) struct AuditCanonicalAlignment<'a> {
    pub(crate) evidence_class: &'a str,
    pub(crate) item_kind: &'a str,
    pub(crate) gap_state: &'a str,
    pub(crate) actionability: &'a str,
    pub(crate) raw_signal_count: usize,
}

fn audit_ingest_presentation_text_alignment(
    record: &Value,
    canonical_item: &Value,
    alignment: &AuditCanonicalAlignment<'_>,
    summary: &mut Lane1EvidenceAuditFindingAlignmentSummary,
) {
    let presentation_text = audit_get(record, &["presentation_text"])
        .filter(|value| value.is_object())
        .or_else(|| {
            audit_get(canonical_item, &["presentation_text"]).filter(|value| value.is_object())
        });
    if alignment.evidence_class != "presentation_text" && presentation_text.is_none() {
        return;
    }

    summary.presentation_text_total += 1;
    if alignment.raw_signal_count > 1 {
        summary.presentation_text_duplicate_groups += 1;
    }
    if alignment.item_kind == "gap" || alignment.gap_state == "actionable" {
        summary.presentation_text_unobserved += 1;
    }
    if alignment.item_kind == "observed" || alignment.gap_state == "already_observed" {
        summary.presentation_text_observed += 1;
    }
    if alignment.item_kind == "no_action" || alignment.gap_state == "internal_only" {
        summary.presentation_text_internal_only += 1;
        summary.presentation_text_no_action += 1;
    }
    if alignment.item_kind == "limitation" || alignment.gap_state == "static_limitation" {
        summary.presentation_text_static_limitations += 1;
    }
    if matches!(
        alignment.actionability,
        "add_output_observer" | "add_output_test" | "snapshot_or_help_output_test"
    ) {
        summary.presentation_text_actionable_total += 1;
        summary.presentation_text_actionable_snapshot += 1;
    }

    let visibility = presentation_text
        .and_then(|value| audit_string(value, &["visibility"]))
        .unwrap_or_default();
    let observer = presentation_text
        .and_then(|value| {
            audit_string(value, &["observer"]).or_else(|| audit_string(value, &["observer_kind"]))
        })
        .unwrap_or_default();

    match visibility.as_str() {
        "user_visible" => summary.presentation_text_user_visible += 1,
        "internal_only"
            if alignment.item_kind != "no_action" && alignment.gap_state != "internal_only" =>
        {
            summary.presentation_text_internal_only += 1;
        }
        "unknown" => {
            summary.presentation_text_visibility_unknown += 1;
            summary.visibility_unknown_total += 1;
        }
        _ => {}
    }
    if observer == "unknown" {
        summary.presentation_text_observer_unknown += 1;
    }
}

fn audit_evidence_record_field_health(
    record: &Value,
    health: &mut BTreeMap<String, Lane1EvidenceAuditFieldHealth>,
) {
    for (field, path) in [
        ("schema_version", &["schema_version"][..]),
        ("seam_id", &["seam_id"]),
        ("canonical_gap_id", &["canonical_gap_id"]),
        ("canonical_gap_group_size", &["canonical_gap_group_size"]),
        ("canonical_gap_reason", &["canonical_gap_reason"]),
        ("raw_findings", &["raw_findings"]),
        (
            "canonical_item.canonical_gap_id",
            &["canonical_item", "canonical_gap_id"],
        ),
        (
            "canonical_item.raw_group_size",
            &["canonical_item", "raw_group_size"],
        ),
        (
            "canonical_item.canonical_item_kind",
            &["canonical_item", "canonical_item_kind"],
        ),
        (
            "canonical_item.evidence_class",
            &["canonical_item", "evidence_class"],
        ),
        ("canonical_item.gap_state", &["canonical_item", "gap_state"]),
        (
            "canonical_item.actionability",
            &["canonical_item", "actionability"],
        ),
        ("canonical_item.why", &["canonical_item", "why"]),
        (
            "canonical_item.recommended_repair",
            &["canonical_item", "recommended_repair"],
        ),
        (
            "canonical_item.verify_command",
            &["canonical_item", "verify_command"],
        ),
        (
            "canonical_item.confidence.basis",
            &["canonical_item", "confidence", "basis"],
        ),
        ("owner", &["owner"]),
        ("location.file", &["location", "file"]),
        ("location.line", &["location", "line"]),
        ("seam_kind", &["seam_kind"]),
        ("grip_class", &["grip_class"]),
        ("headline_eligible", &["headline_eligible"]),
        ("evidence_path.reach", &["evidence_path", "reach"]),
        ("evidence_path.activate", &["evidence_path", "activate"]),
        ("evidence_path.propagate", &["evidence_path", "propagate"]),
        ("evidence_path.observe", &["evidence_path", "observe"]),
        (
            "evidence_path.discriminate",
            &["evidence_path", "discriminate"],
        ),
        ("observed_values", &["observed_values"]),
        ("missing_discriminators", &["missing_discriminators"]),
        ("related_tests_total", &["related_tests_total"]),
        ("related_tests", &["related_tests"]),
        ("recommendation.action", &["recommendation", "action"]),
        (
            "recommendation.verify_command",
            &["recommendation", "verify_command"],
        ),
        ("actionability.class", &["actionability", "class"]),
        ("calibration.availability", &["calibration", "availability"]),
        ("calibration.confidence", &["calibration", "confidence"]),
        ("calibration.agreement", &["calibration", "agreement"]),
        ("static_limitations", &["static_limitations"]),
        ("presentation_text", &["presentation_text"]),
    ] {
        let entry =
            health
                .entry(field.to_string())
                .or_insert_with(|| Lane1EvidenceAuditFieldHealth {
                    field: field.to_string(),
                    ..Lane1EvidenceAuditFieldHealth::default()
                });
        match audit_get(record, path) {
            None => entry.missing += 1,
            Some(value) if value.is_null() => entry.null += 1,
            Some(value) if audit_value_is_empty(value) => {
                entry.present += 1;
                entry.empty += 1;
            }
            Some(_) => entry.present += 1,
        }
    }
}

fn audit_value_is_empty(value: &Value) -> bool {
    value.as_str().is_some_and(str::is_empty)
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_object().is_some_and(serde_json::Map::is_empty)
}

fn audit_evidence_path_complete(record: &Value) -> bool {
    ["reach", "activate", "propagate", "observe", "discriminate"]
        .iter()
        .all(|stage| {
            let path = ["evidence_path", *stage];
            let Some(stage_value) = audit_get(record, &path) else {
                return false;
            };
            audit_string(stage_value, &["state"]).is_some()
                && audit_string(stage_value, &["confidence"]).is_some()
                && audit_string(stage_value, &["summary"]).is_some()
        })
}

fn audit_unknown_stage_count(record: &Value) -> usize {
    ["reach", "activate", "propagate", "observe", "discriminate"]
        .iter()
        .filter(|stage| {
            let path = ["evidence_path", *stage, "state"];
            audit_string(record, &path)
                .is_some_and(|state| matches!(state.as_str(), "unknown" | "opaque" | "no"))
        })
        .count()
}

fn audit_missing_discriminator_signature(missing: &[Value]) -> Option<String> {
    if missing.is_empty() {
        return None;
    }
    let mut values = missing
        .iter()
        .filter_map(|value| audit_string(value, &["value"]))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    if values.is_empty() {
        Some("missing_discriminator".to_string())
    } else {
        Some(values.join(" + "))
    }
}

fn audit_oracle_semantics_key(related: &Value) -> String {
    let observes = audit_string(related, &["oracle_semantics", "observes"])
        .unwrap_or_else(|| "missing_observes".to_string());
    let missing = audit_string(related, &["oracle_semantics", "missing"])
        .unwrap_or_else(|| "missing_gap".to_string());
    let upgrade = audit_get(related, &["oracle_semantics", "upgrade_suggestion"])
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("no_upgrade");
    format!("observes={observes}; missing={missing}; upgrade={upgrade}")
}

pub(crate) fn static_limitation_category(stage: &str, state: &str, reason: &str) -> &'static str {
    let reason = reason.to_ascii_lowercase();
    if reason.contains("cross-file")
        || reason.contains("cross file")
        || reason.contains("unresolved constant")
        || reason.contains("constant boundary")
    {
        "cross_file_constant_unresolved"
    } else if reason.contains("macro") || reason.contains("generated") {
        "macro_generated_value"
    } else if reason.contains("opaque helper") || reason.contains("opaque fixture") {
        "opaque_helper_call"
    } else if reason.contains("dynamic dispatch") || reason.contains("opaque dispatch") {
        "dynamic_dispatch"
    } else if reason.contains("mock") {
        "unsupported_mock_shape"
    } else if reason.contains("snapshot") {
        "snapshot_field_unknown"
    } else if reason.contains("side effect")
        || reason.contains("side-effect")
        || reason.contains("effect sink")
    {
        "side_effect_sink_unknown"
    } else if reason.contains("no direct owner call observed for value-insensitive seam") {
        "activation_owner_call_absent"
    } else if reason.contains("owner call") {
        "activation_owner_call_unresolved"
    } else if reason.contains("no concrete activation values observed")
        || reason.contains("no literal activation values")
    {
        "activation_value_unresolved"
    } else if stage == "classification" || state == "opaque" {
        "opaque_static_evidence"
    } else {
        match stage {
            "reach" => "reachability_static_unknown",
            "activate" => "activation_static_unknown",
            "propagate" => "propagation_static_unknown",
            "observe" => "observation_static_unknown",
            "discriminate" => "discrimination_static_unknown",
            _ => "static_limitation_unclassified",
        }
    }
}

pub(crate) fn static_limitation_repair_route(category: &str) -> &'static str {
    match category {
        "activation_owner_call_absent" => "analysis/owner-call-absence-triage",
        "activation_owner_call_absent_call_presence_target_affinity" => {
            "analysis/call-presence-target-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_assertion_target_affinity" => {
            "analysis/assertion-target-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_affinity_only" => {
            "analysis/related-test-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_same_file_only" => "analysis/same-file-owner-call-tracing",
        "activation_owner_call_unresolved" => "analysis/related-test-ranking-audit-fixes",
        "activation_value_unresolved" => "analysis/value-resolution-audit-fixes",
        "cross_file_constant_unresolved" => "analysis/cross-file-constant-resolution",
        CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY => CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE,
        CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY => {
            CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE
        }
        "macro_generated_value" => "analysis/macro-generated-value-fixtures",
        "opaque_helper_call" => "analysis/oracle-semantics-audit-fixes",
        "dynamic_dispatch" => "calibration/runtime-fixtures-v3",
        "unsupported_mock_shape" => "analysis/oracle-semantics-audit-fixes",
        "snapshot_field_unknown" => "analysis/oracle-semantics-audit-fixes",
        "side_effect_sink_unknown" => "analysis/oracle-semantics-audit-fixes",
        "opaque_static_evidence" => "analysis/static-limitation-taxonomy",
        "reachability_static_unknown" => "analysis/related-test-ranking-audit-fixes",
        "activation_static_unknown" => "analysis/static-limitation-taxonomy",
        "propagation_static_unknown" => "analysis/static-limitation-taxonomy",
        "observation_static_unknown" => "analysis/oracle-semantics-audit-fixes",
        "discrimination_static_unknown" => "analysis/oracle-semantics-audit-fixes",
        "static_limitation_unclassified" => "analysis/static-limitation-taxonomy",
        "lane1_repo_exposure_sampled"
        | "lane1_repo_exposure_timeout"
        | "lane1_repo_exposure_incomplete"
        | "lane1_repo_exposure_runner_error"
        | "lane1_repo_exposure_large_cache_preflight_skip"
        | "lane1_repo_exposure_cache_store_skipped_large_entry" => {
            "report/lane1-audit-bounded-diagnostics"
        }
        _ => "analysis/static-limitation-taxonomy",
    }
}

fn audit_upsert_group(
    groups: &mut BTreeMap<String, Lane1EvidenceAuditGroup>,
    group: Lane1EvidenceAuditGroup,
) {
    let key = group.key.clone();
    let reported_group_size = group.reported_group_size;
    let entry = groups.entry(key).or_insert(group);
    entry.count += 1;
    entry.reported_group_size = match (entry.reported_group_size, reported_group_size) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (None, Some(right)) => Some(right),
        (current, None) => current,
    };
}

fn audit_sorted_groups(mut groups: Vec<Lane1EvidenceAuditGroup>) -> Vec<Lane1EvidenceAuditGroup> {
    groups.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| {
                right
                    .reported_group_size
                    .unwrap_or(0)
                    .cmp(&left.reported_group_size.unwrap_or(0))
            })
            .then_with(|| left.key.cmp(&right.key))
    });
    groups
}

fn audit_file_debt<'a>(
    files: &'a mut BTreeMap<String, Lane1EvidenceAuditFileDebt>,
    file: &str,
) -> &'a mut Lane1EvidenceAuditFileDebt {
    files
        .entry(file.to_string())
        .or_insert_with(|| Lane1EvidenceAuditFileDebt {
            file: file.to_string(),
            ..Lane1EvidenceAuditFileDebt::default()
        })
}

pub(crate) fn audit_push_count(out: &mut String, name: &str, count: usize) {
    out.push_str(&format!("| {name} | {count} |\n"));
}

fn audit_push_counts_table_limited(
    out: &mut String,
    heading: &str,
    counts: &BTreeMap<String, usize>,
    limit: usize,
) {
    if counts.is_empty() {
        out.push_str(&format!(
            "No {} counts were reported.\n\n",
            heading.to_lowercase()
        ));
        return;
    }
    out.push_str(&format!("| {heading} | Count |\n"));
    out.push_str("| --- | ---: |\n");
    let mut rows = counts.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    for (key, count) in rows.iter().take(limit) {
        out.push_str(&format!("| {} | {} |\n", audit_markdown_cell(key), count));
    }
    out.push('\n');
}

fn audit_push_top_count_table(
    out: &mut String,
    heading: &str,
    rows: &[Lane1EvidenceAuditTopCount],
) {
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
        out.push_str(&format!(
            "| {} | {} |\n",
            audit_markdown_cell(&row.label),
            row.count
        ));
    }
    out.push('\n');
}

fn audit_push_alignment_class_coverage_table(
    out: &mut String,
    rows: &[Lane1EvidenceAuditAlignmentClassCoverage],
) {
    if rows.is_empty() {
        out.push_str("No finding alignment coverage rows were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | Raw | Canonical | Aligned raw | Unaligned raw | Actionable | Observed | No-action | Limitations | Unknown |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows.iter().take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.evidence_class),
            row.raw_findings,
            row.canonical_items,
            row.aligned_raw_findings,
            row.unaligned_raw_findings,
            row.actionable_items,
            row.already_observed_items,
            row.internal_no_action_items,
            row.static_limitation_items,
            row.unknown_items,
        ));
    }
    out.push('\n');
}

fn audit_push_evidence_class_work_queue_table(
    out: &mut String,
    rows: &[Lane1EvidenceClassWorkItem],
) {
    out.push_str("### Evidence Class Work Queue\n\n");
    if rows.is_empty() {
        out.push_str("No evidence-class work queue rows were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | Work score | Dominant signal | Static category | Static route | Actionable | Limitations | Unknown | Unaligned | Duplicate raw | Next repair |\n");
    out.push_str("| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for row in rows {
        let static_category = row
            .dominant_static_limitation_category
            .as_deref()
            .unwrap_or("n/a");
        let static_route = row
            .dominant_static_limitation_repair_route
            .as_deref()
            .unwrap_or("n/a");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.evidence_class),
            row.work_score,
            audit_markdown_cell(&row.dominant_signal),
            audit_markdown_cell(static_category),
            audit_markdown_cell(static_route),
            row.actionable_items,
            row.static_limitation_items,
            row.unknown_items,
            row.unaligned_raw_findings,
            row.duplicate_raw_signals,
            audit_markdown_cell(&row.next_repair),
        ));
    }
    out.push('\n');
}

fn audit_push_runtime_confidence_by_class_table(
    out: &mut String,
    rows: &[Lane1EvidenceAuditRuntimeConfidenceClassCoverage],
) {
    out.push_str("### Runtime Confidence By Evidence Class\n\n");
    if rows.is_empty() {
        out.push_str("No runtime confidence coverage rows were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | Canonical | Calibrated supported | Fixture-backed | Static-only | Unknown confidence | Uncalibrated | Actionable | Limitations |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows.iter().take(LANE1_EVIDENCE_AUDIT_TOP_LIMIT) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.evidence_class),
            row.canonical_items,
            row.calibrated_supported,
            row.fixture_backed,
            row.static_only,
            row.unknown_confidence,
            row.uncalibrated,
            row.actionable_items,
            row.static_limitation_items,
        ));
    }
    out.push('\n');
}

fn audit_push_unaligned_examples_table(
    out: &mut String,
    rows: &[Lane1EvidenceAuditUnalignedExample],
) {
    if rows.is_empty() {
        out.push_str("No unaligned raw finding examples were reported.\n\n");
        return;
    }
    out.push_str("| Evidence class | File | Line | Kind | Reason | Expression |\n");
    out.push_str("| --- | --- | ---: | --- | --- | --- |\n");
    for row in rows {
        let line = row
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.evidence_class),
            audit_markdown_cell(&row.file),
            line,
            audit_markdown_cell(&row.kind),
            audit_markdown_cell(&row.reason),
            audit_markdown_cell(&row.expression),
        ));
    }
    out.push('\n');
}

fn audit_push_same_line_duplicate_table(
    out: &mut String,
    rows: &[Lane1EvidenceAuditSameLineDuplicateGroup],
) {
    if rows.is_empty() {
        out.push_str("No same-line duplicate raw finding groups were reported.\n\n");
        return;
    }
    out.push_str(
        "| File | Line | Raw findings | Evidence classes | Kinds | Example expression |\n",
    );
    out.push_str("| --- | ---: | ---: | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.file),
            row.line,
            row.raw_findings,
            audit_markdown_cell(&row.evidence_classes.join(", ")),
            audit_markdown_cell(&row.kinds.join(", ")),
            audit_markdown_cell(row.example_expression.as_deref().unwrap_or("n/a")),
        ));
    }
    out.push('\n');
}

pub(crate) fn audit_push_value_counts_table_limited(
    out: &mut String,
    heading: &str,
    value: &Value,
    path: &[&str],
    limit: usize,
) {
    let Some(object) = audit_get(value, path).and_then(Value::as_object) else {
        out.push_str(&format!(
            "No {} counts were reported.\n\n",
            heading.to_lowercase()
        ));
        return;
    };
    let counts = object
        .iter()
        .filter_map(|(key, value)| value.as_u64().map(|count| (key.clone(), count as usize)))
        .collect::<BTreeMap<_, _>>();
    audit_push_counts_table_limited(out, heading, &counts, limit);
}

fn audit_push_group_table(out: &mut String, groups: &[Lane1EvidenceAuditGroup]) {
    if groups.is_empty() {
        out.push_str("No groups were reported.\n\n");
        return;
    }
    out.push_str("| Group | Count | Reported size | Owner | Seam kind | Flow sink | Missing discriminator | Assertion shape | Example seam | File |\n");
    out.push_str("| --- | ---: | ---: | --- | --- | --- | --- | --- | --- | --- |\n");
    for group in groups {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(
                group
                    .canonical_gap_id
                    .as_deref()
                    .unwrap_or(group.key.as_str())
            ),
            group.count,
            group
                .reported_group_size
                .map_or_else(|| "n/a".to_string(), |size| size.to_string()),
            audit_markdown_cell(group.owner.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.seam_kind.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.flow_sink.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.missing_discriminator.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.assertion_shape.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.example_seam_id.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(group.example_file.as_deref().unwrap_or("n/a")),
        ));
    }
    out.push('\n');
}

pub(crate) fn audit_markdown_cell(value: &str) -> String {
    value.replace('\n', " ").replace('|', "\\|")
}
