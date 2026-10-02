//! Blind-journey contract and validator (#4603, RIPR-SPEC-0200).
//!
//! Typed, versioned DTOs for an auditable blind installed-agent journey:
//! `BlindJourneyPromptV1`, `BlindJourneyAnswerKeyV1`, `BlindJourneyEventV1`,
//! `BlindJourneyInterventionV1` and `BlindJourneyReceiptV1`, plus the offline
//! validator that decides whether one packet is an internally consistent,
//! audit-complete transcript. Candidate/process/artifact admission stays with
//! the #4510 harness; this contract references those identities and validates
//! the operator-visible transcript rather than copying admission authority.
//!
//! Two contamination judgments stay mechanically distinct: a closed,
//! structured mechanical scan over the exact prompt bytes rejects evaluator
//! only fields, internal references and preselected item/target/command/artifact
//! hints, and a retained named-reviewer verdict over the exact prompt and the
//! separately stored answer key addresses semantic answer leakage. A reviewer
//! label never overrides a mechanical finding, and a missing or non-accepted
//! review refuses a positive blind result.
//!
//! The validator runs no candidate, launches no process and decides no release
//! verdict; synthetic validator success claims only that one committed packet
//! is an auditable transcript under this contract.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const BLIND_JOURNEY_PROMPT_SCHEMA_VERSION: &str = "blind_journey_prompt.v1";
pub(crate) const BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION: &str = "blind_journey_answer_key.v1";
pub(crate) const BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION: &str = "blind_journey_receipt.v1";
pub(crate) const BLIND_JOURNEY_CLAIM_BOUNDARY: &str = "Static blind-journey transcript contract: \
 validator success defines auditability of one committed operator-visible transcript only; it \
 claims no installed usefulness, no candidate qualification, no blind acceptance and no release \
 verdict.";

/// One closed intervention taxonomy (#4603). The first four may appear in a
/// positive run; `InstrumentOnlyNotOperatorVisible` may record watchdog,
/// process or filesystem observation without changing the operator path or
/// product state; the remaining five make the positive blind claim false.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyInterventionV1 {
    ProductSupported,
    OrdinarySourceInspection,
    PublicDocumentationLookup,
    OperatorChoiceWithinProductOptions,
    HiddenOperatorKnowledge,
    ManualArtifactPlumbing,
    PrivateHarnessHint,
    WorkspaceBinarySubstitution,
    UnsafeOrUnboundedAction,
    InstrumentOnlyNotOperatorVisible,
}

impl BlindJourneyInterventionV1 {
    /// Interventions that may appear in a positive run.
    pub(crate) fn positive_allowed(self) -> bool {
        matches!(
            self,
            Self::ProductSupported
                | Self::OrdinarySourceInspection
                | Self::PublicDocumentationLookup
                | Self::OperatorChoiceWithinProductOptions
        )
    }

    /// The terminal result this intervention forces when present in a run.
    pub(crate) fn disqualifying_result(self) -> Option<BlindJourneyResultV1> {
        match self {
            Self::HiddenOperatorKnowledge | Self::PrivateHarnessHint => {
                Some(BlindJourneyResultV1::HiddenOperatorAssistance)
            }
            Self::ManualArtifactPlumbing => Some(BlindJourneyResultV1::ManualArtifactPlumbing),
            Self::WorkspaceBinarySubstitution => {
                Some(BlindJourneyResultV1::CandidateIdentityFailure)
            }
            Self::UnsafeOrUnboundedAction => Some(BlindJourneyResultV1::UnsafeOrWrongEdit),
            Self::ProductSupported
            | Self::OrdinarySourceInspection
            | Self::PublicDocumentationLookup
            | Self::OperatorChoiceWithinProductOptions
            | Self::InstrumentOnlyNotOperatorVisible => None,
        }
    }
}

/// Observable activity kinds. The transcript captures observable actions only;
/// it never requires or stores private chain-of-thought.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyEventKindV1 {
    ProductCommandInvocation,
    ProductOutputFollowed,
    PublicDocumentationLookup,
    OrdinaryTargetSourceRead,
    OperatorQuestion,
    OperatorProductOptionSelection,
    FileEdit,
    ProjectVerificationExecution,
    StaticAnalysisExecution,
    ReceiptExecution,
    ProcessCancellationRestartCleanup,
    HarnessIntervention,
}

impl BlindJourneyEventKindV1 {
    /// Events the operator originates. Their subjects record what the operator
    /// actually saw or did, so an honest transcript may legitimately name a
    /// quiet neighbor, a forbidden path or a limitation; the answer-key secrecy
    /// scan must not treat the operator's own actions as a leak. Harness- and
    /// verifier-originated events stay in the scanned projection: their
    /// subjects and reasons are written by the evaluator side.
    pub(crate) fn operator_originated(self) -> bool {
        matches!(
            self,
            Self::ProductCommandInvocation
                | Self::ProductOutputFollowed
                | Self::PublicDocumentationLookup
                | Self::OrdinaryTargetSourceRead
                | Self::OperatorQuestion
                | Self::OperatorProductOptionSelection
                | Self::FileEdit
        )
    }
}

/// The one terminal result vocabulary (#4603). Only
/// `passed_blind_journey` is the positive row; negative controls retain their
/// own expected result and no aggregate percentage converts them into success.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyResultV1 {
    PassedBlindJourney,
    HonestLimitation,
    ProductDiscoverabilityFailure,
    HiddenOperatorAssistance,
    ManualArtifactPlumbing,
    UnsafeOrWrongEdit,
    WrongOrStaleSubject,
    VerificationFailureVisible,
    VerificationNotRunVisible,
    FalseOrStrengthenedReceipt,
    CandidateIdentityFailure,
    InstrumentFailure,
    NotRun,
}

impl BlindJourneyResultV1 {
    /// Results a producer may legitimately claim for a complete packet.
    /// `FalseOrStrengthenedReceipt` and `InstrumentFailure` are evaluator
    /// judgments over broken or absent packets, not producer claims.
    pub(crate) fn producer_claimable(self) -> bool {
        !matches!(
            self,
            Self::FalseOrStrengthenedReceipt | Self::InstrumentFailure
        )
    }
}

/// Retained reviewer verdict over the exact prompt bytes and the separately
/// stored answer key. Keyword absence is not evidence of semantic blindness,
/// so a named reviewer verdict is required before any positive blind result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyPromptReviewVerdictV1 {
    Accepted,
    Rejected,
    NotReviewed,
    Stale,
}

/// The mechanical contamination judgment recorded by the producer. The
/// validator recomputes the scan and rejects a packet whose recorded result
/// disagrees with the recomputed one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyContaminationResultV1 {
    MechanicallyClean,
    MechanicallyContaminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyCurrentnessV1 {
    Current,
    Stale,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneySelectionCorrectnessV1 {
    CorrectActionable,
    QuietNeighborSelected,
    NotActionable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyEditCageVerdictV1 {
    WithinCage,
    Violation,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyVerificationStatusV1 {
    Passed,
    Failed,
    NotRun,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyStaticMovementV1 {
    Improved,
    Unchanged,
    Regressed,
    Unknown,
}

/// Receipt-movement axis; the state names deliberately reuse the existing
/// receipt lifecycle vocabulary instead of inventing a parallel one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyReceiptStatusV1 {
    ReceiptFound,
    ReceiptGapMismatch,
    ReceiptMissing,
    ReceiptMovementImproved,
    ReceiptMovementUnchanged,
    ReceiptNotApplicable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyAssistanceStateV1 {
    NoneObserved,
    AssistanceObserved,
    Unknown,
}

/// Candidate/package/binary identity reference plus the target
/// repository/base/head/tree/root identity. Concrete root spelling is retained
/// evidence but stays telemetry: it never enters the portable identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyCandidateRefV1 {
    pub repository: String,
    pub base: String,
    pub head: String,
    pub tree: String,
    pub root: String,
    pub package: String,
    pub binary: String,
    pub binary_digest: String,
}

impl BlindJourneyCandidateRefV1 {
    /// Identity comparison that ignores concrete root spelling, so equivalent
    /// roots stay portable while concrete root evidence remains retained.
    pub(crate) fn portable_eq(&self, other: &Self) -> bool {
        self.repository == other.repository
            && self.base == other.base
            && self.head == other.head
            && self.tree == other.tree
            && self.package == other.package
            && self.binary == other.binary
            && self.binary_digest == other.binary_digest
    }
}

/// The retained reviewer verdict binding. The review covers the exact prompt
/// bytes and the separately stored answer key; both digests must match the
/// packet contents or the review is stale.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyPromptReviewV1 {
    pub reviewer: String,
    pub verdict: BlindJourneyPromptReviewVerdictV1,
    pub prompt_digest: String,
    pub answer_key_digest: String,
}

/// What the operator was initially told. A generic safe-repair goal, the
/// target repository location and permission to inspect ordinary source are
/// allowed; the mechanical scan rejects evaluator-only content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyPromptV1 {
    pub schema_version: String,
    pub candidate: BlindJourneyCandidateRefV1,
    pub operator_goal: String,
    /// Public docs/help inputs made available to the operator.
    pub public_inputs: Vec<String>,
    /// Ordinary permissions, e.g. running public product entrypoints and
    /// project verification commands.
    pub ordinary_permissions: Vec<String>,
    /// Explicit prohibited hints restated to the operator.
    pub prohibited_hints: Vec<String>,
    /// Exact operator-visible prompt bytes.
    pub prompt_bytes: String,
    /// SHA-256 hex over the canonical operator-visible prompt surface (this
    /// byte string plus every restated operator-visible field).
    pub prompt_digest: String,
    pub review: BlindJourneyPromptReviewV1,
    /// Producer-recorded mechanical scan result over the operator-visible
    /// prompt surface.
    pub contamination_result: BlindJourneyContaminationResultV1,
}

/// Evaluator-only answer key, stored separately from the operator input. The
/// operator never receives it; the receipt compares the recorded selection
/// against it without exposing it during execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyAnswerKeyV1 {
    pub schema_version: String,
    /// Eligible canonical items; several may be represented.
    pub eligible_items: Vec<String>,
    /// Quiet/already-covered neighbors that must not be presented as
    /// actionable.
    pub quiet_neighbors: Vec<String>,
    /// Expected safe target/edit cage.
    pub expected_edit_cage: Vec<String>,
    pub discriminator_family: String,
    pub known_product_limitations: Vec<String>,
    /// Forbidden production/unrelated edits.
    pub forbidden_edits: Vec<String>,
    /// SHA-256 hex over the canonical digest input of every field above.
    pub answer_key_digest: String,
}

