use crate::workspace_status::{WorkspaceState, WorkspaceStatus};
use serde::Serialize;
use serde_json::{Value, json};

use super::gaps::GAP_SCHEMA_VERSION;
use super::workspace::{
    AttemptFailure, CODE_NO_SNAPSHOT, REFRESH_SCHEMA_VERSION, RESERVED_FAILURE_CODES,
    SESSION_SCHEMA_VERSION, SessionProfile, WorkspaceSession,
};

pub(super) const STATUS_TOOL_NAME: &str = "ripr_workspace_status";
pub(super) const REFRESH_TOOL_NAME: &str = "ripr_refresh";
pub(super) const LIST_GAPS_TOOL_NAME: &str = "ripr_list_gaps";
pub(super) const GET_GAP_TOOL_NAME: &str = "ripr_get_gap";
pub(super) const STATUS_RESOURCE_URI: &str = "ripr://workspace/status";
pub(super) const SNAPSHOT_RESOURCE_TEMPLATE: &str = "ripr://snapshot/{snapshot_id}";
pub(super) const GAP_RESOURCE_TEMPLATE: &str = "ripr://gap/{canonical_item_id}";

/// The typed-failure vocabulary every evidence tool can return. Codes a
/// tool cannot reach in this slice stay named (reserved) so the wire
/// contract is stable when the owning slice lands; each tool description
/// says which codes it can actually return.
fn failure_vocabulary() -> Vec<&'static str> {
    let mut codes = vec![
        "workspace_unavailable",
        "analysis_failed",
        "unsupported_profile",
        CODE_NO_SNAPSHOT,
        "analysis_in_flight",
        "stale_snapshot",
        "item_not_found",
        "result_too_large",
    ];
    codes.extend(RESERVED_FAILURE_CODES.iter().copied());
    codes
}

/// What a host shows a model before any call. Name what RIPR answers, what
/// this server does not, and the CLI route that runs analysis outside this
/// session. Naming a route is not invoking it: the server still edits
/// nothing and executes nothing (ADR 0022).
pub(super) const INSTRUCTIONS: &str = "RIPR is a static analyzer that asks whether the current tests would notice if the behavior changed in a diff were wrong. This MCP server exposes one read-only workspace session. Call `ripr_workspace_status` for root discovery, authority, and session facts; call `ripr_refresh` to run one bounded static analysis and commit a completed snapshot; call `ripr_list_gaps` for the deterministically bounded working set of canonical items; then call `ripr_get_gap` (or read `ripr://gap/{canonical_item_id}`) for one item's complete bounded evidence. The server never edits source, runs tests or mutation, executes verification commands, or loads project-local provider configuration, and no evidence document is ever a repair authorization. To analyze outside this session, run the ripr CLI in the repository: `ripr check --format json` names each changed-behavior gap and its missing test input.";

const STATUS_TOOL_DESCRIPTION: &str = "Report the RIPR workspace and session state. The document contains repository-root discovery state (validated or unavailable, with repository markers and any root error code), launch-trust and authority facts (source edit, verification execution, mutation execution, and model provider are all none), and a session block: current desired input (workspace diff against the default branch, draft mode), current attempt state (no_snapshot, in_flight, completed, or failed), the last completed snapshot identity, last-known-good state, freshness as of the last refresh, the typed AnalysisOutcome of the committed snapshot, and the built-in profile and support facts. `workspace_state: ready` means only that a repository root was discovered — not that analysis ran or that no issues were found. This tool returns session facts only: no gap evidence. It never edits source, runs tests or mutation, executes verification commands, or loads project-local provider configuration. To run analysis, call ripr_refresh.";

