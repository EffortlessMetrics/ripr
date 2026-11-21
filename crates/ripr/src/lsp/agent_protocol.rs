use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, REFRESH_COMMAND,
};
use crate::domain::{CommandExecutionMode, CommandRole, CommandSpec};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use std::fmt;
use tower_lsp_server::ls_types::LSPAny;

pub(super) const RIPR_AGENT_PROTOCOL_VERSION: &str = "0.1";
/// Additive schema minor bump (#1617 slice 4, RIPR-SPEC-0131): the success
/// envelope gains route-readiness and typed-command-spec fields under schema
/// 0.2. The major stays 0, so the major-gated parse contract keeps
/// 0.1-versioned clients parsing the version identity; only readers that
/// enforce the closed DTO field set must adopt the 0.2 shape. The protocol
/// version stays 0.1: the wire vocabulary and compatibility rules did not
/// change, only the serialized DTO shape did.
pub(crate) const RIPR_AGENT_SCHEMA_VERSION: &str = "0.2";
const RIPR_AGENT_SUPPORTED_PROTOCOL_MAJOR: u16 = 0;
const RIPR_AGENT_SUPPORTED_SCHEMA_MAJOR: u16 = 0;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct RiprAgentProtocolVersion(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiprAgentProtocolVersionError {
    InvalidFormat(String),
    UnsupportedMajor { received: u16, supported: u16 },
}

impl fmt::Display for RiprAgentProtocolVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(value) => write!(formatter, "invalid protocol version `{value}`"),
            Self::UnsupportedMajor {
                received,
                supported,
            } => write!(
                formatter,
                "unsupported protocol major `{received}`; supported major is `{supported}`"
            ),
        }
    }
}

impl RiprAgentProtocolVersion {
    fn current() -> Self {
        Self(RIPR_AGENT_PROTOCOL_VERSION.to_string())
    }

    /// The wire string for this version, for bounded status projections
    /// (#1987, RIPR-SPEC-0143).
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn parse(value: &str) -> Result<Self, RiprAgentProtocolVersionError> {
        let Some((major, minor)) = value.split_once('.') else {
            return Err(RiprAgentProtocolVersionError::InvalidFormat(
                value.to_string(),
            ));
        };
        if major.is_empty()
            || minor.is_empty()
            || !major.chars().all(|character| character.is_ascii_digit())
            || !minor.chars().all(|character| character.is_ascii_digit())
        {
            return Err(RiprAgentProtocolVersionError::InvalidFormat(
                value.to_string(),
            ));
        }
        let major = major
            .parse::<u16>()
            .map_err(|_error| RiprAgentProtocolVersionError::InvalidFormat(value.to_string()))?;
        if major != RIPR_AGENT_SUPPORTED_PROTOCOL_MAJOR {
            return Err(RiprAgentProtocolVersionError::UnsupportedMajor {
                received: major,
                supported: RIPR_AGENT_SUPPORTED_PROTOCOL_MAJOR,
            });
        }
        Ok(Self(value.to_string()))
    }
}

impl<'de> Deserialize<'de> for RiprAgentProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct RiprAgentSchemaVersion(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiprAgentSchemaVersionError {
    InvalidFormat(String),
    UnsupportedMajor { received: u16, supported: u16 },
}

impl fmt::Display for RiprAgentSchemaVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(value) => write!(formatter, "invalid schema version `{value}`"),
            Self::UnsupportedMajor {
                received,
                supported,
            } => write!(
                formatter,
                "unsupported schema major `{received}`; supported major is `{supported}`"
            ),
        }
    }
}

impl RiprAgentSchemaVersion {
    fn current() -> Self {
        Self(RIPR_AGENT_SCHEMA_VERSION.to_string())
    }

    fn parse(value: &str) -> Result<Self, RiprAgentSchemaVersionError> {
        let Some((major, minor)) = value.split_once('.') else {
            return Err(RiprAgentSchemaVersionError::InvalidFormat(
                value.to_string(),
            ));
        };
        if major.is_empty()
            || minor.is_empty()
            || !major.chars().all(|character| character.is_ascii_digit())
            || !minor.chars().all(|character| character.is_ascii_digit())
        {
            return Err(RiprAgentSchemaVersionError::InvalidFormat(
                value.to_string(),
            ));
        }
        let major = major
            .parse::<u16>()
            .map_err(|_error| RiprAgentSchemaVersionError::InvalidFormat(value.to_string()))?;
        if major != RIPR_AGENT_SUPPORTED_SCHEMA_MAJOR {
            return Err(RiprAgentSchemaVersionError::UnsupportedMajor {
                received: major,
                supported: RIPR_AGENT_SUPPORTED_SCHEMA_MAJOR,
            });
        }
        Ok(Self(value.to_string()))
    }
}

impl<'de> Deserialize<'de> for RiprAgentSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// A nullable wire field that must be present, even when its value is `null`.
///
/// `Option<T>` alone cannot distinguish an omitted field from an explicit
/// `null`, so protocol envelopes use this wrapper where the schema requires a
/// field to be present for forward-compatible validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiprAgentRequiredNullable<T> {
    Null,
    Value(T),
}

impl<T: Serialize> Serialize for RiprAgentRequiredNullable<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Null => serializer.serialize_none(),
            Self::Value(value) => serializer.serialize_some(value),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for RiprAgentRequiredNullable<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

fn require_nullable<T: DeserializeOwned>(
    value: serde_json::Value,
    field: &'static str,
) -> Result<RiprAgentRequiredNullable<T>, String> {
    if value.is_null() {
        Ok(RiprAgentRequiredNullable::Null)
    } else {
        serde_json::from_value(value)
            .map(RiprAgentRequiredNullable::Value)
            .map_err(|error| format!("{field}: {error}"))
    }
}

fn require_nullable_nonempty_string(
    value: serde_json::Value,
    field: &'static str,
) -> Result<RiprAgentRequiredNullable<String>, String> {
    match require_nullable::<String>(value, field)? {
        RiprAgentRequiredNullable::Null => Ok(RiprAgentRequiredNullable::Null),
        RiprAgentRequiredNullable::Value(text) if text.is_empty() => {
            Err(format!("{field}: identity strings must be non-empty"))
        }
        RiprAgentRequiredNullable::Value(text) => Ok(RiprAgentRequiredNullable::Value(text)),
    }
}

fn readiness_from_execution_mode(mode: CommandExecutionMode) -> RiprAgentRouteReadiness {
    match mode {
        CommandExecutionMode::Direct => RiprAgentRouteReadiness::TypedDirect,
        CommandExecutionMode::ShellRequired => RiprAgentRouteReadiness::TypedShellRequired,
        CommandExecutionMode::Manual => RiprAgentRouteReadiness::Manual,
    }
}

/// What the payload declares for one readiness field.
///
/// Omission (`Absent`) is distinct from an explicit `null`
/// (`ExplicitNull`): omission is tolerated only for schema-0.1 payloads,
/// while an explicit `null` is a committed claim that the route is `null`.
enum DeclaredReadiness {
    Absent,
    ExplicitNull,
    Value(RiprAgentRouteReadiness),
}

