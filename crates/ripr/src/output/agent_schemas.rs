//! Central registry of all agent-artifact schema versions.
//!
//! Each artifact type that serializes to JSON carries a `schema_version` field.
//! This module enumerates them in one place so the fragmentation is visible
//! and version drift can be detected by a single check (#2646).
//!
//! ## Current version landscape
//!
//! | Artifact          | Version | Constant                              |
//! |-------------------|---------|---------------------------------------|
//! | Seam packet       | 0.4     | `app::AGENT_SEAM_PACKET_SCHEMA_VERSION`     |
//! | Agent receipt     | 0.5     | `output::agent_receipt::AGENT_RECEIPT_SCHEMA_VERSION` |
//! | Agent brief       | 0.1     | `output::agent_brief::AGENT_BRIEF_SCHEMA_VERSION`     |
//! | Verify result     | 0.1     | `output::outcome::AGENT_VERIFY_SCHEMA_VERSION`        |
//! | Workflow manifest | 0.1     | `app::agent_workflow::AGENT_WORKFLOW_SCHEMA_VERSION`  |
//! | Agent status      | 0.1     | `app::agent_status::AGENT_STATUS_SCHEMA_VERSION`      |
//! | Review summary    | 0.1     | `agent_review_summary::AGENT_REVIEW_SUMMARY_SCHEMA_VERSION` |
//! | riprAgent         | 0.1     | `lsp::agent_protocol::RIPR_AGENT_SCHEMA_VERSION`      |
//! | Review receipt    | 0.1     | `output::review_comments_receipt::SCHEMA_VERSION`     |

/// Returns a sorted list of (artifact_name, version) pairs for diagnostic
/// and audit purposes. This is the single discovery point for all agent
/// artifact schema versions (#2646).
pub(crate) fn agent_artifact_schema_versions() -> Vec<(&'static str, &'static str)> {
    let mut versions = vec![
        ("seam_packet", crate::app::AGENT_SEAM_PACKET_SCHEMA_VERSION),
        (
            "agent_receipt",
            crate::output::agent_receipt::AGENT_RECEIPT_SCHEMA_VERSION,
        ),
        (
            "agent_brief",
            crate::output::agent_brief::AGENT_BRIEF_SCHEMA_VERSION,
        ),
        (
            "verify_result",
            crate::output::outcome::AGENT_VERIFY_SCHEMA_VERSION,
        ),
        (
            "workflow_manifest",
            crate::app::agent_workflow::AGENT_WORKFLOW_SCHEMA_VERSION,
        ),
        (
            "agent_status",
            crate::app::agent_status::AGENT_STATUS_SCHEMA_VERSION,
        ),
        (
            "review_summary",
            crate::app::agent_review_summary::types::AGENT_REVIEW_SUMMARY_SCHEMA_VERSION,
        ),
        (
            "review_receipt",
            crate::output::review_comments_receipt::REVIEW_COMMENTS_RECEIPT_SCHEMA_VERSION,
        ),
        (
            "ripr_agent",
            crate::lsp::agent_protocol::RIPR_AGENT_SCHEMA_VERSION,
        ),
    ];
    versions.sort_by(|a, b| a.0.cmp(&b.0));
    versions
}
