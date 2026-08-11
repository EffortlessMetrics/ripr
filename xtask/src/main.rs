#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use ripr::output::receipt_lifecycle::{
    RECEIPT_FOUND, RECEIPT_GAP_MISMATCH, RECEIPT_MISSING, RECEIPT_MOVEMENT_IMPROVED,
    RECEIPT_MOVEMENT_UNCHANGED, RECEIPT_NOT_APPLICABLE, receipt_lifecycle_state_from_movement,
    receipt_lifecycle_state_is_present,
};
use ripr::output::start_here_state::{
    START_HERE_ACTIONABLE_GAP, START_HERE_CLEAN, START_HERE_MISSING_ARTIFACTS,
    START_HERE_PREVIEW_LIMITED,
};

mod agent_skills;
mod branch_inventory;
mod cache;
mod command;
mod dispatch;
mod dogfood;
mod evidence_audit;
mod evidence_promotion;
mod evidence_quality;
mod fixture_contracts;
mod no_panic;
mod policy;
mod public_api_surface;
mod repo_readiness;
mod schema_pattern;
mod types;
pub(crate) use types::*;
mod reports;
mod ripr_swarm;
mod run;
mod verification_contracts;
mod version;
mod windows_advisory;

use command::{
    CommandCatalogEntry, XtaskCommand, command_catalog, known_command_root, known_commands,
};
#[cfg(test)]
use command::{help_message, unknown_command_message};
#[cfg(test)]
pub(crate) use dogfood::{
    BunUbPreviewSummaryArgs, ConfiguredBridgeInventoryArgs, CrossLanguageOracleGraphCase,
    CrossLanguageOracleGraphRawRef, DogfoodBunUbCrossLanguageRun,
    DogfoodBunUbCrossLanguageScenario, DogfoodEditorFirstPrBridgeRun, DogfoodEditorGapCockpitRun,
    DogfoodFindingAlignmentRun, DogfoodFindingAlignmentScenario, DogfoodFirstActionRun,
    DogfoodFirstPrRun, DogfoodFrontPanelRun, DogfoodGateRun, DogfoodGeneratedCiCockpitRun,
    DogfoodLanguagePreviewRun, DogfoodPrInlineCommentRun, DogfoodPreviewProjectionRuns,
    DogfoodPythonNoActionEvalScenario, DogfoodPythonRankedFinding, DogfoodPythonRealRepoEvalRun,
    DogfoodPythonRealRepoEvalScenario, DogfoodPythonStaticLimitEvalScenario,
    DogfoodRealRepairAttemptRun, DogfoodRealRepairAttemptScenario, DogfoodReportInputs,
    DogfoodReportPacketIndexRun, DogfoodRun, DogfoodSurfaceProjectionAlignmentRun,
    DogfoodTypescriptPreviewRepairLoopRun, DogfoodTypescriptPreviewRepairLoopScenario,
    DogfoodUserSurfaceProjectionRun, GENERATED_CI_FIRST_ACTION_REPAIR,
    GENERATED_CI_FIRST_PR_REPAIR, GENERATED_CI_FRONT_PANEL_REPAIR,
    GENERATED_CI_PACKET_INDEX_REPAIR, TypeScriptBunUbCalibrationCase,
    bun_ub_calibration_report_markdown, bun_ub_calibration_report_value,
    bun_ub_preview_summary_markdown, bun_ub_preview_summary_report_value,
    configured_bridge_inventory_markdown, configured_bridge_inventory_report_value,
    cross_language_oracle_graph_cases, cross_language_oracle_graph_corpus_path,
    dogfood_bun_ub_cross_language_scenarios, dogfood_class_counts,
    dogfood_editor_first_pr_bridge_run, dogfood_editor_first_pr_bridge_scenarios,
    dogfood_editor_gap_cockpit_run, dogfood_editor_gap_cockpit_scenarios,
    dogfood_first_action_scenarios, dogfood_first_pr_metrics, dogfood_first_pr_run,
    dogfood_first_pr_scenarios, dogfood_gate_adoption_run, dogfood_gate_adoption_scenarios,
    dogfood_generated_ci_cockpit_run_from_workflow, dogfood_language_preview_run,
    dogfood_language_preview_scenarios, dogfood_pr_inline_comment_run,
    dogfood_pr_inline_comment_scenarios, dogfood_pr_review_front_panel_run,
    dogfood_pr_review_front_panel_scenarios, dogfood_push_python_quality_ratio_json,
    dogfood_push_python_ranked_findings_json, dogfood_python_no_action_eval_scenarios,
    dogfood_python_ranked_findings, dogfood_python_real_repo_eval_scenarios,
    dogfood_python_static_limit_eval_scenarios, dogfood_report_json, dogfood_report_markdown,
    dogfood_report_packet_index_run, dogfood_report_packet_index_scenarios,
    dogfood_typescript_false_actionable_audit_summary,
    dogfood_typescript_preview_repair_loop_scenarios, finding_alignment_verify_command_is_missing,
    json_number_after, parse_bun_ub_preview_summary_args, parse_configured_bridge_inventory_args,
    repo_rooted_fixture_path, typescript_bun_ub_calibration_cases,
    typescript_preview_false_actionable_audit_cases,
};
pub(crate) use dogfood::{
    DogfoodSurfaceProjectionAlignmentScenario, DogfoodUserSurfaceProjectionScenario,
    bun_ub_calibration_impl, bun_ub_preview_summary_impl, configured_bridge_inventory_impl,
    cross_language_oracle_graph_case_errors, cross_language_oracle_graph_cases_at,
    cross_language_oracle_route_quality_push_markdown,
    cross_language_oracle_route_quality_report_value, dogfood_bun_ub_cross_language_run,
    dogfood_bun_ub_cross_language_scenarios_at, dogfood_finding_alignment_run,
    dogfood_finding_alignment_scenarios, dogfood_python_no_action_eval_run,
    dogfood_python_no_action_eval_scenarios_at, dogfood_python_real_repo_eval_run,
    dogfood_python_real_repo_eval_scenarios_at, dogfood_python_repair_routing_quality_summary,
    dogfood_python_static_limit_eval_run, dogfood_python_static_limit_eval_scenarios_at,
    dogfood_real_repair_attempt_run, dogfood_real_repair_attempt_scenarios,
    dogfood_surface_projection_alignment_run, dogfood_surface_projection_alignment_scenarios,
    dogfood_typescript_preview_repair_loop_run,
    dogfood_typescript_preview_repair_loop_scenarios_at, dogfood_user_surface_projection_run,
    dogfood_user_surface_projection_scenarios, json_string_array_field, json_summary_count,
    typescript_bun_ub_calibration_case_errors, typescript_bun_ub_calibration_cases_at,
    typescript_preview_false_actionable_audit_case_errors,
    typescript_preview_false_actionable_audit_cases_at,
};
#[cfg(test)]
pub(crate) use evidence_audit::{
    AuditActionableGapProjectionInput, LANE1_EVIDENCE_AUDIT_DEFAULT_TIMEOUT_MS,
    Lane1EvidenceAuditRepoExposureGeneration, Lane1EvidenceAuditRepoExposureOutcome,
    audit_actionable_gap_projection_exclusion_reasons,
    audit_actionable_gap_verify_command_with_source, lane1_evidence_audit_from_repo_exposure,
    lane1_evidence_audit_limited_report, lane1_evidence_audit_limited_report_from_run_limitation,
    lane1_evidence_audit_repo_exposure_args,
    lane1_evidence_audit_report_from_complete_repo_exposure, lane1_evidence_audit_timeout_error,
    lane1_evidence_audit_timeout_ms_from_env, lane1_parse_cache_store_skip_status,
    lane1_repo_exposure_cache_store_limitation, lane1_repo_exposure_file_looks_complete,
    lane1_repo_exposure_large_cache_preflight_limitation_for_root,
    lane1_static_limitation_backlog_json, parse_actionable_gap_outcomes_args,
    parse_lane1_evidence_audit_sample_seam_limit, static_limitation_category,
    static_limitation_repair_route_for_subroute, static_limitation_subroute,
    write_lane1_evidence_audit_repo_exposure_with_runner,
};
pub(crate) use evidence_audit::{
    CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE,
    CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY,
    CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY, CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE,
    EVIDENCE_QUALITY_SCORECARD_AUDIT_REGENERATION_FAILED, LANE1_EVIDENCE_AUDIT_TOP_LIMIT,
    Lane1EvidenceAuditAlignmentClassCoverage, Lane1EvidenceAuditBuilder, Lane1EvidenceAuditReport,
    Lane1EvidenceAuditRunLimitation, Lane1EvidenceClassWorkItem, Lane1RuntimeStatus,
    Lane1StaticLimitationBacklogSample, actionable_gap_id_candidates,
    actionable_gap_outcome_state_counts_from_entries, actionable_gap_outcomes_json,
    actionable_gap_outcomes_markdown, actionable_gap_outcomes_missing_verify_result_count,
    actionable_gap_outcomes_report_from_values, actionable_gap_outcomes_report_impl,
    actionable_gap_push_id_candidate, audit_actionable_gap_target_test_shape, audit_array,
    audit_bool, audit_count_rows_json, audit_count_rows_map, audit_get,
    audit_guidance_field_is_missing, audit_identifier_slug, audit_increment, audit_markdown_cell,
    audit_non_empty_string, audit_push_count, audit_push_projection_exclusion_reason,
    audit_push_value_counts_table_limited, audit_slug, audit_string, audit_string_array,
    audit_structured_raw_evidence_refs_count, audit_usize, audit_verify_command_is_missing,
    audit_verify_command_is_unbounded_repo_exposure_snapshot_compare,
    lane1_actionable_gap_packets_json, lane1_actionable_gap_packets_markdown,
    lane1_evidence_audit_json, lane1_evidence_audit_markdown, lane1_evidence_audit_report_impl,
    lane1_runtime_status_from_report_value, lane1_runtime_status_full, lane1_runtime_status_json,
    lane1_runtime_status_limited_input, lane1_runtime_status_priority,
    lane1_runtime_status_push_markdown, lane1_runtime_status_with_input_path,
    lane1_static_limitation_backlog_sample_json, repo_exposure_latency_trace_json,
    static_limitation_backlog_packet_non_claims, static_limitation_repair_route,
    static_limitation_unlock_condition, static_limitation_why_not_actionable,
};
#[cfg(test)]
pub(crate) use evidence_promotion::{
    EVIDENCE_PROMOTION_EXTERNAL_JSON, EVIDENCE_PROMOTION_EXTERNAL_MD,
    EVIDENCE_PROMOTION_HONESTY_CORPUS, EvidencePromotionExternalCase,
    EvidencePromotionExternalLaunch, EvidencePromotionExternalRun,
    EvidencePromotionSemanticAssertion, ExpectedRepairPacketDetail,
    evidence_promotion_external_failure_kind, evidence_promotion_external_semantic_violations,
    evidence_promotion_human_class_line_matches, evidence_promotion_human_oracle_line_matches,
    evidence_promotion_pure_failure_kind, evidence_promotion_semantic_violations,
    validate_evidence_promotion_honesty_corpus_at, write_evidence_promotion_external_report,
};
pub(crate) use evidence_promotion::{
    check_evidence_promotion_honesty, validate_evidence_promotion_honesty_corpus,
};
pub(crate) use evidence_quality::{
    EVIDENCE_QUALITY_TREND_PREVIOUS_ARTIFACT_UNAVAILABLE, audit_evidence_class_work_queue,
    evidence_quality_scorecard_report_impl, evidence_quality_trend_report_impl,
    generated_at_unix_ms, report_has_run_limitations,
};
#[cfg(test)]
pub(crate) use evidence_quality::{
    EvidenceQualityScorecardInput, EvidenceQualityScorecardInputs, EvidenceQualityScorecardReport,
    EvidenceQualityTrendInputs, EvidenceQualityTrendReport,
    evidence_quality_scorecard_audit_regeneration_failure_audit,
    evidence_quality_scorecard_from_values, evidence_quality_scorecard_json,
    evidence_quality_scorecard_markdown, evidence_quality_trend_from_values,
    evidence_quality_trend_json, evidence_quality_trend_markdown,
    finding_alignment_raw_to_canonical_ratio,
};
pub(crate) use fixture_contracts::{
    BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE, BUN_FFI_NEGATIVE_OFFSET_TS_TEST_SURFACE,
    BUN_MARKDOWN_TS_TEST_FILE, BUN_NODE_FS_TS_TEST_FILE, BUN_UB_CROSS_LANGUAGE_DOGFOOD_CORPUS,
    BUN_WRITE_TS_TEST_FILE, CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS, EVIDENCE_RECORD_CONTRACT_CORPUS,
    PYTHON_REAL_REPO_EVAL_CORPUS, REAL_REPAIR_ATTEMPTS_CORPUS, SURFACE_PROJECTION_ALIGNMENT_CORPUS,
    TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS, TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS,
    TYPESCRIPT_PREVIEW_REPAIR_LOOP_CORPUS, USER_SURFACE_PROJECTION_ALIGNMENT_CORPUS,
    USER_SURFACE_PROJECTION_REQUIRED_SURFACES, check_fixture_contracts,
    editor_first_pr_bridge_case_requires_first_screen_contract,
    validate_assistant_loop_health_fixture_corpus,
    validate_assistant_loop_health_fixture_corpus_at,
    validate_editor_first_pr_bridge_first_screen_contract,
    validate_first_successful_pr_actionable_json, validate_first_successful_pr_actionable_markdown,
};
#[cfg(test)]
pub(crate) use fixture_contracts::{
    BUN_UB_CROSS_LANGUAGE_DOGFOOD_REQUIRED_CASES, CROSS_LANGUAGE_ORACLE_GRAPH_REQUIRED_CASES,
    EDITOR_ADOPTION_ASSURANCE_CASES, EDITOR_FIRST_RUN_USABILITY_CASES,
    PYTHON_REAL_REPO_EVAL_REQUIRED_CASES, PYTHON_REAL_REPO_EVAL_REQUIRED_NO_ACTION_CASES,
    PYTHON_REAL_REPO_EVAL_REQUIRED_STATIC_LIMIT_CASES, REAL_REPAIR_ATTEMPTS_REQUIRED_CASES,
    TYPESCRIPT_BUN_UB_CALIBRATION_REQUIRED_CASES,
    TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_REQUIRED_CASES,
    TYPESCRIPT_PREVIEW_REPAIR_LOOP_REQUIRED_CASES, USER_SURFACE_PROJECTION_REQUIRED_RUN_STATUSES,
    editor_adoption_assurance_case_fails_closed, first_successful_pr_outcome_input_path,
    user_surface_projection_required_run_status_violations,
    user_surface_projection_source_alignment_errors, validate_actionable_gap_outcomes_fixture_case,
    validate_actionable_gap_outcomes_fixture_corpus,
    validate_actionable_gap_outcomes_fixture_corpus_at,
    validate_cross_language_oracle_graph_fixture_corpus_at,
    validate_editor_adoption_assurance_fixture_corpus,
    validate_editor_first_run_usability_fixture_corpus, validate_editor_gap_cockpit_fixture_case,
    validate_editor_gap_cockpit_fixture_corpus, validate_evidence_quality_benchmark_corpus_value,
    validate_evidence_quality_benchmark_fixture_corpus_at,
    validate_evidence_record_contract_fixture_corpus_at,
    validate_finding_alignment_dogfood_fixture_corpus,
    validate_first_successful_pr_fixture_corpus_at, validate_first_successful_pr_outcome_receipt,
    validate_gap_decision_ledger_corpus_value, validate_gap_decision_ledger_fixture_corpus_at,
    validate_lane1_evidence_quality_failure_fixture_corpus_at,
    validate_perl_lsp_facts_exporter_fixture_corpus,
    validate_perl_lsp_facts_exporter_fixture_corpus_at,
    validate_perl_real_repo_eval_fixture_corpus, validate_perl_real_repo_eval_fixture_corpus_at,
    validate_pr_inline_comment_publisher_fixture_corpus_at,
    validate_pr_review_front_panel_fixture_corpus_at,
    validate_report_packet_index_fixture_corpus_at, validate_swarm_plan_packet_fixture_case,
    validate_swarm_plan_packet_fixture_corpus,
    validate_typescript_bun_ub_calibration_fixture_corpus_at,
};
use no_panic::{
    contains_word, parse_string_value, parse_toml_key_value, parse_usize_value,
    strip_toml_value_comment,
};
use policy::{
    check_allow_attributes, check_ci_lane_whitelist, check_doc_roles, check_droid_review_config,
    check_executable_files, check_file_policy, check_local_context, check_network_policy,
    check_no_panic_family, check_positioning_language, check_process_policy, check_product_copy,
    check_proof_packs, check_release_targets, check_static_language, check_workflows,
};
use public_api_surface::public_api_surface;
#[cfg(test)]
use repo_readiness::{PrReadyStep, pr_ready_next_action, pr_ready_status_from_report_status};
use repo_readiness::{
    cockpit_json, cockpit_markdown, pr_ready_json, pr_ready_markdown, pr_ready_status,
    run_readiness_step,
};
#[cfg(test)]
pub(crate) use reports::release_server::{
    ReleaseServerAsset, create_zip_archive, normalize_release_version, release_server_archive,
    release_server_assets, release_server_manifest, release_server_readme, required_release_arg,
};
#[cfg(test)]
pub(crate) use reports::{
    BADGE_ENDPOINT_FILES, BadgeArtifactJob, BadgeBasisReport, BadgeBasisSignal,
    BadgeCanonicalProjection, BadgeCountBreakdown, BadgeEndpointSnapshot, BadgeNativeAuditSnapshot,
    BadgeNativeSlot, REPO_BADGE_ARTIFACT_DEFAULT_TIMEOUT_MS, REPO_BADGE_ARTIFACT_TIMEOUT_ENV,
    RepoBadgeArtifactOptions, badge_artifact_command_args, badge_artifact_command_label,
    badge_artifact_jobs, badge_artifact_native_slot, badge_artifacts_impl_with_runners,
    badge_artifacts_summary_markdown, badge_basis_canonical_projection,
    badge_basis_derived_ripr_plus_snapshot, badge_basis_needs_repo_badge_plus_job,
    badge_basis_report_json, badge_basis_report_markdown, badge_basis_seam_native_counts,
    badge_endpoint_violation, badge_native_audit_snapshot, check_badge_diff_policy_with_context,
    compute_badge_endpoint_violations, copy_badge_endpoints_from_reports, error_ripr_plus_receipt,
    extract_json_object_usize_map, extract_json_string, extract_json_warnings,
    limited_badge_artifacts_json, limited_badge_artifacts_markdown,
    parse_repo_badge_artifact_options, parse_repo_exposure_summary_counts,
    read_repo_exposure_summary_artifact, repo_badge_artifact_command_args,
    repo_badge_artifact_jobs, repo_badge_artifact_stdout_from_output,
    repo_badge_artifact_timeout_ms_from_env, repo_badge_artifacts_summary_markdown,
    ripr_plus_receipt_from_badge, ripr_plus_receipt_from_options,
    ripr_plus_receipt_from_repo_badge_json, ripr_plus_receipt_from_repo_exposure_summary_json,
    ripr_plus_receipt_from_repo_exposure_summary_json_with_source, ripr_plus_receipt_markdown,
    run_repo_badge_artifact_command, validate_shields_endpoint_bytes,
    write_badge_artifacts_after_build, write_badge_artifacts_from_diff,
};
pub(crate) use reports::{
    FixtureCheckFormat, fixture_dirs, goldens_check, is_manifest_only_fixture_dir,
    normalize_fixture_human_output, normalize_fixture_json_output, ripr_fixture_binary,
    run_fixture_check, yes_no,
};
#[cfg(test)]
pub(crate) use reports::{
    GoldenDriftEntry, GoldenDriftSemantics, first_line_difference, fixture_cache_dir,
    fixture_contract_violations, golden_assistant_loop_health_contract_violations_at,
    golden_drift_semantics, golden_drift_type, goldens_check_failure_message,
    json_string_values_for_key, normalize_golden_text, parse_reason, run_fixture,
    run_fixture_outputs, validate_bless_reason,
};
#[cfg(test)]
pub(crate) use reports::{
    MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT, MutationOutcomeRecord,
    SarifMissingBaseline, SarifPolicyMode, SarifPolicyResult, SarifPolicyThreshold,
    build_mutation_calibration_report, build_sarif_policy_report, mutation_calibration_report_json,
    mutation_calibration_report_markdown, parse_mutation_calibration_args,
    parse_mutation_outcomes_json, parse_sarif_policy_args, parse_sarif_policy_results,
    read_mutation_input_json, sarif_policy_report_json, sarif_policy_report_markdown,
};
use reports::{
    check_badge_diff_policy, dogfood, fixtures, metrics_report, pr_summary, receipts_write,
    reports_index, test_oracle_report,
};
#[cfg(test)]
use reports::{lsp_cockpit_report, targeted_test_outcome};
pub(crate) use ripr_swarm::{
    RiprSwarmAttemptLedgerEntry, RiprSwarmReadinessInput, ripr_swarm,
    ripr_swarm_attempt_related_target_file, ripr_swarm_attempt_workspace_relative_file_token,
    ripr_swarm_plan_field_missing, ripr_swarm_plan_from_actionable_gaps_value,
    ripr_swarm_plan_packet_is_high_confidence, ripr_swarm_plan_packets_json,
    ripr_swarm_plan_related_target_file, ripr_swarm_plan_summary_json,
    ripr_swarm_push_static_limitation_backlog_markdown, ripr_swarm_readiness_from_values,
    ripr_swarm_readiness_json, ripr_swarm_route_quality_report,
};
#[cfg(test)]
pub(crate) use ripr_swarm::{
    RiprSwarmAttemptLedgerReport, RiprSwarmCommand, RiprSwarmReadinessNextActionSources,
    RiprSwarmRepairRouteQualityRow, parse_ripr_swarm_args, parse_ripr_swarm_plan_args,
    ripr_swarm_attempt_allowed_file_line, ripr_swarm_attempt_dry_run_from_actionable_gaps_value,
    ripr_swarm_attempt_dry_run_markdown, ripr_swarm_attempt_ledger_entries_from_value,
    ripr_swarm_attempt_ledger_from_values,
    ripr_swarm_attempt_ledger_from_values_with_real_repair_attempts,
    ripr_swarm_attempt_ledger_json, ripr_swarm_attempt_ledger_latest_attempts,
    ripr_swarm_attempt_ledger_markdown, ripr_swarm_attempt_ledger_repair_route_quality,
    ripr_swarm_plan_blocked_packets, ripr_swarm_plan_blocked_report,
    ripr_swarm_plan_blocked_state_examples_json, ripr_swarm_plan_json, ripr_swarm_plan_markdown,
    ripr_swarm_plan_ready_packets, ripr_swarm_read_optional_json,
    ripr_swarm_readiness_limited_runtime_command, ripr_swarm_readiness_markdown,
    ripr_swarm_readiness_next_actions, ripr_swarm_readiness_state, ripr_swarm_readiness_summary,
    ripr_swarm_readiness_top_limitation_routes, ripr_swarm_repair_route_quality_attempt_is_failure,
    ripr_swarm_repair_route_quality_failure_count, ripr_swarm_repair_route_quality_success_rate,
    ripr_swarm_route_quality_from_ledger_value, ripr_swarm_route_quality_report_json,
};
use run::{
    TimedFileOutput, TimedOutput, capture_output, capture_output_with_timeout,
    capture_stdout_to_file_with_timeout, command_success_owned, run, run_in_dir,
    run_in_dir_with_envs, run_output, run_output_optional, run_output_owned, run_owned,
    run_with_envs,
};

/// Process-wide fair reader-writer gate serialising tests that mutate the process
/// working directory. The xtask test suite spawns subprocesses (rustc, cargo,
/// git) that inherit the current cwd; meanwhile other tests (`with_temp_cwd`,
/// `with_repo_cwd`) call `std::env::set_current_dir` and delete the temporary
/// directory on teardown. Without serialisation, a subprocess spawn can land
/// in the tiny window between `set_current_dir(temp)` and
/// `remove_dir_all(temp)`, producing "Could not locate working directory" or
/// exit-status-1 failures in unrelated `run_output*` / `capture_output*`
/// tests.
///
/// Readers still run in parallel, while a waiting writer blocks new readers.
/// The standard library `RwLock` admitted enough new readers under Windows
/// parallel test load to starve the dispatch test's writer indefinitely (see
/// #2875), so this small gate makes writer admission explicit without
/// serialising the entire spawning-test lane. See issues #2044 and #2124.
#[cfg(test)]
struct CwdLockState {
    active_readers: usize,
    active_writer: bool,
    waiting_writers: usize,
}

#[cfg(test)]
pub(crate) struct CwdLock {
    state: std::sync::Mutex<CwdLockState>,
    available: std::sync::Condvar,
}

#[cfg(test)]
impl CwdLock {
    fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(CwdLockState {
                active_readers: 0,
                active_writer: false,
                waiting_writers: 0,
            }),
            available: std::sync::Condvar::new(),
        }
    }

    /// Acquire a shared reader slot.
    ///
    /// This gate is not reentrant: callers must not acquire another reader
    /// while holding a `CwdReadGuard`, because a queued writer would block the
    /// nested acquisition while the existing guard blocks that writer.
    fn read(&self) -> CwdReadGuard<'_> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while state.active_writer || state.waiting_writers > 0 {
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        state.active_readers += 1;
        CwdReadGuard { lock: self }
    }

    /// Acquire the exclusive writer slot.
    ///
    /// This gate is not reentrant: callers must not acquire a writer while
    /// holding either a `CwdReadGuard` or a `CwdWriteGuard`.
    fn write(&self) -> CwdWriteGuard<'_> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.waiting_writers += 1;
        while state.active_writer || state.active_readers > 0 {
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        state.waiting_writers = state.waiting_writers.saturating_sub(1);
        state.active_writer = true;
        CwdWriteGuard { lock: self }
    }
}

#[cfg(test)]
pub(crate) struct CwdReadGuard<'a> {
    lock: &'a CwdLock,
}

#[cfg(test)]
impl Drop for CwdReadGuard<'_> {
    fn drop(&mut self) {
        let active_readers = {
            let mut state = self
                .lock
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.active_readers = state.active_readers.saturating_sub(1);
            state.active_readers
        };
        if active_readers == 0 {
            self.lock.available.notify_all();
        }
    }
}

#[cfg(test)]
pub(crate) struct CwdWriteGuard<'a> {
    lock: &'a CwdLock,
}

#[cfg(test)]
impl Drop for CwdWriteGuard<'_> {
    fn drop(&mut self) {
        {
            let mut state = self
                .lock
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.active_writer = false;
        }
        self.lock.available.notify_all();
    }
}

#[cfg(test)]
pub(crate) static CWD_LOCK: std::sync::OnceLock<CwdLock> = std::sync::OnceLock::new();

/// Acquire the exclusive write guard for cwd-manipulating tests (those that
/// call `set_current_dir` or delete a temp cwd). Excludes every other cwd-
/// sensitive test — both writers and readers.
#[cfg(test)]
pub(crate) fn acquire_test_cwd_write_guard() -> CwdWriteGuard<'static> {
    CWD_LOCK.get_or_init(CwdLock::new).write()
}

/// Acquire the shared read guard for subprocess-spawning tests (those that
/// inherit the process cwd but do not change it). Multiple spawning tests can
/// hold the read guard concurrently; they are only excluded while a
/// cwd-manipulating test holds the write guard.
#[cfg(test)]
pub(crate) fn acquire_test_cwd_read_guard() -> CwdReadGuard<'static> {
    CWD_LOCK.get_or_init(CwdLock::new).read()
}

fn main() {
    let command = XtaskCommand::parse(std::env::args().skip(1));
    let result = dispatch::execute(command);
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn ci_fast() -> Result<(), String> {
    ci_fast_with_envs(&[])
}

fn ci_fast_with_envs(envs: &[(&str, &str)]) -> Result<(), String> {
    run_with_envs("cargo", &["fmt", "--check"], envs)?;
    run_with_envs("cargo", &["check", "--workspace", "--all-targets"], envs)?;
    run_with_envs("cargo", &["test", "--workspace"], envs)?;
    run_policy_checks()
}

/// The `cargo xtask` gate commands `precommit()` runs, in run order (the
/// leading `cargo fmt --check` is not an xtask command and stays out). The
/// command-catalog CI drift check expands an enforced `cargo xtask precommit`
/// workflow invocation into these gates (issue #2258), so the routed-rust
/// lanes can invoke the shared table once instead of enumerating each gate.
/// Keep aligned with `precommit()` and `precommit_report_body()`; the
/// `precommit_gate_commands_match_report` test pins the report to this list.
const PRECOMMIT_GATE_COMMANDS: &[&str] = &[
    "check-static-language",
    "check-no-panic-family",
    "check-allow-attributes",
    "check-local-context",
    "check-file-policy",
    "check-executable-files",
    "check-workflows",
    "check-droid-review-config",
    "check-spec-format",
    "check-spec-numbering",
    "check-fixture-contracts",
    "check-traceability",
    "check-capabilities",
    "check-workspace-shape",
    "check-architecture",
    "check-public-api",
    "check-output-contracts",
    "check-doc-artifacts",
    "check-doc-index",
    "check-readme-state",
    "markdown-links",
    "check-pr-shape",
    "check-command-catalog",
    "check-generated",
    "check-badge-diff-policy",
    "check-generated-clean",
    "check-proof-packs",
    "check-release-targets",
    "check-dependencies",
    "check-process-policy",
    "check-network-policy",
    "check-lint-policy",
];

fn precommit() -> Result<(), String> {
    ensure_reports_dir()?;
    run("cargo", &["fmt", "--check"])?;
    check_static_language()?;
    check_no_panic_family()?;
    check_allow_attributes()?;
    check_local_context()?;
    check_file_policy()?;
    check_executable_files()?;
    check_workflows()?;
    check_droid_review_config()?;
    check_spec_format()?;
    check_spec_numbering()?;
    check_fixture_contracts()?;
    check_traceability()?;
    check_capabilities()?;
    check_workspace_shape()?;
    check_architecture()?;
    check_public_api()?;
    check_output_contracts()?;
    check_doc_artifacts()?;
    check_doc_index()?;
    check_readme_state()?;
    markdown_links()?;
    check_pr_shape()?;
    check_command_catalog()?;
    check_generated()?;
    check_badge_diff_policy()?;
    check_generated_clean()?;
    check_proof_packs()?;
    check_release_targets()?;
    check_dependencies()?;
    check_process_policy()?;
    check_network_policy()?;
    check_lint_policy()?;
    let body = precommit_report_body();
    write_report("precommit.md", &body)
}

/// Diff-aware fast gate runner (#2343). Runs only the gates relevant to
/// changed files, plus a cheap always-run floor. Target: sub-30s for
/// doc-only changes, ~2min for Rust source changes.
pub(crate) fn check_fast() -> Result<(), String> {
    ensure_reports_dir()?;
    let changed = changed_files_vs_origin_main().unwrap_or_default();
    let categories = categorize_changed_files(&changed);
    let mut ran = Vec::new();
    let mut skipped = Vec::new();

    // Always run: these are cheap and catch cross-cutting drift.
    run("cargo", &["fmt", "--check"])?;
    ran.push("fmt --check");
    check_static_language()?;
    ran.push("check-static-language");
    check_command_catalog()?;
    ran.push("check-command-catalog");

    // Conditional gates based on changed file categories.
    if categories.rust_src {
        check_no_panic_family()?;
        ran.push("check-no-panic-family");
        check_allow_attributes()?;
        ran.push("check-allow-attributes");
        check_file_policy()?;
        ran.push("check-file-policy");
        run(
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        )?;
        ran.push("clippy");
    } else {
        skipped.extend_from_slice(&[
            "check-no-panic-family",
            "check-allow-attributes",
            "check-file-policy",
            "clippy",
        ]);
    }

    if categories.workflow {
        check_workflows()?;
        ran.push("check-workflows");
    }

    if categories.policy {
        check_process_policy()?;
        ran.push("check-process-policy");
        check_network_policy()?;
        ran.push("check-network-policy");
    }

    if categories.fixture {
        check_fixture_contracts()?;
        ran.push("check-fixture-contracts");
    } else {
        skipped.push("check-fixture-contracts");
    }

    // Always run these (cheap, cross-cutting).
    check_generated()?;
    ran.push("check-generated");
    check_generated_clean()?;
    ran.push("check-generated-clean");
    check_lint_policy()?;
    ran.push("check-lint-policy");

    eprintln!(
        "check-fast: ran {} gate(s){}, base=origin/main, {} file(s) changed",
        ran.len(),
        if skipped.is_empty() {
            String::new()
        } else {
            format!(", skipped {} ({})", skipped.len(), skipped.join(", "))
        },
        changed.len()
    );

    let body = format!(
        "# check-fast report\n\nStatus: pass\n\nRan:\n{}\n\nSkipped:\n{}\n",
        ran.iter()
            .map(|g| format!("- {g}"))
            .collect::<Vec<_>>()
            .join("\n"),
        if skipped.is_empty() {
            "- none".to_string()
        } else {
            skipped
                .iter()
                .map(|g| format!("- {g}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    );
    write_report("check-fast.md", &body)
}

struct ChangedFileCategories {
    rust_src: bool,
    workflow: bool,
    policy: bool,
    fixture: bool,
}

fn categorize_changed_files(files: &[String]) -> ChangedFileCategories {
    ChangedFileCategories {
        rust_src: files.iter().any(|f| {
            f.ends_with(".rs") && (f.contains("crates/ripr/src") || f.contains("xtask/src"))
        }),
        workflow: files.iter().any(|f| f.starts_with(".github/workflows/")),
        policy: files
            .iter()
            .any(|f| f.starts_with("policy/") || f.starts_with(".ripr/")),
        fixture: files.iter().any(|f| f.starts_with("fixtures/")),
    }
}

fn changed_files_vs_origin_main() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", "origin/main...HEAD"])
        .output()
        .map_err(|err| format!("git diff --name-only failed: {err}"))?;
    if !output.status.success() {
        return Err(
            "git diff --name-only origin/main...HEAD failed; if origin/main is not available, run `git fetch origin main` first".to_string(),
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().map(String::from).collect())
}

fn check_pr() -> Result<(), String> {
    ensure_reports_dir()?;
    let temp_env = check_pr_temp_env()?;
    let temp_env_refs = temp_env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    // #3036: gates are lazy closures evaluated strictly in order — the first
    // failure stops the sequence, so later gates neither spend work nor emit
    // side effects after the outcome is already decided.
    let gates = [
        CheckPrGate {
            name: "ci-fast",
            reproduce: "cargo xtask ci-fast",
            run: &|| ci_fast_with_envs(&temp_env_refs),
        },
        CheckPrGate {
            name: "clippy",
            reproduce: "cargo clippy --workspace --all-targets -- -D warnings",
            run: &|| {
                run_with_envs(
                    "cargo",
                    &[
                        "clippy",
                        "--workspace",
                        "--all-targets",
                        "--",
                        "-D",
                        "warnings",
                    ],
                    &temp_env_refs,
                )
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(format!("clippy exited with {status}"))
                    }
                })
            },
        },
        CheckPrGate {
            name: "doc",
            reproduce: "cargo doc --workspace --no-deps",
            run: &|| {
                run_with_envs(
                    "cargo",
                    &["doc", "--workspace", "--no-deps"],
                    &temp_env_refs,
                )
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(format!("doc exited with {status}"))
                    }
                })
            },
        },
    ];
    run_check_pr_gates(&gates, &|failure| {
        write_report("check-pr.md", &check_pr_report(Some(failure)))
    })?;
    // If all gates passed, fall through to the original success path.
    pr_summary()?;
    write_report("check-pr.md", &check_pr_report(None))?;
    suggested_fixes()?;
    receipts_write()?;
    pr_summary()?;
    reports_index()
}

/// One check-pr sub-gate: a name, the command that reproduces just that gate,
/// and the lazy closure that runs it (#3036).
struct CheckPrGate<'a> {
    name: &'a str,
    reproduce: &'a str,
    run: &'a dyn Fn() -> Result<(), String>,
}

/// The first failed gate plus the bounded evidence the failure report
/// retains (#3036): the reproduce command, up to five CRLF-normalized error
/// lines (with an explicit truncation marker when more were dropped), and
/// the gates that were never invoked.
struct CheckPrGateFailure {
    name: String,
    reproduce: String,
    bounded_error: String,
    not_run: Vec<String>,
}

/// Run the gates in order and stop at the first failure (#3036). On failure,
/// the current failure report is published via `write_failure_report` before
/// the labeled gate error is returned; a report-publication failure is
/// returned as a distinct error that still carries the original gate
/// diagnostic and reproduce command, so "gate failed" and "report failed"
/// are never conflated and no failure evidence is discarded precisely when
/// it is most needed.
fn run_check_pr_gates(
    gates: &[CheckPrGate],
    write_failure_report: &dyn Fn(&CheckPrGateFailure) -> Result<(), String>,
) -> Result<(), String> {
    for (index, gate) in gates.iter().enumerate() {
        if let Err(err) = (gate.run)() {
            let failure = CheckPrGateFailure {
                name: gate.name.to_string(),
                reproduce: gate.reproduce.to_string(),
                bounded_error: bound_first_failure_lines(&err),
                not_run: gates[index + 1..]
                    .iter()
                    .map(|later| later.name.to_string())
                    .collect(),
            };
            write_failure_report(&failure).map_err(|write_err| {
                format!(
                    "check-pr gate `{}` failed, and publishing the failure report also failed: {write_err}\nreproduce: {}\n{err}",
                    gate.name, gate.reproduce
                )
            })?;
            return label_check_pr_gate(gate.name, gate.reproduce, Err(err));
        }
    }
    Ok(())
}

/// Bound the retained first-failure evidence to five lines with CRLF
/// normalized away (#3036). An empty error stays empty; the report composer
/// is responsible for making that state explicit. When lines are dropped, an
/// explicit truncation marker records how many, so bounded evidence never
/// presents itself as the whole diagnostic.
fn bound_first_failure_lines(err: &str) -> String {
    let normalized = err.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut kept: Vec<String> = lines
        .iter()
        .take(5)
        .map(|line| (*line).to_string())
        .collect();
    if lines.len() > 5 {
        kept.push(format!("(+{} more lines truncated)", lines.len() - 5));
    }
    kept.join("\n  ")
}

/// Attribute a `check-pr` sub-gate failure to the gate that produced it, with the
/// command that reproduces *just* that gate and a pointer to the reports. Without
/// this, a clippy or doc failure surfaces a bare tool error with no indication it
/// came from `check-pr` or how to re-run only the failing step.
fn label_check_pr_gate<T>(
    name: &str,
    reproduce: &str,
    result: Result<T, String>,
) -> Result<T, String> {
    result.map_err(|err| {
        format!(
            "check-pr gate `{name}` failed\nreproduce: {reproduce}\nreports: target/ripr/reports/check-pr.md\n{err}"
        )
    })
}

fn check_pr_temp_env() -> Result<Vec<(String, String)>, String> {
    let temp_dir = std::env::current_dir()
        .map_err(|err| format!("failed to resolve current directory for check-pr temp dir: {err}"))?
        .join("target")
        .join("tmp")
        .join("check-pr");
    fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "failed to create check-pr temp dir {}: {err}",
            temp_dir.display()
        )
    })?;
    let temp_dir = temp_dir.to_string_lossy().into_owned();
    Ok(vec![
        ("TEMP".to_string(), temp_dir.clone()),
        ("TMP".to_string(), temp_dir.clone()),
        ("TMPDIR".to_string(), temp_dir),
    ])
}

fn run_policy_checks() -> Result<(), String> {
    check_static_language()?;
    check_no_panic_family()?;
    check_allow_attributes()?;
    check_local_context()?;
    check_file_policy()?;
    check_executable_files()?;
    check_workflows()?;
    check_droid_review_config()?;
    check_spec_format()?;
    check_spec_numbering()?;
    check_fixture_contracts()?;
    check_traceability()?;
    check_capabilities()?;
    check_workspace_shape()?;
    check_architecture()?;
    check_public_api()?;
    check_output_contracts()?;
    check_doc_artifacts()?;
    check_doc_index()?;
    check_readme_state()?;
    markdown_links()?;
    check_pr_shape()?;
    check_command_catalog()?;
    check_generated()?;
    check_badge_diff_policy()?;
    check_generated_clean()?;
    check_proof_packs()?;
    check_release_targets()?;
    check_dependencies()?;
    check_process_policy()?;
    check_network_policy()?;
    check_lint_policy()
}

fn ci_full() -> Result<(), String> {
    check_pr()?;
    run_ci_full_evidence_gates(&ci_full_evidence_gates())?;
    run("cargo", &["package", "-p", "ripr", "--list"])?;
    run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
}

#[derive(Debug, Eq, PartialEq)]
struct CwdCommand {
    program: PathBuf,
    args: Vec<String>,
    cwd: PathBuf,
}

fn run_cwd_command(command: &CwdCommand) -> Result<(), String> {
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    run_in_dir(&command.program, &args, &command.cwd).map(|_| ())
}

fn vscode_compile() -> Result<(), String> {
    run_cwd_command(&vscode_compile_command())
}

fn vscode_package() -> Result<(), String> {
    let extension_dir = vscode_extension_dir();
    let dist = extension_dir.join("dist");
    fs::create_dir_all(&dist)
        .map_err(|err| format!("failed to create {}: {err}", dist.display()))?;
    let version = vscode_package_version(&extension_dir.join("package.json"))?;
    run_cwd_command(&vscode_package_command(&version))
}

fn vscode_test() -> Result<(), String> {
    // #2437: npm test must actually run tests, not just compile.
    // Delegate to the e2e runner which builds ripr, compiles TS, and
    // invokes the mocha/test-electron suite.
    vscode_test_e2e()
}

fn vscode_test_e2e() -> Result<(), String> {
    let provided_server = std::env::var_os("RIPR_TEST_SERVER_PATH")
        .and_then(|value| (!value.is_empty()).then_some(value))
        .map(PathBuf::from);
    let (server_path, build_server) =
        select_vscode_test_server(provided_server.as_deref(), &vscode_test_server_path()?)?;
    if build_server {
        run("cargo", &["build", "-p", "ripr"])?;
    }
    vscode_compile()?;
    let workspace_path = vscode_test_workspace_path()?;
    let mut envs = vec![(
        "RIPR_TEST_WORKSPACE_PATH",
        path_to_utf8(&workspace_path, "VS Code test workspace path")?,
    )];
    if provided_server.is_none() {
        envs.push((
            "RIPR_TEST_SERVER_PATH",
            path_to_utf8(&server_path, "VS Code test server path")?,
        ));
    }
    run_cwd_command_with_envs(&vscode_test_e2e_command(), &envs)
}

fn select_vscode_test_server(
    provided: Option<&Path>,
    built: &Path,
) -> Result<(PathBuf, bool), String> {
    if let Some(path) = provided {
        if !path.is_file() {
            return Err(format!(
                "RIPR_TEST_SERVER_PATH does not name an installed server binary: {}",
                path.display()
            ));
        }
        return Ok((path.to_path_buf(), false));
    }
    Ok((built.to_path_buf(), true))
}

fn vscode_compile_command() -> CwdCommand {
    CwdCommand {
        program: vscode_local_bin("tsc"),
        args: vec!["-p".to_string(), "./".to_string()],
        cwd: vscode_extension_dir(),
    }
}

fn vscode_package_command(version: &str) -> CwdCommand {
    CwdCommand {
        program: vscode_local_bin("vsce"),
        args: vec![
            "package".to_string(),
            "--out".to_string(),
            format!("dist/ripr-{version}.vsix"),
        ],
        cwd: vscode_extension_dir(),
    }
}

fn vscode_test_e2e_command() -> CwdCommand {
    CwdCommand {
        program: PathBuf::from("node"),
        args: vec!["out/test/runTest.js".to_string()],
        cwd: vscode_extension_dir(),
    }
}

fn run_cwd_command_with_envs(command: &CwdCommand, envs: &[(&str, &str)]) -> Result<(), String> {
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    run_in_dir_with_envs(&command.program, &args, &command.cwd, envs).map(|_| ())
}

fn vscode_test_server_path() -> Result<PathBuf, String> {
    let binary = if cfg!(windows) { "ripr.exe" } else { "ripr" };
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            repo_root()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("target")
        });
    Ok(target_dir.join("debug").join(binary))
}

fn vscode_test_workspace_path() -> Result<PathBuf, String> {
    Ok(repo_root()?
        .join("fixtures")
        .join("boundary_gap")
        .join("input"))
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

fn path_to_utf8<'a>(path: &'a Path, label: &str) -> Result<&'a str, String> {
    path.to_str()
        .ok_or_else(|| format!("{label} is not valid UTF-8: {}", path.display()))
}

fn vscode_extension_dir() -> PathBuf {
    repo_root()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("editors/vscode")
}

fn vscode_local_bin(name: &str) -> PathBuf {
    let executable = if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_string()
    };
    vscode_extension_dir()
        .join("node_modules")
        .join(".bin")
        .join(executable)
}

fn vscode_package_version(package_json: &Path) -> Result<String, String> {
    let value = read_json_value(package_json)?;
    value
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{} is missing a string version", package_json.display()))
}

fn ci_full_evidence_gates() -> [CiFullEvidenceGate; 5] {
    [
        CiFullEvidenceGate {
            name: "fixtures",
            run: ci_full_fixtures,
        },
        CiFullEvidenceGate {
            name: "goldens check",
            run: goldens_check,
        },
        CiFullEvidenceGate {
            name: "test-oracle-report",
            run: test_oracle_report,
        },
        CiFullEvidenceGate {
            name: "dogfood",
            run: dogfood,
        },
        CiFullEvidenceGate {
            name: "metrics",
            run: metrics_report,
        },
    ]
}

fn ci_full_fixtures() -> Result<(), String> {
    fixtures(None)
}

fn run_ci_full_evidence_gates(gates: &[CiFullEvidenceGate]) -> Result<(), String> {
    for gate in gates {
        (gate.run)()
            .map_err(|err| format!("ci-full evidence gate `{}` failed: {err}", gate.name))?;
    }
    Ok(())
}

fn install_hooks(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("install-hooks does not accept arguments".to_string());
    }

    let hook = install_hooks_in(Path::new("."))?;
    eprintln!("installed hook: {}", hook.display());
    Ok(())
}

fn install_hooks_in(root: &Path) -> Result<PathBuf, String> {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return Err(format!(
            "missing .git directory under {}; run from a git worktree",
            root.display()
        ));
    }

    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|err| format!("failed to create {}: {err}", hooks_dir.display()))?;

    let hook = hooks_dir.join("pre-commit");
    if hook.exists() {
        let current = fs::read_to_string(&hook)
            .map_err(|err| format!("failed to read {}: {err}", hook.display()))?;
        if !is_ripr_managed_hook(&current) {
            return Err(format!(
                "refusing to overwrite unmanaged hook at {}; remove it or install the ripr precommit hook manually",
                hook.display()
            ));
        }
    }

    fs::write(&hook, ripr_pre_commit_hook())
        .map_err(|err| format!("failed to write {}: {err}", hook.display()))?;
    make_hook_executable(&hook)?;
    Ok(hook)
}

fn ripr_pre_commit_hook() -> String {
    "#!/usr/bin/env sh\n# ripr-managed pre-commit hook\nset -eu\ncargo xtask precommit\n"
        .to_string()
}

fn is_ripr_managed_hook(text: &str) -> bool {
    text.contains("# ripr-managed pre-commit hook")
}

#[cfg(unix)]
fn make_hook_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|err| format!("failed to read {} metadata: {err}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|err| format!("failed to set executable bit on {}: {err}", path.display()))
}

#[cfg(not(unix))]
fn make_hook_executable(path: &Path) -> Result<(), String> {
    // Windows does not use the POSIX exec bit, so the hook runs there
    // regardless — but a checkout later used from WSL or Linux would
    // silently skip it (#2091). Say so at install time instead of
    // no-oping quietly.
    // Display the POSIX form: backslashes from a Windows display path are
    // escapes in a WSL/Linux shell, so the printed remedy would target the
    // wrong file (#2177 review).
    let posix_path = path.display().to_string().replace('\\', "/");
    eprintln!(
        "install-hooks: {} was written without an executable bit (unused on this platform); \
         if this checkout is later used from WSL or Linux, run `chmod +x {}`",
        posix_path, posix_path
    );
    Ok(())
}

fn shape() -> Result<(), String> {
    ensure_reports_dir()?;
    run("cargo", &["fmt"])?;
    let sorted = sort_allowlist_files()?;
    // Surface the tracked-file rewrites before the contributor commits: a
    // silent `policy/*.txt` / `.ripr/*.txt` rewrite is a surprising diff in
    // a PR that never touched policy (#2088).
    if let Some(notice) = shape_rewrite_notice(&sorted) {
        eprintln!("{notice}");
    }
    let body = shape_report_body(&sorted);
    write_report("shape.md", &body)
}

/// The stderr notice listing tracked allowlist files `shape` rewrote, or
/// `None` when nothing changed (#2088).
fn shape_rewrite_notice(sorted: &[String]) -> Option<String> {
    if sorted.is_empty() {
        return None;
    }
    let mut notice = format!(
        "shape: rewrote {} tracked allowlist file(s) (review before `git add`):",
        sorted.len()
    );
    for path in sorted {
        notice.push_str(&format!("\n  - {path}"));
    }
    Some(notice)
}

fn fix_pr() -> Result<(), String> {
    shape()?;
    pr_summary()?;
    let body = "# ripr fix-pr report\n\nStatus: pass\n\nActions:\n\n- Ran `cargo xtask shape`.\n- Ran `cargo xtask pr-summary`.\n\nReports:\n\n- `target/ripr/reports/shape.md`\n- `target/ripr/reports/pr-summary.md`\n\nNext commands:\n\n```bash\ncargo xtask check-pr\n```\n";
    write_report("fix-pr.md", body)
}

fn commands_report() -> Result<(), String> {
    let entries = command_catalog();
    write_report("commands.md", &commands_report_markdown(&entries))?;
    write_report("commands.json", &commands_report_json(&entries))
}

fn pr_ready() -> Result<(), String> {
    let steps = vec![
        run_readiness_step(
            "worktree_doctor",
            "cargo xtask worktree doctor",
            "target/ripr/reports/worktree-doctor.md",
            true,
            worktree_doctor,
        ),
        run_readiness_step(
            "command_mutability_catalog",
            "cargo xtask commands",
            "target/ripr/reports/commands.md",
            false,
            commands_report,
        ),
        run_readiness_step(
            "pr_summary",
            "cargo xtask pr-summary",
            "target/ripr/reports/pr-summary.md",
            false,
            pr_summary,
        ),
        run_readiness_step(
            "critic",
            "cargo xtask critic",
            "target/ripr/reports/critic.md",
            false,
            critic_impl,
        ),
        run_readiness_step(
            "receipts_check",
            "cargo xtask receipts check",
            "target/ripr/reports/receipts.md",
            false,
            receipts_check,
        ),
        run_readiness_step(
            "suggested_fixes",
            "cargo xtask suggested-fixes",
            "target/ripr/reports/suggested-fixes.md",
            false,
            suggested_fixes,
        ),
        run_readiness_step(
            "generated_clean",
            "cargo xtask check-generated-clean",
            "target/ripr/reports/generated-clean.md",
            true,
            check_generated_clean,
        ),
        run_readiness_step(
            "badge_diff_policy",
            "cargo xtask check-badge-diff-policy",
            "target/ripr/reports/badge-diff-policy.md",
            true,
            check_badge_diff_policy,
        ),
    ];
    let status = pr_ready_status(&steps);
    write_report("pr-ready.md", &pr_ready_markdown(&steps))?;
    write_report("pr-ready.json", &pr_ready_json(&steps))?;
    let index_result = reports_index();

    if status == "fail" {
        let _ = index_result;
        Err(
            "pr-ready found blocking repo-ops issues; see target/ripr/reports/pr-ready.md"
                .to_string(),
        )
    } else {
        index_result
    }
}

fn cockpit() -> Result<(), String> {
    let steps = vec![
        run_readiness_step(
            "worktree_doctor",
            "cargo xtask worktree doctor",
            "target/ripr/reports/worktree-doctor.md",
            true,
            worktree_doctor,
        ),
        run_readiness_step(
            "command_mutability_catalog",
            "cargo xtask commands",
            "target/ripr/reports/commands.md",
            false,
            commands_report,
        ),
        run_readiness_step(
            "command_catalog_check",
            "cargo xtask check-command-catalog",
            "target/ripr/reports/command-catalog.md",
            true,
            check_command_catalog,
        ),
        run_readiness_step(
            "spec_numbering",
            "cargo xtask check-spec-numbering",
            "target/ripr/reports/spec-numbering.md",
            true,
            check_spec_numbering,
        ),
        run_readiness_step(
            "pr_triage",
            "cargo xtask pr-triage-report",
            "target/ripr/reports/pr-triage.md",
            false,
            reports::pr_triage_report,
        ),
        run_readiness_step(
            "generated_clean",
            "cargo xtask check-generated-clean",
            "target/ripr/reports/generated-clean.md",
            true,
            check_generated_clean,
        ),
        run_readiness_step(
            "badge_diff_policy",
            "cargo xtask check-badge-diff-policy",
            "target/ripr/reports/badge-diff-policy.md",
            true,
            check_badge_diff_policy,
        ),
    ];

    write_report("cockpit.md", &cockpit_markdown(&steps))?;
    write_report("cockpit.json", &cockpit_json(&steps))?;
    reports_index()?;

    if steps
        .iter()
        .any(|step| step.required && step.status == "fail")
    {
        Err("repo cockpit found blocking repo-ops issues".to_string())
    } else {
        Ok(())
    }
}

fn check_command_catalog() -> Result<(), String> {
    let commands = known_commands();
    let catalog = command_catalog();
    let mut violations = command_catalog_violations(&commands, &catalog);
    violations.extend(command_catalog_ci_drift_violations_for_repo(&catalog)?);

    finish_policy_report(
        PolicyReportSpec {
            report_file: "command-catalog.md",
            check: "check-command-catalog",
            why_it_matters: "The command mutability catalog is the repo-ops map for agents. Every xtask command must stay classified so workers know what is safe to run, what writes generated evidence, what requires judgment, and which checks a CI workflow enforces.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Add a command catalog entry for every new xtask command.",
                "Remove catalog entries for commands that no longer exist.",
                "Document writes for mutating, external-state, and argument-dependent commands.",
                "Mark external-state mutations as judgment-required.",
                "Keep ci_enforced aligned with the commands CI workflows invoke without advisory shielding.",
            ],
            rerun_command: "cargo xtask check-command-catalog",
            exception_template: None,
        },
        &violations,
    )
}

fn command_catalog_violations(
    commands: &[&'static str],
    catalog: &[CommandCatalogEntry],
) -> Vec<String> {
    let known_roots = command_roots(commands.iter().copied());
    let catalog_roots = command_roots(catalog.iter().map(|entry| entry.command));
    let mut violations = Vec::new();

    for root in known_roots.difference(&catalog_roots) {
        violations.push(format!(
            "command `{root}` is listed in help but missing from the command mutability catalog"
        ));
    }
    for root in catalog_roots.difference(&known_roots) {
        violations.push(format!(
            "command catalog entry `{root}` does not match any known xtask command"
        ));
    }
    if let Some(order_violation) = command_catalog_order_violation(commands, catalog) {
        violations.push(order_violation);
    }

    let mut seen = BTreeSet::<&str>::new();
    for entry in catalog {
        if !seen.insert(entry.command) {
            violations.push(format!(
                "command catalog has duplicate entry `{}`",
                entry.command
            ));
        }
        if !is_command_mutability(entry.mutability) {
            violations.push(format!(
                "command `{}` uses unknown mutability `{}`",
                entry.command, entry.mutability
            ));
        }
        if entry.writes.trim().is_empty() {
            violations.push(format!("command `{}` must document writes", entry.command));
        }
        if entry.mutability == "external_state_mutating" && !entry.judgment_required {
            violations.push(format!(
                "external-state mutating command `{}` must be judgment-required",
                entry.command
            ));
        }
        if entry.mutability == "argument_dependent" {
            let notes = entry.notes.to_ascii_lowercase();
            if !(notes.contains("depending")
                || notes.contains("--check")
                || notes.contains("--propose")
                || notes.contains("default"))
            {
                violations.push(format!(
                    "argument-dependent command `{}` must explain when it writes",
                    entry.command
                ));
            }
        }
    }

    violations
}

fn command_catalog_order_violation(
    commands: &[&'static str],
    catalog: &[CommandCatalogEntry],
) -> Option<String> {
    let mut order = BTreeMap::new();
    for (index, command) in commands.iter().enumerate() {
        order.insert(*command, index);
    }

    let mut last = None;
    for entry in catalog {
        let Some(index) = order.get(entry.command).copied() else {
            continue;
        };
        if last.is_some_and(|last| index < last) {
            return Some(
                "command catalog entries must follow the xtask help catalog order".to_string(),
            );
        }
        last = Some(index);
    }
    None
}

fn command_roots<'a>(commands: impl Iterator<Item = &'a str>) -> BTreeSet<&'a str> {
    commands.map(known_command_root).collect()
}

fn is_command_mutability(value: &str) -> bool {
    matches!(
        value,
        "mutating"
            | "non_mutating_check"
            | "report_only"
            | "external_state_read"
            | "external_state_mutating"
            | "argument_dependent"
    )
}

/// One `cargo xtask` invocation extracted from a workflow run block:
/// the command root and its bare subcommand (empty when none).
type WorkflowXtaskInvocation = (String, String);

/// Scans one workflow file and returns the xtask invocations CI enforces:
/// run-block commands without `|| true` shielding and outside any job or step
/// marked `continue-on-error: true`. Line-scanning on purpose: check-workflows
/// reads workflows the same way and the repo does not take a YAML dependency.
fn ci_enforced_xtask_invocations(workflow: &str) -> BTreeSet<WorkflowXtaskInvocation> {
    let mut enforced = BTreeSet::new();
    let mut job_continue_on_error = false;
    let mut step_continue_on_error = false;
    let mut pending: Vec<WorkflowXtaskInvocation> = Vec::new();
    let mut run_block_indent: Option<usize> = None;

    for line in workflow.lines() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if let Some(run_indent) = run_block_indent {
            if trimmed.is_empty() || indent > run_indent {
                if let Some(invocation) = workflow_run_xtask_invocation(trimmed) {
                    pending.push(invocation);
                }
                continue;
            }
            run_block_indent = None;
        }

        if indent == 2 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            flush_workflow_step_invocations(
                &mut pending,
                &mut enforced,
                job_continue_on_error || step_continue_on_error,
            );
            job_continue_on_error = false;
            step_continue_on_error = false;
            continue;
        }
        if indent == 4 && trimmed == "continue-on-error: true" {
            job_continue_on_error = true;
            continue;
        }
        if indent == 6 && trimmed.starts_with("- ") {
            flush_workflow_step_invocations(
                &mut pending,
                &mut enforced,
                job_continue_on_error || step_continue_on_error,
            );
            step_continue_on_error = false;
            if let Some(inline) = trimmed.strip_prefix("- run:") {
                let inline = inline.trim();
                if inline.starts_with('|') || inline.starts_with('>') {
                    run_block_indent = Some(indent);
                } else if let Some(invocation) = workflow_run_xtask_invocation(inline) {
                    pending.push(invocation);
                }
            }
            continue;
        }
        if indent == 8 {
            if trimmed == "continue-on-error: true" {
                step_continue_on_error = true;
                continue;
            }
            if let Some(inline) = trimmed.strip_prefix("run:") {
                let inline = inline.trim();
                if inline.starts_with('|') || inline.starts_with('>') {
                    run_block_indent = Some(indent);
                } else if let Some(invocation) = workflow_run_xtask_invocation(inline) {
                    pending.push(invocation);
                }
            }
        }
    }
    flush_workflow_step_invocations(
        &mut pending,
        &mut enforced,
        job_continue_on_error || step_continue_on_error,
    );
    enforced
}

/// Moves the current step's invocations into the enforced set unless the
/// enclosing job or step is advisory (`continue-on-error: true`).
fn flush_workflow_step_invocations(
    pending: &mut Vec<WorkflowXtaskInvocation>,
    enforced: &mut BTreeSet<WorkflowXtaskInvocation>,
    advisory: bool,
) {
    if !advisory {
        enforced.extend(pending.drain(..));
    } else {
        pending.clear();
    }
}

/// Extracts the xtask command invoked by one run-block line, if the line
/// invokes `cargo xtask` directly. Advisory `|| true` shielding is handled by
/// the caller's step scan, so this only parses the command shape.
fn workflow_run_xtask_invocation(line: &str) -> Option<WorkflowXtaskInvocation> {
    if line.contains("|| true") {
        return None;
    }
    let rest = line.strip_prefix("cargo xtask ")?;
    let mut tokens = rest.split_whitespace();
    let root = tokens.next()?.trim_matches('"');
    if root.is_empty() || root.starts_with('$') {
        return None;
    }
    let subcommand = tokens
        .next()
        .filter(|token| {
            token.starts_with(|c: char| c.is_ascii_lowercase())
                && token.chars().all(|c| c.is_ascii_lowercase() || c == '-')
        })
        .unwrap_or_default();
    Some((root.to_string(), subcommand.to_string()))
}

fn catalog_command_matches_ci_invocation(
    command: &str,
    (root, subcommand): &WorkflowXtaskInvocation,
) -> bool {
    let mut words = command.split_whitespace();
    if words.next() != Some(root.as_str()) {
        return false;
    }
    subcommand.is_empty() || words.next() == Some(subcommand.as_str())
}

fn command_catalog_ci_drift_violations(
    catalog: &[CommandCatalogEntry],
    enforced: &BTreeSet<WorkflowXtaskInvocation>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for entry in catalog {
        let expected = enforced
            .iter()
            .any(|invocation| catalog_command_matches_ci_invocation(entry.command, invocation));
        if expected && !entry.ci_enforced {
            violations.push(format!(
                "command `{}` is invoked by a CI workflow without advisory shielding but the catalog marks ci_enforced=false",
                entry.command
            ));
        }
        if !expected && entry.ci_enforced {
            violations.push(format!(
                "command `{}` is marked ci_enforced=true but no CI workflow invokes it in an enforced lane",
                entry.command
            ));
        }
    }
    violations
}

fn command_catalog_ci_drift_violations_for_repo(
    catalog: &[CommandCatalogEntry],
) -> Result<Vec<String>, String> {
    let mut enforced = BTreeSet::new();
    for path in collect_files(Path::new(".github/workflows"))? {
        let normalized = normalize_path(&path);
        if !(normalized.ends_with(".yml") || normalized.ends_with(".yaml")) {
            continue;
        }
        let text = read_text_lossy(&path)?;
        enforced.extend(ci_enforced_xtask_invocations(&text));
    }
    expand_precommit_ci_invocations(&mut enforced);
    Ok(command_catalog_ci_drift_violations(catalog, &enforced))
}

/// An enforced `cargo xtask precommit` invocation transitively enforces every
/// gate in the precommit table (issue #2258): expand it so gates invoked only
/// through precommit still count as CI-enforced. Advisory precommit
/// invocations never reach the enforced set, so they expand nothing.
fn expand_precommit_ci_invocations(enforced: &mut BTreeSet<WorkflowXtaskInvocation>) {
    if !enforced.contains(&("precommit".to_string(), String::new())) {
        return;
    }
    for gate in PRECOMMIT_GATE_COMMANDS {
        enforced.insert(((*gate).to_string(), String::new()));
    }
}

fn commands_report_markdown(entries: &[CommandCatalogEntry]) -> String {
    let mut body = "# ripr command mutability catalog\n\n".to_string();
    body.push_str("Status: pass\n");
    body.push_str("Mode: advisory\n\n");
    body.push_str("Purpose:\n\n");
    body.push_str(
        "- distinguish commands that may edit the worktree from checks and generated reports\n",
    );
    body.push_str("- keep generated evidence separate from authored source-of-truth\n");
    body.push_str("- make judgment-required operations visible before agents run them\n");
    body.push_str(
        "- mark commands a CI workflow enforces so load-bearing gates stay distinct from advisory checks\n\n",
    );
    body.push_str("Boundaries:\n\n");
    body.push_str("- `check-pr` is non-mutating for tracked files\n");
    body.push_str("- `target/ripr/**` outputs are generated evidence\n");
    body.push_str("- judgment-required commands need explicit review before use\n");
    body.push_str(
        "- CI enforced means a CI workflow fails when the command fails; unmarked commands are advisory or local-only\n\n",
    );
    body.push_str("| Command | Mutability | Writes | Judgment required | CI enforced | Notes |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for entry in entries {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} |\n",
            markdown_cell(entry.command),
            markdown_cell(entry.mutability),
            markdown_cell(entry.writes),
            if entry.judgment_required { "yes" } else { "no" },
            if entry.ci_enforced { "yes" } else { "no" },
            markdown_cell(entry.notes)
        ));
    }
    body
}

fn commands_report_json(entries: &[CommandCatalogEntry]) -> String {
    let mut body = "{\n".to_string();
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str("  \"commands\": [\n");
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"command\": \"{}\",\n",
            json_escape(entry.command)
        ));
        body.push_str(&format!(
            "      \"mutability\": \"{}\",\n",
            json_escape(entry.mutability)
        ));
        body.push_str(&format!(
            "      \"writes\": \"{}\",\n",
            json_escape(entry.writes)
        ));
        body.push_str(&format!(
            "      \"judgment_required\": {},\n",
            entry.judgment_required
        ));
        body.push_str(&format!("      \"ci_enforced\": {},\n", entry.ci_enforced));
        body.push_str(&format!(
            "      \"notes\": \"{}\"\n",
            json_escape(entry.notes)
        ));
        body.push_str("    }");
    }
    body.push_str("\n  ]\n");
    body.push_str("}\n");
    body
}

fn suggested_fixes() -> Result<(), String> {
    ensure_reports_dir()?;
    let (patch, files) = suggested_fixes_patch()?;
    write_report("suggested-fixes.patch", &patch)?;
    write_report(
        "suggested-fixes.md",
        &suggested_fixes_report_body(&files, patch.is_empty()),
    )
}

fn suggested_fixes_patch() -> Result<(String, Vec<String>), String> {
    let mut patch = String::new();
    let mut files = Vec::new();
    for path in deterministic_suggested_fix_allowlist_files()? {
        let original = read_text_lossy(&path)?;
        let sorted = sorted_allowlist_content(&original);
        if sorted == original {
            continue;
        }
        append_whole_file_patch(&mut patch, &path, &original, &sorted);
        files.push(normalize_path(&path));
    }
    for path in deterministic_suggested_fix_docs_index_files() {
        let original = read_text_lossy(&path)?;
        let sorted = sorted_markdown_index_table_content(&original);
        if sorted == original {
            continue;
        }
        append_whole_file_patch(&mut patch, &path, &original, &sorted);
        files.push(normalize_path(&path));
    }
    for path in deterministic_suggested_fix_traceability_files() {
        let original = read_text_lossy(&path)?;
        let sorted = sorted_traceability_behavior_blocks_content(&original);
        if sorted == original {
            continue;
        }
        append_whole_file_patch(&mut patch, &path, &original, &sorted);
        files.push(normalize_path(&path));
    }
    for path in deterministic_suggested_fix_capability_files() {
        let original = read_text_lossy(&path)?;
        let sorted = sorted_capability_blocks_content(&original);
        if sorted == original {
            continue;
        }
        append_whole_file_patch(&mut patch, &path, &original, &sorted);
        files.push(normalize_path(&path));
    }
    for path in deterministic_suggested_fix_command_catalog_files() {
        let original = read_text_lossy(&path)?;
        let sorted = sorted_command_catalog_content(&original);
        if sorted == original {
            continue;
        }
        append_whole_file_patch(&mut patch, &path, &original, &sorted);
        files.push(normalize_path(&path));
    }
    files.sort();
    Ok((patch, files))
}

fn deterministic_suggested_fix_allowlist_files() -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for root in [Path::new(".ripr"), Path::new("policy")] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) == Some("txt") {
                paths.push(path);
            }
        }
    }
    paths.sort_by_key(|path| normalize_path(path));
    Ok(paths)
}

fn deterministic_suggested_fix_docs_index_files() -> Vec<PathBuf> {
    [
        PathBuf::from("docs/adr/README.md"),
        PathBuf::from("docs/specs/README.md"),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .collect()
}

fn deterministic_suggested_fix_traceability_files() -> Vec<PathBuf> {
    [PathBuf::from(".ripr/traceability.toml")]
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}

fn deterministic_suggested_fix_capability_files() -> Vec<PathBuf> {
    [PathBuf::from("metrics/capabilities.toml")]
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}

fn deterministic_suggested_fix_command_catalog_files() -> Vec<PathBuf> {
    [PathBuf::from("xtask/src/command.rs")]
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}

fn sorted_markdown_index_table_content(text: &str) -> String {
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let Some(header_index) = lines
        .iter()
        .position(|line| is_markdown_index_table_header(line))
    else {
        return text.to_string();
    };
    let Some(separator) = lines.get(header_index + 1) else {
        return text.to_string();
    };
    if !is_markdown_index_table_separator(separator) {
        return text.to_string();
    }

    let rows_start = header_index + 2;
    let mut rows_end = rows_start;
    while rows_end < lines.len() && is_markdown_table_row(&lines[rows_end]) {
        rows_end += 1;
    }
    if rows_end <= rows_start {
        return text.to_string();
    }

    let mut sorted_rows = lines[rows_start..rows_end].to_vec();
    sorted_rows.sort_by_key(|line| line.to_ascii_lowercase());
    if sorted_rows == lines[rows_start..rows_end] {
        return text.to_string();
    }

    lines.splice(rows_start..rows_end, sorted_rows);
    let mut output = lines.join("\n");
    if text.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn is_markdown_index_table_header(line: &str) -> bool {
    matches!(
        line.trim(),
        "| ADR | Status | Decision |" | "| Spec | Status | Topic |"
    )
}

fn is_markdown_index_table_separator(line: &str) -> bool {
    line.trim() == "| --- | --- | --- |"
}

fn is_markdown_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|')
}

fn sorted_traceability_behavior_blocks_content(text: &str) -> String {
    let mut block_starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim() == "[[behavior]]" {
            block_starts.push(offset);
        }
        offset += line.len();
    }
    if block_starts.len() <= 1 {
        return text.to_string();
    }

    let prefix = &text[..block_starts[0]];
    let mut ids = BTreeSet::new();
    let mut blocks = Vec::new();
    for (index, start) in block_starts.iter().copied().enumerate() {
        let end = block_starts.get(index + 1).copied().unwrap_or(text.len());
        let block = &text[start..end];
        if toml_array_table_block_is_unsafe_to_sort(block, "[[behavior]]") {
            return text.to_string();
        }
        let Some(id) = traceability_behavior_block_id(block) else {
            return text.to_string();
        };
        if !is_spec_id(&id) || !ids.insert(id.clone()) {
            return text.to_string();
        }
        blocks.push((id, block));
    }

    let original_order = blocks.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    blocks.sort_by(|left, right| left.0.cmp(&right.0));
    let sorted_order = blocks.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    if original_order == sorted_order {
        return text.to_string();
    }

    let mut output = prefix.to_string();
    for (_, block) in blocks {
        output.push_str(block);
    }
    output
}

fn sorted_capability_blocks_content(text: &str) -> String {
    let mut block_starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim() == "[[capability]]" {
            block_starts.push(offset);
        }
        offset += line.len();
    }
    if block_starts.len() <= 1 {
        return text.to_string();
    }

    let (capabilities, parse_violations) =
        parse_capabilities_manifest_text("metrics/capabilities.toml", text);
    if !parse_violations.is_empty() || capabilities.len() != block_starts.len() {
        return text.to_string();
    }

    let prefix = &text[..block_starts[0]];
    let mut ids = BTreeSet::new();
    let mut blocks = Vec::new();
    for ((index, start), capability) in block_starts
        .iter()
        .copied()
        .enumerate()
        .zip(capabilities.iter())
    {
        let end = block_starts.get(index + 1).copied().unwrap_or(text.len());
        let block = &text[start..end];
        if toml_array_table_block_is_unsafe_to_sort(block, "[[capability]]") {
            return text.to_string();
        }
        let Some((spec, id)) = capability_sort_key(capability) else {
            return text.to_string();
        };
        if !ids.insert(id.clone()) {
            return text.to_string();
        }
        blocks.push(((spec, id), block));
    }

    let original_order = blocks
        .iter()
        .map(|((spec, id), _)| (spec.clone(), id.clone()))
        .collect::<Vec<_>>();
    blocks.sort_by(|left, right| left.0.cmp(&right.0));
    let sorted_order = blocks
        .iter()
        .map(|((spec, id), _)| (spec.clone(), id.clone()))
        .collect::<Vec<_>>();
    if original_order == sorted_order {
        return text.to_string();
    }

    let mut output = prefix.to_string();
    for (_, block) in blocks {
        output.push_str(block);
    }
    let (sorted_capabilities, sorted_violations) =
        parse_capabilities_manifest_text("metrics/capabilities.toml", &output);
    let Some(sorted_capability_order) = capability_sort_keys(&sorted_capabilities) else {
        return text.to_string();
    };
    if !sorted_violations.is_empty()
        || sorted_capabilities.len() != block_starts.len()
        || sorted_capability_order != sorted_order
    {
        return text.to_string();
    }
    output
}

fn sorted_command_catalog_content(text: &str) -> String {
    let start_marker = "pub(crate) fn command_catalog() -> Vec<CommandCatalogEntry> {\n    vec![\n";
    let Some(start) = text.find(start_marker) else {
        return text.to_string();
    };
    let entries_start = start + start_marker.len();
    let Some(end_relative) = text[entries_start..].find("\n    ]\n}") else {
        return text.to_string();
    };
    let entries_end = entries_start + end_relative;
    let body = &text[entries_start..entries_end];
    let Some(sorted_body) = sorted_command_catalog_entry_blocks(body) else {
        return text.to_string();
    };
    if sorted_body == body {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len());
    output.push_str(&text[..entries_start]);
    output.push_str(&sorted_body);
    output.push_str(&text[entries_end..]);
    output
}

fn sorted_command_catalog_entry_blocks(body: &str) -> Option<String> {
    let mut order = BTreeMap::new();
    for (index, command) in known_commands().iter().enumerate() {
        order.insert(command.to_string(), index);
    }

    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut in_block = false;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim();
        if !in_block {
            if trimmed.is_empty() {
                current.push_str(line);
                continue;
            }
            if trimmed != "command_entry(" {
                return None;
            }
            in_block = true;
            current.push_str(line);
            continue;
        }

        current.push_str(line);
        if trimmed == ")," {
            let command = command_catalog_entry_block_command(&current)?;
            let index = *order.get(&command)?;
            blocks.push((index, blocks.len(), std::mem::take(&mut current)));
            in_block = false;
        }
    }
    if in_block || !current.trim().is_empty() || blocks.len() <= 1 {
        return None;
    }

    let original_order = blocks
        .iter()
        .map(|(order, original_index, _)| (*order, *original_index))
        .collect::<Vec<_>>();
    blocks.sort_by_key(|(order, original_index, _)| (*order, *original_index));
    let sorted_order = blocks
        .iter()
        .map(|(order, original_index, _)| (*order, *original_index))
        .collect::<Vec<_>>();
    if original_order == sorted_order {
        return Some(body.to_string());
    }

    Some(blocks.into_iter().map(|(_, _, block)| block).collect())
}

fn command_catalog_entry_block_command(block: &str) -> Option<String> {
    for line in block.lines().skip(1) {
        let trimmed = line.trim().trim_end_matches(',').trim();
        if !trimmed.starts_with('"') {
            continue;
        }
        return parse_quoted_value(trimmed).ok();
    }
    None
}

fn capability_sort_keys(capabilities: &[Capability]) -> Option<Vec<(String, String)>> {
    capabilities.iter().map(capability_sort_key).collect()
}

fn capability_sort_key(capability: &Capability) -> Option<(String, String)> {
    let id = capability.id.as_ref()?;
    let spec = capability.spec.as_ref()?;
    if !is_snake_case_id(id) || !is_spec_id(spec) {
        return None;
    }
    Some((spec.clone(), id.clone()))
}

fn toml_array_table_block_is_unsafe_to_sort(block: &str, table_header: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut active_array = false;
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == table_header {
            continue;
        }
        if active_array {
            if trimmed.starts_with(']') {
                active_array = false;
            } else if parse_array_item(trimmed).is_err() {
                return true;
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !seen.insert(key.to_string()) {
            return true;
        }
        let value = value.trim();
        if value == "[" {
            active_array = true;
        } else if value.starts_with('[') && parse_inline_array(value).is_err() {
            return true;
        }
    }
    active_array
}

fn traceability_behavior_block_id(block: &str) -> Option<String> {
    for line in block.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "id" {
            continue;
        }
        return parse_quoted_value(value).ok();
    }
    None
}

fn append_whole_file_patch(body: &mut String, path: &Path, before: &str, after: &str) {
    let normalized = normalize_path(path);
    let before_count = patch_line_count(before);
    let after_count = patch_line_count(after);
    body.push_str(&format!("diff --git a/{normalized} b/{normalized}\n"));
    body.push_str(&format!("--- a/{normalized}\n"));
    body.push_str(&format!("+++ b/{normalized}\n"));
    body.push_str(&format!(
        "@@ {} {} @@\n",
        patch_hunk_range(false, before_count),
        patch_hunk_range(true, after_count)
    ));
    for line in before.lines() {
        body.push('-');
        body.push_str(line);
        body.push('\n');
    }
    for line in after.lines() {
        body.push('+');
        body.push_str(line);
        body.push('\n');
    }
}

fn patch_line_count(text: &str) -> usize {
    text.lines().count()
}

fn patch_hunk_range(addition: bool, count: usize) -> String {
    let sign = if addition { '+' } else { '-' };
    if count == 0 {
        format!("{sign}0,0")
    } else {
        format!("{sign}1,{count}")
    }
}

fn suggested_fixes_report_body(files: &[String], patch_empty: bool) -> String {
    let mut body = "# ripr suggested fixes\n\nStatus: pass\n\n".to_string();
    body.push_str("Patch: `target/ripr/reports/suggested-fixes.patch`\n\n");
    body.push_str("Scope:\n\n");
    body.push_str("- deterministic allowlist ordering under `.ripr/*.txt` and `policy/*.txt`\n");
    body.push_str("- deterministic docs index table ordering for specs and ADRs\n");
    body.push_str("- deterministic traceability behavior block ordering by spec id\n");
    body.push_str("- deterministic capability block ordering by spec ID and capability ID\n");
    body.push_str("- deterministic command catalog ordering by xtask help order\n");
    body.push_str("- no badge value edits\n");
    body.push_str("- no golden blessings\n");
    body.push_str("- no baselines, suppressions, dependency exceptions, or schema changes\n\n");
    if patch_empty {
        body.push_str("No deterministic patch suggestions were found.\n");
    } else {
        body.push_str("Suggested patch files:\n\n");
        for file in files {
            body.push_str(&format!("- `{file}`\n"));
        }
    }
    body
}

pub(crate) fn pr_summary_impl() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let mut body = pr_summary_body(&changes);
    // Advisory proof-route section; computation failure degrades to an
    // in-section note and never fails pr-summary.
    body.push_str(&reports::pr_summary_proof_route_section());
    write_report("pr-summary.md", &body)
}

fn check_pr_shape() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let warnings = pr_shape_warnings(&changes);
    write_report("pr-shape.md", &pr_shape_report_body(&warnings))
}

pub(crate) fn pr_triage_report_impl() -> Result<(), String> {
    let prs = collect_open_prs_for_triage()?;
    let today = current_epoch_day()?;
    let generated_at = generated_at_unix_ms()?;
    let findings = pr_triage_findings(&prs, today);
    write_report("pr-triage.md", &pr_triage_markdown(&prs, &findings, today))?;
    write_report(
        "pr-triage.json",
        &pr_triage_json(&prs, &findings, today, &generated_at),
    )
}

pub(crate) fn gh_pr_status_impl(args: &[String]) -> Result<(), String> {
    let number = parse_gh_pr_status_args(args)?;
    let pr = collect_gh_pr_status(number)?;
    let mut warnings = Vec::new();
    let (required_contexts, required_contexts_available) =
        match collect_required_status_contexts(&pr.base_ref_name) {
            Ok(contexts) => (contexts, true),
            Err(err) => {
                warnings.push(format!(
                    "required status context lookup failed; using status rollup only: {err}"
                ));
                (Vec::new(), false)
            }
        };
    let readiness = gh_pr_status_readiness(
        &pr,
        &required_contexts,
        required_contexts_available,
        warnings,
    );
    let body = gh_pr_status_markdown(&pr, &required_contexts, &readiness);
    write_report("gh-pr-status.md", &body)?;
    write_report(
        "gh-pr-status.json",
        &gh_pr_status_json(&pr, &required_contexts, &readiness),
    )?;
    println!(
        "PR #{} safe next action: {}",
        pr.number, readiness.safe_next_action
    );
    Ok(())
}

fn parse_gh_pr_status_args(args: &[String]) -> Result<u64, String> {
    if args.is_empty() {
        return Err("cargo xtask gh-pr-status requires `--pr <number>`".to_string());
    }
    let mut pr_number = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--pr" {
            let Some(value) = args.get(index + 1) else {
                return Err("cargo xtask gh-pr-status requires a number after `--pr`".to_string());
            };
            pr_number = Some(parse_positive_u64(value, "--pr")?);
            index += 2;
            continue;
        }
        if pr_number.is_none() && !arg.starts_with('-') {
            pr_number = Some(parse_positive_u64(arg, "PR number")?);
            index += 1;
            continue;
        }
        return Err(format!(
            "unknown gh-pr-status argument `{arg}`; use `cargo xtask gh-pr-status --pr <number>`"
        ));
    }
    pr_number.ok_or_else(|| "cargo xtask gh-pr-status requires `--pr <number>`".to_string())
}

fn parse_positive_u64(value: &str, label: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|err| format!("{label} must be a positive integer: {err}"))?;
    if parsed == 0 {
        return Err(format!("{label} must be greater than zero"));
    }
    Ok(parsed)
}

fn collect_gh_pr_status(number: u64) -> Result<GhPrStatusPullRequest, String> {
    let fields = [
        "number",
        "title",
        "isDraft",
        "mergeStateStatus",
        "headRefName",
        "baseRefName",
        "reviewDecision",
        "latestReviews",
        "statusCheckRollup",
    ]
    .join(",");
    let output = run_output_owned(
        "gh",
        &[
            "pr".to_string(),
            "view".to_string(),
            number.to_string(),
            "--json".to_string(),
            fields,
        ],
    )?;
    parse_gh_pr_status_pull_request(&output)
}

fn collect_required_status_contexts(base_ref_name: &str) -> Result<Vec<String>, String> {
    let repo_output = run_output("gh", &["repo", "view", "--json", "owner,name"])?;
    let repo_value: Value = serde_json::from_str(&repo_output)
        .map_err(|err| format!("failed to parse gh repo JSON: {err}"))?;
    let owner = repo_value
        .get("owner")
        .and_then(|owner| owner.get("login"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("gh repo JSON is missing owner.login: {repo_value}"))?;
    let name = repo_value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("gh repo JSON is missing name: {repo_value}"))?;
    let branch = if base_ref_name.trim().is_empty() {
        "main"
    } else {
        base_ref_name.trim()
    };
    let endpoint = format!(
        "repos/{owner}/{name}/branches/{branch}/protection/required_status_checks/contexts"
    );
    let output = run_output_owned("gh", &["api".to_string(), endpoint])?;
    parse_required_status_contexts(&output)
}

fn parse_gh_pr_status_pull_request(text: &str) -> Result<GhPrStatusPullRequest, String> {
    let item: Value =
        serde_json::from_str(text).map_err(|err| format!("failed to parse gh PR JSON: {err}"))?;
    let number = item
        .get("number")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("gh PR JSON is missing numeric `number`: {item}"))?;
    let checks = item
        .get("statusCheckRollup")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(parse_pr_triage_check)
        .collect::<Vec<_>>();
    let reviews = item
        .get("latestReviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(parse_gh_pr_status_review)
        .collect::<Vec<_>>();
    Ok(GhPrStatusPullRequest {
        number,
        title: json_value_string(&item, "title"),
        is_draft: item
            .get("isDraft")
            .and_then(Value::as_bool)
            .is_some_and(|value| value),
        merge_state_status: json_value_string(&item, "mergeStateStatus"),
        head_ref_name: json_value_string(&item, "headRefName"),
        base_ref_name: json_value_string(&item, "baseRefName"),
        review_decision: json_value_string(&item, "reviewDecision"),
        checks,
        reviews,
    })
}

fn parse_gh_pr_status_review(value: &Value) -> GhPrStatusReview {
    let author = match value
        .get("author")
        .and_then(|author| author.get("login"))
        .and_then(Value::as_str)
    {
        Some(login) => login.to_string(),
        None => String::new(),
    };
    GhPrStatusReview {
        author,
        state: json_value_string(value, "state"),
    }
}

fn parse_required_status_contexts(text: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("failed to parse required status contexts JSON: {err}"))?;
    let Some(items) = value.as_array() else {
        return Err("required status contexts JSON must be an array".to_string());
    };
    let mut contexts = items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    contexts.sort();
    contexts.dedup();
    Ok(contexts)
}

fn gh_pr_status_readiness(
    pr: &GhPrStatusPullRequest,
    required_contexts: &[String],
    required_contexts_available: bool,
    warnings: Vec<String>,
) -> GhPrStatusReadiness {
    let failed_checks = pr
        .checks
        .iter()
        .filter(|check| pr_check_failed(check))
        .map(format_gh_pr_check_status)
        .collect::<Vec<_>>();
    let pending_checks = pr
        .checks
        .iter()
        .filter(|check| pr_check_pending(check))
        .map(format_gh_pr_check_status)
        .collect::<Vec<_>>();
    let required_checks_outstanding = pr
        .checks
        .iter()
        .filter(|check| pr_check_pending(check))
        .filter(|check| {
            !required_contexts_available
                || required_contexts
                    .iter()
                    .any(|context| context.eq_ignore_ascii_case(&check.name))
        })
        .map(format_gh_pr_check_status)
        .collect::<Vec<_>>();
    let droid_checks = pr
        .checks
        .iter()
        .filter(|check| check.name.to_ascii_lowercase().contains("droid"))
        .map(format_gh_pr_check_status)
        .collect::<Vec<_>>();
    let behind_main = pr.merge_state_status.eq_ignore_ascii_case("BEHIND");
    let mut readiness = GhPrStatusReadiness {
        behind_main,
        required_contexts_available,
        required_checks_outstanding,
        failed_checks,
        pending_checks,
        droid_checks,
        safe_next_action: String::new(),
        warnings,
    };
    readiness.safe_next_action = gh_pr_safe_next_action(pr, &readiness).to_string();
    readiness
}

fn gh_pr_safe_next_action(
    pr: &GhPrStatusPullRequest,
    readiness: &GhPrStatusReadiness,
) -> &'static str {
    if !readiness.failed_checks.is_empty()
        || pr.review_decision.eq_ignore_ascii_case("CHANGES_REQUESTED")
    {
        return "inspect failure";
    }
    if readiness.behind_main {
        return "rebase";
    }
    if pr.is_draft
        || pr.review_decision.eq_ignore_ascii_case("REVIEW_REQUIRED")
        || !readiness.required_checks_outstanding.is_empty()
        || !readiness.pending_checks.is_empty()
    {
        return "wait";
    }
    let merge_state = pr.merge_state_status.to_ascii_uppercase();
    match merge_state.as_str() {
        "CLEAN" => "merge",
        "BLOCKED" | "DRAFT" | "HAS_HOOKS" | "UNKNOWN" | "UNSTABLE" => "wait",
        "DIRTY" => "inspect failure",
        _ => "inspect failure",
    }
}

fn format_gh_pr_check_status(check: &PrTriageCheck) -> String {
    let status = if check.status.trim().is_empty() {
        "unknown"
    } else {
        check.status.trim()
    };
    let conclusion = if check.conclusion.trim().is_empty() {
        "unknown"
    } else {
        check.conclusion.trim()
    };
    format!("{} (status={status}, conclusion={conclusion})", check.name)
}

fn gh_pr_status_markdown(
    pr: &GhPrStatusPullRequest,
    required_contexts: &[String],
    readiness: &GhPrStatusReadiness,
) -> String {
    let mut body = String::new();
    body.push_str("# GitHub PR Status\n\n");
    body.push_str(&format!("- PR: #{} {}\n", pr.number, pr.title));
    body.push_str(&format!(
        "- Branch: `{}` -> `{}`\n",
        pr.head_ref_name, pr.base_ref_name
    ));
    body.push_str(&format!(
        "- mergeable_state: `{}`\n",
        value_or_unknown(&pr.merge_state_status)
    ));
    body.push_str(&format!(
        "- review_decision: `{}`\n",
        value_or_unknown(&pr.review_decision)
    ));
    body.push_str(&format!("- draft: `{}`\n", pr.is_draft));
    body.push_str(&format!("- behind main: `{}`\n", readiness.behind_main));
    body.push_str(&format!(
        "- safe next action: `{}`\n\n",
        readiness.safe_next_action
    ));

    body.push_str("## Required Checks Outstanding\n\n");
    if readiness.required_contexts_available {
        body.push_str(&format!(
            "- Required context lookup: available ({} context(s))\n",
            required_contexts.len()
        ));
    } else {
        body.push_str("- Required context lookup: unavailable; using status rollup only\n");
    }
    append_markdown_list(&mut body, &readiness.required_checks_outstanding, "None");

    body.push_str("\n## Failed Checks\n\n");
    append_markdown_list(&mut body, &readiness.failed_checks, "None");

    body.push_str("\n## Pending Checks\n\n");
    append_markdown_list(&mut body, &readiness.pending_checks, "None");

    body.push_str("\n## Reviews\n\n");
    body.push_str(&format!(
        "- Review decision: `{}`\n",
        value_or_unknown(&pr.review_decision)
    ));
    if pr.reviews.is_empty() {
        body.push_str("- No latest reviews returned\n");
    } else {
        for review in &pr.reviews {
            body.push_str(&format!(
                "- {}: `{}`\n",
                value_or_unknown(&review.author),
                value_or_unknown(&review.state)
            ));
        }
    }

    body.push_str("\n## Droid Status\n\n");
    append_markdown_list(
        &mut body,
        &readiness.droid_checks,
        "No droid check entries returned",
    );

    body.push_str("\n## Warnings\n\n");
    append_markdown_list(&mut body, &readiness.warnings, "None");
    body
}

fn gh_pr_status_json(
    pr: &GhPrStatusPullRequest,
    required_contexts: &[String],
    readiness: &GhPrStatusReadiness,
) -> String {
    let advisory_checks = gh_pr_advisory_checks(pr, required_contexts, readiness);
    let mut body = "{\n".to_string();
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!("  \"pr_number\": {},\n", pr.number));
    body.push_str(&format!("  \"title\": \"{}\",\n", json_escape(&pr.title)));
    body.push_str("  \"branch\": {\n");
    body.push_str(&format!(
        "    \"head\": \"{}\",\n",
        json_escape(&pr.head_ref_name)
    ));
    body.push_str(&format!(
        "    \"base\": \"{}\"\n",
        json_escape(&pr.base_ref_name)
    ));
    body.push_str("  },\n");
    body.push_str(&format!(
        "  \"merge_state\": \"{}\",\n",
        json_escape(&pr.merge_state_status)
    ));
    body.push_str(&format!("  \"behind_main\": {},\n", readiness.behind_main));
    body.push_str(&format!("  \"draft\": {},\n", pr.is_draft));
    body.push_str(&format!(
        "  \"required_contexts_available\": {},\n",
        readiness.required_contexts_available
    ));
    body.push_str("  \"required_checks_outstanding\": [");
    write_json_string_array(&mut body, &readiness.required_checks_outstanding);
    body.push_str("],\n");
    body.push_str("  \"failed_checks\": [");
    write_json_string_array(&mut body, &readiness.failed_checks);
    body.push_str("],\n");
    body.push_str("  \"pending_checks\": [");
    write_json_string_array(&mut body, &readiness.pending_checks);
    body.push_str("],\n");
    body.push_str("  \"advisory_checks\": [");
    write_json_string_array(&mut body, &advisory_checks);
    body.push_str("],\n");
    body.push_str("  \"droid_status\": [");
    write_json_string_array(&mut body, &readiness.droid_checks);
    body.push_str("],\n");
    body.push_str(&format!(
        "  \"review_decision\": \"{}\",\n",
        json_escape(&pr.review_decision)
    ));
    body.push_str("  \"reviews\": [\n");
    for (index, review) in pr.reviews.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"author\": \"{}\",\n",
            json_escape(&review.author)
        ));
        body.push_str(&format!(
            "      \"state\": \"{}\"\n",
            json_escape(&review.state)
        ));
        body.push_str("    }");
    }
    body.push_str("\n  ],\n");
    body.push_str(&format!(
        "  \"safe_next_action\": \"{}\",\n",
        json_escape(&readiness.safe_next_action)
    ));
    body.push_str("  \"warnings\": [");
    write_json_string_array(&mut body, &readiness.warnings);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn gh_pr_advisory_checks(
    pr: &GhPrStatusPullRequest,
    required_contexts: &[String],
    readiness: &GhPrStatusReadiness,
) -> Vec<String> {
    if !readiness.required_contexts_available {
        return Vec::new();
    }
    pr.checks
        .iter()
        .filter(|check| {
            !required_contexts
                .iter()
                .any(|context| context.eq_ignore_ascii_case(&check.name))
        })
        .map(format_gh_pr_check_status)
        .collect()
}

fn value_or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "unknown"
    } else {
        value.trim()
    }
}

fn append_markdown_list(body: &mut String, items: &[String], empty: &str) {
    if items.is_empty() {
        body.push_str(&format!("- {empty}\n"));
        return;
    }
    for item in items {
        body.push_str(&format!("- {item}\n"));
    }
}

fn collect_open_prs_for_triage() -> Result<Vec<PrTriagePullRequest>, String> {
    let fields = [
        "number",
        "title",
        "body",
        "isDraft",
        "createdAt",
        "updatedAt",
        "mergeStateStatus",
        "headRefName",
        "baseRefName",
        "reviewDecision",
        "labels",
        "files",
        "statusCheckRollup",
    ]
    .join(",");
    let output = run_output(
        "gh",
        &[
            "pr", "list", "--limit", "100", "--state", "open", "--json", &fields,
        ],
    )?;
    parse_pr_triage_pull_requests(&output)
}

fn parse_pr_triage_pull_requests(text: &str) -> Result<Vec<PrTriagePullRequest>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("failed to parse gh PR JSON: {err}"))?;
    let Some(items) = value.as_array() else {
        return Err("gh PR JSON must be an array".to_string());
    };
    let mut prs = Vec::new();
    for item in items {
        let number = item
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("gh PR JSON item is missing numeric `number`: {item}"))?;
        let title = json_value_string(item, "title");
        let body = json_value_string(item, "body");
        let mut files = item
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|file| file.get("path").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        let mut labels = item
            .get("labels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|label| {
                label
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        labels.sort();
        labels.dedup();
        let checks = item
            .get("statusCheckRollup")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(parse_pr_triage_check)
            .collect::<Vec<_>>();
        prs.push(PrTriagePullRequest {
            number,
            title,
            body,
            is_draft: item
                .get("isDraft")
                .and_then(Value::as_bool)
                .is_some_and(|value| value),
            created_at: json_value_string(item, "createdAt"),
            updated_at: json_value_string(item, "updatedAt"),
            merge_state_status: json_value_string(item, "mergeStateStatus"),
            head_ref_name: json_value_string(item, "headRefName"),
            base_ref_name: json_value_string(item, "baseRefName"),
            review_decision: json_value_string(item, "reviewDecision"),
            labels,
            files,
            checks,
        });
    }
    prs.sort_by_key(|pr| pr.number);
    Ok(prs)
}

fn parse_pr_triage_check(value: &Value) -> PrTriageCheck {
    let name = match value
        .get("name")
        .or_else(|| value.get("context"))
        .and_then(Value::as_str)
    {
        Some(name) => name.to_string(),
        None => String::new(),
    };
    let status = match value
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| value.get("state").and_then(Value::as_str))
    {
        Some(status) => status.to_string(),
        None => String::new(),
    };
    let conclusion = match value
        .get("conclusion")
        .and_then(Value::as_str)
        .or_else(|| value.get("state").and_then(Value::as_str))
    {
        Some(conclusion) => conclusion.to_string(),
        None => String::new(),
    };
    PrTriageCheck {
        name,
        status,
        conclusion,
    }
}

fn json_value_string(value: &Value, key: &str) -> String {
    match value.get(key).and_then(Value::as_str) {
        Some(item) => item.to_string(),
        None => String::new(),
    }
}

fn current_epoch_day() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before UNIX_EPOCH: {err}"))?;
    Ok((duration.as_secs() / 86_400) as i64)
}

fn pr_triage_findings(prs: &[PrTriagePullRequest], today: i64) -> Vec<PrTriageFinding> {
    let mut findings = Vec::new();
    findings.extend(pr_triage_title_family_findings(prs));
    findings.extend(pr_triage_file_set_findings(prs));
    findings.extend(pr_triage_stale_draft_findings(prs, today));
    findings.extend(pr_triage_behind_findings(prs));
    findings.extend(pr_triage_validation_findings(prs));
    findings.extend(pr_triage_sensitive_surface_findings(prs));
    findings.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.prs.cmp(&right.prs))
            .then_with(|| left.message.cmp(&right.message))
    });
    findings
}

fn pr_triage_title_family_findings(prs: &[PrTriagePullRequest]) -> Vec<PrTriageFinding> {
    let mut families = BTreeMap::<String, Vec<&PrTriagePullRequest>>::new();
    for pr in prs {
        let family = pr_title_family(&pr.title);
        if !family.is_empty() {
            families.entry(family).or_default().push(pr);
        }
    }
    families
        .into_iter()
        .filter_map(|(family, prs)| {
            if prs.len() < 2 {
                return None;
            }
            Some(PrTriageFinding {
                category: "same title family".to_string(),
                severity: "warn".to_string(),
                message: format!("{} open PRs share title family `{family}`", prs.len()),
                prs: prs.iter().map(|pr| pr.number).collect(),
                details: prs.iter().map(|pr| format_pr_triage_ref(pr)).collect(),
                recommended_action:
                    "Choose the canonical PR and close, retitle, or restack the duplicate variants."
                        .to_string(),
            })
        })
        .collect()
}

fn pr_triage_file_set_findings(prs: &[PrTriagePullRequest]) -> Vec<PrTriageFinding> {
    let mut file_sets = BTreeMap::<String, Vec<&PrTriagePullRequest>>::new();
    for pr in prs {
        if pr.files.is_empty() {
            continue;
        }
        file_sets.entry(pr.files.join("\n")).or_default().push(pr);
    }
    file_sets
        .into_iter()
        .filter_map(|(file_set, prs)| {
            if prs.len() < 2 {
                return None;
            }
            let preview = file_set
                .lines()
                .take(8)
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ");
            Some(PrTriageFinding {
                category: "same changed file set".to_string(),
                severity: "warn".to_string(),
                message: format!("{} open PRs touch the same changed file set", prs.len()),
                prs: prs.iter().map(|pr| pr.number).collect(),
                details: vec![
                    format!(
                        "PRs: {}",
                        prs.iter()
                            .map(|pr| format_pr_triage_ref(pr))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                    format!("Files: {preview}"),
                ],
                recommended_action:
                    "Pick the canonical branch before review work drifts across equivalent diffs."
                        .to_string(),
            })
        })
        .collect()
}

fn pr_triage_stale_draft_findings(prs: &[PrTriagePullRequest], today: i64) -> Vec<PrTriageFinding> {
    const STALE_DRAFT_DAYS: i64 = 7;
    prs.iter()
        .filter_map(|pr| {
            if !pr.is_draft {
                return None;
            }
            let created_day = pr_created_day(&pr.created_at)?;
            let age_days = today - created_day;
            if age_days < STALE_DRAFT_DAYS {
                return None;
            }
            Some(PrTriageFinding {
                category: "stale draft".to_string(),
                severity: "warn".to_string(),
                message: format!("#{} has been draft for {age_days} day(s)", pr.number),
                prs: vec![pr.number],
                details: vec![format_pr_triage_ref(pr)],
                recommended_action:
                    "Refresh the draft, mark it ready for review, or close it if superseded."
                        .to_string(),
            })
        })
        .collect()
}

fn pr_triage_behind_findings(prs: &[PrTriagePullRequest]) -> Vec<PrTriageFinding> {
    prs.iter()
        .filter(|pr| pr.merge_state_status.eq_ignore_ascii_case("BEHIND"))
        .map(|pr| PrTriageFinding {
            category: "behind main".to_string(),
            severity: "warn".to_string(),
            message: format!("#{} is behind {}", pr.number, pr.base_ref_name),
            prs: vec![pr.number],
            details: vec![format!(
                "{} merge_state_status={}",
                format_pr_triage_ref(pr),
                pr.merge_state_status
            )],
            recommended_action: "Update the branch before relying on CI or merge-readiness state."
                .to_string(),
        })
        .collect()
}

fn pr_triage_validation_findings(prs: &[PrTriagePullRequest]) -> Vec<PrTriageFinding> {
    let mut findings = Vec::new();
    for pr in prs {
        let mut details = Vec::new();
        let failed = pr
            .checks
            .iter()
            .filter(|check| pr_check_failed(check))
            .map(|check| format!("{}={}", check.name, check.conclusion))
            .collect::<Vec<_>>();
        if !failed.is_empty() {
            details.push(format!("failed checks: {}", failed.join(", ")));
        }
        let pending = pr
            .checks
            .iter()
            .filter(|check| pr_check_pending(check))
            .map(|check| check.name.clone())
            .collect::<Vec<_>>();
        if !pending.is_empty() {
            details.push(format!("pending checks: {}", pending.join(", ")));
        }
        if pr.checks.is_empty() {
            details.push("no check rollup entries returned".to_string());
        }
        if let Some(body_warning) = pr_body_validation_warning(&pr.body) {
            details.push(body_warning);
        }
        if details.is_empty() {
            continue;
        }
        findings.push(PrTriageFinding {
            category: "incomplete validation".to_string(),
            severity: "warn".to_string(),
            message: format!("#{} needs validation follow-up", pr.number),
            prs: vec![pr.number],
            details,
            recommended_action: "Inspect the failing or missing validation before merge; update the PR body with the actual commands run.".to_string(),
        });
    }
    findings
}

fn pr_triage_sensitive_surface_findings(prs: &[PrTriagePullRequest]) -> Vec<PrTriageFinding> {
    let mut findings = Vec::new();
    for pr in prs {
        let mut details = Vec::new();
        for file in &pr.files {
            if let Some(reason) = pr_sensitive_file_reason(file) {
                details.push(format!("`{file}`: {reason}"));
            }
        }
        if details.is_empty() {
            continue;
        }
        findings.push(PrTriageFinding {
            category: "policy-sensitive surface".to_string(),
            severity: "warn".to_string(),
            message: format!(
                "#{} touches policy, gate, or generated workflow surfaces",
                pr.number
            ),
            prs: vec![pr.number],
            details,
            recommended_action:
                "Route review through the owning lane and check for policy authority drift."
                    .to_string(),
        });
    }
    findings
}

fn pr_triage_queue_dispositions(
    prs: &[PrTriagePullRequest],
    findings: &[PrTriageFinding],
) -> Vec<PrTriageQueueDisposition> {
    prs.iter()
        .map(|pr| pr_triage_queue_disposition(pr, findings))
        .collect()
}

fn pr_triage_queue_disposition(
    pr: &PrTriagePullRequest,
    findings: &[PrTriageFinding],
) -> PrTriageQueueDisposition {
    let (disposition, reason, recommended_action) = if pr_has_superseded_signal(pr) {
        (
            "superseded",
            "PR title, body, or labels declare it superseded",
            "Close or replace only after confirming the successor PR is current.",
        )
    } else if pr_has_duplicate_signal(pr) {
        (
            "close_duplicate",
            "PR title, body, or labels declare it duplicate",
            "Close only after confirming the canonical PR is selected.",
        )
    } else if pr_has_finding(findings, pr.number, "same title family")
        || pr_has_finding(findings, pr.number, "same changed file set")
        || pr_has_finding(findings, pr.number, "stale draft")
    {
        (
            "needs_owner_decision",
            "duplicate or stale work needs canonical owner selection",
            "Choose the canonical branch, refresh the stale draft, or close superseded variants.",
        )
    } else if pr.merge_state_status.eq_ignore_ascii_case("BEHIND") {
        (
            "needs_rebase",
            "branch is behind its base",
            "Update the branch before relying on CI or merge-readiness state.",
        )
    } else if pr_has_finding(findings, pr.number, "policy-sensitive surface") {
        (
            "do_not_touch_wrong_lane",
            "PR touches policy, generated badge, workflow, or gate-sensitive files",
            "Leave ownership to the matching lane unless explicitly assigned.",
        )
    } else if pr_has_finding(findings, pr.number, "incomplete validation")
        || pr.checks.iter().any(pr_check_failed)
        || pr.checks.iter().any(pr_check_pending)
    {
        (
            "needs_fresh_validation",
            "validation is pending, failing, absent, or not fully recorded",
            "Wait for checks, inspect failures, or update the PR body with actual commands run.",
        )
    } else if pr.is_draft || pr.review_decision.eq_ignore_ascii_case("REVIEW_REQUIRED") {
        (
            "needs_review",
            "PR is draft or still requires review",
            "Finish review before merge or further queue disposition.",
        )
    } else if pr.merge_state_status.eq_ignore_ascii_case("CLEAN") {
        (
            "merge_candidate",
            "PR is current, non-draft, and has no detected validation or policy-sensitive queue risks",
            "Review the diff and merge if the scope is still the canonical path.",
        )
    } else {
        (
            "needs_review",
            "merge state is not clean enough for an automatic queue recommendation",
            "Inspect the PR state before taking action.",
        )
    };
    PrTriageQueueDisposition {
        pr_number: pr.number,
        disposition: disposition.to_string(),
        reason: reason.to_string(),
        recommended_action: recommended_action.to_string(),
    }
}

fn pr_has_finding(findings: &[PrTriageFinding], number: u64, category: &str) -> bool {
    findings
        .iter()
        .any(|finding| finding.category == category && finding.prs.contains(&number))
}

fn pr_has_superseded_signal(pr: &PrTriagePullRequest) -> bool {
    pr_triage_title_or_label_contains(pr, "superseded")
        || pr.body.to_ascii_lowercase().contains("superseded by")
}

fn pr_has_duplicate_signal(pr: &PrTriagePullRequest) -> bool {
    pr_triage_title_or_label_contains(pr, "duplicate")
        || pr.body.to_ascii_lowercase().contains("duplicate of")
}

fn pr_triage_title_or_label_contains(pr: &PrTriagePullRequest, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    pr.title.to_ascii_lowercase().contains(&needle)
        || pr
            .labels
            .iter()
            .any(|label| label.to_ascii_lowercase().contains(&needle))
}

fn pr_check_failed(check: &PrTriageCheck) -> bool {
    matches!(
        check.conclusion.to_ascii_uppercase().as_str(),
        "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
    )
}

fn pr_check_pending(check: &PrTriageCheck) -> bool {
    matches!(
        check.status.to_ascii_uppercase().as_str(),
        "PENDING" | "EXPECTED" | "IN_PROGRESS" | "QUEUED" | "WAITING" | "REQUESTED"
    )
}

fn pr_body_validation_warning(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    if !lower.contains("validation") {
        return Some("PR body has no validation section".to_string());
    }
    if lower.contains("pre-existing failure") || lower.contains("preexisting failure") {
        return Some("PR body mentions pre-existing validation failures".to_string());
    }
    if lower.contains("not run") || lower.contains("not_run") {
        return Some("PR body lists validation that was not run".to_string());
    }
    if !lower.contains("cargo xtask check-pr") {
        return Some("PR body validation does not list cargo xtask check-pr".to_string());
    }
    None
}

fn pr_sensitive_file_reason(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if path.starts_with(".github/workflows/") {
        return Some("workflow behavior can alter generated CI or branch checks");
    }
    if path.starts_with("policy/") || path.starts_with(".ripr/") || path.starts_with("docs/policy/")
    {
        return Some("policy ledger or policy documentation");
    }
    if lower.starts_with("badges/") {
        return Some("generated public badge endpoint surface");
    }
    if lower.contains("gate") || lower.contains("baseline") || lower.contains("suppression") {
        return Some("policy authority, gate, baseline, or suppression semantics");
    }
    if lower.contains("generated") && (lower.contains("ci") || lower.contains("workflow")) {
        return Some("generated CI workflow surface");
    }
    None
}

fn pr_title_family(title: &str) -> String {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            push_title_family_token(&mut tokens, &current);
            current.clear();
        }
    }
    if !current.is_empty() {
        push_title_family_token(&mut tokens, &current);
    }
    tokens.join(" ")
}

fn push_title_family_token(tokens: &mut Vec<String>, token: &str) {
    if matches!(token, "wip" | "draft" | "v2" | "v3" | "variant") {
        return;
    }
    tokens.push(token.to_string());
}

fn pr_created_day(created_at: &str) -> Option<i64> {
    let date = created_at.get(0..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    Some(days_from_civil(year, month, day))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let month_for_formula = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * month_for_formula + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn pr_triage_markdown(
    prs: &[PrTriagePullRequest],
    findings: &[PrTriageFinding],
    today: i64,
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let dispositions = pr_triage_queue_dispositions(prs, findings);
    let disposition_by_pr = dispositions
        .iter()
        .map(|disposition| (disposition.pr_number, disposition))
        .collect::<BTreeMap<_, _>>();
    let mut body = format!("# ripr PR triage report\n\nStatus: {status}\n\n");
    body.push_str("Mode: advisory\n\n");
    body.push_str("This report summarizes open PR queue risks for agents. It does not close, merge, update, or mutate PRs.\n\n");
    body.push_str(&format!("Open PRs scanned: {}\n\n", prs.len()));

    body.push_str("## Findings\n\n");
    if findings.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for finding in findings {
            body.push_str(&format!(
                "### {} ({})\n\n{}\n\n",
                finding.category, finding.severity, finding.message
            ));
            if !finding.prs.is_empty() {
                body.push_str("PRs:\n\n");
                for number in &finding.prs {
                    body.push_str(&format!("- #{}\n", number));
                }
                body.push('\n');
            }
            body.push_str("Details:\n\n");
            write_path_list(&mut body, &finding.details);
            body.push_str("\nRecommended action:\n\n```text\n");
            body.push_str(&finding.recommended_action);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Queue Disposition\n\n");
    if dispositions.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        body.push_str("| PR | Disposition | Reason | Recommended action |\n");
        body.push_str("| --- | --- | --- | --- |\n");
        for disposition in &dispositions {
            body.push_str(&format!(
                "| #{} | `{}` | {} | {} |\n",
                disposition.pr_number,
                markdown_escape_table(&disposition.disposition),
                markdown_escape_table(&disposition.reason),
                markdown_escape_table(&disposition.recommended_action)
            ));
        }
    }
    body.push('\n');

    body.push_str("## Open PRs\n\n");
    if prs.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        body.push_str("| PR | Disposition | Draft | Age | Merge state | Checks | Files |\n");
        body.push_str("| --- | --- | --- | ---: | --- | --- | ---: |\n");
        for pr in prs {
            let age = pr_age_label(pr, today);
            let disposition = disposition_by_pr
                .get(&pr.number)
                .map(|item| item.disposition.as_str())
                .unwrap_or("unknown");
            body.push_str(&format!(
                "| #{} {} | `{}` | {} | {} | {} | {} | {} |\n",
                pr.number,
                markdown_escape_table(&pr.title),
                markdown_escape_table(disposition),
                pr.is_draft,
                age,
                markdown_escape_table(&pr.merge_state_status),
                markdown_escape_table(&pr_checks_summary(&pr.checks)),
                pr.files.len()
            ));
        }
    }
    body.push_str("\n## Next Commands\n\n```bash\n");
    body.push_str("cargo xtask pr-triage-report\n");
    body.push_str("gh pr view <number> --json mergeStateStatus,statusCheckRollup,files\n");
    body.push_str("```\n");
    body
}

fn pr_triage_json(
    prs: &[PrTriagePullRequest],
    findings: &[PrTriageFinding],
    today: i64,
    generated_at: &str,
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let mut body = "{\n".to_string();
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str(&format!(
        "  \"generated_at\": \"{}\",\n",
        json_escape(generated_at)
    ));
    let dispositions = pr_triage_queue_dispositions(prs, findings);
    body.push_str("  \"open_prs\": [\n");
    for (index, pr) in prs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        write_pr_triage_pull_request_json(&mut body, pr, today);
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"queue_disposition\": [\n");
    for (index, disposition) in dispositions.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        write_pr_triage_queue_disposition_json(&mut body, disposition, 4);
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"findings\": [\n");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        write_pr_triage_finding_json(&mut body, finding, 4);
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"recommended_actions\": [\n");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"category\": \"{}\",\n",
            json_escape(&finding.category)
        ));
        body.push_str("      \"prs\": ");
        write_json_u64_array(&mut body, &finding.prs);
        body.push_str(",\n");
        body.push_str(&format!(
            "      \"action\": \"{}\"\n",
            json_escape(&finding.recommended_action)
        ));
        body.push_str("    }");
    }
    body.push_str("\n  ]\n");
    body.push_str("}\n");
    body
}

fn write_pr_triage_pull_request_json(body: &mut String, pr: &PrTriagePullRequest, today: i64) {
    body.push_str("    {\n");
    body.push_str(&format!("      \"number\": {},\n", pr.number));
    body.push_str(&format!(
        "      \"title\": \"{}\",\n",
        json_escape(&pr.title)
    ));
    body.push_str(&format!("      \"is_draft\": {},\n", pr.is_draft));
    body.push_str(&format!(
        "      \"created_at\": \"{}\",\n",
        json_escape(&pr.created_at)
    ));
    body.push_str(&format!(
        "      \"updated_at\": \"{}\",\n",
        json_escape(&pr.updated_at)
    ));
    body.push_str("      \"age_days\": ");
    match pr_created_day(&pr.created_at) {
        Some(created_day) => body.push_str(&(today - created_day).to_string()),
        None => body.push_str("null"),
    }
    body.push_str(",\n");
    body.push_str(&format!(
        "      \"merge_state_status\": \"{}\",\n",
        json_escape(&pr.merge_state_status)
    ));
    body.push_str(&format!(
        "      \"head_ref_name\": \"{}\",\n",
        json_escape(&pr.head_ref_name)
    ));
    body.push_str(&format!(
        "      \"base_ref_name\": \"{}\",\n",
        json_escape(&pr.base_ref_name)
    ));
    body.push_str(&format!(
        "      \"review_decision\": \"{}\",\n",
        json_escape(&pr.review_decision)
    ));
    body.push_str(&format!(
        "      \"checks_summary\": \"{}\",\n",
        json_escape(&pr_checks_summary(&pr.checks))
    ));
    body.push_str("      \"labels\": [");
    write_json_string_array(body, &pr.labels);
    body.push_str("],\n      \"files\": [");
    write_json_string_array(body, &pr.files);
    body.push_str("],\n      \"checks\": [\n");
    for (index, check) in pr.checks.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("        {\n");
        body.push_str(&format!(
            "          \"name\": \"{}\",\n",
            json_escape(&check.name)
        ));
        body.push_str(&format!(
            "          \"status\": \"{}\",\n",
            json_escape(&check.status)
        ));
        body.push_str(&format!(
            "          \"conclusion\": \"{}\"\n",
            json_escape(&check.conclusion)
        ));
        body.push_str("        }");
    }
    body.push_str("\n      ]\n");
    body.push_str("    }");
}

fn write_pr_triage_queue_disposition_json(
    body: &mut String,
    disposition: &PrTriageQueueDisposition,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let inner = " ".repeat(indent + 2);
    body.push_str(&format!("{pad}{{\n"));
    body.push_str(&format!(
        "{inner}\"pr_number\": {},\n",
        disposition.pr_number
    ));
    body.push_str(&format!(
        "{inner}\"disposition\": \"{}\",\n",
        json_escape(&disposition.disposition)
    ));
    body.push_str(&format!(
        "{inner}\"reason\": \"{}\",\n",
        json_escape(&disposition.reason)
    ));
    body.push_str(&format!(
        "{inner}\"recommended_action\": \"{}\"\n",
        json_escape(&disposition.recommended_action)
    ));
    body.push_str(&format!("{pad}}}"));
}

fn write_pr_triage_finding_json(body: &mut String, finding: &PrTriageFinding, indent: usize) {
    let pad = " ".repeat(indent);
    let inner = " ".repeat(indent + 2);
    body.push_str(&format!("{pad}{{\n"));
    body.push_str(&format!(
        "{inner}\"category\": \"{}\",\n",
        json_escape(&finding.category)
    ));
    body.push_str(&format!(
        "{inner}\"severity\": \"{}\",\n",
        json_escape(&finding.severity)
    ));
    body.push_str(&format!(
        "{inner}\"message\": \"{}\",\n",
        json_escape(&finding.message)
    ));
    body.push_str(&format!("{inner}\"prs\": "));
    write_json_u64_array(body, &finding.prs);
    body.push_str(&format!(",\n{inner}\"details\": ["));
    write_json_string_array(body, &finding.details);
    body.push_str(&format!(
        "],\n{inner}\"recommended_action\": \"{}\"\n",
        json_escape(&finding.recommended_action)
    ));
    body.push_str(&format!("{pad}}}"));
}

fn write_json_u64_array(body: &mut String, values: &[u64]) {
    body.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&value.to_string());
    }
    body.push(']');
}

fn pr_age_label(pr: &PrTriagePullRequest, today: i64) -> String {
    match pr_created_day(&pr.created_at) {
        Some(created) => format!("{}d", today - created),
        None => "unknown".to_string(),
    }
}

fn pr_checks_summary(checks: &[PrTriageCheck]) -> String {
    if checks.is_empty() {
        return "none".to_string();
    }
    let failed = checks.iter().filter(|check| pr_check_failed(check)).count();
    let pending = checks
        .iter()
        .filter(|check| pr_check_pending(check))
        .count();
    if failed > 0 {
        format!("{failed} failed, {pending} pending")
    } else if pending > 0 {
        format!("{pending} pending")
    } else {
        format!("{} passed", checks.len())
    }
}

fn format_pr_triage_ref(pr: &PrTriagePullRequest) -> String {
    format!("#{} {}", pr.number, pr.title)
}

fn markdown_escape_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

pub(crate) fn critic_impl() -> Result<(), String> {
    ensure_reports_dir()?;
    let changes = collect_pr_changes()?;
    let reports = report_index_entries()?;
    let receipts = receipt_index_entries()?;
    let findings = critic_findings(&changes, &reports, &receipts);
    write_report(
        "critic.md",
        &critic_markdown(&findings, &reports, &receipts),
    )?;
    write_report("critic.json", &critic_json(&findings, &reports, &receipts))
}

pub(crate) fn reports_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("index") => reports_index(),
        Some(other) => Err(format!(
            "unknown reports command `{other}`\nusage: cargo xtask reports index"
        )),
        None => Err("missing reports command\nusage: cargo xtask reports index".to_string()),
    }
}

pub(crate) fn receipts_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => receipts_write(),
        Some("check") => receipts_check(),
        Some(other) => Err(format!(
            "unknown receipts command `{other}`\nusage: cargo xtask receipts\n       cargo xtask receipts check"
        )),
    }
}

pub(crate) fn reports_index_impl() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let reports = report_index_entries()?;
    let receipts = receipt_index_entries()?;
    let missing = report_index_missing_expected(&reports, &changes);
    let lane1_packets = report_index_lane1_readiness_packets(&reports);
    let status = report_index_status(&reports, &missing, &[]);
    let next_commands = report_index_next_commands(&missing, &lane1_packets);

    let markdown = report_index_markdown(status, &reports, &receipts, &missing, &next_commands);
    let json = report_index_json(status, &reports, &receipts, &missing, &next_commands);
    write_report("index.md", &markdown)?;
    write_report("index.json", &json)
}

pub(crate) fn receipts_write_impl() -> Result<(), String> {
    ensure_receipts_dir()?;
    let git = receipt_git_metadata();
    let mut records = Vec::new();
    for spec in receipt_specs() {
        let reports = spec
            .reports
            .iter()
            .map(|report| format!("target/ripr/reports/{report}"))
            .collect::<Vec<_>>();
        let status = receipt_status_from_reports(&reports);
        let record = ReceiptRecord {
            file: spec.file.to_string(),
            command: spec.command.to_string(),
            status,
            reports,
        };
        write_receipt(spec.file, &receipt_json(&record, &git))?;
        records.push(record);
    }
    write_report(
        "receipts.md",
        &receipts_report_markdown("pass", &records, &[]),
    )
}

fn receipts_check() -> Result<(), String> {
    let violations = receipts_check_violations()?;
    let records = read_receipt_records();
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    write_report(
        "receipts.md",
        &receipts_report_markdown(status, &records, &violations),
    )?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err("receipt validation failed; see target/ripr/reports/receipts.md".to_string())
    }
}

fn receipt_specs() -> Vec<ReceiptSpec> {
    vec![
        ReceiptSpec {
            file: "shape.json",
            command: "cargo xtask shape",
            reports: &["shape.md"],
        },
        ReceiptSpec {
            file: "fix-pr.json",
            command: "cargo xtask fix-pr",
            reports: &["fix-pr.md", "shape.md", "pr-summary.md"],
        },
        ReceiptSpec {
            file: "ci-fast.json",
            command: "cargo xtask ci-fast",
            reports: &[
                "static-language.md",
                "no-panic-family.md",
                "allow-attributes.md",
                "local-context.md",
                "local-context.json",
                "file-policy.md",
                "executable-files.md",
                "workflows.md",
                "spec-format.md",
                "fixture-contracts.md",
                "traceability.md",
                "capabilities.md",
                "workspace-shape.md",
                "architecture.md",
                "public-api.md",
                "output-contracts.md",
                "doc-index.md",
                "readme-state.md",
                "markdown-links.md",
                "campaign.md",
                "pr-shape.md",
                "generated.md",
                "dependencies.md",
                "process-policy.md",
                "network-policy.md",
            ],
        },
        ReceiptSpec {
            file: "check-pr.json",
            command: "cargo xtask check-pr",
            reports: &["check-pr.md", "pr-summary.md"],
        },
        ReceiptSpec {
            file: "fixtures.json",
            command: "cargo xtask fixtures",
            reports: &["fixtures.md"],
        },
        ReceiptSpec {
            file: "goldens.json",
            command: "cargo xtask goldens check",
            reports: &["goldens.md"],
        },
        ReceiptSpec {
            file: "test-oracles.json",
            command: "cargo xtask test-oracle-report",
            reports: &["test-oracles.md", "test-oracles.json"],
        },
        ReceiptSpec {
            file: "badge-artifacts.json",
            command: "cargo xtask badge-artifacts",
            reports: &[
                "ripr-badge.json",
                "ripr-badge-shields.json",
                "ripr-plus-badge.json",
                "ripr-plus-badge-shields.json",
                "ripr-badges.md",
            ],
        },
        ReceiptSpec {
            file: "repo-badge-artifacts.json",
            command: "cargo xtask repo-badge-artifacts",
            reports: &[
                "repo-ripr-badge.json",
                "repo-ripr-badge-shields.json",
                "repo-ripr-plus-badge.json",
                "repo-ripr-plus-badge-shields.json",
                "repo-ripr-badges.md",
            ],
        },
        ReceiptSpec {
            file: "dogfood.json",
            command: "cargo xtask dogfood",
            reports: &["dogfood.md", "dogfood.json"],
        },
        ReceiptSpec {
            file: "metrics.json",
            command: "cargo xtask metrics",
            reports: &["metrics.md", "metrics.json"],
        },
    ]
}

fn receipt_status_from_reports(reports: &[String]) -> String {
    let mut saw_report = false;
    let mut saw_warn = false;
    for report in reports {
        let path = Path::new(report);
        if !path.exists() {
            continue;
        }
        saw_report = true;
        match report_entry_status(path).as_str() {
            "fail" | "failed" => return "failed".to_string(),
            "warn" | "warning" => saw_warn = true,
            _ => {}
        }
    }
    if !saw_report {
        "missing".to_string()
    } else if saw_warn {
        "warn".to_string()
    } else {
        "passed".to_string()
    }
}

fn receipt_git_metadata() -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    values.insert(
        "branch".to_string(),
        git_value(&["rev-parse", "--abbrev-ref", "HEAD"]),
    );
    values.insert("commit".to_string(), git_value(&["rev-parse", "HEAD"]));
    values
}

fn git_value(args: &[&str]) -> String {
    let value = run_output_optional("git", args).unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn receipt_json(record: &ReceiptRecord, git: &BTreeMap<String, String>) -> String {
    let branch = git.get("branch").map(String::as_str).unwrap_or("unknown");
    let commit = git.get("commit").map(String::as_str).unwrap_or("unknown");
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!(
        "  \"command\": \"{}\",\n",
        json_escape(&record.command)
    ));
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(&record.status)
    ));
    body.push_str("  \"duration_ms\": 0,\n");
    body.push_str("  \"git\": {\n");
    body.push_str(&format!("    \"branch\": \"{}\",\n", json_escape(branch)));
    body.push_str(&format!("    \"commit\": \"{}\"\n", json_escape(commit)));
    body.push_str("  },\n");
    body.push_str("  \"reports\": [");
    write_json_string_array(&mut body, &record.reports);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn receipts_check_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        if !path.exists() {
            violations.push(format!("missing receipt `{}`", normalize_path(&path)));
            continue;
        }
        let text = read_text_lossy(&path)?;
        if !text.contains("\"schema_version\": \"0.1\"") {
            violations.push(format!(
                "`{}` is missing schema_version 0.1",
                normalize_path(&path)
            ));
        }
        if !text.contains("\"command\"") {
            violations.push(format!("`{}` is missing command", normalize_path(&path)));
        }
        if !text.contains("\"status\"") {
            violations.push(format!("`{}` is missing status", normalize_path(&path)));
        }
        if !text.contains("\"git\"") {
            violations.push(format!(
                "`{}` is missing git metadata",
                normalize_path(&path)
            ));
        }
        if !text.contains("\"reports\"") {
            violations.push(format!(
                "`{}` is missing report paths",
                normalize_path(&path)
            ));
        }
        if let Some(status) = report_status_from_text(&text) {
            if !is_receipt_status(&status) {
                violations.push(format!(
                    "`{}` has unknown status `{status}`",
                    normalize_path(&path)
                ));
            }
        } else {
            violations.push(format!(
                "`{}` has no parseable status",
                normalize_path(&path)
            ));
        }
    }
    Ok(violations)
}

fn is_receipt_status(status: &str) -> bool {
    matches!(status, "passed" | "warn" | "failed" | "missing")
}

fn read_receipt_records() -> Vec<ReceiptRecord> {
    let mut records = Vec::new();
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        let status = if path.exists() {
            report_entry_status(&path)
        } else {
            "missing".to_string()
        };
        let reports = spec
            .reports
            .iter()
            .map(|report| format!("target/ripr/reports/{report}"))
            .collect::<Vec<_>>();
        records.push(ReceiptRecord {
            file: spec.file.to_string(),
            command: spec.command.to_string(),
            status,
            reports,
        });
    }
    records
}

fn receipts_report_markdown(
    status: &str,
    records: &[ReceiptRecord],
    violations: &[String],
) -> String {
    let mut body = format!("# ripr receipts report\n\nStatus: {status}\n\n");
    body.push_str("Receipts are machine-readable evidence for gate and report runs.\n\n");
    body.push_str("## Receipts\n\n");
    body.push_str("| Receipt | Command | Status |\n| --- | --- | --- |\n");
    for record in records {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` |\n",
            markdown_cell(&format!("target/ripr/receipts/{}", record.file)),
            markdown_cell(&record.command),
            markdown_cell(&record.status)
        ));
    }
    body.push_str("\n## Validation\n\n");
    if violations.is_empty() {
        body.push_str("- All required receipts are present and structurally valid.\n");
    } else {
        for violation in violations {
            body.push_str(&format!("- {violation}\n"));
        }
    }
    body
}

fn precommit_report_body() -> String {
    "# ripr precommit report\n\nStatus: pass\n\nChecks:\n\n- `cargo fmt --check`\n- `cargo xtask check-static-language`\n- `cargo xtask check-no-panic-family`\n- `cargo xtask check-allow-attributes`\n- `cargo xtask check-local-context`\n- `cargo xtask check-file-policy`\n- `cargo xtask check-executable-files`\n- `cargo xtask check-workflows`\n- `cargo xtask check-droid-review-config`\n- `cargo xtask check-spec-format`\n- `cargo xtask check-spec-numbering`\n- `cargo xtask check-fixture-contracts`\n- `cargo xtask check-traceability`\n- `cargo xtask check-capabilities`\n- `cargo xtask check-workspace-shape`\n- `cargo xtask check-architecture`\n- `cargo xtask check-public-api`\n- `cargo xtask check-output-contracts`\n- `cargo xtask check-doc-artifacts`\n- `cargo xtask check-doc-index`\n- `cargo xtask check-readme-state`\n- `cargo xtask markdown-links`\n- `cargo xtask check-pr-shape`\n- `cargo xtask check-command-catalog`\n- `cargo xtask check-generated`\n- `cargo xtask check-badge-diff-policy`\n- `cargo xtask check-generated-clean`\n- `cargo xtask check-proof-packs`\n- `cargo xtask check-release-targets`\n- `cargo xtask check-dependencies`\n- `cargo xtask check-process-policy`\n- `cargo xtask check-network-policy`\n- `cargo xtask check-lint-policy`\n\nNext command:\n\n```bash\ncargo xtask check-pr\n```\n".to_string()
}

/// Compose the check-pr report for either terminal state (#3036). One
/// authority owns the status line, checks inventory, and footer so the
/// success and failure artifacts cannot drift apart; a failure report names
/// the failed gate, its reproduce command, bounded error lines, and the
/// gates that were never run, never carries a `Status: pass` line, and
/// marks `pr-summary.md` as not refreshed because the failure path returns
/// before `pr_summary()` runs.
fn check_pr_report(failure: Option<&CheckPrGateFailure>) -> String {
    let status = if failure.is_some() { "fail" } else { "pass" };
    let mut body = format!("# ripr check-pr report\n\nStatus: {status}\n");
    if let Some(failure) = failure {
        let first_lines = if failure.bounded_error.is_empty() {
            "(gate produced no error output)"
        } else {
            failure.bounded_error.as_str()
        };
        body.push_str(&format!(
            "\n## First actionable failure\n\nGate: `{}`\nReproduce: `{}`\n\nFirst lines:\n  {first_lines}\n\nNot run after the first failure:\n",
            failure.name, failure.reproduce
        ));
        if failure.not_run.is_empty() {
            body.push_str("- (none — the failed gate was the last)\n");
        } else {
            for name in &failure.not_run {
                body.push_str(&format!("- `{name}`\n"));
            }
        }
    }
    let pr_summary_entry = if failure.is_some() {
        // The failure path returns before pr_summary() runs, so advertising
        // it without the marker would present a stale artifact as current.
        "- `target/ripr/reports/pr-summary.md` (not refreshed — the run stopped at the first failed gate)"
    } else {
        "- `target/ripr/reports/pr-summary.md`"
    };
    body.push_str(&format!("\nChecks:\n\n- `cargo xtask ci-fast`\n- `cargo clippy --workspace --all-targets -- -D warnings`\n- `cargo doc --workspace --no-deps`\n- `cargo xtask pr-summary`\n\nReports:\n\n{pr_summary_entry}\n- `target/ripr/reports/check-pr.md`\n\nRelease/package gates are intentionally left to `cargo xtask ci-full` or release-specific workflows.\n"));
    body
}

pub(crate) fn finish_policy_report(
    spec: PolicyReportSpec<'_>,
    violations: &[String],
) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# {}\n\nStatus: {status}\n\n", spec.check);
    body.push_str("## Why This Matters\n\n");
    body.push_str(spec.why_it_matters);
    body.push_str("\n\n");

    if violations.is_empty() {
        body.push_str("## Violations\n\nNone detected.\n\n");
    } else {
        body.push_str("## Violations\n\n");
        for violation in violations {
            body.push_str("```text\n");
            body.push_str(violation);
            body.push_str("\n```\n\n");
        }
    }

    if !violations.is_empty() {
        body.push_str("## Fix Kind\n\n```text\n");
        body.push_str(fix_kind_name(&spec.fix_kind));
        body.push_str("\n```\n\n");

        body.push_str("## Recommended Fixes\n\n");
        for (index, fix) in spec.recommended_fixes.iter().enumerate() {
            body.push_str(&format!("{}. {fix}\n", index + 1));
        }
        body.push('\n');

        if let Some(template) = spec.exception_template {
            body.push_str("## Exception Template\n\n```text\n");
            body.push_str(template);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Rerun\n\n```bash\n");
    body.push_str(spec.rerun_command);
    body.push_str("\n```\n");

    write_report(spec.report_file, &body)?;

    if violations.is_empty() {
        println!(
            "{}: pass (target/ripr/reports/{})",
            spec.check, spec.report_file
        );
        Ok(())
    } else {
        Err(format!(
            "{} failed; see target/ripr/reports/{}\n{}",
            spec.check,
            spec.report_file,
            violations.join("\n")
        ))
    }
}

fn fix_kind_name(fix_kind: &FixKind) -> &'static str {
    match fix_kind {
        FixKind::AutoFixable => "auto_fixable",
        FixKind::AuthorDecisionRequired => "author_decision_required",
        FixKind::ReviewerDecisionRequired => "reviewer_decision_required",
        FixKind::PolicyExceptionRequired => "policy_exception_required",
    }
}

fn check_static_language_impl() -> Result<(), String> {
    let report_spec = PolicyReportSpec {
        report_file: "static-language.md",
        check: "check-static-language",
        why_it_matters: "Static output must preserve the boundary between draft exposure evidence and real mutation results.",
        fix_kind: FixKind::ReviewerDecisionRequired,
        recommended_fixes: &[
            "Rewrite static product output to use the approved exposure vocabulary.",
            "For an innocent word in a comment, append `ripr-allow: static-language: <reason>` to that single line.",
            "If a whole file is explanatory documentation, add a reasoned `[[allow]]` entry to the static-language allowlist.",
        ],
        rerun_command: "cargo xtask check-static-language",
        exception_template: Some(
            ".ripr/static-language-allowlist.toml entry:\n[[allow]]\npath = \"path/to/file.md\"\nowner = \"team\"\nreason = \"why this file may quote prohibited vocabulary\"",
        ),
    };

    let allowed = match load_static_language_allowlist() {
        Ok(entries) => entries,
        Err(violations) => return finish_policy_report(report_spec, &violations),
    };
    let forbidden = forbidden_static_terms();
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !should_scan_static_language_path(&allowed, &normalized) {
            continue;
        }
        let text = read_text_lossy(&path)?;
        for (line_number, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            // A reasoned inline allow suppresses this line's forbidden terms.
            // It is finer-grained than a whole-file allowlist entry (smaller
            // bypass surface, reviewable in the diff) and must carry a reason.
            if line_has_static_language_inline_allow(&lower) {
                continue;
            }
            for term in &forbidden {
                if contains_word(&lower, term) {
                    violations.push(static_language_violation_message(
                        &normalized,
                        line_number + 1,
                        term,
                        line,
                    ));
                }
            }
        }
    }

    finish_policy_report(report_spec, &violations)
}

fn check_allow_attributes_impl() -> Result<(), String> {
    let allowlist = read_count_allowlist(".ripr/allow-attributes.txt")?;
    let guarded = guarded_allow_attribute_lints();
    let mut counts = BTreeMap::<(String, String), Vec<usize>>::new();

    for path in tracked_files()? {
        if !path.ends_with(".rs") {
            continue;
        }
        let file_path = Path::new(&path);
        if !file_path.exists() {
            continue;
        }
        let text = read_text_lossy(file_path)?;
        for (line, attribute) in guarded_allow_attributes_in_text(&text, &guarded) {
            counts
                .entry((path.clone(), attribute))
                .or_default()
                .push(line);
        }
    }

    let mut violations = Vec::new();
    for ((path, attribute), lines) in &counts {
        let allowed = allowlist
            .get(&(path.clone(), attribute.clone()))
            .copied()
            .unwrap_or(0);
        if lines.len() > allowed {
            violations.push(format!(
                "{path}:{} contains `{attribute}` {} time(s), allowed {allowed}\n  preferred: fix the lint or add a narrow allowlist entry with a reason",
                allow_attribute_line_summary(lines),
                lines.len()
            ));
        }
    }

    for ((path, attribute), allowed) in &allowlist {
        if !guarded.contains(attribute_lint_name(attribute).unwrap_or(attribute)) {
            violations.push(format!(
                ".ripr/allow-attributes.txt contains unsupported guarded attribute `{attribute}` for {path}; remove stale or out-of-scope exceptions"
            ));
            continue;
        }
        let actual = counts
            .get(&(path.clone(), attribute.clone()))
            .map(Vec::len)
            .unwrap_or(0);
        if actual > *allowed {
            violations.push(format!(
                "{path} contains `{attribute}` {actual} time(s), allowed {allowed}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "allow-attributes.md",
            check: "check-allow-attributes",
            why_it_matters: "Lint suppressions should not be used to hide repo guardrails. If a suppression is unavoidable, it needs a narrow reviewed exception with a reason.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove the lint suppression and fix the underlying warning.",
                "If the suppression is temporary and intentional, add a narrow allowlist entry with a reason.",
                "Do not allowlist panic-family, unsafe, or broad warning suppressions unless the PR explicitly owns that exception.",
            ],
            rerun_command: "cargo xtask check-allow-attributes",
            exception_template: Some(
                ".ripr/allow-attributes.txt entry:\npath/to/file.rs|allow(clippy::unwrap_used)|1|reason",
            ),
        },
        &violations,
    )
}

fn check_local_context_impl() -> Result<(), String> {
    let allowlist = read_local_context_allowlist("policy/local_context_allowlist.txt")?;
    let mut violations = validate_local_context_allowlist(&allowlist);
    let mut grouped = BTreeMap::<(String, String), (BTreeSet<String>, Vec<Option<usize>>)>::new();

    for path in tracked_files()? {
        let file_path = Path::new(&path);
        if !file_path.exists() || path == "policy/local_context_allowlist.txt" {
            continue;
        }

        for finding in local_context_findings_for_path(&path)? {
            let entry = grouped
                .entry((finding.path.clone(), finding.pattern.clone()))
                .or_insert_with(|| (BTreeSet::new(), Vec::new()));
            entry.0.insert(finding.problem);
            entry.1.push(finding.line);
        }
    }

    let allowed = allowlist
        .iter()
        .map(|entry| ((entry.path.clone(), entry.pattern.clone()), entry.max_count))
        .collect::<BTreeMap<_, _>>();

    for ((path, pattern), (problems, lines)) in &grouped {
        let actual = lines.len();
        let allowed_count = allowed
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if actual <= allowed_count && actual == 0 {
            continue;
        }
        if actual > allowed_count {
            let line_summary = local_context_line_summary(lines);
            violations.push(format!(
                "Path: {path}\nProblem: {}\nPattern: {pattern}\nCount: {actual}, allowed: {allowed_count}\nLines: {line_summary}\nWhy this matters: Repository docs should contain durable project state, not local runtime/session state from one machine or Codex run.\nRecommended fixes:\n1. Delete runtime/session artifacts instead of committing them.\n2. Move durable learnings to docs/LEARNINGS.md.\n3. Move generated state to target/ripr/reports, target/ripr/receipts, or target/ripr/learning.",
                problems.iter().cloned().collect::<Vec<_>>().join("; ")
            ));
        } else if actual < allowed_count && actual > 0 {
            // Stale bound: the allowlist grants more budget than the code uses
            // (#2413). Flag so the bound is tightened.
            violations.push(format!(
                "Path: {path}\nPattern: {pattern}\nStale allowlist bound: max_count={allowed_count} but actual={actual}; tighten to {actual}"
            ));
        }
    }

    // Flag orphaned allowlist entries whose path/pattern no longer appears in
    // any tracked file (count dropped to zero or the file was removed).
    for entry in &allowlist {
        let key = (entry.path.clone(), entry.pattern.clone());
        if !grouped.contains_key(&key) && entry.max_count > 0 {
            violations.push(format!(
                "Path: {}\nPattern: {}\nOrphaned allowlist entry: max_count={} but pattern not found in any tracked file; remove the entry",
                entry.path, entry.pattern, entry.max_count
            ));
        }
    }

    write_local_context_json(&violations)?;
    finish_policy_report(
        PolicyReportSpec {
            report_file: "local-context.md",
            check: "check-local-context",
            why_it_matters: "Repository state must be durable and portable. Machine paths, Codex memory paths, sandbox references, local transcripts, and session-state documents belong in generated artifacts or local notes, not committed repo knowledge.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Delete committed runtime/session artifacts.",
                "Move durable repo knowledge to docs/LEARNINGS.md or campaign/capability metadata.",
                "Move generated state to target/ripr/reports, target/ripr/receipts, or target/ripr/learning.",
                "Use policy/local_context_allowlist.txt only for narrow generic examples with a reason.",
            ],
            rerun_command: "cargo xtask check-local-context",
            exception_template: Some(
                "policy/local_context_allowlist.txt entry:\npath|pattern|max_count|reason",
            ),
        },
        &violations,
    )
}

pub(crate) fn rust_conversion_candidates() -> Result<(), String> {
    let candidates = collect_rust_conversion_candidates()?;
    write_report(
        "rust-conversion-candidates.md",
        &rust_conversion_candidates_markdown(&candidates),
    )?;
    write_report(
        "rust-conversion-candidates.json",
        &rust_conversion_candidates_json(&candidates)?,
    )
}

fn collect_rust_conversion_candidates() -> Result<Vec<RustConversionCandidate>, String> {
    let mut candidates = Vec::new();

    for path in tracked_files()? {
        if is_non_rust_programming_candidate(&path) {
            candidates.extend(non_rust_source_conversion_candidate(&path));
        }
    }

    for path in tracked_files()?
        .into_iter()
        .filter(|path| path.starts_with(".github/workflows/"))
        .filter(|path| path.ends_with(".yml") || path.ends_with(".yaml"))
    {
        let text = read_text_lossy(Path::new(&path))?;
        for block in extract_workflow_run_blocks(&text) {
            if let Some(candidate) = workflow_run_conversion_candidate(&path, &block) {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|left, right| {
        conversion_priority_rank(&left.priority)
            .cmp(&conversion_priority_rank(&right.priority))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    Ok(candidates)
}

fn non_rust_source_conversion_candidate(path: &str) -> Option<RustConversionCandidate> {
    if path.starts_with("editors/vscode/") && path.ends_with(".ts") {
        return Some(RustConversionCandidate {
            path: path.to_string(),
            line: None,
            kind: "retained_external_runtime".to_string(),
            priority: "retained".to_string(),
            current_surface: "VS Code extension TypeScript".to_string(),
            recommendation: "Keep this code in the editor adapter; only move server behavior into ripr Rust modules or xtask.".to_string(),
            reason: "The VS Code Extension Host API is TypeScript-native, so this is an approved adapter boundary rather than core automation.".to_string(),
        });
    }

    if path.starts_with("fixtures/") {
        return Some(RustConversionCandidate {
            path: path.to_string(),
            line: None,
            kind: "retained_fixture_input".to_string(),
            priority: "retained".to_string(),
            current_surface: "fixture workspace input".to_string(),
            recommendation: "Keep as fixture input unless the fixture no longer maps to a spec; move reusable fixture orchestration into Rust/xtask instead.".to_string(),
            reason: "Fixture workspaces are analyzed inputs for preview language adapters, not repository automation.".to_string(),
        });
    }

    Some(RustConversionCandidate {
        path: path.to_string(),
        line: None,
        kind: "non_rust_programming_without_retention_rule".to_string(),
        priority: "high".to_string(),
        current_surface: "unapproved non-Rust implementation or automation".to_string(),
        recommendation: "Move this behavior into Rust under crates/ripr for product logic or xtask for repository automation.".to_string(),
        reason: "Rust is the default implementation surface, and this file has no approved external-runtime or fixture retention rule.".to_string(),
    })
}

fn workflow_run_conversion_candidate(
    path: &str,
    block: &RunBlock,
) -> Option<RustConversionCandidate> {
    let text = block.text.trim();
    if text.is_empty() || text.starts_with("cargo xtask ") {
        return None;
    }
    if workflow_run_is_external_runtime(text) {
        return Some(RustConversionCandidate {
            path: path.to_string(),
            line: Some(block.line_number),
            kind: "retained_external_runtime".to_string(),
            priority: "retained".to_string(),
            current_surface: "workflow command for an external toolchain".to_string(),
            recommendation: "Keep as a direct workflow call unless repo-owned report assembly grows around it; put repo-owned assembly into xtask.".to_string(),
            reason: "The command delegates to Cargo, npm, Codecov, or another external tool rather than implementing repo policy in shell.".to_string(),
        });
    }

    if workflow_run_contains_shell_logic(text) {
        return Some(RustConversionCandidate {
            path: path.to_string(),
            line: Some(block.line_number),
            kind: "workflow_shell_logic".to_string(),
            priority: "medium".to_string(),
            current_surface: "GitHub Actions shell run block".to_string(),
            recommendation: "Move repo-owned file/report/summary assembly into a focused cargo xtask command, then leave this workflow step as a short cargo xtask invocation.".to_string(),
            reason: "Workflow shell is harder to type-check and test than Rust/xtask, especially when it creates local reports or step summaries.".to_string(),
        });
    }

    None
}

fn workflow_run_is_external_runtime(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("cargo ")
        || trimmed.starts_with("npm ")
        || trimmed.starts_with("code ")
        || trimmed.starts_with("npx ")
        || trimmed.starts_with("codecov")
}

fn workflow_run_contains_shell_logic(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let markers = [
        "mkdir ",
        "printf ",
        "echo ",
        "git diff",
        "ls ",
        "tee ",
        "cat ",
        "if ",
        "fi",
        "for ",
        "while ",
        "<<",
        "&&",
        "||",
        "$github_step_summary",
        "$github_env",
    ];
    text.lines().count() > 1 || markers.iter().any(|marker| lower.contains(marker))
}

fn conversion_priority_rank(priority: &str) -> u8 {
    match priority {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        "retained" => 3,
        _ => 4,
    }
}

fn rust_conversion_candidates_markdown(candidates: &[RustConversionCandidate]) -> String {
    let actionable = candidates
        .iter()
        .filter(|candidate| candidate.priority != "retained")
        .count();
    let retained = candidates.len().saturating_sub(actionable);
    let mut body = format!(
        "# ripr Rust conversion candidates\n\nStatus: advisory\n\nActionable candidates: {actionable}\nRetained boundaries inspected: {retained}\n\nThis report keeps the Rust-first policy actionable without treating fixture inputs or editor adapter code as core implementation debt.\n\n"
    );

    body.push_str("## Actionable candidates\n\n");
    if actionable == 0 {
        body.push_str("No unretained non-Rust source files or workflow shell migration candidates were found.\n");
    } else {
        body.push_str("| Priority | Path | Line | Kind | Recommendation | Reason |\n");
        body.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.priority != "retained")
        {
            push_rust_conversion_candidate_row(&mut body, candidate);
        }
    }

    body.push_str("\n## Retained boundaries\n\n");
    if retained == 0 {
        body.push_str("No retained non-Rust runtime or fixture boundaries were found.\n");
    } else {
        body.push_str("| Path | Line | Surface | Recommendation | Reason |\n");
        body.push_str("| --- | --- | --- | --- | --- |\n");
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.priority == "retained")
        {
            let line = candidate
                .line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "-".to_string());
            body.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                markdown_cell(&candidate.path),
                line,
                markdown_cell(&candidate.current_surface),
                markdown_cell(&candidate.recommendation),
                markdown_cell(&candidate.reason)
            ));
        }
    }

    body.push_str("\nNext command:\n\n```bash\ncargo xtask check-file-policy\n```\n");
    body
}

fn push_rust_conversion_candidate_row(body: &mut String, candidate: &RustConversionCandidate) {
    let line = candidate
        .line
        .map(|line| line.to_string())
        .unwrap_or_else(|| "-".to_string());
    body.push_str(&format!(
        "| {} | `{}` | {} | {} | {} | {} |\n",
        markdown_cell(&candidate.priority),
        markdown_cell(&candidate.path),
        line,
        markdown_cell(&candidate.kind),
        markdown_cell(&candidate.recommendation),
        markdown_cell(&candidate.reason)
    ));
}

fn rust_conversion_candidates_json(
    candidates: &[RustConversionCandidate],
) -> Result<String, String> {
    let items = candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "path": candidate.path,
                "line": candidate.line,
                "kind": candidate.kind,
                "priority": candidate.priority,
                "current_surface": candidate.current_surface,
                "recommendation": candidate.recommendation,
                "reason": candidate.reason,
            })
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema_version": 1,
        "actionable_count": candidates.iter().filter(|candidate| candidate.priority != "retained").count(),
        "retained_count": candidates.iter().filter(|candidate| candidate.priority == "retained").count(),
        "candidates": items,
    });
    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to serialize rust-conversion-candidates report: {err}"))
}

fn check_executable_files_impl() -> Result<(), String> {
    let allowlist = read_path_allowlist_optional("policy/executable_allowlist.txt")?;
    let output = run_output("git", &["ls-files", "--stage"])?;
    let mut violations = Vec::new();

    for line in output.lines() {
        let Some((mode, path)) = parse_git_stage_line(line) else {
            continue;
        };
        let normalized = normalize_slashes(path);
        if mode == "100755" && !allowlist.contains(&normalized) {
            violations.push(format!(
                "checked-in executable file is not allowlisted: {normalized}\n  preferred: use cargo xtask instead of executable scripts"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "executable-files.md",
            check: "check-executable-files",
            why_it_matters: "Checked-in executable scripts make automation drift away from the Rust-first xtask surface.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove the executable bit from ordinary files.",
                "Move script behavior into xtask.",
                "If an executable file is truly required, add a reviewed allowlist entry.",
            ],
            rerun_command: "cargo xtask check-executable-files",
            exception_template: Some("policy/executable_allowlist.txt entry:\npath/to/file"),
        },
        &violations,
    )
}

fn check_workflows_impl() -> Result<(), String> {
    let budgets = read_workflow_budgets("policy/workflow_allowlist.txt")?;
    let runtime_allowlist = read_count_allowlist("policy/workflow_action_runtime_allowlist.txt")?;
    let mut violations = Vec::new();

    for path in collect_files(Path::new(".github/workflows"))? {
        let normalized = normalize_path(&path);
        if !(normalized.ends_with(".yml") || normalized.ends_with(".yaml")) {
            continue;
        }
        let Some(budget) = budgets.get(&normalized) else {
            violations.push(format!(
                "missing workflow budget for {normalized} in policy/workflow_allowlist.txt"
            ));
            continue;
        };
        let text = read_text_lossy(&path)?;
        violations.extend(workflow_runtime_violations(
            &normalized,
            &text,
            &runtime_allowlist,
        ));
        violations.extend(workflow_review_thread_mutation_violations(
            &normalized,
            &text,
        ));
        violations.extend(workflow_bare_self_hosted_violations(&normalized, &text));
        violations.extend(workflow_plain_scalar_comment_violations(&normalized, &text));
        for block in extract_workflow_run_blocks(&text) {
            if block.non_empty_lines > budget.max_non_empty_lines {
                violations.push(format!(
                    "{normalized}:{} run block has {} non-empty line(s), allowed {} ({})",
                    block.line_number,
                    block.non_empty_lines,
                    budget.max_non_empty_lines,
                    budget.reason
                ));
            }
            let lower = block.text.to_ascii_lowercase();
            if lower.contains(shell_fetch_tool_name()) && lower.contains("| sh") {
                violations.push(format!(
                    "{normalized}:{} run block contains network fetch piped to sh",
                    block.line_number
                ));
            }
            if lower.contains(shell_fetch_tool_name()) && lower.contains("| bash") {
                violations.push(format!(
                    "{normalized}:{} run block contains network fetch piped to bash",
                    block.line_number
                ));
            }
        }
    }
    violations.extend(repository_owned_review_thread_mutation_violations()?);
    validate_assistant_loop_health_fixture_corpus(&mut violations)?;
    violations.extend(routed_rust_workflow_contract_violations_for_repo()?);

    finish_policy_report(
        PolicyReportSpec {
            report_file: "workflows.md",
            check: "check-workflows",
            why_it_matters: "GitHub Actions should orchestrate xtask, Cargo, and npm commands instead of hiding complex shell logic in workflow YAML.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move complex workflow logic into xtask or an npm script owned by the extension surface.",
                "Keep workflow run blocks under the documented line budget.",
                "Use Node-24-backed action majors where official releases exist.",
                "Use Node 24 for VS Code extension build and publish workflows.",
                "Add or adjust a workflow budget entry only when the workflow surface is intentionally larger.",
            ],
            rerun_command: "cargo xtask check-workflows",
            exception_template: Some(
                "policy/workflow_allowlist.txt entry:\n.github/workflows/name.yml|max_non_empty_lines|reason\n\npolicy/workflow_action_runtime_allowlist.txt entry:\n.github/workflows/name.yml|action/ref|max_count|reason",
            ),
        },
        &violations,
    )
}

/// Workflow automation must not perform review-thread resolution without adjudication.
///
/// A workflow-side review-thread mutation turns provider failure or an
/// unreviewed finding into an apparently resolved conversation. Review-thread
/// resolution remains an explicit, evidence-backed operator action; the policy
/// rejects common GraphQL/name variants so a renamed blind resolver cannot
/// re-enter unnoticed. Repository-owned delegated automation is scanned too,
/// so a local composite action or xtask helper cannot hide the mutation.
fn workflow_review_thread_mutation_violations(path: &str, text: &str) -> Vec<String> {
    if text.lines().any(review_thread_mutation_line) {
        return vec![format!(
            "{path}: workflow contains a review-thread resolution mutation; review-thread resolution requires explicit adjudication outside automated workflow mutation"
        )];
    }
    Vec::new()
}

fn review_thread_mutation_line(line: &str) -> bool {
    let normalized = line
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    normalized.contains(&review_thread_mutation_token())
}

fn review_thread_mutation_token() -> String {
    [
        'r', 'e', 's', 'o', 'l', 'v', 'e', 'r', 'e', 'v', 'i', 'e', 'w', 't', 'h', 'r', 'e', 'a',
        'd',
    ]
    .into_iter()
    .collect()
}

fn repository_owned_review_thread_mutation_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();

    for root in [
        Path::new(".github/actions"),
        Path::new(".github/scripts"),
        Path::new("scripts"),
        Path::new("tools"),
        Path::new("xtask/src"),
    ] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            let normalized = normalize_path(&path);
            if normalized.ends_with("xtask/src/tests.rs") {
                continue;
            }
            let text = read_text_lossy(&path)?;
            if text.lines().any(review_thread_mutation_line) {
                violations.push(format!(
                    "{normalized}: repository-owned automation contains a review-thread resolution mutation; review-thread resolution requires explicit adjudication outside automated workflow mutation"
                ));
            }
        }
    }

    Ok(violations)
}

/// Flag a `run:` written as a plain YAML scalar that contains ` #`.
///
/// YAML treats ` #` in a plain scalar as the start of a comment, so the command
/// is silently truncated at that point. When the truncated remainder holds an
/// unterminated quote the shell fails with `unexpected EOF`, and when it does
/// not the step runs a *different, shorter* command with no error at all — the
/// worse outcome. A block scalar (`run: |`) has no comment rule and is immune.
///
/// This is enforced because it actually happened: a `printf` summary line
/// containing an issue reference was cut mid-string, and nothing local caught it
/// — `check-workflows` passed because it never parsed the YAML.
fn workflow_plain_scalar_comment_violations(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some((_, after)) = line.split_once("run:") else {
            continue;
        };
        // Only plain scalars are affected; `run: |` and `run: >` are safe, and a
        // fully quoted scalar carries its own delimiters.
        let body = after.trim_start();
        if body.starts_with('|') || body.starts_with('>') || body.is_empty() {
            continue;
        }
        if body.starts_with('"') || body.starts_with('\'') {
            continue;
        }
        if body.contains(" #") {
            violations.push(format!(
                "{path}:{} plain-scalar `run:` contains ` #`, which YAML reads as a comment and truncates the command; use a `run: |` block scalar",
                index + 1
            ));
        }
    }
    violations
}

fn workflow_bare_self_hosted_violations(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("runs-on:")
            && lower.contains('[')
            && lower.contains("self-hosted")
            && lower.contains("linux")
            && lower.contains("x64")
        {
            violations.push(format!(
                "{path}:{} bare inline self-hosted/linux/x64 runs-on is forbidden; use explicit group and capacity labels",
                index + 1
            ));
        }

        if line.trim() != "- self-hosted" {
            continue;
        }
        let end = (index + 17).min(lines.len());
        let start = index.saturating_sub(8);
        let window = &lines[start..end];
        let has_linux = window.iter().any(|candidate| candidate.trim() == "- linux");
        let has_x64 = window.iter().any(|candidate| candidate.trim() == "- x64");
        let has_group = window
            .iter()
            .any(|candidate| candidate.trim_start().starts_with("group: em-ci-"));
        let has_capacity = window.iter().any(|candidate| {
            matches!(
                candidate.trim(),
                "- em-ci"
                    | "- ci-nano"
                    | "- policy-nano"
                    | "- workflow-nano"
                    | "- rust-tiny"
                    | "- rust-medium"
                    | "- rust-large"
                    | "- rust-16gb"
                    | "- cx23"
                    | "- cx33"
                    | "- cx43"
                    | "- cx53"
                    | "- cpx42"
            )
        });
        if has_linux && has_x64 && !(has_group && has_capacity) {
            violations.push(format!(
                "{path}:{} bare self-hosted block lacks group/capacity labels",
                index + 1
            ));
        }
    }
    violations
}

fn routed_rust_workflow_contract_violations_for_repo() -> Result<Vec<String>, String> {
    let workflow_path = Path::new(".github/workflows/routed-rust.yml");
    if !workflow_path.exists() {
        return Ok(Vec::new());
    }

    let workflow = read_text_lossy(workflow_path)?;
    let settings = optional_policy_text(".github/settings.yml")?;
    let lane_whitelist = optional_policy_text("policy/ci-lane-whitelist.toml")?;

    Ok(routed_rust_workflow_contract_violations(
        &workflow,
        settings.as_deref(),
        lane_whitelist.as_deref(),
    ))
}

fn optional_policy_text(path: &str) -> Result<Option<String>, String> {
    let path = Path::new(path);
    if path.exists() {
        read_text_lossy(path).map(Some)
    } else {
        Ok(None)
    }
}

/// Routed-rust jobs that must each carry an explicit `timeout-minutes`
/// deadline (issue #2230).
const ROUTED_RUST_DEADLINE_JOBS: [&str; 8] = [
    "route",
    "detect-docs-only",
    "rust-cx43",
    "rust-cpx42",
    "rust-cx53",
    "rust-github",
    "docs-gate",
    "result",
];

/// Whether the named job block in a workflow text sets `timeout-minutes`.
/// Job keys are exactly two-space-indented `name:` lines under `jobs:`;
/// anything deeper belongs to the current block.
fn routed_rust_job_block_has_deadline(workflow: &str, job: &str) -> bool {
    let job_header = format!("{job}:");
    let mut in_block = false;
    for line in workflow.lines() {
        let job_level_key = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('-');
        if job_level_key {
            in_block = line.trim() == job_header;
            continue;
        }
        if in_block && !line.trim_start().starts_with('#') && line.contains("timeout-minutes:") {
            return true;
        }
    }
    false
}

fn routed_rust_workflow_contract_violations(
    workflow: &str,
    settings: Option<&str>,
    lane_whitelist: Option<&str>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let required_workflow_snippets = [
        (
            "org runner discovery",
            "orgs/EffortlessMetrics/actions/runners",
        ),
        (
            "runner read token fallback",
            "secrets.EM_RUNNER_READ_TOKEN || github.token",
        ),
        ("slurped idle runner query", "jq -s -e --arg model"),
        ("trusted fork fallback reason", "fork_or_untrusted_pr"),
        ("runner API fallback reason", "runner_api_failed"),
        ("no-idle fallback reason", "no_idle_runner"),
        (
            "runner capacity fallback reason",
            "runner_capacity_unavailable",
        ),
        ("CX43 idle route reason", "cx43_idle"),
        ("CPX42 idle route reason", "cpx42_idle"),
        ("CX53 idle route reason", "cx53_idle"),
        ("CX43 capacity label", "rust-medium"),
        ("CPX42 capacity label", "rust-16gb"),
        ("CX53 capacity label", "rust-large"),
        ("normalized result job", "name: Ripr Rust Small Result"),
        (
            "CX43 conditional implementation job",
            "if: needs.route.outputs.router_target == 'cx43'",
        ),
        (
            "CPX42 conditional implementation job",
            "if: needs.route.outputs.router_target == 'cpx42'",
        ),
        (
            "CX53 conditional implementation job",
            "if: needs.route.outputs.router_target == 'cx53'",
        ),
        (
            "hosted fallback conditional job",
            "needs.route.outputs.router_target == 'github'",
        ),
        (
            "hosted fallback docs-detection guard",
            "needs.detect-docs-only.result == 'success'",
        ),
        (
            "self-hosted scratch tempfail output",
            "scratch_status: ${{ steps.scratch.outputs.status }}",
        ),
        (
            "CX43 tempfail fallback predicate",
            "needs.rust-cx43.outputs.scratch_status == 'tempfail'",
        ),
        (
            "CPX42 tempfail fallback predicate",
            "needs.rust-cpx42.outputs.scratch_status == 'tempfail'",
        ),
        (
            "CX53 tempfail fallback predicate",
            "needs.rust-cx53.outputs.scratch_status == 'tempfail'",
        ),
        (
            "normalized tempfail fallback result",
            "disk-guard tempfailed; GitHub-hosted fallback succeeded",
        ),
        (
            "normalized docs detection failure",
            "docs-surface detection result was $DOCS_DETECT_RESULT",
        ),
        (
            "CX43 scratch free-space floor",
            "ci-disk-guard /mnt/ci-scratch 35",
        ),
        (
            "CPX42 scratch free-space floor",
            "ci-disk-guard /mnt/ci-scratch 35",
        ),
        (
            "CX53 scratch free-space floor",
            "ci-disk-guard /mnt/ci-scratch 50",
        ),
    ];

    for (label, snippet) in required_workflow_snippets {
        if !workflow.contains(snippet) {
            violations.push(format!(
                ".github/workflows/routed-rust.yml is missing {label}: `{snippet}`"
            ));
        }
    }

    let toolchain_temp_steps = workflow.matches("name: Prepare toolchain temp").count();
    let toolchain_temp_mkdirs = workflow.matches("run: mkdir -p \"$TMPDIR\"").count();
    if toolchain_temp_steps < 3 || toolchain_temp_mkdirs < 3 {
        violations.push(format!(
            ".github/workflows/routed-rust.yml must include `Prepare toolchain temp` before setup for all three self-hosted implementation jobs; found {toolchain_temp_steps} step(s) and {toolchain_temp_mkdirs} mkdir command(s)"
        ));
    }

    let scratch_cargo_home =
        "CARGO_HOME: /mnt/ci-scratch/cargo-home/${{ github.run_id }}-${{ github.run_attempt }}";
    let scratch_cargo_homes = workflow.matches(scratch_cargo_home).count();
    let scratch_cargo_home_cleanups = workflow
        .matches("rm -rf \"$CARGO_HOME\" \"$CARGO_TARGET_DIR\" \"$TMPDIR\"")
        .count();
    if scratch_cargo_homes < 3 || scratch_cargo_home_cleanups < 3 {
        violations.push(format!(
            ".github/workflows/routed-rust.yml must use scratch CARGO_HOME and clean it for all three self-hosted implementation jobs; found {scratch_cargo_homes} scratch home(s) and {scratch_cargo_home_cleanups} cleanup command(s)"
        ));
    }

    // Proof-routing slice 6 (docs/PROOF_ROUTING.md): every PR-evidence path must
    // emit the proof route as an advisory dry-run artifact. The command is
    // appended with `|| true` so a route-computation failure never fails the
    // lane, and it runs on all three self-hosted jobs and the hosted fallback so
    // the artifact cannot silently regress. No lane is skipped or gated by it.
    let proof_route_dry_runs = workflow
        .matches("cargo xtask proof route --base \"$BASE_SHA\" --head \"$HEAD_SHA\" || true")
        .count();
    if proof_route_dry_runs < 4 {
        violations.push(format!(
            ".github/workflows/routed-rust.yml must emit the advisory proof-route dry-run artifact (`cargo xtask proof route --base \"$BASE_SHA\" --head \"$HEAD_SHA\" || true`) on the PR-evidence path of all three self-hosted jobs and the hosted fallback; found {proof_route_dry_runs} occurrence(s)"
        ));
    }

    // Issue #2230 (PR #2228 hang): every routed-rust job carries an explicit
    // job deadline so a hung step fails the job in bounded time instead of
    // holding the required aggregate check open indefinitely. The check is
    // anchored to each named job block, not a global occurrence count: a
    // duplicate or stray `timeout-minutes:` token elsewhere cannot stand in
    // for a job that lost its deadline.
    for job in ROUTED_RUST_DEADLINE_JOBS {
        if !routed_rust_job_block_has_deadline(workflow, job) {
            violations.push(format!(
                ".github/workflows/routed-rust.yml job `{job}` must set an explicit `timeout-minutes` job deadline so a hung step fails in bounded time"
            ));
        }
    }

    if workflow.contains("repos/${REPOSITORY}/actions/runners")
        || workflow.contains("repos/$REPOSITORY/actions/runners")
        || workflow.contains("repos/EffortlessMetrics/ripr-swarm/actions/runners")
    {
        violations.push(
            ".github/workflows/routed-rust.yml must use organization runner discovery, not repo-local runner discovery".to_string(),
        );
    }

    if !(workflow.contains("[ \"$EVENT_NAME\" = \"pull_request\" ]")
        && workflow.contains("[ \"$HEAD_REPO\" != \"$REPOSITORY\" ]"))
    {
        violations.push(
            ".github/workflows/routed-rust.yml must guard pull_request events from forks before selecting self-hosted runners".to_string(),
        );
    }

    for forbidden in [
        "github.event.pull_request.head.repo.full_name == github.repository",
        "github.event.pull_request.head.repo.full_name != github.repository",
    ] {
        if workflow.contains(forbidden) {
            violations.push(format!(
                ".github/workflows/routed-rust.yml must keep fork routing in the route job, not on self-hosted implementation job condition `{forbidden}`"
            ));
        }
    }

    if let Some(settings) = settings
        && (settings.contains("name: ripr-swarm") || settings.contains("Ripr Rust Small Result"))
    {
        if !settings.contains("Ripr Rust Small Result") {
            violations.push(
                ".github/settings.yml must require the normalized `Ripr Rust Small Result` check for ripr-swarm".to_string(),
            );
        }
        for forbidden in [
            "Route Ripr Rust Small",
            "Ripr Rust Small on CX43",
            "Ripr Rust Small on CPX42",
            "Ripr Rust Small on CX53",
            "Ripr Rust Small on GitHub Hosted",
        ] {
            if settings.contains(forbidden) {
                violations.push(format!(
                    ".github/settings.yml must not require conditional implementation job `{forbidden}`"
                ));
            }
        }
    }

    if let Some(lane_whitelist) = lane_whitelist
        && lane_whitelist.contains("routed-rust-small")
    {
        if !lane_whitelist.contains("workflow = \".github/workflows/routed-rust.yml\"") {
            violations.push(
                "policy/ci-lane-whitelist.toml must point `routed-rust-small` at `.github/workflows/routed-rust.yml`".to_string(),
            );
        }
        if !lane_whitelist.contains("jobs = [\"Ripr Rust Small Result\"]") {
            violations.push(
                "policy/ci-lane-whitelist.toml must list only `Ripr Rust Small Result` for the routed Rust lane".to_string(),
            );
        }
    }

    violations.sort();
    violations.dedup();
    violations
}

fn workflow_runtime_violations(
    path: &str,
    text: &str,
    allowlist: &BTreeMap<(String, String), usize>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (old_ref, new_ref) in deprecated_workflow_action_refs() {
        let count = text.matches(old_ref).count();
        if count > 0 {
            violations.push(format!(
                "{path} uses deprecated action runtime ref `{old_ref}` {count} time(s); use `{new_ref}`"
            ));
        }
    }

    if is_extension_node_workflow(path) {
        for (line_number, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if matches!(
                trimmed,
                "node-version: 20" | "node-version: '20'" | "node-version: \"20\""
            ) {
                violations.push(format!(
                    "{path}:{} uses Node 20 for extension tooling; use Node 24",
                    line_number + 1
                ));
            }
        }
    }

    for pattern in workflow_runtime_exception_patterns() {
        let count = text.matches(pattern).count();
        if count == 0 {
            continue;
        }
        let allowed = allowlist
            .get(&(path.to_string(), pattern.to_string()))
            .copied()
            .unwrap_or(0);
        if count > allowed {
            violations.push(format!(
                "{path} uses `{pattern}` {count} time(s), allowed {allowed}; add a reviewed workflow action runtime exception or upgrade the action"
            ));
        }
    }

    for ((allowed_path, pattern), allowed) in allowlist {
        if allowed_path != path {
            continue;
        }
        if !workflow_runtime_exception_patterns().contains(&pattern.as_str()) {
            violations.push(format!(
                "policy/workflow_action_runtime_allowlist.txt has unsupported exception `{pattern}` for {allowed_path}"
            ));
            continue;
        }
        let count = text.matches(pattern).count();
        if count > *allowed {
            violations.push(format!(
                "{path} uses `{pattern}` {count} time(s), allowed {allowed}"
            ));
        }
    }

    violations.sort();
    violations.dedup();
    violations
}

fn deprecated_workflow_action_refs() -> &'static [(&'static str, &'static str)] {
    &[
        ("actions/checkout@v4", "actions/checkout@v6"),
        ("actions/setup-node@v4", "actions/setup-node@v6"),
        ("actions/upload-artifact@v4", "actions/upload-artifact@v7"),
        (
            "actions/download-artifact@v4",
            "actions/download-artifact@v8",
        ),
        ("codecov/codecov-action@v4", "codecov/codecov-action@v6"),
    ]
}

fn workflow_runtime_exception_patterns() -> &'static [&'static str] {
    &["actions/dependency-review-action@v4"]
}

fn is_extension_node_workflow(path: &str) -> bool {
    matches!(
        path,
        ".github/workflows/ci.yml" | ".github/workflows/publish-extension.yml"
    )
}

fn check_spec_format() -> Result<(), String> {
    let mut violations = Vec::new();
    let spec_dir = Path::new("docs/specs");
    for path in collect_files(spec_dir)? {
        let normalized = normalize_path(&path);
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with("RIPR-SPEC-") || !file_name.ends_with(".md") {
            continue;
        }
        let Some(spec_id) = spec_id_from_file_name(file_name) else {
            violations.push(format!("{normalized} has invalid RIPR-SPEC filename"));
            continue;
        };
        let text = read_text_lossy(&path)?;
        let first_line = text.lines().next().unwrap_or_default();
        if !first_line.starts_with(&format!("# {spec_id}: ")) {
            violations.push(format!(
                "{normalized}:1 title must start with `# {spec_id}: `"
            ));
        }
        let status = spec_status(&text);
        match status.as_deref() {
            Some("proposed" | "planned" | "accepted" | "deprecated") => {}
            Some(value) => violations.push(format!("{normalized} has invalid status `{value}`")),
            None => violations.push(format!("{normalized} is missing `Status: ...`")),
        }
        // #2708: flag proposed specs older than 90 days without review.
        // Accepted/deprecated specs are exempt — they have been reviewed.
        // #3035: age evidence comes from Git history, never filesystem mtime.
        if status.as_deref() == Some("proposed")
            && let Some(finding) = proposed_spec_age_finding(&normalized)
        {
            violations.push(finding);
        }
        for heading in required_spec_headings() {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{normalized} is missing `{heading}`"));
            }
        }
        if text.contains("- \n") {
            violations.push(format!(
                "{normalized} contains empty placeholder list items"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "spec-format.md",
            check: "check-spec-format",
            why_it_matters: "Specs are the behavior contracts that let humans and agents trace intent to tests, code, outputs, and metrics.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update the spec to match docs/SPEC_FORMAT.md.",
                "Use docs/templates/SPEC_TEMPLATE.md for new behavior specs.",
                "Keep planned specs explicit when implementation mapping is not available yet.",
            ],
            rerun_command: "cargo xtask check-spec-format",
            exception_template: None,
        },
        &violations,
    )
}

/// Maximum age of a `Status: proposed` spec before the lifecycle gate fires
/// (#2708).
const PROPOSED_SPEC_MAX_AGE_SECS: u64 = 90 * 24 * 60 * 60;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
/// Tolerated future-dating of the last spec-changing commit before the age
/// evidence is rejected as not_proven (#3035 review): ordinary clock skew
/// between committer and runner stays within a day.
const PROPOSED_SPEC_CLOCK_SKEW_SECS: u64 = SECONDS_PER_DAY;

/// #3035: proposed-spec review age is judged on repository evidence, never on
/// filesystem mtime. Git does not preserve tracked-file mtime across
/// clone/checkout, so the mtime-based check made an old proposed spec look new
/// exactly where the gate matters (fresh CI checkout). The authority is the
/// committer timestamp of the last commit that changed the spec, resolved with
/// the exact path-safe invocation `git log -1 --format=%ct -- <path>`.
/// Unavailable, missing, or malformed evidence is a named `not_proven`
/// finding — never a silent pass.
fn proposed_spec_age_finding(normalized: &str) -> Option<String> {
    let output = match run_output("git", &["log", "-1", "--format=%ct", "--", normalized]) {
        Ok(output) => output,
        Err(err) => {
            return Some(format!(
                "{normalized} proposed-age evidence is not_proven: \
                 `git log -1 --format=%ct -- {normalized}` failed ({err}); \
                 the lifecycle check requires repository evidence and never \
                 falls back to filesystem mtime"
            ));
        }
    };
    let timestamp = output.trim();
    if timestamp.is_empty() {
        return Some(format!(
            "{normalized} proposed-age evidence is not_proven: the spec has no Git \
             history (untracked path); commit it so `git log -1 --format=%ct -- \
             {normalized}` resolves — the check never falls back to filesystem mtime"
        ));
    }
    let Ok(commit_unix) = timestamp.parse::<u64>() else {
        return Some(format!(
            "{normalized} proposed-age evidence is not_proven: `git log` returned a \
             malformed commit timestamp `{timestamp}`"
        ));
    };
    let now_unix = match now_unix_seconds() {
        Ok(now_unix) => now_unix,
        Err(err) => {
            return Some(format!(
                "{normalized} proposed-age evidence is not_proven: {err}; the \
                 lifecycle check requires a real current-time read and never \
                 fabricates one"
            ));
        }
    };
    proposed_spec_age_violation(normalized, commit_unix, now_unix)
}

fn proposed_spec_age_violation(
    normalized: &str,
    commit_unix: u64,
    now_unix: u64,
) -> Option<String> {
    // A committer date beyond a bounded clock-skew window is corrupted or
    // forged age evidence (for example an explicit `GIT_COMMITTER_DATE`):
    // fail closed as not_proven rather than letting `saturating_sub` turn a
    // far-future timestamp into a silent pass (#3035 review).
    if commit_unix > now_unix + PROPOSED_SPEC_CLOCK_SKEW_SECS {
        return Some(format!(
            "{normalized} proposed-age evidence is not_proven: the last Git commit \
             that changed the spec is dated in the future beyond the tolerated \
             clock skew; correct the commit metadata — the check never falls back \
             to filesystem mtime"
        ));
    }
    let age_secs = now_unix.saturating_sub(commit_unix);
    if age_secs <= PROPOSED_SPEC_MAX_AGE_SECS {
        return None;
    }
    let days = age_secs / SECONDS_PER_DAY;
    Some(format!(
        "{normalized} has been `proposed` for {days} days without review (age from \
         the last Git commit that changed the spec, not filesystem mtime); \
         promote to `accepted`, re-scope, or add evidence to justify the status"
    ))
}

fn now_unix_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| format!("failed to read current Unix time: {err}"))
}

fn specs(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("next") => {
            println!("{}", next_spec_id(Path::new("."))?);
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown specs command `{other}`\nusage: cargo xtask specs next"
        )),
        None => Err("missing specs command\nusage: cargo xtask specs next".to_string()),
    }
}

fn check_spec_numbering() -> Result<(), String> {
    let violations = spec_numbering_violations(Path::new("."))?;
    finish_spec_numbering_report(&violations)
}

fn finish_spec_numbering_report(violations: &[String]) -> Result<(), String> {
    finish_policy_report(
        PolicyReportSpec {
            report_file: "spec-numbering.md",
            check: "check-spec-numbering",
            why_it_matters: "Spec IDs are source-of-truth identifiers; current numbering and references should be mechanical instead of agent memory.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Run `cargo xtask specs next` before creating a new docs/specs/RIPR-SPEC-NNNN file.",
                "Add every spec file to docs/specs/README.md.",
                "Use only existing RIPR-SPEC-NNNN IDs in traceability and capability surfaces.",
            ],
            rerun_command: "cargo xtask check-spec-numbering",
            exception_template: None,
        },
        violations,
    )
}

fn next_spec_id(root: &Path) -> Result<String, String> {
    let specs = collect_spec_files_for_root(root)?;
    Ok(next_spec_id_from_ids(
        specs.iter().map(|spec| spec.id.as_str()),
    ))
}

fn next_spec_id_from_ids<'a>(ids: impl Iterator<Item = &'a str>) -> String {
    let next = match ids.filter_map(spec_number_from_id).max() {
        Some(max) => max + 1,
        None => 1,
    };
    format!("RIPR-SPEC-{next:04}")
}

fn spec_numbering_violations(root: &Path) -> Result<Vec<String>, String> {
    let specs = collect_spec_files_for_root(root)?;
    let mut violations = Vec::new();
    let mut ids = BTreeMap::<String, Vec<String>>::new();
    for spec in &specs {
        ids.entry(spec.id.clone())
            .or_default()
            .push(spec.relative_path.clone());
    }
    for (id, paths) in &ids {
        if paths.len() > 1 {
            violations.push(format!(
                "{id} is used by multiple spec files: {}",
                paths.join(", ")
            ));
        }
    }

    validate_specs_readme_index(root, &specs, &ids, &mut violations)?;
    validate_spec_references(root, &ids, &mut violations)?;

    violations.sort();
    violations.dedup();
    Ok(violations)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SpecFile {
    id: String,
    file_name: String,
    relative_path: String,
}

fn collect_spec_files_for_root(root: &Path) -> Result<Vec<SpecFile>, String> {
    let spec_dir = root.join("docs/specs");
    let mut specs = Vec::new();
    if !spec_dir.exists() {
        return Ok(specs);
    }
    for path in collect_files(&spec_dir)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with("RIPR-SPEC-") {
            continue;
        }
        let Some(id) = spec_id_from_path(&path) else {
            continue;
        };
        specs.push(SpecFile {
            id,
            file_name: file_name.to_string(),
            relative_path: root_relative_path(root, &path),
        });
    }
    specs.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    Ok(specs)
}

fn validate_specs_readme_index(
    root: &Path,
    specs: &[SpecFile],
    ids: &BTreeMap<String, Vec<String>>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let readme = root.join("docs/specs/README.md");
    if !readme.exists() {
        violations.push("docs/specs/README.md is missing".to_string());
        return Ok(());
    }
    let text = read_text_lossy(&readme)?;
    for spec in specs {
        let link = format!("[{}]({})", spec.id, spec.file_name);
        if !text.contains(&link) {
            violations.push(format!(
                "docs/specs/README.md is missing index link `{link}` for {}",
                spec.relative_path
            ));
        }
    }
    for id in spec_ids_in_text(&text) {
        if !ids.contains_key(&id) {
            violations.push(format!(
                "docs/specs/README.md references missing spec `{id}`"
            ));
        }
    }
    Ok(())
}

fn validate_spec_references(
    root: &Path,
    ids: &BTreeMap<String, Vec<String>>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for relative in [
        ".ripr/traceability.toml",
        "metrics/capabilities.toml",
        "docs/CAPABILITY_MATRIX.md",
    ] {
        let path = root.join(relative);
        if !path.exists() {
            continue;
        }
        let text = read_text_lossy(&path)?;
        for id in spec_ids_in_text(&text) {
            if !ids.contains_key(&id) {
                violations.push(format!("{relative} references missing spec `{id}`"));
            }
        }
    }
    Ok(())
}

fn spec_ids_in_text(text: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let id_len = "RIPR-SPEC-0000".len();
    for (offset, _) in text.match_indices("RIPR-SPEC-") {
        let Some(candidate) = text.get(offset..offset + id_len) else {
            continue;
        };
        if text
            .as_bytes()
            .get(offset + id_len)
            .is_some_and(u8::is_ascii_digit)
        {
            continue;
        }
        if is_spec_id(candidate) {
            ids.insert(candidate.to_string());
        }
    }
    ids
}

fn spec_number_from_id(id: &str) -> Option<u32> {
    let suffix = id.strip_prefix("RIPR-SPEC-")?;
    if suffix.len() == 4 && is_ascii_digits(suffix) {
        suffix.parse::<u32>().ok()
    } else {
        None
    }
}

fn root_relative_path(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative,
        Err(_) => path,
    }
    .to_string_lossy()
    .replace('\\', "/")
}

fn check_traceability() -> Result<(), String> {
    let manifest = Path::new(".ripr/traceability.toml");
    let mut violations = Vec::new();
    let mut advisories = Vec::new();
    if !manifest.exists() {
        violations.push(".ripr/traceability.toml is missing".to_string());
        return finish_traceability_report(&violations, &advisories);
    }

    let (behaviors, parse_violations) = parse_traceability_manifest(manifest)?;
    violations.extend(parse_violations);
    if behaviors.is_empty() {
        violations.push(".ripr/traceability.toml has no [[behavior]] entries".to_string());
    }

    let specs = collect_spec_statuses()?;
    let mut behavior_ids = BTreeSet::new();
    for behavior in &behaviors {
        validate_trace_behavior(
            behavior,
            &mut behavior_ids,
            &mut violations,
            &mut advisories,
        )?;
    }

    for spec_id in specs.keys() {
        if !behavior_ids.contains(spec_id) {
            violations.push(format!(
                "{spec_id} exists in docs/specs but is missing from .ripr/traceability.toml"
            ));
        }
    }

    validate_fixture_spec_references(&specs, &mut violations)?;
    finish_traceability_report(&violations, &advisories)
}

fn finish_traceability_report(violations: &[String], advisories: &[String]) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# check-traceability\n\nStatus: {status}\n\n");
    body.push_str("## Why This Matters\n\n");
    body.push_str(
        "Traceability keeps behavior specs, tests, fixtures, code, outputs, and metrics \
         discoverable for long-context human and agent work.",
    );
    body.push_str("\n\n");

    if violations.is_empty() {
        body.push_str("## Violations\n\nNone detected.\n\n");
    } else {
        body.push_str("## Violations\n\n");
        for violation in violations {
            body.push_str("```text\n");
            body.push_str(violation);
            body.push_str("\n```\n\n");
        }
    }

    if !violations.is_empty() {
        body.push_str("## Fix Kind\n\n```text\n");
        body.push_str(fix_kind_name(&FixKind::AuthorDecisionRequired));
        body.push_str("\n```\n\n");

        body.push_str("## Recommended Fixes\n\n");
        for (index, fix) in traceability_recommended_fixes().iter().enumerate() {
            body.push_str(&format!("{}. {fix}\n", index + 1));
        }
        body.push('\n');
    }

    // Render advisory diagnostics (non-blocking) so they stay visible in the
    // report without failing the gate (#2549). These are symbol-suffix
    // references that point to existing files but cannot be structurally
    // verified until a cargo-proof symbol resolver lands (#2345).
    if !advisories.is_empty() {
        body.push_str("## Advisories (non-blocking)\n\n");
        body.push_str(
            "These references point to existing files but carry a `::symbol` suffix that the \
             structural checker cannot verify yet. They do not fail the gate.\n\n",
        );
        for advisory in advisories {
            body.push_str("```text\n");
            body.push_str(advisory);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Rerun\n\n```bash\ncargo xtask check-traceability\n```\n");

    write_report("traceability.md", &body)?;

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "check-traceability failed; see target/ripr/reports/traceability.md\n{}",
            violations.join("\n")
        ))
    }
}

fn traceability_recommended_fixes() -> &'static [&'static str] {
    &[
        "Add or update the matching [[behavior]] entry in .ripr/traceability.toml.",
        "Run `cargo xtask specs next` to get the next free RIPR-SPEC-NNNN number.",
        "Known spec IDs are listed in docs/specs/README.md.",
        "Keep every docs/specs/RIPR-SPEC-*.md file represented in the manifest.",
        "Use valid RIPR-SPEC-NNNN IDs (exactly 4 digits) in specs, fixtures, and manifest entries.",
        "List only paths that exist, or leave planned fields empty until the artifact exists.",
    ]
}

fn validate_trace_behavior(
    behavior: &TraceBehavior,
    behavior_ids: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
    advisories: &mut Vec<String>,
) -> Result<(), String> {
    let Some(id) = behavior.id.as_ref() else {
        violations.push(format!(
            "behavior at line {} is missing `id`",
            behavior.line
        ));
        return Ok(());
    };
    if !is_spec_id(id) {
        violations.push(format!(
            "behavior at line {} has invalid spec id `{id}`",
            behavior.line
        ));
    }
    if !behavior_ids.insert(id.clone()) {
        violations.push(format!("duplicate traceability behavior id `{id}`"));
    }
    if behavior
        .name
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        violations.push(format!("{id} is missing a non-empty `name`"));
    }

    let spec_status = match behavior.spec.as_ref() {
        Some(spec) => validate_behavior_spec_path(id, spec, violations)?,
        None => {
            violations.push(format!("{id} is missing `spec`"));
            None
        }
    };

    validate_trace_paths(id, "tests", &behavior.tests, violations, advisories);
    validate_trace_paths(id, "fixtures", &behavior.fixtures, violations, advisories);
    validate_trace_paths(id, "code", &behavior.code, violations, advisories);
    validate_trace_paths(id, "outputs", &behavior.outputs, violations, advisories);

    if behavior.metrics.is_empty() && spec_status.as_deref() != Some("deprecated") {
        violations.push(format!("{id} has no metrics"));
    }
    for metric in &behavior.metrics {
        if metric.trim().is_empty() {
            violations.push(format!("{id} has an empty metric entry"));
        }
    }

    if spec_status.as_deref() == Some("accepted")
        && behavior.tests.is_empty()
        && behavior.fixtures.is_empty()
    {
        violations.push(format!(
            "{id} is accepted but has no current test or fixture mapping"
        ));
    }

    Ok(())
}

fn validate_behavior_spec_path(
    id: &str,
    spec: &str,
    violations: &mut Vec<String>,
) -> Result<Option<String>, String> {
    let path = Path::new(spec);
    if !path.exists() {
        violations.push(format!("{id} spec path does not exist: {spec}"));
        return Ok(None);
    }
    match spec_id_from_path(path) {
        Some(spec_id) if spec_id == id => {}
        Some(spec_id) => violations.push(format!(
            "{id} points at spec path with mismatched id {spec_id}: {spec}"
        )),
        None => violations.push(format!(
            "{id} spec path does not use RIPR-SPEC-NNNN filename: {spec}"
        )),
    }
    match spec_status_from_file(path)? {
        Some(status) => Ok(Some(status)),
        None => {
            violations.push(format!("{id} spec is missing `Status: ...`: {spec}"));
            Ok(None)
        }
    }
}

fn validate_trace_paths(
    id: &str,
    field: &str,
    values: &[String],
    violations: &mut Vec<String>,
    advisories: &mut Vec<String>,
) {
    for value in values {
        let (path_text, suffix) = split_trace_reference(value);
        if path_text.trim().is_empty() {
            violations.push(format!("{id} has an empty `{field}` path"));
            continue;
        }
        if !Path::new(path_text).exists() {
            violations.push(format!("{id} `{field}` path does not exist: {path_text}"));
            continue;
        }
        // #2345 Phase 1: if the reference carries a `::symbol` suffix, emit an
        // advisory that the suffix is not structurally verified. This replaces
        // the previous silent truncation (the suffix was discarded without
        // any signal). The advisory does NOT fail the gate — it surfaces the
        // unverified symbol so a human or agent knows the reference is
        // file-existence-verified only, not symbol-resolved (#2549).
        if let Some(suffix) = suffix
            && !suffix.trim().is_empty()
        {
            advisories.push(format!(
                "{id} `{field}` symbol suffix `{suffix}` is not verified by the structural checker \
                 (file `{path_text}` exists; symbol resolution is a planned cargo-proof provider, see #2345)"
            ));
        }
    }
}

/// Split a trace reference into (path, optional_symbol_suffix).
/// `crates/ripr/src/lib.rs::tests::some_fn` → (`crates/ripr/src/lib.rs`, `tests::some_fn`)
/// `crates/ripr/src/lib.rs` → (`crates/ripr/src/lib.rs`, None)
fn split_trace_reference(value: &str) -> (&str, Option<&str>) {
    match value.split_once("::") {
        Some((path, suffix)) => (path, Some(suffix)),
        None => (value, None),
    }
}

#[allow(
    dead_code,
    reason = "retained for compatibility with callers that only need the path part"
)]
fn trace_path_part(value: &str) -> &str {
    match value.split_once("::") {
        Some((path, _)) => path,
        None => value,
    }
}

fn validate_fixture_spec_references(
    specs: &BTreeMap<String, String>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for fixture in fixture_dirs()? {
        let spec_path = fixture.join("SPEC.md");
        if !spec_path.exists() {
            continue;
        }
        let text = read_text_lossy(&spec_path)?;
        for line in text.lines() {
            let Some(value) = line.strip_prefix("Spec:") else {
                continue;
            };
            let spec_id = value.trim();
            if !is_spec_id(spec_id) {
                violations.push(format!(
                    "{} references invalid spec id `{spec_id}`",
                    normalize_path(&spec_path)
                ));
            } else if !specs.contains_key(spec_id) {
                violations.push(format!(
                    "{} references unknown spec id `{spec_id}`",
                    normalize_path(&spec_path)
                ));
            }
        }
    }
    Ok(())
}

fn collect_spec_statuses() -> Result<BTreeMap<String, String>, String> {
    let specs_dir = Path::new("docs/specs");
    let mut specs = BTreeMap::new();
    if !specs_dir.exists() {
        return Ok(specs);
    }
    for path in collect_files(specs_dir)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(spec_id) = spec_id_from_path(&path) else {
            continue;
        };
        let status = spec_status_from_file(&path)?.unwrap_or_else(|| "missing".to_string());
        specs.insert(spec_id, status);
    }
    Ok(specs)
}

fn spec_status_from_file(path: &Path) -> Result<Option<String>, String> {
    let text = read_text_lossy(path)?;
    for line in text.lines() {
        let Some(value) = line.strip_prefix("Status:") else {
            continue;
        };
        return Ok(Some(value.trim().to_string()));
    }
    Ok(None)
}

fn spec_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let mut parts = stem.split('-');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next()?;
    if first == "RIPR" && second == "SPEC" && third.len() == 4 && is_ascii_digits(third) {
        Some(format!("RIPR-SPEC-{third}"))
    } else {
        None
    }
}

fn is_spec_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("RIPR-SPEC-") else {
        return false;
    };
    suffix.len() == 4 && is_ascii_digits(suffix)
}

fn is_ascii_digits(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_traceability_manifest(path: &Path) -> Result<(Vec<TraceBehavior>, Vec<String>), String> {
    let text = read_text_lossy(path)?;
    let mut behaviors = Vec::new();
    let mut violations = Vec::new();
    let mut current: Option<TraceBehavior> = None;
    let mut active_array: Option<(String, Vec<String>, usize)> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, values, start_line)) = active_array.as_mut() {
            if trimmed.starts_with(']') {
                let Some(mut behavior) = current.take() else {
                    violations.push(format!(
                        "{}:{} array `{key}` is outside a behavior entry",
                        normalize_path(path),
                        start_line
                    ));
                    active_array = None;
                    continue;
                };
                assign_trace_array(
                    &mut behavior,
                    key,
                    values.clone(),
                    *start_line,
                    &mut violations,
                );
                current = Some(behavior);
                active_array = None;
                continue;
            }
            match parse_array_item(trimmed) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        if trimmed == "[[behavior]]" {
            if let Some(behavior) = current.take() {
                behaviors.push(behavior);
            }
            current = Some(TraceBehavior {
                line: line_number,
                ..TraceBehavior::default()
            });
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{}:{line_number} expected `key = value`",
                normalize_path(path)
            ));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(behavior) = current.as_mut() else {
            violations.push(format!(
                "{}:{line_number} `{key}` appears outside a [[behavior]] entry",
                normalize_path(path)
            ));
            continue;
        };
        if value == "[" {
            active_array = Some((key.to_string(), Vec::new(), line_number));
            continue;
        }
        if value.starts_with('[') {
            match parse_inline_array(value) {
                Ok(values) => {
                    assign_trace_array(behavior, key, values, line_number, &mut violations)
                }
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        match parse_quoted_value(value) {
            Ok(parsed) => assign_trace_string(behavior, key, parsed, line_number, &mut violations),
            Err(message) => {
                violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
            }
        }
    }

    if let Some((key, _, start_line)) = active_array {
        violations.push(format!(
            "{}:{start_line} array `{key}` is missing closing `]`",
            normalize_path(path)
        ));
    }
    if let Some(behavior) = current {
        behaviors.push(behavior);
    }
    Ok((behaviors, violations))
}

fn assign_trace_string(
    behavior: &mut TraceBehavior,
    key: &str,
    value: String,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "id" => behavior.id = Some(value),
        "name" => behavior.name = Some(value),
        "spec" => behavior.spec = Some(value),
        _ => violations.push(format!(
            "traceability line {line_number} uses unsupported string field `{key}`"
        )),
    }
}

fn assign_trace_array(
    behavior: &mut TraceBehavior,
    key: &str,
    values: Vec<String>,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "tests" => behavior.tests = values,
        "fixtures" => behavior.fixtures = values,
        "code" => behavior.code = values,
        "outputs" => behavior.outputs = values,
        "metrics" => behavior.metrics = values,
        _ => violations.push(format!(
            "traceability line {line_number} uses unsupported array field `{key}`"
        )),
    }
}

fn parse_inline_array(value: &str) -> Result<Vec<String>, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("expected string array".to_string());
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut values = Vec::new();
    for item in inner.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        values.push(parse_quoted_value(item)?);
    }
    Ok(values)
}

fn parse_array_item(value: &str) -> Result<Option<String>, String> {
    let trimmed = value.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        Ok(None)
    } else {
        parse_quoted_value(trimmed).map(Some)
    }
}

fn parse_quoted_value(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(format!("expected quoted string, got `{trimmed}`"));
    }
    Ok(trimmed[1..trimmed.len() - 1].to_string())
}

pub(crate) fn metrics_report_impl() -> Result<(), String> {
    let (capabilities, violations) =
        parse_capabilities_manifest(Path::new("metrics/capabilities.toml"))?;
    if !violations.is_empty() {
        finish_capabilities_report(&violations)?;
        return Err(format!(
            "metrics source is invalid; see target/ripr/reports/capabilities.md\n{}",
            violations.join("\n")
        ));
    }
    write_report("metrics.md", &capability_metrics_markdown(&capabilities))?;
    write_report("metrics.json", &capability_metrics_json(&capabilities))
}

pub(crate) fn test_oracle_report_impl() -> Result<(), String> {
    let tests = collect_test_oracle_tests()?;
    write_report("test-oracles.md", &test_oracle_report_markdown(&tests))?;
    write_report("test-oracles.json", &test_oracle_report_json(&tests))
}

fn collect_test_oracle_tests() -> Result<Vec<TestOracleTest>, String> {
    let mut tests = Vec::new();
    for root in [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let text = read_text_lossy(&path)?;
            tests.extend(test_oracle_tests_in_text(&path, &text));
        }
    }
    tests.sort_by(|left, right| {
        normalize_path(&left.path)
            .cmp(&normalize_path(&right.path))
            .then(left.line.cmp(&right.line))
            .then(left.name.cmp(&right.name))
    });
    Ok(tests)
}

fn test_oracle_tests_in_text(path: &Path, text: &str) -> Vec<TestOracleTest> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut tests = Vec::new();
    let mut pending_test_attr_line = None;
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if is_test_attribute(trimmed) {
            pending_test_attr_line = Some(index + 1);
            index += 1;
            continue;
        }

        if let Some(attr_line) = pending_test_attr_line {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                index += 1;
                continue;
            }

            if let Some(name) = test_fn_name(trimmed) {
                let end = test_function_end(&lines, index);
                let observations = test_oracle_observations(&lines[index..=end], index + 1);
                let class = observations
                    .iter()
                    .map(|observation| observation.class)
                    .max_by_key(|class| class.rank())
                    .unwrap_or(TestOracleClass::Smoke);
                tests.push(TestOracleTest {
                    path: path.to_path_buf(),
                    name,
                    line: attr_line,
                    body_line: index + 1,
                    body: lines[index..=end].join("\n"),
                    class,
                    observations,
                });
                pending_test_attr_line = None;
                index = end + 1;
                continue;
            }

            if !trimmed.starts_with("//") {
                pending_test_attr_line = None;
            }
        }

        index += 1;
    }

    tests
}

fn is_test_attribute(trimmed: &str) -> bool {
    let compact = trimmed.replace(' ', "");
    if !compact.starts_with("#[") {
        return false;
    }
    compact == "#[test]"
        || compact.starts_with("#[tokio::test")
        || compact.starts_with("#[async_std::test")
        || compact.starts_with("#[rstest")
}

fn test_fn_name(trimmed: &str) -> Option<String> {
    let fn_pos = trimmed.find("fn ")?;
    let after_fn = &trimmed[fn_pos + 3..];
    let name = after_fn
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if name.is_empty() { None } else { Some(name) }
}

fn test_function_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0isize;
    let mut saw_body = false;

    for (offset, line) in lines[start..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    saw_body = true;
                }
                '}' if saw_body => depth -= 1,
                _ => {}
            }
        }
        if saw_body && depth <= 0 {
            return start + offset;
        }
    }

    lines.len().saturating_sub(1)
}

fn test_oracle_observations(lines: &[&str], first_line: usize) -> Vec<TestOracleObservation> {
    let mut observations = Vec::new();
    for (offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if let Some(observation) = test_oracle_observation(trimmed, first_line + offset) {
            observations.push(observation);
        }
    }

    if observations.is_empty() {
        observations.push(TestOracleObservation {
            line: first_line,
            class: TestOracleClass::Smoke,
            pattern: "no assertion".to_string(),
            detail: "test body has no detected assertion-like oracle".to_string(),
        });
    }

    observations
}

fn test_oracle_observation(trimmed: &str, line: usize) -> Option<TestOracleObservation> {
    if trimmed.is_empty() {
        return None;
    }

    if contains_any(trimmed, &["assert_eq!(", "assert_ne!(", "assert_matches!("]) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Strong,
            "exact assertion",
            "exact equality, inequality, or variant assertion",
        ));
    }
    if trimmed.contains("matches!(") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Strong,
            "matches!",
            "pattern assertion can discriminate an exact variant or shape",
        ));
    }
    if trimmed.contains("status.success()") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Smoke,
            "status.success",
            "exit-status check proves execution but little behavior",
        ));
    }
    if contains_any(
        trimmed,
        &[
            ".is_ok()",
            ".is_err()",
            ".is_some()",
            ".is_none()",
            ".is_empty()",
            ".contains(",
            "contains(",
        ],
    ) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Weak,
            "broad predicate",
            "broad predicate may miss changed behavior or exact discriminator drift",
        ));
    }
    if trimmed.contains("assert!(") && contains_any(trimmed, &[" == ", " != ", " >= ", " <= "]) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Medium,
            "boolean comparison",
            "boolean comparison gives some discrimination without structured equality",
        ));
    }
    if trimmed.contains("assert!(") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Weak,
            "generic assert",
            "generic boolean assertion needs review for discriminator strength",
        ));
    }

    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn test_oracle_observation_for(
    line: usize,
    class: TestOracleClass,
    pattern: &str,
    detail: &str,
) -> TestOracleObservation {
    TestOracleObservation {
        line,
        class,
        pattern: pattern.to_string(),
        detail: detail.to_string(),
    }
}

fn is_bdd_test_name(name: &str) -> bool {
    let compact = name.to_ascii_lowercase();
    if !compact.starts_with("given_") {
        return false;
    }

    let Some(when_index) = compact.find("_when_") else {
        return false;
    };
    let Some(then_index) = compact.find("_then_") else {
        return false;
    };

    when_index > "given_".len() && then_index > when_index + "_when_".len()
}

fn test_oracle_counts(tests: &[TestOracleTest]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([
        ("strong", 0usize),
        ("medium", 0usize),
        ("weak", 0usize),
        ("smoke", 0usize),
    ]);
    for test in tests {
        if let Some(count) = counts.get_mut(test.class.as_str()) {
            *count += 1;
        }
    }
    counts
}

fn test_oracle_report_status(tests: &[TestOracleTest]) -> &'static str {
    if tests
        .iter()
        .any(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
    {
        "warn"
    } else {
        "pass"
    }
}

fn test_oracle_report_markdown(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let bdd_named = tests
        .iter()
        .filter(|test| is_bdd_test_name(&test.name))
        .count();
    let mut body = format!(
        "# ripr test oracle report\n\nStatus: {}\n\nMode: advisory\n\nThis report measures the apparent discriminator strength of `ripr`'s own Rust tests. It does not fail existing debt yet.\n\n## Summary\n\n- Strong: {}\n- Medium: {}\n- Weak: {}\n- Smoke: {}\n- BDD-shaped names: {} / {}\n\n",
        test_oracle_report_status(tests),
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0),
        bdd_named,
        tests.len(),
    );

    body.push_str("## Weak Or Smoke Tests\n\n");
    let weak_or_smoke = tests
        .iter()
        .filter(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
        .collect::<Vec<_>>();
    if weak_or_smoke.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for test in weak_or_smoke {
            body.push_str(&format!(
                "- `{}`:{} `{}` classified `{}`\n",
                normalize_path(&test.path),
                test.line,
                test.name,
                test.class.as_str()
            ));
            for observation in &test.observations {
                body.push_str(&format!(
                    "  - line {}: `{}` - {}\n",
                    observation.line, observation.pattern, observation.detail
                ));
            }
        }
        body.push('\n');
    }

    body.push_str("## All Tests\n\n| Test | Class | Evidence |\n| --- | --- | --- |\n");
    for test in tests {
        let evidence = test
            .observations
            .iter()
            .map(|observation| format!("{}: {}", observation.line, observation.pattern))
            .collect::<Vec<_>>()
            .join("<br>");
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | {} |\n",
            normalize_path(&test.path),
            test.line,
            markdown_cell(&test.name),
            test.class.as_str(),
            markdown_cell(&evidence)
        ));
    }
    body
}

fn test_oracle_report_json(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"counts\": {{\n    \"strong\": {},\n    \"medium\": {},\n    \"weak\": {},\n    \"smoke\": {}\n  }},\n  \"tests\": [\n",
        test_oracle_report_status(tests),
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0)
    );

    for (test_index, test) in tests.iter().enumerate() {
        if test_index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&normalize_path(&test.path))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&test.name)
        ));
        body.push_str(&format!("      \"line\": {},\n", test.line));
        body.push_str(&format!("      \"class\": \"{}\",\n", test.class.as_str()));
        body.push_str("      \"observations\": [\n");
        for (observation_index, observation) in test.observations.iter().enumerate() {
            if observation_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("        {\n");
            body.push_str(&format!("          \"line\": {},\n", observation.line));
            body.push_str(&format!(
                "          \"class\": \"{}\",\n",
                observation.class.as_str()
            ));
            body.push_str(&format!(
                "          \"pattern\": \"{}\",\n",
                json_escape(&observation.pattern)
            ));
            body.push_str(&format!(
                "          \"detail\": \"{}\"\n",
                json_escape(&observation.detail)
            ));
            body.push_str("        }");
        }
        body.push_str("\n      ]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

pub(crate) fn test_efficiency_report_impl() -> Result<(), String> {
    let tests = collect_test_oracle_tests()?;
    let mut entries = tests.iter().map(test_efficiency_entry).collect::<Vec<_>>();
    let duplicate_groups = apply_duplicate_discriminator_groups(&mut entries);
    let test_intent_summary = match load_test_intent_manifest() {
        Ok(declarations) => {
            let mut violations = validate_test_intent_paths_on_disk(&declarations);
            violations.extend(apply_test_intent_to_entries(&mut entries, &declarations));
            if !violations.is_empty() {
                return Err(format!(
                    "{TEST_INTENT_PATH} validation failed:\n{}",
                    violations.join("\n")
                ));
            }
            TestIntentReportSummary {
                declared: declarations.len(),
                matched: entries
                    .iter()
                    .filter(|e| e.declared_intent.is_some())
                    .count(),
            }
        }
        Err(violations) => {
            return Err(format!(
                "{TEST_INTENT_PATH} parse failed:\n{}",
                violations.join("\n")
            ));
        }
    };
    write_report(
        "test-efficiency.md",
        &test_efficiency_report_markdown(&entries, &duplicate_groups, &test_intent_summary),
    )?;
    write_report(
        "test-efficiency.json",
        &test_efficiency_report_json(&entries, &duplicate_groups, &test_intent_summary),
    )
}

fn test_efficiency_entry(test: &TestOracleTest) -> TestEfficiencyEntry {
    let reached_owners = test_efficiency_reached_owners(test);
    let observed_values = test_efficiency_observed_values(test);
    let reasons = test_efficiency_reasons(test, &reached_owners, &observed_values);
    let mut static_limitations = test_efficiency_static_limitations(test);
    if reached_owners.is_empty() {
        static_limitations.push(
            "no direct owner call detected; test may route through helpers, fixtures, or macros"
                .to_string(),
        );
    }
    if observed_values.is_empty() {
        static_limitations.push("no literal activation values detected".to_string());
    }

    TestEfficiencyEntry {
        path: test.path.clone(),
        name: test.name.clone(),
        line: test.line,
        class: test_efficiency_class(test, &reached_owners, &reasons),
        oracle_kind: test_efficiency_oracle_kind(test).to_string(),
        oracle_strength: test.class.as_str(),
        reached_owners,
        observed_values,
        reasons,
        static_limitations,
        duplicate_group_id: None,
        declared_intent: None,
    }
}

const DUPLICATE_DISCRIMINATOR_NEXT_STEP: &str = "Keep both if they document distinct business cases. Otherwise consider adding a different activation value or oracle shape.";

const DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON: &str = "duplicate_activation_and_oracle_shape";

/// Groups eligible test-efficiency entries that share an owner set, an
/// activation signature, and an oracle shape. Mutates eligible entries in
/// place: their class becomes `"duplicative"`, the
/// `duplicate_activation_and_oracle_shape` reason is appended to their
/// reasons (preserving any existing reasons such as `smoke_oracle_only`),
/// and `duplicate_group_id` is set.
///
/// Eligibility is conservative: a test is eligible only if its base class is
/// `strong_discriminator`, `useful_but_broad`, or `smoke_only`. Tests already
/// classified `opaque`, `likely_vacuous`, or `possibly_circular` are kept on
/// their existing class because that signal is more actionable than
/// "duplicate." Tests with no observed activation literals are also excluded
/// — we cannot build a credible activation signature for them.
///
/// The activation signature is role-aware: it preserves the order and
/// `(context, value)` pairing of `observed_values`, so `score(2) == 3` and
/// `score(3) == 2` produce different signatures even though the raw value
/// set is identical.
///
/// In v1 the grouping key does not include explicit flow-sink evidence —
/// the test-efficiency ledger does not currently emit it. The role-aware
/// activation signature acts as a narrow proxy because the
/// `assertion_argument` context naturally captures the sink-side values of
/// the oracle. A future PR can promote explicit sink evidence into the
/// ledger and tighten the key.
fn apply_duplicate_discriminator_groups(
    entries: &mut [TestEfficiencyEntry],
) -> Vec<DuplicateDiscriminatorGroup> {
    let mut buckets: BTreeMap<DuplicateGroupKey, Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if !is_duplicate_discriminator_eligible(entry) {
            continue;
        }
        let key = duplicate_discriminator_key(entry);
        buckets.entry(key).or_default().push(index);
    }

    let mut groups: Vec<(usize, DuplicateGroupKey, Vec<usize>)> = buckets
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(key, members)| {
            // Safe: filter above guarantees `members.len() >= 2`.
            let first = members[0];
            (first, key, members)
        })
        .collect();
    groups.sort_by_key(|(first, _, _)| *first);

    let mut rendered = Vec::with_capacity(groups.len());
    for (group_index, (_, key, members)) in groups.into_iter().enumerate() {
        let id = format!("duplicate_group_{}", group_index + 1);
        let group_members: Vec<DuplicateGroupMember> = members
            .iter()
            .map(|&i| DuplicateGroupMember {
                path: normalize_path(&entries[i].path),
                name: entries[i].name.clone(),
                line: entries[i].line,
            })
            .collect();
        for &i in &members {
            entries[i].class = "duplicative";
            if !entries[i]
                .reasons
                .iter()
                .any(|r| r == DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON)
            {
                entries[i]
                    .reasons
                    .push(DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON.to_string());
                entries[i].reasons.sort();
            }
            entries[i].duplicate_group_id = Some(id.clone());
        }
        let DuplicateGroupKey {
            owners,
            oracle_kind,
            oracle_strength,
            activation_signature,
        } = key;
        rendered.push(DuplicateDiscriminatorGroup {
            id,
            members: group_members,
            shared_evidence: DuplicateGroupSharedEvidence {
                owners,
                oracle_kind,
                oracle_strength,
                activation_signature: activation_signature
                    .into_iter()
                    .map(|(context, value)| DuplicateGroupActivation { context, value })
                    .collect(),
            },
            suggested_next_step: DUPLICATE_DISCRIMINATOR_NEXT_STEP.to_string(),
        });
    }
    rendered
}

fn is_duplicate_discriminator_eligible(entry: &TestEfficiencyEntry) -> bool {
    matches!(
        entry.class,
        "strong_discriminator" | "useful_but_broad" | "smoke_only"
    ) && !entry.reached_owners.is_empty()
        && !entry.observed_values.is_empty()
}

fn duplicate_discriminator_key(entry: &TestEfficiencyEntry) -> DuplicateGroupKey {
    let mut owners = entry.reached_owners.clone();
    owners.sort();
    owners.dedup();
    let activation_signature = entry
        .observed_values
        .iter()
        .map(|value| (value.context, value.value.clone()))
        .collect();
    DuplicateGroupKey {
        owners,
        oracle_kind: entry.oracle_kind.clone(),
        oracle_strength: entry.oracle_strength,
        activation_signature,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DuplicateGroupKey {
    owners: Vec<String>,
    oracle_kind: String,
    oracle_strength: &'static str,
    activation_signature: Vec<(&'static str, String)>,
}

const TEST_INTENT_PATH: &str = ".ripr/test_intent.toml";

/// Top-level summary of the test-intent layer rendered in both Markdown
/// and JSON. Always emitted, even when no manifest exists, so consumers
/// get a stable shape.
#[derive(Clone, Debug, Default)]
struct TestIntentReportSummary {
    declared: usize,
    matched: usize,
}

/// Parses the `.ripr/test_intent.toml` manifest text into declarations.
/// Returns the parsed declarations alongside any structural violations.
/// The parser is pure (no I/O) so it can be unit-tested directly.
fn parse_test_intent_manifest(text: &str) -> (Vec<TestIntentDeclaration>, Vec<String>) {
    let mut entries: Vec<TestIntentDeclaration> = Vec::new();
    let mut violations = Vec::new();
    let mut schema_seen = false;
    let mut current: Option<PendingTestIntent> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[test_intent]]" {
            if let Some(pending) = current.take() {
                finalize_test_intent_entry(pending, &mut entries, &mut violations);
            }
            current = Some(PendingTestIntent::new(line_number));
            continue;
        }
        let Some((key, raw_value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{line_number} expected `key = value`"
            ));
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        if let Some(pending) = current.as_mut() {
            match key {
                "test" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.test = Some((parsed, line_number))
                    })
                }
                "path" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.path = Some((parsed, line_number))
                    })
                }
                "intent" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.intent = Some((parsed, line_number))
                    })
                }
                "owner" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.owner = Some((parsed, line_number))
                    })
                }
                "reason" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.reason = Some((parsed, line_number))
                    })
                }
                _ => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} unsupported `[[test_intent]]` field `{key}`"
                )),
            }
        } else if key == "schema_version" {
            schema_seen = true;
            match raw_value.parse::<u32>() {
                Ok(1) => {}
                Ok(other) => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
                )),
                Err(_) => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} schema_version must be an integer literal"
                )),
            }
        } else {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{line_number} unsupported top-level field `{key}`"
            ));
        }
    }

    if let Some(pending) = current.take() {
        finalize_test_intent_entry(pending, &mut entries, &mut violations);
    }

    if !schema_seen {
        violations.push(format!(
            "{TEST_INTENT_PATH} is missing required `schema_version = 1` header"
        ));
    }

    let mut seen: BTreeMap<(String, Option<String>), usize> = BTreeMap::new();
    for entry in &entries {
        let key = (entry.test.clone(), entry.path.clone());
        if let Some(&first) = seen.get(&key) {
            let location = match &entry.path {
                Some(path) => format!("`{}` at `{}`", entry.test, path),
                None => format!("`{}`", entry.test),
            };
            violations.push(format!(
                "{TEST_INTENT_PATH} duplicate selector {location} (first declared near line {first})"
            ));
        } else {
            seen.insert(key, entry.block_line);
        }
    }

    (entries, violations)
}

struct PendingTestIntent {
    block_line: usize,
    test: Option<(String, usize)>,
    path: Option<(String, usize)>,
    intent: Option<(String, usize)>,
    owner: Option<(String, usize)>,
    reason: Option<(String, usize)>,
}

impl PendingTestIntent {
    fn new(block_line: usize) -> Self {
        Self {
            block_line,
            test: None,
            path: None,
            intent: None,
            owner: None,
            reason: None,
        }
    }
}

fn assign_test_intent_field<F>(
    raw_value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    mut assign: F,
) where
    F: FnMut(String),
{
    match parse_quoted_value(raw_value) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!("{TEST_INTENT_PATH}:{line_number} {message}")),
    }
}

fn finalize_test_intent_entry(
    pending: PendingTestIntent,
    entries: &mut Vec<TestIntentDeclaration>,
    violations: &mut Vec<String>,
) {
    let block_line = pending.block_line;

    let test = match pending.test {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{TEST_INTENT_PATH}:{line} `test` is blank"));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `test`"
            ));
            None
        }
    };

    let path = match pending.path {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{TEST_INTENT_PATH}:{line} `path` is empty"));
                None
            } else if value.contains('\\') {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `path` `{value}` uses backslashes; use `/` separators"
                ));
                None
            } else if is_absolute_path_like(&value) {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `path` `{value}` is absolute; entries must be repository-relative"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    };

    let intent = match pending.intent {
        Some((value, line)) => match TestIntentKind::from_str(&value) {
            Some(kind) => Some(kind),
            None => {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} unsupported intent `{value}`; supported: {}",
                    TestIntentKind::supported().join(", ")
                ));
                None
            }
        },
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `intent`"
            ));
            None
        }
    };

    let owner = match pending.owner {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `owner` is blank; name a responsible team or maintainer"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `owner`"
            ));
            None
        }
    };

    let reason = match pending.reason {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `reason` is blank; explain why this declaration exists"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `reason`"
            ));
            None
        }
    };

    if let (Some(test), Some(intent), Some(owner), Some(reason)) = (test, intent, owner, reason) {
        entries.push(TestIntentDeclaration {
            test,
            path,
            intent,
            owner,
            reason,
            block_line,
        });
    }
}

/// Loads the test-intent manifest from disk. Returns an empty list when
/// the file does not exist (this is a normal state — most projects will
/// have no declarations). Parse and validation violations are returned as
/// `Err` so the caller can surface them through the policy report.
fn load_test_intent_manifest() -> Result<Vec<TestIntentDeclaration>, Vec<String>> {
    let path = Path::new(TEST_INTENT_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = read_text_lossy(path).map_err(|err| vec![err])?;
    let (entries, violations) = parse_test_intent_manifest(&text);
    if violations.is_empty() {
        Ok(entries)
    } else {
        Err(violations)
    }
}

/// Path-existence guard for `path = "..."` declarations. Kept separate
/// from `apply_test_intent_to_entries` so the matcher stays hermetic for
/// unit tests; this function is the I/O-aware companion the orchestrator
/// runs against real declarations.
fn validate_test_intent_paths_on_disk(declarations: &[TestIntentDeclaration]) -> Vec<String> {
    declarations
        .iter()
        .filter_map(|declaration| {
            declaration.path.as_ref().and_then(|path| {
                if Path::new(path).exists() {
                    None
                } else {
                    Some(format!(
                        "{TEST_INTENT_PATH}:{} `path` `{}` does not exist on disk",
                        declaration.block_line, path
                    ))
                }
            })
        })
        .collect()
}

/// Applies test-intent declarations to a slice of entries, attaching
/// `declared_intent` metadata when a declaration matches a single entry.
/// Returns violations for unmatched declarations and ambiguous name-only
/// selectors. Path-existence is **not** checked here — see
/// `validate_test_intent_paths_on_disk` for the I/O-aware companion.
fn apply_test_intent_to_entries(
    entries: &mut [TestEfficiencyEntry],
    declarations: &[TestIntentDeclaration],
) -> Vec<String> {
    let mut violations = Vec::new();
    for declaration in declarations {
        let matches: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.name == declaration.test
                    && declaration
                        .path
                        .as_ref()
                        .map(|path| normalize_path(&entry.path) == *path)
                        .unwrap_or(true)
            })
            .map(|(index, _)| index)
            .collect();

        match matches.len() {
            0 => {
                let location = match &declaration.path {
                    Some(path) => format!("`{}` at `{}`", declaration.test, path),
                    None => format!("`{}`", declaration.test),
                };
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{} test intent selector {location} did not match any test",
                    declaration.block_line
                ));
            }
            1 => {
                let index = matches[0];
                entries[index].declared_intent = Some(DeclaredIntent {
                    intent: declaration.intent,
                    owner: declaration.owner.clone(),
                    reason: declaration.reason.clone(),
                    source: TEST_INTENT_PATH.to_string(),
                });
            }
            _ if declaration.path.is_none() => {
                let candidates = matches
                    .iter()
                    .map(|&i| normalize_path(&entries[i].path))
                    .collect::<Vec<_>>()
                    .join(", ");
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{} test intent selector `{}` matched multiple tests; add `path` to disambiguate (candidates: {candidates})",
                    declaration.block_line, declaration.test
                ));
            }
            _ => {
                // Multiple matches WITH path; attach to all of them so a
                // genuinely-shared name across files behaves predictably.
                // (In practice the path narrows to one file, so this is
                // rare; we still want determinism if it happens.)
                for &index in &matches {
                    entries[index].declared_intent = Some(DeclaredIntent {
                        intent: declaration.intent,
                        owner: declaration.owner.clone(),
                        reason: declaration.reason.clone(),
                        source: TEST_INTENT_PATH.to_string(),
                    });
                }
            }
        }
    }
    violations
}

fn test_efficiency_class(
    test: &TestOracleTest,
    reached_owners: &[String],
    reasons: &[String],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| reason == "expected_value_computed_from_detected_owner_path")
    {
        return "possibly_circular";
    }
    if reached_owners.is_empty() {
        return "opaque";
    }
    if reasons
        .iter()
        .any(|reason| reason == "no_assertion_detected")
    {
        return "likely_vacuous";
    }
    match test.class {
        TestOracleClass::Strong => "strong_discriminator",
        TestOracleClass::Medium | TestOracleClass::Weak => "useful_but_broad",
        TestOracleClass::Smoke => "smoke_only",
    }
}

fn test_efficiency_oracle_kind(test: &TestOracleTest) -> &'static str {
    test.observations
        .iter()
        .max_by_key(|observation| observation.class.rank())
        .map(|observation| match observation.pattern.as_str() {
            "exact assertion" => "exact assertion",
            "matches!" => "pattern assertion",
            "boolean comparison" => "relational check",
            "broad predicate" => "broad predicate",
            "status.success" => "smoke execution",
            "generic assert" => "generic boolean assertion",
            "no assertion" => "no assertion detected",
            _ => "opaque oracle",
        })
        .unwrap_or("opaque oracle")
}

fn test_efficiency_static_limitations(test: &TestOracleTest) -> Vec<String> {
    let mut limitations = Vec::new();
    if test
        .observations
        .iter()
        .any(|observation| observation.pattern == "no assertion")
    {
        limitations.push("no assertion-like oracle detected".to_string());
    }
    match test.class {
        TestOracleClass::Strong => {}
        TestOracleClass::Medium => limitations.push(
            "relational oracle; static ledger cannot confirm exact changed value".to_string(),
        ),
        TestOracleClass::Weak => limitations
            .push("broad oracle; static ledger may miss exact discriminator drift".to_string()),
        TestOracleClass::Smoke => limitations.push(
            "smoke-only oracle; static ledger sees execution but little discriminator detail"
                .to_string(),
        ),
    }
    limitations.sort();
    limitations.dedup();
    limitations
}

fn test_efficiency_reasons(
    test: &TestOracleTest,
    reached_owners: &[String],
    observed_values: &[TestEfficiencyValue],
) -> Vec<String> {
    let mut reasons = BTreeSet::new();
    if test
        .observations
        .iter()
        .any(|observation| observation.pattern == "no assertion")
    {
        reasons.insert("no_assertion_detected".to_string());
    }
    match test.class {
        TestOracleClass::Strong => {}
        TestOracleClass::Medium => {
            reasons.insert("relational_oracle".to_string());
        }
        TestOracleClass::Weak => {
            reasons.insert("broad_oracle".to_string());
            if !reached_owners.is_empty() {
                reasons.insert("assertion_may_not_match_detected_owner".to_string());
            }
        }
        TestOracleClass::Smoke => {
            reasons.insert("smoke_oracle_only".to_string());
        }
    }
    if reached_owners.is_empty() {
        reasons.insert("opaque_helper_or_fixture_boundary".to_string());
    }
    if observed_values.is_empty() {
        reasons.insert("no_activation_literal_detected".to_string());
    }
    if expected_value_uses_reached_owner(test, reached_owners) {
        reasons.insert("expected_value_computed_from_detected_owner_path".to_string());
    }
    reasons.into_iter().collect()
}

fn expected_value_uses_reached_owner(test: &TestOracleTest, reached_owners: &[String]) -> bool {
    if reached_owners.is_empty() {
        return false;
    }
    for line in test.body.lines() {
        let trimmed = line.trim();
        if (trimmed.starts_with("let expected") || trimmed.contains(" expected ="))
            && reached_owners
                .iter()
                .any(|owner| trimmed.contains(&format!("{owner}(")))
        {
            return true;
        }
        if let Some(arguments) = assert_eq_arguments(trimmed) {
            for expected_side in arguments.iter().skip(1) {
                if reached_owners
                    .iter()
                    .any(|owner| expected_side.contains(&format!("{owner}(")))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn assert_eq_arguments(line: &str) -> Option<Vec<String>> {
    let marker = "assert_eq!(";
    let start = line.find(marker)? + marker.len();
    let mut depth = 0isize;
    let mut in_string = false;
    let mut escaped = false;
    let mut argument_start = start;
    let mut arguments = Vec::new();
    let bytes = line.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        let ch = line[index..].chars().next()?;
        let ch_len = ch.len_utf8();
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += ch_len;
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => {
                arguments.push(line[argument_start..index].trim().to_string());
                return Some(arguments);
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                arguments.push(line[argument_start..index].trim().to_string());
                argument_start = index + ch_len;
            }
            _ => {}
        }
        index += ch_len;
    }
    None
}

fn test_efficiency_reached_owners(test: &TestOracleTest) -> Vec<String> {
    let mut calls = BTreeSet::new();
    for line in test.body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
        {
            continue;
        }
        for call in call_names_in_line(trimmed) {
            if !ignored_test_efficiency_call(&call) {
                calls.insert(call);
            }
        }
    }
    calls.into_iter().collect()
}

fn call_names_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut calls = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'(' {
            index += 1;
            continue;
        }
        if index > 0 && bytes[index - 1] == b'!' {
            index += 1;
            continue;
        }

        let mut start = index;
        while start > 0 && is_call_token_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == index {
            index += 1;
            continue;
        }
        let token = &line[start..index];
        if let Some(call) = normalized_call_name(token) {
            calls.push(call);
        }
        index += 1;
    }
    calls
}

fn is_call_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b':'
}

fn normalized_call_name(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(':');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .split("::")
        .any(|segment| segment.chars().next().is_some_and(char::is_uppercase))
    {
        return None;
    }
    let last = trimmed.rsplit("::").next().unwrap_or(trimmed);
    if last.is_empty() || !last.chars().next().unwrap_or('_').is_ascii_alphabetic() {
        return None;
    }
    Some(trimmed.to_string())
}

fn ignored_test_efficiency_call(call: &str) -> bool {
    let last = call.rsplit("::").next().unwrap_or(call);
    matches!(
        last,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "assert_matches"
            | "matches"
            | "format"
            | "format_args"
            | "include_str"
            | "println"
            | "eprintln"
            | "panic"
            | "dbg"
            | "vec"
            | "default"
            | "new"
            | "join"
            | "to_string"
            | "to_owned"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "is_ok"
            | "is_err"
            | "is_some"
            | "is_none"
            | "is_empty"
            | "unwrap"
            | "expect"
            | "clone"
            | "collect"
            | "map"
            | "filter"
            | "iter"
            | "into_iter"
            | "push"
            | "len"
            | "get"
            | "insert"
            | "from"
            | "write"
            | "read_to_string"
            | "test"
    )
}

fn test_efficiency_observed_values(test: &TestOracleTest) -> Vec<TestEfficiencyValue> {
    let mut values = Vec::new();
    for (offset, line) in test.body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let line_number = test.body_line + offset;
        let context = test_efficiency_value_context(trimmed);
        for value in string_literals_in_line(trimmed)
            .into_iter()
            .chain(number_literals_in_line(trimmed))
        {
            values.push(TestEfficiencyValue {
                line: line_number,
                context,
                value,
                text: trimmed.to_string(),
            });
        }
    }
    values
}

fn test_efficiency_value_context(line: &str) -> &'static str {
    if line.contains("assert") {
        "assertion_argument"
    } else if line.contains("vec![") || line.contains('[') {
        "table_or_collection"
    } else if line.contains('(') {
        "function_argument"
    } else {
        "literal"
    }
}

fn string_literals_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut values = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                index += 1;
                values.push(line[start..index].to_string());
                break;
            }
            index += 1;
        }
    }
    values
}

fn number_literals_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut values = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let token_boundary = index == 0 || !is_identifier_byte(bytes[index - 1]);
        if !token_boundary || !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_digit() || bytes[index] == b'_' || bytes[index] == b'.')
        {
            index += 1;
        }
        if index == bytes.len() || !is_identifier_byte(bytes[index]) {
            values.push(line[start..index].to_string());
        }
    }
    values
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn test_efficiency_counts(entries: &[TestEfficiencyEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([
        ("strong_discriminator", 0usize),
        ("useful_but_broad", 0usize),
        ("smoke_only", 0usize),
        ("likely_vacuous", 0usize),
        ("possibly_circular", 0usize),
        ("duplicative", 0usize),
        ("opaque", 0usize),
    ]);
    for entry in entries {
        if let Some(count) = counts.get_mut(entry.class) {
            *count += 1;
        }
    }
    counts
}

fn test_efficiency_reason_counts(entries: &[TestEfficiencyEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for reason in entries.iter().flat_map(|entry| &entry.reasons) {
        *counts.entry(reason.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestEfficiencyMetrics {
    tests_scanned: usize,
    class_counts: BTreeMap<&'static str, usize>,
    reason_counts: BTreeMap<String, usize>,
    duplicate_discriminator_group_count: usize,
}

/// Builds the stable advisory metrics surface for the test-efficiency
/// report. Computed directly from the entries and groups already used to
/// render the report — the JSON and Markdown renderers do not parse their
/// own output to derive metrics.
///
/// `class_counts` is keyed by the seven emitted class strings and always
/// includes every class with a zero default. `reason_counts` is keyed by
/// the reason strings actually present in the entries. `tests_scanned` is
/// the total entry count. `duplicate_discriminator_group_count` is the
/// number of duplicate groups, **not** the number of tests classified
/// `duplicative` — those are reported separately as
/// `class_counts["duplicative"]`.
fn test_efficiency_metrics(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
) -> TestEfficiencyMetrics {
    TestEfficiencyMetrics {
        tests_scanned: entries.len(),
        class_counts: test_efficiency_counts(entries),
        reason_counts: test_efficiency_reason_counts(entries),
        duplicate_discriminator_group_count: duplicate_groups.len(),
    }
}

fn test_efficiency_report_status(entries: &[TestEfficiencyEntry]) -> &'static str {
    if entries
        .iter()
        .any(|entry| entry.class != "strong_discriminator")
    {
        "warn"
    } else {
        "pass"
    }
}

fn test_efficiency_report_markdown(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
    test_intent: &TestIntentReportSummary,
) -> String {
    let metrics = test_efficiency_metrics(entries, duplicate_groups);
    let counts = &metrics.class_counts;
    let reason_counts = &metrics.reason_counts;
    let mut body = format!(
        "# ripr test efficiency report\n\nStatus: {}\n\nMode: advisory\n\nThis report builds a per-test evidence ledger from static Rust test facts. It records apparent owner calls, oracle shape, activation values, and static limitations so reviewers can spot low-discriminator patterns without making the report blocking.\n\n## Summary\n\n- Strong discriminator: {}\n- Useful but broad: {}\n- Smoke only: {}\n- Likely vacuous: {}\n- Possibly circular: {}\n- Duplicative: {}\n- Opaque: {}\n- Duplicate discriminator groups: {}\n- Tests scanned: {}\n\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("likely_vacuous").copied().unwrap_or(0),
        counts.get("possibly_circular").copied().unwrap_or(0),
        counts.get("duplicative").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0),
        duplicate_groups.len(),
        entries.len(),
    );

    body.push_str("## Metrics\n\n| Metric | Value |\n| --- | ---: |\n");
    body.push_str(&format!("| Tests scanned | {} |\n", metrics.tests_scanned));
    body.push_str(&format!(
        "| Strong discriminator | {} |\n",
        counts.get("strong_discriminator").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Useful but broad | {} |\n",
        counts.get("useful_but_broad").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Smoke only | {} |\n",
        counts.get("smoke_only").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Likely vacuous | {} |\n",
        counts.get("likely_vacuous").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Possibly circular | {} |\n",
        counts.get("possibly_circular").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Duplicative | {} |\n",
        counts.get("duplicative").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Opaque | {} |\n",
        counts.get("opaque").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Duplicate discriminator groups | {} |\n\n",
        metrics.duplicate_discriminator_group_count
    ));

    body.push_str("## Signal Reasons\n\n");
    if reason_counts.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for (reason, count) in reason_counts {
            body.push_str(&format!("- `{reason}`: {count}\n"));
        }
        body.push('\n');
    }

    body.push_str("## Static Limitations\n\n");
    let limited_entries = entries
        .iter()
        .filter(|entry| !entry.static_limitations.is_empty())
        .collect::<Vec<_>>();
    if limited_entries.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for entry in limited_entries {
            body.push_str(&format!(
                "- `{}`:{} `{}` classified `{}`\n",
                normalize_path(&entry.path),
                entry.line,
                entry.name,
                entry.class
            ));
            for limitation in &entry.static_limitations {
                body.push_str(&format!("  - {limitation}\n"));
            }
        }
        body.push('\n');
    }

    body.push_str("## Declared Test Intent\n\n");
    body.push_str(&format!(
        "Source: `{TEST_INTENT_PATH}` · declared: {} · matched: {}\n\n",
        test_intent.declared, test_intent.matched
    ));
    let declared_entries = entries
        .iter()
        .filter(|entry| entry.declared_intent.is_some())
        .collect::<Vec<_>>();
    if declared_entries.is_empty() {
        body.push_str("None declared.\n\n");
    } else {
        body.push_str("| Test | Intent | Owner | Reason |\n| --- | --- | --- | --- |\n");
        for entry in declared_entries {
            if let Some(intent) = &entry.declared_intent {
                body.push_str(&format!(
                    "| `{}`:{} `{}` | `{}` | `{}` | {} |\n",
                    normalize_path(&entry.path),
                    entry.line,
                    markdown_cell(&entry.name),
                    intent.intent.as_str(),
                    markdown_cell(&intent.owner),
                    markdown_cell(&intent.reason)
                ));
            }
        }
        body.push('\n');
    }

    body.push_str("## Duplicate Discriminator Groups\n\n");
    if duplicate_groups.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for group in duplicate_groups {
            body.push_str(&format!("### {}\n\n", group.id));
            let owners = if group.shared_evidence.owners.is_empty() {
                "none detected".to_string()
            } else {
                group.shared_evidence.owners.join(", ")
            };
            body.push_str(&format!("- Owners: {owners}\n"));
            body.push_str(&format!(
                "- Oracle: `{}` / `{}`\n",
                group.shared_evidence.oracle_kind, group.shared_evidence.oracle_strength
            ));
            let activation = group
                .shared_evidence
                .activation_signature
                .iter()
                .map(|item| format!("{}=`{}`", item.context, item.value))
                .collect::<Vec<_>>()
                .join(", ");
            body.push_str(&format!("- Activation signature: {activation}\n"));
            body.push_str("- Members:\n");
            for member in &group.members {
                body.push_str(&format!(
                    "  - `{}`:{} `{}`\n",
                    member.path, member.line, member.name
                ));
            }
            body.push_str(&format!(
                "- Suggested next step: {}\n\n",
                group.suggested_next_step
            ));
        }
    }

    body.push_str("## Ledger\n\n| Test | Class | Reasons | Oracle | Reached owners | Observed values | Static limitations |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    for entry in entries {
        let owners = if entry.reached_owners.is_empty() {
            "none detected".to_string()
        } else {
            entry.reached_owners.join("<br>")
        };
        let values = if entry.observed_values.is_empty() {
            "none detected".to_string()
        } else {
            entry
                .observed_values
                .iter()
                .map(|value| format!("{} `{}` ({})", value.line, value.value, value.context))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        let limitations = if entry.static_limitations.is_empty() {
            "none".to_string()
        } else {
            entry.static_limitations.join("<br>")
        };
        let reasons = if entry.reasons.is_empty() {
            "none".to_string()
        } else {
            entry.reasons.join("<br>")
        };
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | {} | `{}` / `{}` | {} | {} | {} |\n",
            normalize_path(&entry.path),
            entry.line,
            markdown_cell(&entry.name),
            entry.class,
            markdown_cell(&reasons),
            entry.oracle_kind,
            entry.oracle_strength,
            markdown_cell(&owners),
            markdown_cell(&values),
            markdown_cell(&limitations)
        ));
    }
    body
}

fn test_efficiency_report_json(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
    test_intent: &TestIntentReportSummary,
) -> String {
    let metrics = test_efficiency_metrics(entries, duplicate_groups);
    let counts = &metrics.class_counts;
    let reason_counts = &metrics.reason_counts;
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"counts\": {{\n    \"strong_discriminator\": {},\n    \"useful_but_broad\": {},\n    \"smoke_only\": {},\n    \"likely_vacuous\": {},\n    \"possibly_circular\": {},\n    \"duplicative\": {},\n    \"opaque\": {}\n  }},\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("likely_vacuous").copied().unwrap_or(0),
        counts.get("possibly_circular").copied().unwrap_or(0),
        counts.get("duplicative").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0)
    );

    body.push_str("  \"reason_counts\": {\n");
    for (index, (reason, count)) in reason_counts.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str(&format!("    \"{}\": {}", json_escape(reason), count));
    }
    body.push_str("\n  },\n  \"tests\": [\n");

    for (entry_index, entry) in entries.iter().enumerate() {
        if entry_index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&normalize_path(&entry.path))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&entry.name)
        ));
        body.push_str(&format!("      \"line\": {},\n", entry.line));
        body.push_str(&format!("      \"class\": \"{}\",\n", entry.class));
        body.push_str(&format!(
            "      \"oracle_kind\": \"{}\",\n",
            json_escape(&entry.oracle_kind)
        ));
        body.push_str(&format!(
            "      \"oracle_strength\": \"{}\",\n",
            entry.oracle_strength
        ));
        body.push_str("      \"reached_owners\": [");
        write_json_string_array(&mut body, &entry.reached_owners);
        body.push_str("],\n");
        body.push_str("      \"reasons\": [");
        write_json_string_array(&mut body, &entry.reasons);
        body.push_str("],\n");
        body.push_str("      \"observed_values\": [\n");
        for (value_index, value) in entry.observed_values.iter().enumerate() {
            if value_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("        {\n");
            body.push_str(&format!("          \"line\": {},\n", value.line));
            body.push_str(&format!("          \"context\": \"{}\",\n", value.context));
            body.push_str(&format!(
                "          \"value\": \"{}\",\n",
                json_escape(&value.value)
            ));
            body.push_str(&format!(
                "          \"text\": \"{}\"\n",
                json_escape(&value.text)
            ));
            body.push_str("        }");
        }
        body.push_str("\n      ],\n");
        body.push_str("      \"static_limitations\": [");
        write_json_string_array(&mut body, &entry.static_limitations);
        body.push(']');
        if let Some(group_id) = &entry.duplicate_group_id {
            body.push_str(&format!(
                ",\n      \"duplicate_group_id\": \"{}\"",
                json_escape(group_id)
            ));
        }
        if let Some(intent) = &entry.declared_intent {
            body.push_str(",\n      \"declared_intent\": {\n");
            body.push_str(&format!(
                "        \"intent\": \"{}\",\n",
                intent.intent.as_str()
            ));
            body.push_str(&format!(
                "        \"owner\": \"{}\",\n",
                json_escape(&intent.owner)
            ));
            body.push_str(&format!(
                "        \"reason\": \"{}\",\n",
                json_escape(&intent.reason)
            ));
            body.push_str(&format!(
                "        \"source\": \"{}\"\n",
                json_escape(&intent.source)
            ));
            body.push_str("      }");
        }
        body.push_str("\n    }");
    }
    body.push_str("\n  ],\n  \"duplicate_groups\": [");
    if duplicate_groups.is_empty() {
        body.push(']');
    } else {
        body.push('\n');
        for (group_index, group) in duplicate_groups.iter().enumerate() {
            if group_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("    {\n");
            body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(&group.id)));
            body.push_str("      \"members\": [\n");
            for (member_index, member) in group.members.iter().enumerate() {
                if member_index > 0 {
                    body.push_str(",\n");
                }
                body.push_str("        {\n");
                body.push_str(&format!(
                    "          \"path\": \"{}\",\n",
                    json_escape(&member.path)
                ));
                body.push_str(&format!(
                    "          \"name\": \"{}\",\n",
                    json_escape(&member.name)
                ));
                body.push_str(&format!("          \"line\": {}\n", member.line));
                body.push_str("        }");
            }
            body.push_str("\n      ],\n      \"shared_evidence\": {\n");
            body.push_str("        \"owners\": [");
            write_json_string_array(&mut body, &group.shared_evidence.owners);
            body.push_str("],\n");
            body.push_str(&format!(
                "        \"oracle_kind\": \"{}\",\n",
                json_escape(&group.shared_evidence.oracle_kind)
            ));
            body.push_str(&format!(
                "        \"oracle_strength\": \"{}\",\n",
                group.shared_evidence.oracle_strength
            ));
            body.push_str("        \"activation_signature\": [\n");
            for (activation_index, activation) in group
                .shared_evidence
                .activation_signature
                .iter()
                .enumerate()
            {
                if activation_index > 0 {
                    body.push_str(",\n");
                }
                body.push_str("          {\n");
                body.push_str(&format!(
                    "            \"context\": \"{}\",\n",
                    activation.context
                ));
                body.push_str(&format!(
                    "            \"value\": \"{}\"\n",
                    json_escape(&activation.value)
                ));
                body.push_str("          }");
            }
            body.push_str("\n        ]\n      },\n");
            body.push_str(&format!(
                "      \"suggested_next_step\": \"{}\"\n",
                json_escape(&group.suggested_next_step)
            ));
            body.push_str("    }");
        }
        body.push_str("\n  ]");
    }
    body.push_str(",\n  \"test_intent\": {\n");
    body.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(TEST_INTENT_PATH)
    ));
    body.push_str(&format!("    \"declared\": {},\n", test_intent.declared));
    body.push_str(&format!("    \"matched\": {}\n", test_intent.matched));
    body.push_str("  }");
    body.push_str(",\n  \"metrics\": {\n");
    body.push_str(&format!(
        "    \"tests_scanned\": {},\n",
        metrics.tests_scanned
    ));
    body.push_str("    \"class_counts\": {\n");
    body.push_str(&format!(
        "      \"strong_discriminator\": {},\n",
        counts.get("strong_discriminator").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"useful_but_broad\": {},\n",
        counts.get("useful_but_broad").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"smoke_only\": {},\n",
        counts.get("smoke_only").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"likely_vacuous\": {},\n",
        counts.get("likely_vacuous").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"possibly_circular\": {},\n",
        counts.get("possibly_circular").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"duplicative\": {},\n",
        counts.get("duplicative").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"opaque\": {}\n",
        counts.get("opaque").copied().unwrap_or(0)
    ));
    body.push_str("    },\n    \"reason_counts\": {");
    for (index, (reason, count)) in reason_counts.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&format!("\n      \"{}\": {}", json_escape(reason), count));
    }
    if reason_counts.is_empty() {
        body.push('}');
    } else {
        body.push_str("\n    }");
    }
    body.push_str(&format!(
        ",\n    \"duplicate_discriminator_group_count\": {}\n",
        metrics.duplicate_discriminator_group_count
    ));
    body.push_str("  }\n}\n");
    body
}

/// Run the repo seam inventory and write
/// `target/ripr/reports/repo-seams.{json,md}` per RIPR-SPEC-0005.
/// Shells out to the ripr CLI's `check --format repo-seams-*` paths
/// (the inventory walker is crate-private, so xtask cannot call it
/// directly).
pub(crate) fn repo_seam_inventory_impl() -> Result<(), String> {
    let json_args = repo_seam_inventory_command_args("repo-seams-json");
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("repo-seams.json", &json_output)?;

    let md_args = repo_seam_inventory_command_args("repo-seams-md");
    let md_output = run_output_owned("cargo", &md_args)?;
    write_report("repo-seams.md", &md_output)
}

fn repo_seam_inventory_command_args(format: &str) -> Vec<String> {
    repo_seam_inventory_command_args_for_root(format, ".")
}

fn repo_seam_inventory_command_args_for_root(format: &str, root: &str) -> Vec<String> {
    // Mirrors `repo_badge_artifact_command_args`: no `--diff` / `--base`
    // because the seam inventory must not depend on
    // `git diff origin/main...HEAD`.
    vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--format".to_string(),
        format.to_string(),
    ]
}

/// Run the repo exposure report (classified seam inventory) and
/// write `target/ripr/reports/repo-exposure.{json,md}` per
/// RIPR-SPEC-0005. Same CLI shell-out pattern as
/// `repo_seam_inventory`, but routes through the
/// `repo-exposure-json|md` formats which compute test-grip evidence
/// and `SeamGripClass` per seam. This is the full evidence-heavy path;
/// ordinary local metrics should use `repo_exposure_summary_report_impl`.
pub(crate) fn repo_exposure_report_impl() -> Result<(), String> {
    let json_args = repo_seam_inventory_command_args("repo-exposure-json");
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("repo-exposure.json", &json_output)?;

    let md_args = repo_seam_inventory_command_args("repo-exposure-md");
    let md_output = run_output_owned("cargo", &md_args)?;
    write_report("repo-exposure.md", &md_output)
}

/// Run the bounded repo exposure summary report and write
/// `target/ripr/reports/repo-exposure-summary.json`.
pub(crate) fn repo_exposure_summary_report_impl() -> Result<(), String> {
    let json_args = repo_seam_inventory_command_args("repo-exposure-summary-json");
    let timeout = Duration::from_millis(repo_exposure_summary_report_timeout_ms());
    write_repo_exposure_summary_report_with_runner(
        &json_args,
        timeout,
        run_repo_exposure_summary_report_command,
    )
}

const REPO_EXPOSURE_SUMMARY_REPORT_TIMEOUT_ENV: &str = "RIPR_REPO_EXPOSURE_SUMMARY_TIMEOUT_MS";
const REPO_EXPOSURE_SUMMARY_REPORT_DEFAULT_TIMEOUT_MS: u64 = 240_000;
const REPO_EXPOSURE_SUMMARY_REPORT_SCHEMA_VERSION: &str = "0.1";

fn repo_exposure_summary_report_timeout_ms() -> u64 {
    repo_exposure_summary_report_timeout_ms_from_env(
        std::env::var(REPO_EXPOSURE_SUMMARY_REPORT_TIMEOUT_ENV).ok(),
    )
}

fn repo_exposure_summary_report_timeout_ms_from_env(value: Option<String>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(REPO_EXPOSURE_SUMMARY_REPORT_DEFAULT_TIMEOUT_MS)
}

fn run_repo_exposure_summary_report_command(
    args: &[String],
    timeout: Duration,
) -> Result<TimedOutput, String> {
    capture_output_with_timeout(
        "cargo",
        args,
        &[(REPO_EXPOSURE_LATENCY_TRACE_ENV, "1")],
        timeout,
        "repo exposure summary report",
    )
}

fn write_repo_exposure_summary_report_with_runner<F>(
    args: &[String],
    timeout: Duration,
    mut run_summary: F,
) -> Result<(), String>
where
    F: FnMut(&[String], Duration) -> Result<TimedOutput, String>,
{
    let command = format!("cargo {}", args.join(" "));
    let started = Instant::now();
    let output = match run_summary(args, timeout) {
        Ok(output) => output,
        Err(err) => {
            let output = TimedOutput {
                status: None,
                stdout: String::new(),
                stderr: err.clone(),
                duration: started.elapsed(),
                timed_out: false,
            };
            return write_limited_repo_exposure_summary_report(
                &command,
                timeout,
                &output,
                "repo_exposure_summary_runner_error",
                Some("runner_error"),
                Some(&err),
            );
        }
    };

    if output.timed_out {
        return write_limited_repo_exposure_summary_report(
            &command,
            timeout,
            &output,
            "repo_exposure_summary_timeout",
            None,
            None,
        );
    }

    let Some(status) = output.status else {
        return write_limited_repo_exposure_summary_report(
            &command,
            timeout,
            &output,
            "repo_exposure_summary_incomplete",
            Some("missing_exit_status"),
            Some("repo exposure summary generation finished without an exit status"),
        );
    };

    if !status.success() {
        return write_limited_repo_exposure_summary_report(
            &command,
            timeout,
            &output,
            "repo_exposure_summary_incomplete",
            None,
            Some("repo exposure summary generation exited non-zero"),
        );
    }

    if let Err(err) = validate_repo_exposure_summary_stdout(&output.stdout) {
        return write_limited_repo_exposure_summary_report(
            &command,
            timeout,
            &output,
            "repo_exposure_summary_incomplete",
            Some("pass_incomplete"),
            Some(&err),
        );
    }

    write_report("repo-exposure-summary.json", &output.stdout)
}

fn validate_repo_exposure_summary_stdout(stdout: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|err| format!("failed to parse repo-exposure-summary-json stdout: {err}"))?;
    let format = value.get("format").and_then(Value::as_str);
    if format != Some("repo-exposure-summary-json") {
        return Err(format!(
            "repo exposure summary stdout used unexpected format {format:?}"
        ));
    }
    let basis = value.get("basis").and_then(Value::as_str);
    if basis != Some("canonical_actionable_gap") {
        return Err(format!(
            "repo exposure summary stdout used unexpected basis {basis:?}"
        ));
    }
    if !value.get("metrics").is_some_and(Value::is_object) {
        return Err("repo exposure summary stdout is missing metrics object".to_string());
    }
    if !value.get("top_files").is_some_and(Value::is_array) {
        return Err("repo exposure summary stdout is missing top_files array".to_string());
    }
    Ok(())
}

fn write_limited_repo_exposure_summary_report(
    command: &str,
    timeout: Duration,
    output: &TimedOutput,
    limitation: &str,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<(), String> {
    write_report(
        "repo-exposure-summary.json",
        &limited_repo_exposure_summary_report_json(
            command,
            timeout,
            output,
            limitation,
            generation_status,
            failure_reason,
        )?,
    )
}

fn limited_repo_exposure_summary_report_json(
    command: &str,
    timeout: Duration,
    output: &TimedOutput,
    limitation: &str,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<String, String> {
    let runtime_state = repo_exposure_summary_limited_runtime_state(limitation);
    let summary = repo_exposure_summary_limited_summary(limitation);
    let repair_route = repo_exposure_summary_limited_repair_route(limitation);
    let (latency_trace_events_total, latency_trace_tail) =
        evidence_health_latency_trace_tail(output);
    let value = serde_json::json!({
        "schema_version": REPO_EXPOSURE_SUMMARY_REPORT_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "repo-exposure-summary",
        "format": "repo-exposure-summary-json",
        "scope": "repo",
        "basis": "limited_runtime_status",
        "status": "warn",
        "run_status": runtime_state,
        "runtime_status": {
            "state": runtime_state,
            "phase": "repo_exposure_summary_generation",
            "duration_ms": output.duration.as_millis(),
            "limit_ms": timeout.as_millis(),
            "input_kind": "repo-exposure-summary-json",
            "input_path": "target/ripr/reports/repo-exposure-summary.json",
            "limitation_category": limitation,
            "repair_route": repair_route,
            "downstream_consumable": false,
        },
        "generation": {
            "command": command,
            "timeout_ms": timeout.as_millis(),
            "status": generation_status.unwrap_or(if output.timed_out { "timeout" } else { "fail" }),
            "duration_ms": output.duration.as_millis(),
            "timed_out": output.timed_out,
            "exit_code": output.status.and_then(|status| status.code()),
            "stdout_bytes": output.stdout.len(),
            "stderr_bytes": output.stderr.len(),
            "stdout_excerpt": evidence_health_output_excerpt(&output.stdout),
            "stderr_excerpt": evidence_health_output_excerpt(&output.stderr),
            "failure_reason": failure_reason,
            "latency_trace_events_total": latency_trace_events_total,
            "latency_trace_tail": latency_trace_tail
                .iter()
                .map(repo_exposure_latency_trace_json)
                .collect::<Vec<_>>(),
        },
        "metrics": {},
        "reason_breakdown": {},
        "limits": {
            "top_files_limit": 0,
            "top_files_total": 0,
            "top_files_truncated": false,
            "timeout_ms": timeout.as_millis(),
        },
        "top_files": [],
        "run_limitations": [
            {
                "category": limitation,
                "phase": "repo_exposure_summary_generation",
                "input": "repo-exposure-summary-json",
                "summary": summary,
                "repair_route": repair_route,
                "timeout_ms": timeout.as_millis(),
                "duration_ms": output.duration.as_millis(),
                "command": command,
                "exit_code": output.status.and_then(|status| status.code()),
                "stdout_bytes": output.stdout.len(),
                "stderr_bytes": output.stderr.len(),
                "stdout_excerpt": evidence_health_output_excerpt(&output.stdout),
                "stderr_excerpt": evidence_health_output_excerpt(&output.stderr),
                "failure_reason": failure_reason,
                "downstream_consumable": false,
                "latency_trace_events_total": latency_trace_events_total,
                "latency_trace_tail": latency_trace_tail
                    .iter()
                    .map(repo_exposure_latency_trace_json)
                    .collect::<Vec<_>>(),
            }
        ],
        "non_claims": [
            "not a canonical actionable gap count",
            "not raw seam inventory",
            "not runtime mutation confirmation",
            "not downstream consumable"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render limited repo exposure summary JSON: {err}"))
}

fn repo_exposure_summary_limited_runtime_state(limitation: &str) -> &'static str {
    match limitation {
        "repo_exposure_summary_timeout" => "limited_timeout",
        "repo_exposure_summary_runner_error" => "limited_runner_failure",
        _ => "limited_incomplete_input",
    }
}

fn repo_exposure_summary_limited_summary(limitation: &str) -> &'static str {
    match limitation {
        "repo_exposure_summary_timeout" => {
            "Repo exposure summary generation exceeded its bounded runtime; partial stdout was discarded and no repair debt count is claimed."
        }
        "repo_exposure_summary_runner_error" => {
            "Repo exposure summary generation could not be started or captured; no repair debt count is claimed."
        }
        _ => {
            "Repo exposure summary generation ended before producing a complete canonical actionable summary; no repair debt count is claimed."
        }
    }
}

fn repo_exposure_summary_limited_repair_route(limitation: &str) -> &'static str {
    match limitation {
        "repo_exposure_summary_timeout" => {
            "inspect repo-exposure summary runtime, rerun with RIPR_REPO_EXPOSURE_SUMMARY_TIMEOUT_MS for an explicit large-repo refresh, or use a fresh downstream-consumable summary artifact"
        }
        "repo_exposure_summary_runner_error" => {
            "inspect repo exposure summary command availability, report directory permissions, and child process capture before rerunning"
        }
        _ => {
            "inspect repo exposure summary exit status, stdout/stderr, and output schema before treating the artifact as planning input"
        }
    }
}

/// Run the Lane 1 evidence health report and write
/// `target/ripr/reports/evidence-health.{json,md}`. If an imported mutation
/// calibration report already exists, include it only as calibration
/// availability context.
pub(crate) fn evidence_health_report_impl() -> Result<(), String> {
    ensure_reports_dir()?;
    let binary = ripr_debug_binary();
    let args = evidence_health_args();
    let timeout = Duration::from_millis(evidence_health_timeout_ms());
    write_evidence_health_report_with_runners(
        &binary,
        &args,
        timeout,
        evidence_health_build_binary,
        evidence_health_run_binary,
    )
}

const EVIDENCE_HEALTH_TIMEOUT_ENV: &str = "RIPR_EVIDENCE_HEALTH_TIMEOUT_MS";
const EVIDENCE_HEALTH_SCHEMA_VERSION: &str = "0.2";
const EVIDENCE_HEALTH_DEFAULT_TIMEOUT_MS: u64 = 240_000;
const EVIDENCE_HEALTH_OUTPUT_EXCERPT_LINES: usize = 8;
const EVIDENCE_HEALTH_OUTPUT_EXCERPT_CHARS: usize = 4_000;
const EVIDENCE_HEALTH_LATENCY_TRACE_TAIL_LIMIT: usize = 12;

fn evidence_health_args() -> Vec<String> {
    let mut args = vec![
        "evidence-health".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--out".to_string(),
        "target/ripr/reports/evidence-health.json".to_string(),
        "--out-md".to_string(),
        "target/ripr/reports/evidence-health.md".to_string(),
    ];
    let calibration = Path::new("target/ripr/reports/mutation-calibration.json");
    if calibration.exists() {
        args.push("--mutation-calibration".to_string());
        args.push(calibration.display().to_string());
    }
    args
}

fn evidence_health_timeout_ms() -> u64 {
    std::env::var(EVIDENCE_HEALTH_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(EVIDENCE_HEALTH_DEFAULT_TIMEOUT_MS)
}

fn evidence_health_build_binary(timeout: Duration) -> Result<TimedOutput, String> {
    capture_output_with_timeout(
        "cargo",
        &["build".to_string(), "-p".to_string(), "ripr".to_string()],
        &[],
        timeout,
        "Lane 1 evidence-health build",
    )
}

fn evidence_health_run_binary(
    binary: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<TimedOutput, String> {
    let binary_text = binary.display().to_string();
    capture_output_with_timeout(
        &binary_text,
        args,
        &evidence_health_child_envs(),
        timeout,
        "Lane 1 evidence-health report",
    )
}

fn evidence_health_child_envs() -> [(&'static str, &'static str); 1] {
    [(REPO_EXPOSURE_LATENCY_TRACE_ENV, "1")]
}

fn write_evidence_health_report_with_runners<BuildRunner, ReportRunner>(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    mut build_ripr: BuildRunner,
    run_evidence_health: ReportRunner,
) -> Result<(), String>
where
    BuildRunner: FnMut(Duration) -> Result<TimedOutput, String>,
    ReportRunner: FnMut(&Path, &[String], Duration) -> Result<TimedOutput, String>,
{
    let build_started = Instant::now();
    let build_output = match build_ripr(timeout) {
        Ok(output) => output,
        Err(err) => {
            return write_limited_evidence_health_reports_for_runner_error(
                "cargo build -p ripr",
                "evidence_health_build",
                timeout,
                build_started.elapsed(),
                err,
            );
        }
    };
    if build_output.timed_out {
        return write_limited_evidence_health_reports_for_command(
            "cargo build -p ripr",
            "evidence_health_build",
            timeout,
            &build_output,
        );
    }
    match build_output.status {
        Some(status) if status.success() => {
            write_evidence_health_report_with_runner(binary, args, timeout, run_evidence_health)
        }
        Some(_) | None => write_limited_evidence_health_reports_for_command(
            "cargo build -p ripr",
            "evidence_health_build",
            timeout,
            &build_output,
        ),
    }
}

fn write_evidence_health_report_with_runner<F>(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    mut run_evidence_health: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &[String], Duration) -> Result<TimedOutput, String>,
{
    remove_evidence_health_report_artifacts();
    let started = Instant::now();
    let output = match run_evidence_health(binary, args, timeout) {
        Ok(output) => output,
        Err(err) => {
            return write_limited_evidence_health_reports_for_runner_error(
                &normalize_report_path(&format!("{} {}", binary.display(), args.join(" "))),
                "evidence_health_generation",
                timeout,
                started.elapsed(),
                err,
            );
        }
    };
    if output.timed_out {
        return write_limited_evidence_health_reports(binary, args, timeout, &output);
    }
    match output.status {
        Some(status) if status.success() => match evidence_health_report_artifacts_are_complete() {
            Ok(()) => Ok(()),
            Err(reason) => write_limited_evidence_health_reports_with_status(
                binary,
                args,
                timeout,
                &output,
                Some("pass_incomplete"),
                Some(&reason),
            ),
        },
        Some(_) | None => write_limited_evidence_health_reports(binary, args, timeout, &output),
    }
}

fn write_limited_evidence_health_reports(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    output: &TimedOutput,
) -> Result<(), String> {
    write_limited_evidence_health_reports_with_status(binary, args, timeout, output, None, None)
}

fn write_limited_evidence_health_reports_with_status(
    binary: &Path,
    args: &[String],
    timeout: Duration,
    output: &TimedOutput,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<(), String> {
    write_limited_evidence_health_reports_for_command_with_status(
        &normalize_report_path(&format!("{} {}", binary.display(), args.join(" "))),
        "evidence_health_generation",
        timeout,
        output,
        generation_status,
        failure_reason,
    )
}

fn write_limited_evidence_health_reports_for_command(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
) -> Result<(), String> {
    write_limited_evidence_health_reports_for_command_with_status(
        command, phase, timeout, output, None, None,
    )
}

fn write_limited_evidence_health_reports_for_runner_error(
    command: &str,
    phase: &str,
    timeout: Duration,
    duration: Duration,
    err: String,
) -> Result<(), String> {
    let output = TimedOutput {
        status: None,
        stdout: String::new(),
        stderr: err.clone(),
        duration,
        timed_out: false,
    };
    write_limited_evidence_health_reports_for_command_with_status_and_kind(
        command,
        phase,
        timeout,
        &output,
        Some("runner_error"),
        Some(&err),
        Some("evidence_health_runner_error"),
    )
}

fn write_limited_evidence_health_reports_for_command_with_status(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
) -> Result<(), String> {
    write_limited_evidence_health_reports_for_command_with_status_and_kind(
        command,
        phase,
        timeout,
        output,
        generation_status,
        failure_reason,
        None,
    )
}

fn write_limited_evidence_health_reports_for_command_with_status_and_kind(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
    limitation_kind: Option<&str>,
) -> Result<(), String> {
    remove_evidence_health_report_artifacts();
    write_report(
        "evidence-health.json",
        &limited_evidence_health_json(
            command,
            phase,
            timeout,
            output,
            generation_status,
            failure_reason,
            limitation_kind,
        )?,
    )?;
    write_report(
        "evidence-health.md",
        &limited_evidence_health_markdown(
            command,
            phase,
            timeout,
            output,
            failure_reason,
            limitation_kind,
        ),
    )
}

fn remove_evidence_health_report_artifacts() {
    let json_path = Path::new("target/ripr/reports/evidence-health.json");
    let md_path = Path::new("target/ripr/reports/evidence-health.md");
    let _ = fs::remove_file(json_path);
    let _ = fs::remove_file(md_path);
}

fn evidence_health_report_artifacts_are_complete() -> Result<(), String> {
    let json_path = Path::new("target/ripr/reports/evidence-health.json");
    let md_path = Path::new("target/ripr/reports/evidence-health.md");
    let json_text = fs::read_to_string(json_path).map_err(|err| {
        format!(
            "failed to read evidence-health JSON artifact {}: {err}",
            json_path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&json_text).map_err(|err| {
        format!(
            "failed to parse evidence-health JSON artifact {}: {err}",
            json_path.display()
        )
    })?;
    if value.get("schema_version").and_then(Value::as_str) != Some(EVIDENCE_HEALTH_SCHEMA_VERSION) {
        return Err(format!(
            "evidence-health JSON artifact {} did not use schema_version {}",
            json_path.display(),
            EVIDENCE_HEALTH_SCHEMA_VERSION
        ));
    }
    if value.get("status").and_then(Value::as_str) != Some("advisory") {
        return Err(format!(
            "evidence-health JSON artifact {} did not report advisory status",
            json_path.display()
        ));
    }
    validate_complete_evidence_health_json(&value, json_path)?;

    let markdown = fs::read_to_string(md_path).map_err(|err| {
        format!(
            "failed to read evidence-health Markdown artifact {}: {err}",
            md_path.display()
        )
    })?;
    if markdown.trim().is_empty() {
        return Err(format!(
            "evidence-health Markdown artifact {} was empty",
            md_path.display()
        ));
    }
    if !evidence_health_markdown_reports_advisory(&markdown) {
        return Err(format!(
            "evidence-health Markdown artifact {} did not report advisory status",
            md_path.display()
        ));
    }
    Ok(())
}

fn evidence_health_markdown_reports_advisory(markdown: &str) -> bool {
    if markdown.contains("Status: advisory") {
        return true;
    }
    markdown.lines().any(|line| {
        let cells = line
            .split('|')
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .collect::<Vec<_>>();
        cells.len() >= 2 && cells[0] == "Status" && cells[1] == "advisory"
    })
}

fn validate_complete_evidence_health_json(value: &Value, json_path: &Path) -> Result<(), String> {
    for path in [
        &["inputs"][..],
        &["metrics"],
        &["evidence_quality"],
        &["calibration"],
    ] {
        require_json_object(value, path, json_path)?;
    }
    require_json_array(value, &["top_static_limitations"], json_path)?;

    for path in [&["inputs", "root"][..], &["calibration", "status"]] {
        require_json_string(value, path, json_path)?;
    }
    require_json_present(value, &["inputs", "mutation_calibration"], json_path)?;

    for path in [
        &["metrics", "seams_total"][..],
        &["metrics", "headline_eligible_total"],
        &["metrics", "weakly_gripped_total"],
        &["metrics", "ungripped_total"],
        &["metrics", "missing_discriminators_total"],
        &["metrics", "seams_with_missing_discriminators"],
        &["metrics", "observed_values_total"],
        &["metrics", "seams_with_observed_values"],
        &["metrics", "related_tests_total"],
        &["metrics", "seams_with_related_tests"],
        &["metrics", "opaque_oracle_count"],
    ] {
        require_json_number(value, path, json_path)?;
    }

    for path in [
        &["metrics", "grip_class_counts"][..],
        &["metrics", "stage_state_counts"],
        &["metrics", "unknown_stage_counts"],
        &["metrics", "unknown_stop_reason_counts"],
        &["metrics", "observed_value_context_counts"],
        &["metrics", "related_test_confidence_counts"],
        &["metrics", "oracle_strength_counts"],
        &["metrics", "oracle_kind_counts"],
        &["evidence_quality", "actionability_class_counts"],
        &["evidence_quality", "static_limitation_stage_counts"],
        &["evidence_quality", "static_limitation_category_counts"],
        &["evidence_quality", "calibration_availability_counts"],
        &["evidence_quality", "movement_availability"],
    ] {
        require_json_object(value, path, json_path)?;
    }

    for path in [
        &["metrics", "missing_discriminator_counts"][..],
        &["evidence_quality", "largest_canonical_groups"],
        &["evidence_quality", "static_limitation_reason_counts"],
        &["evidence_quality", "top_evidence_quality_risks"],
    ] {
        require_json_array(value, path, json_path)?;
    }

    for path in [
        &["evidence_quality", "canonical_gap_groups_total"][..],
        &["evidence_quality", "duplicate_looking_groups_total"],
        &[
            "evidence_quality",
            "movement_availability",
            "records_with_seam_id",
        ],
        &[
            "evidence_quality",
            "movement_availability",
            "records_with_canonical_gap_id",
        ],
        &[
            "evidence_quality",
            "movement_availability",
            "records_with_complete_evidence_path",
        ],
        &[
            "evidence_quality",
            "movement_availability",
            "records_with_recommendation",
        ],
        &[
            "evidence_quality",
            "movement_availability",
            "records_with_verify_command",
        ],
        &["calibration", "matched_total"],
        &["calibration", "static_without_runtime_total"],
        &["calibration", "runtime_without_static_total"],
        &["calibration", "ambiguous_file_line_total"],
        &["calibration", "unmatched_runtime_total"],
    ] {
        require_json_number(value, path, json_path)?;
    }

    require_json_present(value, &["calibration", "source"], json_path)?;
    Ok(())
}

fn require_json_present<'a>(
    value: &'a Value,
    path: &[&str],
    json_path: &Path,
) -> Result<&'a Value, String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment).ok_or_else(|| {
            format!(
                "evidence-health JSON artifact {} is missing `{}`",
                json_path.display(),
                path.join(".")
            )
        })?;
    }
    Ok(current)
}

fn require_json_object(value: &Value, path: &[&str], json_path: &Path) -> Result<(), String> {
    let field = require_json_present(value, path, json_path)?;
    if field.is_object() {
        Ok(())
    } else {
        Err(format!(
            "evidence-health JSON artifact {} expected `{}` to be an object",
            json_path.display(),
            path.join(".")
        ))
    }
}

fn require_json_array(value: &Value, path: &[&str], json_path: &Path) -> Result<(), String> {
    let field = require_json_present(value, path, json_path)?;
    if field.is_array() {
        Ok(())
    } else {
        Err(format!(
            "evidence-health JSON artifact {} expected `{}` to be an array",
            json_path.display(),
            path.join(".")
        ))
    }
}

fn require_json_number(value: &Value, path: &[&str], json_path: &Path) -> Result<(), String> {
    let field = require_json_present(value, path, json_path)?;
    if field.is_number() {
        Ok(())
    } else {
        Err(format!(
            "evidence-health JSON artifact {} expected `{}` to be a number",
            json_path.display(),
            path.join(".")
        ))
    }
}

fn require_json_string(value: &Value, path: &[&str], json_path: &Path) -> Result<(), String> {
    let field = require_json_present(value, path, json_path)?;
    if field.is_string() {
        Ok(())
    } else {
        Err(format!(
            "evidence-health JSON artifact {} expected `{}` to be a string",
            json_path.display(),
            path.join(".")
        ))
    }
}

fn limited_evidence_health_json(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
    limitation_kind: Option<&str>,
) -> Result<String, String> {
    let limitation = limitation_kind.unwrap_or_else(|| evidence_health_limited_kind(output));
    let summary = evidence_health_limited_summary(limitation);
    let repair_route = evidence_health_limited_repair_route(limitation);
    let (latency_trace_events_total, latency_trace_tail) =
        evidence_health_latency_trace_tail(output);
    let value = serde_json::json!({
        "schema_version": EVIDENCE_HEALTH_SCHEMA_VERSION,
        "tool": "ripr",
        "scope": "repo",
        "status": "warn",
        "inputs": {
            "root": ".",
            "mutation_calibration": null,
            "generation": evidence_health_generation_json(
                command,
                phase,
                timeout,
                output,
                generation_status,
                failure_reason
            ),
        },
        "metrics": {
            "seams_total": 0,
            "headline_eligible_total": 0,
            "weakly_gripped_total": 0,
            "ungripped_total": 0,
            "grip_class_counts": {},
            "stage_state_counts": {},
            "unknown_stage_counts": {},
            "unknown_stop_reason_counts": {},
            "missing_discriminators_total": 0,
            "seams_with_missing_discriminators": 0,
            "missing_discriminator_counts": {},
            "observed_values_total": 0,
            "seams_with_observed_values": 0,
            "observed_value_context_counts": {},
            "related_tests_total": 0,
            "seams_with_related_tests": 0,
            "related_test_confidence_counts": {},
            "oracle_strength_counts": {},
            "oracle_kind_counts": {},
            "opaque_oracle_count": 0,
        },
        "evidence_quality": {
            "canonical_gap_groups_total": 0,
            "duplicate_looking_groups_total": 0,
            "largest_canonical_groups": [],
            "actionability_class_counts": {},
            "static_limitation_stage_counts": {},
            "static_limitation_reason_counts": {},
            "static_limitation_category_counts": {
                limitation: 1
            },
            "calibration_availability_counts": {},
            "movement_availability": {
                "records_with_seam_id": 0,
                "records_with_canonical_gap_id": 0,
                "records_with_complete_evidence_path": 0,
                "records_with_recommendation": 0,
                "records_with_verify_command": 0,
            },
            "top_evidence_quality_risks": [
                {
                    "kind": limitation,
                    "count": 1,
                    "summary": summary
                }
            ],
        },
        "calibration": {
            "status": "not_evaluated",
            "source": null,
            "matched_total": 0,
            "static_without_runtime_total": 0,
            "runtime_without_static_total": 0,
            "ambiguous_file_line_total": 0,
            "unmatched_runtime_total": 0,
        },
        "top_static_limitations": [
            {
                "kind": limitation,
                "count": 1,
                "summary": format!("{summary} No user test debt is claimed from this limited artifact."),
                "example_seam_id": null,
                "repair_route": repair_route
            }
        ],
        "run_limitations": [
            {
                "category": limitation,
                "phase": phase,
                "input": "repo",
                "summary": format!("{summary} Partial outputs were discarded."),
                "repair_route": repair_route,
                "timeout_ms": timeout.as_millis(),
                "duration_ms": output.duration.as_millis(),
                "command": command,
                "exit_code": output.status.and_then(|status| status.code()),
                "stdout_bytes": output.stdout.len(),
                "stderr_bytes": output.stderr.len(),
                "stdout_excerpt": evidence_health_output_excerpt(&output.stdout),
                "stderr_excerpt": evidence_health_output_excerpt(&output.stderr),
                "failure_reason": failure_reason,
                "latency_trace_events_total": latency_trace_events_total,
                "latency_trace_tail": latency_trace_tail
                    .iter()
                    .map(repo_exposure_latency_trace_json)
                    .collect::<Vec<_>>(),
            }
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render limited evidence-health JSON: {err}"))
}

fn evidence_health_generation_json(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
    generation_status: Option<&str>,
    failure_reason: Option<&str>,
) -> Value {
    let (latency_trace_events_total, latency_trace_tail) =
        evidence_health_latency_trace_tail(output);
    serde_json::json!({
        "command": command,
        "phase": phase,
        "timeout_ms": timeout.as_millis(),
        "status": generation_status.unwrap_or(if output.timed_out { "timeout" } else { "fail" }),
        "duration_ms": output.duration.as_millis(),
        "exit_code": output.status.and_then(|status| status.code()),
        "stdout_bytes": output.stdout.len(),
        "stderr_bytes": output.stderr.len(),
        "stdout_excerpt": evidence_health_output_excerpt(&output.stdout),
        "stderr_excerpt": evidence_health_output_excerpt(&output.stderr),
        "failure_reason": failure_reason,
        "latency_trace_events_total": latency_trace_events_total,
        "latency_trace_tail": latency_trace_tail
            .iter()
            .map(repo_exposure_latency_trace_json)
            .collect::<Vec<_>>(),
    })
}

fn evidence_health_latency_trace_tail(
    output: &TimedOutput,
) -> (usize, Vec<RepoExposureLatencyTrace>) {
    let trace = repo_exposure_latency_trace(&output.stderr);
    let trace_tail_start = trace
        .len()
        .saturating_sub(EVIDENCE_HEALTH_LATENCY_TRACE_TAIL_LIMIT);
    (trace.len(), trace[trace_tail_start..].to_vec())
}

fn evidence_health_output_excerpt(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    let mut lines = text
        .lines()
        .rev()
        .take(EVIDENCE_HEALTH_OUTPUT_EXCERPT_LINES)
        .collect::<Vec<_>>();
    lines.reverse();
    let joined = lines.join("\n");
    let char_count = joined.chars().count();
    if char_count <= EVIDENCE_HEALTH_OUTPUT_EXCERPT_CHARS {
        return Some(joined);
    }
    let tail = joined
        .chars()
        .rev()
        .take(EVIDENCE_HEALTH_OUTPUT_EXCERPT_CHARS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    Some(format!("[truncated]\n{tail}"))
}

fn evidence_health_limited_kind(output: &TimedOutput) -> &'static str {
    if output.timed_out {
        "evidence_health_timeout"
    } else {
        "evidence_health_incomplete"
    }
}

fn evidence_health_limited_summary(kind: &str) -> &'static str {
    match kind {
        "evidence_health_timeout" => {
            "Evidence-health generation timed out before a complete report was available."
        }
        "evidence_health_incomplete" => {
            "Evidence-health generation ended before producing a complete report."
        }
        "evidence_health_runner_error" => {
            "Evidence-health runner failed before producing a complete report."
        }
        _ => "Evidence-health generation did not produce a complete report.",
    }
}

fn evidence_health_limited_repair_route(kind: &str) -> &'static str {
    match kind {
        "evidence_health_timeout" => {
            "inspect evidence-health runtime, increase RIPR_EVIDENCE_HEALTH_TIMEOUT_MS for slower machines, or add a narrower fixture-backed analyzer path"
        }
        "evidence_health_incomplete" => {
            "inspect evidence-health exit status, stdout/stderr, and live repo size; rerun with RIPR_EVIDENCE_HEALTH_TIMEOUT_MS or add a bounded fixture-backed analyzer path"
        }
        "evidence_health_runner_error" => {
            "inspect local runner process setup, temp/output capture, and child process permissions; rerun evidence-health after the runner can start, capture, poll, and read the child"
        }
        _ => "inspect evidence-health runtime and rerun with bounded diagnostics",
    }
}

fn limited_evidence_health_markdown(
    command: &str,
    phase: &str,
    timeout: Duration,
    output: &TimedOutput,
    failure_reason: Option<&str>,
    limitation_kind: Option<&str>,
) -> String {
    let limitation = limitation_kind.unwrap_or_else(|| evidence_health_limited_kind(output));
    let summary = evidence_health_limited_summary(limitation);
    let repair_route = evidence_health_limited_repair_route(limitation);
    let mut out = String::new();
    out.push_str("# RIPR evidence health report\n\n");
    out.push_str("Status: warn\n\n");
    out.push_str(summary);
    out.push_str(
        " Partial outputs were discarded, and no user test debt is claimed from this limited artifact.\n\n",
    );
    out.push_str("## Run Limitation\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!("| Category | `{limitation}` |\n"));
    out.push_str(&format!("| Phase | `{phase}` |\n"));
    out.push_str(&format!("| Timeout | {} ms |\n", timeout.as_millis()));
    out.push_str(&format!(
        "| Duration | {} ms |\n",
        output.duration.as_millis()
    ));
    let exit = output
        .status
        .and_then(|status| status.code())
        .map(|code| code.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    out.push_str(&format!("| Exit code | {} |\n", exit));
    out.push_str(&format!(
        "| Command | `{}` |\n",
        audit_markdown_cell(command)
    ));
    if let Some(reason) = failure_reason {
        out.push_str(&format!(
            "| Failure reason | {} |\n",
            audit_markdown_cell(reason)
        ));
    }
    out.push_str(&format!("| Repair route | {repair_route} |\n\n"));
    if !output.stderr.trim().is_empty() {
        out.push_str("## Stderr Tail\n\n```text\n");
        for line in output
            .stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("```\n");
    }
    let (latency_trace_events_total, latency_trace_tail) =
        evidence_health_latency_trace_tail(output);
    if !latency_trace_tail.is_empty() {
        out.push_str("\n## Repo-exposure Latency Trace Tail\n\n");
        out.push_str(&format!(
            "Captured {} repo-exposure latency trace events.\n\n",
            latency_trace_events_total
        ));
        out.push_str("| Phase | Status | Duration |\n");
        out.push_str("| --- | --- | ---: |\n");
        for trace in latency_trace_tail {
            out.push_str(&format!(
                "| `{}` | `{}` | {} ms |\n",
                audit_markdown_cell(&trace.phase),
                audit_markdown_cell(&trace.status),
                trace.duration_ms
            ));
        }
    }
    out
}

const REPO_EXPOSURE_LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";
const REPO_EXPOSURE_LATENCY_TIMEOUT_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS";
const REPO_EXPOSURE_LATENCY_DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Debug)]
struct RepoExposureLatencyReport {
    status: String,
    timeout_ms: u64,
    binary: String,
    runs: Vec<RepoExposureLatencyRun>,
}

#[derive(Clone, Debug)]
struct RepoExposureLatencyRun {
    format: String,
    status: String,
    duration_ms: u128,
    exit_code: Option<i32>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    trace: Vec<RepoExposureLatencyTrace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoExposureLatencyTrace {
    phase: String,
    status: String,
    duration_ms: u128,
}

/// Write a bounded repo exposure latency report without changing the
/// repo-exposure JSON/Markdown schemas. This command is diagnostic:
/// it reports timeouts as `warn` in its own report instead of blocking
/// the operator lane indefinitely.
pub(crate) fn repo_exposure_latency_report_impl() -> Result<(), String> {
    let timeout_ms = repo_exposure_latency_timeout_ms();
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = ripr_debug_binary();
    write_repo_exposure_latency_report(&binary, timeout_ms, repo_exposure_latency_run)
}

fn write_repo_exposure_latency_report<F>(
    binary: &Path,
    timeout_ms: u64,
    run_format: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &str, Duration) -> Result<RepoExposureLatencyRun, String>,
{
    let report = build_repo_exposure_latency_report(binary, timeout_ms, run_format)?;
    write_report(
        "repo-exposure-latency.json",
        &repo_exposure_latency_json(&report),
    )?;
    write_report(
        "repo-exposure-latency.md",
        &repo_exposure_latency_markdown(&report),
    )
}

fn build_repo_exposure_latency_report<F>(
    binary: &Path,
    timeout_ms: u64,
    mut run_format: F,
) -> Result<RepoExposureLatencyReport, String>
where
    F: FnMut(&Path, &str, Duration) -> Result<RepoExposureLatencyRun, String>,
{
    let binary_display = binary.display().to_string();
    let timeout = Duration::from_millis(timeout_ms);

    let mut runs = Vec::new();
    let json_run = run_format(binary, "repo-exposure-json", timeout)?;
    let should_run_markdown = json_run.status != "timeout";
    runs.push(json_run);
    if should_run_markdown {
        runs.push(run_format(binary, "repo-exposure-md", timeout)?);
    } else {
        runs.push(RepoExposureLatencyRun {
            format: "repo-exposure-md".to_string(),
            status: "skipped_after_json_timeout".to_string(),
            duration_ms: 0,
            exit_code: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            trace: Vec::new(),
        });
    }

    let report = RepoExposureLatencyReport {
        status: repo_exposure_latency_status(&runs),
        timeout_ms,
        binary: binary_display,
        runs,
    };
    Ok(report)
}

fn repo_exposure_latency_timeout_ms() -> u64 {
    std::env::var(REPO_EXPOSURE_LATENCY_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(REPO_EXPOSURE_LATENCY_DEFAULT_TIMEOUT_MS)
}

fn ripr_debug_binary() -> PathBuf {
    ripr_debug_binary_in(std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from))
}

fn ripr_debug_binary_in(target_dir: Option<PathBuf>) -> PathBuf {
    let binary_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    target_dir
        .unwrap_or_else(|| PathBuf::from("target"))
        .join("debug")
        .join(binary_name)
}

fn repo_exposure_latency_run(
    binary: &Path,
    format: &str,
    timeout: Duration,
) -> Result<RepoExposureLatencyRun, String> {
    let args = vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    let binary_text = binary.display().to_string();
    let envs = [(REPO_EXPOSURE_LATENCY_TRACE_ENV, "1")];
    let output = capture_output_with_timeout(&binary_text, &args, &envs, timeout, format)?;
    Ok(repo_exposure_latency_run_from_output(format, output))
}

fn repo_exposure_latency_run_from_output(
    format: &str,
    output: TimedOutput,
) -> RepoExposureLatencyRun {
    let status = if output.timed_out {
        "timeout"
    } else if output.status.is_some_and(|status| status.success()) {
        "pass"
    } else {
        "fail"
    };
    RepoExposureLatencyRun {
        format: format.to_string(),
        status: status.to_string(),
        duration_ms: output.duration.as_millis(),
        exit_code: output.status.and_then(|status| status.code()),
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        trace: repo_exposure_latency_trace(&output.stderr),
    }
}

fn repo_exposure_latency_status(runs: &[RepoExposureLatencyRun]) -> String {
    if runs.iter().any(|run| run.status == "fail") {
        return "fail".to_string();
    }
    if runs
        .iter()
        .any(|run| run.status == "timeout" || run.status == "skipped_after_json_timeout")
    {
        return "warn".to_string();
    }
    "pass".to_string()
}

fn repo_exposure_latency_trace(stderr: &str) -> Vec<RepoExposureLatencyTrace> {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("ripr_repo_exposure_latency ")?;
            let mut phase: Option<String> = None;
            let mut status: Option<String> = None;
            let mut duration_ms: Option<u128> = None;
            for field in rest.split_whitespace() {
                if let Some(value) = field.strip_prefix("phase=") {
                    phase = Some(value.to_string());
                } else if let Some(value) = field.strip_prefix("status=") {
                    status = Some(value.to_string());
                } else if let Some(value) = field.strip_prefix("duration_ms=") {
                    duration_ms = value.parse::<u128>().ok();
                }
            }
            Some(RepoExposureLatencyTrace {
                phase: phase?,
                status: status?,
                duration_ms: duration_ms?,
            })
        })
        .collect()
}

fn repo_exposure_latency_json(report: &RepoExposureLatencyReport) -> String {
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"tool\": \"ripr\",\n");
    body.push_str("  \"report\": \"repo-exposure-latency\",\n");
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(&report.status)
    ));
    body.push_str(&format!("  \"timeout_ms\": {},\n", report.timeout_ms));
    body.push_str(&format!(
        "  \"binary\": \"{}\",\n",
        json_escape(&normalize_report_path(&report.binary))
    ));
    body.push_str("  \"runs\": [\n");
    for (index, run) in report.runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"format\": \"{}\",\n",
            json_escape(&run.format)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!("      \"duration_ms\": {},\n", run.duration_ms));
        match run.exit_code {
            Some(code) => body.push_str(&format!("      \"exit_code\": {},\n", code)),
            None => body.push_str("      \"exit_code\": null,\n"),
        }
        body.push_str(&format!("      \"stdout_bytes\": {},\n", run.stdout_bytes));
        body.push_str(&format!("      \"stderr_bytes\": {},\n", run.stderr_bytes));
        body.push_str("      \"trace\": [");
        for (trace_index, trace) in run.trace.iter().enumerate() {
            if trace_index > 0 {
                body.push_str(", ");
            }
            body.push_str(&format!(
                "{{\"phase\": \"{}\", \"status\": \"{}\", \"duration_ms\": {}}}",
                json_escape(&trace.phase),
                json_escape(&trace.status),
                trace.duration_ms
            ));
        }
        body.push_str("]\n");
        body.push_str("    }");
    }
    body.push_str("\n  ]\n");
    body.push_str("}\n");
    body
}

fn repo_exposure_latency_markdown(report: &RepoExposureLatencyReport) -> String {
    let mut body = String::new();
    body.push_str("# Repo Exposure Latency Report\n\n");
    body.push_str(&format!("Status: `{}`\n\n", report.status));
    body.push_str(&format!(
        "Timeout: `{}` ms per format\n\n",
        report.timeout_ms
    ));
    body.push_str(&format!(
        "Binary: `{}`\n\n",
        normalize_report_path(&report.binary)
    ));
    body.push_str("| Format | Status | Duration | Exit | Stdout | Stderr |\n");
    body.push_str("| --- | --- | ---: | ---: | ---: | ---: |\n");
    for run in &report.runs {
        let exit = run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        body.push_str(&format!(
            "| `{}` | `{}` | {} ms | {} | {} bytes | {} bytes |\n",
            run.format, run.status, run.duration_ms, exit, run.stdout_bytes, run.stderr_bytes
        ));
    }
    body.push_str("\n## Analyzer Trace\n\n");
    if report.runs.iter().all(|run| run.trace.is_empty()) {
        body.push_str("No analyzer trace lines were captured before the command ended.\n");
    } else {
        for run in &report.runs {
            if run.trace.is_empty() {
                continue;
            }
            body.push_str(&format!("### `{}`\n\n", run.format));
            body.push_str("| Phase | Status | Duration |\n");
            body.push_str("| --- | --- | ---: |\n");
            for trace in &run.trace {
                body.push_str(&format!(
                    "| `{}` | `{}` | {} ms |\n",
                    trace.phase, trace.status, trace.duration_ms
                ));
            }
            body.push('\n');
        }
    }
    body.push_str("\n## Next Step\n\n");
    body.push_str(
        "Use this report to identify whether the repo-exposure path is waiting on \
cache collection, cache load, cold compute, cache store, or rendering before \
changing cache behavior.\n",
    );
    body
}

/// Run the agent seam packet renderer and write
/// `target/ripr/reports/agent-seam-packets.json`.
pub(crate) fn agent_seam_packets_report_impl(root: Option<&String>) -> Result<(), String> {
    let root = root.map_or(".", String::as_str);
    let json_args = repo_seam_inventory_command_args_for_root("agent-seam-packets-json", root);
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("agent-seam-packets.json", &json_output)
}

#[derive(Clone, Debug)]
struct LspCockpitReport {
    status: String,
    fixtures: Vec<LspCockpitFixture>,
    vscode: LspCockpitVscodeCoverage,
}

#[derive(Clone, Debug)]
struct LspCockpitFixture {
    fixture: String,
    diagnostics_path: String,
    code_actions_path: String,
    diagnostic_count: usize,
    seam_diagnostic_count: usize,
    finding_diagnostic_count: usize,
    seam_ids: Vec<String>,
    grip_classes: Vec<String>,
    action_titles: Vec<String>,
    action_commands: Vec<String>,
    action_argument_fields: Vec<String>,
    context: LspCockpitContext,
}

#[derive(Clone, Debug, Default)]
struct LspCockpitContext {
    seam_packet_available: bool,
    targeted_test_brief_available: bool,
    agent_packet_command_available: bool,
    agent_brief_command_available: bool,
    after_snapshot_command_available: bool,
    agent_verify_command_available: bool,
    agent_receipt_command_available: bool,
    assertion_available: bool,
    related_test_available: bool,
    refresh_available: bool,
}

impl LspCockpitContext {
    fn agent_loop_commands_available(&self) -> bool {
        self.agent_packet_command_available
            && self.agent_brief_command_available
            && self.after_snapshot_command_available
            && self.agent_verify_command_available
            && self.agent_receipt_command_available
    }
}

#[derive(Clone, Debug)]
struct LspCockpitVscodeCoverage {
    test_file: String,
    contributed_commands: Vec<String>,
    covered_commands: Vec<String>,
    covered_contributed_commands: Vec<String>,
    uncovered_contributed_commands: Vec<String>,
}

pub(crate) fn lsp_cockpit_report_impl() -> Result<(), String> {
    let report = build_lsp_cockpit_report()?;
    write_report("lsp-cockpit.json", &lsp_cockpit_report_json(&report)?)?;
    write_report("lsp-cockpit.md", &lsp_cockpit_report_markdown(&report))
}

fn build_lsp_cockpit_report() -> Result<LspCockpitReport, String> {
    let mut fixtures = Vec::new();
    for (name, fixture) in lsp_cockpit_fixture_dirs()? {
        if let Some(report) = lsp_cockpit_fixture_report(&name, &fixture)? {
            fixtures.push(report);
        }
    }
    let vscode = lsp_cockpit_vscode_coverage()?;
    let has_missing_agent_loop_commands = fixtures.iter().any(|fixture| {
        fixture.seam_diagnostic_count > 0 && !fixture.context.agent_loop_commands_available()
    });
    let status = if fixtures.is_empty()
        || has_missing_agent_loop_commands
        || !vscode.uncovered_contributed_commands.is_empty()
    {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    Ok(LspCockpitReport {
        status,
        fixtures,
        vscode,
    })
}

fn lsp_cockpit_fixture_dirs() -> Result<Vec<(String, PathBuf)>, String> {
    let mut fixtures = Vec::new();
    for fixture in fixture_dirs()? {
        let name = fixture
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid fixture path {}", fixture.display()))?
            .to_string();
        fixtures.push((name, fixture));
    }
    let editor_gap_cockpit = Path::new("fixtures/editor_gap_cockpit");
    if editor_gap_cockpit.exists() {
        for entry in fs::read_dir(editor_gap_cockpit)
            .map_err(|err| format!("failed to read fixtures/editor_gap_cockpit: {err}"))?
        {
            let entry = entry
                .map_err(|err| format!("failed to read fixtures/editor_gap_cockpit: {err}"))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid fixture path {}", path.display()))?;
            fixtures.push((format!("editor_gap_cockpit/{name}"), path));
        }
    }
    let editor_first_pr_bridge = Path::new("fixtures/editor_first_pr_bridge");
    if editor_first_pr_bridge.exists() {
        for entry in fs::read_dir(editor_first_pr_bridge)
            .map_err(|err| format!("failed to read fixtures/editor_first_pr_bridge: {err}"))?
        {
            let entry = entry
                .map_err(|err| format!("failed to read fixtures/editor_first_pr_bridge: {err}"))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid fixture path {}", path.display()))?;
            fixtures.push((format!("editor_first_pr_bridge/{name}"), path));
        }
    }
    let editor_adoption_assurance = Path::new("fixtures/editor_adoption_assurance");
    if editor_adoption_assurance.exists() {
        for entry in fs::read_dir(editor_adoption_assurance)
            .map_err(|err| format!("failed to read fixtures/editor_adoption_assurance: {err}"))?
        {
            let entry = entry.map_err(|err| {
                format!("failed to read fixtures/editor_adoption_assurance: {err}")
            })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid fixture path {}", path.display()))?;
            fixtures.push((format!("editor_adoption_assurance/{name}"), path));
        }
    }
    fixtures.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(fixtures)
}

fn lsp_cockpit_fixture_report(
    fixture_name: &str,
    fixture: &Path,
) -> Result<Option<LspCockpitFixture>, String> {
    let expected = fixture.join("expected");
    let diagnostics_path = expected.join("lsp-diagnostics.json");
    let code_actions_path = expected.join("lsp-code-actions.json");
    if !diagnostics_path.exists() && !code_actions_path.exists() {
        return Ok(None);
    }
    if !diagnostics_path.exists() || !code_actions_path.exists() {
        return Err(format!(
            "{} has partial LSP cockpit fixtures; expected both lsp-diagnostics.json and lsp-code-actions.json",
            normalize_path(fixture)
        ));
    }

    let diagnostics_json = read_lsp_cockpit_json_value(&diagnostics_path)?;
    let code_actions_json = read_lsp_cockpit_json_value(&code_actions_path)?;
    let diagnostics = diagnostics_json
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "{} is missing a diagnostics array",
                normalize_path(&diagnostics_path)
            )
        })?;
    let actions = code_actions_json
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "{} is missing an actions array",
                normalize_path(&code_actions_path)
            )
        })?;

    let mut seam_ids = BTreeSet::new();
    let mut grip_classes = BTreeSet::new();
    let mut seam_diagnostic_count = 0;
    let mut finding_diagnostic_count = 0;
    for diagnostic in diagnostics {
        let data = diagnostic.get("data").unwrap_or(&Value::Null);
        if let Some(seam_id) = json_str_field(data, "seam_id") {
            seam_diagnostic_count += 1;
            seam_ids.insert(seam_id.to_string());
        }
        if json_str_field(data, "finding_id").is_some() {
            finding_diagnostic_count += 1;
        }
        if let Some(class) =
            json_str_field(data, "grip_class").or_else(|| json_str_field(data, "classification"))
        {
            grip_classes.insert(class.to_string());
        }
    }

    let mut action_titles = Vec::new();
    let mut action_commands = Vec::new();
    let mut action_argument_fields = BTreeSet::new();
    let mut context = LspCockpitContext::default();
    for action in actions {
        let title = json_str_field(action, "title").unwrap_or("unknown");
        let command = json_str_field(action, "command").unwrap_or("unknown");
        action_titles.push(title.to_string());
        action_commands.push(command.to_string());
        if let Some(arguments) = action.get("arguments").and_then(Value::as_array) {
            for argument in arguments {
                if let Some(object) = argument.as_object() {
                    for key in object.keys() {
                        action_argument_fields.insert(key.clone());
                    }
                }
            }
        }
        match command {
            "ripr.copyContext" if title == "Inspect Test Gap - Copy Context" => {
                context.seam_packet_available = true;
            }
            "ripr.copyTargetedTestBrief" => {
                context.targeted_test_brief_available = action_has_string_argument(action, "brief");
            }
            "ripr.copyAgentPacketCommand" => {
                context.agent_packet_command_available =
                    action_has_string_argument(action, "command");
            }
            "ripr.copyAgentBriefCommand" => {
                context.agent_brief_command_available =
                    action_has_string_argument(action, "command");
            }
            "ripr.copyAfterSnapshotCommand" => {
                context.after_snapshot_command_available =
                    action_has_string_argument(action, "command");
            }
            "ripr.copyAgentVerifyCommand" => {
                context.agent_verify_command_available =
                    action_has_string_argument(action, "command");
            }
            "ripr.copyAgentReceiptCommand" => {
                context.agent_receipt_command_available =
                    action_has_string_argument(action, "command");
            }
            "ripr.copySuggestedAssertion" => {
                context.assertion_available = action_has_string_argument(action, "assertion");
            }
            "ripr.openRelatedTest" => {
                context.related_test_available = action_has_string_argument(action, "uri");
            }
            "ripr.refresh" => {
                context.refresh_available = true;
            }
            _ => {}
        }
    }

    Ok(Some(LspCockpitFixture {
        fixture: fixture_name.to_string(),
        diagnostics_path: normalize_path(&diagnostics_path),
        code_actions_path: normalize_path(&code_actions_path),
        diagnostic_count: diagnostics.len(),
        seam_diagnostic_count,
        finding_diagnostic_count,
        seam_ids: seam_ids.into_iter().collect(),
        grip_classes: grip_classes.into_iter().collect(),
        action_titles,
        action_commands,
        action_argument_fields: action_argument_fields.into_iter().collect(),
        context,
    }))
}

fn read_lsp_cockpit_json_value(path: &Path) -> Result<Value, String> {
    let text = read_text_lossy(path)?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", normalize_path(path)))
}

fn json_str_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn action_has_string_argument(action: &Value, field: &str) -> bool {
    action
        .get("arguments")
        .and_then(Value::as_array)
        .is_some_and(|arguments| {
            arguments
                .iter()
                .any(|argument| json_str_field(argument, field).is_some())
        })
}

fn lsp_cockpit_vscode_coverage() -> Result<LspCockpitVscodeCoverage, String> {
    let test_file = Path::new("editors/vscode/test/suite/extension.test.ts");
    let test_text = read_text_lossy(test_file)?;
    let contributed_commands = vscode_contributed_commands()?;
    let covered_commands = ripr_command_literals_in_text(&test_text);
    let covered_set = covered_commands.iter().collect::<BTreeSet<_>>();
    let covered_contributed_commands = contributed_commands
        .iter()
        .filter(|command| covered_set.contains(command))
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_contributed_commands = contributed_commands
        .iter()
        .filter(|command| !covered_set.contains(command))
        .cloned()
        .collect::<Vec<_>>();
    Ok(LspCockpitVscodeCoverage {
        test_file: normalize_path(test_file),
        contributed_commands,
        covered_commands,
        covered_contributed_commands,
        uncovered_contributed_commands,
    })
}

fn vscode_contributed_commands() -> Result<Vec<String>, String> {
    let package = read_lsp_cockpit_json_value(Path::new("editors/vscode/package.json"))?;
    let commands = package
        .get("contributes")
        .and_then(|value| value.get("commands"))
        .and_then(Value::as_array)
        .ok_or_else(|| "editors/vscode/package.json is missing contributes.commands".to_string())?;
    let mut out = BTreeSet::new();
    for command in commands {
        if let Some(id) = json_str_field(command, "command")
            && id.starts_with("ripr.")
        {
            out.insert(id.to_string());
        }
    }
    Ok(out.into_iter().collect())
}

fn ripr_command_literals_in_text(text: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    collect_quoted_prefixed_strings(text, "ripr.", '\'', &mut out);
    collect_quoted_prefixed_strings(text, "ripr.", '"', &mut out);
    out.into_iter().collect()
}

fn collect_quoted_prefixed_strings(
    text: &str,
    prefix: &str,
    quote: char,
    out: &mut BTreeSet<String>,
) {
    let marker = format!("{quote}{prefix}");
    let mut search_start = 0;
    while let Some(relative_start) = text[search_start..].find(&marker) {
        let value_start = search_start + relative_start + quote.len_utf8();
        let after_start = &text[value_start..];
        let Some(relative_end) = after_start.find(quote) else {
            break;
        };
        out.insert(after_start[..relative_end].to_string());
        search_start = value_start + relative_end + quote.len_utf8();
    }
}

fn lsp_cockpit_report_json(report: &LspCockpitReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": report.status.as_str(),
        "fixtures": report.fixtures.iter().map(|fixture| {
            serde_json::json!({
                "fixture": fixture.fixture.as_str(),
                "diagnostics_path": fixture.diagnostics_path.as_str(),
                "code_actions_path": fixture.code_actions_path.as_str(),
                "diagnostics": {
                    "total": fixture.diagnostic_count,
                    "seams": fixture.seam_diagnostic_count,
                    "findings": fixture.finding_diagnostic_count,
                    "seam_ids": fixture.seam_ids,
                    "grip_classes": fixture.grip_classes
                },
                "actions": {
                    "titles": fixture.action_titles,
                    "commands": fixture.action_commands,
                    "argument_fields": fixture.action_argument_fields
                },
                "context": {
                    "seam_packet_available": fixture.context.seam_packet_available,
                    "targeted_test_brief_available": fixture.context.targeted_test_brief_available,
                    "agent_packet_command_available": fixture.context.agent_packet_command_available,
                    "agent_brief_command_available": fixture.context.agent_brief_command_available,
                    "after_snapshot_command_available": fixture.context.after_snapshot_command_available,
                    "agent_verify_command_available": fixture.context.agent_verify_command_available,
                    "agent_receipt_command_available": fixture.context.agent_receipt_command_available,
                    "assertion_available": fixture.context.assertion_available,
                    "related_test_available": fixture.context.related_test_available,
                    "refresh_available": fixture.context.refresh_available
                }
            })
        }).collect::<Vec<_>>(),
        "vscode_e2e": {
            "test_file": report.vscode.test_file.as_str(),
            "contributed_commands": report.vscode.contributed_commands,
            "covered_commands": report.vscode.covered_commands,
            "covered_contributed_commands": report.vscode.covered_contributed_commands,
            "uncovered_contributed_commands": report.vscode.uncovered_contributed_commands
        }
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render LSP cockpit JSON: {err}"))
}

fn lsp_cockpit_report_markdown(report: &LspCockpitReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr LSP cockpit report\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    if report.fixtures.is_empty() {
        out.push_str("No fixtures with pinned LSP diagnostics/actions were found.\n\n");
    }
    for fixture in &report.fixtures {
        out.push_str(&format!("## Fixture: {}\n\n", md_escape(&fixture.fixture)));
        out.push_str("Diagnostics:\n");
        out.push_str(&format!("- total: {}\n", fixture.diagnostic_count));
        out.push_str(&format!(
            "- seam diagnostics: {}\n",
            fixture.seam_diagnostic_count
        ));
        out.push_str(&format!(
            "- finding diagnostics: {}\n",
            fixture.finding_diagnostic_count
        ));
        push_markdown_list_line(&mut out, "seam ids", &fixture.seam_ids);
        push_markdown_list_line(&mut out, "grip classes", &fixture.grip_classes);

        out.push_str("\nActions:\n");
        for (title, command) in fixture.action_titles.iter().zip(&fixture.action_commands) {
            out.push_str(&format!(
                "- {} (`{}`)\n",
                md_escape(title),
                md_escape(command)
            ));
        }
        push_markdown_list_line(
            &mut out,
            "action argument fields",
            &fixture.action_argument_fields,
        );

        out.push_str("\nContext:\n");
        out.push_str(&format!(
            "- seam packet available: {}\n",
            yes_no(fixture.context.seam_packet_available)
        ));
        out.push_str(&format!(
            "- targeted test brief available: {}\n",
            yes_no(fixture.context.targeted_test_brief_available)
        ));
        out.push_str(&format!(
            "- agent packet command available: {}\n",
            yes_no(fixture.context.agent_packet_command_available)
        ));
        out.push_str(&format!(
            "- agent brief command available: {}\n",
            yes_no(fixture.context.agent_brief_command_available)
        ));
        out.push_str(&format!(
            "- after-snapshot command available: {}\n",
            yes_no(fixture.context.after_snapshot_command_available)
        ));
        out.push_str(&format!(
            "- agent verify command available: {}\n",
            yes_no(fixture.context.agent_verify_command_available)
        ));
        out.push_str(&format!(
            "- agent receipt command available: {}\n",
            yes_no(fixture.context.agent_receipt_command_available)
        ));
        out.push_str(&format!(
            "- assertion available: {}\n",
            yes_no(fixture.context.assertion_available)
        ));
        out.push_str(&format!(
            "- related test available: {}\n",
            yes_no(fixture.context.related_test_available)
        ));
        out.push_str(&format!(
            "- refresh available: {}\n",
            yes_no(fixture.context.refresh_available)
        ));
        out.push('\n');
    }

    out.push_str("## VS Code e2e\n\n");
    out.push_str(&format!(
        "- test file: `{}`\n",
        md_escape(&report.vscode.test_file)
    ));
    push_markdown_list_line(
        &mut out,
        "contributed commands",
        &report.vscode.contributed_commands,
    );
    push_markdown_list_line(
        &mut out,
        "covered commands",
        &report.vscode.covered_commands,
    );
    push_markdown_list_line(
        &mut out,
        "covered contributed commands",
        &report.vscode.covered_contributed_commands,
    );
    push_markdown_list_line(
        &mut out,
        "uncovered contributed commands",
        &report.vscode.uncovered_contributed_commands,
    );
    out
}

fn push_markdown_list_line(out: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        out.push_str(&format!("- {label}: none\n"));
    } else {
        out.push_str(&format!(
            "- {label}: {}\n",
            values
                .iter()
                .map(|value| format!("`{}`", md_escape(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

const SEAM_GRIP_CLASS_ORDER: &[&str] = &[
    "strongly_gripped",
    "weakly_gripped",
    "ungripped",
    "reachable_unrevealed",
    "activation_unknown",
    "propagation_unknown",
    "observation_unknown",
    "discrimination_unknown",
    "opaque",
    "intentional",
    "suppressed",
];

pub(crate) fn targeted_test_outcome_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_targeted_test_outcome_args(args)?;
    let before_text = read_text_lossy(&parsed.before)?;
    let after_text = read_text_lossy(&parsed.after)?;
    let before = parse_repo_exposure_static_seams(&before_text)?;
    let after = parse_repo_exposure_static_seams(&after_text)?;
    let report = build_targeted_test_outcome_report(
        &before,
        &after,
        normalize_path(&parsed.before),
        normalize_path(&parsed.after),
    )?;
    write_report(
        "targeted-test-outcome.json",
        &targeted_test_outcome_report_json(&report)?,
    )?;
    write_report(
        "targeted-test-outcome.md",
        &targeted_test_outcome_report_markdown(&report),
    )
}

fn parse_targeted_test_outcome_args(args: &[String]) -> Result<TargetedTestOutcomeArgs, String> {
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--before" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        targeted_test_outcome_usage()
                    ));
                };
                before = Some(PathBuf::from(path));
            }
            "--after" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        targeted_test_outcome_usage()
                    ));
                };
                after = Some(PathBuf::from(path));
            }
            "--help" | "-h" => return Err(targeted_test_outcome_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown targeted-test-outcome option `{flag}`\n{}",
                    targeted_test_outcome_usage()
                ));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument `{other}`\n{}",
                    targeted_test_outcome_usage()
                ));
            }
        }
        index += 1;
    }

    let Some(before) = before else {
        return Err(format!(
            "targeted-test-outcome requires `--before <path>`\n{}",
            targeted_test_outcome_usage()
        ));
    };
    let Some(after) = after else {
        return Err(format!(
            "targeted-test-outcome requires `--after <path>`\n{}",
            targeted_test_outcome_usage()
        ));
    };

    Ok(TargetedTestOutcomeArgs { before, after })
}

fn targeted_test_outcome_usage() -> String {
    "usage: cargo xtask targeted-test-outcome --before <repo-exposure-json> --after <repo-exposure-json>"
        .to_string()
}

fn build_targeted_test_outcome_report(
    before: &[StaticSeamRecord],
    after: &[StaticSeamRecord],
    before_path: String,
    after_path: String,
) -> Result<TargetedTestOutcomeReport, String> {
    let before_by_id = targeted_outcome_seams_by_id(before, "before")?;
    let after_by_id = targeted_outcome_seams_by_id(after, "after")?;
    let mut moved = Vec::new();
    let mut unchanged = Vec::new();
    let mut regressed = Vec::new();
    let mut removed = Vec::new();

    for (seam_id, before_seam) in &before_by_id {
        match after_by_id.get(seam_id) {
            Some(after_seam) => {
                let movement = targeted_test_outcome_movement(before_seam, after_seam);
                if movement.before == movement.after {
                    unchanged.push(movement);
                } else if targeted_outcome_grip_rank(&movement.after)
                    < targeted_outcome_grip_rank(&movement.before)
                {
                    regressed.push(movement);
                } else {
                    moved.push(movement);
                }
            }
            None => removed.push(targeted_test_outcome_seam(before_seam)),
        }
    }

    let mut new = Vec::new();
    for (seam_id, after_seam) in &after_by_id {
        if !before_by_id.contains_key(seam_id) {
            new.push(targeted_test_outcome_seam(after_seam));
        }
    }

    Ok(TargetedTestOutcomeReport {
        before_path,
        after_path,
        before_counts: targeted_outcome_class_counts(before),
        after_counts: targeted_outcome_class_counts(after),
        moved,
        unchanged,
        regressed,
        new,
        removed,
    })
}

fn targeted_outcome_seams_by_id(
    seams: &[StaticSeamRecord],
    label: &str,
) -> Result<BTreeMap<String, StaticSeamRecord>, String> {
    let mut out = BTreeMap::new();
    for seam in seams {
        if out.insert(seam.seam_id.clone(), seam.clone()).is_some() {
            return Err(format!(
                "{label} repo exposure JSON contains duplicate seam_id `{}`",
                seam.seam_id
            ));
        }
    }
    Ok(out)
}

fn targeted_outcome_class_counts(seams: &[StaticSeamRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    counts.insert("seams_total".to_string(), seams.len());
    for class in SEAM_GRIP_CLASS_ORDER {
        counts.insert((*class).to_string(), 0);
    }
    for seam in seams {
        *counts.entry(seam.seam_grip_class.clone()).or_insert(0) += 1;
    }
    counts
}

fn targeted_test_outcome_movement(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
) -> TargetedTestOutcomeMovement {
    let before_rank = targeted_outcome_grip_rank(&before.seam_grip_class);
    let after_rank = targeted_outcome_grip_rank(&after.seam_grip_class);
    let direction = if before.seam_grip_class == after.seam_grip_class {
        "unchanged"
    } else if after_rank > before_rank {
        "improved"
    } else if after_rank < before_rank {
        "regressed"
    } else {
        "changed"
    };
    let evidence_delta = targeted_outcome_evidence_delta(before, after);
    TargetedTestOutcomeMovement {
        seam_id: before.seam_id.clone(),
        seam_kind: before.seam_kind.clone(),
        file: before.file.clone(),
        line: before.line,
        before: before.seam_grip_class.clone(),
        after: after.seam_grip_class.clone(),
        direction: direction.to_string(),
        evidence_delta,
    }
}

fn targeted_test_outcome_seam(seam: &StaticSeamRecord) -> TargetedTestOutcomeSeam {
    TargetedTestOutcomeSeam {
        seam_id: seam.seam_id.clone(),
        seam_kind: seam.seam_kind.clone(),
        file: seam.file.clone(),
        line: seam.line,
        grip_class: seam.seam_grip_class.clone(),
    }
}

fn targeted_outcome_grip_rank(class: &str) -> u8 {
    match class {
        "strongly_gripped" | "intentional" | "suppressed" => 7,
        "weakly_gripped" => 5,
        "reachable_unrevealed" => 4,
        "activation_unknown"
        | "propagation_unknown"
        | "observation_unknown"
        | "discrimination_unknown" => 3,
        "opaque" => 2,
        "ungripped" => 1,
        _ => 0,
    }
}

fn targeted_outcome_evidence_delta(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
) -> Vec<String> {
    let mut deltas = Vec::new();
    if before.seam_grip_class != after.seam_grip_class {
        deltas.push(format!(
            "grip class moved from {} to {}",
            before.seam_grip_class, after.seam_grip_class
        ));
    }

    let before_missing = before
        .missing_discriminators
        .iter()
        .collect::<BTreeSet<_>>();
    let after_missing = after.missing_discriminators.iter().collect::<BTreeSet<_>>();
    for value in before_missing.difference(&after_missing) {
        deltas.push(format!(
            "missing discriminator no longer reported: {}",
            md_escape(value)
        ));
    }
    for value in after_missing.difference(&before_missing) {
        deltas.push(format!(
            "new missing discriminator reported: {}",
            md_escape(value)
        ));
    }

    let before_values = before.observed_values.iter().collect::<BTreeSet<_>>();
    let after_values = after.observed_values.iter().collect::<BTreeSet<_>>();
    for value in after_values.difference(&before_values) {
        deltas.push(format!("new observed value: {}", md_escape(value)));
    }
    for value in before_values.difference(&after_values) {
        deltas.push(format!(
            "previous observed value absent: {}",
            md_escape(value)
        ));
    }

    let before_oracle_rank = oracle_strength_rank(&before.oracle_strength);
    let after_oracle_rank = oracle_strength_rank(&after.oracle_strength);
    if after_oracle_rank > before_oracle_rank {
        deltas.push(format!(
            "stronger related oracle visible: {} -> {}",
            before.oracle_strength, after.oracle_strength
        ));
    } else if after_oracle_rank < before_oracle_rank {
        deltas.push(format!(
            "related oracle strength decreased: {} -> {}",
            before.oracle_strength, after.oracle_strength
        ));
    } else if before.oracle_kind != after.oracle_kind {
        deltas.push(format!(
            "related oracle kind changed: {} -> {}",
            before.oracle_kind, after.oracle_kind
        ));
    }

    if deltas.is_empty() && before.seam_grip_class != after.seam_grip_class {
        deltas.push("grip class changed without rendered evidence details".to_string());
    }
    deltas
}

fn targeted_test_outcome_report_json(report: &TargetedTestOutcomeReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": report.before_path.as_str(),
            "after": report.after_path.as_str()
        },
        "before": report.before_counts,
        "after": report.after_counts,
        "summary": {
            "moved": report.moved.len(),
            "unchanged": report.unchanged.len(),
            "regressed": report.regressed.len(),
            "new": report.new.len(),
            "removed": report.removed.len()
        },
        "moved": report.moved.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "unchanged": report.unchanged.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "regressed": report.regressed.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "new": report.new.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>(),
        "removed": report.removed.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>(),
        "review_receipt": targeted_test_outcome_review_receipt_json(report)
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render targeted-test outcome JSON: {err}"))
}

fn targeted_test_outcome_movement_json(movement: &TargetedTestOutcomeMovement) -> Value {
    serde_json::json!({
        "seam_id": movement.seam_id.as_str(),
        "seam_kind": movement.seam_kind.as_str(),
        "file": movement.file.as_str(),
        "line": movement.line,
        "before": movement.before.as_str(),
        "after": movement.after.as_str(),
        "direction": movement.direction.as_str(),
        "evidence_delta": movement.evidence_delta
    })
}

fn targeted_test_outcome_seam_json(seam: &TargetedTestOutcomeSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id.as_str(),
        "seam_kind": seam.seam_kind.as_str(),
        "file": seam.file.as_str(),
        "line": seam.line,
        "grip_class": seam.grip_class.as_str()
    })
}

fn targeted_test_outcome_report_markdown(report: &TargetedTestOutcomeReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr targeted-test outcome report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str("Inputs:\n");
    out.push_str(&format!("- before: `{}`\n", md_escape(&report.before_path)));
    out.push_str(&format!("- after: `{}`\n\n", md_escape(&report.after_path)));

    out.push_str("## Summary\n\n");
    out.push_str("| Bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!("| moved | {} |\n", report.moved.len()));
    out.push_str(&format!("| unchanged | {} |\n", report.unchanged.len()));
    out.push_str(&format!("| regressed | {} |\n", report.regressed.len()));
    out.push_str(&format!("| new | {} |\n", report.new.len()));
    out.push_str(&format!("| removed | {} |\n", report.removed.len()));

    out.push_str("\n## Grip Counts\n\n");
    out.push_str("| Class | Before | After |\n| --- | ---: | ---: |\n");
    for class in std::iter::once("seams_total").chain(SEAM_GRIP_CLASS_ORDER.iter().copied()) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            class,
            report.before_counts.get(class).copied().unwrap_or(0),
            report.after_counts.get(class).copied().unwrap_or(0)
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

fn targeted_test_outcome_review_receipt_json(report: &TargetedTestOutcomeReport) -> Value {
    serde_json::json!({
        "what_changed": review_what_changed(report),
        "ripr_flagged_before": review_ripr_flagged_before(report),
        "focused_proof_added": review_focused_proof_added(report),
        "movement_after_verification": review_movement_after_verification(report),
        "remaining_weak_or_unknown": review_remaining_weak_or_unknown(report),
        "reviewer_should_inspect": review_should_inspect(report),
        "reviewer_should_not_believe": reviewer_should_not_believe()
    })
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
            "- `{}` {}:{} {} -> {} ({})\n",
            md_escape(&movement.seam_id),
            md_escape(&movement.file),
            movement.line,
            movement.before,
            movement.after,
            movement.direction
        ));
        for delta in &movement.evidence_delta {
            out.push_str(&format!("  - {}\n", md_escape(delta)));
        }
    }
}

fn push_targeted_outcome_review_receipt_md(out: &mut String, report: &TargetedTestOutcomeReport) {
    out.push_str("\n## Review Receipt\n\n");
    push_review_receipt_list_md(out, "What changed?", &review_what_changed(report));
    push_review_receipt_list_md(
        out,
        "What RIPR flagged before?",
        &review_ripr_flagged_before(report),
    );
    push_review_receipt_list_md(
        out,
        "What focused proof changed?",
        &review_focused_proof_added(report),
    );
    push_review_receipt_list_md(
        out,
        "What moved after verification?",
        &review_movement_after_verification(report),
    );
    push_review_receipt_list_md(
        out,
        "What remains weak or unknown?",
        &review_remaining_weak_or_unknown(report),
    );
    push_review_receipt_list_md(
        out,
        "Reviewer should inspect",
        &review_should_inspect(report),
    );
    push_review_receipt_list_md(
        out,
        "Reviewer should not believe",
        &reviewer_should_not_believe(),
    );
}

fn push_review_receipt_list_md(out: &mut String, title: &str, items: &[String]) {
    out.push_str(&format!("### {title}\n\n"));
    for item in items {
        out.push_str(&format!("- {}\n", md_escape(item)));
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
            "- `{}` {}:{} {} ({})\n",
            md_escape(&seam.seam_id),
            md_escape(&seam.file),
            seam.line,
            seam.grip_class,
            seam.seam_kind
        ));
    }
}

fn review_what_changed(report: &TargetedTestOutcomeReport) -> Vec<String> {
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

fn review_ripr_flagged_before(report: &TargetedTestOutcomeReport) -> Vec<String> {
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

fn review_focused_proof_added(report: &TargetedTestOutcomeReport) -> Vec<String> {
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
            "{} at {}:{} shows static evidence movement for focused proof: {}.",
            movement.seam_kind,
            movement.file,
            movement.line,
            proof_deltas.join("; ")
        ));
    }
    review_limit_or_default(
        items,
        "No focused proof signal was visible in the rendered static snapshots.",
    )
}

fn review_movement_after_verification(report: &TargetedTestOutcomeReport) -> Vec<String> {
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

fn review_remaining_weak_or_unknown(report: &TargetedTestOutcomeReport) -> Vec<String> {
    let mut items = Vec::new();
    for movement in report.unchanged.iter().chain(report.regressed.iter()) {
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

fn review_should_inspect(report: &TargetedTestOutcomeReport) -> Vec<String> {
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

fn reviewer_should_not_believe() -> Vec<String> {
    vec![
        "Runtime mutation result.".to_string(),
        "Coverage adequacy.".to_string(),
        "General correctness.".to_string(),
        "Merge approval.".to_string(),
        "That RIPR edited source or generated tests.".to_string(),
    ]
}

fn review_attention_class(class: &str) -> bool {
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

fn json_string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(json_scalar_as_string)
}

fn json_usize_field(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(json_scalar_as_usize)
}

fn json_bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_bool_summary_field(value: &Value, key: &str) -> Option<bool> {
    value
        .get("summary")
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_bool)
}

fn md_escape(value: &str) -> String {
    value.replace('`', "\\`").replace(['\r', '\n'], " ")
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let text = read_text_lossy(path)?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse JSON from {}: {err}", normalize_path(path)))
}

fn parse_repo_exposure_static_seams(json: &str) -> Result<Vec<StaticSeamRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo exposure JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "repo exposure JSON is missing `seams` array".to_string())?;

    let mut records = Vec::new();
    for seam in seams {
        let seam_id = required_json_string(seam, "seam_id")?;
        let seam_kind = required_json_string(seam, "kind")?;
        let file = normalize_report_path(&required_json_string(seam, "file")?);
        let line = required_json_usize(seam, "line")?;
        let seam_grip_class = required_json_string(seam, "grip_class")?;
        let (oracle_kind, oracle_strength) = strongest_related_oracle(seam);
        records.push(StaticSeamRecord {
            seam_id,
            seam_kind,
            file,
            line,
            seam_grip_class,
            oracle_kind,
            oracle_strength,
            observed_values: string_array_field(seam, "observed_values"),
            missing_discriminators: missing_discriminator_strings(seam),
        });
    }
    Ok(records)
}

fn required_json_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(json_scalar_as_string)
        .ok_or_else(|| format!("repo exposure seam is missing string field `{key}`"))
}

fn required_json_usize(value: &Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(json_scalar_as_usize)
        .ok_or_else(|| format!("repo exposure seam is missing numeric field `{key}`"))
}

fn strongest_related_oracle(seam: &Value) -> (String, String) {
    let mut best_kind = "unknown".to_string();
    let mut best_strength = "unknown".to_string();
    let mut best_rank = 0;

    if let Some(related) = seam.get("related_tests").and_then(Value::as_array) {
        for test in related {
            let strength = test
                .get("oracle_strength")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let rank = oracle_strength_rank(strength);
            if rank > best_rank {
                best_rank = rank;
                best_strength = strength.to_string();
                best_kind = test
                    .get("oracle_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
            }
        }
    }

    (best_kind, best_strength)
}

fn oracle_strength_rank(strength: &str) -> u8 {
    match strength {
        "strong" => 5,
        "medium" => 4,
        "weak" => 3,
        "smoke" => 2,
        "none" => 1,
        _ => 0,
    }
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(json_scalar_as_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn missing_discriminator_strings(seam: &Value) -> Vec<String> {
    seam.get("missing_discriminators")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(value) = json_scalar_as_string(item) {
                        return Some(value);
                    }
                    let value = item.get("value").and_then(json_scalar_as_string)?;
                    match item.get("reason").and_then(json_scalar_as_string) {
                        Some(reason) if !reason.is_empty() => Some(format!("{value} ({reason})")),
                        _ => Some(value),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn json_scalar_as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}

fn normalize_report_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(normalized.as_str())
        .to_string()
}

fn check_capabilities() -> Result<(), String> {
    let manifest = Path::new("metrics/capabilities.toml");
    let mut violations = Vec::new();
    if !manifest.exists() {
        violations.push("metrics/capabilities.toml is missing".to_string());
        return finish_capabilities_report(&violations);
    }
    let (capabilities, parse_violations) = parse_capabilities_manifest(manifest)?;
    violations.extend(parse_violations);
    validate_capabilities(&capabilities, &mut violations)?;
    validate_capability_matrix(&capabilities, &mut violations)?;
    finish_capabilities_report(&violations)
}

fn finish_capabilities_report(violations: &[String]) -> Result<(), String> {
    finish_policy_report(
        PolicyReportSpec {
            report_file: "capabilities.md",
            check: "check-capabilities",
            why_it_matters: "Capability status should be a checked source of truth, not README prose that can drift from specs and fixtures.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update metrics/capabilities.toml with status, spec, next checkpoint, and metric fields.",
                "Keep docs/CAPABILITY_MATRIX.md capability rows aligned with metrics/capabilities.toml.",
                "Keep capability statuses to planned, alpha, usable alpha, stable, or calibrated.",
                "Reference only specs that exist in docs/specs.",
                "Use cargo xtask metrics to regenerate target/ripr/reports/metrics.md and metrics.json.",
            ],
            rerun_command: "cargo xtask check-capabilities",
            exception_template: None,
        },
        violations,
    )
}

fn validate_capability_matrix(
    capabilities: &[Capability],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let matrix_path = Path::new("docs/CAPABILITY_MATRIX.md");
    if !matrix_path.exists() {
        violations.push("docs/CAPABILITY_MATRIX.md is missing".to_string());
        return Ok(());
    }

    let matrix = read_text_lossy(matrix_path)?;
    let Some(header_index) = matrix.lines().position(|line| {
        line.trim()
            == "| Capability | Status | Spec | Current evidence | Next checkpoint | Metric |"
    }) else {
        violations
            .push("docs/CAPABILITY_MATRIX.md is missing the capability table header".to_string());
        return Ok(());
    };

    let mut matrix_names = Vec::new();
    for line in matrix.lines().skip(header_index + 2) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            break;
        }
        let Some(name) = trimmed
            .split('|')
            .nth(1)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            violations.push(
                "docs/CAPABILITY_MATRIX.md contains a capability row without a name".to_string(),
            );
            continue;
        };
        matrix_names.push(name.to_string());
    }

    let manifest_names = capabilities
        .iter()
        .filter_map(|capability| capability.name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    let manifest_set = manifest_names.iter().copied().collect::<BTreeSet<_>>();
    let matrix_set = matrix_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    for name in manifest_set.difference(&matrix_set) {
        violations.push(format!(
            "docs/CAPABILITY_MATRIX.md is missing capability `{name}` from metrics/capabilities.toml"
        ));
    }
    for name in matrix_set.difference(&manifest_set) {
        violations.push(format!(
            "docs/CAPABILITY_MATRIX.md contains capability `{name}` absent from metrics/capabilities.toml"
        ));
    }
    for (name, count) in
        matrix_names
            .iter()
            .fold(BTreeMap::<&str, usize>::new(), |mut counts, name| {
                *counts.entry(name.as_str()).or_default() += 1;
                counts
            })
    {
        if count > 1 {
            violations.push(format!(
                "docs/CAPABILITY_MATRIX.md contains capability `{name}` {count} times"
            ));
        }
    }
    if matrix_names.len() != manifest_names.len() {
        violations.push(format!(
            "docs/CAPABILITY_MATRIX.md has {} capability rows but metrics/capabilities.toml has {} entries",
            matrix_names.len(),
            manifest_names.len()
        ));
    }
    Ok(())
}

fn validate_capabilities(
    capabilities: &[Capability],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let specs = collect_spec_statuses()?;
    let mut ids = BTreeSet::new();
    if capabilities.is_empty() {
        violations.push("metrics/capabilities.toml has no [[capability]] entries".to_string());
    }
    for capability in capabilities {
        let Some(id) = capability.id.as_ref() else {
            violations.push(format!(
                "capability at line {} is missing `id`",
                capability.line
            ));
            continue;
        };
        if !is_snake_case_id(id) {
            violations.push(format!(
                "capability at line {} has invalid id `{id}`; use snake_case",
                capability.line
            ));
        }
        if !ids.insert(id.clone()) {
            violations.push(format!("duplicate capability id `{id}`"));
        }
        if capability
            .name
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing a non-empty `name`"));
        }
        match capability.status.as_deref() {
            Some("planned" | "alpha" | "usable alpha" | "stable" | "calibrated") => {}
            Some(status) => violations.push(format!("{id} has unsupported status `{status}`")),
            None => violations.push(format!("{id} is missing `status`")),
        }
        match capability.spec.as_ref() {
            Some(spec) if is_spec_id(spec) && specs.contains_key(spec) => {}
            Some(spec) if is_spec_id(spec) => {
                violations.push(format!("{id} references missing spec `{spec}`"));
            }
            Some(spec) => violations.push(format!("{id} has invalid spec id `{spec}`")),
            None => violations.push(format!("{id} is missing `spec`")),
        }
        if capability
            .next
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing `next`"));
        }
        if capability
            .metric
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing `metric`"));
        }
        if capability.status.as_deref() != Some("planned") && capability.evidence.is_empty() {
            violations.push(format!("{id} is not planned but has no evidence entries"));
        }
        for fixture in &capability.fixtures {
            if !fixture.trim().is_empty() && !Path::new(fixture).exists() {
                violations.push(format!("{id} fixture path does not exist: {fixture}"));
            }
        }
        if matches!(
            capability.status.as_deref(),
            Some("usable alpha" | "stable")
        ) && capability.fixtures.is_empty()
        {
            violations.push(format!(
                "{id} is usable alpha or stable but has no fixture entries"
            ));
        }
        if capability.status.as_deref() == Some("calibrated")
            && !capability
                .evidence
                .iter()
                .any(|value| value.contains("calibration"))
        {
            violations.push(format!(
                "{id} is calibrated but has no calibration evidence entry"
            ));
        }
    }
    Ok(())
}

fn parse_capabilities_manifest(path: &Path) -> Result<(Vec<Capability>, Vec<String>), String> {
    let text = read_text_lossy(path)?;
    Ok(parse_capabilities_manifest_text(
        &normalize_path(path),
        &text,
    ))
}

fn parse_capabilities_manifest_text(
    path_label: &str,
    text: &str,
) -> (Vec<Capability>, Vec<String>) {
    let mut capabilities = Vec::new();
    let mut violations = Vec::new();
    let mut current: Option<Capability> = None;
    let mut active_array: Option<(String, Vec<String>, usize)> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, values, start_line)) = active_array.as_mut() {
            if trimmed.starts_with(']') {
                let Some(mut capability) = current.take() else {
                    violations.push(format!(
                        "{path_label}:{start_line} array `{key}` is outside a capability entry"
                    ));
                    active_array = None;
                    continue;
                };
                assign_capability_array(
                    &mut capability,
                    key,
                    values.clone(),
                    *start_line,
                    &mut violations,
                );
                current = Some(capability);
                active_array = None;
                continue;
            }
            match parse_array_item(trimmed) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(message) => violations.push(format!("{path_label}:{line_number} {message}")),
            }
            continue;
        }
        if trimmed == "[[capability]]" {
            if let Some(capability) = current.take() {
                capabilities.push(capability);
            }
            current = Some(Capability {
                line: line_number,
                ..Capability::default()
            });
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            violations.push(format!("{path_label}:{line_number} expected `key = value`"));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(capability) = current.as_mut() else {
            violations.push(format!(
                "{path_label}:{line_number} `{key}` appears outside a [[capability]] entry"
            ));
            continue;
        };
        if value == "[" {
            active_array = Some((key.to_string(), Vec::new(), line_number));
            continue;
        }
        if value.starts_with('[') {
            match parse_inline_array(value) {
                Ok(values) => {
                    assign_capability_array(capability, key, values, line_number, &mut violations);
                }
                Err(message) => violations.push(format!("{path_label}:{line_number} {message}")),
            }
            continue;
        }
        match parse_quoted_value(value) {
            Ok(parsed) => {
                assign_capability_string(capability, key, parsed, line_number, &mut violations);
            }
            Err(message) => violations.push(format!("{path_label}:{line_number} {message}")),
        }
    }

    if let Some((key, _, start_line)) = active_array {
        violations.push(format!(
            "{path_label}:{start_line} array `{key}` is missing closing `]`"
        ));
    }
    if let Some(capability) = current {
        capabilities.push(capability);
    }
    (capabilities, violations)
}

fn assign_capability_string(
    capability: &mut Capability,
    key: &str,
    value: String,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "id" => capability.id = Some(value),
        "name" => capability.name = Some(value),
        "status" => capability.status = Some(value),
        "spec" => capability.spec = Some(value),
        "next" => capability.next = Some(value),
        "metric" => capability.metric = Some(value),
        _ => violations.push(format!(
            "capability line {line_number} uses unsupported string field `{key}`"
        )),
    }
}

fn assign_capability_array(
    capability: &mut Capability,
    key: &str,
    values: Vec<String>,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "evidence" => capability.evidence = values,
        "fixtures" => capability.fixtures = values,
        _ => violations.push(format!(
            "capability line {line_number} uses unsupported array field `{key}`"
        )),
    }
}

fn capability_metrics_markdown(capabilities: &[Capability]) -> String {
    let mut body = "# ripr capability metrics\n\n".to_string();
    body.push_str("| Capability | Status | Spec | Evidence | Next | Metric |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for capability in capabilities {
        body.push_str(&format!(
            "| {} | `{}` | `{}` | {} | `{}` | {} |\n",
            markdown_cell(capability.name.as_deref().unwrap_or("")),
            markdown_cell(capability.status.as_deref().unwrap_or("")),
            markdown_cell(capability.spec.as_deref().unwrap_or("")),
            markdown_cell(&capability.evidence.join(", ")),
            markdown_cell(capability.next.as_deref().unwrap_or("")),
            markdown_cell(capability.metric.as_deref().unwrap_or(""))
        ));
    }
    body
}

fn capability_metrics_json(capabilities: &[Capability]) -> String {
    let mut body = "{\n  \"schema_version\": \"0.1\",\n  \"capabilities\": [\n".to_string();
    for (index, capability) in capabilities.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"id\": \"{}\",\n",
            json_escape(capability.id.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(capability.name.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(capability.status.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"spec\": \"{}\",\n",
            json_escape(capability.spec.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"next\": \"{}\",\n",
            json_escape(capability.next.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"metric\": \"{}\",\n",
            json_escape(capability.metric.as_deref().unwrap_or(""))
        ));
        body.push_str("      \"evidence\": [");
        write_json_string_array(&mut body, &capability.evidence);
        body.push_str("],\n");
        body.push_str("      \"fixtures\": [");
        write_json_string_array(&mut body, &capability.fixtures);
        body.push_str("]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

fn write_json_string_array(body: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push('"');
        body.push_str(&json_escape(value));
        body.push('"');
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

fn json_optional_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn is_snake_case_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !value.starts_with('_')
        && !value.ends_with('_')
        && !value.contains("__")
}

fn check_workspace_shape() -> Result<(), String> {
    let records = read_pipe_records("policy/workspace_shape.txt", 3)?;
    let mut allowed_members = BTreeSet::new();
    let mut allowed_manifests = Vec::new();
    let mut violations = Vec::new();

    for record in records {
        match record[0].as_str() {
            "workspace_member" => {
                allowed_members.insert(record[1].clone());
            }
            "cargo_manifest" => allowed_manifests.push(GlobAllow {
                glob: record[1].clone(),
            }),
            other => violations.push(format!(
                "policy/workspace_shape.txt uses unsupported kind `{other}`"
            )),
        }
    }

    for member in workspace_members()? {
        if !allowed_members.contains(&member) {
            violations.push(format!(
                "workspace member is not allowlisted: {member}\n  preferred: keep one published `crates/ripr` package and `xtask` automation unless an ADR approves a new package"
            ));
        }
    }
    for member in &allowed_members {
        if !Path::new(member).exists() {
            violations.push(format!(
                "allowlisted workspace member does not exist: {member}"
            ));
        }
    }
    for file in tracked_files()? {
        if !file.ends_with("Cargo.toml") {
            continue;
        }
        if !matches_any_glob(&allowed_manifests, &file) {
            violations.push(format!(
                "Cargo manifest is not allowlisted by workspace shape policy: {file}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "workspace-shape.md",
            check: "check-workspace-shape",
            why_it_matters: "ripr intentionally stays one published package with internal module seams; new packages need explicit review.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Keep product code inside crates/ripr unless an ADR approves a new package.",
                "Keep repo automation inside xtask.",
                "If a new Cargo manifest is truly needed, add a workspace-shape policy entry with owner and reason.",
            ],
            rerun_command: "cargo xtask check-workspace-shape",
            exception_template: Some("kind|path|reason"),
        },
        &violations,
    )
}

fn check_architecture() -> Result<(), String> {
    let rules = read_pipe_records("policy/architecture.txt", 3)?;
    let files = tracked_files()?;
    let mut violations = Vec::new();
    for rule in rules {
        let glob = &rule[0];
        let forbidden = &rule[1];
        let reason = &rule[2];
        for file in &files {
            if !glob_matches(glob, file) {
                continue;
            }
            let text = read_text_lossy(Path::new(file))?;
            if text.contains(forbidden) {
                violations.push(format!(
                    "{file} contains forbidden architecture pattern `{forbidden}`\n  reason: {reason}"
                ));
            }
        }
    }

    // RIPR-SPEC-0087 §8 (issue #2028): repair-packet authority coupling guard.
    for file in &files {
        if !file.starts_with("crates/ripr/src/") || !file.ends_with(".rs") {
            continue;
        }
        if file == REPAIR_PACKET_AUTHORITY_PATH
            || REPAIR_PACKET_AUTHORITY_COMPAT_PATHS.contains(&file.as_str())
        {
            continue;
        }
        let text = read_text_lossy(Path::new(file))?;
        if let Some(forbidden) = repair_packet_authority_forbidden_call(file, &text) {
            violations.push(format!(
                "{file} calls readiness internal `{forbidden}` outside the producer-owned repair-packet authority\n  reason: RIPR-SPEC-0087 §8 (issue #2028): `repair_packet_eligibility` / `is_safe_for_repair_packet` in analysis/repair_route.rs is the single authority for the safe-for-repair-packet flip; route the decision through it or declare a compatibility exemption"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "architecture.md",
            check: "check-architecture",
            why_it_matters: "Internal module seams replace premature crate splits, so dependency direction has to be checked mechanically.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Move rendering logic into output modules.",
                "Keep domain model types independent from CLI, LSP, output, and JSON adapters.",
                "Keep analysis logic out of CLI, LSP, and output adapters.",
                "Update policy/architecture.txt only when the architecture rule itself changes.",
            ],
            rerun_command: "cargo xtask check-architecture",
            exception_template: Some("glob|forbidden_pattern|reason"),
        },
        &violations,
    )
}

/// RIPR-SPEC-0087 §8 (issue #2028): the producer-owned repair-packet
/// eligibility authority. Production code outside this module must route the
/// safe-for-repair-packet decision through `repair_packet_eligibility` /
/// `is_safe_for_repair_packet` instead of calling readiness internals.
const REPAIR_PACKET_AUTHORITY_PATH: &str = "crates/ripr/src/analysis/repair_route.rs";
/// Declared compatibility consumers that legitimately read readiness
/// internals: the evidence-record renderer (the PR 1 re-export site for the
/// cross-language helpers) and the targeted-rerun parity comparison, which
/// diffs producer readiness structs field by field.
const REPAIR_PACKET_AUTHORITY_COMPAT_PATHS: [&str; 2] = [
    "crates/ripr/src/output/evidence_record.rs",
    "crates/ripr/src/cli/rerun.rs",
];
const REPAIR_PACKET_AUTHORITY_FORBIDDEN_CALLS: [&str; 2] =
    ["repair_projection_ready", "repair_route_readiness"];

fn repair_packet_authority_forbidden_call(file: &str, text: &str) -> Option<&'static str> {
    if !file.starts_with("crates/ripr/src/") || !file.ends_with(".rs") {
        return None;
    }
    if file == REPAIR_PACKET_AUTHORITY_PATH || REPAIR_PACKET_AUTHORITY_COMPAT_PATHS.contains(&file)
    {
        return None;
    }
    REPAIR_PACKET_AUTHORITY_FORBIDDEN_CALLS
        .into_iter()
        .find(|forbidden| contains_call(text, forbidden))
}

/// True when `text` contains a call to `name`: the identifier followed by
/// optional whitespace and `(`. Matching the bare identifier plus a paren
/// scan (instead of the literal `name(`) keeps `name (args)` and line-break
/// splits before the call paren from evading the guard. Field mentions
/// (`name: value`), re-exports (`use ...::name;`), and prose without a call
/// paren do not match.
fn contains_call(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let bounded_left = start == 0
            || text[..start]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric() && c != '_');
        let calls = text[start + name.len()..]
            .chars()
            .find(|c| !c.is_whitespace())
            == Some('(');
        bounded_left && calls
    })
}

#[cfg(test)]
mod repair_packet_authority_guard_tests {
    use super::repair_packet_authority_forbidden_call;

    #[test]
    fn guard_fires_on_readiness_internal_call_outside_authority() -> Result<(), String> {
        let text = "use crate::analysis::repair_route::repair_projection_ready;\n\
                    fn brief(entry: &ClassifiedSeam) -> bool { repair_projection_ready(entry) }";
        let fired = repair_packet_authority_forbidden_call("crates/ripr/src/lsp/actions.rs", text)
            .ok_or_else(|| "guard did not fire on a direct readiness call".to_string())?;
        assert_eq!(fired, "repair_projection_ready");
        Ok(())
    }

    #[test]
    fn guard_fires_on_repair_route_readiness_call_outside_authority() -> Result<(), String> {
        let text = "let readiness = repair_route_readiness(entry);";
        let fired = repair_packet_authority_forbidden_call("crates/ripr/src/lsp/backend.rs", text)
            .ok_or_else(|| "guard did not fire on a readiness-struct call".to_string())?;
        assert_eq!(fired, "repair_route_readiness");
        Ok(())
    }

    #[test]
    fn guard_fires_on_whitespace_separated_call_forms() -> Result<(), String> {
        for text in [
            "let readiness = repair_route_readiness (entry);",
            "let readiness = repair_route_readiness\n    (entry);",
            "let ready = repair_projection_ready\t(entry);",
        ] {
            if repair_packet_authority_forbidden_call("crates/ripr/src/lsp/backend.rs", text)
                .is_none()
            {
                return Err(format!(
                    "guard did not fire on whitespace-separated call: {text:?}"
                ));
            }
        }
        // A longer identifier that merely ends with a forbidden name is not a
        // call to the readiness internal.
        let lookalike = "let readiness = my_repair_route_readiness_helper(entry);";
        if repair_packet_authority_forbidden_call("crates/ripr/src/lsp/backend.rs", lookalike)
            .is_some()
        {
            return Err("guard fired on a lookalike identifier".to_string());
        }
        Ok(())
    }

    #[test]
    fn guard_skips_authority_and_declared_compat_paths() -> Result<(), String> {
        let text = "let readiness = repair_route_readiness(entry); repair_projection_ready(entry);";
        for file in [
            "crates/ripr/src/analysis/repair_route.rs",
            "crates/ripr/src/output/evidence_record.rs",
            "crates/ripr/src/cli/rerun.rs",
        ] {
            if let Some(forbidden) = repair_packet_authority_forbidden_call(file, text) {
                return Err(format!("guard fired on exempt path {file}: {forbidden}"));
            }
        }
        Ok(())
    }

    #[test]
    fn guard_ignores_non_call_mentions_and_out_of_scope_paths() -> Result<(), String> {
        // Field names, re-exports, and prose mentions are not calls.
        let text = "repair_route_readiness: readiness,\n\
                    pub(crate) use crate::analysis::repair_route::repair_route_readiness;\n\
                    // repair_projection_ready is the authority-internal helper";
        if repair_packet_authority_forbidden_call("crates/ripr/src/app.rs", text).is_some() {
            return Err("guard fired on non-call mentions".to_string());
        }
        let call = "repair_projection_ready(entry)";
        if repair_packet_authority_forbidden_call("xtask/src/main.rs", call).is_some() {
            return Err("guard fired outside crates/ripr/src".to_string());
        }
        // The authority surface itself is the sanctioned route.
        let sanctioned = "repair_packet_eligibility(entry).eligible()";
        if repair_packet_authority_forbidden_call("crates/ripr/src/lsp/actions.rs", sanctioned)
            .is_some()
        {
            return Err("guard fired on the sanctioned authority route".to_string());
        }
        Ok(())
    }
}

fn check_public_api() -> Result<(), String> {
    let allowed = read_public_api_allowlist("policy/public_api.txt")?;
    let actual = public_api_surface(Path::new("crates/ripr/src/lib.rs"), "ripr")?;
    let allowed_set = allowed.iter().cloned().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let mut violations = Vec::new();

    for line in &actual {
        if !allowed_set.contains(line) {
            violations.push(format!(
                "public API export is not allowlisted: {line}\n  update policy/public_api.txt only when this is an intentional public contract"
            ));
        }
    }
    for line in &allowed {
        if !actual_set.contains(line) {
            violations.push(format!(
                "public API allowlist entry is no longer reachable from the ripr crate root: {line}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "public-api.md",
            check: "check-public-api",
            why_it_matters: "The crate is the published product surface, so accidental public exports create compatibility expectations. This gate records every module-level `pub` item reachable from the crate root through `pub mod`, including items declared outside lib.rs (#3052). A `#[macro_export]` macro is recorded as `pub macro ripr::<name>` even when its declaring module is private, because Rust exports it at the crate root regardless of that module's visibility. It does not cover public struct fields, enum variants, trait items, or associated functions in `impl` blocks, and it does not resolve names: a `pub use` is recorded as the name it binds, and a glob re-export is recorded as a glob because a syntax walk cannot expand it.",
            fix_kind: FixKind::ReviewerDecisionRequired,
            recommended_fixes: &[
                "Keep new implementation modules and items private unless they are part of the crate contract.",
                "If the public export is intentional, update policy/public_api.txt and explain the contract in the PR.",
                "Prefer output DTOs and app APIs over exposing internal analyzer structures directly.",
                "Avoid `pub use path::*`: the gate cannot expand a glob, so it records the glob itself.",
            ],
            rerun_command: "cargo xtask check-public-api",
            exception_template: Some("pub const ripr::example::EXAMPLE"),
        },
        &violations,
    )
}

fn workspace_members() -> Result<Vec<String>, String> {
    let text = read_text_lossy(Path::new("Cargo.toml"))?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix("members") else {
            continue;
        };
        let Some((_, raw_array)) = value.split_once('=') else {
            continue;
        };
        return parse_inline_array(raw_array);
    }
    Ok(Vec::new())
}

fn read_public_api_allowlist(path: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        entries.push(trimmed.to_string());
    }
    Ok(entries)
}

fn read_pipe_records(path: &str, field_count: usize) -> Result<Vec<Vec<String>>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields = trimmed
            .split('|')
            .map(|field| field.trim().to_string())
            .collect::<Vec<_>>();
        if fields.len() != field_count || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "{}:{} expected {field_count} non-empty pipe-separated fields",
                path,
                index + 1
            ));
        }
        records.push(fields);
    }
    Ok(records)
}

fn check_output_contracts() -> Result<(), String> {
    let records = read_pipe_records("policy/output_contracts.txt", 3)?;
    let mut domain = String::new();
    for path in [
        "crates/ripr/src/domain/mod.rs",
        "crates/ripr/src/domain/classification.rs",
        "crates/ripr/src/domain/evidence.rs",
        "crates/ripr/src/domain/probe.rs",
        "crates/ripr/src/domain/summary.rs",
        "crates/ripr/src/domain/support.rs",
    ] {
        domain.push_str(&read_text_lossy(Path::new(path))?);
        domain.push('\n');
    }
    let app = read_text_lossy(Path::new("crates/ripr/src/app.rs"))?;
    let evidence_record = read_text_lossy(Path::new("crates/ripr/src/output/evidence_record.rs"))?;
    let mutation_calibration =
        read_text_lossy(Path::new("crates/ripr/src/output/mutation_calibration.rs"))?;
    let swarm_ingest = read_text_lossy(Path::new("crates/ripr/src/output/swarm_ingest.rs"))?;
    let mut json_output = String::new();
    for path in [
        "crates/ripr/src/output/json/mod.rs",
        "crates/ripr/src/output/json/context_packet.rs",
        "crates/ripr/src/output/json/formatter.rs",
        "crates/ripr/src/output/json/report.rs",
    ] {
        json_output.push_str(&read_text_lossy(Path::new(path))?);
        json_output.push('\n');
    }
    let schema = read_text_lossy(Path::new("docs/OUTPUT_SCHEMA.md"))?;
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();

    for record in records {
        let kind = &record[0];
        let value = &record[1];
        if !seen.insert(format!("{kind}|{value}")) {
            violations.push(format!("duplicate output contract entry: {kind}|{value}"));
        }
        match kind.as_str() {
            "schema_version" => {
                require_contract_value(
                    "crates/ripr/src/app.rs",
                    &app,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "evidence_record_schema_version" => {
                require_contract_value(
                    "crates/ripr/src/output/evidence_record.rs",
                    &evidence_record,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
                validate_evidence_record_contract_schema_version(value, &mut violations)?;
            }
            "context_version" => {
                require_contract_value(
                    "crates/ripr/src/output/json/",
                    &json_output,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "confidence_label" => {
                require_contract_value(
                    "crates/ripr/src/output/mutation_calibration.rs",
                    &mutation_calibration,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "exposure_class" | "severity" | "probe_family" | "delta" | "flow_sink"
            | "stage_state" | "confidence" | "oracle_kind" | "oracle_strength" | "stop_reason"
            | "value_context" | "oracle_alignment" => {
                require_contract_value(
                    "crates/ripr/src/domain/",
                    &domain,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "ingest_reason" => {
                require_contract_value(
                    "crates/ripr/src/output/swarm_ingest.rs",
                    &swarm_ingest,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            other => violations.push(format!(
                "policy/output_contracts.txt uses unsupported kind `{other}`"
            )),
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "output-contracts.md",
            check: "check-output-contracts",
            why_it_matters: "Output enum values and schema versions are integration contracts for CLI JSON, LSP diagnostics, CI, and agents.",
            fix_kind: FixKind::ReviewerDecisionRequired,
            recommended_fixes: &[
                "Update policy/output_contracts.txt when a new output enum value is intentionally added.",
                "Update docs/OUTPUT_SCHEMA.md when output values or schema versions change.",
                "Keep static output language within the registered conservative exposure classes.",
            ],
            rerun_command: "cargo xtask check-output-contracts",
            exception_template: Some("kind|value|reason"),
        },
        &violations,
    )
}

fn validate_evidence_record_contract_schema_version(
    expected_version: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let path = Path::new(EVIDENCE_RECORD_CONTRACT_CORPUS);
    if !path.exists() {
        violations.push(format!(
            "evidence-record schema contract is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }
    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "schema_version").as_deref() != Some(expected_version) {
        violations.push(format!(
            "{} schema_version must match evidence_record_schema_version {expected_version}",
            normalize_path(path)
        ));
    }
    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        let Some(record) = case.get("record") else {
            continue;
        };
        if json_string_field(record, "schema_version").as_deref() != Some(expected_version) {
            violations.push(format!(
                "evidence-record case {case_id} schema_version must match {expected_version}"
            ));
        }
    }
    Ok(())
}

fn require_contract_value(
    path: &str,
    text: &str,
    value: &str,
    kind: &str,
    violations: &mut Vec<String>,
) {
    if !text.contains(value) {
        violations.push(format!(
            "{path} does not mention {kind} contract value `{value}`"
        ));
    }
}

fn check_doc_index() -> Result<(), String> {
    let mut violations = Vec::new();
    require_index_mentions_files(
        Path::new("docs/adr/README.md"),
        Path::new("docs/adr"),
        &["README.md"],
        &mut violations,
    )?;
    require_index_mentions_files(
        Path::new("docs/specs/README.md"),
        Path::new("docs/specs"),
        &["README.md"],
        &mut violations,
    )?;

    let documentation = read_text_lossy(Path::new("docs/DOCUMENTATION.md"))?;
    for required in [
        "CODEX_GOALS.md",
        "IMPLEMENTATION_CAMPAIGNS.md",
        "SCOPED_PR_CONTRACT.md",
        "PR_AUTOMATION.md",
        "CAPABILITY_MATRIX.md",
        "METRICS.md",
        "ROADMAP.md",
        "IMPLEMENTATION_PLAN.md",
        "adr/",
        "specs/",
    ] {
        if !documentation.contains(required) {
            violations.push(format!(
                "docs/DOCUMENTATION.md does not reference `{required}`"
            ));
        }
    }

    let readme = read_text_lossy(Path::new("README.md"))?;
    for required in [
        "docs/DOCUMENTATION.md",
        "docs/ROADMAP.md",
        "docs/IMPLEMENTATION_PLAN.md",
        "docs/IMPLEMENTATION_CAMPAIGNS.md",
        "docs/CODEX_GOALS.md",
        "docs/SCOPED_PR_CONTRACT.md",
        "docs/PR_AUTOMATION.md",
        "docs/specs/README.md",
        "docs/adr/README.md",
        "docs/METRICS.md",
        "docs/CAPABILITY_MATRIX.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("README.md does not reference `{required}`"));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "doc-index.md",
            check: "check-doc-index",
            why_it_matters: "Docs are the durable context for humans and long-context agents; indexes must expose current specs, ADRs, and front-door process docs.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update docs/adr/README.md when adding or removing ADRs.",
                "Update docs/specs/README.md when adding or removing specs.",
                "Keep README.md and docs/DOCUMENTATION.md linked to the active planning, automation, metrics, ADR, and spec docs.",
            ],
            rerun_command: "cargo xtask check-doc-index",
            exception_template: None,
        },
        &violations,
    )
}

const DOC_ARTIFACT_LEDGER: &str = "policy/doc-artifacts.toml";
const DOC_ARTIFACT_SCHEMA_VERSION: &str = "1.0";
const SUPPORT_TIERS_PATH: &str = "docs/status/SUPPORT_TIERS.md";
const DOC_ARTIFACT_KINDS: &[&str] = &[
    "adr",
    "closeout",
    "goal",
    "plan",
    "policy-ledger",
    "policy_ledger",
    "proposal",
    "roadmap",
    "spec",
    "support-tier",
    "support_tier",
];
const DOC_ARTIFACT_STATUSES: &[&str] = &[
    "accepted",
    "active",
    "blocked",
    "deprecated",
    "done",
    "draft",
    "implemented",
    "planned",
    "proposed",
    "ready",
    "rejected",
    "superseded",
    "withdrawn",
];

#[derive(Clone, Debug, Default)]
struct DocArtifactLedger {
    schema_version: Option<String>,
    artifacts: Vec<DocArtifactEntry>,
}

#[derive(Clone, Debug, Default)]
struct DocArtifactEntry {
    line: usize,
    seen_fields: BTreeSet<String>,
    id: Option<String>,
    kind: Option<String>,
    path: Option<String>,
    status: Option<String>,
    owner: Option<String>,
    linked_proposal: Option<String>,
    linked_spec: Option<String>,
    linked_adr: Option<String>,
    linked_plan: Option<String>,
    standalone_reason: Option<String>,
    superseded_by: Option<String>,
    replacement: Option<String>,
}

fn check_doc_artifacts() -> Result<(), String> {
    let mut violations = doc_artifact_violations(Path::new("."), Path::new(DOC_ARTIFACT_LEDGER))?;
    // #1718: reverse-direction check — every RIPR-SPEC on disk must be registered
    // in the ledger. This is ratcheted: the current unregistered count is 82, so
    // the gate fails only when NEW unregistered specs appear (count > baseline).
    let ledger = parse_doc_artifact_ledger(Path::new(DOC_ARTIFACT_LEDGER))?;
    let registered: BTreeSet<String> = ledger
        .artifacts
        .iter()
        .filter_map(|a| a.id.clone())
        .collect();
    let specs_dir = Path::new("docs/specs");
    let entries = match std::fs::read_dir(specs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // docs/specs doesn't exist (e.g. test fixtures). Skip the reverse
            // check — the ledger→file check above is the authority.
            return finish_policy_report(
                PolicyReportSpec {
                    report_file: "doc-artifacts.md",
                    check: "check-doc-artifacts",
                    why_it_matters: "The document artifact ledger is the machine-readable source-of-truth graph for proposals, specs, ADRs, plans, goals, support tiers, policy ledgers, and closeouts.",
                    fix_kind: FixKind::AuthorDecisionRequired,
                    recommended_fixes: &[
                        "Keep policy/doc-artifacts.toml parseable with schema_version = \"1.0\".",
                        "Register each source-of-truth artifact with a unique id, kind, path, status, and owner.",
                        "Keep artifact files present and make sure the artifact id appears in the registered file.",
                        "Link accepted specs to a proposal or provide a standalone_reason.",
                        "Point superseded artifacts at a registered replacement.",
                    ],
                    rerun_command: "cargo xtask check-doc-artifacts",
                    exception_template: None,
                },
                &violations,
            );
        }
        Err(err) => return Err(format!("failed to read {}: {err}", specs_dir.display())),
    };
    let mut unregistered: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(spec_id) = spec_id_from_path(&path)
            && !registered.contains(&spec_id)
        {
            unregistered.push(spec_id);
        }
    }
    // Ratchet: fail only when the unregistered count GROWS beyond the baseline.
    // The baseline (82) is the count at HEAD when this check was added (#1718).
    // As specs are registered, the baseline should be lowered in this constant.
    const UNREGISTERED_SPEC_BASELINE: usize = 82;
    if unregistered.len() > UNREGISTERED_SPEC_BASELINE
        && let Some(excess) = unregistered.len().checked_sub(UNREGISTERED_SPEC_BASELINE)
    {
        violations.push(format!(
            "reverse-direction: {excess} new RIPR-SPEC file(s) on disk are not registered in {} \
             (baseline: {UNREGISTERED_SPEC_BASELINE}, current: {}). \
             Register the new spec(s) or lower the baseline if the count was intentionally reduced.",
            DOC_ARTIFACT_LEDGER,
            unregistered.len()
        ));
    }
    finish_policy_report(
        PolicyReportSpec {
            report_file: "doc-artifacts.md",
            check: "check-doc-artifacts",
            why_it_matters: "The document artifact ledger is the machine-readable source-of-truth graph for proposals, specs, ADRs, plans, goals, support tiers, policy ledgers, and closeouts.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep policy/doc-artifacts.toml parseable with schema_version = \"1.0\".",
                "Register each source-of-truth artifact with a unique id, kind, path, status, and owner.",
                "Keep artifact files present and make sure the artifact id appears in the registered file.",
                "Link accepted specs to a proposal or provide a standalone_reason.",
                "Point superseded artifacts at a registered replacement.",
                "Register every new RIPR-SPEC file in the ledger (reverse-direction check, #1718).",
            ],
            rerun_command: "cargo xtask check-doc-artifacts",
            exception_template: None,
        },
        &violations,
    )
}

fn doc_artifact_violations(root: &Path, ledger_path: &Path) -> Result<Vec<String>, String> {
    let ledger = parse_doc_artifact_ledger(ledger_path)?;
    let ledger_display = display_repo_path(root, ledger_path);
    let mut violations = Vec::new();

    match ledger.schema_version.as_deref() {
        Some(DOC_ARTIFACT_SCHEMA_VERSION) => {}
        Some(version) => violations.push(format!(
            "{ledger_display} schema_version must be {DOC_ARTIFACT_SCHEMA_VERSION} (got {version})"
        )),
        None => violations.push(format!(
            "{ledger_display} is missing schema_version = \"{DOC_ARTIFACT_SCHEMA_VERSION}\""
        )),
    }

    if ledger.artifacts.is_empty() {
        violations.push(format!("{ledger_display} has no [[artifact]] entries"));
    }

    let mut ids_by_line = BTreeMap::new();
    for artifact in &ledger.artifacts {
        let Some(id) = artifact.id.as_deref() else {
            violations.push(format!(
                "{ledger_display}:{} artifact is missing required field `id`",
                artifact.line
            ));
            continue;
        };
        if let Some(previous_line) = ids_by_line.insert(id.to_string(), artifact.line) {
            violations.push(format!(
                "{ledger_display}:{} duplicate artifact id `{id}`; first defined at line {previous_line}",
                artifact.line
            ));
        }
    }
    let artifact_ids = ids_by_line.keys().cloned().collect::<BTreeSet<_>>();

    for artifact in &ledger.artifacts {
        validate_doc_artifact_entry(
            root,
            &ledger_display,
            artifact,
            &artifact_ids,
            &mut violations,
        )?;
    }

    Ok(violations)
}

fn parse_doc_artifact_ledger(path: &Path) -> Result<DocArtifactLedger, String> {
    let text = read_text_lossy(path)?;
    let display = normalize_path(path);
    parse_doc_artifact_ledger_text(&display, &text)
}

fn parse_doc_artifact_ledger_text(display: &str, text: &str) -> Result<DocArtifactLedger, String> {
    let mut ledger = DocArtifactLedger::default();
    let mut current: Option<DocArtifactEntry> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "[[artifact]]" {
            if let Some(artifact) = current.take() {
                ledger.artifacts.push(artifact);
            }
            current = Some(DocArtifactEntry {
                line: line_number,
                ..DocArtifactEntry::default()
            });
            continue;
        }

        if trimmed.starts_with('[') {
            return Err(format!(
                "{display}:{line_number} unsupported TOML section `{trimmed}`; use [[artifact]]"
            ));
        }

        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            return Err(format!(
                "{display}:{line_number} expected `key = \"value\"`"
            ));
        };
        let parsed = parse_string_value(value, display, line_number)?;

        if let Some(artifact) = current.as_mut() {
            set_doc_artifact_field(artifact, key, parsed, display, line_number)?;
        } else if key == "schema_version" {
            ledger.schema_version = Some(parsed);
        } else {
            return Err(format!(
                "{display}:{line_number} `{key}` must appear inside [[artifact]]"
            ));
        }
    }

    if let Some(artifact) = current {
        ledger.artifacts.push(artifact);
    }

    Ok(ledger)
}

fn set_doc_artifact_field(
    artifact: &mut DocArtifactEntry,
    key: &str,
    value: String,
    display: &str,
    line_number: usize,
) -> Result<(), String> {
    if !matches!(
        key,
        "id" | "kind"
            | "path"
            | "status"
            | "owner"
            | "linked_proposal"
            | "linked_spec"
            | "linked_adr"
            | "linked_plan"
            | "standalone_reason"
            | "superseded_by"
            | "replacement"
            | "notes"
            | "reason"
            | "review_posture"
    ) {
        return Err(format!(
            "{display}:{line_number} unknown field `{key}` in [[artifact]] section"
        ));
    }
    if !artifact.seen_fields.insert(key.to_string()) {
        return Err(format!(
            "{display}:{line_number} duplicate field `{key}` in [[artifact]] section"
        ));
    }
    match key {
        "id" => artifact.id = Some(value),
        "kind" => artifact.kind = Some(value),
        "path" => artifact.path = Some(value),
        "status" => artifact.status = Some(value),
        "owner" => artifact.owner = Some(value),
        "linked_proposal" => artifact.linked_proposal = Some(value),
        "linked_spec" => artifact.linked_spec = Some(value),
        "linked_adr" => artifact.linked_adr = Some(value),
        "linked_plan" => artifact.linked_plan = Some(value),
        "standalone_reason" => artifact.standalone_reason = Some(value),
        "superseded_by" => artifact.superseded_by = Some(value),
        "replacement" => artifact.replacement = Some(value),
        "notes" | "reason" | "review_posture" => {}
        _ => {
            return Err(format!(
                "{display}:{line_number} unknown field `{key}` in [[artifact]] section"
            ));
        }
    }
    Ok(())
}

fn validate_doc_artifact_entry(
    root: &Path,
    ledger_display: &str,
    artifact: &DocArtifactEntry,
    artifact_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let id_label = artifact.id.as_deref().unwrap_or("<missing id>");
    let Some(id) = artifact.id.as_deref() else {
        return Ok(());
    };
    let Some(kind) = require_doc_artifact_field(ledger_display, artifact, "kind", violations)
    else {
        return Ok(());
    };
    let Some(path) = require_doc_artifact_field(ledger_display, artifact, "path", violations)
    else {
        return Ok(());
    };
    let Some(status) = require_doc_artifact_field(ledger_display, artifact, "status", violations)
    else {
        return Ok(());
    };
    let _owner = require_doc_artifact_field(ledger_display, artifact, "owner", violations);

    let path_is_safe = match doc_artifact_path_safety_violation(path) {
        Some(message) => {
            violations.push(format!(
                "{ledger_display}:{} artifact `{id_label}` path {message}: `{path}`",
                artifact.line
            ));
            false
        }
        None => true,
    };

    if !DOC_ARTIFACT_KINDS.contains(&kind) {
        violations.push(format!(
            "{ledger_display}:{} artifact `{id_label}` has unsupported kind `{kind}`",
            artifact.line
        ));
    } else if path_is_safe && !doc_artifact_kind_matches_path(kind, path) {
        violations.push(format!(
            "{ledger_display}:{} artifact `{id_label}` kind `{kind}` does not match path `{path}`",
            artifact.line
        ));
    }

    if !DOC_ARTIFACT_STATUSES.contains(&status) {
        violations.push(format!(
            "{ledger_display}:{} artifact `{id_label}` has unsupported status `{status}`",
            artifact.line
        ));
    }

    if path_is_safe {
        let artifact_path = root.join(path);
        if !artifact_path.exists() {
            violations.push(format!(
                "{ledger_display}:{} artifact `{id_label}` points at missing file `{path}`",
                artifact.line
            ));
        } else {
            let text = read_text_lossy(&artifact_path)?;
            if !text.contains(id) {
                violations.push(format!(
                    "{ledger_display}:{} artifact `{id_label}` file `{path}` does not mention `{id}`",
                    artifact.line
                ));
            }
        }
    }

    for (field, linked_id) in [
        ("linked_proposal", artifact.linked_proposal.as_deref()),
        ("linked_spec", artifact.linked_spec.as_deref()),
        ("linked_adr", artifact.linked_adr.as_deref()),
        ("linked_plan", artifact.linked_plan.as_deref()),
    ] {
        if let Some(linked_id) = linked_id {
            validate_doc_artifact_link(
                ledger_display,
                artifact,
                id_label,
                field,
                linked_id,
                artifact_ids,
                violations,
            );
        }
    }

    if status == "superseded" {
        match doc_artifact_replacement_id(artifact) {
            Some(replacement_id) => {
                if replacement_id == id {
                    violations.push(format!(
                        "{ledger_display}:{} superseded artifact `{id_label}` must not replace itself",
                        artifact.line
                    ));
                } else {
                    validate_doc_artifact_link(
                        ledger_display,
                        artifact,
                        id_label,
                        "superseded_by",
                        replacement_id,
                        artifact_ids,
                        violations,
                    );
                }
            }
            None => violations.push(format!(
                "{ledger_display}:{} superseded artifact `{id_label}` must set superseded_by or replacement",
                artifact.line
            )),
        }
    }

    if kind == "spec"
        && status == "accepted"
        && artifact.linked_proposal.is_none()
        && artifact
            .standalone_reason
            .as_deref()
            .is_none_or(str::is_empty)
    {
        violations.push(format!(
            "{ledger_display}:{} accepted spec `{id_label}` must set linked_proposal or standalone_reason",
            artifact.line
        ));
    }

    if kind == "plan"
        && status == "active"
        && artifact.linked_proposal.is_none()
        && artifact.linked_spec.is_none()
    {
        violations.push(format!(
            "{ledger_display}:{} active plan `{id_label}` must link to at least one proposal or spec",
            artifact.line
        ));
    }

    Ok(())
}

fn require_doc_artifact_field<'a>(
    ledger_display: &str,
    artifact: &'a DocArtifactEntry,
    field: &str,
    violations: &mut Vec<String>,
) -> Option<&'a str> {
    let value = match field {
        "kind" => artifact.kind.as_deref(),
        "path" => artifact.path.as_deref(),
        "status" => artifact.status.as_deref(),
        "owner" => artifact.owner.as_deref(),
        _ => None,
    };

    match value {
        Some(value) if !value.trim().is_empty() => Some(value),
        _ => {
            let id = artifact.id.as_deref().unwrap_or("<missing id>");
            violations.push(format!(
                "{ledger_display}:{} artifact `{id}` is missing required field `{field}`",
                artifact.line
            ));
            None
        }
    }
}

fn validate_doc_artifact_link(
    ledger_display: &str,
    artifact: &DocArtifactEntry,
    id_label: &str,
    field: &str,
    linked_id: &str,
    artifact_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    if !artifact_ids.contains(linked_id) {
        violations.push(format!(
            "{ledger_display}:{} artifact `{id_label}` {field} references unknown artifact `{linked_id}`",
            artifact.line
        ));
    }
}

fn doc_artifact_path_safety_violation(path: &str) -> Option<&'static str> {
    if path.trim().is_empty() {
        return Some("must be a non-empty repo-relative path");
    }

    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Some("must be repo-relative");
    }

    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir => return Some("must not contain `..` traversal"),
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Some("must be repo-relative");
            }
            std::path::Component::CurDir | std::path::Component::Normal(_) => {}
        }
    }

    None
}

fn doc_artifact_replacement_id(artifact: &DocArtifactEntry) -> Option<&str> {
    artifact
        .superseded_by
        .as_deref()
        .or(artifact.replacement.as_deref())
}

fn doc_artifact_kind_matches_path(kind: &str, path: &str) -> bool {
    match kind {
        "adr" => path.starts_with("docs/adr/") && path.ends_with(".md"),
        "closeout" => {
            (path.starts_with("docs/handoffs/") || path.starts_with("plans/"))
                && path.ends_with(".md")
        }
        "goal" => path.starts_with(".ripr/goals/") && path.ends_with(".toml"),
        "plan" => path.starts_with("plans/") && path.ends_with(".md"),
        "policy-ledger" | "policy_ledger" => path.starts_with("policy/") && path.ends_with(".toml"),
        "proposal" => path.starts_with("docs/proposals/") && path.ends_with(".md"),
        "roadmap" => path == "docs/ROADMAP.md" || path == "ROADMAP.md",
        "spec" => path.starts_with("docs/specs/") && path.ends_with(".md"),
        "support-tier" | "support_tier" => {
            path == "docs/status/SUPPORT_TIERS.md"
                || (path.starts_with("docs/status/") && path.ends_with(".md"))
        }
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SupportTierRow {
    line: usize,
    capability: String,
    tier: String,
    surface: String,
    proof: String,
    known_limits: String,
}

fn check_support_tiers() -> Result<(), String> {
    let violations = support_tier_violations(Path::new("."), Path::new(SUPPORT_TIERS_PATH))?;
    finish_policy_report(
        PolicyReportSpec {
            report_file: "support-tiers.md",
            check: "check-support-tiers",
            why_it_matters: "Support tiers are the product claim to proof-command map. Stable and usable claims should not drift away from evidence or overstate RIPR's static-advisory boundary.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep docs/status/SUPPORT_TIERS.md parseable with the Current Support Map table.",
                "Give stable building block, usable, and usable alpha rows non-empty proof cells.",
                "Use known cargo xtask commands when proof cells name repo proof commands.",
                "Link specs with support-tier impact back to docs/status/SUPPORT_TIERS.md.",
            ],
            rerun_command: "cargo xtask check-support-tiers",
            exception_template: None,
        },
        &violations,
    )
}

fn support_tier_violations(root: &Path, support_tiers_path: &Path) -> Result<Vec<String>, String> {
    let support_text = read_text_lossy(support_tiers_path)?;
    let display = display_repo_path(root, support_tiers_path);
    let rows = support_tier_rows(&support_text, &display)?;
    let mut violations = Vec::new();

    if !has_markdown_heading(&support_text, "# Support Tiers") {
        violations.push(format!("{display} is missing `# Support Tiers`"));
    }
    if rows.is_empty() {
        violations.push(format!(
            "{display} is missing the `Current Support Map` table"
        ));
    }

    for row in &rows {
        validate_support_tier_row(root, &display, row, &mut violations);
    }

    validate_support_tier_spec_links(root, &mut violations)?;
    validate_readme_support_tier_pointer(root, &mut violations)?;
    Ok(violations)
}

fn support_tier_rows(text: &str, display: &str) -> Result<Vec<SupportTierRow>, String> {
    let mut rows = Vec::new();
    let mut in_table = false;
    let mut seen_header = false;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "| Capability | Tier | Surface | Proof | Known limits |" {
            in_table = true;
            seen_header = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if trimmed.starts_with("| ---") {
            continue;
        }
        if !trimmed.starts_with('|') {
            break;
        }

        let cells = markdown_table_cells(trimmed);
        if cells.len() != 5 {
            return Err(format!(
                "{display}:{line_number} support-tier row must have 5 cells"
            ));
        }
        rows.push(SupportTierRow {
            line: line_number,
            capability: cells[0].clone(),
            tier: cells[1].clone(),
            surface: cells[2].clone(),
            proof: cells[3].clone(),
            known_limits: cells[4].clone(),
        });
    }

    if !seen_header {
        return Ok(Vec::new());
    }
    Ok(rows)
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect()
}

fn validate_support_tier_row(
    root: &Path,
    display: &str,
    row: &SupportTierRow,
    violations: &mut Vec<String>,
) {
    for (field, value) in [
        ("Capability", row.capability.as_str()),
        ("Tier", row.tier.as_str()),
        ("Surface", row.surface.as_str()),
        ("Proof", row.proof.as_str()),
        ("Known limits", row.known_limits.as_str()),
    ] {
        if value.trim().is_empty() {
            violations.push(format!(
                "{display}:{} support-tier row `{}` has empty `{field}`",
                row.line,
                support_tier_row_label(row)
            ));
        }
    }

    let tier = normalized_support_tier(&row.tier);
    if !known_support_tier(&tier) {
        violations.push(format!(
            "{display}:{} support-tier row `{}` has unknown tier `{}`",
            row.line,
            support_tier_row_label(row),
            row.tier
        ));
    }

    let proof_spans = inline_code_spans(&row.proof);
    let has_known_proof_reference = proof_spans
        .iter()
        .any(|command| support_tier_proof_reference_is_known(root, command));

    if support_tier_requires_proof(&tier) && row.proof.trim().is_empty() {
        violations.push(format!(
            "{display}:{} support-tier row `{}` with tier `{tier}` must name proof",
            row.line,
            support_tier_row_label(row)
        ));
    }
    if support_tier_requires_proof(&tier) && !has_known_proof_reference {
        violations.push(format!(
            "{display}:{} support-tier row `{}` with tier `{tier}` must name a known proof command or proof artifact",
            row.line,
            support_tier_row_label(row)
        ));
    }

    for command in proof_spans {
        validate_support_tier_proof_command(root, display, row, &command, violations);
    }
}

fn normalized_support_tier(tier: &str) -> String {
    tier.trim().trim_matches('`').to_ascii_lowercase()
}

fn known_support_tier(tier: &str) -> bool {
    matches!(
        tier,
        "stable building block"
            | "usable"
            | "usable alpha"
            | "preview"
            | "scaffold"
            | "blocked"
            | "deferred"
    )
}

fn support_tier_requires_proof(tier: &str) -> bool {
    matches!(tier, "stable building block" | "usable" | "usable alpha")
}

fn support_tier_row_label(row: &SupportTierRow) -> &str {
    if row.capability.trim().is_empty() {
        "<missing capability>"
    } else {
        row.capability.trim()
    }
}

fn inline_code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        spans.push(after_start[..end].trim().to_string());
        rest = &after_start[end + 1..];
    }
    spans
}

fn support_tier_proof_reference_is_known(root: &Path, command: &str) -> bool {
    if let Some(rest) = command.strip_prefix("cargo xtask ") {
        let command_name = rest.split_whitespace().next().unwrap_or_default();
        return known_xtask_command(command_name);
    }

    (command.starts_with(".github/workflows/") || command.starts_with("scripts/"))
        && doc_artifact_path_safety_violation(command).is_none()
        && root.join(command).exists()
}

fn validate_support_tier_proof_command(
    root: &Path,
    display: &str,
    row: &SupportTierRow,
    command: &str,
    violations: &mut Vec<String>,
) {
    if command.is_empty() {
        violations.push(format!(
            "{display}:{} support-tier row `{}` has an empty proof command",
            row.line,
            support_tier_row_label(row)
        ));
        return;
    }
    if let Some(rest) = command.strip_prefix("cargo xtask ") {
        let command_name = rest.split_whitespace().next().unwrap_or_default();
        if !known_xtask_command(command_name) {
            violations.push(format!(
                "{display}:{} support-tier row `{}` references unknown xtask command `{command_name}`",
                row.line,
                support_tier_row_label(row)
            ));
        }
    } else if command.starts_with(".github/workflows/") {
        if !root.join(command).exists() {
            violations.push(format!(
                "{display}:{} support-tier row `{}` references missing workflow `{command}`",
                row.line,
                support_tier_row_label(row)
            ));
        }
    } else if command.starts_with("scripts/") && !root.join(command).exists() {
        violations.push(format!(
            "{display}:{} support-tier row `{}` references missing script `{command}`",
            row.line,
            support_tier_row_label(row)
        ));
    }
}

fn validate_support_tier_spec_links(
    root: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let specs_root = root.join("docs/specs");
    for path in collect_files(&specs_root)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let text = read_text_lossy(&path)?;
        if text.contains("Support-tier impact:")
            && !spec_support_tier_impact_is_none(&text)
            && !text.contains("SUPPORT_TIERS.md")
        {
            violations.push(format!(
                "{}: spec declares `Support-tier impact:` but does not reference docs/status/SUPPORT_TIERS.md\n    reason: spec_support_tier_reference_missing\n    next: add a docs/status/SUPPORT_TIERS.md link in the Support-tier impact section, or set the impact to `None` if the spec has none",
                display_repo_path(root, &path)
            ));
        }
    }
    Ok(())
}

fn spec_support_tier_impact_is_none(text: &str) -> bool {
    let Some(after_marker) = text.split("Support-tier impact:").nth(1) else {
        return false;
    };
    let Some(first_entry) = after_marker
        .lines()
        .map(|line| line.trim().trim_start_matches("- ").trim())
        .find(|line| !line.is_empty())
    else {
        return false;
    };
    let normalized = first_entry
        .trim_end_matches('.')
        .trim()
        .to_ascii_lowercase();
    normalized == "none" || normalized.starts_with("none for ")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoContractArtifact {
    id: String,
    kind: String,
    path: String,
    status: String,
    owner: String,
    linked_proposal: Option<String>,
    linked_spec: Option<String>,
    linked_adr: Option<String>,
    linked_plan: Option<String>,
    superseded_by: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoContractSummary {
    artifacts: Vec<RepoContractArtifact>,
    support_rows: Vec<SupportTierRow>,
    policy_ledgers: Vec<String>,
    missing_links: Vec<String>,
}

fn repo_contract_report() -> Result<(), String> {
    let (markdown, json) = repo_contract_report_from_root(Path::new("."))?;
    write_report("source-of-truth-graph.md", &markdown)?;
    write_report("source-of-truth-graph.json", &json)?;
    let markdown_path = reports_dir().join("source-of-truth-graph.md");
    let json_path = reports_dir().join("source-of-truth-graph.json");
    println!(
        "wrote {} and {}",
        normalize_path(&markdown_path),
        normalize_path(&json_path)
    );
    Ok(())
}

fn repo_contract_report_from_root(root: &Path) -> Result<(String, String), String> {
    let summary = repo_contract_summary(root)?;
    Ok((
        repo_contract_report_markdown(&summary),
        repo_contract_report_json(&summary),
    ))
}

fn repo_contract_summary(root: &Path) -> Result<RepoContractSummary, String> {
    let ledger_path = root.join(DOC_ARTIFACT_LEDGER);
    let ledger = parse_doc_artifact_ledger(&ledger_path)?;
    let artifacts = ledger
        .artifacts
        .iter()
        .map(repo_contract_artifact_from_entry)
        .collect::<Vec<_>>();

    let mut missing_links = Vec::new();
    missing_links.extend(doc_artifact_violations(root, &ledger_path)?);

    let support_path = root.join(SUPPORT_TIERS_PATH);
    let support_rows = if support_path.exists() {
        let support_text = read_text_lossy(&support_path)?;
        support_tier_rows(&support_text, &display_repo_path(root, &support_path))?
    } else {
        Vec::new()
    };
    missing_links.extend(support_tier_violations(root, &support_path)?);
    missing_links.sort();
    missing_links.dedup();

    Ok(RepoContractSummary {
        artifacts,
        support_rows,
        policy_ledgers: repo_contract_policy_ledgers(root)?,
        missing_links,
    })
}

fn repo_contract_artifact_from_entry(entry: &DocArtifactEntry) -> RepoContractArtifact {
    RepoContractArtifact {
        id: entry.id.clone().unwrap_or_else(|| "<missing>".to_string()),
        kind: entry
            .kind
            .clone()
            .unwrap_or_else(|| "<missing>".to_string()),
        path: entry
            .path
            .clone()
            .unwrap_or_else(|| "<missing>".to_string()),
        status: entry
            .status
            .clone()
            .unwrap_or_else(|| "<missing>".to_string()),
        owner: entry
            .owner
            .clone()
            .unwrap_or_else(|| "<missing>".to_string()),
        linked_proposal: entry.linked_proposal.clone(),
        linked_spec: entry.linked_spec.clone(),
        linked_adr: entry.linked_adr.clone(),
        linked_plan: entry.linked_plan.clone(),
        superseded_by: entry
            .superseded_by
            .clone()
            .or_else(|| entry.replacement.clone()),
    }
}

fn repo_contract_policy_ledgers(root: &Path) -> Result<Vec<String>, String> {
    let mut ledgers = Vec::new();
    for path in collect_files(&root.join("policy"))? {
        if path.extension().and_then(|value| value.to_str()) == Some("toml")
            || path.file_name().and_then(|value| value.to_str()) == Some("workflow_allowlist.txt")
        {
            ledgers.push(display_repo_path(root, &path));
        }
    }
    ledgers.sort();
    Ok(ledgers)
}

fn repo_contract_report_markdown(summary: &RepoContractSummary) -> String {
    let mut body = String::from("# Source-of-Truth Contract Graph\n\n");
    body.push_str(&format!(
        "Status: {}\n\n",
        repo_contract_report_status(summary)
    ));
    body.push_str("Mode: advisory\n\n");
    body.push_str("## Accepted Proposals\n\n");
    write_repo_contract_artifact_list(&mut body, &summary.artifacts, "proposal", "accepted");

    body.push_str("## Accepted Specs\n\n");
    write_repo_contract_artifact_list(&mut body, &summary.artifacts, "spec", "accepted");

    body.push_str("## Open ADRs\n\n");
    let open_adrs = summary
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == "adr"
                && !matches!(
                    artifact.status.as_str(),
                    "accepted" | "done" | "superseded" | "rejected" | "withdrawn"
                )
        })
        .collect::<Vec<_>>();
    if open_adrs.is_empty() {
        body.push_str("None registered.\n\n");
    } else {
        for artifact in open_adrs {
            body.push_str(&format!(
                "- `{}`: `{}` at `{}`\n",
                artifact.id, artifact.status, artifact.path
            ));
        }
        body.push('\n');
    }

    body.push_str("## Support-Tier Impacts\n\n");
    if summary.support_rows.is_empty() {
        body.push_str("No support-tier rows found.\n\n");
    } else {
        body.push_str("| Capability | Tier | Proof |\n| --- | --- | --- |\n");
        for row in &summary.support_rows {
            body.push_str(&format!(
                "| {} | {} | {} |\n",
                markdown_cell(&row.capability),
                markdown_cell(&row.tier),
                markdown_cell(&row.proof)
            ));
        }
        body.push('\n');
    }

    body.push_str("## Policy Impacts\n\n");
    if summary.policy_ledgers.is_empty() {
        body.push_str("No policy ledgers found.\n\n");
    } else {
        for ledger in &summary.policy_ledgers {
            body.push_str(&format!("- `{ledger}`\n"));
        }
        body.push('\n');
    }

    body.push_str("## Missing Links\n\n");
    if summary.missing_links.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for violation in &summary.missing_links {
            body.push_str(&format!("- {violation}\n"));
        }
        body.push('\n');
    }

    body.push_str("## Superseded Artifacts\n\n");
    let superseded = summary
        .artifacts
        .iter()
        .filter(|artifact| artifact.status == "superseded" || artifact.superseded_by.is_some())
        .collect::<Vec<_>>();
    if superseded.is_empty() {
        body.push_str("None registered.\n\n");
    } else {
        for artifact in superseded {
            body.push_str(&format!(
                "- `{}` -> `{}`\n",
                artifact.id,
                artifact.superseded_by.as_deref().unwrap_or("<missing>")
            ));
        }
        body.push('\n');
    }

    body
}

fn write_repo_contract_artifact_list(
    body: &mut String,
    artifacts: &[RepoContractArtifact],
    kind: &str,
    status: &str,
) {
    let filtered = artifacts
        .iter()
        .filter(|artifact| artifact.kind == kind && artifact.status == status)
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        body.push_str("None registered.\n\n");
        return;
    }
    for artifact in filtered {
        body.push_str(&format!(
            "- `{}` owned by `{}` at `{}`\n",
            artifact.id, artifact.owner, artifact.path
        ));
    }
    body.push('\n');
}

fn repo_contract_report_json(summary: &RepoContractSummary) -> String {
    let mut body = String::new();
    let status = repo_contract_report_status(summary);
    let artifacts = summary.artifacts.iter().collect::<Vec<_>>();
    let accepted_proposals = summary
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == "proposal" && artifact.status == "accepted")
        .collect::<Vec<_>>();
    let accepted_specs = summary
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == "spec" && artifact.status == "accepted")
        .collect::<Vec<_>>();
    let open_adrs = summary
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == "adr"
                && !matches!(
                    artifact.status.as_str(),
                    "accepted" | "done" | "superseded" | "rejected" | "withdrawn"
                )
        })
        .collect::<Vec<_>>();
    let superseded_artifacts = summary
        .artifacts
        .iter()
        .filter(|artifact| artifact.status == "superseded" || artifact.superseded_by.is_some())
        .collect::<Vec<_>>();

    body.push_str("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"report_id\": \"source_of_truth_graph\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str("  \"artifacts\": [");
    write_repo_contract_artifact_json_array(&mut body, &artifacts);
    body.push_str("],\n");
    body.push_str("  \"accepted_proposals\": [");
    write_repo_contract_artifact_json_array(&mut body, &accepted_proposals);
    body.push_str("],\n");
    body.push_str("  \"accepted_specs\": [");
    write_repo_contract_artifact_json_array(&mut body, &accepted_specs);
    body.push_str("],\n");
    body.push_str("  \"open_adrs\": [");
    write_repo_contract_artifact_json_array(&mut body, &open_adrs);
    body.push_str("],\n");
    body.push_str("  \"superseded_artifacts\": [");
    write_repo_contract_artifact_json_array(&mut body, &superseded_artifacts);
    body.push_str("],\n");
    body.push_str("  \"support_tiers\": [\n");
    for (index, row) in summary.support_rows.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str(&format!(
            "    {{ \"capability\": \"{}\", \"tier\": \"{}\", \"surface\": \"{}\", \"proof\": \"{}\" }}",
            json_escape(&row.capability),
            json_escape(&row.tier),
            json_escape(&row.surface),
            json_escape(&row.proof)
        ));
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"policy_ledgers\": [");
    write_json_string_array(&mut body, &summary.policy_ledgers);
    body.push_str("],\n");
    body.push_str("  \"missing_links\": [");
    write_json_string_array(&mut body, &summary.missing_links);
    body.push_str("]\n}\n");
    body
}

fn repo_contract_report_status(summary: &RepoContractSummary) -> &'static str {
    if summary.missing_links.is_empty() {
        "pass"
    } else {
        "warn"
    }
}

fn write_repo_contract_artifact_json_array(body: &mut String, artifacts: &[&RepoContractArtifact]) {
    for (index, artifact) in artifacts.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push_str(&format!(
            "{{ \"id\": \"{}\", \"kind\": \"{}\", \"path\": \"{}\", \"status\": \"{}\", \"owner\": \"{}\", \"linked_proposal\": {}, \"linked_spec\": {}, \"linked_adr\": {}, \"linked_plan\": {}, \"superseded_by\": {} }}",
            json_escape(&artifact.id),
            json_escape(&artifact.kind),
            json_escape(&artifact.path),
            json_escape(&artifact.status),
            json_escape(&artifact.owner),
            json_optional_string(artifact.linked_proposal.as_deref()),
            json_optional_string(artifact.linked_spec.as_deref()),
            json_optional_string(artifact.linked_adr.as_deref()),
            json_optional_string(artifact.linked_plan.as_deref()),
            json_optional_string(artifact.superseded_by.as_deref())
        ));
    }
}

fn validate_readme_support_tier_pointer(
    root: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let readme_path = root.join("README.md");
    if !readme_path.exists() {
        return Ok(());
    }
    let readme = read_text_lossy(&readme_path)?;
    let lower = readme.to_ascii_lowercase();
    if (lower.contains("stable") || lower.contains("usable") || lower.contains("preview"))
        && !readme.contains("docs/status/SUPPORT_TIERS.md")
    {
        violations.push(
            "README.md names support-tier language but does not link docs/status/SUPPORT_TIERS.md"
                .to_string(),
        );
    }
    Ok(())
}

fn display_repo_path(root: &Path, path: &Path) -> String {
    let display_path = path.strip_prefix(root).unwrap_or(path);
    normalize_path(display_path)
}

fn check_readme_state() -> Result<(), String> {
    let readme_path = Path::new("README.md");
    let readme = read_text_lossy(readme_path)?;
    let mut violations = Vec::new();

    if !has_markdown_heading(&readme, "# ripr")
        && !readme.contains(r#"<h1 align="center">ripr</h1>"#)
    {
        violations.push("README.md is missing `# ripr` or centered HTML h1".to_string());
    }
    // README front-door contract: the README is a front door, not a support
    // ledger. Required sections are the contract spine; aging capability state
    // lives in docs/CAPABILITY_MATRIX.md, not the front door.
    for heading in [
        "## The first useful run",
        "## Where it fits",
        "## What you get",
        "## Status",
        "## Docs",
        "## Contributing",
        "## License",
    ] {
        if !has_markdown_heading(&readme, heading) {
            violations.push(format!("README.md is missing `{heading}`"));
        }
    }

    for required in [
        "docs/METRICS.md",
        "docs/CAPABILITY_MATRIX.md",
        "docs/IMPLEMENTATION_CAMPAIGNS.md",
        "docs/CODEX_GOALS.md",
        "docs/SCOPED_PR_CONTRACT.md",
        "docs/PR_AUTOMATION.md",
        "docs/DOCUMENTATION.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("README.md does not reference `{required}`"));
        }
    }

    let capabilities_source = read_text_lossy(Path::new("metrics/capabilities.toml"))?;
    let matrix = read_text_lossy(Path::new("docs/CAPABILITY_MATRIX.md"))?;
    if !matrix.contains("metrics/capabilities.toml") {
        violations.push(
            "docs/CAPABILITY_MATRIX.md does not reference metrics/capabilities.toml".to_string(),
        );
    }
    for status in ["planned", "alpha", "usable alpha", "stable", "calibrated"] {
        let marker = format!("`{status}`");
        if !matrix.contains(&marker) {
            violations.push(format!(
                "docs/CAPABILITY_MATRIX.md does not describe status `{status}`"
            ));
        }
    }
    for checkpoint in next_checkpoints_from_capabilities(&capabilities_source)? {
        if !checkpoint.trim().is_empty()
            && !readme.contains(&checkpoint)
            && !matrix.contains(&checkpoint)
        {
            violations.push(format!(
                "capability next checkpoint `{checkpoint}` is missing from README.md and docs/CAPABILITY_MATRIX.md"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "readme-state.md",
            check: "check-readme-state",
            why_it_matters: "README is the front door for humans and Codex Goals state recovery; it should summarize current capability without drifting from metrics and campaign docs.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep README.md linked to active planning, metrics, campaign, and automation docs.",
                "Keep README's capability snapshot compact and aligned with docs/CAPABILITY_MATRIX.md.",
                "Update metrics/capabilities.toml and docs/CAPABILITY_MATRIX.md when capability status or next checkpoints change.",
            ],
            rerun_command: "cargo xtask check-readme-state",
            exception_template: None,
        },
        &violations,
    )
}

fn markdown_links() -> Result<(), String> {
    let mut violations = Vec::new();
    for file in tracked_files()? {
        if !file.ends_with(".md") {
            continue;
        }
        if should_skip_path(&file) {
            continue;
        }
        let path = Path::new(&file);
        if !path.exists() {
            continue;
        }
        let text = read_text_lossy(path)?;
        for link in markdown_links_in_text(&text) {
            let Some(target_path) = local_markdown_target(&link.target) else {
                continue;
            };
            let resolved = resolve_markdown_link(path, &target_path);
            if !resolved.exists() {
                violations.push(format!(
                    "{file}:{} links to missing local target `{}`",
                    link.line, link.target
                ));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "markdown-links.md",
            check: "markdown-links",
            why_it_matters: "Markdown links are repo state for humans and long-context agents; links to deleted or renamed docs should fail before review.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update links when docs are renamed or deleted.",
                "Use relative links for repo-local Markdown targets.",
                "Run cargo xtask markdown-links before opening docs-heavy PRs.",
            ],
            rerun_command: "cargo xtask markdown-links",
            exception_template: None,
        },
        &violations,
    )
}

fn next_checkpoints_from_capabilities(text: &str) -> Result<Vec<String>, String> {
    let mut checkpoints = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("next") {
            continue;
        }
        let Some((_, value)) = trimmed.split_once('=') else {
            return Err(format!(
                "metrics/capabilities.toml:{} expected `next = \"...\"`",
                line_number + 1
            ));
        };
        checkpoints.push(parse_quoted_value(value.trim()).map_err(|message| {
            format!("metrics/capabilities.toml:{} {message}", line_number + 1)
        })?);
    }
    Ok(checkpoints)
}

fn markdown_links_in_text(text: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut in_fence = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        links.extend(markdown_links_in_line(line, index + 1));
    }
    links
}

fn markdown_links_in_line(line: &str, line_number: usize) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(start) = line[offset..].find("](") {
        let target_start = offset + start + 2;
        let Some(end) = line[target_start..].find(')') else {
            break;
        };
        let target = line[target_start..target_start + end].trim();
        if !target.is_empty() {
            links.push(MarkdownLink {
                line: line_number,
                target: target.to_string(),
            });
        }
        offset = target_start + end + 1;
    }
    links
}

fn local_markdown_target(raw_target: &str) -> Option<String> {
    let mut target = raw_target.trim();
    if target.starts_with('<') {
        let end = target.find('>')?;
        target = &target[1..end];
    } else if let Some((first, _)) = target.split_once(char::is_whitespace) {
        target = first;
    }
    if target.is_empty() || target.starts_with('#') {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("app://")
        || lower.starts_with("plugin://")
    {
        return None;
    }
    let without_query = target.split('?').next().unwrap_or(target);
    let without_anchor = without_query.split('#').next().unwrap_or(without_query);
    let local = without_anchor.trim();
    if local.is_empty() {
        None
    } else {
        Some(local.trim_start_matches('/').to_string())
    }
}

fn resolve_markdown_link(source: &Path, target: &str) -> PathBuf {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        source
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target_path)
    }
}

fn worktree(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("doctor") => worktree_doctor(),
        Some(other) => Err(format!(
            "unknown worktree command `{other}`\nusage: cargo xtask worktree doctor"
        )),
        None => Err("missing worktree command\nusage: cargo xtask worktree doctor".to_string()),
    }
}

fn worktree_doctor() -> Result<(), String> {
    let branch = git_value(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let status_changes = collect_worktree_status_changes()?;
    let behind_origin_main = branch_behind_origin_main()?;
    let target_ripr_exists = Path::new("target/ripr").exists();
    let sample_target_exists = Path::new("crates/ripr/examples/sample/target").exists();
    let badge_refresh_context = badge_refresh_context();
    let findings = worktree_doctor_findings(
        &branch,
        behind_origin_main,
        &status_changes,
        target_ripr_exists,
        sample_target_exists,
        badge_refresh_context,
    );
    finish_worktree_doctor_report(&findings)
}

fn branch_behind_origin_main() -> Result<usize, String> {
    let output = run_output_optional(
        "git",
        &["rev-list", "--left-right", "--count", "HEAD...origin/main"],
    )?;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    let mut parts = trimmed.split_whitespace();
    let _ahead = parts.next();
    let Some(behind) = parts.next() else {
        return Ok(0);
    };
    behind
        .parse::<usize>()
        .map_err(|err| format!("failed to parse origin/main behind count `{behind}`: {err}"))
}

fn worktree_doctor_findings(
    branch: &str,
    behind_origin_main: usize,
    status_changes: &[ChangedPath],
    target_ripr_exists: bool,
    sample_target_exists: bool,
    badge_refresh_context: bool,
) -> Vec<WorktreeDoctorFinding> {
    let mut findings = Vec::new();
    let dirty = !status_changes.is_empty();

    if branch == "HEAD" {
        findings.push(worktree_error(
            "worktree is on a detached HEAD; start PR work from a named branch based on origin/main",
        ));
    }

    if branch == "main" && dirty {
        findings.push(worktree_error(
            "main branch has uncommitted changes; start PR work in a fresh worktree from origin/main",
        ));
    }

    if behind_origin_main > 0 {
        findings.push(worktree_error(&format!(
            "branch is behind origin/main by {behind_origin_main} commit(s); refresh before opening or updating a PR",
        )));
    }

    for change in status_changes {
        let path = change.path.trim_end_matches('/');
        if is_badge_endpoint_json(path) && !badge_refresh_context {
            findings.push(worktree_error(&format!(
                "generated badge endpoint is dirty outside a badge refresh context: {}",
                format_changed_path(change)
            )));
            continue;
        }
        if is_ripr_target_artifact(path) && !is_deletion_only(change) {
            findings.push(worktree_error(&format!(
                "generated RIPR target artifact is dirty: {}",
                format_changed_path(change)
            )));
            continue;
        }
        if is_sample_target_artifact(path) && !is_deletion_only(change) {
            findings.push(worktree_error(&format!(
                "sample workspace target artifact is dirty: {}",
                format_changed_path(change)
            )));
        }
    }

    if target_ripr_exists {
        findings.push(worktree_warning(
            "target/ripr exists; remove local report artifacts before final worktree cleanup if this workspace is done",
        ));
    }
    if sample_target_exists {
        findings.push(worktree_warning(
            "crates/ripr/examples/sample/target exists; run `cargo clean` in that sample workspace or remove the residue before handoff",
        ));
    }

    let layers = changed_source_layers(status_changes);
    if layers.len() > 3 && !has_work_item_marker(status_changes) {
        findings.push(worktree_warning(&format!(
            "changes span multiple source-of-truth layers without an obvious work item marker: {}",
            layers.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }

    findings
}

fn worktree_error(message: &str) -> WorktreeDoctorFinding {
    WorktreeDoctorFinding {
        severity: WorktreeDoctorSeverity::Error,
        message: message.to_string(),
    }
}

fn worktree_warning(message: &str) -> WorktreeDoctorFinding {
    WorktreeDoctorFinding {
        severity: WorktreeDoctorSeverity::Warning,
        message: message.to_string(),
    }
}

fn changed_source_layers(changes: &[ChangedPath]) -> BTreeSet<&'static str> {
    changes
        .iter()
        .filter_map(|change| source_layer_for_path(change.path.trim_end_matches('/')))
        .collect()
}

fn source_layer_for_path(path: &str) -> Option<&'static str> {
    if path.starts_with(".github/workflows/") {
        Some("workflows")
    } else if path == ".ripr/traceability.toml" {
        Some("traceability")
    } else if path.starts_with(".ripr/goals/") {
        Some("goal-manifest")
    } else if path.starts_with("metrics/") {
        Some("metrics")
    } else if path.starts_with("docs/specs/") {
        Some("specs")
    } else if path.starts_with("docs/policy/") {
        Some("policy-docs")
    } else if path.starts_with("docs/IMPLEMENTATION_") || path == "docs/ROADMAP.md" {
        Some("planning-docs")
    } else if path.starts_with("docs/") {
        Some("docs")
    } else if path.starts_with("crates/") || path.starts_with("xtask/") {
        Some("code")
    } else if path.starts_with("fixtures/") {
        Some("fixtures")
    } else if path.starts_with("badges/") {
        Some("badge-endpoints")
    } else if path.starts_with("schemas/") {
        Some("schemas")
    } else if path.starts_with("policy/") {
        Some("policy")
    } else {
        None
    }
}

fn has_work_item_marker(changes: &[ChangedPath]) -> bool {
    changes.iter().any(|change| {
        let path = change.path.trim_end_matches('/');
        path.starts_with(".ripr/goals/")
            || path == "docs/IMPLEMENTATION_PLAN.md"
            || path == "docs/IMPLEMENTATION_CAMPAIGNS.md"
            || path == "docs/ROADMAP.md"
    })
}

fn finish_worktree_doctor_report(findings: &[WorktreeDoctorFinding]) -> Result<(), String> {
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == WorktreeDoctorSeverity::Error)
        .collect::<Vec<_>>();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == WorktreeDoctorSeverity::Warning)
        .collect::<Vec<_>>();
    let status = if !errors.is_empty() {
        "fail"
    } else if !warnings.is_empty() {
        "warn"
    } else {
        "pass"
    };
    let next_actions = worktree_doctor_next_actions(findings);
    let mut body = format!("# ripr worktree doctor\n\nStatus: {status}\n\n");
    body.push_str("Checks:\n\n");
    body.push_str("- branch is not dirty main\n");
    body.push_str("- branch is current with origin/main\n");
    body.push_str("- generated badge endpoints are not dirty in ordinary work\n");
    body.push_str("- generated target/sample artifacts are not dirty\n");
    body.push_str("- broad source-of-truth diffs have an obvious work item marker\n");
    if !errors.is_empty() {
        body.push_str("\nErrors:\n\n");
        for finding in &errors {
            body.push_str(&format!("- {}\n", finding.message));
        }
    }
    if !warnings.is_empty() {
        body.push_str("\nWarnings:\n\n");
        for finding in &warnings {
            body.push_str(&format!("- {}\n", finding.message));
        }
    }
    if findings.is_empty() {
        body.push_str("\nNo findings.\n");
    }
    body.push_str("\nNext actions:\n\n");
    for action in &next_actions {
        body.push_str(&format!("- {action}\n"));
    }
    write_report("worktree-doctor.md", &body)?;
    println!("{body}");
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "worktree doctor found blocking issues; see target/ripr/reports/worktree-doctor.md\n{}",
            errors
                .iter()
                .map(|finding| finding.message.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn worktree_doctor_next_actions(findings: &[WorktreeDoctorFinding]) -> Vec<String> {
    if findings.is_empty() {
        return vec!["continue with `cargo xtask precommit`".to_string()];
    }

    let mut actions = BTreeSet::new();
    for finding in findings {
        let message = finding.message.as_str();
        if message.contains("main branch has uncommitted changes") {
            actions.insert(
                "move this work to a feature branch or fresh worktree before continuing"
                    .to_string(),
            );
        }
        if message.contains("behind origin/main") {
            actions.insert(
                "refresh the branch from `origin/main` before opening or updating a PR".to_string(),
            );
        }
        if message.contains("generated badge endpoint") {
            actions.insert(
                "restore `badges/*.json` unless this is an intentional badge refresh".to_string(),
            );
        }
        if message.contains("generated RIPR target artifact")
            || message.contains("target/ripr exists")
        {
            actions.insert("remove local `target/ripr` report artifacts when the workspace is ready for handoff".to_string());
        }
        if message.contains("sample workspace target artifact")
            || message.contains("crates/ripr/examples/sample/target exists")
        {
            actions.insert("remove `crates/ripr/examples/sample/target` or run `cargo clean` in that sample workspace".to_string());
        }
        if message.contains("changes span multiple source-of-truth layers") {
            actions.insert("add or update an explicit work item marker before continuing broad source-of-truth work".to_string());
        }
    }
    if actions.is_empty() {
        actions.insert(
            "inspect the findings above and rerun `cargo xtask worktree doctor`".to_string(),
        );
    } else {
        actions.insert("rerun `cargo xtask worktree doctor` after cleanup".to_string());
    }
    actions.into_iter().collect()
}

fn normalize_repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(normalize_path)
        .unwrap_or_else(|_| normalize_path(path))
}

fn is_kebab_case_id(value: &str) -> bool {
    let mut previous_dash = false;
    let mut saw_char = false;
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => {
                saw_char = true;
                previous_dash = false;
            }
            b'-' if saw_char && !previous_dash => previous_dash = true,
            _ => return false,
        }
    }
    saw_char && !previous_dash
}

fn known_xtask_command(command: &str) -> bool {
    known_commands()
        .into_iter()
        .map(known_command_root)
        .any(|known| known == command)
}

fn require_index_mentions_files(
    index_path: &Path,
    directory: &Path,
    excluded_names: &[&str],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let index = read_text_lossy(index_path)?;
    for path in collect_files(directory)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if excluded_names.contains(&name) {
            continue;
        }
        if !index.contains(name) {
            violations.push(format!(
                "{} does not index {}",
                normalize_path(index_path),
                normalize_path(&path)
            ));
        }
    }
    Ok(())
}

fn check_generated() -> Result<(), String> {
    let allowlist = read_glob_allowlist("policy/generated_allowlist.txt")?;
    let mut violations = Vec::new();

    for normalized in tracked_files()? {
        if !is_generated_candidate(&normalized) {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "tracked generated output is not allowlisted: {normalized}\n  preferred: keep generated outputs out of git unless they are an intentional lockfile or fixture golden"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "generated.md",
            check: "check-generated",
            why_it_matters: "Generated files should be reproducible and intentionally checked in only for approved surfaces such as lockfiles or fixture goldens.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove accidental build artifacts from git.",
                "Regenerate approved outputs from their source command.",
                "Add an allowlist entry only when the generated file is an intentional repository artifact.",
            ],
            rerun_command: "cargo xtask check-generated",
            exception_template: Some(
                "policy/generated_allowlist.txt entry:\nglob|kind|owner|reason",
            ),
        },
        &violations,
    )
}

fn check_generated_clean() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let badge_refresh_context = badge_refresh_context();
    let violations = generated_clean_violations(&changes, badge_refresh_context);

    finish_policy_report(
        PolicyReportSpec {
            report_file: "generated-clean.md",
            check: "check-generated-clean",
            why_it_matters: "Generated evidence and build residue should not leak into ordinary PR diffs. Public badge endpoint counts are generated trust markers, and target artifacts are local/CI outputs.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Remove generated target artifacts from the PR diff.",
                "Remove badges/*.json diffs unless this is the generated badge endpoint refresh PR.",
                "For public badge count refreshes, use `cargo xtask badges` or the Badge Endpoints workflow on an explicit badge refresh branch.",
            ],
            rerun_command: "cargo xtask check-generated-clean",
            exception_template: None,
        },
        &violations,
    )
}

fn badge_refresh_context() -> bool {
    let mut candidates = vec![git_value(&["rev-parse", "--abbrev-ref", "HEAD"])];
    for key in [
        "GITHUB_HEAD_REF",
        "GITHUB_REF_NAME",
        "GITHUB_REF",
        "BRANCH_NAME",
        "GITHUB_PR_TITLE",
        "PR_TITLE",
        "PULL_REQUEST_TITLE",
        "RIPR_PR_TITLE",
        "RIPR_WORK_ITEM",
    ] {
        if let Ok(value) = std::env::var(key) {
            candidates.push(value);
        }
    }
    if let Some(title) = github_event_pull_request_title() {
        candidates.push(title);
    }
    candidates
        .iter()
        .any(|candidate| is_badge_refresh_context(candidate))
}

fn github_event_pull_request_title() -> Option<String> {
    let event_path = std::env::var("GITHUB_EVENT_PATH").ok()?;
    let text = read_text_lossy(&PathBuf::from(event_path)).ok()?;
    github_event_pull_request_title_from_text(&text)
}

fn github_event_pull_request_title_from_text(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value
        .get("pull_request")
        .and_then(|pull_request| pull_request.get("title"))
        .or_else(|| value.get("issue").and_then(|issue| issue.get("title")))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_badge_refresh_context(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_start_matches("refs/heads/")
        .to_ascii_lowercase();
    normalized == "automation/badge-endpoints"
        || normalized == "badge: refresh public endpoints"
        || normalized.contains("badge-refresh")
        || normalized.contains("badge/endpoints")
        || normalized.contains("badge-endpoints")
}

fn generated_clean_violations(changes: &[ChangedPath], badge_refresh_context: bool) -> Vec<String> {
    let mut violations = badge_diff_policy_violations(changes, badge_refresh_context);
    for change in changes {
        let path = change.path.trim_end_matches('/');
        if is_badge_endpoint_json(path) {
            continue;
        }

        if is_ripr_target_artifact(path) && !is_deletion_only(change) {
            violations.push(format!(
                "generated RIPR target artifact is present in the PR diff: {}\n  rule: keep PR-scoped RIPR evidence under ignored target/ripr artifacts, not committed source control",
                format_changed_path(change)
            ));
            continue;
        }

        if is_sample_target_artifact(path) && !is_deletion_only(change) {
            violations.push(format!(
                "sample workspace build output is present in the PR diff: {}\n  rule: remove crates/ripr/examples/sample/target residue before review",
                format_changed_path(change)
            ));
        }
    }
    violations
}

fn badge_diff_policy_violations(
    changes: &[ChangedPath],
    badge_refresh_context: bool,
) -> Vec<String> {
    let mut violations = Vec::new();
    for change in changes {
        let path = change.path.trim_end_matches('/');
        if is_badge_endpoint_json(path) && !badge_refresh_context {
            violations.push(format!(
                "generated badge endpoint changed in an ordinary PR: {}\n  rule: do not manually edit RIPR badge numbers; remove this diff or move it to the generated `badge: refresh public endpoints` PR",
                format_changed_path(change)
            ));
        }
        if is_public_badge_basis_surface(path)
            && !is_deletion_only(change)
            && let Ok(text) = read_text_lossy(&PathBuf::from(path))
        {
            violations.extend(public_badge_basis_violations(path, &text));
        }
    }
    violations
}

fn is_public_badge_basis_surface(path: &str) -> bool {
    matches!(
        path,
        "README.md"
            | "crates/ripr/README.md"
            | "editors/vscode/README.md"
            | "editors/vscode/package.json"
            | "docs/RELEASE_MARKETPLACE.md"
    )
}

fn public_badge_basis_violations(path: &str, text: &str) -> Vec<String> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut violations = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let normalized_line = normalize_badge_basis_text(line);
        if !normalized_line.contains("seam_native") {
            continue;
        }
        let context = public_badge_basis_context(&lines, index);
        if context.contains("seam inventory")
            || context.contains("seam-native inventory")
            || context.contains("internal inventory")
            || context.contains("inventory badge")
        {
            continue;
        }
        if context.contains("badge") || path.ends_with("package.json") {
            violations.push(format!(
                "public badge surface uses `seam_native` as a repair badge basis: {path}:{}\n  rule: README/crate/store badges must use `canonical_actionable_gap` for public repair counts, or explicitly relabel the badge as seam inventory before using `seam_native`",
                index + 1
            ));
        }
    }
    violations
}

fn public_badge_basis_context(lines: &[&str], index: usize) -> String {
    let start = index.saturating_sub(2);
    let end = (index + 3).min(lines.len());
    lines[start..end]
        .iter()
        .map(|line| normalize_badge_basis_text(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_badge_basis_text(text: &str) -> String {
    text.to_ascii_lowercase().replace('-', "_")
}

fn is_badge_endpoint_json(path: &str) -> bool {
    path.starts_with("badges/")
        && path.ends_with(".json")
        && path["badges/".len()..].find('/').is_none()
}

fn is_ripr_target_artifact(path: &str) -> bool {
    path == "target/ripr" || path.starts_with("target/ripr/")
}

fn is_sample_target_artifact(path: &str) -> bool {
    path == "crates/ripr/examples/sample/target"
        || path.starts_with("crates/ripr/examples/sample/target/")
}

fn is_deletion_only(change: &ChangedPath) -> bool {
    !change.statuses.is_empty()
        && change
            .statuses
            .iter()
            .all(|status| status.chars().all(|character| character == 'D'))
}

/// Parse a `[workspace.lints.<section>]` block from `Cargo.toml`-shaped TOML.
///
/// Returns a map of bare lint name (e.g. `unwrap_used`, `unsafe_code`) to its
/// level string (e.g. `deny`, `forbid`, `warn`, `allow`). Only direct
/// `name = "level"` lines are recognized; this matches how the workspace
/// declares lints today and keeps the parser dependency-free.
fn parse_workspace_lints_section(text: &str, section: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let header = format!("[workspace.lints.{section}]");
    let mut in_section = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            continue;
        }
        if line == header {
            in_section = true;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_section = false;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = raw_value.trim().trim_end_matches('#').trim();
        // Take the first quoted token as the level. Anything more elaborate
        // (table-form lint configuration) is intentionally not supported here
        // — workspace lints today are flat `name = "level"` lines.
        let level = value
            .strip_prefix('"')
            .and_then(|rest| rest.split_once('"').map(|(level, _)| level.to_string()));
        if let Some(level) = level {
            out.insert(key.to_string(), level);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerLintEntry {
    name: String,
    level: String,
    activate_when_msrv: Option<String>,
    block_line: usize,
    is_planned: bool,
}

/// Parse `[[active.<group>]]` and `[[planned]]` blocks out of
/// `policy/clippy-lints.toml`. Returns the entries in source order.
fn parse_clippy_lints_ledger(text: &str) -> (Vec<LedgerLintEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut violations = Vec::new();
    let mut current: Option<LedgerLintEntry> = None;

    fn flush(
        current: &mut Option<LedgerLintEntry>,
        entries: &mut Vec<LedgerLintEntry>,
        violations: &mut Vec<String>,
    ) {
        if let Some(entry) = current.take() {
            if entry.name.is_empty() {
                violations.push(format!(
                    "policy/clippy-lints.toml:{} entry missing required `name`",
                    entry.block_line
                ));
                return;
            }
            if entry.level.is_empty() {
                violations.push(format!(
                    "policy/clippy-lints.toml:{} entry `{}` missing required `level`",
                    entry.block_line, entry.name
                ));
                return;
            }
            entries.push(entry);
        }
    }

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("[[") {
            let header = rest.trim_end_matches(']').trim_end_matches('[').trim();
            // Only reset when we cross into a new array-of-tables that
            // matters to the lint policy. Other top-level tables (like
            // `[policy]`) are flushed too so the next array-of-tables starts
            // clean.
            flush(&mut current, &mut entries, &mut violations);
            let is_active = header.starts_with("active.");
            let is_planned = header == "planned";
            if is_active || is_planned {
                current = Some(LedgerLintEntry {
                    name: String::new(),
                    level: String::new(),
                    activate_when_msrv: None,
                    block_line: line_number,
                    is_planned,
                });
            }
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            // Plain table header (e.g. `[policy]`); not a lint entry.
            flush(&mut current, &mut entries, &mut violations);
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = raw_value.trim();
        let unquoted = value
            .strip_prefix('"')
            .and_then(|rest| rest.split_once('"').map(|(token, _)| token.to_string()));
        match key {
            "name" => {
                if let Some(name) = unquoted {
                    entry.name = name;
                }
            }
            "level" => {
                if let Some(level) = unquoted {
                    entry.level = level;
                }
            }
            "activate_when_msrv" => {
                if let Some(msrv) = unquoted {
                    entry.activate_when_msrv = Some(msrv);
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut entries, &mut violations);
    (entries, violations)
}

/// Strip the `clippy::` prefix from a ledger-style lint name so it can be
/// looked up in the `[workspace.lints.clippy]` map. Rust lints have no
/// prefix and remain unchanged.
fn ledger_name_to_lookup(name: &str) -> (&str, &'static str) {
    if let Some(rest) = name.strip_prefix("clippy::") {
        (rest, "clippy")
    } else {
        (name, "rust")
    }
}

fn check_lint_policy() -> Result<(), String> {
    let cargo_text = fs::read_to_string("Cargo.toml")
        .map_err(|err| format!("failed to read Cargo.toml: {err}"))?;
    let ledger_text = fs::read_to_string("policy/clippy-lints.toml")
        .map_err(|err| format!("failed to read policy/clippy-lints.toml: {err}"))?;

    let cargo_clippy = parse_workspace_lints_section(&cargo_text, "clippy");
    let cargo_rust = parse_workspace_lints_section(&cargo_text, "rust");
    let (entries, mut violations) = parse_clippy_lints_ledger(&ledger_text);

    for entry in &entries {
        let (bare, group) = ledger_name_to_lookup(&entry.name);
        let cargo_map = match group {
            "clippy" => &cargo_clippy,
            _ => &cargo_rust,
        };
        let actual = cargo_map.get(bare).cloned();
        if entry.is_planned {
            if let Some(level) = actual {
                violations.push(format!(
                    "policy/clippy-lints.toml:{} declares `{}` as `[[planned]]` but Cargo.toml `[workspace.lints.{}]` already activates it at level `{}`. Promote the ledger entry to `[[active.<group>]]` or remove the Cargo.toml line.",
                    entry.block_line, entry.name, group, level
                ));
            }
            continue;
        }
        match actual {
            None => violations.push(format!(
                "policy/clippy-lints.toml:{} declares `{}` as active at level `{}` but Cargo.toml `[workspace.lints.{}]` does not configure it. Add the lint to Cargo.toml or move the ledger entry to `[[planned]]`.",
                entry.block_line, entry.name, entry.level, group
            )),
            Some(level) if level != entry.level => violations.push(format!(
                "policy/clippy-lints.toml:{} declares `{}` at level `{}` but Cargo.toml `[workspace.lints.{}]` configures it at level `{}`. Reconcile the two.",
                entry.block_line, entry.name, entry.level, group, level
            )),
            Some(_) => {}
        }
    }

    let active_lookup: BTreeSet<(String, String)> = entries
        .iter()
        .filter(|entry| !entry.is_planned)
        .map(|entry| {
            let (bare, group) = ledger_name_to_lookup(&entry.name);
            (group.to_string(), bare.to_string())
        })
        .collect();

    for (name, level) in cargo_clippy.iter().chain(cargo_rust.iter()) {
        let group = if cargo_clippy.contains_key(name) {
            "clippy"
        } else {
            "rust"
        };
        if !active_lookup.contains(&(group.to_string(), name.clone())) {
            violations.push(format!(
                "Cargo.toml `[workspace.lints.{group}]` configures `{name}` at level `{level}` but policy/clippy-lints.toml has no matching `[[active.<group>]]` entry. Add the lint to the ledger or remove it from Cargo.toml."
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "lint-policy.md",
            check: "check-lint-policy",
            why_it_matters: "`policy/clippy-lints.toml` is the reviewable ledger of the workspace lint stance, including planned 1.94 / 1.95 flips. If Cargo.toml drifts from the ledger, reviewers lose the trajectory and the dual-rail design (clippy + semantic checker) loses its receipt.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Make `Cargo.toml` and `policy/clippy-lints.toml` agree: every `[[active.<group>]]` entry must appear in `[workspace.lints.*]` at the same level, and `[[planned]]` entries must not yet appear there.",
                "When promoting a planned lint, move the ledger entry from `[[planned]]` to `[[active.<group>]]` and add the matching `Cargo.toml` line in the same PR.",
                "Document `[[active.<group>]]` family blocks in `docs/CLIPPY_POLICY.md` so the public surface stays in sync.",
            ],
            rerun_command: "cargo xtask check-lint-policy",
            exception_template: None,
        },
        &violations,
    )
}

#[derive(Clone, Debug)]
struct CiLedgerValue {
    line: usize,
    raw: String,
}

#[derive(Clone, Debug)]
struct CiLedgerTable {
    header: String,
    line: usize,
    values: BTreeMap<String, CiLedgerValue>,
}

#[derive(Clone, Debug, Default)]
struct CiLedgerDocument {
    top_level: BTreeMap<String, CiLedgerValue>,
    tables: Vec<CiLedgerTable>,
}

fn check_ci_lane_whitelist_impl() -> Result<(), String> {
    let mut violations = Vec::new();

    let budget = read_ci_ledger_document("policy/ci-budget.toml", &mut violations);
    let lanes = read_ci_ledger_document("policy/ci-lane-whitelist.toml", &mut violations);
    let risk_packs = read_ci_ledger_document("policy/ci-risk-packs.toml", &mut violations);
    let exceptions =
        read_ci_ledger_document("policy/ci-whitelist-exceptions.toml", &mut violations);

    if let (Some(budget), Some(lanes), Some(risk_packs), Some(exceptions)) =
        (&budget, &lanes, &risk_packs, &exceptions)
    {
        violations.extend(ci_lane_whitelist_violations(
            budget, lanes, risk_packs, exceptions,
        ));
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "ci-lane-whitelist.md",
            check: "check-ci-lane-whitelist",
            why_it_matters: "The CI lane ledgers are the reviewable contract for future PR planning, advisory evidence, artifact families, and budget actuals. They should stay structurally coherent before any workflow starts consuming them.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep `policy/ci-budget.toml`, `policy/ci-lane-whitelist.toml`, `policy/ci-risk-packs.toml`, and `policy/ci-whitelist-exceptions.toml` on schema version 0.1 with advisory, non-enforcing metadata.",
                "Add lane IDs and artifact families to `policy/ci-lane-whitelist.toml` before risk packs or exceptions reference them.",
                "Keep referenced lane IDs, artifact families, budget bands, owners, and reasons explicit before workflow or planner code consumes the ledgers.",
            ],
            rerun_command: "cargo xtask check-ci-lane-whitelist",
            exception_template: None,
        },
        &violations,
    )
}

fn read_ci_ledger_document(path: &str, violations: &mut Vec<String>) -> Option<CiLedgerDocument> {
    let path_ref = Path::new(path);
    if !path_ref.exists() {
        violations.push(format!("{path} is missing"));
        return None;
    }
    let text = match fs::read_to_string(path_ref) {
        Ok(text) => text,
        Err(err) => {
            violations.push(format!("failed to read {path}: {err}"));
            return None;
        }
    };
    let (document, mut parse_violations) = parse_ci_ledger_document(path, &text);
    violations.append(&mut parse_violations);
    Some(document)
}

fn parse_ci_ledger_document(path: &str, text: &str) -> (CiLedgerDocument, Vec<String>) {
    let mut document = CiLedgerDocument::default();
    let mut violations = Vec::new();
    let mut current: Option<CiLedgerTable> = None;

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("[[") && line.ends_with("]]") {
            if let Some(table) = current.take() {
                document.tables.push(table);
            }
            let header = line
                .trim_start_matches("[[")
                .trim_end_matches("]]")
                .trim()
                .to_string();
            if header.is_empty() {
                violations.push(format!("{path}:{line_number} empty table header"));
                continue;
            }
            current = Some(CiLedgerTable {
                header,
                line: line_number,
                values: BTreeMap::new(),
            });
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if let Some(table) = current.take() {
                document.tables.push(table);
            }
            let header = line
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            if header.is_empty() {
                violations.push(format!("{path}:{line_number} empty table header"));
                continue;
            }
            current = Some(CiLedgerTable {
                header,
                line: line_number,
                values: BTreeMap::new(),
            });
            continue;
        }

        let Some((key, value)) = parse_toml_key_value(line) else {
            violations.push(format!(
                "{path}:{line_number} invalid TOML line (expected key = value)"
            ));
            continue;
        };
        let value = strip_toml_value_comment(value).trim().to_string();
        if let Some(table) = current.as_mut() {
            insert_ci_ledger_value(
                path,
                &mut table.values,
                key,
                value,
                line_number,
                &mut violations,
            );
        } else {
            insert_ci_ledger_value(
                path,
                &mut document.top_level,
                key,
                value,
                line_number,
                &mut violations,
            );
        }
    }

    if let Some(table) = current {
        document.tables.push(table);
    }

    (document, violations)
}

fn insert_ci_ledger_value(
    path: &str,
    values: &mut BTreeMap<String, CiLedgerValue>,
    key: &str,
    raw: String,
    line: usize,
    violations: &mut Vec<String>,
) {
    if values
        .insert(key.to_string(), CiLedgerValue { line, raw })
        .is_some()
    {
        violations.push(format!("{path}:{line} duplicate key `{key}`"));
    }
}

fn ci_lane_whitelist_violations(
    budget: &CiLedgerDocument,
    lanes: &CiLedgerDocument,
    risk_packs: &CiLedgerDocument,
    exceptions: &CiLedgerDocument,
) -> Vec<String> {
    let mut violations = Vec::new();

    validate_ci_common_metadata("policy/ci-budget.toml", budget, &mut violations);
    validate_ci_common_metadata("policy/ci-lane-whitelist.toml", lanes, &mut violations);
    validate_ci_common_metadata("policy/ci-risk-packs.toml", risk_packs, &mut violations);
    validate_ci_common_metadata(
        "policy/ci-whitelist-exceptions.toml",
        exceptions,
        &mut violations,
    );

    let budget_bands = validate_ci_budget("policy/ci-budget.toml", budget, &mut violations);
    let (lane_ids, _lane_postures, artifact_ids) =
        validate_ci_lanes("policy/ci-lane-whitelist.toml", lanes, &mut violations);
    validate_ci_risk_packs(
        "policy/ci-risk-packs.toml",
        risk_packs,
        &lane_ids,
        &artifact_ids,
        &mut violations,
    );
    validate_ci_exceptions(
        "policy/ci-whitelist-exceptions.toml",
        exceptions,
        &lane_ids,
        &artifact_ids,
        &mut violations,
    );
    validate_ci_budget_references(
        "policy/ci-budget.toml",
        budget,
        &budget_bands,
        &mut violations,
    );

    violations
}

fn validate_ci_common_metadata(
    path: &str,
    document: &CiLedgerDocument,
    violations: &mut Vec<String>,
) {
    ci_expect_top_string(path, document, "schema_version", "0.1", violations);
    ci_expect_top_string(
        path,
        document,
        "policy_state",
        "advisory-ledger",
        violations,
    );
    ci_expect_top_string(path, document, "enforcement", "none", violations);
    ci_required_non_empty_top_string(path, document, "owner", violations);
    ci_required_non_empty_top_string(path, document, "reason", violations);
}

fn validate_ci_budget(
    path: &str,
    document: &CiLedgerDocument,
    violations: &mut Vec<String>,
) -> BTreeSet<String> {
    ci_expect_top_string(path, document, "unit", "lem", violations);

    let mut budget_bands = BTreeSet::new();
    let bands = ci_tables(document, "budget_band");
    if bands.is_empty() {
        violations.push(format!("{path} has no [[budget_band]] entries"));
    }
    for table in bands {
        let Some(id) = ci_required_table_id(path, table, "id", "budget band", violations) else {
            continue;
        };
        if !budget_bands.insert(id.clone()) {
            violations.push(format!(
                "{path}:{} duplicate budget band id `{id}`",
                table.line
            ));
        }
        let min = ci_required_table_usize(path, table, "min_lem", violations);
        let max = ci_optional_table_usize(path, table, "max_lem", violations);
        if let (Some(min), Some(max)) = (min, max)
            && max < min
        {
            violations.push(format!(
                "{path}:{} budget band `{id}` has max_lem below min_lem",
                table.line
            ));
        }
        ci_required_non_empty_table_string(path, table, "posture", violations);
        ci_required_non_empty_table_string(path, table, "description", violations);
    }

    let labels = ci_tables(document, "label");
    if labels.is_empty() {
        violations.push(format!("{path} has no [[label]] entries"));
    }
    let mut label_names = BTreeSet::new();
    for table in labels {
        let Some(name) = ci_required_non_empty_table_string(path, table, "name", violations) else {
            continue;
        };
        if !label_names.insert(name.clone()) {
            violations.push(format!("{path}:{} duplicate label `{name}`", table.line));
        }
        ci_required_non_empty_table_string(path, table, "effect", violations);
        ci_required_non_empty_table_string(path, table, "budget_effect", violations);
        ci_required_non_empty_table_string(path, table, "notes", violations);
    }

    budget_bands
}

fn validate_ci_budget_references(
    path: &str,
    document: &CiLedgerDocument,
    budget_bands: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let defaults = ci_tables(document, "defaults");
    if defaults.len() != 1 {
        violations.push(format!("{path} should have exactly one [defaults] table"));
    }
    if let Some(table) = defaults.first() {
        for key in ["required_pr_budget", "advisory_pr_budget", "release_budget"] {
            if let Some(value) = ci_required_non_empty_table_string(path, table, key, violations)
                && !budget_bands.contains(&value)
            {
                violations.push(format!(
                    "{path}:{} defaults `{key}` references unknown budget band `{value}`",
                    table.line
                ));
            }
        }
    }

    for table in ci_tables(document, "label") {
        if let Some(effect) =
            ci_required_non_empty_table_string(path, table, "budget_effect", violations)
            && effect != "none"
            && !budget_bands.contains(&effect)
        {
            violations.push(format!(
                "{path}:{} label budget_effect references unknown budget band `{effect}`",
                table.line
            ));
        }
    }
}

fn validate_ci_lanes(
    path: &str,
    document: &CiLedgerDocument,
    violations: &mut Vec<String>,
) -> (BTreeSet<String>, BTreeMap<String, String>, BTreeSet<String>) {
    let mut artifact_ids = BTreeSet::new();
    let artifacts = ci_tables(document, "artifact_family");
    if artifacts.is_empty() {
        violations.push(format!("{path} has no [[artifact_family]] entries"));
    }
    for table in artifacts {
        let Some(id) = ci_required_table_id(path, table, "id", "artifact family", violations)
        else {
            continue;
        };
        if !artifact_ids.insert(id.clone()) {
            violations.push(format!(
                "{path}:{} duplicate artifact family id `{id}`",
                table.line
            ));
        }
        if let Some(paths) = ci_required_table_array(path, table, "paths", violations)
            && paths.is_empty()
        {
            violations.push(format!(
                "{path}:{} artifact family `{id}` has empty paths",
                table.line
            ));
        }
    }

    let mut lane_ids = BTreeSet::new();
    let mut lane_postures = BTreeMap::new();
    let lanes = ci_tables(document, "lane");
    if lanes.is_empty() {
        violations.push(format!("{path} has no [[lane]] entries"));
    }
    for table in lanes {
        let Some(id) = ci_required_table_id(path, table, "id", "lane", violations) else {
            continue;
        };
        if !lane_ids.insert(id.clone()) {
            violations.push(format!("{path}:{} duplicate lane id `{id}`", table.line));
        }
        let posture = ci_required_non_empty_table_string(path, table, "posture", violations);
        if let Some(posture) = posture {
            if !matches!(
                posture.as_str(),
                "required" | "advisory" | "on_demand_release"
            ) {
                violations.push(format!(
                    "{path}:{} lane `{id}` has unsupported posture `{posture}`",
                    table.line
                ));
            }
            lane_postures.insert(id.clone(), posture);
        }
        ci_required_non_empty_table_string(path, table, "workflow", violations);
        ci_required_table_array(path, table, "jobs", violations);
        ci_required_table_array(path, table, "commands", violations);
        ci_required_table_usize(path, table, "estimated_lem", violations);
        if let Some(families) =
            ci_required_table_array(path, table, "artifact_families", violations)
        {
            validate_ci_artifact_references(
                path,
                table.line,
                &format!("lane `{id}`"),
                &families,
                &artifact_ids,
                violations,
            );
        }
        ci_required_non_empty_table_string(path, table, "description", violations);
    }

    (lane_ids, lane_postures, artifact_ids)
}

fn validate_ci_risk_packs(
    path: &str,
    document: &CiLedgerDocument,
    lane_ids: &BTreeSet<String>,
    artifact_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let risk_packs: Vec<&CiLedgerTable> = document
        .tables
        .iter()
        .filter(|table| table.header.starts_with("risk_pack."))
        .collect();
    if risk_packs.is_empty() {
        violations.push(format!("{path} has no [risk_pack.<id>] entries"));
    }

    let mut pack_ids = BTreeSet::new();
    for table in risk_packs {
        let id = table.header.trim_start_matches("risk_pack.");
        if !is_snake_case_id(id) {
            violations.push(format!(
                "{path}:{} risk pack id `{id}` should be snake_case",
                table.line
            ));
        }
        if !pack_ids.insert(id.to_string()) {
            violations.push(format!(
                "{path}:{} duplicate risk pack id `{id}`",
                table.line
            ));
        }
        if let Some(paths) = ci_required_table_array(path, table, "paths", violations)
            && paths.is_empty()
        {
            violations.push(format!(
                "{path}:{} risk pack `{id}` has empty paths",
                table.line
            ));
        }
        validate_ci_lane_bucket(path, table, id, "required", lane_ids, violations);
        validate_ci_lane_bucket(path, table, id, "advisory", lane_ids, violations);
        validate_ci_lane_bucket(path, table, id, "on_demand", lane_ids, violations);
        if let Some(families) =
            ci_required_table_array(path, table, "artifact_families", violations)
        {
            validate_ci_artifact_references(
                path,
                table.line,
                &format!("risk pack `{id}`"),
                &families,
                artifact_ids,
                violations,
            );
        }
        ci_required_table_usize(path, table, "estimated_lem", violations);
        ci_required_non_empty_table_string(path, table, "owner", violations);
        ci_required_non_empty_table_string(path, table, "reason", violations);
    }
}

fn validate_ci_exceptions(
    path: &str,
    document: &CiLedgerDocument,
    lane_ids: &BTreeSet<String>,
    artifact_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let exceptions = ci_tables(document, "exception");
    if exceptions.is_empty() {
        violations.push(format!("{path} has no [[exception]] entries"));
    }

    let mut exception_ids = BTreeSet::new();
    for table in exceptions {
        let Some(id) = ci_required_non_empty_table_string(path, table, "id", violations) else {
            continue;
        };
        if !exception_ids.insert(id.clone()) {
            violations.push(format!(
                "{path}:{} duplicate exception id `{id}`",
                table.line
            ));
        }
        ci_required_non_empty_table_string(path, table, "kind", violations);
        if let Some(paths) = ci_required_table_array(path, table, "paths", violations)
            && paths.is_empty()
        {
            violations.push(format!(
                "{path}:{} exception `{id}` has empty paths",
                table.line
            ));
        }
        ci_required_non_empty_table_string(path, table, "current_behavior", violations);
        ci_required_non_empty_table_string(path, table, "target_behavior", violations);
        ci_required_non_empty_table_string(path, table, "review_note", violations);

        if let Some(lanes) = ci_optional_table_array(path, table, "lanes", violations) {
            validate_ci_lane_references(
                path,
                table.line,
                &format!("exception `{id}`"),
                &lanes,
                lane_ids,
                violations,
            );
        }
        if let Some(families) =
            ci_optional_table_array(path, table, "artifact_families", violations)
        {
            validate_ci_artifact_references(
                path,
                table.line,
                &format!("exception `{id}`"),
                &families,
                artifact_ids,
                violations,
            );
        }
    }
}

fn validate_ci_lane_bucket(
    path: &str,
    table: &CiLedgerTable,
    risk_pack_id: &str,
    key: &str,
    lane_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let Some(lanes) = ci_required_table_array(path, table, key, violations) else {
        return;
    };
    for lane in lanes {
        if !lane_ids.contains(&lane) {
            violations.push(format!(
                "{path}:{} risk pack `{risk_pack_id}` references unknown lane `{lane}` in `{key}`",
                table.line
            ));
        }
    }
}

fn validate_ci_lane_references(
    path: &str,
    line: usize,
    label: &str,
    lanes: &[String],
    lane_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    for lane in lanes {
        if !lane_ids.contains(lane) {
            violations.push(format!(
                "{path}:{line} {label} references unknown lane `{lane}`"
            ));
        }
    }
}

fn validate_ci_artifact_references(
    path: &str,
    line: usize,
    label: &str,
    families: &[String],
    artifact_ids: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    for family in families {
        if !artifact_ids.contains(family) {
            violations.push(format!(
                "{path}:{line} {label} references unknown artifact family `{family}`"
            ));
        }
    }
}

fn ci_expect_top_string(
    path: &str,
    document: &CiLedgerDocument,
    key: &str,
    expected: &str,
    violations: &mut Vec<String>,
) {
    if let Some(actual) = ci_required_non_empty_top_string(path, document, key, violations)
        && actual != expected
    {
        violations.push(format!(
            "{path}: top-level `{key}` should be `{expected}`, got `{actual}`"
        ));
    }
}

fn ci_required_non_empty_top_string(
    path: &str,
    document: &CiLedgerDocument,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<String> {
    let Some(value) = document.top_level.get(key) else {
        violations.push(format!("{path}: missing top-level `{key}`"));
        return None;
    };
    ci_string_value(path, "top-level", key, value, violations).and_then(|parsed| {
        if parsed.trim().is_empty() {
            violations.push(format!("{path}:{} top-level `{key}` is empty", value.line));
            None
        } else {
            Some(parsed)
        }
    })
}

fn ci_required_table_id(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    label: &str,
    violations: &mut Vec<String>,
) -> Option<String> {
    let value = ci_required_non_empty_table_string(path, table, key, violations)?;
    if !is_kebab_case_id(&value) {
        violations.push(format!(
            "{path}:{} {label} id `{value}` should be kebab-case",
            table.line
        ));
    }
    Some(value)
}

fn ci_required_non_empty_table_string(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<String> {
    let Some(value) = table.values.get(key) else {
        violations.push(format!(
            "{path}:{} table `[{}]` missing `{key}`",
            table.line, table.header
        ));
        return None;
    };
    ci_string_value(
        path,
        &format!("table `[{}]`", table.header),
        key,
        value,
        violations,
    )
    .and_then(|parsed| {
        if parsed.trim().is_empty() {
            violations.push(format!(
                "{path}:{} table `[{}]` field `{key}` is empty",
                value.line, table.header
            ));
            None
        } else {
            Some(parsed)
        }
    })
}

fn ci_string_value(
    path: &str,
    label: &str,
    key: &str,
    value: &CiLedgerValue,
    violations: &mut Vec<String>,
) -> Option<String> {
    match parse_string_value(&value.raw, path, value.line) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            violations.push(format!("{path}:{} {label} `{key}`: {err}", value.line));
            None
        }
    }
}

fn ci_required_table_array(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<Vec<String>> {
    let Some(value) = table.values.get(key) else {
        violations.push(format!(
            "{path}:{} table `[{}]` missing `{key}`",
            table.line, table.header
        ));
        return None;
    };
    ci_array_value(path, table, key, value, violations)
}

fn ci_optional_table_array(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<Vec<String>> {
    let value = table.values.get(key)?;
    ci_array_value(path, table, key, value, violations)
}

fn ci_array_value(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    value: &CiLedgerValue,
    violations: &mut Vec<String>,
) -> Option<Vec<String>> {
    match parse_inline_array(&value.raw) {
        Ok(values) => Some(values),
        Err(err) => {
            violations.push(format!(
                "{path}:{} table `[{}]` field `{key}`: {err}",
                value.line, table.header
            ));
            None
        }
    }
}

fn ci_required_table_usize(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<usize> {
    let Some(value) = table.values.get(key) else {
        violations.push(format!(
            "{path}:{} table `[{}]` missing `{key}`",
            table.line, table.header
        ));
        return None;
    };
    ci_usize_value(path, table, key, value, violations)
}

fn ci_optional_table_usize(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    violations: &mut Vec<String>,
) -> Option<usize> {
    let value = table.values.get(key)?;
    ci_usize_value(path, table, key, value, violations)
}

fn ci_usize_value(
    path: &str,
    table: &CiLedgerTable,
    key: &str,
    value: &CiLedgerValue,
    violations: &mut Vec<String>,
) -> Option<usize> {
    match parse_usize_value(&value.raw, path, value.line) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            violations.push(format!(
                "{path}:{} table `[{}]` field `{key}`: {err}",
                value.line, table.header
            ));
            None
        }
    }
}

fn ci_tables<'a>(document: &'a CiLedgerDocument, header: &str) -> Vec<&'a CiLedgerTable> {
    document
        .tables
        .iter()
        .filter(|table| table.header == header)
        .collect()
}

const PROOF_PACK_MANIFEST_PATH: &str = "policy/proof-packs.toml";
const PROOF_PACK_LANE_WHITELIST_PATH: &str = "policy/ci-lane-whitelist.toml";
const PROOF_PACK_RELEASE_PACK_ID: &str = "release-package";

/// Release surfaces the `release-package` pack MUST cover so that any change to
/// a version file, the changelog, or a release workflow trips
/// `release_proof_required`. Asserted by `check-proof-packs` (the release-proof
/// protection slice of docs/PROOF_ROUTING.md) so a future routing slice cannot
/// weaken release detection by dropping a surface from the pack.
const PROOF_PACK_RELEASE_REQUIRED_PATHS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "crates/ripr/Cargo.toml",
    "CHANGELOG.md",
    "editors/vscode/package.json",
    "editors/vscode/package-lock.json",
    ".github/workflows/publish-extension.yml",
    ".github/workflows/release-server-binaries.yml",
];

/// The full release proof the `release-package` pack MUST require: workspace
/// tests, clippy, check-pr, package contents, publish dry-run, and the
/// release-readiness gate (release-notes and known-limits docs validation).
/// Asserted by `check-proof-packs` so a future routing slice cannot weaken
/// release validation by dropping a gate from the pack.
const PROOF_PACK_RELEASE_REQUIRED_COMMANDS: &[&str] = &[
    "cargo test --workspace",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo xtask check-pr",
    "cargo package -p ripr --list",
    "cargo publish -p ripr --dry-run",
    "cargo xtask release-readiness",
];

/// Non-xtask repo commands a proof pack may reference. `cargo xtask <command>`
/// entries are validated against the known xtask command roots instead.
const PROOF_PACK_KNOWN_REPO_COMMANDS: &[&str] = &[
    "cargo fmt --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace",
    "cargo test -p ripr",
    "cargo test -p xtask",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo doc --workspace --no-deps",
    "cargo package -p ripr --list",
    "cargo publish -p ripr --dry-run",
];

fn check_proof_packs_impl() -> Result<(), String> {
    let mut violations = Vec::new();

    let manifest = read_ci_ledger_document(PROOF_PACK_MANIFEST_PATH, &mut violations);
    let lanes = read_ci_ledger_document(PROOF_PACK_LANE_WHITELIST_PATH, &mut violations);

    if let (Some(manifest), Some(lanes)) = (&manifest, &lanes) {
        let lane_ids = proof_pack_lane_ids(lanes);
        violations.extend(proof_pack_violations(manifest, &lane_ids));
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "proof-packs.md",
            check: "check-proof-packs",
            why_it_matters: "policy/proof-packs.toml is the routing unit for proof-aware validation (docs/PROOF_ROUTING.md). While state is manifest-only, nothing routes on it yet, but the manifest must stay parseable, name only real repo commands and CI lanes, and keep the release-package pack pinned to full proof before any routing behavior consumes it.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep `policy/proof-packs.toml` on version 0.1 with `state = \"manifest-only\"` and `unknown_surface_policy = \"full-proof\"`.",
                "Give every pack a unique kebab-case id, at least one path, and at least one required command.",
                "Reference only known `cargo xtask` commands or known repo commands, and a `ci_lane` declared in `policy/ci-lane-whitelist.toml`.",
                "Keep `never_routed = true` on the release-package pack; release proof is never routed away.",
                "Keep the release-package pack covering every release surface (version files, changelog, release workflows) and requiring the full release proof (workspace tests, clippy, check-pr, package, publish dry-run, release-readiness).",
            ],
            rerun_command: "cargo xtask check-proof-packs",
            exception_template: None,
        },
        &violations,
    )
}

fn proof_pack_lane_ids(lanes: &CiLedgerDocument) -> BTreeSet<String> {
    let mut ignored = Vec::new();
    ci_tables(lanes, "lane")
        .into_iter()
        .filter_map(|table| {
            let value = table.values.get("id")?;
            ci_string_value(
                PROOF_PACK_LANE_WHITELIST_PATH,
                "table `[[lane]]`",
                "id",
                value,
                &mut ignored,
            )
        })
        .collect()
}

fn proof_pack_violations(manifest: &CiLedgerDocument, lane_ids: &BTreeSet<String>) -> Vec<String> {
    let path = PROOF_PACK_MANIFEST_PATH;
    let mut violations = Vec::new();

    ci_expect_top_string(path, manifest, "version", "0.1", &mut violations);
    ci_expect_top_string(path, manifest, "state", "manifest-only", &mut violations);
    ci_expect_top_string(
        path,
        manifest,
        "unknown_surface_policy",
        "full-proof",
        &mut violations,
    );

    if ci_tables(manifest, "pack").is_empty() {
        violations.push(format!("{path} has no [[pack]] entries"));
    }

    let packs = policy::proof_packs::parse_proof_packs(manifest, &mut violations);

    let mut seen_ids = BTreeSet::new();
    let mut release_pack_seen = false;
    for pack in &packs {
        if !seen_ids.insert(pack.id.clone()) {
            violations.push(format!(
                "{path}:{} duplicate proof pack id `{}`",
                pack.line, pack.id
            ));
        }

        for command in &pack.required_commands {
            proof_pack_command_violation(
                path,
                pack.line,
                &pack.id,
                "required_commands",
                command,
                &mut violations,
            );
        }
        for command in &pack.advisory_commands {
            proof_pack_command_violation(
                path,
                pack.line,
                &pack.id,
                "advisory_commands",
                command,
                &mut violations,
            );
        }

        if let Some(ci_lane) = &pack.ci_lane
            && !lane_ids.contains(ci_lane)
        {
            violations.push(format!(
                "{path}:{} proof pack `{}` references unknown ci_lane `{ci_lane}`\n  rule: ci_lane must be a lane id declared in {PROOF_PACK_LANE_WHITELIST_PATH}",
                pack.line, pack.id
            ));
        }

        if pack.id == PROOF_PACK_RELEASE_PACK_ID {
            release_pack_seen = true;
            if !pack.never_routed {
                violations.push(format!(
                    "{path}:{} proof pack `{PROOF_PACK_RELEASE_PACK_ID}` must set `never_routed = true`\n  rule: release proof is never routed away (docs/PROOF_ROUTING.md)",
                    pack.line
                ));
            }
            for surface in PROOF_PACK_RELEASE_REQUIRED_PATHS {
                if !pack.paths.iter().any(|covered| covered == surface) {
                    violations.push(format!(
                        "{path}:{} proof pack `{PROOF_PACK_RELEASE_PACK_ID}` must cover release surface `{surface}`\n  rule: every release surface (version files, changelog, release workflows) must trip release_proof_required (docs/PROOF_ROUTING.md, release-proof protection slice)",
                        pack.line
                    ));
                }
            }
            for required in PROOF_PACK_RELEASE_REQUIRED_COMMANDS {
                if !pack.required_commands.iter().any(|cmd| cmd == required) {
                    violations.push(format!(
                        "{path}:{} proof pack `{PROOF_PACK_RELEASE_PACK_ID}` must require `{required}`\n  rule: the release-package pack must name the full release proof (docs/PROOF_ROUTING.md, release-proof protection slice)",
                        pack.line
                    ));
                }
            }
        }
    }

    if !release_pack_seen {
        violations.push(format!(
            "{path} must define a `{PROOF_PACK_RELEASE_PACK_ID}` pack with `never_routed = true`"
        ));
    }

    violations
}

fn proof_pack_command_violation(
    path: &str,
    line: usize,
    pack_id: &str,
    field: &str,
    command: &str,
    violations: &mut Vec<String>,
) {
    if proof_pack_command_is_known(command) {
        return;
    }
    violations.push(format!(
        "{path}:{line} proof pack `{pack_id}` field `{field}` names unknown repo command `{command}`\n  rule: use a `cargo xtask <command>` from `cargo xtask help` or a known repo proof command"
    ));
}

fn proof_pack_command_is_known(command: &str) -> bool {
    let trimmed = command.trim();
    if let Some(rest) = trimmed.strip_prefix("cargo xtask ") {
        let root = known_command_root(rest.trim());
        return !root.is_empty()
            && known_commands()
                .into_iter()
                .any(|known| known_command_root(known) == root);
    }
    PROOF_PACK_KNOWN_REPO_COMMANDS.contains(&trimmed)
}

fn check_dependencies() -> Result<(), String> {
    policy::check_dependency_suppression_expiry(Path::new("deny.toml"))?;
    let allowlist = read_glob_allowlist("policy/dependency_allowlist.txt")?;
    let mut violations = Vec::new();

    for normalized in tracked_files()? {
        if !is_dependency_surface_candidate(&normalized) {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "dependency surface is not allowlisted: {normalized}\n  preferred: keep dependency managers scoped to approved Cargo, VS Code, or fixture surfaces"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "dependencies.md",
            check: "check-dependencies",
            why_it_matters: "Dependency manager surfaces change build and supply-chain behavior, so they need an explicit owner and reason.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Keep dependency files inside approved Cargo, VS Code, or fixture surfaces.",
                "Explain new dependency surfaces in the PR.",
                "Add an allowlist entry only when the surface is intentional.",
            ],
            rerun_command: "cargo xtask check-dependencies",
            exception_template: Some(
                "policy/dependency_allowlist.txt entry:\nglob|kind|owner|reason",
            ),
        },
        &violations,
    )
}

fn check_supply_chain() -> Result<(), String> {
    ensure_reports_dir()?;

    let args = ["deny", "check", "advisories", "licenses", "bans", "sources"];
    eprintln!("$ cargo {}", args.join(" "));
    let output = capture_output("cargo", &args, "cargo deny")?;

    let status = if output.status.success() {
        "pass"
    } else {
        "fail"
    };
    let stdout = redact_current_dir(&output.stdout);
    let stderr = redact_current_dir(&output.stderr);
    let mut body = format!(
        "# ripr supply-chain report\n\nStatus: {status}\n\nCommand:\n\n```bash\ncargo deny check advisories licenses bans sources\n```\n\n"
    );
    body.push_str("Policy:\n\n");
    body.push_str(
        "- advisories, licenses, bans, and source registries are checked by `cargo-deny`.\n",
    );
    body.push_str(
        "- duplicate dependency findings are warnings in `deny.toml` during baseline setup.\n\n",
    );
    body.push_str("Output:\n\n```text\n");
    if stdout.is_empty() && stderr.is_empty() {
        body.push_str("<no output>\n");
    } else {
        body.push_str(&stdout);
        if !stdout.ends_with('\n') && !stdout.is_empty() {
            body.push('\n');
        }
        body.push_str(&stderr);
        if !stderr.ends_with('\n') && !stderr.is_empty() {
            body.push('\n');
        }
    }
    body.push_str("```\n");
    write_report("supply-chain.md", &body)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo deny check advisories licenses bans sources failed with {}",
            output.status
        ))
    }
}

fn redact_current_dir(text: &str) -> String {
    let Ok(current_dir) = std::env::current_dir() else {
        return text.to_string();
    };
    let current_dir = current_dir.display().to_string();
    let slash_dir = current_dir.replace('\\', "/");
    text.replace(&current_dir, ".").replace(&slash_dir, ".")
}

fn check_process_policy_impl() -> Result<(), String> {
    check_count_policy(
        "process policy",
        "policy/process_allowlist.txt",
        &process_policy_patterns(),
        is_process_policy_candidate,
    )
}

fn check_network_policy_impl() -> Result<(), String> {
    check_count_policy(
        "network policy",
        "policy/network_allowlist.txt",
        &network_policy_patterns(),
        is_network_policy_candidate,
    )
}

fn check_count_policy(
    label: &str,
    allowlist_path: &str,
    patterns: &[String],
    is_candidate: fn(&str) -> bool,
) -> Result<(), String> {
    let allowlist = read_count_policy_allowlist(allowlist_path)?;
    let mut counts = BTreeMap::<(String, String), usize>::new();

    for normalized in tracked_files()? {
        if !is_candidate(&normalized) {
            continue;
        }
        let text = read_text_lossy(Path::new(&normalized))?;
        for pattern in patterns {
            let count = text.matches(pattern).count();
            if count > 0 {
                counts.insert((normalized.clone(), pattern.clone()), count);
            }
        }
    }

    let mut violations = Vec::new();
    for ((path, pattern), count) in &counts {
        let allowed = allowlist
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if *count > allowed {
            violations.push(format!(
                "{path} contains `{pattern}` {count} time(s), allowed {allowed}\n  to allowlist, add to {allowlist_path}:\n  {path}|{pattern}|{count}|owner|reason"
            ));
        } else if *count < allowed {
            // Stale bound: the allowlist grants more budget than the code
            // uses. This is the reverse-direction check that prevents the
            // bound from silently rotting (#2413). A stale bound lets new
            // occurrences slip through without an allowlist update, so flag
            // it so the bound is tightened to the actual count.
            violations.push(format!(
                "{path} allowlist entry for `{pattern}` is stale: max_count={allowed} but actual={count}; tighten to {count}"
            ));
        }
    }

    // Also flag allowlist entries for paths that no longer contain the pattern
    // at all (count dropped to zero, or the file was removed). These are fully
    // orphaned entries whose bound provides invisible slack.
    for ((path, pattern), allowed) in &allowlist {
        if !counts.contains_key(&(path.clone(), pattern.clone())) && *allowed > 0 {
            violations.push(format!(
                "{path} allowlist entry for `{pattern}` is orphaned: max_count={allowed} but pattern not found in any tracked file; remove the entry"
            ));
        }
    }

    let report_file = format!("{}.md", label.replace(' ', "-"));
    let why = format!(
        "{label} entries are explicit because hidden side effects make automation and analyzer behavior harder to review."
    );
    let template = format!("{allowlist_path} entry:\npath|pattern|max_count|owner|reason");
    let check = format!("check-{}", label.replace(' ', "-"));
    let rerun_command = format!("cargo xtask {check}");
    finish_policy_report(
        PolicyReportSpec {
            report_file: &report_file,
            check: &check,
            why_it_matters: &why,
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move the behavior to the approved adapter or automation surface.",
                "Reduce the process or network usage when it is not required.",
                "Add an allowlist entry only when the behavior is intentional and owned.",
            ],
            rerun_command: &rerun_command,
            exception_template: Some(&template),
        },
        &violations,
    )
}

fn sort_allowlist_files() -> Result<Vec<String>, String> {
    let mut changed = Vec::new();
    for root in [Path::new(".ripr"), Path::new("policy")] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("txt") {
                continue;
            }
            let original = read_text_lossy(&path)?;
            let sorted = sorted_allowlist_content(&original);
            if sorted != original {
                fs::write(&path, sorted).map_err(|err| {
                    format!(
                        "failed to write sorted allowlist {}: {err}\nrerun with `cargo xtask shape` after fixing file permissions",
                        path.display()
                    )
                })?;
                changed.push(normalize_path(&path));
            }
        }
    }
    changed.sort();
    Ok(changed)
}

fn sorted_allowlist_content(text: &str) -> String {
    let mut prefix = Vec::new();
    let mut entries = Vec::new();
    let mut saw_entry = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if !saw_entry && (trimmed.is_empty() || trimmed.starts_with('#')) {
            prefix.push(line.trim_end().to_string());
            continue;
        }
        saw_entry = true;
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            entries.push(trimmed.to_string());
        }
    }

    entries.sort();
    let mut output = String::new();
    if !prefix.is_empty() {
        output.push_str(&prefix.join("\n"));
        output.push('\n');
    }
    if !entries.is_empty() {
        if !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&entries.join("\n"));
        output.push('\n');
    }
    if output.is_empty() {
        output.push('\n');
    }
    output
}

fn shape_report_body(sorted: &[String]) -> String {
    let mut body = String::from(
        "# ripr shape report\n\nStatus: pass\n\nActions:\n\n- Ran `cargo fmt`.\n- Ensured `target/ripr/reports` exists.\n",
    );
    if sorted.is_empty() {
        body.push_str("- Allowlist files were already sorted.\n");
    } else {
        body.push_str("- Sorted allowlist files:\n");
        for path in sorted {
            body.push_str(&format!("  - `{path}`\n"));
        }
    }
    body.push_str("\nNext commands:\n\n```bash\ncargo xtask ci-fast\n```\n");
    body
}

fn ensure_reports_dir() -> Result<(), String> {
    fs::create_dir_all(reports_dir()).map_err(|err| {
        format!(
            "failed to create {}: {err}\nrerun with `cargo xtask shape` after fixing directory permissions",
            reports_dir().display()
        )
    })
}

fn ensure_receipts_dir() -> Result<(), String> {
    fs::create_dir_all(receipts_dir()).map_err(|err| {
        format!(
            "failed to create {}: {err}\nrerun with `cargo xtask receipts` after fixing directory permissions",
            receipts_dir().display()
        )
    })
}

pub(crate) fn write_report(file_name: &str, body: &str) -> Result<(), String> {
    ensure_reports_dir()?;
    let path = reports_dir().join(file_name);
    fs::write(&path, body).map_err(|err| {
        format!(
            "failed to write {}: {err}\nrerun with `cargo xtask shape` after fixing file permissions",
            path.display()
        )
    })
}

fn write_receipt(file_name: &str, body: &str) -> Result<(), String> {
    ensure_receipts_dir()?;
    let path = receipts_dir().join(file_name);
    fs::write(&path, body).map_err(|err| {
        format!(
            "failed to write {}: {err}\nrerun with `cargo xtask receipts` after fixing file permissions",
            path.display()
        )
    })
}

fn reports_dir() -> PathBuf {
    Path::new("target").join("ripr").join("reports")
}

fn receipts_dir() -> PathBuf {
    Path::new("target").join("ripr").join("receipts")
}

fn report_index_entries() -> Result<Vec<ReportIndexEntry>, String> {
    file_index_entries(&reports_dir(), &["index.md", "index.json"])
}

fn receipt_index_entries() -> Result<Vec<ReportIndexEntry>, String> {
    file_index_entries(&receipts_dir(), &[])
}

fn report_index_lane1_readiness_packets(
    reports: &[ReportIndexEntry],
) -> Vec<ReportIndexRepoOpsPacket> {
    lane1_readiness_packet_specs()
        .iter()
        .map(|spec| {
            let artifacts = spec
                .artifacts
                .iter()
                .map(|path| report_index_report_artifact(path, reports))
                .collect::<Vec<_>>();
            ReportIndexRepoOpsPacket {
                id: spec.id,
                label: spec.label,
                status: report_index_lane1_readiness_status(&artifacts),
                command: spec.command,
                description: spec.description,
                artifacts,
            }
        })
        .collect()
}

fn lane1_readiness_packet_specs() -> &'static [RepoOpsPacketSpec] {
    &[
        RepoOpsPacketSpec {
            id: "evidence_health",
            label: "Evidence health",
            command: "cargo xtask evidence-health",
            description: "Checks Lane 1 evidence-health generation and bounded limited-artifact diagnostics.",
            artifacts: &[
                "target/ripr/reports/evidence-health.json",
                "target/ripr/reports/evidence-health.md",
            ],
        },
        RepoOpsPacketSpec {
            id: "lane1_evidence_audit",
            label: "Lane 1 evidence audit",
            command: "cargo xtask lane1-evidence-audit",
            description: "Produces raw-to-canonical/actionability counts and actionable-gap packet inputs.",
            artifacts: &[
                "target/ripr/reports/lane1-evidence-audit.json",
                "target/ripr/reports/lane1-evidence-audit.md",
                "target/ripr/reports/actionable-gaps.json",
                "target/ripr/reports/actionable-gaps.md",
            ],
        },
        RepoOpsPacketSpec {
            id: "evidence_quality_scorecard",
            label: "Evidence quality scorecard",
            command: "cargo xtask evidence-quality-scorecard",
            description: "Summarizes Lane 1 counts, unknowns, repair-route coverage, and verify-command coverage.",
            artifacts: &[
                "target/ripr/reports/evidence-quality-scorecard.json",
                "target/ripr/reports/evidence-quality-scorecard.md",
            ],
        },
        RepoOpsPacketSpec {
            id: "evidence_quality_trend",
            label: "Evidence quality trend",
            command: "cargo xtask evidence-quality-trend",
            description: "Compares scorecard movement while keeping limited current inputs unknown.",
            artifacts: &[
                "target/ripr/reports/evidence-quality-trend.json",
                "target/ripr/reports/evidence-quality-trend.md",
            ],
        },
        RepoOpsPacketSpec {
            id: "badge_basis",
            label: "Badge basis",
            command: "cargo xtask badge-basis",
            description: "Audits whether canonical actionable gaps are ready to support badge semantics.",
            artifacts: &[
                "target/ripr/reports/badge-basis.json",
                "target/ripr/reports/badge-basis.md",
            ],
        },
    ]
}

fn report_index_report_artifact(
    path: &str,
    reports: &[ReportIndexEntry],
) -> ReportIndexRepoOpsArtifact {
    let source = path
        .strip_prefix("target/ripr/reports/")
        .and_then(|file| reports.iter().find(|entry| entry.file == file));
    let status = source
        .map(|entry| entry.status.clone())
        .unwrap_or_else(|| "missing".to_string());
    ReportIndexRepoOpsArtifact {
        path: path.to_string(),
        available: source.is_some(),
        status,
    }
}

fn report_index_lane1_readiness_status(artifacts: &[ReportIndexRepoOpsArtifact]) -> String {
    if artifacts.iter().all(|artifact| !artifact.available) {
        return "missing".to_string();
    }
    if artifacts.iter().any(|artifact| artifact.status == "fail") {
        return "fail".to_string();
    }
    if artifacts
        .iter()
        .any(|artifact| !artifact.available || is_warning_report_status(&artifact.status))
    {
        return "warn".to_string();
    }
    "present".to_string()
}

fn report_index_lane1_overall_status(packets: &[ReportIndexRepoOpsPacket]) -> String {
    if packets.iter().any(|packet| packet.status == "fail") {
        return "fail".to_string();
    }
    if packets.iter().any(|packet| packet.status != "present") {
        return "warn".to_string();
    }
    "present".to_string()
}

fn report_index_missing_artifact_count(packets: &[ReportIndexRepoOpsPacket]) -> usize {
    packets
        .iter()
        .flat_map(|packet| packet.artifacts.iter())
        .filter(|artifact| !artifact.available)
        .count()
}

fn report_index_warning_artifact_count(packets: &[ReportIndexRepoOpsPacket]) -> usize {
    packets
        .iter()
        .flat_map(|packet| packet.artifacts.iter())
        .filter(|artifact| artifact.available && is_warning_report_status(&artifact.status))
        .count()
}

fn report_index_failing_artifact_count(packets: &[ReportIndexRepoOpsPacket]) -> usize {
    packets
        .iter()
        .flat_map(|packet| packet.artifacts.iter())
        .filter(|artifact| artifact.status == "fail")
        .count()
}

fn is_warning_report_status(status: &str) -> bool {
    matches!(
        status,
        "warn" | "timeout" | "incomplete" | "unreadable" | "stale" | "unknown"
    )
}

fn report_index_repo_ops_packets(
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> Vec<ReportIndexRepoOpsPacket> {
    repo_ops_packet_specs()
        .iter()
        .map(|spec| {
            let artifacts = spec
                .artifacts
                .iter()
                .map(|path| report_index_repo_ops_artifact(path, reports, receipts))
                .collect::<Vec<_>>();
            ReportIndexRepoOpsPacket {
                id: spec.id,
                label: spec.label,
                status: report_index_repo_ops_status(&artifacts),
                command: spec.command,
                description: spec.description,
                artifacts,
            }
        })
        .collect()
}

fn repo_ops_packet_specs() -> &'static [RepoOpsPacketSpec] {
    &[
        RepoOpsPacketSpec {
            id: "command_mutability_catalog",
            label: "Command mutability catalog",
            command: "cargo xtask commands",
            description: "Classifies xtask commands by mutability and judgment requirements.",
            artifacts: &[
                "target/ripr/reports/commands.md",
                "target/ripr/reports/commands.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "command_catalog_check",
            label: "Command catalog check",
            command: "cargo xtask check-command-catalog",
            description: "Verifies every xtask command has a current mutability catalog entry.",
            artifacts: &["target/ripr/reports/command-catalog.md"],
        },
        RepoOpsPacketSpec {
            id: "pr_ready",
            label: "PR ready cockpit",
            command: "cargo xtask pr-ready",
            description: "Composes local repo-ops checks into one advisory PR readiness packet.",
            artifacts: &[
                "target/ripr/reports/pr-ready.md",
                "target/ripr/reports/pr-ready.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "repo_cockpit",
            label: "Repo cockpit",
            command: "cargo xtask cockpit",
            description: "Composes repo-level operating packets into one advisory maintainer front panel.",
            artifacts: &[
                "target/ripr/reports/cockpit.md",
                "target/ripr/reports/cockpit.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "worktree_doctor",
            label: "Worktree doctor",
            command: "cargo xtask worktree doctor",
            description: "Reports local branch, generated-residue, and worktree hygiene.",
            artifacts: &["target/ripr/reports/worktree-doctor.md"],
        },
        RepoOpsPacketSpec {
            id: "pr_triage",
            label: "Open PR triage",
            command: "cargo xtask pr-triage-report",
            description: "Summarizes stale, duplicate, behind, sensitive, and generated-artifact PRs.",
            artifacts: &[
                "target/ripr/reports/pr-triage.md",
                "target/ripr/reports/pr-triage.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "gh_pr_status",
            label: "PR merge readiness",
            command: "cargo xtask gh-pr-status --pr <number>",
            description: "Summarizes one PR's merge state, checks, reviews, and safe next action.",
            artifacts: &[
                "target/ripr/reports/gh-pr-status.md",
                "target/ripr/reports/gh-pr-status.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "generated_clean",
            label: "Generated-clean guard",
            command: "cargo xtask check-generated-clean",
            description: "Rejects generated residue and badge endpoint diffs in ordinary PRs.",
            artifacts: &["target/ripr/reports/generated-clean.md"],
        },
        RepoOpsPacketSpec {
            id: "badge_diff_policy",
            label: "Badge diff policy",
            command: "cargo xtask check-badge-diff-policy",
            description: "Confirms public badge endpoint JSON is owned by badge-refresh automation.",
            artifacts: &["target/ripr/reports/badge-diff-policy.md"],
        },
        RepoOpsPacketSpec {
            id: "critic",
            label: "Critic report",
            command: "cargo xtask critic",
            description: "Advisory adversarial review packet for missing evidence and risky drift.",
            artifacts: &[
                "target/ripr/reports/critic.md",
                "target/ripr/reports/critic.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "receipts",
            label: "Gate receipts",
            command: "cargo xtask receipts check",
            description: "Machine-readable local gate evidence and receipt validation summary.",
            artifacts: &[
                "target/ripr/reports/receipts.md",
                "target/ripr/receipts/check-pr.json",
            ],
        },
        RepoOpsPacketSpec {
            id: "suggested_fixes",
            label: "Suggested fixes",
            command: "cargo xtask suggested-fixes",
            description: "Deterministic repair patch for safe repo-hygiene-only fixes.",
            artifacts: &[
                "target/ripr/reports/suggested-fixes.md",
                "target/ripr/reports/suggested-fixes.patch",
            ],
        },
        RepoOpsPacketSpec {
            id: "check_pr",
            label: "Review-ready gate",
            command: "cargo xtask check-pr",
            description: "Local review-ready gate packet and matching check-pr receipt.",
            artifacts: &[
                "target/ripr/reports/check-pr.md",
                "target/ripr/receipts/check-pr.json",
            ],
        },
    ]
}

fn report_index_repo_ops_artifact(
    path: &str,
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> ReportIndexRepoOpsArtifact {
    let source = if let Some(file) = path.strip_prefix("target/ripr/reports/") {
        reports.iter().find(|entry| entry.file == file)
    } else if let Some(file) = path.strip_prefix("target/ripr/receipts/") {
        receipts.iter().find(|entry| entry.file == file)
    } else {
        None
    };
    let status = source
        .map(|entry| entry.status.clone())
        .unwrap_or_else(|| "missing".to_string());
    ReportIndexRepoOpsArtifact {
        path: path.to_string(),
        available: source.is_some(),
        status,
    }
}

fn report_index_repo_ops_status(artifacts: &[ReportIndexRepoOpsArtifact]) -> String {
    if artifacts.iter().all(|artifact| !artifact.available) {
        return "missing".to_string();
    }
    if artifacts.iter().any(|artifact| artifact.status == "fail") {
        return "fail".to_string();
    }
    if artifacts.iter().any(|artifact| artifact.status == "warn") {
        return "warn".to_string();
    }
    if artifacts
        .iter()
        .any(|artifact| artifact.status == "actionable")
    {
        return "actionable".to_string();
    }
    if artifacts.iter().any(|artifact| !artifact.available) {
        return "incomplete".to_string();
    }
    if artifacts.iter().all(|artifact| artifact.status == "pass") {
        return "pass".to_string();
    }
    "present".to_string()
}

fn file_index_entries(dir: &Path, exclude_names: &[&str]) -> Result<Vec<ReportIndexEntry>, String> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return Ok(entries);
    }
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", normalize_path(dir)))?
    {
        let entry = entry
            .map_err(|err| format!("failed to read entry under {}: {err}", normalize_path(dir)))?;
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "failed to read file type for {}: {err}",
                normalize_path(&entry.path())
            )
        })?;
        if !file_type.is_file() {
            continue;
        }
        let file = entry.file_name().to_string_lossy().to_string();
        if exclude_names.iter().any(|name| *name == file) {
            continue;
        }
        let path = entry.path();
        entries.push(ReportIndexEntry {
            file,
            path: normalize_path(&path),
            status: report_entry_status(&path),
        });
    }
    entries.sort_by(|left, right| left.file.cmp(&right.file));
    Ok(entries)
}

fn report_entry_status(path: &Path) -> String {
    if normalize_path(path).ends_with("target/ripr/reports/metrics.json") {
        return "present".to_string();
    }
    match read_text_lossy(path) {
        Ok(text) => {
            let status = report_status_from_text(&text).unwrap_or_else(|| "present".to_string());
            if status != "fail" && report_text_has_run_limitations(&text) {
                "warn".to_string()
            } else {
                status
            }
        }
        Err(_) => "unreadable".to_string(),
    }
}

fn report_text_has_run_limitations(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .is_some_and(|value| report_has_run_limitations(&value))
}

fn report_status_from_text(text: &str) -> Option<String> {
    for line in text.lines().take(24) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Status:") {
            return Some(normalize_report_status(rest));
        }
        if let Some((_, rest)) = trimmed.split_once("\"status\"")
            && let Some((_, value)) = rest.split_once(':')
        {
            return Some(normalize_report_status(value));
        }
    }
    None
}

fn normalize_report_status(value: &str) -> String {
    let cleaned = value.trim().trim_matches(|ch| {
        ch == '"'
            || ch == '\''
            || ch == '`'
            || ch == ','
            || ch == '{'
            || ch == '}'
            || ch == '['
            || ch == ']'
    });
    let lower = cleaned.to_ascii_lowercase();
    let mut token = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            token.push(ch);
        } else {
            break;
        }
    }
    if token.is_empty() {
        "present".to_string()
    } else {
        token
    }
}

fn report_index_missing_expected(
    reports: &[ReportIndexEntry],
    changes: &[ChangedPath],
) -> Vec<String> {
    let existing = reports
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::<String>::new();
    expected.insert("pr-summary.md".to_string());
    expected.insert("check-pr.md".to_string());

    if changes.iter().any(|change| is_docs_path(&change.path)) {
        expected.insert("doc-index.md".to_string());
        expected.insert("markdown-links.md".to_string());
    }
    if changes
        .iter()
        .any(|change| change.path == "README.md" || change.path == "docs/CAPABILITY_MATRIX.md")
    {
        expected.insert("readme-state.md".to_string());
    }
    if changes.iter().any(|change| is_analysis_path(&change.path)) {
        expected.insert("pr-shape.md".to_string());
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("capabilities.md".to_string());
    }
    if changes
        .iter()
        .any(|change| is_output_surface_path(&change.path))
    {
        expected.insert("output-contracts.md".to_string());
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("golden-drift.md".to_string());
    }
    if changes.iter().any(|change| is_fixture_path(&change.path)) {
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("golden-drift.md".to_string());
    }
    if changes.iter().any(|change| is_metrics_path(&change.path)) {
        expected.insert("capabilities.md".to_string());
        expected.insert("metrics.md".to_string());
    }

    expected
        .into_iter()
        .filter(|file| !existing.contains(file))
        .map(|file| format!("target/ripr/reports/{file}"))
        .collect()
}

fn is_docs_path(path: &str) -> bool {
    path == "README.md"
        || path == "AGENTS.md"
        || path == "CONTRIBUTING.md"
        || path == "CHANGELOG.md"
        || path.starts_with("docs/")
        || is_plan_path(path)
}

fn is_plan_path(path: &str) -> bool {
    path.starts_with("plans/")
}

fn is_analysis_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/analysis/")
}

fn is_output_surface_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/output/")
        || path.starts_with("crates/ripr/src/domain/")
        || path == "crates/ripr/src/lsp.rs"
        || path == "docs/OUTPUT_SCHEMA.md"
        || path == "policy/output_contracts.txt"
}

fn is_metrics_path(path: &str) -> bool {
    path.starts_with("metrics/") || path == "docs/CAPABILITY_MATRIX.md"
}

fn report_index_status(
    reports: &[ReportIndexEntry],
    missing: &[String],
    campaign_issues: &[String],
) -> &'static str {
    if reports
        .iter()
        .any(|entry| entry.status == "fail" && !report_index_is_lane1_readiness_file(&entry.file))
    {
        return "fail";
    }
    let lane1_packets = report_index_lane1_readiness_packets(reports);
    if !missing.is_empty()
        || !campaign_issues.is_empty()
        || lane1_packets
            .iter()
            .any(|packet| packet.status != "present")
        || reports.iter().any(|entry| entry.status == "warn")
    {
        "warn"
    } else {
        "pass"
    }
}

fn report_index_is_lane1_readiness_file(file: &str) -> bool {
    lane1_readiness_packet_specs()
        .iter()
        .flat_map(|spec| spec.artifacts.iter())
        .filter_map(|path| path.strip_prefix("target/ripr/reports/"))
        .any(|expected| expected == file)
}

fn report_index_next_commands(
    missing: &[String],
    lane1_packets: &[ReportIndexRepoOpsPacket],
) -> Vec<String> {
    let mut commands = BTreeSet::<String>::new();
    if missing
        .iter()
        .any(|path| path.ends_with("/pr-summary.md") || path.ends_with("\\pr-summary.md"))
    {
        commands.insert("cargo xtask pr-summary".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/fixtures.md") || path.ends_with("\\fixtures.md"))
    {
        commands.insert("cargo xtask fixtures".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/goldens.md") || path.ends_with("\\goldens.md"))
    {
        commands.insert("cargo xtask goldens check".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/golden-drift.md") || path.ends_with("\\golden-drift.md"))
    {
        commands.insert("cargo xtask golden-drift".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/goals-next.md") || path.ends_with("\\goals-next.md"))
    {
        commands.insert("cargo xtask goals next".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/metrics.md") || path.ends_with("\\metrics.md"))
    {
        commands.insert("cargo xtask metrics".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/capabilities.md") || path.ends_with("\\capabilities.md"))
    {
        commands.insert("cargo xtask check-capabilities".to_string());
    }
    for packet in lane1_packets {
        if packet.status != "present" {
            commands.insert(packet.command.to_string());
        }
    }
    commands.insert("cargo xtask check-pr".to_string());
    commands.insert("cargo xtask reports index".to_string());
    commands.into_iter().collect()
}

fn collect_pr_changes() -> Result<Vec<ChangedPath>, String> {
    let mut changes = BTreeMap::<String, BTreeSet<String>>::new();

    add_name_status_output(
        &mut changes,
        &run_output_optional("git", &["diff", "--name-status", "origin/main...HEAD"])?,
    );
    add_name_status_output(
        &mut changes,
        &run_output("git", &["diff", "--name-status"])?,
    );
    add_name_status_output(
        &mut changes,
        &run_output("git", &["diff", "--cached", "--name-status"])?,
    );
    add_short_status_output(&mut changes, &run_output("git", &["status", "--short"])?);

    Ok(changes
        .into_iter()
        .map(|(path, statuses)| ChangedPath { path, statuses })
        .collect())
}

fn collect_worktree_status_changes() -> Result<Vec<ChangedPath>, String> {
    let mut changes = BTreeMap::<String, BTreeSet<String>>::new();
    add_short_status_output(&mut changes, &run_output("git", &["status", "--short"])?);
    Ok(changes
        .into_iter()
        .map(|(path, statuses)| ChangedPath { path, statuses })
        .collect())
}

fn add_name_status_output(changes: &mut BTreeMap<String, BTreeSet<String>>, output: &str) {
    for line in output.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        let status = parts[0].trim();
        let Some(path) = parts.last() else {
            continue;
        };
        add_changed_path(changes, path, status);
    }
}

fn add_short_status_output(changes: &mut BTreeMap<String, BTreeSet<String>>, output: &str) {
    for line in output.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = line[..2].trim();
        let mut path = line[3..].trim();
        if let Some((_, new_path)) = path.split_once(" -> ") {
            path = new_path.trim();
        }
        if status.is_empty() {
            continue;
        }
        add_changed_path(changes, path, status);
    }
}

fn add_changed_path(changes: &mut BTreeMap<String, BTreeSet<String>>, path: &str, status: &str) {
    let normalized = normalize_slashes(path.trim().trim_matches('"'));
    if normalized.is_empty() {
        return;
    }
    changes
        .entry(normalized)
        .or_default()
        .insert(status.to_string());
}

fn pr_summary_body(changes: &[ChangedPath]) -> String {
    let mut body = String::from("# ripr PR readiness summary\n\n");
    body.push_str(&pr_actionable_delta_front_panel(changes));
    body.push('\n');

    body.push_str("## Scope\n\n");
    body.push_str("Production delta:\n");
    write_path_list(&mut body, &paths_matching(changes, is_production_path));
    body.push_str("\nEvidence/support delta:\n");
    write_path_list(&mut body, &paths_matching(changes, is_evidence_path));

    body.push_str("\n## Detected Surfaces\n\n");
    for (label, paths) in detected_surface_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Public Contracts Touched\n\n");
    for (label, paths) in public_contract_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Policy Exceptions\n\n");
    for (label, paths) in policy_exception_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Suggested Reviewer Focus\n\n");
    let focus = reviewer_focus(changes);
    if focus.is_empty() {
        body.push_str("- No changed files detected.\n");
    } else {
        for (index, path) in focus.iter().enumerate() {
            body.push_str(&format!("{}. `{path}`\n", index + 1));
        }
    }

    body.push_str("\n## Commands\n\n");
    body.push_str("- `cargo xtask fix-pr`\n");
    body.push_str("- `cargo xtask check-pr`\n");
    body.push_str("- `cargo xtask pr-summary`\n");

    body.push_str("\n## Receipts\n\n");
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        let status = if path.exists() {
            report_entry_status(&path)
        } else {
            "missing".to_string()
        };
        body.push_str(&format!("- `{}`: {status}\n", normalize_path(&path)));
    }
    body
}

#[derive(Clone, Debug, PartialEq)]
enum PrActionableInput {
    Missing,
    Read(Value),
    Malformed(String),
}

impl PrActionableInput {
    fn status(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Read(_) => "read",
            Self::Malformed(_) => "malformed",
        }
    }

    fn value(&self) -> Option<&Value> {
        match self {
            Self::Read(value) => Some(value),
            Self::Missing | Self::Malformed(_) => None,
        }
    }

    fn note(&self) -> Option<&str> {
        match self {
            Self::Malformed(err) => Some(err.as_str()),
            Self::Missing | Self::Read(_) => None,
        }
    }
}

fn pr_actionable_delta_front_panel(changes: &[ChangedPath]) -> String {
    let actionable =
        read_pr_actionable_input(Path::new("target/ripr/reports/actionable-gaps.json"));
    let outcomes = read_pr_actionable_input(Path::new(
        "target/ripr/reports/actionable-gap-outcomes.json",
    ));
    let front_panel =
        read_pr_actionable_input(Path::new("target/ripr/reports/pr-review-front-panel.json"));
    pr_actionable_delta_front_panel_from_inputs(changes, &actionable, &outcomes, &front_panel)
}

fn read_pr_actionable_input(path: &Path) -> PrActionableInput {
    if !path.exists() {
        return PrActionableInput::Missing;
    }
    match read_json_value(path) {
        Ok(value) => PrActionableInput::Read(value),
        Err(err) => PrActionableInput::Malformed(err),
    }
}

fn pr_actionable_delta_front_panel_from_inputs(
    changes: &[ChangedPath],
    actionable: &PrActionableInput,
    outcomes: &PrActionableInput,
    front_panel: &PrActionableInput,
) -> String {
    let mut body = String::from("## Actionable Repair Front Panel\n\n");
    body.push_str("Status: advisory static projection\n\n");
    body.push_str("Inputs:\n");
    write_pr_actionable_input_status(
        &mut body,
        "actionable gaps",
        "target/ripr/reports/actionable-gaps.json",
        actionable,
    );
    write_pr_actionable_input_status(
        &mut body,
        "actionable outcomes",
        "target/ripr/reports/actionable-gap-outcomes.json",
        outcomes,
    );
    write_pr_actionable_input_status(
        &mut body,
        "PR review front panel",
        "target/ripr/reports/pr-review-front-panel.json",
        front_panel,
    );

    body.push_str("\nActionable delta:\n");
    if let Some(report) = actionable.value() {
        let packets = pr_actionable_packets(report);
        let changed_paths = pr_changed_path_set(changes);
        let pr_local_packets = packets
            .iter()
            .filter(|packet| pr_packet_touches_changed_path(packet, &changed_paths))
            .count();
        let static_limited_packets = packets
            .iter()
            .filter(|packet| pr_packet_static_limitation_count(packet) > 0)
            .count();
        body.push_str(&format!(
            "- repo actionable gaps: `{}`\n",
            pr_usize_path(report, &["summary", "actionable_gaps"])
                .map_or_else(|| "unknown".to_string(), |value| value.to_string())
        ));
        body.push_str(&format!(
            "- PR-local actionable gaps: `{pr_local_packets}`\n"
        ));
        body.push_str(&format!(
            "- repair packets emitted: `{}`\n",
            pr_usize_path(report, &["summary", "packets_emitted"])
                .map_or_else(|| packets.len().to_string(), |value| value.to_string())
        ));
        body.push_str(&format!(
            "- public repair packets: `{}`\n",
            pr_usize_path(report, &["summary", "public_projection_eligible_packets"])
                .map_or_else(|| "unknown".to_string(), |value| value.to_string())
        ));
        body.push_str(&format!(
            "- blocked/static-limited gaps: `{}` static limitation(s), `{}` static-limited packet(s)\n",
            pr_usize_path(report, &["summary", "static_limitations"])
                .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            static_limited_packets
        ));
    } else {
        body.push_str("- repo actionable gaps: `unknown` (missing actionable-gaps artifact)\n");
        body.push_str("- PR-local actionable gaps: `unknown` (missing actionable-gaps artifact)\n");
        body.push_str(
            "- blocked/static-limited gaps: `unknown` (missing actionable-gaps artifact)\n",
        );
    }

    write_pr_actionable_delta_movement(&mut body, outcomes, front_panel);
    write_pr_actionable_top_packet(&mut body, changes, actionable);
    write_pr_python_top_repair_card(&mut body, changes, actionable);

    body.push_str("\nBoundary:\n");
    body.push_str("- Static RIPR evidence only.\n");
    body.push_str("- Does not run mutation testing or claim runtime adequacy.\n");
    body.push_str("- Does not edit source, generate tests, publish comments, or decide gates.\n");
    body
}

fn write_pr_actionable_input_status(
    body: &mut String,
    label: &str,
    path: &str,
    input: &PrActionableInput,
) {
    body.push_str(&format!("- {label}: `{}` (`{path}`)", input.status()));
    if let Some(note) = input.note() {
        body.push_str(&format!(" - {}", md_escape(note)));
    }
    body.push('\n');
}

fn write_pr_actionable_delta_movement(
    body: &mut String,
    outcomes: &PrActionableInput,
    front_panel: &PrActionableInput,
) {
    let new_actionable = front_panel
        .value()
        .and_then(|value| {
            pr_usize_path(value, &["summary", "new_policy_eligible"])
                .or_else(|| pr_usize_path(value, &["debt_delta", "new_policy_eligible"]))
        })
        .map_or_else(|| "unavailable".to_string(), |value| value.to_string());
    let resolved = outcomes
        .value()
        .and_then(|value| pr_usize_path(value, &["summary", "resolved"]))
        .map_or_else(|| "unavailable".to_string(), |value| value.to_string());
    let receipt_state = outcomes.value().map_or_else(
        || {
            "unavailable; run `cargo xtask actionable-gap-outcomes` after a repair attempt"
                .to_string()
        },
        pr_receipt_state_summary,
    );

    body.push_str(&format!("- new actionable gaps: `{new_actionable}`\n"));
    body.push_str(&format!("- resolved actionable gaps: `{resolved}`\n"));
    body.push_str(&format!("- receipt state: {receipt_state}\n"));
}

fn pr_receipt_state_summary(outcomes: &Value) -> String {
    let receipts_present = pr_usize_path(outcomes, &["summary", "receipts_present"]).unwrap_or(0);
    let missing_after_input =
        pr_usize_path(outcomes, &["summary", "receipts_missing_after_input"]).unwrap_or(0);
    let orphaned = pr_usize_path(outcomes, &["summary", "orphaned_receipts"]).unwrap_or(0);
    let improved = pr_usize_path(outcomes, &["summary", "evidence_improved"]).unwrap_or(0);
    let unchanged = pr_usize_path(outcomes, &["summary", "evidence_unchanged"]).unwrap_or(0);
    format!(
        "`receipts_present={receipts_present}`, `missing_after_input={missing_after_input}`, \
         `orphaned={orphaned}`, `improved={improved}`, `unchanged={unchanged}`"
    )
}

fn write_pr_actionable_top_packet(
    body: &mut String,
    changes: &[ChangedPath],
    actionable: &PrActionableInput,
) {
    body.push_str("\nTop next repair packet:\n");
    let Some(report) = actionable.value() else {
        body.push_str(
            "- unavailable: run `cargo xtask lane1-evidence-audit` to refresh actionable-gaps.\n",
        );
        return;
    };
    let packets = pr_actionable_packets(report);
    let changed_paths = pr_changed_path_set(changes);
    let selected = packets
        .iter()
        .copied()
        .find(|packet| pr_packet_touches_changed_path(packet, &changed_paths))
        .or_else(|| packets.first().copied());
    let Some(packet) = selected else {
        body.push_str("- unavailable: no repair packet was emitted.\n");
        return;
    };

    body.push_str(&format!(
        "- canonical gap: `{}`\n",
        pr_string_path(packet, &["canonical_gap_id"]).unwrap_or_else(|| "unknown".to_string())
    ));
    if let Some(source_file) = pr_string_path(packet, &["source_file"]) {
        body.push_str(&format!("- source: `{}`\n", md_escape(&source_file)));
    }
    if let Some(changed) = pr_string_path(packet, &["changed_behavior"]) {
        body.push_str(&format!("- Changed behavior: {}\n", md_escape(&changed)));
    }
    body.push_str(&format!(
        "- repair kind: `{}`\n",
        pr_string_path(packet, &["repair_kind"]).unwrap_or_else(|| "unknown".to_string())
    ));
    if let Some(missing) = pr_packet_missing_discriminator(packet) {
        body.push_str(&format!(
            "- Missing discriminator: {}\n",
            md_escape(&missing)
        ));
    }
    if let Some(intent) = pr_packet_focused_proof_intent(packet) {
        body.push_str(&format!("- Focused proof intent: {}\n", md_escape(&intent)));
    }
    if let Some(related) = pr_related_test_summary(packet) {
        body.push_str(&format!("- related test or observer: {related}\n"));
    }
    if let Some(repair) = pr_string_path(packet, &["recommended_repair"]) {
        body.push_str(&format!("- repair: {}\n", md_escape(&repair)));
    }
    if let Some(verify) = pr_string_path(packet, &["verify_command"]) {
        body.push_str(&format!("- verify: `{}`\n", md_escape(&verify)));
    }
    if let Some(receipt) = pr_string_path(packet, &["receipt_command_or_path"])
        .or_else(|| pr_string_path(packet, &["receipt_command"]))
    {
        body.push_str(&format!("- receipt: `{}`\n", md_escape(&receipt)));
    }
}

fn write_pr_python_top_repair_card(
    body: &mut String,
    changes: &[ChangedPath],
    actionable: &PrActionableInput,
) {
    let Some(report) = actionable.value() else {
        return;
    };
    let packets = pr_actionable_packets(report);
    let changed_paths = pr_changed_path_set(changes);
    let selected = packets
        .iter()
        .copied()
        .find(|packet| {
            pr_packet_is_python(packet) && pr_packet_touches_changed_path(packet, &changed_paths)
        })
        .or_else(|| {
            packets
                .iter()
                .copied()
                .find(|packet| pr_packet_is_python(packet))
        });
    let Some(packet) = selected else {
        return;
    };

    body.push_str("\nTop Python repair card:\n");
    body.push_str("- language: `python` (`preview`)\n");
    if let Some(authority) = pr_python_packet_authority_boundary(packet) {
        body.push_str(&format!(
            "- authority boundary: `{}`\n",
            md_escape(&authority)
        ));
    }
    body.push_str(&format!(
        "- canonical gap: `{}`\n",
        pr_string_path(packet, &["canonical_gap_id"]).unwrap_or_else(|| "unknown".to_string())
    ));
    if let Some(owner) = pr_python_packet_changed_owner(packet) {
        body.push_str(&format!("- Changed owner: `{}`\n", md_escape(&owner)));
    }
    if let Some(changed) = pr_python_packet_changed_behavior(packet) {
        body.push_str(&format!("- Changed behavior: {}\n", md_escape(&changed)));
    }
    if let Some(evidence) = pr_python_packet_current_test_evidence(packet) {
        body.push_str(&format!(
            "- Current test evidence: {}\n",
            md_escape(&evidence)
        ));
    }
    if let Some(missing) = pr_packet_missing_discriminator(packet) {
        body.push_str(&format!(
            "- Missing discriminator: {}\n",
            md_escape(&missing)
        ));
    }
    if let Some(action) = pr_python_packet_repair_action(packet) {
        body.push_str(&format!("- repair action: `{}`\n", md_escape(&action)));
    }
    if let Some(shape) = pr_python_packet_test_shape(packet) {
        body.push_str(&format!("- test shape: {}\n", md_escape(&shape)));
    }
    if let Some(assertion) = pr_python_packet_suggested_assertion(packet) {
        body.push_str(&format!(
            "- Suggested assertion: {}\n",
            md_escape(&assertion)
        ));
    }
    match (
        pr_python_packet_suggested_test_name(packet),
        pr_python_packet_suggested_test_file(packet),
    ) {
        (Some(name), Some(file)) => body.push_str(&format!(
            "- Suggested test target: `{}` in `{}`\n",
            md_escape(&name),
            md_escape(&file)
        )),
        (Some(name), None) => {
            body.push_str(&format!("- Suggested test: `{}`\n", md_escape(&name)));
        }
        (None, Some(file)) => {
            body.push_str(&format!("- Suggested file: `{}`\n", md_escape(&file)));
        }
        (None, None) => {}
    }
    if let Some(verify) = pr_string_path(packet, &["verify_command"]) {
        body.push_str(&format!("- Verify: `{}`\n", md_escape(&verify)));
    }
    if let Some(receipt) = pr_string_path(packet, &["receipt_command_or_path"])
        .or_else(|| pr_string_path(packet, &["receipt_command"]))
    {
        body.push_str(&format!("- Receipt: `{}`\n", md_escape(&receipt)));
    }
    let stop_conditions = pr_python_packet_stop_conditions(packet);
    if !stop_conditions.is_empty() {
        body.push_str("- stop if:\n");
        for condition in stop_conditions.iter().take(3) {
            body.push_str(&format!("  - {}\n", md_escape(condition)));
        }
    }
}

fn pr_packet_is_python(packet: &Value) -> bool {
    pr_first_string_path(packet, &[&["language"], &["repair_card", "language"]]).as_deref()
        == Some("python")
}

fn pr_python_packet_authority_boundary(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["authority_boundary"],
            &["repair_card", "authority_boundary"],
            &["repair_card", "authority_boundary", "kind"],
        ],
    )
}

fn pr_python_packet_changed_owner(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["changed_owner"],
            &["repair_card", "changed_owner"],
            &["anchor", "owner"],
            &["primary_anchor", "owner"],
        ],
    )
}

fn pr_python_packet_changed_behavior(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["changed_behavior"],
            &["repair_card", "changed_behavior"],
            &["repair_route", "changed_behavior"],
        ],
    )
}

fn pr_python_packet_current_test_evidence(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["current_test_evidence"],
            &["repair_card", "current_test_evidence"],
            &["evidence_summary"],
        ],
    )
}

fn pr_python_packet_repair_action(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["repair_action"],
            &["repair_card", "repair_action"],
            &["repair_route", "repair_kind"],
            &["repair_route", "route_kind"],
            &["repair_kind"],
        ],
    )
}

fn pr_python_packet_test_shape(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["recommended_test_shape"],
            &["repair_card", "recommended_test_shape"],
            &["repair_route", "target_test_type"],
        ],
    )
}

fn pr_python_packet_suggested_assertion(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["suggested_assertion"],
            &["repair_card", "suggested_assertion"],
            &["repair_route", "assertion_shape"],
        ],
    )
}

fn pr_python_packet_suggested_test_file(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["suggested_test_file"],
            &["repair_card", "suggested_test_file"],
            &["repair_route", "target_file"],
        ],
    )
    .or_else(|| pr_first_string_array_path(packet, &["allowed_files"]))
    .or_else(|| pr_first_string_array_path(packet, &["allowed_edit_surface"]))
}

fn pr_python_packet_suggested_test_name(packet: &Value) -> Option<String> {
    pr_first_string_path(
        packet,
        &[
            &["suggested_test_name"],
            &["repair_card", "suggested_test_name"],
            &["repair_route", "related_test"],
            &["related_test_or_observer", "name"],
        ],
    )
}

fn pr_python_packet_stop_conditions(packet: &Value) -> Vec<String> {
    audit_string_array(packet, &["stop_if"])
        .or_else(|| audit_string_array(packet, &["stop_conditions"]))
        .or_else(|| audit_string_array(packet, &["repair_card", "stop_conditions"]))
        .or_else(|| audit_string_array(packet, &["repair_route", "stop_conditions"]))
        .unwrap_or_default()
}

fn pr_first_string_path(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| pr_string_path(value, path))
}

fn pr_first_string_array_path(value: &Value, path: &[&str]) -> Option<String> {
    audit_get(value, path)
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(json_scalar_as_string)
}

fn pr_packet_missing_discriminator(packet: &Value) -> Option<String> {
    pr_string_path(packet, &["missing_discriminator"])
        .or_else(|| {
            audit_get(packet, &["missing_discriminators"])
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| pr_string_path(item, &["value"]))
        })
        .or_else(|| pr_string_path(packet, &["candidate_value_or_observer"]))
        .or_else(|| pr_string_path(packet, &["assertion_shape"]))
}

fn pr_packet_focused_proof_intent(packet: &Value) -> Option<String> {
    pr_string_path(packet, &["focused_proof_intent"])
        .or_else(|| pr_string_path(packet, &["recommended_repair"]))
        .or_else(|| {
            pr_string_path(packet, &["assertion_shape"])
                .map(|assertion| format!("Add or strengthen `{assertion}`."))
        })
}

fn pr_related_test_summary(packet: &Value) -> Option<String> {
    let related = audit_get(packet, &["related_test_or_observer"])?;
    let file = pr_string_path(related, &["file"])?;
    let name = pr_string_path(related, &["name"]);
    let line = pr_usize_path(related, &["line"]);
    Some(match (name, line) {
        (Some(name), Some(line)) => format!("`{}:{line}` `{}`", md_escape(&file), md_escape(&name)),
        (Some(name), None) => format!("`{}` `{}`", md_escape(&file), md_escape(&name)),
        (None, Some(line)) => format!("`{}:{line}`", md_escape(&file)),
        (None, None) => format!("`{}`", md_escape(&file)),
    })
}

fn pr_actionable_packets(report: &Value) -> Vec<&Value> {
    audit_get(report, &["packets"])
        .and_then(Value::as_array)
        .map(|packets| {
            packets
                .iter()
                .filter(|packet| {
                    pr_string_path(packet, &["gap_state"]).as_deref() == Some("actionable")
                        && pr_bool_path(packet, &["public_projection_eligible"]).unwrap_or(true)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pr_changed_path_set(changes: &[ChangedPath]) -> BTreeSet<String> {
    changes
        .iter()
        .map(|change| normalize_slashes(&change.path))
        .collect()
}

fn pr_packet_touches_changed_path(packet: &Value, changed_paths: &BTreeSet<String>) -> bool {
    if changed_paths.is_empty() {
        return false;
    }
    pr_packet_paths(packet)
        .iter()
        .any(|path| changed_paths.contains(path))
}

fn pr_packet_paths(packet: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for path in [
        pr_string_path(packet, &["source_file"]),
        pr_string_path(packet, &["primary_anchor", "file"]),
        pr_string_path(packet, &["related_test_or_observer", "file"]),
    ]
    .into_iter()
    .flatten()
    {
        paths.insert(normalize_slashes(&path));
    }
    if let Some(raw_findings) = audit_get(packet, &["raw_findings"]).and_then(Value::as_array) {
        for raw in raw_findings {
            if let Some(path) = pr_string_path(raw, &["file"]) {
                paths.insert(normalize_slashes(&path));
            }
        }
    }
    paths
}

fn pr_packet_static_limitation_count(packet: &Value) -> usize {
    audit_get(packet, &["static_limitations"])
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn pr_usize_path(value: &Value, path: &[&str]) -> Option<usize> {
    audit_get(value, path).and_then(json_scalar_as_usize)
}

fn pr_string_path(value: &Value, path: &[&str]) -> Option<String> {
    audit_get(value, path).and_then(json_scalar_as_string)
}

fn pr_bool_path(value: &Value, path: &[&str]) -> Option<bool> {
    audit_get(value, path).and_then(Value::as_bool)
}

fn report_index_markdown(
    status: &str,
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
    missing: &[String],
    next_commands: &[String],
) -> String {
    let mut body = format!("# ripr report index\n\nStatus: {status}\n\n");
    body.push_str("This is the reviewer front door for generated `ripr` artifacts.\n\n");

    body.push_str("\n## Summary\n\n");
    body.push_str(&format!("- available reports: {}\n", reports.len()));
    body.push_str(&format!("- available receipts: {}\n", receipts.len()));
    body.push_str(&format!("- missing expected reports: {}\n", missing.len()));
    body.push_str(&format!(
        "- failed reports: {}\n",
        reports
            .iter()
            .filter(|entry| entry.status == "fail")
            .count()
    ));
    body.push_str(&format!(
        "- warning reports: {}\n",
        reports
            .iter()
            .filter(|entry| entry.status == "warn")
            .count()
    ));
    let lane1_packets = report_index_lane1_readiness_packets(reports);
    body.push_str(&format!(
        "- lane1 readiness status: `{}`\n",
        report_index_lane1_overall_status(&lane1_packets)
    ));
    body.push_str(&format!(
        "- missing lane1 readiness artifacts: {}\n",
        report_index_missing_artifact_count(&lane1_packets)
    ));

    body.push_str("\n## Suggested Reviewer Path\n\n");
    body.push_str("1. Read `target/ripr/reports/pr-summary.md`.\n");
    body.push_str("2. Read `target/ripr/reports/critic.md`, if present.\n");
    body.push_str("3. Inspect `target/ripr/reports/fixtures.md` and `target/ripr/reports/goldens.md` when fixtures or output changed.\n");
    body.push_str(
        "4. Inspect `target/ripr/reports/golden-drift.md`, if present and output changed.\n",
    );
    body.push_str("5. Inspect `target/ripr/receipts/check-pr.json`, when receipts exist.\n");

    body.push_str("\n## Key Report Status\n\n");
    for file in [
        "pr-summary.md",
        "check-pr.md",
        "pr-shape.md",
        "fixtures.md",
        "goldens.md",
        "golden-drift.md",
        "allow-attributes.md",
        "local-context.md",
        "test-oracles.md",
        "dogfood.md",
        "metrics.md",
    ] {
        body.push_str(&format!(
            "- `{file}`: {}\n",
            status_for_report(reports, file)
        ));
    }

    body.push_str("\n## Lane 1 Evidence Readiness\n\n");
    body.push_str(
        "| Artifact group | Status | Artifacts | Next command |\n| --- | --- | --- | --- |\n",
    );
    for packet in &lane1_packets {
        body.push_str(&format!(
            "| {} | `{}` | {} | `{}` |\n",
            markdown_cell(packet.label),
            markdown_cell(&packet.status),
            markdown_cell(&repo_ops_artifacts_markdown(&packet.artifacts)),
            markdown_cell(packet.command)
        ));
    }

    let repo_ops_packets = report_index_repo_ops_packets(reports, receipts);
    body.push_str("\n## Repo-Ops Packets\n\n");
    body.push_str("| Packet | Status | Artifacts | Next command |\n| --- | --- | --- | --- |\n");
    for packet in &repo_ops_packets {
        body.push_str(&format!(
            "| {} | `{}` | {} | `{}` |\n",
            markdown_cell(packet.label),
            markdown_cell(&packet.status),
            markdown_cell(&repo_ops_artifacts_markdown(&packet.artifacts)),
            markdown_cell(packet.command)
        ));
    }

    body.push_str("\n## Available Reports\n\n");
    if reports.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        body.push_str("| Report | Status |\n| --- | --- |\n");
        for entry in reports {
            body.push_str(&format!(
                "| `{}` | `{}` |\n",
                markdown_cell(&entry.path),
                markdown_cell(&entry.status)
            ));
        }
    }

    body.push_str("\n## Missing Expected Reports\n\n");
    write_path_list(&mut body, missing);

    body.push_str("\n## Receipts\n\n");
    if receipts.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        for receipt in receipts {
            body.push_str(&format!("- `{}`\n", receipt.path));
        }
    }

    body.push_str("\n## Suggested Next Commands\n\n");
    for command in next_commands {
        body.push_str(&format!("- `{command}`\n"));
    }

    body
}

fn report_index_json(
    status: &str,
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
    missing: &[String],
    next_commands: &[String],
) -> String {
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str("  \"reports\": [\n");
    write_report_index_entry_array(&mut body, reports);
    body.push_str("  ],\n");
    body.push_str("  \"receipts\": [\n");
    write_report_index_entry_array(&mut body, receipts);
    body.push_str("  ],\n");
    body.push_str("  \"repo_ops_packets\": [\n");
    write_report_index_repo_ops_packet_array(
        &mut body,
        &report_index_repo_ops_packets(reports, receipts),
    );
    body.push_str("  ],\n");
    let lane1_packets = report_index_lane1_readiness_packets(reports);
    body.push_str("  \"lane1_readiness\": {\n");
    body.push_str(&format!(
        "    \"status\": \"{}\",\n",
        json_escape(&report_index_lane1_overall_status(&lane1_packets))
    ));
    body.push_str(&format!(
        "    \"missing_artifacts\": {},\n",
        report_index_missing_artifact_count(&lane1_packets)
    ));
    body.push_str(&format!(
        "    \"warning_artifacts\": {},\n",
        report_index_warning_artifact_count(&lane1_packets)
    ));
    body.push_str(&format!(
        "    \"failing_artifacts\": {},\n",
        report_index_failing_artifact_count(&lane1_packets)
    ));
    body.push_str("    \"packets\": [\n");
    write_report_index_repo_ops_packet_array(&mut body, &lane1_packets);
    body.push_str("    ]\n");
    body.push_str("  },\n");
    body.push_str("  \"missing_expected_reports\": [");
    write_json_string_array(&mut body, missing);
    body.push_str("],\n");
    body.push_str("  \"suggested_next_commands\": [");
    write_json_string_array(&mut body, next_commands);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn repo_ops_artifacts_markdown(artifacts: &[ReportIndexRepoOpsArtifact]) -> String {
    artifacts
        .iter()
        .map(|artifact| format!("{} ({})", artifact.path, artifact.status))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn write_report_index_entry_array(body: &mut String, entries: &[ReportIndexEntry]) {
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"file\": \"{}\",\n",
            json_escape(&entry.file)
        ));
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&entry.path)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\"\n",
            json_escape(&entry.status)
        ));
        body.push_str("    }");
    }
    if !entries.is_empty() {
        body.push('\n');
    }
}

fn write_report_index_repo_ops_packet_array(
    body: &mut String,
    packets: &[ReportIndexRepoOpsPacket],
) {
    for (index, packet) in packets.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(packet.id)));
        body.push_str(&format!(
            "      \"label\": \"{}\",\n",
            json_escape(packet.label)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(&packet.status)
        ));
        body.push_str(&format!(
            "      \"next_command\": \"{}\",\n",
            json_escape(packet.command)
        ));
        body.push_str(&format!(
            "      \"description\": \"{}\",\n",
            json_escape(packet.description)
        ));
        body.push_str("      \"artifacts\": [\n");
        for (artifact_index, artifact) in packet.artifacts.iter().enumerate() {
            if artifact_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("        {\n");
            body.push_str(&format!(
                "          \"path\": \"{}\",\n",
                json_escape(&artifact.path)
            ));
            body.push_str(&format!(
                "          \"status\": \"{}\",\n",
                json_escape(&artifact.status)
            ));
            body.push_str(&format!(
                "          \"available\": {}\n",
                artifact.available
            ));
            body.push_str("        }");
        }
        if !packet.artifacts.is_empty() {
            body.push('\n');
        }
        body.push_str("      ]\n");
        body.push_str("    }");
    }
    if !packets.is_empty() {
        body.push('\n');
    }
}

fn status_for_report(reports: &[ReportIndexEntry], file: &str) -> String {
    reports
        .iter()
        .find(|entry| entry.file == file)
        .map(|entry| entry.status.clone())
        .unwrap_or_else(|| "missing".to_string())
}

fn pr_shape_warnings(changes: &[ChangedPath]) -> Vec<String> {
    let mut warnings = Vec::new();
    let has_production = changes
        .iter()
        .any(|change| is_production_path(&change.path));
    let has_evidence = changes
        .iter()
        .any(|change| is_evidence_path(&change.path) && !is_production_path(&change.path));
    if has_production && !has_evidence {
        warnings.push(
            "Production code changed without an evidence/support file. Add or update a spec, test, fixture, golden, metric, or doc, or explain why this is a pure refactor."
                .to_string(),
        );
    }

    let analysis_changed = changes
        .iter()
        .any(|change| change.path.starts_with("crates/ripr/src/analysis/"));
    let analysis_evidence = changes.iter().any(|change| {
        is_spec_path(&change.path)
            || is_test_path(&change.path)
            || is_fixture_path(&change.path)
            || change.path.starts_with("metrics/")
            || change.path == ".ripr/traceability.toml"
    });
    if analysis_changed && !analysis_evidence {
        warnings.push(
            "Analysis code changed without spec/test/fixture/metric/traceability evidence. Analyzer PRs should carry behavior evidence unless this is a narrow mechanical refactor."
                .to_string(),
        );
    }

    let output_changed = changes.iter().any(|change| {
        change.path.starts_with("crates/ripr/src/output/")
            || change.path.starts_with("crates/ripr/src/domain/")
            || change.path == "crates/ripr/src/lsp.rs"
    });
    let output_evidence = changes.iter().any(|change| {
        change.path == "docs/OUTPUT_SCHEMA.md"
            || change.path == "policy/output_contracts.txt"
            || is_golden_path(&change.path)
            || is_fixture_path(&change.path)
    });
    if output_changed && !output_evidence {
        warnings.push(
            "Output-facing code changed without output schema, contract registry, fixture, or golden evidence. Add the matching output evidence or explain why output is unchanged."
                .to_string(),
        );
    }

    let policy_changed = changes.iter().any(|change| is_policy_path(&change.path));
    let policy_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md" | "CONTRIBUTING.md" | "README.md" | "docs/CI.md" | "docs/PR_AUTOMATION.md"
        )
    });
    if policy_changed && !policy_docs_changed {
        warnings.push(
            "Policy metadata changed without front-door process docs. Update AGENTS, CONTRIBUTING, README, docs/CI.md, or docs/PR_AUTOMATION.md when policy behavior changes."
                .to_string(),
        );
    }

    let xtask_changed = changes
        .iter()
        .any(|change| change.path.starts_with("xtask/"));
    let xtask_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md"
                | "CONTRIBUTING.md"
                | "README.md"
                | "docs/CI.md"
                | "docs/PR_AUTOMATION.md"
                | "docs/TESTING.md"
        )
    });
    if xtask_changed && !xtask_docs_changed {
        warnings.push(
            "xtask behavior changed without command/process docs. Update the relevant front-door docs or explain why the command surface is unchanged."
                .to_string(),
        );
    }

    warnings
}

fn pr_shape_report_body(warnings: &[String]) -> String {
    let status = if warnings.is_empty() { "pass" } else { "warn" };
    let mut body = format!("# ripr PR shape report\n\nStatus: {status}\n\n");
    body.push_str(
        "This report is advisory. It highlights likely missing evidence before review.\n\n",
    );
    body.push_str("## Warnings\n\n");
    if warnings.is_empty() {
        body.push_str("None detected.\n");
    } else {
        for warning in warnings {
            body.push_str("```text\n");
            body.push_str(warning);
            body.push_str("\n```\n\n");
        }
    }
    body
}

fn critic_findings(
    changes: &[ChangedPath],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> Vec<CriticFinding> {
    let mut findings = Vec::new();

    let analysis_changed = changes.iter().any(|change| is_analysis_path(&change.path));
    let analysis_evidence = changes.iter().any(|change| {
        is_spec_path(&change.path)
            || is_test_path(&change.path)
            || is_fixture_path(&change.path)
            || is_golden_path(&change.path)
            || change.path.starts_with("metrics/")
            || change.path == ".ripr/traceability.toml"
    });
    if analysis_changed && !analysis_evidence {
        findings.push(CriticFinding {
            id: "analysis_without_behavior_evidence",
            severity: "warn",
            message: "Analyzer code changed without spec, test, fixture, golden, metric, or traceability evidence.",
            evidence: paths_matching(changes, is_analysis_path),
            recommended_action:
                "Add focused behavior evidence or document why this is a mechanical refactor.",
        });
    }
    if analysis_changed && missing_or_bad_report(reports, "fixtures.md") {
        findings.push(CriticFinding {
            id: "analysis_missing_fixture_report",
            severity: "warn",
            message: "Analyzer code changed without a passing fixture report in target/ripr/reports.",
            evidence: vec![format_report_status(reports, "fixtures.md")],
            recommended_action: "Run `cargo xtask fixtures` before review.",
        });
    }
    if analysis_changed && missing_or_bad_report(reports, "goldens.md") {
        findings.push(CriticFinding {
            id: "analysis_missing_golden_report",
            severity: "warn",
            message: "Analyzer code changed without a passing golden report in target/ripr/reports.",
            evidence: vec![format_report_status(reports, "goldens.md")],
            recommended_action: "Run `cargo xtask goldens check` before review.",
        });
    }

    let output_changed = changes
        .iter()
        .any(|change| is_output_surface_path(&change.path));
    let output_evidence = changes.iter().any(|change| {
        change.path == "docs/OUTPUT_SCHEMA.md"
            || change.path == "policy/output_contracts.txt"
            || is_fixture_path(&change.path)
            || is_golden_path(&change.path)
    });
    if output_changed && !output_evidence {
        findings.push(CriticFinding {
            id: "output_without_contract_or_golden_evidence",
            severity: "warn",
            message: "Output-facing code changed without output schema, contract, fixture, or golden evidence.",
            evidence: paths_matching(changes, is_output_surface_path),
            recommended_action:
                "Add output-contract and fixture/golden evidence, or document why rendered output is unchanged.",
        });
    }
    if output_changed && missing_or_bad_report(reports, "output-contracts.md") {
        findings.push(CriticFinding {
            id: "output_missing_contract_report",
            severity: "warn",
            message: "Output-facing code changed without a passing output-contract report.",
            evidence: vec![format_report_status(reports, "output-contracts.md")],
            recommended_action: "Run `cargo xtask check-output-contracts` before review.",
        });
    }
    if output_changed && missing_or_bad_report(reports, "golden-drift.md") {
        findings.push(CriticFinding {
            id: "output_missing_golden_drift_report",
            severity: "warn",
            message: "Output-facing code changed without a semantic golden-drift report.",
            evidence: vec![format_report_status(reports, "golden-drift.md")],
            recommended_action: "Run `cargo xtask golden-drift` before review.",
        });
    }
    if output_changed
        && !changes
            .iter()
            .any(|change| change.path == "policy/output_contracts.txt")
    {
        findings.push(CriticFinding {
            id: "public_output_terms_without_registry_update",
            severity: "warn",
            message: "Public output surface changed without an output contract registry update.",
            evidence: paths_matching(changes, is_output_surface_path),
            recommended_action:
                "Confirm no public output terms changed, or update `policy/output_contracts.txt`.",
        });
    }

    let capability_docs_changed = changes
        .iter()
        .any(|change| change.path == "docs/CAPABILITY_MATRIX.md");
    let capability_metrics_changed = changes
        .iter()
        .any(|change| change.path == "metrics/capabilities.toml");
    if capability_docs_changed && !capability_metrics_changed {
        findings.push(CriticFinding {
            id: "capability_docs_without_metrics",
            severity: "warn",
            message: "Capability matrix changed without machine-readable capability metrics.",
            evidence: paths_matching(changes, |path| path == "docs/CAPABILITY_MATRIX.md"),
            recommended_action:
                "Update `metrics/capabilities.toml` or document why the change is prose-only.",
        });
    }
    if (capability_docs_changed || capability_metrics_changed)
        && missing_or_bad_report(reports, "capabilities.md")
    {
        findings.push(CriticFinding {
            id: "capability_missing_report",
            severity: "warn",
            message: "Capability state changed without a passing capability report.",
            evidence: vec![format_report_status(reports, "capabilities.md")],
            recommended_action: "Run `cargo xtask check-capabilities` before review.",
        });
    }

    let missing_blessings = golden_changes_without_blessing(changes);
    if !missing_blessings.is_empty() {
        findings.push(CriticFinding {
            id: "golden_changed_without_blessing_reason",
            severity: "warn",
            message: "Fixture expected output changed without a matching blessing reason changelog.",
            evidence: missing_blessings,
            recommended_action:
                "Record the intentional output change in the fixture expected-output changelog.",
        });
    }

    let policy_changed = changes.iter().any(|change| is_policy_path(&change.path));
    let process_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md" | "CONTRIBUTING.md" | "README.md" | "docs/CI.md" | "docs/PR_AUTOMATION.md"
        )
    });
    if policy_changed && !process_docs_changed {
        findings.push(CriticFinding {
            id: "policy_without_process_docs",
            severity: "warn",
            message: "Policy files or workflows changed without process documentation.",
            evidence: paths_matching(changes, is_policy_path),
            recommended_action:
                "Update front-door process docs or document why behavior did not change.",
        });
    }

    let extension_changed = changes
        .iter()
        .any(|change| change.path.starts_with("editors/vscode/"));
    if extension_changed {
        findings.push(CriticFinding {
            id: "extension_requires_package_evidence",
            severity: "warn",
            message: "VS Code extension files changed; local xtask reports do not prove npm compile/package evidence.",
            evidence: paths_matching(changes, |path| path.starts_with("editors/vscode/")),
            recommended_action: "Verify `npm run compile` and `npm run package`, or inspect the CI vscode job.",
        });
    }

    if missing_or_bad_report(reports, "pr-summary.md") {
        findings.push(CriticFinding {
            id: "missing_pr_summary",
            severity: "warn",
            message: "The reviewer packet is missing a PR summary report.",
            evidence: vec![format_report_status(reports, "pr-summary.md")],
            recommended_action: "Run `cargo xtask pr-summary` before review.",
        });
    }
    if missing_or_bad_report(reports, "pr-shape.md") {
        findings.push(CriticFinding {
            id: "missing_pr_shape_report",
            severity: "warn",
            message: "The advisory PR shape report is missing or not passing.",
            evidence: vec![format_report_status(reports, "pr-shape.md")],
            recommended_action: "Run `cargo xtask check-pr-shape` before review.",
        });
    }
    if receipts.is_empty() {
        findings.push(CriticFinding {
            id: "missing_receipts",
            severity: "warn",
            message: "No machine-readable receipts were found for this reviewer packet.",
            evidence: vec!["target/ripr/receipts: missing or empty".to_string()],
            recommended_action: "Run `cargo xtask receipts` and `cargo xtask receipts check`.",
        });
    }

    findings
}

fn missing_or_bad_report(reports: &[ReportIndexEntry], file: &str) -> bool {
    !matches!(
        status_for_report(reports, file).as_str(),
        "pass" | "present"
    )
}

fn format_report_status(reports: &[ReportIndexEntry], file: &str) -> String {
    format!(
        "target/ripr/reports/{file}: {}",
        status_for_report(reports, file)
    )
}

fn golden_changes_without_blessing(changes: &[ChangedPath]) -> Vec<String> {
    let changed_paths = changes
        .iter()
        .map(|change| change.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut missing = Vec::new();
    for change in changes.iter().filter(|change| is_golden_path(&change.path)) {
        let Some(fixture) = fixture_name_from_expected_output(&change.path) else {
            continue;
        };
        let changelog = format!("fixtures/{fixture}/expected/CHANGELOG.md");
        if !changed_paths.contains(changelog.as_str()) {
            missing.push(format!(
                "{} changed without `{changelog}`",
                format_changed_path(change)
            ));
        }
    }
    missing.sort();
    missing.dedup();
    missing
}

fn fixture_name_from_expected_output(path: &str) -> Option<String> {
    let rest = path.strip_prefix("fixtures/")?;
    let (fixture, after_fixture) = rest.split_once('/')?;
    if after_fixture.starts_with("expected/") && after_fixture != "expected/CHANGELOG.md" {
        Some(fixture.to_string())
    } else {
        None
    }
}

fn critic_markdown(
    findings: &[CriticFinding],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let mut body = format!("# ripr critic report\n\nStatus: {status}\n\n");
    body.push_str("Mode: advisory\n\n");
    body.push_str("This report is a deterministic adversarial review packet. It flags likely missing evidence from the current diff, reports, and receipts. It does not block CI.\n\n");

    body.push_str("## Findings\n\n");
    if findings.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for finding in findings {
            body.push_str(&format!(
                "### {} ({})\n\n{}\n\n",
                finding.id, finding.severity, finding.message
            ));
            body.push_str("Evidence:\n\n");
            write_path_list(&mut body, &finding.evidence);
            body.push_str("\nRecommended action:\n\n```text\n");
            body.push_str(finding.recommended_action);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Inputs\n\n");
    body.push_str(&format!("- reports available: {}\n", reports.len()));
    body.push_str(&format!("- receipts available: {}\n\n", receipts.len()));
    body.push_str("## Next Commands\n\n");
    body.push_str("```bash\n");
    body.push_str("cargo xtask pr-summary\n");
    body.push_str("cargo xtask reports index\n");
    body.push_str("cargo xtask receipts\n");
    body.push_str("cargo xtask receipts check\n");
    body.push_str("```\n");
    body
}

fn critic_json(
    findings: &[CriticFinding],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!("  \"status\": \"{status}\",\n"));
    body.push_str(&format!("  \"reports_available\": {},\n", reports.len()));
    body.push_str(&format!("  \"receipts_available\": {},\n", receipts.len()));
    body.push_str("  \"findings\": [\n");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(finding.id)));
        body.push_str(&format!(
            "      \"severity\": \"{}\",\n",
            json_escape(finding.severity)
        ));
        body.push_str(&format!(
            "      \"message\": \"{}\",\n",
            json_escape(finding.message)
        ));
        body.push_str("      \"evidence\": [");
        write_json_string_array(&mut body, &finding.evidence);
        body.push_str("],\n");
        body.push_str(&format!(
            "      \"recommended_action\": \"{}\"\n",
            json_escape(finding.recommended_action)
        ));
        body.push_str("    }");
    }
    if !findings.is_empty() {
        body.push('\n');
    }
    body.push_str("  ]\n");
    body.push_str("}\n");
    body
}

fn write_path_list(body: &mut String, paths: &[String]) {
    if paths.is_empty() {
        body.push_str("- None detected.\n");
        return;
    }
    for path in paths {
        body.push_str(&format!("- `{path}`\n"));
    }
}

fn paths_matching(changes: &[ChangedPath], predicate: fn(&str) -> bool) -> Vec<String> {
    changes
        .iter()
        .filter(|change| predicate(&change.path))
        .map(format_changed_path)
        .collect()
}

fn detected_surface_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "Rust product code",
            paths_matching(changes, |path| path.starts_with("crates/ripr/src/")),
        ),
        (
            "Rust tests",
            paths_matching(changes, |path| path.starts_with("crates/ripr/tests/")),
        ),
        (
            "Automation/tooling",
            paths_matching(changes, |path| path.starts_with("xtask/")),
        ),
        (
            "Fixtures",
            paths_matching(changes, |path| path.starts_with("fixtures/")),
        ),
        (
            "Goldens",
            paths_matching(changes, |path| {
                path.contains("/expected/") || path.contains("/golden")
            }),
        ),
        (
            "Docs",
            paths_matching(changes, |path| {
                path.starts_with("docs/")
                    || is_plan_path(path)
                    || matches!(
                        path,
                        "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
                    )
            }),
        ),
        (
            "Policies",
            paths_matching(changes, |path| {
                path.starts_with("policy/") || path.starts_with(".ripr/")
            }),
        ),
        (
            "Workflows",
            paths_matching(changes, |path| path.starts_with(".github/")),
        ),
        (
            "Extension",
            paths_matching(changes, |path| path.starts_with("editors/vscode/")),
        ),
    ]
}

fn is_human_output_path(path: &str) -> bool {
    path == "crates/ripr/src/output/human.rs" || path.starts_with("crates/ripr/src/output/human/")
}

fn public_contract_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "CLI",
            paths_matching(changes, |path| {
                path.starts_with("crates/ripr/src/cli/")
                    || path == "crates/ripr/src/main.rs"
                    || path.starts_with("docs/reference/cli")
            }),
        ),
        (
            "JSON",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/output/json.rs"
                    || path.starts_with("crates/ripr/src/output/json/")
                    || path == "docs/OUTPUT_SCHEMA.md"
            }),
        ),
        (
            "Human output",
            paths_matching(changes, is_human_output_path),
        ),
        (
            "LSP",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/lsp.rs" || path.starts_with("editors/vscode/")
            }),
        ),
        (
            "GitHub/SARIF",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/output/github.rs"
                    || path.to_ascii_lowercase().contains("sarif")
            }),
        ),
        (
            "Config",
            paths_matching(changes, |path| {
                path == "ripr.toml.example" || path.contains("config") || path.contains("ripr-toml")
            }),
        ),
        (
            "Docs",
            paths_matching(changes, |path| {
                path.starts_with("docs/")
                    || is_plan_path(path)
                    || matches!(
                        path,
                        "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
                    )
            }),
        ),
    ]
}

fn policy_exception_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "Non-Rust files",
            paths_matching(changes, |path| {
                is_file_policy_candidate(path) && !path.ends_with(".rs")
            }),
        ),
        (
            "Executable files",
            paths_matching(changes, |path| path == "policy/executable_allowlist.txt"),
        ),
        (
            "Panic-family allowlist",
            paths_matching(changes, |path| path == ".ripr/no-panic-allowlist.txt"),
        ),
        (
            "Static-language allowlist",
            paths_matching(changes, |path| {
                path == STATIC_LANGUAGE_ALLOWLIST_PATH
                    || path == STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH
            }),
        ),
        (
            "Workflow budget",
            paths_matching(changes, |path| path == "policy/workflow_allowlist.txt"),
        ),
        (
            "Dependencies",
            paths_matching(changes, |path| {
                path == "policy/dependency_allowlist.txt" || is_dependency_surface_candidate(path)
            }),
        ),
    ]
}

fn reviewer_focus(changes: &[ChangedPath]) -> Vec<String> {
    let mut focus = Vec::new();
    for predicate in [
        is_production_path as fn(&str) -> bool,
        is_test_path,
        is_spec_path,
        is_fixture_path,
        is_golden_path,
        is_automation_path,
        is_policy_path,
    ] {
        for path in paths_matching(changes, predicate) {
            let raw_path = strip_status_suffix(&path).to_string();
            if !focus.contains(&raw_path) {
                focus.push(raw_path);
            }
            if focus.len() >= 8 {
                return focus;
            }
        }
    }
    focus
}

fn is_production_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/") || path.starts_with("editors/vscode/src/")
}

fn is_evidence_path(path: &str) -> bool {
    is_test_path(path)
        || is_spec_path(path)
        || is_fixture_path(path)
        || is_golden_path(path)
        || is_automation_path(path)
        || is_policy_path(path)
        || is_plan_path(path)
        || path.starts_with("docs/")
        || path.starts_with("metrics/")
        || matches!(
            path,
            "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
        )
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("crates/ripr/tests/") || path.contains("/tests/")
}

fn is_spec_path(path: &str) -> bool {
    path.starts_with("docs/specs/") || path == "docs/SPEC_FORMAT.md"
}

fn is_fixture_path(path: &str) -> bool {
    path.starts_with("fixtures/")
}

fn is_golden_path(path: &str) -> bool {
    path.contains("/expected/") || path.contains("/golden")
}

fn is_automation_path(path: &str) -> bool {
    path.starts_with("xtask/")
}

fn is_policy_path(path: &str) -> bool {
    path.starts_with("policy/") || path.starts_with(".ripr/") || path.starts_with(".github/")
}

fn format_changed_path(change: &ChangedPath) -> String {
    let status = change
        .statuses
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    if status.is_empty() {
        change.path.clone()
    } else {
        format!("{} ({status})", change.path)
    }
}

fn strip_status_suffix(path: &str) -> &str {
    match path.rsplit_once(" (") {
        Some((raw_path, _)) => raw_path,
        None => path,
    }
}

fn read_path_allowlist(path: &str) -> Result<BTreeSet<String>, String> {
    let mut allowed = BTreeSet::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        allowed.insert(normalize_slashes(trimmed));
    }
    Ok(allowed)
}

fn read_count_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_count_policy_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 5 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|owner|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty()
            || parts[1].trim().is_empty()
            || parts[3].trim().is_empty()
            || parts[4].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, owner, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_local_context_allowlist(path: &str) -> Result<Vec<LocalContextAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty() || parts[1].trim().is_empty() || parts[3].trim().is_empty() {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.push(LocalContextAllow {
            path: normalize_slashes(parts[0].trim()),
            pattern: parts[1].trim().to_string(),
            max_count,
            line: line_number + 1,
        });
    }
    Ok(allowed)
}

fn validate_local_context_allowlist(allowlist: &[LocalContextAllow]) -> Vec<String> {
    let mut violations = Vec::new();
    for entry in allowlist {
        if !is_local_context_candidate(&entry.path) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist entry targets a file type that is not scanned\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Local context exceptions should stay narrow and reviewable.\nRecommended fixes:\n1. Remove the stale exception.\n2. If the file should be scanned, add its extension to the checker intentionally.",
                entry.pattern, entry.line
            ));
        }
        if forbidden_local_context_allowlist_pattern(&entry.pattern) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist tries to permit real machine or session state\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Real machine paths, Codex memory paths, and sandbox paths must be removed, not allowlisted.\nRecommended fixes:\n1. Delete the local context from the committed file.\n2. Keep only generic examples in durable docs.",
                entry.pattern, entry.line
            ));
        }
    }
    violations
}

fn forbidden_local_context_allowlist_pattern(pattern: &str) -> bool {
    let lower = pattern.to_ascii_lowercase();
    if lower.contains(concat!(".", "codex"))
        || lower.contains(concat!("memory", ".md"))
        || lower.contains(concat!("sandbox:", "/mnt", "/data"))
        || lower.contains(concat!("/mnt", "/data"))
        || lower.contains(concat!("contentreference", "[oaicite"))
    {
        return true;
    }
    for token in windows_absolute_path_tokens(pattern) {
        let generic_example = token
            .to_ascii_lowercase()
            .replace('/', "\\")
            .contains(concat!(":\\", "path", "\\to\\"));
        if !generic_example {
            return true;
        }
    }
    !unix_home_path_tokens(pattern).is_empty()
}

fn read_glob_allowlist(path: &str) -> Result<Vec<GlobAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected glob|kind|owner|reason",
                line_number + 1
            ));
        }
        let entry = GlobAllow {
            glob: normalize_slashes(parts[0]),
        };
        if entry.glob.is_empty()
            || parts[1].trim().is_empty()
            || parts[2].trim().is_empty()
            || parts[3].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require glob, kind, owner, and reason",
                line_number + 1
            ));
        }
        allowed.push(entry);
    }
    Ok(allowed)
}

pub(crate) fn read_file_policy_allowlist(path: &str) -> Result<Vec<GlobAllow>, String> {
    let entries = parse_file_policy_allowlist(path)?;
    Ok(entries
        .into_iter()
        .map(|entry| GlobAllow {
            glob: normalize_slashes(&entry.glob.unwrap_or_default()),
        })
        .collect())
}

fn parse_file_policy_allowlist(path: &str) -> Result<Vec<FilePolicyAllowEntry>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut current = FilePolicyAllowEntry::default();
    let mut in_entry = false;

    let lines = text.lines().collect::<Vec<_>>();
    let mut idx = 0;
    while idx < lines.len() {
        let line_number = idx + 1;
        let trimmed = lines[idx].trim();
        idx += 1;
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[allow]]" {
            if in_entry {
                validate_file_policy_allow_entry(path, &current)?;
                entries.push(current);
            }
            current = FilePolicyAllowEntry {
                line: line_number,
                ..FilePolicyAllowEntry::default()
            };
            in_entry = true;
            continue;
        }
        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            continue;
        };
        if !in_entry {
            continue;
        }
        match key {
            "glob" => current.glob = Some(parse_string_value(value, path, line_number)?),
            "kind" => current.kind = Some(parse_string_value(value, path, line_number)?),
            "owner" => current.owner = Some(parse_string_value(value, path, line_number)?),
            "surface" => current.surface = Some(parse_string_value(value, path, line_number)?),
            "classification" => {
                current.classification = Some(parse_string_value(value, path, line_number)?)
            }
            "reason" => current.reason = Some(parse_string_value(value, path, line_number)?),
            "generated_by" => {
                current.generated_by = Some(parse_string_value(value, path, line_number)?)
            }
            "covered_by" => {
                let value = collect_toml_array_value(path, line_number, value, &lines, &mut idx)?;
                current.covered_by = Some(parse_inline_array(&value)?);
            }
            "expires" | "retired" => {}
            other => {
                return Err(format!(
                    "{path}:{line_number} unsupported non-Rust allowlist field `{other}`"
                ));
            }
        }
    }

    if in_entry {
        validate_file_policy_allow_entry(path, &current)?;
        entries.push(current);
    }
    if entries.is_empty() {
        return Err(format!("{path} has no [[allow]] entries"));
    }
    Ok(entries)
}

fn collect_toml_array_value(
    path: &str,
    line_number: usize,
    first_value: &str,
    lines: &[&str],
    idx: &mut usize,
) -> Result<String, String> {
    let mut value = first_value.trim().to_string();
    if !value.starts_with('[') || value.ends_with(']') {
        return Ok(value);
    }
    while *idx < lines.len() {
        let next = lines[*idx].trim();
        *idx += 1;
        value.push(' ');
        value.push_str(next);
        if next.ends_with(']') {
            return Ok(value);
        }
    }
    Err(format!(
        "{path}:{line_number} unterminated non-Rust allowlist array"
    ))
}

fn validate_file_policy_allow_entry(
    path: &str,
    entry: &FilePolicyAllowEntry,
) -> Result<(), String> {
    let required = [
        ("glob", entry.glob.as_deref()),
        ("kind", entry.kind.as_deref()),
        ("owner", entry.owner.as_deref()),
        ("surface", entry.surface.as_deref()),
        ("classification", entry.classification.as_deref()),
        ("reason", entry.reason.as_deref()),
    ];
    for (field, value) in required {
        if value.unwrap_or_default().trim().is_empty() {
            return Err(format!(
                "{path}:{} non-Rust allowlist entry requires `{field}`",
                entry.line
            ));
        }
    }
    let covered_by = entry.covered_by.as_ref().ok_or_else(|| {
        format!(
            "{path}:{} non-Rust allowlist entry requires `covered_by`",
            entry.line
        )
    })?;
    if covered_by.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "{path}:{} non-Rust allowlist `covered_by` values must be non-empty",
            entry.line
        ));
    }
    Ok(())
}

fn read_workflow_budgets(path: &str) -> Result<BTreeMap<String, WorkflowBudget>, String> {
    let mut budgets = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!(
                "{path}:{} expected path|max_non_empty_lines|reason",
                line_number + 1
            ));
        }
        let max_non_empty_lines = parts[1].parse::<usize>().map_err(|err| {
            format!(
                "{path}:{} invalid max_non_empty_lines: {err}",
                line_number + 1
            )
        })?;
        let budget = WorkflowBudget {
            path: normalize_slashes(parts[0]),
            max_non_empty_lines,
            reason: parts[2].trim().to_string(),
        };
        if budget.reason.is_empty() {
            return Err(format!(
                "{path}:{} reason must not be empty",
                line_number + 1
            ));
        }
        budgets.insert(budget.path.clone(), budget);
    }
    Ok(budgets)
}

fn read_path_allowlist_optional(path: &str) -> Result<BTreeSet<String>, String> {
    if Path::new(path).exists() {
        read_path_allowlist(path)
    } else {
        Ok(BTreeSet::new())
    }
}

fn spec_id_from_file_name(file_name: &str) -> Option<String> {
    let mut parts = file_name.split('-');
    let prefix = parts.next()?;
    let kind = parts.next()?;
    let number = parts.next()?;
    if prefix == "RIPR"
        && kind == "SPEC"
        && number.len() == 4
        && number.chars().all(|value| value.is_ascii_digit())
    {
        Some(format!("{prefix}-{kind}-{number}"))
    } else {
        None
    }
}

fn spec_status(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("Status: "))
        .map(|value| value.trim().to_string())
}

fn required_spec_headings() -> Vec<&'static str> {
    vec![
        "## Problem",
        "## Behavior",
        "## Required Evidence",
        "## Non-Goals",
        "## Acceptance Examples",
        "## Test Mapping",
        "## Implementation Mapping",
        "## Metrics",
    ]
}

fn has_markdown_heading(text: &str, heading: &str) -> bool {
    text.lines().any(|line| line.trim_end() == heading)
}

pub(crate) fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_inner(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let normalized = normalize_path(path);
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_normalized = normalize_path(relative);
    if should_skip_path(&relative_normalized) {
        return Ok(());
    }
    let metadata =
        fs::metadata(path).map_err(|err| format!("failed to inspect {normalized}: {err}"))?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|err| format!("failed to read {normalized}: {err}"))?
        {
            let entry = entry.map_err(|err| format!("failed to read {normalized}: {err}"))?;
            collect_files_inner(root, &entry.path(), files)?;
        }
    }
    Ok(())
}

fn tracked_files() -> Result<Vec<String>, String> {
    let output = run_output("git", &["ls-files"])?;
    Ok(output
        .lines()
        .map(normalize_slashes)
        .filter(|path| !path.is_empty())
        .collect())
}

fn should_skip_path(path: &str) -> bool {
    path == ".git"
        || path.starts_with(".git/")
        || path == ".claude"
        || path.starts_with(".claude/")
        || path == "target"
        || path.starts_with("target/")
        || path.ends_with("/target")
        || path.contains("/target/")
        || path == ".ripr/release"
        || path.starts_with(".ripr/release/")
        || path.ends_with("/.vscode-test")
        || path.contains("/.vscode-test/")
        || path.ends_with("/node_modules")
        || path.contains("/node_modules/")
        || path.ends_with("/out")
        || path.contains("/out/")
        || path.ends_with("/dist")
        || path.contains("/dist/")
}

fn is_static_language_candidate(path: &str) -> bool {
    // Skip fixture CHANGELOG files (#2338): these contain bless reasons that
    // may legitimately reference banned words (e.g. "reword 'proven'"). They
    // are bookkeeping, not product output or source prose.
    if path.contains("/expected/CHANGELOG.md") || path == "expected/CHANGELOG.md" {
        return false;
    }
    let extensions = [
        ".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml", ".ts", ".tsx", ".js", ".jsx",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn read_text_lossy(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn guarded_allow_attribute_lints() -> BTreeSet<&'static str> {
    [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::dbg_macro",
        "unwrap_used",
        "expect_used",
        "panic",
        "todo",
        "unimplemented",
        "dbg_macro",
        "unsafe_code",
        "dead_code",
        "unused_imports",
        "unused_variables",
        "warnings",
    ]
    .into_iter()
    .collect()
}

fn guarded_allow_attributes_in_text(
    text: &str,
    guarded: &BTreeSet<&'static str>,
) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let line = byte_line_number(text, index);
        let mut cursor = index + 1;
        if cursor < bytes.len() && bytes[cursor] == b'!' {
            cursor += 1;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'[' {
            index += 1;
            continue;
        }
        cursor += 1;
        cursor = skip_ascii_whitespace(bytes, cursor);

        let ident_start = cursor;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_')
        {
            cursor += 1;
        }
        let kind = &text[ident_start..cursor];
        if kind != "allow" && kind != "expect" {
            index += 1;
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'(' {
            index += 1;
            continue;
        }

        let Some((content_start, content_end, next_index)) = attribute_paren_span(bytes, cursor)
        else {
            index += 1;
            continue;
        };
        for lint in attribute_lints(&text[content_start..content_end]) {
            if guarded.contains(lint.as_str()) {
                findings.push((line, format!("{kind}({lint})")));
            }
        }
        index = next_index;
    }
    findings
}

fn attribute_paren_span(bytes: &[u8], open: usize) -> Option<(usize, usize, usize)> {
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((open + 1, index, index + 1));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn attribute_lints(content: &str) -> Vec<String> {
    content
        .split(',')
        .filter_map(|part| {
            let lint = part.trim();
            if lint.is_empty() || lint.contains('=') {
                None
            } else {
                Some(lint.to_string())
            }
        })
        .collect()
}

fn attribute_lint_name(attribute: &str) -> Option<&str> {
    let (_, rest) = attribute.split_once('(')?;
    Some(rest.strip_suffix(')').unwrap_or(rest).trim())
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn byte_line_number(text: &str, byte_index: usize) -> usize {
    text.as_bytes()[..byte_index]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn allow_attribute_line_summary(lines: &[usize]) -> String {
    let mut unique = lines.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn local_context_findings_for_path(path: &str) -> Result<Vec<LocalContextFinding>, String> {
    let mut findings = Vec::new();
    let Some(file_name) = path.rsplit('/').next() else {
        return Ok(findings);
    };

    if suspicious_runtime_file_names()
        .iter()
        .any(|name| file_name.eq_ignore_ascii_case(name))
    {
        findings.push(LocalContextFinding {
            path: path.to_string(),
            line: None,
            pattern: file_name.to_string(),
            problem: "committed runtime/session artifact filename".to_string(),
        });
    }

    if !is_local_context_candidate(path) {
        return Ok(findings);
    }

    let text = read_text_lossy(Path::new(path))?;
    for (line_index, line) in text.lines().enumerate() {
        for (pattern, problem) in local_context_line_findings(line) {
            findings.push(LocalContextFinding {
                path: path.to_string(),
                line: Some(line_index + 1),
                pattern,
                problem,
            });
        }
    }
    Ok(findings)
}

fn local_context_line_findings(line: &str) -> Vec<(String, String)> {
    let mut findings = BTreeSet::<(String, String)>::new();

    for token in windows_absolute_path_tokens(line) {
        findings.insert((token, "local absolute Windows path".to_string()));
    }
    for token in unix_home_path_tokens(line) {
        findings.insert((token, "local absolute Unix home path".to_string()));
    }

    let lower = line.to_ascii_lowercase();
    for (marker, problem) in local_context_markers() {
        if lower.contains(&marker.to_ascii_lowercase()) {
            findings.insert((marker, problem));
        }
    }

    if contains_recorded_date(line) {
        findings.insert((
            recorded_on_pattern().to_string(),
            "session timestamp language".to_string(),
        ));
    }
    if lower.contains(concat!("working tree", " is dirty before")) {
        findings.insert((
            concat!("working tree", " is dirty before").to_string(),
            "transient local worktree state".to_string(),
        ));
    }
    if lower.contains(concat!("before any", " codex edits")) {
        findings.insert((
            concat!("before any", " Codex edits").to_string(),
            "transient Codex session state".to_string(),
        ));
    }
    if lower.contains(concat!("current local", " state")) {
        findings.insert((
            concat!("current local", " state").to_string(),
            "transient local state language".to_string(),
        ));
    }
    if lower.contains(concat!("current", " branch:")) {
        findings.insert((
            concat!("Current", " branch:").to_string(),
            "transient local branch state".to_string(),
        ));
    }

    for token in file_reference_tokens(line) {
        let problem = if token.starts_with("file_") {
            "opaque uploaded file artifact reference"
        } else {
            "chat transcript file reference"
        };
        findings.insert((token, problem.to_string()));
    }

    findings.into_iter().collect()
}

fn local_context_markers() -> Vec<(String, String)> {
    vec![
        (
            concat!(".", "codex").to_string(),
            "Codex local memory path".to_string(),
        ),
        (
            concat!("MEMORY", ".md").to_string(),
            "Codex memory artifact".to_string(),
        ),
        (
            concat!("sandbox:", "/mnt", "/data").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("/mnt", "/data/").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("contentReference", "[oaicite").to_string(),
            "chat citation artifact".to_string(),
        ),
    ]
}

fn suspicious_runtime_file_names() -> Vec<String> {
    vec![
        concat!("CURRENT", "_STATE.md").to_string(),
        concat!("SESSION", "_STATE.md").to_string(),
        "SCRATCHPAD.md".to_string(),
        concat!("NOTES", "_FROM", "_RUN.md").to_string(),
        concat!("CODEX", "_STATE.md").to_string(),
        concat!("codex", "-", "memory", ".md").to_string(),
        "transcript.md".to_string(),
        "chat.md".to_string(),
    ]
}

fn windows_absolute_path_tokens(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index + 2 < bytes.len() {
        let token_boundary = index == 0 || is_local_context_token_delimiter(bytes[index - 1]);
        if token_boundary
            && bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && (bytes[index + 2] == b'\\' || bytes[index + 2] == b'/')
        {
            let start = index;
            index += 3;
            while index < bytes.len() && !is_local_context_token_delimiter(bytes[index]) {
                index += 1;
            }
            tokens.push(line[start..index].to_string());
        } else {
            index += 1;
        }
    }
    tokens
}

fn unix_home_path_tokens(line: &str) -> Vec<String> {
    ["/Users/", "/home/"]
        .iter()
        .flat_map(|prefix| absolute_path_tokens_with_prefix(line, prefix))
        .collect()
}

fn absolute_path_tokens_with_prefix(line: &str, prefix: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find(prefix) {
        let start = search_start + offset;
        let mut end = start + prefix.len();
        let bytes = line.as_bytes();
        let name_start = end;
        while end < line.len()
            && bytes[end] != b'/'
            && !is_local_context_token_delimiter(bytes[end])
        {
            end += 1;
        }
        if end == name_start || end >= line.len() || bytes[end] != b'/' {
            search_start = end.max(start + prefix.len());
            continue;
        }
        end += 1;
        while end < line.len() && !is_local_context_token_delimiter(bytes[end]) {
            end += 1;
        }
        tokens.push(line[start..end].to_string());
        search_start = end;
    }
    tokens
}

fn is_local_context_token_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'"' | b'\'' | b')' | b']' | b'}' | b'<' | b'>' | b',' | b';'
        )
}

fn contains_recorded_date(line: &str) -> bool {
    let marker = recorded_on_marker();
    let Some(offset) = line.find(marker) else {
        return false;
    };
    let date = &line[offset + marker.len()..];
    date.len() >= 10
        && date.as_bytes()[0..4].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[5..7].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[7] == b'-'
        && date.as_bytes()[8..10].iter().all(u8::is_ascii_digit)
}

fn recorded_on_marker() -> &'static str {
    concat!("Recorded", " on ")
}

fn recorded_on_pattern() -> &'static str {
    concat!("Recorded", " on <date>")
}

fn file_reference_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"file_") {
            let start = index;
            index += "file_".len();
            let hex_start = index;
            while index < bytes.len() && bytes[index].is_ascii_hexdigit() {
                index += 1;
            }
            if index - hex_start >= 8 {
                tokens.push(line[start..index].to_string());
            }
            continue;
        }
        if bytes[index..].starts_with(b"turn") {
            let start = index;
            index += "turn".len();
            let digit_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > digit_start && bytes[index..].starts_with(b"file") {
                index += "file".len();
                let file_digit_start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > file_digit_start {
                    tokens.push(line[start..index].to_string());
                    continue;
                }
            }
            index = start + 1;
            continue;
        }
        index += 1;
    }
    tokens
}

fn local_context_line_summary(lines: &[Option<usize>]) -> String {
    let mut concrete = lines.iter().flatten().copied().collect::<Vec<_>>();
    concrete.sort_unstable();
    concrete.dedup();
    if concrete.is_empty() {
        "file name".to_string()
    } else {
        concrete
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_local_context_candidate(path: &str) -> bool {
    let extensions = [
        ".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml", ".ts", ".tsx",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn write_local_context_json(violations: &[String]) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{status}\",\n  \"violation_count\": {},\n  \"violations\": [",
        violations.len()
    );
    for (index, violation) in violations.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str("\n    \"");
        body.push_str(&json_escape(violation));
        body.push('"');
    }
    if !violations.is_empty() {
        body.push('\n');
    }
    body.push_str("  ]\n}\n");
    write_report("local-context.json", &body)
}

pub(crate) fn normalize_path(path: &Path) -> String {
    normalize_slashes(&path.to_string_lossy())
        .trim_start_matches("./")
        .to_string()
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/")
}

pub(crate) fn is_file_policy_candidate(path: &str) -> bool {
    let extensions = [
        ".bash", ".c", ".cjs", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".json", ".kt",
        ".lua", ".mjs", ".php", ".pl", ".ps1", ".py", ".rb", ".sh", ".swift", ".toml", ".ts",
        ".tsx", ".yaml", ".yml", ".zsh",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn is_non_rust_programming_candidate(path: &str) -> bool {
    let extensions = [
        ".bash", ".c", ".cjs", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".kt", ".lua",
        ".mjs", ".php", ".pl", ".ps1", ".py", ".rb", ".sh", ".swift", ".ts", ".tsx", ".zsh",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn non_rust_programming_retention_reason(path: &str) -> Option<&'static str> {
    if path.starts_with("editors/vscode/") && path.ends_with(".ts") {
        return Some(
            "VS Code extension source and tests must run in the VS Code Extension Host TypeScript API.",
        );
    }

    if path.starts_with("fixtures/")
        && (path.ends_with(".ts")
            || path.ends_with(".tsx")
            || path.ends_with(".js")
            || path.ends_with(".jsx")
            || path.ends_with(".py"))
    {
        return Some(
            "Fixture workspaces may contain TypeScript / JavaScript / Python source as analyzed inputs for the Campaign 27 preview adapters (RIPR-SPEC-0027 / RIPR-SPEC-0028).",
        );
    }

    None
}

fn is_generated_candidate(path: &str) -> bool {
    path == "Cargo.lock"
        || path.ends_with("/package-lock.json")
        || path == "package-lock.json"
        || path.starts_with("target/")
        || path.contains("/target/")
        || path.starts_with(".ripr/release/")
        || path.starts_with("dist/")
        || path.contains("/dist/")
        || path.ends_with(".vsix")
        || path.ends_with(".zip")
        || path.ends_with(".tar.gz")
        || path.ends_with(".sha256")
}

fn is_dependency_surface_candidate(path: &str) -> bool {
    let Some(file_name) = path.rsplit('/').next() else {
        return false;
    };
    matches!(
        file_name,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "Pipfile"
            | "Pipfile.lock"
            | "go.mod"
            | "go.sum"
            | "pom.xml"
            | "build.gradle"
            | "settings.gradle"
            | "gradle.lockfile"
            | "Gemfile"
            | "Gemfile.lock"
    )
}

fn is_process_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".ts")
}

fn is_network_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs")
        || path.ends_with(".ts")
        || path.ends_with(".py")
        || path.ends_with(".js")
        || path.ends_with(".sh")
        || path.ends_with(".ps1")
        || path.ends_with(".yml")
        || path.ends_with(".yaml")
}

fn process_policy_patterns() -> Vec<String> {
    [
        concat!("use std::process::", "Command"),
        concat!("Command", "::new"),
        concat!("child", "_process"),
        concat!("cp.", "spawn"),
        concat!("cp.", "exec("),
        concat!("cp.", "execFile"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn network_policy_patterns() -> Vec<String> {
    [
        // Original patterns.
        concat!("https", ".get"),
        concat!("fetch", "("),
        concat!("req", "west"),
        concat!("u", "req"),
        concat!("Tcp", "Stream"),
        concat!("cu", "rl"),
        concat!("w", "get"),
        // Expanded patterns (#2412): cover common Rust/JS networking crates
        // that were previously invisible to the gate. Split with concat! so
        // the gate does not flag its own source (same technique as the
        // original patterns above).
        concat!("hy", "per"),
        concat!("isa", "hc"),
        concat!("atto", "httpc"),
        concat!("min", "req"),
        concat!("tokio::", "net"),
        concat!("std::net::", "Tcp"),
        concat!("to", "nic::"),
        concat!("tungste", "nite"),
        concat!("ssh", "2::"),
        concat!("req", "west::Client"),
        // Expanded patterns (#2903): UDP, web frameworks, HTTP/3, async
        // runtimes, and PowerShell web cmdlets that were still invisible.
        concat!("Udp", "Socket"),
        concat!("ax", "um"),
        concat!("act", "ix"),
        concat!("soc", "ket2"),
        concat!("qu", "inn"),
        concat!("async-", "std"),
        concat!("Invoke-", "WebRequest"),
        concat!("Invoke-", "RestMethod"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn shell_fetch_tool_name() -> &'static str {
    concat!("cu", "rl")
}

pub(crate) fn matches_any_glob(allowlist: &[GlobAllow], path: &str) -> bool {
    allowlist
        .iter()
        .any(|entry| glob_matches(&entry.glob, path))
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    glob_parts_match(&pattern_parts, &path_parts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StaticLanguageMatcher {
    Path(String),
    Glob(String),
}

impl StaticLanguageMatcher {
    fn as_str(&self) -> &str {
        match self {
            StaticLanguageMatcher::Path(value) | StaticLanguageMatcher::Glob(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticLanguageAllowEntry {
    matcher: StaticLanguageMatcher,
    owner: String,
    reason: String,
}

#[cfg(test)]
impl StaticLanguageAllowEntry {
    fn new_path(
        path: impl Into<String>,
        owner: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            matcher: StaticLanguageMatcher::Path(path.into()),
            owner: owner.into(),
            reason: reason.into(),
        }
    }

    fn new_glob(
        glob: impl Into<String>,
        owner: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            matcher: StaticLanguageMatcher::Glob(glob.into()),
            owner: owner.into(),
            reason: reason.into(),
        }
    }
}

const STATIC_LANGUAGE_ALLOWLIST_PATH: &str = ".ripr/static-language-allowlist.toml";
const STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH: &str = ".ripr/static-language-allowlist.txt";
const STATIC_LANGUAGE_ALLOWED_GLOBS: &[&str] = &["docs/*.md", "docs/**/*.md"];

fn parse_static_language_allowlist(text: &str) -> (Vec<StaticLanguageAllowEntry>, Vec<String>) {
    let mut entries: Vec<StaticLanguageAllowEntry> = Vec::new();
    let mut violations = Vec::new();
    let mut schema_seen = false;
    let mut current: Option<PendingAllowEntry> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[allow]]" {
            if let Some(pending) = current.take() {
                finalize_static_language_entry(pending, &mut entries, &mut violations);
            }
            current = Some(PendingAllowEntry::new(line_number));
            continue;
        }
        let Some((key, raw_value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} expected `key = value`"
            ));
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        if let Some(pending) = current.as_mut() {
            match key {
                "path" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.path = Some((parsed, line_number)),
                ),
                "glob" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.glob = Some((parsed, line_number)),
                ),
                "owner" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.owner = Some((parsed, line_number)),
                ),
                "reason" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.reason = Some((parsed, line_number)),
                ),
                _ => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} unsupported `[[allow]]` field `{key}`"
                )),
            }
        } else if key == "schema_version" {
            schema_seen = true;
            match raw_value.parse::<u32>() {
                Ok(1) => {}
                Ok(other) => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
                )),
                Err(_) => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} schema_version must be an integer literal"
                )),
            }
        } else {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} unsupported top-level field `{key}`"
            ));
        }
    }

    if let Some(pending) = current.take() {
        finalize_static_language_entry(pending, &mut entries, &mut violations);
    }

    if !schema_seen {
        violations.push(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH} is missing required `schema_version = 1` header"
        ));
    }

    let mut seen_matchers: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &entries {
        let matcher = entry.matcher.as_str();
        if let Some(&first) = seen_matchers.get(matcher) {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH} matcher `{matcher}` is duplicated (first declared near line {first})"
            ));
        } else {
            seen_matchers.insert(matcher, 0);
        }
    }

    (entries, violations)
}

struct PendingAllowEntry {
    block_line: usize,
    path: Option<(String, usize)>,
    glob: Option<(String, usize)>,
    owner: Option<(String, usize)>,
    reason: Option<(String, usize)>,
}

impl PendingAllowEntry {
    fn new(block_line: usize) -> Self {
        Self {
            block_line,
            path: None,
            glob: None,
            owner: None,
            reason: None,
        }
    }
}

fn assign_static_language_field<F>(
    raw_value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    mut assign: F,
) where
    F: FnMut(String),
{
    match parse_quoted_value(raw_value) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} {message}"
        )),
    }
}

fn finalize_static_language_entry(
    pending: PendingAllowEntry,
    entries: &mut Vec<StaticLanguageAllowEntry>,
    violations: &mut Vec<String>,
) {
    let block_line = pending.block_line;
    let path_value = pending.path;
    let glob_value = pending.glob;
    let owner_value = pending.owner;
    let reason_value = pending.reason;

    let matcher = match (path_value, glob_value) {
        (Some((path, line)), None) => match validate_static_language_path(&path, line) {
            Ok(()) => Some(StaticLanguageMatcher::Path(path)),
            Err(message) => {
                violations.push(message);
                None
            }
        },
        (None, Some((glob, line))) => match validate_static_language_glob(&glob, line) {
            Ok(()) => Some(StaticLanguageMatcher::Glob(glob)),
            Err(message) => {
                violations.push(message);
                None
            }
        },
        (Some(_), Some(_)) => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry has both `path` and `glob`; declare exactly one"
            ));
            None
        }
        (None, None) => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry must declare either `path` or `glob`"
            ));
            None
        }
    };

    let owner = match owner_value {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line} `owner` is blank; name a responsible team or maintainer"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry is missing required `owner`"
            ));
            None
        }
    };

    let reason = match reason_value {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line} `reason` is blank; explain why this matcher is exempt"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry is missing required `reason`"
            ));
            None
        }
    };

    if let (Some(matcher), Some(owner), Some(reason)) = (matcher, owner, reason) {
        entries.push(StaticLanguageAllowEntry {
            matcher,
            owner,
            reason,
        });
    }
}

fn validate_static_language_path(path: &str, line_number: usize) -> Result<(), String> {
    if path.is_empty() {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` is empty"
        ));
    }
    if path.contains('\\') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` uses backslashes; use `/` separators"
        ));
    }
    if is_absolute_path_like(path) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` is absolute; entries must be repository-relative"
        ));
    }
    if path.contains('*') || path.contains('?') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` contains glob characters; use `glob = ...` instead"
        ));
    }
    Ok(())
}

fn validate_static_language_glob(glob: &str, line_number: usize) -> Result<(), String> {
    if glob.is_empty() {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` is empty"
        ));
    }
    if glob.contains('\\') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` uses backslashes; use `/` separators"
        ));
    }
    if is_absolute_path_like(glob) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` is absolute; entries must be repository-relative"
        ));
    }
    if !STATIC_LANGUAGE_ALLOWED_GLOBS.contains(&glob) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` is not in the scoped set; current allowed globs: {}",
            STATIC_LANGUAGE_ALLOWED_GLOBS.join(", ")
        ));
    }
    Ok(())
}

fn is_absolute_path_like(value: &str) -> bool {
    if value.starts_with('/') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn load_static_language_allowlist() -> Result<Vec<StaticLanguageAllowEntry>, Vec<String>> {
    if Path::new(STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH).exists() {
        return Err(vec![format!(
            "{STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH} still exists; the static-language allowlist moved to {STATIC_LANGUAGE_ALLOWLIST_PATH}. Delete the legacy `.txt` file to avoid split-brain policy."
        )]);
    }
    let path = Path::new(STATIC_LANGUAGE_ALLOWLIST_PATH);
    let text = read_text_lossy(path).map_err(|err| vec![err])?;
    let (entries, mut violations) = parse_static_language_allowlist(&text);
    for entry in &entries {
        if let StaticLanguageMatcher::Path(value) = &entry.matcher
            && !Path::new(value).exists()
        {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH} matcher `{value}` does not exist on disk"
            ));
        }
    }
    if violations.is_empty() {
        Ok(entries)
    } else {
        Err(violations)
    }
}

fn static_language_allowlist_covers(allowlist: &[StaticLanguageAllowEntry], path: &str) -> bool {
    allowlist.iter().any(|entry| match &entry.matcher {
        StaticLanguageMatcher::Path(value) => value == path,
        StaticLanguageMatcher::Glob(value) => glob_matches(value, path),
    })
}

fn should_scan_static_language_path(allowlist: &[StaticLanguageAllowEntry], path: &str) -> bool {
    is_static_language_candidate(path) && !static_language_allowlist_covers(allowlist, path)
}

fn glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return glob_parts_match(&pattern[1..], path)
            || (!path.is_empty() && glob_parts_match(pattern, &path[1..]));
    }
    if path.is_empty() {
        return false;
    }
    segment_matches(pattern[0], path[0]) && glob_parts_match(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, value: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let value_chars = value.chars().collect::<Vec<_>>();
    segment_parts_match(&pattern_chars, &value_chars)
}

fn segment_parts_match(pattern: &[char], value: &[char]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == '*' {
        return segment_parts_match(&pattern[1..], value)
            || (!value.is_empty() && segment_parts_match(pattern, &value[1..]));
    }
    !value.is_empty() && pattern[0] == value[0] && segment_parts_match(&pattern[1..], &value[1..])
}

fn parse_git_stage_line(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let mode = parts.next()?;
    let _object_type = parts.next()?;
    let _hash = parts.next()?;
    let stage_and_path = line.split('\t').nth(1)?;
    Some((mode, stage_and_path))
}

fn extract_workflow_run_blocks(text: &str) -> Vec<RunBlock> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim_start();
        if let Some(rest) = workflow_run_value(trimmed) {
            let indent = line.len() - trimmed.len();
            let run_value = rest.trim();
            if run_value == "|" || run_value == ">" || run_value == "|-" || run_value == ">-" {
                let mut block_lines = Vec::new();
                let mut next_idx = idx + 1;
                while next_idx < lines.len() {
                    let next = lines[next_idx];
                    let next_trimmed = next.trim_start();
                    let next_indent = next.len() - next_trimmed.len();
                    if !next_trimmed.is_empty() && next_indent <= indent {
                        break;
                    }
                    block_lines.push(next_trimmed.to_string());
                    next_idx += 1;
                }
                let non_empty_lines = block_lines
                    .iter()
                    .filter(|value| !value.trim().is_empty())
                    .count();
                blocks.push(RunBlock {
                    line_number: idx + 1,
                    non_empty_lines,
                    text: block_lines.join("\n"),
                });
                idx = next_idx;
                continue;
            }
            blocks.push(RunBlock {
                line_number: idx + 1,
                non_empty_lines: usize::from(!run_value.is_empty()),
                text: run_value.to_string(),
            });
        }
        idx += 1;
    }
    blocks
}

fn workflow_run_value(trimmed_line: &str) -> Option<&str> {
    trimmed_line
        .strip_prefix("run:")
        .or_else(|| trimmed_line.strip_prefix("- run:"))
}

fn forbidden_static_terms() -> Vec<String> {
    ["killed", "survived", "untested", "proven", "adequate"]
        .iter()
        .map(|value| value.to_string())
        .collect()
}

/// Inline marker that suppresses static-language violations for a single line —
/// finer-grained than a whole-file allowlist entry. It MUST be followed by a
/// reason so the suppression is reviewable and not a silent drive-by:
///   `<comment-prefix> ripr-allow: static-language: <reason>`
const STATIC_LANGUAGE_INLINE_ALLOW_MARKER: &str = "ripr-allow: static-language:";

/// True when the (already-lowercased) line carries the inline-allow marker
/// followed by a non-empty reason. A bare marker with no reason does NOT
/// suppress, so an intentional allow always records why.
fn line_has_static_language_inline_allow(lower_line: &str) -> bool {
    match lower_line.find(STATIC_LANGUAGE_INLINE_ALLOW_MARKER) {
        Some(index) => {
            let after = &lower_line[index + STATIC_LANGUAGE_INLINE_ALLOW_MARKER.len()..];
            !after.trim().is_empty()
        }
        None => false,
    }
}

/// A plain-English synonym hint for a prohibited static-language term, used to
/// speed fixes when the term appears in prose/comments rather than analyzer
/// output. The gate scans all tracked prose, not only output strings, so a hit
/// is often an innocent English word that just needs a neutral synonym.
fn static_language_synonym_hint(term: &str) -> &'static str {
    match term {
        "killed" => "removed/terminated",
        "survived" => "persisted/remained",
        "untested" => "unexercised/uncovered",
        "proven" => "demonstrated/established",
        "adequate" => "sufficient/enough",
        _ => "a neutral synonym",
    }
}

/// Build an actionable static-language violation: `path:line`, the offending
/// snippet, and a fix hint. The hint names the common confusion — the gate
/// scans all tracked prose, not just analyzer output — so a hit in a comment or
/// doc is fixed with a plain synonym, while a hit in real output must use the
/// approved exposure vocabulary.
fn static_language_violation_message(
    path: &str,
    line_number: usize,
    term: &str,
    line: &str,
) -> String {
    let trimmed = line.trim();
    let snippet = if trimmed.chars().count() > 80 {
        let mut shortened: String = trimmed.chars().take(77).collect();
        shortened.push_str("...");
        shortened
    } else {
        trimmed.to_string()
    };
    let hint = static_language_synonym_hint(term);
    format!(
        "{path}:{line_number} prohibited static-language term `{term}` in `{snippet}` \
— this gate scans all tracked prose, not just analyzer output. In output, use the \
approved exposure vocabulary (exposed/weakly_exposed/reachable_unrevealed/no_static_path/\
infection_unknown/propagation_unknown/static_unknown). In comments or docs, use a plain \
synonym (e.g. {hint}). To intentionally allow this line, append \
`{STATIC_LANGUAGE_INLINE_ALLOW_MARKER} <reason>`."
    )
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "xtask test code uses unwrap/expect for fail-fast assertion. Production paths are receipted via policy/no-panic-allowlist.toml; the test scope is governed by this single module-level expect."
)]
mod tests;
