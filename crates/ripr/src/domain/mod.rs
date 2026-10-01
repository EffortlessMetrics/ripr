mod candidate_relation;
mod causal_delta;
mod classification;
mod command_spec;
#[doc(hidden)]
pub mod context_packet;
mod diagnostic_witness;
mod evidence;
pub(crate) mod executed_control;
mod feedback;
mod finding_test_evidence;
mod fix_instruction;
mod git_candidate;
mod identity;
mod language;
mod probe;
mod repair_card;
mod summary;
mod support;
mod test_evidence_identity;
mod test_evidence_summary;
mod verification_result;

pub use candidate_relation::CandidateRelation;
pub use causal_delta::{
    AttributionBasis, CanonicalDelta, CanonicalEvidenceState, ComparisonConfidence,
    ComparisonCoverage, DeltaAttribution, GapState, compare_fixture_delta,
};
pub use classification::ExposureClass;
pub(crate) use classification::{
    ASSERTION_SHAPED_INFECTION_UNKNOWN_NEXT_STEP, ASSERTION_SHAPED_NO_STATIC_PATH_NEXT_STEP,
    ASSERTION_SHAPED_OWNER_REASON, ASSERTION_SHAPED_REACHABLE_UNREVEALED_NEXT_STEP,
    ASSERTION_SHAPED_WEAKLY_EXPOSED_NEXT_STEP, LIMITATION_ANALYZER_ROUTE_PREFIX,
    LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX, LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
    LIMITATION_NON_CLAIM_PREFIX, NO_STATIC_PATH_NEXT_STEP, TRANSITIVE_REACH_WITNESS_PREFIX,
};
pub use command_spec::{
    CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandExecutionMode,
    CommandPlatform, CommandRole, CommandSpec, CommandSpecValidationError, EnvironmentAssignment,
    EnvironmentPolicy, ExpectedResultParser, NetworkPolicy, StdinPolicy,
};
pub use diagnostic_witness::{
    DiagnosticConfidence, DiagnosticFixSite, DiagnosticSourceLocation, DiagnosticWitness,
    DiagnosticWitnessLimitation,
};
pub use evidence::{
    Confidence, OracleKind, OracleStrength, RelationConfidence, RelationReason, RevealEvidence,
    RiprEvidence, StageEvidence, StageState,
};
pub(crate) use executed_control::{
    EXECUTED_CONTROL_PACKET_KIND, EXECUTED_CONTROL_SCHEMA_VERSION, ExecutedControlPacketV1,
    ObligationSatisfaction, ResultState,
};
pub(crate) use feedback::{
    ActorKind, FEEDBACK_NOTE_MAX_BYTES, FEEDBACK_SCHEMA_VERSION, FeedbackJudgment, FeedbackPayload,
    FeedbackReason, FeedbackReceipt, ReferenceState, ResultIdentity, ReviewStatus,
    classify_reference,
};
pub use fix_instruction::{FixInstructionState, FixInstructionSummary};
pub use git_candidate::{
    GitCandidateBase, GitCandidateDiffSemantics, GitCandidateSubject, GitCandidateSubjectError,
    GitHashFormat, GitObjectId, GitTreeish,
};
pub use identity::{
    GOVERNED_IDENTITY_SURFACES, IDENTITY_REGISTRY_JSON_PATH, IDENTITY_REGISTRY_MARKDOWN_PATH,
    REQUIRED_TAXONOMY_KINDS, identity_field_disposition, identity_registry_canonical_json,
    identity_registry_markdown, identity_registry_violations,
};
pub(crate) use language::PERL_FACT_EXPORTER;
#[cfg(feature = "lang-perl")]
pub(crate) use language::perl_fact_packet_guidance;
pub use language::{LanguageId, LanguageStatus, OwnerKind, StaticLimitKind};
pub(crate) use language::{PYTEST_VERIFY_PROGRAM, is_pytest_verify_command};
pub use probe::{
    ActivationEvidence, DeltaKind, Finding, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
    MissingDiscriminatorFact, ORACLE_ALIGNMENT_VALUES, Probe, ProbeFamily, RelatedTest,
    SOURCE_CURRENTNESS_VALUES, SourceCurrentness, StopReason, ValueContext, ValueFact,
};
pub use repair_card::{
    CardCurrentnessGoal, DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS,
    DEFAULT_REPAIR_CARD_MAX_INLINE_DETAIL_BYTES, DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES,
    EditCageGoal, FocusedExecutionGoal, MAX_REPAIR_CARD_REJECTED_ALTERNATIVES,
    MutationConfirmationGoal, REPAIR_CARD_BUDGET_VERSION, REPAIR_CARD_CLAIM_BOUNDARY,
    REPAIR_CARD_SCHEMA_VERSION, RepairCardAssertionGoal, RepairCardAttempt, RepairCardBudget,
    RepairCardCommandRef, RepairCardDetailFamily, RepairCardDetailRef, RepairCardDetailState,
    RepairCardDetailSummary, RepairCardDoneWhen, RepairCardOmissionClass,
    RepairCardProposedTestKind, RepairCardReadinessFacts, RepairCardRejectedAlternative,
    RepairCardSnapshot, RepairCardSnapshotCurrentness, RepairCardSubject, RepairCardTarget,
    RepairCardTestKind, RepairCardV1, StaticMovementGoal, repair_card_route_exposable,
};
// Internal formatting convention, not library API: `lib.rs` re-exports
// `pub mod domain`, so this stays crate-private.
pub(crate) use probe::MISSING_DISCRIMINATOR_VALUE_PREFIX;
pub use summary::{LanguageFileCount, Summary};
pub use support::{ProbeId, SourceLocation, SymbolId};
pub use test_evidence_summary::{TestEvidenceEntry, TestEvidenceSummary};
pub(crate) use verification_result::CommandSpecDigest;
pub use verification_result::{
    VERIFICATION_EXECUTION_RESULT_SCHEMA_VERSION, VerificationCurrentnessV1,
    VerificationExecutionResultV1, VerificationExecutionResultValidationError,
    VerificationProcessDispositionV1, command_spec_sha256,
};
