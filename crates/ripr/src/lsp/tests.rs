use super::actions::code_action_response;
use super::backend::Backend;
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::diagnostics::{
    DiagnosticBatch, diagnostic_for_finding, diagnostic_refresh_plan,
    diagnostic_severity_for_class, take_all_uris,
};
use super::hover::hover_response;
use super::state::DocumentStore;
use super::uri::{encode_uri_path, file_uri_for_path, path_from_file_uri};
use super::{COPY_CONTEXT_COMMAND, HOVER_TEXT, REFRESH_COMMAND};
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
    RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use tower_lsp_server::LspService;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, HoverContents, HoverProviderCapability,
    InitializeParams, NumberOrString, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind,
    VersionedTextDocumentIdentifier, WorkspaceFolder,
};

#[test]
fn initialize_result_exposes_existing_lsp_capabilities() -> Result<(), String> {
    let result = initialize_result();

    assert_eq!(
        result.capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
    );
    assert_eq!(
        result.capabilities.hover_provider,
        Some(HoverProviderCapability::Simple(true))
    );
    let Some(provider) = result.capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };
    let commands = provider.commands;
    assert_eq!(commands, vec![REFRESH_COMMAND]);
    Ok(())
}

#[test]
fn hover_response_keeps_current_guidance_text() -> Result<(), String> {
    let hover = hover_response();

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert_eq!(markup.value, HOVER_TEXT);
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn code_action_response_keeps_current_commands() -> Result<(), String> {
    let actions = code_action_response();

    let mut titles_kinds_and_commands = Vec::new();
    for action in &actions {
        match action {
            CodeActionOrCommand::CodeAction(action) => {
                let Some(command) = &action.command else {
                    return Err("expected code action command".to_string());
                };
                let Some(kind) = &action.kind else {
                    return Err("expected code action kind".to_string());
                };
                titles_kinds_and_commands.push((
                    action.title.as_str(),
                    kind.as_str(),
                    command.title.as_str(),
                    command.command.as_str(),
                ));
            }
            CodeActionOrCommand::Command(_) => {
                return Err("expected code action".to_string());
            }
        }
    }

    assert_eq!(
        titles_kinds_and_commands,
        vec![
            (
                "Copy ripr context packet",
                "quickfix",
                "Copy ripr context",
                COPY_CONTEXT_COMMAND,
            ),
            (
                "Run ripr check",
                "source",
                "Refresh ripr analysis",
                REFRESH_COMMAND,
            ),
        ]
    );
    Ok(())
}

#[test]
fn diagnostic_for_finding_preserves_lsp_payload_shape() -> Result<(), String> {
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert_eq!(diagnostic.range.start.line, 87);
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("weakly_exposed".to_string()))
    );
    assert_eq!(diagnostic.source.as_deref(), Some("ripr"));
    assert_eq!(diagnostic.message, "Add an exact boundary assertion.");
    let Some(data) = diagnostic.data else {
        return Err("expected diagnostic data".to_string());
    };
    assert_eq!(data["schema_version"], "0.1");
    assert_eq!(data["finding_id"], "probe:pricing:88:predicate");
    assert_eq!(data["probe_id"], "probe:pricing:88:predicate");
    assert_eq!(data["classification"], "weakly_exposed");
    assert_eq!(data["probe_family"], "predicate");
    assert_eq!(data["confidence"], 0.75);
    assert_eq!(data["source_range"]["file"], "src/pricing.rs");
    assert_eq!(data["source_range"]["line"], 88);
    assert_eq!(data["source_range"]["column"], 1);
    Ok(())
}

#[test]
fn diagnostic_for_finding_attaches_related_test_information() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.related_tests.push(RelatedTest {
        name: "discount_boundary_is_exact".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq!(total, expected)".to_string()),
        oracle_strength: OracleStrength::Strong,
    });

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let Some(related) = diagnostic.related_information else {
        return Err("expected related diagnostic information".to_string());
    };

    assert_eq!(related.len(), 1);
    assert_eq!(
        related[0].location.uri.as_str(),
        "file:///workspace/tests/pricing.rs"
    );
    assert_eq!(related[0].location.range.start.line, 11);
    assert_eq!(
        related[0].message,
        "Related test `discount_boundary_is_exact` has strong oracle: assert_eq!(total, expected)"
    );
    Ok(())
}

