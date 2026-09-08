//! Pure, fail-closed authorization for an exact reverse landing candidate.
//!
//! This module deliberately consumes observations rather than GitHub clients.
//! Transport, protection reads, and receipt authentication belong to their own
//! adapters; this evaluator only joins their already-normalized evidence.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceState {
    Pass,
    Fail,
    Missing,
    Stale,
    Partial,
    Contradictory,
}

impl EvidenceState {
    fn is_terminal_failure(self) -> bool {
        matches!(self, Self::Fail)
    }
    fn is_pass(self) -> bool {
        matches!(self, Self::Pass)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct PredicateEvidence {
    pub(crate) state: EvidenceState,
    pub(crate) detail: String,
}

impl PredicateEvidence {
    pub(crate) fn pass(detail: impl Into<String>) -> Self {
        Self {
            state: EvidenceState::Pass,
            detail: detail.into(),
        }
    }
    pub(crate) fn fail(detail: impl Into<String>) -> Self {
        Self {
            state: EvidenceState::Fail,
            detail: detail.into(),
        }
    }
    pub(crate) fn missing(detail: impl Into<String>) -> Self {
        Self {
            state: EvidenceState::Missing,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ReverseAuthorizationInput {
    pub(crate) candidate_identity: PredicateEvidence,
    pub(crate) source_origin: PredicateEvidence,
    pub(crate) admission: PredicateEvidence,
    pub(crate) review_decision: PredicateEvidence,
    pub(crate) review_dimensions: PredicateEvidence,
    pub(crate) unresolved_threads: PredicateEvidence,
    pub(crate) live_protection: PredicateEvidence,
    pub(crate) required_checks: PredicateEvidence,
    pub(crate) movement: PredicateEvidence,
    pub(crate) provenance: PredicateEvidence,
    pub(crate) retention: PredicateEvidence,
    pub(crate) merge_method: PredicateEvidence,
    pub(crate) capability_boundary: PredicateEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuthorizationDecision {
    Authorized,
    Blocked,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ReverseAuthorizationReceipt {
    pub(crate) decision: AuthorizationDecision,
    pub(crate) predicates: BTreeMap<String, PredicateEvidence>,
    pub(crate) blockers: Vec<String>,
    pub(crate) required_operation: String,
}

pub(crate) fn evaluate_reverse_authorization(
    input: &ReverseAuthorizationInput,
) -> ReverseAuthorizationReceipt {
    let predicates = BTreeMap::from([
        ("admission".to_string(), input.admission.clone()),
        (
            "candidate_identity".to_string(),
            input.candidate_identity.clone(),
        ),
        (
            "capability_boundary".to_string(),
            input.capability_boundary.clone(),
        ),
        ("live_protection".to_string(), input.live_protection.clone()),
        ("merge_method".to_string(), input.merge_method.clone()),
        ("movement".to_string(), input.movement.clone()),
        ("provenance".to_string(), input.provenance.clone()),
        ("required_checks".to_string(), input.required_checks.clone()),
        ("retention".to_string(), input.retention.clone()),
        ("review_decision".to_string(), input.review_decision.clone()),
        (
            "review_dimensions".to_string(),
            input.review_dimensions.clone(),
        ),
        ("source_origin".to_string(), input.source_origin.clone()),
        (
            "unresolved_threads".to_string(),
            input.unresolved_threads.clone(),
        ),
    ]);
    let blockers = predicates
        .iter()
        .filter(|(_, evidence)| !evidence.state.is_pass())
        .map(|(name, evidence)| format!("{name}: {}", evidence.detail))
        .collect::<Vec<_>>();
    let decision = if predicates
        .values()
        .any(|evidence| evidence.state.is_terminal_failure())
    {
        AuthorizationDecision::Rejected
    } else if blockers.is_empty() {
        AuthorizationDecision::Authorized
    } else {
        AuthorizationDecision::Blocked
    };
    ReverseAuthorizationReceipt {
        decision,
        predicates,
        blockers,
        required_operation:
            "protected merge commit of the exact Q head into the declared swarm parent; no bypass"
                .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> ReverseAuthorizationInput {
        let pass = || PredicateEvidence::pass("current and verified");
        ReverseAuthorizationInput {
            candidate_identity: pass(),
            source_origin: pass(),
            admission: pass(),
            review_decision: pass(),
            review_dimensions: pass(),
            unresolved_threads: pass(),
            live_protection: pass(),
            required_checks: pass(),
            movement: pass(),
            provenance: pass(),
            retention: pass(),
            merge_method: pass(),
            capability_boundary: pass(),
        }
    }

    #[test]
    fn exact_current_candidate_is_authorized() {
        let receipt = evaluate_reverse_authorization(&valid());
        assert_eq!(receipt.decision, AuthorizationDecision::Authorized);
        assert!(receipt.blockers.is_empty());
        assert_eq!(receipt.predicates.len(), 13);
    }

    #[test]
    fn missing_evidence_blocks_without_authorizing() {
        let mut input = valid();
        input.live_protection = PredicateEvidence::missing("ruleset observation is unavailable");
        let receipt = evaluate_reverse_authorization(&input);
        assert_eq!(receipt.decision, AuthorizationDecision::Blocked);
        assert_eq!(receipt.blockers.len(), 1);
    }

    #[test]
    fn a_failed_identity_rejects_even_when_everything_else_passes() {
        let mut input = valid();
        input.candidate_identity = PredicateEvidence::fail("Q head differs from immutable receipt");
        let receipt = evaluate_reverse_authorization(&input);
        assert_eq!(receipt.decision, AuthorizationDecision::Rejected);
    }

    #[test]
    fn all_incomplete_evidence_states_block_and_are_reported() {
        let mut input = valid();
        input.review_dimensions = PredicateEvidence {
            state: EvidenceState::Stale,
            detail: "affected dimension review is stale".to_string(),
        };
        input.required_checks = PredicateEvidence {
            state: EvidenceState::Partial,
            detail: "required context set is partial".to_string(),
        };
        input.capability_boundary = PredicateEvidence {
            state: EvidenceState::Contradictory,
            detail: "capability observations contradict".to_string(),
        };
        let receipt = evaluate_reverse_authorization(&input);
        assert_eq!(receipt.decision, AuthorizationDecision::Blocked);
        assert_eq!(receipt.blockers.len(), 3);
        assert!(receipt.blockers[0].starts_with("capability_boundary:"));
        assert!(receipt.blockers[1].starts_with("required_checks:"));
        assert!(receipt.blockers[2].starts_with("review_dimensions:"));
    }

    #[test]
    fn failed_policy_takes_precedence_over_other_blockers() {
        let mut input = valid();
        input.merge_method = PredicateEvidence::fail("bypass merge requested");
        input.provenance = PredicateEvidence::missing("receipt producer unavailable");
        let receipt = evaluate_reverse_authorization(&input);
        assert_eq!(receipt.decision, AuthorizationDecision::Rejected);
        assert_eq!(receipt.blockers.len(), 2);
    }

    #[test]
    fn manual_review_is_not_collapsed_into_admission() {
        let mut input = valid();
        input.review_decision = PredicateEvidence::missing("manual semantic review not recorded");
        let receipt = evaluate_reverse_authorization(&input);
        assert_eq!(receipt.decision, AuthorizationDecision::Blocked);
        assert!(receipt.predicates.contains_key("review_decision"));
    }

    #[test]
    fn receipt_is_deterministically_ordered() {
        let receipt = evaluate_reverse_authorization(&valid());
        let keys = receipt.predicates.keys().collect::<Vec<_>>();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
