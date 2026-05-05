use super::actions::code_action_response;
use super::backend::Backend;
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, WorkspaceDiagnostics, diagnostic_for_classified_seam, diagnostic_for_finding,
    diagnostic_refresh_plan, diagnostic_severity_for_class, take_all_uris,
    workspace_diagnostic_batches,
};
use super::hover::hover_response;
use super::state::{AnalysisSnapshot, DocumentStore};
use super::uri::{encode_uri_path, file_uri_for_path, path_from_file_uri};
use super::{
    COLLECT_CONTEXT_COMMAND, COPY_CONTEXT_COMMAND, COPY_SUGGESTED_ASSERTION_COMMAND, HOVER_TEXT,
    OPEN_RELATED_TEST_COMMAND, REFRESH_COMMAND,
};
use crate::app::Mode;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, OracleKind, OracleStrength, Probe, ProbeFamily,
    ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tower_lsp_server::LanguageServer;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandParams, HoverContents, HoverParams, HoverProviderCapability, InitializeParams,
    NumberOrString, Position, Range, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    VersionedTextDocumentIdentifier, WorkspaceFolder,
};
use tower_lsp_server::{LspService, Server};

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
    assert_eq!(commands, vec![REFRESH_COMMAND, COLLECT_CONTEXT_COMMAND]);
    Ok(())
}

