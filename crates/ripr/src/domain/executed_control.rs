//! Executed-control obligation and result vocabulary (RIPR-SPEC-0183 / #4641).
//!
//! This module owns the closed contract that distinguishes an executed
//! discriminating control from an ordinary positive test or a review argument.
//! It does not inspect live GitHub state, enforce merge eligibility, execute
//! commands, or rewrite historical issues as pass.

use super::GitObjectId;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version for executed-control obligation, result, and packet records.
pub(crate) const EXECUTED_CONTROL_SCHEMA_VERSION: &str = "1";

pub(crate) const EXECUTED_CONTROL_OBLIGATION_KIND: &str = "executed_control_obligation";
pub(crate) const EXECUTED_CONTROL_RESULT_KIND: &str = "executed_control_result";
pub(crate) const EXECUTED_CONTROL_PACKET_KIND: &str = "executed_control_packet";

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;

/// Exact control class named by an obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ControlClass {
    RemovedGuard,
    WrongImplementation,
    NamedMutation,
}

impl ControlClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RemovedGuard => "removed_guard",
            Self::WrongImplementation => "wrong_implementation",
            Self::NamedMutation => "named_mutation",
        }
    }
}

/// Whether closeout later treats the obligation as mandatory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Requiredness {
    Required,
    Advisory,
}

impl Requiredness {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Advisory => "advisory",
        }
    }
}

/// Discriminating outcome the named control must produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscriminatingOutcome {
    FailsBeforePassesAfter,
    RejectsWrongImplementation,
}

impl DiscriminatingOutcome {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FailsBeforePassesAfter => "fails_before_passes_after",
            Self::RejectsWrongImplementation => "rejects_wrong_implementation",
        }
    }
}

/// Evidence forms that may satisfy an executed-control obligation.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceForm {
    RetainedArtifact,
    BoundedLogCommitment,
    DeclaredSubstitute,
}

impl EvidenceForm {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RetainedArtifact => "retained_artifact",
            Self::BoundedLogCommitment => "bounded_log_commitment",
            Self::DeclaredSubstitute => "declared_substitute",
        }
    }
}

/// What a result claims to be offering as evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OfferedEvidenceKind {
    ExecutedDiscriminatingControl,
    OrdinaryPositiveTest,
    ReviewProse,
    StructuralDiscriminationClaim,
    DeclaredSubstitute,
}

impl OfferedEvidenceKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ExecutedDiscriminatingControl => "executed_discriminating_control",
            Self::OrdinaryPositiveTest => "ordinary_positive_test",
            Self::ReviewProse => "review_prose",
            Self::StructuralDiscriminationClaim => "structural_discrimination_claim",
            Self::DeclaredSubstitute => "declared_substitute",
        }
    }
}

/// Closed result state. Non-pass states stay explicit and non-satisfying
/// unless the obligation itself names an accepted substitute.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResultState {
    Passed,
    Failed,
    NotRun,
    NotProven,
    Substituted,
    InstrumentFailure,
}

impl ResultState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotRun => "not_run",
            Self::NotProven => "not_proven",
            Self::Substituted => "substituted",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    pub(crate) const fn claims_satisfaction(self) -> bool {
        matches!(self, Self::Passed | Self::Substituted)
    }
}

/// What was observed when the control ran, or why it did not.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservedOutcome {
    FailedBeforePassedAfter,
    RejectedWrongImplementation,
    CommandSucceededWithoutExercisingSubject,
    OrdinaryPositiveTestsPassed,
    ReviewArgumentOnly,
    StructuralDiscriminationOnly,
    InstrumentUnavailable,
    NotExecuted,
}

impl ObservedOutcome {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FailedBeforePassedAfter => "failed_before_passed_after",
            Self::RejectedWrongImplementation => "rejected_wrong_implementation",
            Self::CommandSucceededWithoutExercisingSubject => {
                "command_succeeded_without_exercising_subject"
            }
            Self::OrdinaryPositiveTestsPassed => "ordinary_positive_tests_passed",
            Self::ReviewArgumentOnly => "review_argument_only",
            Self::StructuralDiscriminationOnly => "structural_discrimination_only",
            Self::InstrumentUnavailable => "instrument_unavailable",
            Self::NotExecuted => "not_executed",
        }
    }
}

/// Conditions that invalidate a previously recorded result.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Invalidator {
    SourceHeadMoved,
    ControlContractChanged,
    ArtifactMissing,
    CommandIdentityChanged,
}

impl Invalidator {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SourceHeadMoved => "source_head_moved",
            Self::ControlContractChanged => "control_contract_changed",
            Self::ArtifactMissing => "artifact_missing",
            Self::CommandIdentityChanged => "command_identity_changed",
        }
    }
}

/// Named subject the control must actually exercise.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionSubject {
    pub command_or_instrument_id: String,
    pub named_wrong_implementation: String,
    pub required_head: String,
}

/// Explicit substitute declared by the obligation, never inferred later.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PermittedSubstitute {
    pub substitute_id: String,
    pub instrument_id: String,
    pub evidence_form: EvidenceForm,
}

/// Logical retained artifact identity. Machine paths are not identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactIdentity {
    pub logical_id: String,
    pub digest: String,
}

/// `executed_control_obligation.v1`
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutedControlObligationV1 {
    pub schema_version: String,
    pub kind: String,
    pub obligation_id: String,
    pub owning_claim: String,
    pub control_class: ControlClass,
    pub intended_wrong_implementation: String,
    pub required_execution_subject: ExecutionSubject,
    pub expected_discriminating_outcome: DiscriminatingOutcome,
    pub acceptable_evidence_forms: Vec<EvidenceForm>,
    pub permitted_substitute: Option<PermittedSubstitute>,
    pub requiredness: Requiredness,
    pub invalidators: Vec<Invalidator>,
}