const REFRESH_TOOL_DESCRIPTION: &str = "Run one bounded static analysis of the workspace diff through RIPR's shared check authority (the same analysis `ripr check` and the language server run) and commit the completed snapshot into this server's session. The call blocks until the attempt reaches a terminal state and reports the attempt state: completed (a snapshot identity is returned, bound to the typed AnalysisOutcome), failed (a typed failure code, bounded detail, and recovery; the last-known-good snapshot is kept), in_flight (a concurrent attempt is running; poll ripr_workspace_status), or workspace_unavailable (the root was not usable; restart the server with `--root <repository>`). An attempt runs to a terminal state; cancelling the MCP request never rolls an attempt back or manufactures a snapshot, and a cancelled or superseded attempt is never committed. Bounded analysis: project-local configuration is not loaded (built-in defaults, draft mode). This tool never edits source, executes verification or mutation commands, or prepares a repair. Recovery vocabulary: analysis_failed (retry; run `ripr check --format json` for the full diagnostic), unsupported_profile (narrow the diff), plus reserved codes (config_invalid, workspace_ambiguous, static_limitation, cancelled, superseded) owned by later slices.";

const LIST_GAPS_TOOL_DESCRIPTION: &str = "Return the deterministic bounded working set of canonical items for the current completed snapshot (or for an explicitly named snapshot_id, which must match the current one or the call fails closed with stale_snapshot and the current identity). The response contains total, eligible, selected, and omitted counts; selected and complete serialized bytes; every omitted identity with its reason; the snapshot/profile/budget identity and selection basis; and one small summary per selected item (canonical_id, exposure class, language, file, line). Selection is the shared CLI/LSP budget authority over the snapshot's canonical items; MCP does not re-rank, never truncates silently, and infers no business risk. Overflow is disclosed with reasons and the omitted identities, and the continuation route is ripr_get_gap. Before the first successful ripr_refresh this tool fails closed with no_snapshot; while an attempt runs it reports analysis_in_flight; a document that cannot fit the response bound fails with result_too_large. Summaries carry no evidence detail; read one item with ripr_get_gap.";

const GET_GAP_TOOL_DESCRIPTION: &str = "Return one canonical item's complete bounded evidence from the current completed snapshot (optional snapshot_id must match the current snapshot or the call fails closed with stale_snapshot). The document binds the item to its snapshot identity and contains: identity and location; the changed behavior (expression, before/after, delta kind, probe family); causal attribution (canonical gap owner, behavior kind, probe kind, normalized discriminator); discriminator availability and the producer's observed/missing evidence; related tests with oracle kind and strength; readiness, which is always repair_packet_ready = false with the reason — this evidence never authorizes an edit; a repair boundary of none_declared, prepared by the repair slice (#3090); and links to the snapshot resource and (as an explicit null) the reserved repair-attempt resource. Unknown ids fail closed with item_not_found; a document over the response bound fails with result_too_large. Before the first successful ripr_refresh this tool fails closed with no_snapshot; while an attempt runs it reports analysis_in_flight. Equivalent evidence reads: the tool ripr_get_gap and the resource ripr://gap/{canonical_item_id} return the same document.";

const STATUS_RESOURCE_DESCRIPTION: &str = "Bounded, read-only workspace discovery, authority, and session status: repository-root discovery state, configuration presence, launch-trust and authority facts, and the current analysis session (attempt state, last completed snapshot identity, typed AnalysisOutcome, profile facts). No gap evidence detail is exposed here; run ripr_refresh, then read items through ripr_get_gap.";

const SNAPSHOT_RESOURCE_TEMPLATE_DESCRIPTION: &str = "Bounded evidence for one completed snapshot: the snapshot identity, the typed AnalysisOutcome, the full canonical item index (identities and locations, not evidence), and the stored bounded-selection summary. Read one item's complete evidence through ripr_get_gap or ripr://gap/{canonical_item_id}. An unknown or superseded snapshot id is rejected with a typed no_snapshot or stale_snapshot failure.";

const GAP_RESOURCE_TEMPLATE_DESCRIPTION: &str = "One canonical item's complete bounded evidence, identical to the ripr_get_gap tool result: changed behavior, causal attribution, discriminator availability, related tests, and an explicit never-repair-ready boundary. The item must exist in the current completed snapshot or the read fails closed with item_not_found.";

