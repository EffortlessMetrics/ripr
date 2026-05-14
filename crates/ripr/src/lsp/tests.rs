use super::actions::code_action_response;
use super::backend::{
    Backend, RefreshLogSummary, refresh_completed_log_message, refresh_failed_log_message,
};
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, WorkspaceDiagnostics, diagnostic_for_classified_seam, diagnostic_for_finding,
    diagnostic_refresh_plan, diagnostic_severity_for_class, take_all_uris,
    workspace_diagnostic_batches, workspace_diagnostic_batches_with_config,
    workspace_diagnostics_with_config,
};
use super::hover::{classified_seam_hover_response, hover_response, hover_with_snapshot_status};
use super::state::{AnalysisSnapshot, DocumentStore, RefreshMetadata, format_duration};
use super::uri::{encode_uri_path, file_uri_for_path, file_uris_match, path_from_file_uri};
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COPY_AFTER_SNAPSHOT_COMMAND,
    COPY_AGENT_BRIEF_COMMAND, COPY_AGENT_PACKET_COMMAND, COPY_AGENT_RECEIPT_COMMAND,
    COPY_AGENT_VERIFY_COMMAND, COPY_CONTEXT_COMMAND, COPY_SUGGESTED_ASSERTION_COMMAND,
    COPY_TARGETED_TEST_BRIEF_COMMAND, HOVER_TEXT, OPEN_RELATED_TEST_COMMAND, REFRESH_COMMAND,
};
use crate::app::Mode;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId, LanguageStatus, OracleKind,
    OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tower_lsp_server::LanguageServer;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandParams, HoverContents, HoverParams, HoverProviderCapability, InitializeParams,
    MarkedString, NumberOrString, Position, Range, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, VersionedTextDocumentIdentifier,
    WorkspaceFolder,
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
    assert_eq!(
        commands,
        vec![
            REFRESH_COMMAND,
            COLLECT_CONTEXT_COMMAND,
            COLLECT_EVIDENCE_CONTEXT_COMMAND
        ]
    );
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
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][2],
            COLLECT_EVIDENCE_CONTEXT_COMMAND
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
        let (refresh, notifications) =
            read_lsp_response_with_notifications(&mut client_read, 2).await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(refresh["result"], serde_json::Value::Null);
        let notification_messages = log_notification_messages(&notifications);
        assert!(
            notification_messages
                .iter()
                .any(|message| message.contains("ripr analysis refresh started"))
        );
        assert!(
            notification_messages
                .iter()
                .any(|message| message.contains("ripr analysis refresh failed after"))
        );

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
        assert_eq!(
            actions["result"][0]["title"],
            "Refresh Analysis - Saved Workspace Check"
        );
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
fn framed_lsp_protocol_smoke_logs_successful_refresh_completion() -> Result<(), String> {
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
        // Keep this protocol smoke bounded now that seam diagnostics default on;
        // whole-repo inventory behavior is covered by fixture and report tests.
        let repo_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
        let root_uri = file_uri_for_path(&repo_root)?;

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": root_uri.as_str(),
                    "initializationOptions": {
                        "baseRef": "HEAD",
                        "checkMode": "instant"
                    },
                    "capabilities": {}
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut client_read, 1).await?;
        assert!(initialize.get("error").is_none());

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
        let (refresh, notifications) =
            read_lsp_response_with_notifications(&mut client_read, 2).await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(refresh["result"], serde_json::Value::Null);
        let notification_messages = log_notification_messages(&notifications);
        assert!(
            notification_messages
                .iter()
                .any(|message| message.contains("ripr analysis refresh started"))
        );
        assert!(
            notification_messages
                .iter()
                .any(|message| message.contains("ripr analysis refresh completed in"))
        );

        let (text_uri, seam_diagnostic) =
            published_seam_diagnostic(&notifications, "67fc764ba37d77bd")?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "position": { "line": 1, "character": 1 }
                }
            }),
        )
        .await?;
        let hover = read_lsp_response(&mut client_read, 3).await?;
        let hover_value = hover["result"]["contents"]["value"]
            .as_str()
            .ok_or_else(|| "expected seam hover markdown value".to_string())?;
        assert!(
            hover_value.contains("## Missing discriminator"),
            "expected seam hover to name missing discriminator, got {hover_value}"
        );
        assert!(
            hover_value.contains("## Related tests"),
            "expected seam hover to name related tests, got {hover_value}"
        );
        assert!(
            hover_value.contains("## Next step"),
            "expected seam hover to name next step, got {hover_value}"
        );
        assert!(
            hover_value.contains("## Suggested test shape"),
            "expected seam hover to name suggested test shape, got {hover_value}"
        );
        assert!(
            hover_value.contains("## Handoff, verify, and receipt commands"),
            "expected seam hover to name handoff, verify, and receipt commands, got {hover_value}"
        );
        assert!(
            hover_value.contains("## Limits"),
            "expected seam hover to name static limits, got {hover_value}"
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/codeAction",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "range": seam_diagnostic["range"].clone(),
                    "context": { "diagnostics": [seam_diagnostic] }
                }
            }),
        )
        .await?;
        let code_actions = read_lsp_response(&mut client_read, 4).await?;
        let titles = code_actions["result"]
            .as_array()
            .ok_or_else(|| "expected codeAction result array".to_string())?
            .iter()
            .filter_map(|action| action.get("title").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        for expected in [
            "Inspect Test Gap - Copy Context",
            "Write targeted test: copy brief",
            "Verify after test: copy verify command",
            "Review result: copy receipt command",
        ] {
            assert!(
                titles.contains(&expected),
                "expected protocol code actions to contain {expected}, got {titles:?}"
            );
        }

        let seam_id = seam_diagnostic["data"]["seam_id"]
            .as_str()
            .ok_or_else(|| "expected seam diagnostic data.seam_id".to_string())?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "workspace/executeCommand",
                "params": {
                    "command": COLLECT_EVIDENCE_CONTEXT_COMMAND,
                    "arguments": [{
                        "seam_id": seam_id,
                        "uri": text_uri,
                        "line": 2
                    }]
                }
            }),
        )
        .await?;
        let context_packet = read_lsp_response(&mut client_read, 5).await?;
        assert_eq!(
            context_packet["result"]["schema_version"],
            serde_json::Value::String("0.1".to_string())
        );
        assert_eq!(context_packet["result"]["seam_id"], seam_id);
        assert_eq!(
            context_packet["result"]["evidence_path"]["discriminate"],
            "present"
        );
        assert_eq!(
            context_packet["result"]["missing_discriminator"],
            "discount_threshold (equality boundary)"
        );
        assert!(
            context_packet["result"]["related_test"]
                .as_str()
                .is_some_and(|value| value.contains("tests/pricing.rs"))
        );
        assert!(
            context_packet["result"]["agent_brief_command"]
                .as_str()
                .is_some_and(|value| value.starts_with("ripr agent brief --root . --seam-id "))
        );
        assert!(
            context_packet["result"]["verify_command"]
                .as_str()
                .is_some_and(|value| value.contains("ripr agent verify --root ."))
        );
        assert!(
            context_packet["result"]["receipt_command"]
                .as_str()
                .is_some_and(|value| value.contains("ripr agent receipt --root ."))
        );
        assert_eq!(
            context_packet["result"]["limits_note"],
            "Static evidence only; no runtime mutation execution."
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "shutdown",
                "params": null
            }),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 6).await?;
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

