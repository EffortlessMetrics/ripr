//! Closed identity kinds and portability classes for the governed registry.

/// Taxonomy identities named by #1932 / #4804, plus serialized siblings that
/// already exist and must keep a distinct disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum IdentityKind {
    CanonicalItemId,
    InstructionSemanticId,
    InstructionInstanceId,
    ActionId,
    EditInstructionId,
    AnalysisAttemptId,
    CompletedAnalysisSnapshotId,
    InputIdentity,
    DiagnosticResultId,
    ContinuationId,
    CommandId,
    RepairAttemptId,
    ReceiptId,
    SeamLocationId,
    FindingId,
    DiagnosticCodeId,
    FeedbackReceiptId,
}

impl IdentityKind {
    pub(crate) const ALL: &[IdentityKind] = &[
        Self::CanonicalItemId,
        Self::InstructionSemanticId,
        Self::InstructionInstanceId,
        Self::ActionId,
        Self::EditInstructionId,
        Self::AnalysisAttemptId,
        Self::CompletedAnalysisSnapshotId,
        Self::InputIdentity,
        Self::DiagnosticResultId,
        Self::ContinuationId,
        Self::CommandId,
        Self::RepairAttemptId,
        Self::ReceiptId,
        Self::SeamLocationId,
        Self::FindingId,
        Self::DiagnosticCodeId,
        Self::FeedbackReceiptId,
    ];

    /// #1932 / #4804 required taxonomy. Serialized siblings may also appear.
    pub(crate) const REQUIRED_TAXONOMY: &[IdentityKind] = &[
        Self::CanonicalItemId,
        Self::InstructionSemanticId,
        Self::InstructionInstanceId,
        Self::ActionId,
        Self::EditInstructionId,
        Self::AnalysisAttemptId,
        Self::CompletedAnalysisSnapshotId,
        Self::InputIdentity,
        Self::DiagnosticResultId,
        Self::ContinuationId,
        Self::CommandId,
        Self::RepairAttemptId,
        Self::ReceiptId,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalItemId => "CanonicalItemId",
            Self::InstructionSemanticId => "InstructionSemanticId",
            Self::InstructionInstanceId => "InstructionInstanceId",
            Self::ActionId => "ActionId",
            Self::EditInstructionId => "EditInstructionId",
            Self::AnalysisAttemptId => "AnalysisAttemptId",
            Self::CompletedAnalysisSnapshotId => "CompletedAnalysisSnapshotId",
            Self::InputIdentity => "InputIdentity",
            Self::DiagnosticResultId => "DiagnosticResultId",
            Self::ContinuationId => "ContinuationId",
            Self::CommandId => "CommandId",
            Self::RepairAttemptId => "RepairAttemptId",
            Self::ReceiptId => "ReceiptId",
            Self::SeamLocationId => "SeamLocationId",
            Self::FindingId => "FindingId",
            Self::DiagnosticCodeId => "DiagnosticCodeId",
            Self::FeedbackReceiptId => "FeedbackReceiptId",
        }
    }
}

/// Portability class recorded by the registry. This is vocabulary, not a
/// claim that every consumer has migrated onto the class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PortabilityClass {
    Portable,
    RepositoryBound,
    SnapshotBound,
    SessionBound,
    TransportBound,
}

impl PortabilityClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Portable => "portable",
            Self::RepositoryBound => "repository-bound",
            Self::SnapshotBound => "snapshot-bound",
            Self::SessionBound => "session-bound",
            Self::TransportBound => "transport-bound",
        }
    }

    pub(crate) const fn is_portable(self) -> bool {
        matches!(self, Self::Portable)
    }
}

/// Inputs that must never enter a portable semantic identity unless a
/// narrower accepted contract already requires one.
pub(crate) const FORBIDDEN_PORTABLE_INPUTS: &[&str] = &[
    "timestamp",
    "timestamps",
    "created_unix_ms",
    "observed_at",
    "recorded_at",
    "pid",
    "client_name",
    "client",
    "display",
    "display_text",
    "human_display",
    "markdown",
    "scheduler_generation",
    "request_order",
    "traversal_order",
    "map_order",
    "absolute_checkout",
    "absolute_path",
    "effective_root",
];

pub(crate) fn is_forbidden_portable_input(name: &str) -> bool {
    FORBIDDEN_PORTABLE_INPUTS.contains(&name)
}
