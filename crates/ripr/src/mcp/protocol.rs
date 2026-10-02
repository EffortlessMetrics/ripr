use crate::workspace_status::{WorkspaceState, WorkspaceStatus};
use serde::Serialize;
use serde_json::{Value, json};

pub(super) const STATUS_TOOL_NAME: &str = "ripr_workspace_status";
pub(super) const STATUS_RESOURCE_URI: &str = "ripr://workspace/status";

/// What a host shows a model before any call. Cold agents read the earlier
/// text ("bounded, read-only static workspace status") as the whole of RIPR
/// and stopped, or guessed the CLI from `ripr --help`. Name what RIPR answers,
/// what this server does not, and the CLI route that does the analysis. Naming
/// a route is not invoking it: the server still executes nothing (ADR 0022).
pub(super) const INSTRUCTIONS: &str = "RIPR is a static analyzer that asks whether the current tests would notice if the behavior changed in a diff were wrong. This MCP server only reports whether the workspace root is usable (tool `ripr_workspace_status`); it does not analyze the diff, edit source, or run tests or mutation. To analyze, run the ripr CLI in the repository: `ripr check --format json` names each changed-behavior gap and its missing test input; `ripr pilot --root .` lists repair seam IDs with the exact `ripr agent repair` commands; `ripr agent status --root . --json` gives the next command in a repair loop.";

const STATUS_TOOL_DESCRIPTION: &str = "Report whether the RIPR workspace root is usable. The document contains only repository-root discovery state (validated or unavailable, with repository markers and any root error code), configuration presence, and the launch-trust and authority facts this server declares (project_config_trust = not_established, authority = none). `workspace_state: ready` means only that a repository root was discovered — not that analysis ran or that no issues were found. This server exposes no analysis findings, gap records, or exposure evidence over MCP; it does not run analysis or load project-local provider configuration. For findings, run `ripr check --format json` in the repository, or use editor diagnostics from the ripr language server.";

pub(super) fn tools_list_result() -> Value {
    json!({"tools": [status_tool_descriptor()]})
}

pub(super) fn resources_list_result() -> Value {
    json!({"resources": [status_resource_descriptor()]})
}

#[derive(Serialize)]
struct McpStatusDocument<'a> {
    schema_version: &'static str,
    workspace: &'a WorkspaceStatus,
    mcp: McpSurfaceStatus,
}

#[derive(Serialize)]
struct McpSurfaceStatus {
    transport: &'static str,
    tools: [&'static str; 1],
    resources: [&'static str; 1],
    bounds: McpBoundsStatus,
}

#[derive(Serialize)]
struct McpBoundsStatus {
    max_message_bytes: usize,
    max_response_bytes: usize,
}

pub(super) fn status_tool_result(
    status: &WorkspaceStatus,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> Result<Value, String> {
    let document = status_document(status, max_message_bytes, max_response_bytes);
    let structured = serde_json::to_value(&document)
        .map_err(|error| format!("serialize workspace status: {error}"))?;
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
        "structuredContent": structured,
        "isError": false
    }))
}

/// One sentence a model can act on when the root is unusable. The status
/// document carries only `root.error_code`; a host shows the text content, so
/// name the cause and the recovery there. The cause is the root owner's own
/// wording (`RootErrorCode::cause`). The server resolves its root once at
/// startup (ADR 0022), so every recovery is a restart with a different root.
fn unavailable_recovery(status: &WorkspaceStatus) -> Option<String> {
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

pub(super) fn status_resource_result(
    status: &WorkspaceStatus,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> Result<Value, String> {
    let document = status_document(status, max_message_bytes, max_response_bytes);
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

fn status_document(
    status: &WorkspaceStatus,
    max_message_bytes: usize,
    max_response_bytes: usize,
) -> McpStatusDocument<'_> {
    McpStatusDocument {
        schema_version: "ripr-mcp-workspace-status-v1",
        workspace: status,
        mcp: McpSurfaceStatus {
            transport: "stdio",
            tools: [STATUS_TOOL_NAME],
            resources: [STATUS_RESOURCE_URI],
            bounds: McpBoundsStatus {
                max_message_bytes,
                max_response_bytes,
            },
        },
    }
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

fn status_resource_descriptor() -> Value {
    json!({
        "uri": STATUS_RESOURCE_URI,
        "name": "ripr-workspace-status",
        "title": "RIPR workspace status",
        "description": "Bounded, read-only workspace discovery and authority status: repository-root discovery state, configuration presence, and launch-trust and authority facts only. No analysis findings, gap records, or exposure evidence are exposed over MCP; run `ripr check --format json` for findings.",
        "mimeType": "application/json"
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
            "mcp": {
                "type": "object",
                "properties": {
                    "transport": {
                        "type": "string",
                        "const": "stdio"
                    },
                    "tools": {
                        "type": "array",
                        "items": { "const": "ripr_workspace_status" },
                        "minItems": 1,
                        "maxItems": 1
                    },
                    "resources": {
                        "type": "array",
                        "items": { "const": "ripr://workspace/status" },
                        "minItems": 1,
                        "maxItems": 1
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
                "required": ["transport", "tools", "resources", "bounds"],
                "additionalProperties": false
            }
        },
        "required": ["schema_version", "workspace", "mcp"],
        "additionalProperties": false
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
        // #5002: the tool description is the only thing an LLM host sees
        // before spending a call. It must state positively what the document
        // contains, disavow the `ready`-means-analyzed misreading, and name
        // the real evidence route — or a future edit can silently restore the
        // exclusion-only framing.
        let tools = tools_list_result();
        if tools.pointer("/tools/0/name").and_then(Value::as_str) != Some(STATUS_TOOL_NAME) {
            return Err(format!("unexpected tools/list payload: {tools}"));
        }
        let description = json_str(&tools, "/tools/0/description")?;
        for required in [
            "repository-root discovery state",
            "project_config_trust = not_established",
            "authority = none",
            "not that analysis ran or that no issues were found",
            "exposes no analysis findings, gap records, or exposure evidence over MCP",
            "ripr check --format json",
            "editor diagnostics",
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
    fn status_resource_description_states_no_analysis_evidence() -> Result<(), String> {
        let resources = resources_list_result();
        let uri = json_str(&resources, "/resources/0/uri")?;
        if uri != STATUS_RESOURCE_URI {
            return Err(format!("unexpected resources/list payload: {resources}"));
        }
        let description = json_str(&resources, "/resources/0/description")?;
        for required in [
            "repository-root discovery state",
            "No analysis findings, gap records, or exposure evidence are exposed over MCP",
            "ripr check --format json",
        ] {
            if !description.contains(required) {
                return Err(format!(
                    "status resource description lost boundary wording {required:?}: {description}"
                ));
            }
        }
        Ok(())
    }
}