pub(super) fn tools_list_result() -> Value {
    json!({"tools": [
        status_tool_descriptor(),
        refresh_tool_descriptor(),
        list_gaps_tool_descriptor(),
        get_gap_tool_descriptor(),
    ]})
}

pub(super) fn resources_list_result() -> Value {
    json!({"resources": [status_resource_descriptor()]})
}

pub(super) fn resource_templates_list_result() -> Value {
    json!({"resourceTemplates": [
        {
            "uriTemplate": SNAPSHOT_RESOURCE_TEMPLATE,
            "name": "ripr-snapshot-evidence",
            "title": "RIPR snapshot evidence",
            "description": SNAPSHOT_RESOURCE_TEMPLATE_DESCRIPTION,
            "mimeType": "application/json",
        },
        {
            "uriTemplate": GAP_RESOURCE_TEMPLATE,
            "name": "ripr-gap-evidence",
            "title": "RIPR gap evidence",
            "description": GAP_RESOURCE_TEMPLATE_DESCRIPTION,
            "mimeType": "application/json",
        },
    ]})
}

#[derive(Serialize)]
struct McpStatusDocument<'a> {
    schema_version: &'static str,
    workspace: &'a WorkspaceStatus,
    session: Value,
    mcp: McpSurfaceStatus,
}

#[derive(Serialize)]
struct McpSurfaceStatus {
    transport: &'static str,
    tools: [&'static str; 4],
    resources: [&'static str; 1],
    resource_templates: [&'static str; 2],
    bounds: McpBoundsStatus,
}

#[derive(Serialize)]
struct McpBoundsStatus {
    max_message_bytes: usize,
    max_response_bytes: usize,
}

/// Build the status document for the tool result and the status resource.
pub(super) fn status_document(
    status: &WorkspaceStatus,
    session: &WorkspaceSession,
    profile: &SessionProfile,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> Value {
    serde_json::to_value(McpStatusDocument {
        schema_version: "ripr-mcp-workspace-status-v1",
        workspace: status,
        session: session.session_document(profile),
        mcp: McpSurfaceStatus {
            transport: "stdio",
            tools: [
                STATUS_TOOL_NAME,
                REFRESH_TOOL_NAME,
                LIST_GAPS_TOOL_NAME,
                GET_GAP_TOOL_NAME,
            ],
            resources: [STATUS_RESOURCE_URI],
            resource_templates: [SNAPSHOT_RESOURCE_TEMPLATE, GAP_RESOURCE_TEMPLATE],
            bounds: McpBoundsStatus {
                max_message_bytes,
                max_response_bytes,
            },
        },
    })
    .unwrap_or_else(|_| {
        // Every field is producer-serializable by contract; a serialization
        // failure here is an instrument problem, and the status surface must
        // never panic (ADR 0022 bounded adapter). A minimal honest document
        // keeps the wire alive without fabricating facts.
        json!({
            "schema_version": "ripr-mcp-workspace-status-v1",
            "workspace": Value::Null,
            "session": Value::Null,
            "mcp": Value::Null,
        })
    })
}

/// The `ripr_workspace_status` tool envelope: the status document plus, when
/// the root is unavailable, one recovery sentence a host can show a model.
/// The document itself carries only `root.error_code`; the text content names
/// the cause and the restart route (the pinned single-tool behavior).
pub(super) fn status_tool_result(
    status: &WorkspaceStatus,
    session: &WorkspaceSession,
    profile: &SessionProfile,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> Result<Value, String> {
    let document = status_document(
        status,
        session,
        profile,
        max_message_bytes,
        max_response_bytes,
    );
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("render workspace status: {error}"))?;
    let mut content = vec![json!({
        "type": "text",
        "text": text
    })];
    if let Some(recovery) = unavailable_recovery(status) {
        content.push(json!({
            "type": "text",
            "text": recovery
        }));
    }
    Ok(json!({
        "content": content,
        "structuredContent": document,
        "isError": false
    }))
}

/// A successful tool result envelope: pretty text for hosts, the document
/// as structured content, and the standard non-error flag.
pub(super) fn tool_result(document: Value) -> Result<Value, String> {
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("render tool document: {error}"))?;
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": document,
        "isError": false,
    }))
}