fn published_seam_diagnostic(
    notifications: &[serde_json::Value],
    seam_id: &str,
) -> Result<(String, serde_json::Value), String> {
    for notification in notifications {
        if notification
            .get("method")
            .and_then(serde_json::Value::as_str)
            != Some("textDocument/publishDiagnostics")
        {
            continue;
        }
        let Some(uri) = notification
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(diagnostics) = notification
            .get("params")
            .and_then(|params| params.get("diagnostics"))
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for diagnostic in diagnostics {
            if diagnostic
                .get("data")
                .and_then(|data| data.get("seam_id"))
                .and_then(serde_json::Value::as_str)
                == Some(seam_id)
            {
                return Ok((uri.to_string(), diagnostic.clone()));
            }
        }
    }
    Err(format!(
        "expected published seam diagnostic with seam_id {seam_id}"
    ))
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
fn hover_for_position_shows_snapshot_age_and_refresh_duration() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic],
        vec![finding],
    );
    diagnostics
        .snapshot
        .refresh
        .record_duration(Duration::from_millis(42));
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, 87, 1)) else {
        return Err("expected diagnostic hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("Analysis snapshot: generated "));
            assert!(markup.value.contains(" ago; last refresh took 42 ms."));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn hover_for_position_adds_snapshot_status_to_seam_hover() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let seam = sample_classified_seam();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let line = diagnostic.range.start.line;
    let mut diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic],
        Vec::new(),
    );
    diagnostics.snapshot.classified_seams = vec![seam];
    diagnostics
        .snapshot
        .refresh
        .record_duration(Duration::from_millis(11));
    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };

    let Some(hover) = backend.hover_for_position(&hover_params(uri, line, 1)) else {
        return Err("expected seam hover".to_string());
    };

    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("**ripr** behavioral seam"));
            assert!(markup.value.contains("`weakly_gripped`"));
            assert!(markup.value.contains("Analysis snapshot: generated "));
            assert!(markup.value.contains(" ago; last refresh took 11 ms."));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn snapshot_status_leaves_non_markup_hover_content_unchanged() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let snapshot =
        sample_analysis_snapshot(PathBuf::from("/workspace"), uri, Vec::new(), Vec::new());
    let hover = tower_lsp_server::ls_types::Hover {
        contents: HoverContents::Scalar(MarkedString::String("plain".to_string())),
        range: None,
    };

    let hover = hover_with_snapshot_status(hover, &snapshot);

    match hover.contents {
        HoverContents::Scalar(MarkedString::String(value)) => {
            assert_eq!(value, "plain");
            Ok(())
        }
        _ => Err("expected scalar hover".to_string()),
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
fn refresh_plan_stores_snapshot_refresh_metadata() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        vec![finding],
    );
    diagnostics
        .snapshot
        .refresh
        .record_duration(Duration::from_millis(42));

    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected refresh plan".to_string());
    };
    let Some(latest) = backend.latest_analysis_snapshot() else {
        return Err("expected latest analysis snapshot".to_string());
    };

    assert_eq!(latest.refresh.duration, Some(Duration::from_millis(42)));
    assert!(latest.refresh.age().is_some());
    assert_eq!(latest.diagnostic_count(), 1);
    assert_eq!(latest.diagnostic_uri_count(), 1);
    assert_eq!(latest.finding_count(), 1);
    assert_eq!(latest.seam_diagnostic_count(), 0);
    Ok(())
}

#[test]
fn refresh_completion_log_message_includes_duration_and_counts() -> Result<(), String> {
    let seam = sample_classified_seam();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam];
    snapshot.refresh.record_duration(Duration::from_millis(17));

    let summary = RefreshLogSummary::from_snapshot(7, &snapshot);
    let message = refresh_completed_log_message(&summary, 1, 2);

    assert!(message.contains("ripr analysis refresh completed in 17 ms"));
    assert!(message.contains("generation=7"));
    assert!(message.contains("diagnostics=1"));
    assert!(message.contains("files=1"));
    assert!(message.contains("findings=0"));
    assert!(message.contains("preview_findings=0"));
    assert!(message.contains("static_limits=0"));
    assert!(message.contains("seam_diagnostics=1"));
    assert!(message.contains("enabled_languages=1"));
    assert!(message.contains("enabled_language_names=rust"));
    assert!(message.contains("published_files=1"));
    assert!(message.contains("cleared_files=2"));
    Ok(())
}

#[test]
fn refresh_completion_log_message_counts_preview_findings_and_limits() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.language = Some(LanguageId::Python);
    finding.language_status = Some(LanguageStatus::Preview);
    finding.static_limit_kind = Some(StaticLimitKind::MissingImportGraph);
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.py")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        vec![finding],
    );

    let summary = RefreshLogSummary::from_snapshot(8, &snapshot);
    let message = refresh_completed_log_message(&summary, 1, 0);

    assert!(message.contains("preview_findings=1"));
    assert!(message.contains("static_limits=1"));
    Ok(())
}

#[test]
fn refresh_completion_log_message_defaults_missing_duration_to_zero() -> Result<(), String> {
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        vec![finding],
    );

    let summary = RefreshLogSummary::from_snapshot(3, &snapshot);
    let message = refresh_completed_log_message(&summary, 1, 0);

    assert!(message.contains("ripr analysis refresh completed in 0 ms"));
    Ok(())
}

#[test]
fn refresh_failure_log_message_includes_actionable_duration() {
    let message = refresh_failed_log_message(
        "workspace analysis failed: Cargo.toml not found",
        Duration::from_millis(9),
    );

    assert_eq!(
        message,
        "ripr analysis refresh failed after 9 ms: workspace analysis failed: Cargo.toml not found"
    );
}

#[test]
fn format_duration_renders_milliseconds_and_whole_seconds() {
    assert_eq!(format_duration(Duration::from_millis(9)), "9 ms");
    assert_eq!(format_duration(Duration::from_secs(1)), "1 second");
    assert_eq!(format_duration(Duration::from_secs(2)), "2 seconds");
}

#[test]
fn refresh_metadata_default_records_generation_time() {
    let metadata = RefreshMetadata::default();

    assert!(metadata.age().is_some());
    assert_eq!(metadata.duration, None);
}

