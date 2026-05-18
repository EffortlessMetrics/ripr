use std::collections::BTreeMap;

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
    /// Counts classified repo seams using configured seam severity.
    SeamNative,
    /// Counts explicit policy-targeted `GapRecord` projection targets.
    GapDecisionLedger,
}

impl BadgeBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeBasis::FindingExposure => "finding_exposure",
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
}

/// The schema_version of the native badge JSON. Bumping it is a public
/// contract change — call it out in the PR. v0.4 added
/// `basis = "gap_decision_ledger"` and `counts.analyzed_gap_records`
/// so public badge endpoints can be rendered from explicit GapRecord
/// policy targets.
pub const BADGE_SCHEMA_VERSION: &str = "0.4";

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