/// A typed tool failure envelope: standard `isError` semantics with the
/// failure as structured content and one recovery sentence in text form.
pub(super) fn tool_failure(
    failure: &AttemptFailure,
    schema_version: &str,
) -> Result<Value, String> {
    let document = failure.document(schema_version);
    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("{}: {} Recovery: {}", failure.code, failure.detail, failure.recovery),
        }],
        "structuredContent": document,
        "isError": true,
    }))
}

/// One sentence a model can act on when the root is unusable. The status
/// document carries only `root.error_code`; a host shows the text content, so
/// name the cause and the recovery there. The cause is the root owner's own
/// wording (`RootErrorCode::cause`). The server resolves its root once at
/// startup (ADR 0022), so every recovery is a restart with a different root.
pub(super) fn unavailable_recovery(status: &WorkspaceStatus) -> Option<String> {
    if status.workspace_state != WorkspaceState::Unavailable {
        return None;
    }
    let (code, cause) = match status.root.error_code {
        Some(code) => (code.as_str(), code.cause(status.root.source)),
        None => ("unknown", "the root could not be validated".to_string()),
    };
    Some(format!(
        "Workspace unavailable ({code}): {cause}. This server resolves its root once at startup; restart it with `ripr mcp --stdio --root <repository>` pointing at the repository to analyze."
    ))
}