#[test]
fn diagnostic_severity_tracks_static_exposure_class() {
    let cases = [
        (ExposureClass::Exposed, DiagnosticSeverity::INFORMATION),
        (ExposureClass::WeaklyExposed, DiagnosticSeverity::WARNING),
        (
            ExposureClass::ReachableUnrevealed,
            DiagnosticSeverity::WARNING,
        ),
        (ExposureClass::NoStaticPath, DiagnosticSeverity::WARNING),
        (ExposureClass::InfectionUnknown, DiagnosticSeverity::WARNING),
        (
            ExposureClass::PropagationUnknown,
            DiagnosticSeverity::INFORMATION,
        ),
        (
            ExposureClass::StaticUnknown,
            DiagnosticSeverity::INFORMATION,
        ),
    ];

    for (class, expected) in cases {
        assert_eq!(diagnostic_severity_for_class(&class), expected);
    }
}

#[test]
fn diagnostic_refresh_plan_clears_stale_previous_uris() -> Result<(), String> {
    let stale_uri = test_uri("file:///workspace/src/stale.rs")?;
    let current_uri = test_uri("file:///workspace/src/current.rs")?;
    let mut previous_uris = BTreeSet::new();
    previous_uris.insert(stale_uri.clone());
    previous_uris.insert(current_uri.clone());

    let plan = diagnostic_refresh_plan(
        &previous_uris,
        vec![DiagnosticBatch {
            uri: current_uri.clone(),
            diagnostics: Vec::new(),
        }],
    );

    assert_eq!(plan.publish_batches.len(), 1);
    assert_eq!(plan.publish_batches[0].uri, current_uri);
    assert_eq!(plan.clear_uris, vec![stale_uri]);
    assert_eq!(plan.current_uris.len(), 1);
    Ok(())
}

#[test]
fn take_all_uris_returns_and_clears_previous_diagnostic_uris() -> Result<(), String> {
    let first_uri = test_uri("file:///workspace/src/first.rs")?;
    let second_uri = test_uri("file:///workspace/src/second.rs")?;
    let mut uris = BTreeSet::new();
    uris.insert(first_uri.clone());
    uris.insert(second_uri.clone());

    let cleared = take_all_uris(&mut uris);

    assert_eq!(cleared, vec![first_uri, second_uri]);
    assert!(uris.is_empty());
    Ok(())
}

#[test]
fn refresh_failure_reports_and_clears_tracked_diagnostics() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let tracked_uri = test_uri("file:///workspace/src/stale.rs")?;
        let Some(_) = backend.refresh_plan(vec![DiagnosticBatch {
            uri: tracked_uri.clone(),
            diagnostics: Vec::new(),
        }]) else {
            return Err("expected refresh plan".to_string());
        };

        backend
            .report_refresh_failure("simulated analysis failure".to_string())
            .await;

        assert!(backend.clear_all_diagnostic_uris().is_empty());
        Ok(())
    })
}

#[test]
fn refresh_generation_marks_older_requests_stale() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();

    let Some(first) = backend.next_refresh_generation() else {
        return Err("expected first refresh generation".to_string());
    };
    assert!(backend.is_current_refresh_generation(first));

    let Some(second) = backend.next_refresh_generation() else {
        return Err("expected second refresh generation".to_string());
    };

    assert!(!backend.is_current_refresh_generation(first));
    assert!(backend.is_current_refresh_generation(second));
    Ok(())
}

#[test]
fn refresh_diagnostics_advances_generation_before_analysis() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) =
            LspService::new(|client| Backend::new(client, PathBuf::from("Cargo.toml")));
        let backend = service.inner();

        backend.refresh_diagnostics().await;

        assert!(backend.is_current_refresh_generation(1));
        Ok(())
    })
}

#[test]
fn document_store_tracks_open_change_and_close() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let mut store = DocumentStore::default();

    store.open(DidOpenTextDocumentParams {
        text_document: TextDocumentItem::new(
            uri.clone(),
            "rust".to_string(),
            1,
            "fn old() {}".to_string(),
        ),
    });

    let Some(opened) = store.documents.get(&uri) else {
        return Err("expected opened document".to_string());
    };
    assert_eq!(opened.path, PathBuf::from("/workspace/src/lib.rs"));
    assert_eq!(opened.version, Some(1));
    assert_eq!(opened.text, "fn old() {}");

    store.change(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 2),
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "fn new() {}".to_string(),
        }],
    });

    let Some(changed) = store.documents.get(&uri) else {
        return Err("expected changed document".to_string());
    };
    assert_eq!(changed.version, Some(2));
    assert_eq!(changed.text, "fn new() {}");

    store.close(DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier::new(uri.clone()),
    });

    assert!(!store.documents.contains_key(&uri));
    Ok(())
}

