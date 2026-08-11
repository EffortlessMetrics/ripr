//! Central registry of agent repair-loop artifact schema versions (#2646-A,
//! #2973).
//!
//! Every artifact an agent consumes across the repair loop (brief, seam
//! packet, verify, receipt, and their workflow/status companions) declares
//! its version here exactly once. The owning emitter modules re-export these
//! constants, so this module is the single definition site: a version bump
//! is one edit, and the compatibility landscape is reviewable in one place.
//!
//! Changelog discipline: when a version below changes, extend its doc
//! comment with the bump reason and the governing issue or spec. Never
//! re-mean an existing version.

/// `ripr agent brief` artifact (`agent_brief.rs`).
///
/// - `0.1`: initial versioned brief; unchanged since introduction.
pub(crate) const AGENT_BRIEF_SCHEMA_VERSION: &str = "0.1";

/// `ripr agent packet` / seam-packet artifact (`app.rs`,
/// `output/agent_seam_packets.rs`).
///
/// - `0.4`: packets preserve typed analysis outcomes (#2897).
pub(crate) const AGENT_SEAM_PACKET_SCHEMA_VERSION: &str = "0.4";

/// `ripr agent verify` JSON (`output/outcome/mod.rs`).
///
/// - `0.2`: added the artifact content-commitment binding
///   (`inputs.before_content_sha256` / `inputs.after_content_sha256`) so a
///   verify result is bound to the exact artifact bytes it compared (#2922
///   PR B); the receipt path fails closed on older or newer schema versions.
/// - `0.3`: corrected the pair-level `artifact_currentness` value domain —
///   mixed pairs were mislabeled `dirty_worktree`; the closed vocabulary now
///   names each side (#3027). A consumer dispatching on 0.2 values breaks,
///   so this is a breaking family change: 0.2 documents keep their own
///   version and are NOT current evidence — there is no migration path.
///   Verify results also bind artifact bytes and reject replayed, stale, or
///   tampered chains (#3045).
pub(crate) const AGENT_VERIFY_SCHEMA_VERSION: &str = "0.3";

/// `ripr agent receipt` JSON (`output/agent_receipt.rs`). Distinct from the
/// RIPR-SPEC-0079 `ripr receipt write/check` artifact, whose version lives at
/// `app::receipt::RECEIPT_SCHEMA_VERSION`.
///
/// - `0.5`: receipts preserve typed analysis outcomes (#2895) and drop the
///   invariant-false `safe_to_merge` field (#2595).
pub(crate) const AGENT_RECEIPT_SCHEMA_VERSION: &str = "0.5";

/// `ripr agent workflow` artifact (`app/agent_workflow.rs`).
///
/// - `0.1`: initial versioned workflow manifest; unchanged since
///   introduction.
pub(crate) const AGENT_WORKFLOW_SCHEMA_VERSION: &str = "0.1";

/// `ripr agent status` JSON (`app/agent_status.rs`).
///
/// - `0.1`: initial versioned status artifact; unchanged since introduction.
pub(crate) const AGENT_STATUS_SCHEMA_VERSION: &str = "0.1";

/// Targeted test-outcome evidence JSON consumed by `agent verify`
/// (`output/outcome/mod.rs`).
///
/// - `0.1`: initial versioned outcome artifact; unchanged since introduction.
pub(crate) const TARGETED_TEST_OUTCOME_SCHEMA_VERSION: &str = "0.1";

/// Schema version of the repo-exposure artifact identity envelope
/// (`agent/artifact.rs`, RIPR-SPEC-0134).
///
/// - `1`: initial envelope identity version; the analysis input-identity
///   algorithm is versioned separately (`artifact::INPUT_IDENTITY_VERSION`).
pub(crate) const ARTIFACT_IDENTITY_SCHEMA_VERSION: &str = "1";

/// Full-repo exposure JSON (`output/repo_exposure.rs`) — the evidence
/// artifact `agent verify` consumes and validates.
///
/// - `0.3`: current repo-exposure shape; the artifact envelope above binds
///   the analysis-input identity separately.
pub(crate) const REPO_EXPOSURE_SCHEMA_VERSION: &str = "0.3";

/// Repo-exposure summary JSON (`output/repo_exposure.rs`) — the bounded
/// summary surface derived from the full exposure artifact.
///
/// - `0.1`: initial versioned summary; unchanged since introduction.
pub(crate) const REPO_EXPOSURE_SUMMARY_SCHEMA_VERSION: &str = "0.1";

/// `ripr agent review-summary` JSON (`app/agent_review_summary/types.rs`).
///
/// - `0.1`: initial versioned review summary; unchanged since introduction.
pub(crate) const AGENT_REVIEW_SUMMARY_SCHEMA_VERSION: &str = "0.1";

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the current registry landscape so a version bump is a deliberate,
    /// reviewable diff in one place (#2646-A).
    #[test]
    fn agent_artifact_schema_versions_match_the_registry() {
        let expected = [
            ("agent_brief", AGENT_BRIEF_SCHEMA_VERSION, "0.1"),
            ("agent_seam_packet", AGENT_SEAM_PACKET_SCHEMA_VERSION, "0.4"),
            ("agent_verify", AGENT_VERIFY_SCHEMA_VERSION, "0.3"),
            ("agent_receipt", AGENT_RECEIPT_SCHEMA_VERSION, "0.5"),
            ("agent_workflow", AGENT_WORKFLOW_SCHEMA_VERSION, "0.1"),
            ("agent_status", AGENT_STATUS_SCHEMA_VERSION, "0.1"),
            (
                "targeted_test_outcome",
                TARGETED_TEST_OUTCOME_SCHEMA_VERSION,
                "0.1",
            ),
            ("artifact_identity", ARTIFACT_IDENTITY_SCHEMA_VERSION, "1"),
            ("repo_exposure", REPO_EXPOSURE_SCHEMA_VERSION, "0.3"),
            (
                "repo_exposure_summary",
                REPO_EXPOSURE_SUMMARY_SCHEMA_VERSION,
                "0.1",
            ),
            (
                "agent_review_summary",
                AGENT_REVIEW_SUMMARY_SCHEMA_VERSION,
                "0.1",
            ),
        ];
        for (artifact, actual, pinned) in expected {
            assert_eq!(actual, pinned, "{artifact} schema version drifted");
        }
    }
}
