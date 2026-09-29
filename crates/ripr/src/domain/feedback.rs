//! Local result-bound usefulness feedback identity (RIPR-SPEC-0179 / #4585).
//!
//! This module owns the typed receipt vocabulary and identity rules. It does
//! not write files, talk to the network, or change analyzer, gate, baseline,
//! suppression, or diagnostic state.

/// Schema version for usefulness-feedback receipts and join documents.
pub(crate) const FEEDBACK_SCHEMA_VERSION: &str = "0.1";

/// Maximum UTF-8 byte length of an optional note.
pub(crate) const FEEDBACK_NOTE_MAX_BYTES: usize = 1024;

/// Closed usefulness judgment. Parent reason codes map onto these five
/// classes so positive, incorrect, unclear, expensive, and intentional
/// no-action remain distinguishable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeedbackJudgment {
    Useful,
    Incorrect,
    Unclear,
    Expensive,
    IntentionalNoAction,
}

impl FeedbackJudgment {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Incorrect => "incorrect",
            Self::Unclear => "unclear",
            Self::Expensive => "expensive",
            Self::IntentionalNoAction => "intentional_no_action",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "useful" => Ok(Self::Useful),
            "incorrect" => Ok(Self::Incorrect),
            "unclear" => Ok(Self::Unclear),
            "expensive" => Ok(Self::Expensive),
            "intentional_no_action" => Ok(Self::IntentionalNoAction),
            other => Err(format!(
                "unknown feedback judgment {other:?}; expected useful, incorrect, unclear, expensive, or intentional_no_action"
            )),
        }
    }
}

/// Closed reason taxonomy. `other` is the escape hatch and requires a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeedbackReason {
    UsefulActionable,
    UsefulLimitation,
    CorrectTarget,
    CorrectDiscriminator,
    ClearExplanation,
    WrongTarget,
    WrongDiscriminator,
    FalseActionable,
    DuplicateItem,
    AlreadyCovered,
    IncorrectCausalAttribution,
    StaleResult,
    MissingContext,
    AmbiguousTarget,
    UnclearExplanation,
    VerifyCommandFailed,
    ReceiptCommandFailed,
    RouteTooBroad,
    TooSlow,
    TooNoisy,
    UnsupportedWorkflow,
    IntentionalDefensiveCode,
    IntentionalLowGrip,
    ManualReviewPreferred,
    Other,
}

