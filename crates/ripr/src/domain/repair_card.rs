//! Compact, versioned repair work object (`RepairCardV1`) projected from the
//! existing repair authorities (#3166, #4663).
//!
//! The card is one provider-neutral work order for one governed repair. It is
//! NOT a new analyzer, readiness validator, instruction state, attempt model,
//! or command authority: every field is a verbatim projection of the witness,
//! `FixInstructionSummary`, repair-route readiness, typed command references
//! and optional `RepairAttempt` state, built by the single app-layer builder
//! (`crate::app::repair_card`). Renderers and transports consume the card;
//! none reconstructs state from prose.
//!
//! The card describes the work order and never marks itself complete:
//! observed completion lives in the RepairAttempt/receipt authorities.

use super::{FixInstructionState, FixInstructionSummary};

/// Versioned card schema. Additive changes keep this version and add
/// `#[serde(default)]` fields; breaking shape changes mint a new version.
pub const REPAIR_CARD_SCHEMA_VERSION: &str = "repair_card.v1";

/// Load-bearing rejected alternatives are bounded so the card stays a compact
/// work object. Callers fail closed when the producer supplies more.
pub const MAX_REPAIR_CARD_REJECTED_ALTERNATIVES: usize = 3;

/// What may be claimed from a card. Completion, correctness and
/// mutation-confirmation claims stay with the attempt/receipt authorities.
pub const REPAIR_CARD_CLAIM_BOUNDARY: &str = "Static repair work order for one governed repair, projected from current product authorities; it does not establish completion, correctness, runtime behavior, or mutation confirmation.";

/// Versioned detail/budget contract. The budget numbers are
/// provisional-but-versioned: #4669 ratifies them against measured agent use.
/// Additive budget changes keep this version; semantic budget changes mint a
/// new one.
pub const REPAIR_CARD_BUDGET_VERSION: &str = "repair-card-budget-v1";

/// Default bound on detail-reference items carried on one wire card. The nine
/// load-bearing evidence families stay well inside this bound.
pub const DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS: usize = 16;

/// Default bound on the normalized serialized size of the wire card (UTF-8
/// bytes of the compact card JSON, detail references included).
pub const DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES: usize = 64 * 1024;

/// Default bound for any single compact prose field embedded in the card.
/// Oversized compact fields fail closed instead of being silently truncated;
/// the full content lives behind the family's detail reference.
pub const DEFAULT_REPAIR_CARD_MAX_INLINE_DETAIL_BYTES: usize = 4 * 1024;

/// Versioned item/byte budget for one wire card. Presentation/detail only:
/// budgeting can never change canonical identity, readiness, target selection
/// or actionability (#4666).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairCardBudget {
    pub max_detail_items: usize,
    pub max_serialized_bytes: usize,
    pub max_inline_detail_bytes: usize,
}

impl Default for RepairCardBudget {
    fn default() -> Self {
        Self {
            max_detail_items: DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS,
            max_serialized_bytes: DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES,
            max_inline_detail_bytes: DEFAULT_REPAIR_CARD_MAX_INLINE_DETAIL_BYTES,
        }
    }
}

impl RepairCardBudget {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_detail_items == 0 {
            return Err("max_detail_items must be greater than zero".to_string());
        }
        if self.max_serialized_bytes == 0 {
            return Err("max_serialized_bytes must be greater than zero".to_string());
        }
        if self.max_inline_detail_bytes == 0 {
            return Err("max_inline_detail_bytes must be greater than zero".to_string());
        }
        Ok(())
    }
}

/// The typed refusal kinds of the `ripr agent card` handoff failure envelope
/// (#5007, RIPR-SPEC-0202). Each kind names one deliberate machine state a
/// loop driver branches on — never a severity — and carries the same remedy
/// family on every surface. The wire spelling is pinned against
/// `policy/output_contracts.txt` and `docs/OUTPUT_SCHEMA.md` by
/// `cargo xtask check-output-contracts`.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentCardRefusalKind {
    /// The requested seam id names no seam in the current inventory.
    SeamNotFound,
    /// The seam's grip class is omitted from agent results by policy.
    PolicyOmitted,
    /// The witness analysis could not produce this seam's witness.
    WitnessUnavailable,
    /// Nothing on this seam names a portable workspace identity.
    IdentityUnnameable,
    /// The card builder, route gate, or budget refused to mint the card.
    BudgetOverflow,
}

