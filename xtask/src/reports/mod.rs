mod annotations;
mod badges;
mod dogfood;
mod first_pr;
mod fixtures;
mod impacted_evidence;
mod index;
mod lsp;
mod metrics;
mod mutation;
mod operator;
mod pr;
mod pr_evidence;
mod pr_evidence_summary;
mod receipts;
mod recommendation;
mod release;
mod repo;
mod review_comments;
mod sarif;
mod targeted_test;
mod test_oracles;

pub(crate) use annotations::ripr_annotations;
pub(crate) use badges::{
    badge_artifacts, check_badge_endpoints, repo_badge_artifacts, update_badge_endpoints,
};
pub(crate) use dogfood::dogfood;
pub(crate) use first_pr::first_pr;
pub(crate) use fixtures::{fixtures, golden_drift, goldens};
pub(crate) use impacted_evidence::impacted_evidence;
pub(crate) use index::{reports, reports_index};
pub(crate) use lsp::lsp_cockpit_report;
pub(crate) use metrics::metrics_report;
pub(crate) use mutation::mutation_calibration;
pub(crate) use operator::operator_cockpit_report;
pub(crate) use pr::{critic, gh_pr_status, pr_summary, pr_triage_report};
pub(crate) use pr_evidence::ripr_pr;
pub(crate) use pr_evidence_summary::ripr_pr_summary;
pub(crate) use receipts::{receipts, receipts_write};
pub(crate) use recommendation::recommendation_calibration;
pub(crate) use release::release_readiness;
pub(crate) use repo::{
    agent_seam_packets_report, evidence_health_report, evidence_quality_scorecard_report,
    evidence_quality_trend_report, lane1_evidence_audit_report, repo_exposure_latency_report,
    repo_exposure_report, repo_seam_inventory,
};
pub(crate) use review_comments::ripr_review_comments;
pub(crate) use sarif::sarif_policy;
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
