#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealEvidence {
    pub observe: StageEvidence,
    pub discriminate: StageEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiprEvidence {
    pub reach: StageEvidence,
    pub infect: StageEvidence,
    pub propagate: StageEvidence,
    pub reveal: RevealEvidence,
}

#[cfg(test)]
mod tests {
    use super::{Confidence, OracleKind, OracleStrength, StageEvidence, StageState};

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
}