impl AgentCardRefusalKind {
    /// The wire spelling of the refusal kind, exactly as it appears in the
    /// envelope's `error.kind` field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SeamNotFound => "seam_not_found",
            Self::PolicyOmitted => "policy_omitted",
            Self::WitnessUnavailable => "witness_unavailable",
            Self::IdentityUnnameable => "identity_unnameable",
            Self::BudgetOverflow => "budget_overflow",
        }
    }
}

/// One load-bearing evidence family the compact card routes to instead of
/// embedding. The families name the authorities #4666 keeps explicitly
/// reachable from a finite card.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardDetailFamily {
    /// The full fix-instruction detail behind the embedded summary.
    FixInstruction,
    /// Witness/stage evidence for the changed behavior.
    WitnessStageEvidence,
    /// Related-test candidates considered for the target.
    RelatedTestCandidates,
    /// Per-limitation detail behind the compact limitation lines.
    LimitationDetail,
    /// The complete canonical packet; never embedded, always routed.
    CanonicalPacket,
    /// The RepairAttempt authority's full status detail.
    RepairAttemptStatus,
    /// The focused-proof receipt for the selected route.
    FocusedProofReceipt,
    /// Static-movement detail behind the compact done-when axis.
    StaticMovement,
    /// Optional mutation-calibration detail; never required to close a card.
    MutationCalibration,
}

/// Producer-reported state of referenced evidence. The card projects the state
/// visibly and never upgrades it: stale evidence stays stale on the card.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardDetailState {
    Current,
    Stale,
    Malformed,
    WrongRoot,
    Missing,
    Unavailable,
}

/// Why a family rides outside the wire card.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardOmissionClass {
    /// The authority never embeds this family (the canonical packet).
    NotEmbeddable,
    /// The full content stays in its owning authority; the card routes to it.
    AuthorityOwnedDetail,
    /// The producer reported the evidence unavailable; the exact reason rides
    /// on the reference.
    Unavailable,
}

/// One stable, typed route to an omitted load-bearing evidence family.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardDetailRef {
    pub family: RepairCardDetailFamily,
    pub state: RepairCardDetailState,
    /// Stable portable retrieval route. `None` exactly when the family is
    /// unavailable; absolute checkout spellings are rejected at build time so
    /// equivalent roots keep one identity.
    #[serde(default)]
    pub route: Option<String>,
    /// Exact producer-owned reason; `Some` exactly when no route exists.
    #[serde(default)]
    pub unavailable_reason: Option<String>,
    /// sha256 hex over the family's normalized serialized content; the
    /// identity of the omitted evidence itself.
    pub detail_digest: String,
    /// Normalized serialized bytes of the omitted content, measured from the
    /// actual representation.
    pub omitted_bytes: usize,
    /// Deterministic position in family-sorted reference order.
    pub ordinal: usize,
    pub omission_class: RepairCardOmissionClass,
}

/// Budget accounting for the wire card. Counts and bytes are measured from
/// the actual normalized representations, never estimated.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardDetailSummary {
    pub budget_version: String,
    /// Families routed by reference.
    pub referenced_items: usize,
    /// Families reported unavailable (exact reason recorded per reference).
    pub unavailable_items: usize,
    /// Normalized serialized bytes of the final wire card.
    pub selected_bytes: usize,
    /// Sum of normalized serialized bytes routed behind references.
    pub omitted_bytes: usize,
    /// `selected_bytes + omitted_bytes`: the complete evidence size.
    pub complete_bytes: usize,
    /// Deterministic (sorted, deduplicated) omission classes present.
    #[serde(default)]
    pub omission_classes: Vec<RepairCardOmissionClass>,
}

impl Default for RepairCardDetailSummary {
    fn default() -> Self {
        Self {
            budget_version: REPAIR_CARD_BUDGET_VERSION.to_string(),
            referenced_items: 0,
            unavailable_items: 0,
            selected_bytes: 0,
            omitted_bytes: 0,
            complete_bytes: 0,
            omission_classes: Vec::new(),
        }
    }
}