/// Resolve one route slot's readiness from what the envelope actually carries
/// (#1617 slice 4, RIPR-SPEC-0131 schema 0.2).
///
/// Fail-closed rules:
///
/// - readiness is `null` exactly when the legacy route string is `null`;
/// - a present command spec must carry the slot's role, must pass
///   [`CommandSpec::validate`], and its readiness is derived from its
///   execution mode — a declared readiness that disagrees is rejected;
/// - a route string without a command spec is `legacy_string_only`;
/// - an absent declared readiness is tolerated only for schema-0.1 payloads
///   (gated by the caller) and means the value is derived truthfully from the
///   route and spec that are present; an explicit `null` readiness is a
///   committed claim and must pair with a `null` route.
///
/// A spec is only credited when the producer owns it; this resolver never
/// synthesizes one from the legacy display string.
fn resolve_route_readiness(
    route: &RiprAgentRequiredNullable<String>,
    declared: DeclaredReadiness,
    command_spec: Option<&CommandSpec>,
    slot_role: CommandRole,
    slot: &'static str,
) -> Result<RiprAgentRequiredNullable<RiprAgentRouteReadiness>, String> {
    let derived = match (route, command_spec) {
        (RiprAgentRequiredNullable::Null, None) => RiprAgentRequiredNullable::Null,
        (RiprAgentRequiredNullable::Null, Some(_)) => {
            return Err(format!("{slot}: a command spec requires a non-null route"));
        }
        (RiprAgentRequiredNullable::Value(_), None) => {
            RiprAgentRequiredNullable::Value(RiprAgentRouteReadiness::LegacyStringOnly)
        }
        (RiprAgentRequiredNullable::Value(_), Some(command_spec)) => {
            if command_spec.role != slot_role {
                return Err(format!(
                    "{slot}: command spec role does not match the slot role"
                ));
            }
            command_spec
                .validate()
                .map_err(|error| format!("{slot}: {error}"))?;
            RiprAgentRequiredNullable::Value(readiness_from_execution_mode(
                command_spec.execution_mode,
            ))
        }
    };
    let declared_matches = match (&derived, &declared) {
        // Absent and explicit-null both agree with a null route.
        (RiprAgentRequiredNullable::Null, DeclaredReadiness::Absent)
        | (RiprAgentRequiredNullable::Null, DeclaredReadiness::ExplicitNull) => true,
        // A declared readiness next to a null route claims readiness for a
        // route that does not exist.
        (RiprAgentRequiredNullable::Null, DeclaredReadiness::Value(_)) => false,
        // Omission derives from what the payload carries.
        (RiprAgentRequiredNullable::Value(_), DeclaredReadiness::Absent) => true,
        // An explicit null beside a non-null route claims that a live route
        // carries no readiness — a contract violation, not a derivation hint.
        (RiprAgentRequiredNullable::Value(_), DeclaredReadiness::ExplicitNull) => false,
        (
            RiprAgentRequiredNullable::Value(derived_value),
            DeclaredReadiness::Value(declared_value),
        ) => derived_value == declared_value,
    };
    if !declared_matches {
        return Err(format!(
            "{slot}: declared readiness does not match the route and command spec"
        ));
    }
    Ok(derived)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RiprAgentVersionIdentity {
    pub(super) protocol_version: RiprAgentProtocolVersion,
    pub(super) schema_version: RiprAgentSchemaVersion,
}

impl RiprAgentVersionIdentity {
    fn current() -> Self {
        Self {
            protocol_version: RiprAgentProtocolVersion::current(),
            schema_version: RiprAgentSchemaVersion::current(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(super) enum RiprAgentRequest {
    #[serde(rename = "ripr/workspaceStatus")]
    WorkspaceStatus,
    #[serde(rename = "ripr/refreshAnalysis")]
    RefreshAnalysis,
    #[serde(rename = "ripr/listActionableItems")]
    ListActionableItems,
    #[serde(rename = "ripr/getRepairPacket")]
    GetRepairPacket,
    #[serde(rename = "ripr/getEvidenceContext")]
    GetEvidenceContext,
    #[serde(rename = "ripr/getTopLimitation")]
    GetTopLimitation,
    #[serde(rename = "ripr/getReceiptStatus")]
    GetReceiptStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentRequestMode {
    ReadOnly,
    Refresh,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentProfile {
    Actionable,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentErrorKind {
    NoSnapshot,
    AnalysisInFlight,
    StaleSnapshot,
    StaleContinuation,
    WorkspaceAmbiguous,
    ConfigInvalid,
    ItemNotFound,
    RouteStaticLimitation,
    UnsupportedProtocolVersion,
    UnsupportedSchemaVersion,
    UnsupportedProfile,
    Cancelled,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentResponseKind {
    WorkspaceStatus,
    AnalysisRefreshed,
    ActionableItems,
    RepairPacket,
    EvidenceContext,
    TopLimitation,
    ReceiptStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentRecoveryRoute {
    Refresh,
    SelectWorkspace,
    FixConfiguration,
    RequestSupportedVersion,
    Retry,
    InspectLimitation,
    DiscardStaleRequest,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentImplementationState {
    CapabilityOnly,
    /// At least one `ripr/*` request handler is registered and served.
    /// The `supported_requests` list in the capability MUST match the
    /// actually-registered handlers.
    Implemented,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentDiagnosticMode {
    Push,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentSourceEditCapability {
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentAllowedEditSurface {
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentMustNotChange {
    SourceEdits,
    WorkspaceEdit,
    AutonomousRepair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentFreshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentRunStatus {
    Ready,
    AnalysisInFlight,
    Stale,
    Unknown,
}

/// Closed readiness vocabulary for the `verify_route`/`receipt_route` slots
/// (#1617 slice 4, RIPR-SPEC-0131 schema 0.2).
///
/// Readiness describes what the producer actually owns for a route slot; it
/// never claims execution happened or that a route is useful. A slot's
/// readiness is `null` exactly when its legacy route string is `null`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentRouteReadiness {
    /// A producer-owned typed CommandSpec with `execution_mode: direct`.
    TypedDirect,
    /// A producer-owned typed CommandSpec with `execution_mode: shell_required`.
    TypedShellRequired,
    /// The route can be described but no executable form is producer-owned.
    /// Declared for the closed vocabulary; no producer emits it today.
    Manual,
    /// Only the legacy display string exists; no typed CommandSpec.
    LegacyStringOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentSuccessStatus {
    #[serde(rename = "ok")]
    Ok,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RiprAgentErrorStatus {
    #[serde(rename = "error")]
    Error,
}

pub(super) const RESERVED_REQUESTS: &[RiprAgentRequest] = &[
    RiprAgentRequest::WorkspaceStatus,
    RiprAgentRequest::RefreshAnalysis,
    RiprAgentRequest::ListActionableItems,
    RiprAgentRequest::GetRepairPacket,
    RiprAgentRequest::GetEvidenceContext,
    RiprAgentRequest::GetTopLimitation,
    RiprAgentRequest::GetReceiptStatus,
];

pub(super) const RESERVED_ERROR_KINDS: &[RiprAgentErrorKind] = &[
    RiprAgentErrorKind::NoSnapshot,
    RiprAgentErrorKind::AnalysisInFlight,
    RiprAgentErrorKind::StaleSnapshot,
    RiprAgentErrorKind::StaleContinuation,
    RiprAgentErrorKind::WorkspaceAmbiguous,
    RiprAgentErrorKind::ConfigInvalid,
    RiprAgentErrorKind::ItemNotFound,
    RiprAgentErrorKind::RouteStaticLimitation,
    RiprAgentErrorKind::UnsupportedProtocolVersion,
    RiprAgentErrorKind::UnsupportedSchemaVersion,
    RiprAgentErrorKind::UnsupportedProfile,
    RiprAgentErrorKind::Cancelled,
    RiprAgentErrorKind::Superseded,
];

pub(super) const RESERVED_PROFILES: &[RiprAgentProfile] =
    &[RiprAgentProfile::Actionable, RiprAgentProfile::Full];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RiprAgentCapability {
    #[serde(flatten)]
    pub(super) versions: RiprAgentVersionIdentity,
    implementation_state: RiprAgentImplementationState,
    pub(super) supported_requests: Vec<RiprAgentRequest>,
    reserved_requests: Vec<RiprAgentRequest>,
    pub(super) supported_profiles: Vec<RiprAgentProfile>,
    reserved_profiles: Vec<RiprAgentProfile>,
    diagnostic_modes: Vec<RiprAgentDiagnosticMode>,
    snapshot_handles: bool,
    continuations: bool,
    work_done_progress: bool,
    cancellation: bool,
    pub(super) source_edit_capability: RiprAgentSourceEditCapability,
    analysis_status_notification: String,
    compatibility_commands: Vec<String>,
    error_kinds: Vec<RiprAgentErrorKind>,
    claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RiprAgentRequestEnvelopeWire")]
pub(super) struct RiprAgentRequestEnvelope {
    #[serde(flatten)]
    pub(super) versions: RiprAgentVersionIdentity,
    pub(super) request: RiprAgentRequest,
    pub(super) mode: RiprAgentRequestMode,
    pub(super) profile: RiprAgentRequiredNullable<RiprAgentProfile>,
    pub(super) snapshot_id: RiprAgentRequiredNullable<String>,
    pub(super) continuation_id: RiprAgentRequiredNullable<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiprAgentRequestEnvelopeWire {
    #[serde(flatten)]
    versions: RiprAgentVersionIdentity,
    request: RiprAgentRequest,
    mode: RiprAgentRequestMode,
    profile: serde_json::Value,
    snapshot_id: serde_json::Value,
    continuation_id: serde_json::Value,
}

impl TryFrom<RiprAgentRequestEnvelopeWire> for RiprAgentRequestEnvelope {
    type Error = String;

    fn try_from(value: RiprAgentRequestEnvelopeWire) -> Result<Self, Self::Error> {
        Ok(Self {
            versions: value.versions,
            request: value.request,
            mode: value.mode,
            profile: require_nullable(value.profile, "profile")?,
            snapshot_id: require_nullable_nonempty_string(value.snapshot_id, "snapshot_id")?,
            continuation_id: require_nullable_nonempty_string(
                value.continuation_id,
                "continuation_id",
            )?,
        })
    }
}

impl RiprAgentCapability {
    fn v0_1() -> Self {
        Self {
            versions: RiprAgentVersionIdentity::current(),
            implementation_state: RiprAgentImplementationState::CapabilityOnly,
            supported_requests: Vec::new(),
            reserved_requests: RESERVED_REQUESTS.to_vec(),
            supported_profiles: Vec::new(),
            reserved_profiles: RESERVED_PROFILES.to_vec(),
            diagnostic_modes: vec![RiprAgentDiagnosticMode::Push],
            snapshot_handles: false,
            continuations: false,
            work_done_progress: false,
            cancellation: false,
            source_edit_capability: RiprAgentSourceEditCapability::None,
            analysis_status_notification: "ripr/analysisStatus".to_string(),
            compatibility_commands: compatibility_commands()
                .into_iter()
                .map(str::to_string)
                .collect(),
            error_kinds: RESERVED_ERROR_KINDS.to_vec(),
            claim_boundary: concat!(
                "Capability negotiation only; ",
                "no riprAgent requests are implemented by this slice."
            )
            .to_string(),
        }
    }

    /// Capability with at least one handler implemented (#1603).
    /// `snapshot_id` is the existing refresh generation identity: an interim
    /// compatibility state for this first slice, NOT #1602's immutable
    /// snapshot-handle contract. It must not be represented as that contract
    /// in responses or docs; the immutable handle binding lands with #1602.
    pub(crate) fn v0_1_implemented() -> Self {
        Self {
            implementation_state: RiprAgentImplementationState::Implemented,
            supported_requests: vec![RiprAgentRequest::ListActionableItems],
            supported_profiles: vec![RiprAgentProfile::Actionable],
            snapshot_handles: true,
            cancellation: true,
            claim_boundary: concat!(
                "ripr/listActionableItems is implemented; ",
                "all other riprAgent requests remain reserved."
            )
            .to_string(),
            ..Self::v0_1()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiprAgentServerCapability {
    #[serde(rename = "riprAgent")]
    ripr_agent: RiprAgentCapability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RiprAgentSuccessEnvelopeWire")]
pub(super) struct RiprAgentSuccessEnvelope {
    #[serde(flatten)]
    pub(super) versions: RiprAgentVersionIdentity,
    pub(super) request: RiprAgentRequest,
    pub(super) kind: RiprAgentResponseKind,
    status: RiprAgentSuccessStatus,
    pub(super) snapshot_id: RiprAgentRequiredNullable<String>,
    pub(super) input_identity: RiprAgentRequiredNullable<String>,
    pub(super) root_identity: RiprAgentRequiredNullable<String>,
    pub(super) config_identity: RiprAgentRequiredNullable<String>,
    pub(super) base_identity: RiprAgentRequiredNullable<String>,
    freshness: RiprAgentFreshness,
    run_status: RiprAgentRunStatus,
    pub(super) profile: RiprAgentRequiredNullable<RiprAgentProfile>,
    pub(super) budget_identity: RiprAgentRequiredNullable<String>,
    selected_count: u64,
    omitted_count: u64,
    total_count: u64,
    complete_evidence_identity: RiprAgentRequiredNullable<String>,
    continuation_identity: RiprAgentRequiredNullable<String>,
    pub(super) allowed_edit_surface: RiprAgentAllowedEditSurface,
    pub(super) must_not_change: Vec<RiprAgentMustNotChange>,
    verify_route: RiprAgentRequiredNullable<String>,
    receipt_route: RiprAgentRequiredNullable<String>,
    /// Readiness of `verify_route`; `null` exactly when the route is `null`
    /// (#1617 slice 4). Legacy `verify_route` strings remain display/compat
    /// fields and carry `legacy_string_only` unless a producer-owned typed
    /// CommandSpec is present.
    verify_route_readiness: RiprAgentRequiredNullable<RiprAgentRouteReadiness>,
    /// Readiness of `receipt_route`; `null` exactly when the route is `null`.
    receipt_route_readiness: RiprAgentRequiredNullable<RiprAgentRouteReadiness>,
    /// Producer-owned typed verify route, if one exists. Never synthesized
    /// from the legacy display string.
    verify_command_spec: RiprAgentRequiredNullable<CommandSpec>,
    /// Producer-owned typed receipt route, if one exists.
    receipt_command_spec: RiprAgentRequiredNullable<CommandSpec>,
    limitations: Vec<String>,
    non_claims: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiprAgentSuccessEnvelopeWire {
    #[serde(flatten)]
    versions: RiprAgentVersionIdentity,
    request: RiprAgentRequest,
    kind: RiprAgentResponseKind,
    status: RiprAgentSuccessStatus,
    snapshot_id: serde_json::Value,
    input_identity: serde_json::Value,
    root_identity: serde_json::Value,
    config_identity: serde_json::Value,
    base_identity: serde_json::Value,
    freshness: RiprAgentFreshness,
    run_status: RiprAgentRunStatus,
    profile: serde_json::Value,
    budget_identity: serde_json::Value,
    selected_count: u64,
    omitted_count: u64,
    total_count: u64,
    complete_evidence_identity: serde_json::Value,
    continuation_identity: serde_json::Value,
    allowed_edit_surface: RiprAgentAllowedEditSurface,
    must_not_change: Vec<RiprAgentMustNotChange>,
    verify_route: serde_json::Value,
    receipt_route: serde_json::Value,
    // #1617 slice 4 (round-1 review): `PresenceTracked` keeps omission
    // distinct from an explicit `null`. A payload that declares schema 0.2
    // or later must carry all four keys explicitly; omission is tolerated
    // only for schema-0.1 payloads, whose readiness is then derived
    // truthfully from the route and spec the payload carries.
    #[serde(default)]
    verify_route_readiness: PresenceTracked,
    #[serde(default)]
    receipt_route_readiness: PresenceTracked,
    #[serde(default)]
    verify_command_spec: PresenceTracked,
    #[serde(default)]
    receipt_command_spec: PresenceTracked,
    limitations: Vec<String>,
    non_claims: Vec<String>,
}

/// A wire field tracked for presence: `None` means the key was omitted,
/// `Some(Value::Null)` an explicit `null`, `Some(value)` a present value.
///
/// A plain `Option<serde_json::Value>` cannot make this distinction: serde
/// routes Option fields through `deserialize_option`, which collapses an
/// explicit `null` into `None` exactly like an omitted key. Deserializing the
/// whole `serde_json::Value` instead keeps `null` observable.
#[derive(Debug, Default)]
struct PresenceTracked(Option<serde_json::Value>);

impl PresenceTracked {
    fn is_present(&self) -> bool {
        self.0.is_some()
    }

    fn into_inner(self) -> Option<serde_json::Value> {
        self.0
    }
}

impl<'de> Deserialize<'de> for PresenceTracked {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde_json::Value::deserialize(deserializer).map(|value| PresenceTracked(Some(value)))
    }
}

/// True when a payload declaring this schema version must carry the route
/// readiness and command-spec fields explicitly. Schema 0.1 predates the
/// fields (#1617 slice 4), so only it may omit them.
fn schema_version_requires_route_fields(version: &str) -> bool {
    match version.split_once('.') {
        Some(("0", minor)) => minor
            .parse::<u16>()
            .map(|minor| minor >= 2)
            .unwrap_or(false),
        _ => false,
    }
}

/// Reject a schema-0.2+ payload that omits one of the additive route fields.
fn require_route_field_presence(
    wire: &PresenceTracked,
    schema_version: &RiprAgentSchemaVersion,
    field: &'static str,
) -> Result<(), String> {
    if !wire.is_present() && schema_version_requires_route_fields(&schema_version.0) {
        return Err(format!(
            "schema {} response omits {field}",
            schema_version.0
        ));
    }
    Ok(())
}

/// Decode a readiness wire field into its declared state: omission is
/// `Absent`, an explicit `null` is `ExplicitNull`, and any other value must
/// decode as the closed readiness vocabulary.
fn decode_declared_readiness(
    wire: PresenceTracked,
    field: &'static str,
) -> Result<DeclaredReadiness, String> {
    match wire.into_inner() {
        None => Ok(DeclaredReadiness::Absent),
        Some(serde_json::Value::Null) => Ok(DeclaredReadiness::ExplicitNull),
        Some(value) => serde_json::from_value(value)
            .map(DeclaredReadiness::Value)
            .map_err(|error| format!("{field}: {error}")),
    }
}

/// The published CommandSpec property set
/// (`schemas/ripr/repair-assurance.schema.json` `$defs.command_spec`). The
/// riprAgent envelope accepts exactly these keys: unknown fields and the
/// serde legacy aliases (`cwd`, `environment_policy`, `network_policy`,
/// `display`) never enter the protocol, so the Rust decode cannot accept a
/// shape the published schema rejects.
const COMMAND_SPEC_WIRE_FIELDS: &[&str] = &[
    "schema_version",
    "command_id",
    "role",
    "execution_mode",
    "program",
    "args",
    "working_directory",
    "env_set",
    "env_passthrough",
    "environment",
    "stdin",
    "timeout_ms",
    "cancellation",
    "network",
    "expected_result_parser",
    "expected_exit_codes",
    "expected_writes",
    "cost_class",
    "platforms",
    "human_display",
    "authority_boundary",
];

/// Decode a command-spec wire field: omission and explicit `null` both mean
/// "no producer-owned spec", any other value must be an object whose keys are
/// exactly the published field set, and must decode as a [`CommandSpec`].
fn decode_command_spec(
    wire: PresenceTracked,
    field: &'static str,
) -> Result<Option<CommandSpec>, String> {
    match wire.into_inner() {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{field}: command spec must be an object"))?;
            for key in object.keys() {
                if !COMMAND_SPEC_WIRE_FIELDS.contains(&key.as_str()) {
                    return Err(format!(
                        "{field}: command spec field `{key}` is not part of the published schema"
                    ));
                }
            }
            serde_json::from_value(value)
                .map(Some)
                .map_err(|error| format!("{field}: {error}"))
        }
    }
}

impl TryFrom<RiprAgentSuccessEnvelopeWire> for RiprAgentSuccessEnvelope {
    type Error = String;

    fn try_from(value: RiprAgentSuccessEnvelopeWire) -> Result<Self, Self::Error> {
        let verify_route = require_nullable_nonempty_string(value.verify_route, "verify_route")?;
        let receipt_route = require_nullable_nonempty_string(value.receipt_route, "receipt_route")?;
        require_route_field_presence(
            &value.verify_route_readiness,
            &value.versions.schema_version,
            "verify_route_readiness",
        )?;
        require_route_field_presence(
            &value.receipt_route_readiness,
            &value.versions.schema_version,
            "receipt_route_readiness",
        )?;
        require_route_field_presence(
            &value.verify_command_spec,
            &value.versions.schema_version,
            "verify_command_spec",
        )?;
        require_route_field_presence(
            &value.receipt_command_spec,
            &value.versions.schema_version,
            "receipt_command_spec",
        )?;
        let declared_verify_readiness =
            decode_declared_readiness(value.verify_route_readiness, "verify_route_readiness")?;
        let declared_receipt_readiness =
            decode_declared_readiness(value.receipt_route_readiness, "receipt_route_readiness")?;
        let verify_command_spec =
            decode_command_spec(value.verify_command_spec, "verify_command_spec")?;
        let receipt_command_spec =
            decode_command_spec(value.receipt_command_spec, "receipt_command_spec")?;
        Ok(Self {
            versions: value.versions,
            request: value.request,
            kind: value.kind,
            status: value.status,
            snapshot_id: require_nullable_nonempty_string(value.snapshot_id, "snapshot_id")?,
            input_identity: require_nullable_nonempty_string(
                value.input_identity,
                "input_identity",
            )?,
            root_identity: require_nullable_nonempty_string(value.root_identity, "root_identity")?,
            config_identity: require_nullable_nonempty_string(
                value.config_identity,
                "config_identity",
            )?,
            base_identity: require_nullable_nonempty_string(value.base_identity, "base_identity")?,
            freshness: value.freshness,
            run_status: value.run_status,
            profile: require_nullable(value.profile, "profile")?,
            budget_identity: require_nullable_nonempty_string(
                value.budget_identity,
                "budget_identity",
            )?,
            selected_count: value.selected_count,
            omitted_count: value.omitted_count,
            total_count: value.total_count,
            complete_evidence_identity: require_nullable_nonempty_string(
                value.complete_evidence_identity,
                "complete_evidence_identity",
            )?,
            continuation_identity: require_nullable_nonempty_string(
                value.continuation_identity,
                "continuation_identity",
            )?,
            allowed_edit_surface: value.allowed_edit_surface,
            must_not_change: value.must_not_change,
            verify_route_readiness: resolve_route_readiness(
                &verify_route,
                declared_verify_readiness,
                verify_command_spec.as_ref(),
                CommandRole::Verify,
                "verify_route",
            )?,
            receipt_route_readiness: resolve_route_readiness(
                &receipt_route,
                declared_receipt_readiness,
                receipt_command_spec.as_ref(),
                CommandRole::Receipt,
                "receipt_route",
            )?,
            verify_command_spec: match verify_command_spec {
                Some(spec) => RiprAgentRequiredNullable::Value(spec),
                None => RiprAgentRequiredNullable::Null,
            },
            receipt_command_spec: match receipt_command_spec {
                Some(spec) => RiprAgentRequiredNullable::Value(spec),
                None => RiprAgentRequiredNullable::Null,
            },
            verify_route,
            receipt_route,
            limitations: value.limitations,
            non_claims: value.non_claims,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RiprAgentErrorWire")]
pub(super) struct RiprAgentError {
    pub(super) kind: RiprAgentErrorKind,
    retryable: bool,
    recovery_route: RiprAgentRequiredNullable<RiprAgentRecoveryRoute>,
    snapshot_id: RiprAgentRequiredNullable<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiprAgentErrorWire {
    kind: RiprAgentErrorKind,
    retryable: bool,
    recovery_route: serde_json::Value,
    snapshot_id: serde_json::Value,
}

impl TryFrom<RiprAgentErrorWire> for RiprAgentError {
    type Error = String;

    fn try_from(value: RiprAgentErrorWire) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: value.kind,
            retryable: value.retryable,
            recovery_route: require_nullable(value.recovery_route, "recovery_route")?,
            snapshot_id: require_nullable_nonempty_string(value.snapshot_id, "snapshot_id")?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RiprAgentErrorEnvelope {
    #[serde(flatten)]
    pub(super) versions: RiprAgentVersionIdentity,
    pub(super) request: RiprAgentRequest,
    status: RiprAgentErrorStatus,
    pub(super) error: RiprAgentError,
    pub(super) allowed_edit_surface: RiprAgentAllowedEditSurface,
    pub(super) must_not_change: Vec<RiprAgentMustNotChange>,
}

fn compatibility_commands() -> [&'static str; 7] {
    [
        REFRESH_COMMAND,
        COLLECT_CONTEXT_COMMAND,
        COLLECT_EVIDENCE_CONTEXT_COMMAND,
        COLLECT_WORKSPACE_STATUS_COMMAND,
        COLLECT_REPAIR_PACKET_COMMAND,
        COLLECT_TOP_LIMITATION_COMMAND,
        COLLECT_RECEIPT_STATUS_COMMAND,
    ]
}

fn reserved_dto_layout() -> usize {
    std::mem::size_of::<RiprAgentRequestEnvelope>()
        + std::mem::size_of::<RiprAgentSuccessEnvelope>()
        + std::mem::size_of::<RiprAgentErrorEnvelope>()
}

pub(super) fn server_capability() -> LSPAny {
    let _ = reserved_dto_layout();
    let capability = RiprAgentServerCapability {
        ripr_agent: RiprAgentCapability::v0_1_implemented(),
    };
    serde_json::to_value(capability).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandPlatform,
        EnvironmentPolicy, ExpectedResultParser, NetworkPolicy, StdinPolicy,
    };
    use std::collections::BTreeSet;
    use std::path::Path;

    fn require_unique<T>(values: &[T], label: &str) -> Result<(), String>
    where
        T: Ord,
    {
        let unique = values.iter().collect::<BTreeSet<_>>();
        if unique.len() != values.len() {
            return Err(format!("{label} must not contain duplicate values"));
        }
        Ok(())
    }

    fn capability_fixture() -> Result<RiprAgentCapability, String> {
        let capability = server_capability();
        let agent = capability
            .get("riprAgent")
            .cloned()
            .ok_or_else(|| "expected riprAgent capability fixture".to_string())?;
        serde_json::from_value(agent).map_err(|error| format!("decode capability fixture: {error}"))
    }

    fn read_only_boundaries() -> Vec<RiprAgentMustNotChange> {
        vec![
            RiprAgentMustNotChange::SourceEdits,
            RiprAgentMustNotChange::WorkspaceEdit,
            RiprAgentMustNotChange::AutonomousRepair,
        ]
    }

    fn success_fixture() -> &'static str {
        r#"
        {
          "protocol_version": "0.1",
          "schema_version": "0.2",
          "request": "ripr/listActionableItems",
          "kind": "actionable_items",
          "status": "ok",
          "snapshot_id": "snapshot:abc",
          "input_identity": "input:def",
          "root_identity": "root:ghi",
          "config_identity": "config:jkl",
          "base_identity": "base:mno",
          "freshness": "fresh",
          "run_status": "ready",
          "profile": "actionable",
          "budget_identity": "budget:pqr",
          "selected_count": 1,
          "omitted_count": 0,
          "total_count": 1,
          "complete_evidence_identity": "evidence:stu",
          "continuation_identity": null,
          "allowed_edit_surface": "read_only",
          "must_not_change": ["source_edits", "workspace_edit", "autonomous_repair"],
          "verify_route": "ripr/verify",
          "receipt_route": "ripr/receipt",
          "verify_route_readiness": "legacy_string_only",
          "receipt_route_readiness": "legacy_string_only",
          "verify_command_spec": null,
          "receipt_command_spec": null,
          "limitations": ["capability_only"],
          "non_claims": ["not_runtime_mutation_proof"]
        }
        "#
    }

    /// A payload as a schema-0.1 producer emitted it before #1617 slice 4:
    /// `schema_version: "0.1"` and none of the route-readiness or
    /// command-spec fields. This is the only shape allowed to omit them.
    fn schema_0_1_success_fixture() -> Result<String, String> {
        let absent_fields = [
            "          \"verify_route_readiness\": \"legacy_string_only\",\n",
            "          \"receipt_route_readiness\": \"legacy_string_only\",\n",
            "          \"verify_command_spec\": null,\n",
            "          \"receipt_command_spec\": null,\n",
        ];
        let mut payload = success_fixture().to_string();
        if !payload.contains("\"schema_version\": \"0.2\"") {
            return Err("test fixture did not declare schema version 0.2".to_string());
        }
        payload = payload.replace("\"schema_version\": \"0.2\"", "\"schema_version\": \"0.1\"");
        for absent in absent_fields {
            if !payload.contains(absent) {
                return Err(format!(
                    "test fixture did not contain the removable field line: {absent}"
                ));
            }
            payload = payload.replace(absent, "");
        }
        if payload.contains("schema_version\": \"0.2\"") || payload.contains("_readiness") {
            return Err("test fixture still carries 0.2 route fields".to_string());
        }
        Ok(payload)
    }

    /// A malformed payload: declares schema 0.2 but omits the named field
    /// line, the exact shape a 0.2 producer must never emit.
    fn schema_0_2_success_fixture_omitting(field_line: &str) -> Result<String, String> {
        let payload = success_fixture().to_string();
        if !payload.contains(field_line) {
            return Err(format!(
                "test fixture did not contain the removable field line: {field_line}"
            ));
        }
        Ok(payload.replace(field_line, ""))
    }

    fn verify_command_spec_fixture() -> CommandSpec {
        CommandSpec {
            schema_version: "1".to_string(),
            command_id: "cmd:verify:pricing".to_string(),
            role: CommandRole::Verify,
            execution_mode: CommandExecutionMode::Direct,
            program: "cargo".to_string(),
            args: vec!["test".to_string(), "-p".to_string(), "pricing".to_string()],
            cwd: ".".to_string(),
            env_set: Vec::new(),
            env_passthrough: Vec::new(),
            environment_policy: EnvironmentPolicy::Inherited,
            stdin: StdinPolicy::Null,
            timeout_ms: 120_000,
            cancellation: CancellationPolicy::Allowed,
            network_policy: NetworkPolicy::Forbidden,
            expected_result_parser: ExpectedResultParser::ExitCode,
            expected_exit_codes: vec![0],
            expected_writes: vec!["target/**".to_string()],
            cost_class: CommandCostClass::CompileOrTest,
            platforms: vec![CommandPlatform::Linux, CommandPlatform::Windows],
            display: "cargo test -p pricing".to_string(),
            authority_boundary: CommandAuthorityBoundary::VerificationRouteOnly,
        }
    }

    fn receipt_command_spec_fixture() -> CommandSpec {
        CommandSpec {
            command_id: "cmd:receipt:gap-42".to_string(),
            role: CommandRole::Receipt,
            execution_mode: CommandExecutionMode::ShellRequired,
            program: "ripr".to_string(),
            args: vec![
                "receipt".to_string(),
                "record".to_string(),
                "--gap".to_string(),
                "gap:42".to_string(),
            ],
            display: "ripr receipt record --gap gap:42".to_string(),
            expected_writes: vec!["target/ripr/receipts/gap-42.json".to_string()],
            cost_class: CommandCostClass::ProjectionOnly,
            authority_boundary: CommandAuthorityBoundary::ReceiptRouteOnly,
            ..verify_command_spec_fixture()
        }
    }

    fn success_envelope_with_routes(
        verify_route: RiprAgentRequiredNullable<String>,
        receipt_route: RiprAgentRequiredNullable<String>,
        verify_route_readiness: RiprAgentRequiredNullable<RiprAgentRouteReadiness>,
        receipt_route_readiness: RiprAgentRequiredNullable<RiprAgentRouteReadiness>,
        verify_command_spec: RiprAgentRequiredNullable<CommandSpec>,
        receipt_command_spec: RiprAgentRequiredNullable<CommandSpec>,
    ) -> RiprAgentSuccessEnvelope {
        RiprAgentSuccessEnvelope {
            versions: RiprAgentVersionIdentity::current(),
            request: RiprAgentRequest::ListActionableItems,
            kind: RiprAgentResponseKind::ActionableItems,
            status: RiprAgentSuccessStatus::Ok,
            snapshot_id: RiprAgentRequiredNullable::Value("snapshot:abc".to_string()),
            input_identity: RiprAgentRequiredNullable::Value("input:def".to_string()),
            root_identity: RiprAgentRequiredNullable::Value("root:ghi".to_string()),
            config_identity: RiprAgentRequiredNullable::Value("config:jkl".to_string()),
            base_identity: RiprAgentRequiredNullable::Value("base:mno".to_string()),
            freshness: RiprAgentFreshness::Fresh,
            run_status: RiprAgentRunStatus::Ready,
            profile: RiprAgentRequiredNullable::Value(RiprAgentProfile::Actionable),
            budget_identity: RiprAgentRequiredNullable::Value("budget:pqr".to_string()),
            selected_count: 1,
            omitted_count: 0,
            total_count: 1,
            complete_evidence_identity: RiprAgentRequiredNullable::Value(
                "evidence:stu".to_string(),
            ),
            continuation_identity: RiprAgentRequiredNullable::Null,
            allowed_edit_surface: RiprAgentAllowedEditSurface::ReadOnly,
            must_not_change: read_only_boundaries(),
            verify_route,
            receipt_route,
            verify_route_readiness,
            receipt_route_readiness,
            verify_command_spec,
            receipt_command_spec,
            limitations: vec!["capability_only".to_string()],
            non_claims: vec!["not_runtime_mutation_proof".to_string()],
        }
    }

    fn error_fixture() -> &'static str {
        r#"
        {
          "protocol_version": "0.1",
          "schema_version": "0.1",
          "request": "ripr/getRepairPacket",
          "status": "error",
          "error": {
            "kind": "stale_snapshot",
            "retryable": true,
            "recovery_route": "refresh",
            "snapshot_id": "snapshot:old"
          },
          "allowed_edit_surface": "read_only",
          "must_not_change": ["source_edits", "workspace_edit", "autonomous_repair"]
        }
        "#
    }

    /// Structurally validate one capability value against the published
    /// capability schema.
    ///
    /// No JSON-Schema validator crate is available in this crate's
    /// dependencies, so this pins the producer↔schema contract with
    /// structural assertions instead: object shape, `additionalProperties`,
    /// the required key set, and per-property `const`, `enum`, array-item
    /// `enum` (including sibling-file `$ref` resolution), `uniqueItems`, and
    /// scalar `type` constraints. Anything the structural pass cannot judge
    /// is a test failure, not a silent pass.
    fn assert_capability_matches_schema(value: &serde_json::Value) -> Result<(), String> {
        let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/ripr/ripr-agent-capability.schema.json");
        let schema_text = std::fs::read_to_string(&schema_path)
            .map_err(|error| format!("read {}: {error}", schema_path.display()))?;
        let schema: serde_json::Value = serde_json::from_str(&schema_text)
            .map_err(|error| format!("decode capability schema: {error}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "capability must be an object".to_string())?;
        if schema["type"] != serde_json::Value::String("object".to_string()) {
            return Err("capability schema must declare an object type".to_string());
        }
        if schema["additionalProperties"] != serde_json::Value::Bool(false) {
            return Err(
                "capability schema must stay closed (additionalProperties false)".to_string(),
            );
        }
        let properties = schema["properties"]
            .as_object()
            .ok_or_else(|| "capability schema must declare properties".to_string())?;
        let required = schema["required"]
            .as_array()
            .ok_or_else(|| "capability schema must declare required keys".to_string())?;
        for key in required {
            let name = key
                .as_str()
                .ok_or_else(|| "capability schema required entry must be a string".to_string())?;
            if !object.contains_key(name) {
                return Err(format!("capability output omits required key `{name}`"));
            }
        }
        for key in object.keys() {
            if !properties.contains_key(key) {
                return Err(format!("capability schema has no property `{key}`"));
            }
        }
        // Resolve `"<sibling>.schema.json#/$defs/<name>"` refs against the
        // schema directory so array-item enums stay checked.
        let resolve_ref = |reference: &str| -> Result<serde_json::Value, String> {
            let (file, pointer) = reference
                .split_once('#')
                .ok_or_else(|| format!("unsupported schema reference `{reference}`"))?;
            let referenced_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/ripr")
                .join(file);
            let text = std::fs::read_to_string(&referenced_path)
                .map_err(|error| format!("read {}: {error}", referenced_path.display()))?;
            let mut node: serde_json::Value =
                serde_json::from_str(&text).map_err(|error| format!("decode {file}: {error}"))?;
            for segment in pointer.split('/').skip(1) {
                node = node
                    .get(segment)
                    .ok_or_else(|| format!("schema reference `{reference}` does not resolve"))?
                    .clone();
            }
            Ok(node)
        };
        for (key, constraint) in properties {
            let actual = &object[key];
            if let Some(expected) = constraint.get("const")
                && actual != expected
            {
                return Err(format!("capability `{key}` drifted from the schema const"));
            }
            if let Some(expected) = constraint.get("enum").and_then(|value| value.as_array())
                && !expected.contains(actual)
            {
                return Err(format!("capability `{key}` is outside the schema enum"));
            }
            if let Some(expected_type) = constraint.get("type").and_then(|value| value.as_str()) {
                let type_matches = match expected_type {
                    "boolean" => actual.is_boolean(),
                    "string" => actual.is_string(),
                    "array" => actual.is_array(),
                    other => return Err(format!("unhandled schema type `{other}` for `{key}`")),
                };
                if !type_matches {
                    return Err(format!("capability `{key}` is not a `{expected_type}`"));
                }
            }
            if let Some(items) = constraint.get("items") {
                let members = actual
                    .as_array()
                    .ok_or_else(|| format!("capability `{key}` must be an array"))?;
                if constraint["uniqueItems"] == serde_json::Value::Bool(true) {
                    let repeats = members
                        .iter()
                        .enumerate()
                        .any(|(index, member)| members[index + 1..].contains(member));
                    if repeats {
                        return Err(format!("capability `{key}` must not repeat members"));
                    }
                }
                if let Some(allowed) = items.get("enum").and_then(|value| value.as_array()) {
                    for member in members {
                        if !allowed.contains(member) {
                            return Err(format!(
                                "capability `{key}` carries a member outside the schema enum"
                            ));
                        }
                    }
                }
                if let Some(reference) = items.get("$ref").and_then(|value| value.as_str()) {
                    let allowed = resolve_ref(reference)?;
                    for member in members {
                        if !allowed["enum"]
                            .as_array()
                            .is_some_and(|allowed| allowed.contains(member))
                        {
                            return Err(format!(
                                "capability `{key}` carries a member outside the referenced enum"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// FIX (round-3 review): COMMAND_SPEC_WIRE_FIELDS duplicates the
    /// published repair-assurance schema's command_spec field set. This test
    /// derives the serde field names from a fully-populated CommandSpec and
    /// pins parity, so a future domain field becomes a test failure here
    /// instead of a silent protocol-valid/schema-invalid drift.
    #[test]
    fn command_spec_wire_fields_match_the_serde_shape() -> Result<(), String> {
        let fully_populated = crate::domain::CommandSpec {
            schema_version: "1".to_string(),
            command_id: "ripr:agent:verify".to_string(),
            role: crate::domain::CommandRole::Verify,
            execution_mode: crate::domain::CommandExecutionMode::Direct,
            program: "ripr".to_string(),
            args: vec!["verify".to_string()],
            cwd: ".".to_string(),
            env_set: Vec::new(),
            env_passthrough: Vec::new(),
            environment_policy: crate::domain::EnvironmentPolicy::Clean,
            stdin: crate::domain::StdinPolicy::Null,
            timeout_ms: 120_000,
            cancellation: crate::domain::CancellationPolicy::Allowed,
            network_policy: crate::domain::NetworkPolicy::Forbidden,
            expected_result_parser: crate::domain::ExpectedResultParser::DeclaredJson,
            expected_exit_codes: vec![0],
            expected_writes: vec!["target/ripr/out.json".to_string()],
            cost_class: crate::domain::CommandCostClass::Unknown,
            platforms: vec![
                crate::domain::CommandPlatform::Linux,
                crate::domain::CommandPlatform::Macos,
                crate::domain::CommandPlatform::Windows,
            ],
            display: "display".to_string(),
            authority_boundary: crate::domain::CommandAuthorityBoundary::VerificationRouteOnly,
        };
        let serialized = serde_json::to_value(&fully_populated)
            .map_err(|error| format!("serialize CommandSpec: {error}"))?;
        let mut serde_fields = serialized
            .as_object()
            .ok_or("serialized CommandSpec is not an object")?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        serde_fields.sort();
        let mut wire_fields = super::COMMAND_SPEC_WIRE_FIELDS.to_vec();
        wire_fields.sort_unstable();
        if serde_fields != wire_fields {
            return Err(format!(
                "COMMAND_SPEC_WIRE_FIELDS drifted from the serde shape: serde {serde_fields:?} vs wire {wire_fields:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn capability_output_matches_the_published_schema() -> Result<(), String> {
        // The live producer bytes, exactly as initialize projects them.
        let capability = server_capability();
        let agent = capability
            .get("riprAgent")
            .cloned()
            .ok_or_else(|| "expected riprAgent capability fixture".to_string())?;
        assert_capability_matches_schema(&agent)?;
        // The committed example must describe the same shape.
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/lsp_agent_protocol/capability.json");
        let fixture_text = std::fs::read_to_string(&fixture_path)
            .map_err(|error| format!("read {}: {error}", fixture_path.display()))?;
        let fixture: serde_json::Value = serde_json::from_str(&fixture_text)
            .map_err(|error| format!("decode capability fixture: {error}"))?;
        assert_capability_matches_schema(&fixture)?;
        if fixture != agent {
            return Err(
                "the capability fixture drifted from the live server_capability() output"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn reserved_protocol_vocabularies_are_closed_and_unique() -> Result<(), String> {
        require_unique(RESERVED_REQUESTS, "reserved requests")?;
        require_unique(RESERVED_ERROR_KINDS, "reserved errors")?;
        require_unique(RESERVED_PROFILES, "reserved profiles")?;

        let capability = capability_fixture()?;
        if capability.reserved_requests != RESERVED_REQUESTS {
            return Err("capability request vocabulary drifted".to_string());
        }
        if capability.reserved_profiles != RESERVED_PROFILES {
            return Err("capability profile vocabulary drifted".to_string());
        }
        if capability.error_kinds != RESERVED_ERROR_KINDS {
            return Err("capability error vocabulary drifted".to_string());
        }

        for (label, unknown) in [
            ("request", r#""ripr/unknown""#),
            ("profile", r#""unknown""#),
            ("error", r#""unknown""#),
        ] {
            let result = match label {
                "request" => serde_json::from_str::<RiprAgentRequest>(unknown).map(|_| ()),
                "profile" => serde_json::from_str::<RiprAgentProfile>(unknown).map(|_| ()),
                "error" => serde_json::from_str::<RiprAgentErrorKind>(unknown).map(|_| ()),
                _ => Ok(()),
            };
            if result.is_ok() {
                return Err(format!(
                    "closed {label} vocabulary accepted an unknown value"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn capability_advertises_only_implemented_handlers() -> Result<(), String> {
        let capability = capability_fixture()?;
        // #1603: listActionableItems is the only implemented handler, so the
        // advertised surface must equal it exactly — direct vector equality,
        // not containment plus a loop over the same vector.
        if capability.supported_requests != vec![RiprAgentRequest::ListActionableItems] {
            return Err(format!(
                "supported_requests drifted from the implemented handler: {:?}",
                capability.supported_requests
            ));
        }
        if capability.supported_profiles != vec![RiprAgentProfile::Actionable] {
            return Err(format!(
                "supported_profiles drifted from the implemented profile: {:?}",
                capability.supported_profiles
            ));
        }
        // Lock the rest of the v0_1_implemented contract: the interim
        // generation identity and cancellation are advertised, every other
        // surface stays fail-closed, and the claim boundary names exactly
        // what is implemented.
        if capability.implementation_state != RiprAgentImplementationState::Implemented {
            return Err("implementation_state must be `implemented`".to_string());
        }
        if !capability.snapshot_handles {
            return Err(
                "snapshot_handles must advertise the interim generation identity".to_string(),
            );
        }
        if !capability.cancellation {
            return Err("cancellation must be advertised".to_string());
        }
        if capability.continuations {
            return Err("continuations must remain fail-closed".to_string());
        }
        if capability.work_done_progress {
            return Err("work_done_progress must remain fail-closed".to_string());
        }
        if capability.diagnostic_modes != vec![RiprAgentDiagnosticMode::Push] {
            return Err("diagnostic_modes drifted from push-only".to_string());
        }
        if capability.claim_boundary
            != "ripr/listActionableItems is implemented; all other riprAgent requests remain reserved."
        {
            return Err(format!(
                "claim_boundary drifted: {}",
                capability.claim_boundary
            ));
        }
        // The capability must remain read-only.
        if capability.source_edit_capability != RiprAgentSourceEditCapability::None {
            return Err("the capability must remain read-only".to_string());
        }
        Ok(())
    }

    #[test]
    fn version_identities_are_explicit_and_independent() -> Result<(), String> {
        let capability = capability_fixture()?;
        if capability.versions.protocol_version.0 != RIPR_AGENT_PROTOCOL_VERSION {
            return Err("protocol version identity must be explicit".to_string());
        }
        if capability.versions.schema_version.0 != RIPR_AGENT_SCHEMA_VERSION {
            return Err("schema version identity must be explicit".to_string());
        }
        let encoded = serde_json::to_value(&capability)
            .map_err(|error| format!("encode capability fixture: {error}"))?;
        for field in ["protocol_version", "schema_version"] {
            if encoded
                .get(field)
                .and_then(serde_json::Value::as_str)
                .is_none()
            {
                return Err(format!(
                    "capability is missing independent `{field}` identity"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn unsupported_protocol_major_is_rejected() -> Result<(), String> {
        match RiprAgentProtocolVersion::parse("1.0") {
            Err(RiprAgentProtocolVersionError::UnsupportedMajor {
                received,
                supported,
            }) if received == 1 && supported == RIPR_AGENT_SUPPORTED_PROTOCOL_MAJOR => {}
            Err(error) => return Err(format!("unexpected protocol-version error: {error}")),
            Ok(_) => return Err("unsupported protocol major was accepted".to_string()),
        }
        if serde_json::from_str::<RiprAgentProtocolVersion>(r#""1.0""#).is_ok() {
            return Err("serde accepted an unsupported protocol major".to_string());
        }
        Ok(())
    }

    #[test]
    fn unsupported_schema_major_is_rejected() -> Result<(), String> {
        match RiprAgentSchemaVersion::parse("1.0") {
            Err(RiprAgentSchemaVersionError::UnsupportedMajor {
                received,
                supported,
            }) if received == 1 && supported == RIPR_AGENT_SUPPORTED_SCHEMA_MAJOR => {}
            Err(error) => return Err(format!("unexpected schema-version error: {error}")),
            Ok(_) => return Err("unsupported schema major was accepted".to_string()),
        }
        if serde_json::from_str::<RiprAgentSchemaVersion>(r#""1.0""#).is_ok() {
            return Err("serde accepted an unsupported schema major".to_string());
        }
        Ok(())
    }

    #[test]
    fn schema_minor_bump_stays_additive_within_major_zero() -> Result<(), String> {
        // #1617 slice 4: the schema minor moved 0.1 -> 0.2 for the additive
        // route fields; the major stayed 0, so both minors keep parsing and
        // the protocol version is untouched.
        if RIPR_AGENT_SCHEMA_VERSION != "0.2" {
            return Err("schema version must be 0.2 after the additive bump".to_string());
        }
        if RIPR_AGENT_PROTOCOL_VERSION != "0.1" {
            return Err("an additive DTO change must not move the protocol version".to_string());
        }
        for minor in ["0.1", "0.2"] {
            match RiprAgentSchemaVersion::parse(minor) {
                Ok(parsed) if parsed.0 == minor => {}
                Ok(parsed) => return Err(format!("parsed schema version drifted: {parsed:?}")),
                Err(error) => {
                    return Err(format!(
                        "minor schema version `{minor}` was rejected: {error}"
                    ));
                }
            }
            if serde_json::from_str::<RiprAgentSchemaVersion>(&format!("\"{minor}\"")).is_err() {
                return Err(format!("serde rejected minor schema version `{minor}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn route_readiness_vocabulary_is_closed() -> Result<(), String> {
        for (wire, expected) in [
            ("typed_direct", RiprAgentRouteReadiness::TypedDirect),
            (
                "typed_shell_required",
                RiprAgentRouteReadiness::TypedShellRequired,
            ),
            ("manual", RiprAgentRouteReadiness::Manual),
            (
                "legacy_string_only",
                RiprAgentRouteReadiness::LegacyStringOnly,
            ),
        ] {
            let decoded: RiprAgentRouteReadiness = serde_json::from_str(&format!("\"{wire}\""))
                .map_err(|error| format!("decode readiness `{wire}`: {error}"))?;
            if decoded != expected {
                return Err(format!("readiness `{wire}` decoded to a different variant"));
            }
            let encoded = serde_json::to_value(expected)
                .map_err(|error| format!("encode readiness `{wire}`: {error}"))?;
            if encoded != serde_json::Value::String(wire.to_string()) {
                return Err(format!("readiness variant lost its wire name `{wire}`"));
            }
        }
        match serde_json::from_str::<RiprAgentRouteReadiness>(r#""unknown""#) {
            Ok(_) => return Err("the readiness vocabulary accepted an unknown value".to_string()),
            Err(error) => {
                let message = error.to_string();
                if !message.contains("unknown variant `unknown`") {
                    return Err(format!("unexpected readiness error message: {message}"));
                }
            }
        }
        let unknown_readiness =
            success_fixture().replace("legacy_string_only", "unknown_readiness");
        match serde_json::from_str::<RiprAgentSuccessEnvelope>(&unknown_readiness) {
            Ok(_) => {
                return Err("a success envelope accepted an unknown readiness value".to_string());
            }
            Err(error) => {
                let message = error.to_string();
                if !message.contains("unknown variant `unknown_readiness`") {
                    return Err(format!("unexpected envelope error message: {message}"));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn typed_command_specs_carry_typed_readiness() -> Result<(), String> {
        let envelope = success_envelope_with_routes(
            RiprAgentRequiredNullable::Value("cargo test -p pricing".to_string()),
            RiprAgentRequiredNullable::Value("ripr receipt record --gap gap:42".to_string()),
            RiprAgentRequiredNullable::Value(RiprAgentRouteReadiness::TypedDirect),
            RiprAgentRequiredNullable::Value(RiprAgentRouteReadiness::TypedShellRequired),
            RiprAgentRequiredNullable::Value(verify_command_spec_fixture()),
            RiprAgentRequiredNullable::Value(receipt_command_spec_fixture()),
        );
        let encoded = serde_json::to_value(&envelope)
            .map_err(|error| format!("encode typed-spec envelope: {error}"))?;
        for (field, expected) in [
            ("verify_route_readiness", "typed_direct"),
            ("receipt_route_readiness", "typed_shell_required"),
        ] {
            let actual = encoded
                .get(field)
                .ok_or_else(|| format!("envelope omitted `{field}`"))?;
            if actual != &serde_json::Value::String(expected.to_string()) {
                return Err(format!("envelope readiness `{field}` drifted to {actual}"));
            }
        }
        for (field, role, mode) in [
            ("verify_command_spec", "verify", "direct"),
            ("receipt_command_spec", "receipt", "shell_required"),
        ] {
            let spec = encoded
                .get(field)
                .ok_or_else(|| format!("envelope omitted `{field}`"))?;
            for (member, expected) in [("role", role), ("execution_mode", mode)] {
                if spec.get(member) != Some(&serde_json::Value::String(expected.to_string())) {
                    return Err(format!("`{field}` lost its typed `{member}`"));
                }
            }
            if spec
                .get("human_display")
                .and_then(|value| value.as_str())
                .is_none()
            {
                return Err(format!("`{field}` omitted the human display string"));
            }
        }
        let decoded: RiprAgentSuccessEnvelope = serde_json::from_value(encoded)
            .map_err(|error| format!("re-decode typed-spec envelope: {error}"))?;
        if decoded != envelope {
            return Err("the typed-spec envelope did not round-trip".to_string());
        }
        Ok(())
    }

    #[test]
    fn legacy_route_strings_stay_legacy_string_only() -> Result<(), String> {
        let envelope = success_envelope_with_routes(
            RiprAgentRequiredNullable::Value("ripr/verify".to_string()),
            RiprAgentRequiredNullable::Value("ripr/receipt".to_string()),
            RiprAgentRequiredNullable::Value(RiprAgentRouteReadiness::LegacyStringOnly),
            RiprAgentRequiredNullable::Value(RiprAgentRouteReadiness::LegacyStringOnly),
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
        );
        let encoded = serde_json::to_value(&envelope)
            .map_err(|error| format!("encode legacy envelope: {error}"))?;
        for field in [
            "verify_route_readiness",
            "receipt_route_readiness",
            "verify_command_spec",
            "receipt_command_spec",
        ] {
            let actual = encoded
                .get(field)
                .ok_or_else(|| format!("envelope omitted `{field}`"))?;
            let expected = if field.ends_with("readiness") {
                serde_json::Value::String("legacy_string_only".to_string())
            } else {
                serde_json::Value::Null
            };
            if actual != &expected {
                return Err(format!("envelope field `{field}` drifted to {actual}"));
            }
        }
        Ok(())
    }

    #[test]
    fn null_route_carries_null_readiness_and_null_spec() -> Result<(), String> {
        let envelope = success_envelope_with_routes(
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
            RiprAgentRequiredNullable::Null,
        );
        let encoded = serde_json::to_value(&envelope)
            .map_err(|error| format!("encode null-route envelope: {error}"))?;
        for field in [
            "verify_route",
            "receipt_route",
            "verify_route_readiness",
            "receipt_route_readiness",
            "verify_command_spec",
            "receipt_command_spec",
        ] {
            if encoded.get(field) != Some(&serde_json::Value::Null) {
                return Err(format!("null-route envelope emitted a non-null `{field}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn schema_0_1_payload_without_route_fields_still_decodes() -> Result<(), String> {
        let legacy_payload = schema_0_1_success_fixture()?;
        // The tolerance path is only for a true 0.1 payload: it declares
        // schema 0.1 and carries none of the 0.2 route fields.
        if !legacy_payload.contains("\"schema_version\": \"0.1\"") {
            return Err("compat fixture must declare schema version 0.1".to_string());
        }
        let decoded: RiprAgentSuccessEnvelope = serde_json::from_str(&legacy_payload)
            .map_err(|error| format!("decode 0.1-shaped payload: {error}"))?;
        for field in ["verify_route", "receipt_route"] {
            if !matches!(
                decoded_route_field(&decoded, field),
                RiprAgentRequiredNullable::Value(_)
            ) {
                return Err(format!("0.1 payload lost its `{field}` string"));
            }
        }
        for (field, expected) in [
            (
                "verify_route_readiness",
                RiprAgentRouteReadiness::LegacyStringOnly,
            ),
            (
                "receipt_route_readiness",
                RiprAgentRouteReadiness::LegacyStringOnly,
            ),
        ] {
            let readiness = decoded_readiness_field(&decoded, field);
            if *readiness != RiprAgentRequiredNullable::Value(expected) {
                return Err(format!(
                    "0.1 payload readiness `{field}` did not derive to legacy_string_only"
                ));
            }
        }
        for field in ["verify_command_spec", "receipt_command_spec"] {
            if decoded_spec_field(&decoded, field) != &RiprAgentRequiredNullable::Null {
                return Err(format!("0.1 payload derived a spec for `{field}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn schema_0_2_payload_omitting_route_field_is_rejected() -> Result<(), String> {
        // A payload that declares schema 0.2 must carry every additive route
        // field explicitly; omission decodes nowhere.
        for field_line in [
            "          \"verify_route_readiness\": \"legacy_string_only\",\n",
            "          \"receipt_route_readiness\": \"legacy_string_only\",\n",
            "          \"verify_command_spec\": null,\n",
            "          \"receipt_command_spec\": null,\n",
        ] {
            let malformed = schema_0_2_success_fixture_omitting(field_line)?;
            let field_name = field_line
                .trim()
                .split('"')
                .nth(1)
                .ok_or_else(|| format!("could not read field name from {field_line}"))?;
            match serde_json::from_str::<RiprAgentSuccessEnvelope>(&malformed) {
                Ok(_) => {
                    return Err(format!(
                        "schema 0.2 response omitted `{field_name}` and still decoded"
                    ));
                }
                Err(error) => {
                    let message = error.to_string();
                    if !(message.contains("omits") && message.contains(field_name)) {
                        return Err(format!(
                            "omission of `{field_name}` produced an unexpected error: {message}"
                        ));
                    }
                }
            }
        }
        // Explicit nulls stay distinct from omission: null readiness next to
        // a non-null route is a contract violation even on a fully-present
        // 0.2 payload.
        let null_readiness = success_fixture().replace(
            "          \"verify_route_readiness\": \"legacy_string_only\",\n",
            "          \"verify_route_readiness\": null,\n",
        );
        if null_readiness == success_fixture() {
            return Err("test fixture did not null the verify readiness".to_string());
        }
        match serde_json::from_str::<RiprAgentSuccessEnvelope>(&null_readiness) {
            Ok(_) => return Err("a null readiness beside a route decoded".to_string()),
            Err(error) => {
                let message = error.to_string();
                if !message.contains(
                    "verify_route: declared readiness does not match the route and command spec",
                ) {
                    return Err(format!("unexpected null-readiness error: {message}"));
                }
            }
        }
        Ok(())
    }

    fn decoded_route_field<'a>(
        envelope: &'a RiprAgentSuccessEnvelope,
        field: &str,
    ) -> &'a RiprAgentRequiredNullable<String> {
        match field {
            "verify_route" => &envelope.verify_route,
            _ => &envelope.receipt_route,
        }
    }

    fn decoded_readiness_field<'a>(
        envelope: &'a RiprAgentSuccessEnvelope,
        field: &str,
    ) -> &'a RiprAgentRequiredNullable<RiprAgentRouteReadiness> {
        match field {
            "verify_route_readiness" => &envelope.verify_route_readiness,
            _ => &envelope.receipt_route_readiness,
        }
    }

    fn decoded_spec_field<'a>(
        envelope: &'a RiprAgentSuccessEnvelope,
        field: &str,
    ) -> &'a RiprAgentRequiredNullable<CommandSpec> {
        match field {
            "verify_command_spec" => &envelope.verify_command_spec,
            _ => &envelope.receipt_command_spec,
        }
    }

    /// Assert one malformed envelope fails with the named slot error.
    fn assert_decode_rejected(payload: &str, expected_substring: &str) -> Result<(), String> {
        match serde_json::from_str::<RiprAgentSuccessEnvelope>(payload) {
            Ok(_) => Err(format!(
                "malformed envelope decoded although it should have failed with `{expected_substring}`"
            )),
            Err(error) => {
                let message = error.to_string();
                if !message.contains(expected_substring) {
                    return Err(format!(
                        "unexpected error message: {message} (expected `{expected_substring}`)"
                    ));
                }
                Ok(())
            }
        }
    }

    #[test]
    fn readiness_must_agree_with_route_and_spec() -> Result<(), String> {
        // A typed readiness without a spec claims a producer-owned route the
        // envelope does not carry.
        let typed_without_spec = success_fixture().replace("legacy_string_only", "typed_direct");
        assert_decode_rejected(
            &typed_without_spec,
            "verify_route: declared readiness does not match the route and command spec",
        )?;
        // A spec under a legacy-string-only readiness contradicts itself.
        let spec = serde_json::to_string(&verify_command_spec_fixture())
            .map_err(|error| error.to_string())?;
        let legacy_with_spec = success_fixture().replace(
            "          \"verify_command_spec\": null,\n",
            &format!("          \"verify_command_spec\": {spec},\n"),
        );
        if legacy_with_spec == success_fixture() {
            return Err("test fixture did not inject the command spec".to_string());
        }
        assert_decode_rejected(
            &legacy_with_spec,
            "verify_route: declared readiness does not match the route and command spec",
        )?;
        // Readiness follows the nullable route: a declared readiness next to
        // a null route is rejected.
        let readiness_with_null_route = success_fixture().replace(
            "\"verify_route\": \"ripr/verify\"",
            "\"verify_route\": null",
        );
        if readiness_with_null_route == success_fixture() {
            return Err("test fixture did not null the verify route".to_string());
        }
        assert_decode_rejected(
            &readiness_with_null_route,
            "verify_route: declared readiness does not match the route and command spec",
        )?;
        // An explicit null readiness beside a non-null route is a committed
        // claim that contradicts the route.
        let null_readiness_with_route = success_fixture().replace(
            "          \"verify_route_readiness\": \"legacy_string_only\",\n",
            "          \"verify_route_readiness\": null,\n",
        );
        assert_decode_rejected(
            &null_readiness_with_route,
            "verify_route: declared readiness does not match the route and command spec",
        )?;
        // A spec whose role does not match the slot is rejected.
        let mismatched_role = success_fixture().replace(
            "          \"verify_command_spec\": null,\n",
            &format!(
                "          \"verify_command_spec\": {},\n",
                serde_json::to_string(&receipt_command_spec_fixture())
                    .map_err(|error| error.to_string())?
            ),
        );
        assert_decode_rejected(
            &mismatched_role,
            "verify_route: command spec role does not match the slot role",
        )?;
        // A command spec next to a null route is rejected.
        let spec_with_null_route = legacy_with_spec.replace(
            "\"verify_route\": \"ripr/verify\"",
            "\"verify_route\": null",
        );
        if spec_with_null_route == legacy_with_spec {
            return Err("test fixture did not null the verify route".to_string());
        }
        assert_decode_rejected(
            &spec_with_null_route,
            "verify_route: a command spec requires a non-null route",
        )?;
        Ok(())
    }

    #[test]
    fn command_spec_wire_shape_matches_the_domain_type() -> Result<(), String> {
        // The published success schema reuses the repair-assurance
        // CommandSpec definition; this pins the serde wire names it encodes
        // (including the working_directory/environment/network/human_display
        // renames) so the schema cannot drift from the domain type silently.
        let encoded = serde_json::to_value(verify_command_spec_fixture())
            .map_err(|error| format!("encode command spec: {error}"))?;
        for (field, expected) in [
            ("schema_version", "1"),
            ("role", "verify"),
            ("execution_mode", "direct"),
            ("working_directory", "."),
            ("environment", "inherited"),
            ("network", "forbidden"),
            ("stdin", "null"),
            ("cancellation", "allowed"),
            ("expected_result_parser", "exit_code"),
            ("cost_class", "compile_or_test"),
            ("human_display", "cargo test -p pricing"),
            ("authority_boundary", "verification_route_only"),
        ] {
            if encoded.get(field) != Some(&serde_json::Value::String(expected.to_string())) {
                return Err(format!(
                    "command spec wire name for `{field}` drifted from `{expected}`"
                ));
            }
        }
        if encoded
            .get("platforms")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|platforms| platforms.len() != 2)
        {
            return Err("command spec lost its platforms list".to_string());
        }
        if encoded
            .get("expected_exit_codes")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|codes| codes.is_empty())
        {
            return Err("command spec lost its expected exit codes".to_string());
        }
        Ok(())
    }

    #[test]
    fn command_spec_decoding_is_closed_to_the_published_shape() -> Result<(), String> {
        // The serde legacy alias must not enter the protocol envelope: the
        // published schema has no `cwd` property, so the Rust decode rejects
        // it instead of silently accepting a shape the schema rejects.
        let mut alias_spec = serde_json::to_value(verify_command_spec_fixture())
            .map_err(|error| error.to_string())?;
        let alias_object = alias_spec
            .as_object_mut()
            .ok_or_else(|| "spec fixture must be an object".to_string())?;
        alias_object.remove("working_directory");
        alias_object.insert(
            "cwd".to_string(),
            serde_json::Value::String(".".to_string()),
        );
        let alias_payload = success_fixture().replace(
            "          \"verify_command_spec\": null,\n",
            &format!("          \"verify_command_spec\": {alias_spec},\n"),
        );
        assert_decode_rejected(
            &alias_payload,
            "verify_command_spec: command spec field `cwd` is not part of the published schema",
        )?;
        // Unknown fields are equally rejected.
        let mut unknown_spec = serde_json::to_value(verify_command_spec_fixture())
            .map_err(|error| error.to_string())?;
        unknown_spec
            .as_object_mut()
            .ok_or_else(|| "spec fixture must be an object".to_string())?
            .insert("extra".to_string(), serde_json::Value::Bool(true));
        let unknown_payload = success_fixture().replace(
            "          \"verify_command_spec\": null,\n",
            &format!("          \"verify_command_spec\": {unknown_spec},\n"),
        );
        assert_decode_rejected(
            &unknown_payload,
            "verify_command_spec: command spec field `extra` is not part of the published schema",
        )?;
        Ok(())
    }

    #[test]
    fn nullable_protocol_fields_must_be_explicit() -> Result<(), String> {
        let missing_request_field = r#"
        {
          "protocol_version": "0.1",
          "schema_version": "0.1",
          "request": "ripr/listActionableItems",
          "mode": "read_only",
          "profile": null,
          "continuation_id": null
        }
        "#;
        if let Ok(decoded) = serde_json::from_str::<RiprAgentRequestEnvelope>(missing_request_field)
        {
            return Err(format!(
                "request accepted an omitted nullable snapshot_id: {decoded:?}"
            ));
        }

        let missing_success_field =
            success_fixture().replace("          \"snapshot_id\": \"snapshot:abc\",\n", "");
        if serde_json::from_str::<RiprAgentSuccessEnvelope>(&missing_success_field).is_ok() {
            return Err("success accepted an omitted nullable snapshot_id".to_string());
        }

        let missing_error_field = error_fixture().replace(
            "            \"recovery_route\": \"refresh\",\n            \"snapshot_id\": \"snapshot:old\"\n",
            "            \"recovery_route\": \"refresh\"\n",
        );
        if serde_json::from_str::<RiprAgentErrorEnvelope>(&missing_error_field).is_ok() {
            return Err("error accepted an omitted nullable snapshot_id".to_string());
        }
        Ok(())
    }

    #[test]
    fn nullable_identity_strings_reject_empty_values() -> Result<(), String> {
        let request = RiprAgentRequestEnvelope {
            versions: RiprAgentVersionIdentity::current(),
            request: RiprAgentRequest::ListActionableItems,
            mode: RiprAgentRequestMode::ReadOnly,
            profile: RiprAgentRequiredNullable::Value(RiprAgentProfile::Actionable),
            snapshot_id: RiprAgentRequiredNullable::Value("snapshot:6".to_string()),
            continuation_id: RiprAgentRequiredNullable::Null,
        };
        let base = serde_json::to_string_pretty(&request).map_err(|error| error.to_string())?;
        let empty_snapshot = base.replace("\"snapshot:6\"", "\"\"");
        if empty_snapshot == base {
            return Err("test fixture did not inject the empty identity".to_string());
        }
        if serde_json::from_str::<RiprAgentRequestEnvelope>(&empty_snapshot).is_ok() {
            return Err(
                "request envelope accepted an empty identity string the schema rejects".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn envelopes_reject_unknown_fields_through_flattened_identity() -> Result<(), String> {
        let request = RiprAgentRequestEnvelope {
            versions: RiprAgentVersionIdentity::current(),
            request: RiprAgentRequest::ListActionableItems,
            mode: RiprAgentRequestMode::ReadOnly,
            profile: RiprAgentRequiredNullable::Value(RiprAgentProfile::Actionable),
            snapshot_id: RiprAgentRequiredNullable::Value("snapshot:6".to_string()),
            continuation_id: RiprAgentRequiredNullable::Null,
        };
        let base = serde_json::to_string_pretty(&request).map_err(|error| error.to_string())?;
        if serde_json::from_str::<RiprAgentRequestEnvelope>(&base).is_err() {
            return Err("valid request envelope was rejected".to_string());
        }
        let with_unknown = base.replace(
            "\"continuation_id\": null",
            "\"continuation_id\": null,\n  \"unexpected_field\": true",
        );
        if with_unknown == base {
            return Err("test fixture did not inject the unknown field".to_string());
        }
        if serde_json::from_str::<RiprAgentRequestEnvelope>(&with_unknown).is_ok() {
            return Err(
                "request envelope accepted an unknown field through the flattened identity"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn request_envelope_keeps_mode_and_profile_typed() -> Result<(), String> {
        let request = RiprAgentRequestEnvelope {
            versions: RiprAgentVersionIdentity::current(),
            request: RiprAgentRequest::ListActionableItems,
            mode: RiprAgentRequestMode::ReadOnly,
            profile: RiprAgentRequiredNullable::Value(RiprAgentProfile::Actionable),
            snapshot_id: RiprAgentRequiredNullable::Value("snapshot:6".to_string()),
            continuation_id: RiprAgentRequiredNullable::Null,
        };
        let encoded = serde_json::to_value(request).map_err(|error| error.to_string())?;
        for (field, expected) in [
            ("request", "ripr/listActionableItems"),
            ("mode", "read_only"),
            ("profile", "actionable"),
        ] {
            let actual = encoded
                .get(field)
                .ok_or_else(|| format!("request envelope omitted `{field}`"))?;
            if actual != &serde_json::Value::String(expected.to_string()) {
                return Err(format!("request envelope lost typed `{field}` wire field"));
            }
        }
        Ok(())
    }

    #[test]
    fn success_and_error_envelopes_are_typed_and_bounded() -> Result<(), String> {
        let success = serde_json::from_str::<RiprAgentSuccessEnvelope>(success_fixture())
            .map_err(|error| format!("decode success fixture: {error}"))?;
        if success.allowed_edit_surface != RiprAgentAllowedEditSurface::ReadOnly {
            return Err("success envelope must be read-only".to_string());
        }
        if success.must_not_change != read_only_boundaries() {
            return Err("success envelope read-only boundary drifted".to_string());
        }

        let error = serde_json::from_str::<RiprAgentErrorEnvelope>(error_fixture())
            .map_err(|error| format!("decode error fixture: {error}"))?;
        if error.error.kind != RiprAgentErrorKind::StaleSnapshot {
            return Err("error envelope kind did not remain typed".to_string());
        }
        if error.allowed_edit_surface != RiprAgentAllowedEditSurface::ReadOnly {
            return Err("error envelope must be read-only".to_string());
        }

        let bounded = format!("{}\n", success_fixture().trim());
        let unknown_field = bounded.replace(
            "\"status\": \"ok\"",
            "\"status\": \"ok\", \"unbounded\": true",
        );
        if serde_json::from_str::<RiprAgentSuccessEnvelope>(&unknown_field).is_ok() {
            return Err("success envelope accepted an unknown field".to_string());
        }
        Ok(())
    }

    #[test]
    fn source_edit_boundary_is_explicitly_read_only() -> Result<(), String> {
        let capability = capability_fixture()?;
        if capability.source_edit_capability != RiprAgentSourceEditCapability::None {
            return Err("capability advertised a source-edit surface".to_string());
        }
        let success = serde_json::from_str::<RiprAgentSuccessEnvelope>(success_fixture())
            .map_err(|error| format!("decode success fixture: {error}"))?;
        if success.allowed_edit_surface != RiprAgentAllowedEditSurface::ReadOnly {
            return Err("success envelope advertised a source-edit surface".to_string());
        }
        if !success
            .must_not_change
            .contains(&RiprAgentMustNotChange::SourceEdits)
        {
            return Err("success envelope must forbid source edits".to_string());
        }
        Ok(())
    }
}
