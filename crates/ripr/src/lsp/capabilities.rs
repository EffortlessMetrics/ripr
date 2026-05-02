use super::REFRESH_COMMAND;
use super::uri::path_from_file_uri;
use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::{
    CodeActionProviderCapability, ExecuteCommandOptions, HoverProviderCapability, InitializeParams,
    InitializeResult, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};

pub(super) fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: vec![REFRESH_COMMAND.to_string()],
                ..ExecuteCommandOptions::default()
            }),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "ripr".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        offset_encoding: None,
    }
}

#[allow(deprecated)]
pub(super) fn root_from_initialize_params(
    params: &InitializeParams,
    fallback_root: &Path,
) -> PathBuf {
    params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .and_then(|folder| path_from_file_uri(&folder.uri))
        .or_else(|| params.root_uri.as_ref().and_then(path_from_file_uri))
        .unwrap_or_else(|| fallback_root.to_path_buf())
}