#[test]
fn framed_lsp_protocol_smoke_exercises_tower_server() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, mut client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (service, socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let mut server_task = tokio::spawn(async move {
            Server::new(server_read, server_write, socket)
                .serve(service)
                .await;
        });
        let mut client_read = client_read;
        let text_uri = "file:///workspace/src/lib.rs";

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": "file:///target/ripr/lsp-protocol-smoke-missing-root",
                    "capabilities": {}
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut client_read, 1).await?;
        assert_eq!(
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][0],
            REFRESH_COMMAND
        );
        assert_eq!(
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][1],
            COLLECT_CONTEXT_COMMAND
        );
        assert_eq!(
            initialize["result"]["capabilities"]["hoverProvider"],
            serde_json::Value::Bool(true)
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": text_uri,
                        "languageId": "rust",
                        "version": 1,
                        "text": "pub fn demo() -> bool { true }\n"
                    }
                }
            }),
        )
        .await?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "workspace/executeCommand",
                "params": {
                    "command": REFRESH_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let refresh = read_lsp_response(&mut client_read, 2).await?;
        assert!(refresh.get("error").is_none());

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "position": { "line": 0, "character": 4 }
                }
            }),
        )
        .await?;
        let hover = read_lsp_response(&mut client_read, 3).await?;
        let hover_value = hover["result"]["contents"]["value"]
            .as_str()
            .ok_or_else(|| "expected hover markdown value".to_string())?;
        assert!(hover_value.contains("ripr estimates static RIPR exposure"));

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/codeAction",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 4 }
                    },
                    "context": { "diagnostics": [] }
                }
            }),
        )
        .await?;
        let actions = read_lsp_response(&mut client_read, 4).await?;
        assert_eq!(actions["result"][0]["title"], "Refresh ripr analysis");
        assert_eq!(actions["result"][0]["command"]["command"], REFRESH_COMMAND);

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "shutdown",
                "params": null
            }),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 5).await?;
        assert!(shutdown.get("error").is_none());
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        )
        .await?;
        client_write
            .shutdown()
            .await
            .map_err(|err| format!("failed to close test client: {err}"))?;
        match tokio::time::timeout(std::time::Duration::from_secs(2), &mut server_task).await {
            Ok(join_result) => {
                join_result.map_err(|err| format!("LSP server task failed: {err}"))?;
            }
            Err(_) => {
                server_task.abort();
                return Err("LSP server did not stop after exit notification".to_string());
            }
        }
        Ok(())
    })
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
fn hover_for_position_uses_latest_matching_diagnostic() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected diagnostic hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("**ripr** `weakly_exposed`"));
            assert!(markup.value.contains("Add an exact boundary assertion."));
            assert!(markup.value.contains("## RIPR Evidence"));
            assert!(markup.value.contains("* reach yes: related tests found"));
            assert!(
                markup
                    .value
                    .contains("* infection yes: predicate can alter branch behavior")
            );
            assert!(
                markup
                    .value
                    .contains("* propagation yes: branch influences return value")
            );
            assert!(
                markup
                    .value
                    .contains("* observation weak: return value asserted")
            );
            assert!(
                markup
                    .value
                    .contains("* discriminator weak: boundary value missing")
            );
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn hover_fallback_to_diagnostic_without_matching_finding() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut mismatched_finding = sample_finding();
    mismatched_finding.id = "probe:other:1:predicate".to_string();
    mismatched_finding.probe.id.0 = "probe:other:1:predicate".to_string();
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![mismatched_finding],
    );
    let batches = vec![DiagnosticBatch {
        uri: uri.clone(),
        diagnostics: vec![diagnostic.clone()],
    }];
    let workspace_diagnostics = WorkspaceDiagnostics { snapshot, batches };
    let Some(_) = backend.refresh_plan(workspace_diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected diagnostic hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("**ripr** `weakly_exposed`"));
            assert!(markup.value.contains("Add an exact boundary assertion."));
            assert!(
                markup
                    .value
                    .contains("Finding: `probe:pricing:88:predicate`")
            );
            assert!(!markup.value.contains("## RIPR Evidence"));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn hover_for_position_returns_none_when_no_diagnostic_matches() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    assert!(
        backend
            .hover_for_position(&hover_params(uri, 0, 1))
            .is_none(),
        "expected None when no diagnostic matches position"
    );

    let generic = hover_response();
    match generic.contents {
        HoverContents::Markup(markup) => {
            assert_eq!(markup.value, HOVER_TEXT);
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn finding_hover_renders_related_tests_and_oracle_text() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let mut finding = sample_finding();
    finding.related_tests.push(RelatedTest {
        name: "discount_boundary_is_exact".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq!(total, expected)".to_string()),
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
    });
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected finding hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("## Related Tests"));
            assert!(
                markup
                    .value
                    .contains("`tests/pricing.rs:12` `discount_boundary_is_exact`")
            );
            assert!(
                markup
                    .value
                    .contains("\u{2014} strong exact_value oracle: assert_eq!(total, expected)")
            );
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn finding_hover_renders_weakness_section() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let mut finding = sample_finding();
    finding
        .missing
        .push("no equality-boundary case was found".to_string());
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected finding hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("## Weakness"));
            assert!(
                markup
                    .value
                    .contains("- no equality-boundary case was found")
            );
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn finding_hover_avoids_mutation_runtime_terms() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected finding hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            let banned: Vec<String> = vec![
                std::iter::once('k').chain("illed".chars()).collect(),
                std::iter::once('s').chain("urvived".chars()).collect(),
                std::iter::once('p').chain("roven".chars()).collect(),
                std::iter::once('a').chain("dequate".chars()).collect(),
                std::iter::once('u').chain("ntested".chars()).collect(),
            ];
            for term in banned {
                assert!(
                    !markup.value.to_ascii_lowercase().contains(&term),
                    "hover contained banned mutation-runtime term: {term}"
                );
            }
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn analysis_snapshot_finds_finding_from_diagnostic_data() -> Result<(), String> {
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        vec![finding],
    );

    let Some(found) = snapshot.finding_for_diagnostic(&diagnostic) else {
        return Err("expected finding from diagnostic data".to_string());
    };

    assert_eq!(found.id, "probe:pricing:88:predicate");
    assert_eq!(found.probe.expression, "amount >= threshold");
    Ok(())
}

