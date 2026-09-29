use super::protocol;
use crate::workspace_status::WorkspaceStatus;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde::de::DeserializeOwned;

/// Application adapter only. The SDK owns RPC dispatch and lifecycle.
pub(super) struct McpServer {
    tools: ListToolsResult,
    resources: ListResourcesResult,
    tool_status: CallToolResult,
    resource_status: ReadResourceResult,
}

impl McpServer {
    pub(super) fn new(status: WorkspaceStatus) -> Result<Self, ErrorData> {
        let mut server = Self {
            tools: typed(protocol::tools_list_result())?,
            resources: typed(protocol::resources_list_result())?,
            tool_status: typed(
                protocol::status_tool_result(
                    &status,
                    super::MAX_MESSAGE_BYTES,
                    super::MAX_RESPONSE_BYTES,
                )
                .map_err(|_error| ErrorData::internal_error("serialize workspace status", None))?,
            )?,
            resource_status: typed(
                protocol::status_resource_result(
                    &status,
                    super::MAX_MESSAGE_BYTES,
                    super::MAX_RESPONSE_BYTES,
                )
                .map_err(|_error| ErrorData::internal_error("serialize workspace status", None))?,
            )?,
        };
        // Domain JSON carries status/catalog data, not version-dependent SDK
        // result metadata. SDK constructors supply current result fields and
        // its handler owns stripping them for older peers.
        server.tools = ListToolsResult::with_all_items(server.tools.tools)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private);
        server.resources = ListResourcesResult::with_all_items(server.resources.resources)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private);
        server.tool_status.result_type = Some(ResultType::COMPLETE);
        server.resource_status = ReadResourceResult::new(server.resource_status.contents)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private);
        Ok(server)
    }
}
fn typed<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, ErrorData> {
    serde_json::from_value(value)
        .map_err(|_error| ErrorData::internal_error("invalid status projection", None))
}
impl ServerHandler for McpServer {
    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools
            .tools
            .iter()
            .find(|tool| tool.name == name)
            .cloned()
    }
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(protocol::INSTRUCTIONS)
        .with_server_info(Implementation::new("ripr", env!("CARGO_PKG_VERSION")))
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(self.tools.clone())
    }
    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(self.resources.clone())
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if request.name != protocol::STATUS_TOOL_NAME {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "unknown tool; available: ripr_workspace_status",
                Some(serde_json::json!({"available": [protocol::STATUS_TOOL_NAME]})),
            ));
        }
        if request
            .arguments
            .as_ref()
            .is_some_and(|arguments| !arguments.is_empty())
        {
            return Err(ErrorData::invalid_params(
                "ripr_workspace_status does not accept arguments",
                None,
            ));
        }
        Ok(self.tool_status.clone().into())
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != protocol::STATUS_RESOURCE_URI {
            return Err(ErrorData::resource_not_found(
                "unknown resource; available: ripr://workspace/status",
                Some(serde_json::json!({"available": [protocol::STATUS_RESOURCE_URI]})),
            ));
        }
        Ok(self.resource_status.clone().into())
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