#[test]
fn document_store_creates_document_from_full_change_when_missing() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let mut store = DocumentStore::default();

    store.change(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 7),
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "fn discovered() {}".to_string(),
        }],
    });

    let Some(document) = store.documents.get(&uri) else {
        return Err("expected document from full change".to_string());
    };
    assert_eq!(document.version, Some(7));
    assert_eq!(document.text, "fn discovered() {}");
    Ok(())
}

#[test]
fn initialize_root_prefers_first_workspace_folder() -> Result<(), String> {
    let fallback = PathBuf::from("/fallback");
    let params = initialize_params(
        Some(vec![
            WorkspaceFolder {
                uri: test_uri("file:///workspace/main")?,
                name: "main".to_string(),
            },
            WorkspaceFolder {
                uri: test_uri("file:///workspace/other")?,
                name: "other".to_string(),
            },
        ]),
        Some(test_uri("file:///workspace/root-uri")?),
    );

    let root = root_from_initialize_params(&params, &fallback);

    assert_eq!(root, PathBuf::from("/workspace/main"));
    Ok(())
}

#[test]
fn initialize_root_uses_root_uri_when_workspace_folders_are_missing() -> Result<(), String> {
    let fallback = PathBuf::from("/fallback");
    let params = initialize_params(None, Some(test_uri("file:///workspace/root-uri")?));

    let root = root_from_initialize_params(&params, &fallback);

    assert_eq!(root, PathBuf::from("/workspace/root-uri"));
    Ok(())
}

#[test]
fn initialize_root_falls_back_to_process_cwd_when_no_lsp_root_exists() {
    let fallback = PathBuf::from("/fallback");
    let params = initialize_params(None, None);

    let root = root_from_initialize_params(&params, &fallback);

    assert_eq!(root, fallback);
}

#[test]
fn file_uri_to_path_decodes_spaces_and_windows_drive_prefix() -> Result<(), String> {
    let uri = test_uri(&format!("file:///{}{}", "C%3A", "/path/to/ripr%20repo"))?;

    let Some(path) = path_from_file_uri(&uri) else {
        return Err("expected path from file URI".to_string());
    };

    assert_eq!(
        path,
        PathBuf::from(format!("{}{}", "C:", "/path/to/ripr repo"))
    );
    Ok(())
}

#[test]
fn file_uri_for_path_uses_valid_encoded_file_uri() -> Result<(), String> {
    let uri = file_uri_for_path(&PathBuf::from("src lib.rs"))?;

    assert_eq!(uri.as_str(), "file:///src%20lib.rs");
    Ok(())
}

#[test]
fn uri_path_encoding_preserves_path_syntax_and_escapes_spaces() {
    assert_eq!(
        encode_uri_path("workspace/src lib.rs"),
        "workspace/src%20lib.rs"
    );
}

fn test_uri(uri: &str) -> Result<tower_lsp_server::ls_types::Uri, String> {
    uri.parse::<tower_lsp_server::ls_types::Uri>()
        .map_err(|err| format!("failed to parse test URI: {err}"))
}

#[allow(deprecated)]
fn initialize_params(
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    root_uri: Option<tower_lsp_server::ls_types::Uri>,
) -> InitializeParams {
    InitializeParams {
        workspace_folders,
        root_uri,
        ..InitializeParams::default()
    }
}

fn sample_finding() -> Finding {
    Finding {
        id: "probe:pricing:88:predicate".to_string(),
        probe: Probe {
            id: ProbeId("probe:pricing:88:predicate".to_string()),
            location: SourceLocation {
                file: PathBuf::from("src/pricing.rs"),
                line: 88,
                column: 1,
            },
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: None,
            expression: "amount >= threshold".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        },
        class: ExposureClass::WeaklyExposed,
        ripr: RiprEvidence {
            reach: StageEvidence::new(StageState::Yes, Confidence::High, "related tests found"),
            infect: StageEvidence::new(
                StageState::Yes,
                Confidence::High,
                "predicate can alter branch behavior",
            ),
            propagate: StageEvidence::new(
                StageState::Yes,
                Confidence::Medium,
                "branch influences return value",
            ),
            reveal: RevealEvidence {
                observe: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "return value asserted",
                ),
                discriminate: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "boundary value missing",
                ),
            },
        },
        confidence: 0.75,
        evidence: Vec::new(),
        missing: Vec::new(),
        stop_reasons: Vec::new(),
        related_tests: Vec::new(),
        recommended_next_step: Some("Add an exact boundary assertion.".to_string()),
    }
}