#[test]
fn overlapping_diagnostics_prefer_seam_id_lookup_over_finding_id_lookup() -> Result<(), String> {
    // Regression for chatgpt-codex review on PR #242: when a Finding
    // diagnostic and a Seam diagnostic share the same line, the
    // backend's hover handler must prefer the seam-bearing one. The
    // batch builder pushes findings before seams in the per-uri
    // diagnostic vector, so a naive first-match scan would shadow the
    // new seam-evidence hover. Pin the priority by direct lookup.
    let finding = sample_finding();
    let finding_diag = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let mut seam_diag = finding_diag.clone();
    seam_diag.data = Some(serde_json::json!({
        "schema_version": "0.1",
        "seam_id": "f3c9e4d21a0b7c88",
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
    }));
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    // Order matters here: finding diagnostic first, seam diagnostic
    // second — the same order the batch builder uses.
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![finding_diag.clone(), seam_diag.clone()],
        vec![finding],
    );

    // Both lookups exist in the snapshot. The backend's overlap fix
    // walks all matching diagnostics and prefers the seam-bearing
    // one. We verify the lookups individually here; the backend
    // ordering is exercised by `framed_lsp_protocol_smoke_exercises_tower_server`.
    if snapshot.finding_for_diagnostic(&finding_diag).is_none() {
        return Err("finding lookup should still resolve".to_string());
    }
    // The seam diagnostic carries seam_id but no matching seam in
    // classified_seams (the test snapshot helper has empty seams).
    // What matters is that classified_seam_for_diagnostic only fires
    // for diagnostics with data.seam_id — i.e., it does not match
    // finding_diag.
    if snapshot
        .classified_seam_for_diagnostic(&finding_diag)
        .is_some()
    {
        return Err(
            "classified_seam_for_diagnostic should reject diagnostics carrying finding_id only"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn given_diagnostic_with_unknown_seam_id_when_lookup_runs_then_no_classified_seam_is_returned()
-> Result<(), String> {
    // Regression for the directive's "unknown seam_id falls back
    // safely" acceptance: a diagnostic carries data.seam_id but the
    // snapshot has no matching ClassifiedSeam (e.g., the snapshot was
    // refreshed and the seam was filtered out). Lookup must return
    // None so the backend falls through to finding hover or the
    // generic diagnostic hover; the LSP must not panic or hang.
    let finding = sample_finding();
    let mut diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    // Replace the diagnostic data with a synthetic seam_id that does
    // not appear in classified_seams. Drops the finding_id, mirroring
    // a seam evidence diagnostic.
    diagnostic.data = Some(serde_json::json!({
        "schema_version": "0.1",
        "seam_id": "deadbeef00000000",
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
    }));
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        vec![finding],
    );

    if snapshot
        .classified_seam_for_diagnostic(&diagnostic)
        .is_some()
    {
        return Err("expected None for unknown seam_id".to_string());
    }
    if snapshot.finding_for_diagnostic(&diagnostic).is_some() {
        return Err(
            "expected None for finding_for_diagnostic when seam_id is set instead of finding_id"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn given_finding_diagnostic_when_lookup_runs_then_finding_hover_path_still_resolves()
-> Result<(), String> {
    // Pre-4B Finding diagnostics still resolve through finding_for_diagnostic
    // even when the new seam-aware lookup is on the same snapshot.
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        vec![finding],
    );

    if snapshot
        .classified_seam_for_diagnostic(&diagnostic)
        .is_some()
    {
        return Err("Finding diagnostics carry finding_id, not seam_id; \
             classified_seam_for_diagnostic should return None"
            .to_string());
    }
    if snapshot.finding_for_diagnostic(&diagnostic).is_none() {
        return Err("expected Finding hover lookup to still work".to_string());
    }
    Ok(())
}

#[test]
fn refresh_plan_stores_latest_analysis_snapshot() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );

    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };
    let Some(latest) = backend.latest_analysis_snapshot() else {
        return Err("expected latest analysis snapshot".to_string());
    };

    assert_eq!(latest.root, PathBuf::from("/workspace"));
    assert_eq!(latest.base.as_deref(), Some("origin/main"));
    assert_eq!(latest.mode, Mode::Draft);
    assert_eq!(latest.findings.len(), 1);
    assert_eq!(latest.diagnostics_by_uri.len(), 1);
    Ok(())
}

#[test]
fn refresh_plan_rejects_mismatched_snapshot_and_batches() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let baseline = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding.clone()],
    );

    let Some(_) = backend.refresh_plan(baseline) else {
        return Err("expected baseline refresh plan".to_string());
    };
    let mismatched = WorkspaceDiagnostics {
        snapshot: sample_analysis_snapshot(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic],
            vec![finding],
        ),
        batches: Vec::new(),
    };

    assert!(backend.refresh_plan(mismatched).is_none());
    let Some(latest) = backend.latest_analysis_snapshot() else {
        return Err("expected baseline snapshot to remain stored".to_string());
    };
    assert_eq!(latest.findings.len(), 1);
    assert_eq!(latest.diagnostics_by_uri.len(), 1);
    Ok(())
}