/// One observable transcript event. Each event binds sequence/predecessor,
/// the exact subject, input/output digests and whether it was operator visible
/// at that point.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyEventV1 {
    /// 1-based position in the transcript.
    pub sequence: u64,
    /// Sequence of the preceding event; `None` only for the first event.
    pub predecessor_sequence: Option<u64>,
    pub kind: BlindJourneyEventKindV1,
    /// Exact subject: argv digest reference, path, doc id or option id.
    pub subject: String,
    pub input_digest: Option<String>,
    pub output_digest: Option<String>,
    pub operator_visible: bool,
    /// Classification required for every non-product action.
    pub intervention: Option<BlindJourneyInterventionV1>,
    /// Required when `intervention` is present.
    pub actor: Option<String>,
    /// Required when `intervention` is present; free-form "assisted a little"
    /// is invalid, so the reason must name the exact basis.
    pub reason: Option<String>,
}

/// Evidence axes referenced, never collapsed: a positive static or
/// verification axis cannot hide `hidden_operator_knowledge`, and an honest
/// limitation cannot become a completed repair.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyEvidenceAxesV1 {
    pub candidate_currentness: BlindJourneyCurrentnessV1,
    pub selection_correctness: BlindJourneySelectionCorrectnessV1,
    pub edit_cage_verdict: BlindJourneyEditCageVerdictV1,
    pub project_verification_status: BlindJourneyVerificationStatusV1,
    pub static_movement: BlindJourneyStaticMovementV1,
    pub receipt_status: BlindJourneyReceiptStatusV1,
    pub operator_assistance_state: BlindJourneyAssistanceStateV1,
    pub external_runtime_mutation_evidence: Option<String>,
}

/// The journey receipt. `terminal_result` is the producer's claim; the
/// validator checks it against the transcript, the answer-key comparison and
/// the retained review, and rejects strengthened or hidden-assistance claims.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyReceiptV1 {
    pub schema_version: String,
    pub candidate: BlindJourneyCandidateRefV1,
    /// Digest binding to the exact reviewed prompt bytes.
    pub prompt_digest: Option<String>,
    /// Digest binding to the exact answer key used for comparison.
    pub answer_key_digest: Option<String>,
    pub review: BlindJourneyPromptReviewV1,
    /// Operator selection, compared against the answer key.
    pub selected_item: Option<String>,
    pub selected_edit: Option<String>,
    pub events: Vec<BlindJourneyEventV1>,
    pub axes: BlindJourneyEvidenceAxesV1,
    pub terminal_result: BlindJourneyResultV1,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
}

/// One committed blind-journey packet: prompt, separately stored answer key
/// and receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlindJourneyPacketV1 {
    pub prompt: BlindJourneyPromptV1,
    pub answer_key: BlindJourneyAnswerKeyV1,
    pub receipt: BlindJourneyReceiptV1,
}

/// One mechanical contamination finding over the exact prompt bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct BlindJourneyContaminationFindingV1 {
    pub category: &'static str,
    pub matched_pattern: String,
}

/// The validator's assessment of one packet. `accepted` means the receipt is
/// internally consistent and audit-complete under this contract; a non-positive
/// terminal can absolutely be accepted. `positive` is exactly
/// `terminal_result == passed_blind_journey` on an accepted packet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct BlindJourneyAssessmentV1 {
    pub accepted: bool,
    pub terminal_result: BlindJourneyResultV1,
    pub rejection_reasons: Vec<String>,
    pub contamination_findings: Vec<BlindJourneyContaminationFindingV1>,
    pub disqualifiers: Vec<BlindJourneyResultV1>,
    pub event_count: usize,
    pub portable_identity: String,
}

impl BlindJourneyAssessmentV1 {
    pub(crate) fn positive(&self) -> bool {
        self.accepted && self.terminal_result == BlindJourneyResultV1::PassedBlindJourney
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The scoped semantic surface of the answer key: every field except the
/// digest itself. Field order is the canonical digest identity.
#[derive(Serialize)]
struct BlindJourneyAnswerKeyDigestInput<'a> {
    schema_version: &'a str,
    eligible_items: &'a [String],
    quiet_neighbors: &'a [String],
    expected_edit_cage: &'a [String],
    discriminator_family: &'a str,
    known_product_limitations: &'a [String],
    forbidden_edits: &'a [String],
}

/// The operator-visible surface of the prompt: every field the operator was
/// actually shown. Field order is the canonical digest identity; the review,
/// the mechanical scan and the secrecy projection all bind this same text, so
/// a producer cannot restate a reviewed field after review.
#[derive(Serialize)]
struct BlindJourneyPromptDigestInput<'a> {
    schema_version: &'a str,
    operator_goal: &'a str,
    public_inputs: &'a [String],
    ordinary_permissions: &'a [String],
    prohibited_hints: &'a [String],
    prompt_bytes: &'a str,
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("canonical serialization failed: {error}"))
}

/// SHA-256 hex over the canonical answer-key digest input.
pub(crate) fn blind_journey_answer_key_digest(
    answer_key: &BlindJourneyAnswerKeyV1,
) -> Result<String, String> {
    let input = BlindJourneyAnswerKeyDigestInput {
        schema_version: &answer_key.schema_version,
        eligible_items: &answer_key.eligible_items,
        quiet_neighbors: &answer_key.quiet_neighbors,
        expected_edit_cage: &answer_key.expected_edit_cage,
        discriminator_family: &answer_key.discriminator_family,
        known_product_limitations: &answer_key.known_product_limitations,
        forbidden_edits: &answer_key.forbidden_edits,
    };
    Ok(sha256_hex(canonical_json(&input)?.as_bytes()))
}

/// SHA-256 hex over the canonical operator-visible prompt surface: the exact
/// prompt bytes plus every restated operator-visible field.
fn prompt_digest(prompt: &BlindJourneyPromptV1) -> String {
    let input = BlindJourneyPromptDigestInput {
        schema_version: &prompt.schema_version,
        operator_goal: &prompt.operator_goal,
        public_inputs: &prompt.public_inputs,
        ordinary_permissions: &prompt.ordinary_permissions,
        prohibited_hints: &prompt.prohibited_hints,
        prompt_bytes: &prompt.prompt_bytes,
    };
    match canonical_json(&input) {
        Ok(json) => sha256_hex(json.as_bytes()),
        Err(error) => format!("prompt_digest_error:{error}"),
    }
}

/// Portable semantic identity of one packet: candidate identity without
/// concrete root spelling, plus the prompt and answer-key digest bindings.
/// Equivalent roots therefore share one portable identity while concrete root
/// evidence remains retained in the packet.
pub(crate) fn blind_journey_portable_identity(
    packet: &BlindJourneyPacketV1,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct PortableIdentityInput<'a> {
        schema_version: &'a str,
        repository: &'a str,
        base: &'a str,
        head: &'a str,
        tree: &'a str,
        package: &'a str,
        binary: &'a str,
        binary_digest: &'a str,
        prompt_digest: &'a str,
        answer_key_digest: &'a str,
    }
    let input = PortableIdentityInput {
        schema_version: &packet.receipt.schema_version,
        repository: &packet.receipt.candidate.repository,
        base: &packet.receipt.candidate.base,
        head: &packet.receipt.candidate.head,
        tree: &packet.receipt.candidate.tree,
        package: &packet.receipt.candidate.package,
        binary: &packet.receipt.candidate.binary,
        binary_digest: &packet.receipt.candidate.binary_digest,
        prompt_digest: packet.receipt.prompt_digest.as_deref().unwrap_or_default(),
        answer_key_digest: packet
            .receipt
            .answer_key_digest
            .as_deref()
            .unwrap_or_default(),
    };
    Ok(sha256_hex(canonical_json(&input)?.as_bytes()))
}

