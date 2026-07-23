use super::agent_protocol::server_capability;
use super::state::WorkspaceFolderSet;
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
    HoverProviderCapability, InitializeParams, InitializeResult, OneOf, PositionEncodingKind,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceRootResolution {
    Selected(PathBuf),
    Ambiguous(Vec<PathBuf>),
    Unavailable(String),
}

/// The closed `CodeActionOptions.code_action_kinds` hierarchy (#1829,
/// RIPR-SPEC-0129). This is the single source of truth for the advertised
/// set: the code-action parity tests in `lsp/tests.rs` assert every emitted
/// kind against these constants so the advertisement and the emitters
/// cannot drift in the same direction. `quickfix.ripr` and
/// `source.ripr.verify` are advertised-but-unemitted (reserved); the emitters
/// in `lsp/actions.rs` use the `source.ripr.*` inspect/navigate/refresh
/// kinds.
pub(super) const ADVERTISED_CODE_ACTION_KINDS: [&str; 5] = [
    "quickfix.ripr",
    "source.ripr.inspect",
    "source.ripr.navigate",
    "source.ripr.verify",
    "source.ripr.refresh",
];

#[cfg(test)]
pub(super) fn initialize_result() -> InitializeResult {
    initialize_result_for_client(true, PositionEncodingKind::UTF16)
}

pub(super) fn initialize_result_for_client(
    supports_pull_diagnostics: bool,
    position_encoding: PositionEncodingKind,
) -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            position_encoding: Some(position_encoding),
            diagnostic_provider: supports_pull_diagnostics.then_some(
                DiagnosticServerCapabilities::Options(DiagnosticOptions {
                    inter_file_dependencies: true,
                    workspace_diagnostics: true,
                    ..DiagnosticOptions::default()
                }),
            ),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(
                    ADVERTISED_CODE_ACTION_KINDS
                        .into_iter()
                        .map(CodeActionKind::new)
                        .collect(),
                ),
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

/// How session configuration reaches the server for the five governed
/// session keys (#2031, RIPR-SPEC-0136). Negotiated once at `initialize`
/// from client capabilities only — never inferred from the client name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigurationMode {
    /// The client answers `workspace/configuration`; the server pulls the
    /// bounded `ripr` section and re-pulls on `workspace/didChangeConfiguration`.
    Pull,
    /// The client cannot answer `workspace/configuration` but advertises
    /// `workspace/didChangeConfiguration`; pushed values keep applying.
    PushFallback,
    /// The client neither pulls nor advertises push support; initialization
    /// options are the only client-supplied settings.
    InitializationOnly,
}

impl ConfigurationMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pull => "pull",
            Self::PushFallback => "push_fallback",
            Self::InitializationOnly => "initialization_only",
        }
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

/// Retain the canonical workspace-folder set at `initialize` (#2036,
/// RIPR-SPEC-0139) from the same inputs `root_from_initialize_params`
/// resolves: `workspaceFolders` when present, otherwise the documented
/// `rootUri` fallback. Entries canonicalize (dedup + path order) so later
/// deltas and reconciliations compare order-insensitively. An unparseable
/// list yields an empty set; the root resolution above stays the authority
/// that reports the typed unavailable state for it.
#[expect(
    deprecated,
    reason = "rootUri remains the LSP compatibility fallback when workspaceFolders is absent"
)]
pub(super) fn workspace_folder_set_from_initialize_params(
    params: &InitializeParams,
) -> WorkspaceFolderSet {
    if let Some(folders) = params.workspace_folders.as_ref() {
        return WorkspaceFolderSet::from_folder_list(folders).unwrap_or_default();
    }
    params
        .root_uri
        .as_ref()
        .map_or_else(WorkspaceFolderSet::default, |uri| {
            WorkspaceFolderSet::from_folder_list(&[tower_lsp_server::ls_types::WorkspaceFolder {
                uri: uri.clone(),
                name: "root".to_string(),
            }])
            .unwrap_or_default()
        })
}

#[cfg(test)]
mod tests {
    use super::super::client_features::ClientFeatureProfile;
    use super::*;
    use tower_lsp_server::ls_types::{
        DiagnosticClientCapabilities, DiagnosticWorkspaceClientCapabilities,
        GeneralClientCapabilities, TextDocumentClientCapabilities, WindowClientCapabilities,
        WorkspaceClientCapabilities,
    };

    fn params_with_position_encodings(
        encodings: Option<Vec<PositionEncodingKind>>,
    ) -> InitializeParams {
        let mut params = InitializeParams::default();
        params.capabilities.general = Some(GeneralClientCapabilities {
            position_encodings: encodings,
            ..GeneralClientCapabilities::default()
        });
        params
    }

