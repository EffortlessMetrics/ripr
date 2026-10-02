//! Shared behavior-evidence witness and parity corpus (#4790 / RIPR-SPEC-0204).
//!
//! This module is an additive projection over existing producer facts. It does
//! not recompute stage meaning, public classes, actionability, cache
//! generation, or route readiness. Existing classifiers remain authoritative.

mod adapters;
#[cfg(test)]
mod parity;
mod relations;
mod stage;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) use adapters::{
    AdapterInput, repo_adapter_input, retain_classified_seams, retain_finding_projection,
};
#[cfg(test)]
pub(crate) use adapters::{
    from_classified_seam, from_classified_seam_with_input, from_finding, from_finding_with_input,
};
#[cfg(test)]
pub(crate) use parity::{ParityDisposition, ParityReport, compare_pair, pair_by_portable_id};
pub(in crate::analysis) use relations::RelationWitness;
pub(in crate::analysis) use stage::StageWitness;

pub(in crate::analysis) const SCHEMA_VERSION: u16 = 1;
pub(in crate::analysis) const GENERATION: u16 = 1;

/// Which analysis path produced the source facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisPath {
    Diff,
    Repo,
}

/// Declared subject set for one adapter invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubjectSet {
    DiffOnly,
    WorkspaceComplete,
    PartialIndex,
    Unspecified,
}

impl SubjectSet {
    pub(in crate::analysis::witness) fn as_str(self) -> &'static str {
        match self {
            Self::DiffOnly => "diff_only_subject_set",
            Self::WorkspaceComplete => "workspace_complete",
            Self::PartialIndex => "partial_index",
            Self::Unspecified => "unspecified_input_completeness",
        }
    }
}

/// Declared input currentness for one adapter invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputCurrentness {
    Current,
    Stale,
    Wrong,
    Unspecified,
}