/// The `ripr://workspace/status` resource payload.
pub(super) fn status_resource_result(
    status: &WorkspaceStatus,
    session: &WorkspaceSession,
    profile: &SessionProfile,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> Result<Value, String> {
    let document = status_document(
        status,
        session,
        profile,
        max_message_bytes,
        max_response_bytes,
    );
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("render workspace status: {error}"))?;
    Ok(json!({
        "contents": [{
            "uri": STATUS_RESOURCE_URI,
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

fn status_tool_descriptor() -> Value {
    json!({
        "name": STATUS_TOOL_NAME,
        "title": "RIPR workspace status",
        "description": STATUS_TOOL_DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": false
        },
        "outputSchema": status_output_schema(),
        "annotations": {
            "title": "RIPR workspace status",
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn refresh_tool_descriptor() -> Value {
    json!({
        "name": REFRESH_TOOL_NAME,
        "title": "RIPR refresh",
        "description": REFRESH_TOOL_DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": false
        },
        "outputSchema": refresh_output_schema(),
        "annotations": {
            "title": "RIPR refresh",
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": false,
            "openWorldHint": true
        }
    })
}

fn list_gaps_tool_descriptor() -> Value {
    json!({
        "name": LIST_GAPS_TOOL_NAME,
        "title": "RIPR bounded gap list",
        "description": LIST_GAPS_TOOL_DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": {
                "snapshot_id": { "type": "string" }
            },
            "additionalProperties": false
        },
        "outputSchema": gap_list_output_schema(),
        "annotations": {
            "title": "RIPR bounded gap list",
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn get_gap_tool_descriptor() -> Value {
    json!({
        "name": GET_GAP_TOOL_NAME,
        "title": "RIPR gap evidence",
        "description": GET_GAP_TOOL_DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": {
                "gap_id": { "type": "string", "minLength": 1 },
                "snapshot_id": { "type": "string" }
            },
            "required": ["gap_id"],
            "additionalProperties": false
        },
        "outputSchema": gap_output_schema(),
        "annotations": {
            "title": "RIPR gap evidence",
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn status_resource_descriptor() -> Value {
    json!({
        "uri": STATUS_RESOURCE_URI,
        "name": "ripr-workspace-status",
        "title": "RIPR workspace status",
        "description": STATUS_RESOURCE_DESCRIPTION,
        "mimeType": "application/json"
    })
}

fn failure_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "code": { "type": "string", "enum": failure_vocabulary() },
            "detail": { "type": "string" },
            "recovery": { "type": "string" },
            "data": { "type": "object" }
        },
        "required": ["code", "detail", "recovery", "data"],
        "additionalProperties": false
    })
}

fn typed_failure_document_schema(schema_version: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            "schema_version": { "type": "string", "const": schema_version },
            "failure": failure_schema()
        },
        "required": ["schema_version", "failure"],
        "additionalProperties": false
    })
}

fn status_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "schema_version": {
                "type": "string",
                "const": "ripr-mcp-workspace-status-v1"
            },
            "workspace": {
                "type": "object",
                "properties": {
                    "schema_version": {
                        "type": "string",
                        "const": "ripr-workspace-status-v1"
                    },
                    "workspace_state": {
                        "type": "string",
                        "enum": ["ready", "unavailable"]
                    },
                    "root": {
                        "type": "object",
                        "properties": {
                            "state": {
                                "type": "string",
                                "enum": ["validated", "unavailable"]
                            },
                            "source": {
                                "type": "string",
                                "enum": [
                                    "explicit",
                                    "current_directory",
                                    "ancestor_discovery",
                                    "unavailable"
                                ]
                            },
                            "identity": { "type": "string" },
                            "repository_markers": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "error_code": {
                                "type": "string",
                                "enum": [
                                    "current_directory_unavailable",
                                    "root_missing",
                                    "root_not_directory",
                                    "root_canonicalize_failed",
                                    "repository_marker_missing"
                                ]
                            }
                        },
                        "required": ["state", "source", "repository_markers"],
                        "additionalProperties": false
                    },
                    "configuration": {
                        "type": "object",
                        "properties": {
                            "project_config_state": {
                                "type": "string",
                                "enum": [
                                    "built_in_defaults_only",
                                    "detected_not_loaded",
                                    "unavailable"
                                ]
                            }
                        },
                        "required": ["project_config_state"],
                        "additionalProperties": false
                    },
                    "trust": {
                        "type": "object",
                        "properties": {
                            "project_config_trust": {
                                "type": "string",
                                "const": "not_established"
                            },
                            "effective_access": {
                                "type": "string",
                                "const": "read_only_status"
                            }
                        },
                        "required": ["project_config_trust", "effective_access"],
                        "additionalProperties": false
                    },
                    "authority": {
                        "type": "object",
                        "properties": {
                            "source_edit_capability": {
                                "type": "string",
                                "const": "none"
                            },
                            "verification_execution_capability": {
                                "type": "string",
                                "const": "none"
                            },
                            "mutation_execution_capability": {
                                "type": "string",
                                "const": "none"
                            },
                            "model_provider": {
                                "type": "string",
                                "const": "none"
                            }
                        },
                        "required": [
                            "source_edit_capability",
                            "verification_execution_capability",
                            "mutation_execution_capability",
                            "model_provider"
                        ],
                        "additionalProperties": false
                    },
                    "claim_boundary": { "type": "string" },
                    "limitations": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": [
                    "schema_version",
                    "workspace_state",
                    "root",
                    "configuration",
                    "trust",
                    "authority",
                    "claim_boundary",
                    "limitations"
                ],
                "additionalProperties": false
            },
            "session": {
                "type": "object",
                "properties": {
                    "schema_version": {
                        "type": "string",
                        "const": SESSION_SCHEMA_VERSION
                    },
                    "attempt_state": {
                        "type": "string",
                        "enum": ["no_snapshot", "in_flight", "completed", "failed"]
                    },
                    "current_attempt": {
                        "type": ["string", "null"],
                        "enum": ["in_flight", null]
                    },
                    "current_desired_input": {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "const": "workspace_diff" },
                            "base": { "type": "string", "const": "default_branch" },
                            "mode": { "type": "string", "const": "draft" },
                            "scope": { "type": "string", "const": "diff" }
                        },
                        "required": ["kind", "base", "mode", "scope"],
                        "additionalProperties": false
                    },
                    "last_completed_snapshot": { "type": ["object", "null"] },
                    "last_known_good": { "type": ["object", "null"] },
                    "last_failure": { "type": ["object", "null"] },
                    "freshness": {
                        "type": "object",
                        "properties": {
                            "state": {
                                "type": "string",
                                "enum": [
                                    "current_at_last_refresh",
                                    "stale_after_failed_attempt",
                                    "none"
                                ]
                            },
                            "note": { "type": "string" }
                        },
                        "required": ["state", "note"],
                        "additionalProperties": false
                    },
                    "analysis_outcome": { "type": ["object", "null"] },
                    "profile": {
                        "type": "object",
                        "properties": {
                            "mode": { "type": "string", "const": "draft" },
                            "languages": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "project_config": {
                                "type": "string",
                                "const": "detected_not_loaded"
                            },
                            "support": {
                                "type": "string",
                                "const": "built_in_defaults"
                            }
                        },
                        "required": ["mode", "languages", "project_config", "support"],
                        "additionalProperties": false
                    },
                    "limitations": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": [
                    "schema_version",
                    "attempt_state",
                    "current_attempt",
                    "current_desired_input",
                    "last_completed_snapshot",
                    "last_known_good",
                    "last_failure",
                    "freshness",
                    "analysis_outcome",
                    "profile",
                    "limitations"
                ],
                "additionalProperties": false
            },
            "mcp": {
                "type": "object",
                "properties": {
                    "transport": {
                        "type": "string",
                        "const": "stdio"
                    },
                    "tools": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 4,
                        "maxItems": 4
                    },
                    "resources": {
                        "type": "array",
                        "items": { "const": "ripr://workspace/status" },
                        "minItems": 1,
                        "maxItems": 1
                    },
                    "resource_templates": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 2,
                        "maxItems": 2
                    },
                    "bounds": {
                        "type": "object",
                        "properties": {
                            "max_message_bytes": {
                                "type": "integer",
                                "minimum": 1
                            },
                            "max_response_bytes": {
                                "type": "integer",
                                "minimum": 1
                            }
                        },
                        "required": ["max_message_bytes", "max_response_bytes"],
                        "additionalProperties": false
                    }
                },
                "required": ["transport", "tools", "resources", "resource_templates", "bounds"],
                "additionalProperties": false
            }
        },
        "required": ["schema_version", "workspace", "session", "mcp"],
        "additionalProperties": false
    })
}