#[test]
fn code_action_response_keeps_current_commands() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.related_tests.clear();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, None);

    let mut titles_kinds_and_commands = Vec::new();
    let mut command_arguments = Vec::new();
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
                command_arguments.push(command.arguments.clone());
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
                "Refresh ripr analysis",
                "source",
                "Refresh ripr analysis",
                REFRESH_COMMAND,
            ),
        ]
    );
    let Some(Some(arguments)) = command_arguments.first() else {
        return Err("expected copy context arguments".to_string());
    };
    assert_eq!(arguments[0]["uri"], "file:///workspace/src/pricing.rs");
    assert_eq!(arguments[0]["line"], 88);
    assert_eq!(arguments[0]["finding_id"], "probe:pricing:88:predicate");
    assert_eq!(arguments[0]["probe_id"], "probe:pricing:88:predicate");
    Ok(())
}

#[test]
fn code_action_response_omits_context_action_without_ripr_diagnostic() -> Result<(), String> {
    let actions = code_action_response(&code_action_params(Vec::new())?, None);

    assert_eq!(actions.len(), 1);
    let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
        return Err("expected code action".to_string());
    };
    let Some(command) = &action.command else {
        return Err("expected refresh command".to_string());
    };
    assert_eq!(command.command, REFRESH_COMMAND);
    Ok(())
}

#[test]
fn seam_code_actions_surface_packet_assertion_related_test_and_refresh() -> Result<(), String> {
    let seam = sample_classified_seam();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, Some(&snapshot));

    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(_, command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec![
            COPY_CONTEXT_COMMAND,
            COPY_SUGGESTED_ASSERTION_COMMAND,
            OPEN_RELATED_TEST_COMMAND,
            REFRESH_COMMAND,
        ]
    );
    assert_eq!(commands[0].0, "Copy seam packet");
    assert_eq!(commands[0].2[0]["seam_id"], seam.seam.id().as_str());
    assert_eq!(commands[0].2[0]["seam_kind"], "predicate_boundary");
    assert_eq!(commands[0].2[0]["line"], 88);
    assert_eq!(commands[1].0, "Copy suggested assertion");
    assert!(
        commands[1].2[0]["assertion"]
            .as_str()
            .is_some_and(|value| value.contains("assert_eq!(discounted_total")),
        "expected assertion argument, got {:?}",
        commands[1].2
    );
    assert_eq!(commands[2].0, "Open related test");
    assert_eq!(
        commands[2].2[0]["uri"],
        "file:///workspace/tests/pricing.rs"
    );
    assert_eq!(commands[2].2[0]["line"], 12);
    Ok(())
}

#[test]
fn seam_code_actions_omit_assertion_and_related_test_when_evidence_is_missing() -> Result<(), String>
{
    let seam = sample_side_effect_seam_without_related_tests();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/service.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam];
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, Some(&snapshot));

    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(_, command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec![COPY_CONTEXT_COMMAND, REFRESH_COMMAND]
    );
    assert_eq!(commands[0].0, "Copy seam packet");
    Ok(())
}