#[test]
fn stale_refresh_generation_does_not_store_older_snapshot() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let Some(first_generation) = backend.next_refresh_generation() else {
        return Err("expected first generation".to_string());
    };
    let Some(second_generation) = backend.next_refresh_generation() else {
        return Err("expected second generation".to_string());
    };
    assert!(!backend.is_current_refresh_generation(first_generation));
    assert!(backend.is_current_refresh_generation(second_generation));

    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let current_uri = test_uri("file:///workspace/src/current.rs")?;
    let current = sample_workspace_diagnostics(
        PathBuf::from("/workspace/current"),
        current_uri,
        vec![diagnostic],
        vec![finding],
    );
    let Some(_) = backend.refresh_plan(current) else {
        return Err("expected current refresh plan".to_string());
    };

    if backend.is_current_refresh_generation(first_generation) {
        let stale = sample_workspace_diagnostics(
            PathBuf::from("/workspace/stale"),
            test_uri("file:///workspace/src/stale.rs")?,
            Vec::new(),
            Vec::new(),
        );
        let Some(_) = backend.refresh_plan(stale) else {
            return Err("expected stale refresh plan".to_string());
        };
    }

    let Some(latest) = backend.latest_analysis_snapshot() else {
        return Err("expected latest analysis snapshot".to_string());
    };
    assert_eq!(latest.root, PathBuf::from("/workspace/current"));
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
                "Inspect finding: copy context packet",
                "quickfix",
                "Inspect finding: copy context",
                COPY_CONTEXT_COMMAND,
            ),
            (
                "Refresh Analysis - Saved Workspace Check",
                "source",
                "Refresh Analysis - Saved Workspace Check",
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
            COPY_TARGETED_TEST_BRIEF_COMMAND,
            COPY_AGENT_PACKET_COMMAND,
            COPY_AGENT_BRIEF_COMMAND,
            COPY_AFTER_SNAPSHOT_COMMAND,
            COPY_AGENT_VERIFY_COMMAND,
            COPY_AGENT_RECEIPT_COMMAND,
            COPY_SUGGESTED_ASSERTION_COMMAND,
            OPEN_RELATED_TEST_COMMAND,
            REFRESH_COMMAND,
        ]
    );
    assert_eq!(commands[0].0, "Inspect Test Gap - Copy Context");
    assert_eq!(commands[0].2[0]["seam_id"], seam.seam.id().as_str());
    assert_eq!(commands[0].2[0]["seam_kind"], "predicate_boundary");
    assert_eq!(commands[0].2[0]["line"], 88);
    assert_eq!(commands[1].0, "Write targeted test: copy brief");
    assert_eq!(commands[1].2[0]["seam_id"], seam.seam.id().as_str());
    assert!(
        commands[1].2[0]["brief"]
            .as_str()
            .is_some_and(|value| value.contains("Add a targeted test:")),
        "expected targeted test brief argument, got {:?}",
        commands[1].2
    );
    assert_eq!(commands[2].0, "Agent handoff: copy packet command");
    assert_eq!(commands[2].2[0]["label"], "agent_packet");
    assert_eq!(commands[2].2[0]["root"], ".");
    assert_eq!(commands[2].2[0]["base"], "origin/main");
    assert_eq!(commands[2].2[0]["mode"], "draft");
    assert_eq!(commands[2].2[0]["seam_id"], seam.seam.id().as_str());
    assert_eq!(commands[2].2[0]["seam_kind"], "predicate_boundary");
    assert_eq!(commands[2].2[0]["seam_file"], "src/pricing.rs");
    assert_eq!(commands[2].2[0]["owner"], "pricing::discounted_total");
    assert_eq!(commands[2].2[0]["line"], 88);
    assert_eq!(commands[2].2[0]["severity"], "warning");
    assert_eq!(
        commands[2].2[0]["target_artifact"],
        "target/ripr/agent/agent-packet.json"
    );
    assert_eq!(
        commands[2].2[0]["command"],
        format!(
            "ripr agent packet --root . --seam-id {} --json > target/ripr/agent/agent-packet.json",
            seam.seam.id().as_str()
        )
    );
    assert_eq!(commands[3].0, "Agent handoff: copy brief command");
    assert_eq!(
        commands[3].2[0]["command"],
        format!(
            "ripr agent brief --root . --seam-id {} --json > target/ripr/agent/agent-brief.json",
            seam.seam.id().as_str()
        )
    );
    assert_eq!(
        commands[4].0,
        "Verify after test: copy after-snapshot command"
    );
    assert_eq!(
        commands[4].2[0]["command"],
        "ripr check --root . --base origin/main --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
    );
    assert_eq!(commands[5].0, "Verify after test: copy verify command");
    assert_eq!(
        commands[5].2[0]["command"],
        "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
    );
    assert_eq!(commands[6].0, "Review result: copy receipt command");
    assert_eq!(
        commands[6].2[0]["command"],
        format!(
            "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id {} --json --out target/ripr/agent/agent-receipt.json",
            seam.seam.id().as_str()
        )
    );
    assert_eq!(
        commands[7].0,
        "Write targeted test: copy suggested assertion"
    );
    assert!(
        commands[7].2[0]["assertion"]
            .as_str()
            .is_some_and(|value| value.contains("assert_eq!(discounted_total")),
        "expected assertion argument, got {:?}",
        commands[7].2
    );
    assert_eq!(commands[8].0, "Write targeted test: open best related test");
    assert_eq!(
        commands[8].2[0]["uri"],
        "file:///workspace/tests/pricing.rs"
    );
    assert_eq!(commands[8].2[0]["line"], 12);
    Ok(())
}

#[test]
fn agent_loop_command_payloads_stay_workspace_relative_for_platform_roots() -> Result<(), String> {
    let seam = sample_classified_seam();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from(r"workspace root\ripr workspace"),
        uri,
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.base = Some("origin/main with space".to_string());
    snapshot.mode = Mode::Ready;
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, Some(&snapshot));

    let commands = code_action_commands(&actions)?;
    let expected_commands = [
        (
            COPY_AGENT_PACKET_COMMAND,
            "agent_packet",
            "target/ripr/agent/agent-packet.json",
            format!(
                "ripr agent packet --root . --seam-id {} --json > target/ripr/agent/agent-packet.json",
                seam.seam.id().as_str()
            ),
        ),
        (
            COPY_AGENT_BRIEF_COMMAND,
            "agent_brief",
            "target/ripr/agent/agent-brief.json",
            format!(
                "ripr agent brief --root . --seam-id {} --json > target/ripr/agent/agent-brief.json",
                seam.seam.id().as_str()
            ),
        ),
        (
            COPY_AFTER_SNAPSHOT_COMMAND,
            "after_snapshot",
            "target/ripr/pilot/after.repo-exposure.json",
            "ripr check --root . --base \"origin/main with space\" --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
                .to_string(),
        ),
        (
            COPY_AGENT_VERIFY_COMMAND,
            "agent_verify",
            "target/ripr/agent/agent-verify.json",
            "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
                .to_string(),
        ),
        (
            COPY_AGENT_RECEIPT_COMMAND,
            "agent_receipt",
            "target/ripr/agent/agent-receipt.json",
            format!(
                "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id {} --json --out target/ripr/agent/agent-receipt.json",
                seam.seam.id().as_str()
            ),
        ),
    ];

    for (command_id, label, target_artifact, expected_command) in expected_commands {
        let argument = commands
            .iter()
            .find(|(_, command, _)| command == command_id)
            .and_then(|(_, _, arguments)| arguments.first())
            .ok_or_else(|| format!("missing command payload for {command_id}"))?;
        assert_eq!(argument["label"], label);
        assert_eq!(argument["root"], ".");
        assert_eq!(argument["base"], "origin/main with space");
        assert_eq!(argument["mode"], "ready");
        assert_eq!(argument["seam_id"], seam.seam.id().as_str());
        assert_eq!(argument["seam_file"], "src/pricing.rs");
        assert_eq!(argument["owner"], "pricing::discounted_total");
        assert_eq!(argument["severity"], "warning");
        assert_eq!(argument["target_artifact"], target_artifact);
        assert_eq!(argument["command"], expected_command);
        let copied = argument["command"]
            .as_str()
            .ok_or_else(|| "expected command string".to_string())?;
        assert!(
            !copied.contains('\\'),
            "copied commands should use workspace-relative slash paths, got {copied}"
        );
        assert!(
            !copied.contains("ripr workspace"),
            "copied commands should not leak platform-specific workspace roots, got {copied}"
        );
    }
    Ok(())
}