impl FeedbackReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UsefulActionable => "useful_actionable",
            Self::UsefulLimitation => "useful_limitation",
            Self::CorrectTarget => "correct_target",
            Self::CorrectDiscriminator => "correct_discriminator",
            Self::ClearExplanation => "clear_explanation",
            Self::WrongTarget => "wrong_target",
            Self::WrongDiscriminator => "wrong_discriminator",
            Self::FalseActionable => "false_actionable",
            Self::DuplicateItem => "duplicate_item",
            Self::AlreadyCovered => "already_covered",
            Self::IncorrectCausalAttribution => "incorrect_causal_attribution",
            Self::StaleResult => "stale_result",
            Self::MissingContext => "missing_context",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::UnclearExplanation => "unclear_explanation",
            Self::VerifyCommandFailed => "verify_command_failed",
            Self::ReceiptCommandFailed => "receipt_command_failed",
            Self::RouteTooBroad => "route_too_broad",
            Self::TooSlow => "too_slow",
            Self::TooNoisy => "too_noisy",
            Self::UnsupportedWorkflow => "unsupported_workflow",
            Self::IntentionalDefensiveCode => "intentional_defensive_code",
            Self::IntentionalLowGrip => "intentional_low_grip",
            Self::ManualReviewPreferred => "manual_review_preferred",
            Self::Other => "other",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        for reason in Self::all() {
            if reason.as_str() == value {
                return Ok(reason);
            }
        }
        Err(format!(
            "unknown feedback reason {value:?}; run `ripr feedback --help` for the closed taxonomy"
        ))
    }

    pub(crate) const fn judgment(self) -> FeedbackJudgment {
        match self {
            Self::UsefulActionable
            | Self::UsefulLimitation
            | Self::CorrectTarget
            | Self::CorrectDiscriminator
            | Self::ClearExplanation => FeedbackJudgment::Useful,
            Self::WrongTarget
            | Self::WrongDiscriminator
            | Self::FalseActionable
            | Self::DuplicateItem
            | Self::AlreadyCovered
            | Self::IncorrectCausalAttribution
            | Self::StaleResult => FeedbackJudgment::Incorrect,
            Self::MissingContext | Self::AmbiguousTarget | Self::UnclearExplanation => {
                FeedbackJudgment::Unclear
            }
            Self::VerifyCommandFailed
            | Self::ReceiptCommandFailed
            | Self::RouteTooBroad
            | Self::TooSlow
            | Self::TooNoisy
            | Self::UnsupportedWorkflow => FeedbackJudgment::Expensive,
            Self::IntentionalDefensiveCode
            | Self::IntentionalLowGrip
            | Self::ManualReviewPreferred => FeedbackJudgment::IntentionalNoAction,
            Self::Other => {
                // `other` has no implied class; callers must supply `--judgment`.
                FeedbackJudgment::Unclear
            }
        }
    }

    pub(crate) const fn requires_note(self) -> bool {
        matches!(self, Self::Other)
    }

    pub(crate) const fn all() -> [Self; 25] {
        [
            Self::UsefulActionable,
            Self::UsefulLimitation,
            Self::CorrectTarget,
            Self::CorrectDiscriminator,
            Self::ClearExplanation,
            Self::WrongTarget,
            Self::WrongDiscriminator,
            Self::FalseActionable,
            Self::DuplicateItem,
            Self::AlreadyCovered,
            Self::IncorrectCausalAttribution,
            Self::StaleResult,
            Self::MissingContext,
            Self::AmbiguousTarget,
            Self::UnclearExplanation,
            Self::VerifyCommandFailed,
            Self::ReceiptCommandFailed,
            Self::RouteTooBroad,
            Self::TooSlow,
            Self::TooNoisy,
            Self::UnsupportedWorkflow,
            Self::IntentionalDefensiveCode,
            Self::IntentionalLowGrip,
            Self::ManualReviewPreferred,
            Self::Other,
        ]
    }
}

/// Who recorded the receipt. Distinct from review status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActorKind {
    Human,
    Agent,
    Unknown,
}

impl ActorKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!(
                "unknown feedback actor {other:?}; expected human, agent, or unknown"
            )),
        }
    }
}

/// Review disposition, independent of [`ActorKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewStatus {
    Unreviewed,
    ReviewedAccepted,
    ReviewedRejected,
}

impl ReviewStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Unreviewed => "unreviewed",
            Self::ReviewedAccepted => "reviewed_accepted",
            Self::ReviewedRejected => "reviewed_rejected",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "unreviewed" => Ok(Self::Unreviewed),
            "reviewed_accepted" => Ok(Self::ReviewedAccepted),
            "reviewed_rejected" => Ok(Self::ReviewedRejected),
            other => Err(format!(
                "unknown feedback review status {other:?}; expected unreviewed, reviewed_accepted, or reviewed_rejected"
            )),
        }
    }

    pub(crate) const fn is_reviewed(self) -> bool {
        !matches!(self, Self::Unreviewed)
    }
}

/// Whether a stored identity still names the live result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceState {
    Current,
    Historical,
    Mismatched,
}

impl ReferenceState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Historical => "historical",
            Self::Mismatched => "mismatched",
        }
    }
}

/// Immutable analysis/result identity. File/line is not part of this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResultIdentity {
    pub snapshot_id: String,
    pub canonical_item: Option<String>,
    pub route_digest: Option<String>,
    pub attempt_id: Option<String>,
    pub receipt_id: Option<String>,
}

