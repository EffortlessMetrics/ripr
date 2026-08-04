mod annotations;
mod badges;
mod bun;
mod candidate_control;
mod ci_budget;
mod dogfood;
mod eval_sweep;
mod first_pr;
mod fixtures;
mod impacted_evidence;
mod index;
mod issue_intake;
mod lsp;
mod metrics;
mod module_health;
mod mutation;
mod operator;
mod pr;
mod pr_causal_delta;
mod pr_evidence;
mod pr_evidence_summary;
mod proof_preflight;
mod proof_route;
mod receipts;
mod recommendation;
mod release;
mod release_control;
mod release_denominator;
mod release_scope;
pub(crate) mod release_server;
mod repo;
mod review_comments;
mod rust_repair_trust;
mod sarif;
mod targeted_rerun;
mod targeted_test;
mod test_oracles;

pub(crate) use annotations::ripr_annotations;
#[cfg(test)]
pub(crate) use badges::{
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
pub(crate) use badges::{
    badge_artifacts, badge_basis, check_badge_diff_policy, check_badge_endpoints,
    repo_badge_artifacts, ripr_plus, update_badge_endpoints,
};
pub(crate) use bun::{bun_ub_calibration, bun_ub_preview_summary, configured_bridge_inventory};
pub(crate) use ci_budget::ci_budget;
pub(crate) use dogfood::dogfood;
pub(crate) use eval_sweep::eval_sweep;
pub(crate) use first_pr::first_pr;
pub(crate) use fixtures::{
    FixtureCheckFormat, fixture_dirs, fixtures, fixtures_with_args, golden_drift, goldens,
    goldens_check, is_manifest_only_fixture_dir, normalize_fixture_human_output,
    normalize_fixture_json_output, ripr_fixture_binary, run_fixture_check, yes_no,
};
#[cfg(test)]
pub(crate) use fixtures::{
    GoldenDriftEntry, GoldenDriftSemantics, first_line_difference, fixture_contract_violations,
    golden_assistant_loop_health_contract_violations_at, golden_drift_semantics, golden_drift_type,
    goldens_check_failure_message, json_string_values_for_key, normalize_golden_text, parse_reason,
    validate_bless_reason,
};
pub(crate) use impacted_evidence::impacted_evidence;
pub(crate) use index::{reports, reports_index};
pub(crate) use issue_intake::issue_intake;
pub(crate) use lsp::lsp_cockpit_report;
pub(crate) use metrics::metrics_report;
pub(crate) use module_health::module_health;
pub(crate) use mutation::mutation_calibration;
#[cfg(test)]
pub(crate) use mutation::{
    MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT, MutationOutcomeRecord,
    build_mutation_calibration_report, mutation_calibration_report_json,
    mutation_calibration_report_markdown, parse_mutation_calibration_args,
    parse_mutation_outcomes_json, read_mutation_input_json,
};
pub(crate) use operator::operator_cockpit_report;
pub(crate) use pr::{critic, gh_pr_status, pr_summary, pr_triage_report};
pub(crate) use pr_evidence::ripr_pr;
pub(crate) use pr_evidence_summary::ripr_pr_summary;
pub(crate) use proof_route::{pr_summary_proof_route_section, proof};
pub(crate) use receipts::{receipts, receipts_write};
pub(crate) use recommendation::recommendation_calibration;
pub(crate) use release::release_readiness;
pub(crate) use release_control::release_control;
pub(crate) use release_denominator::release_denominator;
pub(crate) use release_scope::release_scope;
pub(crate) use repo::{
    actionable_gap_outcomes_report, agent_seam_packets_report, evidence_health_report,
    evidence_quality_scorecard_report, evidence_quality_trend_report, lane1_evidence_audit_report,
    repo_exposure_latency_report, repo_exposure_report, repo_exposure_summary_report,
    repo_seam_inventory,
};
pub(crate) use review_comments::ripr_review_comments;
pub(crate) use rust_repair_trust::rust_repair_trust_report;
pub(crate) use sarif::sarif_policy;
#[cfg(test)]
pub(crate) use sarif::{
    SarifMissingBaseline, SarifPolicyMode, SarifPolicyResult, SarifPolicyThreshold,
    build_sarif_policy_report, parse_sarif_policy_args, parse_sarif_policy_results,
    sarif_policy_report_json, sarif_policy_report_markdown,
};
pub(crate) use targeted_rerun::targeted_rerun_benchmark;
pub(crate) use targeted_test::targeted_test_outcome;
pub(crate) use test_oracles::{test_efficiency_report, test_oracle_report};

fn ensure_parent_dir(path: &std::path::Path, label: &str) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("{label} has no parent directory"));
    };
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {} parent: {err}", parent.display()))
}

fn write_parented_file(
    path: &std::path::Path,
    label: &str,
    contents: impl AsRef<[u8]>,
) -> Result<(), String> {
    ensure_parent_dir(path, label)?;
    std::fs::write(path, contents).map_err(|err| format!("failed to write {label}: {err}"))
}
