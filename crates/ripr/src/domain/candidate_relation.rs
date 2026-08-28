//! Candidate relation vocabulary for requirement-aware test candidate
//! discovery (#1664 / #1680 / #1784).
//!
//! This vocabulary is distinct from [`crate::domain::RelationReason`]:
//! `RelationReason` classifies *evidence* for an existing related test;
//! `CandidateRelation` classifies the *relation channel* through which a
//! test candidate was discovered for a requirement seam. The two are
//! related but serve different consumers — do not unify them
//! (AGENTS.md:237-238: different taxonomies, different edge policies).
//!
//! Producer-backed relations (the first group) may be **preferred** as
//! evidence candidates. Proximity/coincidence relations (the second
//! group) are **context-only** — they may be returned for navigation
//! but must never be preferred as evidence.

/// The closed candidate-relation vocabulary.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRelation {
    // --- Producer-backed (may be preferred as evidence) ---
    /// The test directly calls the changed owner function.
    DirectOwnerCall,
    /// The test reaches the changed owner through an established
    /// one-hop helper chain resolved from source.
    EstablishedOneHopHelperChain,
    /// The test is a recognized entrypoint (e.g. integration test,
    /// public API test) that exercises the changed owner.
    EstablishedEntrypointRelation,
    /// The test owns a fixture or golden that the changed owner
    /// produces or consumes.
    EstablishedFixtureOrGoldenOwner,
    /// An existing requirement-evidence edge links this test to the
    /// seam (from the traceability graph, #1678).
    ExistingRequirementEvidenceEdge,
    /// A configured static-checker rule identifies this test as
    /// relevant (e.g. a test naming convention mapping).
    ConfiguredStaticCheckerRule,

    // --- Proximity / coincidence (context-only, never preferred) ---
    /// The test is in the same file as the changed owner but has no
    /// established call/relation chain.
    FileProximityOnly,
    /// The test name contains a token that coincidentally matches the
    /// changed owner's name (the false-exposed family).
    TokenCoincidenceOnly,
    /// The test exercises an opaque helper or macro whose internals
    /// cannot be resolved statically.
    OpaqueHelperOrMacro,
    /// The relation channel is unknown.
    Unknown,
}

impl CandidateRelation {
    /// Returns `true` when this relation is producer-backed and may be
    /// preferred as evidence.
    pub fn is_producer_backed(self) -> bool {
        matches!(
            self,
            CandidateRelation::DirectOwnerCall
                | CandidateRelation::EstablishedOneHopHelperChain
                | CandidateRelation::EstablishedEntrypointRelation
                | CandidateRelation::EstablishedFixtureOrGoldenOwner
                | CandidateRelation::ExistingRequirementEvidenceEdge
                | CandidateRelation::ConfiguredStaticCheckerRule
        )
    }

    /// Returns `true` when this relation is proximity/coincidence-based
    /// and must never be preferred as evidence.
    pub fn is_context_only(self) -> bool {
        matches!(
            self,
            CandidateRelation::FileProximityOnly
                | CandidateRelation::TokenCoincidenceOnly
                | CandidateRelation::OpaqueHelperOrMacro
                | CandidateRelation::Unknown
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CandidateRelation::DirectOwnerCall => "direct_owner_call",
            CandidateRelation::EstablishedOneHopHelperChain => "established_one_hop_helper_chain",
            CandidateRelation::EstablishedEntrypointRelation => "established_entrypoint_relation",
            CandidateRelation::EstablishedFixtureOrGoldenOwner => {
                "established_fixture_or_golden_owner"
            }
            CandidateRelation::ExistingRequirementEvidenceEdge => {
                "existing_requirement_evidence_edge"
            }
            CandidateRelation::ConfiguredStaticCheckerRule => "configured_static_checker_rule",
            CandidateRelation::FileProximityOnly => "file_proximity_only",
            CandidateRelation::TokenCoincidenceOnly => "token_coincidence_only",
            CandidateRelation::OpaqueHelperOrMacro => "opaque_helper_or_macro",
            CandidateRelation::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_backed_relations_are_preferred() {
        assert!(CandidateRelation::DirectOwnerCall.is_producer_backed());
        assert!(CandidateRelation::EstablishedOneHopHelperChain.is_producer_backed());
        assert!(CandidateRelation::EstablishedEntrypointRelation.is_producer_backed());
        assert!(CandidateRelation::ExistingRequirementEvidenceEdge.is_producer_backed());
    }

    #[test]
    fn context_only_relations_are_never_preferred() {
        assert!(CandidateRelation::FileProximityOnly.is_context_only());
        assert!(CandidateRelation::TokenCoincidenceOnly.is_context_only());
        assert!(CandidateRelation::OpaqueHelperOrMacro.is_context_only());
        assert!(CandidateRelation::Unknown.is_context_only());
    }

    #[test]
    fn producer_and_context_are_mutually_exclusive() {
        for relation in [
            CandidateRelation::DirectOwnerCall,
            CandidateRelation::EstablishedOneHopHelperChain,
            CandidateRelation::EstablishedEntrypointRelation,
            CandidateRelation::EstablishedFixtureOrGoldenOwner,
            CandidateRelation::ExistingRequirementEvidenceEdge,
            CandidateRelation::ConfiguredStaticCheckerRule,
            CandidateRelation::FileProximityOnly,
            CandidateRelation::TokenCoincidenceOnly,
            CandidateRelation::OpaqueHelperOrMacro,
            CandidateRelation::Unknown,
        ] {
            assert!(
                relation.is_producer_backed() != relation.is_context_only(),
                "{relation:?} must be either producer-backed or context-only, not both or neither"
            );
        }
    }

    #[test]
    fn as_str_round_trips() {
        for relation in [
            CandidateRelation::DirectOwnerCall,
            CandidateRelation::TokenCoincidenceOnly,
            CandidateRelation::Unknown,
        ] {
            let wire = relation.as_str();
            assert!(!wire.is_empty());
            assert!(wire.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }
}