/// The accepted producer (#4603): stamp digest bindings and the mechanical
/// contamination result onto one packet before validation or execution. A
/// local modification to this producer invalidates a #4604 run rather than
/// becoming an execution convenience.
pub(crate) fn stamp_blind_journey_packet(
    mut prompt: BlindJourneyPromptV1,
    mut answer_key: BlindJourneyAnswerKeyV1,
    mut receipt: BlindJourneyReceiptV1,
) -> Result<BlindJourneyPacketV1, String> {
    if !prompt.candidate.portable_eq(&receipt.candidate) {
        return Err(
            "blind journey packet is refused: receipt candidate identity does not match the prompt candidate identity"
                .to_string(),
        );
    }
    let prompt_digest = prompt_digest(&prompt);
    let answer_key_digest = blind_journey_answer_key_digest(&answer_key)?;
    if !prompt.review.reviewer.trim().is_empty() {
        // An empty binding is stamped now; a present but different binding is
        // a stale review and refuses the packet.
        if prompt.review.prompt_digest.is_empty() {
            prompt.review.prompt_digest = prompt_digest.clone();
        } else if prompt.review.prompt_digest != prompt_digest {
            return Err(
                "blind journey packet is refused: prompt review binds different prompt bytes"
                    .to_string(),
            );
        }
        if prompt.review.answer_key_digest.is_empty() {
            prompt.review.answer_key_digest = answer_key_digest.clone();
        } else if prompt.review.answer_key_digest != answer_key_digest {
            return Err(
                "blind journey packet is refused: prompt review binds a different answer key"
                    .to_string(),
            );
        }
    }
    prompt.prompt_digest = prompt_digest.clone();
    prompt.contamination_result = if mechanical_contamination_findings(&prompt).is_empty() {
        BlindJourneyContaminationResultV1::MechanicallyClean
    } else {
        BlindJourneyContaminationResultV1::MechanicallyContaminated
    };
    answer_key.answer_key_digest = answer_key_digest.clone();
    receipt.prompt_digest = Some(prompt_digest);
    receipt.answer_key_digest = Some(answer_key_digest);
    // The receipt retains the same review, not a second opinion: the producer
    // copies the reviewed binding instead of restating it.
    receipt.review = prompt.review.clone();
    Ok(BlindJourneyPacketV1 {
        prompt,
        answer_key,
        receipt,
    })
}

/// The exact operator-visible prompt surface: the goal, restated inputs,
/// permissions and hints, and the exact prompt bytes. The review digest, the
/// mechanical contamination scan and the answer-key secrecy projection all
/// bind this one text, so none of them can be restated after review.
pub(crate) fn prompt_operator_visible_text(prompt: &BlindJourneyPromptV1) -> String {
    let mut text = String::new();
    text.push_str(&prompt.operator_goal);
    text.push('\n');
    for input in &prompt.public_inputs {
        text.push_str(input);
        text.push('\n');
    }
    for permission in &prompt.ordinary_permissions {
        text.push_str(permission);
        text.push('\n');
    }
    for hint in &prompt.prohibited_hints {
        text.push_str(hint);
        text.push('\n');
    }
    text.push_str(&prompt.prompt_bytes);
    text.push('\n');
    text
}

/// Mechanical contamination scan over the exact operator-visible prompt
/// surface. Each category is checked independently; every match is a finding.
/// This is the closed structured judgment: it rejects evaluator-only fields,
/// internal references and preselected item/target/command/artifact hints.
/// Keyword absence is not evidence of semantic blindness, which is why the
/// retained reviewer verdict stays a separate, required judgment.
pub(crate) fn mechanical_contamination_findings(
    prompt: &BlindJourneyPromptV1,
) -> Vec<BlindJourneyContaminationFindingV1> {
    const PATTERN_CATEGORIES: [(&str, [&str; 3]); 9] = [
        (
            "internal_reference",
            ["issues/", "pull/", "github.com/effortlessmetrics"],
        ),
        (
            "fixture_family_selection",
            ["fixture family", "corpus", "journey corpus"],
        ),
        (
            "preselected_item_reference",
            ["ripr-spec-", "ripr-prop-", "gap-"],
        ),
        ("target_file_or_edit", [".rs:", "/tests/", "test_"]),
        (
            "expected_command_sequence",
            ["cargo test --", "cargo xtask", "cargo nextest"],
        ),
        (
            "private_artifact_path",
            ["target/ripr/", "receipt.json", ".patch"],
        ),
        (
            "private_expected_result",
            ["answer key", "expected result", "workaround"],
        ),
        (
            "internal_source_reference",
            ["ripr-swarm", "crates/ripr", ".agents/"],
        ),
        (
            "prior_transcript_reference",
            ["transcript", "prior journey", "earlier attempt"],
        ),
    ];
    let text = prompt_operator_visible_text(prompt).to_ascii_lowercase();
    let mut findings = Vec::new();
    for (category, patterns) in PATTERN_CATEGORIES {
        for pattern in patterns {
            if text.contains(pattern) {
                findings.push(BlindJourneyContaminationFindingV1 {
                    category,
                    matched_pattern: pattern.to_string(),
                });
            }
        }
    }
    // Issue/PR numbers ("#1234") get their own scan so an ordinary '#' in
    // prose does not false-positive.
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            findings.push(BlindJourneyContaminationFindingV1 {
                category: "internal_reference",
                matched_pattern: "#<digits>".to_string(),
            });
            break;
        }
    }
    findings
}

/// Operator-visible material only: the exact operator-visible prompt surface
/// and the subjects/reasons of operator-visible harness- or verifier-originated
/// events. Operator-originated events (selections, edits, ordinary reads,
/// questions) are excluded: an honest transcript records what the operator
/// actually did, which may legitimately name a quiet neighbor or a forbidden
/// path. Answer-key secrecy is tested over exactly this projection.
pub(crate) fn blind_journey_operator_visible_text(packet: &BlindJourneyPacketV1) -> String {
    let mut text = prompt_operator_visible_text(&packet.prompt);
    for event in &packet.receipt.events {
        if event.operator_visible && !event.kind.operator_originated() {
            text.push_str(&event.subject);
            text.push('\n');
            if let Some(reason) = &event.reason {
                text.push_str(reason);
                text.push('\n');
            }
        }
    }
    text
}

fn event_kind_present(packet: &BlindJourneyPacketV1, kind: BlindJourneyEventKindV1) -> bool {
    packet.receipt.events.iter().any(|event| event.kind == kind)
}

