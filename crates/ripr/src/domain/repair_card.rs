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

/// Verbatim readiness facts copied from the route-readiness authority. The
/// counts and lists are never re-derived or recomputed here.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepairCardReadinessFacts {
    /// The readiness authority's own repair-ready flip, copied verbatim.
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
}
