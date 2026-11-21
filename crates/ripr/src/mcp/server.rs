mod rpc;

use super::protocol;
use crate::workspace_status::WorkspaceStatus;
use rpc::{
    RpcError, error_response, inline_protocol_version, readable_id, require_object,
    required_params_object, success_response,
};
use serde_json::{Map, Value};

pub(super) fn bounded_error_response(code: i64, message: &str, data: Option<Value>) -> Value {
    rpc::bounded_error_response(code, message, data)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Lifecycle {
    New,
    Legacy {
        protocol_version: String,
        initialized: bool,
    },
    Inline,
}

pub(super) struct McpServer {
    lifecycle: Lifecycle,
    status: WorkspaceStatus,
}

impl McpServer {
    pub(super) fn new(status: WorkspaceStatus) -> Self {
        Self {
            lifecycle: Lifecycle::New,
            status,
        }
    }

    pub(super) fn handle_frame(&mut self, frame: &[u8]) -> Option<Value> {
        let message = match serde_json::from_slice::<Value>(frame) {
            Ok(message) => message,
            Err(_error) => {
                return Some(error_response(
                    None,
                    protocol::ERROR_PARSE,
                    "Parse error",
                    None,
                ));
            }
        };
        self.handle_message(message)
    }

    fn handle_message(&mut self, message: Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(error_response(
                None,
                protocol::ERROR_INVALID_REQUEST,
                "Invalid Request",
                None,
            ));
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(error_response(
                readable_id(object),
                protocol::ERROR_INVALID_REQUEST,
                "Invalid Request",
                None,
            ));
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Some(error_response(
                readable_id(object),
                protocol::ERROR_INVALID_REQUEST,
                "Invalid Request",
                None,
            ));
        };
        let id = readable_id(object);
        let is_notification = !object.contains_key("id");
        if is_notification {
            self.handle_notification(method);
            return None;
        }
        let Some(id) = id else {
            return Some(error_response(
                None,
                protocol::ERROR_INVALID_REQUEST,
                "Invalid Request",
                None,
            ));
        };

        let result = match method {
            "initialize" => self.initialize(object),
            "server/discover" => self.discover(object),
            "ping" if matches!(self.lifecycle, Lifecycle::New) => Ok(protocol::empty_result(false)),
            _ => self.handle_ready_request(method, object),
        };
        Some(match result {
            Ok(result) => success_response(id, result),
            Err(error) => error_response(Some(id), error.code, error.message, error.data),
        })
    }

    fn handle_notification(&mut self, method: &str) {
        if method == "notifications/initialized"
            && let Lifecycle::Legacy { initialized, .. } = &mut self.lifecycle
        {
            *initialized = true;
        }
    }

    fn initialize(&mut self, request: &Map<String, Value>) -> Result<Value, RpcError> {
        if matches!(self.lifecycle, Lifecycle::Inline) {
            return Err(RpcError::invalid_request(
                "initialize cannot follow server/discover",
            ));
        }
        let params = required_params_object(request)?;
        let protocol_version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("initialize requires protocolVersion"))?;
        require_object(params, "capabilities", "initialize requires capabilities")?;
        require_object(params, "clientInfo", "initialize requires clientInfo")?;
        let negotiated = if protocol::is_supported_protocol_version(protocol_version) {
            protocol_version
        } else {
            protocol::CURRENT_PROTOCOL_VERSION
        };
        if let Lifecycle::Legacy {
            protocol_version: previous,
            ..
        } = &self.lifecycle
            && previous != negotiated
        {
            return Err(RpcError::invalid_request(
                "initialize cannot renegotiate an active session",
            ));
        }
        let initialized = matches!(
            self.lifecycle,
            Lifecycle::Legacy {
                initialized: true,
                ..
            }
        );
        self.lifecycle = Lifecycle::Legacy {
            protocol_version: negotiated.to_string(),
            initialized,
        };
        Ok(protocol::initialize_result(negotiated))
    }

    fn discover(&mut self, request: &Map<String, Value>) -> Result<Value, RpcError> {
        if matches!(self.lifecycle, Lifecycle::Legacy { .. }) {
            return Err(RpcError::invalid_request(
                "server/discover cannot replace an initialized session",
            ));
        }
        inline_protocol_version(request)?;
        self.lifecycle = Lifecycle::Inline;
        Ok(protocol::discover_result())
    }

    fn handle_ready_request(
        &self,
        method: &str,
        request: &Map<String, Value>,
    ) -> Result<Value, RpcError> {
        let protocol_version = self.request_protocol_version(request)?;
        let current_protocol = protocol::is_current_protocol(protocol_version);
        let ping_supported = matches!(
            &self.lifecycle,
            Lifecycle::Legacy {
                protocol_version,
                initialized: true,
            } if !protocol::is_current_protocol(protocol_version)
        );
        match method {
            "ping" if ping_supported => Ok(protocol::empty_result(false)),
            "ping" => Err(RpcError::method_not_found(method)),
            "tools/list" => Ok(protocol::tools_list_result(current_protocol)),
            "tools/call" => self.call_tool(request, current_protocol),
            "resources/list" => Ok(protocol::resources_list_result(current_protocol)),
            "resources/templates/list" => {
                Ok(protocol::resource_templates_list_result(current_protocol))
            }
            "resources/read" => self.read_resource(request, current_protocol),
            _ => Err(RpcError::method_not_found(method)),
        }
    }

    fn request_protocol_version<'a>(
        &'a self,
        request: &'a Map<String, Value>,
    ) -> Result<&'a str, RpcError> {
        match &self.lifecycle {
            Lifecycle::New => Err(RpcError::invalid_request(
                "MCP session is not initialized; call initialize or server/discover first",
            )),
            Lifecycle::Legacy {
                protocol_version,
                initialized,
            } => {
                if !initialized {
                    return Err(RpcError::invalid_request(
                        "MCP session is waiting for notifications/initialized",
                    ));
                }
                Ok(protocol_version.as_str())
            }
            Lifecycle::Inline => inline_protocol_version(request),
        }
    }

    fn call_tool(
        &self,
        request: &Map<String, Value>,
        current_protocol: bool,
    ) -> Result<Value, RpcError> {
        let params = required_params_object(request)?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("tools/call requires name"))?;
        if name != protocol::STATUS_TOOL_NAME {
            return Err(RpcError::invalid_params("unknown RIPR tool"));
        }
        if let Some(arguments) = params.get("arguments") {
            let Some(arguments) = arguments.as_object() else {
                return Err(RpcError::invalid_params(
                    "ripr_workspace_status arguments must be an object",
                ));
            };
            if !arguments.is_empty() {
                return Err(RpcError::invalid_params(
                    "ripr_workspace_status does not accept arguments",
                ));
            }
        }
        protocol::status_tool_result(
            &self.status,
            super::MAX_MESSAGE_BYTES,
            super::MAX_RESPONSE_BYTES,
            current_protocol,
        )
        .map_err(RpcError::internal)
    }

    fn read_resource(
        &self,
        request: &Map<String, Value>,
        current_protocol: bool,
    ) -> Result<Value, RpcError> {
        let params = required_params_object(request)?;
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("resources/read requires uri"))?;
        if uri != protocol::STATUS_RESOURCE_URI {
            return if current_protocol {
                Err(RpcError::invalid_params("unknown RIPR resource"))
            } else {
                Err(RpcError::resource_not_found(uri))
            };
        }
        protocol::status_resource_result(
            &self.status,
            super::MAX_MESSAGE_BYTES,
            super::MAX_RESPONSE_BYTES,
            current_protocol,
        )
        .map_err(RpcError::internal)
    }
}

#[cfg(test)]
mod tests;