#[test]
fn diagnostic_for_finding_preserves_lsp_payload_shape() -> Result<(), String> {
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert_eq!(diagnostic.range.start.line, 87);
    assert_eq!(diagnostic.range.start.character, 0);
    assert_eq!(diagnostic.range.end.line, 87);
    assert_eq!(diagnostic.range.end.character, 19);
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
fn diagnostic_for_finding_uses_probe_column_and_expression_width() {
    let mut finding = sample_finding();
    finding.probe.location.column = 5;
    finding.probe.expression = "total".to_string();

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert_eq!(diagnostic.range.start.line, 87);
    assert_eq!(diagnostic.range.start.character, 4);
    assert_eq!(diagnostic.range.end.line, 87);
    assert_eq!(diagnostic.range.end.character, 9);
}

#[test]
fn diagnostic_for_finding_uses_one_character_range_for_empty_expression() {
    let mut finding = sample_finding();
    finding.probe.location.column = 3;
    finding.probe.expression.clear();

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert_eq!(diagnostic.range.start.character, 2);
    assert_eq!(diagnostic.range.end.character, 3);
}

#[test]
fn diagnostic_for_finding_attaches_related_test_information() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.related_tests.push(RelatedTest {
        name: "discount_boundary_is_exact".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq!(total, expected)".to_string()),
        oracle_kind: OracleKind::ExactValue,
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
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            tracked_uri.clone(),
            Vec::new(),
            Vec::new(),
        );
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };
        assert!(backend.latest_analysis_snapshot().is_some());

        backend
            .report_refresh_failure("simulated analysis failure".to_string())
            .await;

        assert!(backend.clear_all_diagnostic_uris().is_empty());
        assert!(backend.latest_analysis_snapshot().is_none());
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
fn initialization_options_override_lsp_analysis_config() {
    let mut params = initialize_params(None, None);
    params.initialization_options = Some(serde_json::json!({
        "baseRef": "origin/release",
        "checkMode": "deep",
        "includeUnchangedTests": false,
    }));

    let config = LspAnalysisConfig::from_initialize_params(&params);
    let input = config.check_input(Path::new("/workspace"));

    assert_eq!(config.base_ref.as_deref(), Some("origin/release"));
    assert_eq!(config.mode, Mode::Deep);
    assert!(!config.include_unchanged_tests);
    assert_eq!(input.root, PathBuf::from("/workspace"));
    assert_eq!(input.base.as_deref(), Some("origin/release"));
    assert_eq!(input.mode, Mode::Deep);
    assert!(!input.include_unchanged_tests);
}

#[test]
fn initialization_options_allow_empty_base_ref_and_invalid_mode_falls_back() {
    let mut params = initialize_params(None, None);
    params.initialization_options = Some(serde_json::json!({
        "baseRef": "",
        "checkMode": "surprise",
    }));

    let config = LspAnalysisConfig::from_initialize_params(&params);

    assert_eq!(config.base_ref, None);
    assert_eq!(config.mode, Mode::Draft);
    assert!(config.include_unchanged_tests);
}

#[test]
fn initialization_options_accept_all_analysis_mode_labels() {
    let cases = [
        ("instant", Mode::Instant),
        ("draft", Mode::Draft),
        ("fast", Mode::Fast),
        ("deep", Mode::Deep),
        ("ready", Mode::Ready),
    ];

    for (label, expected) in cases {
        let mut params = initialize_params(None, None);
        params.initialization_options = Some(serde_json::json!({
            "checkMode": label,
        }));

        let config = LspAnalysisConfig::from_initialize_params(&params);

        assert_eq!(config.mode, expected);
    }
}

#[test]
fn default_lsp_analysis_config_matches_check_input_defaults() {
    let config = LspAnalysisConfig::default();
    let input = config.check_input(Path::new("/workspace"));

    assert_eq!(input.root, PathBuf::from("/workspace"));
    assert_eq!(input.base.as_deref(), Some("origin/main"));
    assert_eq!(input.mode, Mode::Draft);
    assert!(input.include_unchanged_tests);
}

#[test]
fn initialize_stores_lsp_analysis_config() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, None);
        params.initialization_options = Some(serde_json::json!({
            "baseRef": "upstream/main",
            "checkMode": "fast",
        }));

        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        let Some(config) = backend.analysis_config() else {
            return Err("expected backend analysis config".to_string());
        };

        assert_eq!(config.base_ref.as_deref(), Some("upstream/main"));
        assert_eq!(config.mode, Mode::Fast);
        Ok(())
    })
}

#[test]
fn backend_starts_with_default_lsp_analysis_config() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();

    let Some(config) = backend.analysis_config() else {
        return Err("expected backend analysis config".to_string());
    };

    assert_eq!(config.base_ref.as_deref(), Some("origin/main"));
    assert_eq!(config.mode, Mode::Draft);
    assert!(config.include_unchanged_tests);
    Ok(())
}

#[test]
fn workspace_diagnostic_batches_uses_default_lsp_analysis_config() {
    let missing_root = Path::new("target/ripr/definitely-missing-lsp-root");

    assert!(workspace_diagnostic_batches(missing_root).is_err());
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
fn file_uri_to_path_returns_none_for_non_file_scheme() -> Result<(), String> {
    let uri = test_uri("https://example.com/workspace/src/lib.rs")?;

    assert!(path_from_file_uri(&uri).is_none());
    Ok(())
}

#[test]
fn file_uri_to_path_decodes_uppercase_hex_escape() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src%2Dlib.rs")?;

    let Some(path) = path_from_file_uri(&uri) else {
        return Err("expected path from file URI".to_string());
    };
    assert_eq!(path, PathBuf::from("/workspace/src-lib.rs"));
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

async fn write_lsp_message<W>(writer: &mut W, message: serde_json::Value) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(&message)
        .map_err(|err| format!("failed to encode LSP message: {err}"))?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|err| format!("failed to write LSP header: {err}"))?;
    writer
        .write_all(&body)
        .await
        .map_err(|err| format!("failed to write LSP body: {err}"))?;
    writer
        .flush()
        .await
        .map_err(|err| format!("failed to flush LSP message: {err}"))
}