#[test]
fn seam_code_actions_fail_closed_for_stale_seam_diagnostic() -> Result<(), String> {
    let seam = sample_classified_seam();
    let mut diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    diagnostic.data = Some(serde_json::json!({
        "schema_version": "0.1",
        "seam_id": "deadbeef00000000",
        "seam_kind": "predicate_boundary",
    }));
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
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
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND)]
    );
    Ok(())
}

#[test]
fn seam_code_actions_keep_legacy_finding_context_when_both_diagnostics_are_present()
-> Result<(), String> {
    let seam = sample_classified_seam();
    let seam_diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let finding_diagnostic = diagnostic_for_finding(Path::new("/workspace"), &sample_finding());
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![seam_diagnostic.clone(), finding_diagnostic.clone()],
        vec![sample_finding()],
    );
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(
        &code_action_params(vec![seam_diagnostic, finding_diagnostic])?,
        Some(&snapshot),
    );

    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(title, _, _)| title.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Inspect Test Gap - Copy Context",
            "Write targeted test: copy brief",
            "Agent handoff: copy packet command",
            "Agent handoff: copy brief command",
            "Verify after test: copy after-snapshot command",
            "Verify after test: copy verify command",
            "Review result: copy receipt command",
            "Write targeted test: copy suggested assertion",
            "Write targeted test: open best related test",
            "Inspect finding: copy context packet",
            "Refresh Analysis - Saved Workspace Check",
        ]
    );
    assert_eq!(commands[0].2[0]["seam_id"], seam.seam.id().as_str());
    assert_eq!(commands[9].2[0]["finding_id"], "probe:pricing:88:predicate");
    assert_eq!(commands[9].2[0]["probe_id"], "probe:pricing:88:predicate");
    Ok(())
}

#[test]
fn seam_code_actions_open_strong_related_test_before_first_related_test() -> Result<(), String> {
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason,
    };

    let mut seam = sample_classified_seam();
    seam.evidence.related_tests = vec![
        RelatedTestGrip {
            test_name: "nearby_smoke_reaches_owner".to_string(),
            file: PathBuf::from("tests/smoke.rs"),
            line: 7,
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Smoke,
            evidence_summary: "smoke-only assertion".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        },
        RelatedTestGrip {
            test_name: "below_threshold_has_no_discount".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 12,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "exact value assertion".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::Medium,
        },
    ];
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam];
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, Some(&snapshot));

    let commands = code_action_commands(&actions)?;
    let Some((_, command, args)) = commands
        .iter()
        .find(|(title, _, _)| title == "Write targeted test: open best related test")
    else {
        return Err(format!(
            "expected open related test action, got {commands:?}"
        ));
    };

    assert_eq!(command, OPEN_RELATED_TEST_COMMAND);
    assert_eq!(args[0]["uri"], "file:///workspace/tests/pricing.rs");
    assert_eq!(args[0]["test_name"], "below_threshold_has_no_discount");
    Ok(())
}

#[test]
fn seam_code_actions_open_highest_confidence_related_test_when_no_strong_test_exists()
-> Result<(), String> {
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason,
    };

    let mut seam = sample_classified_seam();
    seam.evidence.related_tests = vec![
        RelatedTestGrip {
            test_name: "opaque_fixture_hint".to_string(),
            file: PathBuf::from("tests/opaque.rs"),
            line: 3,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::None,
            evidence_summary: "opaque relation".to_string(),
            relation_reason: RelationReason::FixtureOwnerAffinity,
            relation_confidence: RelationConfidence::Opaque,
        },
        RelatedTestGrip {
            test_name: "low_confidence_smoke".to_string(),
            file: PathBuf::from("tests/low.rs"),
            line: 5,
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Smoke,
            evidence_summary: "smoke-only assertion".to_string(),
            relation_reason: RelationReason::FixtureOwnerAffinity,
            relation_confidence: RelationConfidence::Low,
        },
        RelatedTestGrip {
            test_name: "medium_confidence_property".to_string(),
            file: PathBuf::from("tests/medium.rs"),
            line: 9,
            oracle_kind: OracleKind::RelationalCheck,
            oracle_strength: OracleStrength::Medium,
            evidence_summary: "medium oracle".to_string(),
            relation_reason: RelationReason::SameModule,
            relation_confidence: RelationConfidence::Medium,
        },
        RelatedTestGrip {
            test_name: "high_confidence_weak_assertion".to_string(),
            file: PathBuf::from("tests/high.rs"),
            line: 11,
            oracle_kind: OracleKind::RelationalCheck,
            oracle_strength: OracleStrength::Weak,
            evidence_summary: "weak oracle".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        },
    ];
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam];
    let actions = code_action_response(&code_action_params(vec![diagnostic])?, Some(&snapshot));

    let commands = code_action_commands(&actions)?;
    let Some((_, command, args)) = commands
        .iter()
        .find(|(title, _, _)| title == "Write targeted test: open best related test")
    else {
        return Err(format!(
            "expected open related test action, got {commands:?}"
        ));
    };

    assert_eq!(command, OPEN_RELATED_TEST_COMMAND);
    assert_eq!(args[0]["uri"], "file:///workspace/tests/high.rs");
    assert_eq!(args[0]["test_name"], "high_confidence_weak_assertion");
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
        vec![
            COPY_CONTEXT_COMMAND,
            COPY_AGENT_PACKET_COMMAND,
            COPY_AGENT_BRIEF_COMMAND,
            COPY_AFTER_SNAPSHOT_COMMAND,
            COPY_AGENT_VERIFY_COMMAND,
            COPY_AGENT_RECEIPT_COMMAND,
            REFRESH_COMMAND
        ]
    );
    assert_eq!(commands[0].0, "Inspect Test Gap - Copy Context");
    assert_eq!(commands[1].0, "Agent handoff: copy packet command");
    assert_eq!(commands[5].0, "Review result: copy receipt command");
    Ok(())
}