impl InputCurrentness {
    pub(in crate::analysis::witness) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale_input",
            Self::Wrong => "wrong_input",
            Self::Unspecified => "unspecified_currentness",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct OwnerIdentity {
    pub(in crate::analysis) identity: String,
    pub(in crate::analysis) source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct ExpressionIdentity {
    pub(in crate::analysis) normalized: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct DiscriminatorIdentity {
    pub(in crate::analysis) identity: String,
    pub(in crate::analysis) source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(in crate::analysis) enum TargetState {
    SelectedExisting { identities: Vec<String> },
    TypedAbsence { reason: String },
}

impl TargetState {
    #[cfg(test)]
    pub(in crate::analysis::witness) fn kind(&self) -> &'static str {
        match self {
            Self::SelectedExisting { .. } => "selected_existing",
            Self::TypedAbsence { .. } => "typed_absence",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct UnresolvedEdge {
    pub(in crate::analysis) stage: String,
    pub(in crate::analysis) detail: String,
}

/// Internal comparable evidence record for one Rust analysis path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::analysis) struct BehaviorEvidenceWitnessV1 {
    pub(in crate::analysis) schema_version: u16,
    pub(in crate::analysis) generation: u16,
    pub(in crate::analysis) path: AnalysisPath,
    pub(in crate::analysis) portable_item_id: String,
    pub(in crate::analysis) subject_set: SubjectSet,
    pub(in crate::analysis) currentness: InputCurrentness,
    pub(in crate::analysis) language: String,
    pub(in crate::analysis) language_status: Option<String>,
    pub(in crate::analysis) family: String,
    pub(in crate::analysis) owner: OwnerIdentity,
    pub(in crate::analysis) expression: ExpressionIdentity,
    pub(in crate::analysis) required_discriminator: DiscriminatorIdentity,
    pub(in crate::analysis) expected_sink: String,
    pub(in crate::analysis) candidate_relations: Vec<RelationWitness>,
    pub(in crate::analysis) established_relations: Vec<RelationWitness>,
    pub(in crate::analysis) reach: StageWitness,
    pub(in crate::analysis) activation: StageWitness,
    pub(in crate::analysis) propagation: StageWitness,
    pub(in crate::analysis) observation: StageWitness,
    pub(in crate::analysis) discrimination: StageWitness,
    pub(in crate::analysis) selected_target: TargetState,
    pub(in crate::analysis) earliest_unresolved_edge: Option<UnresolvedEdge>,
    pub(in crate::analysis) limitations: Vec<String>,
    pub(in crate::analysis) non_claims: Vec<String>,
    pub(in crate::analysis) public_class: String,
    pub(in crate::analysis) semantic_digest: String,
}

impl BehaviorEvidenceWitnessV1 {
    pub(in crate::analysis::witness) fn finalize(mut self) -> Self {
        self.normalize_collections();
        self.earliest_unresolved_edge = earliest_unresolved_edge(&self);
        self.semantic_digest = self.compute_semantic_digest();
        self
    }

    fn normalize_collections(&mut self) {
        sort_unique(&mut self.limitations);
        sort_unique(&mut self.non_claims);
        self.candidate_relations.sort();
        self.established_relations.sort();
        self.reach.normalize();
        self.activation.normalize();
        self.propagation.normalize();
        self.observation.normalize();
        self.discrimination.normalize();
        if let TargetState::SelectedExisting { identities } = &mut self.selected_target {
            sort_unique(identities);
        }
    }

    pub(in crate::analysis) fn digest_matches(&self) -> bool {
        !self.semantic_digest.is_empty() && self.semantic_digest == self.compute_semantic_digest()
    }

    fn compute_semantic_digest(&self) -> String {
        let mut canonical = Vec::new();
        append_field(&mut canonical, "behavior-evidence-witness-v1");
        append_field(&mut canonical, &self.schema_version.to_string());
        append_field(&mut canonical, &self.generation.to_string());
        append_field(&mut canonical, &self.portable_item_id);
        append_field(&mut canonical, &self.language);
        append_field(&mut canonical, &self.family);
        append_field(&mut canonical, &self.owner.identity);
        append_field(&mut canonical, &self.expression.normalized);
        append_field(&mut canonical, &self.required_discriminator.identity);
        append_field(&mut canonical, &self.expected_sink);
        append_field(&mut canonical, &self.public_class);
        append_relations(&mut canonical, "established", &self.established_relations);
        append_relations(&mut canonical, "candidate", &self.candidate_relations);
        append_stage(&mut canonical, "reach", &self.reach);
        append_stage(&mut canonical, "activation", &self.activation);
        append_stage(&mut canonical, "propagation", &self.propagation);
        append_stage(&mut canonical, "observation", &self.observation);
        append_stage(&mut canonical, "discrimination", &self.discrimination);
        match &self.selected_target {
            TargetState::SelectedExisting { identities } => {
                append_field(&mut canonical, "selected_existing");
                append_field(&mut canonical, &identities.len().to_string());
                for identity in identities {
                    append_field(&mut canonical, identity);
                }
            }
            TargetState::TypedAbsence { reason } => {
                append_field(&mut canonical, "typed_absence");
                append_field(&mut canonical, reason);
            }
        }
        append_field(&mut canonical, &self.limitations.len().to_string());
        for limitation in &self.limitations {
            append_field(&mut canonical, limitation);
        }
        append_field(&mut canonical, &self.non_claims.len().to_string());
        for non_claim in &self.non_claims {
            append_field(&mut canonical, non_claim);
        }
        hex_digest(&canonical)
    }
}

fn append_relations(canonical: &mut Vec<u8>, label: &str, relations: &[RelationWitness]) {
    append_field(canonical, label);
    append_field(canonical, &relations.len().to_string());
    for relation in relations {
        append_field(canonical, &relation.semantic_identity());
    }
}

fn append_stage(canonical: &mut Vec<u8>, label: &str, stage: &StageWitness) {
    append_field(canonical, label);
    append_field(canonical, &stage.state);
    append_field(canonical, &stage.confidence);
    append_field(
        canonical,
        stage.first_unresolved_edge.as_deref().unwrap_or(""),
    );
    append_field(canonical, &stage.source_identities.len().to_string());
    for identity in &stage.source_identities {
        append_field(canonical, identity);
    }
    append_field(canonical, &stage.established_facts.len().to_string());
    for fact in &stage.established_facts {
        append_field(canonical, fact);
    }
    append_field(canonical, &stage.candidate_facts.len().to_string());
    for fact in &stage.candidate_facts {
        append_field(canonical, fact);
    }
}

fn append_field(canonical: &mut Vec<u8>, value: &str) {
    canonical.extend_from_slice(&(value.len() as u64).to_le_bytes());
    canonical.extend_from_slice(value.as_bytes());
}

fn hex_digest(canonical: &[u8]) -> String {
    let digest = Sha256::digest(canonical);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn sort_unique(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn earliest_unresolved_edge(witness: &BehaviorEvidenceWitnessV1) -> Option<UnresolvedEdge> {
    let stages = [
        ("reach", &witness.reach),
        ("activation", &witness.activation),
        ("propagation", &witness.propagation),
        ("observation", &witness.observation),
        ("discrimination", &witness.discrimination),
    ];
    stages.into_iter().find_map(|(name, stage)| {
        if stage.state == "yes" {
            None
        } else {
            Some(UnresolvedEdge {
                stage: name.to_string(),
                detail: stage
                    .first_unresolved_edge
                    .clone()
                    .unwrap_or_else(|| format!("unresolved:{name}:{}", stage.state)),
            })
        }
    })
}

fn normalize_semantic_text(text: &str) -> String {
    text.trim()
        .trim_end_matches([',', ';'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
