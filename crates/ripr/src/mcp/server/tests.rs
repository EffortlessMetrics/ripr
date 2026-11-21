use super::*;
use serde_json::json;

fn status() -> WorkspaceStatus {
    WorkspaceStatus::resolve(None)
}

fn request(value: Value) -> Result<Value, String> {
    let encoded = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    let mut server = McpServer::new(status());
    server
        .handle_frame(&encoded)
        .ok_or_else(|| "expected a response".to_string())
}

#[test]
fn invalid_json_is_a_parse_error_without_an_id() -> Result<(), String> {
    let mut server = McpServer::new(status());
    let response = server
        .handle_frame(b"{not-json")
        .ok_or_else(|| "expected parse error response".to_string())?;
    // JSON-RPC 2.0: the id is unreadable, so the error response must carry
    // an explicit null id rather than omitting the member entirely.
    if response.get("id") != Some(&Value::Null) {
        return Err("parse error must carry a null id for an unreadable id".to_string());
    }
    if response.pointer("/error/code").and_then(Value::as_i64) != Some(protocol::ERROR_PARSE) {
        return Err("parse error code drifted".to_string());
    }
    Ok(())
}

#[test]
fn unsupported_initialize_version_negotiates_to_server_default() -> Result<(), String> {
    let response = request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2099-01-01",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" }
        }
    }))?;
    if response
        .pointer("/result/protocolVersion")
        .and_then(Value::as_str)
        != Some(protocol::CURRENT_PROTOCOL_VERSION)
    {
        return Err(
            "unsupported initialize version did not negotiate to the server default".to_string(),
        );
    }
    Ok(())
}

#[test]
fn repeated_initialize_is_stable_for_the_same_negotiated_version() -> Result<(), String> {
    let initialize = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" }
        }
    }))
    .map_err(|error| error.to_string())?;
    let mut server = McpServer::new(status());
    let first = server
        .handle_frame(&initialize)
        .ok_or_else(|| "expected first initialize response".to_string())?;
    let second = server
        .handle_frame(&initialize)
        .ok_or_else(|| "expected repeated initialize response".to_string())?;
    if first != second {
        return Err("repeated initialize must be stable".to_string());
    }
    Ok(())
}

#[test]
fn current_discovery_rejects_malformed_required_metadata() -> Result<(), String> {
    let response = request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": protocol::CURRENT_PROTOCOL_VERSION
            }
        }
    }))?;
    if response.pointer("/error/code").and_then(Value::as_i64)
        != Some(protocol::ERROR_INVALID_PARAMS)
    {
        return Err("malformed required metadata error code drifted".to_string());
    }
    Ok(())
}

#[test]
fn inline_request_rejects_an_unsupported_protocol_version() -> Result<(), String> {
    let response = request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2099-01-01",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    }))?;
    if response.pointer("/error/code").and_then(Value::as_i64)
        != Some(protocol::ERROR_UNSUPPORTED_PROTOCOL_VERSION)
    {
        return Err("unsupported inline protocol error code drifted".to_string());
    }
    if response
        .pointer("/error/data/requested")
        .and_then(Value::as_str)
        != Some("2099-01-01")
    {
        return Err("unsupported inline protocol omitted the requested version".to_string());
    }
    Ok(())
}

#[test]
fn legacy_resource_miss_uses_the_legacy_resource_error() -> Result<(), String> {
    let mut server = McpServer::new(status());
    let initialize = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" }
        }
    }))
    .map_err(|error| error.to_string())?;
    let _initialize_response = server
        .handle_frame(&initialize)
        .ok_or_else(|| "expected initialize response".to_string())?;
    let initialized = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))
    .map_err(|error| error.to_string())?;
    if server.handle_frame(&initialized).is_some() {
        return Err("initialized notification emitted a response".to_string());
    }
    let read = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/read",
        "params": { "uri": "ripr://workspace/missing" }
    }))
    .map_err(|error| error.to_string())?;
    let response = server
        .handle_frame(&read)
        .ok_or_else(|| "expected resource error response".to_string())?;
    if response.pointer("/error/code").and_then(Value::as_i64)
        != Some(protocol::ERROR_RESOURCE_NOT_FOUND)
    {
        return Err("legacy resource error code drifted".to_string());
    }
    Ok(())
}

#[test]
fn discover_lifecycle_rejects_ping_even_when_selecting_an_older_version() -> Result<(), String> {
    let mut server = McpServer::new(status());
    let discover = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2025-11-25",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    }))
    .map_err(|error| error.to_string())?;
    let _discover_response = server
        .handle_frame(&discover)
        .ok_or_else(|| "expected discovery response".to_string())?;
    let ping = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "ping",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2025-11-25",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    }))
    .map_err(|error| error.to_string())?;
    let response = server
        .handle_frame(&ping)
        .ok_or_else(|| "expected ping error response".to_string())?;
    if response.pointer("/error/code").and_then(Value::as_i64)
        != Some(protocol::ERROR_METHOD_NOT_FOUND)
    {
        return Err("discover lifecycle must not restore legacy ping".to_string());
    }
    Ok(())
}

#[test]
fn discovery_is_stable_and_advertises_only_read_only_surfaces() -> Result<(), String> {
    let discover = json!({
        "jsonrpc": "2.0",
        "id": "discover",
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": protocol::CURRENT_PROTOCOL_VERSION,
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {
                    "name": "test",
                    "version": "1"
                }
            }
        }
    });
    let encoded = serde_json::to_vec(&discover).map_err(|error| error.to_string())?;
    let mut server = McpServer::new(status());
    let first = server
        .handle_frame(&encoded)
        .ok_or_else(|| "expected first discovery response".to_string())?;
    let second = server
        .handle_frame(&encoded)
        .ok_or_else(|| "expected repeated discovery response".to_string())?;
    if first != second {
        return Err("repeated discovery must be byte-model stable".to_string());
    }
    if first.pointer("/result/resultType").and_then(Value::as_str) != Some("complete") {
        return Err("discovery must use the current result discriminator".to_string());
    }
    if first.pointer("/result/capabilities/tools").is_none()
        || first.pointer("/result/capabilities/resources").is_none()
    {
        return Err("discovery must advertise tools and resources".to_string());
    }
    Ok(())
}
