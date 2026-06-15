#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OracleKind {
    ExactValue,
    ExactErrorVariant,
    WholeObjectEquality,
    Snapshot,
    RelationalCheck,
    BroadError,
    SmokeOnly,
    MockExpectation,
    Unknown,
}

impl OracleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            OracleKind::ExactValue => "exact_value",
            OracleKind::ExactErrorVariant => "exact_error_variant",
            OracleKind::WholeObjectEquality => "whole_object_equality",
            OracleKind::Snapshot => "snapshot",
            OracleKind::RelationalCheck => "relational_check",
            OracleKind::BroadError => "broad_error",
            OracleKind::SmokeOnly => "smoke_only",
            OracleKind::MockExpectation => "mock_expectation",
            OracleKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OracleStrength {
    Strong,
    Medium,
    Weak,
    Smoke,
    None,
    Unknown,
}

impl OracleStrength {
    pub fn as_str(&self) -> &'static str {
        match self {
            OracleStrength::Strong => "strong",
            OracleStrength::Medium => "medium",
            OracleStrength::Weak => "weak",
            OracleStrength::Smoke => "smoke",
            OracleStrength::None => "none",
            OracleStrength::Unknown => "unknown",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            OracleStrength::Strong => 5,
            OracleStrength::Medium => 4,
            OracleStrength::Weak => 3,
            OracleStrength::Smoke => 2,
            OracleStrength::Unknown => 1,
            OracleStrength::None => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StageState {
    Yes,
    Weak,
    No,
    Unknown,
    Opaque,
    NotApplicable,
}

impl StageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageState::Yes => "yes",
            StageState::Weak => "weak",
            StageState::No => "no",
            StageState::Unknown => "unknown",
            StageState::Opaque => "opaque",
            StageState::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
            Confidence::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StageEvidence {
    pub state: StageState,
    pub confidence: Confidence,
    pub summary: String,
}

impl StageEvidence {
    pub fn new(state: StageState, confidence: Confidence, summary: impl Into<String>) -> Self {
        Self {
            state,
            confidence,
            summary: summary.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevealEvidence {
    pub observe: StageEvidence,
    pub discriminate: StageEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RiprEvidence {
    pub reach: StageEvidence,
    pub infect: StageEvidence,
    pub propagate: StageEvidence,
    pub reveal: RevealEvidence,
}

/// Why a test is related to a seam or probe. Single highest-priority reason
/// per test. The diff-check path (`analysis/classify/related_tests.rs`) tags
/// each related test with the reason it matched so consumers can filter weak
/// matches. The repo-exposure path (`analysis/test_grip_evidence.rs`) uses the
/// same enum for richer grip-evidence ranking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationReason {
    DirectOwnerCall,
    HelperOwnerCall,
    AssertionTargetAffinity,
    SameTestFile,
    SameModule,
    OwnerNamedTest,
    ImportPathAffinity,
    FixtureOwnerAffinity,
    /// Matched via probe-token substring in file path or test name.
    /// This signal is deliberately broad — a short token can match many
    /// unrelated tests. Consumers should treat `relation_confidence: low`
    /// as advisory only.
    WeakTokenSubstring,
    /// Matched through a single-hop explicit re-export chain resolved
    /// entirely from in-source `export { N } from './A'` statements.
    /// The test imports the name from an intermediate re-exporting file;
    /// one hop was followed to verify the chain leads to the changed owner.
    /// Two-hop and deeper chains are NOT followed (fail-closed).
    ReExportChainFollowed,
}

impl RelationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectOwnerCall => "direct_owner_call",
            Self::HelperOwnerCall => "helper_owner_call",
            Self::AssertionTargetAffinity => "assertion_target_affinity",
            Self::SameTestFile => "same_test_file",
            Self::SameModule => "same_module",
            Self::OwnerNamedTest => "owner_named_test",
            Self::ImportPathAffinity => "import_path_affinity",
            Self::FixtureOwnerAffinity => "fixture_owner_affinity",
            Self::WeakTokenSubstring => "weak_token_substring",
            Self::ReExportChainFollowed => "re_export_chain_followed",
        }
    }

    /// Sort order: lower value sorts first (highest-priority reason first).
    pub fn priority(self) -> u8 {
        match self {
            Self::DirectOwnerCall => 0,
            Self::HelperOwnerCall => 1,
            Self::AssertionTargetAffinity => 2,
            Self::SameTestFile => 3,
            Self::SameModule => 4,
            Self::OwnerNamedTest => 5,
            Self::ImportPathAffinity => 6,
            // Re-export chain follows: same priority as import-path affinity —
            // the inference is explicit in-source but involves one hop of indirection.
            Self::ReExportChainFollowed => 6,
            Self::FixtureOwnerAffinity => 7,
            Self::WeakTokenSubstring => 8,
        }
    }

    /// Derive the matching confidence for this reason.
    pub fn confidence(self) -> RelationConfidence {
        match self {
            Self::DirectOwnerCall | Self::HelperOwnerCall => RelationConfidence::High,
            Self::AssertionTargetAffinity
            | Self::SameTestFile
            | Self::SameModule
            | Self::OwnerNamedTest
            | Self::ImportPathAffinity
            // Re-export tracing is medium confidence: it is explicit in-source but
            // one hop of indirection means ripr cannot see deeper aliasing.
            | Self::ReExportChainFollowed => RelationConfidence::Medium,
            Self::FixtureOwnerAffinity | Self::WeakTokenSubstring => RelationConfidence::Low,
        }
    }
}

/// Confidence that a related test grips the seam. Independent of
/// oracle strength: a `Low` relation can still carry a strong oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationConfidence {
    High,
    Medium,
    Low,
    Opaque,
}

impl RelationConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Opaque => "opaque",
        }
    }

    /// Sort rank: lower value sorts first (highest confidence first).
    pub fn rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
            Self::Opaque => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Confidence, OracleKind, OracleStrength, RelationConfidence, RelationReason, StageEvidence,
        StageState,
    };

    #[test]
    fn oracle_kind_labels_are_stable_contract_terms() {
        let cases = [
            (OracleKind::ExactValue, "exact_value"),
            (OracleKind::ExactErrorVariant, "exact_error_variant"),
            (OracleKind::WholeObjectEquality, "whole_object_equality"),
            (OracleKind::Snapshot, "snapshot"),
            (OracleKind::RelationalCheck, "relational_check"),
            (OracleKind::BroadError, "broad_error"),
            (OracleKind::SmokeOnly, "smoke_only"),
            (OracleKind::MockExpectation, "mock_expectation"),
            (OracleKind::Unknown, "unknown"),
        ];

        for (kind, label) in cases {
            assert_eq!(kind.as_str(), label);
        }
    }

    #[test]
    fn oracle_strength_labels_and_ranks_are_stable_contract_terms() {
        let cases = [
            (OracleStrength::Strong, "strong", 5),
            (OracleStrength::Medium, "medium", 4),
            (OracleStrength::Weak, "weak", 3),
            (OracleStrength::Smoke, "smoke", 2),
            (OracleStrength::Unknown, "unknown", 1),
            (OracleStrength::None, "none", 0),
        ];

        for (strength, label, rank) in cases {
            assert_eq!(strength.as_str(), label);
            assert_eq!(strength.rank(), rank);
        }
    }

    #[test]
    fn stage_state_and_confidence_labels_are_stable_contract_terms() {
        let stage_states = [
            (StageState::Yes, "yes"),
            (StageState::Weak, "weak"),
            (StageState::No, "no"),
            (StageState::Unknown, "unknown"),
            (StageState::Opaque, "opaque"),
            (StageState::NotApplicable, "not_applicable"),
        ];
        for (state, label) in stage_states {
            assert_eq!(state.as_str(), label);
        }

        let confidences = [
            (Confidence::High, "high"),
            (Confidence::Medium, "medium"),
            (Confidence::Low, "low"),
            (Confidence::Unknown, "unknown"),
        ];
        for (confidence, label) in confidences {
            assert_eq!(confidence.as_str(), label);
        }
    }

    #[test]
    fn stage_evidence_new_sets_all_fields() {
        let evidence = StageEvidence::new(StageState::Weak, Confidence::Medium, "summary");
        assert_eq!(evidence.state, StageState::Weak);
        assert_eq!(evidence.confidence, Confidence::Medium);
        assert_eq!(evidence.summary, "summary");
    }

    #[test]
    fn relation_reason_labels_are_stable_contract_terms() {
        let cases = [
            (RelationReason::DirectOwnerCall, "direct_owner_call"),
            (RelationReason::HelperOwnerCall, "helper_owner_call"),
            (
                RelationReason::AssertionTargetAffinity,
                "assertion_target_affinity",
            ),
            (RelationReason::SameTestFile, "same_test_file"),
            (RelationReason::SameModule, "same_module"),
            (RelationReason::OwnerNamedTest, "owner_named_test"),
            (RelationReason::ImportPathAffinity, "import_path_affinity"),
            (
                RelationReason::FixtureOwnerAffinity,
                "fixture_owner_affinity",
            ),
            (RelationReason::WeakTokenSubstring, "weak_token_substring"),
            (
                RelationReason::ReExportChainFollowed,
                "re_export_chain_followed",
            ),
        ];
        for (reason, label) in cases {
            assert_eq!(reason.as_str(), label);
        }
    }

    #[test]
    fn relation_confidence_labels_are_stable_contract_terms() {
        let cases = [
            (RelationConfidence::High, "high"),
            (RelationConfidence::Medium, "medium"),
            (RelationConfidence::Low, "low"),
            (RelationConfidence::Opaque, "opaque"),
        ];
        for (confidence, label) in cases {
            assert_eq!(confidence.as_str(), label);
        }
    }
}