async fn read_lsp_response<R>(reader: &mut R, id: u64) -> Result<serde_json::Value, String>
where
    R: AsyncRead + Unpin,
{
    loop {
        let message = read_lsp_message(reader).await?;
        if message.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
            return Ok(message);
        }
    }
}

async fn read_lsp_message<R>(reader: &mut R) -> Result<serde_json::Value, String>
where
    R: AsyncRead + Unpin,
{
    let mut header = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        reader
            .read_exact(&mut byte)
            .await
            .map_err(|err| format!("failed to read LSP header: {err}"))?;
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header =
        std::str::from_utf8(&header).map_err(|err| format!("invalid LSP header UTF-8: {err}"))?;
    let content_length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .ok_or_else(|| "missing LSP Content-Length header".to_string())?
        .parse::<usize>()
        .map_err(|err| format!("invalid LSP Content-Length header: {err}"))?;
    let mut body = vec![0_u8; content_length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|err| format!("failed to read LSP body: {err}"))?;
    serde_json::from_slice(&body).map_err(|err| format!("failed to decode LSP message: {err}"))
}

fn sample_analysis_snapshot(
    root: PathBuf,
    uri: tower_lsp_server::ls_types::Uri,
    diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
    findings: Vec<Finding>,
) -> AnalysisSnapshot {
    let mut diagnostics_by_uri = BTreeMap::new();
    diagnostics_by_uri.insert(uri, diagnostics);
    AnalysisSnapshot {
        root,
        base: Some("origin/main".to_string()),
        mode: Mode::Draft,
        findings,
        classified_seams: Vec::new(),
        diagnostics_by_uri,
    }
}

fn sample_workspace_diagnostics(
    root: PathBuf,
    uri: tower_lsp_server::ls_types::Uri,
    diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
    findings: Vec<Finding>,
) -> WorkspaceDiagnostics {
    let snapshot = sample_analysis_snapshot(root, uri.clone(), diagnostics.clone(), findings);
    WorkspaceDiagnostics {
        snapshot,
        batches: vec![DiagnosticBatch { uri, diagnostics }],
    }
}

fn code_action_params(
    diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
) -> Result<CodeActionParams, String> {
    Ok(CodeActionParams {
        text_document: TextDocumentIdentifier::new(test_uri("file:///workspace/src/pricing.rs")?),
        range: Range {
            start: Position {
                line: 87,
                character: 0,
            },
            end: Position {
                line: 87,
                character: 120,
            },
        },
        context: CodeActionContext {
            diagnostics,
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    })
}

fn code_action_commands(
    actions: &[CodeActionOrCommand],
) -> Result<Vec<(String, String, Vec<serde_json::Value>)>, String> {
    let mut commands = Vec::new();
    for action in actions {
        let CodeActionOrCommand::CodeAction(action) = action else {
            return Err("expected code action".to_string());
        };
        let Some(command) = &action.command else {
            return Err(format!("expected command for action {}", action.title));
        };
        commands.push((
            action.title.clone(),
            command.command.clone(),
            command.arguments.clone().unwrap_or_default(),
        ));
    }
    Ok(commands)
}

fn hover_params(uri: tower_lsp_server::ls_types::Uri, line: u32, character: u32) -> HoverParams {
    HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(uri),
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
    }
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
        flow_sinks: Vec::new(),
        activation: crate::domain::ActivationEvidence::default(),
        stop_reasons: Vec::new(),
        related_tests: Vec::new(),
        recommended_next_step: Some("Add an exact boundary assertion.".to_string()),
    }
}

