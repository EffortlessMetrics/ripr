//! Dogfood cluster: scenario-driven dogfood runs and report rendering (first
//! action, first PR, review front panel, packet index, generated-CI cockpit,
//! language previews, editor gap cockpit, finding alignment, real repair
//! attempts, python evals, bun UB calibration, configured bridge inventory,
//! cross-language oracle graph, surface projection, and PR inline comments).
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported from `main.rs` so
//! existing call sites (`dispatch.rs`, the `reports/` shims, and `tests.rs`)
//! compile unchanged.

use super::*;

#[derive(Debug)]
pub(crate) struct DogfoodScenario {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) diff: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DogfoodRun {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) diff: PathBuf,
    pub(crate) actual_dir: PathBuf,
    pub(crate) duration_ms: u128,
    pub(crate) findings: usize,
    pub(crate) class_counts: BTreeMap<String, usize>,
    pub(crate) stop_reason_mentions: usize,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodGateScenario {
    pub(crate) name: &'static str,
    pub(crate) mode: &'static str,
    pub(crate) expected_dir: &'static str,
    pub(crate) pr_guidance: &'static str,
    pub(crate) labels_json: Option<&'static str>,
    pub(crate) baseline: Option<&'static str>,
    pub(crate) recommendation_calibration: Option<&'static str>,
    pub(crate) mutation_calibration: Option<&'static str>,
    pub(crate) expected_status: &'static str,
    pub(crate) expected_blocking: usize,
    pub(crate) expected_acknowledged: usize,
    pub(crate) expected_advisory: usize,
    pub(crate) expected_exit_success: bool,
}

#[derive(Debug)]
pub(crate) struct DogfoodGateRun {
    pub(crate) name: String,
    pub(crate) mode: String,
    pub(crate) actual_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) duration_ms: u128,
    pub(crate) status: String,
    pub(crate) blocking: usize,
    pub(crate) acknowledged: usize,
    pub(crate) advisory: usize,
    pub(crate) expected_status: String,
    pub(crate) expected_blocking: usize,
    pub(crate) expected_acknowledged: usize,
    pub(crate) expected_advisory: usize,
    pub(crate) exit_success: bool,
    pub(crate) expected_exit_success: bool,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodFirstActionScenario {
    pub(crate) name: &'static str,
    pub(crate) expected_dir: &'static str,
    pub(crate) expected_status: &'static str,
    pub(crate) expected_action_kind: &'static str,
    pub(crate) expected_audience: &'static str,
    pub(crate) expected_selected: bool,
    pub(crate) expected_static_movement: &'static str,
}

#[derive(Debug)]
pub(crate) struct DogfoodFirstActionRun {
    pub(crate) name: String,
    pub(crate) expected_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) status: String,
    pub(crate) action_kind: String,
    pub(crate) audience: String,
    pub(crate) selected: bool,
    pub(crate) static_movement: String,
    pub(crate) expected_status: String,
    pub(crate) expected_action_kind: String,
    pub(crate) expected_audience: String,
    pub(crate) expected_selected: bool,
    pub(crate) expected_static_movement: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodFirstPrScenario {
    pub(crate) name: String,
    pub(crate) expected_dir: PathBuf,
    pub(crate) expected_status: String,
    pub(crate) expected_state: String,
    pub(crate) description: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodFirstPrRun {
    pub(crate) name: String,
    pub(crate) expected_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) status: String,
    pub(crate) state: String,
    pub(crate) top_gap_kind: String,
    pub(crate) verify_command: Option<String>,
    pub(crate) next_command: Option<String>,
    pub(crate) expected_status: String,
    pub(crate) expected_state: String,
    pub(crate) description: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DogfoodFirstPrMetrics {
    pub(crate) packets_total: usize,
    pub(crate) top_gap_selected_total: usize,
    pub(crate) no_action_total: usize,
    pub(crate) blocked_total: usize,
    pub(crate) missing_artifact_total: usize,
    pub(crate) stale_artifact_total: usize,
    pub(crate) wrong_root_total: usize,
    pub(crate) malformed_artifact_total: usize,
    pub(crate) timeout_total: usize,
}

#[derive(Debug)]
pub(crate) struct DogfoodFrontPanelScenario {
    pub(crate) name: String,
    pub(crate) report_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) expected_status: String,
    pub(crate) expected_top_issue_state: String,
    pub(crate) expected_policy_state: String,
    pub(crate) expected_placement: String,
    pub(crate) expected_movement_state: String,
    pub(crate) expected_coverage_grip_state: String,
    pub(crate) expected_new_policy_eligible: usize,
    pub(crate) expected_baseline_resolved: usize,
    pub(crate) expected_blocking_candidates: usize,
    pub(crate) expected_warnings: usize,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodFrontPanelRun {
    pub(crate) name: String,
    pub(crate) report_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) status: String,
    pub(crate) top_issue_state: String,
    pub(crate) policy_state: String,
    pub(crate) placement: String,
    pub(crate) movement_state: String,
    pub(crate) coverage_grip_state: String,
    pub(crate) new_policy_eligible: usize,
    pub(crate) baseline_resolved: usize,
    pub(crate) blocking_candidates: usize,
    pub(crate) warnings: usize,
    pub(crate) expected_status: String,
    pub(crate) expected_top_issue_state: String,
    pub(crate) expected_policy_state: String,
    pub(crate) expected_placement: String,
    pub(crate) expected_movement_state: String,
    pub(crate) expected_coverage_grip_state: String,
    pub(crate) expected_new_policy_eligible: usize,
    pub(crate) expected_baseline_resolved: usize,
    pub(crate) expected_blocking_candidates: usize,
    pub(crate) expected_warnings: usize,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodReportPacketIndexScenario {
    pub(crate) name: String,
    pub(crate) scenario: String,
    pub(crate) expected_report: PathBuf,
    pub(crate) expected_markdown: PathBuf,
    pub(crate) expected_status: String,
    pub(crate) expected_missing_expected: usize,
    pub(crate) expected_failures: usize,
    pub(crate) expected_warnings: usize,
    pub(crate) expected_start_here_available: bool,
    pub(crate) expected_gate_authority_present: bool,
    pub(crate) expected_required_groups: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodReportPacketIndexRun {
    pub(crate) name: String,
    pub(crate) actual_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) status: String,
    pub(crate) missing_expected: usize,
    pub(crate) failures: usize,
    pub(crate) warnings: usize,
    pub(crate) start_here_available: bool,
    pub(crate) gate_authority_present: bool,
    pub(crate) groups: Vec<String>,
    pub(crate) expected_status: String,
    pub(crate) expected_missing_expected: usize,
    pub(crate) expected_failures: usize,
    pub(crate) expected_warnings: usize,
    pub(crate) expected_start_here_available: bool,
    pub(crate) expected_gate_authority_present: bool,
    pub(crate) expected_required_groups: Vec<String>,
    pub(crate) reason: String,
    pub(crate) expected_report: PathBuf,
    pub(crate) expected_markdown: PathBuf,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodGeneratedCiCockpitRun {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) duration_ms: u128,
    pub(crate) start_here: bool,
    pub(crate) repair_commands: usize,
    pub(crate) expected_repair_commands: usize,
    pub(crate) gate_authority_boundary: bool,
    pub(crate) default_advisory: bool,
    pub(crate) artifact_upload: bool,
    pub(crate) language_grouping_status: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodLanguagePreviewScenario {
    pub(crate) name: String,
    pub(crate) language: String,
    pub(crate) root: PathBuf,
    pub(crate) diff: PathBuf,
    pub(crate) expected_findings: usize,
    pub(crate) expected_preview_findings: usize,
    pub(crate) expected_missing_preview_status: usize,
    pub(crate) expected_related_tests: usize,
    pub(crate) expected_classifications: Vec<String>,
    pub(crate) expected_static_limit_kinds: Vec<String>,
    pub(crate) preview_enabled: bool,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodLanguagePreviewRun {
    pub(crate) name: String,
    pub(crate) language: String,
    pub(crate) root: PathBuf,
    pub(crate) diff: PathBuf,
    pub(crate) actual_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) human_path: PathBuf,
    pub(crate) duration_ms: u128,
    pub(crate) findings: usize,
    pub(crate) language_findings: usize,
    pub(crate) preview_findings: usize,
    pub(crate) missing_preview_status: usize,
    pub(crate) related_tests: usize,
    pub(crate) classifications: Vec<String>,
    pub(crate) static_limit_kinds: Vec<String>,
    pub(crate) expected_findings: usize,
    pub(crate) expected_preview_findings: usize,
    pub(crate) expected_missing_preview_status: usize,
    pub(crate) expected_related_tests: usize,
    pub(crate) expected_classifications: Vec<String>,
    pub(crate) expected_static_limit_kinds: Vec<String>,
    pub(crate) preview_enabled: bool,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodEditorGapCockpitScenario {
    pub(crate) name: String,
    pub(crate) expected_state: String,
    pub(crate) expected_language: Option<String>,
    pub(crate) expected_language_status: Option<String>,
    pub(crate) expected_diagnostics: usize,
    pub(crate) expected_fail_closed: bool,
    pub(crate) expected_actions: Vec<String>,
    pub(crate) expected_static_limit_kind: Option<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodEditorGapCockpitRun {
    pub(crate) name: String,
    pub(crate) expected_dir: PathBuf,
    pub(crate) projection_path: PathBuf,
    pub(crate) diagnostics_path: PathBuf,
    pub(crate) hover_path: PathBuf,
    pub(crate) code_actions_path: PathBuf,
    pub(crate) status_path: PathBuf,
    pub(crate) state: String,
    pub(crate) language: Option<String>,
    pub(crate) language_status: Option<String>,
    pub(crate) diagnostics_projected: usize,
    pub(crate) actual_diagnostics: usize,
    pub(crate) fail_closed: bool,
    pub(crate) actions_projected: Vec<String>,
    pub(crate) actual_actions: usize,
    pub(crate) static_limit_kind: Option<String>,
    pub(crate) hover_static_before_action: bool,
    pub(crate) expected_state: String,
    pub(crate) expected_language: Option<String>,
    pub(crate) expected_language_status: Option<String>,
    pub(crate) expected_diagnostics: usize,
    pub(crate) expected_fail_closed: bool,
    pub(crate) expected_actions: Vec<String>,
    pub(crate) expected_static_limit_kind: Option<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodEditorFirstPrBridgeScenario {
    pub(crate) name: String,
    pub(crate) expected_packet_state: String,
    pub(crate) expected_safe_actions: Vec<String>,
    pub(crate) expected_suppressed_actions: Vec<String>,
    pub(crate) expected_diagnostics: usize,
    pub(crate) expected_fail_closed: bool,
    pub(crate) expected_receipt_movement: Option<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodEditorFirstPrBridgeRun {
    pub(crate) name: String,
    pub(crate) expected_dir: PathBuf,
    pub(crate) packet_path: PathBuf,
    pub(crate) diagnostics_path: PathBuf,
    pub(crate) code_actions_path: PathBuf,
    pub(crate) status_path: PathBuf,
    pub(crate) diagnosis_path: PathBuf,
    pub(crate) packet_state: String,
    pub(crate) safe_actions: Vec<String>,
    pub(crate) suppressed_actions: Vec<String>,
    pub(crate) receipt_movement: Option<String>,
    pub(crate) diagnostics: usize,
    pub(crate) action_commands: Vec<String>,
    pub(crate) first_pr_actions: Vec<String>,
    pub(crate) fail_closed: bool,
    pub(crate) expected_packet_state: String,
    pub(crate) expected_safe_actions: Vec<String>,
    pub(crate) expected_suppressed_actions: Vec<String>,
    pub(crate) expected_diagnostics: usize,
    pub(crate) expected_fail_closed: bool,
    pub(crate) expected_receipt_movement: Option<String>,
    pub(crate) runtime_adequacy_claim: bool,
    pub(crate) mutation_proof_claim: bool,
    pub(crate) policy_gate_claim: bool,
    pub(crate) pr_ready_claim: bool,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodFindingAlignmentScenario {
    pub(crate) name: String,
    pub(crate) source_pr: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) raw_findings_total: usize,
    pub(crate) canonical_items_total: usize,
    pub(crate) raw_finding_summary: String,
    pub(crate) gap_state: String,
    pub(crate) actionability: String,
    pub(crate) user_outcome: String,
    pub(crate) repair_kind: String,
    pub(crate) target_test_type: String,
    pub(crate) verify_command: String,
    pub(crate) static_limitation_category: Option<String>,
    pub(crate) static_limitation_repair_route: Option<String>,
    pub(crate) raw_findings_supporting_only: bool,
    pub(crate) recommended_repair: String,
    pub(crate) before_after_context: String,
    pub(crate) must_not_claim: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodFindingAlignmentRun {
    pub(crate) name: String,
    pub(crate) source_pr: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) raw_findings_total: usize,
    pub(crate) canonical_items_total: usize,
    pub(crate) raw_finding_summary: String,
    pub(crate) gap_state: String,
    pub(crate) actionability: String,
    pub(crate) user_outcome: String,
    pub(crate) repair_kind: String,
    pub(crate) target_test_type: String,
    pub(crate) verify_command: String,
    pub(crate) static_limitation_category: Option<String>,
    pub(crate) static_limitation_repair_route: Option<String>,
    pub(crate) raw_findings_supporting_only: bool,
    pub(crate) recommended_repair: String,
    pub(crate) before_after_context: String,
    pub(crate) must_not_claim: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodSurfaceProjectionAlignmentScenario {
    pub(crate) name: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) evidence_class: String,
    pub(crate) repair_kind: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_state: String,
    pub(crate) outcome: String,
    pub(crate) expected_top_next_action_kind: String,
    pub(crate) advisory_consumers: Vec<String>,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) swarm_plan: Value,
    pub(crate) actionable_gap_outcomes: Value,
    pub(crate) attempt_ledger: Value,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodSurfaceProjectionAlignmentRun {
    pub(crate) name: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) repair_kind: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_state: String,
    pub(crate) outcome: String,
    pub(crate) top_next_action_kind: String,
    pub(crate) top_next_action_command: Option<String>,
    pub(crate) readiness_status: String,
    pub(crate) attempted_packets: usize,
    pub(crate) improved_packets: usize,
    pub(crate) advisory_consumers: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodRealRepairAttemptScenario {
    pub(crate) name: String,
    pub(crate) source_ref: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) language: Option<String>,
    pub(crate) evidence_class: Option<String>,
    pub(crate) source_file: Option<String>,
    pub(crate) repair_kind: String,
    pub(crate) target_test_or_observer_shape: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_path: Option<String>,
    pub(crate) receipt_state: String,
    pub(crate) actor_kind: String,
    pub(crate) before_gap_state: String,
    pub(crate) after_gap_state: String,
    pub(crate) outcome: String,
    pub(crate) attempted_repair: String,
    pub(crate) evidence_movement: String,
    pub(crate) operator_note: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) missing_receipt_reason: Option<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodRealRepairAttemptRun {
    pub(crate) name: String,
    pub(crate) source_ref: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) language: Option<String>,
    pub(crate) evidence_class: Option<String>,
    pub(crate) source_file: Option<String>,
    pub(crate) repair_kind: String,
    pub(crate) target_test_or_observer_shape: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_path: Option<String>,
    pub(crate) receipt_state: String,
    pub(crate) actor_kind: String,
    pub(crate) before_gap_state: String,
    pub(crate) after_gap_state: String,
    pub(crate) outcome: String,
    pub(crate) attempted_repair: String,
    pub(crate) evidence_movement: String,
    pub(crate) operator_note: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) missing_receipt_reason: Option<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DogfoodPythonRankedFinding {
    pub(crate) rank: usize,
    pub(crate) canonical_gap_id: String,
    pub(crate) repair_card_present: bool,
    pub(crate) usability: String,
    pub(crate) missing_discriminator: String,
    pub(crate) suggested_test_file: String,
    pub(crate) verify_command: String,
    pub(crate) false_positive_notes: String,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonRealRepoEvalScenario {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) top_finding_summary: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) repair_card_present: bool,
    pub(crate) repair_action: String,
    pub(crate) agent_packet_present: bool,
    pub(crate) agent_packet_task: String,
    pub(crate) agent_packet_command: String,
    pub(crate) agent_packet_allowed_files: Vec<String>,
    pub(crate) agent_packet_forbidden_files: Vec<String>,
    pub(crate) agent_packet_stop_if: Vec<String>,
    pub(crate) changed_owner: String,
    pub(crate) missing_discriminator: String,
    pub(crate) suggested_test_file: String,
    pub(crate) suggested_test_name: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) verify_summary: String,
    pub(crate) after_command: String,
    pub(crate) after_runtime_ms: usize,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) closed_gaps: usize,
    pub(crate) usability: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) unsupported_limitations: Vec<String>,
    pub(crate) ranked_top_3_findings: Vec<DogfoodPythonRankedFinding>,
    pub(crate) ranked_top_3_limit_reason: Option<String>,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonRealRepoEvalRun {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) top_finding_summary: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) repair_card_present: bool,
    pub(crate) repair_action: String,
    pub(crate) agent_packet_present: bool,
    pub(crate) agent_packet_task: String,
    pub(crate) agent_packet_command: String,
    pub(crate) agent_packet_allowed_files: Vec<String>,
    pub(crate) agent_packet_forbidden_files: Vec<String>,
    pub(crate) agent_packet_stop_if: Vec<String>,
    pub(crate) changed_owner: String,
    pub(crate) missing_discriminator: String,
    pub(crate) suggested_test_file: String,
    pub(crate) suggested_test_name: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) verify_summary: String,
    pub(crate) after_command: String,
    pub(crate) after_runtime_ms: usize,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) closed_gaps: usize,
    pub(crate) usability: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) unsupported_limitations: Vec<String>,
    pub(crate) ranked_top_3_findings: Vec<DogfoodPythonRankedFinding>,
    pub(crate) ranked_top_3_limit_reason: Option<String>,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonStaticLimitEvalScenario {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) finding_id: String,
    pub(crate) changed_owner: String,
    pub(crate) static_limit_kind: String,
    pub(crate) classification: String,
    pub(crate) stop_reasons: Vec<String>,
    pub(crate) related_test_file: String,
    pub(crate) related_test_name: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_card_present: bool,
    pub(crate) agent_packet_present: bool,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonStaticLimitEvalRun {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) finding_id: String,
    pub(crate) changed_owner: String,
    pub(crate) static_limit_kind: String,
    pub(crate) classification: String,
    pub(crate) stop_reasons: Vec<String>,
    pub(crate) related_test_file: String,
    pub(crate) related_test_name: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_card_present: bool,
    pub(crate) agent_packet_present: bool,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonNoActionEvalScenario {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) finding_id: String,
    pub(crate) changed_owner: String,
    pub(crate) no_action_kind: String,
    pub(crate) classification: String,
    pub(crate) stop_reasons: Vec<String>,
    pub(crate) related_test_file: String,
    pub(crate) related_test_name: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_card_present: bool,
    pub(crate) agent_packet_present: bool,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodPythonNoActionEvalRun {
    pub(crate) name: String,
    pub(crate) repo_shape: String,
    pub(crate) source_kind: String,
    pub(crate) source_ref: String,
    pub(crate) command: String,
    pub(crate) runtime_ms: usize,
    pub(crate) finding_id: String,
    pub(crate) changed_owner: String,
    pub(crate) no_action_kind: String,
    pub(crate) classification: String,
    pub(crate) stop_reasons: Vec<String>,
    pub(crate) related_test_file: String,
    pub(crate) related_test_name: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_card_present: bool,
    pub(crate) agent_packet_present: bool,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_result: String,
    pub(crate) gap_movement: String,
    pub(crate) false_positive_notes: String,
    pub(crate) limitation_notes: String,
    pub(crate) claim_boundary: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct DogfoodPythonRepairRoutingQualitySummary {
    pub(crate) cases: usize,
    pub(crate) top_1_actionable_usable: usize,
    pub(crate) verify_command_valid: usize,
    pub(crate) agent_packet_bounded: usize,
    pub(crate) concrete_discriminator: usize,
    pub(crate) suggested_test_location: usize,
    pub(crate) false_actionable: usize,
    pub(crate) crashes: usize,
    pub(crate) receipt_closed: usize,
    pub(crate) top_3_ranked_findings_checked: usize,
    pub(crate) top_3_actionable_usable: usize,
    pub(crate) top_3_cases_with_ranked_capture: usize,
    pub(crate) full_top_3_capture_cases: usize,
    pub(crate) unsupported_limitation_distribution: Vec<(String, usize)>,
    pub(crate) gate_status: String,
    pub(crate) gate_reason: String,
}

#[derive(Debug, Default)]
pub(crate) struct DogfoodTypescriptFalseActionableAuditSummary {
    pub(crate) cases: usize,
    pub(crate) must_remain_non_actionable: usize,
    pub(crate) repair_packet_ready_true: usize,
    pub(crate) actionable_gap_state: usize,
    pub(crate) complete_packet_category: usize,
    pub(crate) preview_boundary_violations: usize,
    pub(crate) false_actionable: usize,
    pub(crate) gate_status: String,
    pub(crate) gate_reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodTypescriptPreviewRepairLoopScenario {
    pub(crate) name: String,
    pub(crate) source_fixture: String,
    pub(crate) source_finding_id: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) classification: String,
    pub(crate) changed_owner: String,
    pub(crate) probe_family: String,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) gap_state: String,
    pub(crate) actionability_category: String,
    pub(crate) static_limit_kind: Option<String>,
    pub(crate) repair_packet_ready: bool,
    pub(crate) must_have_verify_command: bool,
    pub(crate) must_have_receipt_command: bool,
    pub(crate) must_not_invent_verify_command: bool,
    pub(crate) must_not_emit_repair_packet: bool,
    pub(crate) authority_boundary: String,
    pub(crate) expected_test_or_observer_shape: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_state: String,
    pub(crate) outcome: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) operator_note: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodTypescriptPreviewRepairLoopRun {
    pub(crate) name: String,
    pub(crate) source_fixture: String,
    pub(crate) source_finding_id: String,
    pub(crate) language: String,
    pub(crate) classification: String,
    pub(crate) changed_owner: String,
    pub(crate) probe_family: String,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) gap_state: String,
    pub(crate) actionability_category: String,
    pub(crate) static_limit_kind: Option<String>,
    pub(crate) repair_packet_ready: bool,
    pub(crate) expected_test_or_observer_shape: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: String,
    pub(crate) receipt_command: String,
    pub(crate) receipt_state: String,
    pub(crate) outcome: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) operator_note: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DogfoodBunUbCrossLanguageScenario {
    pub(crate) name: String,
    pub(crate) source_case: String,
    pub(crate) route_quality_case: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) ts_test_file: String,
    pub(crate) expected_state: String,
    pub(crate) observed_state: String,
    pub(crate) missing_discriminators: Vec<String>,
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) suggested_test_file: String,
    pub(crate) manual_verdict: String,
    pub(crate) operator_action: String,
    pub(crate) review_before: String,
    pub(crate) review_after: String,
    pub(crate) bridge_verdict: String,
    pub(crate) placement_verdict: String,
    pub(crate) proof_mode: String,
    pub(crate) receipt_state: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) authority_boundary: String,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodBunUbCrossLanguageRun {
    pub(crate) name: String,
    pub(crate) source_case: String,
    pub(crate) route_quality_case: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) ts_test_file: String,
    pub(crate) expected_state: String,
    pub(crate) observed_state: String,
    pub(crate) missing_discriminators: Vec<String>,
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) suggested_test_file: String,
    pub(crate) manual_verdict: String,
    pub(crate) operator_action: String,
    pub(crate) review_before: String,
    pub(crate) review_after: String,
    pub(crate) bridge_verdict: String,
    pub(crate) placement_verdict: String,
    pub(crate) proof_mode: String,
    pub(crate) receipt_state: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) authority_boundary: String,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct TypeScriptPreviewFalseActionableAuditCase {
    pub(crate) name: String,
    pub(crate) source_fixture: String,
    pub(crate) source_finding_id: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) risk_class: String,
    pub(crate) evidence_kind: String,
    pub(crate) oracle_kind: Option<String>,
    pub(crate) gap_state: String,
    pub(crate) actionability_category: String,
    pub(crate) static_limit_kind: Option<String>,
    pub(crate) disposition: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) authority_boundary: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) future_support: String,
    pub(crate) must_remain_non_actionable: bool,
    pub(crate) required_evidence_fragment: String,
    pub(crate) raw_evidence_refs: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TypeScriptBunUbCalibrationCase {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) ts_test_file: String,
    pub(crate) ts_entrypoints: Vec<String>,
    pub(crate) shared_array_buffer: bool,
    pub(crate) resizable_array_buffer: bool,
    pub(crate) view_backed_blob_input: bool,
    pub(crate) stable_byte_copy_oracle: bool,
    pub(crate) max_byte_length_mention_only: bool,
    pub(crate) expected_verdict: String,
    pub(crate) expected_missing_discriminators: Vec<String>,
    pub(crate) bridge_confidence: String,
    pub(crate) expected_action: String,
    pub(crate) suggested_test_file: String,
    pub(crate) suggested_shape: Option<String>,
    pub(crate) repair_packet_ready: bool,
    pub(crate) authority_boundary: String,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BunUbCalibrationArgs {
    pub(crate) corpus: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BunUbPreviewSummaryArgs {
    pub(crate) calibration_corpus: PathBuf,
    pub(crate) graph_corpus: PathBuf,
    pub(crate) dogfood_corpus: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfiguredBridgeInventoryArgs {
    pub(crate) graph_corpus: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct CrossLanguageOracleGraphCase {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) profile: String,
    pub(crate) profile_status: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) rust_file: String,
    pub(crate) rust_line: Option<usize>,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) binding_edge_kind: String,
    pub(crate) binding_edge_confidence: String,
    pub(crate) external_callsite_file: String,
    pub(crate) external_callsite_line: Option<usize>,
    pub(crate) external_entrypoints: Vec<String>,
    pub(crate) shared_array_buffer: bool,
    pub(crate) resizable_array_buffer: bool,
    pub(crate) view_backed_blob_input: bool,
    pub(crate) stable_byte_copy_oracle: bool,
    pub(crate) max_byte_length_mention_only: bool,
    pub(crate) external_oracle_file: String,
    pub(crate) external_oracle_line: Option<usize>,
    pub(crate) external_oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) expected_state: String,
    pub(crate) gap_state: String,
    pub(crate) limitation_category: String,
    pub(crate) repair_route: String,
    pub(crate) authority_boundary: String,
    pub(crate) public_projection_eligible: bool,
    pub(crate) repair_packet_ready: bool,
    pub(crate) suggested_test_file: String,
    pub(crate) allowed_edit_surface: Vec<String>,
    pub(crate) verify_command: Option<String>,
    pub(crate) receipt_command: Option<String>,
    pub(crate) missing_discriminators: Vec<String>,
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) unlock_condition: String,
    pub(crate) proof_mode: String,
    pub(crate) raw_evidence_refs: Vec<CrossLanguageOracleGraphRawRef>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CrossLanguageOracleGraphRawRef {
    pub(crate) leg: String,
    pub(crate) file: String,
    pub(crate) line: Option<usize>,
    pub(crate) kind: String,
    pub(crate) source_id: String,
    pub(crate) sample: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodUserSurfaceProjectionScenario {
    pub(crate) name: String,
    pub(crate) surface: String,
    pub(crate) artifact: String,
    pub(crate) headline: String,
    pub(crate) run_status: String,
    pub(crate) projection_basis: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) repair_kind: String,
    pub(crate) top_next_action_kind: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command: String,
    pub(crate) source_alignment_case: String,
    pub(crate) limitation_category: String,
    pub(crate) runtime_repair_command: String,
    pub(crate) actionable_count: usize,
    pub(crate) raw_findings_total: usize,
    pub(crate) consumes_canonical_state: bool,
    pub(crate) reinterprets_raw_findings: bool,
    pub(crate) raw_findings_headline: bool,
    pub(crate) advisory: bool,
    pub(crate) blocking_default: bool,
    pub(crate) limited_state_visible: bool,
    pub(crate) stale_state_visible: bool,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodUserSurfaceProjectionRun {
    pub(crate) name: String,
    pub(crate) surface: String,
    pub(crate) artifact: String,
    pub(crate) headline: String,
    pub(crate) run_status: String,
    pub(crate) projection_basis: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: String,
    pub(crate) repair_kind: String,
    pub(crate) top_next_action_kind: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command: String,
    pub(crate) source_alignment_case: String,
    pub(crate) limitation_category: String,
    pub(crate) runtime_repair_command: String,
    pub(crate) actionable_count: usize,
    pub(crate) raw_findings_total: usize,
    pub(crate) consumes_canonical_state: bool,
    pub(crate) reinterprets_raw_findings: bool,
    pub(crate) raw_findings_headline: bool,
    pub(crate) advisory: bool,
    pub(crate) blocking_default: bool,
    pub(crate) limited_state_visible: bool,
    pub(crate) stale_state_visible: bool,
    pub(crate) reason: String,
    pub(crate) errors: Vec<String>,
}

pub(crate) struct DogfoodPreviewProjectionRuns<'a> {
    pub(crate) generated_ci_cockpit: &'a [DogfoodGeneratedCiCockpitRun],
    pub(crate) language_preview: &'a [DogfoodLanguagePreviewRun],
    pub(crate) editor_gap_cockpit: &'a [DogfoodEditorGapCockpitRun],
    pub(crate) editor_first_pr_bridge: &'a [DogfoodEditorFirstPrBridgeRun],
}

pub(crate) struct DogfoodReportInputs<'a> {
    pub(crate) runs: &'a [DogfoodRun],
    pub(crate) gate_runs: &'a [DogfoodGateRun],
    pub(crate) first_action_runs: &'a [DogfoodFirstActionRun],
    pub(crate) first_pr_runs: &'a [DogfoodFirstPrRun],
    pub(crate) front_panel_runs: &'a [DogfoodFrontPanelRun],
    pub(crate) report_packet_index_runs: &'a [DogfoodReportPacketIndexRun],
    pub(crate) preview_projection_runs: &'a DogfoodPreviewProjectionRuns<'a>,
    pub(crate) finding_alignment_runs: &'a [DogfoodFindingAlignmentRun],
    pub(crate) surface_projection_alignment_runs: &'a [DogfoodSurfaceProjectionAlignmentRun],
    pub(crate) real_repair_attempt_runs: &'a [DogfoodRealRepairAttemptRun],
    pub(crate) python_real_repo_eval_runs: &'a [DogfoodPythonRealRepoEvalRun],
    pub(crate) python_static_limit_eval_runs: &'a [DogfoodPythonStaticLimitEvalRun],
    pub(crate) python_no_action_eval_runs: &'a [DogfoodPythonNoActionEvalRun],
    pub(crate) typescript_preview_repair_loop_runs: &'a [DogfoodTypescriptPreviewRepairLoopRun],
    pub(crate) bun_ub_cross_language_runs: &'a [DogfoodBunUbCrossLanguageRun],
    pub(crate) user_surface_projection_runs: &'a [DogfoodUserSurfaceProjectionRun],
    pub(crate) pr_inline_comment_runs: &'a [DogfoodPrInlineCommentRun],
}

#[derive(Debug)]
pub(crate) struct DogfoodPrInlineCommentScenario {
    pub(crate) name: String,
    pub(crate) scenario: String,
    pub(crate) expected_report: PathBuf,
    pub(crate) expected_markdown: PathBuf,
    pub(crate) expected_status: String,
    pub(crate) expected_mode: String,
    pub(crate) expected_publishable: usize,
    pub(crate) expected_skipped: usize,
    pub(crate) expected_blocked: usize,
    pub(crate) expected_safe_to_publish: bool,
    pub(crate) expected_operations: Vec<String>,
    pub(crate) expected_skip_reasons: Vec<String>,
    pub(crate) expected_blocked_reasons: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct DogfoodPrInlineCommentRun {
    pub(crate) name: String,
    pub(crate) actual_dir: PathBuf,
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) status: String,
    pub(crate) mode: String,
    pub(crate) publishable: usize,
    pub(crate) skipped: usize,
    pub(crate) blocked: usize,
    pub(crate) safe_to_publish: bool,
    pub(crate) operations: Vec<String>,
    pub(crate) skip_reasons: Vec<String>,
    pub(crate) blocked_reasons: Vec<String>,
    pub(crate) expected_status: String,
    pub(crate) expected_mode: String,
    pub(crate) expected_publishable: usize,
    pub(crate) expected_skipped: usize,
    pub(crate) expected_blocked: usize,
    pub(crate) expected_safe_to_publish: bool,
    pub(crate) expected_operations: Vec<String>,
    pub(crate) expected_skip_reasons: Vec<String>,
    pub(crate) expected_blocked_reasons: Vec<String>,
    pub(crate) reason: String,
    pub(crate) expected_report: PathBuf,
    pub(crate) expected_markdown: PathBuf,
    pub(crate) errors: Vec<String>,
}

pub(crate) fn dogfood_impl() -> Result<(), String> {
    // Same shared-binary build as fixtures_impl (#2110).
    run("cargo", &["build", "-p", "ripr"])?;
    let runs = dogfood_scenarios()
        .into_iter()
        .map(|scenario| dogfood_run(&scenario))
        .collect::<Result<Vec<_>, _>>()?;
    let gate_binary = ripr_fixture_binary()?;
    let gate_runs = dogfood_gate_adoption_scenarios()
        .into_iter()
        .map(|scenario| dogfood_gate_adoption_run_with_binary(&scenario, &gate_binary))
        .collect::<Result<Vec<_>, _>>()?;
    let first_action_runs = dogfood_first_action_scenarios()
        .into_iter()
        .map(|scenario| dogfood_first_action_run(&scenario))
        .collect::<Vec<_>>();
    let first_pr_runs = dogfood_first_pr_scenarios()
        .into_iter()
        .map(|scenario| dogfood_first_pr_run(&scenario))
        .collect::<Vec<_>>();
    let front_panel_runs = dogfood_pr_review_front_panel_scenarios()
        .into_iter()
        .map(|scenario| dogfood_pr_review_front_panel_run(&scenario))
        .collect::<Vec<_>>();
    let report_packet_index_runs = dogfood_report_packet_index_scenarios()
        .into_iter()
        .map(|scenario| dogfood_report_packet_index_run(&scenario))
        .collect::<Result<Vec<_>, _>>()?;
    let generated_ci_cockpit_runs = vec![dogfood_generated_ci_cockpit_run()?];
    let language_preview_runs = dogfood_language_preview_scenarios()
        .into_iter()
        .map(|scenario| dogfood_language_preview_run(&scenario))
        .collect::<Vec<_>>();
    let editor_gap_cockpit_runs = dogfood_editor_gap_cockpit_scenarios()
        .into_iter()
        .map(|scenario| dogfood_editor_gap_cockpit_run(&scenario))
        .collect::<Vec<_>>();
    let editor_first_pr_bridge_runs = dogfood_editor_first_pr_bridge_scenarios()
        .into_iter()
        .map(|scenario| dogfood_editor_first_pr_bridge_run(&scenario))
        .collect::<Vec<_>>();
    let finding_alignment_runs = dogfood_finding_alignment_scenarios()
        .into_iter()
        .map(|scenario| dogfood_finding_alignment_run(&scenario))
        .collect::<Vec<_>>();
    let surface_projection_alignment_runs = dogfood_surface_projection_alignment_scenarios()
        .into_iter()
        .map(|scenario| dogfood_surface_projection_alignment_run(&scenario))
        .collect::<Vec<_>>();
    let real_repair_attempt_runs = dogfood_real_repair_attempt_scenarios()
        .into_iter()
        .map(|scenario| dogfood_real_repair_attempt_run(&scenario))
        .collect::<Vec<_>>();
    let python_real_repo_eval_runs = dogfood_python_real_repo_eval_scenarios()
        .into_iter()
        .map(|scenario| dogfood_python_real_repo_eval_run(&scenario))
        .collect::<Vec<_>>();
    let python_static_limit_eval_runs = dogfood_python_static_limit_eval_scenarios()
        .into_iter()
        .map(|scenario| dogfood_python_static_limit_eval_run(&scenario))
        .collect::<Vec<_>>();
    let python_no_action_eval_runs = dogfood_python_no_action_eval_scenarios()
        .into_iter()
        .map(|scenario| dogfood_python_no_action_eval_run(&scenario))
        .collect::<Vec<_>>();
    let typescript_preview_repair_loop_runs = dogfood_typescript_preview_repair_loop_scenarios()
        .into_iter()
        .map(|scenario| dogfood_typescript_preview_repair_loop_run(&scenario))
        .collect::<Vec<_>>();
    let bun_ub_cross_language_runs = dogfood_bun_ub_cross_language_scenarios()
        .into_iter()
        .map(|scenario| dogfood_bun_ub_cross_language_run(&scenario))
        .collect::<Vec<_>>();
    let user_surface_projection_runs = dogfood_user_surface_projection_scenarios()
        .into_iter()
        .map(|scenario| dogfood_user_surface_projection_run(&scenario))
        .collect::<Vec<_>>();
    let preview_projection_runs = DogfoodPreviewProjectionRuns {
        generated_ci_cockpit: &generated_ci_cockpit_runs,
        language_preview: &language_preview_runs,
        editor_gap_cockpit: &editor_gap_cockpit_runs,
        editor_first_pr_bridge: &editor_first_pr_bridge_runs,
    };
    let pr_inline_comment_runs = dogfood_pr_inline_comment_scenarios()
        .into_iter()
        .map(|scenario| dogfood_pr_inline_comment_run(&scenario))
        .collect::<Result<Vec<_>, _>>()?;
    let report_inputs = DogfoodReportInputs {
        runs: &runs,
        gate_runs: &gate_runs,
        first_action_runs: &first_action_runs,
        first_pr_runs: &first_pr_runs,
        front_panel_runs: &front_panel_runs,
        report_packet_index_runs: &report_packet_index_runs,
        preview_projection_runs: &preview_projection_runs,
        finding_alignment_runs: &finding_alignment_runs,
        surface_projection_alignment_runs: &surface_projection_alignment_runs,
        real_repair_attempt_runs: &real_repair_attempt_runs,
        python_real_repo_eval_runs: &python_real_repo_eval_runs,
        python_static_limit_eval_runs: &python_static_limit_eval_runs,
        python_no_action_eval_runs: &python_no_action_eval_runs,
        typescript_preview_repair_loop_runs: &typescript_preview_repair_loop_runs,
        bun_ub_cross_language_runs: &bun_ub_cross_language_runs,
        user_surface_projection_runs: &user_surface_projection_runs,
        pr_inline_comment_runs: &pr_inline_comment_runs,
    };
    write_report("dogfood.md", &dogfood_report_markdown(&report_inputs))?;
    write_report("dogfood.json", &dogfood_report_json(&report_inputs))?;

    // Aggregate scenario outcomes into the gate exit code (#2411).
    // Previously the gate returned Ok(()) as long as the report file wrote,
    // regardless of whether scenarios recorded errors. Now we scan all run
    // families for non-empty errors vectors and return Err if any failed.
    let mut failed: Vec<String> = Vec::new();
    for run in runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in gate_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in first_action_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in first_pr_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in front_panel_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in report_packet_index_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in finding_alignment_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in surface_projection_alignment_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in real_repair_attempt_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in python_real_repo_eval_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in python_static_limit_eval_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in python_no_action_eval_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in typescript_preview_repair_loop_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in bun_ub_cross_language_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in user_surface_projection_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    for run in pr_inline_comment_runs {
        if !run.errors.is_empty() {
            failed.push(format!("{}: {} error(s)", run.name, run.errors.len()));
        }
    }
    if !failed.is_empty() {
        return Err(format!(
            "dogfood: {} scenario family/families recorded errors: {}",
            failed.len(),
            failed.join("; ")
        ));
    }
    Ok(())
}

pub(crate) fn dogfood_scenarios() -> Vec<DogfoodScenario> {
    ["boundary_gap".to_string(), "weak_error_oracle".to_string()]
        .into_iter()
        .map(|name| {
            let base = Path::new("fixtures").join(&name);
            DogfoodScenario {
                name,
                root: base.join("input"),
                diff: base.join("diff.patch"),
            }
        })
        .collect()
}

pub(crate) fn dogfood_run(scenario: &DogfoodScenario) -> Result<DogfoodRun, String> {
    let started = Instant::now();
    let actual_dir = Path::new("target")
        .join("ripr")
        .join("dogfood")
        .join(&scenario.name);
    fs::create_dir_all(&actual_dir).map_err(|err| {
        format!(
            "failed to create dogfood output directory {}: {err}",
            normalize_path(&actual_dir)
        )
    })?;

    let mut errors = Vec::new();
    let mut findings = 0usize;
    let mut class_counts = BTreeMap::new();
    let mut stop_reason_mentions = 0usize;

    if !scenario.root.exists() {
        errors.push(format!(
            "fixture root does not exist: {}",
            normalize_path(&scenario.root)
        ));
    }
    if !scenario.diff.exists() {
        errors.push(format!(
            "fixture diff does not exist: {}",
            normalize_path(&scenario.diff)
        ));
    }

    if errors.is_empty() {
        let root = normalize_path(&scenario.root);
        let diff = normalize_path(&scenario.diff);
        match run_fixture_check(&root, &diff, FixtureCheckFormat::Json, None) {
            Ok(json) => {
                let normalized = normalize_fixture_json_output(&json);
                findings = json_number_after(&normalized, "\"findings\":").unwrap_or(0);
                class_counts = dogfood_class_counts(&normalized);
                stop_reason_mentions = normalized.matches("\"stop_reasons\"").count();
                let path = actual_dir.join("check.json");
                fs::write(&path, normalized).map_err(|err| {
                    format!(
                        "failed to write dogfood JSON output {}: {err}",
                        normalize_path(&path)
                    )
                })?;
            }
            Err(err) => errors.push(err),
        }

        match run_fixture_check(&root, &diff, FixtureCheckFormat::Human, None) {
            Ok(human) => {
                let normalized = normalize_fixture_human_output(&human);
                let path = actual_dir.join("human.txt");
                fs::write(&path, normalized).map_err(|err| {
                    format!(
                        "failed to write dogfood human output {}: {err}",
                        normalize_path(&path)
                    )
                })?;
            }
            Err(err) => errors.push(err),
        }
    }

    Ok(DogfoodRun {
        name: scenario.name.clone(),
        root: scenario.root.clone(),
        diff: scenario.diff.clone(),
        actual_dir,
        duration_ms: started.elapsed().as_millis(),
        findings,
        class_counts,
        stop_reason_mentions,
        errors,
    })
}

pub(crate) fn dogfood_gate_adoption_scenarios() -> Vec<DogfoodGateScenario> {
    vec![
        DogfoodGateScenario {
            name: "visible-only-advisory",
            mode: "visible-only",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/visible-only",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            baseline: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            expected_status: "advisory",
            expected_blocking: 0,
            expected_acknowledged: 0,
            expected_advisory: 1,
            expected_exit_success: true,
        },
        DogfoodGateScenario {
            name: "acknowledged-waiver",
            mode: "acknowledgeable",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/acknowledged",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: Some(
                "fixtures/boundary_gap/expected/gate-adoption/acknowledged/labels.json",
            ),
            baseline: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            expected_status: "acknowledged",
            expected_blocking: 0,
            expected_acknowledged: 1,
            expected_advisory: 0,
            expected_exit_success: true,
        },
        DogfoodGateScenario {
            name: "baseline-check-existing",
            mode: "baseline-check",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/baseline-aware",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/gate-adoption/baseline-aware/baseline.json",
            ),
            recommendation_calibration: None,
            mutation_calibration: None,
            expected_status: "advisory",
            expected_blocking: 0,
            expected_acknowledged: 0,
            expected_advisory: 1,
            expected_exit_success: true,
        },
        DogfoodGateScenario {
            name: "baseline-check-new-gap",
            mode: "baseline-check",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/baseline-new-gap",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/gate-adoption/baseline-new-gap/baseline.json",
            ),
            recommendation_calibration: None,
            mutation_calibration: None,
            expected_status: "blocked",
            expected_blocking: 1,
            expected_acknowledged: 0,
            expected_advisory: 0,
            expected_exit_success: false,
        },
        DogfoodGateScenario {
            name: "calibrated-high-confidence-new-gap",
            mode: "calibrated-gate",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/calibrated-gate",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/gate-adoption/calibrated-gate/baseline.json",
            ),
            recommendation_calibration: Some(
                "fixtures/boundary_gap/expected/gate-adoption/calibrated-gate/recommendation-calibration.json",
            ),
            mutation_calibration: None,
            expected_status: "blocked",
            expected_blocking: 1,
            expected_acknowledged: 0,
            expected_advisory: 0,
            expected_exit_success: false,
        },
        DogfoodGateScenario {
            name: "missing-baseline-config",
            mode: "baseline-check",
            expected_dir: "fixtures/boundary_gap/expected/gate-adoption/missing-baseline-config",
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            baseline: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            expected_status: "config_error",
            expected_blocking: 0,
            expected_acknowledged: 0,
            expected_advisory: 0,
            expected_exit_success: false,
        },
    ]
}

#[cfg(test)]
pub(crate) fn dogfood_gate_adoption_run(
    scenario: &DogfoodGateScenario,
) -> Result<DogfoodGateRun, String> {
    // Direct test callers must refresh the binary so gate receipts cannot
    // validate a stale artifact left by an earlier build.
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = ripr_fixture_binary()?;
    dogfood_gate_adoption_run_with_binary(scenario, &binary)
}

pub(crate) fn dogfood_gate_adoption_run_with_binary(
    scenario: &DogfoodGateScenario,
    binary: &str,
) -> Result<DogfoodGateRun, String> {
    let started = Instant::now();
    let actual_dir = Path::new("target")
        .join("ripr")
        .join("dogfood")
        .join("gate-adoption")
        .join(scenario.name);
    fs::create_dir_all(&actual_dir).map_err(|err| {
        format!(
            "failed to create gate adoption dogfood directory {}: {err}",
            normalize_path(&actual_dir)
        )
    })?;

    // #3065: evaluate against a private, empty root so repo-local
    // `target/ripr/pr/` state (a canonical delta left by `cargo xtask ripr-pr`)
    // cannot enter the gate's causal comparison — the gate loads
    // `<root>/target/ripr/pr/canonical-delta.json` whenever it exists. The CLI
    // surface stays byte-identical (`--root .` plus the same relative fixture
    // paths), so rendered inputs and committed goldens are unchanged; only the
    // process cwd moves to the private root, with the scenario's file inputs
    // mirrored there under the same relative paths.
    let gate_root = actual_dir.join("gate-root");
    if gate_root.exists() {
        fs::remove_dir_all(&gate_root).map_err(|err| {
            format!(
                "failed to clear gate adoption private root {}: {err}",
                normalize_path(&gate_root)
            )
        })?;
    }
    mirror_gate_scenario_inputs(scenario, &gate_root)?;

    let json_path = actual_dir.join("gate-decision.json");
    let markdown_path = actual_dir.join("gate-decision.md");
    let json_out = absolute_gate_out(&json_path)?;
    let markdown_out = absolute_gate_out(&markdown_path)?;
    let args = dogfood_gate_adoption_args(scenario, Path::new(&json_out), Path::new(&markdown_out));
    let output = capture_output_in_dir(binary, &args, &gate_root, "ripr gate evaluate (dogfood)")?;
    let exit_success = output.status.success();
    let mut errors = Vec::new();
    if exit_success != scenario.expected_exit_success {
        let stderr = output.stderr.trim();
        if stderr.is_empty() {
            errors.push(format!(
                "expected exit success {}, got {}",
                scenario.expected_exit_success, exit_success
            ));
        } else {
            errors.push(format!(
                "expected exit success {}, got {}; stderr: {}",
                scenario.expected_exit_success,
                exit_success,
                one_line(stderr)
            ));
        }
    }

    let mut status = "missing".to_string();
    let mut blocking = 0usize;
    let mut acknowledged = 0usize;
    let mut advisory = 0usize;

    match fs::read_to_string(&json_path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                status = value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                blocking = json_summary_count(&value, "blocking");
                acknowledged = json_summary_count(&value, "acknowledged");
                advisory = json_summary_count(&value, "advisory");
            }
            Err(err) => errors.push(format!(
                "failed to parse gate decision JSON {}: {err}",
                normalize_path(&json_path)
            )),
        },
        Err(err) => errors.push(format!(
            "failed to read gate decision JSON {}: {err}",
            normalize_path(&json_path)
        )),
    }

    if !markdown_path.exists() {
        errors.push(format!(
            "missing gate decision Markdown {}",
            normalize_path(&markdown_path)
        ));
    }
    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if blocking != scenario.expected_blocking {
        errors.push(format!(
            "expected {} blocking decision(s), got {}",
            scenario.expected_blocking, blocking
        ));
    }
    if acknowledged != scenario.expected_acknowledged {
        errors.push(format!(
            "expected {} acknowledged decision(s), got {}",
            scenario.expected_acknowledged, acknowledged
        ));
    }
    if advisory != scenario.expected_advisory {
        errors.push(format!(
            "expected {} advisory decision(s), got {}",
            scenario.expected_advisory, advisory
        ));
    }
    let expected_dir = Path::new(scenario.expected_dir);
    compare_expected_text(
        &json_path,
        &expected_dir.join("gate-decision.json"),
        "gate decision JSON",
        &mut errors,
    );
    compare_expected_text(
        &markdown_path,
        &expected_dir.join("gate-decision.md"),
        "gate decision Markdown",
        &mut errors,
    );

    Ok(DogfoodGateRun {
        name: scenario.name.to_string(),
        mode: scenario.mode.to_string(),
        actual_dir,
        json_path,
        markdown_path,
        duration_ms: started.elapsed().as_millis(),
        status,
        blocking,
        acknowledged,
        advisory,
        expected_status: scenario.expected_status.to_string(),
        expected_blocking: scenario.expected_blocking,
        expected_acknowledged: scenario.expected_acknowledged,
        expected_advisory: scenario.expected_advisory,
        exit_success,
        expected_exit_success: scenario.expected_exit_success,
        errors,
    })
}

/// Absolute form for the gate's `--out`/`--out-md`: the process cwd moves to
/// the private gate root (#3065), so outputs must be addressed absolutely to
/// land next to the scenario's other actual artifacts.
fn absolute_gate_out(path: &Path) -> Result<String, String> {
    std::path::absolute(path)
        .map(|absolute| absolute.to_string_lossy().into_owned())
        .map_err(|err| format!("resolve {} failed: {err}", normalize_path(path)))
}

/// Mirror the scenario's file inputs into the private gate root under their
/// original relative paths, so the gate resolves the same bytes while its
/// `<root>/target/ripr/pr/` defaults stay empty (#3065).
fn mirror_gate_scenario_inputs(
    scenario: &DogfoodGateScenario,
    gate_root: &Path,
) -> Result<(), String> {
    for input in [
        Some(scenario.pr_guidance),
        scenario.labels_json,
        scenario.baseline,
        scenario.recommendation_calibration,
        scenario.mutation_calibration,
    ]
    .into_iter()
    .flatten()
    {
        let destination = gate_root.join(input);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create private gate root input directory {}: {err}",
                    normalize_path(parent)
                )
            })?;
        }
        fs::copy(input, &destination).map_err(|err| {
            format!(
                "failed to mirror gate input {input} into {}: {err}",
                normalize_path(gate_root)
            )
        })?;
    }
    Ok(())
}

pub(crate) fn dogfood_gate_adoption_args(
    scenario: &DogfoodGateScenario,
    json_path: &Path,
    markdown_path: &Path,
) -> Vec<String> {
    let mut args = vec![
        "gate".to_string(),
        "evaluate".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--pr-guidance".to_string(),
        scenario.pr_guidance.to_string(),
        "--mode".to_string(),
        scenario.mode.to_string(),
        "--out".to_string(),
        normalize_path(json_path),
        "--out-md".to_string(),
        normalize_path(markdown_path),
    ];
    if let Some(path) = scenario.labels_json {
        args.push("--labels-json".to_string());
        args.push(path.to_string());
    }
    if let Some(path) = scenario.baseline {
        args.push("--baseline".to_string());
        args.push(path.to_string());
    }
    if let Some(path) = scenario.recommendation_calibration {
        args.push("--recommendation-calibration".to_string());
        args.push(path.to_string());
    }
    if let Some(path) = scenario.mutation_calibration {
        args.push("--mutation-calibration".to_string());
        args.push(path.to_string());
    }
    args
}

pub(crate) fn dogfood_first_action_scenarios() -> Vec<DogfoodFirstActionScenario> {
    vec![
        DogfoodFirstActionScenario {
            name: "actionable",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/actionable",
            expected_status: "actionable",
            expected_action_kind: "write_focused_test",
            expected_audience: "developer",
            expected_selected: true,
            expected_static_movement: "unknown",
        },
        DogfoodFirstActionScenario {
            name: "baseline-only",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/baseline-only",
            expected_status: "baseline_only",
            expected_action_kind: "acknowledge_baseline",
            expected_audience: "reviewer",
            expected_selected: true,
            expected_static_movement: "unknown",
        },
        DogfoodFirstActionScenario {
            name: "stale",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/stale",
            expected_status: "stale",
            expected_action_kind: "refresh_evidence",
            expected_audience: "developer",
            expected_selected: true,
            expected_static_movement: "unknown",
        },
        DogfoodFirstActionScenario {
            name: "missing-required-artifact",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/missing-required-artifact",
            expected_status: "missing_required_artifact",
            expected_action_kind: "generate_missing_artifact",
            expected_audience: "agent",
            expected_selected: false,
            expected_static_movement: "unknown",
        },
        DogfoodFirstActionScenario {
            name: "unchanged-after-attempt",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt",
            expected_status: "unchanged_after_attempt",
            expected_action_kind: "revise_focused_test",
            expected_audience: "agent",
            expected_selected: true,
            expected_static_movement: "unchanged",
        },
        DogfoodFirstActionScenario {
            name: "no-actionable-seam",
            expected_dir: "fixtures/boundary_gap/expected/first-useful-action/no-actionable-seam",
            expected_status: "no_actionable_seam",
            expected_action_kind: "no_action",
            expected_audience: "developer",
            expected_selected: false,
            expected_static_movement: "unknown",
        },
    ]
}

pub(crate) fn dogfood_first_action_run(
    scenario: &DogfoodFirstActionScenario,
) -> DogfoodFirstActionRun {
    let expected_dir = Path::new(scenario.expected_dir).to_path_buf();
    let json_path = expected_dir.join("first-useful-action.json");
    let markdown_path = expected_dir.join("first-useful-action.md");
    let mut errors = Vec::new();
    let mut status = "missing".to_string();
    let mut action_kind = "missing".to_string();
    let mut audience = "missing".to_string();
    let mut selected = false;
    let mut static_movement = "unknown".to_string();

    match fs::read_to_string(&json_path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                status = value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                action_kind = value
                    .get("action_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                audience = value
                    .get("audience")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                selected = value.get("selected").is_some_and(|value| !value.is_null());
                static_movement = value
                    .get("evidence")
                    .and_then(|evidence| evidence.get("static_movement"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                if !value
                    .get("limits")
                    .and_then(Value::as_array)
                    .is_some_and(|limits| {
                        limits
                            .iter()
                            .any(|limit| limit.as_str() == Some("Static evidence only."))
                    })
                {
                    errors.push("missing static-evidence limit".to_string());
                }
            }
            Err(err) => errors.push(format!(
                "failed to parse first useful action JSON {}: {err}",
                normalize_path(&json_path)
            )),
        },
        Err(err) => errors.push(format!(
            "failed to read first useful action JSON {}: {err}",
            normalize_path(&json_path)
        )),
    }

    match fs::read_to_string(&markdown_path) {
        Ok(markdown) => {
            if !markdown.contains(&format!("Status: {}", scenario.expected_status)) {
                errors.push(format!(
                    "Markdown should pin status {}",
                    scenario.expected_status
                ));
            }
            if !markdown.contains(&format!("Action: {}", scenario.expected_action_kind)) {
                errors.push(format!(
                    "Markdown should pin action {}",
                    scenario.expected_action_kind
                ));
            }
        }
        Err(err) => errors.push(format!(
            "failed to read first useful action Markdown {}: {err}",
            normalize_path(&markdown_path)
        )),
    }

    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if action_kind != scenario.expected_action_kind {
        errors.push(format!(
            "expected action {}, got {}",
            scenario.expected_action_kind, action_kind
        ));
    }
    if audience != scenario.expected_audience {
        errors.push(format!(
            "expected audience {}, got {}",
            scenario.expected_audience, audience
        ));
    }
    if selected != scenario.expected_selected {
        errors.push(format!(
            "expected selected {}, got {}",
            scenario.expected_selected, selected
        ));
    }
    if static_movement != scenario.expected_static_movement {
        errors.push(format!(
            "expected static movement {}, got {}",
            scenario.expected_static_movement, static_movement
        ));
    }

    DogfoodFirstActionRun {
        name: scenario.name.to_string(),
        expected_dir,
        json_path,
        markdown_path,
        status,
        action_kind,
        audience,
        selected,
        static_movement,
        expected_status: scenario.expected_status.to_string(),
        expected_action_kind: scenario.expected_action_kind.to_string(),
        expected_audience: scenario.expected_audience.to_string(),
        expected_selected: scenario.expected_selected,
        expected_static_movement: scenario.expected_static_movement.to_string(),
        errors,
    }
}

pub(crate) fn dogfood_first_pr_scenarios() -> Vec<DogfoodFirstPrScenario> {
    let corpus_path = Path::new("fixtures/first_successful_pr/corpus.json");
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => {
            return vec![DogfoodFirstPrScenario {
                name: "corpus".to_string(),
                expected_dir: corpus_path.to_path_buf(),
                expected_status: "missing".to_string(),
                expected_state: "missing".to_string(),
                description: err,
            }];
        }
    };
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return vec![DogfoodFirstPrScenario {
            name: "corpus".to_string(),
            expected_dir: corpus_path.to_path_buf(),
            expected_status: "missing".to_string(),
            expected_state: "missing".to_string(),
            description: "first successful PR corpus is missing cases array".to_string(),
        }];
    };

    cases
        .iter()
        .map(|case| {
            let name = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
            let expected_dir = Path::new("fixtures/first_successful_pr")
                .join(&name)
                .join("expected");
            DogfoodFirstPrScenario {
                name,
                expected_dir,
                expected_status: json_string_field(case, "expected_status")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_state: json_string_field(case, "expected_state")
                    .unwrap_or_else(|| "missing".to_string()),
                description: json_string_field(case, "description")
                    .unwrap_or_else(|| "first-pr corpus case has no description".to_string()),
            }
        })
        .collect()
}

pub(crate) fn dogfood_first_pr_run(scenario: &DogfoodFirstPrScenario) -> DogfoodFirstPrRun {
    let json_path = scenario.expected_dir.join("start-here.json");
    let markdown_path = scenario.expected_dir.join("start-here.md");
    let mut errors = Vec::new();
    let mut status = "missing".to_string();
    let mut state = "missing".to_string();
    let mut top_gap_kind = "none".to_string();
    let mut verify_command = None;
    let mut next_command = None;

    match read_json_value(&json_path) {
        Ok(packet) => {
            if json_string_field(&packet, "kind").as_deref() != Some("first_pr_start_here") {
                errors.push("start-here kind must be first_pr_start_here".to_string());
            }
            status = json_string_field(&packet, "status").unwrap_or_else(|| "missing".to_string());
            state = audit_string(&packet, &["selected", "state"])
                .unwrap_or_else(|| "missing".to_string());
            top_gap_kind =
                audit_string(&packet, &["selected", "kind"]).unwrap_or_else(|| "none".to_string());
            verify_command = audit_string(&packet, &["selected", "verify_command"]);
            next_command = audit_string(&packet, &["commands", "next"]);
            if json_string_field(&packet, "posture").as_deref() != Some("advisory") {
                errors.push("start-here posture must stay advisory".to_string());
            }
            let limits = packet
                .get("limits")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            for required in [
                "Composes explicit RIPR artifacts only.",
                "Does not run hidden analysis.",
                "Does not edit source or generate tests.",
                "Does not run mutation testing.",
                "Does not change CI blocking or gate policy.",
            ] {
                if !limits.contains(&required) {
                    errors.push(format!("start-here packet is missing limit `{required}`"));
                }
            }
            if scenario.expected_status == "actionable" && verify_command.is_none() {
                errors.push("actionable start-here receipt must name verify_command".to_string());
            }
            if scenario.expected_status == "actionable" {
                validate_first_successful_pr_actionable_json(&scenario.name, &packet, &mut errors);
            }
            if scenario.expected_status == "blocked" && next_command.is_none() {
                errors.push("blocked start-here receipt must name next command".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    match fs::read_to_string(&markdown_path) {
        Ok(markdown) => {
            if !markdown.contains("# RIPR First PR Start Here") {
                errors.push("Markdown must use the first PR start-here heading".to_string());
            }
            if !markdown.contains("Status: advisory") {
                errors.push("Markdown must pin advisory status".to_string());
            }
            if !markdown.contains("## Authority") {
                errors.push("Markdown must name authority boundary".to_string());
            }
            if !markdown.contains("## Start Here") {
                errors.push("Markdown must show the Start Here state block".to_string());
            }
            let expected_state_marker = format!("- State: `{}`", scenario.expected_state);
            if !markdown.contains(&expected_state_marker) {
                errors.push(format!(
                    "Markdown must show expected state `{}`",
                    scenario.expected_state
                ));
            }
            if scenario.expected_status == "actionable" {
                validate_first_successful_pr_actionable_markdown(
                    &scenario.name,
                    &markdown,
                    &mut errors,
                );
            }
        }
        Err(err) => errors.push(format!(
            "failed to read first PR start-here Markdown {}: {err}",
            normalize_path(&markdown_path)
        )),
    }

    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if state != scenario.expected_state {
        errors.push(format!(
            "expected state {}, got {}",
            scenario.expected_state, state
        ));
    }

    DogfoodFirstPrRun {
        name: scenario.name.clone(),
        expected_dir: scenario.expected_dir.clone(),
        json_path,
        markdown_path,
        status,
        state,
        top_gap_kind,
        verify_command,
        next_command,
        expected_status: scenario.expected_status.clone(),
        expected_state: scenario.expected_state.clone(),
        description: scenario.description.clone(),
        errors,
    }
}

pub(crate) fn dogfood_pr_review_front_panel_scenarios() -> Vec<DogfoodFrontPanelScenario> {
    let corpus_path = Path::new("fixtures/boundary_gap/expected/pr-review-front-panel/corpus.json");
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => {
            return vec![DogfoodFrontPanelScenario {
                name: "corpus".to_string(),
                report_path: corpus_path.to_path_buf(),
                markdown_path: corpus_path.to_path_buf(),
                expected_status: "missing".to_string(),
                expected_top_issue_state: "missing".to_string(),
                expected_policy_state: "missing".to_string(),
                expected_placement: "missing".to_string(),
                expected_movement_state: "missing".to_string(),
                expected_coverage_grip_state: "missing".to_string(),
                expected_new_policy_eligible: 0,
                expected_baseline_resolved: 0,
                expected_blocking_candidates: 0,
                expected_warnings: 0,
                reason: err,
            }];
        }
    };

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return vec![DogfoodFrontPanelScenario {
            name: "corpus".to_string(),
            report_path: corpus_path.to_path_buf(),
            markdown_path: corpus_path.to_path_buf(),
            expected_status: "missing".to_string(),
            expected_top_issue_state: "missing".to_string(),
            expected_policy_state: "missing".to_string(),
            expected_placement: "missing".to_string(),
            expected_movement_state: "missing".to_string(),
            expected_coverage_grip_state: "missing".to_string(),
            expected_new_policy_eligible: 0,
            expected_baseline_resolved: 0,
            expected_blocking_candidates: 0,
            expected_warnings: 0,
            reason: "front-panel corpus is missing cases array".to_string(),
        }];
    };

    cases
        .iter()
        .map(|case| {
            let expected = case.get("expected").unwrap_or(&Value::Null);
            DogfoodFrontPanelScenario {
                name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
                report_path: json_string_field(case, "expected_report")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| corpus_path.to_path_buf()),
                markdown_path: json_string_field(case, "expected_markdown")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| corpus_path.to_path_buf()),
                expected_status: json_string_field(expected, "status")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_top_issue_state: json_string_field(expected, "top_issue_state")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_policy_state: json_string_field(expected, "policy_state")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_placement: json_string_field(expected, "placement")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_movement_state: json_string_field(expected, "movement_state")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_coverage_grip_state: json_string_field(expected, "coverage_grip_state")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_new_policy_eligible: json_usize_field(expected, "new_policy_eligible")
                    .unwrap_or(0),
                expected_baseline_resolved: json_usize_field(expected, "baseline_resolved")
                    .unwrap_or(0),
                expected_blocking_candidates: json_usize_field(expected, "blocking_candidates")
                    .unwrap_or(0),
                expected_warnings: json_usize_field(expected, "warnings").unwrap_or(0),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "front-panel corpus case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn dogfood_pr_review_front_panel_run(
    scenario: &DogfoodFrontPanelScenario,
) -> DogfoodFrontPanelRun {
    let mut errors = Vec::new();
    let mut status = "missing".to_string();
    let mut top_issue_state = "missing".to_string();
    let mut policy_state = "missing".to_string();
    let mut placement = "missing".to_string();
    let mut movement_state = "missing".to_string();
    let mut coverage_grip_state = "missing".to_string();
    let mut new_policy_eligible = 0usize;
    let mut baseline_resolved = 0usize;
    let mut blocking_candidates = 0usize;
    let mut warnings = 0usize;

    match read_json_value(&scenario.report_path) {
        Ok(report) => {
            if json_string_field(&report, "kind").as_deref() != Some("pr_review_front_panel") {
                errors.push("report kind must be pr_review_front_panel".to_string());
            }
            status = json_string_field(&report, "status").unwrap_or_else(|| "missing".to_string());
            if let Some(summary) = report.get("summary") {
                top_issue_state = json_string_field(summary, "top_issue_state")
                    .unwrap_or_else(|| "missing".to_string());
                policy_state = json_string_field(summary, "policy_state")
                    .unwrap_or_else(|| "missing".to_string());
                placement = json_string_field(summary, "placement")
                    .unwrap_or_else(|| "missing".to_string());
                movement_state = json_string_field(summary, "movement_state")
                    .unwrap_or_else(|| "missing".to_string());
                coverage_grip_state = json_string_field(summary, "coverage_grip_state")
                    .unwrap_or_else(|| "missing".to_string());
                new_policy_eligible = json_usize_field(summary, "new_policy_eligible").unwrap_or(0);
                baseline_resolved = json_usize_field(summary, "baseline_resolved").unwrap_or(0);
                blocking_candidates = json_usize_field(summary, "blocking_candidates").unwrap_or(0);
                warnings = json_usize_field(summary, "warnings").unwrap_or(0);
            } else {
                errors.push("report summary is missing".to_string());
            }
            if !report
                .get("limits")
                .and_then(Value::as_array)
                .is_some_and(|limits| {
                    limits
                        .iter()
                        .any(|limit| limit.as_str() == Some("Static RIPR evidence only."))
                })
            {
                errors.push("report is missing static-evidence limit".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    match fs::read_to_string(&scenario.markdown_path) {
        Ok(markdown) => {
            if !markdown.contains("# RIPR PR Review") {
                errors.push("Markdown must use the PR review heading".to_string());
            }
            if !markdown.contains(&format!("Status: {}", scenario.expected_status)) {
                errors.push(format!(
                    "Markdown should pin status {}",
                    scenario.expected_status
                ));
            }
        }
        Err(err) => errors.push(format!(
            "failed to read front-panel Markdown {}: {err}",
            normalize_path(&scenario.markdown_path)
        )),
    }

    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if top_issue_state != scenario.expected_top_issue_state {
        errors.push(format!(
            "expected top_issue_state {}, got {}",
            scenario.expected_top_issue_state, top_issue_state
        ));
    }
    if policy_state != scenario.expected_policy_state {
        errors.push(format!(
            "expected policy_state {}, got {}",
            scenario.expected_policy_state, policy_state
        ));
    }
    if placement != scenario.expected_placement {
        errors.push(format!(
            "expected placement {}, got {}",
            scenario.expected_placement, placement
        ));
    }
    if movement_state != scenario.expected_movement_state {
        errors.push(format!(
            "expected movement_state {}, got {}",
            scenario.expected_movement_state, movement_state
        ));
    }
    if coverage_grip_state != scenario.expected_coverage_grip_state {
        errors.push(format!(
            "expected coverage_grip_state {}, got {}",
            scenario.expected_coverage_grip_state, coverage_grip_state
        ));
    }
    if new_policy_eligible != scenario.expected_new_policy_eligible {
        errors.push(format!(
            "expected new_policy_eligible {}, got {}",
            scenario.expected_new_policy_eligible, new_policy_eligible
        ));
    }
    if baseline_resolved != scenario.expected_baseline_resolved {
        errors.push(format!(
            "expected baseline_resolved {}, got {}",
            scenario.expected_baseline_resolved, baseline_resolved
        ));
    }
    if blocking_candidates != scenario.expected_blocking_candidates {
        errors.push(format!(
            "expected blocking_candidates {}, got {}",
            scenario.expected_blocking_candidates, blocking_candidates
        ));
    }
    if warnings != scenario.expected_warnings {
        errors.push(format!(
            "expected warnings {}, got {}",
            scenario.expected_warnings, warnings
        ));
    }

    DogfoodFrontPanelRun {
        name: scenario.name.clone(),
        report_path: scenario.report_path.clone(),
        markdown_path: scenario.markdown_path.clone(),
        status,
        top_issue_state,
        policy_state,
        placement,
        movement_state,
        coverage_grip_state,
        new_policy_eligible,
        baseline_resolved,
        blocking_candidates,
        warnings,
        expected_status: scenario.expected_status.clone(),
        expected_top_issue_state: scenario.expected_top_issue_state.clone(),
        expected_policy_state: scenario.expected_policy_state.clone(),
        expected_placement: scenario.expected_placement.clone(),
        expected_movement_state: scenario.expected_movement_state.clone(),
        expected_coverage_grip_state: scenario.expected_coverage_grip_state.clone(),
        expected_new_policy_eligible: scenario.expected_new_policy_eligible,
        expected_baseline_resolved: scenario.expected_baseline_resolved,
        expected_blocking_candidates: scenario.expected_blocking_candidates,
        expected_warnings: scenario.expected_warnings,
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_report_packet_index_scenarios() -> Vec<DogfoodReportPacketIndexScenario> {
    let corpus_path = Path::new("fixtures/boundary_gap/expected/report-packet-index/corpus.json");
    let fallback = |reason: String| {
        vec![DogfoodReportPacketIndexScenario {
            name: "corpus".to_string(),
            scenario: reason.clone(),
            expected_report: corpus_path.to_path_buf(),
            expected_markdown: corpus_path.to_path_buf(),
            expected_status: "missing".to_string(),
            expected_missing_expected: 0,
            expected_failures: 0,
            expected_warnings: 0,
            expected_start_here_available: false,
            expected_gate_authority_present: false,
            expected_required_groups: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("report-packet-index corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| {
            let name = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
            let expected_report = json_string_field(case, "expected_report")
                .map(PathBuf::from)
                .unwrap_or_else(|| corpus_path.to_path_buf());
            let expected = case.get("expected").unwrap_or(&Value::Null);
            DogfoodReportPacketIndexScenario {
                name,
                scenario: json_string_field(case, "scenario")
                    .unwrap_or_else(|| "missing scenario".to_string()),
                expected_report,
                expected_markdown: json_string_field(case, "expected_markdown")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| corpus_path.to_path_buf()),
                expected_status: json_string_field(expected, "status")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_missing_expected: json_usize_field(expected, "missing_expected")
                    .unwrap_or(0),
                expected_failures: json_usize_field(expected, "failures").unwrap_or(0),
                expected_warnings: json_usize_field(expected, "warnings").unwrap_or(0),
                expected_start_here_available: json_bool_field(expected, "start_here_available")
                    .unwrap_or(false),
                expected_gate_authority_present: json_bool_field(
                    expected,
                    "gate_authority_present",
                )
                .unwrap_or(false),
                expected_required_groups: json_string_array_field(expected, "required_groups"),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "report-packet-index corpus case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn json_string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn sorted_unique_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

pub(crate) fn json_string_values_from_array(
    value: &Value,
    array_key: &str,
    field_key: &str,
) -> Vec<String> {
    let values = value
        .get(array_key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_string_field(item, field_key))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    sorted_unique_strings(values)
}

pub(crate) fn dogfood_report_packet_index_run(
    scenario: &DogfoodReportPacketIndexScenario,
) -> Result<DogfoodReportPacketIndexRun, String> {
    let actual_dir = scenario
        .expected_report
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let json_path = scenario.expected_report.clone();
    let markdown_path = scenario.expected_markdown.clone();
    let mut errors = Vec::new();

    if !scenario.expected_report.exists() {
        errors.push(format!(
            "expected report fixture is missing: {}",
            normalize_path(&scenario.expected_report)
        ));
    }
    if !scenario.expected_markdown.exists() {
        errors.push(format!(
            "expected Markdown fixture is missing: {}",
            normalize_path(&scenario.expected_markdown)
        ));
    }

    let mut status = "missing".to_string();
    let mut missing_expected = 0usize;
    let mut failures = 0usize;
    let mut warnings = 0usize;
    let mut start_here_available = false;
    let mut gate_authority_present = false;
    let mut groups = Vec::<String>::new();

    match read_json_value(&json_path) {
        Ok(report) => {
            if json_string_field(&report, "kind").as_deref() != Some("report_packet_index") {
                errors.push("report kind must be report_packet_index".to_string());
            }
            status = json_string_field(&report, "status").unwrap_or_else(|| "missing".to_string());
            if let Some(summary) = report.get("summary") {
                missing_expected = json_usize_field(summary, "missing_expected").unwrap_or(0);
                failures = json_usize_field(summary, "failures").unwrap_or(0);
                warnings = json_usize_field(summary, "warnings").unwrap_or(0);
                start_here_available = json_string_field(summary, "start_here")
                    .is_some_and(|value| !value.trim().is_empty());
                gate_authority_present = json_string_field(summary, "gate_authority")
                    .is_some_and(|value| !value.trim().is_empty());
            } else {
                errors.push("report summary is missing".to_string());
            }
            groups = report
                .get("groups")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| json_string_field(item, "group"))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let limits = report
                .get("limits")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            if !limits.contains(&"Advisory report-packet index only.") {
                errors.push("report is missing advisory report-packet index limit".to_string());
            }
            if !limits.contains(&"Gate decision remains pass/fail authority when configured.") {
                errors.push("report is missing gate-authority limit".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    match fs::read_to_string(&markdown_path) {
        Ok(markdown) => {
            if !markdown.contains("# RIPR Report Packet Index") {
                errors.push("Markdown must use the report-packet index heading".to_string());
            }
            if !markdown.contains(&format!("Status: {}", scenario.expected_status)) {
                errors.push(format!(
                    "Markdown should pin status {}",
                    scenario.expected_status
                ));
            }
        }
        Err(err) => errors.push(format!(
            "failed to read report-packet index Markdown {}: {err}",
            normalize_path(&markdown_path)
        )),
    }

    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if missing_expected != scenario.expected_missing_expected {
        errors.push(format!(
            "expected missing_expected {}, got {}",
            scenario.expected_missing_expected, missing_expected
        ));
    }
    if failures != scenario.expected_failures {
        errors.push(format!(
            "expected failures {}, got {}",
            scenario.expected_failures, failures
        ));
    }
    if warnings != scenario.expected_warnings {
        errors.push(format!(
            "expected warnings {}, got {}",
            scenario.expected_warnings, warnings
        ));
    }
    if start_here_available != scenario.expected_start_here_available {
        errors.push(format!(
            "expected start_here_available {}, got {}",
            scenario.expected_start_here_available, start_here_available
        ));
    }
    if gate_authority_present != scenario.expected_gate_authority_present {
        errors.push(format!(
            "expected gate_authority_present {}, got {}",
            scenario.expected_gate_authority_present, gate_authority_present
        ));
    }
    for expected_group in &scenario.expected_required_groups {
        if !groups.iter().any(|group| group == expected_group) {
            errors.push(format!("missing expected group `{expected_group}`"));
        }
    }

    Ok(DogfoodReportPacketIndexRun {
        name: scenario.name.clone(),
        actual_dir,
        json_path,
        markdown_path,
        status,
        missing_expected,
        failures,
        warnings,
        start_here_available,
        gate_authority_present,
        groups,
        expected_status: scenario.expected_status.clone(),
        expected_missing_expected: scenario.expected_missing_expected,
        expected_failures: scenario.expected_failures,
        expected_warnings: scenario.expected_warnings,
        expected_start_here_available: scenario.expected_start_here_available,
        expected_gate_authority_present: scenario.expected_gate_authority_present,
        expected_required_groups: scenario.expected_required_groups.clone(),
        reason: if scenario.scenario.trim().is_empty() {
            scenario.reason.clone()
        } else {
            format!("{}: {}", scenario.scenario, scenario.reason)
        },
        expected_report: scenario.expected_report.clone(),
        expected_markdown: scenario.expected_markdown.clone(),
        errors,
    })
}

pub(crate) const GENERATED_CI_FIRST_ACTION_REPAIR: &str = "Regenerate command: `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md`";
pub(crate) const GENERATED_CI_FIRST_PR_REPAIR: &str = "ripr first-pr --root . --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-packet.json --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports";
pub(crate) const GENERATED_CI_FRONT_PANEL_REPAIR: &str = "Regenerate command: `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md`";
pub(crate) const GENERATED_CI_PACKET_INDEX_REPAIR: &str = "Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`.";

pub(crate) fn dogfood_generated_ci_cockpit_run() -> Result<DogfoodGeneratedCiCockpitRun, String> {
    let args = [
        "run",
        "--quiet",
        "-p",
        "ripr",
        "--",
        "init",
        "--ci",
        "github",
        "--dry-run",
    ]
    .iter()
    .map(|value| (*value).to_string())
    .collect::<Vec<_>>();
    let command = format!("cargo {}", args.join(" "));
    let started = Instant::now();
    let workflow = run_output_owned("cargo", &args)?;
    Ok(dogfood_generated_ci_cockpit_run_from_workflow(
        "generated-pr-ci-review-workflow",
        &command,
        started.elapsed().as_millis(),
        &workflow,
    ))
}

pub(crate) fn dogfood_generated_ci_cockpit_run_from_workflow(
    name: &str,
    command: &str,
    duration_ms: u128,
    workflow: &str,
) -> DogfoodGeneratedCiCockpitRun {
    let start_here = workflow.contains("### Start here")
        && workflow.contains("Open `target/ripr/reports/start-here.md` first")
        && workflow.contains("name: Render RIPR first-pr start-here")
        && workflow.contains("cat target/ripr/reports/start-here.md");
    let repair_commands = [
        GENERATED_CI_FIRST_ACTION_REPAIR,
        GENERATED_CI_FIRST_PR_REPAIR,
        GENERATED_CI_FRONT_PANEL_REPAIR,
        GENERATED_CI_PACKET_INDEX_REPAIR,
    ]
    .iter()
    .filter(|command| workflow.contains(**command))
    .count();
    let expected_repair_commands = 4usize;
    let gate_authority_boundary =
        workflow.contains("ripr gate evaluate") && workflow.contains("Gate authority:");
    let default_advisory = workflow.contains(
        "continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}",
    ) && workflow.contains("RIPR is advisory static evidence");
    let artifact_upload =
        workflow.contains("actions/upload-artifact@v7") && workflow.contains("target/ripr/reports");
    let language_grouping_checked = workflow.contains("if [ -n \"$preview_languages\" ]; then")
        && workflow.contains("### Language preview grouping")
        && workflow.contains("Grouped preview evidence languages")
        && workflow.contains("grouped_preview_languages=\"$grouped_preview_languages javascript\"")
        && workflow.contains("preview-language groups are advisory presentation only")
        && workflow.contains("ripr gate evaluate")
        && workflow.contains("missing_preview_status")
        && workflow.contains("static_limit_kinds")
        && workflow.contains("actionability_states")
        && workflow.contains("actionability_categories")
        && workflow.contains("repair_packet_ready_entries")
        && workflow.contains("gate_impact=\\`none\\`");
    let language_grouping_status = if language_grouping_checked {
        "checked"
    } else {
        "missing"
    }
    .to_string();
    let mut errors = Vec::new();

    if !start_here {
        errors.push("generated CI summary must include Start here guidance".to_string());
    }
    if repair_commands != expected_repair_commands {
        errors.push(format!(
            "expected {expected_repair_commands} cockpit regeneration commands, got {repair_commands}"
        ));
    }
    if !gate_authority_boundary {
        errors.push("generated CI must keep gate-decision authority visible".to_string());
    }
    if !default_advisory {
        errors.push("generated CI must remain advisory by default".to_string());
    }
    if !artifact_upload {
        errors.push("generated CI must upload the report artifact packet".to_string());
    }
    if !language_grouping_checked {
        errors.push(
            "generated CI must keep configured preview-language grouping advisory and opt-in"
                .to_string(),
        );
    }

    DogfoodGeneratedCiCockpitRun {
        name: name.to_string(),
        command: command.to_string(),
        duration_ms,
        start_here,
        repair_commands,
        expected_repair_commands,
        gate_authority_boundary,
        default_advisory,
        artifact_upload,
        language_grouping_status,
        errors,
    }
}

pub(crate) fn dogfood_language_preview_scenarios() -> Vec<DogfoodLanguagePreviewScenario> {
    [
        (
            "typescript_mocked_module_limit",
            "typescript",
            1usize,
            1usize,
            0usize,
            1usize,
            vec!["exposed"],
            vec!["mocked_module"],
            true,
            "TypeScript preview finding keeps preview metadata and mocked-module static limit.",
        ),
        (
            "javascript_js_preview",
            "javascript",
            1usize,
            1usize,
            0usize,
            1usize,
            vec!["exposed"],
            Vec::new(),
            true,
            "JavaScript preview finding keeps separate JavaScript preview metadata through the TypeScript-family adapter.",
        ),
        (
            "python_missing_import_graph_limit",
            "python",
            1usize,
            1usize,
            0usize,
            1usize,
            vec!["exposed"],
            vec!["missing_import_graph"],
            true,
            "Python preview finding keeps preview metadata and missing-import-graph static limit.",
        ),
        (
            "python_mixed_language_no_cross_route",
            "python",
            1usize,
            1usize,
            0usize,
            0usize,
            vec!["no_static_path"],
            Vec::new(),
            true,
            "Mixed-language fixture must not use a TypeScript test as Python related-test evidence.",
        ),
        (
            "python_disabled",
            "python",
            0usize,
            0usize,
            0usize,
            0usize,
            Vec::new(),
            Vec::new(),
            false,
            "Rust-default language config must not emit disabled Python preview findings.",
        ),
    ]
    .into_iter()
    .map(
        |(
            name,
            language,
            expected_findings,
            expected_preview_findings,
            expected_missing_preview_status,
            expected_related_tests,
            expected_classifications,
            expected_static_limit_kinds,
            preview_enabled,
            reason,
        )| {
            let base = Path::new("fixtures").join(name);
            DogfoodLanguagePreviewScenario {
                name: name.to_string(),
                language: language.to_string(),
                root: base.join("input"),
                diff: base.join("diff.patch"),
                expected_findings,
                expected_preview_findings,
                expected_missing_preview_status,
                expected_related_tests,
                expected_classifications: expected_classifications
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                expected_static_limit_kinds: expected_static_limit_kinds
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                preview_enabled,
                reason: reason.to_string(),
            }
        },
    )
    .collect()
}

pub(crate) fn dogfood_language_preview_run(
    scenario: &DogfoodLanguagePreviewScenario,
) -> DogfoodLanguagePreviewRun {
    let started = Instant::now();
    let actual_dir = Path::new("target")
        .join("ripr")
        .join("dogfood")
        .join("language-preview")
        .join(&scenario.name);
    let json_path = actual_dir.join("check.json");
    let human_path = actual_dir.join("human.txt");
    let mut errors = Vec::new();
    let mut findings = 0usize;
    let mut language_findings = 0usize;
    let mut preview_findings = 0usize;
    let mut missing_preview_status = 0usize;
    let mut related_tests = 0usize;
    let mut classifications = Vec::<String>::new();
    let mut static_limit_kinds = Vec::<String>::new();

    if !scenario.root.exists() {
        errors.push(format!(
            "fixture root does not exist: {}",
            normalize_path(&scenario.root)
        ));
    }
    if !scenario.diff.exists() {
        errors.push(format!(
            "fixture diff does not exist: {}",
            normalize_path(&scenario.diff)
        ));
    }
    if let Err(err) = fs::create_dir_all(&actual_dir) {
        errors.push(format!(
            "failed to create language preview dogfood output directory {}: {err}",
            normalize_path(&actual_dir)
        ));
    }

    let mut human_output = String::new();
    if errors.is_empty() {
        let root = normalize_path(&scenario.root);
        let diff = normalize_path(&scenario.diff);
        match run_fixture_check(&root, &diff, FixtureCheckFormat::Json, None) {
            Ok(json) => {
                let normalized = normalize_fixture_json_output(&json);
                if let Err(err) = fs::write(&json_path, &normalized) {
                    errors.push(format!(
                        "failed to write language preview dogfood JSON {}: {err}",
                        normalize_path(&json_path)
                    ));
                }
                match serde_json::from_str::<Value>(&normalized) {
                    Ok(report) => {
                        findings = json_summary_count(&report, "findings");
                        if let Some(finding_values) =
                            report.get("findings").and_then(Value::as_array)
                        {
                            for finding in finding_values {
                                if json_string_field(finding, "language").as_deref()
                                    != Some(scenario.language.as_str())
                                {
                                    continue;
                                }
                                language_findings += 1;
                                if json_string_field(finding, "language_status").as_deref()
                                    == Some("preview")
                                {
                                    preview_findings += 1;
                                } else {
                                    missing_preview_status += 1;
                                }
                                if let Some(classification) =
                                    json_string_field(finding, "classification")
                                {
                                    classifications.push(classification);
                                }
                                if let Some(static_limit_kind) =
                                    json_string_field(finding, "static_limit_kind")
                                {
                                    static_limit_kinds.push(static_limit_kind);
                                }
                                related_tests += finding
                                    .get("related_tests")
                                    .and_then(Value::as_array)
                                    .map(Vec::len)
                                    .unwrap_or(0);
                            }
                        } else {
                            errors.push(
                                "language preview JSON is missing findings array".to_string(),
                            );
                        }
                    }
                    Err(err) => errors.push(format!(
                        "failed to parse language preview JSON for {}: {err}",
                        scenario.name
                    )),
                }
            }
            Err(err) => errors.push(err),
        }

        match run_fixture_check(&root, &diff, FixtureCheckFormat::Human, None) {
            Ok(human) => {
                human_output = normalize_fixture_human_output(&human);
                if let Err(err) = fs::write(&human_path, &human_output) {
                    errors.push(format!(
                        "failed to write language preview dogfood human output {}: {err}",
                        normalize_path(&human_path)
                    ));
                }
            }
            Err(err) => errors.push(err),
        }
    }

    classifications = sorted_unique_strings(classifications);
    static_limit_kinds = sorted_unique_strings(static_limit_kinds);

    if findings != scenario.expected_findings {
        errors.push(format!(
            "expected {} total finding(s), got {}",
            scenario.expected_findings, findings
        ));
    }
    if preview_findings != scenario.expected_preview_findings {
        errors.push(format!(
            "expected {} preview finding(s), got {}",
            scenario.expected_preview_findings, preview_findings
        ));
    }
    if missing_preview_status != scenario.expected_missing_preview_status {
        errors.push(format!(
            "expected {} finding(s) missing preview status, got {}",
            scenario.expected_missing_preview_status, missing_preview_status
        ));
    }
    if related_tests != scenario.expected_related_tests {
        errors.push(format!(
            "expected {} related test(s), got {}",
            scenario.expected_related_tests, related_tests
        ));
    }
    if classifications != scenario.expected_classifications {
        errors.push(format!(
            "expected classifications [{}], got [{}]",
            scenario.expected_classifications.join(", "),
            classifications.join(", ")
        ));
    }
    if static_limit_kinds != scenario.expected_static_limit_kinds {
        errors.push(format!(
            "expected static limit kinds [{}], got [{}]",
            scenario.expected_static_limit_kinds.join(", "),
            static_limit_kinds.join(", ")
        ));
    }
    if !scenario.preview_enabled && (language_findings > 0 || preview_findings > 0) {
        errors.push(format!(
            "{} preview should be disabled but emitted {} language finding(s)",
            scenario.language, language_findings
        ));
    }
    if scenario.preview_enabled
        && scenario.expected_preview_findings > 0
        && !human_output
            .to_ascii_lowercase()
            .contains(&format!("{} preview", scenario.language))
    {
        errors.push(format!(
            "human output should label {} preview evidence",
            scenario.language
        ));
    }
    for static_limit_kind in &scenario.expected_static_limit_kinds {
        if !human_output.contains(static_limit_kind) {
            errors.push(format!(
                "human output should include static limit kind `{static_limit_kind}`"
            ));
        }
    }

    DogfoodLanguagePreviewRun {
        name: scenario.name.clone(),
        language: scenario.language.clone(),
        root: scenario.root.clone(),
        diff: scenario.diff.clone(),
        actual_dir,
        json_path,
        human_path,
        duration_ms: started.elapsed().as_millis(),
        findings,
        language_findings,
        preview_findings,
        missing_preview_status,
        related_tests,
        classifications,
        static_limit_kinds,
        expected_findings: scenario.expected_findings,
        expected_preview_findings: scenario.expected_preview_findings,
        expected_missing_preview_status: scenario.expected_missing_preview_status,
        expected_related_tests: scenario.expected_related_tests,
        expected_classifications: scenario.expected_classifications.clone(),
        expected_static_limit_kinds: scenario.expected_static_limit_kinds.clone(),
        preview_enabled: scenario.preview_enabled,
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_editor_gap_cockpit_scenarios() -> Vec<DogfoodEditorGapCockpitScenario> {
    let scenario = |name: &str,
                    state: &str,
                    language: Option<&str>,
                    language_status: Option<&str>,
                    diagnostics: usize,
                    fail_closed: bool,
                    actions: Vec<&str>,
                    static_limit_kind: Option<&str>,
                    reason: &str| {
        DogfoodEditorGapCockpitScenario {
            name: name.to_string(),
            expected_state: state.to_string(),
            expected_language: language.map(str::to_string),
            expected_language_status: language_status.map(str::to_string),
            expected_diagnostics: diagnostics,
            expected_fail_closed: fail_closed,
            expected_actions: actions.into_iter().map(str::to_string).collect(),
            expected_static_limit_kind: static_limit_kind.map(str::to_string),
            reason: reason.to_string(),
        }
    };

    vec![
        scenario(
            "rust_actionable",
            "actionable",
            Some("rust"),
            Some("stable"),
            1,
            false,
            vec![
                "copy_repair_packet",
                "open_related_test",
                "copy_verify_command",
                "copy_receipt_command",
                "refresh",
            ],
            None,
            "Rust stable gap projects a related test, repair packet, verify command, and receipt command.",
        ),
        scenario(
            "typescript_preview_static_limit",
            "actionable",
            Some("typescript"),
            Some("preview"),
            1,
            false,
            vec!["copy_repair_packet", "copy_static_limit_note", "refresh"],
            Some("mocked_module"),
            "TypeScript preview gap keeps static-limit evidence visible before action language.",
        ),
        scenario(
            "python_preview_static_limit",
            "actionable",
            Some("python"),
            Some("preview"),
            1,
            false,
            vec!["copy_repair_packet", "copy_static_limit_note", "refresh"],
            Some("missing_import_graph"),
            "Python preview gap keeps missing-import-graph limits visible before action language.",
        ),
        scenario(
            "disabled_language",
            "disabled_language",
            Some("python"),
            Some("preview"),
            0,
            true,
            vec!["refresh"],
            None,
            "Disabled preview language produces status and refresh only, not diagnostics or repair packets.",
        ),
        scenario(
            "wrong_root",
            "wrong_root",
            None,
            None,
            0,
            true,
            vec!["refresh"],
            None,
            "Wrong-root artifacts fail closed and leave refresh as the only safe action.",
        ),
        scenario(
            "stale_artifact",
            "stale_artifact",
            None,
            None,
            0,
            true,
            vec!["refresh"],
            None,
            "Stale artifacts suppress diagnostics and repair actions until refreshed.",
        ),
        scenario(
            "no_actionable_gap",
            "no_actionable_gap",
            Some("rust"),
            Some("stable"),
            0,
            true,
            vec!["refresh"],
            None,
            "No-action state explains that no local repair packet should be projected.",
        ),
    ]
}

pub(crate) fn dogfood_editor_gap_cockpit_run(
    scenario: &DogfoodEditorGapCockpitScenario,
) -> DogfoodEditorGapCockpitRun {
    let expected_dir = Path::new("fixtures")
        .join("editor_gap_cockpit")
        .join(&scenario.name)
        .join("expected");
    let projection_path = expected_dir.join("gap-projection.json");
    let diagnostics_path = expected_dir.join("lsp-diagnostics.json");
    let hover_path = expected_dir.join("lsp-hover.md");
    let code_actions_path = expected_dir.join("lsp-code-actions.json");
    let status_path = expected_dir.join("vscode-status.json");
    let mut errors = Vec::new();

    for (label, path) in [
        ("gap projection", &projection_path),
        ("diagnostics", &diagnostics_path),
        ("hover", &hover_path),
        ("code actions", &code_actions_path),
        ("VS Code status", &status_path),
    ] {
        if !path.exists() {
            errors.push(format!(
                "{label} fixture is missing: {}",
                normalize_path(path)
            ));
        }
    }

    let mut state = "missing".to_string();
    let mut language = None;
    let mut language_status = None;
    let mut diagnostics_projected = 0usize;
    let mut fail_closed = false;
    let mut actions_projected = Vec::<String>::new();
    let mut static_limit_kind = None;

    match read_json_value(&projection_path) {
        Ok(projection) => {
            if json_string_field(&projection, "schema_version").as_deref() != Some("0.1") {
                errors.push("gap projection schema_version must be 0.1".to_string());
            }
            if json_string_field(&projection, "case").as_deref() != Some(scenario.name.as_str()) {
                errors.push(format!("gap projection case should be {}", scenario.name));
            }
            state =
                json_string_field(&projection, "state").unwrap_or_else(|| "missing".to_string());
            language = json_string_field(&projection, "language");
            language_status = json_string_field(&projection, "language_status");
            diagnostics_projected =
                json_usize_field(&projection, "diagnostics_projected").unwrap_or(0);
            fail_closed = json_bool_field(&projection, "fail_closed").unwrap_or(false);
            actions_projected = json_string_array_field(&projection, "actions_projected");
            static_limit_kind = json_string_field(&projection, "static_limit_kind");
        }
        Err(err) => errors.push(err),
    }

    let mut actual_diagnostics = 0usize;
    match read_json_value(&diagnostics_path) {
        Ok(diagnostics) => {
            actual_diagnostics = diagnostics
                .get("diagnostics")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let expected_fixture = format!("editor_gap_cockpit/{}", scenario.name);
            if json_string_field(&diagnostics, "fixture").as_deref()
                != Some(expected_fixture.as_str())
            {
                errors.push("diagnostics fixture name does not match scenario".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    let mut actual_actions = 0usize;
    match read_json_value(&code_actions_path) {
        Ok(actions) => {
            let action_items = actions.get("actions").and_then(Value::as_array);
            actual_actions = action_items.map_or(0, Vec::len);
            if scenario
                .expected_actions
                .iter()
                .any(|action| action == "refresh")
            {
                let has_refresh = action_items.is_some_and(|items| {
                    items.iter().any(|item| {
                        json_string_field(item, "command").as_deref() == Some("ripr.refresh")
                    })
                });
                if !has_refresh {
                    errors.push("code actions should include refresh".to_string());
                }
            }
            if scenario
                .expected_actions
                .iter()
                .any(|action| action == "open_related_test")
            {
                let has_open_related_test = action_items.is_some_and(|items| {
                    items.iter().any(|item| {
                        json_string_field(item, "command").as_deref()
                            == Some("ripr.openRelatedTest")
                    })
                });
                if !has_open_related_test {
                    errors.push("code actions should include related-test opening".to_string());
                }
            }
        }
        Err(err) => errors.push(err),
    }

    let mut hover_static_before_action = false;
    match fs::read_to_string(&hover_path) {
        Ok(hover) => {
            if let Some(kind) = &scenario.expected_static_limit_kind {
                let static_needle = format!("Static limit: {kind}");
                match (hover.find(&static_needle), hover.find("Suggested action")) {
                    (Some(static_index), Some(action_index)) if static_index < action_index => {
                        hover_static_before_action = true;
                    }
                    _ => errors.push(format!(
                        "hover should show static limit `{kind}` before suggested action"
                    )),
                }
            }
            if !hover
                .contains("no source edits, generated tests, provider calls, or mutation execution")
            {
                errors.push("hover should preserve projection-only editor limits".to_string());
            }
            if scenario.expected_language_status.as_deref() == Some("preview")
                && !hover.to_ascii_lowercase().contains("preview")
            {
                errors.push("preview hover should label preview evidence".to_string());
            }
        }
        Err(err) => errors.push(format!(
            "failed to read editor gap cockpit hover {}: {err}",
            normalize_path(&hover_path)
        )),
    }

    match read_json_value(&status_path) {
        Ok(status) => {
            if json_string_field(&status, "schema_version").as_deref() != Some("0.1") {
                errors.push("VS Code status schema_version must be 0.1".to_string());
            }
            let states = status.get("states").and_then(Value::as_array);
            if states.is_none_or(|items| items.is_empty()) {
                errors.push("VS Code status should contain at least one state".to_string());
            }
            if let Some(first) = states.and_then(|items| items.first()) {
                if json_string_field(first, "status_bar")
                    .is_none_or(|status_bar| status_bar.trim().is_empty())
                {
                    errors.push("VS Code status should include status_bar copy".to_string());
                }
                if first
                    .get("show_status_contains")
                    .and_then(Value::as_array)
                    .is_none_or(|items| items.is_empty())
                {
                    errors
                        .push("VS Code status should include Show Status expectations".to_string());
                }
            }
        }
        Err(err) => errors.push(err),
    }

    if state != scenario.expected_state {
        errors.push(format!(
            "expected state {}, got {}",
            scenario.expected_state, state
        ));
    }
    if language != scenario.expected_language {
        errors.push(format!(
            "expected language {:?}, got {:?}",
            scenario.expected_language, language
        ));
    }
    if language_status != scenario.expected_language_status {
        errors.push(format!(
            "expected language_status {:?}, got {:?}",
            scenario.expected_language_status, language_status
        ));
    }
    if diagnostics_projected != scenario.expected_diagnostics {
        errors.push(format!(
            "expected {} projected diagnostic(s), got {}",
            scenario.expected_diagnostics, diagnostics_projected
        ));
    }
    if actual_diagnostics != diagnostics_projected {
        errors.push(format!(
            "diagnostics file has {} diagnostic(s), projection says {}",
            actual_diagnostics, diagnostics_projected
        ));
    }
    if fail_closed != scenario.expected_fail_closed {
        errors.push(format!(
            "expected fail_closed {}, got {}",
            scenario.expected_fail_closed, fail_closed
        ));
    }
    if actions_projected != scenario.expected_actions {
        errors.push(format!(
            "expected actions {:?}, got {:?}",
            scenario.expected_actions, actions_projected
        ));
    }
    if actual_actions != actions_projected.len() {
        errors.push(format!(
            "code action file has {} action(s), projection says {}",
            actual_actions,
            actions_projected.len()
        ));
    }
    if static_limit_kind != scenario.expected_static_limit_kind {
        errors.push(format!(
            "expected static_limit_kind {:?}, got {:?}",
            scenario.expected_static_limit_kind, static_limit_kind
        ));
    }
    if fail_closed
        && (actions_projected.len() != 1
            || actions_projected.first().map(String::as_str) != Some("refresh"))
    {
        errors.push("fail-closed cases should project refresh only".to_string());
    }
    if fail_closed && diagnostics_projected > 0 {
        errors.push("fail-closed cases should not project diagnostics".to_string());
    }

    DogfoodEditorGapCockpitRun {
        name: scenario.name.clone(),
        expected_dir,
        projection_path,
        diagnostics_path,
        hover_path,
        code_actions_path,
        status_path,
        state,
        language,
        language_status,
        diagnostics_projected,
        actual_diagnostics,
        fail_closed,
        actions_projected,
        actual_actions,
        static_limit_kind,
        hover_static_before_action,
        expected_state: scenario.expected_state.clone(),
        expected_language: scenario.expected_language.clone(),
        expected_language_status: scenario.expected_language_status.clone(),
        expected_diagnostics: scenario.expected_diagnostics,
        expected_fail_closed: scenario.expected_fail_closed,
        expected_actions: scenario.expected_actions.clone(),
        expected_static_limit_kind: scenario.expected_static_limit_kind.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_editor_first_pr_bridge_scenarios() -> Vec<DogfoodEditorFirstPrBridgeScenario>
{
    let all_repair_actions = vec![
        "open_packet",
        "copy_summary",
        "copy_repair_packet",
        "copy_verify_command",
        "copy_receipt_command",
        "copy_regeneration_guidance",
    ];
    let summary_only_actions = vec!["open_packet", "copy_summary", "copy_regeneration_guidance"];
    let guidance_only_actions = vec!["copy_regeneration_guidance"];
    let repair_scoped_actions = vec![
        "copy_repair_packet",
        "copy_verify_command",
        "copy_receipt_command",
    ];
    let unsafe_packet_suppressed_actions = vec![
        "open_packet",
        "copy_summary",
        "copy_repair_packet",
        "copy_verify_command",
        "copy_receipt_command",
    ];
    let scenario = |name: &str,
                    packet_state: &str,
                    safe_actions: Vec<&str>,
                    suppressed_actions: Vec<&str>,
                    diagnostics: usize,
                    fail_closed: bool,
                    receipt_movement: Option<&str>,
                    reason: &str| {
        DogfoodEditorFirstPrBridgeScenario {
            name: name.to_string(),
            expected_packet_state: packet_state.to_string(),
            expected_safe_actions: safe_actions.into_iter().map(str::to_string).collect(),
            expected_suppressed_actions: suppressed_actions
                .into_iter()
                .map(str::to_string)
                .collect(),
            expected_diagnostics: diagnostics,
            expected_fail_closed: fail_closed,
            expected_receipt_movement: receipt_movement.map(str::to_string),
            reason: reason.to_string(),
        }
    };

    vec![
        scenario(
            "setup_ok",
            "found",
            summary_only_actions.clone(),
            repair_scoped_actions.clone(),
            0,
            false,
            None,
            "Setup diagnosis finds a packet and exposes summary/open actions without diagnostic-scoped repair commands.",
        ),
        scenario(
            "packet_missing",
            "missing",
            guidance_only_actions.clone(),
            unsafe_packet_suppressed_actions.clone(),
            0,
            true,
            None,
            "Missing first-pr packet fails closed and leaves regeneration guidance plus refresh.",
        ),
        scenario(
            "packet_found_repairable",
            "top_repairable_gap",
            all_repair_actions.clone(),
            vec![],
            1,
            false,
            None,
            "Repairable first-pr packet exposes bounded open, summary, repair, verify, receipt, and regeneration actions.",
        ),
        scenario(
            "packet_no_action",
            "no_action",
            summary_only_actions.clone(),
            repair_scoped_actions.clone(),
            0,
            false,
            None,
            "No-action packet remains inspectable without diagnostic-scoped repair commands.",
        ),
        scenario(
            "packet_stale",
            "stale",
            guidance_only_actions.clone(),
            unsafe_packet_suppressed_actions.clone(),
            0,
            true,
            None,
            "Stale packet fails closed and requires regeneration before repair actions.",
        ),
        scenario(
            "packet_wrong_root",
            "wrong_root",
            guidance_only_actions.clone(),
            unsafe_packet_suppressed_actions.clone(),
            0,
            true,
            None,
            "Wrong-root packet fails closed and suppresses open/copy repair actions.",
        ),
        scenario(
            "packet_malformed",
            "malformed",
            guidance_only_actions.clone(),
            unsafe_packet_suppressed_actions.clone(),
            0,
            true,
            None,
            "Malformed packet fails closed and suppresses packet-derived actions.",
        ),
        scenario(
            "receipt_improved_packet_ready",
            "top_repairable_gap",
            all_repair_actions.clone(),
            vec![],
            1,
            false,
            Some("improved"),
            "Improved receipt is visible alongside the packet without claiming PR readiness.",
        ),
        scenario(
            "receipt_unchanged_packet_ready",
            "top_repairable_gap",
            all_repair_actions,
            vec![],
            1,
            false,
            Some("unchanged"),
            "Unchanged receipt stays visible and advisory alongside the first-pr packet.",
        ),
    ]
}

pub(crate) fn dogfood_editor_first_pr_bridge_run(
    scenario: &DogfoodEditorFirstPrBridgeScenario,
) -> DogfoodEditorFirstPrBridgeRun {
    let expected_dir = Path::new("fixtures")
        .join("editor_first_pr_bridge")
        .join(&scenario.name)
        .join("expected");
    let packet_path = expected_dir.join("first-pr-status.json");
    let diagnostics_path = expected_dir.join("lsp-diagnostics.json");
    let code_actions_path = expected_dir.join("lsp-code-actions.json");
    let status_path = expected_dir.join("vscode-status.json");
    let diagnosis_path = expected_dir.join("setup-diagnosis.md");
    let mut errors = Vec::new();

    for (label, path) in [
        ("first-pr status", &packet_path),
        ("diagnostics", &diagnostics_path),
        ("code actions", &code_actions_path),
        ("VS Code status", &status_path),
        ("setup diagnosis", &diagnosis_path),
    ] {
        if !path.exists() {
            errors.push(format!(
                "{label} fixture is missing: {}",
                normalize_path(path)
            ));
        }
    }

    let mut packet_state = "missing".to_string();
    let mut safe_actions = Vec::new();
    let mut suppressed_actions = Vec::new();
    let mut receipt_movement = None;
    let mut runtime_adequacy_claim = true;
    let mut mutation_proof_claim = true;
    let mut policy_gate_claim = true;
    let mut pr_ready_claim = true;

    match read_json_value(&packet_path) {
        Ok(packet) => {
            let expected_fixture = format!("editor_first_pr_bridge/{}", scenario.name);
            if json_string_field(&packet, "schema_version").as_deref() != Some("0.1") {
                errors.push("first-pr status schema_version must be 0.1".to_string());
            }
            if json_string_field(&packet, "fixture").as_deref() != Some(expected_fixture.as_str()) {
                errors.push("first-pr status fixture name does not match scenario".to_string());
            }
            packet_state =
                json_string_field(&packet, "packet_state").unwrap_or_else(|| "missing".to_string());
            if editor_first_pr_bridge_case_requires_first_screen_contract(&scenario.name) {
                validate_editor_first_pr_bridge_first_screen_contract(
                    &scenario.name,
                    &packet,
                    &mut errors,
                );
            }
            safe_actions = json_string_array_field(&packet, "safe_actions");
            suppressed_actions = json_string_array_field(&packet, "suppressed_actions");
            receipt_movement = json_string_field(&packet, "receipt_movement");
            runtime_adequacy_claim =
                json_bool_field(&packet, "runtime_adequacy_claim").unwrap_or(true);
            mutation_proof_claim = json_bool_field(&packet, "mutation_proof_claim").unwrap_or(true);
            policy_gate_claim = json_bool_field(&packet, "policy_gate_claim").unwrap_or(true);
            pr_ready_claim = json_bool_field(&packet, "pr_ready_claim").unwrap_or(true);
        }
        Err(err) => errors.push(err),
    }

    let mut diagnostics = 0usize;
    match read_json_value(&diagnostics_path) {
        Ok(value) => {
            let expected_fixture = format!("editor_first_pr_bridge/{}", scenario.name);
            if json_string_field(&value, "fixture").as_deref() != Some(expected_fixture.as_str()) {
                errors.push("diagnostics fixture name does not match scenario".to_string());
            }
            diagnostics = value
                .get("diagnostics")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
        }
        Err(err) => errors.push(err),
    }

    let mut action_commands = Vec::new();
    let mut first_pr_actions = Vec::new();
    match read_json_value(&code_actions_path) {
        Ok(value) => {
            let expected_fixture = format!("editor_first_pr_bridge/{}", scenario.name);
            if json_string_field(&value, "fixture").as_deref() != Some(expected_fixture.as_str()) {
                errors.push("code actions fixture name does not match scenario".to_string());
            }
            for item in value
                .get("actions")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                if let Some(command) = json_string_field(item, "command") {
                    if let Some(action) = first_pr_bridge_action_from_command(&command) {
                        first_pr_actions.push(action.to_string());
                    }
                    action_commands.push(command);
                }
            }
        }
        Err(err) => errors.push(err),
    }

    let mut fail_closed = false;
    match read_json_value(&status_path) {
        Ok(value) => {
            let expected_fixture = format!("editor_first_pr_bridge/{}", scenario.name);
            if json_string_field(&value, "schema_version").as_deref() != Some("0.1") {
                errors.push("VS Code status schema_version must be 0.1".to_string());
            }
            if json_string_field(&value, "fixture").as_deref() != Some(expected_fixture.as_str()) {
                errors.push("VS Code status fixture name does not match scenario".to_string());
            }
            fail_closed = json_string_field(&value, "projection").as_deref() == Some("fail_closed");
            if json_string_field(&value, "next_safe_action").is_none() {
                errors.push("VS Code status should name a next_safe_action".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    match fs::read_to_string(&diagnosis_path) {
        Ok(diagnosis) => {
            for required in [
                "RIPR setup diagnosis",
                "First PR packet",
                "Next safe action",
                "Limits",
                "no source edits",
            ] {
                if !diagnosis.contains(required) {
                    errors.push(format!("setup diagnosis is missing `{required}`"));
                }
            }
        }
        Err(err) => errors.push(format!(
            "failed to read first-pr bridge setup diagnosis {}: {err}",
            normalize_path(&diagnosis_path)
        )),
    }

    if packet_state != scenario.expected_packet_state {
        errors.push(format!(
            "expected packet_state {}, got {}",
            scenario.expected_packet_state, packet_state
        ));
    }
    if safe_actions != scenario.expected_safe_actions {
        errors.push(format!(
            "expected safe_actions {:?}, got {:?}",
            scenario.expected_safe_actions, safe_actions
        ));
    }
    if suppressed_actions != scenario.expected_suppressed_actions {
        errors.push(format!(
            "expected suppressed_actions {:?}, got {:?}",
            scenario.expected_suppressed_actions, suppressed_actions
        ));
    }
    if diagnostics != scenario.expected_diagnostics {
        errors.push(format!(
            "expected {} diagnostic(s), got {}",
            scenario.expected_diagnostics, diagnostics
        ));
    }
    if fail_closed != scenario.expected_fail_closed {
        errors.push(format!(
            "expected fail_closed {}, got {}",
            scenario.expected_fail_closed, fail_closed
        ));
    }
    if receipt_movement != scenario.expected_receipt_movement {
        errors.push(format!(
            "expected receipt_movement {:?}, got {:?}",
            scenario.expected_receipt_movement, receipt_movement
        ));
    }
    if first_pr_actions != safe_actions {
        errors.push(format!(
            "first-pr action commands {:?} should match safe_actions {:?}",
            first_pr_actions, safe_actions
        ));
    }
    if fail_closed
        && safe_actions
            .iter()
            .any(|action| action != "copy_regeneration_guidance")
    {
        errors.push(
            "fail-closed first-pr bridge cases should expose regeneration guidance only"
                .to_string(),
        );
    }
    if runtime_adequacy_claim || mutation_proof_claim || policy_gate_claim || pr_ready_claim {
        errors.push(
            "first-pr bridge status must deny runtime, mutation, policy, and PR-ready claims"
                .to_string(),
        );
    }

    DogfoodEditorFirstPrBridgeRun {
        name: scenario.name.clone(),
        expected_dir,
        packet_path,
        diagnostics_path,
        code_actions_path,
        status_path,
        diagnosis_path,
        packet_state,
        safe_actions,
        suppressed_actions,
        receipt_movement,
        diagnostics,
        action_commands,
        first_pr_actions,
        fail_closed,
        expected_packet_state: scenario.expected_packet_state.clone(),
        expected_safe_actions: scenario.expected_safe_actions.clone(),
        expected_suppressed_actions: scenario.expected_suppressed_actions.clone(),
        expected_diagnostics: scenario.expected_diagnostics,
        expected_fail_closed: scenario.expected_fail_closed,
        expected_receipt_movement: scenario.expected_receipt_movement.clone(),
        runtime_adequacy_claim,
        mutation_proof_claim,
        policy_gate_claim,
        pr_ready_claim,
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn first_pr_bridge_action_from_command(command: &str) -> Option<&'static str> {
    match command {
        "ripr.openFirstPrPacket" => Some("open_packet"),
        "ripr.copyFirstPrSummary" => Some("copy_summary"),
        "ripr.copyFirstPrRepairPacket" => Some("copy_repair_packet"),
        "ripr.copyFirstPrVerifyCommand" => Some("copy_verify_command"),
        "ripr.copyFirstPrReceiptCommand" => Some("copy_receipt_command"),
        "ripr.copyFirstPrRegenerationGuidance" => Some("copy_regeneration_guidance"),
        _ => None,
    }
}

pub(crate) fn dogfood_finding_alignment_scenarios() -> Vec<DogfoodFindingAlignmentScenario> {
    let corpus_path = Path::new("fixtures/finding-alignment-dogfood/corpus.json");
    let fallback = |reason: String| {
        vec![DogfoodFindingAlignmentScenario {
            name: "corpus".to_string(),
            source_pr: "unknown".to_string(),
            canonical_gap_id: "unknown".to_string(),
            evidence_class: "unknown".to_string(),
            raw_findings_total: 0,
            canonical_items_total: 0,
            raw_finding_summary: reason.clone(),
            gap_state: "missing".to_string(),
            actionability: "missing".to_string(),
            user_outcome: "missing".to_string(),
            repair_kind: "unknown".to_string(),
            target_test_type: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            static_limitation_category: None,
            static_limitation_repair_route: None,
            raw_findings_supporting_only: false,
            recommended_repair: reason.clone(),
            before_after_context: reason.clone(),
            must_not_claim: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback("finding alignment dogfood corpus schema_version must be 0.1".to_string());
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("finding_alignment_dogfood_corpus") {
        return fallback(
            "finding alignment dogfood corpus kind must be finding_alignment_dogfood_corpus"
                .to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("finding alignment dogfood corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| DogfoodFindingAlignmentScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            source_pr: json_string_field(case, "source_pr")
                .unwrap_or_else(|| "unknown".to_string()),
            canonical_gap_id: json_string_field(case, "canonical_gap_id")
                .unwrap_or_else(|| "unknown".to_string()),
            evidence_class: json_string_field(case, "evidence_class")
                .unwrap_or_else(|| "unknown".to_string()),
            raw_findings_total: json_usize_field(case, "raw_findings_total").unwrap_or(0),
            canonical_items_total: json_usize_field(case, "canonical_items_total").unwrap_or(0),
            raw_finding_summary: json_string_field(case, "raw_finding_summary")
                .unwrap_or_else(|| "missing raw finding summary".to_string()),
            gap_state: json_string_field(case, "gap_state")
                .unwrap_or_else(|| "unknown".to_string()),
            actionability: json_string_field(case, "actionability")
                .unwrap_or_else(|| "unknown".to_string()),
            user_outcome: json_string_field(case, "user_outcome")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_kind: json_string_field(case, "repair_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            target_test_type: json_string_field(case, "target_test_type")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            static_limitation_category: json_string_field(case, "static_limitation_category"),
            static_limitation_repair_route: json_string_field(
                case,
                "static_limitation_repair_route",
            ),
            raw_findings_supporting_only: json_bool_field(case, "raw_findings_supporting_only")
                .unwrap_or(false),
            recommended_repair: json_string_field(case, "recommended_repair")
                .unwrap_or_else(|| "missing recommended repair".to_string()),
            before_after_context: json_string_field(case, "before_after_context")
                .unwrap_or_else(|| "missing before/after context".to_string()),
            must_not_claim: json_string_array_field(case, "must_not_claim"),
            reason: json_string_field(case, "reason")
                .unwrap_or_else(|| "missing finding-alignment dogfood reason".to_string()),
        })
        .collect()
}

pub(crate) fn dogfood_finding_alignment_run(
    scenario: &DogfoodFindingAlignmentScenario,
) -> DogfoodFindingAlignmentRun {
    let mut errors = Vec::new();

    if scenario.name.trim().is_empty() || scenario.name == "unknown" {
        errors.push("case id must be present".to_string());
    }
    if !scenario.source_pr.starts_with("EffortlessMetrics/ripr#")
        && !scenario
            .source_pr
            .starts_with("EffortlessMetrics/ripr-swarm#")
    {
        errors.push(format!(
            "source_pr should name a real RIPR PR, got {}",
            scenario.source_pr
        ));
    }
    if scenario.canonical_gap_id.trim().is_empty() || scenario.canonical_gap_id == "unknown" {
        errors.push("canonical_gap_id must be present".to_string());
    }
    if !matches!(
        scenario.evidence_class.as_str(),
        "presentation_text" | "config_or_policy_constant" | "predicate_boundary" | "call_presence"
    ) {
        errors.push(format!(
            "unsupported evidence class for dogfood receipt: {}",
            scenario.evidence_class
        ));
    }
    if scenario.raw_findings_total < scenario.canonical_items_total {
        errors.push(format!(
            "raw findings {} must be >= canonical items {}",
            scenario.raw_findings_total, scenario.canonical_items_total
        ));
    }
    if scenario.canonical_items_total == 0 {
        errors.push("canonical_items_total must be non-zero".to_string());
    }
    if !scenario.raw_findings_supporting_only {
        errors.push("raw findings must be marked supporting-only".to_string());
    }
    if scenario.raw_finding_summary.trim().is_empty()
        || scenario.raw_finding_summary == "missing raw finding summary"
    {
        errors.push("raw_finding_summary must be present".to_string());
    }
    if scenario.recommended_repair.trim().is_empty()
        || scenario.recommended_repair == "missing recommended repair"
    {
        errors.push("recommended_repair must be present".to_string());
    }
    if scenario.before_after_context.trim().is_empty()
        || scenario.before_after_context == "missing before/after context"
    {
        errors.push("before_after_context must be present".to_string());
    }
    if scenario
        .recommended_repair
        .to_ascii_lowercase()
        .contains("mutation")
    {
        errors
            .push("finding alignment dogfood must not route first to mutation testing".to_string());
    }
    if scenario.must_not_claim.is_empty() {
        errors.push("must_not_claim guard list must not be empty".to_string());
    }
    if !scenario.must_not_claim.iter().any(|claim| {
        let claim = claim.to_ascii_lowercase();
        claim.contains("raw") || claim.contains("test debt") || claim.contains("mutation")
    }) {
        errors.push(
            "must_not_claim guards should preserve raw-signal, test-debt, or mutation boundaries"
                .to_string(),
        );
    }

    match scenario.gap_state.as_str() {
        "actionable" => {
            if scenario.user_outcome != "actionable_gap" {
                errors.push(format!(
                    "actionable case should have actionable_gap outcome, got {}",
                    scenario.user_outcome
                ));
            }
            if matches!(scenario.repair_kind.as_str(), "" | "unknown" | "no_action") {
                errors.push("actionable case must carry a concrete repair kind".to_string());
            }
            if matches!(scenario.target_test_type.as_str(), "" | "unknown" | "none") {
                errors.push("actionable case must carry a target test type".to_string());
            }
            if finding_alignment_verify_command_is_missing(&scenario.verify_command) {
                errors.push("actionable case must carry a verify command".to_string());
            }
            if scenario.static_limitation_category.is_some() {
                errors.push(
                    "actionable case should not carry a static limitation category".to_string(),
                );
            }
        }
        "already_observed" => {
            if scenario.user_outcome != "no_action" {
                errors.push(format!(
                    "already_observed case should have no_action outcome, got {}",
                    scenario.user_outcome
                ));
            }
            if scenario.repair_kind != "no_action" {
                errors.push("already_observed case should use no_action repair kind".to_string());
            }
        }
        "internal_only" => {
            if scenario.user_outcome != "no_action" {
                errors.push(format!(
                    "internal_only case should have no_action outcome, got {}",
                    scenario.user_outcome
                ));
            }
            if scenario.repair_kind != "no_action" {
                errors.push("internal_only case should use no_action repair kind".to_string());
            }
        }
        "static_limitation" => {
            if scenario.user_outcome != "static_limitation" {
                errors.push(format!(
                    "static_limitation case should have static_limitation outcome, got {}",
                    scenario.user_outcome
                ));
            }
            if scenario
                .static_limitation_category
                .as_deref()
                .is_none_or(str::is_empty)
            {
                errors.push("static limitation case must name a limitation category".to_string());
            }
            if scenario
                .static_limitation_repair_route
                .as_deref()
                .is_none_or(str::is_empty)
            {
                errors
                    .push("static limitation case must name a limitation repair route".to_string());
            }
        }
        other => errors.push(format!("unsupported gap_state `{other}`")),
    }

    DogfoodFindingAlignmentRun {
        name: scenario.name.clone(),
        source_pr: scenario.source_pr.clone(),
        canonical_gap_id: scenario.canonical_gap_id.clone(),
        evidence_class: scenario.evidence_class.clone(),
        raw_findings_total: scenario.raw_findings_total,
        canonical_items_total: scenario.canonical_items_total,
        raw_finding_summary: scenario.raw_finding_summary.clone(),
        gap_state: scenario.gap_state.clone(),
        actionability: scenario.actionability.clone(),
        user_outcome: scenario.user_outcome.clone(),
        repair_kind: scenario.repair_kind.clone(),
        target_test_type: scenario.target_test_type.clone(),
        verify_command: scenario.verify_command.clone(),
        static_limitation_category: scenario.static_limitation_category.clone(),
        static_limitation_repair_route: scenario.static_limitation_repair_route.clone(),
        raw_findings_supporting_only: scenario.raw_findings_supporting_only,
        recommended_repair: scenario.recommended_repair.clone(),
        before_after_context: scenario.before_after_context.clone(),
        must_not_claim: scenario.must_not_claim.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_real_repair_attempt_scenarios() -> Vec<DogfoodRealRepairAttemptScenario> {
    let corpus_path = Path::new(REAL_REPAIR_ATTEMPTS_CORPUS);
    let fallback = |reason: String| {
        vec![DogfoodRealRepairAttemptScenario {
            name: "corpus".to_string(),
            source_ref: "unknown".to_string(),
            canonical_gap_id: "unknown".to_string(),
            packet_id: "unknown".to_string(),
            language: None,
            evidence_class: None,
            source_file: None,
            repair_kind: "unknown".to_string(),
            target_test_or_observer_shape: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            verify_result: "unknown".to_string(),
            receipt_command: "unknown".to_string(),
            receipt_path: None,
            receipt_state: "unknown".to_string(),
            actor_kind: "unknown".to_string(),
            before_gap_state: "unknown".to_string(),
            after_gap_state: "unknown".to_string(),
            outcome: "unknown".to_string(),
            attempted_repair: "unknown".to_string(),
            evidence_movement: "unknown".to_string(),
            operator_note: "unknown".to_string(),
            must_not_change: Vec::new(),
            raw_evidence_refs: Vec::new(),
            missing_receipt_reason: None,
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback("real repair attempt corpus schema_version must be 0.1".to_string());
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("real_repair_attempts_corpus") {
        return fallback(
            "real repair attempt corpus kind must be real_repair_attempts_corpus".to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0057") {
        return fallback("real repair attempt corpus spec must be RIPR-SPEC-0057".to_string());
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("real repair attempt corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| DogfoodRealRepairAttemptScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            source_ref: json_string_field(case, "source_ref")
                .unwrap_or_else(|| "unknown".to_string()),
            canonical_gap_id: json_string_field(case, "canonical_gap_id")
                .unwrap_or_else(|| "unknown".to_string()),
            packet_id: json_string_field(case, "packet_id")
                .unwrap_or_else(|| "unknown".to_string()),
            language: json_string_field(case, "language"),
            evidence_class: json_string_field(case, "evidence_class"),
            source_file: json_string_field(case, "source_file"),
            repair_kind: json_string_field(case, "repair_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            target_test_or_observer_shape: json_string_field(case, "target_test_or_observer_shape")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_result: json_string_field(case, "verify_result")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_command: json_string_field(case, "receipt_command")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_path: json_string_field(case, "receipt_path"),
            receipt_state: json_string_field(case, "receipt_state")
                .unwrap_or_else(|| "unknown".to_string()),
            actor_kind: json_string_field(case, "actor_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            before_gap_state: json_string_field(case, "before_gap_state")
                .unwrap_or_else(|| "unknown".to_string()),
            after_gap_state: json_string_field(case, "after_gap_state")
                .unwrap_or_else(|| "unknown".to_string()),
            outcome: json_string_field(case, "outcome").unwrap_or_else(|| "unknown".to_string()),
            attempted_repair: json_string_field(case, "attempted_repair")
                .unwrap_or_else(|| "unknown".to_string()),
            evidence_movement: json_string_field(case, "evidence_movement")
                .unwrap_or_else(|| "unknown".to_string()),
            operator_note: json_string_field(case, "operator_note")
                .unwrap_or_else(|| "unknown".to_string()),
            must_not_change: json_string_array_field(case, "must_not_change"),
            raw_evidence_refs: json_string_array_field(case, "raw_evidence_refs"),
            missing_receipt_reason: json_string_field(case, "missing_receipt_reason"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "real repair attempt corpus case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_real_repair_attempt_run(
    scenario: &DogfoodRealRepairAttemptScenario,
) -> DogfoodRealRepairAttemptRun {
    let mut errors = Vec::new();
    if scenario.name.trim().is_empty() || scenario.name == "unknown" {
        errors.push("case id must be present".to_string());
    }
    if scenario.source_ref.trim().is_empty() || scenario.source_ref == "unknown" {
        errors.push("source_ref must name the PR, handoff, or run receipt".to_string());
    }
    if !scenario.canonical_gap_id.starts_with("gap:") {
        errors.push(format!(
            "canonical_gap_id must use gap: identity, got {}",
            scenario.canonical_gap_id
        ));
    }
    for (label, value) in [
        ("packet_id", &scenario.packet_id),
        ("repair_kind", &scenario.repair_kind),
        (
            "target_test_or_observer_shape",
            &scenario.target_test_or_observer_shape,
        ),
        ("verify_command", &scenario.verify_command),
        ("verify_result", &scenario.verify_result),
        ("receipt_command", &scenario.receipt_command),
        ("receipt_state", &scenario.receipt_state),
        ("actor_kind", &scenario.actor_kind),
        ("before_gap_state", &scenario.before_gap_state),
        ("after_gap_state", &scenario.after_gap_state),
        ("outcome", &scenario.outcome),
        ("attempted_repair", &scenario.attempted_repair),
        ("evidence_movement", &scenario.evidence_movement),
        ("operator_note", &scenario.operator_note),
    ] {
        if value.trim().is_empty()
            || (value == "unknown" && !matches!(label, "oracle_kind" | "oracle_strength"))
        {
            errors.push(format!("{label} must be present"));
        }
    }
    if scenario.must_not_change.is_empty() {
        errors.push("must_not_change must name bounded edit constraints".to_string());
    }
    if scenario.raw_evidence_refs.is_empty() {
        errors.push("raw_evidence_refs must keep lineage to source evidence".to_string());
    }
    if !matches!(
        scenario.verify_result.as_str(),
        "pass" | "fail" | "not_run" | "not_applicable"
    ) {
        errors.push(format!(
            "verify_result must be pass, fail, not_run, or not_applicable, got {}",
            scenario.verify_result
        ));
    }
    if !matches!(
        scenario.outcome.as_str(),
        "attempted_no_receipt"
            | "receipt_present"
            | "evidence_improved"
            | "evidence_unchanged"
            | "evidence_regressed"
            | "resolved"
            | "unknown"
    ) {
        errors.push(format!(
            "outcome must be a swarm attempt outcome, got {}",
            scenario.outcome
        ));
    }
    if let Some(language) = scenario.language.as_deref()
        && !matches!(
            language,
            "rust" | "python" | "typescript" | "javascript" | "perl"
        )
    {
        errors.push(format!(
            "language must be rust, python, typescript, javascript, or perl when present, got {language}"
        ));
    }
    if scenario.outcome == "attempted_no_receipt" {
        if scenario
            .missing_receipt_reason
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            errors.push("attempted_no_receipt must include missing_receipt_reason".to_string());
        }
    } else if scenario.receipt_path.as_deref().unwrap_or("").is_empty() {
        errors.push("receipt_path must be present for receipt-backed attempts".to_string());
    }
    if scenario.receipt_command == scenario.verify_command {
        errors.push("receipt_command must stay distinct from verify_command".to_string());
    }
    dogfood_real_repair_attempt_push_movement_consistency_errors(scenario, &mut errors);
    if scenario.reason.trim().is_empty() {
        errors.push("reason must explain why the attempt is useful dogfood".to_string());
    }

    DogfoodRealRepairAttemptRun {
        name: scenario.name.clone(),
        source_ref: scenario.source_ref.clone(),
        canonical_gap_id: scenario.canonical_gap_id.clone(),
        packet_id: scenario.packet_id.clone(),
        language: scenario.language.clone(),
        evidence_class: scenario.evidence_class.clone(),
        source_file: scenario.source_file.clone(),
        repair_kind: scenario.repair_kind.clone(),
        target_test_or_observer_shape: scenario.target_test_or_observer_shape.clone(),
        verify_command: scenario.verify_command.clone(),
        verify_result: scenario.verify_result.clone(),
        receipt_command: scenario.receipt_command.clone(),
        receipt_path: scenario.receipt_path.clone(),
        receipt_state: scenario.receipt_state.clone(),
        actor_kind: scenario.actor_kind.clone(),
        before_gap_state: scenario.before_gap_state.clone(),
        after_gap_state: scenario.after_gap_state.clone(),
        outcome: scenario.outcome.clone(),
        attempted_repair: scenario.attempted_repair.clone(),
        evidence_movement: scenario.evidence_movement.clone(),
        operator_note: scenario.operator_note.clone(),
        must_not_change: scenario.must_not_change.clone(),
        raw_evidence_refs: scenario.raw_evidence_refs.clone(),
        missing_receipt_reason: scenario.missing_receipt_reason.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_real_repair_attempt_push_movement_consistency_errors(
    scenario: &DogfoodRealRepairAttemptScenario,
    errors: &mut Vec<String>,
) {
    let expected_movement = dogfood_real_repair_attempt_outcome_movement_term(&scenario.outcome);
    let conflicting_receipt_terms = dogfood_real_repair_attempt_conflicting_movement_terms(
        &scenario.receipt_state,
        expected_movement,
    );
    if !conflicting_receipt_terms.is_empty() {
        errors.push(format!(
            "receipt_state for {} must not claim conflicting movement terms: {}",
            scenario.outcome,
            conflicting_receipt_terms.join(", ")
        ));
    }

    let conflicting_evidence_tokens =
        dogfood_real_repair_attempt_conflicting_evidence_movement_tokens(
            &scenario.evidence_movement,
            expected_movement,
        );
    if !conflicting_evidence_tokens.is_empty() {
        errors.push(format!(
            "evidence_movement for {} must not claim conflicting movement tokens: {}",
            scenario.outcome,
            conflicting_evidence_tokens.join(", ")
        ));
    }
}

pub(crate) fn dogfood_real_repair_attempt_outcome_movement_term(
    outcome: &str,
) -> Option<&'static str> {
    match outcome {
        "evidence_improved" => Some("improved"),
        "evidence_unchanged" => Some("unchanged"),
        "evidence_regressed" => Some("regressed"),
        _ => None,
    }
}

pub(crate) fn dogfood_real_repair_attempt_conflicting_movement_terms(
    value: &str,
    expected: Option<&str>,
) -> Vec<String> {
    let normalized = value.to_ascii_lowercase();
    ["improved", "unchanged", "regressed"]
        .into_iter()
        .filter(|term| Some(*term) != expected && normalized.contains(term))
        .map(str::to_string)
        .collect()
}

pub(crate) fn dogfood_real_repair_attempt_conflicting_evidence_movement_tokens(
    value: &str,
    expected: Option<&str>,
) -> Vec<String> {
    let normalized = value.to_ascii_lowercase();
    [
        ("evidence_improved", "improved"),
        ("evidence_unchanged", "unchanged"),
        ("evidence_regressed", "regressed"),
    ]
    .into_iter()
    .filter(|(token, term)| Some(*term) != expected && normalized.contains(token))
    .map(|(token, _)| token.to_string())
    .collect()
}

pub(crate) fn dogfood_python_real_repo_eval_scenarios() -> Vec<DogfoodPythonRealRepoEvalScenario> {
    dogfood_python_real_repo_eval_scenarios_at(Path::new(PYTHON_REAL_REPO_EVAL_CORPUS))
}

pub(crate) fn dogfood_python_ranked_findings(case: &Value) -> Vec<DogfoodPythonRankedFinding> {
    case.get("ranked_top_3_findings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| DogfoodPythonRankedFinding {
                    rank: json_usize_field(item, "rank").unwrap_or_default(),
                    canonical_gap_id: json_string_field(item, "canonical_gap_id")
                        .unwrap_or_else(|| "unknown".to_string()),
                    repair_card_present: json_bool_field(item, "repair_card_present")
                        .unwrap_or(false),
                    usability: json_string_field(item, "usability")
                        .unwrap_or_else(|| "unknown".to_string()),
                    missing_discriminator: json_string_field(item, "missing_discriminator")
                        .unwrap_or_else(|| "unknown".to_string()),
                    suggested_test_file: json_string_field(item, "suggested_test_file")
                        .unwrap_or_else(|| "unknown".to_string()),
                    verify_command: json_string_field(item, "verify_command")
                        .unwrap_or_else(|| "unknown".to_string()),
                    false_positive_notes: json_string_field(item, "false_positive_notes")
                        .unwrap_or_else(|| "unknown".to_string()),
                    reason: json_string_field(item, "reason").unwrap_or_else(|| {
                        "ranked Python finding did not document a reason".to_string()
                    }),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn dogfood_python_real_repo_eval_scenarios_at(
    corpus_path: &Path,
) -> Vec<DogfoodPythonRealRepoEvalScenario> {
    let fallback = |reason: String| {
        vec![DogfoodPythonRealRepoEvalScenario {
            name: "corpus".to_string(),
            repo_shape: "unknown".to_string(),
            source_kind: "unknown".to_string(),
            source_ref: "unknown".to_string(),
            command: "unknown".to_string(),
            runtime_ms: 0,
            top_finding_summary: "unknown".to_string(),
            canonical_gap_id: "unknown".to_string(),
            repair_card_present: false,
            repair_action: "unknown".to_string(),
            agent_packet_present: false,
            agent_packet_task: "unknown".to_string(),
            agent_packet_command: "unknown".to_string(),
            agent_packet_allowed_files: Vec::new(),
            agent_packet_forbidden_files: Vec::new(),
            agent_packet_stop_if: Vec::new(),
            changed_owner: "unknown".to_string(),
            missing_discriminator: "unknown".to_string(),
            suggested_test_file: "unknown".to_string(),
            suggested_test_name: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            verify_result: "unknown".to_string(),
            verify_summary: "unknown".to_string(),
            after_command: "unknown".to_string(),
            after_runtime_ms: 0,
            receipt_command: "unknown".to_string(),
            receipt_result: "unknown".to_string(),
            gap_movement: "unknown".to_string(),
            closed_gaps: 0,
            usability: "unknown".to_string(),
            false_positive_notes: "unknown".to_string(),
            limitation_notes: "unknown".to_string(),
            unsupported_limitations: Vec::new(),
            ranked_top_3_findings: Vec::new(),
            ranked_top_3_limit_reason: None,
            claim_boundary: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback("Python real-repo eval corpus schema_version must be 0.1".to_string());
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("python_real_repo_eval_corpus") {
        return fallback(
            "Python real-repo eval corpus kind must be python_real_repo_eval_corpus".to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0028") {
        return fallback("Python real-repo eval corpus spec must be RIPR-SPEC-0028".to_string());
    }
    if json_string_field(&corpus, "related_spec").as_deref() != Some("RIPR-SPEC-0057") {
        return fallback(
            "Python real-repo eval corpus related_spec must be RIPR-SPEC-0057".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("Python real-repo eval corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| DogfoodPythonRealRepoEvalScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            repo_shape: json_string_field(case, "repo_shape")
                .unwrap_or_else(|| "unknown".to_string()),
            source_kind: json_string_field(case, "source_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            source_ref: json_string_field(case, "source_ref")
                .unwrap_or_else(|| "unknown".to_string()),
            command: json_string_field(case, "command").unwrap_or_else(|| "unknown".to_string()),
            runtime_ms: json_usize_field(case, "runtime_ms").unwrap_or_default(),
            top_finding_summary: json_string_field(case, "top_finding_summary")
                .unwrap_or_else(|| "unknown".to_string()),
            canonical_gap_id: json_string_field(case, "canonical_gap_id")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_card_present: json_bool_field(case, "repair_card_present").unwrap_or(false),
            repair_action: json_string_field(case, "repair_action")
                .unwrap_or_else(|| "unknown".to_string()),
            agent_packet_present: json_bool_field(case, "agent_packet_present").unwrap_or(false),
            agent_packet_task: json_string_field(case, "agent_packet_task")
                .unwrap_or_else(|| "unknown".to_string()),
            agent_packet_command: json_string_field(case, "agent_packet_command")
                .unwrap_or_else(|| "unknown".to_string()),
            agent_packet_allowed_files: json_string_array_field(case, "agent_packet_allowed_files"),
            agent_packet_forbidden_files: json_string_array_field(
                case,
                "agent_packet_forbidden_files",
            ),
            agent_packet_stop_if: json_string_array_field(case, "agent_packet_stop_if"),
            changed_owner: json_string_field(case, "changed_owner")
                .unwrap_or_else(|| "unknown".to_string()),
            missing_discriminator: json_string_field(case, "missing_discriminator")
                .unwrap_or_else(|| "unknown".to_string()),
            suggested_test_file: json_string_field(case, "suggested_test_file")
                .unwrap_or_else(|| "unknown".to_string()),
            suggested_test_name: json_string_field(case, "suggested_test_name")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_result: json_string_field(case, "verify_result")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_summary: json_string_field(case, "verify_summary")
                .unwrap_or_else(|| "unknown".to_string()),
            after_command: json_string_field(case, "after_command")
                .unwrap_or_else(|| "unknown".to_string()),
            after_runtime_ms: json_usize_field(case, "after_runtime_ms").unwrap_or_default(),
            receipt_command: json_string_field(case, "receipt_command")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_result: json_string_field(case, "receipt_result")
                .unwrap_or_else(|| "unknown".to_string()),
            gap_movement: json_string_field(case, "gap_movement")
                .unwrap_or_else(|| "unknown".to_string()),
            closed_gaps: json_usize_field(case, "closed_gaps").unwrap_or_default(),
            usability: json_string_field(case, "usability")
                .unwrap_or_else(|| "unknown".to_string()),
            false_positive_notes: json_string_field(case, "false_positive_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            limitation_notes: json_string_field(case, "limitation_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            unsupported_limitations: json_string_array_field(case, "unsupported_limitations"),
            ranked_top_3_findings: dogfood_python_ranked_findings(case),
            ranked_top_3_limit_reason: json_string_field(case, "ranked_top_3_limit_reason"),
            claim_boundary: json_string_array_field(case, "claim_boundary"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "Python real-repo eval case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_python_real_repo_eval_run(
    scenario: &DogfoodPythonRealRepoEvalScenario,
) -> DogfoodPythonRealRepoEvalRun {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &scenario.name),
        ("repo_shape", &scenario.repo_shape),
        ("source_kind", &scenario.source_kind),
        ("source_ref", &scenario.source_ref),
        ("command", &scenario.command),
        ("top_finding_summary", &scenario.top_finding_summary),
        ("canonical_gap_id", &scenario.canonical_gap_id),
        ("repair_action", &scenario.repair_action),
        ("agent_packet_task", &scenario.agent_packet_task),
        ("agent_packet_command", &scenario.agent_packet_command),
        ("changed_owner", &scenario.changed_owner),
        ("missing_discriminator", &scenario.missing_discriminator),
        ("suggested_test_file", &scenario.suggested_test_file),
        ("suggested_test_name", &scenario.suggested_test_name),
        ("verify_command", &scenario.verify_command),
        ("verify_result", &scenario.verify_result),
        ("verify_summary", &scenario.verify_summary),
        ("after_command", &scenario.after_command),
        ("receipt_command", &scenario.receipt_command),
        ("receipt_result", &scenario.receipt_result),
        ("gap_movement", &scenario.gap_movement),
        ("usability", &scenario.usability),
        ("false_positive_notes", &scenario.false_positive_notes),
        ("limitation_notes", &scenario.limitation_notes),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }
    if !matches!(
        scenario.source_kind.as_str(),
        "scratch_repo" | "external_repo" | "local_repo"
    ) {
        errors.push(format!(
            "source_kind must be scratch_repo, external_repo, or local_repo, got {}",
            scenario.source_kind
        ));
    }
    if scenario.runtime_ms == 0 {
        errors.push("runtime_ms must be greater than zero".to_string());
    }
    if scenario.after_runtime_ms == 0 {
        errors.push("after_runtime_ms must be greater than zero".to_string());
    }
    if !scenario.canonical_gap_id.starts_with("gap:python:") {
        errors.push(format!(
            "canonical_gap_id must use gap:python: identity, got {}",
            scenario.canonical_gap_id
        ));
    }
    if !scenario.changed_owner.starts_with("python:") {
        errors.push(format!(
            "changed_owner must use python: identity, got {}",
            scenario.changed_owner
        ));
    }
    if !scenario.repair_card_present {
        errors.push("repair_card_present must be true for recorded eval cases".to_string());
    }
    if !scenario.agent_packet_present {
        errors.push("agent_packet_present must be true for recorded eval cases".to_string());
    }
    if !scenario
        .agent_packet_command
        .starts_with("ripr agent packet ")
    {
        errors.push(format!(
            "agent_packet_command must be a ripr agent packet command, got {}",
            scenario.agent_packet_command
        ));
    }
    if !scenario
        .agent_packet_command
        .contains(&scenario.canonical_gap_id)
    {
        errors.push("agent_packet_command must include the recorded canonical_gap_id".to_string());
    }
    if scenario.agent_packet_allowed_files.is_empty() {
        errors.push("agent_packet_allowed_files must not be empty".to_string());
    }
    if !scenario
        .agent_packet_allowed_files
        .contains(&scenario.suggested_test_file)
    {
        errors.push("agent_packet_allowed_files must include suggested_test_file".to_string());
    }
    for allowed in &scenario.agent_packet_allowed_files {
        if !(allowed.starts_with("tests/")
            || allowed.ends_with("_test.py")
            || allowed.contains("/test_"))
        {
            errors.push(format!(
                "agent_packet_allowed_files must stay test-scoped, got {allowed}"
            ));
        }
    }
    if scenario.agent_packet_forbidden_files.is_empty() {
        errors.push("agent_packet_forbidden_files must name production files".to_string());
    }
    for forbidden in &scenario.agent_packet_forbidden_files {
        if scenario.agent_packet_allowed_files.contains(forbidden) {
            errors.push(format!(
                "agent_packet_forbidden_files overlaps allowed file {forbidden}"
            ));
        }
        if forbidden.starts_with("tests/") {
            errors.push(format!(
                "agent_packet_forbidden_files should not forbid test file {forbidden}"
            ));
        }
    }
    if scenario.agent_packet_stop_if.is_empty() {
        errors.push("agent_packet_stop_if must list stop conditions".to_string());
    }
    for stop_if in &scenario.agent_packet_stop_if {
        if stop_if.trim().is_empty() || stop_if == "unknown" {
            errors.push("agent_packet_stop_if entries must be concrete".to_string());
        }
    }
    if !scenario.verify_command.starts_with("pytest ")
        && !scenario.verify_command.starts_with("python -m unittest ")
    {
        errors.push(format!(
            "verify_command must be a pytest or unittest command, got {}",
            scenario.verify_command
        ));
    }
    if !matches!(
        scenario.verify_result.as_str(),
        "pass" | "fail" | "not_run" | "not_applicable"
    ) {
        errors.push(format!(
            "verify_result must be pass, fail, not_run, or not_applicable, got {}",
            scenario.verify_result
        ));
    }
    if !matches!(
        scenario.receipt_result.as_str(),
        "pass" | "fail" | "not_run" | "not_applicable"
    ) {
        errors.push(format!(
            "receipt_result must be pass, fail, not_run, or not_applicable, got {}",
            scenario.receipt_result
        ));
    }
    if !matches!(
        scenario.gap_movement.as_str(),
        "opened" | "closed" | "improved" | "unchanged" | "regressed" | "no_receipt"
    ) {
        errors.push(format!(
            "gap_movement must be opened, closed, improved, unchanged, regressed, or no_receipt, got {}",
            scenario.gap_movement
        ));
    }
    if scenario.gap_movement == "closed" {
        if scenario.closed_gaps == 0 {
            errors.push("closed gap movement must record closed_gaps > 0".to_string());
        }
        if scenario.verify_result != "pass" {
            errors.push("closed gap movement requires verify_result=pass".to_string());
        }
        if scenario.receipt_result != "pass" {
            errors.push("closed gap movement requires receipt_result=pass".to_string());
        }
    }
    if scenario.receipt_command == scenario.verify_command {
        errors.push("receipt_command must stay distinct from verify_command".to_string());
    }
    for limitation in &scenario.unsupported_limitations {
        if limitation.trim().is_empty() || limitation == "unknown" {
            errors.push("unsupported_limitations entries must be named limitations".to_string());
        }
        if limitation.contains(' ') {
            errors.push(format!(
                "unsupported_limitations entries must be stable tokens, got {limitation}"
            ));
        }
    }
    if scenario
        .limitation_notes
        .to_ascii_lowercase()
        .contains("unsupported")
        && scenario.unsupported_limitations.is_empty()
    {
        errors.push(
            "limitation_notes mention unsupported behavior but unsupported_limitations is empty"
                .to_string(),
        );
    }
    if scenario.claim_boundary.is_empty() {
        errors.push("claim_boundary must keep preview boundary denials visible".to_string());
    }
    for required in [
        "preview",
        "No arbitrary imports or tests were run by RIPR",
        "No support-tier promotion",
    ] {
        if !scenario.claim_boundary.iter().any(|claim| {
            claim
                .to_ascii_lowercase()
                .contains(&required.to_ascii_lowercase())
        }) {
            errors.push(format!("claim_boundary must include {required}"));
        }
    }
    errors.extend(dogfood_python_ranked_findings_errors(scenario));

    DogfoodPythonRealRepoEvalRun {
        name: scenario.name.clone(),
        repo_shape: scenario.repo_shape.clone(),
        source_kind: scenario.source_kind.clone(),
        source_ref: scenario.source_ref.clone(),
        command: scenario.command.clone(),
        runtime_ms: scenario.runtime_ms,
        top_finding_summary: scenario.top_finding_summary.clone(),
        canonical_gap_id: scenario.canonical_gap_id.clone(),
        repair_card_present: scenario.repair_card_present,
        repair_action: scenario.repair_action.clone(),
        agent_packet_present: scenario.agent_packet_present,
        agent_packet_task: scenario.agent_packet_task.clone(),
        agent_packet_command: scenario.agent_packet_command.clone(),
        agent_packet_allowed_files: scenario.agent_packet_allowed_files.clone(),
        agent_packet_forbidden_files: scenario.agent_packet_forbidden_files.clone(),
        agent_packet_stop_if: scenario.agent_packet_stop_if.clone(),
        changed_owner: scenario.changed_owner.clone(),
        missing_discriminator: scenario.missing_discriminator.clone(),
        suggested_test_file: scenario.suggested_test_file.clone(),
        suggested_test_name: scenario.suggested_test_name.clone(),
        verify_command: scenario.verify_command.clone(),
        verify_result: scenario.verify_result.clone(),
        verify_summary: scenario.verify_summary.clone(),
        after_command: scenario.after_command.clone(),
        after_runtime_ms: scenario.after_runtime_ms,
        receipt_command: scenario.receipt_command.clone(),
        receipt_result: scenario.receipt_result.clone(),
        gap_movement: scenario.gap_movement.clone(),
        closed_gaps: scenario.closed_gaps,
        usability: scenario.usability.clone(),
        false_positive_notes: scenario.false_positive_notes.clone(),
        limitation_notes: scenario.limitation_notes.clone(),
        unsupported_limitations: scenario.unsupported_limitations.clone(),
        ranked_top_3_findings: scenario.ranked_top_3_findings.clone(),
        ranked_top_3_limit_reason: scenario.ranked_top_3_limit_reason.clone(),
        claim_boundary: scenario.claim_boundary.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_python_static_limit_eval_scenarios()
-> Vec<DogfoodPythonStaticLimitEvalScenario> {
    dogfood_python_static_limit_eval_scenarios_at(Path::new(PYTHON_REAL_REPO_EVAL_CORPUS))
}

pub(crate) fn dogfood_python_static_limit_eval_scenarios_at(
    corpus_path: &Path,
) -> Vec<DogfoodPythonStaticLimitEvalScenario> {
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(cases) = corpus.get("static_limit_cases").and_then(Value::as_array) else {
        return Vec::new();
    };

    cases
        .iter()
        .map(|case| DogfoodPythonStaticLimitEvalScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            repo_shape: json_string_field(case, "repo_shape")
                .unwrap_or_else(|| "unknown".to_string()),
            source_kind: json_string_field(case, "source_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            source_ref: json_string_field(case, "source_ref")
                .unwrap_or_else(|| "unknown".to_string()),
            command: json_string_field(case, "command").unwrap_or_else(|| "unknown".to_string()),
            runtime_ms: json_usize_field(case, "runtime_ms").unwrap_or_default(),
            finding_id: json_string_field(case, "finding_id")
                .unwrap_or_else(|| "unknown".to_string()),
            changed_owner: json_string_field(case, "changed_owner")
                .unwrap_or_else(|| "unknown".to_string()),
            static_limit_kind: json_string_field(case, "static_limit_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            classification: json_string_field(case, "classification")
                .unwrap_or_else(|| "unknown".to_string()),
            stop_reasons: json_string_array_field(case, "stop_reasons"),
            related_test_file: json_string_field(case, "related_test_file")
                .unwrap_or_else(|| "unknown".to_string()),
            related_test_name: json_string_field(case, "related_test_name")
                .unwrap_or_else(|| "unknown".to_string()),
            why_not_actionable: json_string_field(case, "why_not_actionable")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_card_present: json_bool_field(case, "repair_card_present").unwrap_or(true),
            agent_packet_present: json_bool_field(case, "agent_packet_present").unwrap_or(true),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_result: json_string_field(case, "verify_result")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_command: json_string_field(case, "receipt_command")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_result: json_string_field(case, "receipt_result")
                .unwrap_or_else(|| "unknown".to_string()),
            gap_movement: json_string_field(case, "gap_movement")
                .unwrap_or_else(|| "unknown".to_string()),
            false_positive_notes: json_string_field(case, "false_positive_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            limitation_notes: json_string_field(case, "limitation_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            claim_boundary: json_string_array_field(case, "claim_boundary"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "Python static-limit eval case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_python_static_limit_eval_run(
    scenario: &DogfoodPythonStaticLimitEvalScenario,
) -> DogfoodPythonStaticLimitEvalRun {
    let mut errors = Vec::new();
    let is_generated_file_exclusion = scenario.static_limit_kind == "generated_file";
    for (label, value) in [
        ("case id", &scenario.name),
        ("repo_shape", &scenario.repo_shape),
        ("source_kind", &scenario.source_kind),
        ("source_ref", &scenario.source_ref),
        ("command", &scenario.command),
        ("finding_id", &scenario.finding_id),
        ("changed_owner", &scenario.changed_owner),
        ("static_limit_kind", &scenario.static_limit_kind),
        ("classification", &scenario.classification),
        ("related_test_file", &scenario.related_test_file),
        ("related_test_name", &scenario.related_test_name),
        ("why_not_actionable", &scenario.why_not_actionable),
        ("verify_command", &scenario.verify_command),
        ("verify_result", &scenario.verify_result),
        ("receipt_command", &scenario.receipt_command),
        ("receipt_result", &scenario.receipt_result),
        ("gap_movement", &scenario.gap_movement),
        ("false_positive_notes", &scenario.false_positive_notes),
        ("limitation_notes", &scenario.limitation_notes),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }
    if !matches!(
        scenario.source_kind.as_str(),
        "scratch_repo" | "external_repo" | "local_repo"
    ) {
        errors.push(format!(
            "source_kind must be scratch_repo, external_repo, or local_repo, got {}",
            scenario.source_kind
        ));
    }
    if scenario.runtime_ms == 0 {
        errors.push("runtime_ms must be greater than zero".to_string());
    }
    if is_generated_file_exclusion {
        if !scenario.finding_id.starts_with("excluded:") {
            errors.push(format!(
                "generated-file exclusions must use an excluded: identity, got {}",
                scenario.finding_id
            ));
        }
    } else if !scenario.finding_id.starts_with("probe:") {
        errors.push(format!(
            "finding_id must use the static finding probe identity, got {}",
            scenario.finding_id
        ));
    }
    if !scenario.changed_owner.starts_with("python:") {
        errors.push(format!(
            "changed_owner must use python: identity, got {}",
            scenario.changed_owner
        ));
    }
    if is_generated_file_exclusion {
        if scenario.classification != "excluded" {
            errors.push(format!(
                "generated-file exclusions must record classification=excluded, got {}",
                scenario.classification
            ));
        }
    } else if scenario.classification != "static_unknown" {
        errors.push(format!(
            "classification must be static_unknown for no-action limitation evals, got {}",
            scenario.classification
        ));
    }
    if scenario.static_limit_kind.trim().is_empty()
        || scenario.static_limit_kind == "unknown"
        || scenario.static_limit_kind.contains(' ')
    {
        errors.push(format!(
            "static_limit_kind must be a stable token, got {}",
            scenario.static_limit_kind
        ));
    }
    if scenario.stop_reasons.is_empty() {
        errors.push("stop_reasons must include the static-limit stop reason".to_string());
    }
    let has_static_limit_stop_reason = scenario.stop_reasons.iter().any(|reason| {
        reason.contains(&scenario.static_limit_kind) || reason == "static_probe_unknown"
    });
    if !has_static_limit_stop_reason {
        errors.push(
            "stop_reasons must include either the static limit kind or static_probe_unknown"
                .to_string(),
        );
    }
    if !scenario
        .why_not_actionable
        .contains(&scenario.static_limit_kind)
    {
        errors.push("why_not_actionable must name the static limit kind".to_string());
    }
    if scenario.repair_card_present {
        errors.push("static-limit evals must not emit repair cards".to_string());
    }
    if scenario.agent_packet_present {
        errors.push("static-limit evals must not emit agent packets".to_string());
    }
    if scenario.verify_result != "not_applicable" {
        errors.push("static-limit evals must record verify_result=not_applicable".to_string());
    }
    if scenario.receipt_result != "not_applicable" {
        errors.push("static-limit evals must record receipt_result=not_applicable".to_string());
    }
    if scenario.gap_movement != "no_receipt" {
        errors.push("static-limit evals must record gap_movement=no_receipt".to_string());
    }
    if !dogfood_python_static_limit_false_positive_clean(scenario) {
        errors.push("static-limit eval false_positive_notes must be none observed".to_string());
    }
    for required in [
        "preview",
        "No repair packet emitted",
        "No support-tier promotion",
    ] {
        if !scenario.claim_boundary.iter().any(|claim| {
            claim
                .to_ascii_lowercase()
                .contains(&required.to_ascii_lowercase())
        }) {
            errors.push(format!("claim_boundary must include {required}"));
        }
    }

    DogfoodPythonStaticLimitEvalRun {
        name: scenario.name.clone(),
        repo_shape: scenario.repo_shape.clone(),
        source_kind: scenario.source_kind.clone(),
        source_ref: scenario.source_ref.clone(),
        command: scenario.command.clone(),
        runtime_ms: scenario.runtime_ms,
        finding_id: scenario.finding_id.clone(),
        changed_owner: scenario.changed_owner.clone(),
        static_limit_kind: scenario.static_limit_kind.clone(),
        classification: scenario.classification.clone(),
        stop_reasons: scenario.stop_reasons.clone(),
        related_test_file: scenario.related_test_file.clone(),
        related_test_name: scenario.related_test_name.clone(),
        why_not_actionable: scenario.why_not_actionable.clone(),
        repair_card_present: scenario.repair_card_present,
        agent_packet_present: scenario.agent_packet_present,
        verify_command: scenario.verify_command.clone(),
        verify_result: scenario.verify_result.clone(),
        receipt_command: scenario.receipt_command.clone(),
        receipt_result: scenario.receipt_result.clone(),
        gap_movement: scenario.gap_movement.clone(),
        false_positive_notes: scenario.false_positive_notes.clone(),
        limitation_notes: scenario.limitation_notes.clone(),
        claim_boundary: scenario.claim_boundary.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_python_static_limit_false_positive_clean(
    scenario: &DogfoodPythonStaticLimitEvalScenario,
) -> bool {
    matches!(
        scenario
            .false_positive_notes
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "none observed" | "none"
    )
}

pub(crate) fn dogfood_python_static_limit_eval_distribution(
    runs: &[DogfoodPythonStaticLimitEvalRun],
) -> Vec<(String, usize)> {
    let mut distribution = BTreeMap::<String, usize>::new();
    for run in runs {
        *distribution
            .entry(run.static_limit_kind.clone())
            .or_default() += 1;
    }
    distribution.into_iter().collect()
}

pub(crate) fn dogfood_python_no_action_eval_scenarios() -> Vec<DogfoodPythonNoActionEvalScenario> {
    dogfood_python_no_action_eval_scenarios_at(Path::new(PYTHON_REAL_REPO_EVAL_CORPUS))
}

pub(crate) fn dogfood_python_no_action_eval_scenarios_at(
    corpus_path: &Path,
) -> Vec<DogfoodPythonNoActionEvalScenario> {
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(cases) = corpus.get("no_action_cases").and_then(Value::as_array) else {
        return Vec::new();
    };

    cases
        .iter()
        .map(|case| DogfoodPythonNoActionEvalScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            repo_shape: json_string_field(case, "repo_shape")
                .unwrap_or_else(|| "unknown".to_string()),
            source_kind: json_string_field(case, "source_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            source_ref: json_string_field(case, "source_ref")
                .unwrap_or_else(|| "unknown".to_string()),
            command: json_string_field(case, "command").unwrap_or_else(|| "unknown".to_string()),
            runtime_ms: json_usize_field(case, "runtime_ms").unwrap_or_default(),
            finding_id: json_string_field(case, "finding_id")
                .unwrap_or_else(|| "unknown".to_string()),
            changed_owner: json_string_field(case, "changed_owner")
                .unwrap_or_else(|| "unknown".to_string()),
            no_action_kind: json_string_field(case, "no_action_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            classification: json_string_field(case, "classification")
                .unwrap_or_else(|| "unknown".to_string()),
            stop_reasons: json_string_array_field(case, "stop_reasons"),
            related_test_file: json_string_field(case, "related_test_file")
                .unwrap_or_else(|| "unknown".to_string()),
            related_test_name: json_string_field(case, "related_test_name")
                .unwrap_or_else(|| "unknown".to_string()),
            why_not_actionable: json_string_field(case, "why_not_actionable")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_card_present: json_bool_field(case, "repair_card_present").unwrap_or(true),
            agent_packet_present: json_bool_field(case, "agent_packet_present").unwrap_or(true),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_result: json_string_field(case, "verify_result")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_command: json_string_field(case, "receipt_command")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_result: json_string_field(case, "receipt_result")
                .unwrap_or_else(|| "unknown".to_string()),
            gap_movement: json_string_field(case, "gap_movement")
                .unwrap_or_else(|| "unknown".to_string()),
            false_positive_notes: json_string_field(case, "false_positive_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            limitation_notes: json_string_field(case, "limitation_notes")
                .unwrap_or_else(|| "unknown".to_string()),
            claim_boundary: json_string_array_field(case, "claim_boundary"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "Python no-action eval case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_python_no_action_eval_run(
    scenario: &DogfoodPythonNoActionEvalScenario,
) -> DogfoodPythonNoActionEvalRun {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &scenario.name),
        ("repo_shape", &scenario.repo_shape),
        ("source_kind", &scenario.source_kind),
        ("source_ref", &scenario.source_ref),
        ("command", &scenario.command),
        ("finding_id", &scenario.finding_id),
        ("changed_owner", &scenario.changed_owner),
        ("no_action_kind", &scenario.no_action_kind),
        ("classification", &scenario.classification),
        ("related_test_file", &scenario.related_test_file),
        ("related_test_name", &scenario.related_test_name),
        ("why_not_actionable", &scenario.why_not_actionable),
        ("verify_command", &scenario.verify_command),
        ("verify_result", &scenario.verify_result),
        ("receipt_command", &scenario.receipt_command),
        ("receipt_result", &scenario.receipt_result),
        ("gap_movement", &scenario.gap_movement),
        ("false_positive_notes", &scenario.false_positive_notes),
        ("limitation_notes", &scenario.limitation_notes),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }
    if !matches!(
        scenario.source_kind.as_str(),
        "scratch_repo" | "external_repo" | "local_repo"
    ) {
        errors.push(format!(
            "source_kind must be scratch_repo, external_repo, or local_repo, got {}",
            scenario.source_kind
        ));
    }
    if scenario.runtime_ms == 0 {
        errors.push("runtime_ms must be greater than zero".to_string());
    }
    if !scenario.finding_id.starts_with("probe:") {
        errors.push(format!(
            "finding_id must use the Python finding probe identity, got {}",
            scenario.finding_id
        ));
    }
    if !scenario.changed_owner.starts_with("python:") {
        errors.push(format!(
            "changed_owner must use python: identity, got {}",
            scenario.changed_owner
        ));
    }
    let expected_classification = match scenario.no_action_kind.as_str() {
        "already_observed" => "exposed",
        "heuristic_only" => "weakly_exposed",
        _ => "no_static_path",
    };
    if scenario.classification != expected_classification {
        errors.push(format!(
            "classification must be {expected_classification} for {} no-action evals, got {}",
            scenario.no_action_kind, scenario.classification
        ));
    }
    if scenario.no_action_kind.trim().is_empty()
        || scenario.no_action_kind == "unknown"
        || scenario.no_action_kind.contains(' ')
    {
        errors.push(format!(
            "no_action_kind must be a stable token, got {}",
            scenario.no_action_kind
        ));
    }
    if scenario.stop_reasons.is_empty() {
        errors.push("stop_reasons must explain the no-action decision".to_string());
    }
    if !scenario
        .stop_reasons
        .iter()
        .any(|reason| reason.contains(&scenario.no_action_kind))
    {
        errors.push("stop_reasons must include the no_action_kind".to_string());
    }
    if !scenario
        .why_not_actionable
        .contains(&scenario.no_action_kind)
    {
        errors.push("why_not_actionable must name the no_action_kind".to_string());
    }
    if scenario.repair_card_present {
        errors.push("no-action evals must not emit repair cards".to_string());
    }
    if scenario.agent_packet_present {
        errors.push("no-action evals must not emit agent packets".to_string());
    }
    if scenario.verify_result != "not_applicable" {
        errors.push("no-action evals must record verify_result=not_applicable".to_string());
    }
    if scenario.receipt_result != "not_applicable" {
        errors.push("no-action evals must record receipt_result=not_applicable".to_string());
    }
    if scenario.gap_movement != "no_receipt" {
        errors.push("no-action evals must record gap_movement=no_receipt".to_string());
    }
    if !dogfood_python_no_action_false_positive_clean(scenario) {
        errors.push("no-action eval false_positive_notes must be none observed".to_string());
    }
    for required in [
        "preview",
        "No repair packet emitted",
        "No support-tier promotion",
    ] {
        if !scenario.claim_boundary.iter().any(|claim| {
            claim
                .to_ascii_lowercase()
                .contains(&required.to_ascii_lowercase())
        }) {
            errors.push(format!("claim_boundary must include {required}"));
        }
    }

    DogfoodPythonNoActionEvalRun {
        name: scenario.name.clone(),
        repo_shape: scenario.repo_shape.clone(),
        source_kind: scenario.source_kind.clone(),
        source_ref: scenario.source_ref.clone(),
        command: scenario.command.clone(),
        runtime_ms: scenario.runtime_ms,
        finding_id: scenario.finding_id.clone(),
        changed_owner: scenario.changed_owner.clone(),
        no_action_kind: scenario.no_action_kind.clone(),
        classification: scenario.classification.clone(),
        stop_reasons: scenario.stop_reasons.clone(),
        related_test_file: scenario.related_test_file.clone(),
        related_test_name: scenario.related_test_name.clone(),
        why_not_actionable: scenario.why_not_actionable.clone(),
        repair_card_present: scenario.repair_card_present,
        agent_packet_present: scenario.agent_packet_present,
        verify_command: scenario.verify_command.clone(),
        verify_result: scenario.verify_result.clone(),
        receipt_command: scenario.receipt_command.clone(),
        receipt_result: scenario.receipt_result.clone(),
        gap_movement: scenario.gap_movement.clone(),
        false_positive_notes: scenario.false_positive_notes.clone(),
        limitation_notes: scenario.limitation_notes.clone(),
        claim_boundary: scenario.claim_boundary.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_python_no_action_false_positive_clean(
    scenario: &DogfoodPythonNoActionEvalScenario,
) -> bool {
    matches!(
        scenario
            .false_positive_notes
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "none observed" | "none"
    )
}

pub(crate) fn dogfood_python_no_action_eval_distribution(
    runs: &[DogfoodPythonNoActionEvalRun],
) -> Vec<(String, usize)> {
    let mut distribution = BTreeMap::<String, usize>::new();
    for run in runs {
        *distribution.entry(run.no_action_kind.clone()).or_default() += 1;
    }
    distribution.into_iter().collect()
}

pub(crate) fn dogfood_python_repair_routing_quality_summary(
    runs: &[DogfoodPythonRealRepoEvalRun],
) -> DogfoodPythonRepairRoutingQualitySummary {
    let mut summary = DogfoodPythonRepairRoutingQualitySummary {
        cases: runs.len(),
        ..DogfoodPythonRepairRoutingQualitySummary::default()
    };
    let mut unsupported_limitations = BTreeMap::<String, usize>::new();

    for run in runs {
        if dogfood_python_eval_top_1_actionable_usable(run) {
            summary.top_1_actionable_usable += 1;
        }
        if dogfood_python_eval_verify_command_valid(run) {
            summary.verify_command_valid += 1;
        }
        if dogfood_python_eval_agent_packet_bounded(run) {
            summary.agent_packet_bounded += 1;
        }
        if dogfood_python_eval_has_concrete_discriminator(run) {
            summary.concrete_discriminator += 1;
        }
        if dogfood_python_eval_has_suggested_test_location(run) {
            summary.suggested_test_location += 1;
        }
        if !dogfood_python_eval_false_positive_clean(run) {
            summary.false_actionable += 1;
        }
        if !run.errors.is_empty() {
            summary.crashes += 1;
        }
        if run.gap_movement == "closed" && run.receipt_result == "pass" {
            summary.receipt_closed += 1;
        }
        if !run.ranked_top_3_findings.is_empty() {
            summary.top_3_cases_with_ranked_capture += 1;
        }
        if run.ranked_top_3_findings.len() == 3 {
            summary.full_top_3_capture_cases += 1;
        }
        summary.top_3_ranked_findings_checked += run.ranked_top_3_findings.len();
        summary.top_3_actionable_usable += run
            .ranked_top_3_findings
            .iter()
            .filter(|finding| dogfood_python_ranked_finding_actionable_usable(finding))
            .count();
        for limitation in &run.unsupported_limitations {
            *unsupported_limitations
                .entry(limitation.clone())
                .or_default() += 1;
        }
    }

    summary.unsupported_limitation_distribution =
        unsupported_limitations.into_iter().collect::<Vec<_>>();

    let missing_quality = summary.cases == 0
        || summary.top_1_actionable_usable != summary.cases
        || summary.verify_command_valid != summary.cases
        || summary.agent_packet_bounded != summary.cases
        || summary.concrete_discriminator != summary.cases
        || summary.suggested_test_location != summary.cases
        || summary.false_actionable > 0
        || summary.crashes > 0
        || summary.receipt_closed == 0
        || summary.top_3_cases_with_ranked_capture != summary.cases
        || summary.full_top_3_capture_cases == 0
        || summary.top_3_ranked_findings_checked == 0
        || summary.top_3_actionable_usable != summary.top_3_ranked_findings_checked;
    if missing_quality {
        summary.gate_status = "review".to_string();
        summary.gate_reason =
            "Python repair-routing dogfood quality is incomplete or noisy".to_string();
    } else {
        summary.gate_status = "pass".to_string();
        summary.gate_reason = "All checked top Python repair cards are usable, verifiable, placed, and receipt-backed without observed false actionability".to_string();
    }

    summary
}

pub(crate) fn dogfood_typescript_false_actionable_audit_summary(
    cases: &[TypeScriptPreviewFalseActionableAuditCase],
) -> DogfoodTypescriptFalseActionableAuditSummary {
    let mut summary = DogfoodTypescriptFalseActionableAuditSummary {
        cases: cases.len(),
        ..DogfoodTypescriptFalseActionableAuditSummary::default()
    };

    for case in cases {
        if case.must_remain_non_actionable {
            summary.must_remain_non_actionable += 1;
        }
        if case.repair_packet_ready {
            summary.repair_packet_ready_true += 1;
        }
        if case.gap_state == "actionable" {
            summary.actionable_gap_state += 1;
        }
        if case.actionability_category == "complete_repair_packet" {
            summary.complete_packet_category += 1;
        }
        if case.authority_boundary != "preview_advisory_only" {
            summary.preview_boundary_violations += 1;
        }
        if case.must_remain_non_actionable
            && (case.repair_packet_ready
                || case.gap_state == "actionable"
                || case.actionability_category == "complete_repair_packet")
        {
            summary.false_actionable += 1;
        }
    }

    if summary.cases == 0
        || summary.must_remain_non_actionable != summary.cases
        || summary.false_actionable > 0
        || summary.preview_boundary_violations > 0
    {
        summary.gate_status = "review".to_string();
        summary.gate_reason =
            "TypeScript preview false-actionable audit is incomplete or noisy".to_string();
    } else {
        summary.gate_status = "pass".to_string();
        summary.gate_reason =
            "All checked TypeScript-family preview audit rows remain non-actionable".to_string();
    }

    summary
}

pub(crate) fn dogfood_python_eval_top_1_actionable_usable(
    run: &DogfoodPythonRealRepoEvalRun,
) -> bool {
    run.repair_card_present
        && run.usability == "usable"
        && dogfood_python_eval_false_positive_clean(run)
}

pub(crate) fn dogfood_python_eval_verify_command_valid(run: &DogfoodPythonRealRepoEvalRun) -> bool {
    (run.verify_command.starts_with("pytest ")
        || run.verify_command.starts_with("python -m unittest "))
        && run.verify_result == "pass"
}

pub(crate) fn dogfood_python_eval_agent_packet_bounded(run: &DogfoodPythonRealRepoEvalRun) -> bool {
    run.agent_packet_present
        && run.agent_packet_command.starts_with("ripr agent packet ")
        && run.agent_packet_command.contains(&run.canonical_gap_id)
        && run
            .agent_packet_allowed_files
            .contains(&run.suggested_test_file)
        && !run.agent_packet_forbidden_files.is_empty()
        && run
            .agent_packet_forbidden_files
            .iter()
            .all(|forbidden| !run.agent_packet_allowed_files.contains(forbidden))
        && !run.agent_packet_stop_if.is_empty()
}

pub(crate) fn dogfood_python_eval_has_concrete_discriminator(
    run: &DogfoodPythonRealRepoEvalRun,
) -> bool {
    let discriminator = run.missing_discriminator.trim();
    !discriminator.is_empty()
        && discriminator != "unknown"
        && !discriminator.contains("...")
        && !discriminator.eq_ignore_ascii_case("uncertain")
}

pub(crate) fn dogfood_python_eval_has_suggested_test_location(
    run: &DogfoodPythonRealRepoEvalRun,
) -> bool {
    let file = run.suggested_test_file.trim();
    let name = run.suggested_test_name.trim();
    !file.is_empty()
        && file != "unknown"
        && !name.is_empty()
        && name != "unknown"
        && (file.starts_with("tests/") || file.ends_with("_test.py") || file.contains("/test_"))
}

pub(crate) fn dogfood_python_eval_false_positive_clean(run: &DogfoodPythonRealRepoEvalRun) -> bool {
    matches!(
        run.false_positive_notes
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "none observed" | "none"
    )
}

pub(crate) fn dogfood_python_ranked_findings_errors(
    scenario: &DogfoodPythonRealRepoEvalScenario,
) -> Vec<String> {
    let mut errors = Vec::new();
    let findings = &scenario.ranked_top_3_findings;
    if findings.is_empty() {
        errors
            .push("ranked_top_3_findings must capture at least the top Python finding".to_string());
        return errors;
    }
    if findings.len() > 3 {
        errors.push(format!(
            "ranked_top_3_findings must capture at most three findings, got {}",
            findings.len()
        ));
    }
    if findings.len() < 3
        && scenario
            .ranked_top_3_limit_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty() || reason == "unknown")
    {
        errors.push(
            "ranked_top_3_limit_reason must explain fewer-than-three ranked findings".to_string(),
        );
    }

    let mut ranks = BTreeSet::new();
    for finding in findings {
        if finding.rank == 0 || finding.rank > 3 {
            errors.push(format!(
                "ranked_top_3_findings rank must be between 1 and 3, got {}",
                finding.rank
            ));
        }
        if !ranks.insert(finding.rank) {
            errors.push(format!(
                "ranked_top_3_findings rank {} is duplicated",
                finding.rank
            ));
        }
        if !finding.canonical_gap_id.starts_with("gap:python:") {
            errors.push(format!(
                "ranked_top_3_findings rank {} canonical_gap_id must use gap:python: identity",
                finding.rank
            ));
        }
        if !finding.repair_card_present {
            errors.push(format!(
                "ranked_top_3_findings rank {} must be repair-card-backed",
                finding.rank
            ));
        }
        if !dogfood_python_ranked_finding_actionable_usable(finding) {
            errors.push(format!(
                "ranked_top_3_findings rank {} must be usable, concrete, placed, verifiable, and false-positive clean",
                finding.rank
            ));
        }
        if finding.reason.trim().is_empty() || finding.reason == "unknown" {
            errors.push(format!(
                "ranked_top_3_findings rank {} must document a reason",
                finding.rank
            ));
        }
    }

    match findings.iter().find(|finding| finding.rank == 1) {
        Some(top) => {
            if top.canonical_gap_id != scenario.canonical_gap_id {
                errors.push(
                    "ranked_top_3_findings rank 1 canonical_gap_id must match the recorded top finding"
                        .to_string(),
                );
            }
            if top.missing_discriminator != scenario.missing_discriminator {
                errors.push(
                    "ranked_top_3_findings rank 1 missing_discriminator must match the recorded top finding"
                        .to_string(),
                );
            }
            if top.suggested_test_file != scenario.suggested_test_file {
                errors.push(
                    "ranked_top_3_findings rank 1 suggested_test_file must match the recorded top finding"
                        .to_string(),
                );
            }
            if top.verify_command != scenario.verify_command {
                errors.push(
                    "ranked_top_3_findings rank 1 verify_command must match the recorded top finding"
                        .to_string(),
                );
            }
        }
        None => errors.push("ranked_top_3_findings must include rank 1".to_string()),
    }

    errors
}

pub(crate) fn dogfood_python_ranked_finding_actionable_usable(
    finding: &DogfoodPythonRankedFinding,
) -> bool {
    finding.repair_card_present
        && finding.usability == "usable"
        && dogfood_python_ranked_finding_has_concrete_discriminator(finding)
        && dogfood_python_ranked_finding_has_suggested_test_location(finding)
        && dogfood_python_ranked_finding_verify_command_valid(finding)
        && dogfood_python_ranked_finding_false_positive_clean(finding)
}

pub(crate) fn dogfood_python_ranked_finding_verify_command_valid(
    finding: &DogfoodPythonRankedFinding,
) -> bool {
    finding.verify_command.starts_with("pytest ")
        || finding.verify_command.starts_with("python -m unittest ")
}

pub(crate) fn dogfood_python_ranked_finding_has_concrete_discriminator(
    finding: &DogfoodPythonRankedFinding,
) -> bool {
    let discriminator = finding.missing_discriminator.trim();
    !discriminator.is_empty()
        && discriminator != "unknown"
        && !discriminator.contains("...")
        && !discriminator.eq_ignore_ascii_case("uncertain")
}

pub(crate) fn dogfood_python_ranked_finding_has_suggested_test_location(
    finding: &DogfoodPythonRankedFinding,
) -> bool {
    let file = finding.suggested_test_file.trim();
    !file.is_empty()
        && file != "unknown"
        && (file.starts_with("tests/") || file.ends_with("_test.py") || file.contains("/test_"))
}

pub(crate) fn dogfood_python_ranked_finding_false_positive_clean(
    finding: &DogfoodPythonRankedFinding,
) -> bool {
    matches!(
        finding
            .false_positive_notes
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "none observed" | "none"
    )
}

pub(crate) fn dogfood_push_python_quality_ratio_json(
    body: &mut String,
    name: &str,
    count: usize,
    checked: usize,
    higher_is_better: bool,
    reason: &str,
) {
    let status = if checked == 0 {
        "not_measured"
    } else if (higher_is_better && count == checked) || (!higher_is_better && count == 0) {
        "pass"
    } else {
        "review"
    };
    body.push_str(&format!(
        "      \"{}\": {{ \"status\": \"{}\", \"count\": {}, \"checked\": {}, \"reason\": \"{}\" }},\n",
        json_escape(name),
        status,
        count,
        checked,
        json_escape(reason)
    ));
}

pub(crate) fn dogfood_push_python_ranked_findings_json(
    body: &mut String,
    findings: &[DogfoodPythonRankedFinding],
) {
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&format!(
            "{{ \"rank\": {}, \"canonical_gap_id\": \"{}\", \"repair_card_present\": {}, \"usability\": \"{}\", \"missing_discriminator\": \"{}\", \"suggested_test_file\": \"{}\", \"verify_command\": \"{}\", \"false_positive_notes\": \"{}\", \"reason\": \"{}\" }}",
            finding.rank,
            json_escape(&finding.canonical_gap_id),
            finding.repair_card_present,
            json_escape(&finding.usability),
            json_escape(&finding.missing_discriminator),
            json_escape(&finding.suggested_test_file),
            json_escape(&finding.verify_command),
            json_escape(&finding.false_positive_notes),
            json_escape(&finding.reason)
        ));
    }
}

#[cfg(test)]
pub(crate) fn typescript_bun_ub_calibration_cases() -> Vec<TypeScriptBunUbCalibrationCase> {
    typescript_bun_ub_calibration_cases_at(Path::new(TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS))
}

pub(crate) fn typescript_bun_ub_calibration_cases_at(
    corpus_path: &Path,
) -> Vec<TypeScriptBunUbCalibrationCase> {
    let fallback = |reason: String| {
        vec![TypeScriptBunUbCalibrationCase {
            name: "corpus".to_string(),
            source: "unknown".to_string(),
            language: "unknown".to_string(),
            language_status: "unknown".to_string(),
            rust_file: "unknown".to_string(),
            rust_owner: "unknown".to_string(),
            rust_boundary: "unknown".to_string(),
            ts_test_file: "unknown".to_string(),
            ts_entrypoints: Vec::new(),
            shared_array_buffer: false,
            resizable_array_buffer: false,
            view_backed_blob_input: false,
            stable_byte_copy_oracle: false,
            max_byte_length_mention_only: false,
            expected_verdict: "unknown".to_string(),
            expected_missing_discriminators: Vec::new(),
            bridge_confidence: "unknown".to_string(),
            expected_action: "unknown".to_string(),
            suggested_test_file: "unknown".to_string(),
            suggested_shape: None,
            repair_packet_ready: true,
            authority_boundary: "unknown".to_string(),
            non_claims: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "TypeScript Bun UB calibration corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("typescript_bun_ub_calibration_corpus")
    {
        return fallback(
            "TypeScript Bun UB calibration corpus kind must be typescript_bun_ub_calibration_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0027") {
        return fallback(
            "TypeScript Bun UB calibration corpus spec must be RIPR-SPEC-0027".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("TypeScript Bun UB calibration corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| {
            let rust_seam = case.get("rust_seam").unwrap_or(&Value::Null);
            let observed = case.get("observed_ts_facts").unwrap_or(&Value::Null);
            TypeScriptBunUbCalibrationCase {
                name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
                source: json_string_field(case, "source").unwrap_or_else(|| "unknown".to_string()),
                language: json_string_field(case, "language")
                    .unwrap_or_else(|| "unknown".to_string()),
                language_status: json_string_field(case, "language_status")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_file: json_string_field(rust_seam, "file")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_owner: json_string_field(rust_seam, "owner")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_boundary: json_string_field(rust_seam, "boundary")
                    .unwrap_or_else(|| "unknown".to_string()),
                ts_test_file: json_string_field(case, "ts_test_file")
                    .unwrap_or_else(|| "unknown".to_string()),
                ts_entrypoints: json_string_array_field(case, "ts_entrypoints"),
                shared_array_buffer: json_bool_field(observed, "shared_array_buffer")
                    .unwrap_or(false),
                resizable_array_buffer: json_bool_field(observed, "resizable_array_buffer")
                    .unwrap_or(false),
                view_backed_blob_input: json_bool_field(observed, "view_backed_blob_input")
                    .unwrap_or(false),
                stable_byte_copy_oracle: json_bool_field(observed, "stable_byte_copy_oracle")
                    .unwrap_or(false),
                max_byte_length_mention_only: json_bool_field(
                    observed,
                    "max_byte_length_mention_only",
                )
                .unwrap_or(false),
                expected_verdict: json_string_field(case, "expected_verdict")
                    .unwrap_or_else(|| "unknown".to_string()),
                expected_missing_discriminators: json_string_array_field(
                    case,
                    "expected_missing_discriminators",
                ),
                bridge_confidence: json_string_field(case, "bridge_confidence")
                    .unwrap_or_else(|| "unknown".to_string()),
                expected_action: json_string_field(case, "expected_action")
                    .unwrap_or_else(|| "unknown".to_string()),
                suggested_test_file: json_string_field(case, "suggested_test_file")
                    .unwrap_or_else(|| "unknown".to_string()),
                suggested_shape: json_string_field(case, "suggested_shape"),
                repair_packet_ready: json_bool_field(case, "repair_packet_ready").unwrap_or(true),
                authority_boundary: json_string_field(case, "authority_boundary")
                    .unwrap_or_else(|| "unknown".to_string()),
                non_claims: json_string_array_field(case, "non_claims"),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "TypeScript Bun UB calibration case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn typescript_bun_ub_calibration_case_errors(
    case: &TypeScriptBunUbCalibrationCase,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &case.name),
        ("source", &case.source),
        ("language", &case.language),
        ("language_status", &case.language_status),
        ("rust_file", &case.rust_file),
        ("rust_owner", &case.rust_owner),
        ("rust_boundary", &case.rust_boundary),
        ("ts_test_file", &case.ts_test_file),
        ("expected_verdict", &case.expected_verdict),
        ("bridge_confidence", &case.bridge_confidence),
        ("expected_action", &case.expected_action),
        ("suggested_test_file", &case.suggested_test_file),
        ("authority_boundary", &case.authority_boundary),
        ("reason", &case.reason),
    ] {
        let unknown_is_valid_bridge_state =
            label == "bridge_confidence" && case.expected_verdict == "bridge_unknown";
        if value.trim().is_empty() || (value == "unknown" && !unknown_is_valid_bridge_state) {
            errors.push(format!("{label} must be present"));
        }
    }

    if case.language != "typescript" {
        errors.push(format!(
            "language must be typescript for Bun TS calibration, got {}",
            case.language
        ));
    }
    if case.language_status != "preview" {
        errors.push("language_status must be preview".to_string());
    }
    if case.authority_boundary != "preview_advisory_only" {
        errors.push("authority_boundary must be preview_advisory_only".to_string());
    }
    if case.repair_packet_ready {
        errors.push("repair_packet_ready must remain false for calibration cases".to_string());
    }
    if !typescript_bun_ub_calibration_allowed_verdicts().contains(&case.expected_verdict.as_str()) {
        errors.push(format!(
            "expected_verdict must be a Bun UB calibration verdict, got {}",
            case.expected_verdict
        ));
    }
    if !matches!(
        case.bridge_confidence.as_str(),
        "configured_hint" | "heuristic" | "unknown"
    ) {
        errors.push(format!(
            "bridge_confidence must be configured_hint, heuristic, or unknown, got {}",
            case.bridge_confidence
        ));
    }
    if !case.rust_file.ends_with("Blob.rs") {
        errors.push("rust_file must identify the Bun Blob Rust seam".to_string());
    }
    if case.rust_owner != "Blob::from_js_without_defer_gc" {
        errors.push(
            "rust_owner must pin Blob::from_js_without_defer_gc for the #31648 calibration seam"
                .to_string(),
        );
    }
    if !case.rust_boundary.contains("array_buffer.shared")
        || !case.rust_boundary.contains("array_buffer.resizable")
    {
        errors.push(
            "rust_boundary must include array_buffer.shared and array_buffer.resizable".to_string(),
        );
    }
    if !case.ts_test_file.starts_with("test/js/") || !case.ts_test_file.ends_with(".test.ts") {
        errors.push("ts_test_file must be a Bun test/js TypeScript test path".to_string());
    }
    if !case.suggested_test_file.starts_with("test/js/")
        && case.suggested_test_file != "not_applicable"
    {
        errors.push("suggested_test_file must be not_applicable or a Bun test/js path".to_string());
    }
    if case.non_claims.is_empty() {
        errors.push("non_claims must keep preview boundary denials visible".to_string());
    }
    for required in typescript_bun_ub_calibration_required_non_claims() {
        if !case
            .non_claims
            .iter()
            .any(|non_claim| non_claim.contains(required))
        {
            errors.push(format!("non_claims must deny {required}"));
        }
    }

    match case.expected_verdict.as_str() {
        "ts_discriminated" => {
            if !case.shared_array_buffer
                || !case.resizable_array_buffer
                || !case.view_backed_blob_input
                || !case.stable_byte_copy_oracle
            {
                errors.push(
                    "ts_discriminated requires shared, resizable, Blob input, and stable-byte oracle facts"
                        .to_string(),
                );
            }
            if !case.expected_missing_discriminators.is_empty() {
                errors.push("ts_discriminated must not name missing discriminators".to_string());
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push("ts_discriminated must not suggest a new test file".to_string());
            }
        }
        "ts_missing_resizable" => {
            if !case.shared_array_buffer || case.resizable_array_buffer {
                errors.push(
                    "ts_missing_resizable requires shared present and resizable absent".to_string(),
                );
            }
            require_typescript_bun_ub_missing(
                case,
                "resizable_array_buffer",
                BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE,
                &mut errors,
            );
        }
        "ts_missing_shared" => {
            if case.shared_array_buffer || !case.resizable_array_buffer {
                errors.push(
                    "ts_missing_shared requires shared absent and resizable present".to_string(),
                );
            }
            require_typescript_bun_ub_missing(
                case,
                "shared_array_buffer",
                BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE,
                &mut errors,
            );
        }
        "ts_missing_shared_and_resizable" => {
            if case.shared_array_buffer || case.resizable_array_buffer {
                errors.push(
                    "ts_missing_shared_and_resizable requires both boundary facts absent"
                        .to_string(),
                );
            }
            for missing in ["shared_array_buffer", "resizable_array_buffer"] {
                require_typescript_bun_ub_missing(
                    case,
                    missing,
                    BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE,
                    &mut errors,
                );
            }
        }
        "ts_missing_external_oracle" => {
            if case.view_backed_blob_input && case.stable_byte_copy_oracle {
                errors.push(
                    "ts_missing_external_oracle requires a missing Blob input or stable-byte oracle"
                        .to_string(),
                );
            }
            if !case.view_backed_blob_input && !case.stable_byte_copy_oracle {
                errors.push(
                    "ts_missing_external_oracle requires at least one partial Blob observer fact"
                        .to_string(),
                );
            }
            if !case.expected_missing_discriminators.is_empty() {
                errors.push(
                    "ts_missing_external_oracle must not name boundary discriminators".to_string(),
                );
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push(
                    "ts_missing_external_oracle must keep suggested_test_file=not_applicable"
                        .to_string(),
                );
            }
        }
        "ts_mention_not_observer" => {
            if !case.max_byte_length_mention_only {
                errors.push(
                    "ts_mention_not_observer must record max_byte_length_mention_only=true"
                        .to_string(),
                );
            }
            if case.view_backed_blob_input || case.stable_byte_copy_oracle {
                errors.push(
                    "ts_mention_not_observer must not count Blob input or stable-byte oracle facts"
                        .to_string(),
                );
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push(
                    "ts_mention_not_observer must keep suggested_test_file=not_applicable"
                        .to_string(),
                );
            }
        }
        "bridge_unknown" => {
            if case.bridge_confidence != "unknown" {
                errors.push("bridge_unknown requires bridge_confidence=unknown".to_string());
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push(
                    "bridge_unknown must keep suggested_test_file=not_applicable".to_string(),
                );
            }
        }
        _ => {}
    }

    errors
}

pub(crate) fn require_typescript_bun_ub_missing(
    case: &TypeScriptBunUbCalibrationCase,
    missing: &str,
    suggested_test_file: &str,
    errors: &mut Vec<String>,
) {
    if !case
        .expected_missing_discriminators
        .iter()
        .any(|discriminator| discriminator == missing)
    {
        errors.push(format!(
            "{} must include missing discriminator {}",
            case.expected_verdict, missing
        ));
    }
    if case.suggested_test_file != suggested_test_file {
        errors.push(format!(
            "{} must keep suggested_test_file={}",
            case.expected_verdict, suggested_test_file
        ));
    }
}

pub(crate) fn typescript_bun_ub_calibration_allowed_verdicts() -> &'static [&'static str] {
    &[
        "ts_discriminated",
        "ts_missing_shared",
        "ts_missing_resizable",
        "ts_missing_shared_and_resizable",
        "ts_missing_external_oracle",
        "ts_mention_not_observer",
        "bridge_unknown",
    ]
}

pub(crate) fn typescript_bun_ub_calibration_required_non_claims() -> &'static [&'static str] {
    &[
        "provider",
        "source edits",
        "generated tests",
        "runtime Bun execution",
        "mutation execution",
        "default gates",
        "public badge",
        "baseline",
        "RIPR Zero",
        "support-tier promotion",
    ]
}

pub(crate) fn bun_ub_calibration_impl(args: &[String]) -> Result<(), String> {
    let args = parse_bun_ub_calibration_args(args)?;
    let report = bun_ub_calibration_report_value(&args.corpus);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to render Bun UB calibration JSON: {err}"))?;
    write_parented_text_file(&args.out, "bun-ub-calibration JSON", &json)?;
    write_parented_text_file(
        &args.out_md,
        "bun-ub-calibration Markdown",
        &bun_ub_calibration_report_markdown(&report),
    )
}

pub(crate) fn parse_bun_ub_calibration_args(
    args: &[String],
) -> Result<BunUbCalibrationArgs, String> {
    let mut corpus = PathBuf::from(TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS);
    let mut out = PathBuf::from("target/ripr/reports/bun-ub-calibration.json");
    let mut out_md = PathBuf::from("target/ripr/reports/bun-ub-calibration.md");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--corpus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--corpus`\n{}",
                        bun_ub_calibration_usage()
                    ));
                };
                corpus = PathBuf::from(value);
            }
            "--out" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out`\n{}",
                        bun_ub_calibration_usage()
                    ));
                };
                out = PathBuf::from(value);
            }
            "--out-md" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out-md`\n{}",
                        bun_ub_calibration_usage()
                    ));
                };
                out_md = PathBuf::from(value);
            }
            "-h" | "--help" => {
                return Err(bun_ub_calibration_usage());
            }
            other => {
                return Err(format!(
                    "unknown bun-ub-calibration argument `{other}`\n{}",
                    bun_ub_calibration_usage()
                ));
            }
        }
        index += 1;
    }
    Ok(BunUbCalibrationArgs {
        corpus,
        out,
        out_md,
    })
}

pub(crate) fn bun_ub_calibration_usage() -> String {
    "usage: cargo xtask bun-ub-calibration [--corpus <path>] [--out <path>] [--out-md <path>]"
        .to_string()
}

pub(crate) fn bun_ub_calibration_report_value(corpus_path: &Path) -> Value {
    let cases = typescript_bun_ub_calibration_cases_at(corpus_path);
    let mut rows = Vec::new();
    let mut passing_cases = 0usize;
    let mut failing_cases = 0usize;
    let mut verdict_counts = BTreeMap::<String, usize>::new();
    let mut missing_discriminator_cases = 0usize;
    let mut bridge_unknown_cases = 0usize;
    let mut mention_only_cases = 0usize;
    let mut public_packet_exclusions = 0usize;
    let mut repair_packet_ready_cases = 0usize;

    for case in &cases {
        let observed_state = bun_ub_calibration_observed_state(case);
        let missing_discriminators =
            bun_ub_calibration_observed_missing_discriminators(&observed_state);
        let missing_graph_legs =
            bun_ub_calibration_missing_graph_legs(case, &observed_state, &missing_discriminators);
        let mut errors = typescript_bun_ub_calibration_case_errors(case);
        if observed_state != case.expected_verdict {
            errors.push(format!(
                "observed_state {observed_state} did not match expected_verdict {}",
                case.expected_verdict
            ));
        }
        if case.repair_packet_ready {
            errors.push("calibration report rows must not be repair-packet-ready".to_string());
        }

        if errors.is_empty() {
            passing_cases += 1;
        } else {
            failing_cases += 1;
        }
        *verdict_counts.entry(observed_state.clone()).or_default() += 1;
        if !missing_discriminators.is_empty() {
            missing_discriminator_cases += 1;
        }
        if observed_state == "bridge_unknown" {
            bridge_unknown_cases += 1;
        }
        if observed_state == "ts_mention_not_observer" {
            mention_only_cases += 1;
        }
        if !case.repair_packet_ready {
            public_packet_exclusions += 1;
        }
        if case.repair_packet_ready {
            repair_packet_ready_cases += 1;
        }

        rows.push(serde_json::json!({
            "case_id": case.name,
            "source": case.source,
            "expected_state": case.expected_verdict,
            "observed_state": observed_state,
            "status": if errors.is_empty() { "pass" } else { "fail" },
            "rust_seam": {
                "file": case.rust_file,
                "owner": case.rust_owner,
                "boundary": case.rust_boundary,
            },
            "typescript_evidence": {
                "test_file": case.ts_test_file,
                "entrypoints": case.ts_entrypoints,
                "shared_array_buffer": case.shared_array_buffer,
                "resizable_array_buffer": case.resizable_array_buffer,
                "view_backed_blob_input": case.view_backed_blob_input,
                "stable_byte_copy_oracle": case.stable_byte_copy_oracle,
                "max_byte_length_mention_only": case.max_byte_length_mention_only,
            },
            "bridge_confidence": case.bridge_confidence,
            "expected_action": case.expected_action,
            "expected_missing_discriminators": case.expected_missing_discriminators,
            "missing_discriminators": missing_discriminators,
            "missing_graph_legs": missing_graph_legs,
            "suggested_test_file": case.suggested_test_file,
            "suggested_shape": case.suggested_shape,
            "authority_boundary": case.authority_boundary,
            "repair_packet_ready": case.repair_packet_ready,
            "non_claims": case.non_claims,
            "reason": case.reason,
            "errors": errors,
        }));
    }

    let status = if cases.is_empty() {
        "empty"
    } else if failing_cases == 0 && repair_packet_ready_cases == 0 {
        "pass"
    } else {
        "fail"
    };

    serde_json::json!({
        "schema_version": "0.1",
        "report": "bun-ub-calibration",
        "status": status,
        "source_path": normalize_path(corpus_path),
        "authority_boundary": "preview_advisory_only",
        "summary": {
            "cases_total": cases.len(),
            "passing_cases": passing_cases,
            "failing_cases": failing_cases,
            "ts_discriminated_cases": verdict_counts.get("ts_discriminated").copied().unwrap_or_default(),
            "ts_missing_resizable_cases": verdict_counts.get("ts_missing_resizable").copied().unwrap_or_default(),
            "ts_missing_shared_cases": verdict_counts.get("ts_missing_shared").copied().unwrap_or_default(),
            "ts_missing_shared_and_resizable_cases": verdict_counts.get("ts_missing_shared_and_resizable").copied().unwrap_or_default(),
            "ts_missing_external_oracle_cases": verdict_counts.get("ts_missing_external_oracle").copied().unwrap_or_default(),
            "ts_mention_not_observer_cases": mention_only_cases,
            "bridge_unknown_cases": bridge_unknown_cases,
            "missing_discriminator_cases": missing_discriminator_cases,
            "public_packet_exclusions": public_packet_exclusions,
            "repair_packet_ready_cases": repair_packet_ready_cases,
        },
        "operator_question": "This Rust/FFI seam changed. Do Bun's TypeScript integration tests discriminate the boundary that would catch the stable-byte bug?",
        "calibration_boundary": "Bun UB TypeScript calibration is preview/advisory only. It summarizes manifest evidence and does not run Bun, tsc, tsserver, mutation, provider calls, generated tests, source edits, gates, badges, baselines, RIPR Zero, or support-tier promotion.",
        "non_claims": [
            "no provider calls",
            "no source edits",
            "no generated tests",
            "no runtime Bun execution",
            "no mutation execution",
            "no default gates",
            "no public badge contribution",
            "no baseline authority",
            "no RIPR Zero authority",
            "no support-tier promotion",
            "no public repair packet",
            "no full cross-language proof"
        ],
        "rows": rows,
    })
}

pub(crate) fn bun_ub_calibration_observed_state(case: &TypeScriptBunUbCalibrationCase) -> String {
    if case.bridge_confidence == "unknown" {
        return "bridge_unknown".to_string();
    }
    if case.max_byte_length_mention_only
        && !case.view_backed_blob_input
        && !case.stable_byte_copy_oracle
    {
        return "ts_mention_not_observer".to_string();
    }
    if !case.view_backed_blob_input || !case.stable_byte_copy_oracle {
        if case.view_backed_blob_input || case.stable_byte_copy_oracle {
            return "ts_missing_external_oracle".to_string();
        }
        return "ts_mention_not_observer".to_string();
    }
    match (case.shared_array_buffer, case.resizable_array_buffer) {
        (true, true) => "ts_discriminated",
        (true, false) => "ts_missing_resizable",
        (false, true) => "ts_missing_shared",
        (false, false) => "ts_missing_shared_and_resizable",
    }
    .to_string()
}

pub(crate) fn bun_ub_calibration_observed_missing_discriminators(state: &str) -> Vec<&'static str> {
    match state {
        "ts_missing_resizable" => vec!["resizable_array_buffer"],
        "ts_missing_shared" => vec!["shared_array_buffer"],
        "ts_missing_shared_and_resizable" => {
            vec!["shared_array_buffer", "resizable_array_buffer"]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn bun_ub_calibration_missing_graph_legs(
    case: &TypeScriptBunUbCalibrationCase,
    state: &str,
    missing_discriminators: &[&str],
) -> Vec<String> {
    match state {
        "bridge_unknown" => vec!["binding_or_ffi_edge".to_string()],
        "ts_mention_not_observer" => vec!["external_blob_or_stable_byte_observer".to_string()],
        "ts_missing_external_oracle" => {
            let mut missing = Vec::new();
            if !case.view_backed_blob_input {
                missing.push("external_callsite:view_backed_blob_input".to_string());
            }
            if !case.stable_byte_copy_oracle {
                missing.push("external_oracle:stable_byte_copy".to_string());
            }
            if missing.is_empty() {
                missing.push("external_oracle_path".to_string());
            }
            missing
        }
        _ => missing_discriminators
            .iter()
            .map(|missing| format!("boundary_discriminator:{missing}"))
            .collect(),
    }
}

pub(crate) fn bun_ub_calibration_report_markdown(value: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Bun UB TypeScript Calibration\n\n");
    out.push_str(&format!(
        "Status: `{}`\n\n",
        audit_markdown_cell(
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- source_path: `{}`\n",
        audit_markdown_cell(
            value
                .get("source_path")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- authority_boundary: `{}`\n",
        audit_markdown_cell(
            value
                .get("authority_boundary")
                .and_then(Value::as_str)
                .unwrap_or("preview_advisory_only")
        )
    ));
    out.push_str(&format!(
        "- operator_question: {}\n",
        audit_markdown_cell(
            value
                .get("operator_question")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- calibration_boundary: {}\n\n",
        audit_markdown_cell(
            value
                .get("calibration_boundary")
                .and_then(Value::as_str)
                .unwrap_or("preview/advisory only")
        )
    ));

    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    let summary = value.get("summary").unwrap_or(&Value::Null);
    for (label, key) in [
        ("Cases", "cases_total"),
        ("Passing cases", "passing_cases"),
        ("Failing cases", "failing_cases"),
        ("TS discriminated cases", "ts_discriminated_cases"),
        ("Missing resizable cases", "ts_missing_resizable_cases"),
        ("Missing shared cases", "ts_missing_shared_cases"),
        (
            "Missing shared and resizable cases",
            "ts_missing_shared_and_resizable_cases",
        ),
        (
            "Missing external oracle cases",
            "ts_missing_external_oracle_cases",
        ),
        (
            "Mention-not-observer cases",
            "ts_mention_not_observer_cases",
        ),
        ("Bridge-unknown cases", "bridge_unknown_cases"),
        ("Missing-discriminator cases", "missing_discriminator_cases"),
        ("Public packet exclusions", "public_packet_exclusions"),
        ("Repair-packet-ready cases", "repair_packet_ready_cases"),
    ] {
        audit_push_count(
            &mut out,
            label,
            audit_usize(summary, &[key]).unwrap_or_default(),
        );
    }
    out.push('\n');

    let non_claims = audit_markdown_string_array_cell(
        value
            .get("non_claims")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );
    out.push_str(&format!(
        "- non_claims: {}\n\n",
        audit_markdown_cell(&non_claims)
    ));

    out.push_str("## Cases\n\n");
    out.push_str("| Case | Expected | Observed | Status | Missing discriminators | Missing graph legs | Suggested file | Repair packet ready |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if rows.is_empty() {
        out.push_str("| none |  |  |  |  |  |  |  |\n");
    }
    for row in rows {
        let missing_discriminators = audit_markdown_string_array_cell(
            row.get("missing_discriminators")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        let missing_graph_legs = audit_markdown_string_array_cell(
            row.get("missing_graph_legs")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} | `{}` | `{}` |\n",
            audit_markdown_cell(
                row.get("case_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("expected_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("observed_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(&missing_discriminators),
            audit_markdown_cell(&missing_graph_legs),
            audit_markdown_cell(
                row.get("suggested_test_file")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            row.get("repair_packet_ready")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ));
    }
    out.push('\n');
    out
}

pub(crate) fn bun_ub_preview_summary_impl(args: &[String]) -> Result<(), String> {
    let args = parse_bun_ub_preview_summary_args(args)?;
    let report = bun_ub_preview_summary_report_value(&args);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to render Bun UB preview summary JSON: {err}"))?;
    write_parented_text_file(&args.out, "bun-ub-preview-summary JSON", &json)?;
    write_parented_text_file(
        &args.out_md,
        "bun-ub-preview-summary Markdown",
        &bun_ub_preview_summary_markdown(&report),
    )
}

pub(crate) fn parse_bun_ub_preview_summary_args(
    args: &[String],
) -> Result<BunUbPreviewSummaryArgs, String> {
    let mut calibration_corpus = repo_rooted_fixture_path(TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS);
    let mut graph_corpus = cross_language_oracle_graph_corpus_path();
    let mut dogfood_corpus = repo_rooted_fixture_path(BUN_UB_CROSS_LANGUAGE_DOGFOOD_CORPUS);
    let mut out = PathBuf::from("target/ripr/reports/bun-ub-preview-summary.json");
    let mut out_md = PathBuf::from("target/ripr/reports/bun-ub-preview-summary.md");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--calibration-corpus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--calibration-corpus`\n{}",
                        bun_ub_preview_summary_usage()
                    ));
                };
                calibration_corpus = PathBuf::from(value);
            }
            "--graph-corpus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--graph-corpus`\n{}",
                        bun_ub_preview_summary_usage()
                    ));
                };
                graph_corpus = PathBuf::from(value);
            }
            "--dogfood-corpus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--dogfood-corpus`\n{}",
                        bun_ub_preview_summary_usage()
                    ));
                };
                dogfood_corpus = PathBuf::from(value);
            }
            "--out" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out`\n{}",
                        bun_ub_preview_summary_usage()
                    ));
                };
                out = PathBuf::from(value);
            }
            "--out-md" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out-md`\n{}",
                        bun_ub_preview_summary_usage()
                    ));
                };
                out_md = PathBuf::from(value);
            }
            "-h" | "--help" => return Err(bun_ub_preview_summary_usage()),
            other => {
                return Err(format!(
                    "unknown bun-ub-preview-summary argument `{other}`\n{}",
                    bun_ub_preview_summary_usage()
                ));
            }
        }
        index += 1;
    }

    Ok(BunUbPreviewSummaryArgs {
        calibration_corpus,
        graph_corpus,
        dogfood_corpus,
        out,
        out_md,
    })
}

pub(crate) fn bun_ub_preview_summary_usage() -> String {
    "usage: cargo xtask bun-ub-preview-summary [--calibration-corpus <path>] [--graph-corpus <path>] [--dogfood-corpus <path>] [--out <path>] [--out-md <path>]".to_string()
}

pub(crate) fn repo_rooted_fixture_path(relative: &str) -> PathBuf {
    let path = Path::new(relative);
    repo_root()
        .map(|root| root.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn bun_ub_preview_summary_report_value(args: &BunUbPreviewSummaryArgs) -> Value {
    let calibration = bun_ub_calibration_report_value(&args.calibration_corpus);
    let route_quality = cross_language_oracle_route_quality_from_cases(
        normalize_path(&args.graph_corpus),
        &cross_language_oracle_graph_cases_at(&args.graph_corpus),
    );
    let dogfood_runs = dogfood_bun_ub_cross_language_scenarios_at(&args.dogfood_corpus)
        .iter()
        .map(dogfood_bun_ub_cross_language_run)
        .collect::<Vec<_>>();

    let route_rows = route_quality
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let calibration_rows = calibration
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    let mut route_state_counts = BTreeMap::<String, usize>::new();
    let mut dogfood_state_counts = BTreeMap::<String, usize>::new();
    let mut public_packet_exclusions =
        audit_usize(&calibration, &["summary", "public_packet_exclusions"]).unwrap_or_default()
            + audit_usize(
                &route_quality,
                &["cross_language_oracle_graph_public_packet_exclusions"],
            )
            .unwrap_or_default();
    let mut repair_packet_ready_cases =
        audit_usize(&calibration, &["summary", "repair_packet_ready_cases"]).unwrap_or_default()
            + audit_usize(&route_quality, &["repair_packet_ready_cases"]).unwrap_or_default();
    let mut errors = Vec::<String>::new();

    for report in [
        ("calibration", &calibration),
        ("cross_language_oracle_graph", &route_quality),
    ] {
        if report.1.get("status").and_then(Value::as_str) != Some("pass") {
            errors.push(format!(
                "{} source report status was {}",
                report.0,
                report
                    .1
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ));
        }
        if report.1.get("authority_boundary").and_then(Value::as_str)
            != Some("preview_advisory_only")
        {
            errors.push(format!(
                "{} authority_boundary must be preview_advisory_only",
                report.0
            ));
        }
    }

    let calibrated_routes = route_rows
        .iter()
        .map(|row| {
            let state = json_string_value(row, "observed_state");
            *route_state_counts.entry(state.clone()).or_default() += 1;
            serde_json::json!({
                "route_label": json_string_value(row, "case_id"),
                "profile": json_string_value(row, "profile"),
                "profile_status": json_string_value(row, "profile_status"),
                "case_id": json_string_value(row, "case_id"),
                "state": state,
                "gap_state": json_string_value(row, "gap_state"),
                "limitation_category": json_string_value(row, "limitation_category"),
                "repair_route": json_string_value(row, "repair_route"),
                "suggested_test_file": json_string_value(row, "suggested_test_file"),
                "missing_discriminators": json_string_array_value(row.get("missing_discriminators")),
                "missing_graph_legs": json_string_array_value(row.get("missing_graph_legs")),
                "public_projection_eligible": row
                    .get("public_projection_eligible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "repair_packet_ready": row
                    .get("repair_packet_ready")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "authority_boundary": json_string_value(row, "authority_boundary"),
                "unlock_condition": json_string_value(row, "unlock_condition"),
                "proof_mode": json_string_value(row, "proof_mode"),
            })
        })
        .collect::<Vec<_>>();

    let mut dogfood_receipts = Vec::new();
    for run in &dogfood_runs {
        *dogfood_state_counts
            .entry(run.observed_state.clone())
            .or_default() += 1;
        if run.repair_packet_ready {
            repair_packet_ready_cases += 1;
        } else {
            public_packet_exclusions += 1;
        }
        if run.authority_boundary != "preview_advisory_only" {
            errors.push(format!(
                "{} dogfood authority_boundary must be preview_advisory_only",
                run.name
            ));
        }
        for error in &run.errors {
            errors.push(format!("{}: {error}", run.name));
        }
        dogfood_receipts.push(serde_json::json!({
            "case_id": &run.name,
            "source_case": &run.source_case,
            "route_quality_case": &run.route_quality_case,
            "state": &run.observed_state,
            "receipt_state": &run.receipt_state,
            "operator_action": &run.operator_action,
            "proof_mode": &run.proof_mode,
            "suggested_test_file": &run.suggested_test_file,
            "repair_packet_ready": run.repair_packet_ready,
            "authority_boundary": &run.authority_boundary,
            "errors": &run.errors,
        }));
    }

    if repair_packet_ready_cases > 0 {
        errors.push(format!(
            "repair_packet_ready_cases must remain 0, got {repair_packet_ready_cases}"
        ));
    }

    let source_rows_total = calibration_rows.len() + route_rows.len() + dogfood_runs.len();
    let status = if source_rows_total == 0 {
        "empty"
    } else if errors.is_empty() && repair_packet_ready_cases == 0 {
        "pass"
    } else {
        "fail"
    };

    serde_json::json!({
        "schema_version": "0.1",
        "report": "bun-ub-preview-summary",
        "status": status,
        "authority": "preview_advisory_only",
        "authority_boundary": "preview_advisory_only",
        "repair_packet_ready": false,
        "source_paths": {
            "calibration": bun_ub_preview_summary_source_path(&args.calibration_corpus),
            "cross_language_oracle_graph": bun_ub_preview_summary_source_path(&args.graph_corpus),
            "dogfood": bun_ub_preview_summary_source_path(&args.dogfood_corpus),
        },
        "summary": {
            "calibration_cases_total": audit_usize(&calibration, &["summary", "cases_total"]).unwrap_or_default(),
            "route_quality_cases_total": audit_usize(&route_quality, &["cases_total"]).unwrap_or_default(),
            "dogfood_receipts_total": dogfood_runs.len(),
            "route_state_counts": route_state_counts,
            "dogfood_state_counts": dogfood_state_counts,
            "named_static_limitations": bun_ub_preview_summary_named_limitations(route_rows),
            "public_packet_exclusions": public_packet_exclusions,
            "repair_packet_ready_cases": repair_packet_ready_cases,
        },
        "calibrated_routes": calibrated_routes,
        "dogfood_receipts": dogfood_receipts,
        "non_claims": bun_ub_preview_summary_non_claims(),
        "errors": errors,
    })
}

pub(crate) fn bun_ub_preview_summary_source_path(path: &Path) -> String {
    repo_root()
        .map(|root| normalize_repo_relative(&root, path))
        .unwrap_or_else(|_| normalize_path(path))
}

pub(crate) fn json_string_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn json_string_array_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn bun_ub_preview_summary_named_limitations(rows: &[Value]) -> Vec<Value> {
    let mut by_category = BTreeMap::<String, (usize, BTreeSet<String>, BTreeSet<String>)>::new();
    for row in rows {
        let category = json_string_value(row, "limitation_category");
        if matches!(category.as_str(), "" | "unknown" | "not_applicable") {
            continue;
        }
        let route = json_string_value(row, "repair_route");
        let case_id = json_string_value(row, "case_id");
        let entry = by_category
            .entry(category)
            .or_insert_with(|| (0, BTreeSet::new(), BTreeSet::new()));
        entry.0 += 1;
        if !matches!(route.as_str(), "" | "unknown" | "not_applicable") {
            entry.1.insert(route);
        }
        if !matches!(case_id.as_str(), "" | "unknown") {
            entry.2.insert(case_id);
        }
    }

    by_category
        .into_iter()
        .map(|(category, (count, routes, samples))| {
            serde_json::json!({
                "category": category,
                "count": count,
                "repair_routes": routes.into_iter().collect::<Vec<_>>(),
                "sample_case_ids": samples.into_iter().take(3).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub(crate) fn bun_ub_preview_summary_non_claims() -> Vec<&'static str> {
    vec![
        "preview/advisory only",
        "no public repair packet",
        "no runtime Bun execution",
        "no TypeScript execution",
        "no mutation execution",
        "no provider calls",
        "no generated tests",
        "no source edits",
        "no gates or badges",
        "no support-tier promotion",
        "no full cross-language proof",
    ]
}

pub(crate) fn bun_ub_preview_summary_markdown(value: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Bun UB Preview Summary\n\n");
    out.push_str(&format!(
        "Status: `{}`\n\n",
        audit_markdown_cell(
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "authority = {}\n\n",
        audit_markdown_cell(
            value
                .get("authority")
                .and_then(Value::as_str)
                .unwrap_or("preview_advisory_only")
        )
    ));
    out.push_str(&format!(
        "repair_packet_ready: {}\n\n",
        value
            .get("repair_packet_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    out.push_str("This compact preview is built from existing Bun UB calibration, cross-language oracle graph, and dogfood receipt data. It does not create public repair packets, gates, badges, generated tests, source edits, runtime execution, provider calls, or support-tier promotion.\n\n");

    out.push_str("## Source Paths\n\n");
    out.push_str("| Source | Path |\n");
    out.push_str("| --- | --- |\n");
    let source_paths = value.get("source_paths").unwrap_or(&Value::Null);
    for (label, key) in [
        ("Calibration", "calibration"),
        ("Cross-language oracle graph", "cross_language_oracle_graph"),
        ("Dogfood", "dogfood"),
    ] {
        out.push_str(&format!(
            "| {} | `{}` |\n",
            label,
            audit_markdown_cell(
                source_paths
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            )
        ));
    }
    out.push('\n');

    let summary = value.get("summary").unwrap_or(&Value::Null);
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for (label, key) in [
        ("Calibration cases", "calibration_cases_total"),
        ("Route-quality cases", "route_quality_cases_total"),
        ("Dogfood receipts", "dogfood_receipts_total"),
        ("Public packet exclusions", "public_packet_exclusions"),
        ("Repair-packet-ready cases", "repair_packet_ready_cases"),
    ] {
        audit_push_count(
            &mut out,
            label,
            audit_usize(summary, &[key]).unwrap_or_default(),
        );
    }
    out.push('\n');

    out.push_str("## State Counts\n\n");
    out.push_str("| State | Route count | Dogfood count |\n");
    out.push_str("| --- | ---: | ---: |\n");
    for state in bun_ub_preview_summary_state_names(summary) {
        let route_count =
            audit_usize(summary, &["route_state_counts", state.as_str()]).unwrap_or_default();
        let dogfood_count =
            audit_usize(summary, &["dogfood_state_counts", state.as_str()]).unwrap_or_default();
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            audit_markdown_cell(&state),
            route_count,
            dogfood_count
        ));
    }
    out.push('\n');

    out.push_str("## Named Static Limitations\n\n");
    out.push_str("| Category | Count | Repair routes | Sample cases |\n");
    out.push_str("| --- | ---: | --- | --- |\n");
    let limitations = summary
        .get("named_static_limitations")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if limitations.is_empty() {
        out.push_str("| none | 0 | none | none |\n");
    }
    for limitation in limitations {
        let repair_routes = audit_markdown_string_array_cell(
            limitation
                .get("repair_routes")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        let sample_cases = audit_markdown_string_array_cell(
            limitation
                .get("sample_case_ids")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            audit_markdown_cell(
                limitation
                    .get("category")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_usize(limitation, &["count"]).unwrap_or_default(),
            audit_markdown_cell(&repair_routes),
            audit_markdown_cell(&sample_cases)
        ));
    }
    out.push('\n');

    out.push_str("## Calibrated Routes\n\n");
    out.push_str("| Route | State | Gap state | Limitation | Repair route | Suggested file | Repair packet ready |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    let routes = value
        .get("calibrated_routes")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if routes.is_empty() {
        out.push_str("| none |  |  |  |  |  |  |\n");
    }
    for route in routes {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            audit_markdown_cell(
                route
                    .get("route_label")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                route
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                route
                    .get("gap_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                route
                    .get("limitation_category")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                route
                    .get("repair_route")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                route
                    .get("suggested_test_file")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            route
                .get("repair_packet_ready")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ));
    }
    out.push('\n');

    out.push_str("## Dogfood Receipts\n\n");
    out.push_str("| Case | State | Receipt | Operator action | Proof mode | Suggested file | Repair packet ready |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    let receipts = value
        .get("dogfood_receipts")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if receipts.is_empty() {
        out.push_str("| none |  |  |  |  |  |  |\n");
    }
    for receipt in receipts {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            audit_markdown_cell(
                receipt
                    .get("case_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                receipt
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                receipt
                    .get("receipt_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                receipt
                    .get("operator_action")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                receipt
                    .get("proof_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                receipt
                    .get("suggested_test_file")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            receipt
                .get("repair_packet_ready")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ));
    }
    out.push('\n');

    out.push_str("## Non-Claims\n\n");
    let non_claims = value
        .get("non_claims")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if non_claims.is_empty() {
        out.push_str("- none\n");
    }
    for non_claim in non_claims {
        if let Some(non_claim) = non_claim.as_str() {
            out.push_str(&format!("- {}\n", audit_markdown_cell(non_claim)));
        }
    }
    out
}

pub(crate) fn bun_ub_preview_summary_state_names(summary: &Value) -> Vec<String> {
    let mut states = [
        "rust_ungripped_ts_discriminated",
        "rust_ungripped_ts_missing_discriminator",
        "bridge_unknown",
        "ts_mention_not_observer",
        "rust_ungripped_ts_missing_external_oracle",
        "cross_language_target_unresolved",
        "public_reachable_panic_boundary_unrevealed",
    ]
    .iter()
    .map(|state| (*state).to_string())
    .collect::<BTreeSet<_>>();
    for map_name in ["route_state_counts", "dogfood_state_counts"] {
        if let Some(object) = summary.get(map_name).and_then(Value::as_object) {
            states.extend(object.keys().cloned());
        }
    }
    states.into_iter().collect()
}

pub(crate) fn configured_bridge_inventory_impl(args: &[String]) -> Result<(), String> {
    let args = parse_configured_bridge_inventory_args(args)?;
    let report = configured_bridge_inventory_report_value(&args);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to render configured bridge inventory JSON: {err}"))?;
    write_parented_text_file(&args.out, "configured-bridge-inventory JSON", &json)?;
    write_parented_text_file(
        &args.out_md,
        "configured-bridge-inventory Markdown",
        &configured_bridge_inventory_markdown(&report),
    )
}

pub(crate) fn parse_configured_bridge_inventory_args(
    args: &[String],
) -> Result<ConfiguredBridgeInventoryArgs, String> {
    let mut graph_corpus = cross_language_oracle_graph_corpus_path();
    let mut out = PathBuf::from("target/ripr/reports/configured-bridge-inventory.json");
    let mut out_md = PathBuf::from("target/ripr/reports/configured-bridge-inventory.md");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--graph-corpus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--graph-corpus`\n{}",
                        configured_bridge_inventory_usage()
                    ));
                };
                graph_corpus = PathBuf::from(value);
            }
            "--out" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out`\n{}",
                        configured_bridge_inventory_usage()
                    ));
                };
                out = PathBuf::from(value);
            }
            "--out-md" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `--out-md`\n{}",
                        configured_bridge_inventory_usage()
                    ));
                };
                out_md = PathBuf::from(value);
            }
            "-h" | "--help" => return Err(configured_bridge_inventory_usage()),
            other => {
                return Err(format!(
                    "unknown configured-bridge-inventory argument `{other}`\n{}",
                    configured_bridge_inventory_usage()
                ));
            }
        }
        index += 1;
    }

    Ok(ConfiguredBridgeInventoryArgs {
        graph_corpus,
        out,
        out_md,
    })
}

pub(crate) fn configured_bridge_inventory_usage() -> String {
    "usage: cargo xtask configured-bridge-inventory [--graph-corpus <path>] [--out <path>] [--out-md <path>]".to_string()
}

#[derive(Clone, Debug)]
pub(crate) struct ConfiguredBridgeInventoryEntry {
    pub(crate) profile: String,
    pub(crate) label: String,
    pub(crate) profile_status: String,
    pub(crate) surface_state: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) bridge_kind: String,
    pub(crate) bridge_confidence: String,
    pub(crate) external_surface: String,
    pub(crate) proof_modes: BTreeSet<String>,
    pub(crate) source_cases: BTreeSet<String>,
    pub(crate) missing_graph_legs: BTreeSet<String>,
    pub(crate) unlock_conditions: BTreeSet<String>,
    pub(crate) repair_routes: BTreeSet<String>,
}

impl ConfiguredBridgeInventoryEntry {
    pub(crate) fn from_case(case: &CrossLanguageOracleGraphCase, surface_state: &str) -> Self {
        Self {
            profile: case.profile.clone(),
            label: configured_bridge_inventory_profile_label(&case.profile).to_string(),
            profile_status: case.profile_status.clone(),
            surface_state: surface_state.to_string(),
            rust_file: case.rust_file.clone(),
            rust_owner: case.rust_owner.clone(),
            rust_boundary: case.rust_boundary.clone(),
            bridge_kind: case.binding_edge_kind.clone(),
            bridge_confidence: case.binding_edge_confidence.clone(),
            external_surface: case.external_callsite_file.clone(),
            proof_modes: BTreeSet::new(),
            source_cases: BTreeSet::new(),
            missing_graph_legs: BTreeSet::new(),
            unlock_conditions: BTreeSet::new(),
            repair_routes: BTreeSet::new(),
        }
    }

    pub(crate) fn add_case(&mut self, case: &CrossLanguageOracleGraphCase) {
        self.source_cases.insert(case.name.clone());
        self.proof_modes.insert(case.proof_mode.clone());
        self.missing_graph_legs.extend(
            case.missing_graph_legs
                .iter()
                .filter(|leg| {
                    self.surface_state != "configured"
                        || leg.starts_with("binding_or_ffi_edge")
                        || leg.starts_with("helper:")
                })
                .cloned(),
        );
        if !matches!(
            case.unlock_condition.as_str(),
            "" | "unknown" | "not_applicable"
        ) {
            self.unlock_conditions.insert(case.unlock_condition.clone());
        }
        if !matches!(
            case.repair_route.as_str(),
            "" | "unknown" | "not_applicable"
        ) {
            self.repair_routes.insert(case.repair_route.clone());
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        serde_json::json!({
            "profile": self.profile,
            "label": self.label,
            "profile_status": self.profile_status,
            "surface_state": self.surface_state,
            "rust_file": self.rust_file,
            "rust_owner": self.rust_owner,
            "rust_boundary": self.rust_boundary,
            "bridge_kind": self.bridge_kind,
            "bridge_confidence": self.bridge_confidence,
            "external_surface": self.external_surface,
            "source_cases": self.source_cases.iter().cloned().collect::<Vec<_>>(),
            "missing_graph_legs": self.missing_graph_legs.iter().cloned().collect::<Vec<_>>(),
            "unlock_conditions": self.unlock_conditions.iter().cloned().collect::<Vec<_>>(),
            "repair_routes": self.repair_routes.iter().cloned().collect::<Vec<_>>(),
            "proof_modes": self.proof_modes.iter().cloned().collect::<Vec<_>>(),
            "inventory_action": "inventory_only",
            "repair_packet_ready": false,
            "public_projection_eligible": false,
            "repair_target": "not_applicable",
            "verify_command": "not_applicable",
            "receipt_command": "not_applicable",
            "allowed_edit_surface": Vec::<String>::new(),
            "authority_boundary": "preview_advisory_only",
        })
    }
}

pub(crate) fn configured_bridge_inventory_report_value(
    args: &ConfiguredBridgeInventoryArgs,
) -> Value {
    let cases = cross_language_oracle_graph_cases_at(&args.graph_corpus);
    let mut configured = BTreeMap::<String, ConfiguredBridgeInventoryEntry>::new();
    let mut bridge_unknown = BTreeMap::<String, ConfiguredBridgeInventoryEntry>::new();
    let mut future_surfaces = BTreeMap::<String, ConfiguredBridgeInventoryEntry>::new();
    let mut static_limitations = BTreeMap::<String, ConfiguredBridgeInventoryEntry>::new();
    let mut repair_packet_ready_cases = 0usize;

    for case in &cases {
        if case.repair_packet_ready {
            repair_packet_ready_cases += 1;
        }

        let (bucket, surface_state) = if case.profile_status == "manifest_only" {
            (&mut future_surfaces, "manifest_only")
        } else if case.expected_state == "bridge_unknown" {
            (&mut bridge_unknown, "bridge_unknown")
        } else if case.binding_edge_kind == "configured_bridge"
            && case.binding_edge_confidence == "configured_hint"
            && cross_language_oracle_graph_has_raw_ref(case, "binding_edge")
        {
            (&mut configured, "configured")
        } else {
            (&mut static_limitations, "named_static_limitation")
        };

        let entry = bucket
            .entry(case.profile.clone())
            .or_insert_with(|| ConfiguredBridgeInventoryEntry::from_case(case, surface_state));
        entry.add_case(case);
    }

    let configured_bridges = configured
        .values()
        .map(ConfiguredBridgeInventoryEntry::to_json)
        .collect::<Vec<_>>();
    let bridge_unknown_routes = bridge_unknown
        .values()
        .map(ConfiguredBridgeInventoryEntry::to_json)
        .collect::<Vec<_>>();
    let future_surface_rows = future_surfaces
        .values()
        .map(ConfiguredBridgeInventoryEntry::to_json)
        .collect::<Vec<_>>();
    let static_limitation_rows = static_limitations
        .values()
        .map(ConfiguredBridgeInventoryEntry::to_json)
        .collect::<Vec<_>>();

    let mut errors = Vec::<String>::new();
    for required in [
        "bun_blob_array_buffer",
        "bun_array_buffer_copy_to_unshared",
        "bun_markdown_resizable_array_buffer",
    ] {
        if !configured.contains_key(required) {
            errors.push(format!(
                "configured bridge inventory is missing configured profile {required}"
            ));
        }
    }
    for required in ["bun_node_fs_scalar_write", "bun_write_helper_gated"] {
        if !future_surfaces.contains_key(required) {
            errors.push(format!(
                "configured bridge inventory is missing manifest-only future surface {required}"
            ));
        }
    }
    if repair_packet_ready_cases > 0 {
        errors.push(format!(
            "configured bridge inventory must not expose repair packets, got {repair_packet_ready_cases}"
        ));
    }

    let status = if cases.is_empty() {
        "empty"
    } else if errors.is_empty() {
        "pass"
    } else {
        "fail"
    };

    serde_json::json!({
        "schema_version": "0.1",
        "report": "configured-bridge-inventory",
        "status": status,
        "authority": "preview_advisory_only",
        "authority_boundary": "preview_advisory_only",
        "repair_packet_ready": false,
        "source_path": bun_ub_preview_summary_source_path(&args.graph_corpus),
        "summary": {
            "configured_bridge_profiles": configured_bridges.len(),
            "bridge_unknown_profiles": bridge_unknown_routes.len(),
            "future_or_missing_surfaces": future_surface_rows.len(),
            "named_static_limitation_profiles": static_limitation_rows.len(),
            "repair_packet_ready_cases": repair_packet_ready_cases,
            "s3_surfaces_backed_by_corpus": configured_bridge_inventory_profile_present(&cases, "s3"),
        },
        "configured_bridges": configured_bridges,
        "bridge_unknown": bridge_unknown_routes,
        "future_or_missing_surfaces": future_surface_rows,
        "named_static_limitations": static_limitation_rows,
        "non_claims": configured_bridge_inventory_non_claims(),
        "errors": errors,
    })
}

pub(crate) fn configured_bridge_inventory_profile_present(
    cases: &[CrossLanguageOracleGraphCase],
    needle: &str,
) -> bool {
    cases.iter().any(|case| case.profile.contains(needle))
}

pub(crate) fn configured_bridge_inventory_profile_label(profile: &str) -> &str {
    match profile {
        "bun_blob_array_buffer" => "Blob ArrayBuffer",
        "bun_array_buffer_copy_to_unshared" => "copy_to_unshared",
        "bun_markdown_resizable_array_buffer" => "MarkdownObject",
        "bun_ffi_negative_offset_panic_boundary" => "FFI panic boundary",
        "bun_node_fs_scalar_write" => "node:fs scalar write sink",
        "bun_write_helper_gated" => "Bun.write sink",
        other => other,
    }
}

pub(crate) fn configured_bridge_inventory_non_claims() -> Vec<&'static str> {
    vec![
        "preview/advisory only",
        "report-only bridge inventory",
        "no inferred reachability",
        "no full Bun binding graph",
        "no public repair packet",
        "no placement from missing inventory rows",
        "no runtime Bun execution",
        "no TypeScript execution",
        "no mutation execution",
        "no provider calls",
        "no generated tests",
        "no source edits",
        "no gates or badges",
        "no support-tier promotion",
    ]
}

pub(crate) fn configured_bridge_inventory_markdown(value: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Configured Bridge Inventory\n\n");
    out.push_str(&format!(
        "Status: `{}`\n\n",
        audit_markdown_cell(
            value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "authority = {}\n\n",
        audit_markdown_cell(
            value
                .get("authority")
                .and_then(Value::as_str)
                .unwrap_or("preview_advisory_only")
        )
    ));
    out.push_str(&format!(
        "repair_packet_ready: {}\n\n",
        value
            .get("repair_packet_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    out.push_str("This report lists configured bridge profiles and manifest-only future surfaces from the existing cross-language oracle graph corpus. It does not infer reachability, create repair packets, suggest placement from missing inventory rows, run Bun or TypeScript, edit sources, or promote support status.\n\n");
    out.push_str(&format!(
        "- source_path: `{}`\n\n",
        audit_markdown_cell(
            value
                .get("source_path")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));

    let summary = value.get("summary").unwrap_or(&Value::Null);
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for (label, key) in [
        ("Configured bridge profiles", "configured_bridge_profiles"),
        ("Bridge-unknown profiles", "bridge_unknown_profiles"),
        ("Future or missing surfaces", "future_or_missing_surfaces"),
        (
            "Named static limitation profiles",
            "named_static_limitation_profiles",
        ),
        ("Repair-packet-ready cases", "repair_packet_ready_cases"),
    ] {
        audit_push_count(
            &mut out,
            label,
            audit_usize(summary, &[key]).unwrap_or_default(),
        );
    }
    out.push_str(&format!(
        "| S3 surfaces backed by corpus | `{}` |\n\n",
        summary
            .get("s3_surfaces_backed_by_corpus")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));

    configured_bridge_inventory_push_table(
        &mut out,
        "Configured Bridges",
        value
            .get("configured_bridges")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );
    configured_bridge_inventory_push_table(
        &mut out,
        "Bridge Unknown",
        value
            .get("bridge_unknown")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );
    configured_bridge_inventory_push_table(
        &mut out,
        "Future Or Missing Surfaces",
        value
            .get("future_or_missing_surfaces")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );
    configured_bridge_inventory_push_table(
        &mut out,
        "Named Static Limitations",
        value
            .get("named_static_limitations")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );

    out.push_str("## Non-Claims\n\n");
    let non_claims = value
        .get("non_claims")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if non_claims.is_empty() {
        out.push_str("- none\n");
    }
    for non_claim in non_claims {
        if let Some(non_claim) = non_claim.as_str() {
            out.push_str(&format!("- {}\n", audit_markdown_cell(non_claim)));
        }
    }
    let errors = value
        .get("errors")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if !errors.is_empty() {
        out.push_str("\n## Errors\n\n");
        for error in errors {
            if let Some(error) = error.as_str() {
                out.push_str(&format!("- {}\n", audit_markdown_cell(error)));
            }
        }
    }
    out
}

pub(crate) fn configured_bridge_inventory_push_table(
    out: &mut String,
    heading: &str,
    rows: &[Value],
) {
    out.push_str(&format!("## {heading}\n\n"));
    out.push_str("| Profile | Label | State | Rust owner | Bridge confidence | External surface | Missing graph legs | Action |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    if rows.is_empty() {
        out.push_str("| none |  |  |  |  |  |  |  |\n\n");
        return;
    }
    for row in rows {
        let missing_graph_legs = audit_markdown_string_array_cell(
            row.get("missing_graph_legs")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | `{}` |\n",
            audit_markdown_cell(
                row.get("profile")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("surface_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("rust_owner")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("bridge_confidence")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.get("external_surface")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(&missing_graph_legs),
            audit_markdown_cell(
                row.get("inventory_action")
                    .and_then(Value::as_str)
                    .unwrap_or("inventory_only")
            ),
        ));
    }
    out.push('\n');
}

pub(crate) fn write_parented_text_file(
    path: &Path,
    label: &str,
    contents: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {} parent: {err}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|err| format!("failed to write {label}: {err}"))
}

#[cfg(test)]
pub(crate) fn cross_language_oracle_graph_cases() -> Vec<CrossLanguageOracleGraphCase> {
    cross_language_oracle_graph_cases_at(Path::new(CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS))
}

pub(crate) fn cross_language_oracle_graph_cases_at(
    corpus_path: &Path,
) -> Vec<CrossLanguageOracleGraphCase> {
    let fallback = |reason: String| {
        vec![CrossLanguageOracleGraphCase {
            name: "corpus".to_string(),
            source: "unknown".to_string(),
            profile: "unknown".to_string(),
            profile_status: "unknown".to_string(),
            language: "unknown".to_string(),
            language_status: "unknown".to_string(),
            rust_file: "unknown".to_string(),
            rust_line: None,
            rust_owner: "unknown".to_string(),
            rust_boundary: "unknown".to_string(),
            binding_edge_kind: "unknown".to_string(),
            binding_edge_confidence: "unknown".to_string(),
            external_callsite_file: "unknown".to_string(),
            external_callsite_line: None,
            external_entrypoints: Vec::new(),
            shared_array_buffer: false,
            resizable_array_buffer: false,
            view_backed_blob_input: false,
            stable_byte_copy_oracle: false,
            max_byte_length_mention_only: false,
            external_oracle_file: "unknown".to_string(),
            external_oracle_line: None,
            external_oracle_kind: "unknown".to_string(),
            oracle_strength: "unknown".to_string(),
            expected_state: "unknown".to_string(),
            gap_state: "unknown".to_string(),
            limitation_category: "unknown".to_string(),
            repair_route: "unknown".to_string(),
            authority_boundary: "unknown".to_string(),
            public_projection_eligible: true,
            repair_packet_ready: true,
            suggested_test_file: "unknown".to_string(),
            allowed_edit_surface: Vec::new(),
            verify_command: None,
            receipt_command: None,
            missing_discriminators: Vec::new(),
            missing_graph_legs: Vec::new(),
            unlock_condition: "unknown".to_string(),
            proof_mode: "unknown".to_string(),
            raw_evidence_refs: Vec::new(),
            non_claims: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "Cross-language oracle graph corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("cross_language_oracle_graph_corpus") {
        return fallback(
            "Cross-language oracle graph corpus kind must be cross_language_oracle_graph_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0062") {
        return fallback(
            "Cross-language oracle graph corpus spec must be RIPR-SPEC-0062".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("Cross-language oracle graph corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| {
            let rust_seam = case.get("rust_seam").unwrap_or(&Value::Null);
            let binding_edge = case.get("binding_edge").unwrap_or(&Value::Null);
            let external_callsite = case.get("external_callsite").unwrap_or(&Value::Null);
            let observed = case.get("observed_ts_facts").unwrap_or(&Value::Null);
            let external_oracle = case.get("external_oracle").unwrap_or(&Value::Null);
            let expected = case.get("expected").unwrap_or(&Value::Null);
            CrossLanguageOracleGraphCase {
                name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
                source: json_string_field(case, "source").unwrap_or_else(|| "unknown".to_string()),
                profile: json_string_field(case, "profile")
                    .unwrap_or_else(|| "bun_blob_array_buffer".to_string()),
                profile_status: json_string_field(case, "profile_status")
                    .unwrap_or_else(|| "active".to_string()),
                language: json_string_field(case, "language")
                    .unwrap_or_else(|| "unknown".to_string()),
                language_status: json_string_field(case, "language_status")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_file: json_string_field(rust_seam, "file")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_line: json_usize_field(rust_seam, "line"),
                rust_owner: json_string_field(rust_seam, "owner")
                    .unwrap_or_else(|| "unknown".to_string()),
                rust_boundary: json_string_field(rust_seam, "boundary")
                    .unwrap_or_else(|| "unknown".to_string()),
                binding_edge_kind: json_string_field(binding_edge, "kind")
                    .unwrap_or_else(|| "unknown".to_string()),
                binding_edge_confidence: json_string_field(binding_edge, "confidence")
                    .unwrap_or_else(|| "unknown".to_string()),
                external_callsite_file: json_string_field(external_callsite, "file")
                    .unwrap_or_else(|| "unknown".to_string()),
                external_callsite_line: json_usize_field(external_callsite, "line"),
                external_entrypoints: json_string_array_field(external_callsite, "entrypoints"),
                shared_array_buffer: json_bool_field(observed, "shared_array_buffer")
                    .unwrap_or(false),
                resizable_array_buffer: json_bool_field(observed, "resizable_array_buffer")
                    .unwrap_or(false),
                view_backed_blob_input: json_bool_field(observed, "view_backed_blob_input")
                    .unwrap_or(false),
                stable_byte_copy_oracle: json_bool_field(observed, "stable_byte_copy_oracle")
                    .unwrap_or(false),
                max_byte_length_mention_only: json_bool_field(
                    observed,
                    "max_byte_length_mention_only",
                )
                .unwrap_or(false),
                external_oracle_file: json_string_field(external_oracle, "file")
                    .unwrap_or_else(|| "unknown".to_string()),
                external_oracle_line: json_usize_field(external_oracle, "line"),
                external_oracle_kind: json_string_field(external_oracle, "kind")
                    .unwrap_or_else(|| "unknown".to_string()),
                oracle_strength: json_string_field(external_oracle, "strength")
                    .unwrap_or_else(|| "unknown".to_string()),
                expected_state: json_string_field(expected, "state")
                    .unwrap_or_else(|| "unknown".to_string()),
                gap_state: json_string_field(expected, "gap_state")
                    .unwrap_or_else(|| "unknown".to_string()),
                limitation_category: json_string_field(expected, "limitation_category")
                    .unwrap_or_else(|| "unknown".to_string()),
                repair_route: json_string_field(expected, "repair_route")
                    .unwrap_or_else(|| "unknown".to_string()),
                authority_boundary: json_string_field(expected, "authority_boundary")
                    .unwrap_or_else(|| "unknown".to_string()),
                public_projection_eligible: json_bool_field(expected, "public_projection_eligible")
                    .unwrap_or(true),
                repair_packet_ready: json_bool_field(expected, "repair_packet_ready")
                    .unwrap_or(true),
                suggested_test_file: json_string_field(expected, "suggested_test_file")
                    .unwrap_or_else(|| "unknown".to_string()),
                allowed_edit_surface: json_string_array_field(expected, "allowed_edit_surface"),
                verify_command: json_string_field(expected, "verify_command"),
                receipt_command: json_string_field(expected, "receipt_command"),
                missing_discriminators: json_string_array_field(expected, "missing_discriminators"),
                missing_graph_legs: json_string_array_field(expected, "missing_graph_legs"),
                unlock_condition: json_string_field(expected, "unlock_condition")
                    .unwrap_or_else(|| "unknown".to_string()),
                proof_mode: json_string_field(expected, "proof_mode")
                    .unwrap_or_else(|| "not_applicable".to_string()),
                raw_evidence_refs: cross_language_oracle_graph_raw_refs(case),
                non_claims: json_string_array_field(case, "non_claims"),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "Cross-language oracle graph case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn cross_language_oracle_graph_raw_refs(
    case: &Value,
) -> Vec<CrossLanguageOracleGraphRawRef> {
    case.get("raw_evidence_refs")
        .and_then(Value::as_array)
        .map(|raw_refs| {
            raw_refs
                .iter()
                .map(|raw_ref| CrossLanguageOracleGraphRawRef {
                    leg: json_string_field(raw_ref, "leg").unwrap_or_else(|| "unknown".to_string()),
                    file: json_string_field(raw_ref, "file")
                        .unwrap_or_else(|| "unknown".to_string()),
                    line: json_usize_field(raw_ref, "line"),
                    kind: json_string_field(raw_ref, "kind")
                        .unwrap_or_else(|| "unknown".to_string()),
                    source_id: json_string_field(raw_ref, "source_id")
                        .unwrap_or_else(|| "unknown".to_string()),
                    sample: json_string_field(raw_ref, "sample")
                        .unwrap_or_else(|| "unknown".to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn cross_language_oracle_graph_case_errors(
    case: &CrossLanguageOracleGraphCase,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &case.name),
        ("source", &case.source),
        ("profile", &case.profile),
        ("profile_status", &case.profile_status),
        ("language", &case.language),
        ("language_status", &case.language_status),
        ("rust_file", &case.rust_file),
        ("rust_owner", &case.rust_owner),
        ("rust_boundary", &case.rust_boundary),
        ("binding_edge_kind", &case.binding_edge_kind),
        ("binding_edge_confidence", &case.binding_edge_confidence),
        ("external_callsite_file", &case.external_callsite_file),
        ("external_oracle_file", &case.external_oracle_file),
        ("external_oracle_kind", &case.external_oracle_kind),
        ("oracle_strength", &case.oracle_strength),
        ("expected_state", &case.expected_state),
        ("gap_state", &case.gap_state),
        ("limitation_category", &case.limitation_category),
        ("repair_route", &case.repair_route),
        ("authority_boundary", &case.authority_boundary),
        ("suggested_test_file", &case.suggested_test_file),
        ("unlock_condition", &case.unlock_condition),
        ("proof_mode", &case.proof_mode),
        ("reason", &case.reason),
    ] {
        let unknown_is_valid_bridge_confidence =
            label == "binding_edge_confidence" && case.expected_state == "bridge_unknown";
        if value.trim().is_empty() || (value == "unknown" && !unknown_is_valid_bridge_confidence) {
            errors.push(format!("{label} must be present"));
        }
    }

    let manifest_only = case.profile_status == "manifest_only";
    if case.rust_line.is_none() && !manifest_only {
        errors.push("rust line must be present".to_string());
    }
    let external_location_unresolved =
        cross_language_oracle_graph_external_location_unresolved(case);
    if case.external_callsite_line.is_none() && !external_location_unresolved {
        errors.push("external_callsite line must be present".to_string());
    }
    if case.external_oracle_line.is_none() && !external_location_unresolved {
        errors.push("external_oracle line must be present".to_string());
    }
    if case.language != "typescript" {
        errors.push(format!(
            "language must be typescript for the cross-language oracle graph, got {}",
            case.language
        ));
    }
    if case.language_status != "preview" {
        errors.push("language_status must be preview".to_string());
    }
    if case.authority_boundary != "preview_advisory_only" {
        errors.push("authority_boundary must be preview_advisory_only".to_string());
    }
    if case.repair_packet_ready {
        errors.push("repair_packet_ready must remain false for graph cases".to_string());
    }
    if case.public_projection_eligible {
        errors.push("public_projection_eligible must remain false for graph cases".to_string());
    }
    let configured_missing_discriminator_placement = case.expected_state
        == "rust_ungripped_ts_missing_discriminator"
        && case.suggested_test_file == BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE;
    let configured_node_fs_manifest_placement = case.profile == "bun_node_fs_scalar_write"
        && case.profile_status == "manifest_only"
        && case.suggested_test_file == "test/js/node/fs/fs.test.ts"
        && cross_language_oracle_graph_has_raw_ref(case, "placement");
    if case.suggested_test_file != "not_applicable"
        && !configured_missing_discriminator_placement
        && !configured_node_fs_manifest_placement
    {
        errors.push(
            "suggested_test_file must be not_applicable, the configured Bun Blob TypeScript test file for missing-discriminator rows, or the typed node:fs manifest placement"
                .to_string(),
        );
    }
    if !case.allowed_edit_surface.is_empty() {
        errors.push("allowed_edit_surface must remain empty".to_string());
    }
    if case.verify_command.is_some() {
        errors.push("verify_command must be omitted for graph cases".to_string());
    }
    if case.receipt_command.is_some() {
        errors.push("receipt_command must be omitted for graph cases".to_string());
    }
    if !cross_language_oracle_graph_allowed_states().contains(&case.expected_state.as_str()) {
        errors.push(format!(
            "expected_state must be a cross-language oracle graph state, got {}",
            case.expected_state
        ));
    }
    if !matches!(
        case.binding_edge_confidence.as_str(),
        "configured_hint" | "heuristic" | "manifest_only" | "unknown"
    ) {
        errors.push(format!(
            "binding_edge_confidence must be configured_hint, heuristic, manifest_only, or unknown, got {}",
            case.binding_edge_confidence
        ));
    }
    if !matches!(
        case.binding_edge_kind.as_str(),
        "configured_bridge" | "ffi_binding" | "manifest_only" | "unresolved"
    ) {
        errors.push(format!(
            "binding_edge_kind must be configured_bridge, ffi_binding, manifest_only, or unresolved, got {}",
            case.binding_edge_kind
        ));
    }
    cross_language_oracle_graph_profile_errors(case, &mut errors);
    if external_location_unresolved {
        if !case.external_callsite_file.starts_with("unresolved:") {
            errors.push(
                "unresolved cross-language panic-boundary callsite file must be explicit"
                    .to_string(),
            );
        }
        if !case.external_oracle_file.starts_with("unresolved:") {
            errors.push(
                "unresolved cross-language panic-boundary oracle file must be explicit".to_string(),
            );
        }
    } else {
        if !case.external_callsite_file.starts_with("test/js/")
            || !case.external_callsite_file.ends_with(".test.ts")
        {
            errors.push(
                "external_callsite_file must be a Bun test/js TypeScript test path".to_string(),
            );
        }
        if !case.external_oracle_file.starts_with("test/js/")
            || !case.external_oracle_file.ends_with(".test.ts")
        {
            errors.push(
                "external_oracle_file must be a Bun test/js TypeScript test path".to_string(),
            );
        }
    }
    if case.external_entrypoints.is_empty() {
        errors.push("external_entrypoints must name at least one external callsite".to_string());
    }
    if case.non_claims.is_empty() {
        errors.push("non_claims must keep cross-language preview denials visible".to_string());
    }
    for required in cross_language_oracle_graph_required_non_claims() {
        if !case
            .non_claims
            .iter()
            .any(|non_claim| non_claim.contains(required))
        {
            errors.push(format!("non_claims must deny {required}"));
        }
    }
    cross_language_oracle_graph_raw_ref_errors(case, &mut errors);

    match case.expected_state.as_str() {
        "rust_ungripped_ts_discriminated" => {
            if case.gap_state != "already_observed" {
                errors.push(
                    "rust_ungripped_ts_discriminated must map to gap_state already_observed"
                        .to_string(),
                );
            }
            if case.limitation_category != "not_applicable" {
                errors.push(
                    "rust_ungripped_ts_discriminated must use limitation_category not_applicable"
                        .to_string(),
                );
            }
            if case.repair_route != "manual-review/cross-language-advisory-witness" {
                errors.push(
                    "rust_ungripped_ts_discriminated must use manual advisory witness route"
                        .to_string(),
                );
            }
            let blob_discriminated = case.shared_array_buffer
                && case.resizable_array_buffer
                && case.view_backed_blob_input
                && case.stable_byte_copy_oracle;
            let markdown_discriminated = case.profile == "bun_markdown_resizable_array_buffer"
                && case.resizable_array_buffer
                && case
                    .external_entrypoints
                    .iter()
                    .any(|entrypoint| entrypoint.contains("Bun.markdown"))
                && case.external_oracle_kind == "markdown_strong_oracle";
            if !blob_discriminated && !markdown_discriminated {
                errors.push(
                    "rust_ungripped_ts_discriminated requires either the Blob shared/resizable stable-byte facts or the Bun markdown resizable strong-oracle profile facts"
                        .to_string(),
                );
            }
            if case.max_byte_length_mention_only {
                errors.push(
                    "rust_ungripped_ts_discriminated must not be mention-only evidence".to_string(),
                );
            }
            if case.binding_edge_confidence != "configured_hint" {
                errors.push(
                    "rust_ungripped_ts_discriminated requires configured bridge confidence"
                        .to_string(),
                );
            }
            if !case.missing_discriminators.is_empty() {
                errors.push(
                    "rust_ungripped_ts_discriminated must not name missing discriminators"
                        .to_string(),
                );
            }
            if !case.missing_graph_legs.is_empty() {
                errors.push(
                    "rust_ungripped_ts_discriminated must not name missing graph legs".to_string(),
                );
            }
            if case.unlock_condition != "not_applicable" {
                errors.push(
                    "rust_ungripped_ts_discriminated must use unlock_condition not_applicable"
                        .to_string(),
                );
            }
        }
        "rust_ungripped_ts_missing_discriminator" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_oracle_visibility_unresolved",
                "analysis/cross-language-oracle-visibility",
                &mut errors,
            );
            if case.missing_discriminators.is_empty() {
                errors.push(
                    "rust_ungripped_ts_missing_discriminator must name missing discriminators"
                        .to_string(),
                );
            }
            if case.name.contains("missing_resizable")
                && !case
                    .missing_discriminators
                    .iter()
                    .any(|missing| missing == "resizable_array_buffer")
            {
                errors.push(
                    "bun_blob_missing_resizable case must name resizable_array_buffer".to_string(),
                );
            }
            if case.suggested_test_file != BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE {
                errors.push(
                    "rust_ungripped_ts_missing_discriminator must suggest the configured Bun Blob TypeScript test file"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(
                case,
                "boundary_discriminator:resizable_array_buffer",
            ) {
                errors.push(
                    "rust_ungripped_ts_missing_discriminator must name the missing boundary discriminator leg"
                    .to_string(),
                );
            }
        }
        "rust_ungripped_ts_missing_external_oracle" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_oracle_visibility_unresolved",
                "analysis/cross-language-oracle-visibility",
                &mut errors,
            );
            if case.view_backed_blob_input && case.stable_byte_copy_oracle {
                errors.push(
                    "rust_ungripped_ts_missing_external_oracle requires a missing Blob input or stable-byte oracle"
                        .to_string(),
                );
            }
            if !case.view_backed_blob_input && !case.stable_byte_copy_oracle {
                errors.push(
                    "rust_ungripped_ts_missing_external_oracle requires at least one partial Blob observer fact"
                        .to_string(),
                );
            }
            if !case.missing_discriminators.is_empty() {
                errors.push(
                    "rust_ungripped_ts_missing_external_oracle must not name missing boundary discriminators"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(
                case,
                "external_callsite:view_backed_blob_input",
            ) && !cross_language_oracle_graph_has_missing_leg(
                case,
                "external_oracle:stable_byte_copy",
            ) {
                errors.push(
                    "rust_ungripped_ts_missing_external_oracle must name the missing external observer graph leg"
                        .to_string(),
                );
            }
        }
        "ts_mention_not_observer" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_oracle_visibility_unresolved",
                "analysis/cross-language-oracle-visibility",
                &mut errors,
            );
            if !case.max_byte_length_mention_only {
                errors.push(
                    "ts_mention_not_observer must record max_byte_length_mention_only=true"
                        .to_string(),
                );
            }
            if case.view_backed_blob_input || case.stable_byte_copy_oracle {
                errors.push(
                    "ts_mention_not_observer must not count Blob input or stable-byte oracle facts"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(
                case,
                "external_callsite:view_backed_blob_input",
            ) {
                errors.push(
                    "ts_mention_not_observer must name the missing external Blob callsite graph leg"
                        .to_string(),
                );
            }
        }
        "bridge_unknown" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_oracle_visibility_unresolved",
                "analysis/cross-language-oracle-visibility",
                &mut errors,
            );
            if case.binding_edge_kind != "unresolved" || case.binding_edge_confidence != "unknown" {
                errors.push(
                    "bridge_unknown requires unresolved binding edge and unknown confidence"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(case, "binding_or_ffi_edge") {
                errors.push("bridge_unknown must name binding_or_ffi_edge as missing".to_string());
            }
        }
        "cross_language_target_unresolved" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_target_unresolved",
                "analysis/cross-language-test-target-inference",
                &mut errors,
            );
            if !cross_language_oracle_graph_has_missing_leg(case, "safe_external_observer_target") {
                errors.push(
                    "cross_language_target_unresolved must name safe_external_observer_target as missing"
                    .to_string(),
                );
            }
        }
        "public_reachable_panic_boundary_unrevealed" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_panic_boundary_visibility_unresolved",
                "analysis/cross-language-panic-boundary-visibility",
                &mut errors,
            );
            if !case
                .missing_discriminators
                .iter()
                .any(|missing| missing == "negative_offset")
            {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must name negative_offset as the missing discriminator"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(
                case,
                "external_oracle:negative_offset_panic_boundary",
            ) {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must name the missing negative-offset panic oracle leg"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(case, "safe_external_observer_target") {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must name safe_external_observer_target as missing"
                        .to_string(),
                );
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must not suggest a test file"
                        .to_string(),
                );
            }
            if case.external_oracle_kind != "negative_offset_panic_oracle_unresolved" {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must use negative_offset_panic_oracle_unresolved oracle kind"
                        .to_string(),
                );
            }
            if case.oracle_strength != "missing_boundary" {
                errors.push(
                    "public_reachable_panic_boundary_unrevealed must use missing_boundary oracle strength"
                        .to_string(),
                );
            }
        }
        "named_static_limitation" => {
            require_cross_language_oracle_graph_limitation(
                case,
                "cross_language_profile_manifest_only",
                "analysis/cross-language-profile-intake",
                &mut errors,
            );
            if case.profile_status != "manifest_only" {
                errors.push(
                    "named_static_limitation profile intake rows must be manifest_only".to_string(),
                );
            }
            if case.repair_packet_ready || case.public_projection_eligible {
                errors.push(
                    "named_static_limitation must remain non-actionable and non-public".to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(case, "binding_or_ffi_edge")
                && !cross_language_oracle_graph_has_missing_leg(case, "external_oracle")
            {
                errors.push(
                    "named_static_limitation must name the missing bridge or oracle graph leg"
                        .to_string(),
                );
            }
        }
        _ => {}
    }

    errors
}

pub(crate) fn cross_language_oracle_graph_raw_ref_errors(
    case: &CrossLanguageOracleGraphCase,
    errors: &mut Vec<String>,
) {
    if case.raw_evidence_refs.is_empty() {
        errors.push("raw_evidence_refs must include structured graph refs".to_string());
        return;
    }
    let raw_legs = case
        .raw_evidence_refs
        .iter()
        .map(|raw_ref| raw_ref.leg.as_str())
        .collect::<BTreeSet<_>>();
    let mut required_legs = vec!["rust_seam", "boundary_discriminator"];
    if !cross_language_oracle_graph_has_missing_leg(
        case,
        "external_callsite:view_backed_blob_input",
    ) {
        required_legs.push("external_callsite");
    }
    if !cross_language_oracle_graph_has_missing_leg(case, "external_oracle:stable_byte_copy") {
        required_legs.push("external_oracle");
    }
    for required in required_legs {
        if !raw_legs.contains(required) {
            errors.push(format!("raw_evidence_refs must include {required}"));
        }
    }
    let binding_edge_missing =
        cross_language_oracle_graph_has_missing_leg(case, "binding_or_ffi_edge");
    if case.expected_state != "bridge_unknown"
        && !binding_edge_missing
        && !raw_legs.contains("binding_edge")
    {
        errors.push("raw_evidence_refs must include binding_edge".to_string());
    }
    if (case.expected_state == "bridge_unknown" || binding_edge_missing)
        && raw_legs.contains("binding_edge")
    {
        errors.push(
            "rows with missing binding_or_ffi_edge must not claim a binding_edge raw ref"
                .to_string(),
        );
    }
    for raw_ref in &case.raw_evidence_refs {
        for (label, value) in [
            ("raw_evidence_ref leg", &raw_ref.leg),
            ("raw_evidence_ref file", &raw_ref.file),
            ("raw_evidence_ref kind", &raw_ref.kind),
            ("raw_evidence_ref source_id", &raw_ref.source_id),
            ("raw_evidence_ref sample", &raw_ref.sample),
        ] {
            if value.trim().is_empty() || value == "unknown" {
                errors.push(format!("{label} must be present"));
            }
        }
        if raw_ref.line.is_none() {
            errors.push(format!(
                "raw_evidence_ref {} must include line",
                raw_ref.leg
            ));
        }
    }
}

pub(crate) fn require_cross_language_oracle_graph_limitation(
    case: &CrossLanguageOracleGraphCase,
    category: &str,
    route: &str,
    errors: &mut Vec<String>,
) {
    if case.gap_state != "static_limitation" {
        errors.push(format!(
            "{} must map to gap_state static_limitation",
            case.expected_state
        ));
    }
    if case.limitation_category != category {
        errors.push(format!(
            "{} must use limitation_category {category}",
            case.expected_state
        ));
    }
    if case.repair_route != route {
        errors.push(format!(
            "{} must use repair_route {route}",
            case.expected_state
        ));
    }
    if case.unlock_condition.trim().is_empty() || case.unlock_condition == "unknown" {
        errors.push(format!(
            "{} must document an unlock_condition",
            case.expected_state
        ));
    }
}

pub(crate) fn cross_language_oracle_graph_has_missing_leg(
    case: &CrossLanguageOracleGraphCase,
    leg: &str,
) -> bool {
    case.missing_graph_legs
        .iter()
        .any(|missing| missing == leg || missing.contains(leg))
}

pub(crate) fn cross_language_oracle_graph_has_raw_ref(
    case: &CrossLanguageOracleGraphCase,
    leg: &str,
) -> bool {
    case.raw_evidence_refs
        .iter()
        .any(|raw_ref| raw_ref.leg == leg)
}

pub(crate) fn cross_language_oracle_graph_external_location_unresolved(
    case: &CrossLanguageOracleGraphCase,
) -> bool {
    case.expected_state == "public_reachable_panic_boundary_unrevealed"
        && cross_language_oracle_graph_has_missing_leg(case, "safe_external_observer_target")
}

pub(crate) fn cross_language_oracle_graph_allowed_states() -> &'static [&'static str] {
    &[
        "rust_ungripped_ts_discriminated",
        "rust_ungripped_ts_missing_discriminator",
        "rust_ungripped_ts_missing_external_oracle",
        "ts_mention_not_observer",
        "bridge_unknown",
        "cross_language_target_unresolved",
        "public_reachable_panic_boundary_unrevealed",
        "named_static_limitation",
    ]
}

pub(crate) fn cross_language_oracle_graph_profile_errors(
    case: &CrossLanguageOracleGraphCase,
    errors: &mut Vec<String>,
) {
    match case.profile.as_str() {
        "bun_blob_array_buffer" => {
            if !case.rust_file.ends_with("Blob.rs") {
                errors.push("rust_file must identify the Bun Blob Rust seam".to_string());
            }
            if case.rust_owner != "Blob::from_js_without_defer_gc" {
                errors.push(
                    "rust_owner must pin Blob::from_js_without_defer_gc for the configured Bun Blob route"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("array_buffer.shared")
                || !case.rust_boundary.contains("array_buffer.resizable")
            {
                errors.push(
                    "rust_boundary must include array_buffer.shared and array_buffer.resizable"
                        .to_string(),
                );
            }
        }
        "bun_array_buffer_copy_to_unshared" => {
            if !case.rust_file.ends_with("array_buffer.rs") {
                errors.push(
                    "rust_file must identify the Bun array_buffer Rust seam".to_string(),
                );
            }
            if case.rust_owner != "copy_to_unshared" {
                errors.push(
                    "rust_owner must pin copy_to_unshared for the Bun array_buffer route"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("SharedArrayBuffer")
                || !case.rust_boundary.contains("resizable ArrayBuffer")
            {
                errors.push(
                    "rust_boundary must name SharedArrayBuffer and resizable ArrayBuffer copy semantics"
                        .to_string(),
                );
            }
            match case.expected_state.as_str() {
                "bridge_unknown" => {}
                "rust_ungripped_ts_discriminated" => {
                    if !matches!(
                        case.binding_edge_kind.as_str(),
                        "configured_bridge" | "ffi_binding"
                    ) || case.binding_edge_confidence == "unknown"
                    {
                        errors.push(
                            "bun_array_buffer_copy_to_unshared configured route requires configured or generated binding edge evidence"
                                .to_string(),
                        );
                    }
                }
                _ => errors.push(
                    "bun_array_buffer_copy_to_unshared profile only supports bridge_unknown or rust_ungripped_ts_discriminated in this preview slice"
                        .to_string(),
                ),
            }
        }
        "bun_markdown_resizable_array_buffer" => {
            if !case.rust_file.ends_with("MarkdownObject.rs") {
                errors.push(
                    "rust_file must identify the Bun MarkdownObject Rust seam".to_string(),
                );
            }
            if case.rust_owner != "MarkdownObject::to_string" {
                errors.push(
                    "rust_owner must pin MarkdownObject::to_string for the Bun markdown route"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("self.0.resizable")
                || !case.rust_boundary.contains("!self.0.shared")
            {
                errors.push(
                    "rust_boundary must include self.0.resizable and !self.0.shared".to_string(),
                );
            }
            if case.expected_state != "rust_ungripped_ts_discriminated" {
                errors.push(
                    "bun_markdown_resizable_array_buffer profile currently only admits the complete advisory witness shape"
                        .to_string(),
                );
            }
            if case.binding_edge_kind != "configured_bridge"
                || case.binding_edge_confidence != "configured_hint"
            {
                errors.push(
                    "bun_markdown_resizable_array_buffer advisory witness requires a configured bridge"
                        .to_string(),
                );
            }
            if case.external_oracle_kind != "markdown_strong_oracle" {
                errors.push(
                    "bun_markdown_resizable_array_buffer requires markdown_strong_oracle external evidence"
                        .to_string(),
                );
            }
        }
        "bun_ffi_negative_offset_panic_boundary" => {
            if !case.rust_file.ends_with("FFIObject.rs") {
                errors.push(
                    "rust_file must identify the Bun FFIObject Rust seam".to_string(),
                );
            }
            if case.rust_owner != "FFIObject::read" {
                errors.push(
                    "rust_owner must pin FFIObject::read for the Bun FFI negative-offset route"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("usize::try_from")
                || !case.rust_boundary.contains("expect(\"int cast\")")
            {
                errors.push(
                    "rust_boundary must include usize::try_from and expect(\"int cast\")"
                        .to_string(),
                );
            }
            if case.expected_state != "public_reachable_panic_boundary_unrevealed" {
                errors.push(
                    "bun_ffi_negative_offset_panic_boundary profile currently only supports public_reachable_panic_boundary_unrevealed"
                        .to_string(),
                );
            }
            if case.binding_edge_kind != "ffi_binding"
                || !matches!(
                    case.binding_edge_confidence.as_str(),
                    "heuristic" | "configured_hint"
                )
            {
                errors.push(
                    "bun_ffi_negative_offset_panic_boundary requires FFI binding edge evidence"
                        .to_string(),
                );
            }
            if !case
                .external_entrypoints
                .iter()
                .any(|entrypoint| entrypoint.contains("read.u8"))
            {
                errors.push(
                    "bun_ffi_negative_offset_panic_boundary must name read.u8 as an external entrypoint"
                        .to_string(),
                );
            }
        }
        "bun_node_fs_scalar_write" => {
            if !case.rust_file.starts_with("unresolved:node-fs-scalar-write") {
                errors.push(
                    "rust_file must keep the Bun node:fs scalar write Rust seam unresolved"
                        .to_string(),
                );
            }
            if case.rust_owner != "node:fs scalar write sink" {
                errors.push(
                    "rust_owner must pin the manifest-only node:fs scalar write sink"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("JS-owned bytes")
                || !case.rust_boundary.contains("native write")
            {
                errors.push(
                    "rust_boundary must describe JS-owned bytes crossing the native write boundary"
                        .to_string(),
                );
            }
            if case.expected_state != "named_static_limitation" {
                errors.push(
                    "bun_node_fs_scalar_write profile is manifest_only and must stay a named_static_limitation"
                        .to_string(),
                );
            }
            if case.profile_status != "manifest_only" {
                errors.push("bun_node_fs_scalar_write must be manifest_only".to_string());
            }
            if case.proof_mode != "observable_red_green" {
                errors.push(
                    "bun_node_fs_scalar_write must record proof_mode observable_red_green"
                        .to_string(),
                );
            }
            if case.binding_edge_kind != "manifest_only"
                || case.binding_edge_confidence != "manifest_only"
            {
                errors.push(
                    "bun_node_fs_scalar_write must keep bridge evidence manifest_only"
                        .to_string(),
                );
            }
            if !case
                .external_entrypoints
                .iter()
                .any(|entrypoint| entrypoint.contains("node:fs"))
            {
                errors.push(
                    "bun_node_fs_scalar_write must name node:fs as an external entrypoint"
                        .to_string(),
                );
            }
            if case.external_callsite_file != "test/js/node/fs/fs.test.ts"
                || case.external_oracle_file != "test/js/node/fs/fs.test.ts"
            {
                errors.push(
                    "bun_node_fs_scalar_write must record test/js/node/fs/fs.test.ts as the witness path"
                        .to_string(),
                );
            }
            if case.external_oracle_kind != "stable_byte_scalar_write_oracle_manifest_only"
                || case.oracle_strength != "manifest_only"
            {
                errors.push(
                    "bun_node_fs_scalar_write must keep scalar-write oracle evidence manifest_only"
                        .to_string(),
                );
            }
        }
        "bun_write_helper_gated" => {
            if !case.rust_file.starts_with("unresolved:bun-write-stable-byte") {
                errors.push(
                    "rust_file must keep the Bun.write stable-byte Rust seam unresolved"
                        .to_string(),
                );
            }
            if case.rust_owner != "Bun.write stable-byte sink" {
                errors.push(
                    "rust_owner must pin the manifest-only Bun.write stable-byte sink"
                        .to_string(),
                );
            }
            if !case.rust_boundary.contains("JS-owned bytes")
                || !case.rust_boundary.contains("Bun.write")
            {
                errors.push(
                    "rust_boundary must describe JS-owned bytes crossing the Bun.write boundary"
                        .to_string(),
                );
            }
            if case.expected_state != "named_static_limitation" {
                errors.push(
                    "bun_write_helper_gated profile is manifest_only and must stay a named_static_limitation"
                        .to_string(),
                );
            }
            if case.profile_status != "manifest_only" {
                errors.push("bun_write_helper_gated must be manifest_only".to_string());
            }
            if case.proof_mode != "helper_gated" {
                errors.push(
                    "bun_write_helper_gated must record proof_mode helper_gated".to_string(),
                );
            }
            if case.binding_edge_kind != "manifest_only"
                || case.binding_edge_confidence != "manifest_only"
            {
                errors.push(
                    "bun_write_helper_gated must keep bridge evidence manifest_only"
                        .to_string(),
                );
            }
            if !case
                .external_entrypoints
                .iter()
                .any(|entrypoint| entrypoint.contains("Bun.write"))
            {
                errors.push(
                    "bun_write_helper_gated must name Bun.write as an external entrypoint"
                        .to_string(),
                );
            }
            if case.external_callsite_file != "test/js/bun/write.test.ts"
                || case.external_oracle_file != "test/js/bun/write.test.ts"
            {
                errors.push(
                    "bun_write_helper_gated must record test/js/bun/write.test.ts as the manifest witness path"
                        .to_string(),
                );
            }
            if case.external_oracle_kind != "stable_byte_write_oracle_helper_gated"
                || case.oracle_strength != "helper_gated"
            {
                errors.push(
                    "bun_write_helper_gated must keep write-oracle evidence helper_gated"
                        .to_string(),
                );
            }
            if !cross_language_oracle_graph_has_missing_leg(
                case,
                "helper:bun_write_fixture_helper",
            ) {
                errors.push(
                    "bun_write_helper_gated must name helper:bun_write_fixture_helper as missing"
                        .to_string(),
                );
            }
            if case.suggested_test_file != "not_applicable" {
                errors.push(
                    "bun_write_helper_gated must not suggest placement while helper-gated"
                        .to_string(),
                );
            }
        }
        other => errors.push(format!(
            "profile must be bun_blob_array_buffer, bun_array_buffer_copy_to_unshared, bun_markdown_resizable_array_buffer, bun_ffi_negative_offset_panic_boundary, bun_node_fs_scalar_write, or bun_write_helper_gated, got {other}"
        )),
    }
}

pub(crate) fn cross_language_oracle_graph_required_non_claims() -> &'static [&'static str] {
    &[
        "provider",
        "source edits",
        "generated tests",
        "runtime Bun execution",
        "mutation execution",
        "default gates",
        "public badge",
        "baseline",
        "RIPR Zero",
        "support-tier promotion",
        "public repair packet",
        "TypeScript Rust parity",
        "full cross-language proof",
        "verify command",
        "receipt command",
        "allowed edit surface",
    ]
}

pub(crate) fn cross_language_oracle_graph_corpus_path() -> PathBuf {
    let relative = Path::new(CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS);
    repo_root()
        .map(|root| root.join(relative))
        .unwrap_or_else(|_| relative.to_path_buf())
}

pub(crate) fn cross_language_oracle_route_quality_report_value() -> Value {
    let path = cross_language_oracle_graph_corpus_path();
    cross_language_oracle_route_quality_from_cases(
        normalize_path(Path::new(CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS)),
        &cross_language_oracle_graph_cases_at(&path),
    )
}

pub(crate) fn cross_language_oracle_route_quality_from_cases(
    source_path: String,
    cases: &[CrossLanguageOracleGraphCase],
) -> Value {
    let mut complete_advisory_witnesses = 0usize;
    let mut missing_discriminator_limitations = 0usize;
    let mut missing_external_oracle_limitations = 0usize;
    let mut bridge_unknown_limitations = 0usize;
    let mut mention_only_limitations = 0usize;
    let mut target_unresolved_limitations = 0usize;
    let mut panic_boundary_limitations = 0usize;
    let mut manifest_only_profiles = 0usize;
    let mut public_packet_exclusions = 0usize;
    let mut public_projection_eligible_cases = 0usize;
    let mut repair_packet_ready_cases = 0usize;
    let mut failing_cases = 0usize;

    let rows = cases
        .iter()
        .map(|case| {
            match case.expected_state.as_str() {
                "rust_ungripped_ts_discriminated" => complete_advisory_witnesses += 1,
                "rust_ungripped_ts_missing_discriminator" => missing_discriminator_limitations += 1,
                "rust_ungripped_ts_missing_external_oracle" => {
                    missing_external_oracle_limitations += 1;
                }
                "bridge_unknown" => bridge_unknown_limitations += 1,
                "ts_mention_not_observer" => mention_only_limitations += 1,
                "cross_language_target_unresolved" => target_unresolved_limitations += 1,
                "public_reachable_panic_boundary_unrevealed" => {
                    panic_boundary_limitations += 1;
                }
                "named_static_limitation" if case.profile_status == "manifest_only" => {
                    manifest_only_profiles += 1;
                }
                _ => {}
            }
            if !case.public_projection_eligible {
                public_packet_exclusions += 1;
            } else {
                public_projection_eligible_cases += 1;
            }
            if case.repair_packet_ready {
                repair_packet_ready_cases += 1;
            }

            let errors = cross_language_oracle_graph_case_errors(case);
            let status = if errors.is_empty() {
                "pass"
            } else {
                failing_cases += 1;
                "fail"
            };
            serde_json::json!({
                "case_id": case.name,
                "profile": case.profile,
                "profile_status": case.profile_status,
                "expected_state": case.expected_state,
                "observed_state": case.expected_state,
                "status": status,
                "gap_state": case.gap_state,
                "limitation_category": case.limitation_category,
                "repair_route": case.repair_route,
                "missing_discriminators": case.missing_discriminators,
                "missing_graph_legs": case.missing_graph_legs,
                "binding_edge_confidence": case.binding_edge_confidence,
                "authority_boundary": case.authority_boundary,
                "public_projection_eligible": case.public_projection_eligible,
                "repair_packet_ready": case.repair_packet_ready,
                "suggested_test_file": case.suggested_test_file,
                "unlock_condition": case.unlock_condition,
                "proof_mode": case.proof_mode,
                "reason": case.reason,
                "errors": errors,
            })
        })
        .collect::<Vec<_>>();
    let passing_cases = cases.len().saturating_sub(failing_cases);
    let status = if cases.is_empty() {
        "empty"
    } else if failing_cases == 0 && repair_packet_ready_cases == 0 {
        "pass"
    } else {
        "fail"
    };

    serde_json::json!({
        "status": status,
        "source_path": source_path,
        "cases_total": cases.len(),
        "passing_cases": passing_cases,
        "failing_cases": failing_cases,
        "cross_language_oracle_graph_complete_advisory_witnesses":
            complete_advisory_witnesses,
        "cross_language_oracle_graph_missing_discriminator_limitations":
            missing_discriminator_limitations,
        "cross_language_oracle_graph_missing_external_oracle_limitations":
            missing_external_oracle_limitations,
        "cross_language_oracle_graph_bridge_unknown_limitations": bridge_unknown_limitations,
        "cross_language_oracle_graph_mention_only_limitations": mention_only_limitations,
        "cross_language_oracle_graph_target_unresolved_limitations":
            target_unresolved_limitations,
        "cross_language_oracle_graph_panic_boundary_limitations":
            panic_boundary_limitations,
        "cross_language_oracle_graph_manifest_only_profiles": manifest_only_profiles,
        "cross_language_oracle_graph_public_packet_exclusions": public_packet_exclusions,
        "public_projection_eligible_cases": public_projection_eligible_cases,
        "repair_packet_ready_cases": repair_packet_ready_cases,
        "authority_boundary": "preview_advisory_only",
        "calibration_boundary": "SPEC-0062 cross-language oracle graph route quality is preview/advisory only; profile-backed rows do not create public repair packets, gates, badges, verify commands, receipt commands, source edits, generated tests, or runtime execution.",
        "non_claims": [
            "not a public repair packet",
            "not badge or gate input",
            "not runtime Bun execution",
            "not generated tests",
            "not source edits",
            "not full cross-language proof",
            "not support-tier promotion"
        ],
        "rows": rows,
    })
}

pub(crate) fn cross_language_oracle_route_quality_push_markdown(out: &mut String, value: &Value) {
    out.push_str("## Cross-Language Oracle Route Quality\n\n");
    out.push_str("This section summarizes the SPEC-0062 profile-backed cross-language oracle graph corpus. It is preview/advisory route-quality evidence only and does not create public repair packets, gates, badges, verify commands, receipt commands, source edits, generated tests, or runtime execution.\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    audit_push_count(
        out,
        "Cases",
        audit_usize(value, &["cases_total"]).unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Passing cases",
        audit_usize(value, &["passing_cases"]).unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Complete advisory witnesses",
        audit_usize(
            value,
            &["cross_language_oracle_graph_complete_advisory_witnesses"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Missing discriminator limitations",
        audit_usize(
            value,
            &["cross_language_oracle_graph_missing_discriminator_limitations"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Missing external oracle limitations",
        audit_usize(
            value,
            &["cross_language_oracle_graph_missing_external_oracle_limitations"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Bridge unknown limitations",
        audit_usize(
            value,
            &["cross_language_oracle_graph_bridge_unknown_limitations"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Mention-only limitations",
        audit_usize(
            value,
            &["cross_language_oracle_graph_mention_only_limitations"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Panic boundary limitations",
        audit_usize(
            value,
            &["cross_language_oracle_graph_panic_boundary_limitations"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Manifest-only profiles",
        audit_usize(
            value,
            &["cross_language_oracle_graph_manifest_only_profiles"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Public packet exclusions",
        audit_usize(
            value,
            &["cross_language_oracle_graph_public_packet_exclusions"],
        )
        .unwrap_or_default(),
    );
    audit_push_count(
        out,
        "Repair-packet-ready cases",
        audit_usize(value, &["repair_packet_ready_cases"]).unwrap_or_default(),
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
        "- source_path: `{}`\n",
        audit_markdown_cell(
            value
                .get("source_path")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- authority_boundary: `{}`\n",
        audit_markdown_cell(
            value
                .get("authority_boundary")
                .and_then(Value::as_str)
                .unwrap_or("preview_advisory_only")
        )
    ));
    out.push_str(&format!(
        "- calibration_boundary: {}\n\n",
        audit_markdown_cell(
            value
                .get("calibration_boundary")
                .and_then(Value::as_str)
                .unwrap_or("preview/advisory only")
        )
    ));
    let non_claims = audit_markdown_string_array_cell(
        value
            .get("non_claims")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice),
    );
    out.push_str(&format!(
        "- non_claims: {}\n\n",
        audit_markdown_cell(&non_claims)
    ));

    out.push_str("| Case | Expected | Observed | Status | Missing discriminators | Missing graph legs | Repair packet ready | Unlock condition |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if rows.is_empty() {
        out.push_str("| none |  |  |  |  |  |  | no corpus rows available |\n");
    }
    for row in rows {
        let case_id = row
            .get("case_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let expected = row
            .get("expected_state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let observed = row
            .get("observed_state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let status = row
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let missing_discriminators = audit_markdown_string_array_cell(
            row.get("missing_discriminators")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        let missing_graph_legs = audit_markdown_string_array_cell(
            row.get("missing_graph_legs")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        );
        let repair_packet_ready = row
            .get("repair_packet_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let unlock_condition = row
            .get("unlock_condition")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} | `{}` | {} |\n",
            audit_markdown_cell(case_id),
            audit_markdown_cell(expected),
            audit_markdown_cell(observed),
            audit_markdown_cell(status),
            audit_markdown_cell(&missing_discriminators),
            audit_markdown_cell(&missing_graph_legs),
            repair_packet_ready,
            audit_markdown_cell(unlock_condition),
        ));
    }
    out.push('\n');
}

pub(crate) fn audit_markdown_string_array_cell(values: &[Value]) -> String {
    let strings = values
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if strings.is_empty() {
        "none".to_string()
    } else {
        strings.join(", ")
    }
}

pub(crate) fn dogfood_bun_ub_cross_language_scenarios() -> Vec<DogfoodBunUbCrossLanguageScenario> {
    dogfood_bun_ub_cross_language_scenarios_at(Path::new(BUN_UB_CROSS_LANGUAGE_DOGFOOD_CORPUS))
}

pub(crate) fn dogfood_bun_ub_cross_language_scenarios_at(
    corpus_path: &Path,
) -> Vec<DogfoodBunUbCrossLanguageScenario> {
    let fallback = |reason: String| {
        vec![DogfoodBunUbCrossLanguageScenario {
            name: "corpus".to_string(),
            source_case: "unknown".to_string(),
            route_quality_case: "unknown".to_string(),
            rust_file: "unknown".to_string(),
            rust_owner: "unknown".to_string(),
            rust_boundary: "unknown".to_string(),
            ts_test_file: "unknown".to_string(),
            expected_state: "unknown".to_string(),
            observed_state: "unknown".to_string(),
            missing_discriminators: Vec::new(),
            missing_graph_legs: Vec::new(),
            suggested_test_file: "unknown".to_string(),
            manual_verdict: "unknown".to_string(),
            operator_action: "unknown".to_string(),
            review_before: "unknown".to_string(),
            review_after: "unknown".to_string(),
            bridge_verdict: "unknown".to_string(),
            placement_verdict: "unknown".to_string(),
            proof_mode: "unknown".to_string(),
            receipt_state: "unknown".to_string(),
            repair_packet_ready: true,
            authority_boundary: "unknown".to_string(),
            raw_evidence_refs: Vec::new(),
            non_claims: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "Bun UB cross-language dogfood corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("bun_ub_cross_language_dogfood_corpus")
    {
        return fallback(
            "Bun UB cross-language dogfood corpus kind must be bun_ub_cross_language_dogfood_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0062") {
        return fallback(
            "Bun UB cross-language dogfood corpus spec must be RIPR-SPEC-0062".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("Bun UB cross-language dogfood corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| DogfoodBunUbCrossLanguageScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            source_case: json_string_field(case, "source_case")
                .unwrap_or_else(|| "unknown".to_string()),
            route_quality_case: json_string_field(case, "route_quality_case")
                .unwrap_or_else(|| "unknown".to_string()),
            rust_file: json_string_field(case, "rust_file")
                .unwrap_or_else(|| "unknown".to_string()),
            rust_owner: json_string_field(case, "rust_owner")
                .unwrap_or_else(|| "unknown".to_string()),
            rust_boundary: json_string_field(case, "rust_boundary")
                .unwrap_or_else(|| "unknown".to_string()),
            ts_test_file: json_string_field(case, "ts_test_file")
                .unwrap_or_else(|| "unknown".to_string()),
            expected_state: json_string_field(case, "expected_state")
                .unwrap_or_else(|| "unknown".to_string()),
            observed_state: json_string_field(case, "observed_state")
                .unwrap_or_else(|| "unknown".to_string()),
            missing_discriminators: json_string_array_field(case, "missing_discriminators"),
            missing_graph_legs: json_string_array_field(case, "missing_graph_legs"),
            suggested_test_file: json_string_field(case, "suggested_test_file")
                .unwrap_or_else(|| "unknown".to_string()),
            manual_verdict: json_string_field(case, "manual_verdict")
                .unwrap_or_else(|| "unknown".to_string()),
            operator_action: json_string_field(case, "operator_action")
                .unwrap_or_else(|| "unknown".to_string()),
            review_before: json_string_field(case, "review_before")
                .unwrap_or_else(|| "unknown".to_string()),
            review_after: json_string_field(case, "review_after")
                .unwrap_or_else(|| "unknown".to_string()),
            bridge_verdict: json_string_field(case, "bridge_verdict")
                .unwrap_or_else(|| "unknown".to_string()),
            placement_verdict: json_string_field(case, "placement_verdict")
                .unwrap_or_else(|| "unknown".to_string()),
            proof_mode: json_string_field(case, "proof_mode")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_state: json_string_field(case, "receipt_state")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_packet_ready: json_bool_field(case, "repair_packet_ready").unwrap_or(true),
            authority_boundary: json_string_field(case, "authority_boundary")
                .unwrap_or_else(|| "unknown".to_string()),
            raw_evidence_refs: json_string_array_field(case, "raw_evidence_refs"),
            non_claims: json_string_array_field(case, "non_claims"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "Bun UB cross-language dogfood case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_bun_ub_cross_language_run(
    scenario: &DogfoodBunUbCrossLanguageScenario,
) -> DogfoodBunUbCrossLanguageRun {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &scenario.name),
        ("source_case", &scenario.source_case),
        ("route_quality_case", &scenario.route_quality_case),
        ("rust_file", &scenario.rust_file),
        ("rust_owner", &scenario.rust_owner),
        ("rust_boundary", &scenario.rust_boundary),
        ("ts_test_file", &scenario.ts_test_file),
        ("expected_state", &scenario.expected_state),
        ("observed_state", &scenario.observed_state),
        ("suggested_test_file", &scenario.suggested_test_file),
        ("manual_verdict", &scenario.manual_verdict),
        ("operator_action", &scenario.operator_action),
        ("review_before", &scenario.review_before),
        ("review_after", &scenario.review_after),
        ("bridge_verdict", &scenario.bridge_verdict),
        ("placement_verdict", &scenario.placement_verdict),
        ("proof_mode", &scenario.proof_mode),
        ("receipt_state", &scenario.receipt_state),
        ("authority_boundary", &scenario.authority_boundary),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }

    if scenario.name.starts_with("bun_blob_") {
        if !scenario.source_case.starts_with("bun_blob_") {
            errors.push("source_case must link to a Bun Blob calibration case".to_string());
        }
        if !scenario.route_quality_case.starts_with("bun_blob_") {
            errors
                .push("route_quality_case must link to a Bun Blob route-quality case".to_string());
        }
        if scenario.rust_file != "src/jsc/Blob.rs" {
            errors.push("rust_file must stay on the calibrated Bun Blob seam".to_string());
        }
        if scenario.rust_owner != "Blob::from_js_without_defer_gc" {
            errors.push("rust_owner must stay on Blob::from_js_without_defer_gc".to_string());
        }
        if scenario.rust_boundary != "array_buffer.shared || array_buffer.resizable" {
            errors.push(
                "rust_boundary must stay on array_buffer.shared || array_buffer.resizable"
                    .to_string(),
            );
        }
        if scenario.ts_test_file != BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE {
            errors.push(
                "ts_test_file must stay on the configured Bun Blob TypeScript test file"
                    .to_string(),
            );
        }
    } else if scenario.name.starts_with("bun_array_buffer_") {
        if scenario.source_case != "bun_array_buffer_copy_to_unshared_configured_bridge_advisory" {
            errors.push(
                "source_case must link to the copy_to_unshared route-quality case".to_string(),
            );
        }
        if scenario.route_quality_case
            != "bun_array_buffer_copy_to_unshared_configured_bridge_advisory"
        {
            errors.push(
                "route_quality_case must link to the copy_to_unshared route-quality case"
                    .to_string(),
            );
        }
        if scenario.rust_file != "src/jsc/array_buffer.rs" {
            errors.push("rust_file must stay on the copy_to_unshared Rust seam".to_string());
        }
        if scenario.rust_owner != "copy_to_unshared" {
            errors.push("rust_owner must stay on copy_to_unshared".to_string());
        }
        if scenario.rust_boundary != "SharedArrayBuffer and resizable ArrayBuffer copy semantics" {
            errors.push(
                "rust_boundary must stay on the copy_to_unshared stable-byte boundary".to_string(),
            );
        }
        if scenario.ts_test_file != BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE {
            errors.push(
                "copy_to_unshared receipt must stay on the Blob TypeScript witness file"
                    .to_string(),
            );
        }
    } else if scenario.name.starts_with("bun_markdown_") {
        if scenario.source_case != "bun_markdown_resizable_array_buffer_configured_bridge_advisory"
        {
            errors
                .push("source_case must link to the MarkdownObject route-quality case".to_string());
        }
        if scenario.route_quality_case
            != "bun_markdown_resizable_array_buffer_configured_bridge_advisory"
        {
            errors.push(
                "route_quality_case must link to the MarkdownObject route-quality case".to_string(),
            );
        }
        if scenario.rust_file != "src/runtime/api/MarkdownObject.rs" {
            errors.push("rust_file must stay on the MarkdownObject Rust seam".to_string());
        }
        if scenario.rust_owner != "MarkdownObject::to_string" {
            errors.push("rust_owner must stay on MarkdownObject::to_string".to_string());
        }
        if scenario.rust_boundary != "self.0.resizable && !self.0.shared" {
            errors.push(
                "rust_boundary must stay on the MarkdownObject resizable boundary".to_string(),
            );
        }
        if scenario.ts_test_file != BUN_MARKDOWN_TS_TEST_FILE {
            errors.push(
                "MarkdownObject receipt must stay on the configured Bun markdown TypeScript test file"
                    .to_string(),
            );
        }
    } else if scenario.name.starts_with("bun_node_fs_") {
        if scenario.source_case != "bun_node_fs_scalar_write_manifest_only_profile" {
            errors.push("source_case must link to the node:fs manifest-only profile".to_string());
        }
        if scenario.route_quality_case != "bun_node_fs_scalar_write_manifest_only_profile" {
            errors.push(
                "route_quality_case must link to the node:fs manifest-only profile".to_string(),
            );
        }
        if scenario.rust_file != "unresolved:node-fs-scalar-write-rust-seam" {
            errors.push(
                "node:fs manifest-only receipt must keep the Rust seam unresolved".to_string(),
            );
        }
        if scenario.rust_owner != "node:fs scalar write sink" {
            errors.push("node:fs manifest-only receipt must keep the owner unresolved".to_string());
        }
        if scenario.rust_boundary
            != "JS-owned bytes must be copied before native write scalar sinks"
        {
            errors.push(
                "node:fs manifest-only receipt must keep the stable-byte boundary text".to_string(),
            );
        }
        if scenario.ts_test_file != BUN_NODE_FS_TS_TEST_FILE {
            errors.push("node:fs manifest-only receipt must name fs.test.ts".to_string());
        }
    } else if scenario.name.starts_with("bun_write_") {
        if scenario.source_case != "bun_write_helper_gated_manifest_only_profile" {
            errors.push("source_case must link to the Bun.write helper-gated profile".to_string());
        }
        if scenario.route_quality_case != "bun_write_helper_gated_manifest_only_profile" {
            errors.push(
                "route_quality_case must link to the Bun.write helper-gated profile".to_string(),
            );
        }
        if scenario.rust_file != "unresolved:bun-write-stable-byte-rust-seam" {
            errors.push(
                "Bun.write helper-gated receipt must keep the Rust seam unresolved".to_string(),
            );
        }
        if scenario.rust_owner != "Bun.write stable-byte sink" {
            errors
                .push("Bun.write helper-gated receipt must keep the owner unresolved".to_string());
        }
        if scenario.rust_boundary
            != "JS-owned bytes must not cross Bun.write native sinks without a helper"
        {
            errors.push(
                "Bun.write helper-gated receipt must keep the stable-byte boundary text"
                    .to_string(),
            );
        }
        if scenario.ts_test_file != BUN_WRITE_TS_TEST_FILE {
            errors.push("Bun.write helper-gated receipt must name write.test.ts".to_string());
        }
    } else if scenario.name.starts_with("bun_ffi_") {
        if !scenario.source_case.starts_with("bun_ffi_") {
            errors.push("source_case must link to a Bun FFI calibration case".to_string());
        }
        if !scenario.route_quality_case.starts_with("bun_ffi_") {
            errors.push("route_quality_case must link to a Bun FFI route-quality case".to_string());
        }
        if scenario.rust_file != "src/bun.js/bindings/FFIObject.rs" {
            errors.push("rust_file must stay on the calibrated Bun FFI seam".to_string());
        }
        if scenario.rust_owner != "FFIObject::read" {
            errors.push("rust_owner must stay on FFIObject::read".to_string());
        }
        if !scenario.rust_boundary.contains("usize::try_from")
            || !scenario.rust_boundary.contains("expect(\"int cast\")")
        {
            errors.push(
                "rust_boundary must stay on the negative-offset FFI panic boundary".to_string(),
            );
        }
        if scenario.ts_test_file != BUN_FFI_NEGATIVE_OFFSET_TS_TEST_SURFACE {
            errors.push(
                "ts_test_file must stay unresolved until a safe FFI TypeScript observer exists"
                    .to_string(),
            );
        }
    } else {
        errors.push(
            "case id must start with a supported Bun cross-language dogfood profile".to_string(),
        );
    }
    if scenario.expected_state != scenario.observed_state {
        errors
            .push("expected_state and observed_state must match for dogfood receipts".to_string());
    }
    if !bun_ub_cross_language_dogfood_allowed_states().contains(&scenario.observed_state.as_str()) {
        errors.push(format!(
            "observed_state must be a Bun UB cross-language dogfood state, got {}",
            scenario.observed_state
        ));
    }
    if scenario.manual_verdict != "agrees" {
        errors.push("manual_verdict must be agrees".to_string());
    }
    if scenario.receipt_state != "closed" {
        errors.push("receipt_state must be closed".to_string());
    }
    if scenario.repair_packet_ready {
        errors.push("repair_packet_ready must remain false".to_string());
    }
    if scenario.authority_boundary != "preview_advisory_only" {
        errors.push("authority_boundary must be preview_advisory_only".to_string());
    }
    if scenario.raw_evidence_refs.len() < 2 {
        errors
            .push("raw_evidence_refs must link calibration and route-quality evidence".to_string());
    }
    for raw_ref in &scenario.raw_evidence_refs {
        if !raw_ref.starts_with("fixtures/") || !raw_ref.contains('#') {
            errors.push(format!(
                "raw_evidence_refs must be normalized fixture fragment refs, got {raw_ref}"
            ));
        }
    }
    if scenario.non_claims.is_empty() {
        errors.push("non_claims must keep preview boundary denials visible".to_string());
    }
    for required in bun_ub_cross_language_dogfood_required_non_claims() {
        if !scenario
            .non_claims
            .iter()
            .any(|non_claim| non_claim.contains(required))
        {
            errors.push(format!("non_claims must deny {required}"));
        }
    }

    match scenario.observed_state.as_str() {
        "rust_ungripped_ts_discriminated" => {
            if !scenario.missing_discriminators.is_empty()
                || !scenario.missing_graph_legs.is_empty()
            {
                errors.push(
                    "rust_ungripped_ts_discriminated must not list missing evidence".to_string(),
                );
            }
            if scenario.operator_action != "no_missing_bridge_discriminator" {
                errors.push(
                    "rust_ungripped_ts_discriminated must use action no_missing_bridge_discriminator"
                        .to_string(),
                );
            }
            if scenario.suggested_test_file != "not_applicable" {
                errors.push(
                    "rust_ungripped_ts_discriminated must not suggest a test file".to_string(),
                );
            }
            if !scenario.bridge_verdict.contains("credited") {
                errors.push(
                    "rust_ungripped_ts_discriminated must document credited bridge evidence"
                        .to_string(),
                );
            }
        }
        "rust_ungripped_ts_missing_discriminator" => {
            if !scenario
                .missing_discriminators
                .iter()
                .any(|missing| missing == "resizable_array_buffer")
            {
                errors.push(
                    "missing-discriminator dogfood must name resizable_array_buffer".to_string(),
                );
            }
            if scenario.suggested_test_file != BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE {
                errors.push(
                    "missing-discriminator dogfood must suggest the configured Bun Blob TypeScript test file"
                        .to_string(),
                );
            }
            if !scenario.operator_action.contains("resizable") {
                errors.push(
                    "missing-discriminator dogfood action must name the missing resizable case"
                        .to_string(),
                );
            }
            if !scenario
                .placement_verdict
                .contains(BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE)
            {
                errors.push(
                    "missing-discriminator dogfood placement must name blob.test.ts".to_string(),
                );
            }
        }
        "ts_mention_not_observer" => {
            if !scenario.missing_discriminators.is_empty() {
                errors.push(
                    "ts_mention_not_observer must not turn token text into a missing discriminator"
                        .to_string(),
                );
            }
            if scenario.suggested_test_file != "not_applicable" {
                errors.push("ts_mention_not_observer must not suggest placement".to_string());
            }
            if scenario.operator_action != "reject_token_mention" {
                errors.push("ts_mention_not_observer must reject the token mention".to_string());
            }
            if !scenario.bridge_verdict.contains("token") {
                errors.push(
                    "ts_mention_not_observer must explain that bridge evidence cannot credit a token mention"
                        .to_string(),
                );
            }
        }
        "bridge_unknown" => {
            if !scenario.missing_discriminators.is_empty() {
                errors.push("bridge_unknown must not report missing discriminators".to_string());
            }
            if !scenario
                .missing_graph_legs
                .iter()
                .any(|missing| missing == "binding_or_ffi_edge")
            {
                errors.push("bridge_unknown must name missing binding_or_ffi_edge".to_string());
            }
            if scenario.suggested_test_file != "not_applicable" {
                errors.push("bridge_unknown must not suggest TypeScript placement".to_string());
            }
            if scenario.operator_action != "inspect_or_add_bridge_evidence" {
                errors.push("bridge_unknown must route to bridge inspection".to_string());
            }
            if scenario.placement_verdict != "not_applicable" {
                errors.push("bridge_unknown placement must remain not_applicable".to_string());
            }
            if !scenario.bridge_verdict.contains("binding_or_ffi_edge") {
                errors.push("bridge_unknown must document the missing bridge leg".to_string());
            }
            if scenario.proof_mode != "bridge_unknown" {
                errors.push("bridge_unknown proof mode must stay bridge_unknown".to_string());
            }
        }
        "named_static_limitation" => {
            if !scenario.missing_discriminators.is_empty() {
                errors.push(
                    "named_static_limitation must not invent missing discriminators".to_string(),
                );
            }
            if scenario.missing_graph_legs.is_empty() {
                errors.push("named_static_limitation must name missing graph legs".to_string());
            }
            if !scenario.bridge_verdict.contains("manifest_only")
                || !scenario.bridge_verdict.contains("not_credited")
            {
                errors.push(
                    "named_static_limitation must keep manifest-only bridge evidence uncredited"
                        .to_string(),
                );
            }
            if scenario.name.starts_with("bun_node_fs_") {
                if !scenario
                    .missing_graph_legs
                    .iter()
                    .any(|missing| missing == "binding_or_ffi_edge:node_fs_scalar_write")
                {
                    errors.push(
                        "node:fs named limitation must keep node_fs_scalar_write bridge leg missing"
                            .to_string(),
                    );
                }
                if !scenario
                    .missing_graph_legs
                    .iter()
                    .any(|missing| missing == "external_oracle:stable_byte_scalar_write")
                {
                    errors.push(
                        "node:fs named limitation must keep scalar-write oracle leg missing"
                            .to_string(),
                    );
                }
                if scenario.suggested_test_file != BUN_NODE_FS_TS_TEST_FILE {
                    errors.push(
                        "node:fs named limitation must preserve the typed witness path".to_string(),
                    );
                }
                if !scenario.placement_verdict.contains("not_actionable") {
                    errors.push(
                        "node:fs named limitation placement must remain non-actionable".to_string(),
                    );
                }
                if scenario.proof_mode != "observable_red_green" {
                    errors.push(
                        "node:fs named limitation proof mode must be observable_red_green"
                            .to_string(),
                    );
                }
            } else if scenario.name.starts_with("bun_write_") {
                for required in [
                    "binding_or_ffi_edge:bun_write_sink",
                    "helper:bun_write_fixture_helper",
                    "external_oracle:stable_byte_write",
                ] {
                    if !scenario
                        .missing_graph_legs
                        .iter()
                        .any(|missing| missing == required)
                    {
                        errors.push(format!(
                            "Bun.write named limitation must keep {required} missing"
                        ));
                    }
                }
                if scenario.suggested_test_file != "not_applicable" {
                    errors.push(
                        "Bun.write helper-gated limitation must not suggest placement".to_string(),
                    );
                }
                if scenario.placement_verdict != "not_applicable" {
                    errors.push(
                        "Bun.write helper-gated placement must remain not_applicable".to_string(),
                    );
                }
                if scenario.proof_mode != "helper_gated" {
                    errors.push("Bun.write proof mode must stay helper_gated".to_string());
                }
            } else {
                errors.push(
                    "named_static_limitation dogfood must use a supported manifest-only profile"
                        .to_string(),
                );
            }
        }
        "public_reachable_panic_boundary_unrevealed" => {
            if !scenario
                .missing_discriminators
                .iter()
                .any(|missing| missing == "negative_offset")
            {
                errors.push(
                    "panic-boundary dogfood must name negative_offset as missing".to_string(),
                );
            }
            if !scenario
                .missing_graph_legs
                .iter()
                .any(|missing| missing == "external_oracle:negative_offset_panic_boundary")
            {
                errors.push(
                    "panic-boundary dogfood must keep the negative-offset oracle unresolved"
                        .to_string(),
                );
            }
            if !scenario
                .missing_graph_legs
                .iter()
                .any(|missing| missing == "safe_external_observer_target")
            {
                errors.push(
                    "panic-boundary dogfood must keep the safe external observer target unresolved"
                        .to_string(),
                );
            }
            if scenario.suggested_test_file != "not_applicable" {
                errors.push("panic-boundary dogfood must not suggest placement".to_string());
            }
            if scenario.operator_action != "keep_panic_boundary_limitation" {
                errors.push(
                    "panic-boundary dogfood action must keep the named limitation".to_string(),
                );
            }
            if scenario.placement_verdict != "not_applicable" {
                errors.push(
                    "panic-boundary dogfood placement must remain not_applicable".to_string(),
                );
            }
            if !scenario.bridge_verdict.contains("ffi") {
                errors.push("panic-boundary dogfood must document the FFI bridge".to_string());
            }
            if !scenario.proof_mode.contains("panic_boundary_limitation") {
                errors.push(
                    "panic-boundary dogfood proof mode must stay a limitation receipt".to_string(),
                );
            }
        }
        _ => {}
    }

    DogfoodBunUbCrossLanguageRun {
        name: scenario.name.clone(),
        source_case: scenario.source_case.clone(),
        route_quality_case: scenario.route_quality_case.clone(),
        rust_file: scenario.rust_file.clone(),
        rust_owner: scenario.rust_owner.clone(),
        rust_boundary: scenario.rust_boundary.clone(),
        ts_test_file: scenario.ts_test_file.clone(),
        expected_state: scenario.expected_state.clone(),
        observed_state: scenario.observed_state.clone(),
        missing_discriminators: scenario.missing_discriminators.clone(),
        missing_graph_legs: scenario.missing_graph_legs.clone(),
        suggested_test_file: scenario.suggested_test_file.clone(),
        manual_verdict: scenario.manual_verdict.clone(),
        operator_action: scenario.operator_action.clone(),
        review_before: scenario.review_before.clone(),
        review_after: scenario.review_after.clone(),
        bridge_verdict: scenario.bridge_verdict.clone(),
        placement_verdict: scenario.placement_verdict.clone(),
        proof_mode: scenario.proof_mode.clone(),
        receipt_state: scenario.receipt_state.clone(),
        repair_packet_ready: scenario.repair_packet_ready,
        authority_boundary: scenario.authority_boundary.clone(),
        raw_evidence_refs: scenario.raw_evidence_refs.clone(),
        non_claims: scenario.non_claims.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn bun_ub_cross_language_dogfood_allowed_states() -> &'static [&'static str] {
    &[
        "rust_ungripped_ts_discriminated",
        "rust_ungripped_ts_missing_discriminator",
        "ts_mention_not_observer",
        "bridge_unknown",
        "named_static_limitation",
        "public_reachable_panic_boundary_unrevealed",
    ]
}

pub(crate) fn bun_ub_cross_language_dogfood_required_non_claims() -> &'static [&'static str] {
    &[
        "no source edits",
        "no generated tests",
        "no runtime Bun execution",
        "no mutation execution",
        "no default gates",
        "badge",
        "baseline",
        "RIPR Zero",
        "support-tier",
        "public repair packet",
        "full cross-language proof",
    ]
}

pub(crate) fn dogfood_typescript_preview_repair_loop_scenarios()
-> Vec<DogfoodTypescriptPreviewRepairLoopScenario> {
    dogfood_typescript_preview_repair_loop_scenarios_at(Path::new(
        TYPESCRIPT_PREVIEW_REPAIR_LOOP_CORPUS,
    ))
}

pub(crate) fn dogfood_typescript_preview_repair_loop_scenarios_at(
    corpus_path: &Path,
) -> Vec<DogfoodTypescriptPreviewRepairLoopScenario> {
    let fallback = |reason: String| {
        vec![DogfoodTypescriptPreviewRepairLoopScenario {
            name: "corpus".to_string(),
            source_fixture: "unknown".to_string(),
            source_finding_id: "unknown".to_string(),
            language: "unknown".to_string(),
            language_status: "unknown".to_string(),
            classification: "unknown".to_string(),
            changed_owner: "unknown".to_string(),
            probe_family: "unknown".to_string(),
            oracle_kind: "unknown".to_string(),
            oracle_strength: "unknown".to_string(),
            gap_state: "unknown".to_string(),
            actionability_category: "unknown".to_string(),
            static_limit_kind: None,
            repair_packet_ready: true,
            must_have_verify_command: false,
            must_have_receipt_command: false,
            must_not_invent_verify_command: false,
            must_not_emit_repair_packet: false,
            authority_boundary: "unknown".to_string(),
            expected_test_or_observer_shape: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            verify_result: "unknown".to_string(),
            receipt_command: "unknown".to_string(),
            receipt_state: "unknown".to_string(),
            outcome: "unknown".to_string(),
            why_not_actionable: "unknown".to_string(),
            repair_route: "unknown".to_string(),
            operator_note: "unknown".to_string(),
            must_not_change: Vec::new(),
            raw_evidence_refs: Vec::new(),
            non_claims: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "TypeScript preview repair-loop corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref()
        != Some("typescript_preview_repair_loop_corpus")
    {
        return fallback(
            "TypeScript preview repair-loop corpus kind must be typescript_preview_repair_loop_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0027") {
        return fallback(
            "TypeScript preview repair-loop corpus spec must be RIPR-SPEC-0027".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback(
            "TypeScript preview repair-loop corpus is missing cases array".to_string(),
        );
    };

    cases
        .iter()
        .map(|case| DogfoodTypescriptPreviewRepairLoopScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            source_fixture: json_string_field(case, "source_fixture")
                .unwrap_or_else(|| "unknown".to_string()),
            source_finding_id: json_string_field(case, "source_finding_id")
                .unwrap_or_else(|| "unknown".to_string()),
            language: json_string_field(case, "language").unwrap_or_else(|| "unknown".to_string()),
            language_status: json_string_field(case, "language_status")
                .unwrap_or_else(|| "unknown".to_string()),
            classification: json_string_field(case, "classification")
                .unwrap_or_else(|| "unknown".to_string()),
            changed_owner: json_string_field(case, "changed_owner")
                .unwrap_or_else(|| "unknown".to_string()),
            probe_family: json_string_field(case, "probe_family")
                .unwrap_or_else(|| "unknown".to_string()),
            oracle_kind: json_string_field(case, "oracle_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            oracle_strength: json_string_field(case, "oracle_strength")
                .unwrap_or_else(|| "unknown".to_string()),
            gap_state: json_string_field(case, "gap_state")
                .unwrap_or_else(|| "unknown".to_string()),
            actionability_category: json_string_field(case, "actionability_category")
                .unwrap_or_else(|| "unknown".to_string()),
            static_limit_kind: json_string_field(case, "static_limit_kind"),
            repair_packet_ready: json_bool_field(case, "repair_packet_ready").unwrap_or(true),
            must_have_verify_command: json_bool_field(case, "must_have_verify_command")
                .unwrap_or(false),
            must_have_receipt_command: json_bool_field(case, "must_have_receipt_command")
                .unwrap_or(false),
            must_not_invent_verify_command: json_bool_field(case, "must_not_invent_verify_command")
                .unwrap_or(false),
            must_not_emit_repair_packet: json_bool_field(case, "must_not_emit_repair_packet")
                .unwrap_or(false),
            authority_boundary: json_string_field(case, "authority_boundary")
                .unwrap_or_else(|| "unknown".to_string()),
            expected_test_or_observer_shape: json_string_field(
                case,
                "expected_test_or_observer_shape",
            )
            .unwrap_or_else(|| "unknown".to_string()),
            verify_command: json_string_field(case, "verify_command")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_result: json_string_field(case, "verify_result")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_command: json_string_field(case, "receipt_command")
                .unwrap_or_else(|| "unknown".to_string()),
            receipt_state: json_string_field(case, "receipt_state")
                .unwrap_or_else(|| "unknown".to_string()),
            outcome: json_string_field(case, "outcome").unwrap_or_else(|| "unknown".to_string()),
            why_not_actionable: json_string_field(case, "why_not_actionable")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_route: json_string_field(case, "repair_route")
                .unwrap_or_else(|| "unknown".to_string()),
            operator_note: json_string_field(case, "operator_note")
                .unwrap_or_else(|| "unknown".to_string()),
            must_not_change: json_string_array_field(case, "must_not_change"),
            raw_evidence_refs: json_string_array_field(case, "raw_evidence_refs"),
            non_claims: json_string_array_field(case, "non_claims"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "TypeScript preview repair-loop case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_typescript_preview_repair_loop_run(
    scenario: &DogfoodTypescriptPreviewRepairLoopScenario,
) -> DogfoodTypescriptPreviewRepairLoopRun {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &scenario.name),
        ("source_fixture", &scenario.source_fixture),
        ("source_finding_id", &scenario.source_finding_id),
        ("language", &scenario.language),
        ("language_status", &scenario.language_status),
        ("classification", &scenario.classification),
        ("changed_owner", &scenario.changed_owner),
        ("probe_family", &scenario.probe_family),
        ("oracle_kind", &scenario.oracle_kind),
        ("oracle_strength", &scenario.oracle_strength),
        ("gap_state", &scenario.gap_state),
        ("actionability_category", &scenario.actionability_category),
        ("authority_boundary", &scenario.authority_boundary),
        (
            "expected_test_or_observer_shape",
            &scenario.expected_test_or_observer_shape,
        ),
        ("verify_command", &scenario.verify_command),
        ("verify_result", &scenario.verify_result),
        ("receipt_command", &scenario.receipt_command),
        ("receipt_state", &scenario.receipt_state),
        ("outcome", &scenario.outcome),
        ("why_not_actionable", &scenario.why_not_actionable),
        ("repair_route", &scenario.repair_route),
        ("operator_note", &scenario.operator_note),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty()
            || (value == "unknown" && !matches!(label, "oracle_kind" | "oracle_strength"))
        {
            errors.push(format!("{label} must be present"));
        }
    }

    if !matches!(scenario.language.as_str(), "typescript" | "javascript") {
        errors.push(format!(
            "language must be typescript or javascript, got {}",
            scenario.language
        ));
    }
    if scenario.language_status != "preview" {
        errors.push("language_status must be preview".to_string());
    }
    if scenario.authority_boundary != "preview_advisory_only" {
        errors.push("authority_boundary must be preview_advisory_only".to_string());
    }
    if !matches!(
        scenario.verify_result.as_str(),
        "pass" | "fail" | "not_run" | "not_applicable"
    ) {
        errors.push(format!(
            "verify_result must be pass, fail, not_run, or not_applicable, got {}",
            scenario.verify_result
        ));
    }
    if scenario.receipt_command == scenario.verify_command {
        errors.push("receipt_command must stay distinct from verify_command".to_string());
    }
    if scenario.must_have_verify_command
        && !typescript_preview_repair_loop_concrete_operator_command(&scenario.verify_command)
    {
        errors.push(
            "must_have_verify_command requires a concrete operator verify_command".to_string(),
        );
    }
    if scenario.must_have_receipt_command
        && !typescript_preview_repair_loop_concrete_operator_command(&scenario.receipt_command)
    {
        errors.push(
            "must_have_receipt_command requires a concrete operator receipt_command".to_string(),
        );
    }
    if scenario.must_not_emit_repair_packet && scenario.repair_packet_ready {
        errors.push("must_not_emit_repair_packet requires repair_packet_ready=false".to_string());
    }
    if scenario.must_not_change.is_empty() {
        errors.push("must_not_change must name bounded edit constraints".to_string());
    }
    if scenario.raw_evidence_refs.is_empty() {
        errors.push("raw_evidence_refs must keep lineage to preview evidence".to_string());
    }
    if scenario.non_claims.is_empty() {
        errors.push("non_claims must keep preview boundary denials visible".to_string());
    }
    for required in typescript_preview_repair_loop_required_non_claims() {
        if !scenario
            .non_claims
            .iter()
            .any(|non_claim| non_claim.contains(required))
        {
            errors.push(format!("non_claims must deny {required}"));
        }
    }
    if !scenario.source_fixture.starts_with("fixtures/")
        || scenario.source_fixture.contains("..")
        || scenario.source_fixture.contains('\\')
    {
        errors.push(format!(
            "source_fixture must be a normalized fixtures/ path, got {}",
            scenario.source_fixture
        ));
    }
    if !scenario.repair_packet_ready && scenario.gap_state == "actionable" {
        errors.push(
            "repair_packet_ready=false must not be paired with gap_state=actionable".to_string(),
        );
    }
    if !scenario.repair_packet_ready && scenario.outcome == "resolved" {
        errors.push("repair_packet_ready=false must not claim resolved".to_string());
    }
    if scenario.repair_packet_ready {
        if scenario.gap_state != "actionable"
            || scenario.actionability_category != "complete_repair_packet"
        {
            errors.push(
                "repair_packet_ready=true requires actionable / complete_repair_packet".to_string(),
            );
        }
        if scenario.outcome != "resolved" {
            errors.push("repair_packet_ready=true requires outcome resolved".to_string());
        }
        if scenario.verify_result != "pass" {
            errors.push("repair_packet_ready=true requires verify_result=pass".to_string());
        }
        if scenario.must_not_emit_repair_packet {
            errors.push(
                "repair_packet_ready=true cannot set must_not_emit_repair_packet".to_string(),
            );
        }
    }
    if !typescript_preview_repair_loop_allowed_outcomes().contains(&scenario.outcome.as_str()) {
        errors.push(format!(
            "outcome must be a TypeScript preview repair-loop outcome, got {}",
            scenario.outcome
        ));
    }
    if scenario.static_limit_kind.is_some() && scenario.gap_state != "static_limitation" {
        errors.push("static_limit_kind requires gap_state=static_limitation".to_string());
    }
    if scenario.gap_state == "static_limitation" {
        if scenario
            .static_limit_kind
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            errors.push("static_limitation cases must name static_limit_kind".to_string());
        }
        if scenario.outcome != "static_limitation_recorded" {
            errors.push(
                "static_limitation cases must use outcome static_limitation_recorded".to_string(),
            );
        }
    }
    if scenario.outcome == "weak_oracle_downgraded" {
        if matches!(
            scenario.oracle_kind.as_str(),
            "exact_value" | "exact_error_variant"
        ) || scenario.oracle_strength == "strong"
        {
            errors.push("weak_oracle_downgraded must not use a strong oracle".to_string());
        }
        if scenario.gap_state == "already_observed"
            || scenario.actionability_category == "strong_oracle_observed"
        {
            errors.push(
                "weak_oracle_downgraded must not claim already-observed actionability".to_string(),
            );
        }
    }
    if scenario.outcome == "already_observed_unchanged"
        && (scenario.gap_state != "already_observed"
            || scenario.actionability_category != "strong_oracle_observed")
    {
        errors.push(
            "already_observed_unchanged must preserve already_observed / strong_oracle_observed"
                .to_string(),
        );
    }
    if scenario.outcome == "resolved" {
        dogfood_typescript_preview_repair_loop_check_closed_receipt(scenario, &mut errors);
    }

    dogfood_typescript_preview_repair_loop_check_source_fixture(scenario, &mut errors);

    DogfoodTypescriptPreviewRepairLoopRun {
        name: scenario.name.clone(),
        source_fixture: scenario.source_fixture.clone(),
        source_finding_id: scenario.source_finding_id.clone(),
        language: scenario.language.clone(),
        classification: scenario.classification.clone(),
        changed_owner: scenario.changed_owner.clone(),
        probe_family: scenario.probe_family.clone(),
        oracle_kind: scenario.oracle_kind.clone(),
        oracle_strength: scenario.oracle_strength.clone(),
        gap_state: scenario.gap_state.clone(),
        actionability_category: scenario.actionability_category.clone(),
        static_limit_kind: scenario.static_limit_kind.clone(),
        repair_packet_ready: scenario.repair_packet_ready,
        expected_test_or_observer_shape: scenario.expected_test_or_observer_shape.clone(),
        verify_command: scenario.verify_command.clone(),
        verify_result: scenario.verify_result.clone(),
        receipt_command: scenario.receipt_command.clone(),
        receipt_state: scenario.receipt_state.clone(),
        outcome: scenario.outcome.clone(),
        why_not_actionable: scenario.why_not_actionable.clone(),
        repair_route: scenario.repair_route.clone(),
        operator_note: scenario.operator_note.clone(),
        must_not_change: scenario.must_not_change.clone(),
        raw_evidence_refs: scenario.raw_evidence_refs.clone(),
        non_claims: scenario.non_claims.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_typescript_preview_repair_loop_check_source_fixture(
    scenario: &DogfoodTypescriptPreviewRepairLoopScenario,
    errors: &mut Vec<String>,
) {
    if !scenario.source_fixture.starts_with("fixtures/") || scenario.source_fixture.contains("..") {
        return;
    }
    let check_path = Path::new(&scenario.source_fixture)
        .join("expected")
        .join("check.json");
    let report = match read_json_value(&check_path) {
        Ok(value) => value,
        Err(err) => {
            errors.push(format!(
                "source fixture check output is unavailable at {}: {err}",
                normalize_path(&check_path)
            ));
            return;
        }
    };
    let finding = report
        .get("findings")
        .and_then(Value::as_array)
        .and_then(|findings| {
            findings.iter().find(|finding| {
                json_string_field(finding, "id").as_deref()
                    == Some(scenario.source_finding_id.as_str())
            })
        });
    let Some(finding) = finding else {
        errors.push(format!(
            "source fixture {} does not contain finding {}",
            normalize_path(&check_path),
            scenario.source_finding_id
        ));
        return;
    };

    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        finding,
        "classification",
        &scenario.classification,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        finding,
        "language",
        &scenario.language,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        finding,
        "language_status",
        &scenario.language_status,
    );
    if json_string_field(finding, "oracle_kind").as_deref() != Some(scenario.oracle_kind.as_str()) {
        errors.push(format!(
            "source finding oracle_kind must be {}, got {:?}",
            scenario.oracle_kind,
            json_string_field(finding, "oracle_kind")
        ));
    }
    if json_string_field(finding, "oracle_strength").as_deref()
        != Some(scenario.oracle_strength.as_str())
    {
        errors.push(format!(
            "source finding oracle_strength must be {}, got {:?}",
            scenario.oracle_strength,
            json_string_field(finding, "oracle_strength")
        ));
    }
    if json_string_field(finding, "static_limit_kind") != scenario.static_limit_kind {
        errors.push(format!(
            "source finding static_limit_kind must be {:?}, got {:?}",
            scenario.static_limit_kind,
            json_string_field(finding, "static_limit_kind")
        ));
    }
    if !scenario
        .raw_evidence_refs
        .iter()
        .any(|reference| reference.contains(&scenario.source_finding_id))
    {
        errors.push("raw_evidence_refs must include the source finding id".to_string());
    }

    let Some(probe) = finding.get("probe") else {
        errors.push("source finding is missing probe".to_string());
        return;
    };
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        probe,
        "family",
        &scenario.probe_family,
    );
    let source_owner = json_string_field(probe, "owner").unwrap_or_default();
    if !source_owner.ends_with(&format!("::{}", scenario.changed_owner)) {
        errors.push(format!(
            "source finding owner `{source_owner}` must end with ::{}",
            scenario.changed_owner
        ));
    }

    let Some(actionability) = finding.get("preview_actionability") else {
        errors.push("source finding is missing preview_actionability".to_string());
        return;
    };
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "gap_state",
        &scenario.gap_state,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "actionability_category",
        &scenario.actionability_category,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "authority_boundary",
        &scenario.authority_boundary,
    );
    if json_bool_field(actionability, "repair_packet_ready") != Some(scenario.repair_packet_ready) {
        errors.push(format!(
            "source finding repair_packet_ready must be {}, got {:?}",
            scenario.repair_packet_ready,
            json_bool_field(actionability, "repair_packet_ready")
        ));
    }
    if scenario.must_not_emit_repair_packet {
        if json_bool_field(actionability, "repair_packet_ready") != Some(false) {
            errors.push(
                "must_not_emit_repair_packet requires source preview_actionability repair_packet_ready=false"
                    .to_string(),
            );
        }
        if finding
            .get("typescript_preview_card")
            .and_then(|card| json_bool_field(card, "repair_packet_ready"))
            != Some(false)
        {
            errors.push(
                "must_not_emit_repair_packet requires source preview card repair_packet_ready=false"
                    .to_string(),
            );
        }
    }
    if scenario.repair_packet_ready {
        if finding.get("typescript_repair_packet").is_none() {
            errors.push(
                "repair_packet_ready=true requires source typescript_repair_packet".to_string(),
            );
        }
        if finding
            .get("typescript_preview_card")
            .and_then(|card| json_bool_field(card, "repair_packet_ready"))
            != Some(true)
        {
            errors.push(
                "repair_packet_ready=true requires source preview card repair_packet_ready=true"
                    .to_string(),
            );
        }
        if finding
            .get("typescript_repair_packet")
            .and_then(|packet| json_string_field(packet, "verify_command"))
            .as_deref()
            != Some(scenario.verify_command.as_str())
        {
            errors.push(
                "repair_packet_ready=true requires verify_command to match source packet"
                    .to_string(),
            );
        }
    }
    if scenario.must_not_invent_verify_command {
        if json_string_array_field(finding, "evidence")
            .iter()
            .any(|line| line.starts_with("typescript_verify_command:"))
        {
            errors.push(
                "must_not_invent_verify_command rejects source typescript_verify_command evidence"
                    .to_string(),
            );
        }
        if let Some(command) = finding
            .get("typescript_preview_card")
            .and_then(|card| card.get("verify"))
            .and_then(|verify| json_string_field(verify, "command"))
            .filter(|command| !command.trim().is_empty())
        {
            errors.push(format!(
                "must_not_invent_verify_command requires source preview card verify.command to stay absent, got {command}"
            ));
        }
    }
    if scenario.repair_packet_ready {
        let source_why = json_string_field(actionability, "why_not_actionable").unwrap_or_default();
        if !source_why.contains("complete repair packet")
            || !scenario
                .why_not_actionable
                .contains("complete repair packet")
        {
            errors.push(
                "packet-ready why_not_actionable must document the complete repair packet boundary"
                    .to_string(),
            );
        }
    } else if json_string_field(actionability, "why_not_actionable").as_deref()
        != Some(scenario.why_not_actionable.as_str())
    {
        errors.push("why_not_actionable must match source preview actionability".to_string());
    }
    if json_string_field(actionability, "repair_route").as_deref()
        != Some(scenario.repair_route.as_str())
    {
        errors.push("repair_route must match source preview actionability".to_string());
    }
}

pub(crate) fn typescript_preview_repair_loop_concrete_operator_command(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && !matches!(
            trimmed,
            "unknown"
                | "not_applicable"
                | "verify_command_unknown"
                | "receipt_command_unknown"
                | "command_unknown"
        )
}

pub(crate) fn dogfood_typescript_preview_repair_loop_check_closed_receipt(
    scenario: &DogfoodTypescriptPreviewRepairLoopScenario,
    errors: &mut Vec<String>,
) {
    if !scenario.receipt_command.starts_with("ripr outcome ") {
        errors.push("resolved TypeScript preview receipt must use ripr outcome".to_string());
    }
    let Some(receipt_ref) = scenario.raw_evidence_refs.iter().find(|reference| {
        reference.starts_with(
            "fixtures/first_successful_pr/typescript-preview-gap/expected/outcome/closed.json",
        )
    }) else {
        errors.push(
            "resolved TypeScript preview receipt must cite the closed outcome fixture".to_string(),
        );
        return;
    };
    let receipt_path = receipt_ref.split('#').next().unwrap_or(receipt_ref);
    let receipt = match read_json_value(Path::new(receipt_path)) {
        Ok(value) => value,
        Err(err) => {
            errors.push(format!(
                "resolved TypeScript preview receipt is unavailable at {receipt_path}: {err}"
            ));
            return;
        }
    };
    if json_string_field(&receipt, "status").as_deref() != Some("advisory") {
        errors.push("resolved TypeScript preview receipt must stay advisory".to_string());
    }
    let closed = receipt
        .get("summary")
        .and_then(|summary| summary.get("gap_movement"))
        .and_then(|movement| movement.get("closed"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if closed == 0 {
        errors.push("resolved TypeScript preview receipt must close at least one gap".to_string());
    }
    let Some(expected_gap) =
        typescript_preview_repair_loop_expected_gap_id(&scenario.source_finding_id)
    else {
        errors.push(format!(
            "resolved TypeScript preview receipt cannot derive a canonical gap id from {}",
            scenario.source_finding_id
        ));
        return;
    };
    let moved_contains_gap = receipt
        .get("moved")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|movement| {
            json_string_field(movement, "seam_id").as_deref() == Some(expected_gap.as_str())
                && json_string_field(movement, "gap_movement").as_deref() == Some("closed")
        });
    if !moved_contains_gap {
        errors.push(format!(
            "resolved TypeScript preview receipt must close {expected_gap}"
        ));
    }
}

pub(crate) fn typescript_preview_repair_loop_expected_gap_id(
    source_finding_id: &str,
) -> Option<String> {
    let mut parts = source_finding_id.rsplit(':');
    let digest = parts.next()?.trim();
    let family = parts.next()?.trim();
    if digest.is_empty() || family.is_empty() {
        return None;
    }
    Some(format!("gap:typescript:{family}:{digest}"))
}

pub(crate) fn dogfood_typescript_preview_repair_loop_expect_string(
    errors: &mut Vec<String>,
    value: &Value,
    field: &str,
    expected: &str,
) {
    if json_string_field(value, field).as_deref() != Some(expected) {
        errors.push(format!(
            "source finding {field} must be {expected}, got {:?}",
            json_string_field(value, field)
        ));
    }
}

pub(crate) fn typescript_preview_repair_loop_allowed_outcomes() -> &'static [&'static str] {
    &[
        "proof_improved",
        "weak_oracle_downgraded",
        "static_limitation_recorded",
        "already_observed_unchanged",
        "intentionally_skipped",
        "resolved",
    ]
}

pub(crate) fn typescript_preview_repair_loop_required_non_claims() -> &'static [&'static str] {
    &[
        "provider",
        "source edits",
        "generated tests",
        "runtime Jest/Vitest execution",
        "mutation execution",
        "default gates",
        "public badge",
        "baseline",
        "RIPR Zero",
        "support-tier promotion",
    ]
}

#[cfg(test)]
pub(crate) fn typescript_preview_false_actionable_audit_cases()
-> Vec<TypeScriptPreviewFalseActionableAuditCase> {
    typescript_preview_false_actionable_audit_cases_at(Path::new(
        TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS,
    ))
}

pub(crate) fn typescript_preview_false_actionable_audit_cases_at(
    corpus_path: &Path,
) -> Vec<TypeScriptPreviewFalseActionableAuditCase> {
    let fallback = |reason: String| {
        vec![TypeScriptPreviewFalseActionableAuditCase {
            name: "corpus".to_string(),
            source_fixture: "unknown".to_string(),
            source_finding_id: "unknown".to_string(),
            language: "unknown".to_string(),
            language_status: "unknown".to_string(),
            risk_class: "unknown".to_string(),
            evidence_kind: "unknown".to_string(),
            oracle_kind: None,
            gap_state: "unknown".to_string(),
            actionability_category: "unknown".to_string(),
            static_limit_kind: None,
            disposition: "unknown".to_string(),
            repair_packet_ready: true,
            authority_boundary: "unknown".to_string(),
            why_not_actionable: "unknown".to_string(),
            repair_route: "unknown".to_string(),
            future_support: "unknown".to_string(),
            must_remain_non_actionable: false,
            required_evidence_fragment: "unknown".to_string(),
            raw_evidence_refs: Vec::new(),
            non_claims: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "TypeScript preview false-actionable audit corpus schema_version must be 0.1"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref()
        != Some("typescript_preview_false_actionable_audit_corpus")
    {
        return fallback(
            "TypeScript preview false-actionable audit corpus kind must be typescript_preview_false_actionable_audit_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0027") {
        return fallback(
            "TypeScript preview false-actionable audit corpus spec must be RIPR-SPEC-0027"
                .to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback(
            "TypeScript preview false-actionable audit corpus is missing cases array".to_string(),
        );
    };

    cases
        .iter()
        .map(|case| TypeScriptPreviewFalseActionableAuditCase {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            source_fixture: json_string_field(case, "source_fixture")
                .unwrap_or_else(|| "unknown".to_string()),
            source_finding_id: json_string_field(case, "source_finding_id")
                .unwrap_or_else(|| "unknown".to_string()),
            language: json_string_field(case, "language").unwrap_or_else(|| "unknown".to_string()),
            language_status: json_string_field(case, "language_status")
                .unwrap_or_else(|| "unknown".to_string()),
            risk_class: json_string_field(case, "risk_class")
                .unwrap_or_else(|| "unknown".to_string()),
            evidence_kind: json_string_field(case, "evidence_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            oracle_kind: json_string_field(case, "oracle_kind"),
            gap_state: json_string_field(case, "gap_state")
                .unwrap_or_else(|| "unknown".to_string()),
            actionability_category: json_string_field(case, "actionability_category")
                .unwrap_or_else(|| "unknown".to_string()),
            static_limit_kind: json_string_field(case, "static_limit_kind"),
            disposition: json_string_field(case, "disposition")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_packet_ready: json_bool_field(case, "repair_packet_ready").unwrap_or(true),
            authority_boundary: json_string_field(case, "authority_boundary")
                .unwrap_or_else(|| "unknown".to_string()),
            why_not_actionable: json_string_field(case, "why_not_actionable")
                .unwrap_or_else(|| "unknown".to_string()),
            repair_route: json_string_field(case, "repair_route")
                .unwrap_or_else(|| "unknown".to_string()),
            future_support: json_string_field(case, "future_support")
                .unwrap_or_else(|| "unknown".to_string()),
            must_remain_non_actionable: json_bool_field(case, "must_remain_non_actionable")
                .unwrap_or(false),
            required_evidence_fragment: json_string_field(case, "required_evidence_fragment")
                .unwrap_or_else(|| "unknown".to_string()),
            raw_evidence_refs: json_string_array_field(case, "raw_evidence_refs"),
            non_claims: json_string_array_field(case, "non_claims"),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "TypeScript preview false-actionable audit case did not document a reason"
                    .to_string()
            }),
        })
        .collect()
}

pub(crate) fn typescript_preview_false_actionable_audit_case_errors(
    case: &TypeScriptPreviewFalseActionableAuditCase,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (label, value) in [
        ("case id", &case.name),
        ("source_fixture", &case.source_fixture),
        ("source_finding_id", &case.source_finding_id),
        ("language", &case.language),
        ("language_status", &case.language_status),
        ("risk_class", &case.risk_class),
        ("evidence_kind", &case.evidence_kind),
        ("gap_state", &case.gap_state),
        ("actionability_category", &case.actionability_category),
        ("disposition", &case.disposition),
        ("authority_boundary", &case.authority_boundary),
        ("why_not_actionable", &case.why_not_actionable),
        ("repair_route", &case.repair_route),
        ("future_support", &case.future_support),
        (
            "required_evidence_fragment",
            &case.required_evidence_fragment,
        ),
        ("reason", &case.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }

    if !matches!(case.language.as_str(), "typescript" | "javascript") {
        errors.push(format!(
            "language must be typescript or javascript, got {}",
            case.language
        ));
    }
    if case.language_status != "preview" {
        errors.push("language_status must be preview".to_string());
    }
    if case.authority_boundary != "preview_advisory_only" {
        errors.push("authority_boundary must be preview_advisory_only".to_string());
    }
    if !typescript_preview_false_actionable_audit_allowed_dispositions()
        .contains(&case.disposition.as_str())
    {
        errors.push(format!(
            "disposition must be a TypeScript preview false-actionable audit disposition, got {}",
            case.disposition
        ));
    }
    if case.repair_packet_ready {
        errors.push("repair_packet_ready must remain false for audit cases".to_string());
    }
    if case.gap_state == "actionable" {
        errors.push("audit cases must not be actionable".to_string());
    }
    if !case.must_remain_non_actionable {
        errors.push("must_remain_non_actionable must be true".to_string());
    }
    if case.raw_evidence_refs.is_empty() {
        errors.push("raw_evidence_refs must keep lineage to preview evidence".to_string());
    }
    if case.non_claims.is_empty() {
        errors.push("non_claims must keep preview boundary denials visible".to_string());
    }
    for required in typescript_preview_repair_loop_required_non_claims() {
        if !case
            .non_claims
            .iter()
            .any(|non_claim| non_claim.contains(required))
        {
            errors.push(format!("non_claims must deny {required}"));
        }
    }
    if !case.source_fixture.starts_with("fixtures/")
        || case.source_fixture.contains("..")
        || case.source_fixture.contains('\\')
    {
        errors.push(format!(
            "source_fixture must be a normalized fixtures/ path, got {}",
            case.source_fixture
        ));
    }
    if case.static_limit_kind.is_some() && case.gap_state != "static_limitation" {
        errors.push("static_limit_kind requires gap_state=static_limitation".to_string());
    }
    if case.gap_state == "static_limitation" && case.static_limit_kind.is_none() {
        errors.push("static_limitation cases must name static_limit_kind".to_string());
    }
    if case.disposition == "named_static_limitation" && case.gap_state != "static_limitation" {
        errors.push(
            "named_static_limitation disposition requires gap_state=static_limitation".to_string(),
        );
    }
    if case.disposition != "named_static_limitation" && case.gap_state == "static_limitation" {
        errors.push(
            "static_limitation gap_state must use named_static_limitation disposition".to_string(),
        );
    }

    typescript_preview_false_actionable_audit_check_source_fixture(case, &mut errors);
    errors
}

pub(crate) fn typescript_preview_false_actionable_audit_check_source_fixture(
    case: &TypeScriptPreviewFalseActionableAuditCase,
    errors: &mut Vec<String>,
) {
    if !case.source_fixture.starts_with("fixtures/") || case.source_fixture.contains("..") {
        return;
    }
    let check_path = Path::new(&case.source_fixture)
        .join("expected")
        .join("check.json");
    let report = match read_json_value(&check_path) {
        Ok(value) => value,
        Err(err) => {
            errors.push(format!(
                "source fixture check output is unavailable at {}: {err}",
                normalize_path(&check_path)
            ));
            return;
        }
    };
    let finding = report
        .get("findings")
        .and_then(Value::as_array)
        .and_then(|findings| {
            findings.iter().find(|finding| {
                json_string_field(finding, "id").as_deref() == Some(case.source_finding_id.as_str())
            })
        });
    let Some(finding) = finding else {
        errors.push(format!(
            "source fixture {} does not contain finding {}",
            normalize_path(&check_path),
            case.source_finding_id
        ));
        return;
    };

    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        finding,
        "language",
        &case.language,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        finding,
        "language_status",
        &case.language_status,
    );
    if let Some(expected_oracle) = &case.oracle_kind {
        dogfood_typescript_preview_repair_loop_expect_string(
            errors,
            finding,
            "oracle_kind",
            expected_oracle,
        );
    }
    if json_string_field(finding, "static_limit_kind") != case.static_limit_kind {
        errors.push(format!(
            "source finding static_limit_kind must be {:?}, got {:?}",
            case.static_limit_kind,
            json_string_field(finding, "static_limit_kind")
        ));
    }
    if !case
        .raw_evidence_refs
        .iter()
        .any(|reference| reference.contains(&case.source_finding_id))
    {
        errors.push("raw_evidence_refs must include the source finding id".to_string());
    }
    if !typescript_preview_false_actionable_finding_contains(
        finding,
        &case.required_evidence_fragment,
    ) {
        errors.push(format!(
            "source finding must contain required evidence fragment `{}`",
            case.required_evidence_fragment
        ));
    }

    let Some(actionability) = finding.get("preview_actionability") else {
        errors.push("source finding is missing preview_actionability".to_string());
        return;
    };
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "gap_state",
        &case.gap_state,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "actionability_category",
        &case.actionability_category,
    );
    dogfood_typescript_preview_repair_loop_expect_string(
        errors,
        actionability,
        "authority_boundary",
        &case.authority_boundary,
    );
    if json_bool_field(actionability, "repair_packet_ready") != Some(case.repair_packet_ready) {
        errors.push(format!(
            "source finding repair_packet_ready must be {}, got {:?}",
            case.repair_packet_ready,
            json_bool_field(actionability, "repair_packet_ready")
        ));
    }
    if json_string_field(actionability, "why_not_actionable").as_deref()
        != Some(case.why_not_actionable.as_str())
    {
        errors.push("why_not_actionable must match source preview actionability".to_string());
    }
    if json_string_field(actionability, "repair_route").as_deref()
        != Some(case.repair_route.as_str())
    {
        errors.push("repair_route must match source preview actionability".to_string());
    }
}

pub(crate) fn typescript_preview_false_actionable_finding_contains(
    finding: &Value,
    fragment: &str,
) -> bool {
    if fragment.trim().is_empty() {
        return false;
    }
    if json_string_field(finding, "recommended_next_step")
        .as_deref()
        .is_some_and(|value| value.contains(fragment))
    {
        return true;
    }
    for field in ["evidence", "missing"] {
        if finding
            .get(field)
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|item| item.contains(fragment))
            })
        {
            return true;
        }
    }
    false
}

pub(crate) fn typescript_preview_false_actionable_audit_allowed_dispositions()
-> &'static [&'static str] {
    &[
        "safe_advisory",
        "named_static_limitation",
        "candidate_future_support",
        "must_remain_non_actionable",
    ]
}

pub(crate) fn dogfood_user_surface_projection_scenarios()
-> Vec<DogfoodUserSurfaceProjectionScenario> {
    let corpus_path = Path::new(USER_SURFACE_PROJECTION_ALIGNMENT_CORPUS);
    let fallback = |reason: String| {
        vec![DogfoodUserSurfaceProjectionScenario {
            name: "corpus".to_string(),
            surface: "unknown".to_string(),
            artifact: "unknown".to_string(),
            headline: "unknown".to_string(),
            run_status: "unknown".to_string(),
            projection_basis: "unknown".to_string(),
            canonical_gap_id: "unknown".to_string(),
            packet_id: "unknown".to_string(),
            repair_kind: "unknown".to_string(),
            top_next_action_kind: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            receipt_command: "unknown".to_string(),
            source_alignment_case: "unknown".to_string(),
            limitation_category: "unknown".to_string(),
            runtime_repair_command: "unknown".to_string(),
            actionable_count: 0,
            raw_findings_total: 0,
            consumes_canonical_state: false,
            reinterprets_raw_findings: true,
            raw_findings_headline: true,
            advisory: false,
            blocking_default: true,
            limited_state_visible: false,
            stale_state_visible: false,
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "user surface projection alignment corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref()
        != Some("user_surface_projection_alignment_corpus")
    {
        return fallback(
            "user surface projection alignment corpus kind must be user_surface_projection_alignment_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0059") {
        return fallback(
            "user surface projection alignment corpus spec must be RIPR-SPEC-0059".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback(
            "user surface projection alignment corpus is missing cases array".to_string(),
        );
    };

    cases
        .iter()
        .map(|case| DogfoodUserSurfaceProjectionScenario {
            name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
            surface: json_string_field(case, "surface").unwrap_or_else(|| "unknown".to_string()),
            artifact: json_string_field(case, "artifact").unwrap_or_else(|| "unknown".to_string()),
            headline: json_string_field(case, "headline").unwrap_or_else(|| "unknown".to_string()),
            run_status: json_string_field(case, "run_status")
                .unwrap_or_else(|| "unknown".to_string()),
            projection_basis: json_string_field(case, "projection_basis")
                .unwrap_or_else(|| "unknown".to_string()),
            canonical_gap_id: json_string_field(case, "canonical_gap_id").unwrap_or_default(),
            packet_id: json_string_field(case, "packet_id").unwrap_or_default(),
            repair_kind: json_string_field(case, "repair_kind").unwrap_or_default(),
            top_next_action_kind: json_string_field(case, "top_next_action_kind")
                .unwrap_or_else(|| "unknown".to_string()),
            verify_command: json_string_field(case, "verify_command").unwrap_or_default(),
            receipt_command: json_string_field(case, "receipt_command").unwrap_or_default(),
            source_alignment_case: json_string_field(case, "source_alignment_case")
                .unwrap_or_default(),
            limitation_category: json_string_field(case, "limitation_category").unwrap_or_default(),
            runtime_repair_command: json_string_field(case, "runtime_repair_command")
                .unwrap_or_default(),
            actionable_count: json_usize_field(case, "actionable_count").unwrap_or(0),
            raw_findings_total: json_usize_field(case, "raw_findings_total").unwrap_or(0),
            consumes_canonical_state: case
                .get("consumes_canonical_state")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            reinterprets_raw_findings: case
                .get("reinterprets_raw_findings")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            raw_findings_headline: case
                .get("raw_findings_headline")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            advisory: case
                .get("advisory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            blocking_default: case
                .get("blocking_default")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            limited_state_visible: case
                .get("limited_state_visible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            stale_state_visible: case
                .get("stale_state_visible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            reason: json_string_field(case, "reason").unwrap_or_else(|| {
                "user surface projection alignment case did not document a reason".to_string()
            }),
        })
        .collect()
}

pub(crate) fn dogfood_user_surface_projection_run(
    scenario: &DogfoodUserSurfaceProjectionScenario,
) -> DogfoodUserSurfaceProjectionRun {
    let mut errors = Vec::new();
    if scenario.name.trim().is_empty() || scenario.name == "unknown" {
        errors.push("case id must be present".to_string());
    }
    if !USER_SURFACE_PROJECTION_REQUIRED_SURFACES.contains(&scenario.surface.as_str()) {
        errors.push(format!("unsupported surface {}", scenario.surface));
    }
    for (label, value) in [
        ("artifact", &scenario.artifact),
        ("headline", &scenario.headline),
        ("run_status", &scenario.run_status),
        ("projection_basis", &scenario.projection_basis),
        ("top_next_action_kind", &scenario.top_next_action_kind),
        ("reason", &scenario.reason),
    ] {
        if value.trim().is_empty() || value == "unknown" {
            errors.push(format!("{label} must be present"));
        }
    }
    if !scenario.consumes_canonical_state {
        errors.push("surface must consume canonical state".to_string());
    }
    if scenario.reinterprets_raw_findings {
        errors.push("surface must not reinterpret raw findings".to_string());
    }
    if scenario.raw_findings_headline {
        errors.push("surface must not headline raw findings".to_string());
    }
    if !scenario.advisory {
        errors.push("surface must remain advisory".to_string());
    }
    if scenario.blocking_default {
        errors.push("surface must not be blocking by default".to_string());
    }
    if !scenario.verify_command.trim().is_empty()
        && scenario.verify_command == scenario.receipt_command
    {
        errors.push("receipt_command must stay distinct from verify_command".to_string());
    }
    if !scenario.limited_state_visible {
        errors.push("surface must make limited state visible".to_string());
    }
    if !scenario.stale_state_visible {
        errors.push("surface must make stale state visible".to_string());
    }
    errors.extend(dogfood_user_surface_projection_runtime_state_errors(
        scenario,
    ));

    DogfoodUserSurfaceProjectionRun {
        name: scenario.name.clone(),
        surface: scenario.surface.clone(),
        artifact: scenario.artifact.clone(),
        headline: scenario.headline.clone(),
        run_status: scenario.run_status.clone(),
        projection_basis: scenario.projection_basis.clone(),
        canonical_gap_id: scenario.canonical_gap_id.clone(),
        packet_id: scenario.packet_id.clone(),
        repair_kind: scenario.repair_kind.clone(),
        top_next_action_kind: scenario.top_next_action_kind.clone(),
        verify_command: scenario.verify_command.clone(),
        receipt_command: scenario.receipt_command.clone(),
        source_alignment_case: scenario.source_alignment_case.clone(),
        limitation_category: scenario.limitation_category.clone(),
        runtime_repair_command: scenario.runtime_repair_command.clone(),
        actionable_count: scenario.actionable_count,
        raw_findings_total: scenario.raw_findings_total,
        consumes_canonical_state: scenario.consumes_canonical_state,
        reinterprets_raw_findings: scenario.reinterprets_raw_findings,
        raw_findings_headline: scenario.raw_findings_headline,
        advisory: scenario.advisory,
        blocking_default: scenario.blocking_default,
        limited_state_visible: scenario.limited_state_visible,
        stale_state_visible: scenario.stale_state_visible,
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn dogfood_user_surface_projection_runtime_state_errors(
    scenario: &DogfoodUserSurfaceProjectionScenario,
) -> Vec<String> {
    let mut errors = Vec::new();
    let headline = scenario.headline.to_ascii_lowercase();
    if scenario.run_status == "full" {
        let expected_projection_basis =
            if scenario.top_next_action_kind == "route_static_limitation_backlog" {
                "canonical_limitation_backlog"
            } else {
                "canonical_actionable_gap"
            };
        if scenario.projection_basis != expected_projection_basis {
            errors.push(format!(
                "full run_status projection_basis must be {expected_projection_basis}, got {}",
                scenario.projection_basis
            ));
        }
        if !scenario.canonical_gap_id.starts_with("gap:") {
            errors.push(format!(
                "canonical_gap_id must use gap: identity, got {}",
                scenario.canonical_gap_id
            ));
        }
        for (label, value) in [
            ("packet_id", &scenario.packet_id),
            ("repair_kind", &scenario.repair_kind),
            ("verify_command", &scenario.verify_command),
            ("receipt_command", &scenario.receipt_command),
            ("source_alignment_case", &scenario.source_alignment_case),
        ] {
            if value.trim().is_empty() || value == "unknown" {
                errors.push(format!("{label} must be present for full run_status"));
            }
        }
        if scenario.raw_findings_total <= scenario.actionable_count {
            errors.push("raw_findings_total must exceed actionable_count to prove raw counts are not the headline".to_string());
        }
        if !matches!(
            scenario.top_next_action_kind.as_str(),
            "attempt_ready_packet"
                | "improve_repair_route_quality"
                | "inspect_unchanged_attempts"
                | "collect_missing_attempt_receipts"
                | "route_static_limitation_backlog"
        ) {
            errors.push(format!(
                "full run_status must route a canonical repair-loop next action, got {}",
                scenario.top_next_action_kind
            ));
        }
    } else if scenario.run_status.starts_with("limited_") {
        if scenario.projection_basis != "canonical_runtime_status" {
            errors.push(format!(
                "{} projection_basis must be canonical_runtime_status, got {}",
                scenario.run_status, scenario.projection_basis
            ));
        }
        if scenario.top_next_action_kind != "resolve_limited_runtime_status" {
            errors.push(format!(
                "{} must route resolve_limited_runtime_status, got {}",
                scenario.run_status, scenario.top_next_action_kind
            ));
        }
        if !scenario.canonical_gap_id.trim().is_empty()
            && !scenario.canonical_gap_id.starts_with("gap:")
        {
            errors.push(format!(
                "canonical_gap_id must use gap: identity, got {}",
                scenario.canonical_gap_id
            ));
        }
        if !scenario.canonical_gap_id.trim().is_empty() || !scenario.packet_id.trim().is_empty() {
            errors.push("limited or stale run_status must not carry packet identity".to_string());
        }
        if !scenario.repair_kind.trim().is_empty()
            || !scenario.verify_command.trim().is_empty()
            || !scenario.receipt_command.trim().is_empty()
        {
            errors.push(
                "limited or stale run_status must not carry packet repair commands".to_string(),
            );
        }
        if !scenario.source_alignment_case.trim().is_empty() {
            errors.push(
                "limited or stale run_status must not carry source_alignment_case".to_string(),
            );
        }
        if scenario.limitation_category.trim().is_empty()
            || scenario.limitation_category == "unknown"
        {
            errors.push(format!(
                "{} must name a limitation_category",
                scenario.run_status
            ));
        }
        if scenario.runtime_repair_command.trim().is_empty()
            || scenario.runtime_repair_command == "unknown"
        {
            errors.push(format!(
                "{} must provide a runtime_repair_command",
                scenario.run_status
            ));
        }
        if let Some((category, command)) =
            user_surface_projection_expected_runtime_route(&scenario.run_status)
        {
            if scenario.limitation_category != category {
                errors.push(format!(
                    "{} limitation_category must be {}, got {}",
                    scenario.run_status, category, scenario.limitation_category
                ));
            }
            if scenario.runtime_repair_command != command {
                errors.push(format!(
                    "{} runtime_repair_command must be {}, got {}",
                    scenario.run_status, command, scenario.runtime_repair_command
                ));
            }
        }
        if scenario.run_status == "limited_stale_input" {
            if !headline.contains("stale") {
                errors
                    .push("limited_stale_input headline must make stale state visible".to_string());
            }
        } else if !headline.contains("limited") {
            errors.push(format!(
                "{} headline must make limited state visible",
                scenario.run_status
            ));
        }
        let actionable_count_headline = format!("{} actionable", scenario.actionable_count);
        if headline.contains(&actionable_count_headline) {
            errors.push(
                "limited or stale run_status must not headline an actionable count".to_string(),
            );
        }
    } else {
        errors.push(format!(
            "run_status must be full or a named limited_* state, got {}",
            scenario.run_status
        ));
    }
    errors
}

pub(crate) fn user_surface_projection_expected_runtime_route(
    run_status: &str,
) -> Option<(&'static str, &'static str)> {
    match run_status {
        "limited_large_cache_skip" => Some((
            "limited_large_cache_skip",
            "cargo xtask cache report && cargo xtask cache gc --dry-run",
        )),
        "limited_incomplete_input" => Some((
            "lane1_repo_exposure_incomplete",
            "cargo xtask lane1-evidence-audit",
        )),
        "limited_sampled_input" => Some((
            "lane1_repo_exposure_sampled",
            "cargo xtask lane1-evidence-audit",
        )),
        "limited_stale_input" => Some(("limited_stale_input", "cargo xtask lane1-evidence-audit")),
        _ => None,
    }
}

pub(crate) fn dogfood_surface_projection_alignment_scenarios()
-> Vec<DogfoodSurfaceProjectionAlignmentScenario> {
    let corpus_path = Path::new(SURFACE_PROJECTION_ALIGNMENT_CORPUS);
    let fallback = |reason: String| {
        vec![DogfoodSurfaceProjectionAlignmentScenario {
            name: "corpus".to_string(),
            canonical_gap_id: "unknown".to_string(),
            packet_id: "unknown".to_string(),
            evidence_class: "unknown".to_string(),
            repair_kind: "unknown".to_string(),
            verify_command: "unknown".to_string(),
            receipt_command: "unknown".to_string(),
            receipt_state: "unknown".to_string(),
            outcome: "unknown".to_string(),
            expected_top_next_action_kind: "unknown".to_string(),
            advisory_consumers: Vec::new(),
            must_not_change: Vec::new(),
            swarm_plan: Value::Null,
            actionable_gap_outcomes: Value::Null,
            attempt_ledger: Value::Null,
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        return fallback(
            "surface projection alignment corpus schema_version must be 0.1".to_string(),
        );
    }
    if json_string_field(&corpus, "kind").as_deref() != Some("surface_projection_alignment_corpus")
    {
        return fallback(
            "surface projection alignment corpus kind must be surface_projection_alignment_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0057") {
        return fallback(
            "surface projection alignment corpus spec must be RIPR-SPEC-0057".to_string(),
        );
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("surface projection alignment corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| {
            let expected = case.get("expected").unwrap_or(&Value::Null);
            let artifacts = case.get("artifacts").unwrap_or(&Value::Null);
            DogfoodSurfaceProjectionAlignmentScenario {
                name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
                canonical_gap_id: json_string_field(case, "canonical_gap_id")
                    .unwrap_or_else(|| "unknown".to_string()),
                packet_id: json_string_field(case, "packet_id")
                    .unwrap_or_else(|| "unknown".to_string()),
                evidence_class: json_string_field(case, "evidence_class")
                    .unwrap_or_else(|| "unknown".to_string()),
                repair_kind: json_string_field(case, "repair_kind")
                    .unwrap_or_else(|| "unknown".to_string()),
                verify_command: json_string_field(case, "verify_command")
                    .unwrap_or_else(|| "unknown".to_string()),
                receipt_command: json_string_field(case, "receipt_command")
                    .unwrap_or_else(|| "unknown".to_string()),
                receipt_state: json_string_field(expected, "receipt_state")
                    .unwrap_or_else(|| "unknown".to_string()),
                outcome: json_string_field(expected, "outcome")
                    .unwrap_or_else(|| "unknown".to_string()),
                expected_top_next_action_kind: json_string_field(expected, "top_next_action_kind")
                    .unwrap_or_else(|| "unknown".to_string()),
                advisory_consumers: json_string_array_field(case, "advisory_consumers"),
                must_not_change: json_string_array_field(case, "must_not_change"),
                swarm_plan: artifacts.get("swarm_plan").cloned().unwrap_or(Value::Null),
                actionable_gap_outcomes: artifacts
                    .get("actionable_gap_outcomes")
                    .cloned()
                    .unwrap_or(Value::Null),
                attempt_ledger: artifacts
                    .get("swarm_attempt_ledger")
                    .cloned()
                    .unwrap_or(Value::Null),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "surface projection alignment corpus case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn dogfood_surface_projection_alignment_run(
    scenario: &DogfoodSurfaceProjectionAlignmentScenario,
) -> DogfoodSurfaceProjectionAlignmentRun {
    let mut errors = Vec::new();
    if scenario.name.trim().is_empty() || scenario.name == "unknown" {
        errors.push("case id must be present".to_string());
    }
    if scenario.canonical_gap_id.trim().is_empty() || scenario.canonical_gap_id == "unknown" {
        errors.push("canonical_gap_id must be present".to_string());
    }
    if scenario.packet_id.trim().is_empty() || scenario.packet_id == "unknown" {
        errors.push("packet_id must be present".to_string());
    }
    if !matches!(
        scenario.evidence_class.as_str(),
        "predicate_boundary"
            | "presentation_text"
            | "config_or_policy_constant"
            | "call_presence"
            | "output_observer"
    ) {
        errors.push(format!(
            "unsupported evidence class for surface projection alignment: {}",
            scenario.evidence_class
        ));
    }
    if matches!(scenario.repair_kind.as_str(), "" | "unknown" | "no_action") {
        errors.push("repair_kind must name a bounded repair route".to_string());
    }
    if finding_alignment_verify_command_is_missing(&scenario.verify_command) {
        errors.push("verify_command must be present".to_string());
    }
    if finding_alignment_verify_command_is_missing(&scenario.receipt_command) {
        errors.push("receipt_command must be present".to_string());
    }
    if scenario.must_not_change.is_empty() {
        errors.push("must_not_change constraints must not be empty".to_string());
    }
    for consumer in ["badge", "lsp", "pr_comment", "ci"] {
        if !scenario
            .advisory_consumers
            .iter()
            .any(|actual| actual == consumer)
        {
            errors.push(format!(
                "advisory_consumers must include {consumer} without granting it ranking or gate authority"
            ));
        }
    }
    if !scenario.swarm_plan.is_object() {
        errors.push("artifacts.swarm_plan must be an object".to_string());
    }
    if !scenario.actionable_gap_outcomes.is_object() {
        errors.push("artifacts.actionable_gap_outcomes must be an object".to_string());
    }
    if !scenario.attempt_ledger.is_object() {
        errors.push("artifacts.swarm_attempt_ledger must be an object".to_string());
    }

    let ledger_attempt = audit_array(&scenario.attempt_ledger, &["latest_attempts"])
        .iter()
        .find(|attempt| {
            audit_non_empty_string(attempt, &["canonical_gap_id"]).as_deref()
                == Some(scenario.canonical_gap_id.as_str())
        });
    if scenario.expected_top_next_action_kind == "route_static_limitation_backlog" {
        if ledger_attempt.is_some() {
            errors.push(
                "static-limitation backlog source must not masquerade as an attempted repair"
                    .to_string(),
            );
        }
    } else {
        match ledger_attempt {
            Some(attempt) => {
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["packet_id"],
                    &scenario.packet_id,
                    "attempt ledger packet_id",
                );
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["repair_kind"],
                    &scenario.repair_kind,
                    "attempt ledger repair_kind",
                );
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["verify_command"],
                    &scenario.verify_command,
                    "attempt ledger verify_command",
                );
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["receipt_command"],
                    &scenario.receipt_command,
                    "attempt ledger receipt_command",
                );
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["receipt_state"],
                    &scenario.receipt_state,
                    "attempt ledger receipt_state",
                );
                surface_projection_expect_string(
                    &mut errors,
                    attempt,
                    &["outcome"],
                    &scenario.outcome,
                    "attempt ledger outcome",
                );
            }
            None => errors.push(format!(
                "attempt ledger latest_attempts must include {}",
                scenario.canonical_gap_id
            )),
        }
    }

    let readiness_report = ripr_swarm_readiness_from_values(
        RiprSwarmReadinessInput {
            path: "fixtures/surface-projection-alignment/swarm-plan.json".to_string(),
            state: "read".to_string(),
            limitation: None,
            value: Some(&scenario.swarm_plan),
        },
        RiprSwarmReadinessInput {
            path: "fixtures/surface-projection-alignment/actionable-gap-outcomes.json".to_string(),
            state: "read".to_string(),
            limitation: None,
            value: Some(&scenario.actionable_gap_outcomes),
        },
        RiprSwarmReadinessInput {
            path: "fixtures/surface-projection-alignment/swarm-attempt-ledger.json".to_string(),
            state: "read".to_string(),
            limitation: None,
            value: Some(&scenario.attempt_ledger),
        },
    );
    let readiness_json = ripr_swarm_readiness_json(&readiness_report)
        .and_then(|json| serde_json::from_str::<Value>(&json).map_err(|err| err.to_string()));
    let mut top_next_action_kind = "missing".to_string();
    let mut top_next_action_command = None;
    let mut readiness_status = readiness_report.status.clone();
    let attempted_packets = readiness_report.summary.attempted_packets;
    let improved_packets = readiness_report.summary.improved_packets;

    match readiness_json {
        Ok(value) => {
            readiness_status = json_string_field(&value, "status")
                .unwrap_or_else(|| readiness_report.status.clone());
            let top = value.get("top_next_action").unwrap_or(&Value::Null);
            top_next_action_kind =
                json_string_field(top, "kind").unwrap_or_else(|| "missing".to_string());
            top_next_action_command = json_string_field(top, "command");
            if value
                .get("next_actions")
                .and_then(Value::as_array)
                .and_then(|actions| actions.first())
                != Some(top)
            {
                errors.push(
                    "readiness top_next_action must remain a projection of next_actions[0]"
                        .to_string(),
                );
            }
            surface_projection_expect_string(
                &mut errors,
                top,
                &["kind"],
                &scenario.expected_top_next_action_kind,
                "readiness top_next_action.kind",
            );
            if scenario.expected_top_next_action_kind == "attempt_ready_packet" {
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["packet_id"],
                    &scenario.packet_id,
                    "readiness top_next_action.packet_id",
                );
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["canonical_gap_id"],
                    &scenario.canonical_gap_id,
                    "readiness top_next_action.canonical_gap_id",
                );
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["repair_kind"],
                    &scenario.repair_kind,
                    "readiness top_next_action.repair_kind",
                );
                let expected_attempt_command = format!(
                    "cargo xtask ripr-swarm attempt --packet {} --dry-run",
                    scenario.packet_id
                );
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["command"],
                    &expected_attempt_command,
                    "readiness top_next_action.command",
                );
            } else if scenario.expected_top_next_action_kind == "route_static_limitation_backlog" {
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["packet_id"],
                    &scenario.packet_id,
                    "readiness top_next_action.packet_id",
                );
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["canonical_gap_id"],
                    &scenario.canonical_gap_id,
                    "readiness top_next_action.canonical_gap_id",
                );
                surface_projection_expect_string(
                    &mut errors,
                    top,
                    &["command"],
                    "cargo xtask lane1-evidence-audit",
                    "readiness top_next_action.command",
                );
                let backlog_packets = audit_array(
                    &scenario.swarm_plan,
                    &["static_limitation_backlog", "limitation_backlog_packets"],
                );
                if let Some(backlog_packet) = backlog_packets.first() {
                    if audit_array(backlog_packet, &["sample_canonical_gap_ids"]).is_empty() {
                        errors.push(
                            "limitation backlog packet must include sample_canonical_gap_ids"
                                .to_string(),
                        );
                    }
                    if audit_array(backlog_packet, &["sample_sources"]).is_empty() {
                        errors.push(
                            "limitation backlog packet must include sample_sources".to_string(),
                        );
                    }
                    if !audit_array(backlog_packet, &["non_claims"])
                        .iter()
                        .any(|claim| claim.as_str() == Some("not a public repair packet"))
                    {
                        errors.push(
                            "limitation backlog packet must preserve non_claims denying public repair status"
                                .to_string(),
                        );
                    }
                    if backlog_packet.get("public_projection_eligible").is_some() {
                        errors.push(
                            "limitation backlog packet must not expose public_projection_eligible"
                                .to_string(),
                        );
                    }
                    if backlog_packet.get("swarm_ready").is_some() {
                        errors.push(
                            "limitation backlog packet must not expose swarm_ready".to_string(),
                        );
                    }
                } else {
                    errors.push(
                        "static limitation backlog must include limitation_backlog_packets"
                            .to_string(),
                    );
                }
                let readiness_backlog_packets = audit_array(
                    &value,
                    &["static_limitation_backlog", "limitation_backlog_packets"],
                );
                if let Some(readiness_backlog_packet) = readiness_backlog_packets.first() {
                    if audit_array(readiness_backlog_packet, &["sample_canonical_gap_ids"])
                        .is_empty()
                    {
                        errors.push(
                            "readiness limitation backlog packet must preserve sample_canonical_gap_ids"
                                .to_string(),
                        );
                    }
                    if audit_array(readiness_backlog_packet, &["sample_sources"]).is_empty() {
                        errors.push(
                            "readiness limitation backlog packet must preserve sample_sources"
                                .to_string(),
                        );
                    }
                    if !audit_array(readiness_backlog_packet, &["non_claims"])
                        .iter()
                        .any(|claim| claim.as_str() == Some("not a public repair packet"))
                    {
                        errors.push(
                            "readiness limitation backlog packet must preserve non_claims"
                                .to_string(),
                        );
                    }
                    if readiness_backlog_packet
                        .get("public_projection_eligible")
                        .is_some()
                    {
                        errors.push(
                            "readiness limitation backlog packet must not expose public_projection_eligible"
                                .to_string(),
                        );
                    }
                    if readiness_backlog_packet.get("swarm_ready").is_some() {
                        errors.push(
                            "readiness limitation backlog packet must not expose swarm_ready"
                                .to_string(),
                        );
                    }
                } else {
                    errors.push(
                        "readiness static_limitation_backlog must preserve limitation_backlog_packets"
                            .to_string(),
                    );
                }
                let top_limitation_routes = audit_array(&value, &["top_limitation_routes"]);
                if let Some(top_limitation_route) = top_limitation_routes.first() {
                    if audit_array(top_limitation_route, &["sample_canonical_gap_ids"]).is_empty() {
                        errors.push(
                            "top_limitation_routes must preserve sample_canonical_gap_ids"
                                .to_string(),
                        );
                    }
                    if audit_array(top_limitation_route, &["sample_sources"]).is_empty() {
                        errors
                            .push("top_limitation_routes must preserve sample_sources".to_string());
                    }
                    if !audit_array(top_limitation_route, &["non_claims"])
                        .iter()
                        .any(|claim| claim.as_str() == Some("not a public repair packet"))
                    {
                        errors.push(
                            "top_limitation_routes must preserve non_claims denying public repair status"
                                .to_string(),
                        );
                    }
                    if top_limitation_route
                        .get("public_projection_eligible")
                        .is_some()
                    {
                        errors.push(
                            "top_limitation_routes must not expose public_projection_eligible"
                                .to_string(),
                        );
                    }
                    if top_limitation_route.get("swarm_ready").is_some() {
                        errors
                            .push("top_limitation_routes must not expose swarm_ready".to_string());
                    }
                } else {
                    errors.push(
                        "readiness must expose top_limitation_routes for backlog routing"
                            .to_string(),
                    );
                }
            } else {
                if scenario.expected_top_next_action_kind == "improve_repair_route_quality" {
                    surface_projection_expect_string(
                        &mut errors,
                        top,
                        &["repair_kind"],
                        &scenario.repair_kind,
                        "readiness top_next_action.repair_kind",
                    );
                    let route_quality_packet = format!(
                        "route-quality:{}:{}",
                        audit_slug(&scenario.repair_kind),
                        audit_slug(match scenario.outcome.as_str() {
                            "evidence_regressed" => "regressed",
                            "evidence_unchanged" => "unchanged",
                            "attempted_no_receipt" => "attempted_no_receipt",
                            _ => "unknown",
                        })
                    );
                    surface_projection_expect_string(
                        &mut errors,
                        top,
                        &["packet_id"],
                        &route_quality_packet,
                        "readiness top_next_action.packet_id",
                    );
                    surface_projection_expect_string(
                        &mut errors,
                        top,
                        &["command"],
                        "cargo xtask ripr-swarm readiness",
                        "readiness top_next_action.command",
                    );
                } else {
                    surface_projection_expect_string(
                        &mut errors,
                        top,
                        &["command"],
                        "cargo xtask ripr-swarm attempt-ledger",
                        "readiness top_next_action.command",
                    );
                }
            }
            if !json_string_array_field(&value, "must_not_infer")
                .iter()
                .any(|item| item.contains("not a separate ranking source"))
            {
                errors.push(
                    "readiness must_not_infer must deny separate ranking authority".to_string(),
                );
            }
        }
        Err(err) => errors.push(format!("failed to render readiness projection: {err}")),
    }

    if scenario.expected_top_next_action_kind != "route_static_limitation_backlog"
        && readiness_report.summary.attempted_packets == 0
    {
        errors.push("readiness summary must preserve attempted packet count".to_string());
    }
    if scenario.outcome == "evidence_improved" && readiness_report.summary.improved_packets == 0 {
        errors.push("readiness summary must preserve improved packet count".to_string());
    }

    DogfoodSurfaceProjectionAlignmentRun {
        name: scenario.name.clone(),
        canonical_gap_id: scenario.canonical_gap_id.clone(),
        packet_id: scenario.packet_id.clone(),
        repair_kind: scenario.repair_kind.clone(),
        verify_command: scenario.verify_command.clone(),
        receipt_command: scenario.receipt_command.clone(),
        receipt_state: scenario.receipt_state.clone(),
        outcome: scenario.outcome.clone(),
        top_next_action_kind,
        top_next_action_command,
        readiness_status,
        attempted_packets,
        improved_packets,
        advisory_consumers: scenario.advisory_consumers.clone(),
        reason: scenario.reason.clone(),
        errors,
    }
}

pub(crate) fn surface_projection_expect_string(
    errors: &mut Vec<String>,
    value: &Value,
    path: &[&str],
    expected: &str,
    label: &str,
) {
    let actual = audit_non_empty_string(value, path).unwrap_or_else(|| "missing".to_string());
    if actual != expected {
        errors.push(format!("{label} must be {expected}, got {actual}"));
    }
}

pub(crate) fn finding_alignment_verify_command_is_missing(value: &str) -> bool {
    value.trim().is_empty() || value == "unknown" || value == "none"
}

pub(crate) fn dogfood_pr_inline_comment_scenarios() -> Vec<DogfoodPrInlineCommentScenario> {
    let corpus_path =
        Path::new("fixtures/boundary_gap/expected/pr-inline-comment-publisher/corpus.json");
    let fallback = |reason: String| {
        vec![DogfoodPrInlineCommentScenario {
            name: "corpus".to_string(),
            scenario: reason.clone(),
            expected_report: corpus_path.to_path_buf(),
            expected_markdown: corpus_path.to_path_buf(),
            expected_status: "missing".to_string(),
            expected_mode: "missing".to_string(),
            expected_publishable: 0,
            expected_skipped: 0,
            expected_blocked: 0,
            expected_safe_to_publish: false,
            expected_operations: Vec::new(),
            expected_skip_reasons: Vec::new(),
            expected_blocked_reasons: Vec::new(),
            reason,
        }]
    };

    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => return fallback(err),
    };

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        return fallback("PR inline comment publisher corpus is missing cases array".to_string());
    };

    cases
        .iter()
        .map(|case| {
            let expected = case.get("expected").unwrap_or(&Value::Null);
            DogfoodPrInlineCommentScenario {
                name: json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string()),
                scenario: json_string_field(case, "scenario")
                    .unwrap_or_else(|| "missing scenario".to_string()),
                expected_report: json_string_field(case, "expected_report")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| corpus_path.to_path_buf()),
                expected_markdown: json_string_field(case, "expected_markdown")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| corpus_path.to_path_buf()),
                expected_status: json_string_field(expected, "status")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_mode: json_string_field(expected, "mode")
                    .unwrap_or_else(|| "missing".to_string()),
                expected_publishable: json_usize_field(expected, "publishable").unwrap_or(0),
                expected_skipped: json_usize_field(expected, "skipped").unwrap_or(0),
                expected_blocked: json_usize_field(expected, "blocked").unwrap_or(0),
                expected_safe_to_publish: json_bool_field(expected, "safe_to_publish")
                    .unwrap_or(false),
                expected_operations: sorted_unique_strings(json_string_array_field(
                    expected,
                    "operations",
                )),
                expected_skip_reasons: sorted_unique_strings(json_string_array_field(
                    expected,
                    "skip_reasons",
                )),
                expected_blocked_reasons: sorted_unique_strings(json_string_array_field(
                    expected,
                    "blocked_reasons",
                )),
                reason: json_string_field(case, "reason").unwrap_or_else(|| {
                    "PR inline comment publisher corpus case did not document a reason".to_string()
                }),
            }
        })
        .collect()
}

pub(crate) fn dogfood_pr_inline_comment_run(
    scenario: &DogfoodPrInlineCommentScenario,
) -> Result<DogfoodPrInlineCommentRun, String> {
    let actual_dir = scenario
        .expected_report
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let json_path = scenario.expected_report.clone();
    let markdown_path = scenario.expected_markdown.clone();
    let mut errors = Vec::new();

    if !scenario.expected_report.exists() {
        errors.push(format!(
            "expected PR inline comment publish-plan fixture is missing: {}",
            normalize_path(&scenario.expected_report)
        ));
    }
    if !scenario.expected_markdown.exists() {
        errors.push(format!(
            "expected PR inline comment publish-plan Markdown fixture is missing: {}",
            normalize_path(&scenario.expected_markdown)
        ));
    }

    let mut status = "missing".to_string();
    let mut mode = "missing".to_string();
    let mut publishable = 0usize;
    let mut skipped = 0usize;
    let mut blocked = 0usize;
    let mut safe_to_publish = false;
    let mut operations = Vec::<String>::new();
    let mut skip_reasons = Vec::<String>::new();
    let mut blocked_reasons = Vec::<String>::new();

    match read_json_value(&json_path) {
        Ok(report) => {
            if json_string_field(&report, "kind").as_deref()
                != Some("pr_inline_comment_publish_plan")
            {
                errors.push("report kind must be pr_inline_comment_publish_plan".to_string());
            }
            status = json_string_field(&report, "status").unwrap_or_else(|| "missing".to_string());
            mode = json_string_field(&report, "mode").unwrap_or_else(|| "missing".to_string());
            if let Some(summary) = report.get("summary") {
                publishable = json_usize_field(summary, "publishable").unwrap_or(0);
                skipped = json_usize_field(summary, "skipped").unwrap_or(0);
                blocked = json_usize_field(summary, "blocked").unwrap_or(0);
                safe_to_publish = json_bool_field(summary, "safe_to_publish").unwrap_or(false);
            } else {
                errors.push("report summary is missing".to_string());
            }
            operations = json_string_values_from_array(&report, "operations", "operation");
            skip_reasons = json_string_values_from_array(&report, "skipped", "skip_reason");
            blocked_reasons = json_string_values_from_array(&report, "blocked", "blocked_reason");
            if report
                .get("limits")
                .and_then(|limits| limits.get("comments_default"))
                .and_then(Value::as_str)
                != Some("off")
            {
                errors.push("report must record comments_default=off".to_string());
            }
            if !report
                .get("limits")
                .and_then(|limits| limits.get("advisory"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                errors.push("report must record advisory=true".to_string());
            }
            if json_summary_count(&report, "publishable") > 3 && mode != "off" {
                errors.push("publishable comments should not exceed the default cap".to_string());
            }
            if !json_string_field(&report, "limits_note").is_some_and(|note| {
                let lower = note.to_ascii_lowercase();
                lower.contains("advisory inline-comment publish plan only")
                    && lower.contains("gate decisions remain separate")
            }) {
                errors.push("report is missing advisory/gate-authority limits note".to_string());
            }
        }
        Err(err) => errors.push(err),
    }

    match fs::read_to_string(&markdown_path) {
        Ok(markdown) => {
            if !markdown.contains("# RIPR Inline Comment Publish Plan") {
                errors
                    .push("Markdown must use the inline comment publish-plan heading".to_string());
            }
            if !markdown.contains(&format!("Mode: {}", scenario.expected_mode)) {
                errors.push(format!(
                    "Markdown should pin mode {}",
                    scenario.expected_mode
                ));
            }
            if !markdown.contains(&format!("Status: {}", scenario.expected_status)) {
                errors.push(format!(
                    "Markdown should pin status {}",
                    scenario.expected_status
                ));
            }
            if !markdown.contains("Advisory inline-comment publish plan only.") {
                errors
                    .push("Markdown must preserve the advisory publish-plan boundary".to_string());
            }
        }
        Err(err) => errors.push(format!(
            "failed to read PR inline comment publish-plan Markdown {}: {err}",
            normalize_path(&markdown_path)
        )),
    }

    if status != scenario.expected_status {
        errors.push(format!(
            "expected status {}, got {}",
            scenario.expected_status, status
        ));
    }
    if mode != scenario.expected_mode {
        errors.push(format!(
            "expected mode {}, got {}",
            scenario.expected_mode, mode
        ));
    }
    if publishable != scenario.expected_publishable {
        errors.push(format!(
            "expected publishable {}, got {}",
            scenario.expected_publishable, publishable
        ));
    }
    if skipped != scenario.expected_skipped {
        errors.push(format!(
            "expected skipped {}, got {}",
            scenario.expected_skipped, skipped
        ));
    }
    if blocked != scenario.expected_blocked {
        errors.push(format!(
            "expected blocked {}, got {}",
            scenario.expected_blocked, blocked
        ));
    }
    if safe_to_publish != scenario.expected_safe_to_publish {
        errors.push(format!(
            "expected safe_to_publish {}, got {}",
            scenario.expected_safe_to_publish, safe_to_publish
        ));
    }
    if operations != scenario.expected_operations {
        errors.push(format!(
            "expected operations {:?}, got {:?}",
            scenario.expected_operations, operations
        ));
    }
    if skip_reasons != scenario.expected_skip_reasons {
        errors.push(format!(
            "expected skip reasons {:?}, got {:?}",
            scenario.expected_skip_reasons, skip_reasons
        ));
    }
    if blocked_reasons != scenario.expected_blocked_reasons {
        errors.push(format!(
            "expected blocked reasons {:?}, got {:?}",
            scenario.expected_blocked_reasons, blocked_reasons
        ));
    }

    Ok(DogfoodPrInlineCommentRun {
        name: scenario.name.clone(),
        actual_dir,
        json_path,
        markdown_path,
        status,
        mode,
        publishable,
        skipped,
        blocked,
        safe_to_publish,
        operations,
        skip_reasons,
        blocked_reasons,
        expected_status: scenario.expected_status.clone(),
        expected_mode: scenario.expected_mode.clone(),
        expected_publishable: scenario.expected_publishable,
        expected_skipped: scenario.expected_skipped,
        expected_blocked: scenario.expected_blocked,
        expected_safe_to_publish: scenario.expected_safe_to_publish,
        expected_operations: scenario.expected_operations.clone(),
        expected_skip_reasons: scenario.expected_skip_reasons.clone(),
        expected_blocked_reasons: scenario.expected_blocked_reasons.clone(),
        reason: if scenario.scenario.trim().is_empty() {
            scenario.reason.clone()
        } else {
            format!("{}: {}", scenario.scenario, scenario.reason)
        },
        expected_report: scenario.expected_report.clone(),
        expected_markdown: scenario.expected_markdown.clone(),
        errors,
    })
}

pub(crate) fn compare_expected_text(
    actual_path: &Path,
    expected_path: &Path,
    label: &str,
    errors: &mut Vec<String>,
) {
    let actual = match fs::read_to_string(actual_path) {
        Ok(text) => text,
        Err(err) => {
            errors.push(format!(
                "failed to read actual {label} {}: {err}",
                normalize_path(actual_path)
            ));
            return;
        }
    };
    let expected = match fs::read_to_string(expected_path) {
        Ok(text) => text,
        Err(err) => {
            errors.push(format!(
                "failed to read expected {label} {}: {err}",
                normalize_path(expected_path)
            ));
            return;
        }
    };
    if actual != expected {
        errors.push(format!(
            "{label} drifted: actual {} does not match expected {}",
            normalize_path(actual_path),
            normalize_path(expected_path)
        ));
    }
}

pub(crate) fn json_summary_count(value: &Value, key: &str) -> usize {
    value
        .get("summary")
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

pub(crate) fn one_line(text: &str) -> String {
    text.lines().map(str::trim).collect::<Vec<_>>().join(" ")
}

pub(crate) fn dogfood_class_counts(json: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for class in [
        "exposed",
        "weakly_exposed",
        "reachable_unrevealed",
        "no_static_path",
        "infection_unknown",
        "propagation_unknown",
        "static_unknown",
    ] {
        counts.insert(
            class.to_string(),
            json.matches(&format!("\"classification\": \"{class}\""))
                .count(),
        );
    }
    counts
}

pub(crate) fn json_number_after(text: &str, needle: &str) -> Option<usize> {
    let start = text.find(needle)? + needle.len();
    let digits = text[start..]
        .chars()
        .skip_while(|ch| ch.is_ascii_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}

pub(crate) fn dogfood_report_status(inputs: &DogfoodReportInputs<'_>) -> &'static str {
    let runs = inputs.runs;
    let gate_runs = inputs.gate_runs;
    let first_action_runs = inputs.first_action_runs;
    let first_pr_runs = inputs.first_pr_runs;
    let front_panel_runs = inputs.front_panel_runs;
    let report_packet_index_runs = inputs.report_packet_index_runs;
    let preview_projection_runs = inputs.preview_projection_runs;
    let finding_alignment_runs = inputs.finding_alignment_runs;
    let surface_projection_alignment_runs = inputs.surface_projection_alignment_runs;
    let real_repair_attempt_runs = inputs.real_repair_attempt_runs;
    let python_real_repo_eval_runs = inputs.python_real_repo_eval_runs;
    let python_static_limit_eval_runs = inputs.python_static_limit_eval_runs;
    let python_no_action_eval_runs = inputs.python_no_action_eval_runs;
    let python_repair_quality =
        dogfood_python_repair_routing_quality_summary(python_real_repo_eval_runs);
    let typescript_false_actionable_audit = dogfood_typescript_false_actionable_audit_summary(
        &typescript_preview_false_actionable_audit_cases_at(&repo_rooted_fixture_path(
            TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS,
        )),
    );
    let typescript_preview_repair_loop_runs = inputs.typescript_preview_repair_loop_runs;
    let bun_ub_cross_language_runs = inputs.bun_ub_cross_language_runs;
    let user_surface_projection_runs = inputs.user_surface_projection_runs;
    let pr_inline_comment_runs = inputs.pr_inline_comment_runs;

    if runs.iter().any(|run| !run.errors.is_empty())
        || gate_runs.iter().any(|run| !run.errors.is_empty())
        || first_action_runs.iter().any(|run| !run.errors.is_empty())
        || first_pr_runs.iter().any(|run| !run.errors.is_empty())
        || front_panel_runs.iter().any(|run| !run.errors.is_empty())
        || report_packet_index_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || preview_projection_runs
            .generated_ci_cockpit
            .iter()
            .any(|run| !run.errors.is_empty())
        || preview_projection_runs
            .language_preview
            .iter()
            .any(|run| !run.errors.is_empty())
        || preview_projection_runs
            .editor_gap_cockpit
            .iter()
            .any(|run| !run.errors.is_empty())
        || preview_projection_runs
            .editor_first_pr_bridge
            .iter()
            .any(|run| !run.errors.is_empty())
        || finding_alignment_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || surface_projection_alignment_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || real_repair_attempt_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || python_real_repo_eval_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || python_static_limit_eval_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || python_no_action_eval_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || python_repair_quality.gate_status != "pass"
        || typescript_false_actionable_audit.gate_status != "pass"
        || typescript_preview_repair_loop_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || bun_ub_cross_language_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || user_surface_projection_runs
            .iter()
            .any(|run| !run.errors.is_empty())
        || pr_inline_comment_runs
            .iter()
            .any(|run| !run.errors.is_empty())
    {
        "warn"
    } else {
        "pass"
    }
}

pub(crate) fn dogfood_first_pr_metrics(
    first_pr_runs: &[DogfoodFirstPrRun],
) -> DogfoodFirstPrMetrics {
    let mut metrics = DogfoodFirstPrMetrics {
        packets_total: first_pr_runs.len(),
        ..DogfoodFirstPrMetrics::default()
    };

    for run in first_pr_runs {
        if run.status == "blocked" {
            metrics.blocked_total += 1;
        }
        match run.state.as_str() {
            "top_gap" => metrics.top_gap_selected_total += 1,
            "empty_diff" | "no_action" | "already_observed" => metrics.no_action_total += 1,
            "missing_artifact" => metrics.missing_artifact_total += 1,
            "stale_artifact" => metrics.stale_artifact_total += 1,
            "wrong_root" => metrics.wrong_root_total += 1,
            "malformed_artifact" => metrics.malformed_artifact_total += 1,
            "timeout" => metrics.timeout_total += 1,
            _ => {}
        }
    }

    metrics
}

pub(crate) fn dogfood_report_markdown(inputs: &DogfoodReportInputs<'_>) -> String {
    let runs = inputs.runs;
    let gate_runs = inputs.gate_runs;
    let first_action_runs = inputs.first_action_runs;
    let first_pr_runs = inputs.first_pr_runs;
    let front_panel_runs = inputs.front_panel_runs;
    let report_packet_index_runs = inputs.report_packet_index_runs;
    let preview_projection_runs = inputs.preview_projection_runs;
    let finding_alignment_runs = inputs.finding_alignment_runs;
    let surface_projection_alignment_runs = inputs.surface_projection_alignment_runs;
    let real_repair_attempt_runs = inputs.real_repair_attempt_runs;
    let python_real_repo_eval_runs = inputs.python_real_repo_eval_runs;
    let typescript_preview_repair_loop_runs = inputs.typescript_preview_repair_loop_runs;
    let bun_ub_cross_language_runs = inputs.bun_ub_cross_language_runs;
    let user_surface_projection_runs = inputs.user_surface_projection_runs;
    let pr_inline_comment_runs = inputs.pr_inline_comment_runs;
    let first_pr_metrics = dogfood_first_pr_metrics(first_pr_runs);
    let mut body = format!(
        "# ripr dogfood report\n\nStatus: {}\n\nMode: advisory\n\nThis report runs `ripr check --mode fast` against stable in-repo fixture diffs. It records current product output for review without making dogfood a blocking gate yet.\n\n## Summary\n\n",
        dogfood_report_status(inputs)
    );
    for run in runs {
        body.push_str(&format!(
            "- `{}`: {} finding(s), {} stop-reason field(s), {} ms\n",
            run.name, run.findings, run.stop_reason_mentions, run.duration_ms
        ));
    }

    body.push_str("\n## Runs\n\n");
    for run in runs {
        body.push_str(&format!("### `{}`\n\n", run.name));
        body.push_str(&format!("- Root: `{}`\n", normalize_path(&run.root)));
        body.push_str(&format!("- Diff: `{}`\n", normalize_path(&run.diff)));
        body.push_str(&format!(
            "- Actual outputs: `{}`\n",
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("- Findings: {}\n", run.findings));
        body.push_str("- Exposure classes:\n");
        for (class, count) in &run.class_counts {
            body.push_str(&format!("  - `{class}`: {count}\n"));
        }
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## First Useful Action Receipts\n\n");
    body.push_str("These receipts validate checked `first-useful-action.{json,md}` fixture outputs for the documented Campaign 22 routes. They are advisory projections over existing artifacts; they do not rerun hidden analysis, edit source, generate tests, call providers, run mutation testing, invent policy, or change CI blocking.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str(
        "- Receipt outputs: `fixtures/boundary_gap/expected/first-useful-action/<case>/first-useful-action.{json,md}`\n\n",
    );
    body.push_str("| Case | Status | Action | Audience | Selected | Static movement |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for run in first_action_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.status),
            markdown_cell(&run.action_kind),
            markdown_cell(&run.audience),
            if run.selected { "yes" } else { "no" },
            markdown_cell(&run.static_movement)
        ));
    }
    body.push('\n');
    for run in first_action_runs {
        body.push_str(&format!("### First Action `{}`\n\n", run.name));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!(
            "- Action: `{}`\n",
            markdown_cell(&run.action_kind)
        ));
        body.push_str(&format!(
            "- Expected action: `{}`\n",
            markdown_cell(&run.expected_action_kind)
        ));
        body.push_str(&format!("- Audience: `{}`\n", markdown_cell(&run.audience)));
        body.push_str(&format!(
            "- Expected audience: `{}`\n",
            markdown_cell(&run.expected_audience)
        ));
        body.push_str(&format!("- Selected seam: {}\n", run.selected));
        body.push_str(&format!(
            "- Expected selected seam: {}\n",
            run.expected_selected
        ));
        body.push_str(&format!(
            "- Static movement: `{}`\n",
            markdown_cell(&run.static_movement)
        ));
        body.push_str(&format!(
            "- Expected static movement: `{}`\n",
            markdown_cell(&run.expected_static_movement)
        ));
        body.push_str(&format!(
            "- Receipt JSON: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Receipt Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!(
            "- Expected directory: `{}`\n",
            normalize_path(&run.expected_dir)
        ));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## First Successful PR Receipts\n\n");
    body.push_str("These receipts validate checked `start-here.{json,md}` fixture outputs for the first successful PR path. They record that the first screen selects a repairable Rust gap or a clear no-action/blocked state while preserving advisory limits and gate-authority separation.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str(
        "- Receipt outputs: `fixtures/first_successful_pr/<case>/expected/start-here.{json,md}`\n\n",
    );
    body.push_str("| Metric | Value |\n");
    body.push_str("| --- | ---: |\n");
    body.push_str(&format!(
        "| `first_run_packets_total` | {} |\n",
        first_pr_metrics.packets_total
    ));
    body.push_str(&format!(
        "| `first_run_top_gap_selected_total` | {} |\n",
        first_pr_metrics.top_gap_selected_total
    ));
    body.push_str(&format!(
        "| `first_run_no_action_total` | {} |\n",
        first_pr_metrics.no_action_total
    ));
    body.push_str(&format!(
        "| `first_run_blocked_total` | {} |\n",
        first_pr_metrics.blocked_total
    ));
    body.push_str(&format!(
        "| `first_run_missing_artifact_total` | {} |\n",
        first_pr_metrics.missing_artifact_total
    ));
    body.push_str(&format!(
        "| `first_run_stale_artifact_total` | {} |\n",
        first_pr_metrics.stale_artifact_total
    ));
    body.push_str(&format!(
        "| `first_run_wrong_root_total` | {} |\n",
        first_pr_metrics.wrong_root_total
    ));
    body.push_str(&format!(
        "| `first_run_malformed_artifact_total` | {} |\n",
        first_pr_metrics.malformed_artifact_total
    ));
    body.push_str(&format!(
        "| `first_run_timeout_total` | {} |\n\n",
        first_pr_metrics.timeout_total
    ));
    body.push_str("| Case | Status | State | Top gap | Verify | Next |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for run in first_pr_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.status),
            markdown_cell(&run.state),
            markdown_cell(&run.top_gap_kind),
            run.verify_command
                .as_deref()
                .map(markdown_cell)
                .map(|value| format!("`{value}`"))
                .unwrap_or_else(|| "none".to_string()),
            run.next_command
                .as_deref()
                .map(markdown_cell)
                .map(|value| format!("`{value}`"))
                .unwrap_or_else(|| "none".to_string())
        ));
    }
    body.push('\n');
    for run in first_pr_runs {
        body.push_str(&format!("### First PR `{}`\n\n", run.name));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!("- State: `{}`\n", markdown_cell(&run.state)));
        body.push_str(&format!(
            "- Expected state: `{}`\n",
            markdown_cell(&run.expected_state)
        ));
        body.push_str(&format!(
            "- Top gap kind: `{}`\n",
            markdown_cell(&run.top_gap_kind)
        ));
        body.push_str(&format!(
            "- Receipt JSON: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Receipt Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!(
            "- Expected directory: `{}`\n",
            normalize_path(&run.expected_dir)
        ));
        body.push_str(&format!(
            "- Description: {}\n",
            markdown_cell(&run.description)
        ));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## PR Review Front Panel Receipts\n\n");
    body.push_str("These receipts validate checked `pr-review-front-panel.{json,md}` fixture outputs for the documented Campaign 24 reviewer routes. They are advisory projections over explicit existing artifacts; they do not rerun hidden analysis, edit source, generate tests, call providers, run mutation testing, invent policy, publish inline comments, or change CI blocking.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str(
        "- Receipt outputs: `fixtures/boundary_gap/expected/pr-review-front-panel/<case>/pr-review-front-panel.{json,md}`\n\n",
    );
    body.push_str("| Case | Status | Top issue | Policy | Placement | Movement | Coverage/grip | New | Resolved | Blocking | Warnings |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |\n");
    for run in front_panel_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.status),
            markdown_cell(&run.top_issue_state),
            markdown_cell(&run.policy_state),
            markdown_cell(&run.placement),
            markdown_cell(&run.movement_state),
            markdown_cell(&run.coverage_grip_state),
            run.new_policy_eligible,
            run.baseline_resolved,
            run.blocking_candidates,
            run.warnings
        ));
    }
    body.push('\n');
    for run in front_panel_runs {
        body.push_str(&format!("### Front Panel `{}`\n\n", run.name));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!(
            "- Top issue state: `{}`\n",
            markdown_cell(&run.top_issue_state)
        ));
        body.push_str(&format!(
            "- Expected top issue state: `{}`\n",
            markdown_cell(&run.expected_top_issue_state)
        ));
        body.push_str(&format!(
            "- Policy state: `{}`\n",
            markdown_cell(&run.policy_state)
        ));
        body.push_str(&format!(
            "- Expected policy state: `{}`\n",
            markdown_cell(&run.expected_policy_state)
        ));
        body.push_str(&format!(
            "- Placement: `{}`\n",
            markdown_cell(&run.placement)
        ));
        body.push_str(&format!(
            "- Expected placement: `{}`\n",
            markdown_cell(&run.expected_placement)
        ));
        body.push_str(&format!(
            "- Movement: `{}`\n",
            markdown_cell(&run.movement_state)
        ));
        body.push_str(&format!(
            "- Expected movement: `{}`\n",
            markdown_cell(&run.expected_movement_state)
        ));
        body.push_str(&format!(
            "- Coverage/grip: `{}`\n",
            markdown_cell(&run.coverage_grip_state)
        ));
        body.push_str(&format!(
            "- Expected coverage/grip: `{}`\n",
            markdown_cell(&run.expected_coverage_grip_state)
        ));
        body.push_str(&format!(
            "- Counts: new {}, resolved {}, blocking {}, warnings {}\n",
            run.new_policy_eligible, run.baseline_resolved, run.blocking_candidates, run.warnings
        ));
        body.push_str(&format!(
            "- Expected counts: new {}, resolved {}, blocking {}, warnings {}\n",
            run.expected_new_policy_eligible,
            run.expected_baseline_resolved,
            run.expected_blocking_candidates,
            run.expected_warnings
        ));
        body.push_str(&format!(
            "- Receipt JSON: `{}`\n",
            normalize_path(&run.report_path)
        ));
        body.push_str(&format!(
            "- Receipt Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Report Packet Index Receipts\n\n");
    body.push_str("These receipts validate checked `report-packet-index` fixture outputs for the documented Campaign 25 packet-index routes. They verify reviewer-first grouping, missing-surface counts, start-here discovery, gate-authority visibility, and advisory limits without rerunning hidden analysis or changing pass/fail authority.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str(
        "- Receipt outputs: `fixtures/boundary_gap/expected/report-packet-index/<case>/index.{json,md}`\n\n",
    );
    body.push_str("| Case | Status | Missing | Warnings | Failures | Start here | Gate authority | Groups |\n");
    body.push_str("| --- | --- | ---: | ---: | ---: | --- | --- | --- |\n");
    for run in report_packet_index_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} | {} | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.status),
            run.missing_expected,
            run.warnings,
            run.failures,
            if run.start_here_available {
                "yes"
            } else {
                "no"
            },
            if run.gate_authority_present {
                "yes"
            } else {
                "no"
            },
            markdown_cell(&run.groups.join(", "))
        ));
    }
    body.push('\n');
    for run in report_packet_index_runs {
        body.push_str(&format!("### Report Packet Index `{}`\n\n", run.name));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!(
            "- Counts: missing {}, warnings {}, failures {}\n",
            run.missing_expected, run.warnings, run.failures
        ));
        body.push_str(&format!(
            "- Expected counts: missing {}, warnings {}, failures {}\n",
            run.expected_missing_expected, run.expected_warnings, run.expected_failures
        ));
        body.push_str(&format!(
            "- Start here available: {} (expected {})\n",
            run.start_here_available, run.expected_start_here_available
        ));
        body.push_str(&format!(
            "- Gate authority present: {} (expected {})\n",
            run.gate_authority_present, run.expected_gate_authority_present
        ));
        body.push_str(&format!(
            "- Required groups: `{}`\n",
            markdown_cell(&run.expected_required_groups.join(", "))
        ));
        body.push_str(&format!(
            "- Actual groups: `{}`\n",
            markdown_cell(&run.groups.join(", "))
        ));
        body.push_str(&format!(
            "- Receipt JSON: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Receipt Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!(
            "- Expected report: `{}`\n",
            normalize_path(&run.expected_report)
        ));
        body.push_str(&format!(
            "- Expected Markdown: `{}`\n",
            normalize_path(&run.expected_markdown)
        ));
        body.push_str(&format!(
            "- Actual outputs: `{}`\n",
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Generated CI Cockpit Receipts\n\n");
    body.push_str("These receipts validate the generated GitHub workflow cockpit surface. They check that the job summary starts with reviewer-first guidance, missing cockpit surfaces name regeneration commands, uploaded artifacts include the review packet, gate authority stays separate, and generated CI remains advisory by default.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Default inline comments: off\n");
    body.push_str("- Language grouping: checked for configured preview-language adapters; Rust-only config stays unchanged\n\n");
    body.push_str("| Case | Start here | Repair commands | Gate boundary | Advisory default | Artifact upload | Language grouping |\n");
    body.push_str("| --- | --- | ---: | --- | --- | --- | --- |\n");
    for run in preview_projection_runs.generated_ci_cockpit {
        body.push_str(&format!(
            "| `{}` | {} | {}/{} | {} | {} | {} | `{}` |\n",
            markdown_cell(&run.name),
            if run.start_here { "yes" } else { "no" },
            run.repair_commands,
            run.expected_repair_commands,
            if run.gate_authority_boundary {
                "yes"
            } else {
                "no"
            },
            if run.default_advisory { "yes" } else { "no" },
            if run.artifact_upload { "yes" } else { "no" },
            markdown_cell(&run.language_grouping_status)
        ));
    }
    body.push('\n');
    for run in preview_projection_runs.generated_ci_cockpit {
        body.push_str(&format!("### Generated CI `{}`\n\n", run.name));
        body.push_str(&format!("- Command: `{}`\n", markdown_cell(&run.command)));
        body.push_str(&format!("- Duration: {} ms\n", run.duration_ms));
        body.push_str(&format!("- Start here: {}\n", run.start_here));
        body.push_str(&format!(
            "- Regeneration commands: {} of {}\n",
            run.repair_commands, run.expected_repair_commands
        ));
        body.push_str(&format!(
            "- Gate authority boundary: {}\n",
            run.gate_authority_boundary
        ));
        body.push_str(&format!("- Advisory default: {}\n", run.default_advisory));
        body.push_str(&format!("- Artifact upload: {}\n", run.artifact_upload));
        body.push_str(&format!(
            "- Language grouping: `{}`\n",
            markdown_cell(&run.language_grouping_status)
        ));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Language Preview Receipts\n\n");
    body.push_str("These receipts run checked TypeScript and Python preview fixtures through `ripr check --mode fast`. They prove preview labels, structured static limits, disabled-language behavior, and no cross-language related-test routing without changing analyzer truth, editor routing, CI blocking, provider calls, source edits, generated tests, or mutation execution.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Preview adapters: opt-in through `[languages]`\n");
    body.push_str("- Receipt outputs: `target/ripr/dogfood/language-preview/<case>/check.json` and `human.txt`\n\n");
    body.push_str("| Case | Language | Enabled | Findings | Preview | Missing preview status | Related tests | Classes | Static limits |\n");
    body.push_str("| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- |\n");
    for run in preview_projection_runs.language_preview {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} | {} | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.language),
            if run.preview_enabled { "yes" } else { "no" },
            run.findings,
            run.preview_findings,
            run.missing_preview_status,
            run.related_tests,
            markdown_cell(&run.classifications.join(", ")),
            markdown_cell(&run.static_limit_kinds.join(", "))
        ));
    }
    body.push('\n');
    for run in preview_projection_runs.language_preview {
        body.push_str(&format!("### Language Preview `{}`\n\n", run.name));
        body.push_str(&format!("- Language: `{}`\n", markdown_cell(&run.language)));
        body.push_str(&format!("- Preview enabled: {}\n", run.preview_enabled));
        body.push_str(&format!("- Root: `{}`\n", normalize_path(&run.root)));
        body.push_str(&format!("- Diff: `{}`\n", normalize_path(&run.diff)));
        body.push_str(&format!(
            "- Actual outputs: `{}`\n",
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!(
            "- JSON receipt: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Human receipt: `{}`\n",
            normalize_path(&run.human_path)
        ));
        body.push_str(&format!("- Duration: {} ms\n", run.duration_ms));
        body.push_str(&format!(
            "- Findings: {} (expected {})\n",
            run.findings, run.expected_findings
        ));
        body.push_str(&format!(
            "- Preview findings: {} (expected {})\n",
            run.preview_findings, run.expected_preview_findings
        ));
        body.push_str(&format!(
            "- Missing preview status: {} (expected {})\n",
            run.missing_preview_status, run.expected_missing_preview_status
        ));
        body.push_str(&format!(
            "- Related tests: {} (expected {})\n",
            run.related_tests, run.expected_related_tests
        ));
        body.push_str(&format!(
            "- Classifications: `{}` (expected `{}`)\n",
            markdown_cell(&run.classifications.join(", ")),
            markdown_cell(&run.expected_classifications.join(", "))
        ));
        body.push_str(&format!(
            "- Static limits: `{}` (expected `{}`)\n",
            markdown_cell(&run.static_limit_kinds.join(", ")),
            markdown_cell(&run.expected_static_limit_kinds.join(", "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    let ts_preview_repair_loop_total = typescript_preview_repair_loop_runs.len();
    let ts_preview_repair_loop_typescript = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.language == "typescript")
        .count();
    let ts_preview_repair_loop_javascript = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.language == "javascript")
        .count();
    let ts_preview_repair_loop_static_limits = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.gap_state == "static_limitation")
        .count();
    let ts_preview_repair_loop_weak_oracles = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.outcome == "weak_oracle_downgraded")
        .count();
    let ts_preview_repair_loop_skipped = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.outcome == "intentionally_skipped")
        .count();
    let ts_preview_repair_loop_packet_ready = typescript_preview_repair_loop_runs
        .iter()
        .filter(|run| run.repair_packet_ready)
        .count();
    body.push_str("## TypeScript Preview Repair-Loop Receipts\n\n");
    body.push_str("These receipts pin TypeScript-family preview repair-loop evidence against checked fixture outputs. They record useful advisory routes, weak-oracle downgrades, static limitations, skipped incomplete-packet cases, and checked complete-packet receipts without claiming TypeScript parity or support-tier promotion.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Preview authority: advisory\n");
    body.push_str("- Repair packets: advisory only when `repair_packet_ready` is true\n");
    body.push_str("- Receipt input: `fixtures/typescript-preview-repair-loop/corpus.json`\n");
    body.push_str(&format!(
        "- Cases: {}; TypeScript: {}; JavaScript: {}; static limitations: {}; weak-oracle downgrades: {}; skipped: {}; packet-ready: {}\n\n",
        ts_preview_repair_loop_total,
        ts_preview_repair_loop_typescript,
        ts_preview_repair_loop_javascript,
        ts_preview_repair_loop_static_limits,
        ts_preview_repair_loop_weak_oracles,
        ts_preview_repair_loop_skipped,
        ts_preview_repair_loop_packet_ready
    ));
    body.push_str("| Case | Language | Owner | Probe | Oracle | Gap state | Actionability | Outcome | Packet ready |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for run in typescript_preview_repair_loop_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}`/`{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.language),
            markdown_cell(&run.changed_owner),
            markdown_cell(&run.probe_family),
            markdown_cell(&run.oracle_kind),
            markdown_cell(&run.oracle_strength),
            markdown_cell(&run.gap_state),
            markdown_cell(&run.actionability_category),
            markdown_cell(&run.outcome),
            if run.repair_packet_ready { "yes" } else { "no" }
        ));
    }
    body.push('\n');
    for run in typescript_preview_repair_loop_runs {
        body.push_str(&format!(
            "### TypeScript Preview Repair-Loop `{}`\n\n",
            run.name
        ));
        body.push_str(&format!(
            "- Source fixture: `{}` / `{}`\n",
            markdown_cell(&run.source_fixture),
            markdown_cell(&run.source_finding_id)
        ));
        body.push_str(&format!(
            "- Language: `{}` preview\n",
            markdown_cell(&run.language)
        ));
        body.push_str(&format!(
            "- Classification: `{}`\n",
            markdown_cell(&run.classification)
        ));
        body.push_str(&format!(
            "- Changed owner: `{}`\n",
            markdown_cell(&run.changed_owner)
        ));
        body.push_str(&format!(
            "- Probe/oracle: `{}` / `{}` `{}`\n",
            markdown_cell(&run.probe_family),
            markdown_cell(&run.oracle_kind),
            markdown_cell(&run.oracle_strength)
        ));
        body.push_str(&format!(
            "- Gap/actionability: `{}` / `{}`\n",
            markdown_cell(&run.gap_state),
            markdown_cell(&run.actionability_category)
        ));
        body.push_str(&format!(
            "- Static limit: `{}`\n",
            markdown_cell(run.static_limit_kind.as_deref().unwrap_or("none"))
        ));
        body.push_str(&format!(
            "- Repair packet ready: {}\n",
            run.repair_packet_ready
        ));
        body.push_str(&format!(
            "- Expected observer shape: `{}`\n",
            markdown_cell(&run.expected_test_or_observer_shape)
        ));
        body.push_str(&format!(
            "- Verify route: `{}` ({})\n",
            markdown_cell(&run.verify_command),
            markdown_cell(&run.verify_result)
        ));
        body.push_str(&format!(
            "- Receipt route: `{}` ({})\n",
            markdown_cell(&run.receipt_command),
            markdown_cell(&run.receipt_state)
        ));
        body.push_str(&format!(
            "- Why not actionable: {}\n",
            markdown_cell(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "- Repair route: {}\n",
            markdown_cell(&run.repair_route)
        ));
        body.push_str(&format!(
            "- Operator note: {}\n",
            markdown_cell(&run.operator_note)
        ));
        body.push_str(&format!(
            "- Must not change: `{}`\n",
            markdown_cell(&run.must_not_change.join(", "))
        ));
        body.push_str(&format!(
            "- Non-claims: `{}`\n",
            markdown_cell(&run.non_claims.join(", "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Receipt validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Receipt validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }
    let typescript_false_actionable_cases = typescript_preview_false_actionable_audit_cases_at(
        &repo_rooted_fixture_path(TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS),
    );
    let typescript_false_actionable_audit =
        dogfood_typescript_false_actionable_audit_summary(&typescript_false_actionable_cases);
    body.push_str("## TypeScript False-Actionable Audit\n\n");
    body.push_str("This advisory audit is computed from the checked TypeScript-family preview false-actionable corpus. It measures whether rows that must remain non-actionable have accidentally become repair-packet-ready, actionable, or complete-packet-shaped. It does not rerun analysis, execute TypeScript tests, edit source, generate tests, call providers, run mutation testing, change gates, contribute badge/baseline/RIPR Zero authority, or promote support tiers.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Preview authority: advisory\n");
    body.push_str(&format!(
        "- Audit input: `{}`\n",
        TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS
    ));
    body.push_str(&format!(
        "- Quality gate: `{}` - {}\n",
        markdown_cell(&typescript_false_actionable_audit.gate_status),
        markdown_cell(&typescript_false_actionable_audit.gate_reason)
    ));
    body.push_str(&format!(
        "- False actionable: {} / {} checked rows\n",
        typescript_false_actionable_audit.false_actionable,
        typescript_false_actionable_audit.must_remain_non_actionable
    ));
    body.push_str(&format!(
        "- Repair-packet-ready violations: {}; actionable gap-state violations: {}; complete-packet category violations: {}; preview-boundary violations: {}\n\n",
        typescript_false_actionable_audit.repair_packet_ready_true,
        typescript_false_actionable_audit.actionable_gap_state,
        typescript_false_actionable_audit.complete_packet_category,
        typescript_false_actionable_audit.preview_boundary_violations
    ));
    body.push_str(
        "| Case | Language | Disposition | Gap state | Actionability | False actionable |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for case in &typescript_false_actionable_cases {
        let false_actionable = case.must_remain_non_actionable
            && (case.repair_packet_ready
                || case.gap_state == "actionable"
                || case.actionability_category == "complete_repair_packet");
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&case.name),
            markdown_cell(&case.language),
            markdown_cell(&case.disposition),
            markdown_cell(&case.gap_state),
            markdown_cell(&case.actionability_category),
            if false_actionable { "yes" } else { "no" }
        ));
    }
    body.push('\n');
    let bun_ub_dogfood_total = bun_ub_cross_language_runs.len();
    let bun_ub_dogfood_discriminated = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.observed_state == "rust_ungripped_ts_discriminated")
        .count();
    let bun_ub_dogfood_missing_discriminator = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.observed_state == "rust_ungripped_ts_missing_discriminator")
        .count();
    let bun_ub_dogfood_mention_only = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.observed_state == "ts_mention_not_observer")
        .count();
    let bun_ub_dogfood_bridge_unknown = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.observed_state == "bridge_unknown")
        .count();
    let bun_ub_dogfood_named_limitation = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.observed_state == "named_static_limitation")
        .count();
    let bun_ub_dogfood_packet_ready = bun_ub_cross_language_runs
        .iter()
        .filter(|run| run.repair_packet_ready)
        .count();
    body.push_str("## Bun UB Cross-Language Witness Receipts\n\n");
    body.push_str("These receipts pin the calibrated Bun Blob stable-byte review loop against existing TypeScript/Bun preview route evidence. They record whether the #31648-style seam is TypeScript-discriminated, missing a TypeScript discriminator, or token-only, while staying preview/advisory and packet-ineligible.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Preview authority: advisory\n");
    body.push_str("- Runtime execution: none\n");
    body.push_str("- Repair packets: none\n");
    body.push_str("- Receipt input: `fixtures/bun-ub-cross-language-dogfood/corpus.json`\n");
    body.push_str(&format!(
        "- Cases: {}; TS-discriminated: {}; missing discriminator: {}; mention-only: {}; bridge-unknown: {}; named limitation: {}; packet-ready: {}\n\n",
        bun_ub_dogfood_total,
        bun_ub_dogfood_discriminated,
        bun_ub_dogfood_missing_discriminator,
        bun_ub_dogfood_mention_only,
        bun_ub_dogfood_bridge_unknown,
        bun_ub_dogfood_named_limitation,
        bun_ub_dogfood_packet_ready
    ));
    body.push_str(
        "| Case | Source | Route quality | State | Action | Suggested file | Packet ready |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for run in bun_ub_cross_language_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.source_case),
            markdown_cell(&run.route_quality_case),
            markdown_cell(&run.observed_state),
            markdown_cell(&run.operator_action),
            markdown_cell(&run.suggested_test_file),
            if run.repair_packet_ready { "yes" } else { "no" }
        ));
    }
    body.push('\n');
    for run in bun_ub_cross_language_runs {
        body.push_str(&format!("### Bun UB Cross-Language `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Rust seam: `{}` / `{}` / `{}`\n",
            markdown_cell(&run.rust_file),
            markdown_cell(&run.rust_owner),
            markdown_cell(&run.rust_boundary)
        ));
        body.push_str(&format!(
            "- TypeScript test file: `{}`\n",
            markdown_cell(&run.ts_test_file)
        ));
        body.push_str(&format!(
            "- Expected/observed state: `{}` / `{}`\n",
            markdown_cell(&run.expected_state),
            markdown_cell(&run.observed_state)
        ));
        body.push_str(&format!(
            "- Missing discriminators: `{}`\n",
            markdown_cell(&run.missing_discriminators.join(", "))
        ));
        body.push_str(&format!(
            "- Missing graph legs: `{}`\n",
            markdown_cell(&run.missing_graph_legs.join(", "))
        ));
        body.push_str(&format!(
            "- Suggested test file: `{}`\n",
            markdown_cell(&run.suggested_test_file)
        ));
        body.push_str(&format!(
            "- Manual verdict: `{}`\n",
            markdown_cell(&run.manual_verdict)
        ));
        body.push_str(&format!(
            "- Operator action: `{}`\n",
            markdown_cell(&run.operator_action)
        ));
        body.push_str(&format!(
            "- Review before: {}\n",
            markdown_cell(&run.review_before)
        ));
        body.push_str(&format!(
            "- Review after: {}\n",
            markdown_cell(&run.review_after)
        ));
        body.push_str(&format!(
            "- Bridge verdict: `{}`\n",
            markdown_cell(&run.bridge_verdict)
        ));
        body.push_str(&format!(
            "- Placement verdict: `{}`\n",
            markdown_cell(&run.placement_verdict)
        ));
        body.push_str(&format!(
            "- Proof mode: `{}`\n",
            markdown_cell(&run.proof_mode)
        ));
        body.push_str(&format!(
            "- Receipt state: `{}`\n",
            markdown_cell(&run.receipt_state)
        ));
        body.push_str(&format!(
            "- Authority boundary: `{}`\n",
            markdown_cell(&run.authority_boundary)
        ));
        body.push_str(&format!(
            "- Non-claims: `{}`\n",
            markdown_cell(&run.non_claims.join(", "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Receipt validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Receipt validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }
    body.push_str("## Editor Gap Cockpit Receipts\n\n");
    body.push_str("These receipts validate checked `fixtures/editor_gap_cockpit` projections for the local repair cockpit. They verify actionable Rust repair routing, preview static-limit ordering, disabled-language no-diagnostic state, wrong-root and stale fail-closed behavior, and no-action refresh-only behavior without changing analyzer truth, source files, generated tests, provider calls, mutation execution, policy, gates, or PR comments.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Editor behavior: saved-workspace and projection-only\n");
    body.push_str("- Receipt outputs: `fixtures/editor_gap_cockpit/<case>/expected/*`\n\n");
    body.push_str(
        "| Case | State | Language | Diagnostics | Fail closed | Actions | Static limit |\n",
    );
    body.push_str("| --- | --- | --- | ---: | --- | --- | --- |\n");
    for run in preview_projection_runs.editor_gap_cockpit {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.state),
            markdown_cell(run.language.as_deref().unwrap_or("not_projected")),
            run.diagnostics_projected,
            if run.fail_closed { "yes" } else { "no" },
            markdown_cell(&run.actions_projected.join(", ")),
            markdown_cell(run.static_limit_kind.as_deref().unwrap_or(""))
        ));
    }
    body.push('\n');
    for run in preview_projection_runs.editor_gap_cockpit {
        body.push_str(&format!("### Editor Gap Cockpit `{}`\n\n", run.name));
        body.push_str(&format!("- State: `{}`\n", markdown_cell(&run.state)));
        body.push_str(&format!(
            "- Expected state: `{}`\n",
            markdown_cell(&run.expected_state)
        ));
        body.push_str(&format!(
            "- Language: `{}` (expected `{}`)\n",
            markdown_cell(run.language.as_deref().unwrap_or("not_projected")),
            markdown_cell(run.expected_language.as_deref().unwrap_or("not_projected"))
        ));
        body.push_str(&format!(
            "- Language status: `{}` (expected `{}`)\n",
            markdown_cell(run.language_status.as_deref().unwrap_or("not_projected")),
            markdown_cell(
                run.expected_language_status
                    .as_deref()
                    .unwrap_or("not_projected")
            )
        ));
        body.push_str(&format!(
            "- Diagnostics: {} projected, {} actual, expected {}\n",
            run.diagnostics_projected, run.actual_diagnostics, run.expected_diagnostics
        ));
        body.push_str(&format!(
            "- Fail closed: {} (expected {})\n",
            run.fail_closed, run.expected_fail_closed
        ));
        body.push_str(&format!(
            "- Actions: `{}` (expected `{}`; actual count {})\n",
            markdown_cell(&run.actions_projected.join(", ")),
            markdown_cell(&run.expected_actions.join(", ")),
            run.actual_actions
        ));
        body.push_str(&format!(
            "- Static limit: `{}` (expected `{}`)\n",
            markdown_cell(run.static_limit_kind.as_deref().unwrap_or("")),
            markdown_cell(run.expected_static_limit_kind.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "- Static limit before action: {}\n",
            run.hover_static_before_action
        ));
        body.push_str(&format!(
            "- Projection JSON: `{}`\n",
            normalize_path(&run.projection_path)
        ));
        body.push_str(&format!(
            "- Diagnostics JSON: `{}`\n",
            normalize_path(&run.diagnostics_path)
        ));
        body.push_str(&format!("- Hover: `{}`\n", normalize_path(&run.hover_path)));
        body.push_str(&format!(
            "- Code actions JSON: `{}`\n",
            normalize_path(&run.code_actions_path)
        ));
        body.push_str(&format!(
            "- VS Code status JSON: `{}`\n",
            normalize_path(&run.status_path)
        ));
        body.push_str(&format!(
            "- Expected directory: `{}`\n",
            normalize_path(&run.expected_dir)
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Editor First-PR Bridge Receipts\n\n");
    body.push_str("These receipts validate checked `fixtures/editor_first_pr_bridge` projections for the local repair -> first-pr handoff. They verify packet-missing, repairable, no-action, stale, wrong-root, malformed, receipt-improved, and receipt-unchanged states without creating first-pr packets, publishing PR comments, composing generated CI summaries, editing source, generating tests, calling providers, running mutation, or deciding gates.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Editor behavior: saved-workspace read-only projection over existing first-pr artifacts\n");
    body.push_str("- Receipt outputs: `fixtures/editor_first_pr_bridge/<case>/expected/*`\n\n");
    body.push_str("| Case | Packet state | Diagnostics | Fail closed | Safe actions | Suppressed actions | Receipt movement |\n");
    body.push_str("| --- | --- | ---: | --- | --- | --- | --- |\n");
    for run in preview_projection_runs.editor_first_pr_bridge {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | {} | `{}` | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.packet_state),
            run.diagnostics,
            if run.fail_closed { "yes" } else { "no" },
            markdown_cell(&run.safe_actions.join(", ")),
            markdown_cell(&run.suppressed_actions.join(", ")),
            markdown_cell(run.receipt_movement.as_deref().unwrap_or(""))
        ));
    }
    body.push('\n');
    for run in preview_projection_runs.editor_first_pr_bridge {
        body.push_str(&format!("### Editor First-PR Bridge `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Packet state: `{}` (expected `{}`)\n",
            markdown_cell(&run.packet_state),
            markdown_cell(&run.expected_packet_state)
        ));
        body.push_str(&format!(
            "- Diagnostics: {} (expected {})\n",
            run.diagnostics, run.expected_diagnostics
        ));
        body.push_str(&format!(
            "- Fail closed: {} (expected {})\n",
            run.fail_closed, run.expected_fail_closed
        ));
        body.push_str(&format!(
            "- Safe actions: `{}` (expected `{}`)\n",
            markdown_cell(&run.safe_actions.join(", ")),
            markdown_cell(&run.expected_safe_actions.join(", "))
        ));
        body.push_str(&format!(
            "- Suppressed actions: `{}` (expected `{}`)\n",
            markdown_cell(&run.suppressed_actions.join(", ")),
            markdown_cell(&run.expected_suppressed_actions.join(", "))
        ));
        body.push_str(&format!(
            "- First-pr action commands: `{}`\n",
            markdown_cell(&run.first_pr_actions.join(", "))
        ));
        body.push_str(&format!(
            "- All action commands: `{}`\n",
            markdown_cell(&run.action_commands.join(", "))
        ));
        body.push_str(&format!(
            "- Receipt movement: `{}` (expected `{}`)\n",
            markdown_cell(run.receipt_movement.as_deref().unwrap_or("")),
            markdown_cell(run.expected_receipt_movement.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "- Non-claims: runtime={}, mutation={}, policy_gate={}, pr_ready={}\n",
            run.runtime_adequacy_claim,
            run.mutation_proof_claim,
            run.policy_gate_claim,
            run.pr_ready_claim
        ));
        body.push_str(&format!(
            "- First-pr status JSON: `{}`\n",
            normalize_path(&run.packet_path)
        ));
        body.push_str(&format!(
            "- Diagnostics JSON: `{}`\n",
            normalize_path(&run.diagnostics_path)
        ));
        body.push_str(&format!(
            "- Code actions JSON: `{}`\n",
            normalize_path(&run.code_actions_path)
        ));
        body.push_str(&format!(
            "- VS Code status JSON: `{}`\n",
            normalize_path(&run.status_path)
        ));
        body.push_str(&format!(
            "- Setup diagnosis: `{}`\n",
            normalize_path(&run.diagnosis_path)
        ));
        body.push_str(&format!(
            "- Expected directory: `{}`\n",
            normalize_path(&run.expected_dir)
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Finding Alignment Receipts\n\n");
    body.push_str("These receipts validate real RIPR PR examples of the raw-finding -> canonical-item -> user-outcome model. They keep raw findings as supporting evidence, require canonical item counts and user outcomes, and check that actionable items have repair and verification routes while static limitations name analyzer repair routes. They do not change PR/CI rendering, LSP/editor behavior, gates, public scores, generated tests, provider calls, source edits, or mutation execution.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Receipt input: `fixtures/finding-alignment-dogfood/corpus.json`\n\n");
    body.push_str("| Case | PR | Gap ID | Class | Raw -> canonical | State | Actionability | Outcome | Repair | Verify | Static limitation |\n");
    body.push_str("| --- | --- | --- | --- | ---: | --- | --- | --- | --- | --- | --- |\n");
    for run in finding_alignment_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} -> {} | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.source_pr),
            markdown_cell(&run.canonical_gap_id),
            markdown_cell(&run.evidence_class),
            run.raw_findings_total,
            run.canonical_items_total,
            markdown_cell(&run.gap_state),
            markdown_cell(&run.actionability),
            markdown_cell(&run.user_outcome),
            markdown_cell(&run.repair_kind),
            markdown_cell(&run.verify_command),
            markdown_cell(run.static_limitation_category.as_deref().unwrap_or("none"))
        ));
    }
    body.push('\n');
    for run in finding_alignment_runs {
        body.push_str(&format!("### Finding Alignment `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Source PR: `{}`\n",
            markdown_cell(&run.source_pr)
        ));
        body.push_str(&format!(
            "- Canonical gap ID: `{}`\n",
            markdown_cell(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "- Evidence class: `{}`\n",
            markdown_cell(&run.evidence_class)
        ));
        body.push_str(&format!(
            "- Counts: {} raw finding(s) -> {} canonical item(s)\n",
            run.raw_findings_total, run.canonical_items_total
        ));
        body.push_str(&format!(
            "- Raw finding summary: {}\n",
            markdown_cell(&run.raw_finding_summary)
        ));
        body.push_str(&format!(
            "- Gap state: `{}`\n",
            markdown_cell(&run.gap_state)
        ));
        body.push_str(&format!(
            "- Actionability: `{}`\n",
            markdown_cell(&run.actionability)
        ));
        body.push_str(&format!(
            "- User outcome: `{}`\n",
            markdown_cell(&run.user_outcome)
        ));
        body.push_str(&format!(
            "- Recommended repair: {}\n",
            markdown_cell(&run.recommended_repair)
        ));
        body.push_str(&format!(
            "- Before/after context: {}\n",
            markdown_cell(&run.before_after_context)
        ));
        body.push_str(&format!(
            "- Repair kind: `{}` / target `{}`\n",
            markdown_cell(&run.repair_kind),
            markdown_cell(&run.target_test_type)
        ));
        body.push_str(&format!(
            "- Verify command: `{}`\n",
            markdown_cell(&run.verify_command)
        ));
        body.push_str(&format!(
            "- Static limitation: `{}` via `{}`\n",
            markdown_cell(run.static_limitation_category.as_deref().unwrap_or("none")),
            markdown_cell(
                run.static_limitation_repair_route
                    .as_deref()
                    .unwrap_or("none")
            )
        ));
        body.push_str(&format!(
            "- Raw findings supporting-only: {}\n",
            run.raw_findings_supporting_only
        ));
        body.push_str(&format!(
            "- Must not claim: `{}`\n",
            markdown_cell(&run.must_not_claim.join("; "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Surface Projection Alignment Receipts\n\n");
    body.push_str("These receipts validate that one receipt-backed canonical gap preserves identity, repair route, verify command, receipt command/state, and readiness `top_next_action` across swarm-attempt-ledger and swarm-readiness projections. Badge, LSP, PR, and CI remain advisory consumers of the canonical report state; this dogfood proof does not change their rendering, ranking, gate authority, source edits, generated tests, provider calls, or mutation execution.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Receipt input: `fixtures/surface-projection-alignment/corpus.json`\n\n");
    body.push_str("| Case | Gap ID | Packet | Repair | Outcome | Top next action | Attempted | Improved | Advisory consumers |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- | ---: | ---: | --- |\n");
    for run in surface_projection_alignment_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.canonical_gap_id),
            markdown_cell(&run.packet_id),
            markdown_cell(&run.repair_kind),
            markdown_cell(&run.outcome),
            markdown_cell(&run.top_next_action_kind),
            run.attempted_packets,
            run.improved_packets,
            markdown_cell(&run.advisory_consumers.join(", "))
        ));
    }
    body.push('\n');
    for run in surface_projection_alignment_runs {
        body.push_str(&format!(
            "### Surface Projection Alignment `{}`\n\n",
            run.name
        ));
        body.push_str(&format!(
            "- Canonical gap ID: `{}`\n",
            markdown_cell(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "- Packet ID: `{}`\n",
            markdown_cell(&run.packet_id)
        ));
        body.push_str(&format!(
            "- Repair kind: `{}`\n",
            markdown_cell(&run.repair_kind)
        ));
        body.push_str(&format!(
            "- Verify command: `{}`\n",
            markdown_cell(&run.verify_command)
        ));
        body.push_str(&format!(
            "- Receipt command: `{}`\n",
            markdown_cell(&run.receipt_command)
        ));
        body.push_str(&format!(
            "- Receipt state: `{}`\n",
            markdown_cell(&run.receipt_state)
        ));
        body.push_str(&format!("- Outcome: `{}`\n", markdown_cell(&run.outcome)));
        body.push_str(&format!(
            "- Readiness status: `{}`\n",
            markdown_cell(&run.readiness_status)
        ));
        body.push_str(&format!(
            "- Top next action: `{}` via `{}`\n",
            markdown_cell(&run.top_next_action_kind),
            markdown_cell(run.top_next_action_command.as_deref().unwrap_or("none"))
        ));
        body.push_str(&format!(
            "- Advisory consumers: `{}`\n",
            markdown_cell(&run.advisory_consumers.join(", "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    let real_attempts_total = real_repair_attempt_runs.len();
    let real_attempts_improved = real_repair_attempt_runs
        .iter()
        .filter(|run| run.outcome == "evidence_improved")
        .count();
    let real_attempts_unchanged = real_repair_attempt_runs
        .iter()
        .filter(|run| run.outcome == "evidence_unchanged")
        .count();
    let real_attempts_regressed = real_repair_attempt_runs
        .iter()
        .filter(|run| run.outcome == "evidence_regressed")
        .count();
    let real_attempts_resolved = real_repair_attempt_runs
        .iter()
        .filter(|run| run.outcome == "resolved")
        .count();
    let real_attempts_missing_receipt = real_repair_attempt_runs
        .iter()
        .filter(|run| run.outcome == "attempted_no_receipt")
        .count();
    body.push_str("## Real Repair Attempt Receipts\n\n");
    body.push_str("These receipts record repo-local repair attempts that exercised the Lane 1 packet loop against real PR or handoff evidence. The set intentionally includes non-win outcomes so unchanged or missing-receipt attempts remain visible instead of being curated away.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Receipt input: `fixtures/real-repair-attempts/corpus.json`\n");
    body.push_str(&format!(
        "- Attempted: {}; improved: {}; unchanged: {}; regressed: {}; resolved: {}; missing receipt: {}\n\n",
        real_attempts_total,
        real_attempts_improved,
        real_attempts_unchanged,
        real_attempts_regressed,
        real_attempts_resolved,
        real_attempts_missing_receipt
    ));
    body.push_str("| Case | Source | Language | Gap ID | Packet | Repair | Verify | Outcome |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for run in real_repair_attempt_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.source_ref),
            markdown_cell(run.language.as_deref().unwrap_or("unspecified")),
            markdown_cell(&run.canonical_gap_id),
            markdown_cell(&run.packet_id),
            markdown_cell(&run.repair_kind),
            markdown_cell(&run.verify_result),
            markdown_cell(&run.outcome)
        ));
    }
    body.push('\n');
    for run in real_repair_attempt_runs {
        body.push_str(&format!("### Real Repair Attempt `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Canonical gap ID: `{}`\n",
            markdown_cell(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "- Packet ID: `{}`\n",
            markdown_cell(&run.packet_id)
        ));
        body.push_str(&format!(
            "- Language: `{}`\n",
            markdown_cell(run.language.as_deref().unwrap_or("unspecified"))
        ));
        body.push_str(&format!(
            "- Evidence class: `{}`\n",
            markdown_cell(run.evidence_class.as_deref().unwrap_or("unspecified"))
        ));
        body.push_str(&format!(
            "- Source file: `{}`\n",
            markdown_cell(run.source_file.as_deref().unwrap_or("unspecified"))
        ));
        body.push_str(&format!(
            "- Target shape: `{}`\n",
            markdown_cell(&run.target_test_or_observer_shape)
        ));
        body.push_str(&format!(
            "- Verify command: `{}`\n",
            markdown_cell(&run.verify_command)
        ));
        body.push_str(&format!(
            "- Verify result: `{}`\n",
            markdown_cell(&run.verify_result)
        ));
        body.push_str(&format!(
            "- Receipt command: `{}`\n",
            markdown_cell(&run.receipt_command)
        ));
        body.push_str(&format!(
            "- Receipt path: `{}`\n",
            markdown_cell(run.receipt_path.as_deref().unwrap_or("missing"))
        ));
        body.push_str(&format!(
            "- Receipt state: `{}`\n",
            markdown_cell(&run.receipt_state)
        ));
        body.push_str(&format!(
            "- Before/after: `{}` -> `{}`\n",
            markdown_cell(&run.before_gap_state),
            markdown_cell(&run.after_gap_state)
        ));
        body.push_str(&format!(
            "- Attempted repair: {}\n",
            markdown_cell(&run.attempted_repair)
        ));
        body.push_str(&format!(
            "- Evidence movement: {}\n",
            markdown_cell(&run.evidence_movement)
        ));
        body.push_str(&format!(
            "- Operator note: {}\n",
            markdown_cell(&run.operator_note)
        ));
        if let Some(reason) = &run.missing_receipt_reason {
            body.push_str(&format!(
                "- Missing receipt reason: {}\n",
                markdown_cell(reason)
            ));
        }
        body.push_str(&format!(
            "- Must not change: `{}`\n",
            markdown_cell(&run.must_not_change.join(", "))
        ));
        body.push_str(&format!(
            "- Raw evidence refs: `{}`\n",
            markdown_cell(&run.raw_evidence_refs.join(", "))
        ));
        if run.errors.is_empty() {
            body.push_str("- Receipt validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Receipt validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }

    let python_real_repo_eval_total = python_real_repo_eval_runs.len();
    let python_static_limit_eval_runs = inputs.python_static_limit_eval_runs;
    let python_no_action_eval_runs = inputs.python_no_action_eval_runs;
    let python_real_repo_eval_closed = python_real_repo_eval_runs
        .iter()
        .filter(|run| run.gap_movement == "closed")
        .count();
    let python_real_repo_eval_usable = python_real_repo_eval_runs
        .iter()
        .filter(|run| run.usability == "usable")
        .count();
    let python_repair_quality =
        dogfood_python_repair_routing_quality_summary(python_real_repo_eval_runs);
    let python_static_limit_distribution =
        dogfood_python_static_limit_eval_distribution(python_static_limit_eval_runs);
    let python_no_action_distribution =
        dogfood_python_no_action_eval_distribution(python_no_action_eval_runs);
    body.push_str("## Python Real-Repo Eval Receipts\n\n");
    body.push_str("These receipts record Python repair-routing runs outside analyzer fixture goldens. They are advisory dogfood evidence for the repair card -> verify -> receipt loop, not support-tier promotion or runtime adequacy proof.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Receipt input: `fixtures/python-real-repo-evals/corpus.json`\n");
    body.push_str(&format!(
        "- Repair cases: {}; closed gaps: {}; usable recommendations: {}; static-limit no-action cases: {}; ordinary no-action cases: {}\n\n",
        python_real_repo_eval_total,
        python_real_repo_eval_closed,
        python_real_repo_eval_usable,
        python_static_limit_eval_runs.len(),
        python_no_action_eval_runs.len()
    ));
    body.push_str(
        "| Case | Repo shape | Source | Gap ID | Verify | Receipt | Movement | Usability |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for run in python_real_repo_eval_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.repo_shape),
            markdown_cell(&run.source_kind),
            markdown_cell(&run.canonical_gap_id),
            markdown_cell(&run.verify_result),
            markdown_cell(&run.receipt_result),
            markdown_cell(&run.gap_movement),
            markdown_cell(&run.usability)
        ));
    }
    body.push('\n');
    body.push_str("## Python Repair-Routing Quality Metrics\n\n");
    body.push_str("These metrics are computed from the checked Python real-repo eval receipts. They measure top repair-card usefulness and closure evidence; they are not support-tier promotion and do not make Python gate eligible.\n\n");
    body.push_str(&format!(
        "- Quality gate: `{}` - {}\n",
        markdown_cell(&python_repair_quality.gate_status),
        markdown_cell(&python_repair_quality.gate_reason)
    ));
    body.push_str(&format!(
        "- Top-3 actionable precision: {} / {} ranked findings - fewer-than-three eval outputs must document a stop reason.\n\n",
        python_repair_quality.top_3_actionable_usable,
        python_repair_quality.top_3_ranked_findings_checked
    ));
    body.push_str(&format!(
        "- Full top-3 capture cases: {} / {} evals - at least one eval must preserve all three ranked repair cards.\n\n",
        python_repair_quality.full_top_3_capture_cases, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "- Static-limit no-action cases: {} - tracked separately from repair-card success metrics.\n\n",
        python_static_limit_eval_runs.len()
    ));
    body.push_str(&format!(
        "- Ordinary no-action cases: {} - tracked separately from repair-card success metrics and from static analyzer limitations.\n\n",
        python_no_action_eval_runs.len()
    ));
    body.push_str("| Metric | Passing / Checked |\n");
    body.push_str("| --- | --- |\n");
    body.push_str(&format!(
        "| Top-1 actionable precision | {} / {} |\n",
        python_repair_quality.top_1_actionable_usable, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Top-3 actionable precision | {} / {} |\n",
        python_repair_quality.top_3_actionable_usable,
        python_repair_quality.top_3_ranked_findings_checked
    ));
    body.push_str(&format!(
        "| Full top-3 capture cases | {} / {} |\n",
        python_repair_quality.full_top_3_capture_cases, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Verify-command validity | {} / {} |\n",
        python_repair_quality.verify_command_valid, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Agent-packet boundary validity | {} / {} |\n",
        python_repair_quality.agent_packet_bounded, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Concrete discriminator rate | {} / {} |\n",
        python_repair_quality.concrete_discriminator, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Related test-location rate | {} / {} |\n",
        python_repair_quality.suggested_test_location, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Receipt closure rate | {} / {} |\n",
        python_repair_quality.receipt_closed, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| False-actionable rate | {} / {} |\n",
        python_repair_quality.false_actionable, python_repair_quality.cases
    ));
    body.push_str(&format!(
        "| Crash rate | {} / {} |\n\n",
        python_repair_quality.crashes, python_repair_quality.cases
    ));
    body.push_str("| Unsupported limitation | Cases |\n");
    body.push_str("| --- | --- |\n");
    if python_repair_quality
        .unsupported_limitation_distribution
        .is_empty()
    {
        body.push_str("| none recorded | 0 |\n\n");
    } else {
        for (limitation, count) in &python_repair_quality.unsupported_limitation_distribution {
            body.push_str(&format!(
                "| `{}` | {} |\n",
                markdown_cell(limitation),
                count
            ));
        }
        body.push('\n');
    }
    body.push_str("| No-action static limitation | Cases |\n");
    body.push_str("| --- | --- |\n");
    if python_static_limit_distribution.is_empty() {
        body.push_str("| none recorded | 0 |\n\n");
    } else {
        for (limitation, count) in &python_static_limit_distribution {
            body.push_str(&format!(
                "| `{}` | {} |\n",
                markdown_cell(limitation),
                count
            ));
        }
        body.push('\n');
    }
    body.push_str("| Ordinary no-action state | Cases |\n");
    body.push_str("| --- | --- |\n");
    if python_no_action_distribution.is_empty() {
        body.push_str("| none recorded | 0 |\n\n");
    } else {
        for (state, count) in &python_no_action_distribution {
            body.push_str(&format!("| `{}` | {} |\n", markdown_cell(state), count));
        }
        body.push('\n');
    }
    for run in python_real_repo_eval_runs {
        body.push_str(&format!("### Python Real-Repo Eval `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Source: `{}` / `{}`\n",
            markdown_cell(&run.source_kind),
            markdown_cell(&run.source_ref)
        ));
        body.push_str(&format!(
            "- Command: `{}` ({} ms)\n",
            markdown_cell(&run.command),
            run.runtime_ms
        ));
        body.push_str(&format!(
            "- Top finding: {}\n",
            markdown_cell(&run.top_finding_summary)
        ));
        body.push_str(&format!(
            "- Changed owner: `{}`\n",
            markdown_cell(&run.changed_owner)
        ));
        body.push_str(&format!(
            "- Missing discriminator: `{}`\n",
            markdown_cell(&run.missing_discriminator)
        ));
        body.push_str(&format!(
            "- Suggested test: `{}` / `{}`\n",
            markdown_cell(&run.suggested_test_file),
            markdown_cell(&run.suggested_test_name)
        ));
        body.push_str(&format!(
            "- Repair card present: {}; action: `{}`\n",
            run.repair_card_present,
            markdown_cell(&run.repair_action)
        ));
        body.push_str(&format!(
            "- Agent packet present: {}; task: `{}`\n",
            run.agent_packet_present,
            markdown_cell(&run.agent_packet_task)
        ));
        body.push_str(&format!(
            "- Agent packet command: `{}`\n",
            markdown_cell(&run.agent_packet_command)
        ));
        body.push_str(&format!(
            "- Agent packet allowed files: `{}`\n",
            markdown_cell(&run.agent_packet_allowed_files.join(", "))
        ));
        body.push_str(&format!(
            "- Agent packet forbidden files: `{}`\n",
            markdown_cell(&run.agent_packet_forbidden_files.join(", "))
        ));
        body.push_str(&format!(
            "- Agent packet stop-if: `{}`\n",
            markdown_cell(&run.agent_packet_stop_if.join("; "))
        ));
        body.push_str(&format!(
            "- Verify command: `{}` ({}) - {}\n",
            markdown_cell(&run.verify_command),
            markdown_cell(&run.verify_result),
            markdown_cell(&run.verify_summary)
        ));
        body.push_str(&format!(
            "- After command: `{}` ({} ms)\n",
            markdown_cell(&run.after_command),
            run.after_runtime_ms
        ));
        body.push_str(&format!(
            "- Receipt command: `{}` ({})\n",
            markdown_cell(&run.receipt_command),
            markdown_cell(&run.receipt_result)
        ));
        body.push_str(&format!(
            "- Gap movement: `{}`; closed gaps: {}\n",
            markdown_cell(&run.gap_movement),
            run.closed_gaps
        ));
        body.push_str(&format!(
            "- False-positive notes: {}\n",
            markdown_cell(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "- Limitation notes: {}\n",
            markdown_cell(&run.limitation_notes)
        ));
        if !run.unsupported_limitations.is_empty() {
            body.push_str(&format!(
                "- Unsupported limitations: `{}`\n",
                markdown_cell(&run.unsupported_limitations.join(", "))
            ));
        }
        body.push_str(&format!(
            "- Ranked top-3 findings checked: {}\n",
            run.ranked_top_3_findings.len()
        ));
        if let Some(reason) = &run.ranked_top_3_limit_reason {
            body.push_str(&format!(
                "- Ranked top-3 limit reason: {}\n",
                markdown_cell(reason)
            ));
        }
        for finding in &run.ranked_top_3_findings {
            body.push_str(&format!(
                "  - Rank {}: `{}`; discriminator `{}`; verify `{}`; usability `{}`\n",
                finding.rank,
                markdown_cell(&finding.canonical_gap_id),
                markdown_cell(&finding.missing_discriminator),
                markdown_cell(&finding.verify_command),
                markdown_cell(&finding.usability)
            ));
        }
        body.push_str(&format!(
            "- Claim boundary: `{}`\n",
            markdown_cell(&run.claim_boundary.join("; "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Receipt validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Receipt validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }

    body.push_str("## Python Static-Limit Eval Receipts\n\n");
    body.push_str("These checked no-action cases record where Python repair routing stops instead of emitting a repair card or agent packet. They protect the fail-closed side of the lane and are not counted as successful repair recommendations.\n\n");
    body.push_str(
        "| Case | Repo shape | Static limit | Finding | Related test | Packet | Reason |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for run in python_static_limit_eval_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` / `{}` | `{}` | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.repo_shape),
            markdown_cell(&run.static_limit_kind),
            markdown_cell(&run.finding_id),
            markdown_cell(&run.related_test_file),
            markdown_cell(&run.related_test_name),
            if run.agent_packet_present {
                "emitted"
            } else {
                "none"
            },
            markdown_cell(&run.why_not_actionable)
        ));
    }
    body.push('\n');
    for run in python_static_limit_eval_runs {
        body.push_str(&format!("### Python Static-Limit Eval `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Source: `{}` / `{}`\n",
            markdown_cell(&run.source_kind),
            markdown_cell(&run.source_ref)
        ));
        body.push_str(&format!(
            "- Command: `{}` ({} ms)\n",
            markdown_cell(&run.command),
            run.runtime_ms
        ));
        body.push_str(&format!(
            "- Changed owner: `{}`\n",
            markdown_cell(&run.changed_owner)
        ));
        body.push_str(&format!(
            "- Static limit: `{}`; classification: `{}`\n",
            markdown_cell(&run.static_limit_kind),
            markdown_cell(&run.classification)
        ));
        body.push_str(&format!(
            "- Stop reasons: `{}`\n",
            markdown_cell(&run.stop_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Related test: `{}` / `{}`\n",
            markdown_cell(&run.related_test_file),
            markdown_cell(&run.related_test_name)
        ));
        body.push_str(&format!(
            "- Why not actionable: {}\n",
            markdown_cell(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "- Repair card present: {}; agent packet present: {}\n",
            run.repair_card_present, run.agent_packet_present
        ));
        body.push_str(&format!(
            "- Verify: `{}` ({})\n",
            markdown_cell(&run.verify_command),
            markdown_cell(&run.verify_result)
        ));
        body.push_str(&format!(
            "- Receipt: `{}` ({})\n",
            markdown_cell(&run.receipt_command),
            markdown_cell(&run.receipt_result)
        ));
        body.push_str(&format!(
            "- Gap movement: `{}`\n",
            markdown_cell(&run.gap_movement)
        ));
        body.push_str(&format!(
            "- False-positive notes: {}\n",
            markdown_cell(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "- Limitation notes: {}\n",
            markdown_cell(&run.limitation_notes)
        ));
        body.push_str(&format!(
            "- Claim boundary: `{}`\n",
            markdown_cell(&run.claim_boundary.join("; "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Static-limit validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Static-limit validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }

    body.push_str("## Python No-Action Eval Receipts\n\n");
    body.push_str("These checked no-action cases record ordinary routing stops where no bounded repair card or agent packet is safe even though the analyzer did not hit a static limitation. They keep visible no-path evidence separate from repair-card success metrics and static-limit dogfood.\n\n");
    body.push_str(
        "| Case | Repo shape | No-action state | Finding | Related test | Packet | Reason |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for run in python_no_action_eval_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` / `{}` | `{}` | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.repo_shape),
            markdown_cell(&run.no_action_kind),
            markdown_cell(&run.finding_id),
            markdown_cell(&run.related_test_file),
            markdown_cell(&run.related_test_name),
            if run.agent_packet_present {
                "emitted"
            } else {
                "none"
            },
            markdown_cell(&run.why_not_actionable)
        ));
    }
    body.push('\n');
    for run in python_no_action_eval_runs {
        body.push_str(&format!("### Python No-Action Eval `{}`\n\n", run.name));
        body.push_str(&format!(
            "- Source: `{}` / `{}`\n",
            markdown_cell(&run.source_kind),
            markdown_cell(&run.source_ref)
        ));
        body.push_str(&format!(
            "- Command: `{}` ({} ms)\n",
            markdown_cell(&run.command),
            run.runtime_ms
        ));
        body.push_str(&format!(
            "- Changed owner: `{}`\n",
            markdown_cell(&run.changed_owner)
        ));
        body.push_str(&format!(
            "- No-action state: `{}`; classification: `{}`\n",
            markdown_cell(&run.no_action_kind),
            markdown_cell(&run.classification)
        ));
        body.push_str(&format!(
            "- Stop reasons: `{}`\n",
            markdown_cell(&run.stop_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Related test: `{}` / `{}`\n",
            markdown_cell(&run.related_test_file),
            markdown_cell(&run.related_test_name)
        ));
        body.push_str(&format!(
            "- Why not actionable: {}\n",
            markdown_cell(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "- Repair card present: {}; agent packet present: {}\n",
            run.repair_card_present, run.agent_packet_present
        ));
        body.push_str(&format!(
            "- Verify: `{}` ({})\n",
            markdown_cell(&run.verify_command),
            markdown_cell(&run.verify_result)
        ));
        body.push_str(&format!(
            "- Receipt: `{}` ({})\n",
            markdown_cell(&run.receipt_command),
            markdown_cell(&run.receipt_result)
        ));
        body.push_str(&format!(
            "- Gap movement: `{}`\n",
            markdown_cell(&run.gap_movement)
        ));
        body.push_str(&format!(
            "- False-positive notes: {}\n",
            markdown_cell(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "- Limitation notes: {}\n",
            markdown_cell(&run.limitation_notes)
        ));
        body.push_str(&format!(
            "- Claim boundary: `{}`\n",
            markdown_cell(&run.claim_boundary.join("; "))
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- No-action validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- No-action validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }

    body.push_str("## User Surface Projection Alignment Receipts\n\n");
    body.push_str("These receipts validate that badge, LSP, PR comment, and CI projection examples consume canonical repair state for full runs and canonical runtime state for limited runs instead of independently interpreting raw findings. They keep all four surfaces advisory by default and require limited/stale state visibility.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Receipt input: `fixtures/user-surface-projection-alignment/corpus.json`\n\n");
    body.push_str(
        "| Case | Surface | Headline | Basis | Gap ID | Packet | Advisory | Raw headline |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for run in user_surface_projection_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.surface),
            markdown_cell(&run.headline),
            markdown_cell(&run.projection_basis),
            markdown_cell(&run.canonical_gap_id),
            markdown_cell(&run.packet_id),
            if run.advisory { "yes" } else { "no" },
            if run.raw_findings_headline {
                "yes"
            } else {
                "no"
            }
        ));
    }
    body.push('\n');
    for run in user_surface_projection_runs {
        body.push_str(&format!("### User Surface Projection `{}`\n\n", run.name));
        body.push_str(&format!("- Surface: `{}`\n", markdown_cell(&run.surface)));
        body.push_str(&format!("- Artifact: `{}`\n", markdown_cell(&run.artifact)));
        body.push_str(&format!(
            "- Run status: `{}`\n",
            markdown_cell(&run.run_status)
        ));
        body.push_str(&format!(
            "- Top next action: `{}`\n",
            markdown_cell(&run.top_next_action_kind)
        ));
        body.push_str(&format!(
            "- Repair kind: `{}`\n",
            markdown_cell(&run.repair_kind)
        ));
        body.push_str(&format!(
            "- Verify command: `{}`\n",
            markdown_cell(&run.verify_command)
        ));
        body.push_str(&format!(
            "- Receipt command: `{}`\n",
            markdown_cell(&run.receipt_command)
        ));
        body.push_str(&format!(
            "- Source alignment case: `{}`\n",
            markdown_cell(&run.source_alignment_case)
        ));
        body.push_str(&format!(
            "- Limitation category: `{}`\n",
            markdown_cell(&run.limitation_category)
        ));
        body.push_str(&format!(
            "- Runtime repair command: `{}`\n",
            markdown_cell(&run.runtime_repair_command)
        ));
        body.push_str(&format!(
            "- Counts: actionable {}, raw findings {}\n",
            run.actionable_count, run.raw_findings_total
        ));
        body.push_str(&format!(
            "- Consumes canonical state: {}; reinterprets raw findings: {}; blocking default: {}\n",
            if run.consumes_canonical_state {
                "yes"
            } else {
                "no"
            },
            if run.reinterprets_raw_findings {
                "yes"
            } else {
                "no"
            },
            if run.blocking_default { "yes" } else { "no" }
        ));
        body.push_str(&format!(
            "- Limited visible: {}; stale visible: {}\n",
            if run.limited_state_visible {
                "yes"
            } else {
                "no"
            },
            if run.stale_state_visible { "yes" } else { "no" }
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Receipt validation: pass\n\n");
        } else {
            body.push_str(&format!(
                "- Receipt validation: fail - `{}`\n\n",
                markdown_cell(&run.errors.join("; "))
            ));
        }
    }

    body.push_str("## PR Inline Comment Publisher Receipts\n\n");
    body.push_str("These receipts validate checked `comment-publish-plan.{json,md}` fixture outputs for the documented Campaign 26 inline-comment publisher routes. They verify opt-in modes, safe publish flags, summary-only exclusion, cap behavior, dedupe/upsert, stale-existing cleanup planning, fork or token blockers, missing-input blockers, and advisory limits without posting real PR comments.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str("- Default inline comments: off\n");
    body.push_str(
        "- Receipt outputs: `fixtures/boundary_gap/expected/pr-inline-comment-publisher/<case>/comment-publish-plan.{json,md}`\n\n",
    );
    body.push_str("| Case | Mode | Status | Publishable | Skipped | Blocked | Safe | Operations | Skip reasons | Block reasons |\n");
    body.push_str("| --- | --- | --- | ---: | ---: | ---: | --- | --- | --- | --- |\n");
    for run in pr_inline_comment_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} | `{}` | `{}` | `{}` |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.mode),
            markdown_cell(&run.status),
            run.publishable,
            run.skipped,
            run.blocked,
            if run.safe_to_publish { "yes" } else { "no" },
            markdown_cell(&run.operations.join(", ")),
            markdown_cell(&run.skip_reasons.join(", ")),
            markdown_cell(&run.blocked_reasons.join(", "))
        ));
    }
    body.push('\n');
    for run in pr_inline_comment_runs {
        body.push_str(&format!("### PR Inline Comment `{}`\n\n", run.name));
        body.push_str(&format!("- Mode: `{}`\n", markdown_cell(&run.mode)));
        body.push_str(&format!(
            "- Expected mode: `{}`\n",
            markdown_cell(&run.expected_mode)
        ));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!(
            "- Counts: publishable {}, skipped {}, blocked {}\n",
            run.publishable, run.skipped, run.blocked
        ));
        body.push_str(&format!(
            "- Expected counts: publishable {}, skipped {}, blocked {}\n",
            run.expected_publishable, run.expected_skipped, run.expected_blocked
        ));
        body.push_str(&format!(
            "- Safe to publish: {} (expected {})\n",
            run.safe_to_publish, run.expected_safe_to_publish
        ));
        body.push_str(&format!(
            "- Operations: `{}`\n",
            markdown_cell(&run.operations.join(", "))
        ));
        body.push_str(&format!(
            "- Expected operations: `{}`\n",
            markdown_cell(&run.expected_operations.join(", "))
        ));
        body.push_str(&format!(
            "- Skip reasons: `{}`\n",
            markdown_cell(&run.skip_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Expected skip reasons: `{}`\n",
            markdown_cell(&run.expected_skip_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Block reasons: `{}`\n",
            markdown_cell(&run.blocked_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Expected block reasons: `{}`\n",
            markdown_cell(&run.expected_blocked_reasons.join(", "))
        ));
        body.push_str(&format!(
            "- Receipt JSON: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Receipt Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!(
            "- Expected report: `{}`\n",
            normalize_path(&run.expected_report)
        ));
        body.push_str(&format!(
            "- Expected Markdown: `{}`\n",
            normalize_path(&run.expected_markdown)
        ));
        body.push_str(&format!(
            "- Actual outputs: `{}`\n",
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("- Reason: {}\n", markdown_cell(&run.reason)));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body.push_str("## Gate Adoption Receipts\n\n");
    body.push_str("These receipts run `ripr gate evaluate` against checked boundary-gap PR guidance and calibration evidence. They are repo-local dogfood for explicit gate modes; generated CI still leaves `RIPR_GATE_MODE` unset unless the repository configures it.\n\n");
    body.push_str("- Default CI blocking: no\n");
    body.push_str(
        "- Receipt outputs: `target/ripr/dogfood/gate-adoption/<case>/gate-decision.{json,md}`\n\n",
    );
    body.push_str(
        "| Case | Mode | Status | Blocking | Acknowledged | Advisory | Exit | Expected exit |\n",
    );
    body.push_str("| --- | --- | --- | ---: | ---: | ---: | --- | --- |\n");
    for run in gate_runs {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} | {} |\n",
            markdown_cell(&run.name),
            markdown_cell(&run.mode),
            markdown_cell(&run.status),
            run.blocking,
            run.acknowledged,
            run.advisory,
            if run.exit_success {
                "success"
            } else {
                "non-zero"
            },
            if run.expected_exit_success {
                "success"
            } else {
                "non-zero"
            }
        ));
    }
    body.push('\n');
    for run in gate_runs {
        body.push_str(&format!("### Gate `{}`\n\n", run.name));
        body.push_str(&format!("- Mode: `{}`\n", markdown_cell(&run.mode)));
        body.push_str(&format!("- Status: `{}`\n", markdown_cell(&run.status)));
        body.push_str(&format!(
            "- Expected status: `{}`\n",
            markdown_cell(&run.expected_status)
        ));
        body.push_str(&format!(
            "- Decision counts: blocking {}, acknowledged {}, advisory {}\n",
            run.blocking, run.acknowledged, run.advisory
        ));
        body.push_str(&format!(
            "- Expected counts: blocking {}, acknowledged {}, advisory {}\n",
            run.expected_blocking, run.expected_acknowledged, run.expected_advisory
        ));
        body.push_str(&format!(
            "- Gate decision JSON: `{}`\n",
            normalize_path(&run.json_path)
        ));
        body.push_str(&format!(
            "- Gate decision Markdown: `{}`\n",
            normalize_path(&run.markdown_path)
        ));
        body.push_str(&format!("- Duration: {} ms\n", run.duration_ms));
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body
}

pub(crate) fn dogfood_report_json(inputs: &DogfoodReportInputs<'_>) -> String {
    let runs = inputs.runs;
    let gate_runs = inputs.gate_runs;
    let first_action_runs = inputs.first_action_runs;
    let first_pr_runs = inputs.first_pr_runs;
    let front_panel_runs = inputs.front_panel_runs;
    let report_packet_index_runs = inputs.report_packet_index_runs;
    let preview_projection_runs = inputs.preview_projection_runs;
    let finding_alignment_runs = inputs.finding_alignment_runs;
    let surface_projection_alignment_runs = inputs.surface_projection_alignment_runs;
    let real_repair_attempt_runs = inputs.real_repair_attempt_runs;
    let python_real_repo_eval_runs = inputs.python_real_repo_eval_runs;
    let python_static_limit_eval_runs = inputs.python_static_limit_eval_runs;
    let python_no_action_eval_runs = inputs.python_no_action_eval_runs;
    let python_repair_quality =
        dogfood_python_repair_routing_quality_summary(python_real_repo_eval_runs);
    let python_static_limit_distribution =
        dogfood_python_static_limit_eval_distribution(python_static_limit_eval_runs);
    let python_no_action_distribution =
        dogfood_python_no_action_eval_distribution(python_no_action_eval_runs);
    let typescript_false_actionable_cases = typescript_preview_false_actionable_audit_cases_at(
        &repo_rooted_fixture_path(TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS),
    );
    let typescript_false_actionable_audit =
        dogfood_typescript_false_actionable_audit_summary(&typescript_false_actionable_cases);
    let typescript_preview_repair_loop_runs = inputs.typescript_preview_repair_loop_runs;
    let bun_ub_cross_language_runs = inputs.bun_ub_cross_language_runs;
    let user_surface_projection_runs = inputs.user_surface_projection_runs;
    let pr_inline_comment_runs = inputs.pr_inline_comment_runs;
    let first_pr_metrics = dogfood_first_pr_metrics(first_pr_runs);
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"runs\": [\n",
        dogfood_report_status(inputs)
    );
    for (index, run) in runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "      \"root\": \"{}\",\n",
            json_escape(&normalize_path(&run.root))
        ));
        body.push_str(&format!(
            "      \"diff\": \"{}\",\n",
            json_escape(&normalize_path(&run.diff))
        ));
        body.push_str(&format!(
            "      \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!("      \"duration_ms\": {},\n", run.duration_ms));
        body.push_str(&format!("      \"findings\": {},\n", run.findings));
        body.push_str(&format!(
            "      \"stop_reason_mentions\": {},\n",
            run.stop_reason_mentions
        ));
        body.push_str("      \"class_counts\": {");
        for (class_index, (class, count)) in run.class_counts.iter().enumerate() {
            if class_index > 0 {
                body.push_str(", ");
            }
            body.push_str(&format!("\"{}\": {}", json_escape(class), count));
        }
        body.push_str("},\n");
        body.push_str("      \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n    }");
    }
    body.push_str("\n  ],\n  \"first_useful_action\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/boundary_gap/expected/first-useful-action\",\n    \"cases\": [\n",
    );
    for (index, run) in first_action_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"expected_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!(
            "        \"action_kind\": \"{}\",\n",
            json_escape(&run.action_kind)
        ));
        body.push_str(&format!(
            "        \"audience\": \"{}\",\n",
            json_escape(&run.audience)
        ));
        body.push_str(&format!("        \"selected\": {},\n", run.selected));
        body.push_str(&format!(
            "        \"static_movement\": \"{}\",\n",
            json_escape(&run.static_movement)
        ));
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_action_kind\": \"{}\",\n",
            json_escape(&run.expected_action_kind)
        ));
        body.push_str(&format!(
            "        \"expected_audience\": \"{}\",\n",
            json_escape(&run.expected_audience)
        ));
        body.push_str(&format!(
            "        \"expected_selected\": {},\n",
            run.expected_selected
        ));
        body.push_str(&format!(
            "        \"expected_static_movement\": \"{}\",\n",
            json_escape(&run.expected_static_movement)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"first_successful_pr\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"receipt_dir\": \"fixtures/first_successful_pr\",\n");
    body.push_str("    \"metrics\": {\n");
    body.push_str(&format!(
        "      \"first_run_packets_total\": {},\n",
        first_pr_metrics.packets_total
    ));
    body.push_str(&format!(
        "      \"first_run_top_gap_selected_total\": {},\n",
        first_pr_metrics.top_gap_selected_total
    ));
    body.push_str(&format!(
        "      \"first_run_no_action_total\": {},\n",
        first_pr_metrics.no_action_total
    ));
    body.push_str(&format!(
        "      \"first_run_blocked_total\": {},\n",
        first_pr_metrics.blocked_total
    ));
    body.push_str(&format!(
        "      \"first_run_missing_artifact_total\": {},\n",
        first_pr_metrics.missing_artifact_total
    ));
    body.push_str(&format!(
        "      \"first_run_stale_artifact_total\": {},\n",
        first_pr_metrics.stale_artifact_total
    ));
    body.push_str(&format!(
        "      \"first_run_wrong_root_total\": {},\n",
        first_pr_metrics.wrong_root_total
    ));
    body.push_str(&format!(
        "      \"first_run_malformed_artifact_total\": {},\n",
        first_pr_metrics.malformed_artifact_total
    ));
    body.push_str(&format!(
        "      \"first_run_timeout_total\": {}\n",
        first_pr_metrics.timeout_total
    ));
    body.push_str("    },\n    \"cases\": [\n");
    for (index, run) in first_pr_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"expected_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!(
            "        \"state\": \"{}\",\n",
            json_escape(&run.state)
        ));
        body.push_str(&format!(
            "        \"top_gap_kind\": \"{}\",\n",
            json_escape(&run.top_gap_kind)
        ));
        body.push_str(&format!(
            "        \"verify_command\": {},\n",
            json_optional_string(run.verify_command.as_deref())
        ));
        body.push_str(&format!(
            "        \"next_command\": {},\n",
            json_optional_string(run.next_command.as_deref())
        ));
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_state\": \"{}\",\n",
            json_escape(&run.expected_state)
        ));
        body.push_str(&format!(
            "        \"description\": \"{}\",\n",
            json_escape(&run.description)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"pr_review_front_panel\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/boundary_gap/expected/pr-review-front-panel\",\n    \"cases\": [\n",
    );
    for (index, run) in front_panel_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.report_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!(
            "        \"top_issue_state\": \"{}\",\n",
            json_escape(&run.top_issue_state)
        ));
        body.push_str(&format!(
            "        \"policy_state\": \"{}\",\n",
            json_escape(&run.policy_state)
        ));
        body.push_str(&format!(
            "        \"placement\": \"{}\",\n",
            json_escape(&run.placement)
        ));
        body.push_str(&format!(
            "        \"movement_state\": \"{}\",\n",
            json_escape(&run.movement_state)
        ));
        body.push_str(&format!(
            "        \"coverage_grip_state\": \"{}\",\n",
            json_escape(&run.coverage_grip_state)
        ));
        body.push_str(&format!(
            "        \"new_policy_eligible\": {},\n",
            run.new_policy_eligible
        ));
        body.push_str(&format!(
            "        \"baseline_resolved\": {},\n",
            run.baseline_resolved
        ));
        body.push_str(&format!(
            "        \"blocking_candidates\": {},\n",
            run.blocking_candidates
        ));
        body.push_str(&format!("        \"warnings\": {},\n", run.warnings));
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_top_issue_state\": \"{}\",\n",
            json_escape(&run.expected_top_issue_state)
        ));
        body.push_str(&format!(
            "        \"expected_policy_state\": \"{}\",\n",
            json_escape(&run.expected_policy_state)
        ));
        body.push_str(&format!(
            "        \"expected_placement\": \"{}\",\n",
            json_escape(&run.expected_placement)
        ));
        body.push_str(&format!(
            "        \"expected_movement_state\": \"{}\",\n",
            json_escape(&run.expected_movement_state)
        ));
        body.push_str(&format!(
            "        \"expected_coverage_grip_state\": \"{}\",\n",
            json_escape(&run.expected_coverage_grip_state)
        ));
        body.push_str(&format!(
            "        \"expected_new_policy_eligible\": {},\n",
            run.expected_new_policy_eligible
        ));
        body.push_str(&format!(
            "        \"expected_baseline_resolved\": {},\n",
            run.expected_baseline_resolved
        ));
        body.push_str(&format!(
            "        \"expected_blocking_candidates\": {},\n",
            run.expected_blocking_candidates
        ));
        body.push_str(&format!(
            "        \"expected_warnings\": {},\n",
            run.expected_warnings
        ));
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"report_packet_index\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/boundary_gap/expected/report-packet-index\",\n    \"cases\": [\n",
    );
    for (index, run) in report_packet_index_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!(
            "        \"expected_report\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_report))
        ));
        body.push_str(&format!(
            "        \"expected_markdown\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_markdown))
        ));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!(
            "        \"missing_expected\": {},\n",
            run.missing_expected
        ));
        body.push_str(&format!("        \"warnings\": {},\n", run.warnings));
        body.push_str(&format!("        \"failures\": {},\n", run.failures));
        body.push_str(&format!(
            "        \"start_here_available\": {},\n",
            run.start_here_available
        ));
        body.push_str(&format!(
            "        \"gate_authority_present\": {},\n",
            run.gate_authority_present
        ));
        body.push_str("        \"groups\": [");
        write_json_string_array(&mut body, &run.groups);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_missing_expected\": {},\n",
            run.expected_missing_expected
        ));
        body.push_str(&format!(
            "        \"expected_warnings\": {},\n",
            run.expected_warnings
        ));
        body.push_str(&format!(
            "        \"expected_failures\": {},\n",
            run.expected_failures
        ));
        body.push_str(&format!(
            "        \"expected_start_here_available\": {},\n",
            run.expected_start_here_available
        ));
        body.push_str(&format!(
            "        \"expected_gate_authority_present\": {},\n",
            run.expected_gate_authority_present
        ));
        body.push_str("        \"expected_required_groups\": [");
        write_json_string_array(&mut body, &run.expected_required_groups);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"generated_ci_cockpit\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"default_inline_comments\": \"off\",\n");
    body.push_str("    \"language_grouping\": \"checked\",\n    \"cases\": [\n");
    for (index, run) in preview_projection_runs
        .generated_ci_cockpit
        .iter()
        .enumerate()
    {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"command\": \"{}\",\n",
            json_escape(&run.command)
        ));
        body.push_str(&format!("        \"duration_ms\": {},\n", run.duration_ms));
        body.push_str(&format!("        \"start_here\": {},\n", run.start_here));
        body.push_str(&format!(
            "        \"repair_commands\": {},\n",
            run.repair_commands
        ));
        body.push_str(&format!(
            "        \"expected_repair_commands\": {},\n",
            run.expected_repair_commands
        ));
        body.push_str(&format!(
            "        \"gate_authority_boundary\": {},\n",
            run.gate_authority_boundary
        ));
        body.push_str(&format!(
            "        \"default_advisory\": {},\n",
            run.default_advisory
        ));
        body.push_str(&format!(
            "        \"artifact_upload\": {},\n",
            run.artifact_upload
        ));
        body.push_str(&format!(
            "        \"language_grouping_status\": \"{}\",\n",
            json_escape(&run.language_grouping_status)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"language_preview\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"preview_adapters\": \"opt-in\",\n");
    body.push_str(
        "    \"receipt_dir\": \"target/ripr/dogfood/language-preview\",\n    \"cases\": [\n",
    );
    for (index, run) in preview_projection_runs.language_preview.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"language\": \"{}\",\n",
            json_escape(&run.language)
        ));
        body.push_str(&format!(
            "        \"root\": \"{}\",\n",
            json_escape(&normalize_path(&run.root))
        ));
        body.push_str(&format!(
            "        \"diff\": \"{}\",\n",
            json_escape(&normalize_path(&run.diff))
        ));
        body.push_str(&format!(
            "        \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"human_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.human_path))
        ));
        body.push_str(&format!("        \"duration_ms\": {},\n", run.duration_ms));
        body.push_str(&format!(
            "        \"preview_enabled\": {},\n",
            run.preview_enabled
        ));
        body.push_str(&format!("        \"findings\": {},\n", run.findings));
        body.push_str(&format!(
            "        \"language_findings\": {},\n",
            run.language_findings
        ));
        body.push_str(&format!(
            "        \"preview_findings\": {},\n",
            run.preview_findings
        ));
        body.push_str(&format!(
            "        \"missing_preview_status\": {},\n",
            run.missing_preview_status
        ));
        body.push_str(&format!(
            "        \"related_tests\": {},\n",
            run.related_tests
        ));
        body.push_str("        \"classifications\": [");
        write_json_string_array(&mut body, &run.classifications);
        body.push_str("],\n");
        body.push_str("        \"static_limit_kinds\": [");
        write_json_string_array(&mut body, &run.static_limit_kinds);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"expected_findings\": {},\n",
            run.expected_findings
        ));
        body.push_str(&format!(
            "        \"expected_preview_findings\": {},\n",
            run.expected_preview_findings
        ));
        body.push_str(&format!(
            "        \"expected_missing_preview_status\": {},\n",
            run.expected_missing_preview_status
        ));
        body.push_str(&format!(
            "        \"expected_related_tests\": {},\n",
            run.expected_related_tests
        ));
        body.push_str("        \"expected_classifications\": [");
        write_json_string_array(&mut body, &run.expected_classifications);
        body.push_str("],\n");
        body.push_str("        \"expected_static_limit_kinds\": [");
        write_json_string_array(&mut body, &run.expected_static_limit_kinds);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"typescript_preview_repair_loop\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"preview_authority\": \"advisory\",\n");
    body.push_str("    \"receipt_dir\": \"fixtures/typescript-preview-repair-loop\",\n");
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"cases\": {},\n",
        typescript_preview_repair_loop_runs.len()
    ));
    body.push_str(&format!(
        "      \"typescript\": {},\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.language == "typescript")
            .count()
    ));
    body.push_str(&format!(
        "      \"javascript\": {},\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.language == "javascript")
            .count()
    ));
    body.push_str(&format!(
        "      \"static_limitations\": {},\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.gap_state == "static_limitation")
            .count()
    ));
    body.push_str(&format!(
        "      \"weak_oracle_downgrades\": {},\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.outcome == "weak_oracle_downgraded")
            .count()
    ));
    body.push_str(&format!(
        "      \"intentionally_skipped\": {},\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.outcome == "intentionally_skipped")
            .count()
    ));
    body.push_str(&format!(
        "      \"repair_packet_ready\": {}\n",
        typescript_preview_repair_loop_runs
            .iter()
            .filter(|run| run.repair_packet_ready)
            .count()
    ));
    body.push_str("    },\n    \"cases\": [\n");
    for (index, run) in typescript_preview_repair_loop_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"source_fixture\": \"{}\",\n",
            json_escape(&run.source_fixture)
        ));
        body.push_str(&format!(
            "        \"source_finding_id\": \"{}\",\n",
            json_escape(&run.source_finding_id)
        ));
        body.push_str(&format!(
            "        \"language\": \"{}\",\n",
            json_escape(&run.language)
        ));
        body.push_str(&format!(
            "        \"classification\": \"{}\",\n",
            json_escape(&run.classification)
        ));
        body.push_str(&format!(
            "        \"changed_owner\": \"{}\",\n",
            json_escape(&run.changed_owner)
        ));
        body.push_str(&format!(
            "        \"probe_family\": \"{}\",\n",
            json_escape(&run.probe_family)
        ));
        body.push_str(&format!(
            "        \"oracle_kind\": \"{}\",\n",
            json_escape(&run.oracle_kind)
        ));
        body.push_str(&format!(
            "        \"oracle_strength\": \"{}\",\n",
            json_escape(&run.oracle_strength)
        ));
        body.push_str(&format!(
            "        \"gap_state\": \"{}\",\n",
            json_escape(&run.gap_state)
        ));
        body.push_str(&format!(
            "        \"actionability_category\": \"{}\",\n",
            json_escape(&run.actionability_category)
        ));
        body.push_str(&format!(
            "        \"static_limit_kind\": {},\n",
            json_optional_string(run.static_limit_kind.as_deref())
        ));
        body.push_str(&format!(
            "        \"repair_packet_ready\": {},\n",
            run.repair_packet_ready
        ));
        body.push_str(&format!(
            "        \"expected_test_or_observer_shape\": \"{}\",\n",
            json_escape(&run.expected_test_or_observer_shape)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"verify_result\": \"{}\",\n",
            json_escape(&run.verify_result)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_state\": \"{}\",\n",
            json_escape(&run.receipt_state)
        ));
        body.push_str(&format!(
            "        \"outcome\": \"{}\",\n",
            json_escape(&run.outcome)
        ));
        body.push_str(&format!(
            "        \"why_not_actionable\": \"{}\",\n",
            json_escape(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "        \"repair_route\": \"{}\",\n",
            json_escape(&run.repair_route)
        ));
        body.push_str(&format!(
            "        \"operator_note\": \"{}\",\n",
            json_escape(&run.operator_note)
        ));
        body.push_str("        \"must_not_change\": [");
        write_json_string_array(&mut body, &run.must_not_change);
        body.push_str("],\n");
        body.push_str("        \"raw_evidence_refs\": [");
        write_json_string_array(&mut body, &run.raw_evidence_refs);
        body.push_str("],\n");
        body.push_str("        \"non_claims\": [");
        write_json_string_array(&mut body, &run.non_claims);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"typescript_false_actionable_audit\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"preview_authority\": \"advisory\",\n");
    body.push_str(&format!(
        "    \"input\": \"{}\",\n",
        json_escape(TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS)
    ));
    body.push_str(&format!(
        "    \"quality_gate\": {{ \"status\": \"{}\", \"reason\": \"{}\" }},\n",
        json_escape(&typescript_false_actionable_audit.gate_status),
        json_escape(&typescript_false_actionable_audit.gate_reason)
    ));
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"cases\": {},\n",
        typescript_false_actionable_audit.cases
    ));
    body.push_str(&format!(
        "      \"must_remain_non_actionable\": {},\n",
        typescript_false_actionable_audit.must_remain_non_actionable
    ));
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "false_actionable_rate",
        typescript_false_actionable_audit.false_actionable,
        typescript_false_actionable_audit.must_remain_non_actionable,
        false,
        "audit row that must remain non-actionable became packet-ready, actionable, or complete-packet-shaped",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "repair_packet_ready_violation_rate",
        typescript_false_actionable_audit.repair_packet_ready_true,
        typescript_false_actionable_audit.cases,
        false,
        "preview audit row reported repair_packet_ready=true",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "actionable_gap_state_violation_rate",
        typescript_false_actionable_audit.actionable_gap_state,
        typescript_false_actionable_audit.cases,
        false,
        "preview audit row reported gap_state=actionable",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "complete_packet_category_violation_rate",
        typescript_false_actionable_audit.complete_packet_category,
        typescript_false_actionable_audit.cases,
        false,
        "preview audit row reported actionability_category=complete_repair_packet",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "preview_boundary_violation_rate",
        typescript_false_actionable_audit.preview_boundary_violations,
        typescript_false_actionable_audit.cases,
        false,
        "preview audit row did not keep authority_boundary=preview_advisory_only",
    );
    body.push_str(
        "      \"limits\": [\"advisory TypeScript-family preview audit only\", \"does not rerun analysis or execute TypeScript tests\", \"does not create repair packets, gates, badge inputs, baselines, RIPR Zero input, generated tests, source edits, provider calls, mutation testing, or support-tier promotion\"]\n",
    );
    body.push_str("    },\n    \"cases\": [\n");
    for (index, case) in typescript_false_actionable_cases.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        let false_actionable = case.must_remain_non_actionable
            && (case.repair_packet_ready
                || case.gap_state == "actionable"
                || case.actionability_category == "complete_repair_packet");
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&case.name)
        ));
        body.push_str(&format!(
            "        \"source_fixture\": \"{}\",\n",
            json_escape(&case.source_fixture)
        ));
        body.push_str(&format!(
            "        \"source_finding_id\": \"{}\",\n",
            json_escape(&case.source_finding_id)
        ));
        body.push_str(&format!(
            "        \"language\": \"{}\",\n",
            json_escape(&case.language)
        ));
        body.push_str(&format!(
            "        \"risk_class\": \"{}\",\n",
            json_escape(&case.risk_class)
        ));
        body.push_str(&format!(
            "        \"disposition\": \"{}\",\n",
            json_escape(&case.disposition)
        ));
        body.push_str(&format!(
            "        \"gap_state\": \"{}\",\n",
            json_escape(&case.gap_state)
        ));
        body.push_str(&format!(
            "        \"actionability_category\": \"{}\",\n",
            json_escape(&case.actionability_category)
        ));
        body.push_str(&format!(
            "        \"static_limit_kind\": {},\n",
            json_optional_string(case.static_limit_kind.as_deref())
        ));
        body.push_str(&format!(
            "        \"repair_packet_ready\": {},\n",
            case.repair_packet_ready
        ));
        body.push_str(&format!(
            "        \"must_remain_non_actionable\": {},\n",
            case.must_remain_non_actionable
        ));
        body.push_str(&format!(
            "        \"authority_boundary\": \"{}\",\n",
            json_escape(&case.authority_boundary)
        ));
        body.push_str(&format!(
            "        \"false_actionable\": {},\n",
            false_actionable
        ));
        body.push_str(&format!(
            "        \"repair_route\": \"{}\",\n",
            json_escape(&case.repair_route)
        ));
        body.push_str("        \"non_claims\": [");
        write_json_string_array(&mut body, &case.non_claims);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\"\n",
            json_escape(&case.reason)
        ));
        body.push_str("      }");
    }
    body.push_str("\n    ]\n  },\n  \"bun_ub_cross_language_witnesses\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"preview_authority\": \"advisory\",\n");
    body.push_str("    \"receipt_dir\": \"fixtures/bun-ub-cross-language-dogfood\",\n");
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"cases\": {},\n",
        bun_ub_cross_language_runs.len()
    ));
    body.push_str(&format!(
        "      \"ts_discriminated\": {},\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.observed_state == "rust_ungripped_ts_discriminated")
            .count()
    ));
    body.push_str(&format!(
        "      \"missing_discriminator\": {},\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.observed_state == "rust_ungripped_ts_missing_discriminator")
            .count()
    ));
    body.push_str(&format!(
        "      \"mention_not_observer\": {},\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.observed_state == "ts_mention_not_observer")
            .count()
    ));
    body.push_str(&format!(
        "      \"bridge_unknown\": {},\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.observed_state == "bridge_unknown")
            .count()
    ));
    body.push_str(&format!(
        "      \"named_static_limitation\": {},\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.observed_state == "named_static_limitation")
            .count()
    ));
    body.push_str(&format!(
        "      \"repair_packet_ready\": {}\n",
        bun_ub_cross_language_runs
            .iter()
            .filter(|run| run.repair_packet_ready)
            .count()
    ));
    body.push_str("    },\n    \"cases\": [\n");
    for (index, run) in bun_ub_cross_language_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"source_case\": \"{}\",\n",
            json_escape(&run.source_case)
        ));
        body.push_str(&format!(
            "        \"route_quality_case\": \"{}\",\n",
            json_escape(&run.route_quality_case)
        ));
        body.push_str(&format!(
            "        \"rust_file\": \"{}\",\n",
            json_escape(&run.rust_file)
        ));
        body.push_str(&format!(
            "        \"rust_owner\": \"{}\",\n",
            json_escape(&run.rust_owner)
        ));
        body.push_str(&format!(
            "        \"rust_boundary\": \"{}\",\n",
            json_escape(&run.rust_boundary)
        ));
        body.push_str(&format!(
            "        \"ts_test_file\": \"{}\",\n",
            json_escape(&run.ts_test_file)
        ));
        body.push_str(&format!(
            "        \"expected_state\": \"{}\",\n",
            json_escape(&run.expected_state)
        ));
        body.push_str(&format!(
            "        \"observed_state\": \"{}\",\n",
            json_escape(&run.observed_state)
        ));
        body.push_str("        \"missing_discriminators\": [");
        write_json_string_array(&mut body, &run.missing_discriminators);
        body.push_str("],\n");
        body.push_str("        \"missing_graph_legs\": [");
        write_json_string_array(&mut body, &run.missing_graph_legs);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"suggested_test_file\": \"{}\",\n",
            json_escape(&run.suggested_test_file)
        ));
        body.push_str(&format!(
            "        \"manual_verdict\": \"{}\",\n",
            json_escape(&run.manual_verdict)
        ));
        body.push_str(&format!(
            "        \"operator_action\": \"{}\",\n",
            json_escape(&run.operator_action)
        ));
        body.push_str(&format!(
            "        \"review_before\": \"{}\",\n",
            json_escape(&run.review_before)
        ));
        body.push_str(&format!(
            "        \"review_after\": \"{}\",\n",
            json_escape(&run.review_after)
        ));
        body.push_str(&format!(
            "        \"bridge_verdict\": \"{}\",\n",
            json_escape(&run.bridge_verdict)
        ));
        body.push_str(&format!(
            "        \"placement_verdict\": \"{}\",\n",
            json_escape(&run.placement_verdict)
        ));
        body.push_str(&format!(
            "        \"proof_mode\": \"{}\",\n",
            json_escape(&run.proof_mode)
        ));
        body.push_str(&format!(
            "        \"receipt_state\": \"{}\",\n",
            json_escape(&run.receipt_state)
        ));
        body.push_str(&format!(
            "        \"repair_packet_ready\": {},\n",
            run.repair_packet_ready
        ));
        body.push_str(&format!(
            "        \"authority_boundary\": \"{}\",\n",
            json_escape(&run.authority_boundary)
        ));
        body.push_str("        \"raw_evidence_refs\": [");
        write_json_string_array(&mut body, &run.raw_evidence_refs);
        body.push_str("],\n");
        body.push_str("        \"non_claims\": [");
        write_json_string_array(&mut body, &run.non_claims);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"editor_gap_cockpit\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"editor_behavior\": \"saved-workspace projection-only\",\n");
    body.push_str("    \"receipt_dir\": \"fixtures/editor_gap_cockpit\",\n    \"cases\": [\n");
    for (index, run) in preview_projection_runs
        .editor_gap_cockpit
        .iter()
        .enumerate()
    {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"expected_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_dir))
        ));
        body.push_str(&format!(
            "        \"projection_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.projection_path))
        ));
        body.push_str(&format!(
            "        \"diagnostics_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.diagnostics_path))
        ));
        body.push_str(&format!(
            "        \"hover_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.hover_path))
        ));
        body.push_str(&format!(
            "        \"code_actions_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.code_actions_path))
        ));
        body.push_str(&format!(
            "        \"status_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.status_path))
        ));
        body.push_str(&format!(
            "        \"state\": \"{}\",\n",
            json_escape(&run.state)
        ));
        body.push_str(&format!(
            "        \"language\": {},\n",
            json_optional_string(run.language.as_deref())
        ));
        body.push_str(&format!(
            "        \"language_status\": {},\n",
            json_optional_string(run.language_status.as_deref())
        ));
        body.push_str(&format!(
            "        \"diagnostics_projected\": {},\n",
            run.diagnostics_projected
        ));
        body.push_str(&format!(
            "        \"actual_diagnostics\": {},\n",
            run.actual_diagnostics
        ));
        body.push_str(&format!("        \"fail_closed\": {},\n", run.fail_closed));
        body.push_str("        \"actions_projected\": [");
        write_json_string_array(&mut body, &run.actions_projected);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"actual_actions\": {},\n",
            run.actual_actions
        ));
        body.push_str(&format!(
            "        \"static_limit_kind\": {},\n",
            json_optional_string(run.static_limit_kind.as_deref())
        ));
        body.push_str(&format!(
            "        \"hover_static_before_action\": {},\n",
            run.hover_static_before_action
        ));
        body.push_str(&format!(
            "        \"expected_state\": \"{}\",\n",
            json_escape(&run.expected_state)
        ));
        body.push_str(&format!(
            "        \"expected_language\": {},\n",
            json_optional_string(run.expected_language.as_deref())
        ));
        body.push_str(&format!(
            "        \"expected_language_status\": {},\n",
            json_optional_string(run.expected_language_status.as_deref())
        ));
        body.push_str(&format!(
            "        \"expected_diagnostics\": {},\n",
            run.expected_diagnostics
        ));
        body.push_str(&format!(
            "        \"expected_fail_closed\": {},\n",
            run.expected_fail_closed
        ));
        body.push_str("        \"expected_actions\": [");
        write_json_string_array(&mut body, &run.expected_actions);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"expected_static_limit_kind\": {},\n",
            json_optional_string(run.expected_static_limit_kind.as_deref())
        ));
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"editor_first_pr_bridge\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"editor_behavior\": \"saved-workspace read-only first-pr projection\",\n");
    body.push_str("    \"receipt_dir\": \"fixtures/editor_first_pr_bridge\",\n    \"cases\": [\n");
    for (index, run) in preview_projection_runs
        .editor_first_pr_bridge
        .iter()
        .enumerate()
    {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"expected_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_dir))
        ));
        body.push_str(&format!(
            "        \"packet_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.packet_path))
        ));
        body.push_str(&format!(
            "        \"diagnostics_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.diagnostics_path))
        ));
        body.push_str(&format!(
            "        \"code_actions_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.code_actions_path))
        ));
        body.push_str(&format!(
            "        \"status_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.status_path))
        ));
        body.push_str(&format!(
            "        \"diagnosis_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.diagnosis_path))
        ));
        body.push_str(&format!(
            "        \"packet_state\": \"{}\",\n",
            json_escape(&run.packet_state)
        ));
        body.push_str(&format!(
            "        \"expected_packet_state\": \"{}\",\n",
            json_escape(&run.expected_packet_state)
        ));
        body.push_str("        \"safe_actions\": [");
        write_json_string_array(&mut body, &run.safe_actions);
        body.push_str("],\n");
        body.push_str("        \"expected_safe_actions\": [");
        write_json_string_array(&mut body, &run.expected_safe_actions);
        body.push_str("],\n");
        body.push_str("        \"suppressed_actions\": [");
        write_json_string_array(&mut body, &run.suppressed_actions);
        body.push_str("],\n");
        body.push_str("        \"expected_suppressed_actions\": [");
        write_json_string_array(&mut body, &run.expected_suppressed_actions);
        body.push_str("],\n");
        body.push_str("        \"first_pr_actions\": [");
        write_json_string_array(&mut body, &run.first_pr_actions);
        body.push_str("],\n");
        body.push_str("        \"action_commands\": [");
        write_json_string_array(&mut body, &run.action_commands);
        body.push_str("],\n");
        body.push_str(&format!("        \"diagnostics\": {},\n", run.diagnostics));
        body.push_str(&format!(
            "        \"expected_diagnostics\": {},\n",
            run.expected_diagnostics
        ));
        body.push_str(&format!("        \"fail_closed\": {},\n", run.fail_closed));
        body.push_str(&format!(
            "        \"expected_fail_closed\": {},\n",
            run.expected_fail_closed
        ));
        body.push_str(&format!(
            "        \"receipt_movement\": {},\n",
            json_optional_string(run.receipt_movement.as_deref())
        ));
        body.push_str(&format!(
            "        \"expected_receipt_movement\": {},\n",
            json_optional_string(run.expected_receipt_movement.as_deref())
        ));
        body.push_str(&format!(
            "        \"runtime_adequacy_claim\": {},\n",
            run.runtime_adequacy_claim
        ));
        body.push_str(&format!(
            "        \"mutation_proof_claim\": {},\n",
            run.mutation_proof_claim
        ));
        body.push_str(&format!(
            "        \"policy_gate_claim\": {},\n",
            run.policy_gate_claim
        ));
        body.push_str(&format!(
            "        \"pr_ready_claim\": {},\n",
            run.pr_ready_claim
        ));
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"finding_alignment\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/finding-alignment-dogfood\",\n    \"cases\": [\n",
    );
    for (index, run) in finding_alignment_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"source_pr\": \"{}\",\n",
            json_escape(&run.source_pr)
        ));
        body.push_str(&format!(
            "        \"canonical_gap_id\": \"{}\",\n",
            json_escape(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "        \"evidence_class\": \"{}\",\n",
            json_escape(&run.evidence_class)
        ));
        body.push_str(&format!(
            "        \"raw_findings_total\": {},\n",
            run.raw_findings_total
        ));
        body.push_str(&format!(
            "        \"canonical_items_total\": {},\n",
            run.canonical_items_total
        ));
        body.push_str(&format!(
            "        \"raw_finding_summary\": \"{}\",\n",
            json_escape(&run.raw_finding_summary)
        ));
        body.push_str(&format!(
            "        \"gap_state\": \"{}\",\n",
            json_escape(&run.gap_state)
        ));
        body.push_str(&format!(
            "        \"actionability\": \"{}\",\n",
            json_escape(&run.actionability)
        ));
        body.push_str(&format!(
            "        \"user_outcome\": \"{}\",\n",
            json_escape(&run.user_outcome)
        ));
        body.push_str(&format!(
            "        \"repair_kind\": \"{}\",\n",
            json_escape(&run.repair_kind)
        ));
        body.push_str(&format!(
            "        \"target_test_type\": \"{}\",\n",
            json_escape(&run.target_test_type)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"static_limitation_category\": {},\n",
            json_optional_string(run.static_limitation_category.as_deref())
        ));
        body.push_str(&format!(
            "        \"static_limitation_repair_route\": {},\n",
            json_optional_string(run.static_limitation_repair_route.as_deref())
        ));
        body.push_str(&format!(
            "        \"raw_findings_supporting_only\": {},\n",
            run.raw_findings_supporting_only
        ));
        body.push_str(&format!(
            "        \"recommended_repair\": \"{}\",\n",
            json_escape(&run.recommended_repair)
        ));
        body.push_str(&format!(
            "        \"before_after_context\": \"{}\",\n",
            json_escape(&run.before_after_context)
        ));
        body.push_str("        \"must_not_claim\": [");
        write_json_string_array(&mut body, &run.must_not_claim);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"surface_projection_alignment\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/surface-projection-alignment\",\n    \"cases\": [\n",
    );
    for (index, run) in surface_projection_alignment_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"canonical_gap_id\": \"{}\",\n",
            json_escape(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "        \"packet_id\": \"{}\",\n",
            json_escape(&run.packet_id)
        ));
        body.push_str(&format!(
            "        \"repair_kind\": \"{}\",\n",
            json_escape(&run.repair_kind)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_state\": \"{}\",\n",
            json_escape(&run.receipt_state)
        ));
        body.push_str(&format!(
            "        \"outcome\": \"{}\",\n",
            json_escape(&run.outcome)
        ));
        body.push_str(&format!(
            "        \"readiness_status\": \"{}\",\n",
            json_escape(&run.readiness_status)
        ));
        body.push_str(&format!(
            "        \"top_next_action_kind\": \"{}\",\n",
            json_escape(&run.top_next_action_kind)
        ));
        body.push_str(&format!(
            "        \"top_next_action_command\": {},\n",
            json_optional_string(run.top_next_action_command.as_deref())
        ));
        body.push_str(&format!(
            "        \"attempted_packets\": {},\n",
            run.attempted_packets
        ));
        body.push_str(&format!(
            "        \"improved_packets\": {},\n",
            run.improved_packets
        ));
        body.push_str("        \"advisory_consumers\": [");
        write_json_string_array(&mut body, &run.advisory_consumers);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"real_repair_attempts\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"receipt_dir\": \"fixtures/real-repair-attempts\",\n");
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"attempted\": {},\n",
        real_repair_attempt_runs.len()
    ));
    body.push_str(&format!(
        "      \"improved\": {},\n",
        real_repair_attempt_runs
            .iter()
            .filter(|run| run.outcome == "evidence_improved")
            .count()
    ));
    body.push_str(&format!(
        "      \"unchanged\": {},\n",
        real_repair_attempt_runs
            .iter()
            .filter(|run| run.outcome == "evidence_unchanged")
            .count()
    ));
    body.push_str(&format!(
        "      \"regressed\": {},\n",
        real_repair_attempt_runs
            .iter()
            .filter(|run| run.outcome == "evidence_regressed")
            .count()
    ));
    body.push_str(&format!(
        "      \"resolved\": {},\n",
        real_repair_attempt_runs
            .iter()
            .filter(|run| run.outcome == "resolved")
            .count()
    ));
    body.push_str(&format!(
        "      \"attempted_no_receipt\": {}\n",
        real_repair_attempt_runs
            .iter()
            .filter(|run| run.outcome == "attempted_no_receipt")
            .count()
    ));
    body.push_str("    },\n    \"cases\": [\n");
    for (index, run) in real_repair_attempt_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"source_ref\": \"{}\",\n",
            json_escape(&run.source_ref)
        ));
        body.push_str(&format!(
            "        \"canonical_gap_id\": \"{}\",\n",
            json_escape(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "        \"packet_id\": \"{}\",\n",
            json_escape(&run.packet_id)
        ));
        body.push_str(&format!(
            "        \"language\": {},\n",
            json_optional_string(run.language.as_deref())
        ));
        body.push_str(&format!(
            "        \"evidence_class\": {},\n",
            json_optional_string(run.evidence_class.as_deref())
        ));
        body.push_str(&format!(
            "        \"source_file\": {},\n",
            json_optional_string(run.source_file.as_deref())
        ));
        body.push_str(&format!(
            "        \"repair_kind\": \"{}\",\n",
            json_escape(&run.repair_kind)
        ));
        body.push_str(&format!(
            "        \"target_test_or_observer_shape\": \"{}\",\n",
            json_escape(&run.target_test_or_observer_shape)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"verify_result\": \"{}\",\n",
            json_escape(&run.verify_result)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_path\": {},\n",
            json_optional_string(run.receipt_path.as_deref())
        ));
        body.push_str(&format!(
            "        \"receipt_state\": \"{}\",\n",
            json_escape(&run.receipt_state)
        ));
        body.push_str(&format!(
            "        \"actor_kind\": \"{}\",\n",
            json_escape(&run.actor_kind)
        ));
        body.push_str(&format!(
            "        \"before_gap_state\": \"{}\",\n",
            json_escape(&run.before_gap_state)
        ));
        body.push_str(&format!(
            "        \"after_gap_state\": \"{}\",\n",
            json_escape(&run.after_gap_state)
        ));
        body.push_str(&format!(
            "        \"outcome\": \"{}\",\n",
            json_escape(&run.outcome)
        ));
        body.push_str(&format!(
            "        \"attempted_repair\": \"{}\",\n",
            json_escape(&run.attempted_repair)
        ));
        body.push_str(&format!(
            "        \"evidence_movement\": \"{}\",\n",
            json_escape(&run.evidence_movement)
        ));
        body.push_str(&format!(
            "        \"operator_note\": \"{}\",\n",
            json_escape(&run.operator_note)
        ));
        body.push_str("        \"must_not_change\": [");
        write_json_string_array(&mut body, &run.must_not_change);
        body.push_str("],\n");
        body.push_str("        \"raw_evidence_refs\": [");
        write_json_string_array(&mut body, &run.raw_evidence_refs);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"missing_receipt_reason\": {},\n",
            json_optional_string(run.missing_receipt_reason.as_deref())
        ));
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"python_real_repo_evals\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"receipt_dir\": \"fixtures/python-real-repo-evals\",\n");
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"cases\": {},\n",
        python_real_repo_eval_runs.len()
    ));
    body.push_str(&format!(
        "      \"closed\": {},\n",
        python_real_repo_eval_runs
            .iter()
            .filter(|run| run.gap_movement == "closed")
            .count()
    ));
    body.push_str(&format!(
        "      \"usable\": {},\n",
        python_real_repo_eval_runs
            .iter()
            .filter(|run| run.usability == "usable")
            .count()
    ));
    body.push_str(&format!(
        "      \"static_limit_cases\": {},\n",
        python_static_limit_eval_runs.len()
    ));
    body.push_str(&format!(
        "      \"no_action_cases\": {}\n",
        python_no_action_eval_runs.len()
    ));
    body.push_str("    },\n    \"cases\": [\n");
    for (index, run) in python_real_repo_eval_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"repo_shape\": \"{}\",\n",
            json_escape(&run.repo_shape)
        ));
        body.push_str(&format!(
            "        \"source_kind\": \"{}\",\n",
            json_escape(&run.source_kind)
        ));
        body.push_str(&format!(
            "        \"source_ref\": \"{}\",\n",
            json_escape(&run.source_ref)
        ));
        body.push_str(&format!(
            "        \"command\": \"{}\",\n",
            json_escape(&run.command)
        ));
        body.push_str(&format!("        \"runtime_ms\": {},\n", run.runtime_ms));
        body.push_str(&format!(
            "        \"top_finding_summary\": \"{}\",\n",
            json_escape(&run.top_finding_summary)
        ));
        body.push_str(&format!(
            "        \"canonical_gap_id\": \"{}\",\n",
            json_escape(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "        \"repair_card_present\": {},\n",
            run.repair_card_present
        ));
        body.push_str(&format!(
            "        \"repair_action\": \"{}\",\n",
            json_escape(&run.repair_action)
        ));
        body.push_str(&format!(
            "        \"agent_packet_present\": {},\n",
            run.agent_packet_present
        ));
        body.push_str(&format!(
            "        \"agent_packet_task\": \"{}\",\n",
            json_escape(&run.agent_packet_task)
        ));
        body.push_str(&format!(
            "        \"agent_packet_command\": \"{}\",\n",
            json_escape(&run.agent_packet_command)
        ));
        body.push_str("        \"agent_packet_allowed_files\": [");
        write_json_string_array(&mut body, &run.agent_packet_allowed_files);
        body.push_str("],\n");
        body.push_str("        \"agent_packet_forbidden_files\": [");
        write_json_string_array(&mut body, &run.agent_packet_forbidden_files);
        body.push_str("],\n");
        body.push_str("        \"agent_packet_stop_if\": [");
        write_json_string_array(&mut body, &run.agent_packet_stop_if);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"changed_owner\": \"{}\",\n",
            json_escape(&run.changed_owner)
        ));
        body.push_str(&format!(
            "        \"missing_discriminator\": \"{}\",\n",
            json_escape(&run.missing_discriminator)
        ));
        body.push_str(&format!(
            "        \"suggested_test_file\": \"{}\",\n",
            json_escape(&run.suggested_test_file)
        ));
        body.push_str(&format!(
            "        \"suggested_test_name\": \"{}\",\n",
            json_escape(&run.suggested_test_name)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"verify_result\": \"{}\",\n",
            json_escape(&run.verify_result)
        ));
        body.push_str(&format!(
            "        \"verify_summary\": \"{}\",\n",
            json_escape(&run.verify_summary)
        ));
        body.push_str(&format!(
            "        \"after_command\": \"{}\",\n",
            json_escape(&run.after_command)
        ));
        body.push_str(&format!(
            "        \"after_runtime_ms\": {},\n",
            run.after_runtime_ms
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_result\": \"{}\",\n",
            json_escape(&run.receipt_result)
        ));
        body.push_str(&format!(
            "        \"gap_movement\": \"{}\",\n",
            json_escape(&run.gap_movement)
        ));
        body.push_str(&format!("        \"closed_gaps\": {},\n", run.closed_gaps));
        body.push_str(&format!(
            "        \"usability\": \"{}\",\n",
            json_escape(&run.usability)
        ));
        body.push_str(&format!(
            "        \"false_positive_notes\": \"{}\",\n",
            json_escape(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "        \"limitation_notes\": \"{}\",\n",
            json_escape(&run.limitation_notes)
        ));
        body.push_str("        \"unsupported_limitations\": [");
        write_json_string_array(&mut body, &run.unsupported_limitations);
        body.push_str("],\n");
        body.push_str("        \"ranked_top_3_findings\": [");
        dogfood_push_python_ranked_findings_json(&mut body, &run.ranked_top_3_findings);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"ranked_top_3_limit_reason\": {},\n",
            json_optional_string(run.ranked_top_3_limit_reason.as_deref())
        ));
        body.push_str("        \"claim_boundary\": [");
        write_json_string_array(&mut body, &run.claim_boundary);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ],\n    \"static_limit_cases\": [\n");
    for (index, run) in python_static_limit_eval_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"repo_shape\": \"{}\",\n",
            json_escape(&run.repo_shape)
        ));
        body.push_str(&format!(
            "        \"source_kind\": \"{}\",\n",
            json_escape(&run.source_kind)
        ));
        body.push_str(&format!(
            "        \"source_ref\": \"{}\",\n",
            json_escape(&run.source_ref)
        ));
        body.push_str(&format!(
            "        \"command\": \"{}\",\n",
            json_escape(&run.command)
        ));
        body.push_str(&format!("        \"runtime_ms\": {},\n", run.runtime_ms));
        body.push_str(&format!(
            "        \"finding_id\": \"{}\",\n",
            json_escape(&run.finding_id)
        ));
        body.push_str(&format!(
            "        \"changed_owner\": \"{}\",\n",
            json_escape(&run.changed_owner)
        ));
        body.push_str(&format!(
            "        \"static_limit_kind\": \"{}\",\n",
            json_escape(&run.static_limit_kind)
        ));
        body.push_str(&format!(
            "        \"classification\": \"{}\",\n",
            json_escape(&run.classification)
        ));
        body.push_str("        \"stop_reasons\": [");
        write_json_string_array(&mut body, &run.stop_reasons);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"related_test_file\": \"{}\",\n",
            json_escape(&run.related_test_file)
        ));
        body.push_str(&format!(
            "        \"related_test_name\": \"{}\",\n",
            json_escape(&run.related_test_name)
        ));
        body.push_str(&format!(
            "        \"why_not_actionable\": \"{}\",\n",
            json_escape(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "        \"repair_card_present\": {},\n",
            run.repair_card_present
        ));
        body.push_str(&format!(
            "        \"agent_packet_present\": {},\n",
            run.agent_packet_present
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"verify_result\": \"{}\",\n",
            json_escape(&run.verify_result)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_result\": \"{}\",\n",
            json_escape(&run.receipt_result)
        ));
        body.push_str(&format!(
            "        \"gap_movement\": \"{}\",\n",
            json_escape(&run.gap_movement)
        ));
        body.push_str(&format!(
            "        \"false_positive_notes\": \"{}\",\n",
            json_escape(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "        \"limitation_notes\": \"{}\",\n",
            json_escape(&run.limitation_notes)
        ));
        body.push_str("        \"claim_boundary\": [");
        write_json_string_array(&mut body, &run.claim_boundary);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ],\n    \"no_action_cases\": [\n");
    for (index, run) in python_no_action_eval_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"repo_shape\": \"{}\",\n",
            json_escape(&run.repo_shape)
        ));
        body.push_str(&format!(
            "        \"source_kind\": \"{}\",\n",
            json_escape(&run.source_kind)
        ));
        body.push_str(&format!(
            "        \"source_ref\": \"{}\",\n",
            json_escape(&run.source_ref)
        ));
        body.push_str(&format!(
            "        \"command\": \"{}\",\n",
            json_escape(&run.command)
        ));
        body.push_str(&format!("        \"runtime_ms\": {},\n", run.runtime_ms));
        body.push_str(&format!(
            "        \"finding_id\": \"{}\",\n",
            json_escape(&run.finding_id)
        ));
        body.push_str(&format!(
            "        \"changed_owner\": \"{}\",\n",
            json_escape(&run.changed_owner)
        ));
        body.push_str(&format!(
            "        \"no_action_kind\": \"{}\",\n",
            json_escape(&run.no_action_kind)
        ));
        body.push_str(&format!(
            "        \"classification\": \"{}\",\n",
            json_escape(&run.classification)
        ));
        body.push_str("        \"stop_reasons\": [");
        write_json_string_array(&mut body, &run.stop_reasons);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"related_test_file\": \"{}\",\n",
            json_escape(&run.related_test_file)
        ));
        body.push_str(&format!(
            "        \"related_test_name\": \"{}\",\n",
            json_escape(&run.related_test_name)
        ));
        body.push_str(&format!(
            "        \"why_not_actionable\": \"{}\",\n",
            json_escape(&run.why_not_actionable)
        ));
        body.push_str(&format!(
            "        \"repair_card_present\": {},\n",
            run.repair_card_present
        ));
        body.push_str(&format!(
            "        \"agent_packet_present\": {},\n",
            run.agent_packet_present
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"verify_result\": \"{}\",\n",
            json_escape(&run.verify_result)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"receipt_result\": \"{}\",\n",
            json_escape(&run.receipt_result)
        ));
        body.push_str(&format!(
            "        \"gap_movement\": \"{}\",\n",
            json_escape(&run.gap_movement)
        ));
        body.push_str(&format!(
            "        \"false_positive_notes\": \"{}\",\n",
            json_escape(&run.false_positive_notes)
        ));
        body.push_str(&format!(
            "        \"limitation_notes\": \"{}\",\n",
            json_escape(&run.limitation_notes)
        ));
        body.push_str("        \"claim_boundary\": [");
        write_json_string_array(&mut body, &run.claim_boundary);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"python_repair_routing_quality\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"input\": \"fixtures/python-real-repo-evals/corpus.json\",\n");
    body.push_str(&format!(
        "    \"quality_gate\": {{ \"status\": \"{}\", \"reason\": \"{}\" }},\n",
        json_escape(&python_repair_quality.gate_status),
        json_escape(&python_repair_quality.gate_reason)
    ));
    body.push_str("    \"summary\": {\n");
    body.push_str(&format!(
        "      \"cases\": {},\n",
        python_repair_quality.cases
    ));
    body.push_str(&format!(
        "      \"static_limit_no_action_cases\": {},\n",
        python_static_limit_eval_runs.len()
    ));
    body.push_str(&format!(
        "      \"ordinary_no_action_cases\": {},\n",
        python_no_action_eval_runs.len()
    ));
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "top_1_actionable_precision",
        python_repair_quality.top_1_actionable_usable,
        python_repair_quality.cases,
        true,
        "top finding is usable, repair-card-backed, and has no observed false actionability",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "top_3_actionable_precision",
        python_repair_quality.top_3_actionable_usable,
        python_repair_quality.top_3_ranked_findings_checked,
        true,
        "ranked Python repair-card findings within the top-3 window are usable, concrete, placed, verifiable, and false-positive clean",
    );
    body.push_str(&format!(
        "      \"full_top_3_capture_cases\": {{ \"status\": \"{}\", \"count\": {}, \"checked\": {}, \"reason\": \"at least one eval preserves all three ranked Python repair cards for direct top-3 precision review\" }},\n",
        if python_repair_quality.full_top_3_capture_cases > 0 {
            "pass"
        } else {
            "review"
        },
        python_repair_quality.full_top_3_capture_cases,
        python_repair_quality.cases
    ));
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "verify_command_validity",
        python_repair_quality.verify_command_valid,
        python_repair_quality.cases,
        true,
        "verify command is pytest or unittest and passed in the recorded eval",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "agent_packet_boundary_validity",
        python_repair_quality.agent_packet_bounded,
        python_repair_quality.cases,
        true,
        "agent packet is present, gap-scoped, test-file bounded, production-file-forbidden, and has stop conditions",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "concrete_discriminator_rate",
        python_repair_quality.concrete_discriminator,
        python_repair_quality.cases,
        true,
        "missing discriminator is a concrete non-placeholder value",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "related_test_location_rate",
        python_repair_quality.suggested_test_location,
        python_repair_quality.cases,
        true,
        "suggested test file and test name are present",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "receipt_closure_rate",
        python_repair_quality.receipt_closed,
        python_repair_quality.cases,
        true,
        "before/after receipt closes the canonical Python gap",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "false_actionable_rate",
        python_repair_quality.false_actionable,
        python_repair_quality.cases,
        false,
        "recorded eval reports observed false actionability",
    );
    dogfood_push_python_quality_ratio_json(
        &mut body,
        "crash_rate",
        python_repair_quality.crashes,
        python_repair_quality.cases,
        false,
        "eval validation reported parser/reporting crashes or contract errors",
    );
    body.push_str(&format!(
        "      \"ranked_top_3_cases_with_capture\": {{ \"status\": \"{}\", \"count\": {}, \"checked\": {}, \"reason\": \"every eval case records ranked top-3 finding capture or a fewer-than-three stop reason\" }}\n",
        if python_repair_quality.top_3_cases_with_ranked_capture == python_repair_quality.cases
            && python_repair_quality.cases > 0
        {
            "pass"
        } else {
            "review"
        },
        python_repair_quality.top_3_cases_with_ranked_capture,
        python_repair_quality.cases
    ));
    body.push_str("    },\n    \"unsupported_limitation_distribution\": [");
    for (index, (limitation, count)) in python_repair_quality
        .unsupported_limitation_distribution
        .iter()
        .enumerate()
    {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&format!(
            "{{ \"kind\": \"{}\", \"cases\": {} }}",
            json_escape(limitation),
            count
        ));
    }
    body.push_str("],\n    \"static_limit_no_action_distribution\": [");
    for (index, (limitation, count)) in python_static_limit_distribution.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&format!(
            "{{ \"kind\": \"{}\", \"cases\": {} }}",
            json_escape(limitation),
            count
        ));
    }
    body.push_str("],\n    \"ordinary_no_action_distribution\": [");
    for (index, (state, count)) in python_no_action_distribution.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&format!(
            "{{ \"kind\": \"{}\", \"cases\": {} }}",
            json_escape(state),
            count
        ));
    }
    body.push_str("]\n  },\n  \"user_surface_projection_alignment\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/user-surface-projection-alignment\",\n    \"cases\": [\n",
    );
    for (index, run) in user_surface_projection_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"surface\": \"{}\",\n",
            json_escape(&run.surface)
        ));
        body.push_str(&format!(
            "        \"artifact\": \"{}\",\n",
            json_escape(&run.artifact)
        ));
        body.push_str(&format!(
            "        \"headline\": \"{}\",\n",
            json_escape(&run.headline)
        ));
        body.push_str(&format!(
            "        \"run_status\": \"{}\",\n",
            json_escape(&run.run_status)
        ));
        body.push_str(&format!(
            "        \"projection_basis\": \"{}\",\n",
            json_escape(&run.projection_basis)
        ));
        body.push_str(&format!(
            "        \"canonical_gap_id\": \"{}\",\n",
            json_escape(&run.canonical_gap_id)
        ));
        body.push_str(&format!(
            "        \"packet_id\": \"{}\",\n",
            json_escape(&run.packet_id)
        ));
        body.push_str(&format!(
            "        \"repair_kind\": \"{}\",\n",
            json_escape(&run.repair_kind)
        ));
        body.push_str(&format!(
            "        \"top_next_action_kind\": \"{}\",\n",
            json_escape(&run.top_next_action_kind)
        ));
        body.push_str(&format!(
            "        \"verify_command\": \"{}\",\n",
            json_escape(&run.verify_command)
        ));
        body.push_str(&format!(
            "        \"receipt_command\": \"{}\",\n",
            json_escape(&run.receipt_command)
        ));
        body.push_str(&format!(
            "        \"source_alignment_case\": \"{}\",\n",
            json_escape(&run.source_alignment_case)
        ));
        body.push_str(&format!(
            "        \"limitation_category\": \"{}\",\n",
            json_escape(&run.limitation_category)
        ));
        body.push_str(&format!(
            "        \"runtime_repair_command\": \"{}\",\n",
            json_escape(&run.runtime_repair_command)
        ));
        body.push_str(&format!(
            "        \"actionable_count\": {},\n",
            run.actionable_count
        ));
        body.push_str(&format!(
            "        \"raw_findings_total\": {},\n",
            run.raw_findings_total
        ));
        body.push_str(&format!(
            "        \"consumes_canonical_state\": {},\n",
            run.consumes_canonical_state
        ));
        body.push_str(&format!(
            "        \"reinterprets_raw_findings\": {},\n",
            run.reinterprets_raw_findings
        ));
        body.push_str(&format!(
            "        \"raw_findings_headline\": {},\n",
            run.raw_findings_headline
        ));
        body.push_str(&format!("        \"advisory\": {},\n", run.advisory));
        body.push_str(&format!(
            "        \"blocking_default\": {},\n",
            run.blocking_default
        ));
        body.push_str(&format!(
            "        \"limited_state_visible\": {},\n",
            run.limited_state_visible
        ));
        body.push_str(&format!(
            "        \"stale_state_visible\": {},\n",
            run.stale_state_visible
        ));
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"pr_inline_comment_publisher\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str("    \"default_inline_comments\": \"off\",\n");
    body.push_str(
        "    \"receipt_dir\": \"fixtures/boundary_gap/expected/pr-inline-comment-publisher\",\n    \"cases\": [\n",
    );
    for (index, run) in pr_inline_comment_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!(
            "        \"expected_report\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_report))
        ));
        body.push_str(&format!(
            "        \"expected_markdown\": \"{}\",\n",
            json_escape(&normalize_path(&run.expected_markdown))
        ));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!(
            "        \"mode\": \"{}\",\n",
            json_escape(&run.mode)
        ));
        body.push_str(&format!("        \"publishable\": {},\n", run.publishable));
        body.push_str(&format!("        \"skipped\": {},\n", run.skipped));
        body.push_str(&format!("        \"blocked\": {},\n", run.blocked));
        body.push_str(&format!(
            "        \"safe_to_publish\": {},\n",
            run.safe_to_publish
        ));
        body.push_str("        \"operations\": [");
        write_json_string_array(&mut body, &run.operations);
        body.push_str("],\n");
        body.push_str("        \"skip_reasons\": [");
        write_json_string_array(&mut body, &run.skip_reasons);
        body.push_str("],\n");
        body.push_str("        \"blocked_reasons\": [");
        write_json_string_array(&mut body, &run.blocked_reasons);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_mode\": \"{}\",\n",
            json_escape(&run.expected_mode)
        ));
        body.push_str(&format!(
            "        \"expected_publishable\": {},\n",
            run.expected_publishable
        ));
        body.push_str(&format!(
            "        \"expected_skipped\": {},\n",
            run.expected_skipped
        ));
        body.push_str(&format!(
            "        \"expected_blocked\": {},\n",
            run.expected_blocked
        ));
        body.push_str(&format!(
            "        \"expected_safe_to_publish\": {},\n",
            run.expected_safe_to_publish
        ));
        body.push_str("        \"expected_operations\": [");
        write_json_string_array(&mut body, &run.expected_operations);
        body.push_str("],\n");
        body.push_str("        \"expected_skip_reasons\": [");
        write_json_string_array(&mut body, &run.expected_skip_reasons);
        body.push_str("],\n");
        body.push_str("        \"expected_blocked_reasons\": [");
        write_json_string_array(&mut body, &run.expected_blocked_reasons);
        body.push_str("],\n");
        body.push_str(&format!(
            "        \"reason\": \"{}\",\n",
            json_escape(&run.reason)
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  },\n  \"gate_adoption\": {\n");
    body.push_str("    \"default_ci_blocking\": false,\n");
    body.push_str(
        "    \"receipt_dir\": \"target/ripr/dogfood/gate-adoption\",\n    \"cases\": [\n",
    );
    for (index, run) in gate_runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("      {\n");
        body.push_str(&format!(
            "        \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "        \"mode\": \"{}\",\n",
            json_escape(&run.mode)
        ));
        body.push_str(&format!(
            "        \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!(
            "        \"json_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.json_path))
        ));
        body.push_str(&format!(
            "        \"markdown_path\": \"{}\",\n",
            json_escape(&normalize_path(&run.markdown_path))
        ));
        body.push_str(&format!("        \"duration_ms\": {},\n", run.duration_ms));
        body.push_str(&format!(
            "        \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!("        \"blocking\": {},\n", run.blocking));
        body.push_str(&format!(
            "        \"acknowledged\": {},\n",
            run.acknowledged
        ));
        body.push_str(&format!("        \"advisory\": {},\n", run.advisory));
        body.push_str(&format!(
            "        \"expected_status\": \"{}\",\n",
            json_escape(&run.expected_status)
        ));
        body.push_str(&format!(
            "        \"expected_blocking\": {},\n",
            run.expected_blocking
        ));
        body.push_str(&format!(
            "        \"expected_acknowledged\": {},\n",
            run.expected_acknowledged
        ));
        body.push_str(&format!(
            "        \"expected_advisory\": {},\n",
            run.expected_advisory
        ));
        body.push_str(&format!(
            "        \"exit_success\": {},\n",
            run.exit_success
        ));
        body.push_str(&format!(
            "        \"expected_exit_success\": {},\n",
            run.expected_exit_success
        ));
        body.push_str("        \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n      }");
    }
    body.push_str("\n    ]\n  }\n}\n");
    body
}
