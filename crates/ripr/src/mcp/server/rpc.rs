use super::super::protocol;
use serde_json::{Map, Value, json};

const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

pub(super) fn required_params_object(
    request: &Map<String, Value>,
) -> Result<&Map<String, Value>, RpcError> {
    request
        .get("params")
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid_params("request params must be an object"))
}

pub(super) fn require_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    message: &'static str,
) -> Result<&'a Map<String, Value>, RpcError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid_params(message))
}

pub(super) fn inline_protocol_version(request: &Map<String, Value>) -> Result<&str, RpcError> {
    let params = required_params_object(request)?;
    let metadata = require_object(
        params,
        "_meta",
        "request _meta is required for the inline MCP lifecycle",
    )?;
    let protocol_version = metadata
        .get(META_PROTOCOL_VERSION)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RpcError::invalid_params(
                "request _meta requires io.modelcontextprotocol/protocolVersion",
            )
        })?;
    if metadata
        .get(META_CLIENT_CAPABILITIES)
        .and_then(Value::as_object)
        .is_none()
    {
        return Err(RpcError::invalid_params(
            "request _meta requires io.modelcontextprotocol/clientCapabilities",
        ));
    }
    if !protocol::is_supported_protocol_version(protocol_version) {
        return Err(RpcError::unsupported_protocol_version(protocol_version));
    }
    Ok(protocol_version)
}

pub(super) fn readable_id(request: &Map<String, Value>) -> Option<Value> {
    let id = request.get("id")?;
    match id {
        Value::Null | Value::Number(_) | Value::String(_) => Some(id.clone()),
        _ => None,
    }
}

pub(super) fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(super) fn bounded_error_response(code: i64, message: &str, data: Option<Value>) -> Value {
    error_response(None, code, message, data)
}

pub(super) fn error_response(
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Value {
    let mut error = Map::from_iter([
        ("code".to_string(), Value::Number(code.into())),
        ("message".to_string(), Value::String(message.to_string())),
    ]);
    if let Some(data) = data {
        error.insert("data".to_string(), data);
    }
    // JSON-RPC 2.0: an error response MUST carry "id" even when the
    // request id could not be determined (parse errors, oversized frames)
    // — omitting it lets validating clients reject the response outright.
    Value::Object(Map::from_iter([
        ("jsonrpc".to_string(), Value::String("2.0".to_string())),
        ("error".to_string(), Value::Object(error)),
        ("id".to_string(), id.unwrap_or(Value::Null)),
    ]))
}

pub(super) struct RpcError {
    pub(super) code: i64,
    pub(super) message: &'static str,
    pub(super) data: Option<Value>,
}

impl RpcError {
    pub(super) fn invalid_request(message: &'static str) -> Self {
        Self {
            code: protocol::ERROR_INVALID_REQUEST,
            message,
            data: None,
        }
    }

    pub(super) fn invalid_params(message: &'static str) -> Self {
        Self {
            code: protocol::ERROR_INVALID_PARAMS,
            message,
            data: None,
        }
    }

    pub(super) fn method_not_found(method: &str) -> Self {
        Self {
            code: protocol::ERROR_METHOD_NOT_FOUND,
            message: "Method not found",
            data: Some(json!({ "method": method })),
        }
    }

    pub(super) fn resource_not_found(uri: &str) -> Self {
        Self {
            code: protocol::ERROR_RESOURCE_NOT_FOUND,
            message: "Resource not found",
            data: Some(json!({ "uri": uri })),
        }
    }

    pub(super) fn unsupported_protocol_version(requested: &str) -> Self {
        Self {
            code: protocol::ERROR_UNSUPPORTED_PROTOCOL_VERSION,
            message: "Unsupported protocol version",
            data: Some(json!({
                "requested": requested,
                "supported": protocol::SUPPORTED_PROTOCOL_VERSIONS
            })),
        }
    }

    pub(super) fn internal(_message: String) -> Self {
        Self {
            code: protocol::ERROR_INTERNAL,
            message: "Internal error",
            data: None,
        }
    }
}