/// `executed_control_result.v1`
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutedControlResultV1 {
    pub schema_version: String,
    pub kind: String,
    pub obligation_id: String,
    pub source_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    pub head: String,
    pub command_or_instrument_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtifactIdentity>,
    pub observed_outcome: ObservedOutcome,
    pub offered_evidence_kind: OfferedEvidenceKind,
    pub state: ResultState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitute_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obligation_digest: Option<String>,
}

/// One reusable obligation/result set. Closeout enforcement consumes this
/// object without parsing issue prose.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutedControlPacketV1 {
    pub schema_version: String,
    pub kind: String,
    pub source_identity: String,
    pub obligations: Vec<ExecutedControlObligationV1>,
    pub results: Vec<ExecutedControlResultV1>,
}

/// Per-obligation satisfaction after a structurally valid packet is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObligationSatisfaction {
    pub obligation_id: String,
    pub requiredness: Requiredness,
    pub state: Option<ResultState>,
    pub satisfies: bool,
}

/// Validated packet evaluation. Absence of a result is explicit non-satisfaction,
/// not an inferred pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PacketEvaluation {
    pub satisfactions: Vec<ObligationSatisfaction>,
}

/// Validation failures for obligations, results, and packets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExecutedControlValidationError {
    EmptyField(&'static str),
    InvalidSchemaVersion {
        kind: &'static str,
        value: String,
    },
    InvalidKind {
        expected: &'static str,
        actual: String,
    },
    InvalidHead {
        field: &'static str,
        value: String,
    },
    InvalidDigest {
        field: &'static str,
        value: String,
    },
    DuplicateObligationId(String),
    UnknownObligation(String),
    OrdinaryPositiveTestCannotSatisfy(String),
    ReviewProseCannotSatisfy(String),
    StructuralClaimCannotSatisfy(String),
    MissingArtifact {
        obligation_id: String,
        state: ResultState,
    },
    HeadMismatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    CommandMismatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    WrongImplementationNotExercised(String),
    ObligationDigestMismatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    MissingObligationDigest(String),
    SubstituteNotDeclared(String),
    SubstituteMismatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    ConflictingSatisfyingResults(String),
    PassedRequiresExecutedControl(String),
    ObservedOutcomeDoesNotMatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    SubstituteRequiresDeclaredEvidence(String),
    SourceIdentityMismatch {
        obligation_id: String,
        expected: String,
        actual: String,
    },
    EvidenceFormNotAccepted {
        obligation_id: String,
        state: ResultState,
    },
    UnexecutedSubstituteCannotSatisfy(String),
    ConflictingSameSubjectResults(String),
}

impl fmt::Display for ExecutedControlValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "{field} must not be empty"),
            Self::InvalidSchemaVersion { kind, value } => {
                write!(
                    formatter,
                    "unsupported {kind} schema version {value:?}; expected {EXECUTED_CONTROL_SCHEMA_VERSION}"
                )
            }
            Self::InvalidKind { expected, actual } => {
                write!(formatter, "kind must be {expected}, got {actual:?}")
            }
            Self::InvalidHead { field, value } => write!(
                formatter,
                "{field} must be a full 40-character Git SHA, got {value:?}"
            ),
            Self::InvalidDigest { field, value } => write!(
                formatter,
                "{field} must be sha256:<64 lowercase hex>, got {value:?}"
            ),
            Self::DuplicateObligationId(id) => {
                write!(formatter, "duplicated obligation_id {id:?}")
            }
            Self::UnknownObligation(id) => {
                write!(formatter, "result names unknown obligation_id {id:?}")
            }
            Self::OrdinaryPositiveTestCannotSatisfy(id) => write!(
                formatter,
                "ordinary positive test cannot satisfy executed-control obligation {id:?}"
            ),
            Self::ReviewProseCannotSatisfy(id) => write!(
                formatter,
                "review prose cannot satisfy executed-control obligation {id:?}"
            ),
            Self::StructuralClaimCannotSatisfy(id) => write!(
                formatter,
                "structural discrimination claim cannot satisfy executed-control obligation {id:?}"
            ),
            Self::MissingArtifact {
                obligation_id,
                state,
            } => write!(
                formatter,
                "{state:?} result for {obligation_id:?} is missing retained artifact identity"
            ),
            Self::HeadMismatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "result for {obligation_id:?} binds head {actual}, expected {expected}"
            ),
            Self::CommandMismatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "result for {obligation_id:?} binds command {actual:?}, expected {expected:?}"
            ),
            Self::WrongImplementationNotExercised(id) => write!(
                formatter,
                "command succeeded without exercising the named wrong implementation for {id:?}"
            ),
            Self::ObligationDigestMismatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "stale obligation digest for {obligation_id:?}: expected {expected}, got {actual}"
            ),
            Self::MissingObligationDigest(id) => write!(
                formatter,
                "satisfying result for {id:?} is missing obligation_digest"
            ),
            Self::SubstituteNotDeclared(id) => write!(
                formatter,
                "substituted result for {id:?} has no permitted substitute on the obligation"
            ),
            Self::SubstituteMismatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "substitute {actual:?} for {obligation_id:?} does not match declared {expected:?}"
            ),
            Self::ConflictingSatisfyingResults(id) => write!(
                formatter,
                "obligation {id:?} has more than one satisfying result"
            ),
            Self::PassedRequiresExecutedControl(id) => write!(
                formatter,
                "passed result for {id:?} requires offered evidence executed_discriminating_control"
            ),
            Self::ObservedOutcomeDoesNotMatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "observed outcome {actual} for {obligation_id:?} does not match expected {expected}"
            ),
            Self::SubstituteRequiresDeclaredEvidence(id) => write!(
                formatter,
                "substituted result for {id:?} must offer declared_substitute evidence"
            ),
            Self::SourceIdentityMismatch {
                obligation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "result for {obligation_id:?} binds source identity {actual:?}, expected {expected:?}"
            ),
            Self::EvidenceFormNotAccepted {
                obligation_id,
                state,
            } => write!(
                formatter,
                "{state:?} result for {obligation_id:?} is not an acceptable evidence form"
            ),
            Self::UnexecutedSubstituteCannotSatisfy(id) => write!(
                formatter,
                "unexecuted substitute cannot satisfy executed-control obligation {id:?}"
            ),
            Self::ConflictingSameSubjectResults(id) => write!(
                formatter,
                "obligation {id:?} has conflicting passed and failed results for the same head and command"
            ),
        }
    }
}

