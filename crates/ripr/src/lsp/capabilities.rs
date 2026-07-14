use super::uri::path_from_file_uri;
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, REFRESH_COMMAND,
};
use std::path::PathBuf;
use tower_lsp_server::ls_types::{
    CodeActionProviderCapability, CodeLensOptions, ExecuteCommandOptions, HoverProviderCapability,
    InitializeParams, InitializeResult, OneOf, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceRootResolution {
    Selected(PathBuf),
    Ambiguous(Vec<PathBuf>),
    Unavailable(String),
}

pub(super) fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            // Advisory codeLens: resolve is disabled; lenses are display-only
            // text hints citing the cached related-test count. No resolve
            // round-trip is needed (RIPR-SPEC-0099).
            code_lens_provider: Some(CodeLensOptions {
                resolve_provider: Some(false),
            }),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: vec![
                    REFRESH_COMMAND.to_string(),
                    COLLECT_CONTEXT_COMMAND.to_string(),
                    COLLECT_EVIDENCE_CONTEXT_COMMAND.to_string(),
                    COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                    COLLECT_REPAIR_PACKET_COMMAND.to_string(),
                    COLLECT_TOP_LIMITATION_COMMAND.to_string(),
                    COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
                ],
                ..ExecuteCommandOptions::default()
            }),
            workspace: Some(WorkspaceServerCapabilities {
                workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    change_notifications: Some(OneOf::Left(true)),
                }),
                ..WorkspaceServerCapabilities::default()
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
    reason = "rootUri remains the LSP compatibility fallback when workspaceFolders is absent"
)]
pub(super) fn root_from_initialize_params(params: &InitializeParams) -> WorkspaceRootResolution {
    if let Some(folders) = params.workspace_folders.as_ref() {
        if folders.is_empty() {
            return WorkspaceRootResolution::Unavailable(
                "the client explicitly reported no workspace folders".to_string(),
            );
        }
        if folders.len() > 1 {
            let mut candidates = Vec::with_capacity(folders.len());
            for folder in folders {
                let Some(path) = path_from_file_uri(&folder.uri) else {
                    return WorkspaceRootResolution::Unavailable(
                        "workspace folder URI is not a valid file URI".to_string(),
                    );
                };
                candidates.push(path);
            }
            return WorkspaceRootResolution::Ambiguous(candidates);
        }
        if let Some(folder) = folders.first() {
            return path_from_workspace_folder(folder);
        }
    }

    params.root_uri.as_ref().map_or_else(
        || {
            WorkspaceRootResolution::Unavailable(
                "the client did not provide a workspace folder or root URI".to_string(),
            )
        },
        |uri| {
            path_from_file_uri(uri).map_or_else(
                || {
                    WorkspaceRootResolution::Unavailable(
                        "root URI is not a valid file URI".to_string(),
                    )
                },
                WorkspaceRootResolution::Selected,
            )
        },
    )
}

fn path_from_workspace_folder(
    folder: &tower_lsp_server::ls_types::WorkspaceFolder,
) -> WorkspaceRootResolution {
    path_from_file_uri(&folder.uri).map_or_else(
        || {
            WorkspaceRootResolution::Unavailable(
                "workspace folder URI is not a valid file URI".to_string(),
            )
        },
        WorkspaceRootResolution::Selected,
    )
}
