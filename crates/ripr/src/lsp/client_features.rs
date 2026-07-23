//! Typed client feature profile (#1987, RIPR-SPEC-0143): the single
//! initialize-capability authority for the LSP session.
//!
//! `ClientFeatureProfile::from_initialize_params` parses the client's
//! advertised capabilities exactly once at `initialize`; downstream code
//! consumes the typed profile (or session state populated from it), never
//! raw `InitializeParams` capability trees and never client-name checks.
//!
//! Rules (normative, from the issue):
//!
//! - Capability absence never implies support: every flag defaults to
//!   `false` / empty unless the client explicitly advertised it.
//! - Unknown or malformed optional experimental fields fail closed to
//!   "unsupported" (the `ripr_editor` / `ripr_agent` blocks become `None`)
//!   without breaking the standard client session.
//! - The profile is immutable for the session. Profile equality is the
//!   semantic capability identity: client name, PID, timing, initialization
//!   options, and unrelated fields are not captured, so two initialize
//!   handshakes compare equal exactly when the selected behavior is equal.
//! - A client capability may weaken projection (which optional fields the
//!   server emits); it never changes producer actionability, canonical
//!   identities, or complete evidence.

use super::agent_protocol::{RiprAgentProfile, RiprAgentProtocolVersion};
use super::capabilities::ConfigurationMode;
use tower_lsp_server::ls_types::{InitializeParams, MarkupKind, PositionEncodingKind};

/// Bound on any single string captured from the experimental capability
/// blocks, mirroring the ingress-bounds discipline for client-supplied
/// values (#2034). Over-bound values are malformed and fail closed.
const MAX_EXPERIMENTAL_STRING_BYTES: usize = 256;

/// Bound on list lengths captured from the experimental capability blocks.
const MAX_EXPERIMENTAL_LIST_ITEMS: usize = 64;

/// Bound on string lists rendered into the bounded status projection, so a
/// pathological client advertisement cannot inflate a status payload.
const MAX_PROJECTED_LIST_ITEMS: usize = 16;

/// The immutable typed client-feature profile for one LSP session (#1987,
/// RIPR-SPEC-0143). Parsed exactly once at `initialize` and stored on the
/// session; equality is the semantic capability identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClientFeatureProfile {
    /// The encodings the client advertised in `general.positionEncodings`,
    /// in advertised order. Empty when the client advertised nothing.
    pub(super) advertised_position_encodings: Vec<PositionEncodingKind>,
    /// The encoding selected for this session (UTF-16 preferred, then the
    /// first advertised encoding the server can produce; UTF-16 default).
    pub(super) selected_position_encoding: PositionEncodingKind,
    /// `general.staleRequestSupport.cancel`: the client actively cancels
    /// stale requests.
    pub(super) stale_request_cancellation: bool,
    /// The client carries a `textDocument.publishDiagnostics` capability
    /// block. Push delivery itself needs no capability; the block gates the
    /// optional-field flags below.
    pub(super) push_diagnostics: bool,
    /// `textDocument.diagnostic` present: the client pulls document (and
    /// workspace) diagnostics.
    pub(super) pull_diagnostics: bool,
    /// The client supports related documents for document diagnostic pulls.
    pub(super) pull_diagnostic_related_documents: bool,
    /// `workspace.diagnostics.refreshSupport`: the client accepts
    /// `workspace/diagnostic/refresh` requests.
    pub(super) diagnostic_refresh: bool,
    /// The client accepts diagnostics with related information.
    pub(super) publish_related_information: bool,
    /// The client handles diagnostic tags (unknown tags gracefully).
    pub(super) publish_tags: bool,
    /// The client interprets the `publishDiagnostics` version property.
    pub(super) publish_version: bool,
    /// The client supports the diagnostic `codeDescription` property.
    pub(super) publish_code_description: bool,
    /// The client preserves the diagnostic `data` property into code-action
    /// requests.
    pub(super) publish_data: bool,
    /// Preferred hover content formats in client preference order. Empty
    /// means the client advertised none (the protocol default is plaintext).
    pub(super) hover_content_formats: Vec<MarkupKind>,
    /// The client accepts `CodeAction` literals as code-action responses.
    pub(super) code_action_literal: bool,
    /// The code-action kind value set the client advertised (empty when
    /// literal support is absent).
    pub(super) code_action_kind_value_set: Vec<String>,
    /// The client preserves the `CodeAction.data` property into
    /// `codeAction/resolve`.
    pub(super) code_action_data: bool,
    /// The client supports the `CodeAction.disabled` property.
    pub(super) code_action_disabled: bool,
    /// The client supports the `CodeAction.isPreferred` property.
    pub(super) code_action_is_preferred: bool,
    /// The code-action properties the client can resolve lazily via
    /// `codeAction/resolve` (empty when resolve is unsupported).
    pub(super) code_action_resolve_properties: Vec<String>,
    /// The client honors change annotations in code-action workspace edits.
    pub(super) code_action_honors_change_annotations: bool,
    /// The client supports versioned document changes in `WorkspaceEdit`s.
    pub(super) workspace_edit_document_changes: bool,
    /// The client supports change annotations on workspace edits.
    pub(super) workspace_edit_change_annotations: bool,
    /// The client accepts `window/showDocument` requests.
    pub(super) show_document: bool,
    /// The client accepts server-initiated work-done progress
    /// (`window/workDoneProgress/create` + `$/progress`).
    pub(super) work_done_progress: bool,
    /// The client supports workspace folders.
    pub(super) workspace_folders: bool,
    /// The client accepts `workspace/codeLens/refresh` requests
    /// (RIPR-SPEC-0138).
    pub(super) code_lens_refresh: bool,
    /// The client supports dynamic registration for
    /// `workspace/didChangeWatchedFiles`.
    pub(super) watched_files_dynamic_registration: bool,
    /// The negotiated session-configuration transport (RIPR-SPEC-0136).
    pub(super) configuration_mode: ConfigurationMode,
    /// The RIPR editor extension block from `capabilities.experimental`.
    /// `None` when absent or malformed (fail closed to unsupported).
    pub(super) ripr_editor: Option<RiprEditorClientCapabilities>,
    /// The RIPR agent protocol preferences from `capabilities.experimental`.
    /// `None` when absent or malformed (fail closed to unsupported).
    pub(super) ripr_agent: Option<RiprAgentClientPreferences>,
}