/// Portable snapshot identity. `workspace_identity` is producer-supplied and
/// portable (never an absolute checkout spelling); absolute paths, timestamps
/// and presentation formatting never enter the card identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardSnapshot {
    /// Producer-owned portable workspace identity (see
    /// `TestTargetEvidence::workspace_identity` precedent).
    pub workspace_identity: String,
    /// Repository head the snapshot was taken at.
    pub repository_head: String,
    pub currentness: RepairCardSnapshotCurrentness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardSnapshotCurrentness {
    Current,
    AcceptedDirtyDraft,
}

/// The one repair this card orders.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardSubject {
    pub seam_id: String,
    pub canonical_gap_id: Option<String>,
    pub finding_id: Option<String>,
}

/// Shared route-readiness facts with the repair-attempt edit-cage ceiling.
/// A cage refusal is retained verbatim as missing evidence by the app builder;
/// renderers do not re-derive this decision.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardReadinessFacts {
    /// The route is ready and the packet's selected edit surface was admitted.
    pub repair_ready: bool,
    pub required_evidence: Vec<String>,
    pub present_evidence: Vec<String>,
    pub missing_evidence: Vec<String>,
}

/// Selected or proposed test/fix target. `Existing` and `Proposed` stay
/// distinct variants: a proposed new-test target can never be represented as
/// an existing test.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepairCardTarget {
    Existing {
        symbol_id: String,
        file: String,
        line: usize,
        test_kind: RepairCardTestKind,
        /// The producer-owned relation basis (for example
        /// `RelationReason::as_str`).
        relation: String,
        workspace_identity: String,
    },
    Proposed {
        file: String,
        owner: String,
        proposal_kind: RepairCardProposedTestKind,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardTestKind {
    InlineUnit,
    Integration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardProposedTestKind {
    InlineUnit,
    Integration,
}

/// The assertion/observer outcome the edited test must establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCardAssertionGoal {
    /// A producer-owned suggested assertion is available; the card carries
    /// its text.
    SuggestedAssertion,
    /// Effect route: the observer setup fact is the goal; the card carries
    /// the producer-owned observer description.
    ObserverSetup,
}

/// Completion axes stay separate (#3166): the card names each axis and never
/// collapses one into another.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticMovementGoal {
    ClosedBySelectedRoute,
    ImprovedAccordingToSelectedRoute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusedExecutionGoal {
    VerifiedPass,
    ExplicitlyNotRun,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditCageGoal {
    Compliant,
}

/// Optional and separately stated: mutation confirmation is never required to
/// close a card axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationConfirmationGoal {
    NotRequested,
    OptionalSeparateConfirmation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardCurrentnessGoal {
    Current,
    AcceptedDirtyDraft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardDoneWhen {
    pub static_movement: StaticMovementGoal,
    pub focused_test_execution: FocusedExecutionGoal,
    pub edit_cage: EditCageGoal,
    pub mutation_confirmation: MutationConfirmationGoal,
    pub currentness: CardCurrentnessGoal,
}

/// One typed command reference. This is a reference to a `CommandSpec`
/// authority (identity + role + bounded display), never a reconstruction of
/// argv from display prose.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardCommandRef {
    pub command_id: String,
    pub role: String,
    /// Presentation-only: excluded from the semantic digest.
    pub display: String,
}

/// A load-bearing rejected alternative with its producer-owned reason.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardRejectedAlternative {
    pub target: String,
    pub reason: String,
}

/// Observed attempt state, copied from the RepairAttempt authority. The card
/// never upgrades or completes it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardAttempt {
    pub attempt_id: String,
    /// The attempt authority's state, serialized snake_case (for example
    /// `awaiting_edit`).
    pub state: String,
}

