use serde::{Deserialize, Serialize};

use crate::domain::StageEvidence;

/// One five-stage record projected from a producer stage, never recomputed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct StageWitness {
    pub(in crate::analysis) state: String,
    pub(in crate::analysis) basis: String,
    pub(in crate::analysis) source_identities: Vec<String>,
    pub(in crate::analysis) established_facts: Vec<String>,
    pub(in crate::analysis) candidate_facts: Vec<String>,
    pub(in crate::analysis) first_unresolved_edge: Option<String>,
    pub(in crate::analysis) confidence: String,
    pub(in crate::analysis) limitations: Vec<String>,
    pub(in crate::analysis) non_claims: Vec<String>,
}

impl StageWitness {
    pub(in crate::analysis::witness) fn from_producer(
        name: &str,
        evidence: &StageEvidence,
    ) -> Self {
        let state = evidence.state.as_str().to_string();
        let first_unresolved_edge = if state == "yes" {
            None
        } else {
            Some(format!("unresolved:{name}:{state}"))
        };
        let mut limitations = Vec::new();
        if state == "opaque" {
            limitations.push("opaque_stage".to_string());
        }
        Self {
            state,
            basis: "producer_stage".to_string(),
            source_identities: Vec::new(),
            established_facts: Vec::new(),
            candidate_facts: Vec::new(),
            first_unresolved_edge,
            confidence: evidence.confidence.as_str().to_string(),
            limitations,
            non_claims: vec!["adapter_does_not_recompute_stage_meaning".to_string()],
        }
    }

    #[cfg(test)]
    pub(in crate::analysis::witness) fn absent(name: &str) -> Self {
        Self {
            state: "unknown".to_string(),
            basis: "unrepresentable_source_fact".to_string(),
            source_identities: Vec::new(),
            established_facts: Vec::new(),
            candidate_facts: Vec::new(),
            first_unresolved_edge: Some(format!("unresolved:{name}:absent")),
            confidence: "unknown".to_string(),
            limitations: vec!["unrepresentable_stage".to_string()],
            non_claims: vec!["adapter_does_not_invent_missing_stage".to_string()],
        }
    }

    pub(in crate::analysis::witness) fn normalize(&mut self) {
        super::sort_unique(&mut self.source_identities);
        super::sort_unique(&mut self.established_facts);
        super::sort_unique(&mut self.candidate_facts);
        super::sort_unique(&mut self.limitations);
        super::sort_unique(&mut self.non_claims);
    }

    pub(in crate::analysis::witness) fn push_source(&mut self, identity: String) {
        self.source_identities.push(identity);
    }
}