/// The client's `riprEditor` experimental capability block: the extension
/// version, the client commands it offers, and the guarded-test-edit
/// opt-in. Captured, never trusted: every field is bounded and a malformed
/// known field fails the whole block closed to unsupported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiprEditorClientCapabilities {
    /// The extension version string (bounded, non-empty).
    pub(super) version: String,
    /// The client commands the editor can execute, in advertised order.
    pub(super) commands: Vec<String>,
    /// Explicit opt-in to guarded test edits. Absence is `false`; a
    /// non-boolean value is malformed and fails the block closed.
    pub(super) guarded_test_edit: bool,
}

/// The client's `riprAgent` experimental capability block: protocol version
/// plus profile and delivery preferences drawn from the closed protocol
/// vocabularies in `agent_protocol` (RIPR-SPEC-0096 lineage).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiprAgentClientPreferences {
    /// The advertised protocol version. Only supported majors parse;
    /// anything else fails the block closed.
    pub(super) protocol_version: RiprAgentProtocolVersion,
    /// Preferred diagnostic profiles, in advertised preference order.
    pub(super) profiles: Vec<RiprAgentProfile>,
    /// Preferred delivery channels, in advertised preference order.
    pub(super) delivery: Vec<RiprAgentDeliveryPreference>,
}

/// A delivery channel a headless agent client prefers, from a closed
/// vocabulary so unknown channels fail closed instead of being remembered.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RiprAgentDeliveryPreference {
    PushDiagnostics,
    PullDiagnostics,
    StatusNotifications,
}

impl RiprAgentDeliveryPreference {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "push_diagnostics" => Some(Self::PushDiagnostics),
            "pull_diagnostics" => Some(Self::PullDiagnostics),
            "status_notifications" => Some(Self::StatusNotifications),
            _ => None,
        }
    }
}

impl ClientFeatureProfile {
    /// The pre-initialize profile: no capabilities negotiated yet, so every
    /// optional feature is unsupported and defaults apply (UTF-16 encoding,
    /// initialization-only configuration transport).
    pub(super) fn unsupported() -> Self {
        Self {
            advertised_position_encodings: Vec::new(),
            selected_position_encoding: PositionEncodingKind::UTF16,
            stale_request_cancellation: false,
            push_diagnostics: false,
            pull_diagnostics: false,
            pull_diagnostic_related_documents: false,
            diagnostic_refresh: false,
            publish_related_information: false,
            publish_tags: false,
            publish_version: false,
            publish_code_description: false,
            publish_data: false,
            hover_content_formats: Vec::new(),
            code_action_literal: false,
            code_action_kind_value_set: Vec::new(),
            code_action_data: false,
            code_action_disabled: false,
            code_action_is_preferred: false,
            code_action_resolve_properties: Vec::new(),
            code_action_honors_change_annotations: false,
            workspace_edit_document_changes: false,
            workspace_edit_change_annotations: false,
            show_document: false,
            work_done_progress: false,
            workspace_folders: false,
            code_lens_refresh: false,
            watched_files_dynamic_registration: false,
            configuration_mode: ConfigurationMode::InitializationOnly,
            ripr_editor: None,
            ripr_agent: None,
        }
    }