/// The compact work order. Field-for-field projections are documented at each
/// site; the builder owns all derivation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardV1 {
    pub schema_version: String,
    /// Semantic digest over the portable card surface; reminted when any
    /// load-bearing field changes.
    pub repair_card_id: String,
    pub snapshot: RepairCardSnapshot,
    pub subject: RepairCardSubject,
    /// The fix-instruction vocabulary, verbatim from its authority.
    pub instruction: FixInstructionSummary,
    pub readiness: RepairCardReadinessFacts,
    pub changed_behavior: String,
    /// The exact missing discriminator or named limitation, when the producer
    /// owns one; never fabricated by the card.
    pub exact_blocker: Option<String>,
    pub selected_target: Option<RepairCardTarget>,
    pub assertion_goal: Option<RepairCardAssertionGoal>,
    /// Producer-owned suggested assertion or observer description.
    pub assertion_goal_detail: Option<String>,
    /// Producer-owned candidate input/value when the route supplies one.
    pub candidate_value: Option<String>,
    /// Edit-cage surfaces, supplied by the caller from the existing packet
    /// authority; the card never derives membership.
    pub allowed_files: Vec<String>,
    pub forbidden_files: Vec<String>,
    pub done_when: RepairCardDoneWhen,
    pub stop_conditions: Vec<String>,
    /// One next action as a typed command reference. Absent when the card is
    /// stale, limited, unavailable, or has no ready route.
    pub next_action: Option<RepairCardCommandRef>,
    /// Producer-owned basis for the selected target (relation vocabulary).
    pub selected_basis: Option<String>,
    pub rejected_alternatives: Vec<RepairCardRejectedAlternative>,
    pub attempt: Option<RepairCardAttempt>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
    /// Stable typed routes to every omitted load-bearing evidence family
    /// (#4666), in deterministic family-sorted order.
    #[serde(default)]
    pub detail_references: Vec<RepairCardDetailRef>,
    /// Measured budget accounting for this wire card (#4666).
    #[serde(default)]
    pub detail_summary: RepairCardDetailSummary,
    /// Identity of the complete evidence: the semantic card id joined with
    /// every routed family's content digest. Budget-independent.
    #[serde(default)]
    pub complete_evidence_digest: String,
}

/// Every exposure gate in one place: a card may expose a current edit or
/// execution route only when the instruction vocabulary and the readiness
/// authority's own flip both allow it. The builder consults this; no
/// renderer re-implements it. The semantic digest lives at the crate root
/// (`crate::repair_card_digest`), which owns JSON rendering; domain stays
/// free of it.
pub fn repair_card_route_exposable(instruction: FixInstructionState, repair_ready: bool) -> bool {
    matches!(instruction, FixInstructionState::FixSiteReady) && repair_ready
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_exposure_follows_instruction_and_readiness_together() {
        assert!(repair_card_route_exposable(
            FixInstructionState::FixSiteReady,
            true
        ));
        for instruction in [
            FixInstructionState::Stale,
            FixInstructionState::StaticLimitation,
            FixInstructionState::Unavailable,
            FixInstructionState::InspectOnly,
        ] {
            assert!(!repair_card_route_exposable(instruction, true));
        }
        assert!(!repair_card_route_exposable(
            FixInstructionState::FixSiteReady,
            false
        ));
    }

    #[test]
    fn budget_validate_rejects_zero_limits() {
        let cases = [
            (
                RepairCardBudget {
                    max_detail_items: 0,
                    ..RepairCardBudget::default()
                },
                "max_detail_items must be greater than zero",
            ),
            (
                RepairCardBudget {
                    max_serialized_bytes: 0,
                    ..RepairCardBudget::default()
                },
                "max_serialized_bytes must be greater than zero",
            ),
            (
                RepairCardBudget {
                    max_inline_detail_bytes: 0,
                    ..RepairCardBudget::default()
                },
                "max_inline_detail_bytes must be greater than zero",
            ),
        ];
        for (budget, expected) in cases {
            assert_eq!(budget.validate().err().as_deref(), Some(expected));
        }
        assert!(matches!(RepairCardBudget::default().validate(), Ok(())));
    }

    #[test]
    fn detail_summary_default_names_the_budget_version() {
        let summary = RepairCardDetailSummary::default();
        assert_eq!(summary.budget_version, REPAIR_CARD_BUDGET_VERSION);
        assert_eq!(summary.referenced_items, 0);
        assert_eq!(summary.unavailable_items, 0);
        assert_eq!(summary.omitted_bytes, 0);
        assert!(summary.omission_classes.is_empty());
    }
}
