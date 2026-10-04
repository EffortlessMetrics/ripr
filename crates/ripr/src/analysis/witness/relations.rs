use serde::{Deserialize, Serialize};

use crate::domain::RelationReason;

/// Candidate or established test relation projected from producer facts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(in crate::analysis) struct RelationWitness {
    pub(in crate::analysis) relation_reason: String,
    pub(in crate::analysis) relation_confidence: String,
    pub(in crate::analysis) oracle_kind: String,
    pub(in crate::analysis) oracle_strength: String,
    pub(in crate::analysis) status: RelationStatus,
    pub(in crate::analysis) has_test_target: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::analysis) enum RelationStatus {
    Established,
    Candidate,
}

impl RelationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Candidate => "candidate",
        }
    }
}

impl RelationWitness {
    pub(in crate::analysis::witness) fn semantic_identity(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.status.as_str(),
            self.relation_reason,
            self.relation_confidence,
            self.oracle_kind,
            self.oracle_strength
        )
    }
}

pub(in crate::analysis::witness) fn established_reason(reason: RelationReason) -> bool {
    matches!(
        reason,
        RelationReason::DirectOwnerCall | RelationReason::HelperOwnerCall
    )
}