/// Validate one stamped packet. The assessment collects every violation in a
/// fixed check order, so exact inputs produce deterministic output. A
/// non-positive terminal result on an accepted packet is a valid honest
/// receipt; only `passed_blind_journey` is the positive row.
pub(crate) fn assess_blind_journey_packet(
    packet: &BlindJourneyPacketV1,
) -> BlindJourneyAssessmentV1 {
    let mut reasons: Vec<String> = Vec::new();

    // 1. Schema versions: an unsupported future schema stays a rejection, not
    //    a clean pass.
    if packet.prompt.schema_version != BLIND_JOURNEY_PROMPT_SCHEMA_VERSION {
        reasons.push(format!(
            "unsupported_schema:prompt:{}",
            packet.prompt.schema_version
        ));
    }
    if packet.answer_key.schema_version != BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION {
        reasons.push(format!(
            "unsupported_schema:answer_key:{}",
            packet.answer_key.schema_version
        ));
    }
    if packet.receipt.schema_version != BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION {
        reasons.push(format!(
            "unsupported_schema:receipt:{}",
            packet.receipt.schema_version
        ));
    }

    // 2. Digest bindings: recompute and compare every binding.
    let expected_prompt_digest = prompt_digest(&packet.prompt);
    if packet.prompt.prompt_digest != expected_prompt_digest {
        reasons.push(
            "prompt_digest_mismatch:recorded digest does not match the prompt bytes".to_string(),
        );
    }
    let expected_key_digest = match blind_journey_answer_key_digest(&packet.answer_key) {
        Ok(digest) => digest,
        Err(error) => format!("answer_key_digest_error:{error}"),
    };
    if packet.answer_key.answer_key_digest != expected_key_digest {
        reasons.push(
            "answer_key_digest_mismatch:recorded digest does not match the answer key fields"
                .to_string(),
        );
    }
    if packet.receipt.prompt_digest.as_deref() != Some(expected_prompt_digest.as_str()) {
        reasons.push(
            "prompt_digest_mismatch:receipt does not bind the exact reviewed prompt".to_string(),
        );
    }
    if packet.receipt.answer_key_digest.as_deref() != Some(expected_key_digest.as_str()) {
        reasons.push(
            "answer_key_digest_mismatch:receipt does not bind the exact answer key".to_string(),
        );
    }

    // 3. Retained review: bind the exact prompt and answer key, keep the two
    //    contamination judgments mechanically distinct, and never let a
    //    reviewer label override a mechanical finding.
    let findings = mechanical_contamination_findings(&packet.prompt);
    let review = &packet.prompt.review;
    if review.reviewer.trim().is_empty() {
        reasons.push("review_missing:the prompt carries no named reviewer".to_string());
    }
    if review.prompt_digest != expected_prompt_digest {
        reasons.push("review_stale:the review binds different prompt bytes".to_string());
    }
    if review.answer_key_digest != expected_key_digest {
        reasons.push("review_stale:the review binds a different answer key".to_string());
    }
    match packet.prompt.contamination_result {
        BlindJourneyContaminationResultV1::MechanicallyClean if !findings.is_empty() => {
            reasons.push(
                "contamination_misrecorded:mechanical findings exist but the prompt records a clean scan"
                    .to_string(),
            );
        }
        BlindJourneyContaminationResultV1::MechanicallyContaminated if findings.is_empty() => {
            reasons.push(
                "contamination_misrecorded:the prompt records contamination but the scan is clean"
                    .to_string(),
            );
        }
        BlindJourneyContaminationResultV1::MechanicallyClean
        | BlindJourneyContaminationResultV1::MechanicallyContaminated => {}
    }
    match review.verdict {
        BlindJourneyPromptReviewVerdictV1::Accepted => {
            if !findings.is_empty() {
                reasons.push(
                    "human_machine_disagreement:a reviewer label cannot override a mechanical contamination finding"
                        .to_string(),
                );
            }
        }
        // A rejected verdict may record semantic leakage the keyword scan
        // cannot see; it forbids a positive result in step 10.
        BlindJourneyPromptReviewVerdictV1::Rejected => {}
        BlindJourneyPromptReviewVerdictV1::NotReviewed => {
            if !findings.is_empty() {
                reasons.push(
                    "human_machine_disagreement:mechanical findings require a rejected verdict, not an absent review"
                        .to_string(),
                );
            }
        }
        BlindJourneyPromptReviewVerdictV1::Stale => {
            reasons.push("review_stale:the retained verdict is marked stale".to_string());
        }
    }
    // Mechanical findings reject the packet on their own, independent of the
    // reviewer verdict or the terminal result: a contaminated prompt never
    // becomes a valid transcript, and an honest limitation cannot rest on a
    // hinted prompt.
    if !findings.is_empty() {
        reasons.push(
            "mechanically_contaminated:the operator-visible prompt material fails the mechanical scan"
                .to_string(),
        );
    }
    // The receipt retains a copy of the review; it must be the same review,
    // not a second opinion recorded after the fact.
    if packet.receipt.review != packet.prompt.review {
        reasons.push(
            "review_binding_mismatch:the receipt review differs from the retained prompt review"
                .to_string(),
        );
    }

    // 4. Candidate identity must be portable-equal between prompt and receipt.
    if !packet
        .prompt
        .candidate
        .portable_eq(&packet.receipt.candidate)
    {
        reasons.push(
            "candidate_identity_mismatch:receipt candidate identity differs from the prompt"
                .to_string(),
        );
    }

    // 5. Transcript integrity: contiguous 1-based chain, exact subjects,
    //    intervention completeness and instrument-only visibility.
    let events = &packet.receipt.events;
    let mut previous_sequence: Option<u64> = None;
    for (index, event) in events.iter().enumerate() {
        if event.sequence != index as u64 + 1 {
            reasons.push(format!(
                "event_chain_broken:sequence {} is not contiguous at transcript position {}",
                event.sequence,
                index + 1
            ));
        }
        if event.predecessor_sequence != previous_sequence {
            reasons.push(format!(
                "event_chain_broken:event {} binds predecessor {:?} after {:?}",
                event.sequence, event.predecessor_sequence, previous_sequence
            ));
        }
        previous_sequence = Some(event.sequence);
        if event.subject.trim().is_empty() {
            reasons.push(format!(
                "event_subject_missing:event {} has no exact subject",
                event.sequence
            ));
        }
        for (field, digest) in [
            ("input", &event.input_digest),
            ("output", &event.output_digest),
        ] {
            if let Some(digest) = digest {
                let wellformed = digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
                if !wellformed {
                    reasons.push(format!(
                        "event_digest_malformed:event {} records a malformed {field} digest",
                        event.sequence
                    ));
                }
            }
        }
        if (event.kind == BlindJourneyEventKindV1::HarnessIntervention
            || event.kind == BlindJourneyEventKindV1::ProcessCancellationRestartCleanup)
            && event.intervention.is_none()
        {
            reasons.push(format!(
                "intervention_unclassified:non-product event {} has no closed classification",
                event.sequence
            ));
        }
        if let Some(intervention) = event.intervention {
            if !intervention.positive_allowed()
                && intervention != BlindJourneyInterventionV1::InstrumentOnlyNotOperatorVisible
                && event.reason.as_deref().is_none_or(str::is_empty)
            {
                reasons.push(format!(
                    "intervention_reason_missing:non-product event {} records no exact reason",
                    event.sequence
                ));
            }
            if event.actor.as_deref().is_none_or(str::is_empty) {
                reasons.push(format!(
                    "intervention_actor_missing:event {} records no actor",
                    event.sequence
                ));
            }
            if intervention == BlindJourneyInterventionV1::InstrumentOnlyNotOperatorVisible
                && event.operator_visible
            {
                reasons.push(format!(
                    "instrument_visibility_violation:instrument-only event {} was operator visible",
                    event.sequence
                ));
            }
        }
    }

    // 6. Assistance disqualifiers from the closed intervention taxonomy.
    //    Comparison disqualifiers (wrong subject, cage violation) come from
    //    the answer-key comparison and are not operator assistance, so the
    //    assistance axis binds only to the intervention-based set.
    let mut assistance_disqualifiers: std::collections::BTreeSet<BlindJourneyResultV1> =
        std::collections::BTreeSet::new();
    for event in events {
        if let Some(intervention) = event.intervention
            && let Some(result) = intervention.disqualifying_result()
        {
            assistance_disqualifiers.insert(result);
        }
    }

    // 7. Answer-key comparison: selection and edit cage checked without
    //    exposing the key; several eligible items may be represented. The
    //    comparison binds to what the transcript records, not to fields the
    //    producer could restate: the chosen item is the last recorded
    //    operator selection event, and every recorded file edit must stay
    //    inside the cage and out of the forbidden list.
    let key = &packet.answer_key;
    let mut comparison_disqualifiers: std::collections::BTreeSet<BlindJourneyResultV1> =
        std::collections::BTreeSet::new();
    let mut disqualifiers = assistance_disqualifiers.clone();
    let selection_event_subjects: Vec<&str> = events
        .iter()
        .filter(|event| event.kind == BlindJourneyEventKindV1::OperatorProductOptionSelection)
        .map(|event| event.subject.as_str())
        .collect();
    let edit_event_subjects: Vec<&str> = events
        .iter()
        .filter(|event| event.kind == BlindJourneyEventKindV1::FileEdit)
        .map(|event| event.subject.as_str())
        .collect();
    let mut selection_matches_key = false;
    let chosen_item = selection_event_subjects
        .last()
        .copied()
        .or(packet.receipt.selected_item.as_deref());
    if let Some(chosen) = chosen_item {
        selection_matches_key = true;
        if key.quiet_neighbors.iter().any(|item| item == chosen)
            || !key.eligible_items.iter().any(|item| item == chosen)
        {
            comparison_disqualifiers.insert(BlindJourneyResultV1::WrongOrStaleSubject);
        }
    }
    if let Some(selected) = packet.receipt.selected_item.as_deref() {
        match selection_event_subjects.last() {
            Some(last) if *last == selected => {}
            Some(_) => {
                reasons.push(
                    "selection_event_mismatch:the recorded selection disagrees with the operator selection event"
                        .to_string(),
                );
            }
            None => {
                reasons.push(
                    "selection_event_missing:the recorded selection has no operator selection event"
                        .to_string(),
                );
            }
        }
    }
    for subject in &edit_event_subjects {
        if key.forbidden_edits.iter().any(|path| path == subject)
            || !key.expected_edit_cage.iter().any(|path| path == subject)
        {
            comparison_disqualifiers.insert(BlindJourneyResultV1::UnsafeOrWrongEdit);
        }
    }
    if let Some(edit) = &packet.receipt.selected_edit {
        if key.forbidden_edits.iter().any(|path| path == edit)
            || !key.expected_edit_cage.iter().any(|path| path == edit)
        {
            comparison_disqualifiers.insert(BlindJourneyResultV1::UnsafeOrWrongEdit);
        }
        if !edit_event_subjects.contains(&edit.as_str()) {
            reasons.push(
                "selected_edit_unbound:the recorded selected edit names no recorded file edit"
                    .to_string(),
            );
        }
    }
    disqualifiers.extend(comparison_disqualifiers.iter().copied());

    // 8. Axis/evidence binding: a field must not claim more than the
    //    transcript enforces.
    let axes = &packet.receipt.axes;
    if axes.operator_assistance_state == BlindJourneyAssistanceStateV1::AssistanceObserved
        && assistance_disqualifiers.is_empty()
    {
        reasons.push(
            "assistance_axis_without_disqualifier:assistance_observed is recorded without any classified intervention"
                .to_string(),
        );
    }
    if axes.operator_assistance_state == BlindJourneyAssistanceStateV1::NoneObserved
        && !assistance_disqualifiers.is_empty()
    {
        reasons.push(
            "disqualifier_hidden_by_assistance_axis:a disqualifying intervention is not reflected in the assistance axis"
                .to_string(),
        );
    }
    if axes.selection_correctness == BlindJourneySelectionCorrectnessV1::QuietNeighborSelected
        && !chosen_item.is_some_and(|chosen| key.quiet_neighbors.iter().any(|item| item == chosen))
    {
        reasons.push(
            "selection_axis_misrecorded:quiet_neighbor_selected without a quiet-neighbor selection"
                .to_string(),
        );
    }
    if axes.edit_cage_verdict == BlindJourneyEditCageVerdictV1::Violation
        && !comparison_disqualifiers.contains(&BlindJourneyResultV1::UnsafeOrWrongEdit)
    {
        reasons.push(
            "edit_cage_axis_misrecorded:violation without a forbidden or out-of-cage edit"
                .to_string(),
        );
    }
    if axes.project_verification_status == BlindJourneyVerificationStatusV1::Failed
        && !event_kind_present(
            packet,
            BlindJourneyEventKindV1::ProjectVerificationExecution,
        )
    {
        reasons.push(
            "verification_axis_misrecorded:failed without a project verification execution event"
                .to_string(),
        );
    }
    if axes.project_verification_status == BlindJourneyVerificationStatusV1::Passed
        && !event_kind_present(
            packet,
            BlindJourneyEventKindV1::ProjectVerificationExecution,
        )
    {
        reasons.push(
            "verification_axis_misrecorded:passed without a project verification execution event"
                .to_string(),
        );
    }
    if axes.project_verification_status == BlindJourneyVerificationStatusV1::NotRun
        && event_kind_present(
            packet,
            BlindJourneyEventKindV1::ProjectVerificationExecution,
        )
    {
        reasons.push(
            "verification_axis_misrecorded:not_run although project verification executed"
                .to_string(),
        );
    }
    if axes.static_movement == BlindJourneyStaticMovementV1::Improved
        && !event_kind_present(packet, BlindJourneyEventKindV1::StaticAnalysisExecution)
    {
        reasons.push(
            "static_axis_misrecorded:improved without a static analysis execution event"
                .to_string(),
        );
    }
    if axes.receipt_status == BlindJourneyReceiptStatusV1::ReceiptMovementImproved
        && !event_kind_present(packet, BlindJourneyEventKindV1::ReceiptExecution)
    {
        reasons.push(
            "receipt_axis_misrecorded:movement_improved without a receipt execution event"
                .to_string(),
        );
    }

    // 9. Answer-key secrecy: evaluator-only material must be absent from the
    //    operator-visible projection. Eligible items may appear because the
    //    product presents them; quiet neighbors, forbidden edits, limitations,
    //    the discriminator family and every digest may not.
    let operator_text = blind_journey_operator_visible_text(packet).to_ascii_lowercase();
    let secrecy_violations: Vec<String> = {
        let mut violations = Vec::new();
        let key_only_strings: Vec<&str> = std::iter::once(key.discriminator_family.as_str())
            .chain(key.known_product_limitations.iter().map(String::as_str))
            .chain(key.forbidden_edits.iter().map(String::as_str))
            .chain(key.quiet_neighbors.iter().map(String::as_str))
            .collect();
        for value in key_only_strings {
            let lowered = value.to_ascii_lowercase();
            if !lowered.is_empty() && operator_text.contains(&lowered) {
                violations.push(format!(
                    "answer_key_leak:operator-visible material contains `{value}`"
                ));
            }
        }
        if operator_text.contains(&key.answer_key_digest.to_ascii_lowercase()) {
            violations.push(
                "answer_key_leak:operator-visible material contains the answer-key digest"
                    .to_string(),
            );
        }
        violations
    };
    reasons.extend(secrecy_violations);

    // 10. Terminal result consistency (fail closed on strengthening).
    let terminal = packet.receipt.terminal_result;
    let review_supports_positive =
        review.verdict == BlindJourneyPromptReviewVerdictV1::Accepted && findings.is_empty();
    let disqualifiers: Vec<BlindJourneyResultV1> = disqualifiers.into_iter().collect();
    if !terminal.producer_claimable() {
        reasons.push(format!(
            "invalid_terminal_claim:{terminal:?} is an evaluator judgment, not a producer claim"
        ));
    }
    if terminal == BlindJourneyResultV1::PassedBlindJourney {
        if !disqualifiers.is_empty() {
            reasons.push(format!(
                "positive_despite_disqualifier:{terminal:?} cannot coexist with disqualifiers {disqualifiers:?}"
            ));
        }
        if !review_supports_positive {
            reasons.push(
                "positive_without_accepted_review:a positive blind result requires an accepted retained review over the exact prompt and answer key"
                    .to_string(),
            );
        }
        if packet.receipt.selected_item.is_none() || packet.receipt.selected_edit.is_none() {
            reasons.push(
                "selection_unrecorded:a positive blind result requires the selected item and edit"
                    .to_string(),
            );
        }
        if !selection_matches_key {
            reasons.push(
                "selection_unrecorded:a positive blind result requires an answer-key comparison"
                    .to_string(),
            );
        }
        if axes.candidate_currentness != BlindJourneyCurrentnessV1::Current {
            reasons.push("axis_not_positive:candidate_currentness".to_string());
        }
        if axes.selection_correctness != BlindJourneySelectionCorrectnessV1::CorrectActionable {
            reasons.push("axis_not_positive:selection_correctness".to_string());
        }
        if axes.edit_cage_verdict != BlindJourneyEditCageVerdictV1::WithinCage {
            reasons.push("axis_not_positive:edit_cage_verdict".to_string());
        }
        if axes.project_verification_status != BlindJourneyVerificationStatusV1::Passed {
            reasons.push("axis_not_positive:project_verification_status".to_string());
        }
        if axes.static_movement != BlindJourneyStaticMovementV1::Improved {
            reasons.push("axis_not_positive:static_movement".to_string());
        }
        if axes.receipt_status != BlindJourneyReceiptStatusV1::ReceiptMovementImproved {
            reasons.push("axis_not_positive:receipt_status".to_string());
        }
        if axes.operator_assistance_state != BlindJourneyAssistanceStateV1::NoneObserved {
            reasons.push("axis_not_positive:operator_assistance_state".to_string());
        }
    } else if !disqualifiers.is_empty() {
        if !disqualifiers.contains(&terminal) {
            reasons.push(format!(
                "false_or_strengthened_receipt:terminal {terminal:?} hides disqualifiers {disqualifiers:?}"
            ));
        }
    } else {
        match terminal {
            BlindJourneyResultV1::HonestLimitation => {
                if packet.receipt.limitations.is_empty() {
                    reasons.push(
                        "honest_limitation_without_limitations:the exact limitation is required"
                            .to_string(),
                    );
                }
                if packet
                    .receipt
                    .limitations
                    .iter()
                    .any(|limitation| limitation.trim().is_empty())
                {
                    reasons.push(
                        "honest_limitation_with_blank_limitation:every recorded limitation must be exact"
                            .to_string(),
                    );
                }
                if packet.receipt.non_claims.is_empty() {
                    reasons.push(
                        "honest_limitation_without_non_claim:the exact recovery non-claim is required"
                            .to_string(),
                    );
                }
                if packet
                    .receipt
                    .non_claims
                    .iter()
                    .any(|non_claim| non_claim.trim().is_empty())
                {
                    reasons.push(
                        "honest_limitation_with_blank_non_claim:every recorded non-claim must be exact"
                            .to_string(),
                    );
                }
                if events.is_empty() {
                    reasons.push(
                        "incomplete_transcript:an honest limitation requires the complete transcript"
                            .to_string(),
                    );
                }
            }
            BlindJourneyResultV1::ProductDiscoverabilityFailure => {
                if !event_kind_present(packet, BlindJourneyEventKindV1::OperatorQuestion) {
                    reasons.push(
                        "discoverability_failure_without_question:the operator question is required"
                            .to_string(),
                    );
                }
                if packet.receipt.limitations.is_empty() {
                    reasons.push(
                        "discoverability_failure_without_limitations:the blocked public input is required"
                            .to_string(),
                    );
                }
                if packet
                    .receipt
                    .limitations
                    .iter()
                    .any(|limitation| limitation.trim().is_empty())
                {
                    reasons.push(
                        "discoverability_failure_with_blank_limitation:every recorded limitation must be exact"
                            .to_string(),
                    );
                }
            }
            BlindJourneyResultV1::VerificationFailureVisible => {
                if axes.project_verification_status != BlindJourneyVerificationStatusV1::Failed {
                    reasons.push(
                        "verification_failure_misrecorded:the verification axis must record the visible failure"
                            .to_string(),
                    );
                }
            }
            BlindJourneyResultV1::VerificationNotRunVisible => {
                if axes.project_verification_status != BlindJourneyVerificationStatusV1::NotRun {
                    reasons.push(
                        "verification_not_run_misrecorded:the verification axis must record not_run"
                            .to_string(),
                    );
                }
            }
            BlindJourneyResultV1::NotRun => {
                if !events.is_empty() {
                    reasons.push(
                        "not_run_with_transcript:a not-run journey cannot carry a transcript"
                            .to_string(),
                    );
                }
            }
            _ => {
                // Disqualifier-backed terminals were handled above; any other
                // terminal here has no cause in the transcript.
                reasons.push(format!(
                    "terminal_without_cause:{terminal:?} has no matching evidence in the transcript"
                ));
            }
        }
    }

    let portable_identity = match blind_journey_portable_identity(packet) {
        Ok(identity) => identity,
        Err(error) => format!("portable_identity_error:{error}"),
    };
    BlindJourneyAssessmentV1 {
        accepted: reasons.is_empty(),
        terminal_result: terminal,
        rejection_reasons: reasons,
        contamination_findings: findings,
        disqualifiers,
        event_count: events.len(),
        portable_identity,
    }
}