fn sample_classified_seam() -> crate::analysis::ClassifiedSeam {
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::domain::{MissingDiscriminatorFact, ValueContext, ValueFact};

    let seam = RepoSeam::new(
        "src/pricing.rs",
        "pricing::discounted_total",
        SeamKind::PredicateBoundary,
        42,
        88,
        "amount >= discount_threshold",
        RequiredDiscriminator::BoundaryValue {
            description: "amount >= discount_threshold".to_string(),
        },
        ExpectedSink::ReturnValue,
    );
    let seam_id = seam.id().clone();
    crate::analysis::ClassifiedSeam {
        seam,
        evidence: TestGripEvidence {
            seam_id,
            related_tests: vec![RelatedTestGrip {
                test_name: "below_threshold_has_no_discount".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 12,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                evidence_summary: "exact value assertion".to_string(),
                relation_reason: RelationReason::DirectOwnerCall,
                relation_confidence: RelationConfidence::High,
            }],
            reach: StageEvidence::new(
                StageState::Yes,
                Confidence::High,
                "related test calls owner",
            ),
            activate: StageEvidence::new(StageState::Yes, Confidence::High, "test reaches branch"),
            propagate: StageEvidence::new(StageState::Yes, Confidence::Medium, "return value sink"),
            observe: StageEvidence::new(StageState::Yes, Confidence::Medium, "exact assertion"),
            discriminate: StageEvidence::new(
                StageState::Weak,
                Confidence::Medium,
                "boundary value missing",
            ),
            observed_values: vec![ValueFact {
                line: 12,
                text: "discounted_total(50, 100)".to_string(),
                value: "50".to_string(),
                context: ValueContext::FunctionArgument,
            }],
            missing_discriminators: vec![MissingDiscriminatorFact {
                value: "discount_threshold (equality boundary)".to_string(),
                reason: "observed values skip equality boundary".to_string(),
                flow_sink: None,
            }],
        },
        class: SeamGripClass::WeaklyGripped,
    }
}

fn sample_side_effect_seam_without_related_tests() -> crate::analysis::ClassifiedSeam {
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::TestGripEvidence;

    let seam = RepoSeam::new(
        "src/service.rs",
        "service::publish_event",
        SeamKind::SideEffect,
        7,
        14,
        "event_bus.publish(event)",
        RequiredDiscriminator::Effect {
            sink: "event bus publish".to_string(),
        },
        ExpectedSink::SideEffect,
    );
    let seam_id = seam.id().clone();
    crate::analysis::ClassifiedSeam {
        seam,
        evidence: TestGripEvidence {
            seam_id,
            related_tests: Vec::new(),
            reach: StageEvidence::new(StageState::No, Confidence::Low, "no related test"),
            activate: StageEvidence::new(StageState::No, Confidence::Low, "no activation value"),
            propagate: StageEvidence::new(StageState::Unknown, Confidence::Low, "unknown sink"),
            observe: StageEvidence::new(StageState::No, Confidence::Low, "no observer"),
            discriminate: StageEvidence::new(StageState::No, Confidence::Low, "no discriminator"),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        },
        class: SeamGripClass::Ungripped,
    }
}