impl std::error::Error for ExecutedControlValidationError {}

impl ExecutedControlObligationV1 {
    pub(crate) fn validate(&self) -> Result<(), ExecutedControlValidationError> {
        if self.schema_version != EXECUTED_CONTROL_SCHEMA_VERSION {
            return Err(ExecutedControlValidationError::InvalidSchemaVersion {
                kind: "obligation",
                value: self.schema_version.clone(),
            });
        }
        if self.kind != EXECUTED_CONTROL_OBLIGATION_KIND {
            return Err(ExecutedControlValidationError::InvalidKind {
                expected: EXECUTED_CONTROL_OBLIGATION_KIND,
                actual: self.kind.clone(),
            });
        }
        validate_non_empty("obligation_id", &self.obligation_id)?;
        validate_non_empty("owning_claim", &self.owning_claim)?;
        validate_non_empty(
            "intended_wrong_implementation",
            &self.intended_wrong_implementation,
        )?;
        validate_non_empty(
            "required_execution_subject.command_or_instrument_id",
            &self.required_execution_subject.command_or_instrument_id,
        )?;
        validate_non_empty(
            "required_execution_subject.named_wrong_implementation",
            &self.required_execution_subject.named_wrong_implementation,
        )?;
        validate_head(
            "required_execution_subject.required_head",
            &self.required_execution_subject.required_head,
        )?;
        if self.acceptable_evidence_forms.is_empty() {
            return Err(ExecutedControlValidationError::EmptyField(
                "acceptable_evidence_forms",
            ));
        }
        if let Some(substitute) = &self.permitted_substitute {
            validate_non_empty(
                "permitted_substitute.substitute_id",
                &substitute.substitute_id,
            )?;
            validate_non_empty(
                "permitted_substitute.instrument_id",
                &substitute.instrument_id,
            )?;
            if !self
                .acceptable_evidence_forms
                .contains(&EvidenceForm::DeclaredSubstitute)
            {
                return Err(ExecutedControlValidationError::EmptyField(
                    "acceptable_evidence_forms.declared_substitute",
                ));
            }
        }
        if self.invalidators.is_empty() {
            return Err(ExecutedControlValidationError::EmptyField("invalidators"));
        }
        Ok(())
    }

    /// Digest of the obligation contract. Timestamps, machine paths, and log
    /// order are not part of this identity.
    pub(crate) fn semantic_digest(&self) -> String {
        let mut forms: Vec<&str> = self
            .acceptable_evidence_forms
            .iter()
            .map(|form| form.as_str())
            .collect();
        forms.sort_unstable();
        forms.dedup();
        let mut invalidators: Vec<&str> = self
            .invalidators
            .iter()
            .map(|invalidator| invalidator.as_str())
            .collect();
        invalidators.sort_unstable();
        invalidators.dedup();
        let substitute = match &self.permitted_substitute {
            None => "-".to_string(),
            Some(value) => format!(
                "{}|{}|{}",
                value.substitute_id,
                value.instrument_id,
                value.evidence_form.as_str()
            ),
        };
        let required_head = self
            .required_execution_subject
            .required_head
            .to_ascii_lowercase();
        let forms_joined = forms.join(",");
        let invalidators_joined = invalidators.join(",");
        let canonical = [
            self.obligation_id.as_str(),
            self.owning_claim.as_str(),
            self.control_class.as_str(),
            self.intended_wrong_implementation.as_str(),
            self.required_execution_subject
                .command_or_instrument_id
                .as_str(),
            self.required_execution_subject
                .named_wrong_implementation
                .as_str(),
            required_head.as_str(),
            self.expected_discriminating_outcome.as_str(),
            forms_joined.as_str(),
            substitute.as_str(),
            self.requiredness.as_str(),
            invalidators_joined.as_str(),
        ]
        .join("\n");
        format!("{SHA256_PREFIX}{:x}", Sha256::digest(canonical.as_bytes()))
    }
}

impl ExecutedControlResultV1 {
    pub(crate) fn validate_shape(&self) -> Result<(), ExecutedControlValidationError> {
        if self.schema_version != EXECUTED_CONTROL_SCHEMA_VERSION {
            return Err(ExecutedControlValidationError::InvalidSchemaVersion {
                kind: "result",
                value: self.schema_version.clone(),
            });
        }
        if self.kind != EXECUTED_CONTROL_RESULT_KIND {
            return Err(ExecutedControlValidationError::InvalidKind {
                expected: EXECUTED_CONTROL_RESULT_KIND,
                actual: self.kind.clone(),
            });
        }
        validate_non_empty("obligation_id", &self.obligation_id)?;
        validate_non_empty("source_identity", &self.source_identity)?;
        validate_head("head", &self.head)?;
        validate_non_empty("command_or_instrument_id", &self.command_or_instrument_id)?;
        if let Some(candidate_id) = &self.candidate_id {
            validate_non_empty("candidate_id", candidate_id)?;
        }
        if let Some(limitation) = &self.limitation {
            validate_non_empty("limitation", limitation)?;
        }
        if let Some(substitute_id) = &self.substitute_id {
            validate_non_empty("substitute_id", substitute_id)?;
        }
        if let Some(digest) = &self.obligation_digest {
            validate_sha256("obligation_digest", digest)?;
        }
        if let Some(artifact) = &self.artifact {
            validate_non_empty("artifact.logical_id", &artifact.logical_id)?;
            validate_sha256("artifact.digest", &artifact.digest)?;
        }
        Ok(())
    }
}

