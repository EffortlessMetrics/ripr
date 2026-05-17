use super::uri::path_from_file_uri;
use super::{COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, REFRESH_COMMAND};
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
                commands: vec![
                    REFRESH_COMMAND.to_string(),
                    COLLECT_CONTEXT_COMMAND.to_string(),
                    COLLECT_EVIDENCE_CONTEXT_COMMAND.to_string(),
                ],
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

#[expect(
    deprecated,
    reason = "InitializeParams.root_path is deprecated by LSP but still required as a fallback for clients that have not migrated to workspaceFolders."
)]
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

#[cfg(test)]
mod tests {
    use super::{initialize_result, root_from_initialize_params};
    use crate::lsp::{COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, REFRESH_COMMAND};
    use serde_json::json;
    use std::path::Path;
    use tower_lsp_server::ls_types::{
        CodeActionProviderCapability, HoverProviderCapability, InitializeParams,
        TextDocumentSyncCapability, TextDocumentSyncKind,
    };

    #[test]
    fn initialize_result_advertises_editor_read_capabilities() -> Result<(), String> {
        let result = initialize_result();
        let capabilities = result.capabilities;

        assert_eq!(
            capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
        );
        assert_eq!(
            capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        );
        assert_eq!(
            capabilities.code_action_provider,
            Some(CodeActionProviderCapability::Simple(true))
        );
        let server_info = result
            .server_info
            .ok_or_else(|| "server info should be advertised".to_string())?;
        assert_eq!(server_info.name, "ripr");

        let commands = capabilities
            .execute_command_provider
            .ok_or_else(|| "execute commands should be advertised".to_string())?
            .commands;
        assert_eq!(
            commands,
            vec![
                REFRESH_COMMAND.to_string(),
                COLLECT_CONTEXT_COMMAND.to_string(),
                COLLECT_EVIDENCE_CONTEXT_COMMAND.to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn root_from_initialize_params_prefers_workspace_folder_root_uri_then_fallback()
    -> Result<(), String> {
        let fallback = Path::new("/fallback/root");
        let params = initialize_params(json!({
            "processId": null,
            "rootUri": "file:///root-uri",
            "capabilities": {},
            "workspaceFolders": [
                { "uri": "file:///workspace%20root", "name": "workspace root" }
            ]
        }))?;

        assert_eq!(
            root_from_initialize_params(&params, fallback),
            Path::new("/workspace root")
        );

        let params = initialize_params(json!({
            "processId": null,
            "rootUri": "file:///root-uri",
            "capabilities": {}
        }))?;
        assert_eq!(
            root_from_initialize_params(&params, fallback),
            Path::new("/root-uri")
        );

        let params = initialize_params(json!({
            "processId": null,
            "capabilities": {}
        }))?;
        assert_eq!(root_from_initialize_params(&params, fallback), fallback);
        Ok(())
    }

    fn initialize_params(value: serde_json::Value) -> Result<InitializeParams, String> {
        serde_json::from_value(value)
            .map_err(|err| format!("test initialize params should deserialize: {err}"))
    }
}
