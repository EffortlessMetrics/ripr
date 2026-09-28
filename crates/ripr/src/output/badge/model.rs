use std::collections::BTreeMap;

use crate::analysis_outcome::AnalysisOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    /// Counts unsuppressed static exposure gaps only.
    Ripr,
    /// Counts unsuppressed exposure gaps plus unsuppressed actionable
    /// test-efficiency findings (excluding declared intent).
    RiprPlus,
}

impl BadgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
            BadgeKind::RiprPlus => "ripr_plus",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
            BadgeKind::RiprPlus => "ripr+",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeStatus {
    Pass,
    Warn,
    Fail,
}

impl BadgeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeStatus::Pass => "pass",
            BadgeStatus::Warn => "warn",
            BadgeStatus::Fail => "fail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeBasis {
    /// Counts legacy diff/repo `Finding` exposure classes.
    FindingExposure,
    /// Counts unresolved actionable canonical repair items.
    CanonicalActionableGap,
    /// Counts classified repo seams using configured seam severity.
    #[cfg(test)]
    SeamNative,
    /// Counts explicit policy-targeted `GapRecord` projection targets.
    GapDecisionLedger,
}

impl BadgeBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeBasis::FindingExposure => "finding_exposure",
            BadgeBasis::CanonicalActionableGap => "canonical_actionable_gap",
            #[cfg(test)]
            BadgeBasis::SeamNative => "seam_native",
            BadgeBasis::GapDecisionLedger => "gap_decision_ledger",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeCounts {
    pub unsuppressed_exposure_gaps: usize,
    pub unsuppressed_test_efficiency_findings: usize,
    pub intentional_test_efficiency_findings: usize,
    pub suppressed_exposure_gaps: usize,
    pub suppressed_test_efficiency_findings: usize,
    pub unknowns: usize,
    pub unknowns_test_efficiency: usize,
    pub analyzed_findings: usize,
    pub analyzed_seams: usize,
    pub analyzed_gap_records: usize,
    pub analyzed_tests: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgePolicy {
    pub include_unknowns: bool,
    pub fail_on_nonzero: bool,
    pub test_intent_path: String,
    pub suppressions_path: String,
}

impl Default for BadgePolicy {
    fn default() -> Self {
        Self {
            include_unknowns: false,
            fail_on_nonzero: false,
            test_intent_path: ".ripr/test_intent.toml".to_string(),
            suppressions_path: ".ripr/suppressions.toml".to_string(),
        }
    }
}

/// Whether a badge represents the changed-behavior diff under analysis
/// or the full-repo baseline. Diff-scoped badges feed PR step summaries
/// and PR artifact uploads; only repo-scoped badges are safe as
/// README / store / public Shields endpoints because a no-diff `main`
/// run of the diff-scoped path always reports `0` regardless of the
/// repo's actual exposure profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeScope {
    Diff,
    Repo,
}

impl BadgeScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            BadgeScope::Diff => "diff",
            BadgeScope::Repo => "repo",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeSummary {
    pub kind: BadgeKind,
    pub scope: BadgeScope,
    pub basis: BadgeBasis,
    pub message: String,
    pub status: BadgeStatus,
    pub color: &'static str,
    pub counts: BadgeCounts,
    pub reason_counts: BTreeMap<&'static str, usize>,
    pub policy: BadgePolicy,
    /// Advisory warnings surfaced to the badge consumer — currently
    /// expired suppressions and unmatched suppression selectors. Empty
    /// for the common-case green badge.
    pub warnings: Vec<String>,
    /// Preview-language adapters that were detected in the diff but NOT
    /// enabled. Non-empty means the badge is NOT a clean Rust-grade result:
    /// those files were silently skipped. Consumers MUST treat a non-empty
    /// list as a honesty signal even when `status` is `"warn"` rather
    /// than `"pass"`, because the downgrade from pass to warn already
    /// fired. Empty for clean Rust-only diffs and for diffs where the
    /// preview adapter was enabled.
    pub preview_skipped: Vec<String>,
    /// The public badge projection (RIPR-SPEC-0066). `Some` only for
    /// repo-scoped public badges (canonical-actionable-gap or
    /// gap-decision-ledger basis); `None` for diff-scoped and internal
    /// badges, whose native JSON is unchanged. When present it carries the
    /// closed public state and the six required sidecar fields, and the
    /// summary's `message` / `status` / `color` are projected from it.
    pub projection: Option<super::public_projection::PublicBadgeProjection>,
    /// Typed diff completeness and limitation facts. `None` is expected for
    /// repo-scoped badges that have no diff denominator.
    pub analysis_outcome: Option<AnalysisOutcome>,
}

/// The schema_version of the native badge JSON. Bumping it is a public
/// contract change — call it out in the PR. v0.4 added
/// `basis = "gap_decision_ledger"` and `counts.analyzed_gap_records`
/// so public badge endpoints can be rendered from explicit GapRecord
/// policy targets. v0.5 adds `basis = "canonical_actionable_gap"` for
/// public repair-item badge projection. v0.6 adds `preview_skipped`
/// so consumers can detect when a preview-language diff was not analyzed
/// and the badge result is not a clean Rust-grade result. v0.7 adds the
/// `public_projection` object (RIPR-SPEC-0066): the closed public badge
/// state plus the `run_status`, `generated_at`, `actionable_count`,
/// `limited_reason`, `stale_age_secs`, and `source_report` sidecar fields,
/// present only on repo-scoped public badges. v0.8 adds the typed diff
/// analysis outcome and completeness state for diff-scoped badges.
pub const BADGE_SCHEMA_VERSION: &str = "0.8";

/// All test-efficiency reason strings the badge JSON reports as zero
/// defaults until later PRs read the test-efficiency report. The order
/// matches `RIPR-SPEC-0004` and the existing emitter in `xtask`.
pub(super) const BADGE_REASON_KEYS: &[&str] = &[
    "no_assertion_detected",
    "smoke_oracle_only",
    "relational_oracle",
    "broad_oracle",
    "assertion_may_not_match_detected_owner",
    "opaque_helper_or_fixture_boundary",
    "no_activation_literal_detected",
    "expected_value_computed_from_detected_owner_path",
    "duplicate_activation_and_oracle_shape",
];