impl ExecutedControlPacketV1 {
    pub(crate) fn validate(&self) -> Result<PacketEvaluation, ExecutedControlValidationError> {
        if self.schema_version != EXECUTED_CONTROL_SCHEMA_VERSION {
            return Err(ExecutedControlValidationError::InvalidSchemaVersion {
                kind: "packet",
                value: self.schema_version.clone(),
            });
        }
        if self.kind != EXECUTED_CONTROL_PACKET_KIND {
            return Err(ExecutedControlValidationError::InvalidKind {
                expected: EXECUTED_CONTROL_PACKET_KIND,
                actual: self.kind.clone(),
            });
        }
        validate_non_empty("source_identity", &self.source_identity)?;

        let mut seen = BTreeSet::new();
        let mut by_id = BTreeMap::new();
        for obligation in &self.obligations {
            obligation.validate()?;
            if !seen.insert(obligation.obligation_id.clone()) {
                return Err(ExecutedControlValidationError::DuplicateObligationId(
                    obligation.obligation_id.clone(),
                ));
            }
            by_id.insert(obligation.obligation_id.clone(), obligation);
        }

        for result in &self.results {
            result.validate_shape()?;
            let Some(obligation) = by_id.get(&result.obligation_id) else {
                return Err(ExecutedControlValidationError::UnknownObligation(
                    result.obligation_id.clone(),
                ));
            };
            bind_result(&self.source_identity, obligation, result)?;
        }

        let mut satisfying: BTreeMap<&str, usize> = BTreeMap::new();
        let mut subject_states: BTreeMap<(String, String, String), Vec<ResultState>> =
            BTreeMap::new();
        for result in &self.results {
            if result.state.claims_satisfaction() {
                *satisfying.entry(result.obligation_id.as_str()).or_insert(0) += 1;
            }
            subject_states
                .entry((
                    result.obligation_id.clone(),
                    result.head.to_ascii_lowercase(),
                    result.command_or_instrument_id.clone(),
                ))
                .or_default()
                .push(result.state);
        }
        for (obligation_id, count) in satisfying {
            if count > 1 {
                return Err(
                    ExecutedControlValidationError::ConflictingSatisfyingResults(
                        obligation_id.to_string(),
                    ),
                );
            }
        }
        for ((obligation_id, _, _), states) in subject_states {
            let has_pass = states.contains(&ResultState::Passed);
            let has_fail = states.contains(&ResultState::Failed);
            if has_pass && has_fail {
                return Err(
                    ExecutedControlValidationError::ConflictingSameSubjectResults(obligation_id),
                );
            }
        }

        let mut satisfactions = Vec::with_capacity(self.obligations.len());
        for obligation in &self.obligations {
            let matching: Vec<&ExecutedControlResultV1> = self
                .results
                .iter()
                .filter(|result| result.obligation_id == obligation.obligation_id)
                .collect();
            let satisfying_result = matching
                .iter()
                .find(|result| result.state.claims_satisfaction())
                .copied();
            let state = satisfying_result
                .map(|result| result.state)
                .or_else(|| matching.last().map(|result| result.state));
            let satisfies = satisfying_result.is_some();
            satisfactions.push(ObligationSatisfaction {
                obligation_id: obligation.obligation_id.clone(),
                requiredness: obligation.requiredness,
                state,
                satisfies,
            });
        }
        Ok(PacketEvaluation { satisfactions })
    }

    /// Sort obligations and results so serialization is independent of input order.
    pub(crate) fn canonicalize(&mut self) {
        self.obligations
            .sort_by(|left, right| left.obligation_id.cmp(&right.obligation_id));
        self.results
            .sort_by(|left, right| result_canonical_key(left).cmp(&result_canonical_key(right)));
    }
}

fn result_canonical_key(result: &ExecutedControlResultV1) -> ResultCanonicalKey<'_> {
    ResultCanonicalKey {
        obligation_id: result.obligation_id.as_str(),
        head: result.head.to_ascii_lowercase(),
        state: result.state.as_str(),
        command: result.command_or_instrument_id.as_str(),
        evidence: result.offered_evidence_kind.as_str(),
        observed: result.observed_outcome.as_str(),
        source: result.source_identity.as_str(),
        candidate_id: result.candidate_id.as_deref(),
        substitute_id: result.substitute_id.as_deref(),
        obligation_digest: result.obligation_digest.as_deref(),
        limitation: result.limitation.as_deref(),
        artifact_id: result
            .artifact
            .as_ref()
            .map(|artifact| artifact.logical_id.as_str()),
        artifact_digest: result
            .artifact
            .as_ref()
            .map(|artifact| artifact.digest.as_str()),
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct ResultCanonicalKey<'a> {
    obligation_id: &'a str,
    head: String,
    state: &'a str,
    command: &'a str,
    evidence: &'a str,
    observed: &'a str,
    source: &'a str,
    candidate_id: Option<&'a str>,
    substitute_id: Option<&'a str>,
    obligation_digest: Option<&'a str>,
    limitation: Option<&'a str>,
    artifact_id: Option<&'a str>,
    artifact_digest: Option<&'a str>,
}