    /// Parse the one typed profile from `InitializeParams`. This is the only
    /// place client capabilities are read; every field the client did not
    /// explicitly advertise stays unsupported.
    pub(super) fn from_initialize_params(params: &InitializeParams) -> Self {
        let capabilities = &params.capabilities;
        let general = capabilities.general.as_ref();
        let text_document = capabilities.text_document.as_ref();
        let workspace = capabilities.workspace.as_ref();
        let window = capabilities.window.as_ref();

        let advertised_position_encodings = general
            .and_then(|value| value.position_encodings.clone())
            .unwrap_or_default();
        let publish = text_document.and_then(|value| value.publish_diagnostics.as_ref());
        let diagnostic = text_document.and_then(|value| value.diagnostic.as_ref());
        let code_action = text_document.and_then(|value| value.code_action.as_ref());
        let literal_support =
            code_action.and_then(|value| value.code_action_literal_support.as_ref());
        let workspace_edit = workspace.and_then(|value| value.workspace_edit.as_ref());

        let experimental = capabilities
            .experimental
            .as_ref()
            .and_then(|value| value.as_object());

        Self {
            selected_position_encoding: select_position_encoding(&advertised_position_encodings),
            advertised_position_encodings,
            stale_request_cancellation: general
                .and_then(|value| value.stale_request_support.as_ref())
                .is_some_and(|value| value.cancel),
            push_diagnostics: publish.is_some(),
            pull_diagnostics: diagnostic.is_some(),
            pull_diagnostic_related_documents: diagnostic
                .and_then(|value| value.related_document_support)
                .unwrap_or(false),
            diagnostic_refresh: workspace
                .and_then(|value| value.diagnostics.as_ref())
                .and_then(|value| value.refresh_support)
                .unwrap_or(false),
            publish_related_information: publish
                .and_then(|value| value.related_information)
                .unwrap_or(false),
            publish_tags: publish.is_some_and(|value| value.tag_support.is_some()),
            publish_version: publish
                .and_then(|value| value.version_support)
                .unwrap_or(false),
            publish_code_description: publish
                .and_then(|value| value.code_description_support)
                .unwrap_or(false),
            publish_data: publish
                .and_then(|value| value.data_support)
                .unwrap_or(false),
            hover_content_formats: text_document
                .and_then(|value| value.hover.as_ref())
                .and_then(|value| value.content_format.clone())
                .unwrap_or_default(),
            code_action_literal: literal_support.is_some(),
            code_action_kind_value_set: literal_support
                .map(|value| value.code_action_kind.value_set.clone())
                .unwrap_or_default(),
            code_action_data: code_action
                .and_then(|value| value.data_support)
                .unwrap_or(false),
            code_action_disabled: code_action
                .and_then(|value| value.disabled_support)
                .unwrap_or(false),
            code_action_is_preferred: code_action
                .and_then(|value| value.is_preferred_support)
                .unwrap_or(false),
            code_action_resolve_properties: code_action
                .and_then(|value| value.resolve_support.as_ref())
                .map(|value| value.properties.clone())
                .unwrap_or_default(),
            code_action_honors_change_annotations: code_action
                .and_then(|value| value.honors_change_annotations)
                .unwrap_or(false),
            workspace_edit_document_changes: workspace_edit
                .and_then(|value| value.document_changes)
                .unwrap_or(false),
            workspace_edit_change_annotations: workspace_edit
                .is_some_and(|value| value.change_annotation_support.is_some()),
            show_document: window
                .and_then(|value| value.show_document.as_ref())
                .is_some_and(|value| value.support),
            work_done_progress: window
                .and_then(|value| value.work_done_progress)
                .unwrap_or(false),
            workspace_folders: workspace
                .and_then(|value| value.workspace_folders)
                .unwrap_or(false),
            code_lens_refresh: workspace
                .and_then(|value| value.code_lens.as_ref())
                .and_then(|value| value.refresh_support)
                .unwrap_or(false),
            watched_files_dynamic_registration: workspace
                .and_then(|value| value.did_change_watched_files.as_ref())
                .and_then(|value| value.dynamic_registration)
                .unwrap_or(false),
            configuration_mode: configuration_mode_for(workspace),
            ripr_editor: experimental
                .and_then(|value| value.get("riprEditor"))
                .and_then(parse_ripr_editor),
            ripr_agent: experimental
                .and_then(|value| value.get("riprAgent"))
                .and_then(parse_ripr_agent),
        }
    }