fn refresh_output_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "schema_version": { "type": "string", "const": REFRESH_SCHEMA_VERSION },
                    "attempt": {
                        "type": "object",
                        "properties": {
                            "state": {
                                "type": "string",
                                "enum": ["no_snapshot", "in_flight", "completed", "failed"]
                            },
                            "failure": { "type": ["object", "null"] }
                        },
                        "required": ["state", "failure"],
                        "additionalProperties": false
                    },
                    "snapshot": { "type": ["object", "null"] },
                    "last_known_good": { "type": ["object", "null"] },
                    "claim_boundary": { "type": "string" },
                    "limitations": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["schema_version", "attempt", "snapshot", "last_known_good", "claim_boundary", "limitations"],
                "additionalProperties": false
            },
            typed_failure_document_schema(REFRESH_SCHEMA_VERSION)
        ]
    })
}

fn gap_list_output_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "schema_version": { "type": "string", "const": super::gaps::GAP_LIST_SCHEMA_VERSION },
                    "snapshot_id": { "type": "string" },
                    "requested_snapshot_id": { "type": ["string", "null"] },
                    "snapshot_profile_budget_identity": { "type": "string" },
                    "selection_basis_version": { "type": "string" },
                    "budget": {
                        "type": "object",
                        "properties": {
                            "max_items_per_workspace_response": { "type": "integer", "minimum": 1 },
                            "max_items_per_document": { "type": "integer", "minimum": 1 },
                            "max_serialized_bytes": { "type": "integer", "minimum": 1 },
                            "max_inline_detail_bytes": { "type": "integer", "minimum": 1 }
                        },
                        "required": [
                            "max_items_per_workspace_response",
                            "max_items_per_document",
                            "max_serialized_bytes",
                            "max_inline_detail_bytes"
                        ],
                        "additionalProperties": false
                    },
                    "total": { "type": "integer", "minimum": 0 },
                    "eligible": { "type": "integer", "minimum": 0 },
                    "selected": { "type": "integer", "minimum": 0 },
                    "omitted": { "type": "integer", "minimum": 0 },
                    "selected_bytes": { "type": "integer", "minimum": 0 },
                    "complete_bytes": { "type": "integer", "minimum": 0 },
                    "overflowed": { "type": "boolean" },
                    "overflow_reasons": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": [
                                "document_item_limit",
                                "workspace_item_limit",
                                "serialized_byte_limit",
                                "inline_detail_limit"
                            ]
                        }
                    },
                    "items": { "type": "array" },
                    "omitted_items": { "type": "array" },
                    "continuation": { "type": "object" },
                    "claim_boundary": { "type": "string" },
                    "limitations": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": [
                    "schema_version",
                    "snapshot_id",
                    "requested_snapshot_id",
                    "snapshot_profile_budget_identity",
                    "selection_basis_version",
                    "budget",
                    "total",
                    "eligible",
                    "selected",
                    "omitted",
                    "selected_bytes",
                    "complete_bytes",
                    "overflowed",
                    "overflow_reasons",
                    "items",
                    "omitted_items",
                    "continuation",
                    "claim_boundary",
                    "limitations"
                ],
                "additionalProperties": false
            },
            typed_failure_document_schema(super::gaps::GAP_LIST_SCHEMA_VERSION)
        ]
    })
}