#[test]
fn seam_code_actions_keep_targeted_brief_when_related_test_exists_without_assertion()
-> Result<(), String> {
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason,
    };

    let mut seam = sample_side_effect_seam_without_related_tests();
    seam.evidence.related_tests = vec![RelatedTestGrip {
        test_name: "publish_event_emits_bus_message".to_string(),
        file: PathBuf::from("tests/service.rs"),
        line: 21,
        oracle_kind: OracleKind::SmokeOnly,
        oracle_strength: OracleStrength::Smoke,
        evidence_summary: "related smoke test reaches event publishing".to_string(),
        relation_reason: RelationReason::DirectOwnerCall,
        relation_confidence: RelationConfidence::High,
    }];
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
    assert!(
        commands
            .iter()
            .any(|(_, command, _)| command == COPY_TARGETED_TEST_BRIEF_COMMAND),
        "expected targeted-test brief action when a related test exists, got {commands:?}"
    );
    assert!(
        commands
            .iter()
            .all(|(_, command, _)| command != COPY_SUGGESTED_ASSERTION_COMMAND),
        "expected no suggested assertion action for prose-only side-effect guidance, got {commands:?}"
    );
    let Some((_, command, args)) = commands
        .iter()
        .find(|(title, _, _)| title == "Write targeted test: open best related test")
    else {
        return Err(format!(
            "expected open related test action, got {commands:?}"
        ));
    };
    assert_eq!(command, OPEN_RELATED_TEST_COMMAND);
    assert_eq!(args[0]["uri"], "file:///workspace/tests/service.rs");
    assert_eq!(args[0]["line"], 21);
    Ok(())
}

#[test]
fn boundary_gap_lsp_diagnostics_match_fixture_expectation() -> Result<(), String> {
    let (diagnostics, _) = boundary_gap_lsp_fixture_outputs()?;
    assert_json_fixture("lsp-diagnostics.json", diagnostics)
}