    /// The bounded status/receipt projection of the selected profile
    /// (RIPR-SPEC-0143 rule: status-visible without dumping the raw
    /// capability document). Every projected string list is capped; counts
    /// disclose any omission. Client name, PID, and timing never appear.
    pub(super) fn status_projection(&self) -> serde_json::Value {
        let ripr_editor = self
            .ripr_editor
            .as_ref()
            .map(|editor| {
                serde_json::json!({
                    "version": editor.version,
                    "command_count": editor.commands.len(),
                    "guarded_test_edit": editor.guarded_test_edit,
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let ripr_agent = self
            .ripr_agent
            .as_ref()
            .map(|agent| {
                serde_json::json!({
                    "protocol_version": agent.protocol_version.as_str(),
                    "profiles": agent.profiles,
                    "delivery": agent.delivery,
                })
            })
            .unwrap_or(serde_json::Value::Null);
        serde_json::json!({
            "position_encoding": self.selected_position_encoding.as_str(),
            "advertised_position_encodings": projected_string_list(
                &self
                    .advertised_position_encodings
                    .iter()
                    .map(|kind| kind.as_str().to_string())
                    .collect::<Vec<_>>(),
            ),
            "stale_request_cancellation": self.stale_request_cancellation,
            "push_diagnostics": self.push_diagnostics,
            "pull_diagnostics": self.pull_diagnostics,
            "pull_diagnostic_related_documents": self.pull_diagnostic_related_documents,
            "diagnostic_refresh": self.diagnostic_refresh,
            "publish_diagnostics": {
                "related_information": self.publish_related_information,
                "tags": self.publish_tags,
                "version": self.publish_version,
                "code_description": self.publish_code_description,
                "data": self.publish_data,
            },
            "hover_content_formats": projected_string_list(
                &self
                    .hover_content_formats
                    .iter()
                    .map(markup_kind_label)
                    .collect::<Vec<_>>(),
            ),
            "code_action": {
                "literal": self.code_action_literal,
                "kinds": projected_string_list(&self.code_action_kind_value_set),
                "data": self.code_action_data,
                "disabled": self.code_action_disabled,
                "is_preferred": self.code_action_is_preferred,
                "resolve_properties": projected_string_list(&self.code_action_resolve_properties),
                "honors_change_annotations": self.code_action_honors_change_annotations,
            },
            "workspace_edit": {
                "document_changes": self.workspace_edit_document_changes,
                "change_annotations": self.workspace_edit_change_annotations,
            },
            "show_document": self.show_document,
            "work_done_progress": self.work_done_progress,
            "workspace_folders": self.workspace_folders,
            "code_lens_refresh": self.code_lens_refresh,
            "watched_files_dynamic_registration": self.watched_files_dynamic_registration,
            "configuration_mode": self.configuration_mode.as_str(),
            "ripr_editor": ripr_editor,
            "ripr_agent": ripr_agent,
        })
    }
}

/// Select the session position encoding from the advertised list (#1626 PR
/// B / #1749): UTF-16 whenever the client supports it, otherwise the first
/// advertised encoding the server can produce, otherwise the UTF-16
/// default. An empty advertisement selects the UTF-16 default.
fn select_position_encoding(advertised: &[PositionEncodingKind]) -> PositionEncodingKind {
    if advertised.is_empty() {
        return PositionEncodingKind::UTF16;
    }
    if advertised.contains(&PositionEncodingKind::UTF16) {
        return PositionEncodingKind::UTF16;
    }
    advertised
        .iter()
        .find(|kind| {
            **kind == PositionEncodingKind::UTF8
                || **kind == PositionEncodingKind::UTF16
                || **kind == PositionEncodingKind::UTF32
        })
        .cloned()
        .unwrap_or(PositionEncodingKind::UTF16)
}

fn configuration_mode_for(
    workspace: Option<&tower_lsp_server::ls_types::WorkspaceClientCapabilities>,
) -> ConfigurationMode {
    if workspace
        .and_then(|value| value.configuration)
        .unwrap_or(false)
    {
        ConfigurationMode::Pull
    } else if workspace.is_some_and(|value| value.did_change_configuration.is_some()) {
        ConfigurationMode::PushFallback
    } else {
        ConfigurationMode::InitializationOnly
    }
}

/// Parse the `riprEditor` experimental block. Any malformed known field
/// fails the whole block closed to unsupported (`None`); unknown extra keys
/// are simply not captured. The standard session is never affected.
fn parse_ripr_editor(value: &serde_json::Value) -> Option<RiprEditorClientCapabilities> {
    let object = value.as_object()?;
    let version = bounded_experimental_string(object.get("version")?)?;
    let mut commands = Vec::new();
    if let Some(raw) = object.get("commands") {
        let items = raw.as_array()?;
        if items.len() > MAX_EXPERIMENTAL_LIST_ITEMS {
            return None;
        }
        for item in items {
            commands.push(bounded_experimental_string(item)?);
        }
    }
    let guarded_test_edit = match object.get("guardedTestEdit") {
        None => false,
        Some(raw) => raw.as_bool()?,
    };
    Some(RiprEditorClientCapabilities {
        version,
        commands,
        guarded_test_edit,
    })
}

/// Parse the `riprAgent` experimental block with the same fail-closed
/// discipline as `parse_ripr_editor`: an unsupported protocol major, an
/// unknown profile or delivery literal, a wrong JSON type, or an over-bound
/// string yields `None`.
fn parse_ripr_agent(value: &serde_json::Value) -> Option<RiprAgentClientPreferences> {
    let object = value.as_object()?;
    let protocol_version =
        RiprAgentProtocolVersion::parse(object.get("protocol")?.as_str()?).ok()?;
    let mut profiles = Vec::new();
    if let Some(raw) = object.get("profiles") {
        let items = raw.as_array()?;
        if items.len() > MAX_EXPERIMENTAL_LIST_ITEMS {
            return None;
        }
        for item in items {
            // The per-string byte bound applies before the closed-vocabulary
            // parse, so an over-long string fails closed at the bound
            // (#1987 review).
            let profile = bounded_experimental_string(item)?;
            profiles.push(
                serde_json::from_value::<RiprAgentProfile>(serde_json::Value::String(profile))
                    .ok()?,
            );
        }
    }
    let mut delivery = Vec::new();
    if let Some(raw) = object.get("delivery") {
        let items = raw.as_array()?;
        if items.len() > MAX_EXPERIMENTAL_LIST_ITEMS {
            return None;
        }
        for item in items {
            let channel = bounded_experimental_string(item)?;
            delivery.push(RiprAgentDeliveryPreference::parse(&channel)?);
        }
    }
    Some(RiprAgentClientPreferences {
        protocol_version,
        profiles,
        delivery,
    })
}

fn bounded_experimental_string(value: &serde_json::Value) -> Option<String> {
    let text = value.as_str()?.trim();
    if text.is_empty() || text.len() > MAX_EXPERIMENTAL_STRING_BYTES {
        return None;
    }
    Some(text.to_string())
}

/// Render a client-advertised string list into the bounded status
/// projection: at most `MAX_PROJECTED_LIST_ITEMS` entries plus an omission
/// count, so the projection stays bounded regardless of the client.
fn projected_string_list(values: &[String]) -> serde_json::Value {
    serde_json::json!({
        "values": values
            .iter()
            .take(MAX_PROJECTED_LIST_ITEMS)
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "omitted_count": values.len().saturating_sub(MAX_PROJECTED_LIST_ITEMS),
    })
}

/// The wire label for a hover content format in the bounded projection.
fn markup_kind_label(kind: &MarkupKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::{
        ClientInfo, CodeActionClientCapabilities, CodeActionKindLiteralSupport,
        CodeActionLiteralSupport, DiagnosticClientCapabilities, GeneralClientCapabilities,
        HoverClientCapabilities, PublishDiagnosticsClientCapabilities,
        StaleRequestSupportClientCapabilities, TextDocumentClientCapabilities,
        WindowClientCapabilities, WorkspaceClientCapabilities,
    };

    fn params_from_json(value: serde_json::Value) -> Result<InitializeParams, String> {
        serde_json::from_value(value).map_err(|error| format!("fixture params must parse: {error}"))
    }

    fn minimal_standard_client() -> InitializeParams {
        InitializeParams::default()
    }

    fn full_modern_standard_client() -> InitializeParams {
        let mut params = InitializeParams::default();
        params.capabilities.general = Some(GeneralClientCapabilities {
            position_encodings: Some(vec![
                PositionEncodingKind::UTF8,
                PositionEncodingKind::UTF16,
            ]),
            stale_request_support: Some(StaleRequestSupportClientCapabilities {
                cancel: true,
                retry_on_content_modified: Vec::new(),
            }),
            ..GeneralClientCapabilities::default()
        });
        params.capabilities.text_document = Some(TextDocumentClientCapabilities {
            publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                related_information: Some(true),
                version_support: Some(true),
                code_description_support: Some(true),
                data_support: Some(true),
                ..PublishDiagnosticsClientCapabilities::default()
            }),
            diagnostic: Some(DiagnosticClientCapabilities {
                related_document_support: Some(true),
                ..DiagnosticClientCapabilities::default()
            }),
            hover: Some(HoverClientCapabilities {
                content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                ..HoverClientCapabilities::default()
            }),
            code_action: Some(CodeActionClientCapabilities {
                code_action_literal_support: Some(CodeActionLiteralSupport {
                    code_action_kind: CodeActionKindLiteralSupport {
                        value_set: vec!["quickfix.ripr".to_string()],
                    },
                }),
                is_preferred_support: Some(true),
                disabled_support: Some(true),
                data_support: Some(true),
                resolve_support: Some(
                    tower_lsp_server::ls_types::CodeActionCapabilityResolveSupport {
                        properties: vec!["edit".to_string()],
                    },
                ),
                honors_change_annotations: Some(true),
                ..CodeActionClientCapabilities::default()
            }),
            ..TextDocumentClientCapabilities::default()
        });
        params.capabilities.window = Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            show_document: Some(tower_lsp_server::ls_types::ShowDocumentClientCapabilities {
                support: true,
            }),
            ..WindowClientCapabilities::default()
        });
        params.capabilities.workspace = Some(WorkspaceClientCapabilities {
            workspace_folders: Some(true),
            configuration: Some(true),
            ..WorkspaceClientCapabilities::default()
        });
        params
    }

