use super::*;

#[test]
fn sdk_server_metadata_preserves_bounded_status_instructions() -> Result<(), String> {
    let server = McpServer::new(WorkspaceStatus::resolve_with_root(None).0, None)
        .map_err(|error| error.to_string())?;
    let config = server.get_info();
    let instructions = config
        .instructions
        .ok_or_else(|| "SDK metadata omitted instructions".to_string())?;
    if config.server_info.name != "ripr"
        || !instructions.contains("ripr check --format json")
        || !instructions.contains("never edits source")
        || !instructions.contains("ripr_list_gaps")
    {
        return Err("SDK metadata changed application identity or analysis authority".into());
    }
    let tools = &server.tools;
    if tools.tools.len() != 4 {
        return Err(format!(
            "SDK descriptor must expose the four-tool slice surface, got {} tools",
            tools.tools.len()
        ));
    }
    let tool = tools
        .tools
        .iter()
        .find(|tool| tool.name == protocol::STATUS_TOOL_NAME)
        .ok_or_else(|| "status tool missing".to_string())?;
    let document = serde_json::to_value(tool).map_err(|error| error.to_string())?;
    if document
        .pointer("/annotations/readOnlyHint")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("SDK status tool lost read-only annotation".into());
    }
    Ok(())
}

#[test]
fn sdk_server_declares_snapshot_and_gap_resource_templates() -> Result<(), String> {
    let server = McpServer::new(WorkspaceStatus::resolve_with_root(None).0, None)
        .map_err(|error| error.to_string())?;
    let templates =
        serde_json::to_value(&server.resource_templates).map_err(|error| error.to_string())?;
    let templates = templates
        .pointer("/resourceTemplates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "SDK resource templates missing".to_string())?;
    let uris = templates
        .iter()
        .filter_map(|template| {
            template
                .pointer("/uriTemplate")
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Vec<_>>();
    for expected in [
        protocol::SNAPSHOT_RESOURCE_TEMPLATE,
        protocol::GAP_RESOURCE_TEMPLATE,
    ] {
        if !uris.contains(&expected) {
            return Err(format!("SDK resource templates lost {expected}: {uris:?}"));
        }
    }
    Ok(())
}

#[tokio::test]
async fn list_gaps_before_any_refresh_is_a_typed_no_snapshot_failure() -> Result<(), String> {
    let server = McpServer::new(WorkspaceStatus::resolve_with_root(None).0, None)
        .map_err(|error| error.to_string())?;
    let response = server
        .list_gaps_tool(None)
        .await
        .map_err(|error| error.to_string())?;
    let result = match response {
        rmcp::model::CallToolResponse::Complete(result) => result,
        other => {
            return Err(format!(
                "pre-refresh list_gaps must complete with a typed failure, got {other:?}"
            ));
        }
    };
    let value = serde_json::to_value(result).map_err(|error| error.to_string())?;
    if value
        .pointer("/isError")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(format!(
            "pre-refresh list_gaps must be a typed failure: {value}"
        ));
    }
    if value
        .pointer("/structuredContent/failure/code")
        .and_then(serde_json::Value::as_str)
        != Some(workspace::CODE_NO_SNAPSHOT)
    {
        return Err(format!("pre-refresh list_gaps lost no_snapshot: {value}"));
    }
    Ok(())
}

#[tokio::test]
async fn get_gap_rejects_bad_arguments_at_the_dispatch_edge() -> Result<(), String> {
    let server = McpServer::new(WorkspaceStatus::resolve_with_root(None).0, None)
        .map_err(|error| error.to_string())?;
    for arguments in [
        serde_json::json!({}),
        serde_json::json!({"gap_id": ""}),
        serde_json::json!({"gap_id": 7}),
        serde_json::json!({"gap_id": "gap:x", "verbose": true}),
    ] {
        let arguments = serde_json::from_value(arguments).map_err(|error| error.to_string())?;
        match server.get_gap_tool(arguments).await {
            Err(error) if error.code == rmcp::model::ErrorCode::INVALID_PARAMS => {}
            Ok(response) => {
                return Err(format!(
                    "get_gap must reject malformed arguments with invalid params: {response:?}"
                ));
            }
            Err(error) => {
                return Err(format!(
                    "get_gap must reject malformed arguments with invalid params, got {error:?}"
                ));
            }
        }
    }
    Ok(())
}
