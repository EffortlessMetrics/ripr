pub(crate) mod agent_brief;
pub(crate) mod agent_receipt;
pub(crate) mod agent_seam_packets;
pub(crate) mod agent_workflow;
pub(crate) mod assistant_loop_health;
pub(crate) mod badge;
pub(crate) mod baseline;
pub(crate) mod baseline_delta;
pub(crate) mod baseline_update;
pub(crate) mod coverage_grip_frontier;
pub(crate) mod diff_report;
pub(crate) mod evidence_health;
pub(crate) mod evidence_record;
pub(crate) mod first_pr;
pub(crate) mod first_useful_action;
pub(crate) mod format;
pub(crate) mod gap_decision_ledger;
pub(crate) mod gate;
pub mod github;
pub mod human;
pub mod json;
pub(crate) mod markdown;
pub(crate) mod mutation_calibration;
pub(crate) mod outcome;
pub(crate) mod path;
pub(crate) mod pilot;
pub(crate) mod policy_history;
pub(crate) mod policy_operations;
pub(crate) mod policy_preview_promotion;
pub(crate) mod policy_promotion;
pub(crate) mod policy_readiness;
pub(crate) mod pr_evidence_ledger;
pub(crate) mod pr_inline_comment_publish_plan;
pub(crate) mod pr_review_front_panel;
pub(crate) mod preview_actionability;
pub(crate) mod python_repair_card;
pub mod receipt_lifecycle;
pub(crate) mod render;
pub(crate) mod repo_exposure;
pub(crate) mod repo_seams;
pub(crate) mod report_packet_index;
pub(crate) mod review_comments;
pub(crate) mod ripr_zero_status;
pub(crate) mod sarif;
pub mod start_here_state;
pub(crate) mod suppression_health;
pub(crate) mod suppressions;
pub(crate) mod swarm_ingest;
pub(crate) mod test_oracle_assistant_proof;
pub(crate) mod typescript_preview_card;
pub(crate) mod value_path;
pub(crate) mod waiver_aging;

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    pub(crate) fn repo_root() -> Result<PathBuf, String> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve repo root".to_string())
    }

    pub(crate) fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }
}