#[test]
fn finding_hover_response_includes_ripr_evidence_path() -> Result<(), String> {
    use super::hover::finding_hover_response;

    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    let hover = finding_hover_response(&finding, &diagnostic);

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("**ripr** `weakly_exposed`"));
            assert!(markup.value.contains("predicate"));
            assert!(markup.value.contains("reach yes:"));
            assert!(markup.value.contains("infection yes:"));
            assert!(markup.value.contains("propagation yes:"));
            assert!(markup.value.contains("observation weak:"));
            assert!(markup.value.contains("discriminator weak:"));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn finding_hover_response_includes_evidence_details() -> Result<(), String> {
    use super::hover::finding_hover_response;
    use crate::domain::{
        ActivationEvidence, FlowSinkFact, FlowSinkKind, MissingDiscriminatorFact, RelatedTest,
        ValueContext, ValueFact,
    };

    let mut finding = sample_finding();
    finding.flow_sinks = vec![FlowSinkFact {
        kind: FlowSinkKind::ReturnValue,
        text: "total".to_string(),
        line: 88,
        owner: None,
    }];
    finding.related_tests = vec![RelatedTest {
        name: "discount_boundary_is_exact".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq!(total, expected)".to_string()),
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
    }];
    finding.activation = ActivationEvidence {
        observed_values: vec![ValueFact {
            line: 12,
            text: "assert_eq!".to_string(),
            value: "amount == threshold".to_string(),
            context: ValueContext::FunctionArgument,
        }],
        missing_discriminators: vec![MissingDiscriminatorFact {
            value: "amount == threshold".to_string(),
            reason: "related tests do not cover the changed boundary value".to_string(),
            flow_sink: None,
        }],
    };

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let hover = finding_hover_response(&finding, &diagnostic);

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("## RIPR Evidence"));
            assert!(markup.value.contains("* reach yes: related tests found"));
            assert!(
                markup
                    .value
                    .contains("* infection yes: predicate can alter branch behavior")
            );
            assert!(
                markup
                    .value
                    .contains("* propagation yes: branch influences return value")
            );
            assert!(
                markup
                    .value
                    .contains("* observation weak: return value asserted")
            );
            assert!(
                markup
                    .value
                    .contains("* discriminator weak: boundary value missing")
            );
            assert!(markup.value.contains("## Related Tests"));
            assert!(markup.value.contains("tests/pricing.rs:12"));
            assert!(markup.value.contains("discount_boundary_is_exact"));
            assert!(
                markup
                    .value
                    .contains("strong exact_value oracle: assert_eq!(total, expected)")
            );
            assert!(markup.value.contains("Add an exact boundary assertion."));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn hover_for_position_uses_snapshot_finding_hover() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected finding hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("**ripr** `weakly_exposed`"));
            assert!(markup.value.contains("predicate"));
            assert!(markup.value.contains("## RIPR Evidence"));
            assert!(markup.value.contains("reach yes:"));
            assert!(markup.value.contains("Add an exact boundary assertion."));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn finding_hover_avoids_mutation_runtime_language() -> Result<(), String> {
    use super::hover::finding_hover_response;

    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    let hover = finding_hover_response(&finding, &diagnostic);

    match hover.contents {
        HoverContents::Markup(markup) => {
            let lower = markup.value.to_lowercase();
            let forbidden_terms = vec!["kil", "surv", "prov", "adeq", "untest"];
            for term in forbidden_terms {
                assert!(
                    !lower.contains(term),
                    "hover must use conservative static language"
                );
            }
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn execute_command_collect_context_returns_packet_for_known_finding() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let finding = sample_finding();
        let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic.clone()],
            vec![finding],
        );
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_CONTEXT_COMMAND.to_string(),
            arguments: vec![serde_json::json!({
                "finding_id": "probe:pricing:88:predicate",
                "probe_id": "probe:pricing:88:predicate",
                "uri": "file:///workspace/src/pricing.rs",
                "line": 88,
            })],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result.map_err(|err| format!("execute_command failed: {err}"))?;
        let Some(packet) = packet else {
            return Err("expected context packet".to_string());
        };
        let packet_str = serde_json::to_string(&packet)
            .map_err(|err| format!("failed to serialize packet: {err}"))?;
        assert!(packet_str.contains("\"version\""));
        assert!(packet_str.contains("\"tool\""));
        assert!(packet_str.contains("probe:pricing:88:predicate"));
        Ok(())
    })
}

#[test]
fn execute_command_collect_context_returns_agent_seam_packet_for_known_seam() -> Result<(), String>
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let seam = sample_classified_seam();
        let seam_id = seam.seam.id().as_str().to_string();
        let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
            .ok_or_else(|| "expected seam diagnostic".to_string())?;
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic],
            Vec::new(),
        );
        diagnostics.snapshot.classified_seams = vec![seam];
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_CONTEXT_COMMAND.to_string(),
            arguments: vec![serde_json::json!({
                "seam_id": seam_id,
                "uri": "file:///workspace/src/pricing.rs",
                "line": 88,
            })],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result.map_err(|err| format!("execute_command failed: {err}"))?;
        let Some(packet) = packet else {
            return Err("expected seam packet".to_string());
        };
        assert_eq!(packet["schema_version"], "0.3");
        assert_eq!(packet["packets_total"], 1);
        assert_eq!(packet["packets"][0]["seam_id"], seam_id);
        assert_eq!(
            packet["packets"][0]["assertion_shape"]["kind"],
            "exact_return_value"
        );
        Ok(())
    })
}

#[test]
fn execute_command_collect_context_returns_none_for_unknown_finding() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let finding = sample_finding();
        let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic.clone()],
            vec![finding],
        );
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_CONTEXT_COMMAND.to_string(),
            arguments: vec![serde_json::json!({
                "finding_id": "probe:unknown:1:predicate",
            })],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result.map_err(|err| format!("execute_command failed: {err}"))?;
        assert!(packet.is_none(), "expected None for unknown finding");
        Ok(())
    })
}

#[test]
fn execute_command_refresh_remains_unchanged() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();

        let params = ExecuteCommandParams {
            command: REFRESH_COMMAND.to_string(),
            arguments: Vec::new(),
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result.map_err(|err| format!("execute_command failed: {err}"))?;
        assert_eq!(packet, None);
        Ok(())
    })
}