impl ResultIdentity {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.snapshot_id.trim().is_empty() {
            return Err(
                "feedback requires --snapshot; file and line are not a result identity".to_string(),
            );
        }
        if self.snapshot_id.chars().any(char::is_control) {
            return Err("feedback --snapshot must not contain control characters".to_string());
        }
        for (label, value) in [
            ("--item", self.canonical_item.as_deref()),
            ("--route-digest", self.route_digest.as_deref()),
            ("--attempt", self.attempt_id.as_deref()),
            ("--receipt", self.receipt_id.as_deref()),
        ] {
            if let Some(value) = value
                && (value.trim().is_empty() || value.chars().any(char::is_control))
            {
                return Err(format!(
                    "feedback {label} must be a non-empty identity without control characters"
                ));
            }
        }
        Ok(())
    }
}

/// In-memory usefulness-feedback receipt. Timestamp is outside payload identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FeedbackReceipt {
    pub feedback_id: String,
    pub idempotency_key: String,
    pub identity: ResultIdentity,
    pub actor_kind: ActorKind,
    pub review_status: ReviewStatus,
    pub review_actor_kind: Option<ActorKind>,
    pub reason: FeedbackReason,
    /// Required when `reason` is `other`; otherwise derived from the reason.
    pub judgment_override: Option<FeedbackJudgment>,
    pub note: Option<String>,
    pub reference_state: ReferenceState,
    pub recorded_at: String,
}

impl FeedbackReceipt {
    pub(crate) fn judgment(&self) -> FeedbackJudgment {
        self.judgment_override
            .unwrap_or_else(|| self.reason.judgment())
    }
}

/// Semantic payload used for idempotency comparison. Timestamp is excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FeedbackPayload {
    pub identity: ResultIdentity,
    pub actor_kind: ActorKind,
    pub review_status: ReviewStatus,
    pub review_actor_kind: Option<ActorKind>,
    pub reason: FeedbackReason,
    pub judgment_override: Option<FeedbackJudgment>,
    pub note: Option<String>,
}

impl FeedbackPayload {
    pub(crate) fn from_receipt(receipt: &FeedbackReceipt) -> Self {
        Self {
            identity: receipt.identity.clone(),
            actor_kind: receipt.actor_kind,
            review_status: receipt.review_status,
            review_actor_kind: receipt.review_actor_kind,
            reason: receipt.reason,
            judgment_override: receipt.judgment_override,
            note: receipt.note.clone(),
        }
    }

    pub(crate) fn judgment(&self) -> FeedbackJudgment {
        self.judgment_override
            .unwrap_or_else(|| self.reason.judgment())
    }

