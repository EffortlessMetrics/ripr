use super::*;

#[test]
fn sdk_server_metadata_preserves_bounded_status_instructions() -> Result<(), String> {
    let server =
        McpServer::new(WorkspaceStatus::resolve(None)).map_err(|error| error.to_string())?;
    let config = server.get_info();
    let instructions = config
        .instructions
        .ok_or_else(|| "SDK metadata omitted instructions".to_string())?;
    if config.server_info.name != "ripr"
        || !instructions.contains("ripr check --format json")
        || !instructions.contains("does not analyze")
    {
        return Err("SDK metadata changed application identity or analysis authority".into());
    }
    let tools = &server.tools;
    let tool = tools
        .tools
        .first()
        .ok_or_else(|| "status tool missing".to_string())?;
    if tools.tools.len() != 1 || tool.name != protocol::STATUS_TOOL_NAME {
        return Err("SDK descriptor widened status tool set".into());
    }
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
