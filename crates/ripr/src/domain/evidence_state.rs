//! Consumer-facing evidence state for a canonical review gap.
//!
//! This is a presentation decision over analyzer facts, separate from the
//! producer's `AnalysisOutcome` and its limitation taxonomy.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceState {
    Actionable,
    AlreadyObserved,
    InternalOnly,
    StaticLimitation,
    Unknown,
}

impl EvidenceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Actionable => "actionable",
            Self::AlreadyObserved => "already_observed",
            Self::InternalOnly => "internal_only",
            Self::StaticLimitation => "static_limitation",
            Self::Unknown => "unknown",
        }
    }

    pub const fn is_actionable(self) -> bool {
        matches!(self, Self::Actionable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_states_keep_the_existing_words_and_actionability_boundary() {
        let cases = [
            (EvidenceState::Actionable, "actionable"),
            (EvidenceState::AlreadyObserved, "already_observed"),
            (EvidenceState::InternalOnly, "internal_only"),
            (EvidenceState::StaticLimitation, "static_limitation"),
            (EvidenceState::Unknown, "unknown"),
        ];
        for (state, wire) in cases {
            assert_eq!(state.as_str(), wire);
        }
        assert!(EvidenceState::Actionable.is_actionable());
        assert!(!EvidenceState::Unknown.is_actionable());
        assert!(!EvidenceState::StaticLimitation.is_actionable());
    }
}
