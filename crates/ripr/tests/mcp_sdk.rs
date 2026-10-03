//! Interoperability against the pinned official SDK and the just-built CLI.
use rmcp::model::{
    CallToolRequestParams, ClientConfig, ProtocolVersion, ReadResourceRequestParams,
};
use rmcp::service::{ClientLifecycleMode, ClientServiceExt};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "crate manifest directory has no workspace parent".to_string())
}

async fn sdk_session(
    version: ProtocolVersion,
    lifecycle: ClientLifecycleMode,
    expected_version: ProtocolVersion,
) -> Result<(), String> {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["mcp", "--stdio", "--root"])
        .arg(workspace_root()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("spawn SDK peer: {error}"))?;
    let stderr = child.stderr.take();
    let mut stderr_reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let pipe = stderr.ok_or_else(|| "SDK peer has no stderr pipe".to_string())?;
        pipe.take(1025)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| format!("read SDK peer stderr: {error}"))?;
        Ok::<_, String>(bytes)
    });
    // The child remains owned outside the timed operation. Every success,
    // protocol rejection and timeout reaches the same explicit reap path.
    let operation = tokio::time::timeout(Duration::from_secs(15), async {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "SDK peer has no stdout pipe".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "SDK peer has no stdin pipe".to_string())?;
        let mut client = ClientConfig::default()
            .with_protocol_version(version)
            .serve_with_lifecycle((stdout, stdin), lifecycle)
            .await
            .map_err(|error| format!("SDK lifecycle: {error}"))?;
        let observations = async {
            let info = client
                .peer_info()
                .ok_or_else(|| "SDK lifecycle omitted peer info".to_string())?;
            if info.protocol_version != expected_version {
                return Err(format!(
                    "SDK negotiated {}, expected {expected_version}",
                    info.protocol_version
                ));
            }
            let tools = client
                .list_tools(None)
                .await
                .map_err(|error| error.to_string())?;
            let tool_names = tools
                .tools
                .iter()
                .map(|tool| tool.name.as_ref())
                .collect::<Vec<_>>();
            for expected in [
                "ripr_workspace_status",
                "ripr_refresh",
                "ripr_list_gaps",
                "ripr_get_gap",
            ] {
                if !tool_names.contains(&expected) {
                    return Err(format!(
                        "SDK did not discover the read-only tool {expected}: {tool_names:?}"
                    ));
                }
            }
            let resources = client
                .list_resources(None)
                .await
                .map_err(|error| error.to_string())?;
            let resources = serde_json::to_value(resources).map_err(|error| error.to_string())?;
            if resources
                .get("resources")
                .and_then(Value::as_array)
                .map(Vec::len)
                != Some(1)
                || resources
                    .pointer("/resources/0/uri")
                    .and_then(Value::as_str)
                    != Some("ripr://workspace/status")
            {
                return Err("SDK did not discover exactly the read-only status resource".into());
            }
            let templates = client
                .list_resource_templates(None)
                .await
                .map_err(|error| error.to_string())?;
            let templates = serde_json::to_value(templates).map_err(|error| error.to_string())?;
            let template_uris = templates
                .pointer("/resourceTemplates")
                .and_then(Value::as_array)
                .map(|templates| {
                    templates
                        .iter()
                        .filter_map(|template| {
                            template.pointer("/uriTemplate").and_then(Value::as_str)
                        })
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| "SDK omitted resourceTemplates".to_string())?;
            for expected in [
                "ripr://snapshot/{snapshot_id}",
                "ripr://gap/{canonical_item_id}",
            ] {
                if !template_uris.contains(&expected) {
                    return Err(format!(
                        "SDK did not discover the resource template {expected}: {template_uris:?}"
                    ));
                }
            }
            let tool = client
                .call_tool(CallToolRequestParams::new("ripr_workspace_status"))
                .await
                .map_err(|error| format!("SDK status tool: {error}"))?;
            let tool = serde_json::to_value(tool).map_err(|error| error.to_string())?;
            if tool.get("isError").and_then(Value::as_bool) != Some(false) {
                return Err("SDK status tool returned an error result".into());
            }
            let status = tool
                .get("structuredContent")
                .ok_or_else(|| "SDK status omitted structured content".to_string())?;
            if status
                .pointer("/workspace/workspace_state")
                .and_then(Value::as_str)
                != Some("ready")
            {
                return Err("SDK status did not reach the actual workspace".into());
            }
            if status
                .pointer("/session/attempt_state")
                .and_then(Value::as_str)
                != Some("no_snapshot")
            {
                return Err("SDK status session block drifted".into());
            }
            if status
                .pointer("/mcp/tools")
                .and_then(Value::as_array)
                .map(Vec::len)
                != Some(4)
            {
                return Err("SDK status surface block lost the four-tool contract".into());
            }
            for authority in [
                "source_edit_capability",
                "verification_execution_capability",
                "mutation_execution_capability",
                "model_provider",
            ] {
                if status
                    .pointer(&format!("/workspace/authority/{authority}"))
                    .and_then(Value::as_str)
                    != Some("none")
                {
                    return Err(format!("SDK status expanded {authority}"));
                }
            }
            let resource = client
                .read_resource(ReadResourceRequestParams::new("ripr://workspace/status"))
                .await
                .map_err(|error| format!("SDK status resource: {error}"))?;
            let resource = serde_json::to_value(resource).map_err(|error| error.to_string())?;
            let text = resource
                .pointer("/contents/0/text")
                .and_then(Value::as_str)
                .ok_or_else(|| "SDK resource omitted status text".to_string())?;
            let resource_status: Value =
                serde_json::from_str(text).map_err(|error| error.to_string())?;
            if &resource_status != status {
                return Err("SDK tool and resource projected different status documents".into());
            }
            for (request, code) in [
                (
                    CallToolRequestParams::new("ripr_everything"),
                    rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                ),
                (
                    CallToolRequestParams::new("ripr_workspace_status").with_arguments(
                        serde_json::Map::from_iter([("verbose".to_string(), Value::Bool(true))]),
                    ),
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                ),
            ] {
                match client.call_tool(request).await {
                    Err(rmcp::service::ServiceError::McpError(error)) if error.code == code => {}
                    _ => {
                        return Err("SDK accepted unknown tool or nonempty status arguments".into());
                    }
                }
            }
            let expected_resource_code = if expected_version == ProtocolVersion::V_2026_07_28 {
                -32602
            } else {
                -32002
            };
            match client
                .read_resource(ReadResourceRequestParams::new("ripr://workspace/missing"))
                .await
            {
                Err(rmcp::service::ServiceError::McpError(error))
                    if serde_json::to_value(error.code).map_err(|error| error.to_string())?
                        == serde_json::json!(expected_resource_code) =>
                {
                    if error.message != "unknown resource; available: ripr://workspace/status"
                        || error
                            .data
                            .as_ref()
                            .and_then(|data| data.pointer("/available/0"))
                            .and_then(Value::as_str)
                            != Some("ripr://workspace/status")
                    {
                        return Err(
                            "SDK resource miss changed bounded message or available URI data"
                                .into(),
                        );
                    }
                }
                _ => {
                    return Err(
                        "SDK resource miss did not retain its version-specific protocol code"
                            .into(),
                    );
                }
            }
            Ok(())
        }
        .await;
        let closed = client
            .close()
            .await
            .map(|_| ())
            .map_err(|error| format!("close SDK transport: {error}"));
        observations.and(closed)
    })
    .await;
    let operation = operation
        .map_err(|_error| "SDK session exceeded its owned deadline".to_string())
        .and_then(|result| result);
    let mut forced_cleanup = false;
    let cleanup = async {
        match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
            Ok(status) => status.map_err(|error| format!("reap SDK peer: {error}")),
            Err(_) => {
                forced_cleanup = true;
                child
                    .start_kill()
                    .map_err(|error| format!("request SDK peer termination: {error}"))?;
                tokio::time::timeout(Duration::from_secs(3), child.wait())
                    .await
                    .map_err(|_error| "terminated SDK peer exceeded its reap deadline".to_string())?
                    .map_err(|error| format!("reap terminated SDK peer: {error}"))
            }
        }
    }
    .await;
    let stderr = match tokio::time::timeout(Duration::from_secs(2), &mut stderr_reader).await {
        Ok(result) => result.map_err(|error| format!("join SDK stderr reader: {error}"))??,
        Err(_) => {
            stderr_reader.abort();
            let _ = stderr_reader.await;
            return Err("SDK stderr reader exceeded its owned deadline".into());
        }
    };
    let status = cleanup?;
    operation?;
    if forced_cleanup {
        return Err("SDK peer required forced cleanup after transport close".into());
    }
    if !status.success() || !stderr.is_empty() {
        return Err("SDK peer did not exit cleanly with uncontaminated stderr".into());
    }
    Ok(())
}

