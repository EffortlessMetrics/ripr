mod candidate_relation;
mod causal_delta;
mod classification;
#[doc(hidden)]
pub mod context_packet;
mod diagnostic_witness;
mod evidence;
mod fix_instruction;
mod language;
mod probe;
mod summary;
mod support;
mod test_evidence_summary;

pub use candidate_relation::CandidateRelation;
pub use causal_delta::{
    AttributionBasis, CanonicalDelta, CanonicalEvidenceState, ComparisonConfidence,
    ComparisonCoverage, DeltaAttribution, GapState, compare_fixture_delta,
};
pub use classification::ExposureClass;
pub(crate) use classification::{
    LIMITATION_ANALYZER_ROUTE_PREFIX, LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX,
    LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX, LIMITATION_NON_CLAIM_PREFIX, NO_STATIC_PATH_NEXT_STEP,
    TRANSITIVE_REACH_WITNESS_PREFIX,
};
pub use diagnostic_witness::{
    DiagnosticConfidence, DiagnosticFixSite, DiagnosticSourceLocation, DiagnosticWitness,
    DiagnosticWitnessLimitation,
};
pub use evidence::{
    Confidence, OracleKind, OracleStrength, RelationConfidence, RelationReason, RevealEvidence,
    RiprEvidence, StageEvidence, StageState,
};
pub use fix_instruction::{FixInstructionState, FixInstructionSummary};
pub use language::{LanguageId, LanguageStatus, OwnerKind, StaticLimitKind};
pub use probe::{
    ActivationEvidence, DeltaKind, Finding, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
    MissingDiscriminatorFact, ORACLE_ALIGNMENT_VALUES, Probe, ProbeFamily, RelatedTest, StopReason,
    ValueContext, ValueFact,
};
pub use summary::{LanguageFileCount, Summary};
pub use support::{ProbeId, SourceLocation, SymbolId};
pub use test_evidence_summary::{TestEvidenceEntry, TestEvidenceSummary};