    /// Collapse a matching `--judgment` onto the derived class so reload and
    /// repeat recording compare the same payload.
    pub(crate) fn normalized(mut self) -> Self {
        if self.reason != FeedbackReason::Other
            && self.judgment_override == Some(self.reason.judgment())
        {
            self.judgment_override = None;
        }
        self
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.identity.validate()?;
        if self.reason.requires_note()
            && self.note.as_ref().is_none_or(|note| note.trim().is_empty())
        {
            return Err("feedback reason `other` requires a bounded --note".to_string());
        }
        if self.reason == FeedbackReason::Other && self.judgment_override.is_none() {
            return Err(
                "feedback reason `other` requires --judgment useful|incorrect|unclear|expensive|intentional_no_action"
                    .to_string(),
            );
        }
        if self.reason != FeedbackReason::Other
            && self
                .judgment_override
                .is_some_and(|judgment| judgment != self.reason.judgment())
        {
            return Err(
                "feedback --judgment must match the closed reason class unless --reason is `other`"
                    .to_string(),
            );
        }
        if let Some(note) = &self.note {
            if note.len() > FEEDBACK_NOTE_MAX_BYTES {
                return Err(format!(
                    "feedback --note exceeds {FEEDBACK_NOTE_MAX_BYTES} bytes"
                ));
            }
            if note.chars().any(|ch| ch == '\0') {
                return Err("feedback --note must not contain NUL".to_string());
            }
        }
        if self.review_status.is_reviewed() && self.review_actor_kind.is_none() {
            return Err(
                "reviewed feedback requires --review-actor (human, agent, or unknown)".to_string(),
            );
        }
        if !self.review_status.is_reviewed() && self.review_actor_kind.is_some() {
            return Err(
                "unreviewed feedback cannot carry --review-actor; omit it or set --review"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(crate) fn canonical_bytes(&self) -> String {
        format!(
            "v1\nsnapshot={}\nitem={}\nroute={}\nattempt={}\nreceipt={}\nactor={}\nreview={}\nreview_actor={}\nreason={}\njudgment={}\nnote={}\n",
            self.identity.snapshot_id,
            self.identity.canonical_item.as_deref().unwrap_or(""),
            self.identity.route_digest.as_deref().unwrap_or(""),
            self.identity.attempt_id.as_deref().unwrap_or(""),
            self.identity.receipt_id.as_deref().unwrap_or(""),
            self.actor_kind.as_str(),
            self.review_status.as_str(),
            self.review_actor_kind.map(ActorKind::as_str).unwrap_or(""),
            self.reason.as_str(),
            self.judgment().as_str(),
            self.note.as_deref().unwrap_or(""),
        )
    }
}

/// Classify a stored identity against a live result identity.
///
/// Absent live identity is not evidence of staleness: recording and export
/// without a comparison subject keep the binding `current`. Historical is
/// reserved for a later attempt/receipt on the same result. Mismatch is a
/// different snapshot or item, not an absent comparison.
pub(crate) fn classify_reference(
    stored: &ResultIdentity,
    live: Option<&ResultIdentity>,
) -> ReferenceState {
    let Some(live) = live else {
        return ReferenceState::Current;
    };
    if stored == live {
        return ReferenceState::Current;
    }
    if stored.snapshot_id == live.snapshot_id
        && stored.canonical_item == live.canonical_item
        && stored.route_digest == live.route_digest
    {
        // Same result, later attempt/receipt: retain the historical binding.
        return ReferenceState::Historical;
    }
    ReferenceState::Mismatched
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(snapshot: &str, item: Option<&str>) -> ResultIdentity {
        ResultIdentity {
            snapshot_id: snapshot.to_string(),
            canonical_item: item.map(str::to_string),
            route_digest: Some("add_missing_test".to_string()),
            attempt_id: None,
            receipt_id: None,
        }
    }

    #[test]
    fn every_reason_maps_onto_one_of_the_five_judgments() {
        for reason in FeedbackReason::all() {
            let _ = reason.judgment();
            assert_eq!(FeedbackReason::parse(reason.as_str()).ok(), Some(reason));
        }
        assert_eq!(
            FeedbackReason::UsefulLimitation.judgment(),
            FeedbackJudgment::Useful
        );
        assert_eq!(
            FeedbackReason::WrongDiscriminator.judgment(),
            FeedbackJudgment::Incorrect
        );
        assert_eq!(
            FeedbackReason::UnclearExplanation.judgment(),
            FeedbackJudgment::Unclear
        );
        assert_eq!(
            FeedbackReason::TooSlow.judgment(),
            FeedbackJudgment::Expensive
        );
        assert_eq!(
            FeedbackReason::ManualReviewPreferred.judgment(),
            FeedbackJudgment::IntentionalNoAction
        );
        assert!(FeedbackReason::Other.requires_note());
        assert!(!FeedbackReason::UsefulLimitation.requires_note());
    }

    #[test]
    fn actor_kind_is_not_review_status() {
        assert_ne!(
            ActorKind::Agent.as_str(),
            ReviewStatus::ReviewedAccepted.as_str()
        );
        assert!(!ReviewStatus::Unreviewed.is_reviewed());
        assert!(ReviewStatus::ReviewedAccepted.is_reviewed());
    }

    #[test]
    fn snapshot_is_required_and_file_line_is_not_identity() -> Result<(), String> {
        let missing = ResultIdentity {
            snapshot_id: String::new(),
            canonical_item: None,
            route_digest: None,
            attempt_id: None,
            receipt_id: None,
        };
        let error = missing
            .validate()
            .err()
            .ok_or_else(|| "empty snapshot must fail".to_string())?;
        assert!(error.contains("--snapshot"));
        assert!(error.contains("file and line"));
        Ok(())
    }

    #[test]
    fn useful_limitation_may_omit_canonical_item() -> Result<(), String> {
        identity("snap-1", None)
            .validate()
            .map_err(|error| format!("limitation without a gap id is a valid identity: {error}"))?;
        Ok(())
    }

    #[test]
    fn other_requires_a_note_and_explicit_judgment() -> Result<(), String> {
        let mut payload = FeedbackPayload {
            identity: identity("snap-1", None),
            actor_kind: ActorKind::Human,
            review_status: ReviewStatus::Unreviewed,
            review_actor_kind: None,
            reason: FeedbackReason::Other,
            judgment_override: None,
            note: None,
        };
        let error = payload
            .validate()
            .err()
            .ok_or_else(|| "other without note".to_string())?;
        assert!(error.contains("other"));
        assert!(error.contains("--note"));
        payload.note = Some("custom operator judgment".to_string());
        let error = payload
            .validate()
            .err()
            .ok_or_else(|| "other without judgment".to_string())?;
        assert!(error.contains("--judgment"));
        payload.judgment_override = Some(FeedbackJudgment::Expensive);
        payload
            .validate()
            .map_err(|error| format!("other with note and judgment: {error}"))?;
        assert_eq!(payload.judgment(), FeedbackJudgment::Expensive);
        Ok(())
    }

    #[test]
    fn closed_reason_rejects_a_mismatched_judgment() -> Result<(), String> {
        let payload = FeedbackPayload {
            identity: identity("snap-1", None),
            actor_kind: ActorKind::Human,
            review_status: ReviewStatus::Unreviewed,
            review_actor_kind: None,
            reason: FeedbackReason::UsefulLimitation,
            judgment_override: Some(FeedbackJudgment::Incorrect),
            note: None,
        };
        let error = payload
            .validate()
            .err()
            .ok_or_else(|| "mismatched judgment".to_string())?;
        assert!(error.contains("--judgment"));
        assert!(error.contains("other"));
        Ok(())
    }

    #[test]
    fn absent_live_identity_is_current_not_staleness() {
        assert_eq!(
            classify_reference(&identity("snap-1", None), None),
            ReferenceState::Current
        );
    }

    #[test]
    fn reviewed_feedback_requires_a_review_actor() -> Result<(), String> {
        let payload = FeedbackPayload {
            identity: identity("snap-1", Some("gap:example")),
            actor_kind: ActorKind::Agent,
            review_status: ReviewStatus::ReviewedAccepted,
            review_actor_kind: None,
            reason: FeedbackReason::UsefulActionable,
            judgment_override: None,
            note: None,
        };
        let error = payload
            .validate()
            .err()
            .ok_or_else(|| "reviewed without actor".to_string())?;
        assert!(error.contains("--review-actor"));
        Ok(())
    }

    #[test]
    fn same_file_line_different_item_is_a_different_identity() {
        let left = identity("snap-1", Some("gap:alpha"));
        let right = identity("snap-1", Some("gap:beta"));
        assert_ne!(left, right);
        assert_eq!(
            classify_reference(&left, Some(&right)),
            ReferenceState::Mismatched
        );
    }

    #[test]
    fn a_later_attempt_keeps_the_historical_reference() {
        let mut stored = identity("snap-1", Some("gap:alpha"));
        stored.attempt_id = Some("attempt-a".to_string());
        let mut live = stored.clone();
        live.attempt_id = Some("attempt-b".to_string());
        assert_eq!(
            classify_reference(&stored, Some(&live)),
            ReferenceState::Historical
        );
    }

    #[test]
    fn payload_excludes_timestamp_from_identity() {
        let payload = FeedbackPayload {
            identity: identity("snap-1", Some("gap:alpha")),
            actor_kind: ActorKind::Human,
            review_status: ReviewStatus::Unreviewed,
            review_actor_kind: None,
            reason: FeedbackReason::UsefulLimitation,
            judgment_override: None,
            note: None,
        };
        let first = payload.canonical_bytes();
        let second = payload.canonical_bytes();
        assert_eq!(first, second);
        assert!(!first.contains("recorded_at"));
        assert!(!first.contains("unix_ms"));
    }
}