/// Scenario expectation recorded in the committed fixture corpus.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyFixtureExpectationV1 {
    pub accepted: bool,
    pub terminal: Option<BlindJourneyResultV1>,
    pub same_portable_identity_as: Option<String>,
}

/// One committed fixture scenario: an expected outcome plus one full packet.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyFixtureScenarioV1 {
    pub id: String,
    pub expected: BlindJourneyFixtureExpectationV1,
    pub packet: BlindJourneyPacketWireV1,
}

/// Wire shape of one committed packet in the fixture corpus.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyPacketWireV1 {
    pub prompt: BlindJourneyPromptV1,
    pub answer_key: BlindJourneyAnswerKeyV1,
    pub receipt: BlindJourneyReceiptV1,
}

/// The committed blind-journey fixture corpus (RIPR-SPEC-0200).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyFixtureCorpusV1 {
    pub schema_version: String,
    pub scenarios: Vec<BlindJourneyFixtureScenarioV1>,
}

pub(crate) const BLIND_JOURNEY_FIXTURE_CORPUS_SCHEMA_VERSION: &str =
    "blind_journey_fixture_corpus.v1";

/// The fixture scenarios #4603 requires; the committed corpus must cover all
/// of them and the live validator decides each outcome independently.
pub(crate) const REQUIRED_BLIND_JOURNEY_SCENARIO_IDS: [&str; 24] = [
    "clean_generic_prompt_accepted",
    "prompt_contamination_issue_reference",
    "prompt_contamination_target_test",
    "prompt_contamination_gap_id",
    "prompt_contamination_artifact_path",
    "prompt_contamination_expected_command",
    "public_docs_and_source_reads_allowed",
    "private_hint_after_failed_command",
    "manual_artifact_copying",
    "workspace_binary_substitution",
    "instrument_watchdog_observes_only",
    "operator_selects_first_eligible_item",
    "operator_selects_second_eligible_item",
    "quiet_neighbor_selected_as_actionable",
    "passing_axes_with_hidden_hint",
    "honest_limitation_keeps_transcript",
    "transcript_missing_predecessor",
    "transcript_reordered_trace",
    "transcript_altered_prompt_binding",
    "transcript_changed_answer_key",
    "reviewer_overrides_mechanical_finding",
    "operator_projection_hides_answer_key",
    "equivalent_root_spelling_a_packet",
    "equivalent_root_spelling_b_packet",
];

