use super::agent_protocol::server_capability;
use super::uri::path_from_file_uri;
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, REFRESH_COMMAND,
};
use std::path::PathBuf;
use tower_lsp_server::ls_types::{
    CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CodeLensOptions,
    DiagnosticOptions, DiagnosticServerCapabilities, ExecuteCommandOptions,
    HoverProviderCapability, InitializeParams, InitializeResult, OneOf, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceRootResolution {
    Selected(PathBuf),
    Ambiguous(Vec<PathBuf>),
    Unavailable(String),
}

#[cfg(test)]
pub(super) fn initialize_result() -> InitializeResult {
    initialize_result_for_client(true)
}

pub(super) fn initialize_result_for_client(supports_pull_diagnostics: bool) -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            diagnostic_provider: supports_pull_diagnostics.then_some(
                DiagnosticServerCapabilities::Options(DiagnosticOptions {
                    inter_file_dependencies: true,
                    workspace_diagnostics: true,
                    ..DiagnosticOptions::default()
                }),
            ),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![
                    CodeActionKind::new("quickfix.ripr"),
                    CodeActionKind::new("source.ripr.inspect"),
                    CodeActionKind::new("source.ripr.navigate"),
                    CodeActionKind::new("source.ripr.verify"),
                    CodeActionKind::new("source.ripr.refresh"),
                ]),
                resolve_provider: Some(false),
                ..CodeActionOptions::default()
            })),
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
            experimental: Some(server_capability()),
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

pub(super) fn client_supports_pull_diagnostics(params: &InitializeParams) -> bool {
    params
        .capabilities
        .text_document
        .as_ref()
        .and_then(|text_document| text_document.diagnostic.as_ref())
        .is_some()
}

pub(super) fn client_supports_diagnostic_refresh(params: &InitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.diagnostics.as_ref())
        .and_then(|diagnostics| diagnostics.refresh_support)
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::{
        DiagnosticClientCapabilities, DiagnosticWorkspaceClientCapabilities,
        TextDocumentClientCapabilities, WorkspaceClientCapabilities,
    };

    #[test]
    fn initialize_result_advertises_the_fail_closed_agent_capability() -> Result<(), String> {
        let result = initialize_result();
        let experimental = result
            .capabilities
            .experimental
            .ok_or_else(|| "expected experimental LSP capabilities".to_string())?;
        if experimental != server_capability() {
            return Err("initialize capability drifted from the protocol authority".to_string());
        }
        Ok(())
    }

    #[test]
    fn initialize_result_advertises_pull_diagnostic_provider() -> Result<(), String> {
        let provider = initialize_result()
            .capabilities
            .diagnostic_provider
            .ok_or_else(|| "pull diagnostic provider was not advertised".to_string())?;
        let DiagnosticServerCapabilities::Options(options) = provider else {
            return Err("expected static diagnostic options".to_string());
        };
        if !options.inter_file_dependencies || !options.workspace_diagnostics {
            return Err("diagnostic provider lost workspace dependency support".to_string());
        }
        Ok(())
    }

    #[test]
    fn push_only_client_does_not_receive_pull_provider_advertisement() -> Result<(), String> {
        if initialize_result_for_client(false)
            .capabilities
            .diagnostic_provider
            .is_some()
        {
            return Err("push-only client received a pull provider".to_string());
        }
        if initialize_result_for_client(true)
            .capabilities
            .diagnostic_provider
            .is_none()
        {
            return Err("pull-capable client lost pull provider".to_string());
        }
        Ok(())
    }

    #[test]
    fn pull_mode_requires_text_document_diagnostic_capability() -> Result<(), String> {
        let mut pull = InitializeParams::default();
        pull.capabilities.text_document = Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..TextDocumentClientCapabilities::default()
        });
        if !client_supports_pull_diagnostics(&pull) || client_supports_diagnostic_refresh(&pull) {
            return Err("pull-only capability was classified incorrectly".to_string());
        }

        let mut push = InitializeParams::default();
        push.capabilities.workspace = Some(WorkspaceClientCapabilities {
            diagnostics: Some(DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..WorkspaceClientCapabilities::default()
        });
        if client_supports_pull_diagnostics(&push) || !client_supports_diagnostic_refresh(&push) {
            return Err("push refresh capability was classified incorrectly".to_string());
        }

        let neither = InitializeParams::default();
        if client_supports_pull_diagnostics(&neither)
            || client_supports_diagnostic_refresh(&neither)
        {
            return Err("empty capabilities were classified as pull-capable".to_string());
        }
        Ok(())
    }
}