    #[test]
    fn negotiate_prefers_utf16_when_the_client_supports_it() {
        let params = params_with_position_encodings(Some(vec![
            PositionEncodingKind::UTF8,
            PositionEncodingKind::UTF16,
        ]));
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&params).selected_position_encoding,
            PositionEncodingKind::UTF16
        );
    }

    #[test]
    fn negotiate_selects_utf8_when_the_client_advertises_only_utf8() {
        let params = params_with_position_encodings(Some(vec![PositionEncodingKind::UTF8]));
        assert_eq!(
            ClientFeatureProfile::from_initialize_params(&params).selected_position_encoding,
            PositionEncodingKind::UTF8
        );
    }

    #[test]
    fn negotiate_defaults_to_utf16_when_the_client_advertises_nothing() {
        for params in [
            params_with_position_encodings(None),
            params_with_position_encodings(Some(vec![])),
            InitializeParams::default(),
        ] {
            assert_eq!(
                ClientFeatureProfile::from_initialize_params(&params).selected_position_encoding,
                PositionEncodingKind::UTF16
            );
        }
    }

    #[test]
    fn initialize_result_echoes_the_negotiated_position_encoding() {
        let result = initialize_result_for_client(true, PositionEncodingKind::UTF8);
        assert_eq!(
            result.capabilities.position_encoding,
            Some(PositionEncodingKind::UTF8)
        );
    }

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
        if initialize_result_for_client(false, PositionEncodingKind::UTF16)
            .capabilities
            .diagnostic_provider
            .is_some()
        {
            return Err("push-only client received a pull provider".to_string());
        }
        if initialize_result_for_client(true, PositionEncodingKind::UTF16)
            .capabilities
            .diagnostic_provider
            .is_none()
        {
            return Err("pull-capable client lost pull provider".to_string());
        }
        Ok(())
    }

    #[test]
    fn work_done_progress_requires_window_capability() -> Result<(), String> {
        let mut capable = InitializeParams::default();
        capable.capabilities.window = Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..WindowClientCapabilities::default()
        });
        if !ClientFeatureProfile::from_initialize_params(&capable).work_done_progress {
            return Err("window.workDoneProgress=true must enable progress".to_string());
        }

        let mut declined = InitializeParams::default();
        declined.capabilities.window = Some(WindowClientCapabilities {
            work_done_progress: Some(false),
            ..WindowClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&declined).work_done_progress
            || ClientFeatureProfile::from_initialize_params(&InitializeParams::default())
                .work_done_progress
        {
            return Err("missing or declined window.workDoneProgress must disable progress".into());
        }
        Ok(())
    }

    #[test]
    fn code_lens_refresh_follows_workspace_capability() -> Result<(), String> {
        let mut supported = InitializeParams::default();
        supported.capabilities.workspace = Some(WorkspaceClientCapabilities {
            code_lens: Some(
                tower_lsp_server::ls_types::CodeLensWorkspaceClientCapabilities {
                    refresh_support: Some(true),
                },
            ),
            ..WorkspaceClientCapabilities::default()
        });
        if !ClientFeatureProfile::from_initialize_params(&supported).code_lens_refresh {
            return Err("workspace.codeLens.refreshSupport=true must enable refresh".to_string());
        }

        let mut declined = InitializeParams::default();
        declined.capabilities.workspace = Some(WorkspaceClientCapabilities {
            code_lens: Some(
                tower_lsp_server::ls_types::CodeLensWorkspaceClientCapabilities {
                    refresh_support: Some(false),
                },
            ),
            ..WorkspaceClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&declined).code_lens_refresh {
            return Err("workspace.codeLens.refreshSupport=false must disable refresh".to_string());
        }

        let mut present_without_flag = InitializeParams::default();
        present_without_flag.capabilities.workspace = Some(WorkspaceClientCapabilities {
            code_lens: Some(
                tower_lsp_server::ls_types::CodeLensWorkspaceClientCapabilities {
                    refresh_support: None,
                },
            ),
            ..WorkspaceClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&present_without_flag).code_lens_refresh
            || ClientFeatureProfile::from_initialize_params(&InitializeParams::default())
                .code_lens_refresh
        {
            return Err(
                "absent workspace.codeLens.refreshSupport must disable refresh".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn configuration_mode_follows_workspace_capabilities() -> Result<(), String> {
        let mut pull = InitializeParams::default();
        pull.capabilities.workspace = Some(WorkspaceClientCapabilities {
            configuration: Some(true),
            ..WorkspaceClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&pull).configuration_mode
            != ConfigurationMode::Pull
        {
            return Err("workspace.configuration=true must negotiate pull mode".to_string());
        }

        let mut declined = InitializeParams::default();
        declined.capabilities.workspace = Some(WorkspaceClientCapabilities {
            configuration: Some(false),
            ..WorkspaceClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&declined).configuration_mode
            != ConfigurationMode::InitializationOnly
        {
            return Err("workspace.configuration=false must not negotiate pull mode".to_string());
        }

        let mut push = InitializeParams::default();
        push.capabilities.workspace = Some(WorkspaceClientCapabilities {
            did_change_configuration: Some(
                tower_lsp_server::ls_types::DidChangeConfigurationClientCapabilities {
                    dynamic_registration: Some(true),
                },
            ),
            ..WorkspaceClientCapabilities::default()
        });
        if ClientFeatureProfile::from_initialize_params(&push).configuration_mode
            != ConfigurationMode::PushFallback
        {
            return Err(
                "didChangeConfiguration support without pull must negotiate push fallback"
                    .to_string(),
            );
        }

        if ClientFeatureProfile::from_initialize_params(&InitializeParams::default())
            .configuration_mode
            != ConfigurationMode::InitializationOnly
        {
            return Err("empty capabilities must negotiate initialization-only mode".to_string());
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
        let profile = ClientFeatureProfile::from_initialize_params(&pull);
        if !profile.pull_diagnostics || profile.diagnostic_refresh {
            return Err("pull-only capability was classified incorrectly".to_string());
        }

        let mut push = InitializeParams::default();
        push.capabilities.workspace = Some(WorkspaceClientCapabilities {
            diagnostics: Some(DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..WorkspaceClientCapabilities::default()
        });
        let profile = ClientFeatureProfile::from_initialize_params(&push);
        if profile.pull_diagnostics || !profile.diagnostic_refresh {
            return Err("push refresh capability was classified incorrectly".to_string());
        }

        let neither = ClientFeatureProfile::from_initialize_params(&InitializeParams::default());
        if neither.pull_diagnostics || neither.diagnostic_refresh {
            return Err("empty capabilities were classified as pull-capable".to_string());
        }
        Ok(())
    }
}