fn gap_output_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "schema_version": { "type": "string", "const": GAP_SCHEMA_VERSION },
                    "snapshot_id": { "type": "string" },
                    "item": { "type": "object" },
                    "claim_boundary": { "type": "string" },
                    "limitations": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["schema_version", "snapshot_id", "item", "claim_boundary", "limitations"],
                "additionalProperties": false
            },
            typed_failure_document_schema(GAP_SCHEMA_VERSION)
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_str<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{pointer} missing or not a string"))
    }

    #[test]
    fn status_tool_description_names_payload_scope_and_analysis_boundary() -> Result<(), String> {
        // The tool description is the only thing an LLM host sees before
        // spending a call. It must state positively what the document
        // contains, disavow the `ready`-means-analyzed misreading, name the
        // session contract, and point at the refresh route — or a future
        // edit can silently restore the exclusion-only framing.
        let tools = tools_list_result();
        let names = tools
            .pointer("/tools")
            .and_then(Value::as_array)
            .map(|tools| tools.len())
            .ok_or_else(|| "tools/list payload drifted".to_string())?;
        if names != 4 {
            return Err(format!(
                "expected the four-tool slice surface, got {names} tools"
            ));
        }
        let description = json_str(&tools, "/tools/0/description")?;
        for required in [
            "repository-root discovery state",
            "source edit, verification execution, mutation execution, and model provider are all none",
            "not that analysis ran or that no issues were found",
            "session facts only: no gap evidence",
            "never edits source, runs tests or mutation, executes verification commands",
            "To run analysis, call ripr_refresh",
        ] {
            if !description.contains(required) {
                return Err(format!(
                    "status tool description lost payload boundary wording {required:?}: {description}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn tool_descriptions_state_positive_contracts_and_bounds() -> Result<(), String> {
        let tools = tools_list_result();
        let descriptions: [(&str, &str, &[&str]); 3] = [
            (
                "/tools/1/description",
                REFRESH_TOOL_NAME,
                &[
                    "shared check authority",
                    "never edits source, executes verification or mutation commands",
                    "cancelled or superseded attempt is never committed",
                    "workspace_unavailable",
                    "unsupported_profile",
                ],
            ),
            (
                "/tools/2/description",
                LIST_GAPS_TOOL_NAME,
                &[
                    "deterministic bounded working set",
                    "never truncates silently",
                    "no business risk",
                    "stale_snapshot",
                    "no_snapshot",
                    "result_too_large",
                    "ripr_get_gap",
                ],
            ),
            (
                "/tools/3/description",
                GET_GAP_TOOL_NAME,
                &[
                    "complete bounded evidence",
                    "repair_packet_ready = false",
                    "never authorizes an edit",
                    "item_not_found",
                    "ripr://gap/{canonical_item_id}",
                ],
            ),
        ];
        for (pointer, name, required_words) in descriptions {
            let tool_name = json_str(&tools, pointer.replace("/description", "/name").as_str())?;
            if tool_name != name {
                return Err(format!("tool order drifted at {pointer}: {tool_name}"));
            }
            let description = json_str(&tools, pointer)?;
            for required in required_words {
                if !description.contains(required) {
                    return Err(format!(
                        "{name} description lost contract wording {required:?}: {description}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn status_resource_description_states_session_not_evidence() -> Result<(), String> {
        let resources = resources_list_result();
        let uri = json_str(&resources, "/resources/0/uri")?;
        if uri != STATUS_RESOURCE_URI {
            return Err(format!("unexpected resources/list payload: {resources}"));
        }
        let description = json_str(&resources, "/resources/0/description")?;
        for required in [
            "workspace discovery, authority, and session status",
            "No gap evidence detail is exposed here",
            "ripr_get_gap",
        ] {
            if !description.contains(required) {
                return Err(format!(
                    "status resource description lost boundary wording {required:?}: {description}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn resource_templates_expose_snapshot_and_gap_routes() -> Result<(), String> {
        let templates = resource_templates_list_result();
        let templates = templates
            .pointer("/resourceTemplates")
            .and_then(Value::as_array)
            .ok_or_else(|| "resourceTemplates payload drifted".to_string())?;
        if templates.len() != 2 {
            return Err(format!(
                "expected two resource templates, got {}",
                templates.len()
            ));
        }
        for (index, expected) in [(0, SNAPSHOT_RESOURCE_TEMPLATE), (1, GAP_RESOURCE_TEMPLATE)] {
            let value = templates
                .get(index)
                .ok_or_else(|| format!("template {index} missing"))?;
            let uri = value
                .pointer("/uriTemplate")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("template {index} lost its uriTemplate"))?;
            if uri != expected {
                return Err(format!("template {index} drifted: {uri}"));
            }
            let description = value
                .pointer("/description")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("template {index} lost its description"))?;
            if description.is_empty() {
                return Err(format!("template {index} carries an empty description"));
            }
        }
        Ok(())
    }

    #[test]
    fn failure_vocabulary_covers_the_issue_contract() -> Result<(), String> {
        let codes = failure_vocabulary();
        for required in [
            "no_snapshot",
            "analysis_in_flight",
            "stale_snapshot",
            "workspace_unavailable",
            "unsupported_profile",
            "item_not_found",
            "result_too_large",
            "analysis_failed",
            "config_invalid",
            "workspace_ambiguous",
            "static_limitation",
            "cancelled",
            "superseded",
        ] {
            if !codes.contains(&required) {
                return Err(format!("failure vocabulary lost {required}: {codes:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn instructions_name_the_progressive_flow_and_the_cli_route() -> Result<(), String> {
        for required in [
            "ripr_workspace_status",
            "ripr_refresh",
            "ripr_list_gaps",
            "ripr_get_gap",
            "ripr://gap/{canonical_item_id}",
            "never edits source",
            "ripr check --format json",
        ] {
            if !INSTRUCTIONS.contains(required) {
                return Err(format!("instructions lost {required:?}: {INSTRUCTIONS}"));
            }
        }
        Ok(())
    }
}
