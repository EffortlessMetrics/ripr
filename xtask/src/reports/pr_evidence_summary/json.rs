//! Compatibility forwarding for the shared `ripr` PR-evidence projection.
//!
//! The binary and xtask routes must not maintain separate completeness,
//! outcome, JSON, or Markdown logic. Keep this module as a small forwarding
//! surface so the compatibility route remains explicit.

pub(super) use ripr::app::pr_summary::{
    build_pr_evidence_summary, render_pr_evidence_summary_json,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_route_preserves_incomplete_markdown_projection() {
        let diff = serde_json::json!({
            "run_status": "complete",
            "summary": {"changed_files": 2},
            "analysis_outcome": {
                "analysis_complete": false,
                "outcome": {
                    "kind": "unsupported_input",
                    "limitations": [{
                        "kind": "unresolved_conflict_markers",
                        "recovery": {"kind": "resolve_conflicts"}
                    }]
                }
            }
        });

        let summary = build_pr_evidence_summary(None, None, None, Some(&diff), None, None);
        let json = render_pr_evidence_summary_json(&summary);
        let markdown = ripr::app::pr_summary::render_evidence_summary_md(&summary);

        assert!(json.contains("\"analysis_complete\": false"));
        assert!(markdown.contains("**Analysis Complete**: `false`"));
        assert!(markdown.contains("**Analysis Outcome**: `unsupported_input`"));
    }

    #[test]
    fn compatibility_route_does_not_promote_repo_status_without_diff_artifact() {
        let repo_exposure = serde_json::json!({
            "run_status": "complete",
            "limitations": []
        });
        let summary = build_pr_evidence_summary(None, None, Some(&repo_exposure), None, None, None);
        let json = render_pr_evidence_summary_json(&summary);

        assert!(json.contains("\"run_status\": \"unknown\""));
        assert!(!json.contains("\"run_status\": \"complete\""));
        assert!(!json.contains("\"analysis_complete\": true"));
    }
}