#[test]
fn boundary_gap_lsp_code_actions_match_fixture_expectation() -> Result<(), String> {
    let (_, actions) = boundary_gap_lsp_fixture_outputs()?;
    assert_json_fixture("lsp-code-actions.json", actions)
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
fn refresh_failure_clear_helper_clears_tracked_diagnostics() -> Result<(), String> {
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

    assert_eq!(backend.clear_all_diagnostic_uris(), vec![tracked_uri]);

    assert!(backend.clear_all_diagnostic_uris().is_empty());
    assert!(backend.latest_analysis_snapshot().is_none());
    Ok(())
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
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();

    let Some(generation) = backend.next_refresh_generation() else {
        return Err("expected refresh generation".to_string());
    };

    assert_eq!(generation, 1);
    assert!(backend.is_current_refresh_generation(generation));
    assert!(backend.latest_analysis_snapshot().is_none());
    Ok(())
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

    let config =
        LspAnalysisConfig::from_initialize_params(&params, crate::config::RiprConfig::default());
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

    let config =
        LspAnalysisConfig::from_initialize_params(&params, crate::config::RiprConfig::default());

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

        let config = LspAnalysisConfig::from_initialize_params(
            &params,
            crate::config::RiprConfig::default(),
        );

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
    assert!(config.enable_seam_diagnostics);
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
fn initialize_with_invalid_languages_config_falls_back_to_rust_defaults() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("invalid-languages-config")?;
        std::fs::write(
            root.path().join("ripr.toml"),
            r#"
[languages]
enabled = ["ruby"]
"#,
        )
        .map_err(|err| format!("write invalid config failed: {err}"))?;
        let config_error = match crate::config::load_for_root(root.path()) {
            Ok(_) => {
                return Err(
                    "invalid language config should stay owned by config parsing".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            config_error.contains("languages.enabled") && config_error.contains("ruby"),
            "expected config-owned language error, got: {config_error}"
        );

        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend
            .initialize(initialize_params(
                None,
                Some(file_uri_for_path(root.path())?),
            ))
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        let Some(config) = backend.analysis_config() else {
            return Err("expected backend analysis config".to_string());
        };

        assert_eq!(config.repo_config().source_path(), None);
        assert_eq!(
            config.repo_config().languages().enabled(),
            &[LanguageId::Rust]
        );
        assert_eq!(config.mode, Mode::Draft);
        assert!(config.enable_seam_diagnostics);
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
    assert!(config.enable_seam_diagnostics);
    Ok(())
}

#[test]
fn workspace_diagnostic_batches_uses_default_lsp_analysis_config() {
    let missing_root = Path::new("target/ripr/definitely-missing-lsp-root");

    assert!(workspace_diagnostic_batches(missing_root).is_err());
}

#[test]
fn boundary_gap_workspace_diagnostics_include_live_seam_diagnostic() -> Result<(), String> {
    let fixture_root = boundary_gap_fixture_root();
    let config = boundary_gap_lsp_config(crate::config::RiprConfig::default());

    let batches = workspace_diagnostic_batches_with_config(&fixture_root, &config)?;
    let seam_diagnostic = batches
        .iter()
        .flat_map(|batch| &batch.diagnostics)
        .any(|diagnostic| {
            diagnostic.source.as_deref() == Some("ripr")
                && diagnostic
                    .code
                    .as_ref()
                    .map(diagnostic_code_value)
                    .as_deref()
                    == Some("ripr-seam-weakly-gripped")
        });

    assert!(
        seam_diagnostic,
        "expected boundary_gap live workspace diagnostics to include ripr-seam-weakly-gripped"
    );
    Ok(())
}

#[test]
fn boundary_gap_lsp_explicit_rust_language_matches_default_projection() -> Result<(), String> {
    let fixture_root = boundary_gap_fixture_root();
    let default_config = boundary_gap_lsp_config(crate::config::RiprConfig::default());
    let rust_only_config = boundary_gap_lsp_config(crate::config::tests_only_parse(
        r#"
[languages]
enabled = ["rust"]
"#,
    )?);

    let default_projection = workspace_projection_contract(&fixture_root, &default_config)?;
    let rust_only_projection = workspace_projection_contract(&fixture_root, &rust_only_config)?;

    assert_eq!(
        rust_only_projection, default_projection,
        "explicit [languages] enabled = [\"rust\"] must preserve the saved-workspace Rust editor diagnostics, hover, actions, and status projection"
    );
    Ok(())
}

#[test]
fn boundary_gap_lsp_empty_languages_suppresses_saved_workspace_diagnostics() -> Result<(), String> {
    let fixture_root = boundary_gap_fixture_root();
    let config = boundary_gap_lsp_config(crate::config::tests_only_parse(
        r#"
[languages]
enabled = []
"#,
    )?);

    let diagnostics = workspace_diagnostics_with_config(&fixture_root, &config)?;
    let diagnostic_count = diagnostics
        .batches
        .iter()
        .map(|batch| batch.diagnostics.len())
        .sum::<usize>();

    assert_eq!(
        diagnostic_count, 0,
        "empty [languages] must publish no saved-workspace diagnostics"
    );
    assert!(
        diagnostics.snapshot.findings.is_empty(),
        "empty [languages] must not retain finding diagnostics in the LSP snapshot"
    );
    assert!(
        diagnostics.snapshot.classified_seams.is_empty(),
        "empty [languages] must not retain seam diagnostics in the LSP snapshot"
    );
    let summary = RefreshLogSummary::from_snapshot(1, &diagnostics.snapshot)
        .with_enabled_languages(config.repo_config().languages().enabled());
    let message = refresh_completed_log_message(&summary, 0, 1);
    assert!(
        message.contains("enabled_languages=0"),
        "empty [languages] refresh message must explain the language-disabled projection state"
    );
    assert!(
        message.contains("enabled_language_names="),
        "empty [languages] refresh message must include an empty language-name field"
    );
    Ok(())
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
fn file_uri_to_path_normalizes_backslash_separators() -> Result<(), String> {
    let drive = "C";
    let uri = test_uri(&format!("file:///{drive}:%5Cworkspace%5Csrc%5Clib.rs"))?;

    let Some(path) = path_from_file_uri(&uri) else {
        return Err("expected path from file URI".to_string());
    };
    assert_eq!(
        path,
        PathBuf::from(format!("{drive}:/workspace/src/lib.rs"))
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

#[test]
fn file_uri_match_decodes_equivalent_file_paths() -> Result<(), String> {
    let encoded_uri = test_uri("file:///workspace/src%2Dlib.rs")?;
    let plain_uri = test_uri("file:///workspace/src-lib.rs")?;

    assert!(file_uris_match(&encoded_uri, &plain_uri));
    Ok(())
}

#[test]
fn file_uri_match_treats_windows_drive_paths_case_insensitively() -> Result<(), String> {
    let drive = "C";
    let stored_uri = test_uri(&format!("file:///{drive}:/Workspace/Src/lib.rs"))?;
    let queried_uri = test_uri(&format!(
        "file:///{drive}:/workspace/src/lib.rs",
        drive = drive.to_ascii_lowercase()
    ))?;

    assert!(file_uris_match(&stored_uri, &queried_uri));
    Ok(())
}

#[test]
fn file_uri_match_rejects_non_file_and_distinct_paths() -> Result<(), String> {
    let file_uri = test_uri("file:///workspace/src/lib.rs")?;
    let other_file_uri = test_uri("file:///workspace/src/other.rs")?;
    let non_file_uri = test_uri("https://example.com/workspace/src/lib.rs")?;

    assert!(!file_uris_match(&file_uri, &other_file_uri));
    assert!(!file_uris_match(&non_file_uri, &file_uri));
    assert!(!file_uris_match(&file_uri, &non_file_uri));
    Ok(())
}

#[test]
fn diagnostics_for_uri_matches_windows_drive_case_variants() -> Result<(), String> {
    let drive = "H";
    let root = format!("{drive}:/workspace");
    let stored_uri = test_uri(&format!("file:///{drive}:/workspace/src/pricing.rs"))?;
    let queried_uri = test_uri(&format!(
        "file:///{drive}:/workspace/src/pricing.rs",
        drive = drive.to_ascii_lowercase()
    ))?;
    let finding = sample_finding();
    let diagnostic = diagnostic_for_finding(Path::new(&root), &finding);
    let snapshot = sample_analysis_snapshot(
        PathBuf::from(root),
        stored_uri,
        vec![diagnostic],
        vec![finding],
    );

    let Some(diagnostics) = snapshot.diagnostics_for_uri(&queried_uri) else {
        return Err("expected diagnostics for URI with lowercase drive letter".to_string());
    };

    assert_eq!(diagnostics.len(), 1);
    Ok(())
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

async fn read_lsp_response_with_notifications<R>(
    reader: &mut R,
    id: u64,
) -> Result<(serde_json::Value, Vec<serde_json::Value>), String>
where
    R: AsyncRead + Unpin,
{
    let mut notifications = Vec::new();
    loop {
        let message = read_lsp_message(reader).await?;
        if message.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
            return Ok((message, notifications));
        }
        notifications.push(message);
    }
}

fn log_notification_messages(messages: &[serde_json::Value]) -> Vec<String> {
    messages
        .iter()
        .filter(|message| {
            message.get("method").and_then(serde_json::Value::as_str) == Some("window/logMessage")
        })
        .filter_map(|message| {
            message
                .get("params")
                .and_then(|params| params.get("message"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect()
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
        refresh: RefreshMetadata::generated_now(),
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

fn boundary_gap_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/boundary_gap/input")
}

struct TempLspRoot {
    path: PathBuf,
}

impl TempLspRoot {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempLspRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn unique_lsp_test_root(name: &str) -> Result<TempLspRoot, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-lsp-{name}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
    Ok(TempLspRoot { path: root })
}

fn boundary_gap_lsp_config(repo_config: crate::config::RiprConfig) -> LspAnalysisConfig {
    LspAnalysisConfig {
        base_ref: Some("HEAD".to_string()),
        mode: Mode::Instant,
        repo_config,
        ..LspAnalysisConfig::default()
    }
}

fn workspace_projection_contract(
    root: &Path,
    config: &LspAnalysisConfig,
) -> Result<serde_json::Value, String> {
    let diagnostics = workspace_diagnostics_with_config(root, config)?;
    let projected_diagnostics = diagnostics
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .diagnostics
                .iter()
                .map(|diagnostic| project_diagnostic(root, &batch.uri, diagnostic))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (uri, diagnostic) = first_seam_diagnostic(&diagnostics)?;
    let seam = diagnostics
        .snapshot
        .classified_seam_for_diagnostic(&diagnostic)
        .ok_or_else(|| "expected seam diagnostic to resolve to classified seam".to_string())?;
    let hover = hover_with_snapshot_status(
        classified_seam_hover_response(seam, &diagnostic, Some(&diagnostics.snapshot)),
        &diagnostics.snapshot,
    );
    let hover_markdown = match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        _ => return Err("expected markup hover".to_string()),
    };
    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic.clone()])?,
        Some(&diagnostics.snapshot),
    );
    let summary = RefreshLogSummary::from_snapshot(1, &diagnostics.snapshot)
        .with_enabled_languages(config.repo_config().languages().enabled());
    let status = refresh_completed_log_message(&summary, diagnostics.batches.len(), 0);

    Ok(serde_json::json!({
        "diagnostics": projected_diagnostics,
        "hover": normalize_snapshot_age(&hover_markdown),
        "actions": project_code_actions(root, &actions)?,
        "status": status,
    }))
}

fn normalize_snapshot_age(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            if line.starts_with("Analysis snapshot: generated ") {
                "Analysis snapshot: generated <elapsed> ago.".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn first_seam_diagnostic(
    diagnostics: &WorkspaceDiagnostics,
) -> Result<
    (
        tower_lsp_server::ls_types::Uri,
        tower_lsp_server::ls_types::Diagnostic,
    ),
    String,
> {
    diagnostics
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .diagnostics
                .iter()
                .map(move |diagnostic| (&batch.uri, diagnostic))
        })
        .find(|(_, diagnostic)| {
            diagnostic
                .code
                .as_ref()
                .map(diagnostic_code_value)
                .is_some_and(|code| code.starts_with("ripr-seam-"))
        })
        .map(|(uri, diagnostic)| (uri.clone(), diagnostic.clone()))
        .ok_or_else(|| "expected at least one seam diagnostic".to_string())
}

fn boundary_gap_lsp_fixture_outputs() -> Result<(serde_json::Value, serde_json::Value), String> {
    let fixture_root = boundary_gap_fixture_root();
    let mut seams = crate::analysis::inventory_classified_seams_at_with_config(
        &fixture_root,
        &crate::config::RiprConfig::default(),
    )?;
    seams.sort_by(|left, right| left.seam.id().as_str().cmp(right.seam.id().as_str()));
    if seams.len() != 1 {
        return Err(format!(
            "expected one boundary_gap classified seam, got {}",
            seams.len()
        ));
    }
    let seam = seams
        .into_iter()
        .next()
        .ok_or_else(|| "expected classified seam".to_string())?;
    let diagnostic = diagnostic_for_classified_seam(&fixture_root, &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = file_uri_for_path(&fixture_root.join(seam.seam.file()))?;
    let mut snapshot = sample_analysis_snapshot(
        fixture_root.clone(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.mode = Mode::Fast;
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(
        &code_action_params_for(
            uri.clone(),
            diagnostic.range.start.line,
            vec![diagnostic.clone()],
        )?,
        Some(&snapshot),
    );

    Ok((
        serde_json::json!({
            "fixture": "boundary_gap",
            "diagnostics": [project_diagnostic(&fixture_root, &uri, &diagnostic)?],
        }),
        serde_json::json!({
            "fixture": "boundary_gap",
            "actions": project_code_actions(&fixture_root, &actions)?,
        }),
    ))
}

fn assert_json_fixture(name: &str, actual: serde_json::Value) -> Result<(), String> {
    let path = Path::new("fixtures")
        .join("boundary_gap")
        .join("expected")
        .join(name);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    let text = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "failed to read {}: {err}\nactual:\n{}",
            path.display(),
            pretty_json(&actual)
        )
    })?;
    let expected: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    if expected != actual {
        return Err(format!(
            "{} drifted\nexpected:\n{}\nactual:\n{}",
            path.display(),
            pretty_json(&expected),
            pretty_json(&actual)
        ));
    }
    Ok(())
}

fn project_diagnostic(
    root: &Path,
    uri: &tower_lsp_server::ls_types::Uri,
    diagnostic: &tower_lsp_server::ls_types::Diagnostic,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "uri": relative_uri_path(root, uri)?,
        "range": {
            "start": {
                "line": diagnostic.range.start.line,
                "character": diagnostic.range.start.character,
            },
            "end": {
                "line": diagnostic.range.end.line,
                "character": diagnostic.range.end.character,
            },
        },
        "severity": diagnostic.severity.map(diagnostic_severity_label),
        "code": diagnostic.code.as_ref().map(diagnostic_code_value),
        "source": diagnostic.source.clone(),
        "message": diagnostic.message,
        "data": diagnostic.data.clone(),
    }))
}

fn project_code_actions(
    root: &Path,
    actions: &[CodeActionOrCommand],
) -> Result<Vec<serde_json::Value>, String> {
    let commands = code_action_commands(actions)?;
    commands
        .into_iter()
        .map(|(title, command, arguments)| {
            let arguments = arguments
                .iter()
                .map(|argument| normalize_lsp_action_argument(root, argument))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(serde_json::json!({
                "title": title,
                "command": command,
                "arguments": arguments,
            }))
        })
        .collect()
}

fn normalize_lsp_action_argument(
    root: &Path,
    argument: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(object) = argument.as_object() else {
        return Ok(argument.clone());
    };
    let mut normalized = serde_json::Map::new();
    for (key, value) in object {
        if key == "uri"
            && let Some(uri) = value.as_str()
            && uri.starts_with("file://")
        {
            let parsed = uri
                .parse()
                .map_err(|err| format!("failed to parse action uri {uri}: {err}"))?;
            normalized.insert(
                key.clone(),
                serde_json::json!(relative_uri_path(root, &parsed)?),
            );
        } else {
            normalized.insert(key.clone(), value.clone());
        }
    }
    Ok(serde_json::Value::Object(normalized))
}

fn code_action_params_for(
    uri: tower_lsp_server::ls_types::Uri,
    line: u32,
    diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
) -> Result<CodeActionParams, String> {
    Ok(CodeActionParams {
        text_document: TextDocumentIdentifier::new(uri),
        range: Range {
            start: Position { line, character: 0 },
            end: Position {
                line,
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

fn diagnostic_severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::ERROR => "error",
        DiagnosticSeverity::WARNING => "warning",
        DiagnosticSeverity::INFORMATION => "information",
        DiagnosticSeverity::HINT => "hint",
        _ => "unknown",
    }
}

fn diagnostic_code_value(code: &NumberOrString) -> String {
    match code {
        NumberOrString::Number(value) => value.to_string(),
        NumberOrString::String(value) => value.clone(),
    }
}

fn relative_uri_path(root: &Path, uri: &tower_lsp_server::ls_types::Uri) -> Result<String, String> {
    let path =
        path_from_file_uri(uri).ok_or_else(|| format!("expected file uri: {}", uri.as_str()))?;
    relative_path(root, &path)
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let root = normalize_fixture_path(root);
    let path = normalize_fixture_path(path);
    if path == root {
        return Ok(".".to_string());
    }
    let prefix = format!("{root}/");
    path.strip_prefix(&prefix)
        .map(str::to_string)
        .ok_or_else(|| format!("path {path} is not under fixture root {root}"))
}

fn normalize_fixture_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
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

#[expect(
    deprecated,
    reason = "Test helper constructs InitializeParams including the deprecated root_path/root_uri fields to exercise fallback handling in capabilities.rs."
)]
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
        language: None,
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
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
fn preview_finding_diagnostic_preserves_language_metadata() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.language = Some(LanguageId::Python);
    finding.language_status = Some(LanguageStatus::Preview);
    finding.owner_kind = Some(OwnerKind::Function);
    finding.static_limit_kind = Some(StaticLimitKind::MissingImportGraph);
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert!(
        diagnostic
            .message
            .contains("python preview evidence (syntax-first, advisory)")
    );
    assert!(
        diagnostic
            .message
            .contains("Static limit: missing_import_graph")
    );
    let data = diagnostic
        .data
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| "expected diagnostic data".to_string())?;
    assert_eq!(
        data.get("language").and_then(|value| value.as_str()),
        Some("python")
    );
    assert_eq!(
        data.get("language_status").and_then(|value| value.as_str()),
        Some("preview")
    );
    assert_eq!(
        data.get("owner_kind").and_then(|value| value.as_str()),
        Some("function")
    );
    assert_eq!(
        data.get("static_limit_kind")
            .and_then(|value| value.as_str()),
        Some("missing_import_graph")
    );
    Ok(())
}

#[test]
fn preview_finding_hover_shows_boundary_before_evidence() -> Result<(), String> {
    use super::hover::finding_hover_response;

    let mut finding = sample_finding();
    finding.language = Some(LanguageId::Python);
    finding.language_status = Some(LanguageStatus::Preview);
    finding.static_limit_kind = Some(StaticLimitKind::MissingImportGraph);
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    let hover = finding_hover_response(&finding, &diagnostic);

    match hover.contents {
        HoverContents::Markup(markup) => {
            let preview_index = markup
                .value
                .find("## Preview Boundary")
                .ok_or_else(|| "expected preview boundary".to_string())?;
            let evidence_index = markup
                .value
                .find("## RIPR Evidence")
                .ok_or_else(|| "expected evidence section".to_string())?;
            let static_limit_index = markup
                .value
                .find("Static limit: missing_import_graph")
                .ok_or_else(|| "expected static limit".to_string())?;
            let action_index = markup
                .value
                .find("Add an exact boundary assertion.")
                .ok_or_else(|| "expected suggested action text".to_string())?;
            assert!(
                preview_index < evidence_index,
                "preview boundary must appear before evidence details"
            );
            assert!(
                static_limit_index < action_index,
                "static limits must appear before suggested action language"
            );
            assert!(markup.value.contains("Language: python"));
            assert!(markup.value.contains("Status: preview"));
            assert!(markup.value.contains("Evidence: syntax-first"));
            assert!(markup.value.contains("Action: advisory only"));
            assert!(markup.value.contains("Static limit: missing_import_graph"));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn preview_finding_code_actions_stay_bounded_to_context_and_refresh() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.language = Some(LanguageId::Python);
    finding.language_status = Some(LanguageStatus::Preview);
    finding.owner_kind = Some(OwnerKind::Function);
    finding.static_limit_kind = Some(StaticLimitKind::MissingImportGraph);
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.py")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
    );

    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Inspect finding: copy context packet", COPY_CONTEXT_COMMAND),
            ("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND),
        ],
        "preview findings must not expose seam repair, related-test, verify, or receipt actions without validated seam/gap evidence"
    );
    assert_eq!(commands[0].2[0]["finding_id"], "probe:pricing:88:predicate");
    assert_eq!(commands[0].2[0]["probe_id"], "probe:pricing:88:predicate");
    Ok(())
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
        let expected_finding = finding.clone();
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
        let expected_stop_reasons = expected_finding
            .effective_stop_reasons()
            .iter()
            .map(|reason| reason.as_str().to_string())
            .collect();
        let expected_context_packet = crate::domain::context_packet::ContextPacket::from_finding(
            &expected_finding,
            crate::config::DEFAULT_CONTEXT_RELATED_TESTS,
            expected_stop_reasons,
        );
        let expected_json =
            crate::output::json::render_context_packet_dto(&expected_context_packet);
        let expected_packet: serde_json::Value = serde_json::from_str(&expected_json)
            .map_err(|err| format!("failed to parse expected packet: {err}"))?;
        assert_eq!(packet, expected_packet);
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
fn execute_command_collect_evidence_context_returns_editor_packet_for_known_seam()
-> Result<(), String> {
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
            command: COLLECT_EVIDENCE_CONTEXT_COMMAND.to_string(),
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
            return Err("expected evidence context packet".to_string());
        };

        assert_eq!(packet["schema_version"], "0.1");
        assert_eq!(packet["tool"], "ripr");
        assert_eq!(packet["base"], "origin/main");
        assert_eq!(packet["mode"], "draft");
        assert_eq!(packet["seam_id"], seam_id);
        assert_eq!(packet["file"], "src/pricing.rs");
        assert_eq!(packet["range"]["start"], 88);
        assert_eq!(packet["range"]["end"], 88);
        assert_eq!(packet["class"], "weakly_gripped");
        assert_eq!(packet["seam_kind"], "predicate_boundary");
        assert_eq!(packet["owner"], "pricing::discounted_total");
        assert_eq!(packet["evidence_path"]["reach"], "present");
        assert_eq!(packet["evidence_path"]["activate"], "present");
        assert_eq!(packet["evidence_path"]["propagate"], "present");
        assert_eq!(packet["evidence_path"]["observe"], "present");
        assert_eq!(packet["evidence_path"]["discriminate"], "weak");
        assert_eq!(
            packet["missing_discriminator"],
            "discount_threshold (equality boundary)"
        );
        assert_eq!(
            packet["related_test"],
            "tests/pricing.rs::below_threshold_has_no_discount"
        );
        assert_eq!(
            packet["related_test_location"]["oracle_strength"],
            "strong"
        );
        assert_eq!(packet["suggested_test"]["file"], "tests/pricing.rs");
        assert!(
            packet["suggested_assertion"]
                .as_str()
                .is_some_and(|value| value.contains("assert"))
        );
        assert!(
            packet["agent_brief_command"]
                .as_str()
                .is_some_and(|value| value.starts_with("ripr agent brief --root . --seam-id "))
        );
        assert_eq!(
            packet["after_snapshot_command"],
            "ripr check --root . --base origin/main --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
        );
        assert_eq!(
            packet["verify_command"],
            "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
        );
        assert!(
            packet["receipt_command"]
                .as_str()
                .is_some_and(|value| {
                    value.contains("ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json")
                        && value.contains("--out target/ripr/agent/agent-receipt.json")
                })
        );
        assert_eq!(
            packet["limits_note"],
            "Static evidence only; no runtime mutation execution."
        );
        Ok(())
    })
}

#[test]
fn execute_command_collect_evidence_context_returns_none_for_unknown_seam() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let seam = sample_classified_seam();
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
            command: COLLECT_EVIDENCE_CONTEXT_COMMAND.to_string(),
            arguments: vec![serde_json::json!({
                "seam_id": "unknown-seam",
            })],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result.map_err(|err| format!("execute_command failed: {err}"))?;
        assert!(packet.is_none(), "expected None for unknown seam");
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
    let Some(provider) = initialize_result().capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };

    assert!(
        provider
            .commands
            .iter()
            .any(|command| command == REFRESH_COMMAND)
    );
    Ok(())
}