#[tokio::test]
async fn official_sdk_legacy_client_lists_calls_and_reads_status() -> Result<(), String> {
    sdk_session(
        ProtocolVersion::V_2025_11_25,
        ClientLifecycleMode::Initialize,
        ProtocolVersion::V_2025_11_25,
    )
    .await
}

#[tokio::test]
async fn official_sdk_discovery_client_lists_calls_and_reads_status() -> Result<(), String> {
    sdk_session(
        ProtocolVersion::V_2026_07_28,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
        ProtocolVersion::V_2026_07_28,
    )
    .await
}

#[tokio::test]
async fn initialize_with_discovery_only_version_negotiates_a_handshake_version()
-> Result<(), String> {
    sdk_session(
        ProtocolVersion::V_2026_07_28,
        ClientLifecycleMode::Initialize,
        ProtocolVersion::LATEST_WITH_INITIALIZE,
    )
    .await
}

#[tokio::test]
async fn initialize_with_unknown_version_negotiates_a_supported_handshake_version()
-> Result<(), String> {
    let unknown = serde_json::from_value::<ProtocolVersion>(serde_json::json!("2099-01-01"))
        .map_err(|error| format!("decode unknown protocol version: {error}"))?;
    sdk_session(
        unknown,
        ClientLifecycleMode::Initialize,
        ProtocolVersion::LATEST_WITH_INITIALIZE,
    )
    .await
}