fn bind_result(
    packet_source: &str,
    obligation: &ExecutedControlObligationV1,
    result: &ExecutedControlResultV1,
) -> Result<(), ExecutedControlValidationError> {
    if result.source_identity != packet_source {
        return Err(ExecutedControlValidationError::SourceIdentityMismatch {
            obligation_id: result.obligation_id.clone(),
            expected: packet_source.to_string(),
            actual: result.source_identity.clone(),
        });
    }
    match result.offered_evidence_kind {
        OfferedEvidenceKind::OrdinaryPositiveTest if result.state.claims_satisfaction() => {
            return Err(
                ExecutedControlValidationError::OrdinaryPositiveTestCannotSatisfy(
                    result.obligation_id.clone(),
                ),
            );
        }
        OfferedEvidenceKind::ReviewProse if result.state.claims_satisfaction() => {
            return Err(ExecutedControlValidationError::ReviewProseCannotSatisfy(
                result.obligation_id.clone(),
            ));
        }
        OfferedEvidenceKind::StructuralDiscriminationClaim
            if result.state.claims_satisfaction() =>
        {
            return Err(
                ExecutedControlValidationError::StructuralClaimCannotSatisfy(
                    result.obligation_id.clone(),
                ),
            );
        }
        _ => {}
    }

    if result.state == ResultState::Passed
        && result.offered_evidence_kind != OfferedEvidenceKind::ExecutedDiscriminatingControl
    {
        return Err(
            ExecutedControlValidationError::PassedRequiresExecutedControl(
                result.obligation_id.clone(),
            ),
        );
    }
    if result.state == ResultState::Substituted
        && result.offered_evidence_kind != OfferedEvidenceKind::DeclaredSubstitute
    {
        return Err(
            ExecutedControlValidationError::SubstituteRequiresDeclaredEvidence(
                result.obligation_id.clone(),
            ),
        );
    }

    if result.observed_outcome == ObservedOutcome::CommandSucceededWithoutExercisingSubject
        && result.state.claims_satisfaction()
    {
        return Err(
            ExecutedControlValidationError::WrongImplementationNotExercised(
                result.obligation_id.clone(),
            ),
        );
    }

    if result.state == ResultState::Passed
        && !obligation.acceptable_evidence_forms.iter().any(|form| {
            matches!(
                form,
                EvidenceForm::RetainedArtifact | EvidenceForm::BoundedLogCommitment
            )
        })
    {
        return Err(ExecutedControlValidationError::EvidenceFormNotAccepted {
            obligation_id: result.obligation_id.clone(),
            state: result.state,
        });
    }

    if result.state == ResultState::Substituted {
        if obligation.permitted_substitute.is_none() {
            return Err(ExecutedControlValidationError::SubstituteNotDeclared(
                result.obligation_id.clone(),
            ));
        }
        if !obligation
            .acceptable_evidence_forms
            .contains(&EvidenceForm::DeclaredSubstitute)
        {
            return Err(ExecutedControlValidationError::EvidenceFormNotAccepted {
                obligation_id: result.obligation_id.clone(),
                state: result.state,
            });
        }
        if substitute_outcome_is_unexecuted(result.observed_outcome) {
            return Err(
                ExecutedControlValidationError::UnexecutedSubstituteCannotSatisfy(
                    result.obligation_id.clone(),
                ),
            );
        }
    }

    let requires_artifact = result.state.claims_satisfaction()
        || (result.state == ResultState::Failed
            && result.offered_evidence_kind == OfferedEvidenceKind::ExecutedDiscriminatingControl);
    if requires_artifact && result.artifact.is_none() {
        return Err(ExecutedControlValidationError::MissingArtifact {
            obligation_id: result.obligation_id.clone(),
            state: result.state,
        });
    }

    if result.state.claims_satisfaction() {
        let expected_head = obligation
            .required_execution_subject
            .required_head
            .to_ascii_lowercase();
        if result.head.to_ascii_lowercase() != expected_head {
            return Err(ExecutedControlValidationError::HeadMismatch {
                obligation_id: result.obligation_id.clone(),
                expected: expected_head,
                actual: result.head.to_ascii_lowercase(),
            });
        }
        if result.command_or_instrument_id
            != obligation
                .required_execution_subject
                .command_or_instrument_id
            && result.state == ResultState::Passed
        {
            return Err(ExecutedControlValidationError::CommandMismatch {
                obligation_id: result.obligation_id.clone(),
                expected: obligation
                    .required_execution_subject
                    .command_or_instrument_id
                    .clone(),
                actual: result.command_or_instrument_id.clone(),
            });
        }
        if result.state == ResultState::Substituted {
            let Some(declared) = &obligation.permitted_substitute else {
                return Err(ExecutedControlValidationError::SubstituteNotDeclared(
                    result.obligation_id.clone(),
                ));
            };
            let Some(actual) = &result.substitute_id else {
                return Err(ExecutedControlValidationError::SubstituteMismatch {
                    obligation_id: result.obligation_id.clone(),
                    expected: declared.substitute_id.clone(),
                    actual: String::new(),
                });
            };
            if actual != &declared.substitute_id {
                return Err(ExecutedControlValidationError::SubstituteMismatch {
                    obligation_id: result.obligation_id.clone(),
                    expected: declared.substitute_id.clone(),
                    actual: actual.clone(),
                });
            }
            if result.command_or_instrument_id != declared.instrument_id {
                return Err(ExecutedControlValidationError::CommandMismatch {
                    obligation_id: result.obligation_id.clone(),
                    expected: declared.instrument_id.clone(),
                    actual: result.command_or_instrument_id.clone(),
                });
            }
        }
        if result.state == ResultState::Passed {
            let expected_outcome = match obligation.expected_discriminating_outcome {
                DiscriminatingOutcome::FailsBeforePassesAfter => {
                    ObservedOutcome::FailedBeforePassedAfter
                }
                DiscriminatingOutcome::RejectsWrongImplementation => {
                    ObservedOutcome::RejectedWrongImplementation
                }
            };
            if result.observed_outcome != expected_outcome {
                return Err(
                    ExecutedControlValidationError::ObservedOutcomeDoesNotMatch {
                        obligation_id: result.obligation_id.clone(),
                        expected: expected_outcome.as_str().to_string(),
                        actual: result.observed_outcome.as_str().to_string(),
                    },
                );
            }
        }
        let expected_digest = obligation.semantic_digest();
        match &result.obligation_digest {
            None => {
                return Err(ExecutedControlValidationError::MissingObligationDigest(
                    result.obligation_id.clone(),
                ));
            }
            Some(actual) if actual != &expected_digest => {
                return Err(ExecutedControlValidationError::ObligationDigestMismatch {
                    obligation_id: result.obligation_id.clone(),
                    expected: expected_digest,
                    actual: actual.clone(),
                });
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn substitute_outcome_is_unexecuted(outcome: ObservedOutcome) -> bool {
    matches!(
        outcome,
        ObservedOutcome::NotExecuted
            | ObservedOutcome::OrdinaryPositiveTestsPassed
            | ObservedOutcome::ReviewArgumentOnly
            | ObservedOutcome::StructuralDiscriminationOnly
            | ObservedOutcome::CommandSucceededWithoutExercisingSubject
    )
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ExecutedControlValidationError> {
    if value.trim().is_empty() {
        Err(ExecutedControlValidationError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_head(field: &'static str, value: &str) -> Result<(), ExecutedControlValidationError> {
    match GitObjectId::parse(value) {
        Ok(parsed) if parsed.hash_format() == super::GitHashFormat::Sha1 => Ok(()),
        Ok(_) | Err(_) => Err(ExecutedControlValidationError::InvalidHead {
            field,
            value: value.to_string(),
        }),
    }
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), ExecutedControlValidationError> {
    let Some(hex) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(ExecutedControlValidationError::InvalidDigest {
            field,
            value: value.to_string(),
        });
    };
    if hex.len() != SHA256_HEX_LENGTH
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ExecutedControlValidationError::InvalidDigest {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const HEAD_AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    pub(crate) const HEAD_BEFORE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    pub(crate) const HEAD_OTHER: &str = "cccccccccccccccccccccccccccccccccccccccc";
    pub(crate) const ARTIFACT_DIGEST: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    pub(crate) const COMMAND: &str = "cargo test -p example -- removed_guard_control";
    pub(crate) const OBLIGATION_ID: &str = "claim:example:removed-guard";

    pub(crate) fn sample_obligation() -> ExecutedControlObligationV1 {
        ExecutedControlObligationV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_OBLIGATION_KIND.to_string(),
            obligation_id: OBLIGATION_ID.to_string(),
            owning_claim: "issue:4641".to_string(),
            control_class: ControlClass::RemovedGuard,
            intended_wrong_implementation: "eager parse of the lazy tail".to_string(),
            required_execution_subject: ExecutionSubject {
                command_or_instrument_id: COMMAND.to_string(),
                named_wrong_implementation: "eager_file_count_parse".to_string(),
                required_head: HEAD_AFTER.to_string(),
            },
            expected_discriminating_outcome: DiscriminatingOutcome::FailsBeforePassesAfter,
            acceptable_evidence_forms: vec![
                EvidenceForm::RetainedArtifact,
                EvidenceForm::BoundedLogCommitment,
            ],
            permitted_substitute: None,
            requiredness: Requiredness::Required,
            invalidators: vec![
                Invalidator::SourceHeadMoved,
                Invalidator::ControlContractChanged,
                Invalidator::ArtifactMissing,
                Invalidator::CommandIdentityChanged,
            ],
        }
    }

    fn artifact() -> ArtifactIdentity {
        ArtifactIdentity {
            logical_id: "artifact:removed-guard-log".to_string(),
            digest: ARTIFACT_DIGEST.to_string(),
        }
    }

    pub(crate) fn passing_result(
        obligation: &ExecutedControlObligationV1,
    ) -> ExecutedControlResultV1 {
        ExecutedControlResultV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_RESULT_KIND.to_string(),
            obligation_id: obligation.obligation_id.clone(),
            source_identity: "EffortlessMetrics/ripr-swarm".to_string(),
            candidate_id: Some("candidate:example".to_string()),
            head: HEAD_AFTER.to_string(),
            command_or_instrument_id: obligation
                .required_execution_subject
                .command_or_instrument_id
                .clone(),
            artifact: Some(artifact()),
            observed_outcome: ObservedOutcome::FailedBeforePassedAfter,
            offered_evidence_kind: OfferedEvidenceKind::ExecutedDiscriminatingControl,
            state: ResultState::Passed,
            limitation: None,
            substitute_id: None,
            obligation_digest: Some(obligation.semantic_digest()),
        }
    }

    fn failed_before_result(obligation: &ExecutedControlObligationV1) -> ExecutedControlResultV1 {
        ExecutedControlResultV1 {
            head: HEAD_BEFORE.to_string(),
            observed_outcome: ObservedOutcome::RejectedWrongImplementation,
            state: ResultState::Failed,
            obligation_digest: None,
            ..passing_result(obligation)
        }
    }

    fn packet(
        obligations: Vec<ExecutedControlObligationV1>,
        results: Vec<ExecutedControlResultV1>,
    ) -> ExecutedControlPacketV1 {
        ExecutedControlPacketV1 {
            schema_version: EXECUTED_CONTROL_SCHEMA_VERSION.to_string(),
            kind: EXECUTED_CONTROL_PACKET_KIND.to_string(),
            source_identity: "EffortlessMetrics/ripr-swarm".to_string(),
            obligations,
            results,
        }
    }

    fn require_valid(packet: ExecutedControlPacketV1) -> Result<PacketEvaluation, String> {
        packet
            .validate()
            .map_err(|error| format!("unexpected validation error: {error}"))
    }

    fn require_invalid(
        packet: ExecutedControlPacketV1,
    ) -> Result<ExecutedControlValidationError, String> {
        match packet.validate() {
            Err(error) => Ok(error),
            Ok(_) => Err("expected validation to fail".to_string()),
        }
    }

    fn issue_3858_obligation() -> ExecutedControlObligationV1 {
        ExecutedControlObligationV1 {
            obligation_id: "issue:3858:eager-file-count-removal-control".to_string(),
            owning_claim: "issue:3858".to_string(),
            intended_wrong_implementation:
                "eager parse / removed lazy file-count guard (admit_file_count)".to_string(),
            required_execution_subject: ExecutionSubject {
                command_or_instrument_id: "cargo test -- removed_guard_or_eager_file_count_control"
                    .to_string(),
                named_wrong_implementation: "eager_admit_file_count".to_string(),
                required_head: HEAD_AFTER.to_string(),
            },
            ..sample_obligation()
        }
    }

    #[test]
    fn executed_removal_control_fails_before_and_passes_after_the_repair() -> Result<(), String> {
        let obligation = sample_obligation();
        let evaluation = require_valid(packet(
            vec![obligation.clone()],
            vec![
                failed_before_result(&obligation),
                passing_result(&obligation),
            ],
        ))?;
        assert_eq!(evaluation.satisfactions.len(), 1);
        assert!(evaluation.satisfactions[0].satisfies);
        assert_eq!(evaluation.satisfactions[0].state, Some(ResultState::Passed));
        Ok(())
    }

    #[test]
    fn ordinary_positive_test_cannot_silently_satisfy_the_obligation() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.offered_evidence_kind = OfferedEvidenceKind::OrdinaryPositiveTest;
        result.observed_outcome = ObservedOutcome::OrdinaryPositiveTestsPassed;
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::OrdinaryPositiveTestCannotSatisfy(_)
        ));
        Ok(())
    }

    #[test]
    fn prose_claim_without_artifact_cannot_pass() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.offered_evidence_kind = OfferedEvidenceKind::ReviewProse;
        result.observed_outcome = ObservedOutcome::ReviewArgumentOnly;
        result.artifact = None;
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::ReviewProseCannotSatisfy(_)
        ));
        Ok(())
    }

    #[test]
    fn control_run_on_another_head_cannot_pass() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.head = HEAD_OTHER.to_string();
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::HeadMismatch { .. }
        ));
        Ok(())
    }

    #[test]
    fn command_success_without_named_wrong_implementation_cannot_pass() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.observed_outcome = ObservedOutcome::CommandSucceededWithoutExercisingSubject;
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::WrongImplementationNotExercised(_)
        ));
        Ok(())
    }

    #[test]
    fn unavailable_instrument_with_declared_substitute_is_explicit_and_satisfying()
    -> Result<(), String> {
        let mut obligation = sample_obligation();
        obligation
            .acceptable_evidence_forms
            .push(EvidenceForm::DeclaredSubstitute);
        obligation.permitted_substitute = Some(PermittedSubstitute {
            substitute_id: "hosted-eager-variant".to_string(),
            instrument_id: "hosted-mutation-runner".to_string(),
            evidence_form: EvidenceForm::DeclaredSubstitute,
        });
        let result = ExecutedControlResultV1 {
            command_or_instrument_id: "hosted-mutation-runner".to_string(),
            offered_evidence_kind: OfferedEvidenceKind::DeclaredSubstitute,
            observed_outcome: ObservedOutcome::InstrumentUnavailable,
            state: ResultState::Substituted,
            substitute_id: Some("hosted-eager-variant".to_string()),
            obligation_digest: Some(obligation.semantic_digest()),
            limitation: Some(
                "local eager-variant instrument unavailable; declared hosted substitute ran"
                    .to_string(),
            ),
            ..passing_result(&obligation)
        };
        let evaluation = require_valid(packet(vec![obligation], vec![result]))?;
        assert!(evaluation.satisfactions[0].satisfies);
        assert_eq!(
            evaluation.satisfactions[0].state,
            Some(ResultState::Substituted)
        );
        Ok(())
    }

    #[test]
    fn unavailable_instrument_without_substitute_stays_explicit_and_non_satisfying()
    -> Result<(), String> {
        let obligation = sample_obligation();
        let result = ExecutedControlResultV1 {
            offered_evidence_kind: OfferedEvidenceKind::ExecutedDiscriminatingControl,
            observed_outcome: ObservedOutcome::InstrumentUnavailable,
            state: ResultState::InstrumentFailure,
            artifact: None,
            obligation_digest: None,
            limitation: Some("eager-variant instrument was not available".to_string()),
            ..passing_result(&obligation)
        };
        let evaluation = require_valid(packet(vec![obligation], vec![result]))?;
        assert!(!evaluation.satisfactions[0].satisfies);
        assert_eq!(
            evaluation.satisfactions[0].state,
            Some(ResultState::InstrumentFailure)
        );
        Ok(())
    }

    #[test]
    fn inferred_substitute_without_declaration_is_rejected() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.state = ResultState::Substituted;
        result.offered_evidence_kind = OfferedEvidenceKind::DeclaredSubstitute;
        result.substitute_id = Some("invented-after-the-fact".to_string());
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::SubstituteNotDeclared(_)
        ));
        Ok(())
    }

    #[test]
    fn duplicated_obligation_ids_are_rejected() -> Result<(), String> {
        let obligation = sample_obligation();
        let error = require_invalid(packet(
            vec![obligation.clone(), obligation.clone()],
            vec![passing_result(&obligation)],
        ))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::DuplicateObligationId(_)
        ));
        Ok(())
    }

    #[test]
    fn result_for_unknown_obligation_is_rejected() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.obligation_id = "claim:unknown".to_string();
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::UnknownObligation(_)
        ));
        Ok(())
    }

    #[test]
    fn stale_obligation_digest_cannot_pass() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.obligation_digest = Some(
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        );
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::ObligationDigestMismatch { .. }
        ));
        Ok(())
    }

    #[test]
    fn issue_3858_documentation_fixture_stays_not_proven() -> Result<(), String> {
        let obligation = issue_3858_obligation();
        let result = ExecutedControlResultV1 {
            offered_evidence_kind: OfferedEvidenceKind::ReviewProse,
            observed_outcome: ObservedOutcome::NotExecuted,
            state: ResultState::NotProven,
            artifact: None,
            obligation_digest: None,
            limitation: Some(
                "Issue #3858 / PR #4063 recorded no retained eager or removed-guard execution artifact. Execution is not_proven and must not be rewritten as passed."
                    .to_string(),
            ),
            ..passing_result(&obligation)
        };
        let evaluation = require_valid(packet(vec![obligation], vec![result]))?;
        assert!(!evaluation.satisfactions[0].satisfies);
        assert_eq!(
            evaluation.satisfactions[0].state,
            Some(ResultState::NotProven)
        );
        Ok(())
    }

    #[test]
    fn semantic_digest_is_independent_of_form_and_invalidator_order() {
        let mut left = sample_obligation();
        let mut right = sample_obligation();
        left.acceptable_evidence_forms = vec![
            EvidenceForm::BoundedLogCommitment,
            EvidenceForm::RetainedArtifact,
        ];
        right.acceptable_evidence_forms = vec![
            EvidenceForm::RetainedArtifact,
            EvidenceForm::BoundedLogCommitment,
        ];
        left.invalidators = vec![
            Invalidator::ArtifactMissing,
            Invalidator::SourceHeadMoved,
            Invalidator::CommandIdentityChanged,
            Invalidator::ControlContractChanged,
        ];
        assert_eq!(left.semantic_digest(), right.semantic_digest());
    }

    #[test]
    fn canonicalize_orders_obligations_and_results_deterministically() {
        let first = sample_obligation();
        let mut second = sample_obligation();
        second.obligation_id = "claim:example:other".to_string();
        let mut reversed = packet(
            vec![second.clone(), first.clone()],
            vec![passing_result(&second), passing_result(&first)],
        );
        let mut ordered = packet(
            vec![first.clone(), second.clone()],
            vec![passing_result(&first), passing_result(&second)],
        );
        reversed.canonicalize();
        ordered.canonicalize();
        assert_eq!(reversed, ordered);
    }

    #[test]
    fn missing_result_is_non_satisfying_not_an_inferred_pass() -> Result<(), String> {
        let evaluation = require_valid(packet(vec![sample_obligation()], Vec::new()))?;
        assert!(!evaluation.satisfactions[0].satisfies);
        assert_eq!(evaluation.satisfactions[0].state, None);
        Ok(())
    }

    #[test]
    fn structural_discrimination_claim_cannot_pass() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.offered_evidence_kind = OfferedEvidenceKind::StructuralDiscriminationClaim;
        result.observed_outcome = ObservedOutcome::StructuralDiscriminationOnly;
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::StructuralClaimCannotSatisfy(_)
        ));
        Ok(())
    }

    #[test]
    fn foreign_source_identity_cannot_satisfy() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut result = passing_result(&obligation);
        result.source_identity = "other/repo".to_string();
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::SourceIdentityMismatch { .. }
        ));
        Ok(())
    }

    #[test]
    fn unexecuted_substitute_cannot_satisfy() -> Result<(), String> {
        let mut obligation = sample_obligation();
        obligation
            .acceptable_evidence_forms
            .push(EvidenceForm::DeclaredSubstitute);
        obligation.permitted_substitute = Some(PermittedSubstitute {
            substitute_id: "hosted-eager-variant".to_string(),
            instrument_id: "hosted-mutation-runner".to_string(),
            evidence_form: EvidenceForm::DeclaredSubstitute,
        });
        let result = ExecutedControlResultV1 {
            command_or_instrument_id: "hosted-mutation-runner".to_string(),
            offered_evidence_kind: OfferedEvidenceKind::DeclaredSubstitute,
            observed_outcome: ObservedOutcome::NotExecuted,
            state: ResultState::Substituted,
            substitute_id: Some("hosted-eager-variant".to_string()),
            obligation_digest: Some(obligation.semantic_digest()),
            ..passing_result(&obligation)
        };
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::UnexecutedSubstituteCannotSatisfy(_)
        ));
        Ok(())
    }

    #[test]
    fn passed_result_requires_an_artifact_backed_acceptable_form() -> Result<(), String> {
        let mut obligation = sample_obligation();
        obligation.acceptable_evidence_forms = vec![EvidenceForm::DeclaredSubstitute];
        obligation.permitted_substitute = Some(PermittedSubstitute {
            substitute_id: "hosted-eager-variant".to_string(),
            instrument_id: "hosted-mutation-runner".to_string(),
            evidence_form: EvidenceForm::DeclaredSubstitute,
        });
        let result = passing_result(&obligation);
        let error = require_invalid(packet(vec![obligation], vec![result]))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::EvidenceFormNotAccepted { .. }
        ));
        Ok(())
    }

    #[test]
    fn same_head_pass_and_fail_are_rejected() -> Result<(), String> {
        let obligation = sample_obligation();
        let mut failed = passing_result(&obligation);
        failed.state = ResultState::Failed;
        failed.observed_outcome = ObservedOutcome::RejectedWrongImplementation;
        failed.obligation_digest = None;
        let error = require_invalid(packet(
            vec![obligation.clone()],
            vec![failed, passing_result(&obligation)],
        ))?;
        assert!(matches!(
            error,
            ExecutedControlValidationError::ConflictingSameSubjectResults(_)
        ));
        Ok(())
    }

    #[test]
    fn canonicalize_orders_equal_key_results_by_payload() {
        let obligation = sample_obligation();
        let mut left = passing_result(&obligation);
        left.state = ResultState::NotProven;
        left.offered_evidence_kind = OfferedEvidenceKind::ReviewProse;
        left.observed_outcome = ObservedOutcome::NotExecuted;
        left.artifact = None;
        left.obligation_digest = None;
        left.limitation = Some("runner A unavailable".to_string());
        let mut right = left.clone();
        right.limitation = Some("runner B unavailable".to_string());
        let mut reversed = packet(vec![obligation.clone()], vec![right.clone(), left.clone()]);
        let mut ordered = packet(vec![obligation], vec![left, right]);
        reversed.canonicalize();
        ordered.canonicalize();
        assert_eq!(reversed, ordered);
    }
}