    fn vscode_enhanced_client() -> Result<InitializeParams, String> {
        params_from_json(serde_json::json!({
            "processId": 4242,
            "clientInfo": {"name": "Visual Studio Code", "version": "1.90.0"},
            "rootUri": "file:///workspace/demo",
            "capabilities": {
                "general": {"positionEncodings": ["utf-16"]},
                "textDocument": {
                    "publishDiagnostics": {
                        "relatedInformation": true,
                        "versionSupport": false,
                        "codeDescriptionSupport": true,
                        "dataSupport": true
                    },
                    "diagnostic": {"dynamicRegistration": true, "relatedDocumentSupport": false},
                    "hover": {"contentFormat": ["markdown", "plaintext"]},
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": {"valueSet": ["quickfix.ripr", "source.ripr.inspect"]}
                        },
                        "isPreferredSupport": true,
                        "disabledSupport": true,
                        "dataSupport": true,
                        "resolveSupport": {"properties": ["edit"]}
                    }
                },
                "window": {"workDoneProgress": true, "showDocument": {"support": true}},
                "workspace": {
                    "workspaceFolders": true,
                    "configuration": true,
                    "workspaceEdit": {"documentChanges": true}
                },
                "experimental": {
                    "riprEditor": {
                        "version": "0.10.0",
                        "commands": ["ripr.refresh", "ripr.collectWorkspaceStatus"],
                        "guardedTestEdit": false,
                        "futureUnknownKey": {"anything": true}
                    }
                }
            }
        }))
    }

    fn headless_agent_client() -> Result<InitializeParams, String> {
        params_from_json(serde_json::json!({
            "processId": null,
            "capabilities": {
                "general": {"positionEncodings": ["utf-8", "utf-16"]},
                "experimental": {
                    "riprAgent": {
                        "protocol": "0.1",
                        "profiles": ["actionable", "full"],
                        "delivery": ["status_notifications", "pull_diagnostics"]
                    }
                }
            }
        }))
    }

    #[test]
    fn minimal_standard_client_gets_only_protocol_defaults() {
        let profile = ClientFeatureProfile::from_initialize_params(&minimal_standard_client());
        assert_eq!(profile, ClientFeatureProfile::unsupported());
        assert_eq!(
            profile.selected_position_encoding,
            PositionEncodingKind::UTF16
        );
        assert_eq!(
            profile.configuration_mode,
            ConfigurationMode::InitializationOnly
        );
        assert!(profile.ripr_editor.is_none() && profile.ripr_agent.is_none());
    }

    #[test]
    fn full_modern_standard_client_enables_every_standard_feature() -> Result<(), String> {
        let profile = ClientFeatureProfile::from_initialize_params(&full_modern_standard_client());
        let expected_true = [
            (
                "stale_request_cancellation",
                profile.stale_request_cancellation,
            ),
            ("push_diagnostics", profile.push_diagnostics),
            ("pull_diagnostics", profile.pull_diagnostics),
            (
                "pull_diagnostic_related_documents",
                profile.pull_diagnostic_related_documents,
            ),
            (
                "publish_related_information",
                profile.publish_related_information,
            ),
            ("publish_version", profile.publish_version),
            ("publish_code_description", profile.publish_code_description),
            ("publish_data", profile.publish_data),
            ("code_action_literal", profile.code_action_literal),
            ("code_action_data", profile.code_action_data),
            ("code_action_disabled", profile.code_action_disabled),
            ("code_action_is_preferred", profile.code_action_is_preferred),
            (
                "code_action_honors_change_annotations",
                profile.code_action_honors_change_annotations,
            ),
            ("show_document", profile.show_document),
            ("work_done_progress", profile.work_done_progress),
            ("workspace_folders", profile.workspace_folders),
        ];
        for (field, value) in expected_true {
            if !value {
                return Err(format!("full modern client must advertise {field}"));
            }
        }
        assert_eq!(
            profile.selected_position_encoding,
            PositionEncodingKind::UTF16
        );
        assert_eq!(
            profile.hover_content_formats,
            vec![MarkupKind::Markdown, MarkupKind::PlainText]
        );
        assert_eq!(
            profile.code_action_kind_value_set,
            vec!["quickfix.ripr".to_string()]
        );
        assert_eq!(
            profile.code_action_resolve_properties,
            vec!["edit".to_string()]
        );
        assert_eq!(profile.configuration_mode, ConfigurationMode::Pull);
        if profile.publish_tags || profile.diagnostic_refresh || profile.code_lens_refresh {
            return Err("unadvertised optional features must stay unsupported".to_string());
        }
        Ok(())
    }

    #[test]
    fn vscode_enhanced_client_parses_editor_block_and_ignores_unknown_keys() -> Result<(), String> {
        let profile = ClientFeatureProfile::from_initialize_params(&vscode_enhanced_client()?);
        let editor = profile
            .ripr_editor
            .as_ref()
            .ok_or_else(|| "VS Code client must surface the riprEditor block".to_string())?;
        assert_eq!(editor.version, "0.10.0");
        assert_eq!(
            editor.commands,
            vec![
                "ripr.refresh".to_string(),
                "ripr.collectWorkspaceStatus".to_string()
            ]
        );
        assert!(!editor.guarded_test_edit);
        assert_eq!(
            profile.selected_position_encoding,
            PositionEncodingKind::UTF16
        );
        assert!(profile.pull_diagnostics && profile.work_done_progress && profile.show_document);
        assert!(profile.workspace_edit_document_changes);
        if !profile.code_action_literal || !profile.code_action_data {
            return Err("VS Code code-action support must be captured".to_string());
        }
        Ok(())
    }

    #[test]
    fn headless_agent_client_parses_agent_preferences() -> Result<(), String> {
        let profile = ClientFeatureProfile::from_initialize_params(&headless_agent_client()?);
        let agent = profile
            .ripr_agent
            .as_ref()
            .ok_or_else(|| "headless client must surface the riprAgent block".to_string())?;
        assert_eq!(agent.protocol_version.as_str(), "0.1");
        assert_eq!(
            agent.profiles,
            vec![RiprAgentProfile::Actionable, RiprAgentProfile::Full]
        );
        assert_eq!(
            agent.delivery,
            vec![
                RiprAgentDeliveryPreference::StatusNotifications,
                RiprAgentDeliveryPreference::PullDiagnostics
            ]
        );
        if profile.ripr_editor.is_some() {
            return Err("headless client advertised no riprEditor block".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_or_unknown_experimental_fields_fail_closed_without_breaking_the_session()
    -> Result<(), String> {
        let malformed_blocks = [
            (
                "non-object riprEditor",
                serde_json::json!({"riprEditor": "yes"}),
            ),
            (
                "missing riprEditor version",
                serde_json::json!({"riprEditor": {"commands": []}}),
            ),
            (
                "wrong guardedTestEdit type",
                serde_json::json!({"riprEditor": {"version": "0.10.0", "guardedTestEdit": "yes"}}),
            ),
            (
                "unsupported riprAgent major",
                serde_json::json!({"riprAgent": {"protocol": "1.0"}}),
            ),
            (
                "unknown riprAgent profile",
                serde_json::json!({"riprAgent": {"protocol": "0.1", "profiles": ["loud"]}}),
            ),
            (
                "unknown riprAgent delivery channel",
                serde_json::json!({"riprAgent": {"protocol": "0.1", "delivery": ["pigeon"]}}),
            ),
            (
                "oversized command list",
                serde_json::json!({"riprEditor": {"version": "0.10.0", "commands": vec!["ripr.refresh"; 65]}}),
            ),
        ];
        for (label, experimental) in malformed_blocks {
            let params = params_from_json(serde_json::json!({
                "capabilities": {
                    "textDocument": {"diagnostic": {}},
                    "experimental": experimental,
                }
            }))?;
            let profile = ClientFeatureProfile::from_initialize_params(&params);
            if profile.ripr_editor.is_some() || profile.ripr_agent.is_some() {
                return Err(format!("{label} must fail closed to unsupported"));
            }
            if !profile.pull_diagnostics {
                return Err(format!(
                    "{label} must not break the standard session negotiation"
                ));
            }
        }
        // Unknown experimental keys are not captured; a session advertising
        // only unknown blocks is indistinguishable from one advertising none.
        let unknown_only = params_from_json(serde_json::json!({
            "capabilities": {"experimental": {"riprFuture": {"anything": true}}}
        }))?;
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&unknown_only),
            ClientFeatureProfile::unsupported()
        );
        Ok(())
    }

    #[test]
    fn absent_and_explicitly_false_capabilities_are_indistinguishable() -> Result<(), String> {
        let absent = params_from_json(serde_json::json!({
            "capabilities": {"window": {}}
        }))?;
        let explicit_false = params_from_json(serde_json::json!({
            "capabilities": {"window": {"workDoneProgress": false}}
        }))?;
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&absent),
            ClientFeatureProfile::from_initialize_params(&explicit_false)
        );
        Ok(())
    }

    #[test]
    fn equivalent_capability_maps_with_different_key_order_select_equal_profiles()
    -> Result<(), String> {
        let forward = params_from_json(serde_json::json!({
            "capabilities": {
                "general": {"positionEncodings": ["utf-8", "utf-16"]},
                "textDocument": {"diagnostic": {}, "hover": {"contentFormat": ["markdown"]}},
                "experimental": {"riprAgent": {"protocol": "0.1", "profiles": ["actionable"]}}
            }
        }))?;
        let reversed = params_from_json(serde_json::json!({
            "capabilities": {
                "experimental": {"riprAgent": {"profiles": ["actionable"], "protocol": "0.1"}},
                "textDocument": {"hover": {"contentFormat": ["markdown"]}, "diagnostic": {}},
                "general": {"positionEncodings": ["utf-8", "utf-16"]}
            }
        }))?;
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&forward),
            ClientFeatureProfile::from_initialize_params(&reversed)
        );
        Ok(())
    }

    #[test]
    fn session_capability_identity_changes_only_when_selected_behavior_changes()
    -> Result<(), String> {
        let base = vscode_enhanced_client()?;

        // Client name, version, PID, and initialization options are not
        // semantic: the selected behavior is unchanged.
        let mut renamed = base.clone();
        renamed.process_id = Some(9999);
        renamed.client_info = Some(ClientInfo {
            name: "A Completely Different Client".to_string(),
            version: Some("0.0.1".to_string()),
        });
        renamed.initialization_options = Some(serde_json::json!({"checkMode": "deep"}));
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&base),
            ClientFeatureProfile::from_initialize_params(&renamed)
        );

        // A captured capability change is semantic and must change the
        // identity.
        let mut weaker = base.clone();
        weaker.capabilities.window = weaker.capabilities.window.take().map(|mut window| {
            window.work_done_progress = Some(false);
            window
        });
        if ClientFeatureProfile::from_initialize_params(&base)
            == ClientFeatureProfile::from_initialize_params(&weaker)
        {
            return Err(
                "a behavior-changing capability difference must change the profile".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn guarded_test_edit_requires_explicit_opt_in() -> Result<(), String> {
        for (label, experimental, expected) in [
            (
                "absent",
                serde_json::json!({"riprEditor": {"version": "0.10.0"}}),
                false,
            ),
            (
                "explicit true",
                serde_json::json!({"riprEditor": {"version": "0.10.0", "guardedTestEdit": true}}),
                true,
            ),
            (
                "explicit false",
                serde_json::json!({"riprEditor": {"version": "0.10.0", "guardedTestEdit": false}}),
                false,
            ),
        ] {
            let params = params_from_json(serde_json::json!({
                "capabilities": {"experimental": experimental}
            }))?;
            let profile = ClientFeatureProfile::from_initialize_params(&params);
            let editor = profile
                .ripr_editor
                .as_ref()
                .ok_or_else(|| format!("{label} must still parse the riprEditor block"))?;
            if editor.guarded_test_edit != expected {
                return Err(format!("{label} guarded_test_edit mismatch"));
            }
        }
        Ok(())
    }

    #[test]
    fn status_projection_is_bounded_and_never_includes_client_identity() -> Result<(), String> {
        let mut params = full_modern_standard_client();
        params.client_info = Some(ClientInfo {
            name: "identity-must-not-leak".to_string(),
            version: Some("9.9.9".to_string()),
        });
        params.process_id = Some(31337);
        let kinds = (0..40)
            .map(|index| format!("kind.{index}"))
            .collect::<Vec<_>>();
        if let Some(code_action) = params
            .capabilities
            .text_document
            .as_mut()
            .and_then(|text_document| text_document.code_action.as_mut())
        {
            code_action.code_action_literal_support = Some(CodeActionLiteralSupport {
                code_action_kind: CodeActionKindLiteralSupport { value_set: kinds },
            });
        }
        let projection = ClientFeatureProfile::from_initialize_params(&params).status_projection();
        let rendered = projection.to_string();
        for forbidden in ["identity-must-not-leak", "9.9.9", "31337"] {
            if rendered.contains(forbidden) {
                return Err(format!(
                    "status projection leaked client identity `{forbidden}`"
                ));
            }
        }
        let kinds = projection
            .get("code_action")
            .and_then(|code_action| code_action.get("kinds"))
            .ok_or_else(|| "projection must carry code-action kinds".to_string())?;
        if kinds
            .get("values")
            .and_then(|values| values.as_array())
            .is_none_or(|values| values.len() > MAX_PROJECTED_LIST_ITEMS)
        {
            return Err("status projection must cap client-advertised lists".to_string());
        }
        assert_eq!(
            kinds.get("omitted_count").and_then(|count| count.as_u64()),
            Some(24)
        );
        if projection.get("ripr_editor") != Some(&serde_json::Value::Null)
            || projection.get("ripr_agent") != Some(&serde_json::Value::Null)
        {
            return Err("absent experimental blocks must project as null".to_string());
        }
        Ok(())
    }

    #[test]
    fn ripr_agent_entries_fail_closed_beyond_the_string_byte_bound() -> Result<(), String> {
        let overlong = "a".repeat(MAX_EXPERIMENTAL_STRING_BYTES + 1);
        for (label, block) in [
            (
                "profile",
                serde_json::json!({"protocol": "0.1", "profiles": [overlong]}),
            ),
            (
                "delivery",
                serde_json::json!({"protocol": "0.1", "delivery": [overlong]}),
            ),
        ] {
            let params = params_from_json(
                serde_json::json!({"capabilities": {"experimental": {"riprAgent": block}}}),
            )?;
            if ClientFeatureProfile::from_initialize_params(&params)
                .ripr_agent
                .is_some()
            {
                return Err(format!("over-long {label} string must fail closed"));
            }
        }
        // At the bound the string is accepted: a padded valid literal trims
        // to the closed-vocabulary entry and the block parses.
        let padded = format!("{:>width$}", "full", width = MAX_EXPERIMENTAL_STRING_BYTES);
        let params = params_from_json(
            serde_json::json!({"capabilities": {"experimental": {"riprAgent": {"protocol": "0.1", "profiles": [padded]}}}}),
        )?;
        let agent = ClientFeatureProfile::from_initialize_params(&params)
            .ripr_agent
            .ok_or_else(|| "an at-bound string must be accepted".to_string())?;
        assert_eq!(agent.profiles, vec![RiprAgentProfile::Full]);
        Ok(())
    }

    #[test]
    fn status_projection_caps_every_projected_list() -> Result<(), String> {
        let encodings = (0..20)
            .map(|index| format!("enc-{index}"))
            .collect::<Vec<_>>();
        let formats = (0..20)
            .map(|index| {
                if index % 2 == 0 {
                    "markdown"
                } else {
                    "plaintext"
                }
            })
            .collect::<Vec<_>>();
        let params = params_from_json(serde_json::json!({
            "capabilities": {
                "general": {"positionEncodings": encodings},
                "textDocument": {"hover": {"contentFormat": formats}}
            }
        }))?;
        let projection = ClientFeatureProfile::from_initialize_params(&params).status_projection();
        for key in ["advertised_position_encodings", "hover_content_formats"] {
            let list = projection
                .get(key)
                .ok_or_else(|| format!("projection must carry `{key}`"))?;
            let values = list
                .get("values")
                .and_then(|values| values.as_array())
                .ok_or_else(|| format!("`{key}` must project a capped values list"))?;
            if values.len() != MAX_PROJECTED_LIST_ITEMS {
                return Err(format!(
                    "`{key}` must be capped at {MAX_PROJECTED_LIST_ITEMS} entries, got {}",
                    values.len()
                ));
            }
            assert_eq!(
                list.get("omitted_count").and_then(|count| count.as_u64()),
                Some(4),
                "`{key}` must disclose the omission count"
            );
        }

        // A short list projects unchanged with a zero omission count.
        let short = params_from_json(serde_json::json!({
            "capabilities": {
                "general": {"positionEncodings": ["utf-16"]},
                "textDocument": {"hover": {"contentFormat": ["markdown"]}}
            }
        }))?;
        let projection = ClientFeatureProfile::from_initialize_params(&short).status_projection();
        assert_eq!(
            projection.get("advertised_position_encodings"),
            Some(&serde_json::json!({"values": ["utf-16"], "omitted_count": 0}))
        );
        assert_eq!(
            projection.get("hover_content_formats"),
            Some(&serde_json::json!({"values": ["markdown"], "omitted_count": 0}))
        );
        Ok(())
    }

    #[test]
    fn position_encoding_selection_matches_the_documented_preference() {
        let cases: [(&[PositionEncodingKind], PositionEncodingKind); 4] = [
            (&[], PositionEncodingKind::UTF16),
            (
                &[PositionEncodingKind::UTF8, PositionEncodingKind::UTF16],
                PositionEncodingKind::UTF16,
            ),
            (&[PositionEncodingKind::UTF8], PositionEncodingKind::UTF8),
            (
                &[PositionEncodingKind::UTF32, PositionEncodingKind::UTF8],
                PositionEncodingKind::UTF32,
            ),
        ];
        for (advertised, expected) in cases {
            assert_eq!(select_position_encoding(advertised), expected);
        }
    }
}