/// Required scenario ids absent from one committed corpus id set.
pub(crate) fn missing_blind_journey_required_scenarios<'a>(
    present: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let present: std::collections::BTreeSet<&str> = present.into_iter().collect();
    REQUIRED_BLIND_JOURNEY_SCENARIO_IDS
        .iter()
        .filter(|required| !present.contains(**required))
        .copied()
        .collect()
}

/// Load and parse one committed fixture corpus.
pub(crate) fn load_blind_journey_fixture_corpus(
    body: &str,
) -> Result<BlindJourneyFixtureCorpusV1, String> {
    let corpus: BlindJourneyFixtureCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse blind journey fixture corpus: {error}"))?;
    if corpus.schema_version != BLIND_JOURNEY_FIXTURE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported blind journey fixture corpus schema `{}`",
            corpus.schema_version
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if !ids.insert(scenario.id.clone()) {
            return Err(format!(
                "duplicate blind journey fixture scenario id `{}`",
                scenario.id
            ));
        }
    }
    Ok(corpus)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate_ref(root: &str) -> BlindJourneyCandidateRefV1 {
        BlindJourneyCandidateRefV1 {
            repository: "example.com/operator/target".to_string(),
            base: "base-sha".to_string(),
            head: "head-sha".to_string(),
            tree: "tree-sha".to_string(),
            root: root.to_string(),
            package: "ripr".to_string(),
            binary: "ripr".to_string(),
            binary_digest: "binary-sha256".to_string(),
        }
    }

    fn clean_prompt_bytes() -> String {
        "You are given an installed analysis tool and a target repository. \
         Improve the tests for one behavior you can reach through the product. \
         You may read the public docs and the ordinary target source. Do not \
         ask anyone for the intended change."
            .to_string()
    }

    fn answer_key() -> BlindJourneyAnswerKeyV1 {
        BlindJourneyAnswerKeyV1 {
            schema_version: BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION.to_string(),
            eligible_items: vec!["item-a".to_string(), "item-b".to_string()],
            quiet_neighbors: vec!["quiet-neighbor".to_string()],
            expected_edit_cage: vec!["src/lib.rs".to_string()],
            discriminator_family: "boundary-constant".to_string(),
            known_product_limitations: vec!["no-network".to_string()],
            forbidden_edits: vec!["src/main.rs".to_string()],
            answer_key_digest: String::new(),
        }
    }

    fn event(
        sequence: u64,
        predecessor: Option<u64>,
        kind: BlindJourneyEventKindV1,
        subject: &str,
    ) -> BlindJourneyEventV1 {
        BlindJourneyEventV1 {
            sequence,
            predecessor_sequence: predecessor,
            kind,
            subject: subject.to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: true,
            intervention: None,
            actor: None,
            reason: None,
        }
    }

    fn positive_events() -> Vec<BlindJourneyEventV1> {
        vec![
            event(
                1,
                None,
                BlindJourneyEventKindV1::PublicDocumentationLookup,
                "docs:cli-check",
            ),
            event(
                2,
                Some(1),
                BlindJourneyEventKindV1::OrdinaryTargetSourceRead,
                "src/lib.rs",
            ),
            event(
                3,
                Some(2),
                BlindJourneyEventKindV1::ProductCommandInvocation,
                "ripr check",
            ),
            event(
                4,
                Some(3),
                BlindJourneyEventKindV1::OperatorProductOptionSelection,
                "item-a",
            ),
            event(5, Some(4), BlindJourneyEventKindV1::FileEdit, "src/lib.rs"),
            event(
                6,
                Some(5),
                BlindJourneyEventKindV1::ProjectVerificationExecution,
                "cargo test",
            ),
            event(
                7,
                Some(6),
                BlindJourneyEventKindV1::StaticAnalysisExecution,
                "ripr check",
            ),
            event(
                8,
                Some(7),
                BlindJourneyEventKindV1::ReceiptExecution,
                "ripr receipts",
            ),
        ]
    }

    fn positive_axes() -> BlindJourneyEvidenceAxesV1 {
        BlindJourneyEvidenceAxesV1 {
            candidate_currentness: BlindJourneyCurrentnessV1::Current,
            selection_correctness: BlindJourneySelectionCorrectnessV1::CorrectActionable,
            edit_cage_verdict: BlindJourneyEditCageVerdictV1::WithinCage,
            project_verification_status: BlindJourneyVerificationStatusV1::Passed,
            static_movement: BlindJourneyStaticMovementV1::Improved,
            receipt_status: BlindJourneyReceiptStatusV1::ReceiptMovementImproved,
            operator_assistance_state: BlindJourneyAssistanceStateV1::NoneObserved,
            external_runtime_mutation_evidence: None,
        }
    }

    fn stamped_positive_packet(root: &str) -> Result<BlindJourneyPacketV1, String> {
        let candidate = candidate_ref(root);
        let key = answer_key();
        let bound_key_digest = blind_journey_answer_key_digest(&key)?;
        let prompt = BlindJourneyPromptV1 {
            schema_version: BLIND_JOURNEY_PROMPT_SCHEMA_VERSION.to_string(),
            candidate: candidate.clone(),
            operator_goal: "improve one test through the installed product".to_string(),
            public_inputs: vec!["docs/cli.md".to_string()],
            ordinary_permissions: vec!["run public product entrypoints".to_string()],
            prohibited_hints: vec!["no intended change hints".to_string()],
            prompt_bytes: clean_prompt_bytes(),
            prompt_digest: String::new(),
            review: BlindJourneyPromptReviewV1 {
                reviewer: "reviewer-1".to_string(),
                verdict: BlindJourneyPromptReviewVerdictV1::Accepted,
                prompt_digest: String::new(),
                answer_key_digest: bound_key_digest,
            },
            contamination_result: BlindJourneyContaminationResultV1::MechanicallyClean,
        };
        let receipt = BlindJourneyReceiptV1 {
            schema_version: BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION.to_string(),
            candidate,
            prompt_digest: None,
            answer_key_digest: None,
            review: prompt.review.clone(),
            selected_item: Some("item-a".to_string()),
            selected_edit: Some("src/lib.rs".to_string()),
            events: positive_events(),
            axes: positive_axes(),
            terminal_result: BlindJourneyResultV1::PassedBlindJourney,
            limitations: Vec::new(),
            non_claims: Vec::new(),
        };
        stamp_blind_journey_packet(prompt, key, receipt)
    }

    fn reason_contains(assessment: &BlindJourneyAssessmentV1, needle: &str) -> bool {
        assessment
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains(needle))
    }

    #[test]
    fn clean_generic_prompt_validates_positive() -> Result<(), String> {
        let packet = stamped_positive_packet("/srv/journey")?;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "a clean generic prompt must validate positive, got {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn each_contamination_category_rejects_independently() -> Result<(), String> {
        let contaminated_prompts = [
            (
                "internal_reference",
                "Fix the bug reported in issues/1234 today.",
            ),
            (
                "preselected_item_reference",
                "Improve the tests for gap-042.",
            ),
            (
                "target_file_or_edit",
                "Edit src/lib.rs:12 and tests/add.rs for the change.",
            ),
            (
                "private_artifact_path",
                "Use the target/ripr/reports/check.json artifact.",
            ),
            (
                "expected_command_sequence",
                "Run cargo test --package target to verify.",
            ),
        ];
        for (category, text) in contaminated_prompts {
            let mut packet = stamped_positive_packet("/srv/journey")?;
            packet.prompt.prompt_bytes = text.to_string();
            packet.prompt.contamination_result =
                BlindJourneyContaminationResultV1::MechanicallyContaminated;
            let assessment = assess_blind_journey_packet(&packet);
            if assessment.accepted {
                return Err(format!(
                    "prompt contaminated via {category} must be rejected"
                ));
            }
            if !assessment
                .contamination_findings
                .iter()
                .any(|finding| finding.category == category)
            {
                return Err(format!(
                    "contamination via {category} must surface its own category finding"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn public_docs_and_ordinary_source_reads_stay_allowed() -> Result<(), String> {
        let packet = stamped_positive_packet("/tmp/journey")?;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "public docs lookup and ordinary source inspection must stay allowed: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn private_hint_after_passing_verification_stays_non_positive() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "harness hint after failed first attempt".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::PrivateHarnessHint),
            actor: Some("evaluator".to_string()),
            reason: Some("named the intended edit after the command failed".to_string()),
        });
        packet.receipt.axes.operator_assistance_state =
            BlindJourneyAssistanceStateV1::AssistanceObserved;
        packet.receipt.terminal_result = BlindJourneyResultV1::HiddenOperatorAssistance;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted {
            return Err(format!(
                "a private hint must record a consistent non-positive receipt: {:?}",
                assessment.rejection_reasons
            ));
        }
        if assessment.positive()
            || assessment.terminal_result != BlindJourneyResultV1::HiddenOperatorAssistance
        {
            return Err("a private hint must record hidden_operator_assistance".to_string());
        }
        Ok(())
    }

    #[test]
    fn hidden_hint_cannot_hide_behind_passing_axes() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "operator given the answer".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::HiddenOperatorKnowledge),
            actor: Some("evaluator".to_string()),
            reason: Some("operator already knew the intended change".to_string()),
        });
        packet.receipt.axes.operator_assistance_state =
            BlindJourneyAssistanceStateV1::AssistanceObserved;
        packet.receipt.terminal_result = BlindJourneyResultV1::HiddenOperatorAssistance;
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.positive() {
            return Err(
                "passing verification and static axes must not hide hidden operator knowledge"
                    .to_string(),
            );
        }
        if !assessment.accepted {
            return Err(format!(
                "an honestly recorded hidden-hint receipt must still be audit-complete: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn manual_artifact_plumbing_is_classified_and_non_positive() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "copied before/verify artifact".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::ManualArtifactPlumbing),
            actor: Some("evaluator".to_string()),
            reason: Some("placed a before/verify artifact the product did not emit".to_string()),
        });
        packet.receipt.axes.operator_assistance_state =
            BlindJourneyAssistanceStateV1::AssistanceObserved;
        packet.receipt.terminal_result = BlindJourneyResultV1::ManualArtifactPlumbing;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted
            || assessment.terminal_result != BlindJourneyResultV1::ManualArtifactPlumbing
        {
            return Err(format!(
                "manual artifact plumbing must record its own terminal result: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn workspace_binary_substitution_records_candidate_identity_failure() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "replaced the installed binary".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::WorkspaceBinarySubstitution),
            actor: Some("evaluator".to_string()),
            reason: Some("planted another binary in the workspace".to_string()),
        });
        packet.receipt.axes.operator_assistance_state =
            BlindJourneyAssistanceStateV1::AssistanceObserved;
        packet.receipt.terminal_result = BlindJourneyResultV1::CandidateIdentityFailure;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted
            || assessment.terminal_result != BlindJourneyResultV1::CandidateIdentityFailure
        {
            return Err(format!(
                "workspace binary substitution must record candidate_identity_failure: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn instrument_watchdog_observes_without_helping() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::ProcessCancellationRestartCleanup,
            subject: "watchdog observed process bounds".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::InstrumentOnlyNotOperatorVisible),
            actor: Some("harness".to_string()),
            reason: Some("watchdog inventory only; no operator path change".to_string()),
        });
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "an instrument-only watchdog must not change a positive run: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn either_eligible_product_presented_choice_validates() -> Result<(), String> {
        for selected in ["item-a", "item-b"] {
            let mut packet = stamped_positive_packet("/srv/journey")?;
            packet.receipt.selected_item = Some(selected.to_string());
            for event in &mut packet.receipt.events {
                if event.kind == BlindJourneyEventKindV1::OperatorProductOptionSelection {
                    event.subject = selected.to_string();
                }
            }
            let assessment = assess_blind_journey_packet(&packet);
            if !assessment.positive() {
                return Err(format!(
                    "selecting either eligible item must validate, `{selected}` did not: {:?}",
                    assessment.rejection_reasons
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn quiet_neighbor_selection_fails_the_answer_key_comparison() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.selected_item = Some("quiet-neighbor".to_string());
        for event in &mut packet.receipt.events {
            if event.kind == BlindJourneyEventKindV1::OperatorProductOptionSelection {
                event.subject = "quiet-neighbor".to_string();
            }
        }
        packet.receipt.axes.selection_correctness =
            BlindJourneySelectionCorrectnessV1::QuietNeighborSelected;
        packet.receipt.terminal_result = BlindJourneyResultV1::WrongOrStaleSubject;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted
            || assessment.terminal_result != BlindJourneyResultV1::WrongOrStaleSubject
        {
            return Err(format!(
                "a quiet neighbor must fail the answer-key comparison: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn forbidden_edit_fails_the_edit_cage_comparison() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.selected_edit = Some("src/main.rs".to_string());
        for event in &mut packet.receipt.events {
            if event.kind == BlindJourneyEventKindV1::FileEdit {
                event.subject = "src/main.rs".to_string();
            }
        }
        packet.receipt.axes.edit_cage_verdict = BlindJourneyEditCageVerdictV1::Violation;
        packet.receipt.terminal_result = BlindJourneyResultV1::UnsafeOrWrongEdit;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted
            || assessment.terminal_result != BlindJourneyResultV1::UnsafeOrWrongEdit
        {
            return Err(format!(
                "a forbidden edit must fail the edit-cage comparison: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn honest_limitation_keeps_the_complete_transcript() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events = packet.receipt.events[..3].to_vec();
        for (index, event) in packet.receipt.events.iter_mut().enumerate() {
            event.sequence = index as u64 + 1;
            event.predecessor_sequence = if index == 0 { None } else { Some(index as u64) };
        }
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 4,
            predecessor_sequence: Some(3),
            kind: BlindJourneyEventKindV1::OperatorQuestion,
            subject: "operator recorded explicit uncertainty".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: true,
            intervention: None,
            actor: None,
            reason: None,
        });
        packet.receipt.axes.project_verification_status = BlindJourneyVerificationStatusV1::NotRun;
        packet.receipt.axes.static_movement = BlindJourneyStaticMovementV1::Unknown;
        packet.receipt.axes.receipt_status = BlindJourneyReceiptStatusV1::ReceiptNotApplicable;
        packet.receipt.axes.edit_cage_verdict = BlindJourneyEditCageVerdictV1::Unknown;
        packet.receipt.axes.selection_correctness = BlindJourneySelectionCorrectnessV1::Unknown;
        packet.receipt.selected_item = None;
        packet.receipt.selected_edit = None;
        packet.receipt.limitations =
            vec!["the installed product exposed no actionable gap for this repository".to_string()];
        packet.receipt.non_claims = vec![
            "no completed repair is claimed; recovery requires a product discoverability fix"
                .to_string(),
        ];
        packet.receipt.terminal_result = BlindJourneyResultV1::HonestLimitation;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted
            || assessment.terminal_result != BlindJourneyResultV1::HonestLimitation
        {
            return Err(format!(
                "an honest limitation must remain an accepted non-positive receipt: {:?}",
                assessment.rejection_reasons
            ));
        }
        if assessment.positive() {
            return Err(
                "an honest limitation must never aggregate into a positive row".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn missing_predecessor_rejects_the_transcript() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events[4].predecessor_sequence = Some(99);
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "event_chain_broken") {
            return Err("a missing event predecessor must reject the transcript".to_string());
        }
        Ok(())
    }

    #[test]
    fn reordered_trace_rejects_the_transcript() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.swap(2, 5);
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "event_chain_broken") {
            return Err("a reordered trace must reject the transcript".to_string());
        }
        Ok(())
    }

    #[test]
    fn altered_prompt_bytes_reject_the_digest_binding() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.prompt.prompt_bytes.push_str(" #9999");
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "prompt_digest_mismatch") {
            return Err("altered prompt bytes must reject the digest binding".to_string());
        }
        Ok(())
    }

    #[test]
    fn changed_answer_key_rejects_the_digest_binding() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.answer_key.eligible_items.push("item-c".to_string());
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "answer_key_digest_mismatch") {
            return Err("a changed answer key must reject the digest binding".to_string());
        }
        Ok(())
    }

    #[test]
    fn reviewer_label_cannot_override_mechanical_rejection() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.prompt.prompt_bytes = "Fix gap-042 as intended.".to_string();
        // A local repair cannot re-stamp: the recorded review still binds the
        // old bytes, and the recorded scan result disagrees with the scan.
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "human_machine_disagreement") {
            return Err(
                "an accepted reviewer label must not override a mechanical finding".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn answer_key_never_reaches_the_operator_projection() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "the answer key names quiet-neighbor".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: true,
            intervention: Some(BlindJourneyInterventionV1::PrivateHarnessHint),
            actor: Some("evaluator".to_string()),
            reason: Some("leaked evaluator-only material".to_string()),
        });
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "answer_key_leak") {
            return Err("answer-key material in operator-visible text must reject".to_string());
        }
        Ok(())
    }

    #[test]
    fn equivalent_roots_share_portable_identity() -> Result<(), String> {
        let short_root_packet = stamped_positive_packet("/srv/journey")?;
        let deep_root_packet = stamped_positive_packet("/tmp/journey")?;
        let short_root = assess_blind_journey_packet(&short_root_packet);
        let deep_root = assess_blind_journey_packet(&deep_root_packet);
        if !short_root.positive() || !deep_root.positive() {
            return Err("equivalent-root packets must both validate positive".to_string());
        }
        if short_root.portable_identity != deep_root.portable_identity {
            return Err("equivalent roots must share one portable semantic identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn receipt_review_divergence_rejects() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.review.verdict = BlindJourneyPromptReviewVerdictV1::Rejected;
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "review_binding_mismatch") {
            return Err(
                "a receipt review that differs from the retained prompt review must reject"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn forbidden_edit_event_rejects_even_when_the_selected_edit_is_allowed() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        let last = packet.receipt.events.len() as u64;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: last + 1,
            predecessor_sequence: Some(last),
            kind: BlindJourneyEventKindV1::FileEdit,
            subject: "src/main.rs".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: true,
            intervention: None,
            actor: None,
            reason: None,
        });
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.positive() {
            return Err("a forbidden edit event must not validate positive".to_string());
        }
        if !assessment
            .disqualifiers
            .contains(&BlindJourneyResultV1::UnsafeOrWrongEdit)
        {
            return Err(
                "a forbidden edit event must disqualify as unsafe_or_wrong_edit".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn honest_forbidden_edit_run_is_accepted() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        let last = packet.receipt.events.len() as u64;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: last + 1,
            predecessor_sequence: Some(last),
            kind: BlindJourneyEventKindV1::FileEdit,
            subject: "src/main.rs".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: true,
            intervention: None,
            actor: None,
            reason: None,
        });
        packet.receipt.axes.edit_cage_verdict = BlindJourneyEditCageVerdictV1::Violation;
        packet.receipt.terminal_result = BlindJourneyResultV1::UnsafeOrWrongEdit;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted {
            return Err(format!(
                "an honest unsafe_or_wrong_edit receipt must stay accepted, got {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn selection_event_mismatch_rejects() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.selected_item = Some("item-b".to_string());
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "selection_event_mismatch") {
            return Err(
                "a recorded selection that disagrees with the operator selection event must reject"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn unsupported_future_schema_rejects_every_packet() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.prompt.schema_version = "blind_journey_prompt.v2".to_string();
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "unsupported_schema") {
            return Err("an unsupported future schema must reject the packet".to_string());
        }
        Ok(())
    }

    #[test]
    fn positive_receipt_requires_an_accepted_review() -> Result<(), String> {
        let candidate = candidate_ref("/srv/journey");
        let key = answer_key();
        let mut prompt = BlindJourneyPromptV1 {
            schema_version: BLIND_JOURNEY_PROMPT_SCHEMA_VERSION.to_string(),
            candidate: candidate.clone(),
            operator_goal: "improve one test through the installed product".to_string(),
            public_inputs: vec!["docs/cli.md".to_string()],
            ordinary_permissions: vec!["run public product entrypoints".to_string()],
            prohibited_hints: vec!["no intended change hints".to_string()],
            prompt_bytes: clean_prompt_bytes(),
            prompt_digest: String::new(),
            review: BlindJourneyPromptReviewV1 {
                reviewer: "reviewer-1".to_string(),
                verdict: BlindJourneyPromptReviewVerdictV1::NotReviewed,
                prompt_digest: String::new(),
                answer_key_digest: String::new(),
            },
            contamination_result: BlindJourneyContaminationResultV1::MechanicallyClean,
        };
        // Pre-bind the canonical operator-visible digests the way a completed
        // review would, so stamping keeps the reviewed binding instead of
        // stamping a fresh one.
        prompt.review.prompt_digest = prompt_digest(&prompt);
        prompt.review.answer_key_digest = blind_journey_answer_key_digest(&key)?;
        let receipt = BlindJourneyReceiptV1 {
            schema_version: BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION.to_string(),
            candidate,
            prompt_digest: None,
            answer_key_digest: None,
            review: prompt.review.clone(),
            selected_item: Some("item-a".to_string()),
            selected_edit: Some("src/lib.rs".to_string()),
            events: positive_events(),
            axes: positive_axes(),
            terminal_result: BlindJourneyResultV1::PassedBlindJourney,
            limitations: Vec::new(),
            non_claims: Vec::new(),
        };
        let packet = stamp_blind_journey_packet(prompt, key, receipt)?;
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.positive()
            || !reason_contains(&assessment, "positive_without_accepted_review")
        {
            return Err(
                "until a current accepted reviewer verdict exists, a positive blind result must be refused"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn non_product_action_requires_closed_classification() -> Result<(), String> {
        let mut packet = stamped_positive_packet("/srv/journey")?;
        packet.receipt.events.push(BlindJourneyEventV1 {
            sequence: 9,
            predecessor_sequence: Some(8),
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "evaluator did something".to_string(),
            input_digest: None,
            output_digest: None,
            operator_visible: false,
            intervention: None,
            actor: None,
            reason: None,
        });
        let assessment = assess_blind_journey_packet(&packet);
        if assessment.accepted || !reason_contains(&assessment, "intervention_unclassified") {
            return Err(
                "a non-product action without a closed classification must reject".to_string(),
            );
        }
        Ok(())
    }
}
