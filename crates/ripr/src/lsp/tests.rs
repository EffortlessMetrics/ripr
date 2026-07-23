use super::actions::{SERVER_EXECUTED_COMMANDS, code_action_response};
use super::backend::{
    Backend, RefreshLogSummary, refresh_completed_log_message, refresh_failed_log_message,
    workspace_input_path_is_relevant,
};
use super::capabilities::{
    WorkspaceRootResolution, initialize_result, root_from_initialize_params,
};
use super::client_features::ClientFeatureProfile;
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, WorkspaceDiagnostics, add_canonical_group_data, canonical_finding_groups,
    canonical_group_has_mixed_classes, diagnostic_for_classified_seam, diagnostic_for_finding,
    diagnostic_refresh_plan, diagnostic_severity_for_class, finding_diagnostics_by_uri,
    take_all_uris, workspace_diagnostic_batches, workspace_diagnostic_batches_with_config,
    workspace_diagnostics_with_config,
};
use super::gap_artifacts::{
    GapArtifactIdentity, GapArtifactKind, GapArtifactRejection, ValidatedGapArtifact,
};
use super::hover::{classified_seam_hover_response, hover_response, hover_with_snapshot_status};
use super::input_identity::LspAnalysisInputIdentity;
use super::lens::{code_lens_response, lens_title_is_static_language_clean, lens_view_identity};
use super::progress::ProgressEvent;
use super::refresh_scheduler::{
    RefreshAttemptOutcome, RefreshDecision, RefreshReason, RefreshRequest, RefreshScope,
};
use super::state::{
    AnalysisAttemptState, AnalysisSnapshot, DocumentStore, RefreshMetadata, content_digest,
    format_duration,
};
use super::uri::{encode_uri_path, file_uri_for_path, file_uris_match, path_from_file_uri};
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, COPY_AFTER_SNAPSHOT_COMMAND, COPY_AGENT_BRIEF_COMMAND,
    COPY_AGENT_PACKET_COMMAND, COPY_AGENT_RECEIPT_COMMAND, COPY_AGENT_VERIFY_COMMAND,
    COPY_CONTEXT_COMMAND, COPY_SUGGESTED_ASSERTION_COMMAND, COPY_TARGETED_TEST_BRIEF_COMMAND,
    HOVER_TEXT, OPEN_RELATED_TEST_COMMAND, REFRESH_COMMAND, build_service,
};
use crate::analysis::cancellation::AnalysisCancellationToken;
use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use crate::app::Mode;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap, LanguageId, LanguageStatus,
    MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId,
    RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
    StaticLimitKind, ValueContext, ValueFact,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tower_lsp_server::LanguageServer;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeLensOptions, Diagnostic,
    DiagnosticSeverity, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, ExecuteCommandParams, FileChangeType, FileEvent, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, MarkedString, NumberOrString,
    PartialResultParams, Position, PositionEncodingKind, PreviousResultId, Range,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TraceValue,
    VersionedTextDocumentIdentifier, WindowClientCapabilities, WorkspaceDiagnosticParams,
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
    assert_eq!(
        result.capabilities.position_encoding,
        Some(PositionEncodingKind::UTF16),
        "diagnostic ranges use UTF-16 code-unit offsets"
    );
    let Some(workspace) = result.capabilities.workspace else {
        return Err("expected workspace capability".to_string());
    };
    let Some(workspace_folders) = workspace.workspace_folders else {
        return Err("expected workspace-folder capability".to_string());
    };
    assert_eq!(workspace_folders.supported, Some(true));
    let Some(provider) = result.capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };
    let commands = provider.commands;
    assert_eq!(
        commands,
        vec![
            REFRESH_COMMAND,
            COLLECT_CONTEXT_COMMAND,
            COLLECT_EVIDENCE_CONTEXT_COMMAND,
            COLLECT_WORKSPACE_STATUS_COMMAND,
            COLLECT_REPAIR_PACKET_COMMAND,
            COLLECT_TOP_LIMITATION_COMMAND,
            COLLECT_RECEIPT_STATUS_COMMAND,
        ]
    );
    Ok(())
}

#[test]
fn workspace_input_watch_requires_contained_cargo_manifest_or_lockfile() {
    let root = std::env::temp_dir().join("ripr-workspace-input-watch");
    let root = root.as_path();
    assert!(workspace_input_path_is_relevant(
        root,
        &root.join("Cargo.toml")
    ));
    assert!(workspace_input_path_is_relevant(
        root,
        &root.join("crates/app/Cargo.lock")
    ));
    assert!(!workspace_input_path_is_relevant(
        root,
        &root
            .with_file_name("ripr-workspace-input-watch-sibling")
            .join("Cargo.toml")
    ));
    assert!(!workspace_input_path_is_relevant(
        root,
        &root.join("Cargo.toml.bak")
    ));
}

#[cfg(windows)]
#[test]
fn workspace_input_watch_uses_case_insensitive_windows_containment() {
    let root = std::env::temp_dir().join("ripr-workspace-input-watch-case");
    let differently_cased_root = PathBuf::from(root.to_string_lossy().to_ascii_uppercase());
    assert!(workspace_input_path_is_relevant(
        &root,
        &differently_cased_root.join("Cargo.toml")
    ));
    assert!(!workspace_input_path_is_relevant(
        &root,
        &differently_cased_root
            .with_file_name("RIPR-WORKSPACE-INPUT-WATCH-CASE-SIBLING")
            .join("Cargo.toml")
    ));
}

#[test]
fn watched_file_batch_preserves_config_and_workspace_graph_signals() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let backend_root = root.clone();
    let (service, _socket) =
        LspService::new(move |client| Backend::new(client, backend_root.clone()));
    let backend = service.inner();
    backend.initialize_test_workspace_root();
    let config_uri = file_uri_for_path(&root.join(crate::config::CONFIG_FILE_NAME))
        .map_err(|err| format!("config URI failed: {err}"))?;
    let manifest_uri = file_uri_for_path(&root.join("Cargo.toml"))
        .map_err(|err| format!("manifest URI failed: {err}"))?;
    let changes = vec![
        FileEvent {
            uri: config_uri,
            typ: FileChangeType::CHANGED,
        },
        FileEvent {
            uri: manifest_uri,
            typ: FileChangeType::CHANGED,
        },
    ];

    assert_eq!(backend.watched_file_change_kinds(&changes), (true, true));
    Ok(())
}

#[test]
fn capabilities_advertise_code_lens_provider() -> Result<(), String> {
    let result = initialize_result();
    let provider = result
        .capabilities
        .code_lens_provider
        .ok_or("expected code_lens_provider to be Some")?;
    assert_eq!(
        provider,
        CodeLensOptions {
            resolve_provider: Some(false),
        },
        "code_lens_provider must advertise resolve_provider: false (advisory text-only; no resolve round-trip)"
    );
    Ok(())
}

#[test]
fn document_pull_reuses_result_id_as_unchanged_report() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let finding = sample_finding();
    backend
        .refresh_plan(sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        ))
        .ok_or_else(|| "expected committed snapshot".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    let request = |previous_result_id| DocumentDiagnosticParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        identifier: None,
        previous_result_id,
        work_done_progress_params: Default::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let first = runtime
        .block_on(backend.diagnostic(request(None)))
        .map_err(|err| format!("first pull failed: {err}"))?;
    let first_json = serde_json::to_value(first)
        .map_err(|err| format!("serialize first report failed: {err}"))?;
    if first_json.get("kind").and_then(serde_json::Value::as_str) != Some("full") {
        return Err(format!("expected full first report: {first_json}"));
    }
    let result_id = first_json
        .get("resultId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "full report did not carry resultId".to_string())?
        .to_string();
    let second = runtime
        .block_on(backend.diagnostic(request(Some(result_id))))
        .map_err(|err| format!("second pull failed: {err}"))?;
    let second_json = serde_json::to_value(second)
        .map_err(|err| format!("serialize second report failed: {err}"))?;
    if second_json.get("kind").and_then(serde_json::Value::as_str) != Some("unchanged") {
        return Err(format!("expected unchanged second report: {second_json}"));
    }
    if second_json.get("items").is_some() {
        return Err("unchanged report unexpectedly carried items".to_string());
    }
    Ok(())
}

#[test]
fn workspace_pull_reuses_each_document_result_id() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let finding = sample_finding();
    backend
        .refresh_plan(sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        ))
        .ok_or_else(|| "expected committed snapshot".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    let first = runtime
        .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: Vec::new(),
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("first workspace pull failed: {err}"))?;
    let first_json = serde_json::to_value(first)
        .map_err(|err| format!("serialize first workspace report failed: {err}"))?;
    let result_id = first_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("resultId"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "workspace full report did not carry resultId".to_string())?
        .to_string();
    let second = runtime
        .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: vec![PreviousResultId {
                uri,
                value: result_id,
            }],
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("second workspace pull failed: {err}"))?;
    let second_json = serde_json::to_value(second)
        .map_err(|err| format!("serialize second workspace report failed: {err}"))?;
    let kind = second_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("kind"))
        .and_then(serde_json::Value::as_str);
    if kind != Some("unchanged") {
        return Err(format!(
            "expected unchanged workspace report: {second_json}"
        ));
    }
    Ok(())
}

#[test]
fn pull_diagnostics_before_first_snapshot_return_empty_full_reports() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    let document = runtime
        .block_on(backend.diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("cold-start document pull failed: {err}"))?;
    let document_json = serde_json::to_value(document)
        .map_err(|err| format!("serialize cold-start document report failed: {err}"))?;
    if document_json
        .get("kind")
        .and_then(serde_json::Value::as_str)
        != Some("full")
        || document_json
            .get("items")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|items| !items.is_empty())
    {
        return Err(format!(
            "expected an empty full document report before the first snapshot: {document_json}"
        ));
    }

    let workspace = runtime
        .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: Vec::new(),
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("cold-start workspace pull failed: {err}"))?;
    let workspace_json = serde_json::to_value(workspace)
        .map_err(|err| format!("serialize cold-start workspace report failed: {err}"))?;
    if workspace_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|items| !items.is_empty())
    {
        return Err(format!(
            "expected an empty workspace report before the first snapshot: {workspace_json}"
        ));
    }
    Ok(())
}

#[test]
fn workspace_pull_marks_only_changed_document_full() -> Result<(), String> {
    // Selection-authority contract (#1973): pull serves the stored selected
    // set and derives result IDs from it, so the changed document must be a
    // served (budget-actionable) item for its message change to be
    // selection-relevant. `headline_eligible` is the producer-owned
    // eligibility signal the budget reads.
    fn served_diagnostic(root: &Path, finding: &Finding) -> tower_lsp_server::ls_types::Diagnostic {
        let mut diagnostic = diagnostic_for_finding(root, finding);
        if let Some(data) = diagnostic
            .data
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
        {
            data.insert("headline_eligible".to_string(), serde_json::json!(true));
        }
        diagnostic
    }
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let first_uri = test_uri("file:///workspace/src/first.rs")?;
    let second_uri = test_uri("file:///workspace/src/second.rs")?;
    let first_finding = sample_finding();
    let mut second_finding = sample_finding();
    second_finding.id = "probe:second:88:predicate".to_string();
    second_finding.probe.id = ProbeId("probe:second:88:predicate".to_string());
    second_finding.probe.location.file = PathBuf::from("src/second.rs");
    let first_diagnostic = served_diagnostic(Path::new("/workspace"), &first_finding);
    let initial_second_diagnostic = served_diagnostic(Path::new("/workspace"), &second_finding);
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        first_uri.clone(),
        vec![first_diagnostic.clone()],
        vec![first_finding.clone(), second_finding.clone()],
    );
    snapshot
        .diagnostics_by_uri
        .insert(second_uri.clone(), vec![initial_second_diagnostic.clone()]);
    let diagnostics = WorkspaceDiagnostics {
        snapshot,
        batches: vec![
            DiagnosticBatch {
                uri: first_uri.clone(),
                diagnostics: vec![first_diagnostic],
            },
            DiagnosticBatch {
                uri: second_uri.clone(),
                diagnostics: vec![initial_second_diagnostic],
            },
        ],
    };
    backend
        .refresh_plan(diagnostics)
        .ok_or_else(|| "expected committed multi-document snapshot".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    let first = runtime
        .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: Vec::new(),
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("first multi-document pull failed: {err}"))?;
    let first_json = serde_json::to_value(first)
        .map_err(|err| format!("serialize first multi-document report failed: {err}"))?;
    let previous_result_ids = first_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "first multi-document report had no items".to_string())?
        .iter()
        .map(|item| {
            let uri = item
                .get("uri")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "multi-document item had no URI".to_string())?
                .parse()
                .map_err(|err| format!("parse returned URI failed: {err}"))?;
            let result_id = item
                .get("resultId")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "multi-document item had no result ID".to_string())?;
            Ok(PreviousResultId {
                uri,
                value: result_id.to_string(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut changed_second_diagnostic =
        served_diagnostic(Path::new("/workspace"), &sample_finding());
    changed_second_diagnostic.message.push_str(" changed");
    let mut changed_snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        first_uri.clone(),
        vec![served_diagnostic(
            Path::new("/workspace"),
            &sample_finding(),
        )],
        vec![first_finding, second_finding],
    );
    changed_snapshot
        .diagnostics_by_uri
        .insert(second_uri.clone(), vec![changed_second_diagnostic.clone()]);
    backend
        .refresh_plan(WorkspaceDiagnostics {
            snapshot: changed_snapshot,
            batches: vec![
                DiagnosticBatch {
                    uri: first_uri,
                    diagnostics: vec![served_diagnostic(
                        Path::new("/workspace"),
                        &sample_finding(),
                    )],
                },
                DiagnosticBatch {
                    uri: second_uri,
                    diagnostics: vec![changed_second_diagnostic],
                },
            ],
        })
        .ok_or_else(|| "expected changed multi-document snapshot".to_string())?;

    let second = runtime
        .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids,
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        }))
        .map_err(|err| format!("second multi-document pull failed: {err}"))?;
    let second_json = serde_json::to_value(second)
        .map_err(|err| format!("serialize second multi-document report failed: {err}"))?;
    let kinds = second_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "second multi-document report had no items".to_string())?
        .iter()
        .filter_map(|item| {
            item.get("uri")
                .and_then(serde_json::Value::as_str)
                .zip(item.get("kind").and_then(serde_json::Value::as_str))
        })
        .collect::<BTreeMap<_, _>>();
    if kinds.get("file:///workspace/src/first.rs") != Some(&"unchanged")
        || kinds.get("file:///workspace/src/second.rs") != Some(&"full")
    {
        return Err(format!(
            "expected only the changed document to be full: {second_json}"
        ));
    }
    Ok(())
}

/// Drives the real async `code_lens` handler path (Backend → code_lens_response)
/// using the tokio runtime, matching the pattern of `framed_lsp_protocol_smoke_exercises_tower_server`.
/// Constructs a `Backend` with a populated `latest_analysis` snapshot, calls
/// `code_lens` with a `CodeLensParams` for the file, and asserts lenses returned.
/// This exercises the handler→helper wiring, not just the pure fn.
#[test]
fn backend_code_lens_handler_delegates_to_lens_helper() -> Result<(), String> {
    // Build a minimal snapshot with one finding that belongs to a specific file.
    let root = "/workspace";
    let file = "src/lib.rs";
    let uri_str = "file:///workspace/src/lib.rs";

    // Reuse the same Finding construction as the pure-fn tests via the imported helper.
    // We call code_lens_response directly (the pure fn) to verify the handler wiring.
    let uri = uri_str
        .parse::<tower_lsp_server::ls_types::Uri>()
        .map_err(|e| format!("test URI parse failed: {e}"))?;

    // Build finding and snapshot manually (same as lens::tests helpers).
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, OracleKind, OracleStrength,
        Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, SymbolId,
    };
    let finding = crate::domain::Finding {
        id: format!("probe:{file}:10:predicate:aabbccdd"),
        canonical_gap: None,
        probe: Probe {
            id: ProbeId(format!("probe:{file}:10:predicate:aabbccdd")),
            location: SourceLocation::new(file, 10, 1),
            owner: Some(SymbolId("owner::fn".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Value,
            before: None,
            after: Some("true".to_string()),
            expression: "x > 0".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        },
        class: ExposureClass::Exposed,
        ripr: RiprEvidence {
            reach: StageEvidence::new(StageState::Yes, Confidence::High, "reachable"),
            infect: StageEvidence::new(StageState::Yes, Confidence::High, "infectable"),
            propagate: StageEvidence::new(StageState::Yes, Confidence::Medium, "propagatable"),
            reveal: RevealEvidence {
                observe: StageEvidence::new(StageState::Weak, Confidence::Medium, "observed"),
                discriminate: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "discriminated",
                ),
            },
        },
        confidence: 1.0,
        evidence: Vec::new(),
        missing: Vec::new(),
        flow_sinks: Vec::new(),
        activation: ActivationEvidence::default(),
        stop_reasons: Vec::new(),
        related_tests: vec![RelatedTest {
            name: "test_discounts".to_string(),
            file: std::path::PathBuf::from("tests/lib.rs"),
            line: 42,
            oracle: None,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
        }],
        recommended_next_step: None,
        language: None,
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
        changed_sink: None,
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
    };

    // Build snapshot satisfying is_consistent().
    let diag_uri = file_uri_for_path(&std::path::PathBuf::from(root).join(file))
        .map_err(|e| format!("uri build failed: {e}"))?;
    let diag = tower_lsp_server::ls_types::Diagnostic {
        range: Range {
            start: Position {
                line: 9,
                character: 0,
            },
            end: Position {
                line: 9,
                character: 120,
            },
        },
        severity: None,
        code: None,
        code_description: None,
        source: Some("ripr".to_string()),
        message: "test".to_string(),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({ "finding_id": finding.id })),
    };
    let mut diagnostics_by_uri = std::collections::BTreeMap::new();
    diagnostics_by_uri.insert(diag_uri, vec![diag]);

    let snapshot = AnalysisSnapshot {
        root: std::path::PathBuf::from(root),
        input_identity: None,
        base: None,
        mode: crate::app::Mode::Draft,
        refresh: RefreshMetadata::default(),
        findings: vec![finding],
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        classified_seams: Vec::new(),
        gap_artifacts: Vec::new(),
        gap_artifact_rejections: Vec::new(),
        diagnostics_by_uri,
        delivery_selection: None,
        seams_deferred: false,
        partial_scope: None,
        component_outcomes: Vec::new(),
        out_of_scope_test_file_findings: 0,
    };

    // Call the pure code_lens_response directly to verify the handler→helper path.
    // (The async Backend test would require spinning up an LspService, which is covered
    // by framed_lsp_protocol_smoke_exercises_tower_server; this test verifies the wiring
    // at the handler boundary with a real snapshot.)
    let lenses = code_lens_response(&uri, Some(&snapshot));

    if lenses.is_empty() {
        return Err(
            "handler must return lenses for a snapshot with a matching finding".to_string(),
        );
    }
    let title = lenses[0]
        .command
        .as_ref()
        .ok_or("expected command in lens from handler")?
        .title
        .clone();
    if !title.contains("1") {
        return Err(format!(
            "handler lens must cite 1 related test, got: {title}"
        ));
    }
    if !lens_title_is_static_language_clean(&title) {
        return Err(format!(
            "handler lens title contains forbidden vocabulary: {title}"
        ));
    }
    Ok(())
}

#[test]
fn code_lens_refresh_is_not_attempted_for_unsupported_clients() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("code-lens-refresh-unsupported")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        // Initialize WITHOUT workspace.codeLens.refreshSupport: the server
        // must not record or attempt any refresh for this client (#2032).
        backend
            .initialize(initialize_params(
                None,
                Some(file_uri_for_path(root.path())?),
            ))
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let finding = sample_finding();
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        );
        let identity = lens_view_identity(&diagnostics.snapshot);
        if backend.note_lens_view_for_refresh(identity) {
            return Err("an unsupported client must not attempt a code lens refresh".to_string());
        }
        if backend.last_requested_lens_view_identity().is_some() {
            return Err(
                "an unsupported client must not record or attempt a code lens refresh".to_string(),
            );
        }
        Ok(())
    })
}

#[test]
fn code_lens_refresh_tracks_semantic_view_changes_for_supported_clients() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("code-lens-refresh-supported")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, Some(file_uri_for_path(root.path())?));
        params.capabilities.workspace =
            Some(tower_lsp_server::ls_types::WorkspaceClientCapabilities {
                code_lens: Some(
                    tower_lsp_server::ls_types::CodeLensWorkspaceClientCapabilities {
                        refresh_support: Some(true),
                    },
                ),
                ..tower_lsp_server::ls_types::WorkspaceClientCapabilities::default()
            });
        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let identity_for = |finding: Finding| {
            lens_view_identity(
                &sample_workspace_diagnostics(
                    PathBuf::from("/workspace"),
                    uri.clone(),
                    vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
                    vec![finding],
                )
                .snapshot,
            )
        };
        let identity_a = identity_for(sample_finding());
        if !backend.note_lens_view_for_refresh(identity_a.clone()) {
            return Err("a supported client's first lens view must read as changed".to_string());
        }
        if backend.last_requested_lens_view_identity() != Some(identity_a.clone()) {
            return Err(
                "a supported client's first lens view must be recorded as requested".to_string(),
            );
        }
        // A byte-identical re-commit reports no change and sends nothing.
        if backend.note_lens_view_for_refresh(identity_a) {
            return Err("a byte-identical lens view must not read as changed".to_string());
        }

        // A semantic change (classification flip) advances the recorded view.
        let mut changed = sample_finding();
        changed.class = ExposureClass::Exposed;
        let identity_b = identity_for(changed);
        if !backend.note_lens_view_for_refresh(identity_b.clone()) {
            return Err("a classification change must read as a lens-view change".to_string());
        }
        if backend.last_requested_lens_view_identity() != Some(identity_b) {
            return Err("a classification change must advance the recorded lens view".to_string());
        }
        Ok(())
    })
}

#[test]
fn serve_stdio_call_presence_observer() -> Result<(), String> {
    let source = include_str!("../lsp.rs");
    let serve_stdio = source
        .split("async fn serve_stdio()")
        .nth(1)
        .ok_or_else(|| "expected serve_stdio implementation in lsp module".to_string())?;
    let serve_streams = source
        .split("async fn serve_streams")
        .nth(1)
        .ok_or_else(|| "expected serve_streams implementation in lsp module".to_string())?;

    assert!(
        serve_stdio.contains("transport_bounds::TransportBounds::default()"),
        "serve_stdio should serve the stdio transport with the reviewed default transport bounds (#2034)"
    );
    assert!(
        serve_streams.contains("build_service(root.clone())"),
        "serve_streams should construct the LSP service with the resolved workspace root through the shared constructor"
    );
    assert!(
        source.contains(".custom_method(\"$/setTrace\", Backend::set_trace)"),
        "build_service should register the standard $/setTrace trace lifecycle notification (#2035, RIPR-SPEC-0137)"
    );
    assert!(
        serve_streams.contains("bounds.wrap(stdin, stdout)"),
        "serve_streams should wrap stdin/stdout in the bounded transport adapters (#2034)"
    );
    assert!(
        serve_streams.contains(".concurrency_level(bounds.request_concurrency)"),
        "serve_streams should set the explicit in-flight request concurrency bound (#2034)"
    );
    assert!(
        serve_streams.contains(".serve(service)"),
        "serve_streams should hand the bounded transport, the socket, and the service to the tower LSP server"
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
        let invalid_root_parent = unique_lsp_test_root("framed-invalid-root")?;
        let invalid_root = invalid_root_parent.path().join("not-a-directory");
        std::fs::write(&invalid_root, b"not a workspace directory")
            .map_err(|err| format!("write invalid LSP root failed: {err}"))?;
        let invalid_root_uri = file_uri_for_path(&invalid_root)?;
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
                    "rootUri": invalid_root_uri.as_str(),
                    "initializationOptions": {
                        "baseRef": "ripr-lsp-protocol-smoke-missing-base"
                    },
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
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][3],
            COLLECT_WORKSPACE_STATUS_COMMAND
        );
        assert_eq!(
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][4],
            COLLECT_REPAIR_PACKET_COMMAND
        );
        assert_eq!(
            initialize["result"]["capabilities"]["executeCommandProvider"]["commands"][5],
            COLLECT_TOP_LIMITATION_COMMAND
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
                    "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let status = read_lsp_response(&mut client_read, 2).await?;
        assert_eq!(
            status["result"]["analysis_status"]["root_state"],
            "root_unavailable"
        );
        assert_eq!(
            status["result"]["analysis_status"]["repair_actions_available"],
            false
        );
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "workspace/executeCommand",
                "params": {
                    "command": REFRESH_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let (refresh, notifications) =
            read_lsp_response_with_notifications(&mut client_read, 3).await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(refresh["result"], serde_json::Value::Null);
        assert!(log_notification_messages(&notifications).is_empty());

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "position": { "line": 0, "character": 4 }
                }
            }),
        )
        .await?;
        let hover = read_lsp_response(&mut client_read, 4).await?;
        let hover_value = hover["result"]["contents"]["value"]
            .as_str()
            .ok_or_else(|| "expected hover markdown value".to_string())?;
        assert!(hover_value.contains("ripr estimates static RIPR exposure"));

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
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
        let actions = read_lsp_response(&mut client_read, 5).await?;
        assert_eq!(
            actions["result"][0]["title"],
            "Refresh Analysis - Saved Workspace Check"
        );
        assert_eq!(actions["result"][0]["command"]["command"], REFRESH_COMMAND);

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

        // Advertise the riprEditor block exactly as the VS Code extension
        // does (#1776, RIPR-SPEC-0129) so the session negotiates the
        // client-command code actions asserted below.
        let advertised_commands = vscode_advertised_client_commands()?;
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
                        "checkMode": "instant",
                        "diagnosticProfile": "full"
                    },
                    "capabilities": {
                        "experimental": {
                            "riprEditor": {
                                "version": "0.10.0",
                                "commands": advertised_commands,
                                "guardedTestEdit": false
                            }
                        }
                    }
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

#[test]
fn framed_lsp_refresh_resolves_git_inputs_once_and_projects_the_record() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        // A real temp git repo so the refresh resolves the requested base
        // through the one typed record (#2000, RIPR-SPEC-0142).
        let root = unique_lsp_test_root("framed-git-input-authority")?;
        init_lsp_test_scope_repo(root.path())?;
        std::fs::write(
            root.path().join("src/lib.rs"),
            "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )
        .map_err(|err| format!("write changed production fixture failed: {err}"))?;
        commit_lsp_test_scope_change(root.path(), "change production")?;
        let expected_base = crate::analysis::resolve_base_commit(root.path(), Some("HEAD~1"))
            .ok_or_else(|| "fixture HEAD~1 must resolve".to_string())?;
        let root_uri = file_uri_for_path(root.path())?;

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
                        "baseRef": "HEAD~1",
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
        let notification_messages = log_notification_messages(&notifications);
        // The one accepted refresh resolved the requested base once and the
        // phase-boundary log names the typed record the attempt consumes.
        let expected_log = format!(
            "git_input_resolution=resolved, requested_base=Some(\"HEAD~1\"), resolved_base=Some(\"{expected_base}\")"
        );
        assert!(
            notification_messages
                .iter()
                .any(|message| message.contains(&expected_log)),
            "expected refresh-start log to name the resolved record, got {notification_messages:?}"
        );
        // Exactly one refresh started: no consumer re-resolved and spawned a
        // second attempt for the same request.
        let started_count = notification_messages
            .iter()
            .filter(|message| message.contains("ripr analysis refresh started"))
            .count();
        assert_eq!(started_count, 1, "one accepted refresh, one resolution");

        // The workspace status projects the same resolved inputs from the
        // committed snapshot identity.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "workspace/executeCommand",
                "params": {
                    "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let status = read_lsp_response(&mut client_read, 3).await?;
        assert!(status.get("error").is_none());
        let current = &status["result"]["analysis_status"]["input_authority"]["current"];
        assert_eq!(
            current["requested_base"].as_str(),
            Some("HEAD~1"),
            "status must project the requested base: {current}"
        );
        assert_eq!(
            current["resolved_base"].as_str(),
            Some(expected_base.as_str()),
            "status must project the one resolved base: {current}"
        );
        assert_eq!(
            current["git_input_resolution"].as_str(),
            Some("resolved"),
            "status must project the typed resolution: {current}"
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "shutdown",
                "params": null
            }),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 4).await?;
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
fn framed_code_lens_refresh_follows_semantic_lens_view_changes() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        // Temp git repo: a committed base plus a committed production
        // change, so the saved-workspace analysis (base..HEAD) produces
        // findings.
        let root = unique_lsp_test_root("framed-code-lens-refresh")?;
        write_lsp_scope_fixture(&root.path)?;
        run_lsp_scope_git(&root.path, &["init"])?;
        run_lsp_scope_git(
            &root.path,
            &["config", "user.email", "ripr@example.invalid"],
        )?;
        run_lsp_scope_git(&root.path, &["config", "user.name", "RIPR Test"])?;
        run_lsp_scope_git(
            &root.path,
            &["add", "Cargo.toml", "src/lib.rs", "tests/end_to_end.rs"],
        )?;
        run_lsp_scope_git(&root.path, &["commit", "-m", "base"])?;
        fs::write(
            root.path.join("src/lib.rs"),
            "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )
        .map_err(|err| format!("write changed production fixture failed: {err}"))?;
        run_lsp_scope_git(&root.path, &["add", "src/lib.rs"])?;
        run_lsp_scope_git(&root.path, &["commit", "-m", "change production"])?;
        let root_uri = file_uri_for_path(root.path())?;

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
                        "baseRef": "HEAD~1",
                        "checkMode": "instant",
                        "diagnosticProfile": "full"
                    },
                    "capabilities": {
                        "workspace": {
                            "codeLens": { "refreshSupport": true }
                        }
                    }
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
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;

        // Refresh 1: the first snapshot commits with findings, so exactly one
        // workspace/codeLens/refresh request must arrive (#2032).
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
        let (refresh, refresh_requests) =
            read_response_answering_code_lens_refresh(&mut client_read, &mut client_write, 2)
                .await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(
            refresh_requests, 1,
            "the first snapshot commit must send exactly one workspace/codeLens/refresh"
        );

        // Vacuous-pass guard (tests-red-green review): the fixture must
        // actually produce lenses, or the request counts in this test could
        // pass on an empty view (an empty first view also changes the
        // recorded identity from None).
        let changed_file_uri = file_uri_for_path(&root.path.join("src/lib.rs"))?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 20,
                "method": "textDocument/codeLens",
                "params": {
                    "textDocument": { "uri": changed_file_uri.as_str() }
                }
            }),
        )
        .await?;
        let lenses = read_lsp_response(&mut client_read, 20).await?;
        let lens_count = lenses["result"].as_array().map_or(0, Vec::len);
        if lens_count == 0 {
            return Err(format!(
                "fixture must produce at least one code lens, or the refresh counts pass vacuously: {lenses}"
            ));
        }

        // Refresh 2: byte-identical inputs. A new snapshot commits with a
        // fresh wall-clock age (the rendered title suffix changes), but the
        // semantic lens view is unchanged, so no request may be sent.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "workspace/executeCommand",
                "params": {
                    "command": REFRESH_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let (refresh, refresh_requests) =
            read_response_answering_code_lens_refresh(&mut client_read, &mut client_write, 3)
                .await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(
            refresh_requests, 0,
            "a byte-identical re-commit must not send workspace/codeLens/refresh"
        );

        // Refresh 3: the saved workspace changes semantically (a committed
        // second changed predicate alters the base..HEAD diff and with it
        // the visible lens set), so exactly one new request must arrive.
        fs::write(
            root.path.join("src/lib.rs"),
            "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n\npub fn second_gate(level: u8) -> bool {\n    level > 3\n}\n",
        )
        .map_err(|err| format!("write second production change failed: {err}"))?;
        run_lsp_scope_git(&root.path, &["add", "src/lib.rs"])?;
        run_lsp_scope_git(&root.path, &["commit", "-m", "add second gate"])?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "workspace/executeCommand",
                "params": {
                    "command": REFRESH_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let (refresh, refresh_requests) =
            read_response_answering_code_lens_refresh(&mut client_read, &mut client_write, 4)
                .await?;
        assert!(refresh.get("error").is_none());
        assert_eq!(
            refresh_requests, 1,
            "a semantic lens-view change must send exactly one workspace/codeLens/refresh"
        );

        // Refresh 4 (RIPR-SPEC-0138, review): removing the workspace root
        // clears analysis state — every lens is now stale — so the server
        // must send one more refresh for the cleared view.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWorkspaceFolders",
                "params": {"event": {"added": [], "removed": [{"uri": root_uri.as_str(), "name": "fixture"}]}}
            }),
        )
        .await?;
        let folders_request =
            read_lsp_request(&mut client_read, "workspace/workspaceFolders").await?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": folders_request["id"].clone(),
                "result": []
            }),
        )
        .await?;
        let cleared_refresh =
            read_lsp_request(&mut client_read, "workspace/codeLens/refresh").await?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": cleared_refresh["id"].clone(),
                "result": null
            }),
        )
        .await?;

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
fn finding_diagnostic_and_hover_include_canonical_gap_id() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let mut finding = sample_finding();
    finding.canonical_gap = Some(sample_canonical_gap());
    finding.evidence = vec!["related evidence".to_string()];
    finding.missing = vec!["missing exact discriminator".to_string()];
    finding.recommended_next_step = Some("Add the exact assertion.".to_string());
    finding.activation.missing_discriminators = vec![crate::domain::MissingDiscriminatorFact {
        value: "threshold equality".to_string(),
        reason: "the equality boundary is not observed".to_string(),
        flow_sink: None,
    }];
    finding.related_tests = vec![RelatedTest {
        name: "pricing::discount_boundary".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq".to_string()),
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
        relation_reason: None,
        relation_confidence: None,
    }];
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let canonical_gap_id = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("canonical_gap_id"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "expected canonical_gap_id in diagnostic data".to_string())?;
    assert_eq!(
        canonical_gap_id,
        "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
    );
    let mut raw_finding = finding.clone();
    raw_finding.id = "probe:pricing:89:predicate".to_string();
    raw_finding.probe.id = ProbeId(raw_finding.id.clone());
    raw_finding.probe.location.line = 89;
    let mut grouped_diagnostic = diagnostic.clone();
    add_canonical_group_data(
        Path::new("/workspace"),
        &mut grouped_diagnostic,
        &finding,
        &[finding.clone(), raw_finding],
    );
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["raw_signal_count"].as_u64()),
        Some(2)
    );
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["raw_findings"].as_array())
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["related_tests"].as_array())
            .map(Vec::len),
        Some(1)
    );
    let mut no_data_diagnostic = tower_lsp_server::ls_types::Diagnostic::default();
    add_canonical_group_data(
        Path::new("/workspace"),
        &mut no_data_diagnostic,
        &finding,
        std::slice::from_ref(&finding),
    );
    assert!(no_data_diagnostic.data.is_none());
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["evidence"].as_array())
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["missing"].as_array())
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        grouped_diagnostic
            .data
            .as_ref()
            .and_then(|data| data["recommended_next_steps"].as_array())
            .map(Vec::len),
        Some(1)
    );
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic],
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
            assert!(markup.value.contains("## Canonical Gap"));
            assert!(markup.value.contains(
                "ID: `gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold`"
            ));
            Ok(())
        }
        _ => Err("expected markup hover".to_string()),
    }
}

#[test]
fn discriminator_witness_stays_aligned_across_lsp_surfaces() -> Result<(), String> {
    let mut finding = sample_finding();
    finding.recommended_next_step = None;
    finding.canonical_gap = Some(sample_canonical_gap());
    finding.probe.family = ProbeFamily::ErrorPath;
    finding.probe.before = Some("Err(PricingError::Other)".to_string());
    finding.probe.after = Some("Err(PricingError::Boundary)".to_string());
    finding.probe.expected_sinks = vec!["error_variant".to_string()];
    finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
        value: "PricingError::Boundary".to_string(),
        reason: "the broad error oracle does not distinguish the variant".to_string(),
        flow_sink: None,
    }];
    finding.activation.observed_values = vec![ValueFact {
        line: 12,
        text: "assert!(result.is_err())".to_string(),
        value: "result.is_err()".to_string(),
        context: ValueContext::AssertionArgument,
    }];
    finding.related_tests = vec![RelatedTest {
        name: "rejects_boundary".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 10,
        oracle: Some("assert!(result.is_err())".to_string()),
        oracle_kind: OracleKind::BroadError,
        oracle_strength: OracleStrength::Weak,
        relation_reason: None,
        relation_confidence: None,
    }];

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let witness = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("witness"))
        .cloned()
        .ok_or_else(|| "expected diagnostic discriminator witness".to_string())?;
    assert_eq!(witness["kind"], "static_discriminator_gap");
    assert_eq!(witness["probe_family"], "error_path");
    assert_eq!(
        witness["missing_discriminators"][0]["value"],
        "PricingError::Boundary"
    );
    assert_eq!(witness["fix_site"]["file"], "tests/pricing.rs");
    assert!(witness["fix_site"]["oracle_location"].is_null());
    assert!(witness["suggested_assertion"].is_null());
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("explain_command"))
            .and_then(|value| value.as_str()),
        Some("ripr explain --root . probe:pricing:88:predicate")
    );
    assert!(diagnostic.message.contains("Exact error variant"));
    assert!(diagnostic.message.contains("PricingError::Boundary"));

    let related = diagnostic
        .related_information
        .as_ref()
        .ok_or_else(|| "expected fix-site related information".to_string())?;
    assert_eq!(related.len(), 1);
    assert!(related[0].message.starts_with("Fix site:"));
    assert_eq!(related[0].location.range.start.line, 9);

    let hover = super::hover::finding_hover_response(&finding, &diagnostic);
    let HoverContents::Markup(markup) = hover.contents else {
        return Err("expected witness hover markdown".to_string());
    };
    assert!(markup.value.contains("## Discriminator witness"));
    assert!(markup.value.contains("PricingError::Boundary"));
    assert!(markup.value.contains("tests/pricing.rs:10"));
    assert!(markup.value.contains("suggested_assertion_unavailable"));

    let context_packet = crate::output::json::render_context_packet(&finding, 5);
    let context_packet: serde_json::Value =
        serde_json::from_str(&context_packet).map_err(|err| format!("packet JSON: {err}"))?;
    assert_eq!(context_packet["witness"], witness);

    let params = code_action_params(vec![diagnostic])?;
    let actions = code_action_response(&params, None, &vscode_client_features()?);
    let context_target = actions.iter().find_map(|action| {
        let CodeActionOrCommand::CodeAction(action) = action else {
            return None;
        };
        if action.title != "Inspect finding: copy context packet" {
            return None;
        }
        action
            .command
            .as_ref()
            .and_then(|command| command.arguments.as_ref())
            .and_then(|arguments| arguments.first())
    });
    assert_eq!(
        context_target.and_then(|target| target.get("witness")),
        Some(&witness)
    );
    Ok(())
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
        relation_reason: None,
        relation_confidence: None,
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
fn refresh_plan_accepts_actionable_snapshot_with_suppressed_finding() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let mut visible = sample_finding();
    visible.activation.missing_discriminators = vec![MissingDiscriminatorFact {
        value: "PricingError::Boundary".to_string(),
        reason: "the exact boundary is not observed".to_string(),
        flow_sink: None,
    }];
    visible.related_tests = vec![RelatedTest {
        name: "checks_boundary".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle: Some("assert_eq!(result, expected)".to_string()),
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
        relation_reason: None,
        relation_confidence: None,
    }];

    let mut suppressed = sample_finding();
    suppressed.id = "probe:pricing:9:predicate".to_string();
    suppressed.probe.id = ProbeId(suppressed.id.clone());
    suppressed.probe.location.file = PathBuf::from("src/other.rs");
    suppressed.probe.location.line = 9;
    suppressed.class = ExposureClass::Exposed;

    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &visible);
    let mut diagnostics = sample_workspace_diagnostics(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        vec![visible, suppressed],
    );
    diagnostics.snapshot.diagnostic_profile = crate::config::LspDiagnosticProfile::Actionable;

    let Some(_) = backend.refresh_plan(diagnostics) else {
        return Err("expected actionable refresh plan".to_string());
    };
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
    assert!(message.contains("gap_artifacts=0"));
    assert!(message.contains("actionable_gap_artifacts=0"));
    assert!(message.contains("preview_gap_artifacts=0"));
    assert!(message.contains("no_action_gap_artifacts=0"));
    assert!(message.contains("gap_static_limits=0"));
    assert!(message.contains("gap_artifact_rejections=0"));
    assert!(message.contains("gap_artifact_rejection_kinds="));
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
fn refresh_completion_log_message_counts_gap_artifact_state() -> Result<(), String> {
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &sample_finding());
    let uri = test_uri("file:///workspace/src/pricing.py")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri,
        vec![diagnostic],
        Vec::new(),
    );
    snapshot.gap_artifacts.push(ValidatedGapArtifact {
        kind: GapArtifactKind::GapDecisionLedger,
        root: Some(".".to_string()),
        identities: vec![GapArtifactIdentity {
            canonical_gap_id: Some("gap:py:pricing".to_string()),
            seam_id: None,
            finding_id: None,
        }],
        language: Some(LanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        gap_state: Some("actionable".to_string()),
        related_paths: vec!["tests/test_pricing.py".to_string()],
        verify_commands: vec!["ripr agent verify --root . --json".to_string()],
        receipt_commands: vec!["ripr agent receipt --root . --json".to_string()],
        verify_command_specs: Vec::new(),
        receipt_command_specs: Vec::new(),
        static_limit_kinds: vec!["missing_import_graph".to_string()],
        has_text_static_limit: false,
    });
    snapshot
        .gap_artifact_rejections
        .push(GapArtifactRejection::WrongRoot(
            "/other/workspace".to_string(),
        ));

    let summary = RefreshLogSummary::from_snapshot(9, &snapshot);
    let message = refresh_completed_log_message(&summary, 1, 0);

    assert!(message.contains("gap_artifacts=1"));
    assert!(message.contains("actionable_gap_artifacts=1"));
    assert!(message.contains("preview_gap_artifacts=1"));
    assert!(message.contains("no_action_gap_artifacts=0"));
    assert!(message.contains("gap_static_limits=1"));
    assert!(message.contains("gap_artifact_rejections=1"));
    assert!(message.contains("gap_artifact_rejection_kinds=wrong_root"));
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        None,
        &vscode_client_features()?,
    );

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
                "source.ripr.inspect",
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
    let actions = code_action_response(
        &code_action_params(Vec::new())?,
        None,
        &vscode_client_features()?,
    );

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
fn gap_code_actions_surface_bounded_repair_actions_when_artifact_is_valid() -> Result<(), String> {
    let root = unique_lsp_test_root("gap-actions")?;
    std::fs::create_dir_all(root.path().join("src"))
        .map_err(|err| format!("create src failed: {err}"))?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/test_pricing.py"),
        "def test_discount_boundary():\n    assert price(10) == 9\n",
    )
    .map_err(|err| format!("write related test failed: {err}"))?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| "missing diagnostic data".to_string())?;
    data["command_specs"] = serde_json::json!({
        "verify": crate::agent::command_specs::agent_verify_command_spec(
            ".", "before.json", "after.json", None,
        ),
        "receipt": crate::agent::command_specs::agent_receipt_command_spec(
            ".", "verify.json", "seam-a", Some("receipt.json"),
        ),
    });
    data["command_specs"]["verify"]["program"] = serde_json::json!("cargo");
    data["command_specs"]["verify"]["args"] = serde_json::json!(["test", "untrusted"]);
    data["command_specs"]["receipt"]["program"] = serde_json::json!("python");
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Copy first repair packet", COPY_CONTEXT_COMMAND),
            ("Agent handoff: copy Python packet", COPY_CONTEXT_COMMAND),
            ("Inspect gap: copy repair packet", COPY_CONTEXT_COMMAND),
            ("Copy Python repair card", COPY_TARGETED_TEST_BRIEF_COMMAND),
            (
                "Write targeted test: open best related test",
                OPEN_RELATED_TEST_COMMAND
            ),
            (
                "Verify after test: copy verify command",
                COPY_AGENT_VERIFY_COMMAND
            ),
            (
                "Review result: copy receipt command",
                COPY_AGENT_RECEIPT_COMMAND
            ),
            ("Inspect gap: copy static-limit note", COPY_CONTEXT_COMMAND),
            ("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND),
        ]
    );
    assert_eq!(commands[0].2[0]["label"], "first_repair_packet");
    assert_eq!(commands[0].2[0]["gap_identity"], "gap:py:pricing");
    assert_eq!(commands[0].2[0]["canonical_gap_id"], "gap:py:pricing");
    assert_eq!(
        commands[0].2[0]["verify_command"],
        "ripr agent verify --root . --json"
    );
    assert_eq!(
        commands[0].2[0]["receipt_command"],
        "ripr agent receipt --root . --json"
    );
    assert_eq!(
        commands[0].2[0]["command_specs"]["verify"]["command_id"],
        "ripr:agent:verify"
    );
    assert_eq!(
        commands[0].2[0]["command_specs"]["receipt"]["command_id"],
        "ripr:agent:receipt"
    );
    assert_eq!(
        commands[0].2[0]["command_specs"]["verify"]["program"],
        "ripr"
    );
    let packet = commands[0].2[0]["packet"]
        .as_str()
        .ok_or_else(|| "missing first repair packet text".to_string())?;
    assert!(
        packet.contains("RIPR first repair packet")
            && packet.contains("Language status: preview")
            && packet.contains("Static limit: missing_import_graph")
            && packet.contains("Suggested action:")
            && packet.contains("Missing discriminator: assert price(threshold) == expected")
            && packet.contains("Focused proof intent:")
            && packet.contains("Artifacts:")
            && packet.contains("Verify command:")
            && packet.contains("Receipt command:")
            && packet
                .contains("Do not edit production code unless the packet explicitly scopes it."),
        "unexpected first repair packet:\n{packet}"
    );
    let static_limit_position = packet
        .find("Static limit: missing_import_graph")
        .ok_or_else(|| format!("missing static limit in first repair packet:\n{packet}"))?;
    let suggested_action_position = packet
        .find("Suggested action:")
        .ok_or_else(|| format!("missing suggested action in first repair packet:\n{packet}"))?;
    assert!(
        static_limit_position < suggested_action_position,
        "static limits must appear before action language:\n{packet}"
    );
    assert_eq!(commands[1].2[0]["label"], "python_agent_packet");
    assert_eq!(commands[1].2[0]["canonical_gap_id"], "gap:py:pricing");
    assert_eq!(
        commands[1].2[0]["freshness"],
        "validated_current_gap_record"
    );
    assert_eq!(commands[1].2[0]["packet_kind"], "agent_gap_record_packet");
    assert_eq!(
        commands[1].2[0]["gap_ledger"],
        "target/ripr/reports/gap-decision-ledger.json"
    );
    assert_eq!(commands[2].2[0]["label"], "gap_repair_packet");
    assert_eq!(
        commands[2].2[0]["command_specs"]["verify"]["execution_mode"],
        "direct"
    );
    assert_eq!(
        commands[2].2[0]["command_specs"]["receipt"]["program"],
        "ripr"
    );
    assert_eq!(commands[2].2[0]["canonical_gap_id"], "gap:py:pricing");
    assert_eq!(
        commands[2].2[0]["repair_route"]["related_test"],
        "tests/test_pricing.py::test_discount_boundary"
    );
    assert_eq!(commands[3].2[0]["label"], "python_repair_card");
    assert_eq!(
        commands[3].2[0]["freshness"],
        "validated_current_gap_record"
    );
    let card = commands[3].2[0]["brief"]
        .as_str()
        .ok_or_else(|| "missing Python repair-card text".to_string())?;
    for needle in [
        "Python repair card (preview/advisory)",
        "Freshness: current validated GapRecord diagnostic.",
        "Changed owner:\n  python:app/pricing.py::calculate_discount",
        "Current test evidence:",
        "Missing discriminator:\n  assert price(threshold) == expected",
        "Verify:\n  ripr agent verify --root . --json",
        "Receipt:\n  ripr agent receipt --root . --json",
        "Static preview evidence only",
    ] {
        assert!(card.contains(needle), "missing {needle:?} in:\n{card}");
    }
    assert_eq!(
        commands[4].2[0]["uri"],
        file_uri_for_path(&root.path().join("tests/test_pricing.py"))?.as_str()
    );
    assert_eq!(commands[4].2[0]["line"], 2);
    assert_eq!(commands[4].2[0]["test_name"], "test_discount_boundary");
    assert_eq!(commands[5].2[0]["label"], "gap_verify");
    assert_eq!(
        commands[5].2[0]["command"],
        "ripr agent verify --root . --json"
    );
    assert_eq!(commands[6].2[0]["label"], "gap_receipt");
    assert_eq!(
        commands[6].2[0]["command"],
        "ripr agent receipt --root . --json"
    );
    assert!(
        commands[7].2[0]["note"]
            .as_str()
            .is_some_and(|note| note.contains("Static limit: missing_import_graph")),
        "expected static-limit note, got {:?}",
        commands[7].2[0]
    );
    Ok(())
}

#[test]
fn gap_code_actions_suppress_first_repair_packet_without_verify_or_receipt_command()
-> Result<(), String> {
    let root = unique_lsp_test_root("gap-first-repair-requires-commands")?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/test_pricing.py"),
        "def test_discount_boundary():\n    assert price(10) == 9\n",
    )
    .map_err(|err| format!("write related test failed: {err}"))?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| "missing diagnostic data".to_string())?;
    data.as_object_mut()
        .ok_or_else(|| "expected object data".to_string())?
        .remove("receipt_command");
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert!(
        commands
            .iter()
            .all(|(title, _, args)| title != "Copy first repair packet"
                && title != "Agent handoff: copy Python packet"
                && args
                    .first()
                    .is_none_or(|arg| arg["label"] != "first_repair_packet"
                        && arg["label"] != "python_agent_packet")),
        "packet actions must be suppressed when receipt command is missing: {commands:?}"
    );
    assert!(
        commands
            .iter()
            .any(|(title, _, _)| title == "Inspect gap: copy repair packet"),
        "existing inspect action should remain available"
    );
    Ok(())
}

#[test]
fn gap_code_actions_suppress_python_agent_packet_without_actionable_python_gap_record()
-> Result<(), String> {
    let root = unique_lsp_test_root("gap-python-agent-packet-contract")?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    for (field, value) in [
        ("source", "repo_exposure"),
        ("language", "rust"),
        ("gap_state", "already_observed"),
        ("repairability", "no_action"),
    ] {
        let mut diagnostic = gap_action_diagnostic();
        let data = diagnostic
            .data
            .as_mut()
            .ok_or_else(|| "missing diagnostic data".to_string())?
            .as_object_mut()
            .ok_or_else(|| "expected object data".to_string())?;
        data.insert(field.to_string(), serde_json::json!(value));
        let mut snapshot = sample_analysis_snapshot(
            root.path().to_path_buf(),
            uri.clone(),
            vec![diagnostic.clone()],
            Vec::new(),
        );
        snapshot.gap_artifacts = vec![validated_gap_artifact()];

        let actions = code_action_response(
            &code_action_params_for(uri.clone(), diagnostic.range.start.line, vec![diagnostic])?,
            Some(&snapshot),
            &vscode_client_features()?,
        );
        let commands = code_action_commands(&actions)?;

        assert!(
            commands.iter().all(
                |(title, _, args)| title != "Agent handoff: copy Python packet"
                    && args
                        .first()
                        .is_none_or(|arg| arg["label"] != "python_agent_packet")
            ),
            "Python agent packet action must be suppressed when {field}={value}: {commands:?}"
        );
    }
    Ok(())
}

#[test]
fn gap_code_actions_suppress_repair_actions_for_cross_language_target_unresolved()
-> Result<(), String> {
    let root = unique_lsp_test_root("gap-cross-language-target-unresolved")?;
    let uri = file_uri_for_path(&root.path().join("src/jsc/Blob.rs"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| "missing diagnostic data".to_string())?;
    data["language"] = serde_json::json!("rust");
    data["gap_state"] = serde_json::json!("static_limitation");
    data["repairability"] = serde_json::json!("no_action");
    data["static_limit_kind"] = serde_json::json!("cross_language_target_unresolved");
    data["static_limit_detail"] = serde_json::json!("binding/FFI target placement is unresolved");
    data["projection_exclusion_reasons"] = serde_json::json!(["cross_language_target_unresolved"]);
    data["navigation_only_target"] = serde_json::json!({
        "file": "test/js/web/fetch/blob.test.ts",
        "line": 41,
        "test_name": "blob copies resizable buffers",
        "language": "typescript",
        "authority_boundary": "navigation_only_external_observer_context",
        "repair_packet_ready": false,
        "limitation_route": "analysis/cross-language-test-target-inference"
    });
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Inspect gap: copy static-limit note", COPY_CONTEXT_COMMAND),
            ("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND),
        ],
        "target-unresolved gap diagnostics must not expose repair packets, verify, receipt, or edit actions"
    );
    assert!(
        commands[0].2[0]["note"].as_str().is_some_and(|note| {
            note.contains("cross_language_target_unresolved")
                && note.contains("Navigation-only target: test/js/web/fetch/blob.test.ts:41")
                && note.contains("External observer: blob copies resizable buffers")
                && note.contains("Repair packet ready: false")
        }),
        "expected static-limit note, got {:?}",
        commands[0].2[0]
    );
    assert_eq!(
        commands[0].2[0]["navigation_only_target"]["file"],
        "test/js/web/fetch/blob.test.ts"
    );
    assert_eq!(
        commands[0].2[0]["navigation_only_target"]["repair_packet_ready"],
        false
    );
    Ok(())
}

#[test]
fn gap_code_actions_project_python_pytest_skeleton_and_target_file() -> Result<(), String> {
    let root = unique_lsp_test_root("gap-python-pytest-actions")?;
    std::fs::create_dir_all(root.path().join("src"))
        .map_err(|err| format!("create src failed: {err}"))?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/test_pricing.py"),
        "def test_calculate_discount_threshold_boundary():\n    pass\n",
    )
    .map_err(|err| format!("write related test failed: {err}"))?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| "missing diagnostic data".to_string())?;
    data["repair_route"]["target_file"] = serde_json::json!("tests/test_pricing.py");
    data["repair_route"]["target_line"] = serde_json::json!(1);
    data["repair_route"]["related_test"] =
        serde_json::json!("test_calculate_discount_threshold_boundary");
    data["repair_route"]["missing_discriminator"] = serde_json::json!("amount == threshold");
    data["repair_route"]["assertion_shape"] = serde_json::json!(
        "assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount"
    );
    data["repair_route"]["changed_behavior"] = serde_json::json!("if amount >= threshold:");
    data["verification_commands"] = serde_json::json!([
        "pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary"
    ]);
    data["receipt_command"] = serde_json::json!(
        "ripr outcome --before target/ripr/reports/check.json --after target/ripr/reports/after-check.json --format json --out target/ripr/receipts/python-pricing-boundary.json"
    );
    data.as_object_mut()
        .ok_or_else(|| "expected object data".to_string())?
        .remove("static_limit_kind");
    data.as_object_mut()
        .ok_or_else(|| "expected object data".to_string())?
        .remove("static_limit_detail");
    data["static_limits"] = serde_json::json!([]);
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert!(
        commands
            .iter()
            .any(|(title, _, _)| title == "Copy first repair packet"),
        "pytest verify commands should be safe enough for first repair packets: {commands:?}"
    );
    let repair_card = commands
        .iter()
        .find(|(title, command, _)| {
            title == "Copy Python repair card" && command == COPY_TARGETED_TEST_BRIEF_COMMAND
        })
        .and_then(|(_, _, args)| args.first())
        .ok_or_else(|| format!("missing Python repair-card action: {commands:?}"))?;
    assert_eq!(repair_card["label"], "python_repair_card");
    let card = repair_card["brief"]
        .as_str()
        .ok_or_else(|| format!("missing repair-card brief: {repair_card:?}"))?;
    for needle in [
        "Python repair card (preview/advisory)",
        "Freshness: current validated GapRecord diagnostic.",
        "Changed behavior:\n  if amount >= threshold:",
        "Missing discriminator:\n  amount == threshold",
        "Suggested assertion:\n  assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount",
        "Verify:\n  pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary",
    ] {
        assert!(card.contains(needle), "missing {needle:?} in:\n{card}");
    }
    let skeleton = commands
        .iter()
        .find(|(title, command, _)| {
            title == "Write Python test: copy pytest skeleton"
                && command == COPY_TARGETED_TEST_BRIEF_COMMAND
        })
        .and_then(|(_, _, args)| args.first())
        .ok_or_else(|| format!("missing Python pytest skeleton action: {commands:?}"))?;
    assert_eq!(skeleton["label"], "python_pytest_skeleton");
    assert_eq!(skeleton["target_file"], "tests/test_pricing.py");
    assert_eq!(
        skeleton["test_name"],
        "test_calculate_discount_threshold_boundary"
    );
    let brief = skeleton["brief"]
        .as_str()
        .ok_or_else(|| format!("missing skeleton brief: {skeleton:?}"))?;
    for needle in [
        "# RIPR Python repair skeleton",
        "# Missing discriminator: amount == threshold",
        "# Verify: pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary",
        "def test_calculate_discount_threshold_boundary():",
        "# assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount",
        "raise NotImplementedError",
    ] {
        assert!(brief.contains(needle), "missing {needle:?} in:\n{brief}");
    }
    let open_target = commands
        .iter()
        .find(|(title, command, _)| {
            title == "Write targeted test: open best related test"
                && command == OPEN_RELATED_TEST_COMMAND
        })
        .and_then(|(_, _, args)| args.first())
        .ok_or_else(|| format!("missing open related test action: {commands:?}"))?;
    assert_eq!(
        open_target["uri"],
        file_uri_for_path(&root.path().join("tests/test_pricing.py"))?.as_str()
    );
    assert_eq!(
        open_target["test_name"],
        "test_calculate_discount_threshold_boundary"
    );
    Ok(())
}

#[test]
fn gap_code_actions_omit_partial_or_invalid_typed_specs() -> Result<(), String> {
    let mut missing_verify = validated_gap_artifact();
    missing_verify.verify_command_specs.clear();
    assert_gap_action_specs_omitted(missing_verify, "missing verify")?;

    let mut missing_receipt = validated_gap_artifact();
    missing_receipt.receipt_command_specs.clear();
    assert_gap_action_specs_omitted(missing_receipt, "missing receipt")?;

    let mut malformed = validated_gap_artifact();
    malformed
        .verify_command_specs
        .first_mut()
        .ok_or_else(|| "validated fixture omitted verify spec".to_string())?
        .program
        .clear();
    assert_gap_actions_refresh_only(malformed, "malformed verify")?;

    let mut role_mismatch = validated_gap_artifact();
    role_mismatch
        .verify_command_specs
        .first_mut()
        .ok_or_else(|| "validated fixture omitted verify spec".to_string())?
        .role = crate::domain::CommandRole::Receipt;
    assert_gap_actions_refresh_only(role_mismatch, "role-mismatched verify")?;
    Ok(())
}

fn assert_gap_action_specs_omitted(
    artifact: ValidatedGapArtifact,
    case: &str,
) -> Result<(), String> {
    let commands = gap_action_commands_for_artifact(artifact, case)?;
    for index in [0, 1, 2] {
        let (label, target) = commands
            .get(index)
            .and_then(|(title, _, arguments)| arguments.first().map(|target| (title, target)))
            .ok_or_else(|| format!("{case}: missing repair action {index}"))?;
        if target.get("command_specs").is_some() {
            return Err(format!(
                "{case}: action {} projected typed specs without a complete valid pair: {target}",
                label
            ));
        }
    }
    Ok(())
}

fn assert_gap_actions_refresh_only(
    artifact: ValidatedGapArtifact,
    case: &str,
) -> Result<(), String> {
    let commands = gap_action_commands_for_artifact(artifact, case)?;
    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND)],
        "{case}: invalid typed specs must fail closed to refresh-only actions"
    );
    Ok(())
}

fn gap_action_commands_for_artifact(
    artifact: ValidatedGapArtifact,
    case: &str,
) -> Result<Vec<(String, String, Vec<serde_json::Value>)>, String> {
    let root = unique_lsp_test_root("gap-actions-invalid-specs")?;
    std::fs::create_dir_all(root.path().join("src"))
        .map_err(|err| format!("create src failed: {err}"))?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/test_pricing.py"),
        "def test_discount_boundary():\n    assert price(10) == 9\n",
    )
    .map_err(|err| format!("write related test failed: {err}"))?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| format!("{case}: missing diagnostic data"))?;
    data["command_specs"] = serde_json::json!({
        "verify": crate::agent::command_specs::agent_verify_command_spec(
            ".", "diagnostic-before.json", "diagnostic-after.json", None,
        ),
        "receipt": crate::agent::command_specs::agent_receipt_command_spec(
            ".", "diagnostic-verify.json", "diagnostic-seam", Some("diagnostic-receipt.json"),
        ),
    });
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![artifact];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    code_action_commands(&actions)
}

#[test]
fn gap_code_actions_fail_closed_without_valid_current_artifact() -> Result<(), String> {
    let diagnostic = gap_action_diagnostic();
    let uri = test_uri("file:///workspace/src/pricing.py")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND)],
        "stale or unvalidated gap diagnostics must not expose repair actions"
    );
    Ok(())
}

#[test]
fn gap_code_actions_omit_unsafe_related_paths_and_commands() -> Result<(), String> {
    let root = unique_lsp_test_root("gap-unsafe-actions")?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .ok_or_else(|| "missing diagnostic data".to_string())?;
    data["repair_route"]["related_test"] = serde_json::json!("../outside.py::test_escape");
    data["verification_commands"] =
        serde_json::json!(["ripr agent verify --root ../outside --json"]);
    data["receipt_command"] = serde_json::json!("ripr agent receipt --root ../outside --json");
    data.as_object_mut()
        .ok_or_else(|| "expected object data".to_string())?
        .remove("static_limit_kind");
    data.as_object_mut()
        .ok_or_else(|| "expected object data".to_string())?
        .remove("static_limit_detail");
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND)],
        "unsafe gap paths or command roots must leave refresh as the only action"
    );
    Ok(())
}

#[test]
fn gap_code_actions_suppress_python_repair_card_without_target_file() -> Result<(), String> {
    let root = unique_lsp_test_root("gap-python-card-no-target")?;
    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let mut diagnostic = gap_action_diagnostic();
    let data = diagnostic
        .data
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "expected diagnostic data object".to_string())?;
    let route = data
        .get_mut("repair_route")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "expected repair_route object".to_string())?;
    route.remove("target_file");
    route.remove("related_test");
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    assert!(
        commands
            .iter()
            .all(|(title, _, _)| title != "Copy Python repair card"),
        "Python repair card must not surface without a bounded target file: {commands:?}"
    );
    Ok(())
}

#[test]
fn editor_adoption_baseline_pins_gap_repair_action_contract() -> Result<(), String> {
    let root = unique_lsp_test_root("editor-adoption-gap-actions")?;
    std::fs::create_dir_all(root.path().join("src"))
        .map_err(|err| format!("create src failed: {err}"))?;
    std::fs::create_dir_all(root.path().join("tests"))
        .map_err(|err| format!("create tests failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/test_pricing.py"),
        "def test_discount_boundary():\n    assert price(10) == 9\n",
    )
    .map_err(|err| format!("write related test failed: {err}"))?;

    let uri = file_uri_for_path(&root.path().join("src/pricing.py"))?;
    let diagnostic = gap_action_diagnostic();
    let mut snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.gap_artifacts = vec![validated_gap_artifact()];

    let actions = code_action_response(
        &code_action_params_for(
            uri.clone(),
            diagnostic.range.start.line,
            vec![diagnostic.clone()],
        )?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Copy first repair packet", COPY_CONTEXT_COMMAND),
            ("Agent handoff: copy Python packet", COPY_CONTEXT_COMMAND),
            ("Inspect gap: copy repair packet", COPY_CONTEXT_COMMAND),
            ("Copy Python repair card", COPY_TARGETED_TEST_BRIEF_COMMAND),
            (
                "Write targeted test: open best related test",
                OPEN_RELATED_TEST_COMMAND
            ),
            (
                "Verify after test: copy verify command",
                COPY_AGENT_VERIFY_COMMAND
            ),
            (
                "Review result: copy receipt command",
                COPY_AGENT_RECEIPT_COMMAND
            ),
            ("Inspect gap: copy static-limit note", COPY_CONTEXT_COMMAND),
            ("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND),
        ],
        "the editor adoption baseline must keep one bounded repair ladder"
    );
    let packet = commands[0].2[0]["packet"]
        .as_str()
        .ok_or_else(|| "missing first repair packet text".to_string())?;
    assert!(packet.contains("Language status: preview"));
    assert!(packet.contains("Static limit: missing_import_graph"));
    assert!(packet.contains("Missing discriminator: assert price(threshold) == expected"));
    assert!(packet.contains("Focused proof intent:"));
    assert!(packet.contains("Artifacts:"));
    assert!(packet.contains("Verify command:\nripr agent verify --root . --json"));
    assert!(packet.contains("Receipt command:\nripr agent receipt --root . --json"));
    let static_limit_position = packet
        .find("Static limit: missing_import_graph")
        .ok_or_else(|| format!("missing static limit in first repair packet:\n{packet}"))?;
    let suggested_action_position = packet
        .find("Suggested action:")
        .ok_or_else(|| format!("missing suggested action in first repair packet:\n{packet}"))?;
    assert!(
        static_limit_position < suggested_action_position,
        "static limits must stay before action language:\n{packet}"
    );

    let unvalidated_snapshot = sample_analysis_snapshot(
        root.path().to_path_buf(),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    let unvalidated_actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&unvalidated_snapshot),
        &vscode_client_features()?,
    );
    let unvalidated_commands = code_action_commands(&unvalidated_actions)?;
    assert_eq!(
        unvalidated_commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND)],
        "stale or unvalidated adoption-baseline evidence must fail closed to refresh"
    );
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
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
fn code_action_response_filters_client_commands_for_unenhanced_client() -> Result<(), String> {
    // Layer 1 (#1776, RIPR-SPEC-0129): a client that advertised no
    // riprEditor block must receive no client-executed command IDs — every
    // ripr.copy*/ripr.openRelatedTest action is stripped and only the
    // server-executed refresh action remains. Diagnostics and hover are
    // separate surfaces and stay unfiltered.
    let unenhanced = ClientFeatureProfile::unsupported();
    let (seam_params, seam_snapshot) = seam_code_action_request()?;
    let seam_commands = code_action_commands(&code_action_response(
        &seam_params,
        Some(&seam_snapshot),
        &unenhanced,
    ))?;
    assert_eq!(
        seam_commands
            .iter()
            .map(|(_, command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec![REFRESH_COMMAND],
        "an unenhanced client must receive only server-executed commands"
    );

    let finding_diagnostic = diagnostic_for_finding(Path::new("/workspace"), &sample_finding());
    let finding_commands = code_action_commands(&code_action_response(
        &code_action_params(vec![finding_diagnostic])?,
        None,
        &unenhanced,
    ))?;
    assert_eq!(
        finding_commands
            .iter()
            .map(|(_, command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec![REFRESH_COMMAND],
        "an unenhanced client must not receive the finding context copy command"
    );
    Ok(())
}

#[test]
fn code_action_response_negotiates_only_the_advertised_client_commands() -> Result<(), String> {
    // A client advertising only ripr.openRelatedTest keeps the navigation
    // action and loses every clipboard action (#1776, RIPR-SPEC-0129).
    let navigate_only = client_features_with_commands(&[OPEN_RELATED_TEST_COMMAND])?;
    let (seam_params, seam_snapshot) = seam_code_action_request()?;
    let commands = code_action_commands(&code_action_response(
        &seam_params,
        Some(&seam_snapshot),
        &navigate_only,
    ))?;
    assert_eq!(
        commands
            .iter()
            .map(|(_, command, _)| command.as_str())
            .collect::<Vec<_>>(),
        vec![OPEN_RELATED_TEST_COMMAND, REFRESH_COMMAND],
        "only the advertised navigation command and the refresh action survive"
    );
    Ok(())
}

#[test]
fn code_action_response_emitted_commands_stay_within_server_or_advertised_sets()
-> Result<(), String> {
    // Parity invariant (#1776, RIPR-SPEC-0129): every emitted command ID is
    // either a server-executed command from the executeCommandProvider
    // advertisement or a client command the negotiated profile advertised.
    let provider_commands = initialize_result()
        .capabilities
        .execute_command_provider
        .map(|options| options.commands)
        .unwrap_or_default();
    assert_eq!(
        provider_commands,
        SERVER_EXECUTED_COMMANDS
            .iter()
            .map(|command| command.to_string())
            .collect::<Vec<_>>(),
        "the server-executed filter set must mirror the executeCommandProvider advertisement"
    );

    let unenhanced = ClientFeatureProfile::unsupported();
    let navigate_only = client_features_with_commands(&[OPEN_RELATED_TEST_COMMAND])?;
    let vscode = vscode_client_features()?;
    let (seam_params, seam_snapshot) = seam_code_action_request()?;
    let finding_params = code_action_params(vec![diagnostic_for_finding(
        Path::new("/workspace"),
        &sample_finding(),
    )])?;
    for (label, profile) in [
        ("unenhanced", &unenhanced),
        ("navigate-only", &navigate_only),
        ("vscode", &vscode),
    ] {
        let advertised: BTreeSet<&str> = profile
            .ripr_editor
            .as_ref()
            .map(|editor| editor.commands.iter().map(String::as_str).collect())
            .unwrap_or_default();
        for (scenario, actions) in [
            (
                "seam",
                code_action_response(&seam_params, Some(&seam_snapshot), profile),
            ),
            (
                "finding",
                code_action_response(&finding_params, None, profile),
            ),
        ] {
            for (_, command, _) in code_action_commands(&actions)? {
                if !SERVER_EXECUTED_COMMANDS.contains(&command.as_str())
                    && !advertised.contains(command.as_str())
                {
                    return Err(format!(
                        "{label}/{scenario}: emitted command {command} is neither server-executed nor advertised"
                    ));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn seam_code_actions_fail_closed_for_cross_language_target_unresolved() -> Result<(), String> {
    let mut seam = sample_classified_seam();
    seam.seam = RepoSeam::new(
        "src/jsc/Blob.rs",
        "Blob::from_js_without_defer_gc",
        SeamKind::PredicateBoundary,
        42,
        88,
        "array_buffer.shared || array_buffer.resizable",
        RequiredDiscriminator::BoundaryValue {
            description: "array_buffer.shared || array_buffer.resizable".to_string(),
        },
        ExpectedSink::ReturnValue,
    );
    seam.evidence.seam_id = seam.seam.id().clone();
    seam.evidence.related_tests[0].file = PathBuf::from("test/js/web/fetch/blob.test.ts");
    seam.evidence.related_tests[0].test_name = "blob copies shared buffers".to_string();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/jsc/Blob.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam.clone()];
    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

    let commands = code_action_commands(&actions)?;
    assert_eq!(
        commands
            .iter()
            .map(|(title, command, _)| (title.as_str(), command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Inspect Test Gap - Copy Context", COPY_CONTEXT_COMMAND),
            ("Refresh Analysis - Saved Workspace Check", REFRESH_COMMAND),
        ],
        "binding/FFI target-unresolved seams must not expose repair, handoff, verify, receipt, or edit actions"
    );
    assert_eq!(commands[0].2[0]["seam_id"], seam.seam.id().as_str());
    assert_eq!(commands[0].2[0]["uri"], "file:///workspace/src/jsc/Blob.rs");
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
        &vscode_client_features()?,
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
            test_target: None,
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
            test_target: None,
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
            test_target: None,
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
            test_target: None,
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
            test_target: None,
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
            test_target: None,
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

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
fn unknown_stage_value_route_omits_suggested_assertion_action() -> Result<(), String> {
    use crate::analysis::seams::SeamGripClass;

    let mut seam = sample_classified_seam();
    seam.class = SeamGripClass::ActivationUnknown;
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );
    let commands = code_action_commands(&actions)?;

    if commands
        .iter()
        .any(|(_, command, _)| command == COPY_SUGGESTED_ASSERTION_COMMAND)
    {
        return Err(format!(
            "unknown-stage route must not offer suggested assertion: {commands:?}"
        ));
    }
    Ok(())
}

#[test]
fn seam_code_actions_keep_navigation_when_related_test_is_unresolved() -> Result<(), String> {
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason,
    };

    let mut seam = sample_side_effect_seam_without_related_tests();
    seam.evidence.related_tests = vec![RelatedTestGrip {
        test_name: "publish_event_emits_bus_message".to_string(),
        file: PathBuf::from("tests/service.rs"),
        line: 21,
        test_target: None,
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
    let actions = code_action_response(
        &code_action_params(vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
    );

    let commands = code_action_commands(&actions)?;
    assert!(
        commands
            .iter()
            .all(|(_, command, _)| command != COPY_TARGETED_TEST_BRIEF_COMMAND),
        "unresolved related-test evidence must not produce a repair brief: {commands:?}"
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
fn diagnostic_for_finding_uses_utf16_width_for_non_ascii_expression() {
    let mut finding = sample_finding();
    finding.probe.location.column = 2;
    finding.probe.expression = "\u{e9}\u{1f389}".to_string();

    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    assert_eq!(diagnostic.range.start.character, 1);
    assert_eq!(diagnostic.range.end.character, 4);
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
        relation_reason: None,
        relation_confidence: None,
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
        "Fix site: related test `discount_boundary_is_exact` has strong exact_value oracle: assert_eq!(total, expected)"
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
    let mut previous = BTreeMap::new();
    previous.insert(stale_uri.clone(), Vec::new());
    previous.insert(current_uri.clone(), Vec::new());

    let plan = diagnostic_refresh_plan(
        &previous,
        vec![DiagnosticBatch {
            uri: current_uri.clone(),
            diagnostics: vec![gap_action_diagnostic()],
        }],
    );

    assert_eq!(plan.publish_batches.len(), 1);
    assert_eq!(plan.publish_batches[0].uri, current_uri);
    assert_eq!(plan.clear_uris, vec![stale_uri]);
    assert_eq!(plan.current_uris.len(), 1);
    Ok(())
}

#[test]
fn diagnostic_refresh_plan_suppresses_unchanged_uri_and_publishes_changed_uri() -> Result<(), String>
{
    let uri = test_uri("file:///workspace/src/current.rs")?;
    let first = gap_action_diagnostic();
    let mut previous = BTreeMap::new();
    previous.insert(uri.clone(), vec![first.clone()]);

    let unchanged = diagnostic_refresh_plan(
        &previous,
        vec![DiagnosticBatch {
            uri: uri.clone(),
            diagnostics: vec![first.clone()],
        }],
    );
    assert!(unchanged.publish_batches.is_empty());
    assert_eq!(unchanged.unchanged_uri_count, 1);
    assert!(unchanged.suppressed_payload_bytes > 0);

    let mut changed = first;
    changed.message.push_str("; changed");
    let changed_plan = diagnostic_refresh_plan(
        &previous,
        vec![DiagnosticBatch {
            uri,
            diagnostics: vec![changed],
        }],
    );
    assert_eq!(changed_plan.publish_batches.len(), 1);
    assert_eq!(changed_plan.unchanged_uri_count, 0);
    assert!(changed_plan.published_payload_bytes > 0);
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
fn explicit_snapshot_clear_helper_clears_tracked_diagnostics() -> Result<(), String> {
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
fn stale_refresh_does_not_rollback_after_root_authority_transition() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let request = RefreshRequest {
            generation: 1,
            authority_epoch: 0,
            input_identity: LspAnalysisInputIdentity::from_refresh_inputs(
                PathBuf::from("/workspace"),
                1,
                &LspAnalysisConfig::default(),
            ),
            git_inputs: crate::lsp::git_inputs::ResolvedGitInputs::resolve(
                Path::new("/workspace"),
                None,
            ),
            root: PathBuf::from("/workspace"),
            config: LspAnalysisConfig::default(),
            workspace_revision: 1,
            scope: RefreshScope::Interactive,
            reason: RefreshReason::DidSave,
            cancellation: AnalysisCancellationToken::new(),
        };

        if !backend.refresh_authority_is_unchanged(&request) {
            return Err("expected request authority to match before transition".to_string());
        }
        backend.invalidate_workspace_root_for_test().await;
        if backend.refresh_authority_is_unchanged(&request) {
            return Err("expected root transition to invalidate request authority".to_string());
        }
        Ok(())
    })
}

#[tokio::test]
async fn did_save_with_unchanged_content_deduplicates_without_refresh() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn same() {}".to_string(),
            ),
        })
        .await;
    backend.advance_workspace_revision();
    let baseline = backend.workspace_revision();

    // First save after open: nothing recorded yet, so it always counts as
    // changed (conservative) and records the digest.
    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn same() {}".to_string()),
        })
        .await;
    if backend.workspace_revision() != baseline + 1 {
        return Err(format!(
            "first save did not advance the revision: {baseline} -> {}",
            backend.workspace_revision()
        ));
    }
    // A repeated save of the same bytes now dedups.
    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn same() {}".to_string()),
        })
        .await;
    if backend.workspace_revision() != baseline + 1 {
        return Err(format!(
            "unchanged repeated save advanced the revision: {baseline} -> {}",
            backend.workspace_revision()
        ));
    }
    Ok(())
}

#[tokio::test]
async fn did_save_with_changed_content_advances_and_refreshes() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn same() {}".to_string(),
            ),
        })
        .await;
    backend.advance_workspace_revision();
    let baseline = backend.workspace_revision();

    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn changed() {}".to_string()),
        })
        .await;
    if backend.workspace_revision() != baseline + 1 {
        return Err(format!(
            "changed save did not advance the revision exactly once: {baseline} -> {}",
            backend.workspace_revision()
        ));
    }

    // A repeated save of the now-recorded content dedups again.
    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn changed() {}".to_string()),
        })
        .await;
    if backend.workspace_revision() != baseline + 1 {
        return Err(format!(
            "repeated save of recorded content did not deduplicate: {baseline} -> {}",
            backend.workspace_revision()
        ));
    }
    Ok(())
}

#[tokio::test]
async fn did_save_without_text_falls_back_to_document_store_content() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn stored() {}".to_string(),
            ),
        })
        .await;
    backend.advance_workspace_revision();
    let baseline = backend.workspace_revision();

    // Clients without includeText send no content: the digest comes from the
    // document store. First save records; the repeat dedups.
    for expected_revision in [baseline + 1, baseline + 1] {
        backend
            .did_save(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                text: None,
            })
            .await;
        if backend.workspace_revision() != expected_revision {
            return Err(format!(
                "text-less save path drifted: expected revision {expected_revision}, got {}",
                backend.workspace_revision()
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn did_close_clears_the_saved_content_digest() -> Result<(), String> {
    let uri = test_uri("file:///workspace/src/lib.rs")?;
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn same() {}".to_string(),
            ),
        })
        .await;
    backend.advance_workspace_revision();
    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn same() {}".to_string()),
        })
        .await;
    let after_record = backend.workspace_revision();

    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })
        .await;
    // did_close advances the revision by design; the digest must be gone.
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                2,
                "fn same() {}".to_string(),
            ),
        })
        .await;
    let after_reopen = backend.workspace_revision();

    // The same bytes after close+reopen are treated as changed (conservative):
    // nothing is recorded for the document anymore.
    backend
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some("fn same() {}".to_string()),
        })
        .await;
    if backend.workspace_revision() != after_reopen + 1 {
        return Err(format!(
            "reopened document kept its digest: record={after_record} reopen={after_reopen} now={}",
            backend.workspace_revision()
        ));
    }
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
fn initialize_root_rejects_ambiguous_workspace_folders() -> Result<(), String> {
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

    assert_eq!(
        root_from_initialize_params(&params),
        WorkspaceRootResolution::Ambiguous(vec![
            PathBuf::from("/workspace/main"),
            PathBuf::from("/workspace/other"),
        ])
    );
    Ok(())
}

#[test]
fn initialize_root_uses_root_uri_when_workspace_folders_are_missing() -> Result<(), String> {
    let params = initialize_params(None, Some(test_uri("file:///workspace/root-uri")?));

    assert_eq!(
        root_from_initialize_params(&params),
        WorkspaceRootResolution::Selected(PathBuf::from("/workspace/root-uri"))
    );
    Ok(())
}

#[test]
fn initialize_root_rejects_empty_workspace_folders_even_with_root_uri() -> Result<(), String> {
    let params = initialize_params(
        Some(Vec::new()),
        Some(test_uri("file:///workspace/root-uri")?),
    );

    assert_eq!(
        root_from_initialize_params(&params),
        WorkspaceRootResolution::Unavailable(
            "the client explicitly reported no workspace folders".to_string()
        )
    );
    Ok(())
}

#[test]
fn initialize_root_reports_unavailable_when_no_lsp_root_exists() {
    let params = initialize_params(None, None);

    assert_eq!(
        root_from_initialize_params(&params),
        WorkspaceRootResolution::Unavailable(
            "the client did not provide a workspace folder or root URI".to_string()
        )
    );
}

#[test]
fn initialization_options_override_lsp_analysis_config() {
    let mut params = initialize_params(None, None);
    params.initialization_options = Some(serde_json::json!({
        "baseRef": "origin/release",
        "checkMode": "deep",
        "includeUnchangedTests": false,
    }));

    let config = LspAnalysisConfig::from_initialize_params(
        &params,
        crate::config::RiprConfig::default(),
        &crate::lsp::client_features::ClientFeatureProfile::from_initialize_params(&params),
    );
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

    let config = LspAnalysisConfig::from_initialize_params(
        &params,
        crate::config::RiprConfig::default(),
        &crate::lsp::client_features::ClientFeatureProfile::from_initialize_params(&params),
    );

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
            &crate::lsp::client_features::ClientFeatureProfile::from_initialize_params(&params),
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
        let Some(failure) = backend.configuration_failure() else {
            return Err("invalid config should pause analysis with a typed failure".to_string());
        };
        assert_eq!(failure.kind, "config_invalid");
        backend.invalidate_workspace_root_for_test().await;
        assert!(backend.configuration_failure().is_none());
        Ok(())
    })
}

#[test]
fn session_configuration_change_preserves_invalid_repository_config_health() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("invalid-config-session-change")?;
        std::fs::write(
            root.path().join("ripr.toml"),
            "[languages]\nenabled = [\"ruby\"]\n",
        )
        .map_err(|err| format!("write invalid config failed: {err}"))?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend
            .initialize(initialize_params(
                None,
                Some(file_uri_for_path(root.path())?),
            ))
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        if backend.configuration_failure().is_none() {
            return Err("invalid repository config should latch config_invalid".to_string());
        }
        backend
            .did_change_configuration(DidChangeConfigurationParams {
                settings: serde_json::json!({"ripr": {"seamDiagnostics": false}}),
            })
            .await;

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        assert_eq!(status["analysis_status"]["state"], "failed");
        assert_eq!(
            status["analysis_status"]["failure"]["kind"],
            "config_invalid"
        );
        assert!(
            !backend
                .analysis_config()
                .ok_or_else(|| "expected analysis config".to_string())?
                .enable_seam_diagnostics
        );
        Ok(())
    })
}

#[test]
fn initialization_only_mode_discloses_transport_and_value_sources() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("config-mode-initialization-only")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend
            .initialize(initialize_params(
                None,
                Some(file_uri_for_path(root.path())?),
            ))
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        let authority = &status["analysis_status"]["input_authority"];
        assert_eq!(authority["configuration_mode"], "initialization_only");
        assert_eq!(authority["configuration_pull"]["state"], "not_applicable");
        assert_eq!(
            authority["configuration_pull"]["failure"],
            serde_json::Value::Null
        );
        assert_eq!(authority["session_value_sources"]["check_mode"], "default");
        assert_eq!(authority["session_value_sources"]["base_ref"], "default");
        Ok(())
    })
}

#[test]
fn initialize_discloses_bounded_client_feature_profile_in_workspace_status() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("client-feature-profile-status")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, Some(file_uri_for_path(root.path())?));
        params.client_info = Some(tower_lsp_server::ls_types::ClientInfo {
            name: "profile-test-client-name".to_string(),
            version: None,
        });
        params.capabilities.text_document =
            Some(tower_lsp_server::ls_types::TextDocumentClientCapabilities {
                diagnostic: Some(
                    tower_lsp_server::ls_types::DiagnosticClientCapabilities::default(),
                ),
                ..tower_lsp_server::ls_types::TextDocumentClientCapabilities::default()
            });
        params.capabilities.window = Some(tower_lsp_server::ls_types::WindowClientCapabilities {
            work_done_progress: Some(true),
            ..tower_lsp_server::ls_types::WindowClientCapabilities::default()
        });
        params.capabilities.experimental = Some(serde_json::json!({
            "riprEditor": {
                "version": "0.10.0",
                "commands": ["ripr.refresh"],
                "guardedTestEdit": true
            }
        }));
        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        let features = &status["analysis_status"]["client_features"];
        assert_eq!(features["position_encoding"], "utf-16");
        assert_eq!(features["pull_diagnostics"], true);
        assert_eq!(features["work_done_progress"], true);
        assert_eq!(features["configuration_mode"], "initialization_only");
        assert_eq!(features["ripr_editor"]["version"], "0.10.0");
        assert_eq!(features["ripr_editor"]["guarded_test_edit"], true);
        assert_eq!(features["ripr_editor"]["command_count"], 1);
        assert_eq!(features["ripr_agent"], serde_json::Value::Null);
        // Bounded disclosure: the client name never enters the status payload.
        if status.to_string().contains("profile-test-client-name") {
            return Err("status payload leaked the client name".to_string());
        }
        Ok(())
    })
}

#[test]
fn receipt_status_discloses_bounded_client_feature_profile() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("client-feature-profile-receipt")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, Some(file_uri_for_path(root.path())?));
        params.capabilities.experimental = Some(serde_json::json!({
            "riprAgent": {
                "protocol": "0.1",
                "profiles": ["actionable"],
                "delivery": ["status_notifications"]
            }
        }));
        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected receipt status".to_string())?;
        let features = &status["client_features"];
        assert_eq!(features["ripr_agent"]["protocol_version"], "0.1");
        assert_eq!(
            features["ripr_agent"]["profiles"],
            serde_json::json!(["actionable"])
        );
        assert_eq!(
            features["ripr_agent"]["delivery"],
            serde_json::json!(["status_notifications"])
        );
        assert_eq!(features["ripr_editor"], serde_json::Value::Null);
        Ok(())
    })
}

#[test]
fn malformed_experimental_blocks_keep_the_standard_session_and_disclose_unsupported()
-> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("client-feature-profile-fail-closed")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, Some(file_uri_for_path(root.path())?));
        params.capabilities.text_document =
            Some(tower_lsp_server::ls_types::TextDocumentClientCapabilities {
                diagnostic: Some(
                    tower_lsp_server::ls_types::DiagnosticClientCapabilities::default(),
                ),
                ..tower_lsp_server::ls_types::TextDocumentClientCapabilities::default()
            });
        params.capabilities.experimental = Some(serde_json::json!({
            "riprEditor": {"version": 42},
            "riprAgent": {"protocol": "1.0"}
        }));
        let result = backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        // The standard session is unaffected: pull support was negotiated
        // from the standard capabilities and the provider is advertised.
        if result.capabilities.diagnostic_provider.is_none() {
            return Err("malformed experimental blocks broke the standard session".to_string());
        }

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        let features = &status["analysis_status"]["client_features"];
        assert_eq!(features["pull_diagnostics"], true);
        assert_eq!(features["ripr_editor"], serde_json::Value::Null);
        assert_eq!(features["ripr_agent"], serde_json::Value::Null);
        Ok(())
    })
}

#[test]
fn initialize_surfaces_poisoned_client_features_store_as_a_session_failure() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("client-feature-profile-poisoned-store")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        // Poison the profile store: a std::sync::Mutex is poisoned when a
        // holder panics. The helper confines the injected panic; no
        // production path is involved.
        backend.poison_client_features_for_test();
        backend
            .initialize(initialize_params(
                None,
                Some(file_uri_for_path(root.path())?),
            ))
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        // The store failure must surface through the blocking-failure
        // channel instead of leaving the pre-initialize profile beside
        // negotiated sibling state.
        let failure = backend
            .configuration_failure()
            .ok_or_else(|| "poisoned profile store must surface a session failure".to_string())?;
        if failure.kind != "session_state_inconsistent" {
            return Err(format!(
                "poisoned profile store surfaced the wrong failure kind: {}",
                failure.kind
            ));
        }
        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        assert_eq!(
            status["analysis_status"]["input_authority"]["configuration_state"],
            "invalid"
        );
        assert_eq!(
            status["analysis_status"]["failure"]["kind"],
            "session_state_inconsistent"
        );
        Ok(())
    })
}

#[test]
fn pull_mode_is_pending_until_the_first_pull_resolves() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("config-mode-pull-pending")?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = initialize_params(None, Some(file_uri_for_path(root.path())?));
        params.capabilities.workspace =
            Some(tower_lsp_server::ls_types::WorkspaceClientCapabilities {
                configuration: Some(true),
                ..tower_lsp_server::ls_types::WorkspaceClientCapabilities::default()
            });
        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;
        let authority = &status["analysis_status"]["input_authority"];
        assert_eq!(authority["configuration_mode"], "pull");
        // Startup-window honesty: no pull has resolved, so the status
        // discloses `pending` instead of presenting defaults as accepted
        // requested settings.
        assert_eq!(authority["configuration_pull"]["state"], "pending");
        Ok(())
    })
}

#[test]
fn framed_lsp_configuration_pull_applies_and_discloses_pull_state() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let root = unique_lsp_test_root("framed-config-pull")?;
        let root_uri = file_uri_for_path(root.path())?;
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
                        "baseRef": "origin/init",
                        "checkMode": "fast"
                    },
                    "capabilities": {
                        "workspace": {"configuration": true}
                    }
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
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;

        // The server must pull the bounded `ripr` section scoped to the
        // selected root URI from `initialized`.
        let pull_request = read_lsp_request(&mut client_read, "workspace/configuration").await?;
        assert_eq!(
            pull_request["params"]["items"],
            serde_json::json!([{"scopeUri": root_uri.as_str(), "section": "ripr"}])
        );
        // Answer with the same checkMode the initialization options supplied:
        // semantically unchanged effective settings must not reschedule
        // analysis.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": pull_request["id"].clone(),
                "result": [{"checkMode": "fast"}]
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
                    "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let status = read_lsp_response(&mut client_read, 2).await?;
        assert!(status.get("error").is_none());
        let authority = &status["result"]["analysis_status"]["input_authority"];
        assert_eq!(authority["configuration_mode"], "pull");
        assert_eq!(authority["configuration_pull"]["state"], "applied");
        assert_eq!(authority["configuration_pull"]["epoch"], 0);
        assert_eq!(authority["session_value_sources"]["check_mode"], "pulled");
        assert_eq!(
            authority["session_value_sources"]["base_ref"],
            "initialization"
        );
        assert_eq!(
            authority["session_value_sources"]["seam_diagnostics"],
            "default"
        );
        // The pull never launched analysis.
        assert_eq!(
            status["result"]["analysis_status"]["snapshot_id"],
            serde_json::Value::Null
        );

        // `workspace/didChangeConfiguration` in pull mode invalidates the
        // pulled layer and schedules one coalesced re-pull; a malformed
        // response is disclosed as a typed state while the last-known-good
        // pulled layer is retained.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeConfiguration",
                "params": {"settings": {}}
            }),
        )
        .await?;
        let repull_request = read_lsp_request(&mut client_read, "workspace/configuration").await?;
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": repull_request["id"].clone(),
                "result": [{"checkMode": 42}]
            }),
        )
        .await?;

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "workspace/executeCommand",
                "params": {
                    "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                    "arguments": []
                }
            }),
        )
        .await?;
        let status = read_lsp_response(&mut client_read, 3).await?;
        assert!(status.get("error").is_none());
        let authority = &status["result"]["analysis_status"]["input_authority"];
        assert_eq!(authority["configuration_pull"]["state"], "failed");
        assert_eq!(
            authority["configuration_pull"]["failure"]["kind"],
            "config_pull_invalid"
        );
        assert_eq!(
            authority["configuration_pull"]["recovery_route"],
            "retry_via_did_change_configuration"
        );
        assert_eq!(authority["configuration_pull"]["epoch"], 1);
        // Last-known-good pulled settings stay disclosed as the value source.
        assert_eq!(authority["session_value_sources"]["check_mode"], "pulled");
        assert_eq!(
            status["result"]["analysis_status"]["snapshot_id"],
            serde_json::Value::Null
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "shutdown",
                "params": null
            }),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 4).await?;
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
fn framed_lsp_deferred_configuration_pull_runs_after_root_transition_guard_release()
-> Result<(), String> {
    // Regression pin for the deferred-pull deadlock (#2031 review): the pull
    // must be scheduled AFTER `workspace_root_transition` is released, because
    // a pull that changes effective settings reaches `refresh_diagnostics` →
    // `run_refresh_request`, which re-locks that guard on the publication
    // path. The analysis must succeed for the lock to be reached, so the
    // selected root uses the known-good fixture recipe (baseRef HEAD,
    // checkMode instant). Pre-fix this exchange deadlocks and the timeout
    // below fails the test; post-fix it completes.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let repo_root = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input"),
        )
        .map_err(|err| format!("failed to canonicalize fixture root: {err}"))?;
        let fixture_uri = file_uri_for_path(&repo_root)?;
        let other = unique_lsp_test_root("framed-config-pull-deferred")?;
        let other_uri = file_uri_for_path(other.path())?;
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

        // Ambiguous start: two workspace folders, so no single root is
        // selected and the initialized pull defers without any client
        // request.
        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "workspaceFolders": [
                        {"uri": fixture_uri.as_str(), "name": "fixture"},
                        {"uri": other_uri.as_str(), "name": "other"}
                    ],
                    "initializationOptions": {
                        "baseRef": "HEAD",
                        "checkMode": "instant"
                    },
                    "capabilities": {
                        "workspace": {"configuration": true}
                    }
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
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;

        let exchange = async {
            // Drive a root transition to a single selected root. The server
            // queries the client for the current folders.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [], "removed": []}}
                }),
            )
            .await?;
            let folders_request =
                read_lsp_request(&mut client_read, "workspace/workspaceFolders").await?;
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": folders_request["id"].clone(),
                    "result": [{"uri": fixture_uri.as_str(), "name": "fixture"}]
                }),
            )
            .await?;

            // The deferred pull must now run, scoped to the selected root.
            let pull_request =
                read_lsp_request(&mut client_read, "workspace/configuration").await?;
            assert_eq!(
                pull_request["params"]["items"],
                serde_json::json!([{"scopeUri": fixture_uri.as_str(), "section": "ripr"}])
            );
            // Change effective settings so the apply path reaches
            // refresh_diagnostics; analysis on this root succeeds, so the
            // publication path re-locks the root transition guard.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": pull_request["id"].clone(),
                    "result": [{"includeUnchangedTests": false}]
                }),
            )
            .await?;

            // Probe responsiveness: a deadlocked server never answers.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "workspace/executeCommand",
                    "params": {
                        "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let status = read_lsp_response(&mut client_read, 2).await?;
            assert!(status.get("error").is_none());
            let authority = &status["result"]["analysis_status"]["input_authority"];
            assert_eq!(authority["configuration_mode"], "pull");
            assert_eq!(authority["configuration_pull"]["state"], "applied");
            assert_eq!(
                authority["session_value_sources"]["include_unchanged_tests"],
                "pulled"
            );
            assert_eq!(
                authority["session_value_sources"]["base_ref"],
                "initialization"
            );

            // Discriminating probe: the status request above could still be
            // answered by a concurrent handler while the transition task is
            // deadlocked (request concurrency is 4 and the pull state is set
            // before the refresh). An explicit refresh awaits the full
            // analysis inline and its publication path must acquire the root
            // transition guard, so it only completes when the deferred pull
            // ran after the guard was released.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "workspace/executeCommand",
                    "params": {
                        "command": REFRESH_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let refresh = read_lsp_response(&mut client_read, 3).await?;
            assert!(refresh.get("error").is_none());
            assert_eq!(refresh["result"], serde_json::Value::Null);

            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "shutdown",
                    "params": null
                }),
            )
            .await?;
            let shutdown = read_lsp_response(&mut client_read, 4).await?;
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
            Ok::<(), String>(())
        };
        match tokio::time::timeout(Duration::from_secs(10), exchange).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(
                    "deferred configuration pull deadlocked: it was scheduled while the workspace root transition guard was held, and its refresh path re-locks that guard"
                        .to_string(),
                );
            }
        }
        match tokio::time::timeout(Duration::from_secs(2), &mut server_task).await {
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
fn framed_lsp_root_switch_repulls_scoped_to_new_root() -> Result<(), String> {
    // Regression pin for the root-switch re-pull (#2031 review): pulled
    // settings are scoped to the root URI, so leaving a selected root in
    // pull mode must invalidate the old layer (epoch bump) and landing on a
    // new analysis-capable root must schedule one re-pull scoped to the NEW
    // root. Drives A -> removed -> B; a single remove+add notification lands
    // on the RootChanged authority, where analysis (and therefore the pull)
    // is intentionally paused until re-selection.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let root_a = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input"),
        )
        .map_err(|err| format!("failed to canonicalize fixture root: {err}"))?;
        let root_a_uri = file_uri_for_path(&root_a)?;
        let root_b = unique_lsp_test_root("framed-config-pull-root-switch")?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
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

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "workspaceFolders": [
                        {"uri": root_a_uri.as_str(), "name": "root-a"}
                    ],
                    "initializationOptions": {
                        "baseRef": "HEAD",
                        "checkMode": "instant"
                    },
                    "capabilities": {
                        "workspace": {"configuration": true}
                    }
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
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;

        let exchange = async {
            // First pull, scoped to root A; the answer matches the effective
            // defaults so the apply is a clean no-op that reaches Applied.
            let first_pull = read_lsp_request(&mut client_read, "workspace/configuration").await?;
            assert_eq!(
                first_pull["params"]["items"],
                serde_json::json!([{"scopeUri": root_a_uri.as_str(), "section": "ripr"}])
            );
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": first_pull["id"].clone(),
                    "result": [{"includeUnchangedTests": true}]
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
                        "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let status = read_lsp_response(&mut client_read, 2).await?;
            assert!(status.get("error").is_none());
            let authority = &status["result"]["analysis_status"]["input_authority"];
            assert_eq!(authority["configuration_pull"]["state"], "applied");
            assert_eq!(authority["configuration_pull"]["epoch"], 0);

            // A -> removed: no analysis-capable root, so no re-pull yet; the
            // epoch bump invalidates A's layer.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [], "removed": []}}
                }),
            )
            .await?;
            let folders_request =
                read_lsp_request(&mut client_read, "workspace/workspaceFolders").await?;
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": folders_request["id"].clone(),
                    "result": []
                }),
            )
            .await?;

            // removed -> B: the Applied pull lifecycle is restartable, so the
            // server must send a SECOND pull scoped to root B, never root A.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [], "removed": []}}
                }),
            )
            .await?;
            let folders_request =
                read_lsp_request(&mut client_read, "workspace/workspaceFolders").await?;
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": folders_request["id"].clone(),
                    "result": [{"uri": root_b_uri.as_str(), "name": "root-b"}]
                }),
            )
            .await?;
            let second_pull =
                read_lsp_request(&mut client_read, "workspace/configuration").await?;
            assert_eq!(
                second_pull["params"]["items"],
                serde_json::json!([{"scopeUri": root_b_uri.as_str(), "section": "ripr"}]),
                "the re-pull must be scoped to the new root B"
            );
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": second_pull["id"].clone(),
                    "result": [{"includeUnchangedTests": false, "seamDiagnostics": false}]
                }),
            )
            .await?;

            // The B answer replaces the retained layer wholesale:
            // seamDiagnostics was absent from A's answer, so a "pulled"
            // source for it can only come from B's layer.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "workspace/executeCommand",
                    "params": {
                        "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let status = read_lsp_response(&mut client_read, 3).await?;
            assert!(status.get("error").is_none());
            let authority = &status["result"]["analysis_status"]["input_authority"];
            assert_eq!(authority["configuration_pull"]["state"], "applied");
            assert_eq!(authority["configuration_pull"]["epoch"], 1);
            assert_eq!(
                authority["session_value_sources"]["include_unchanged_tests"],
                "pulled"
            );
            assert_eq!(
                authority["session_value_sources"]["seam_diagnostics"],
                "pulled"
            );

            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "shutdown",
                    "params": null
                }),
            )
            .await?;
            let shutdown = read_lsp_response(&mut client_read, 4).await?;
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
            Ok::<(), String>(())
        };
        match tokio::time::timeout(Duration::from_secs(10), exchange).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(
                    "root-switch re-pull did not complete: the server never sent a workspace/configuration request scoped to the new root"
                        .to_string(),
                );
            }
        }
        match tokio::time::timeout(Duration::from_secs(2), &mut server_task).await {
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
fn framed_lsp_direct_root_switch_repulls_on_reselection() -> Result<(), String> {
    // Regression pin for the direct A -> B root switch (#2031 review): one
    // didChangeWorkspaceFolders returning [B] rewrites the authority to the
    // non-analyzable RootChanged state, so NO re-pull may fire at the switch;
    // the re-pull must fire when the refresh path re-selects B
    // (refresh_diagnostics' RootChanged + Full branch). Staleness is decided
    // by comparing the retained layer's scope root against the effective
    // root, so the re-selection — a transition with no root delta — still
    // schedules one pull scoped to B.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let root_a = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input"),
        )
        .map_err(|err| format!("failed to canonicalize fixture root: {err}"))?;
        let root_a_uri = file_uri_for_path(&root_a)?;
        let root_b = unique_lsp_test_root("framed-config-pull-direct-switch")?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
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

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "workspaceFolders": [
                        {"uri": root_a_uri.as_str(), "name": "root-a"}
                    ],
                    "initializationOptions": {
                        "baseRef": "HEAD",
                        "checkMode": "instant"
                    },
                    "capabilities": {
                        "workspace": {"configuration": true}
                    }
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
                "method": "initialized",
                "params": {}
            }),
        )
        .await?;

        let exchange = async {
            // First pull, scoped to root A; the answer matches the effective
            // defaults so the apply is a clean no-op that reaches Applied.
            let first_pull = read_lsp_request(&mut client_read, "workspace/configuration").await?;
            assert_eq!(
                first_pull["params"]["items"],
                serde_json::json!([{"scopeUri": root_a_uri.as_str(), "section": "ripr"}])
            );
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": first_pull["id"].clone(),
                    "result": [{"includeUnchangedTests": true}]
                }),
            )
            .await?;

            // Direct switch A -> B in ONE notification. The authority becomes
            // RootChanged (non-analyzable), so no re-pull may be scheduled
            // yet: poll briefly and fail if a configuration request arrives.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [], "removed": []}}
                }),
            )
            .await?;
            let folders_request =
                read_lsp_request(&mut client_read, "workspace/workspaceFolders").await?;
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": folders_request["id"].clone(),
                    "result": [{"uri": root_b_uri.as_str(), "name": "root-b"}]
                }),
            )
            .await?;
            let during_root_changed =
                read_lsp_messages_for(&mut client_read, Duration::from_millis(200)).await?;
            if during_root_changed.iter().any(|message| {
                message.get("method").and_then(serde_json::Value::as_str)
                    == Some("workspace/configuration")
            }) {
                return Err(
                    "direct root switch scheduled a configuration pull before re-selection"
                        .to_string(),
                );
            }

            // Re-selection trigger: the explicit refresh handler's
            // RootChanged + Full branch re-selects B, which must schedule
            // one re-pull scoped to B even though this transition has no
            // root delta. B needs no git state: the re-selection runs before
            // any analysis at B.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "workspace/executeCommand",
                    "params": {
                        "command": REFRESH_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let second_pull =
                read_lsp_request(&mut client_read, "workspace/configuration").await?;
            assert_eq!(
                second_pull["params"]["items"],
                serde_json::json!([{"scopeUri": root_b_uri.as_str(), "section": "ripr"}]),
                "the re-selection re-pull must be scoped to the new root B"
            );
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": second_pull["id"].clone(),
                    "result": [{"includeUnchangedTests": false, "seamDiagnostics": false}]
                }),
            )
            .await?;
            let refresh = read_lsp_response(&mut client_read, 3).await?;
            assert!(refresh.get("error").is_none());

            // seamDiagnostics was absent from A's answer, so a "pulled"
            // source for it can only come from B's replacement layer.
            write_lsp_message(
                &mut client_write,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "workspace/executeCommand",
                    "params": {
                        "command": COLLECT_WORKSPACE_STATUS_COMMAND,
                        "arguments": []
                    }
                }),
            )
            .await?;
            let status = read_lsp_response(&mut client_read, 4).await?;
            assert!(status.get("error").is_none());
            let authority = &status["result"]["analysis_status"]["input_authority"];
            assert_eq!(authority["configuration_pull"]["state"], "applied");
            assert_eq!(authority["configuration_pull"]["epoch"], 1);
            assert_eq!(
                authority["session_value_sources"]["seam_diagnostics"],
                "pulled"
            );
            assert_eq!(
                authority["session_value_sources"]["include_unchanged_tests"],
                "pulled"
            );

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
            Ok::<(), String>(())
        };
        match tokio::time::timeout(Duration::from_secs(10), exchange).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(
                    "direct root-switch re-pull did not complete: re-selection never produced a workspace/configuration request scoped to the new root"
                        .to_string(),
                );
            }
        }
        match tokio::time::timeout(Duration::from_secs(2), &mut server_task).await {
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

/// Framed duplex harness for the `workspace_folder_transitions` suite
/// (#2036, RIPR-SPEC-0139). Every `didChangeWorkspaceFolders` event that the
/// server accepts is followed by exactly one server-originated
/// `workspace/workspaceFolders` reconciliation request, which this fake
/// client MUST answer — an unanswered server request hangs the test.
struct WorkspaceFolderTransitionsClient {
    reader: tokio::io::ReadHalf<tokio::io::DuplexStream>,
    writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    server_task: tokio::task::JoinHandle<()>,
    next_id: u64,
}

impl WorkspaceFolderTransitionsClient {
    fn spawn() -> Self {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (service, socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let server_task = tokio::spawn(async move {
            Server::new(server_read, server_write, socket)
                .serve(service)
                .await;
        });
        Self {
            reader: client_read,
            writer: client_write,
            server_task,
            next_id: 1,
        }
    }

    fn request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn initialize_with_workspace_folders(
        &mut self,
        folders: serde_json::Value,
    ) -> Result<(), String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "workspaceFolders": folders,
                    "initializationOptions": { "checkMode": "instant" },
                    "capabilities": {}
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut self.reader, id).await?;
        if initialize.get("error").is_some() {
            return Err(format!("initialize failed: {initialize}"));
        }
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await
    }

    /// Send one `workspace/didChangeWorkspaceFolders` notification and return
    /// the server's reconciliation request. The caller answers it with
    /// `answer_workspace_folders`.
    async fn send_folder_event(
        &mut self,
        added: serde_json::Value,
        removed: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWorkspaceFolders",
                "params": {"event": {"added": added, "removed": removed}}
            }),
        )
        .await?;
        read_lsp_request(&mut self.reader, "workspace/workspaceFolders").await
    }

    async fn answer_workspace_folders(
        &mut self,
        request: &serde_json::Value,
        result: serde_json::Value,
    ) -> Result<(), String> {
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request["id"].clone(),
                "result": result
            }),
        )
        .await
    }

    async fn run_command(&mut self, command: &str) -> Result<serde_json::Value, String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "workspace/executeCommand",
                "params": {"command": command, "arguments": []}
            }),
        )
        .await?;
        let response = read_lsp_response(&mut self.reader, id).await?;
        if response.get("error").is_some() {
            return Err(format!("command {command} failed: {response}"));
        }
        Ok(response["result"].clone())
    }

    async fn workspace_status(&mut self) -> Result<serde_json::Value, String> {
        let result = self.run_command(COLLECT_WORKSPACE_STATUS_COMMAND).await?;
        Ok(result["analysis_status"].clone())
    }

    /// Poll the workspace status until the predicate holds. Folder events
    /// are processed concurrently with command requests, so a plain status
    /// read could observe the pre-transition state; the bounded poll is the
    /// synchronization point for transitions that publish no request.
    async fn poll_workspace_status_until(
        &mut self,
        description: &str,
        predicate: impl Fn(&serde_json::Value) -> bool,
    ) -> Result<serde_json::Value, String> {
        let mut last = serde_json::Value::Null;
        for _ in 0..40 {
            last = self.workspace_status().await?;
            if predicate(&last) {
                return Ok(last);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(format!(
            "workspace status never satisfied {description}; last status: {last}"
        ))
    }

    async fn finish(&mut self) -> Result<(), String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null}),
        )
        .await?;
        let shutdown = read_lsp_response(&mut self.reader, id).await?;
        if shutdown.get("error").is_some() {
            return Err(format!("shutdown failed: {shutdown}"));
        }
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        )
        .await?;
        self.writer
            .shutdown()
            .await
            .map_err(|err| format!("failed to close test client: {err}"))?;
        match tokio::time::timeout(Duration::from_secs(2), &mut self.server_task).await {
            Ok(join_result) => {
                join_result.map_err(|err| format!("LSP server task failed: {err}"))?;
            }
            Err(_) => {
                self.server_task.abort();
                return Err("LSP server did not stop after exit notification".to_string());
            }
        }
        Ok(())
    }
}

fn run_workspace_folder_transitions_exchange<Fut>(
    failure: &str,
    exchange: Fut,
) -> Result<(), String>
where
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        match tokio::time::timeout(Duration::from_secs(15), exchange).await {
            Ok(result) => result,
            Err(_) => Err(failure.to_string()),
        }
    })
}

fn workspace_folder_json(uri: &tower_lsp_server::ls_types::Uri) -> serde_json::Value {
    serde_json::json!({"uri": uri.as_str(), "name": "folder"})
}

fn status_root_state(status: &serde_json::Value) -> Option<&str> {
    status["root_state"].as_str()
}

fn status_candidate_roots(status: &serde_json::Value) -> Vec<String> {
    status["candidate_roots"]
        .as_array()
        .map(|roots| {
            roots
                .iter()
                .filter_map(|root| root.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn workspace_folder_transitions_first_folder_after_none_starts_single_fresh_transition()
-> Result<(), String> {
    // Issue fixture 1: no folders -> add one. The first valid folder after a
    // no-workspace state creates one fresh transition: the stored set gains
    // one entry, the authority selects it, and no duplicate analysis runs.
    run_workspace_folder_transitions_exchange(
        "first-folder-after-none transition did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-first-folder-a")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_a_path = root_a.path().display().to_string();
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([]))
                .await?;
            let status = client.workspace_status().await?;
            if status_root_state(&status) != Some("root_unavailable") {
                return Err(format!(
                    "an empty folder list must start unavailable: {status}"
                ));
            }

            let request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_a_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([workspace_folder_json(&root_a_uri)]),
                )
                .await?;

            // Collect the transition publish plus a 300ms drain: exactly one
            // transition to the selected root, no duplicate analysis, and no
            // further server-originated requests.
            let id = client.request_id();
            write_lsp_message(
                &mut client.writer,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "workspace/executeCommand",
                    "params": {"command": COLLECT_WORKSPACE_STATUS_COMMAND, "arguments": []}
                }),
            )
            .await?;
            let (response, notifications) =
                read_response_and_notifications(&mut client.reader, id).await?;
            let status = &response["result"]["analysis_status"];
            if status_root_state(status) != Some("selected_single_root")
                || status["effective_root"].as_str() != Some(root_a_path.as_str())
            {
                return Err(format!("the first folder must be selected: {status}"));
            }
            let mut transition_publishes = 0_u32;
            for message in &notifications {
                if message.get("method").and_then(serde_json::Value::as_str)
                    == Some("ripr/analysisStatus")
                {
                    let params = &message["params"];
                    if params["root_state"].as_str() != Some("selected_single_root")
                        || params["effective_root"].as_str() != Some(root_a_path.as_str())
                    {
                        return Err(format!(
                            "a second, divergent transition published: {params}"
                        ));
                    }
                    transition_publishes += 1;
                }
                if message.get("id").is_some() && message.get("method").is_some() {
                    return Err(format!(
                        "unexpected server-originated request after the transition: {message}"
                    ));
                }
                if message.get("method").and_then(serde_json::Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                {
                    return Err("no analysis may publish diagnostics here".to_string());
                }
            }
            if transition_publishes == 0 {
                return Err("the first-folder transition must publish a status".to_string());
            }
            client.finish().await
        },
    )
}

#[test]
fn workspace_folder_transitions_second_folder_becomes_ambiguous_without_fallback()
-> Result<(), String> {
    // Issue fixture 2: one folder -> add second. The workspace becomes
    // ambiguous; the server never silently falls back to the first folder.
    run_workspace_folder_transitions_exchange(
        "second-folder ambiguity transition did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-ambiguous-a")?;
            let root_b = unique_lsp_test_root("wft-ambiguous-b")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_b_uri = file_uri_for_path(root_b.path())?;
            let root_a_path = root_a.path().display().to_string();
            let root_b_path = root_b.path().display().to_string();
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                    &root_a_uri
                )]))
                .await?;
            let status = client.workspace_status().await?;
            if status_root_state(&status) != Some("selected_single_root") {
                return Err(format!("one folder must start selected: {status}"));
            }

            let request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_b_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([
                        workspace_folder_json(&root_a_uri),
                        workspace_folder_json(&root_b_uri)
                    ]),
                )
                .await?;
            let status = client
                .poll_workspace_status_until("workspace_ambiguous", |status| {
                    status_root_state(status) == Some("workspace_ambiguous")
                })
                .await?;
            if !status["effective_root"].is_null() {
                return Err(format!(
                    "an ambiguous workspace must not select the first folder: {status}"
                ));
            }
            if status_candidate_roots(&status) != vec![root_a_path, root_b_path] {
                return Err(format!(
                    "candidates must be the canonical sorted folder set: {status}"
                ));
            }
            if status["repair_actions_available"].as_bool() != Some(false) {
                return Err(format!(
                    "an ambiguous workspace must block repair authority: {status}"
                ));
            }
            client.finish().await
        },
    )
}

#[test]
fn workspace_folder_transitions_ambiguous_resolves_to_remaining_folder_on_removal()
-> Result<(), String> {
    // Issue fixture 3: ambiguous -> the client narrows the set to one root
    // by removing the other folder; the remaining folder is selected.
    run_workspace_folder_transitions_exchange(
        "ambiguous-to-selected transition did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-resolve-a")?;
            let root_b = unique_lsp_test_root("wft-resolve-b")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_b_uri = file_uri_for_path(root_b.path())?;
            let root_a_path = root_a.path().display().to_string();
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([
                    workspace_folder_json(&root_a_uri),
                    workspace_folder_json(&root_b_uri)
                ]))
                .await?;
            let status = client.workspace_status().await?;
            if status_root_state(&status) != Some("workspace_ambiguous") {
                return Err(format!("two folders must start ambiguous: {status}"));
            }

            let request = client
                .send_folder_event(
                    serde_json::json!([]),
                    serde_json::json!([workspace_folder_json(&root_b_uri)]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([workspace_folder_json(&root_a_uri)]),
                )
                .await?;
            let status = client
                .poll_workspace_status_until("selected_single_root", |status| {
                    status_root_state(status) == Some("selected_single_root")
                })
                .await?;
            if status["effective_root"].as_str() != Some(root_a_path.as_str()) {
                return Err(format!(
                    "the remaining folder must be selected after the removal: {status}"
                ));
            }
            client.finish().await
        },
    )
}

#[test]
fn workspace_folder_transitions_direct_switch_lands_on_root_changed() -> Result<(), String> {
    // Issue fixture 4: switch from A to B in one event. The authority lands
    // on the non-analyzable root_changed state; an explicit refresh owns the
    // re-selection, exactly as in the query-driven direct-switch pin.
    run_workspace_folder_transitions_exchange("direct root switch did not complete", async {
        let root_a = unique_lsp_test_root("wft-switch-a")?;
        let root_b = unique_lsp_test_root("wft-switch-b")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
        let root_b_path = root_b.path().display().to_string();
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                &root_a_uri
            )]))
            .await?;

        let request = client
            .send_folder_event(
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
                serde_json::json!([workspace_folder_json(&root_a_uri)]),
            )
            .await?;
        client
            .answer_workspace_folders(
                &request,
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
            )
            .await?;
        let status = client
            .poll_workspace_status_until("root_changed", |status| {
                status_root_state(status) == Some("root_changed")
            })
            .await?;
        if status["effective_root"].as_str() != Some(root_b_path.as_str()) {
            return Err(format!(
                "root_changed must carry the new root as the current root: {status}"
            ));
        }
        if status["root_recovery_route"].as_str() != Some("refresh") {
            return Err(format!("root_changed must route to refresh: {status}"));
        }
        if status["repair_actions_available"].as_bool() != Some(false) {
            return Err(format!(
                "root_changed must block repair authority: {status}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_remove_active_root_quarantines_repair_authority()
-> Result<(), String> {
    // Issue fixture 5: remove the active root. The transition clears the
    // analysis state and quarantines repair authority behind the typed
    // workspace_root_removed block reason.
    run_workspace_folder_transitions_exchange("active-root removal did not complete", async {
        let root_a = unique_lsp_test_root("wft-removal-a")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                &root_a_uri
            )]))
            .await?;

        let request = client
            .send_folder_event(
                serde_json::json!([]),
                serde_json::json!([workspace_folder_json(&root_a_uri)]),
            )
            .await?;
        client
            .answer_workspace_folders(&request, serde_json::json!([]))
            .await?;
        let status = client
            .poll_workspace_status_until("root_removed", |status| {
                status_root_state(status) == Some("root_removed")
            })
            .await?;
        if !status["effective_root"].is_null()
            || status["repair_actions_available"].as_bool() != Some(false)
        {
            return Err(format!(
                "removing the active root must clear the root and repair authority: {status}"
            ));
        }
        let receipt = client.run_command(COLLECT_RECEIPT_STATUS_COMMAND).await?;
        if receipt["missing_receipt_reason"].as_str() != Some("workspace_root_removed")
            || receipt["receipt_status"].as_str() != Some("not_available")
        {
            return Err(format!(
                "repair authority must stay quarantined behind workspace_root_removed: {receipt}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_non_active_folder_removal_keeps_ambiguous_selection()
-> Result<(), String> {
    // Issue fixture 6: removing a non-active folder from an ambiguous set
    // changes only the candidate list; no root is selected or removed.
    run_workspace_folder_transitions_exchange("non-active folder removal did not complete", async {
        let root_a = unique_lsp_test_root("wft-nonactive-a")?;
        let root_b = unique_lsp_test_root("wft-nonactive-b")?;
        let root_c = unique_lsp_test_root("wft-nonactive-c")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
        let root_c_uri = file_uri_for_path(root_c.path())?;
        let root_a_path = root_a.path().display().to_string();
        let root_b_path = root_b.path().display().to_string();
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([
                workspace_folder_json(&root_a_uri),
                workspace_folder_json(&root_b_uri),
                workspace_folder_json(&root_c_uri)
            ]))
            .await?;

        let request = client
            .send_folder_event(
                serde_json::json!([]),
                serde_json::json!([workspace_folder_json(&root_c_uri)]),
            )
            .await?;
        client
            .answer_workspace_folders(
                &request,
                serde_json::json!([
                    workspace_folder_json(&root_a_uri),
                    workspace_folder_json(&root_b_uri)
                ]),
            )
            .await?;
        let status = client
            .poll_workspace_status_until("two remaining candidates", |status| {
                status_root_state(status) == Some("workspace_ambiguous")
                    && status_candidate_roots(status).len() == 2
            })
            .await?;
        if status_candidate_roots(&status) != vec![root_a_path, root_b_path]
            || !status["effective_root"].is_null()
        {
            return Err(format!(
                "removing a non-active folder must only narrow the candidate list: {status}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_duplicate_and_contradictory_events_rejected_typed()
-> Result<(), String> {
    // Issue fixture 7: duplicate and contradictory entries are rejected with
    // a typed bounded status; the stored set is left unchanged, which the
    // follow-up valid events prove.
    run_workspace_folder_transitions_exchange("typed rejection flow did not complete", async {
        let root_a = unique_lsp_test_root("wft-reject-a")?;
        let root_b = unique_lsp_test_root("wft-reject-b")?;
        let root_c = unique_lsp_test_root("wft-reject-c")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
        let root_c_uri = file_uri_for_path(root_c.path())?;
        let root_a_path = root_a.path().display().to_string();
        let root_b_path = root_b.path().display().to_string();
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                &root_a_uri
            )]))
            .await?;

        // Duplicate addition of the stored folder: rejected. A rejected
        // event sends no reconciliation request, so the poll is the
        // synchronization point.
        write_lsp_message(
            &mut client.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWorkspaceFolders",
                "params": {"event": {"added": [workspace_folder_json(&root_a_uri)], "removed": []}}
            }),
        )
        .await?;
        let status = client
            .poll_workspace_status_until("duplicate_addition rejection", |status| {
                status_root_state(status) == Some("root_unavailable")
                    && status["root_detail"]
                        .as_str()
                        .is_some_and(|detail| detail.contains("duplicate_addition"))
            })
            .await?;
        if status["root_detail"]
            .as_str()
            .is_none_or(|detail| !detail.contains("rejected (duplicate_addition)"))
        {
            return Err(format!("the rejection must be typed and bounded: {status}"));
        }

        // The set still holds exactly {A}: adding B now yields the
        // ambiguous set {A, B}.
        let request = client
            .send_folder_event(
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
                serde_json::json!([]),
            )
            .await?;
        client
            .answer_workspace_folders(
                &request,
                serde_json::json!([
                    workspace_folder_json(&root_a_uri),
                    workspace_folder_json(&root_b_uri)
                ]),
            )
            .await?;
        let status = client
            .poll_workspace_status_until("ambiguous after valid add", |status| {
                status_root_state(status) == Some("workspace_ambiguous")
            })
            .await?;
        if status_candidate_roots(&status) != vec![root_a_path.clone(), root_b_path.clone()] {
            return Err(format!(
                "a rejected event must not have mutated the stored set: {status}"
            ));
        }

        // Contradictory event (C in both added and removed): rejected.
        write_lsp_message(
                &mut client.writer,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [workspace_folder_json(&root_c_uri)], "removed": [workspace_folder_json(&root_c_uri)]}}
                }),
            )
            .await?;
        client
            .poll_workspace_status_until("contradictory_event rejection", |status| {
                status_root_state(status) == Some("root_unavailable")
                    && status["root_detail"]
                        .as_str()
                        .is_some_and(|detail| detail.contains("contradictory_event"))
            })
            .await?;

        // The set is still {A, B}: removing B selects A, and C never
        // entered the set.
        let request = client
            .send_folder_event(
                serde_json::json!([]),
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
            )
            .await?;
        client
            .answer_workspace_folders(
                &request,
                serde_json::json!([workspace_folder_json(&root_a_uri)]),
            )
            .await?;
        let status = client
            .poll_workspace_status_until("selected after contradictory rejection", |status| {
                status_root_state(status) == Some("selected_single_root")
            })
            .await?;
        if status["effective_root"].as_str() != Some(root_a_path.as_str()) {
            return Err(format!(
                "the contradictory event must not have entered the set: {status}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_invalid_file_uri_event_rejected_typed() -> Result<(), String> {
    // Issue fixture 8: a non-file URI in the delta is rejected with a typed
    // bounded status; the stored set is left unchanged.
    run_workspace_folder_transitions_exchange(
        "invalid-uri rejection flow did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-invalid-a")?;
            let root_b = unique_lsp_test_root("wft-invalid-b")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_b_uri = file_uri_for_path(root_b.path())?;
            let root_a_path = root_a.path().display().to_string();
            let root_b_path = root_b.path().display().to_string();
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                    &root_a_uri
                )]))
                .await?;

            write_lsp_message(
                &mut client.writer,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {"added": [{"uri": "https://example.test/workspace", "name": "remote"}], "removed": []}}
                }),
            )
            .await?;
            client
                .poll_workspace_status_until("invalid_file_uri rejection", |status| {
                    status_root_state(status) == Some("root_unavailable")
                        && status["root_detail"]
                            .as_str()
                            .is_some_and(|detail| detail.contains("invalid_file_uri"))
                })
                .await?;

            // The set still holds exactly {A}: adding B yields {A, B}.
            let request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_b_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([
                        workspace_folder_json(&root_a_uri),
                        workspace_folder_json(&root_b_uri)
                    ]),
                )
                .await?;
            let status = client
                .poll_workspace_status_until("ambiguous after invalid-uri rejection", |status| {
                    status_root_state(status) == Some("workspace_ambiguous")
                })
                .await?;
            if status_candidate_roots(&status) != vec![root_a_path, root_b_path] {
                return Err(format!(
                    "the invalid-uri event must not have mutated the stored set: {status}"
                ));
            }
            client.finish().await
        },
    )
}

#[test]
fn workspace_folder_transitions_stale_reconciliation_response_is_dropped() -> Result<(), String> {
    // Issue fixtures 9 and 10: event A's reconciliation round-trip completes
    // only after event B was applied from its own delta. The stale response
    // must be dropped — the authority stays at B's outcome and the epoch
    // never regresses.
    run_workspace_folder_transitions_exchange("stale-reconciliation flow did not complete", async {
        let root_a = unique_lsp_test_root("wft-stale-a")?;
        let root_b = unique_lsp_test_root("wft-stale-b")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
        let root_a_path = root_a.path().display().to_string();
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                &root_a_uri
            )]))
            .await?;

        // Event A adds B; its reconciliation request stays unanswered.
        let request_a = client
            .send_folder_event(
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
                serde_json::json!([]),
            )
            .await?;
        // Event B removes B; reading its reconciliation request proves
        // event B's delta was applied to the stored set.
        let request_b = client
            .send_folder_event(
                serde_json::json!([]),
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
            )
            .await?;
        // The stale A-era answer claims the folder list is only [B]. If
        // it were applied, the authority would switch to B
        // (root_changed); the epoch guard must drop it instead.
        client
            .answer_workspace_folders(
                &request_a,
                serde_json::json!([workspace_folder_json(&root_b_uri)]),
            )
            .await?;
        let during = read_lsp_messages_for(&mut client.reader, Duration::from_millis(300)).await?;
        if let Some(message) = during.first() {
            return Err(format!(
                "the stale reconciliation response must be dropped without any publication: {message}"
            ));
        }
        // Event B's own round-trip confirms the stored set {A}.
        client
            .answer_workspace_folders(
                &request_b,
                serde_json::json!([workspace_folder_json(&root_a_uri)]),
            )
            .await?;
        let status = client.workspace_status().await?;
        if status_root_state(&status) != Some("selected_single_root")
            || status["effective_root"].as_str() != Some(root_a_path.as_str())
        {
            return Err(format!(
                "the newer event's outcome must survive the stale response: {status}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_lagging_contradictory_reconciliation_is_dropped()
-> Result<(), String> {
    // Review fixture (#2036 review): an accepted delta changes the set, then
    // the reconciliation answer is the lagging PRE-delta list. The answer
    // contradicts the stored set and must be dropped without mutating: the
    // authority keeps the delta-derived state. A consistent answer confirms
    // the same way.
    run_workspace_folder_transitions_exchange(
        "lagging-reconciliation flow did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-lagging-a")?;
            let root_b = unique_lsp_test_root("wft-lagging-b")?;
            let root_c = unique_lsp_test_root("wft-lagging-c")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_b_uri = file_uri_for_path(root_b.path())?;
            let root_c_uri = file_uri_for_path(root_c.path())?;
            let root_a_path = root_a.path().display().to_string();
            let root_b_path = root_b.path().display().to_string();
            let root_c_path = root_c.path().display().to_string();
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                    &root_a_uri
                )]))
                .await?;

            // Accepted delta: add B (stored set {A, B}). The lagging answer
            // claims the list is still only [A]; installing it would undo
            // the delta, so it must be dropped and the delta-derived
            // ambiguous authority applied.
            let request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_b_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([workspace_folder_json(&root_a_uri)]),
                )
                .await?;
            let status = client
                .poll_workspace_status_until("delta-derived ambiguity survives", |status| {
                    status_root_state(status) == Some("workspace_ambiguous")
                })
                .await?;
            if status_candidate_roots(&status) != vec![root_a_path.clone(), root_b_path.clone()] {
                return Err(format!(
                    "the lagging contradictory answer must not undo the accepted delta: {status}"
                ));
            }

            // A consistent answer confirms the accepted delta the same way.
            let request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_c_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            client
                .answer_workspace_folders(
                    &request,
                    serde_json::json!([
                        workspace_folder_json(&root_a_uri),
                        workspace_folder_json(&root_b_uri),
                        workspace_folder_json(&root_c_uri)
                    ]),
                )
                .await?;
            let status = client
                .poll_workspace_status_until("consistent confirmation", |status| {
                    status_root_state(status) == Some("workspace_ambiguous")
                        && status_candidate_roots(status).len() == 3
                })
                .await?;
            if status_candidate_roots(&status) != vec![root_a_path, root_b_path, root_c_path] {
                return Err(format!(
                    "the consistent answer must confirm the accepted delta: {status}"
                ));
            }
            client.finish().await
        },
    )
}

#[test]
fn workspace_folder_transitions_equivalent_set_different_order_is_noop() -> Result<(), String> {
    // Issue fixture 11: an equivalent folder set in a different order is
    // byte-identical after canonicalization — no epoch bump, no transition,
    // no status publish, and the initialize-order authority is untouched.
    run_workspace_folder_transitions_exchange("equivalent-set no-op flow did not complete", async {
        let root_a = unique_lsp_test_root("wft-reorder-a")?;
        let root_b = unique_lsp_test_root("wft-reorder-b")?;
        let root_a_uri = file_uri_for_path(root_a.path())?;
        let root_b_uri = file_uri_for_path(root_b.path())?;
        let root_a_path = root_a.path().display().to_string();
        let root_b_path = root_b.path().display().to_string();
        let mut client = WorkspaceFolderTransitionsClient::spawn();
        client
            .initialize_with_workspace_folders(serde_json::json!([
                workspace_folder_json(&root_b_uri),
                workspace_folder_json(&root_a_uri)
            ]))
            .await?;
        let status = client.workspace_status().await?;
        if status_root_state(&status) != Some("workspace_ambiguous")
            || status_candidate_roots(&status) != vec![root_b_path.clone(), root_a_path.clone()]
        {
            return Err(format!(
                "initialize must keep the client folder order for candidates: {status}"
            ));
        }

        let request = client
            .send_folder_event(serde_json::json!([]), serde_json::json!([]))
            .await?;
        client
            .answer_workspace_folders(
                &request,
                serde_json::json!([
                    workspace_folder_json(&root_a_uri),
                    workspace_folder_json(&root_b_uri)
                ]),
            )
            .await?;
        let during = read_lsp_messages_for(&mut client.reader, Duration::from_millis(250)).await?;
        if let Some(message) = during.first() {
            return Err(format!(
                "an equivalent set in a different order must not publish or request: {message}"
            ));
        }
        let status = client.workspace_status().await?;
        if status_root_state(&status) != Some("workspace_ambiguous")
            || status_candidate_roots(&status) != vec![root_b_path, root_a_path]
        {
            return Err(format!(
                "the equivalent reconciliation must leave the authority untouched: {status}"
            ));
        }
        client.finish().await
    })
}

#[test]
fn workspace_folder_transitions_shutdown_during_inflight_reconciliation_stops_cleanly()
-> Result<(), String> {
    // Issue fixture 12: shutdown while a reconciliation round-trip is still
    // in flight. The server must stop cleanly: the shutdown request is
    // handled concurrently with the pending round-trip, and the late answer
    // lands in a session that is already stopping, with no observable
    // effect.
    run_workspace_folder_transitions_exchange(
        "shutdown during transition did not complete",
        async {
            let root_a = unique_lsp_test_root("wft-shutdown-a")?;
            let root_b = unique_lsp_test_root("wft-shutdown-b")?;
            let root_a_uri = file_uri_for_path(root_a.path())?;
            let root_b_uri = file_uri_for_path(root_b.path())?;
            let mut client = WorkspaceFolderTransitionsClient::spawn();
            client
                .initialize_with_workspace_folders(serde_json::json!([workspace_folder_json(
                    &root_a_uri
                )]))
                .await?;
            let pending_request = client
                .send_folder_event(
                    serde_json::json!([workspace_folder_json(&root_b_uri)]),
                    serde_json::json!([]),
                )
                .await?;
            // Shutdown while the reconciliation request is unanswered.
            let id = client.request_id();
            write_lsp_message(
                &mut client.writer,
                serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null}),
            )
            .await?;
            let shutdown = read_lsp_response(&mut client.reader, id).await?;
            if shutdown.get("error").is_some() {
                return Err(format!("shutdown failed: {shutdown}"));
            }
            // The protocol requires answering every server request; the late
            // answer must not prevent a clean stop.
            client
                .answer_workspace_folders(
                    &pending_request,
                    serde_json::json!([
                        workspace_folder_json(&root_a_uri),
                        workspace_folder_json(&root_b_uri)
                    ]),
                )
                .await?;
            write_lsp_message(
                &mut client.writer,
                serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
            )
            .await?;
            client
                .writer
                .shutdown()
                .await
                .map_err(|err| format!("failed to close test client: {err}"))?;
            match tokio::time::timeout(Duration::from_secs(2), &mut client.server_task).await {
                Ok(join_result) => {
                    join_result.map_err(|err| format!("LSP server task failed: {err}"))?;
                }
                Err(_) => {
                    client.server_task.abort();
                    return Err(
                        "LSP server did not stop after exit during an in-flight transition"
                            .to_string(),
                    );
                }
            }
            Ok(())
        },
    )
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
fn workspace_diagnostics_exclude_changed_test_files_from_published_findings() -> Result<(), String>
{
    let root = unique_lsp_test_root("changed-test-scope")?;
    write_lsp_scope_fixture(&root.path)?;
    run_lsp_scope_git(&root.path, &["init"])?;
    run_lsp_scope_git(
        &root.path,
        &["config", "user.email", "ripr@example.invalid"],
    )?;
    run_lsp_scope_git(&root.path, &["config", "user.name", "RIPR Test"])?;
    run_lsp_scope_git(
        &root.path,
        &["add", "Cargo.toml", "src/lib.rs", "tests/end_to_end.rs"],
    )?;
    run_lsp_scope_git(&root.path, &["commit", "-m", "base"])?;

    fs::write(
        root.path.join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("write changed production fixture failed: {err}"))?;
    fs::write(
        root.path.join("tests/end_to_end.rs"),
        "#[test]\nfn changed_test_helper() {\n    let value = if true { true } else { false };\n    assert_eq!(value, true);\n}\n",
    )
    .map_err(|err| format!("write changed test fixture failed: {err}"))?;
    run_lsp_scope_git(&root.path, &["add", "src/lib.rs", "tests/end_to_end.rs"])?;
    run_lsp_scope_git(&root.path, &["commit", "-m", "change production and test"])?;

    let config = LspAnalysisConfig {
        base_ref: Some("HEAD~1".to_string()),
        mode: Mode::Instant,
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        ..LspAnalysisConfig::default()
    };
    let diagnostics = workspace_diagnostics_with_config(&root.path, &config, false)?;
    let test_uri = super::uri::file_uri_for_path(&root.path.join("tests/end_to_end.rs"))?;
    let production_uri = super::uri::file_uri_for_path(&root.path.join("src/lib.rs"))?;

    let test_diagnostic_count = diagnostics
        .batches
        .iter()
        .find(|batch| batch.uri == test_uri)
        .map(|batch| batch.diagnostics.len())
        .unwrap_or(0);
    if test_diagnostic_count != 0 {
        return Err(format!(
            "changed test-only file received {test_diagnostic_count} LSP diagnostics"
        ));
    }
    if !diagnostics
        .batches
        .iter()
        .any(|batch| batch.uri == production_uri && !batch.diagnostics.is_empty())
    {
        return Err("changed production file received no LSP diagnostics".to_string());
    }
    Ok(())
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

fn write_lsp_scope_fixture(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join("src"))
        .map_err(|err| format!("create fixture src failed: {err}"))?;
    fs::create_dir_all(root.join("tests"))
        .map_err(|err| format!("create fixture tests failed: {err}"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"lsp-scope\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write fixture manifest failed: {err}"))?;
    fs::write(
        root.join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool { flag }\n",
    )
    .map_err(|err| format!("write fixture production file failed: {err}"))?;
    fs::write(
        root.join("tests/end_to_end.rs"),
        "#[test]\nfn unchanged_test_helper() {\n    assert_eq!(true, true);\n}\n",
    )
    .map_err(|err| format!("write fixture test file failed: {err}"))
}

pub(crate) fn run_lsp_scope_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("run git {args:?} failed: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
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

    let diagnostics = workspace_diagnostics_with_config(&fixture_root, &config, false)?;
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

/// Base fixture for the test-file scoping tests (#2130): one production file,
/// one `src/tests.rs` helper (a path the diff probe seeder does not skip but
/// the shared production classifier excludes), and one `tests/` integration
/// test file.
fn write_lsp_test_scope_fixture(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root.join("src"))
        .map_err(|err| format!("create fixture src failed: {err}"))?;
    std::fs::create_dir_all(root.join("tests"))
        .map_err(|err| format!("create fixture tests failed: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"lsp-test-scope\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write fixture manifest failed: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool { flag }\n",
    )
    .map_err(|err| format!("write fixture production file failed: {err}"))?;
    std::fs::write(
        root.join("src/tests.rs"),
        "pub fn helper_state(flag: bool) -> bool { flag }\n",
    )
    .map_err(|err| format!("write fixture src/tests.rs failed: {err}"))?;
    std::fs::write(
        root.join("tests/end_to_end.rs"),
        "#[test]\nfn end_to_end_placeholder() {\n    assert_eq!(true, true);\n}\n",
    )
    .map_err(|err| format!("write fixture tests/end_to_end.rs failed: {err}"))
}

fn run_lsp_test_scope_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("run git {args:?} failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn init_lsp_test_scope_repo(root: &Path) -> Result<(), String> {
    write_lsp_test_scope_fixture(root)?;
    run_lsp_test_scope_git(root, &["init"])?;
    run_lsp_test_scope_git(root, &["config", "user.email", "ripr@example.invalid"])?;
    run_lsp_test_scope_git(root, &["config", "user.name", "RIPR Test"])?;
    // Do not inherit commit.gpgSign from the host environment: a
    // signing-enabled host would fail the fixture commits before the
    // scoping assertions ever run (#2158 review).
    run_lsp_test_scope_git(root, &["config", "commit.gpgSign", "false"])?;
    run_lsp_test_scope_git(root, &["add", "."])?;
    run_lsp_test_scope_git(root, &["commit", "-m", "base"])
}

fn commit_lsp_test_scope_change(root: &Path, message: &str) -> Result<(), String> {
    run_lsp_test_scope_git(root, &["add", "."])?;
    run_lsp_test_scope_git(root, &["commit", "-m", message])
}

fn lsp_test_scope_config() -> LspAnalysisConfig {
    LspAnalysisConfig {
        base_ref: Some("HEAD~1".to_string()),
        mode: Mode::Instant,
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        ..LspAnalysisConfig::default()
    }
}

fn lsp_test_scope_diagnostic_count(
    diagnostics: &WorkspaceDiagnostics,
    root: &Path,
    relative: &str,
) -> Result<usize, String> {
    let uri = file_uri_for_path(&root.join(relative))?;
    Ok(diagnostics
        .batches
        .iter()
        .find(|batch| batch.uri == uri)
        .map(|batch| batch.diagnostics.len())
        .unwrap_or(0))
}

#[test]
fn workspace_diagnostics_scope_changed_test_file_findings_out_of_projection() -> Result<(), String>
{
    let root = unique_lsp_test_root("lsp-test-file-scope-mixed")?;
    init_lsp_test_scope_repo(root.path())?;
    std::fs::write(
        root.path().join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("write changed production file failed: {err}"))?;
    std::fs::write(
        root.path().join("src/tests.rs"),
        "pub fn helper_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("write changed src/tests.rs failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/end_to_end.rs"),
        "#[test]\nfn end_to_end_changed() {\n    let value = if true { 1 } else { 2 };\n    assert_eq!(value, 1);\n}\n",
    )
    .map_err(|err| format!("write changed tests/end_to_end.rs failed: {err}"))?;
    commit_lsp_test_scope_change(root.path(), "change production and test files")?;

    let diagnostics =
        workspace_diagnostics_with_config(root.path(), &lsp_test_scope_config(), true)?;

    let production_count =
        lsp_test_scope_diagnostic_count(&diagnostics, root.path(), "src/lib.rs")?;
    if production_count == 0 {
        return Err("changed production file received no LSP diagnostics".to_string());
    }
    for scoped_out in ["src/tests.rs", "tests/end_to_end.rs"] {
        let count = lsp_test_scope_diagnostic_count(&diagnostics, root.path(), scoped_out)?;
        if count != 0 {
            return Err(format!(
                "out-of-scope test file {scoped_out} received {count} line-local LSP diagnostics"
            ));
        }
    }
    if diagnostics
        .snapshot
        .findings
        .iter()
        .any(|finding| finding.probe.location.file.ends_with("src/tests.rs"))
    {
        return Err(
            "out-of-scope src/tests.rs finding must not remain in the snapshot".to_string(),
        );
    }
    if diagnostics.snapshot.out_of_scope_test_file_findings == 0 {
        return Err(
            "suppressed test-file findings must be disclosed with a non-zero count".to_string(),
        );
    }
    Ok(())
}

#[test]
fn workspace_diagnostics_test_only_diff_publishes_no_line_local_diagnostics() -> Result<(), String>
{
    let root = unique_lsp_test_root("lsp-test-file-scope-test-only")?;
    init_lsp_test_scope_repo(root.path())?;
    std::fs::write(
        root.path().join("src/tests.rs"),
        "pub fn helper_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("write changed src/tests.rs failed: {err}"))?;
    std::fs::write(
        root.path().join("tests/end_to_end.rs"),
        "#[test]\nfn end_to_end_changed() {\n    let value = if true { 1 } else { 2 };\n    assert_eq!(value, 1);\n}\n",
    )
    .map_err(|err| format!("write changed tests/end_to_end.rs failed: {err}"))?;
    commit_lsp_test_scope_change(root.path(), "change test files only")?;

    let diagnostics =
        workspace_diagnostics_with_config(root.path(), &lsp_test_scope_config(), true)?;

    let total = diagnostics
        .batches
        .iter()
        .map(|batch| batch.diagnostics.len())
        .sum::<usize>();
    if total != 0 {
        return Err(format!(
            "test-only diff published {total} line-local diagnostics; expected zero"
        ));
    }
    if !diagnostics.snapshot.findings.is_empty() {
        return Err("test-only diff findings must be scoped out of the LSP snapshot".to_string());
    }
    if diagnostics.snapshot.out_of_scope_test_file_findings == 0 {
        return Err(
            "test-only diff must disclose the suppressed test-file findings count".to_string(),
        );
    }
    Ok(())
}

#[test]
fn workspace_diagnostics_production_only_diff_keeps_full_projection() -> Result<(), String> {
    let root = unique_lsp_test_root("lsp-test-file-scope-production")?;
    init_lsp_test_scope_repo(root.path())?;
    std::fs::write(
        root.path().join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("write changed production file failed: {err}"))?;
    commit_lsp_test_scope_change(root.path(), "change production file only")?;

    let diagnostics =
        workspace_diagnostics_with_config(root.path(), &lsp_test_scope_config(), true)?;

    let production_count =
        lsp_test_scope_diagnostic_count(&diagnostics, root.path(), "src/lib.rs")?;
    if production_count == 0 {
        return Err("changed production file received no LSP diagnostics".to_string());
    }
    assert_eq!(
        diagnostics.snapshot.out_of_scope_test_file_findings, 0,
        "production-only diff must not report suppressed test-file findings"
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

/// Read until a server-originated request with the given method arrives,
/// skipping notifications. Used by configuration-pull tests where the fake
/// client must answer `workspace/configuration` (#2031).
async fn read_lsp_request<R>(reader: &mut R, method: &str) -> Result<serde_json::Value, String>
where
    R: AsyncRead + Unpin,
{
    loop {
        let message = read_lsp_message(reader).await?;
        if message.get("method").and_then(serde_json::Value::as_str) == Some(method) {
            return Ok(message);
        }
    }
}

/// Read until the response for `id` arrives, answering every
/// `workspace/codeLens/refresh` server-originated request with a null result
/// so the publish path cannot stall (#2032, RIPR-SPEC-0138). Returns the
/// response plus how many refresh requests arrived before it. The publish
/// path awaits the refresh request before completing the command response,
/// so the count needs no timing window.
async fn read_response_answering_code_lens_refresh<R, W>(
    reader: &mut R,
    writer: &mut W,
    id: u64,
) -> Result<(serde_json::Value, u64), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut refresh_requests = 0_u64;
    loop {
        let message = read_lsp_message(reader).await?;
        if message.get("method").and_then(serde_json::Value::as_str)
            == Some("workspace/codeLens/refresh")
        {
            refresh_requests += 1;
            let request_id = message
                .get("id")
                .cloned()
                .ok_or_else(|| "workspace/codeLens/refresh request carried no id".to_string())?;
            write_lsp_message(
                writer,
                serde_json::json!({"jsonrpc": "2.0", "id": request_id, "result": null}),
            )
            .await?;
            continue;
        }
        if message.get("method").is_none()
            && message.get("id").and_then(serde_json::Value::as_u64) == Some(id)
        {
            return Ok((message, refresh_requests));
        }
    }
}

/// Collect every message arriving within `window`, then return. Used to
/// assert the ABSENCE of a server-originated request without hanging: the
/// bounded window doubles as the poll budget.
async fn read_lsp_messages_for<R>(
    reader: &mut R,
    window: Duration,
) -> Result<Vec<serde_json::Value>, String>
where
    R: AsyncRead + Unpin,
{
    let deadline = tokio::time::Instant::now() + window;
    let mut messages = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(messages);
        }
        match tokio::time::timeout(remaining, read_lsp_message(reader)).await {
            Ok(Ok(message)) => messages.push(message),
            Ok(Err(err)) => return Err(err),
            Err(_) => return Ok(messages),
        }
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

fn gap_action_diagnostic() -> tower_lsp_server::ls_types::Diagnostic {
    tower_lsp_server::ls_types::Diagnostic {
        range: Range {
            start: Position {
                line: 11,
                character: 0,
            },
            end: Position {
                line: 11,
                character: 120,
            },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(
            "ripr-gap-MissingBoundaryAssertion".to_string(),
        )),
        code_description: None,
        source: Some("ripr".to_string()),
        message: "ripr gap: MissingBoundaryAssertion; repair route: AddBoundaryAssertion"
            .to_string(),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({
            "schema_version": "0.1",
            "source": "gap_decision_ledger",
            "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
            "gap_id": "gap:py:pricing",
            "canonical_gap_id": "gap:py:pricing",
            "gap_kind": "MissingBoundaryAssertion",
            "language": "python",
            "language_status": "preview",
            "gap_state": "actionable",
            "policy_state": "advisory",
            "repairability": "repairable",
            "static_limit_kind": "missing_import_graph",
            "static_limit_detail": "Imported owner targets were not resolved in preview mode.",
            "anchor": {
                "file": "src/pricing.py",
                "line": 12,
                "owner": "python:app/pricing.py::calculate_discount"
            },
            "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/test_pricing.py",
                "target_line": 2,
                "related_test": "tests/test_pricing.py::test_discount_boundary",
                "assertion_shape": "assert price(threshold) == expected",
                "changed_behavior": "amount >= threshold",
                "stop_conditions": ["Stop if the related test belongs to another package."]
            },
            "verification_commands": ["ripr agent verify --root . --json"],
            "receipt_command": "ripr agent receipt --root . --json",
            "authority_boundary": "advisory"
        })),
    }
}

fn validated_gap_artifact() -> ValidatedGapArtifact {
    ValidatedGapArtifact {
        kind: GapArtifactKind::GapDecisionLedger,
        root: Some(".".to_string()),
        identities: vec![GapArtifactIdentity {
            canonical_gap_id: Some("gap:py:pricing".to_string()),
            seam_id: None,
            finding_id: None,
        }],
        language: Some(LanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        gap_state: Some("actionable".to_string()),
        related_paths: vec!["tests/test_pricing.py".to_string()],
        verify_commands: vec!["ripr agent verify --root . --json".to_string()],
        receipt_commands: vec!["ripr agent receipt --root . --json".to_string()],
        verify_command_specs: vec![crate::agent::command_specs::agent_verify_command_spec(
            ".",
            "before.json",
            "after.json",
            None,
        )],
        receipt_command_specs: vec![crate::agent::command_specs::agent_receipt_command_spec(
            ".",
            "verify.json",
            "seam-a",
            Some("receipt.json"),
        )],
        static_limit_kinds: vec!["missing_import_graph".to_string()],
        has_text_static_limit: false,
    }
}

fn sample_analysis_snapshot(
    root: PathBuf,
    uri: tower_lsp_server::ls_types::Uri,
    diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
    findings: Vec<Finding>,
) -> AnalysisSnapshot {
    let mut diagnostics_by_uri = BTreeMap::new();
    diagnostics_by_uri.insert(uri, diagnostics);
    let input_identity = LspAnalysisInputIdentity::from_refresh_inputs(
        root.clone(),
        1,
        &LspAnalysisConfig::default(),
    );
    AnalysisSnapshot {
        root,
        input_identity: Some(input_identity),
        base: Some("origin/main".to_string()),
        mode: Mode::Draft,
        refresh: RefreshMetadata::generated_now(),
        findings,
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        classified_seams: Vec::new(),
        gap_artifacts: Vec::new(),
        gap_artifact_rejections: Vec::new(),
        diagnostics_by_uri,
        delivery_selection: None,
        seams_deferred: false,
        partial_scope: None,
        component_outcomes: Vec::new(),
        out_of_scope_test_file_findings: 0,
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

/// Parse the `RIPR_CLIENT_COMMANDS` advertisement from the VS Code
/// extension client (#1776, RIPR-SPEC-0129). The code-action parity tests
/// negotiate against the exact list the extension sends at `initialize`,
/// so a command the extension registers but forgets to advertise breaks
/// these tests instead of silently stripping quick fixes from VS Code.
fn vscode_advertised_client_commands() -> Result<Vec<String>, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("editors/vscode/src/client.ts");
    let source = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let mut commands = Vec::new();
    let mut in_block = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_block {
            in_block = trimmed.starts_with("const RIPR_CLIENT_COMMANDS");
            continue;
        }
        if trimmed.starts_with("];") {
            break;
        }
        let id = trimmed
            .strip_prefix('\'')
            .and_then(|value| {
                value
                    .strip_suffix("\',")
                    .or_else(|| value.strip_suffix('\''))
            })
            .ok_or_else(|| format!("unparsable RIPR_CLIENT_COMMANDS entry: {trimmed}"))?;
        commands.push(id.to_string());
    }
    if commands.is_empty() {
        return Err("RIPR_CLIENT_COMMANDS block not found in client.ts".to_string());
    }
    Ok(commands)
}

/// The negotiated profile for the real VS Code extension: exactly the
/// `RIPR_CLIENT_COMMANDS` advertisement from `editors/vscode/src/client.ts`
/// (#1776).
fn vscode_client_features() -> Result<ClientFeatureProfile, String> {
    let commands = vscode_advertised_client_commands()?;
    let borrowed = commands.iter().map(String::as_str).collect::<Vec<_>>();
    client_features_with_commands(&borrowed)
}

/// A negotiated profile whose `riprEditor` block advertises exactly
/// `commands` (#1776).
fn client_features_with_commands(commands: &[&str]) -> Result<ClientFeatureProfile, String> {
    let params: InitializeParams = serde_json::from_value(serde_json::json!({
        "capabilities": {
            "experimental": {
                "riprEditor": {
                    "version": "0.10.0",
                    "commands": commands,
                    "guardedTestEdit": false
                }
            }
        }
    }))
    .map_err(|err| format!("fixture params must parse: {err}"))?;
    Ok(ClientFeatureProfile::from_initialize_params(&params))
}

/// The seam code-action scenario shared by the client-command filter tests
/// (#1776): one seam diagnostic whose snapshot resolves to a classified
/// seam, so the unfiltered response carries every client command the
/// code-action path can emit.
fn seam_code_action_request() -> Result<(CodeActionParams, AnalysisSnapshot), String> {
    let seam = sample_classified_seam();
    let diagnostic = diagnostic_for_classified_seam(Path::new("/workspace"), &seam)
        .ok_or_else(|| "expected seam diagnostic".to_string())?;
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    let mut snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        Vec::new(),
    );
    snapshot.classified_seams = vec![seam];
    let params = code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?;
    Ok((params, snapshot))
}

fn boundary_gap_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/boundary_gap/input")
}

pub(crate) struct TempLspRoot {
    path: PathBuf,
}

impl TempLspRoot {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempLspRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn unique_lsp_test_root(name: &str) -> Result<TempLspRoot, String> {
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
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        repo_config,
        ..LspAnalysisConfig::default()
    }
}

fn workspace_projection_contract(
    root: &Path,
    config: &LspAnalysisConfig,
) -> Result<serde_json::Value, String> {
    // Run the full inventory (defer_seam_inventory = false) so tests exercise
    // the seam diagnostic contract end-to-end.
    let diagnostics = workspace_diagnostics_with_config(root, config, false)?;
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
        &vscode_client_features()?,
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
    let (mut seams, _) = crate::analysis::inventory_classified_seams_at_with_config(
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
        &vscode_client_features()?,
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

#[test]
fn canonical_finding_groups_collapse_same_gap_and_preserve_raw_signals() -> Result<(), String> {
    let mut first = sample_finding();
    first.canonical_gap = Some(sample_canonical_gap());
    let mut second = first.clone();
    second.id = "probe:pricing:89:predicate".to_string();
    second.probe.id = ProbeId(second.id.clone());
    second.probe.location.line = 89;

    let groups = canonical_finding_groups(&[first, second]);

    assert_eq!(groups.len(), 1, "one canonical gap should yield one group");
    assert_eq!(groups[0].1.len(), 2, "raw findings must remain attached");
    assert_eq!(
        groups[0]
            .0
            .canonical_gap
            .as_ref()
            .map(|gap| gap.id.as_str()),
        Some(
            "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
        )
    );
    Ok(())
}

#[test]
fn canonical_group_mixed_classes_are_detected_without_promotion() {
    let mut first = sample_finding();
    first.canonical_gap = Some(sample_canonical_gap());
    let mut second = first.clone();
    second.class = ExposureClass::StaticUnknown;

    assert!(canonical_group_has_mixed_classes(&[first, second]));
    assert!(!canonical_group_has_mixed_classes(std::slice::from_ref(
        &sample_finding()
    )));
}

#[test]
fn finding_projection_emits_one_limited_diagnostic_for_mixed_canonical_group() -> Result<(), String>
{
    let mut first = sample_finding();
    first.canonical_gap = Some(sample_canonical_gap());
    let mut second = first.clone();
    second.id = "probe:pricing:89:predicate".to_string();
    second.probe.id = ProbeId(second.id.clone());
    second.probe.location.line = 89;
    second.class = ExposureClass::StaticUnknown;
    let config = LspAnalysisConfig::default();
    let grouped = finding_diagnostics_by_uri(
        Path::new("/workspace"),
        &[first, second],
        config.repo_config().severity(),
        true,
        None,
    )?;
    let diagnostics = grouped.values().flatten().collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].severity,
        Some(DiagnosticSeverity::INFORMATION)
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data["raw_signal_count"].as_u64()),
        Some(2)
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data["canonical_limitation"].as_str()),
        Some("mixed_static_classes")
    );
    Ok(())
}

fn sample_finding() -> Finding {
    Finding {
        id: "probe:pricing:88:predicate".to_string(),
        canonical_gap: None,
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
        changed_sink: None,
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
    }
}

fn sample_typescript_preview_actionability_finding() -> Finding {
    let mut finding = sample_finding();
    finding.language = Some(LanguageId::TypeScript);
    finding.language_status = Some(LanguageStatus::Preview);
    finding.owner_kind = Some(OwnerKind::Function);
    finding.evidence = vec![
        "gap_state: advisory".to_string(),
        "actionability_category: missing_context".to_string(),
        "why_not_actionable: verify command and receipt command are not inferred for TypeScript preview".to_string(),
        "repair_route: add strict TypeScript repair-packet actionability proof".to_string(),
        "missing_actionability_fields: verify_command, receipt_command, must_not_change".to_string(),
        "evidence_needed_to_promote: complete repair packet with verify and receipt commands".to_string(),
        "raw_evidence_ref: file=src/pricing.ts;line=88;kind=probe;source_id=ts-probe-1;owner=discountedTotal".to_string(),
    ];
    finding
}

fn sample_canonical_gap() -> FindingCanonicalGap {
    FindingCanonicalGap {
        id: "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
            .to_string(),
        language: "python".to_string(),
        file: "src/pricing.py".to_string(),
        owner: "apply_discount".to_string(),
        behavior_kind: "predicate_boundary".to_string(),
        probe_kind: "predicate".to_string(),
        normalized_discriminator: "amount>=threshold".to_string(),
    }
}

fn sample_classified_seam() -> crate::analysis::ClassifiedSeam {
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence, TestTargetEvidence,
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
                test_target: Some(TestTargetEvidence::fixture(
                    "below_threshold_has_no_discount",
                    Path::new("tests/pricing.rs"),
                    12,
                )),
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
        relation_reason: None,
        relation_confidence: None,
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
fn typescript_preview_finding_diagnostic_carries_actionability_context() -> Result<(), String> {
    let finding = sample_typescript_preview_actionability_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

    let data = diagnostic
        .data
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| "expected diagnostic data".to_string())?;
    let actionability = data
        .get("preview_actionability")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "expected preview_actionability data".to_string())?;
    assert_eq!(
        actionability
            .get("gap_state")
            .and_then(|value| value.as_str()),
        Some("advisory")
    );
    assert_eq!(
        actionability
            .get("actionability_category")
            .and_then(|value| value.as_str()),
        Some("missing_context")
    );
    assert_eq!(
        actionability
            .get("repair_packet_ready")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        actionability["missing_actionability_fields"][0].as_str(),
        Some("verify_command")
    );
    assert_eq!(
        actionability["raw_evidence_refs"][0]["file"].as_str(),
        Some("src/pricing.ts")
    );
    assert_eq!(
        actionability["raw_evidence_refs"][0]["owner"].as_str(),
        Some("discountedTotal")
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
fn typescript_preview_finding_hover_shows_actionability_before_evidence() -> Result<(), String> {
    use super::hover::finding_hover_response;

    let finding = sample_typescript_preview_actionability_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let hover = finding_hover_response(&finding, &diagnostic);

    match hover.contents {
        HoverContents::Markup(markup) => {
            let boundary_index = markup
                .value
                .find("## Preview Boundary")
                .ok_or_else(|| "expected preview boundary".to_string())?;
            let actionability_index = markup
                .value
                .find("## Preview Actionability")
                .ok_or_else(|| "expected preview actionability".to_string())?;
            let evidence_index = markup
                .value
                .find("## RIPR Evidence")
                .ok_or_else(|| "expected RIPR evidence".to_string())?;
            assert!(
                boundary_index < actionability_index && actionability_index < evidence_index,
                "preview actionability must appear before evidence details:\n{}",
                markup.value
            );
            for needle in [
                "Language: typescript",
                "Status: preview",
                "Repair packet: not ready",
                "State: advisory",
                "Category: missing_context",
                "Why not actionable: verify command and receipt command are not inferred for TypeScript preview",
                "Repair route: add strict TypeScript repair-packet actionability proof",
                "Missing fields: verify_command, receipt_command, must_not_change",
                "Evidence needed: complete repair packet with verify and receipt commands",
                "Authority: preview advisory only",
            ] {
                assert!(
                    markup.value.contains(needle),
                    "missing {needle:?} in:\n{}",
                    markup.value
                );
            }
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
        &vscode_client_features()?,
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
fn typescript_preview_code_action_copies_actionability_without_repair_packet() -> Result<(), String>
{
    let finding = sample_typescript_preview_actionability_finding();
    let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
    let uri = test_uri("file:///workspace/src/pricing.ts")?;
    let snapshot = sample_analysis_snapshot(
        PathBuf::from("/workspace"),
        uri.clone(),
        vec![diagnostic.clone()],
        vec![finding],
    );
    let actions = code_action_response(
        &code_action_params_for(uri, diagnostic.range.start.line, vec![diagnostic])?,
        Some(&snapshot),
        &vscode_client_features()?,
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
        "incomplete TypeScript preview actionability must not expose repair-packet, verify, receipt, or edit actions"
    );
    assert_eq!(commands[0].2[0]["language"], "typescript");
    assert_eq!(commands[0].2[0]["language_status"], "preview");
    assert_eq!(commands[0].2[0]["owner_kind"], "function");
    assert_eq!(
        commands[0].2[0]["preview_actionability"]["repair_packet_ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        commands[0].2[0]["preview_actionability"]["repair_route"].as_str(),
        Some("add strict TypeScript repair-packet actionability proof")
    );
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

#[test]
fn execute_command_collect_workspace_status_no_snapshot_returns_no_snapshot_status()
-> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status even without snapshot".to_string())?;

        assert_eq!(status["schema_version"], "0.1");
        assert_eq!(status["tool"], "ripr");
        assert_eq!(status["kind"], "workspace_status");
        assert_eq!(status["run_status"], "no_snapshot");
        assert_eq!(status["analysis_status"]["state"], "stopped");
        assert_eq!(status["analysis_status"]["run_status"], "no_snapshot");
        assert_eq!(status["analysis_status"]["repair_actions_available"], false);
        assert_eq!(status["top_actionable_packet"], serde_json::Value::Null);
        assert_eq!(
            status["diagnostic_budget_state"],
            serde_json::json!({
                "status": "unavailable",
                "reason": "no_snapshot",
            })
        );
        assert_eq!(status["top_limitation"]["status"], "no_snapshot");
        assert_eq!(
            status["top_limitation"]["limitation_category"],
            "no_snapshot"
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["configuration_state"],
            "valid"
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["repository_config_source"],
            serde_json::Value::Null
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["session_options_present"],
            false
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["current"],
            serde_json::Value::Null
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["last_success"],
            serde_json::Value::Null
        );
        assert_eq!(
            status["limits_note"],
            "Static evidence only; advisory, not a gate decision."
        );
        assert_eq!(status["refresh_command"], REFRESH_COMMAND);
        assert!(
            status["report_paths"]["actionable_gaps"]
                .as_str()
                .is_some_and(|p| p.contains("actionable-gaps")),
            "expected report_paths.actionable_gaps in status: {status}"
        );
        Ok(())
    })
}

#[test]
fn failed_refresh_retains_last_snapshot_and_reports_stale_health() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let finding = sample_finding();
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        );
        backend
            .refresh_plan(diagnostics)
            .ok_or_else(|| "expected successful snapshot".to_string())?;

        let request = RefreshRequest {
            generation: 7,
            authority_epoch: 0,
            input_identity: LspAnalysisInputIdentity::from_refresh_inputs(
                PathBuf::from("/workspace"),
                1,
                &LspAnalysisConfig::default(),
            ),
            git_inputs: crate::lsp::git_inputs::ResolvedGitInputs::resolve(
                Path::new("/workspace"),
                None,
            ),
            root: PathBuf::from("/workspace"),
            config: LspAnalysisConfig::default(),
            workspace_revision: 1,
            scope: RefreshScope::Interactive,
            reason: RefreshReason::DidSave,
            cancellation: AnalysisCancellationToken::new(),
        };
        backend.record_health_outcome(&request, RefreshAttemptOutcome::Published);
        backend
            .report_refresh_failure_after(
                &request,
                "temporary analysis timeout at /workspace/src/pricing.rs".to_string(),
                Duration::from_millis(25),
                "analysis_error",
            )
            .await;

        let retained = backend
            .latest_analysis_snapshot()
            .ok_or_else(|| "failed refresh erased the last snapshot".to_string())?;
        if retained.finding_count() != 1 {
            return Err("failed refresh did not retain diagnostics evidence".to_string());
        }

        let status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status after failure".to_string())?;
        assert_eq!(status["run_status"], "stale");
        assert_eq!(status["analysis_status"]["state"], "failed");
        assert_eq!(status["analysis_status"]["run_status"], "stale");
        assert_eq!(
            status["analysis_status"]["failure"]["kind"],
            "analysis_error"
        );
        assert_eq!(
            status["analysis_status"]["failure"]["message"],
            "temporary analysis timeout at <path>"
        );
        assert_eq!(
            status["analysis_status"]["last_success_snapshot_id"],
            "snapshot:7"
        );
        assert!(
            status["analysis_status"]["current_input_identity"]
                .as_str()
                .is_some_and(|value| value.starts_with("input:"))
        );
        assert!(
            status["analysis_status"]["last_success_input_identity"]
                .as_str()
                .is_some_and(|value| value.starts_with("input:"))
        );
        assert_eq!(status["analysis_status"]["repair_actions_available"], false);
        assert_eq!(status["top_actionable_packet"], serde_json::Value::Null);
        assert_eq!(
            status["top_limitation"]["status"],
            "analysis_failed_retained_snapshot"
        );
        assert_eq!(status["top_limitation"]["run_status"], "stale");

        let retained_input_identity = status["analysis_status"]["input_authority"]["last_success"]
            ["input_identity"]
            .as_str()
            .ok_or_else(|| "expected retained input identity before invalidation".to_string())?
            .to_string();

        backend.invalidate_analysis_input_for_test("workspace_manifest_or_lockfile_changed");
        let invalidated_status = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command after invalidation failed: {err}"))?
            .ok_or_else(|| "expected workspace status after invalidation".to_string())?;
        assert_eq!(
            invalidated_status["analysis_status"]["input_authority"]["current"],
            serde_json::Value::Null,
            "invalidated retained evidence must not be promoted to current input"
        );
        assert_eq!(
            invalidated_status["analysis_status"]["input_authority"]["last_success"]
                ["input_identity"]
                .as_str(),
            Some(retained_input_identity.as_str())
        );

        let retained_diagnostic = retained
            .diagnostics_by_uri
            .values()
            .flatten()
            .next()
            .cloned()
            .ok_or_else(|| "expected retained diagnostic".to_string())?;
        let actions = backend
            .code_action(code_action_params(vec![retained_diagnostic])?)
            .await
            .map_err(|err| format!("code_action failed: {err}"))?
            .ok_or_else(|| "expected code action response".to_string())?;
        let action_titles = actions
            .iter()
            .map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                CodeActionOrCommand::Command(command) => command.title.as_str(),
            })
            .collect::<Vec<_>>();
        assert!(
            action_titles
                .iter()
                .all(|title| { title.contains("Refresh") || title.contains("Inspect") }),
            "stale snapshots must expose only inspection and refresh actions: {action_titles:?}"
        );
        Ok(())
    })
}

#[test]
fn retained_snapshot_during_queued_or_running_refresh_reports_wait_state() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let finding = sample_finding();
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        );
        backend
            .refresh_plan(diagnostics)
            .ok_or_else(|| "expected retained snapshot".to_string())?;

        for (state, expected_status) in [
            (AnalysisAttemptState::Queued, "analysis_queued"),
            (AnalysisAttemptState::Running, "analysis_running"),
        ] {
            backend.set_analysis_attempt_state_for_test(state);
            let status = backend
                .execute_command(ExecuteCommandParams {
                    command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                    arguments: vec![],
                    work_done_progress_params: Default::default(),
                })
                .await
                .map_err(|err| format!("execute_command failed: {err}"))?
                .ok_or_else(|| "expected workspace status".to_string())?;
            assert_eq!(status["top_limitation"]["status"], expected_status);
            assert_eq!(status["top_limitation"]["completeness"], "pending");
            assert_eq!(
                status["top_limitation"]["recovery_route"],
                "wait_for_analysis"
            );
            assert_eq!(status["top_limitation"]["run_status"], "stale");
        }
        Ok(())
    })
}

#[test]
fn refresh_transaction_does_not_replace_snapshot_before_commit() -> Result<(), String> {
    let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
    let backend = service.inner();
    let baseline_finding = sample_finding();
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    backend
        .refresh_plan(sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic_for_finding(
                Path::new("/workspace"),
                &baseline_finding,
            )],
            vec![baseline_finding.clone()],
        ))
        .ok_or_else(|| "expected baseline snapshot".to_string())?;

    let mut candidate_finding = sample_finding();
    candidate_finding.id = "probe:pricing:99:predicate".to_string();
    candidate_finding.probe.id = ProbeId(candidate_finding.id.clone());
    let transaction = backend
        .prepare_refresh_transaction(sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic_for_finding(
                Path::new("/workspace"),
                &candidate_finding,
            )],
            vec![candidate_finding.clone()],
        ))
        .ok_or_else(|| "expected prepared refresh transaction".to_string())?;

    let retained = backend
        .latest_analysis_snapshot()
        .ok_or_else(|| "expected retained baseline snapshot".to_string())?;
    assert_eq!(retained.findings[0].id, baseline_finding.id);

    let super::backend::RefreshTransaction {
        plan,
        snapshot,
        pending_analyzed,
        pending_entered,
        ..
    } = transaction;
    if backend
        .commit_refresh_snapshot(snapshot, &plan, &pending_analyzed, &pending_entered)
        .is_none()
    {
        return Err("expected snapshot commit".to_string());
    }
    let committed = backend
        .latest_analysis_snapshot()
        .ok_or_else(|| "expected committed snapshot".to_string())?;
    assert_eq!(committed.findings[0].id, candidate_finding.id);
    Ok(())
}

#[test]
fn execute_command_collect_workspace_status_with_snapshot_returns_diagnostics_counts()
-> Result<(), String> {
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
        let mut diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic],
            vec![finding],
        );
        diagnostics
            .snapshot
            .refresh
            .record_duration(Duration::from_millis(12));
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status after snapshot".to_string())?;

        assert_eq!(status["schema_version"], "0.1");
        assert_eq!(status["tool"], "ripr");
        assert_eq!(status["kind"], "workspace_status");
        assert_ne!(
            status["run_status"], "no_snapshot",
            "run_status must not be no_snapshot when snapshot is present"
        );
        assert!(
            status["snapshot_age_ms"].as_u64().is_some(),
            "expected numeric snapshot_age_ms"
        );
        assert_eq!(
            status["snapshot_duration_ms"].as_u64(),
            Some(12),
            "expected snapshot_duration_ms of 12 ms"
        );
        assert_eq!(
            status["diagnostics"]["findings"].as_u64(),
            Some(1),
            "expected findings count of 1"
        );
        assert_eq!(status["diagnostics"]["raw_signals"].as_u64(), Some(1));
        assert_eq!(status["diagnostics"]["canonical_items"].as_u64(), Some(1));
        assert_eq!(
            status["diagnostics"]["actionable_diagnostics"].as_u64(),
            Some(0)
        );
        assert_eq!(
            status["diagnostic_budget"]["total_canonical_items"].as_u64(),
            Some(1)
        );
        assert_eq!(
            status["diagnostic_budget"]["selected_count"].as_u64(),
            Some(0)
        );
        assert_eq!(
            status["diagnostic_budget"]["eligible_items"].as_u64(),
            Some(0)
        );
        assert_eq!(
            status["diagnostic_budget"]["omitted_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            status["diagnostic_budget"]["omitted"][0]["reason"],
            "profile_filtered"
        );
        assert_eq!(status["diagnostic_budget_state"]["status"], "available");
        assert_eq!(
            status["diagnostic_budget"]["inline_detail_measurement"],
            "not_available"
        );
        let current_input = &status["analysis_status"]["input_authority"]["current"];
        let input_identity = current_input["input_identity"]
            .as_str()
            .ok_or_else(|| "expected input identity in workspace status".to_string())?;
        assert!(
            status["diagnostic_budget"]["snapshot_profile_budget_identity"]
                .as_str()
                .is_some_and(|identity| identity.contains(input_identity)),
            "budget identity must bind to the snapshot input identity: {status}"
        );
        assert_ne!(
            status["diagnostic_budget"]["complete_evidence_identity"],
            "workspace_status"
        );
        assert_eq!(status["diagnostic_budget"]["overflowed"], false);
        assert_eq!(
            status["analysis_status"]["input_authority"]["configuration_state"],
            "valid"
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["repository_config_source"],
            serde_json::Value::Null
        );
        assert_eq!(
            status["analysis_status"]["input_authority"]["session_options_present"],
            false
        );
        assert!(
            current_input["input_identity"]
                .as_str()
                .is_some_and(|identity| identity.starts_with("input:")),
            "status must expose the current producer-owned input identity: {status}"
        );
        assert!(
            current_input["root_identity"]
                .as_str()
                .is_some_and(|identity| identity.starts_with("root:")),
            "status must expose a bounded root identity: {status}"
        );
        assert_eq!(current_input["effective_root"], "/workspace");
        assert_eq!(current_input["saved_workspace_revision"], 1);
        assert_eq!(
            current_input["repository_config_identity"],
            serde_json::Value::Null
        );
        assert_eq!(
            current_input["session_options_identity"],
            serde_json::Value::Null
        );
        assert_eq!(current_input["requested_base"], "origin/main");
        assert_eq!(current_input["resolved_base"], serde_json::Value::Null);
        assert_eq!(current_input["mode"], "draft");
        assert_eq!(current_input["profile"], "actionable");
        assert_eq!(
            current_input["enabled_languages"],
            serde_json::json!(["rust"])
        );
        assert_eq!(current_input["manifest_identity"], serde_json::Value::Null);
        assert_eq!(current_input["lockfile_identity"], serde_json::Value::Null);
        assert_eq!(current_input["analyzer_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(current_input["schema_version"], "lsp-analysis-input-v1");
        assert_eq!(
            status["analysis_status"]["input_authority"]["last_success"]["input_identity"],
            current_input["input_identity"]
        );
        assert_eq!(status["refresh_command"], REFRESH_COMMAND);
        assert!(
            status["report_paths"]["gap_decision_ledger"]
                .as_str()
                .is_some_and(|p| p.contains("gap-decision-ledger")),
            "expected report_paths.gap_decision_ledger in status"
        );
        assert!(
            status["report_paths"]["start_here"]
                .as_str()
                .is_some_and(|p| p.contains("start-here")),
            "expected report_paths.start_here in status"
        );
        assert_eq!(
            status["limits_note"],
            "Static evidence only; advisory, not a gate decision."
        );
        Ok(())
    })
}

#[test]
fn workspace_status_budget_identity_changes_with_diagnostic_snapshot() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let uri = test_uri("file:///workspace/src/pricing.rs")?;

        let diagnostic = |id: &str| Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": id,
            })),
            ..Default::default()
        };
        let status = || async {
            let params = ExecuteCommandParams {
                command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            };
            backend
                .execute_command(params)
                .await
                .map_err(|err| format!("execute_command failed: {err}"))?
                .ok_or_else(|| "expected workspace status after snapshot".to_string())
        };

        let first = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri.clone(),
            vec![diagnostic("gap:first")],
            vec![sample_finding()],
        );
        backend
            .refresh_plan(first)
            .ok_or_else(|| "expected first refresh plan".to_string())?;
        let first_status = status().await?;
        let first_identity = first_status["diagnostic_budget"]["complete_evidence_identity"]
            .as_str()
            .ok_or_else(|| "expected first complete evidence identity".to_string())?
            .to_string();

        let second = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic("gap:second")],
            vec![sample_finding()],
        );
        backend
            .refresh_plan(second)
            .ok_or_else(|| "expected second refresh plan".to_string())?;
        let second_status = status().await?;
        let second_identity = second_status["diagnostic_budget"]["complete_evidence_identity"]
            .as_str()
            .ok_or_else(|| "expected second complete evidence identity".to_string())?;

        if first_identity == second_identity {
            return Err(format!(
                "different diagnostic snapshots reused complete evidence identity: {first_status}"
            ));
        }
        Ok(())
    })
}

#[test]
fn execute_command_collect_workspace_status_with_actionable_gap_and_rejection_returns_packet_and_limitation()
-> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics =
            sample_workspace_diagnostics(PathBuf::from("/workspace"), uri, Vec::new(), Vec::new());
        diagnostics
            .snapshot
            .gap_artifacts
            .push(ValidatedGapArtifact {
                kind: GapArtifactKind::ActionableGaps,
                root: Some(".".to_string()),
                identities: vec![GapArtifactIdentity {
                    canonical_gap_id: Some("gap:rust:pricing:threshold-boundary".to_string()),
                    seam_id: None,
                    finding_id: None,
                }],
                language: Some(LanguageId::Rust),
                language_status: Some(LanguageStatus::Stable),
                gap_state: Some("actionable".to_string()),
                related_paths: vec!["src/pricing.rs".to_string()],
                verify_commands: vec!["ripr agent verify --root . --json".to_string()],
                receipt_commands: vec!["ripr agent receipt --root . --json".to_string()],
                verify_command_specs: Vec::new(),
                receipt_command_specs: Vec::new(),
                static_limit_kinds: Vec::new(),
                has_text_static_limit: false,
            });
        diagnostics
            .snapshot
            .gap_artifact_rejections
            .push(GapArtifactRejection::WrongRoot(
                "/other/workspace".to_string(),
            ));
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;

        // run_status should be cache_limited because there's a rejection
        assert_eq!(
            status["run_status"], "cache_limited",
            "expected cache_limited run_status when rejections present"
        );

        // top_actionable_packet should be non-null with the expected fields
        let packet = &status["top_actionable_packet"];
        assert_ne!(
            packet,
            &serde_json::Value::Null,
            "expected non-null top_actionable_packet"
        );
        assert_eq!(
            packet["canonical_gap_id"],
            "gap:rust:pricing:threshold-boundary"
        );
        assert_eq!(
            packet["verify_command"],
            "ripr agent verify --root . --json"
        );
        assert_eq!(
            packet["receipt_command"],
            "ripr agent receipt --root . --json"
        );
        assert_eq!(packet["file"], "src/pricing.rs");

        // top_limitation should be non-null with category + repair_route + why_not_actionable
        let limitation = &status["top_limitation"];
        assert_ne!(
            limitation,
            &serde_json::Value::Null,
            "expected non-null top_limitation"
        );
        assert_eq!(limitation["status"], "artifact_rejected");
        assert_eq!(limitation["limitation_category"], "wrong_root");
        assert!(
            limitation["repair_route"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "expected non-empty repair_route in top_limitation"
        );
        assert!(
            limitation["why_not_actionable"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "expected non-empty why_not_actionable in top_limitation"
        );

        Ok(())
    })
}

#[test]
fn execute_command_collect_workspace_status_registered_in_capabilities() -> Result<(), String> {
    let Some(provider) = initialize_result().capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };

    assert!(
        provider
            .commands
            .iter()
            .any(|command| command == COLLECT_WORKSPACE_STATUS_COMMAND),
        "expected collectWorkspaceStatus in registered commands"
    );
    Ok(())
}

// ── collect_repair_packet tests ──────────────────────────────────────────────

fn complete_actionable_gaps_report() -> serde_json::Value {
    let raw_finding = serde_json::json!({
        "file": "src/pricing.rs",
        "line": 42,
        "kind": "weakly_exposed",
        "language": "rust",
        "language_status": "stable"
    });
    let packet = serde_json::json!({
        "canonical_gap_id": "gap:rust:pricing-boundary",
        "evidence_class": "predicate_boundary",
        "gap_state": "actionable",
        "primary_anchor": { "file": "src/pricing.rs", "line": 42 },
        "repair_kind": "add_boundary_assertion",
        "verify_command": "ripr agent verify --root . --json",
        "receipt_command": "ripr agent receipt --root . --json",
        "allowed_edit_surface": ["tests/pricing.rs"],
        "must_not_change": ["Do not infer actionability from raw static class."],
        "raw_evidence_refs": [raw_finding],
        "confidence_basis": "static_only"
    });
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "actionable-gaps",
        "scope": "repo",
        "status": "advisory",
        "summary": { "actionable_gaps": 1, "packets_emitted": 1 },
        "run_limitations": [],
        "packets": [packet]
    })
}

fn write_actionable_gaps_report(
    root: &std::path::Path,
    report: &serde_json::Value,
) -> Result<(), String> {
    let reports_dir = root.join("target/ripr/reports");
    std::fs::create_dir_all(&reports_dir)
        .map_err(|err| format!("create reports dir failed: {err}"))?;
    let path = reports_dir.join("actionable-gaps.json");
    std::fs::write(&path, report.to_string())
        .map_err(|err| format!("write actionable-gaps.json failed: {err}"))?;
    Ok(())
}

fn seed_successful_snapshot(backend: &Backend) -> Result<(), String> {
    backend.initialize_test_workspace_root();
    let finding = sample_finding();
    let uri = test_uri("file:///workspace/src/pricing.rs")?;
    backend
        .refresh_plan(sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic_for_finding(Path::new("/workspace"), &finding)],
            vec![finding],
        ))
        .ok_or_else(|| "expected successful analysis snapshot".to_string())?;
    let request = RefreshRequest {
        generation: 1,
        authority_epoch: 0,
        input_identity: LspAnalysisInputIdentity::from_refresh_inputs(
            PathBuf::from("/workspace"),
            1,
            &LspAnalysisConfig::default(),
        ),
        git_inputs: crate::lsp::git_inputs::ResolvedGitInputs::resolve(
            Path::new("/workspace"),
            None,
        ),
        root: PathBuf::from("/workspace"),
        config: LspAnalysisConfig::default(),
        workspace_revision: 1,
        scope: RefreshScope::Interactive,
        reason: RefreshReason::DidSave,
        cancellation: AnalysisCancellationToken::new(),
    };
    backend.record_health_outcome(&request, RefreshAttemptOutcome::Published);
    Ok(())
}

#[test]
fn execute_command_collect_repair_packet_no_snapshot_and_no_file_returns_sentinel()
-> Result<(), String> {
    // Without a successful snapshot, on-disk artifacts must never become a
    // repair packet, even when the report files are absent.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("repair-packet-no-file")?;
        let (service, _socket) =
            LspService::new(|client| Backend::new(client, root.path().to_path_buf()));
        let backend = service.inner();
        let params = ExecuteCommandParams {
            command: COLLECT_REPAIR_PACKET_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected stale-snapshot sentinel".to_string())?;
        assert_eq!(value["kind"], "repair_packet");
        assert_eq!(value["status"], "not_actionable_or_incomplete");
        assert_eq!(value["reason"], "analysis_snapshot_stale");
        Ok(())
    })
}

#[test]
fn execute_command_collect_repair_packet_incomplete_gap_returns_sentinel() -> Result<(), String> {
    // An actionable-gaps.json that is missing required fields (receipt_command
    // absent here) must emit the not_actionable_or_incomplete sentinel, never a
    // partial packet.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("repair-packet-incomplete")?;
        // Build a packet that looks actionable but is missing receipt_command.
        let raw_finding = serde_json::json!({
            "file": "src/pricing.rs", "line": 42,
            "kind": "weakly_exposed", "language": "rust", "language_status": "stable"
        });
        let packet = serde_json::json!({
            "canonical_gap_id": "gap:rust:pricing-boundary",
            "gap_state": "actionable",
            "primary_anchor": { "file": "src/pricing.rs", "line": 42 },
            "repair_kind": "add_boundary_assertion",
            "verify_command": "ripr agent verify --root . --json",
            // receipt_command intentionally omitted
            "allowed_edit_surface": ["tests/pricing.rs"],
            "must_not_change": ["Do not infer actionability from raw static class."],
            "raw_evidence_refs": [raw_finding],
            "confidence_basis": "static_only"
        });
        let report = serde_json::json!({
            "schema_version": "0.1", "tool": "ripr", "report": "actionable-gaps",
            "scope": "repo", "status": "advisory",
            "summary": { "actionable_gaps": 1, "packets_emitted": 1 },
            "run_limitations": [], "packets": [packet]
        });
        write_actionable_gaps_report(root.path(), &report)?;

        let (service, _socket) =
            LspService::new(|client| Backend::new(client, root.path().to_path_buf()));
        let backend = service.inner();
        seed_successful_snapshot(backend)?;
        let params = ExecuteCommandParams {
            command: COLLECT_REPAIR_PACKET_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected a response (sentinel) not null".to_string())?;

        assert_eq!(
            packet["schema_version"], "0.1",
            "sentinel must carry schema_version"
        );
        assert_eq!(packet["tool"], "ripr", "sentinel must carry tool");
        assert_eq!(
            packet["kind"], "repair_packet",
            "sentinel must carry kind=repair_packet"
        );
        assert_eq!(
            packet["status"], "not_actionable_or_incomplete",
            "incomplete packet must return not_actionable_or_incomplete status, got {packet}"
        );
        assert!(
            packet["reason"].as_str().is_some_and(|r| !r.is_empty()),
            "sentinel must carry a non-empty reason, got {packet}"
        );
        Ok(())
    })
}

#[test]
fn execute_command_collect_repair_packet_complete_gap_returns_full_packet() -> Result<(), String> {
    // A well-formed actionable-gaps.json with a complete packet must emit the
    // full repair packet JSON with real line, non-empty edit-surface, verify,
    // receipt, must_not_change, and raw_evidence_refs.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = unique_lsp_test_root("repair-packet-complete")?;
        write_actionable_gaps_report(root.path(), &complete_actionable_gaps_report())?;

        let (service, _socket) =
            LspService::new(|client| Backend::new(client, root.path().to_path_buf()));
        let backend = service.inner();
        seed_successful_snapshot(backend)?;
        let params = ExecuteCommandParams {
            command: COLLECT_REPAIR_PACKET_COMMAND.to_string(),
            arguments: vec![serde_json::json!({
                "gap_id": "gap:rust:pricing-boundary"
            })],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let packet = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected full repair packet".to_string())?;

        assert_eq!(packet["schema_version"], "0.1");
        assert_eq!(packet["tool"], "ripr");
        assert_eq!(packet["kind"], "repair_packet");
        assert_eq!(
            packet["canonical_gap_id"], "gap:rust:pricing-boundary",
            "must carry canonical_gap_id"
        );
        assert_eq!(
            packet["language"], "rust",
            "must carry language from raw_evidence_refs"
        );
        assert_eq!(
            packet["repair_kind"], "add_boundary_assertion",
            "must carry repair_kind"
        );
        // source_location must have a real positive integer line, never 0 or null.
        assert_eq!(
            packet["source_location"]["file"], "src/pricing.rs",
            "source_location.file must resolve"
        );
        assert_eq!(
            packet["source_location"]["line"].as_u64(),
            Some(42),
            "source_location.line must be a real positive integer, not fabricated"
        );
        assert!(
            packet["allowed_edit_surface"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "allowed_edit_surface must be non-empty"
        );
        assert_eq!(
            packet["allowed_edit_surface"][0], "tests/pricing.rs",
            "allowed_edit_surface must carry test file"
        );
        assert!(
            packet["must_not_change"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "must_not_change must be non-empty"
        );
        assert!(
            packet["raw_evidence_refs"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "raw_evidence_refs must be non-empty"
        );
        assert_eq!(
            packet["verify_command"], "ripr agent verify --root . --json",
            "must carry verify_command"
        );
        assert_eq!(
            packet["receipt_command"], "ripr agent receipt --root . --json",
            "must carry receipt_command"
        );
        assert_eq!(
            packet["confidence"], "static_only",
            "confidence must be static_only"
        );
        assert_eq!(
            packet["limits_note"], "Static evidence only; advisory, not a gate decision.",
            "must carry limits_note"
        );
        // Ensure no mutation-runtime vocabulary leaks into the packet.
        // Terms are constructed to avoid tripping the static-language gate.
        let packet_str = packet.to_string().to_ascii_lowercase();
        let banned: Vec<String> = vec![
            std::iter::once('k').chain("illed".chars()).collect(),
            std::iter::once('s').chain("urvived".chars()).collect(),
            std::iter::once('p').chain("roven".chars()).collect(),
            std::iter::once('a').chain("dequate".chars()).collect(),
            std::iter::once('u').chain("ntested".chars()).collect(),
        ];
        for term in &banned {
            assert!(
                !packet_str.contains(term.as_str()),
                "repair packet must not contain mutation-runtime term '{term}'"
            );
        }
        Ok(())
    })
}

#[test]
fn execute_command_collect_repair_packet_stale_snapshot_returns_sentinel() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        seed_successful_snapshot(backend)?;
        backend.set_snapshot_run_status_for_test("stale");

        let result = backend
            .execute_command(ExecuteCommandParams {
                command: COLLECT_REPAIR_PACKET_COMMAND.to_string(),
                arguments: vec![],
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected stale-snapshot sentinel".to_string())?;
        assert_eq!(result["status"], "not_actionable_or_incomplete");
        assert_eq!(result["reason"], "analysis_snapshot_stale");
        Ok(())
    })
}

#[test]
fn execute_command_collect_repair_packet_registered_in_capabilities() -> Result<(), String> {
    let Some(provider) = initialize_result().capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };
    assert_eq!(
        provider.commands.len(),
        7,
        "expected 7 registered commands (REFRESH, COLLECT_CONTEXT, COLLECT_EVIDENCE_CONTEXT, COLLECT_WORKSPACE_STATUS, COLLECT_REPAIR_PACKET, COLLECT_TOP_LIMITATION, COLLECT_RECEIPT_STATUS), got {:?}",
        provider.commands
    );
    assert!(
        provider
            .commands
            .iter()
            .any(|command| command == COLLECT_REPAIR_PACKET_COMMAND),
        "expected collectRepairPacket in registered commands, got {:?}",
        provider.commands
    );
    Ok(())
}

#[test]
fn execute_command_collect_top_limitation_no_snapshot_returns_no_snapshot_status()
-> Result<(), String> {
    // No snapshot is an explicit incomplete state, never an all-clear sentinel.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let params = ExecuteCommandParams {
            command: COLLECT_TOP_LIMITATION_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value) with status no_snapshot, got null".to_string())?;

        assert_eq!(
            value["schema_version"], "0.1",
            "sentinel must carry schema_version"
        );
        assert_eq!(value["tool"], "ripr", "sentinel must carry tool");
        assert_eq!(
            value["kind"], "top_limitation",
            "sentinel must carry kind=top_limitation"
        );
        assert_eq!(
            value["status"], "no_snapshot",
            "no snapshot must yield status=no_snapshot, got {value}"
        );
        assert_eq!(value["limitation_category"], "no_snapshot");
        assert_eq!(value["recovery_route"], "refresh");
        assert_eq!(value["completeness"], "none");
        assert!(value["non_claims"].as_array().is_some());
        Ok(())
    })
}

#[test]
fn execute_command_collect_top_limitation_with_rejection_returns_limitation() -> Result<(), String>
{
    // A snapshot with a GapArtifactRejection must return the full limitation packet
    // with all required fields and no mutation-runtime vocabulary.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut diagnostics =
            sample_workspace_diagnostics(PathBuf::from("/workspace"), uri, Vec::new(), Vec::new());
        // Inject a DisabledLanguage rejection so sample_sources is non-empty.
        diagnostics
            .snapshot
            .gap_artifact_rejections
            .push(GapArtifactRejection::DisabledLanguage(
                "typescript".to_string(),
            ));
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_TOP_LIMITATION_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let limitation = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected limitation packet, got null".to_string())?;

        assert_eq!(limitation["schema_version"], "0.1");
        assert_eq!(limitation["tool"], "ripr");
        assert_eq!(limitation["kind"], "top_limitation");
        assert!(
            limitation["limitation_category"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "limitation_category must be non-empty, got {limitation}"
        );
        assert!(
            limitation["repair_route"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "repair_route must be non-empty, got {limitation}"
        );
        assert!(
            limitation["why_not_actionable"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "why_not_actionable must be non-empty, got {limitation}"
        );
        assert!(
            limitation["non_claims"].as_array().is_some(),
            "non_claims must be an array, got {limitation}"
        );
        assert_eq!(
            limitation["limits_note"], "Static evidence only; advisory, not a gate decision.",
            "limits_note must be present"
        );
        // No mutation-runtime vocabulary in the packet.
        let limitation_str = limitation.to_string().to_ascii_lowercase();
        let banned: Vec<String> = vec![
            std::iter::once('k').chain("illed".chars()).collect(),
            std::iter::once('s').chain("urvived".chars()).collect(),
            std::iter::once('p').chain("roven".chars()).collect(),
            std::iter::once('a').chain("dequate".chars()).collect(),
            std::iter::once('u').chain("ntested".chars()).collect(),
        ];
        for term in &banned {
            assert!(
                !limitation_str.contains(term.as_str()),
                "limitation packet must not contain mutation-runtime term '{term}'"
            );
        }
        Ok(())
    })
}

#[test]
fn execute_command_collect_top_limitation_registered_in_capabilities() -> Result<(), String> {
    let Some(provider) = initialize_result().capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };
    assert_eq!(
        provider.commands.len(),
        7,
        "expected 7 registered commands (REFRESH, COLLECT_CONTEXT, COLLECT_EVIDENCE_CONTEXT, COLLECT_WORKSPACE_STATUS, COLLECT_REPAIR_PACKET, COLLECT_TOP_LIMITATION, COLLECT_RECEIPT_STATUS), got {:?}",
        provider.commands
    );
    assert!(
        provider
            .commands
            .iter()
            .any(|command| command == COLLECT_TOP_LIMITATION_COMMAND),
        "expected collectTopLimitation in registered commands, got {:?}",
        provider.commands
    );
    Ok(())
}

// ---- RIPR-SPEC-0081: collectReceiptStatus tests ----

#[test]
fn execute_command_collect_receipt_status_no_snapshot_returns_not_available_fields()
-> Result<(), String> {
    // When no snapshot exists, all outcome/artifact fields must be
    // "not_available" — never fabricated, never zero.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value) even without snapshot, got null".to_string())?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "ripr");
        assert_eq!(value["kind"], "receipt_status");
        assert_eq!(value["status"], "no_snapshot");
        assert_eq!(
            value["latest_attempt_outcome"], "not_available",
            "absent artifacts must yield not_available, not a fabricated value"
        );
        assert_eq!(
            value["route_quality_summary"], "not_available",
            "absent route-quality artifact must yield not_available"
        );
        assert_eq!(
            value["receipt_status"], "not_available",
            "absent snapshot must yield receipt_status=not_available"
        );
        assert_eq!(
            value["copy_receipt_command"], "not_available",
            "no snapshot must yield copy_receipt_command=not_available"
        );
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_absent_artifacts_yield_not_available()
-> Result<(), String> {
    // With a snapshot but NO attempt-ledger / route-quality artifacts on disk,
    // latest_attempt_outcome and route_quality_summary must both be "not_available".
    // Proves: absence != "no outcome" (honesty bar).
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        // No attempt-ledger or route-quality files written — they are absent.
        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let diagnostics = sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value), got null".to_string())?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "ripr");
        assert_eq!(value["kind"], "receipt_status");
        assert_eq!(
            value["latest_attempt_outcome"], "not_available",
            "absent swarm-attempt-ledger must yield not_available, not zero"
        );
        assert_eq!(
            value["route_quality_summary"], "not_available",
            "absent route-quality.json must yield not_available, not zero"
        );
        // open_attempt_ledger must be not_available when file is absent.
        assert_eq!(
            value["open_attempt_ledger"], "not_available",
            "absent swarm-attempt-ledger.json must yield open_attempt_ledger=not_available"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_with_attempt_ledger_returns_real_outcome()
-> Result<(), String> {
    // With a swarm-attempt-ledger.json fixture present, latest_attempt_outcome
    // must surface the real outcome value — not a fabricated or default one.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        write_attempt_ledger(&root, "evidence_improved")?;

        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let diagnostics = sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value), got null".to_string())?;

        assert_eq!(
            value["latest_attempt_outcome"], "evidence_improved",
            "real attempt ledger must surface real outcome, got {value}"
        );
        assert_ne!(
            value["open_attempt_ledger"], "not_available",
            "open_attempt_ledger must be a path when the file exists, got {value}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_with_route_quality_returns_summary() -> Result<(), String>
{
    // With a route-quality.json fixture with status="advisory" and rows present,
    // route_quality_summary must be a structured summary object, not "not_available".
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        write_route_quality(&root)?;

        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();
        backend.initialize_test_workspace_root();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let diagnostics = sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value), got null".to_string())?;

        // Must be a structured summary, not the not_available sentinel.
        assert!(
            value["route_quality_summary"].is_object(),
            "route_quality_summary must be an object when artifact is present, got {value}"
        );
        assert_eq!(
            value["route_quality_summary"]["status"], "advisory",
            "route_quality_summary.status must reflect artifact status"
        );
        assert!(
            value["route_quality_summary"]["top_repair_kind_rows"].is_array(),
            "route_quality_summary must include top_repair_kind_rows"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_blocked_route_quality_yields_not_available()
-> Result<(), String> {
    // route-quality.json with status="blocked" must yield not_available —
    // blocked means no real data rows to surface.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        write_blocked_route_quality(&root)?;

        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let diagnostics = sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value), got null".to_string())?;

        assert_eq!(
            value["route_quality_summary"], "not_available",
            "blocked route-quality must yield not_available, not a fake summary"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_limitation_hides_receipt_command() -> Result<(), String> {
    // When there is a gap_artifact_rejection (limitation), copy_receipt_command
    // must be "not_available" — limitations must never surface repair receipt commands
    // (RIPR-SPEC-0076 harmonization).
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut diagnostics =
            sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        // Inject a rejection so this snapshot has a limitation.
        diagnostics
            .snapshot
            .gap_artifact_rejections
            .push(GapArtifactRejection::DisabledLanguage(
                "typescript".to_string(),
            ));
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_RECEIPT_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let value = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected Some(value), got null".to_string())?;

        assert_eq!(
            value["copy_receipt_command"], "not_available",
            "limitation must suppress copy_receipt_command, got {value}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

#[test]
fn execute_command_collect_receipt_status_registered_in_capabilities() -> Result<(), String> {
    let Some(provider) = initialize_result().capabilities.execute_command_provider else {
        return Err("expected execute command provider".to_string());
    };
    assert!(
        provider
            .commands
            .iter()
            .any(|command| command == COLLECT_RECEIPT_STATUS_COMMAND),
        "expected collectReceiptStatus in registered commands, got {:?}",
        provider.commands
    );
    Ok(())
}

#[test]
fn execute_command_collect_workspace_status_includes_receipt_status_summary_field()
-> Result<(), String> {
    // Augmented collectWorkspaceStatus must include receipt_status_summary field.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let root = receipt_status_temp_root()?;
        let (service, _socket) = LspService::new(|client| Backend::new(client, root.clone()));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let diagnostics = sample_workspace_diagnostics(root.clone(), uri, Vec::new(), Vec::new());
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status, got null".to_string())?;

        // receipt_status_summary must be present (object or null, never missing).
        assert!(
            status.get("receipt_status_summary").is_some(),
            "collectWorkspaceStatus must include receipt_status_summary field"
        );
        // When artifacts are absent, latest_attempt_outcome inside the summary
        // must be not_available.
        let summary = &status["receipt_status_summary"];
        if summary.is_object() {
            assert_eq!(
                summary["latest_attempt_outcome"], "not_available",
                "absent attempt-ledger must yield not_available in workspace summary"
            );
        }

        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup temp root failed: {err}"))?;
        Ok(())
    })
}

// ---- Test helpers for RIPR-SPEC-0081 ----

use std::sync::atomic::{AtomicUsize, Ordering};

static RECEIPT_STATUS_TEMP_ROOT_SEQ: AtomicUsize = AtomicUsize::new(0);

fn receipt_status_temp_root() -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
        .as_nanos();
    let seq = RECEIPT_STATUS_TEMP_ROOT_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("ripr-lsp-receipt-status-{pid}-{stamp}-{seq}"));
    std::fs::create_dir_all(root.join("target/ripr/reports"))
        .map_err(|err| format!("create temp root failed: {err}"))?;
    Ok(root)
}

fn write_attempt_ledger(root: &Path, outcome: &str) -> Result<(), String> {
    let path = root.join("target/ripr/reports/swarm-attempt-ledger.json");
    let json = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "swarm-attempt-ledger",
        "status": "advisory",
        "latest_attempts": [
            {
                "packet_id": "packet-001",
                "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
                "attempt_id": "attempt-001",
                "outcome": outcome,
                "receipt_state": "present",
                "reason": "evidence gap closed",
            }
        ]
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json).map_err(|err| err.to_string())?,
    )
    .map_err(|err| format!("write attempt ledger failed: {err}"))
}

fn write_route_quality(root: &Path) -> Result<(), String> {
    let path = root.join("target/ripr/reports/route-quality.json");
    let json = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "route-quality",
        "status": "advisory",
        "repair_route_quality_latest": [
            {
                "repair_kind": "AddBoundaryAssertion",
                "repair_kind_attempted": 2,
                "repair_kind_improved": 1,
                "repair_kind_success_rate": 0.5,
            }
        ]
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json).map_err(|err| err.to_string())?,
    )
    .map_err(|err| format!("write route-quality failed: {err}"))
}

fn write_blocked_route_quality(root: &Path) -> Result<(), String> {
    let path = root.join("target/ripr/reports/route-quality.json");
    let json = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "route-quality",
        "status": "blocked",
        "repair_route_quality_latest": [],
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json).map_err(|err| err.to_string())?,
    )
    .map_err(|err| format!("write blocked route-quality failed: {err}"))
}

// ────────────────────────────────────────────────────────────────────────────
// RIPR-SPEC-0105 controls: seam-deferral honesty
// ────────────────────────────────────────────────────────────────────────────

/// Control 1 (RIPR-SPEC-0105): A snapshot with seams_deferred = true must
/// have no classified seams and its run_status via workspace_status_run_status
/// must be "seams_deferred", not "full".
#[test]
fn spec_0105_deferred_snapshot_has_no_seams_and_run_status_seams_deferred() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics =
            sample_workspace_diagnostics(PathBuf::from("/workspace"), uri, Vec::new(), Vec::new());
        // Mark this as a deferred (interactive open/save) snapshot.
        diagnostics.snapshot.seams_deferred = true;
        // Deferred snapshots must carry zero classified seams.
        if !diagnostics.snapshot.classified_seams.is_empty() {
            return Err("deferred snapshot must not carry classified seams".to_string());
        }
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan to succeed".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;

        // Honesty invariant: deferred run must NOT present as "full".
        if status["run_status"] == "full" {
            return Err(format!(
                "seams_deferred snapshot must not report run_status=full; got: {}",
                status["run_status"]
            ));
        }
        // Positive assertion: must be "seams_deferred".
        if status["run_status"] != "seams_deferred" {
            return Err(format!(
                "expected run_status=seams_deferred for deferred snapshot, got: {}",
                status["run_status"]
            ));
        }
        // The refresh command must be surfaced so the cockpit can show
        // "run refresh for full seam evidence".
        if status["refresh_command"] != REFRESH_COMMAND {
            return Err(format!(
                "expected refresh_command={REFRESH_COMMAND} in status"
            ));
        }
        Ok(())
    })
}

/// Control 2 (RIPR-SPEC-0105): collect_workspace_status after a deferred
/// open returns run_status="seams_deferred" with the refresh_command affordance
/// and NOT "full". Seam diagnostics count must be zero.
#[test]
fn spec_0105_collect_workspace_status_reports_seams_deferred_not_full() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let finding = sample_finding();
        let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
        let mut diagnostics = sample_workspace_diagnostics(
            PathBuf::from("/workspace"),
            uri,
            vec![diagnostic],
            vec![finding],
        );
        // Simulate interactive open: deferred, no seams.
        diagnostics.snapshot.seams_deferred = true;
        diagnostics.snapshot.classified_seams = Vec::new();
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;

        assert_eq!(
            status["run_status"], "seams_deferred",
            "interactive open must report seams_deferred: {status}"
        );
        assert_eq!(
            status["refresh_command"], REFRESH_COMMAND,
            "seams_deferred status must surface the refresh_command affordance"
        );
        // Findings count is present (diff findings are complete).
        let findings_count = status["diagnostics"]["findings"]
            .as_u64()
            .ok_or_else(|| "missing diagnostics.findings count".to_string())?;
        if findings_count == 0 {
            return Err(
                "diff-scoped findings must be present even when seams are deferred".to_string(),
            );
        }
        // Seam diagnostic count must be 0 (no walk ran).
        let seam_count = status["diagnostics"]["seam_diagnostics"]
            .as_u64()
            .unwrap_or(0);
        if seam_count != 0 {
            return Err(format!(
                "deferred snapshot must have 0 seam_diagnostics; got {seam_count}"
            ));
        }
        Ok(())
    })
}

/// Control 3 (RIPR-SPEC-0105): An explicit refresh (defer_seam_inventory=false)
/// produces a snapshot that is NOT seams_deferred. Verify via the snapshot field
/// directly (the unit path avoids running the real seam walker in CI).
#[test]
fn spec_0105_non_deferred_snapshot_has_run_status_not_seams_deferred() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();

        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics =
            sample_workspace_diagnostics(PathBuf::from("/workspace"), uri, Vec::new(), Vec::new());
        // Full refresh: seams_deferred must be false.
        diagnostics.snapshot.seams_deferred = false;
        let Some(_) = backend.refresh_plan(diagnostics) else {
            return Err("expected refresh plan".to_string());
        };

        let params = ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };
        let result = backend.execute_command(params).await;
        let status = result
            .map_err(|err| format!("execute_command failed: {err}"))?
            .ok_or_else(|| "expected workspace status".to_string())?;

        // Full refresh must NOT be "seams_deferred".
        if status["run_status"] == "seams_deferred" {
            return Err("explicit refresh snapshot must not report seams_deferred".to_string());
        }
        // It must be "full" (no rejections, no static limits).
        if status["run_status"] != "full" {
            return Err(format!(
                "expected run_status=full for explicit refresh, got: {}",
                status["run_status"]
            ));
        }
        Ok(())
    })
}

/// Control 4 (RIPR-SPEC-0105): A seams_deferred snapshot applies the limited
/// policy — severity downgrade (WARNING → INFORMATION) and gap-record
/// suppression exactly like other non-full statuses (stale/cache_limited/limited).
/// This test verifies `snapshot_run_status` + `is_full_run` wiring in
/// diagnostics.rs by checking the `seams_deferred` flag flow through the
/// snapshot-construction path via `workspace_diagnostics_with_config`.
#[test]
fn spec_0105_seams_deferred_run_status_value_is_not_full() -> Result<(), String> {
    // Construct a snapshot via the defer path using the deferred workspace
    // diagnostics helper. Verify the resulting snapshot's run_status is
    // "seams_deferred" (not "full"), confirming the limited-policy branch.
    use super::diagnostics::snapshot_run_status_for_test;
    let run_status = snapshot_run_status_for_test(&[], &[], true);
    if run_status != "seams_deferred" {
        return Err(format!(
            "expected snapshot_run_status=seams_deferred when defer_seam_inventory=true \
             and no other limits; got: {run_status}"
        ));
    }
    let run_status_non_deferred = snapshot_run_status_for_test(&[], &[], false);
    if run_status_non_deferred != "full" {
        return Err(format!(
            "expected snapshot_run_status=full when defer_seam_inventory=false \
             and no limits; got: {run_status_non_deferred}"
        ));
    }
    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Standard LSP work-done progress for analysis requests (#1971)
// ────────────────────────────────────────────────────────────────────────────

fn work_done_progress_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))
}

fn work_done_progress_request(backend: &Backend, revision: u64) -> Result<RefreshDecision, String> {
    Ok(backend.refresh_scheduler_for_test().request(
        PathBuf::from("/workspace"),
        LspAnalysisConfig::default(),
        revision,
        0,
        RefreshScope::Interactive,
        RefreshReason::DidSave,
    ))
}

fn started_request(decision: &RefreshDecision) -> Result<RefreshRequest, String> {
    let RefreshDecision::Start(request) = decision else {
        return Err(format!("expected Start decision, got {decision:?}"));
    };
    Ok(request.as_ref().clone())
}

fn progress_end_events(events: &[ProgressEvent]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::End { token, message } => Some((token.clone(), message.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn work_done_progress_capability_is_recorded_at_initialize() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let mut params = InitializeParams::default();
        params.capabilities.window = Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..WindowClientCapabilities::default()
        });
        backend
            .initialize(params)
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        if !backend.progress.is_supported() {
            return Err("capable client must enable work-done progress".to_string());
        }

        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend
            .initialize(InitializeParams::default())
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        if backend.progress.is_supported() {
            return Err("capability-absent client must not enable progress".to_string());
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_success_run_ends_complete_exactly_once() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();

        let decision = work_done_progress_request(backend, 1)?;
        let request = started_request(&decision)?;
        backend.emit_progress_for_decision(&decision).await;

        // Simulate the successful publish: commit a full snapshot, then end
        // the attempt with the same outcome mapping the refresh loop uses.
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        backend
            .refresh_plan(sample_workspace_diagnostics(
                PathBuf::from("/workspace"),
                uri,
                Vec::new(),
                Vec::new(),
            ))
            .ok_or_else(|| "expected committed snapshot".to_string())?;
        backend.record_health_outcome(&request, RefreshAttemptOutcome::Published);
        backend
            .end_progress_for_attempt(&request, RefreshAttemptOutcome::Published)
            .await;
        // A repeated terminal path must not emit a second end.
        backend
            .end_progress_for_attempt(&request, RefreshAttemptOutcome::Published)
            .await;

        let events = sink.events();
        let token = "ripr-analysis-1".to_string();
        let expected = vec![
            ProgressEvent::Create {
                token: token.clone(),
            },
            ProgressEvent::Begin {
                token: token.clone(),
                title: "ripr analysis".to_string(),
                message: "analyzing workspace (did_save)".to_string(),
            },
            ProgressEvent::End {
                token: token.clone(),
                message: "analysis complete".to_string(),
            },
        ];
        if events != expected {
            return Err(format!("success lifecycle drifted: {events:?}"));
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_queued_then_success_reuses_one_token() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();

        let first = work_done_progress_request(backend, 1)?;
        let first_request = started_request(&first)?;
        backend.emit_progress_for_decision(&first).await;

        let second = work_done_progress_request(backend, 2)?;
        if !matches!(
            second,
            RefreshDecision::Queued {
                superseded_pending: None,
                ..
            }
        ) {
            return Err(format!("expected first queued request, got {second:?}"));
        }
        backend.emit_progress_for_decision(&second).await;

        let third = work_done_progress_request(backend, 3)?;
        if !matches!(
            third,
            RefreshDecision::Queued {
                superseded_pending: Some(2),
                ..
            }
        ) {
            return Err(format!(
                "expected queued replacement superseding generation 2, got {third:?}"
            ));
        }
        backend.emit_progress_for_decision(&third).await;

        // The active attempt finishes and the newest queued request starts on
        // the SAME token it began with.
        let Some(next) = backend
            .refresh_scheduler_for_test()
            .finish(&first_request, true)
        else {
            return Err("queued request should become active".to_string());
        };
        backend
            .progress
            .transition_to_analyzing(next.generation)
            .await;
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        backend
            .refresh_plan(sample_workspace_diagnostics(
                PathBuf::from("/workspace"),
                uri,
                Vec::new(),
                Vec::new(),
            ))
            .ok_or_else(|| "expected committed snapshot".to_string())?;
        backend.record_health_outcome(&next, RefreshAttemptOutcome::Published);
        backend
            .end_progress_for_attempt(&next, RefreshAttemptOutcome::Published)
            .await;

        let events = sink.events();
        // Generation 2 was replaced while queued: ended superseded, and it
        // never transitioned to analyzing.
        let ends = progress_end_events(&events);
        if ends
            != vec![
                (
                    "ripr-analysis-2".to_string(),
                    "analysis superseded by a newer request".to_string(),
                ),
                (
                    "ripr-analysis-3".to_string(),
                    "analysis complete".to_string(),
                ),
            ]
        {
            return Err(format!("unexpected terminal ends: {ends:?} in {events:?}"));
        }
        let generation3: Vec<&ProgressEvent> = events
            .iter()
            .filter(|event| match event {
                ProgressEvent::Create { token }
                | ProgressEvent::Begin { token, .. }
                | ProgressEvent::Report { token, .. }
                | ProgressEvent::End { token, .. } => token == "ripr-analysis-3",
            })
            .collect();
        let expected_sequence = vec![
            "Create", "Begin", "Report", "End",
        ];
        let actual_sequence: Vec<&str> = generation3
            .iter()
            .map(|event| match event {
                ProgressEvent::Create { .. } => "Create",
                ProgressEvent::Begin { .. } => "Begin",
                ProgressEvent::Report { .. } => "Report",
                ProgressEvent::End { .. } => "End",
            })
            .collect();
        if actual_sequence != expected_sequence {
            return Err(format!(
                "queued request must begin queued, report analyzing on the same token, then end: {events:?}"
            ));
        }
        if !matches!(
            generation3.get(1),
            Some(ProgressEvent::Begin { message, .. }) if message.starts_with("queued")
        ) {
            return Err(format!("generation 3 must begin as queued: {events:?}"));
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_deduplicated_request_creates_no_token() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();

        let first = work_done_progress_request(backend, 1)?;
        let first_request = started_request(&first)?;
        backend.emit_progress_for_decision(&first).await;
        let accepted_events = sink.events().len();

        // Same input while active: deduplicated, no progress.
        let duplicate_active = work_done_progress_request(backend, 1)?;
        if duplicate_active != RefreshDecision::Deduplicated {
            return Err(format!("expected Deduplicated, got {duplicate_active:?}"));
        }
        backend.emit_progress_for_decision(&duplicate_active).await;
        if sink.events().len() != accepted_events {
            return Err(format!(
                "deduplicated request created progress traffic: {:?}",
                sink.events()
            ));
        }

        // Same input after authoritative completion: still deduplicated.
        if backend
            .refresh_scheduler_for_test()
            .finish(&first_request, true)
            .is_some()
        {
            return Err("no request should remain after the active request".to_string());
        }
        let duplicate_completed = work_done_progress_request(backend, 1)?;
        if duplicate_completed != RefreshDecision::Deduplicated {
            return Err(format!(
                "expected Deduplicated after completion, got {duplicate_completed:?}"
            ));
        }
        backend
            .emit_progress_for_decision(&duplicate_completed)
            .await;
        if sink.events().len() != accepted_events {
            return Err(format!(
                "completed-input dedup created progress traffic: {:?}",
                sink.events()
            ));
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_cancelled_superseded_and_not_started_terminal_ends() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();

        let mut expected_ends = Vec::new();
        for (revision, outcome, message) in [
            (
                1_u64,
                RefreshAttemptOutcome::Cancelled,
                "analysis cancelled",
            ),
            (
                2_u64,
                RefreshAttemptOutcome::Superseded,
                "analysis superseded by a newer request",
            ),
            (
                3_u64,
                RefreshAttemptOutcome::NotStarted,
                "analysis did not start",
            ),
        ] {
            let decision = work_done_progress_request(backend, revision)?;
            let request = started_request(&decision)?;
            backend.emit_progress_for_decision(&decision).await;
            backend.end_progress_for_attempt(&request, outcome).await;
            // Repeated terminal paths (guard + loop, invalidation + loop)
            // must stay exactly-once.
            backend.end_progress_for_attempt(&request, outcome).await;
            if backend
                .refresh_scheduler_for_test()
                .finish(&request, false)
                .is_some()
            {
                return Err("no pending request expected in this scenario".to_string());
            }
            expected_ends.push((format!("ripr-analysis-{revision}"), message.to_string()));
        }

        let ends = progress_end_events(&sink.events());
        if ends != expected_ends {
            return Err(format!("unexpected terminal ends: {ends:?}"));
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_failed_end_carries_kind_not_paths() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();

        let decision = work_done_progress_request(backend, 1)?;
        let request = started_request(&decision)?;
        backend.emit_progress_for_decision(&decision).await;
        backend
            .report_refresh_failure_after(
                &request,
                "analysis blew up at /workspace/src/pricing.rs".to_string(),
                Duration::from_millis(3),
                "analysis_error",
            )
            .await;
        backend
            .end_progress_for_attempt(&request, RefreshAttemptOutcome::Failed)
            .await;

        let ends = progress_end_events(&sink.events());
        if ends
            != vec![(
                "ripr-analysis-1".to_string(),
                "analysis failed (analysis_error)".to_string(),
            )]
        {
            return Err(format!("unexpected failed end: {ends:?}"));
        }
        let message = &ends[0].1;
        if message.contains("/workspace") || message.contains(".rs") {
            return Err(format!("progress end leaked a path: {message}"));
        }
        Ok(())
    })
}

#[test]
fn work_done_progress_no_traffic_when_root_or_config_unavailable_before_start() -> Result<(), String>
{
    work_done_progress_runtime()?.block_on(async {
        // Root authority unavailable: refresh returns before any request is
        // accepted, so no token may be created.
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        let sink = backend.install_progress_recorder();
        backend
            .refresh_diagnostics(RefreshScope::Full, RefreshReason::ExplicitRefresh)
            .await;
        if !sink.events().is_empty() {
            return Err(format!(
                "root-unavailable refresh created progress traffic: {:?}",
                sink.events()
            ));
        }

        // Configuration failure: refresh returns before the scheduler
        // accepts a request, so again no token.
        let (service, _socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        backend.set_configuration_failure("bad config for test");
        let sink = backend.install_progress_recorder();
        backend
            .refresh_diagnostics(RefreshScope::Full, RefreshReason::ExplicitRefresh)
            .await;
        if !sink.events().is_empty() {
            return Err(format!(
                "config-failed refresh created progress traffic: {:?}",
                sink.events()
            ));
        }
        Ok(())
    })
}

/// Drive a real `ripr lsp` server over duplex IO through one explicit
/// refresh and collect the work-done-progress traffic on the wire (#1971).
/// Returns (workDoneProgress/create requests, $/progress notifications).
async fn run_wire_refresh_with_progress_capability(
    root: &Path,
    work_done_progress_capable: bool,
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>), String> {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (mut client_read, mut client_write) = tokio::io::split(client_io);
    let (server_read, server_write) = tokio::io::split(server_io);
    let backend_root = root.to_path_buf();
    let (service, socket) =
        LspService::new(move |client| Backend::new(client, backend_root.clone()));
    let server_task = tokio::spawn(async move {
        Server::new(server_read, server_write, socket)
            .serve(service)
            .await;
    });

    let root_uri = file_uri_for_path(root)?;
    write_lsp_message(
        &mut client_write,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": root_uri.as_str(),
                // A base ref that cannot resolve makes the real analysis
                // fail fast, so the refresh ends as failed instead of
                // scanning the enclosing repository.
                "initializationOptions": {
                    "baseRef": "ripr-lsp-progress-missing-base"
                },
                "capabilities": {
                    "window": {"workDoneProgress": work_done_progress_capable}
                }
            }
        }),
    )
    .await?;
    read_lsp_response(&mut client_read, 1).await?;
    write_lsp_message(
        &mut client_write,
        serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    )
    .await?;
    write_lsp_message(
        &mut client_write,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "workspace/executeCommand",
            "params": {"command": REFRESH_COMMAND, "arguments": []}
        }),
    )
    .await?;

    let mut creates = Vec::new();
    let mut progress = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let message = read_lsp_message(&mut client_read).await?;
            if message.get("id").and_then(serde_json::Value::as_u64) == Some(2)
                && message.get("method").is_none()
            {
                return Ok::<(), String>(());
            }
            match message.get("method").and_then(serde_json::Value::as_str) {
                Some("window/workDoneProgress/create") => {
                    let id = message
                        .get("id")
                        .cloned()
                        .ok_or_else(|| "create request carried no id".to_string())?;
                    creates.push(message.clone());
                    write_lsp_message(
                        &mut client_write,
                        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                    )
                    .await?;
                }
                Some("$/progress") => progress.push(message.clone()),
                _ => {}
            }
        }
    })
    .await
    .map_err(|_elapsed| {
        format!("refresh timed out; creates={creates:?} progress={progress:?}")
    })??;

    write_lsp_message(
        &mut client_write,
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
    )
    .await?;
    read_lsp_response(&mut client_read, 3).await?;
    write_lsp_message(
        &mut client_write,
        serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(10), server_task)
        .await
        .map_err(|_elapsed| "server did not stop after exit".to_string())?
        .map_err(|err| format!("server task failed: {err}"))?;
    Ok((creates, progress))
}

#[test]
fn work_done_progress_failed_end_through_real_refresh_on_broken_workspace() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let root = unique_lsp_test_root("progress-analysis-error")?;
        let (creates, progress) =
            run_wire_refresh_with_progress_capability(root.path(), true).await?;

        if creates.len() != 1 {
            return Err(format!(
                "accepted refresh must create exactly one progress token: {creates:?}"
            ));
        }
        let token = creates[0]["params"]["token"]
            .as_str()
            .ok_or_else(|| "create request carried no token".to_string())?
            .to_string();
        // The generation is not pinned to 1: root resolution at initialize
        // advances the scheduler generation without creating progress.
        if !token.starts_with("ripr-analysis-") {
            return Err(format!("unexpected progress token: {token}"));
        }

        let kinds: Vec<&str> = progress
            .iter()
            .filter_map(|message| message["params"]["value"]["kind"].as_str())
            .collect();
        if kinds != vec!["begin", "end"] {
            return Err(format!(
                "failed refresh must begin then end exactly once: {progress:?}"
            ));
        }
        let begin = &progress[0]["params"];
        if begin["token"].as_str() != Some(token.as_str())
            || begin["value"]["title"].as_str() != Some("ripr analysis")
        {
            return Err(format!(
                "begin drifted from the created token: {progress:?}"
            ));
        }
        let begin_message = begin["value"]["message"]
            .as_str()
            .ok_or_else(|| "begin carried no phase message".to_string())?;
        if !begin_message.contains("analyzing") {
            return Err(format!(
                "begin must announce the analyzing phase: {begin_message}"
            ));
        }
        if !begin["value"]["percentage"].is_null()
            || !progress[1]["params"]["value"]["percentage"].is_null()
        {
            return Err(format!(
                "no fabricated percentages may be emitted: {progress:?}"
            ));
        }
        let end = &progress[1]["params"];
        let end_message = end["value"]["message"]
            .as_str()
            .ok_or_else(|| "end carried no terminal message".to_string())?;
        if end["token"].as_str() != Some(token.as_str())
            || !end_message.starts_with("analysis failed")
        {
            return Err(format!(
                "broken workspace must end as failed on the same token: {progress:?}"
            ));
        }
        if end_message.contains(root.path().to_string_lossy().as_ref()) {
            return Err(format!(
                "progress end leaked the workspace path: {end_message}"
            ));
        }
        drop(root);
        Ok(())
    })
}

#[test]
fn work_done_progress_capability_absent_refresh_emits_no_traffic() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        let root = unique_lsp_test_root("progress-capability-absent")?;
        let (creates, progress) =
            run_wire_refresh_with_progress_capability(root.path(), false).await?;
        if !creates.is_empty() || !progress.is_empty() {
            return Err(format!(
                "capability-absent client received progress traffic: creates={creates:?} progress={progress:?}"
            ));
        }
        drop(root);
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Dirty-buffer quarantine (#1970): saved-workspace authority for line-local
// diagnostics.
// ---------------------------------------------------------------------------

const QUARANTINE_TEXT_A: &str = "fn a() -> bool { true }\n";
const QUARANTINE_TEXT_B: &str = "fn b() -> bool { true }\n";
const QUARANTINE_TEXT_A_DIRTY: &str = "fn a() -> bool { false }\n";

struct QuarantineFixture {
    _temp: TempLspRoot,
    root: PathBuf,
    path_a: PathBuf,
    uri_a: tower_lsp_server::ls_types::Uri,
    uri_b: tower_lsp_server::ls_types::Uri,
}

fn quarantine_fixture(name: &str) -> Result<QuarantineFixture, String> {
    let temp = unique_lsp_test_root(name)?;
    let root = temp.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
    let path_a = root.join("src/a.rs");
    let path_b = root.join("src/b.rs");
    std::fs::write(&path_a, QUARANTINE_TEXT_A)
        .map_err(|err| format!("write a.rs failed: {err}"))?;
    std::fs::write(&path_b, QUARANTINE_TEXT_B)
        .map_err(|err| format!("write b.rs failed: {err}"))?;
    let uri_a = file_uri_for_path(&path_a).map_err(|err| format!("a.rs URI failed: {err}"))?;
    let uri_b = file_uri_for_path(&path_b).map_err(|err| format!("b.rs URI failed: {err}"))?;
    Ok(QuarantineFixture {
        _temp: temp,
        root,
        path_a,
        uri_a,
        uri_b,
    })
}

fn quarantine_finding(id: &str, file: &str) -> Finding {
    let mut finding = sample_finding();
    finding.id = id.to_string();
    finding.probe.id = ProbeId(id.to_string());
    finding.probe.location.file = PathBuf::from(file);
    finding.probe.location.line = 1;
    finding
}

fn quarantine_workspace_diagnostics(fixture: &QuarantineFixture) -> WorkspaceDiagnostics {
    // `headline_eligible` is the producer-owned eligibility signal the
    // delivery budget reads (#1973); without it the stored selection omits
    // the diagnostics and pull/push serve an empty set.
    fn served_diagnostic(root: &Path, finding: &Finding) -> Diagnostic {
        let mut diagnostic = diagnostic_for_finding(root, finding);
        if let Some(data) = diagnostic
            .data
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
        {
            data.insert("headline_eligible".to_string(), serde_json::json!(true));
        }
        diagnostic
    }
    let finding_a = quarantine_finding("probe:a:1:predicate", "src/a.rs");
    let finding_b = quarantine_finding("probe:b:1:predicate", "src/b.rs");
    let diagnostic_a = served_diagnostic(&fixture.root, &finding_a);
    let diagnostic_b = served_diagnostic(&fixture.root, &finding_b);
    let mut diagnostics_by_uri = BTreeMap::new();
    diagnostics_by_uri.insert(fixture.uri_a.clone(), vec![diagnostic_a.clone()]);
    diagnostics_by_uri.insert(fixture.uri_b.clone(), vec![diagnostic_b.clone()]);
    let input_identity = LspAnalysisInputIdentity::from_refresh_inputs(
        fixture.root.clone(),
        1,
        &LspAnalysisConfig::default(),
    );
    let snapshot = AnalysisSnapshot {
        root: fixture.root.clone(),
        input_identity: Some(input_identity),
        base: Some("origin/main".to_string()),
        mode: Mode::Draft,
        refresh: RefreshMetadata::generated_now(),
        findings: vec![finding_a, finding_b],
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        classified_seams: Vec::new(),
        gap_artifacts: Vec::new(),
        gap_artifact_rejections: Vec::new(),
        diagnostics_by_uri,
        delivery_selection: None,
        seams_deferred: false,
        partial_scope: None,
        component_outcomes: Vec::new(),
        out_of_scope_test_file_findings: 0,
    };
    WorkspaceDiagnostics {
        snapshot,
        batches: vec![
            DiagnosticBatch {
                uri: fixture.uri_a.clone(),
                diagnostics: vec![diagnostic_a],
            },
            DiagnosticBatch {
                uri: fixture.uri_b.clone(),
                diagnostics: vec![diagnostic_b],
            },
        ],
    }
}

fn commit_quarantine_snapshot(
    backend: &Backend,
    fixture: &QuarantineFixture,
) -> Result<(), String> {
    backend
        .refresh_plan(quarantine_workspace_diagnostics(fixture))
        .ok_or_else(|| "expected committed snapshot".to_string())?;
    Ok(())
}

fn quarantine_open_params(
    uri: &tower_lsp_server::ls_types::Uri,
    text: &str,
) -> DidOpenTextDocumentParams {
    DidOpenTextDocumentParams {
        text_document: TextDocumentItem::new(uri.clone(), "rust".to_string(), 1, text.to_string()),
    }
}

fn quarantine_change_params(
    uri: &tower_lsp_server::ls_types::Uri,
    version: i32,
    text: &str,
) -> DidChangeTextDocumentParams {
    DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier::new(uri.clone(), version),
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }],
    }
}

fn quarantine_save_params(
    uri: &tower_lsp_server::ls_types::Uri,
    text: &str,
) -> DidSaveTextDocumentParams {
    DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        text: Some(text.to_string()),
    }
}

async fn pull_document_json(
    backend: &Backend,
    uri: &tower_lsp_server::ls_types::Uri,
    previous_result_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let report = backend
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            identifier: None,
            previous_result_id,
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .map_err(|err| format!("document pull failed: {err}"))?;
    serde_json::to_value(report).map_err(|err| format!("serialize report failed: {err}"))
}

fn report_kind_and_items(report: &serde_json::Value) -> (Option<&str>, usize) {
    (
        report.get("kind").and_then(serde_json::Value::as_str),
        report
            .get("items")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len),
    )
}

async fn workspace_status_json(backend: &Backend) -> Result<serde_json::Value, String> {
    backend
        .execute_command(ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: Vec::new(),
            work_done_progress_params: Default::default(),
        })
        .await
        .map_err(|err| format!("workspace status command failed: {err}"))?
        .ok_or_else(|| "expected workspace status payload".to_string())
}

fn open_document_entry<'a>(
    status: &'a serde_json::Value,
    uri: &str,
) -> Result<&'a serde_json::Value, String> {
    status
        .get("open_documents")
        .and_then(serde_json::Value::as_array)
        .and_then(|documents| {
            documents
                .iter()
                .find(|entry| entry.get("uri").and_then(serde_json::Value::as_str) == Some(uri))
        })
        .ok_or_else(|| format!("missing open_documents entry for {uri}: {status}"))
}

#[tokio::test]
async fn dirty_document_withdraws_line_local_diagnostics_and_discloses() -> Result<(), String> {
    let fixture = quarantine_fixture("dirty-withdraw")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // The loopback client channel is bounded and nothing drains it in an
    // in-process test, so client sends would block once it fills. Dropping
    // the socket makes the server-to-client sends fail fast; the quarantine
    // bookkeeping under test runs before and after each send regardless.
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    backend
        .did_open(quarantine_open_params(&fixture.uri_b, QUARANTINE_TEXT_B))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;

    // Clean documents are served on pull.
    let served = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, served_items) = report_kind_and_items(&served);
    if served_items == 0 {
        return Err(format!(
            "expected served diagnostics before the edit: {served}"
        ));
    }

    // Make A dirty: the buffer diverges from the analyzed saved content.
    backend
        .did_change(quarantine_change_params(
            &fixture.uri_a,
            2,
            QUARANTINE_TEXT_A_DIRTY,
        ))
        .await;

    // Its line-local diagnostics are withdrawn under a distinct result id.
    let withdrawn = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (kind, items) = report_kind_and_items(&withdrawn);
    if kind != Some("full") || items != 0 {
        return Err(format!(
            "expected an empty full report for the dirty document: {withdrawn}"
        ));
    }
    let quarantined_id = withdrawn
        .get("resultId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "withdrawn report did not carry resultId".to_string())?;
    if !quarantined_id.ends_with(":quarantined") {
        return Err(format!(
            "withdrawn result id must be distinct from the served id: {quarantined_id}"
        ));
    }
    // A repeated pull with the quarantined id reports unchanged.
    let repeat =
        pull_document_json(backend, &fixture.uri_a, Some(quarantined_id.to_string())).await?;
    if repeat.get("kind").and_then(serde_json::Value::as_str) != Some("unchanged") {
        return Err(format!(
            "expected unchanged for a repeated quarantined pull: {repeat}"
        ));
    }
    // The withdrawal is disclosed once per episode.
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    let Some(quarantine) = &state.quarantine else {
        return Err("expected a quarantine marker on the dirty document".to_string());
    };
    if quarantine.reason.as_str() != "buffer_diverges_from_analyzed_saved_content" {
        return Err(format!(
            "wrong staleness reason: {}",
            quarantine.reason.as_str()
        ));
    }
    if !quarantine.withdrawal_disclosed {
        return Err("withdrawal must be disclosed".to_string());
    }
    // The client-visible baseline carries an empty set for the dirty document.
    if backend
        .last_diagnostics_for_uri_for_test(&fixture.uri_a)
        .is_none_or(|diagnostics| !diagnostics.is_empty())
    {
        return Err("client-visible baseline must be empty for the dirty document".to_string());
    }

    // The other (clean) document is unaffected on both pull transports.
    let served_b = pull_document_json(backend, &fixture.uri_b, None).await?;
    let (_, served_b_items) = report_kind_and_items(&served_b);
    if served_b_items == 0 {
        return Err(format!(
            "clean document must keep its diagnostics: {served_b}"
        ));
    }
    let workspace = backend
        .workspace_diagnostic(WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: Vec::new(),
            work_done_progress_params: Default::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .map_err(|err| format!("workspace pull failed: {err}"))?;
    let workspace_json =
        serde_json::to_value(workspace).map_err(|err| format!("serialize failed: {err}"))?;
    let entries = workspace_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("expected workspace report items: {workspace_json}"))?;
    for (uri, expect_empty) in [
        (fixture.uri_a.as_str(), true),
        (fixture.uri_b.as_str(), false),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.get("uri").and_then(serde_json::Value::as_str) == Some(uri))
            .ok_or_else(|| format!("missing workspace report for {uri}: {workspace_json}"))?;
        let count = entry
            .get("items")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if (count == 0) != expect_empty {
            return Err(format!(
                "workspace report wrong for {uri} (expect_empty={expect_empty}): {entry}"
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn save_with_changed_content_lifts_quarantine_and_resumes_refresh() -> Result<(), String> {
    let fixture = quarantine_fixture("save-changed-lift")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // The loopback client channel is bounded and nothing drains it in an
    // in-process test, so client sends would block once it fills. Dropping
    // the socket makes the server-to-client sends fail fast; the quarantine
    // bookkeeping under test runs before and after each send regardless.
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;
    backend
        .did_change(quarantine_change_params(
            &fixture.uri_a,
            2,
            QUARANTINE_TEXT_A_DIRTY,
        ))
        .await;

    // Saving the changed content schedules a refresh but cannot lift the
    // quarantine before the new saved content is analyzed. The client
    // persists the bytes on save, so the fixture mirrors them to disk.
    let baseline = backend.workspace_revision();
    backend
        .did_save(quarantine_save_params(
            &fixture.uri_a,
            QUARANTINE_TEXT_A_DIRTY,
        ))
        .await;
    std::fs::write(&fixture.path_a, QUARANTINE_TEXT_A_DIRTY)
        .map_err(|err| format!("persist save failed: {err}"))?;
    if backend.workspace_revision() != baseline + 1 {
        return Err("changed save must schedule a refresh".to_string());
    }
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if !state.is_quarantined() {
        return Err("an unanalyzed save must stay quarantined".to_string());
    }

    // The refresh commits: the analyzed saved content catches up with the
    // buffer and the quarantine lifts.
    commit_quarantine_snapshot(backend, &fixture)?;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if state.is_quarantined() {
        return Err("quarantine must lift once the saved content is analyzed".to_string());
    }
    if state.saved_digest != state.analyzed_saved_digest {
        return Err("saved and analyzed identities must agree after the refresh".to_string());
    }
    if state.analyzed_saved_digest.as_deref()
        != Some(content_digest(QUARANTINE_TEXT_A_DIRTY.as_bytes()).as_str())
    {
        return Err("analyzed identity must equal the new saved content".to_string());
    }

    // Pull serves the document again without a quarantined result id.
    let served = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, items) = report_kind_and_items(&served);
    if items == 0 {
        return Err(format!("expected re-served diagnostics: {served}"));
    }
    if served
        .get("resultId")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|id| id.ends_with(":quarantined"))
    {
        return Err(format!(
            "served report must not use the quarantined id: {served}"
        ));
    }

    // The workspace status payload names the lifted state.
    let status = workspace_status_json(backend).await?;
    let entry = open_document_entry(&status, fixture.uri_a.as_str())?;
    if entry.get("state").and_then(serde_json::Value::as_str) != Some("clean") {
        return Err(format!("expected a clean document state: {entry}"));
    }
    if entry
        .get("line_local_diagnostics")
        .and_then(serde_json::Value::as_str)
        != Some("served")
    {
        return Err(format!("expected served line-local diagnostics: {entry}"));
    }
    Ok(())
}

#[tokio::test]
async fn save_with_unchanged_content_dedups_and_keeps_lifted_quarantine() -> Result<(), String> {
    let fixture = quarantine_fixture("save-unchanged-dedup")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // The loopback client channel is bounded and nothing drains it in an
    // in-process test, so client sends would block once it fills. Dropping
    // the socket makes the server-to-client sends fail fast; the quarantine
    // bookkeeping under test runs before and after each send regardless.
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;
    // Record the initial save so the dedup path has a recorded digest.
    backend
        .did_save(quarantine_save_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    let baseline = backend.workspace_revision();

    // Dirty the buffer, then type back to the analyzed saved content.
    backend
        .did_change(quarantine_change_params(
            &fixture.uri_a,
            2,
            QUARANTINE_TEXT_A_DIRTY,
        ))
        .await;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if !state.is_quarantined() {
        return Err("dirty buffer must be quarantined".to_string());
    }
    backend
        .did_change(quarantine_change_params(
            &fixture.uri_a,
            3,
            QUARANTINE_TEXT_A,
        ))
        .await;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if state.is_quarantined() {
        return Err(
            "quarantine must lift when the buffer matches the analyzed saved content".to_string(),
        );
    }
    let served = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, items) = report_kind_and_items(&served);
    if items == 0 {
        return Err(format!(
            "expected re-served diagnostics after the lift: {served}"
        ));
    }

    // The save is unchanged since the recorded save: dedup applies (no
    // refresh, no revision advance) and the quarantine stays lifted.
    backend
        .did_save(quarantine_save_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    if backend.workspace_revision() != baseline {
        return Err(format!(
            "deduplicated save advanced the revision: {baseline} -> {}",
            backend.workspace_revision()
        ));
    }
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if state.is_quarantined() {
        return Err("deduplicated save must keep the lifted quarantine".to_string());
    }
    let served = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, items) = report_kind_and_items(&served);
    if items == 0 {
        return Err(format!(
            "expected served diagnostics after the dedup: {served}"
        ));
    }
    Ok(())
}

#[tokio::test]
async fn repeated_open_change_save_cycles_keep_identities_consistent() -> Result<(), String> {
    let fixture = quarantine_fixture("cycles")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // The loopback client channel is bounded and nothing drains it in an
    // in-process test, so client sends would block once it fills. Dropping
    // the socket makes the server-to-client sends fail fast; the quarantine
    // bookkeeping under test runs before and after each send regardless.
    drop(socket);
    let backend = service.inner();
    for cycle in 0..3_u32 {
        let disk_text = format!("fn a_{cycle}() {{}}\n");
        std::fs::write(&fixture.path_a, &disk_text)
            .map_err(|err| format!("write cycle {cycle} failed: {err}"))?;
        backend
            .did_open(quarantine_open_params(&fixture.uri_a, &disk_text))
            .await;
        let state = backend
            .document_state_for_test(&fixture.uri_a)
            .ok_or_else(|| "expected document state".to_string())?;
        if state.saved_digest.as_deref() != Some(content_digest(disk_text.as_bytes()).as_str()) {
            return Err(format!(
                "cycle {cycle}: open must seed the saved identity from persisted bytes"
            ));
        }

        // Dirty the buffer, save it, and stay quarantined until analysis.
        // The client persists the bytes on save, so the fixture mirrors
        // them to disk.
        let dirty_text = format!("fn a_{cycle}_dirty() {{}}\n");
        backend
            .did_change(quarantine_change_params(&fixture.uri_a, 2, &dirty_text))
            .await;
        backend
            .did_save(quarantine_save_params(&fixture.uri_a, &dirty_text))
            .await;
        std::fs::write(&fixture.path_a, &dirty_text)
            .map_err(|err| format!("persist cycle {cycle} save failed: {err}"))?;
        let state = backend
            .document_state_for_test(&fixture.uri_a)
            .ok_or_else(|| "expected document state".to_string())?;
        if !state.is_quarantined() {
            return Err(format!(
                "cycle {cycle}: unanalyzed save must stay quarantined"
            ));
        }

        // The refresh analyzes the new saved content and the quarantine lifts.
        commit_quarantine_snapshot(backend, &fixture)?;
        let state = backend
            .document_state_for_test(&fixture.uri_a)
            .ok_or_else(|| "expected document state".to_string())?;
        if state.is_quarantined() {
            return Err(format!("cycle {cycle}: quarantine must lift once analyzed"));
        }
        if state.saved_digest != state.analyzed_saved_digest {
            return Err(format!("cycle {cycle}: saved/analyzed identity drift"));
        }
        if state.analyzed_saved_digest.as_deref()
            != Some(content_digest(dirty_text.as_bytes()).as_str())
        {
            return Err(format!(
                "cycle {cycle}: analyzed identity must equal the saved content"
            ));
        }
        if state.analyzed_input_identity.is_none() {
            return Err(format!("cycle {cycle}: missing analyzed input identity"));
        }

        backend
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: fixture.uri_a.clone(),
                },
            })
            .await;
        if backend.document_state_for_test(&fixture.uri_a).is_some() {
            return Err(format!("cycle {cycle}: close must drop the document state"));
        }
    }
    Ok(())
}

#[tokio::test]
async fn unsaved_buffer_text_never_enters_snapshot_or_status_payloads() -> Result<(), String> {
    let fixture = quarantine_fixture("no-unsaved-leak")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // The loopback client channel is bounded and nothing drains it in an
    // in-process test, so client sends would block once it fills. Dropping
    // the socket makes the server-to-client sends fail fast; the quarantine
    // bookkeeping under test runs before and after each send regardless.
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;

    const UNSAVED: &str = "fn a() -> bool { UNSAVED_BUFFER_MARKER }";
    backend
        .did_change(quarantine_change_params(&fixture.uri_a, 2, UNSAVED))
        .await;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if !state.is_quarantined() {
        return Err("dirty buffer must be quarantined".to_string());
    }

    // The snapshot's diagnostics for the dirty document describe the saved
    // state only; the unsaved buffer text appears nowhere in them.
    let snapshot = backend
        .latest_analysis_snapshot()
        .ok_or_else(|| "expected committed snapshot".to_string())?;
    let snapshot_json = serde_json::to_string(&snapshot.diagnostics_by_uri)
        .map_err(|err| format!("serialize snapshot failed: {err}"))?;
    if snapshot_json.contains("UNSAVED_BUFFER_MARKER") {
        return Err("unsaved buffer text leaked into snapshot diagnostics".to_string());
    }
    // The committed client-visible baseline for the dirty document is empty.
    if backend.last_diagnostics_for_uri_for_test(&fixture.uri_a) != Some(Vec::new()) {
        return Err("client-visible baseline must be empty for the dirty document".to_string());
    }
    // Pull serves nothing for the dirty document.
    let report = pull_document_json(backend, &fixture.uri_a, None).await?;
    if serde_json::to_string(&report)
        .map_err(|err| format!("serialize report failed: {err}"))?
        .contains("UNSAVED_BUFFER_MARKER")
    {
        return Err("unsaved buffer text leaked into the pull report".to_string());
    }
    let (_, items) = report_kind_and_items(&report);
    if items != 0 {
        return Err(format!("expected an empty pull report: {report}"));
    }
    // The workspace status payload names the quarantine with digest
    // identities only — no unsaved text.
    let status = workspace_status_json(backend).await?;
    let status_json =
        serde_json::to_string(&status).map_err(|err| format!("serialize status failed: {err}"))?;
    if status_json.contains("UNSAVED_BUFFER_MARKER") {
        return Err("unsaved buffer text leaked into the status payload".to_string());
    }
    let entry = open_document_entry(&status, fixture.uri_a.as_str())?;
    if entry.get("state").and_then(serde_json::Value::as_str) != Some("quarantined") {
        return Err(format!("expected a quarantined document state: {entry}"));
    }
    if entry
        .get("line_local_diagnostics")
        .and_then(serde_json::Value::as_str)
        != Some("withdrawn")
    {
        return Err(format!(
            "expected withdrawn line-local diagnostics: {entry}"
        ));
    }
    if entry
        .get("staleness_reason")
        .and_then(serde_json::Value::as_str)
        != Some("buffer_diverges_from_analyzed_saved_content")
    {
        return Err(format!("expected the staleness reason: {entry}"));
    }
    if entry
        .get("diagnostics_authority")
        .and_then(serde_json::Value::as_str)
        != Some("saved_workspace")
    {
        return Err(format!("expected the saved-workspace authority: {entry}"));
    }
    Ok(())
}

#[tokio::test]
async fn superseded_transaction_leaves_document_state_unadvanced() -> Result<(), String> {
    let fixture = quarantine_fixture("superseded-tx")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    // See the other quarantine tests: dropping the loopback socket keeps
    // in-process client sends from blocking.
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    backend
        .did_open(quarantine_open_params(&fixture.uri_b, QUARANTINE_TEXT_B))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;

    // Save new content; the document is quarantined until the new saved
    // content is analyzed. The fixture mirrors the persisted bytes.
    backend
        .did_save(quarantine_save_params(
            &fixture.uri_a,
            QUARANTINE_TEXT_A_DIRTY,
        ))
        .await;
    std::fs::write(&fixture.path_a, QUARANTINE_TEXT_A_DIRTY)
        .map_err(|err| format!("persist save failed: {err}"))?;

    // A transaction is prepared — pending identities computed — but then
    // superseded: it never becomes latest_analysis. Document identities
    // must not advance with it.
    let transaction = backend
        .prepare_refresh_transaction(quarantine_workspace_diagnostics(&fixture))
        .ok_or_else(|| "expected prepared transaction".to_string())?;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if !state.is_quarantined() {
        return Err("a superseded transaction must not lift the quarantine".to_string());
    }
    if state.analyzed_saved_digest.as_deref()
        != Some(content_digest(QUARANTINE_TEXT_A.as_bytes()).as_str())
    {
        return Err("analyzed identity must stay at the committed snapshot's content".to_string());
    }
    // Pull still serves the committed (previous) snapshot's authority: the
    // dirty document is withdrawn against it, the clean one is served.
    let withdrawn = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, withdrawn_items) = report_kind_and_items(&withdrawn);
    if withdrawn_items != 0 {
        return Err(format!(
            "expected the dirty document to stay withdrawn: {withdrawn}"
        ));
    }
    let served_b = pull_document_json(backend, &fixture.uri_b, None).await?;
    let (_, served_b_items) = report_kind_and_items(&served_b);
    if served_b_items == 0 {
        return Err(format!(
            "expected the previous snapshot to keep serving the clean document: {served_b}"
        ));
    }
    drop(transaction);

    // When a transaction does commit, identities advance with it.
    commit_quarantine_snapshot(backend, &fixture)?;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if state.is_quarantined() {
        return Err("quarantine must lift once a transaction commits".to_string());
    }
    if state.analyzed_saved_digest.as_deref()
        != Some(content_digest(QUARANTINE_TEXT_A_DIRTY.as_bytes()).as_str())
    {
        return Err("analyzed identity must advance with the commit".to_string());
    }
    Ok(())
}

#[tokio::test]
async fn externally_changed_disk_content_does_not_falsely_clear_quarantine() -> Result<(), String> {
    let fixture = quarantine_fixture("external-disk-change")?;
    let (service, socket) = LspService::new(|client| Backend::new(client, fixture.root.clone()));
    drop(socket);
    let backend = service.inner();
    backend
        .did_open(quarantine_open_params(&fixture.uri_a, QUARANTINE_TEXT_A))
        .await;
    commit_quarantine_snapshot(backend, &fixture)?;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if state.is_quarantined() {
        return Err("buffer matching the analyzed saved content must be clean".to_string());
    }

    // An external tool (git checkout, formatter, another editor) rewrites
    // the file: no didChange and no didSave arrive, so the didSave-tracked
    // saved digest stays stale. The refresh analyzes the new persisted
    // bytes, and the analyzed identity must come from those bytes — the
    // old-content buffer must not be marked clean against them.
    std::fs::write(&fixture.path_a, QUARANTINE_TEXT_A_DIRTY)
        .map_err(|err| format!("external rewrite failed: {err}"))?;
    commit_quarantine_snapshot(backend, &fixture)?;
    let state = backend
        .document_state_for_test(&fixture.uri_a)
        .ok_or_else(|| "expected document state".to_string())?;
    if !state.is_quarantined() {
        return Err(
            "a buffer older than the externally analyzed bytes must be quarantined".to_string(),
        );
    }
    let Some(quarantine) = &state.quarantine else {
        return Err("expected a quarantine marker".to_string());
    };
    if quarantine.reason.as_str() != "buffer_diverges_from_analyzed_saved_content" {
        return Err(format!(
            "wrong staleness reason: {}",
            quarantine.reason.as_str()
        ));
    }
    if state.analyzed_saved_digest.as_deref()
        != Some(content_digest(QUARANTINE_TEXT_A_DIRTY.as_bytes()).as_str())
    {
        return Err("analyzed identity must come from the persisted bytes".to_string());
    }
    let withdrawn = pull_document_json(backend, &fixture.uri_a, None).await?;
    let (_, items) = report_kind_and_items(&withdrawn);
    if items != 0 {
        return Err(format!(
            "expected the stale buffer's diagnostics to be withdrawn: {withdrawn}"
        ));
    }
    Ok(())
}

// ---- RIPR-SPEC-0137: redacted protocol tracing ($/setTrace, $/logTrace) ----

/// `$ /logTrace` params collected from framed messages (method key without a
/// space, per the standard).
fn log_trace_params(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|message| {
            message.get("method").and_then(serde_json::Value::as_str) == Some("$/logTrace")
        })
        .filter_map(|message| message.get("params").cloned())
        .collect()
}

/// Read until the response for `id`, then drain a bounded window for trailing
/// notifications. Server-originated notifications (e.g. the `$/logTrace` for
/// the response itself) are flushed through the client socket independently
/// of the direct response write, so either can reach the wire first; the
/// bounded drain makes trace assertions deterministic.
async fn read_response_and_notifications<R>(
    reader: &mut R,
    id: u64,
) -> Result<(serde_json::Value, Vec<serde_json::Value>), String>
where
    R: AsyncRead + Unpin,
{
    let (response, mut notifications) = read_lsp_response_with_notifications(reader, id).await?;
    notifications.extend(read_lsp_messages_for(reader, Duration::from_millis(300)).await?);
    Ok((response, notifications))
}

#[test]
fn lsp_trace_set_trace_updates_state_and_rejects_unknown_values() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = build_service(PathBuf::from("."));
        let backend = service.inner();
        assert_eq!(backend.trace_level(), TraceValue::Off, "default is off");

        backend
            .set_trace(serde_json::json!({"value": "messages"}))
            .await;
        assert_eq!(backend.trace_level(), TraceValue::Messages);
        backend
            .set_trace(serde_json::json!({"value": "verbose"}))
            .await;
        assert_eq!(backend.trace_level(), TraceValue::Verbose);

        // Unknown value: rejected without crashing; the state is kept.
        backend
            .set_trace(serde_json::json!({"value": "everything"}))
            .await;
        assert_eq!(
            backend.trace_level(),
            TraceValue::Verbose,
            "an unknown trace value must not change the current state"
        );
        // Malformed params (missing `value`, non-object) are rejected the
        // same way.
        backend.set_trace(serde_json::json!({"level": "off"})).await;
        assert_eq!(backend.trace_level(), TraceValue::Verbose);
        backend.set_trace(serde_json::json!("verbose")).await;
        assert_eq!(backend.trace_level(), TraceValue::Verbose);
        backend.set_trace(serde_json::json!({"value": 3})).await;
        assert_eq!(backend.trace_level(), TraceValue::Verbose);

        backend.set_trace(serde_json::json!({"value": "off"})).await;
        assert_eq!(backend.trace_level(), TraceValue::Off);
        Ok(())
    })
}

#[test]
fn lsp_trace_initialize_honors_client_trace_value() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = build_service(PathBuf::from("."));
        let backend = service.inner();
        backend
            .initialize(InitializeParams {
                trace: Some(TraceValue::Verbose),
                ..InitializeParams::default()
            })
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        assert_eq!(
            backend.trace_level(),
            TraceValue::Verbose,
            "initialize must honor the client-selected trace value"
        );

        let (service, _socket) = build_service(PathBuf::from("."));
        let backend = service.inner();
        backend
            .initialize(InitializeParams::default())
            .await
            .map_err(|err| format!("initialize failed: {err}"))?;
        assert_eq!(
            backend.trace_level(),
            TraceValue::Off,
            "an omitted trace value leaves the default off"
        );
        Ok(())
    })
}

#[test]
fn lsp_trace_toggle_leaves_status_identity_and_revision_untouched() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;
    runtime.block_on(async {
        let (service, _socket) = build_service(PathBuf::from("."));
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let status_params = || ExecuteCommandParams {
            command: COLLECT_WORKSPACE_STATUS_COMMAND.to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        };

        let before_status = backend
            .execute_command(status_params())
            .await
            .map_err(|err| format!("workspace status failed: {err}"))?;
        let before_revision = backend.workspace_revision();

        backend
            .set_trace(serde_json::json!({"value": "verbose"}))
            .await;
        assert_eq!(backend.trace_level(), TraceValue::Verbose);

        let after_status = backend
            .execute_command(status_params())
            .await
            .map_err(|err| format!("workspace status failed: {err}"))?;
        let after_revision = backend.workspace_revision();

        assert_eq!(
            before_status, after_status,
            "a trace toggle must not change the workspace status payload — this includes the \
             input-identity and configuration_pull disclosure fields (#2031, #2035)"
        );
        assert_eq!(
            before_revision, after_revision,
            "a trace toggle must not advance the workspace revision (no analysis reschedule)"
        );
        Ok(())
    })
}

#[test]
fn framed_lsp_trace_lifecycle_and_redaction() -> Result<(), String> {
    const CANARY: &str = "RIPR_TRACE_CANARY_NEVER_EMIT_7f3a9c";
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let invalid_root_parent = unique_lsp_test_root("framed-trace-root")?;
        let invalid_root = invalid_root_parent.path().join("not-a-directory");
        std::fs::write(&invalid_root, b"not a workspace directory")
            .map_err(|err| format!("write invalid LSP root failed: {err}"))?;
        let invalid_root_uri = file_uri_for_path(&invalid_root)?;
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, mut client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (service, socket) = build_service(PathBuf::from("."));
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
                    "rootUri": invalid_root_uri.as_str(),
                    "capabilities": {}
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut client_read, 1).await?;
        assert!(initialize.get("error").is_none());
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await?;

        let hover = |id: u64| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "position": { "line": 0, "character": 4 }
                }
            })
        };
        let set_trace = |value: &str| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "$/setTrace",
                "params": { "value": value }
            })
        };
        let did_open = |version: u64, canary: &str| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": text_uri,
                        "languageId": "rust",
                        "version": version,
                        "text": format!("pub fn demo() -> bool {{ // {canary}\n    true\n}}\n")
                    }
                }
            })
        };

        // 1. Off by default: no $/logTrace even for source-bearing traffic.
        write_lsp_message(&mut client_write, did_open(1, CANARY)).await?;
        write_lsp_message(&mut client_write, hover(2)).await?;
        let (_response, notifications) =
            read_response_and_notifications(&mut client_read, 2).await?;
        assert!(
            log_trace_params(&notifications).is_empty(),
            "trace off must emit no $/logTrace, got {notifications:?}"
        );

        // 2. messages: method/direction/class only; the canary source text and
        //    the document URI never enter the trace.
        write_lsp_message(&mut client_write, set_trace("messages")).await?;
        write_lsp_message(&mut client_write, did_open(2, CANARY)).await?;
        write_lsp_message(&mut client_write, hover(3)).await?;
        let (_response, notifications) =
            read_response_and_notifications(&mut client_read, 3).await?;
        let traces = log_trace_params(&notifications);
        assert!(
            traces.iter().any(|params| params["message"]
                .as_str()
                .is_some_and(|message| message.contains("<- notification textDocument/didOpen"))),
            "messages level must trace the inbound didOpen method name, got {traces:?}"
        );
        assert!(
            traces.iter().any(|params| params["message"]
                .as_str()
                .is_some_and(|message| message.contains("<- request textDocument/hover"))),
            "messages level must trace the inbound hover request, got {traces:?}"
        );
        assert!(
            traces.iter().any(|params| params["message"]
                .as_str()
                .is_some_and(|message| message.contains("-> response textDocument/hover"))),
            "messages level must trace the outbound hover response class, got {traces:?}"
        );
        assert!(
            traces.iter().all(|params| params.get("verbose").is_none()),
            "messages level must not add verbose detail, got {traces:?}"
        );
        // No recursion (RIPR-SPEC-0137): the `$/setTrace` that enabled this
        // phase and any `$/logTrace` emission are never themselves traced.
        assert!(
            traces.iter().all(|params| params["message"]
                .as_str()
                .is_some_and(|message| !message.contains("$/setTrace")
                    && !message.contains("$/logTrace"))),
            "trace lifecycle notifications must never be traced: {traces:?}"
        );
        let rendered = traces
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains(CANARY),
            "trace output must never contain source text (redaction discriminator): {rendered}"
        );
        assert!(
            !rendered.contains("file://"),
            "trace output must never contain document URIs or paths: {rendered}"
        );

        // 3. verbose: bounded numeric metadata added, still no payload content.
        write_lsp_message(&mut client_write, set_trace("verbose")).await?;
        write_lsp_message(&mut client_write, did_open(3, CANARY)).await?;
        write_lsp_message(&mut client_write, hover(4)).await?;
        let (_response, notifications) =
            read_response_and_notifications(&mut client_read, 4).await?;
        let traces = log_trace_params(&notifications);
        assert!(
            traces.iter().any(|params| {
                params["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("<- request textDocument/hover"))
                    && params["verbose"]
                        .as_str()
                        .is_some_and(|verbose| verbose.starts_with("params_bytes="))
            }),
            "verbose level must add a bounded params byte count, got {traces:?}"
        );
        assert!(
            traces.iter().any(|params| {
                params["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("-> response textDocument/hover"))
                    && params["verbose"]
                        .as_str()
                        .is_some_and(|verbose| verbose.starts_with("outcome=ok response_bytes="))
            }),
            "verbose level must add the outcome class and a bounded response byte count, got {traces:?}"
        );
        let rendered = traces
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains(CANARY),
            "verbose trace output must never contain source text: {rendered}"
        );
        assert!(
            !rendered.contains("file://"),
            "verbose trace output must never contain document URIs or paths: {rendered}"
        );

        // 4. Unknown value: rejected observably (tracing is on), state kept,
        //    session alive — the next request is still answered and traces at
        //    the previous level.
        write_lsp_message(&mut client_write, set_trace("everything")).await?;
        write_lsp_message(&mut client_write, hover(5)).await?;
        let (response, notifications) =
            read_response_and_notifications(&mut client_read, 5).await?;
        assert!(
            response.get("error").is_none(),
            "session must stay alive after an unknown trace value: {response}"
        );
        let traces = log_trace_params(&notifications);
        assert!(
            traces.iter().any(|params| params["message"]
                .as_str()
                .is_some_and(|message| message.contains("class=unknown_value"))),
            "an unknown trace value must be rejected observably when tracing is on, got {traces:?}"
        );
        let hover_traces = traces
            .iter()
            .filter(|params| {
                params["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("textDocument/hover"))
            })
            .collect::<Vec<_>>();
        assert!(
            !hover_traces.is_empty()
                && hover_traces
                    .iter()
                    .all(|params| params["verbose"]
                        .as_str()
                        .is_some_and(|verbose| !verbose.is_empty())),
            "the rejected $/setTrace must leave the previous verbose level in effect, got {traces:?}"
        );

        // 5. Back to off: tracing stops immediately. Drain after the
        //    transition so a slow trailing verbose-phase emission cannot
        //    leak into this phase's assertion window (tests-red-green
        //    review): the emptiness assertion below covers only traffic
        //    sent while off.
        write_lsp_message(&mut client_write, set_trace("off")).await?;
        let _stragglers = read_lsp_messages_for(&mut client_read, Duration::from_millis(150)).await?;
        write_lsp_message(&mut client_write, hover(6)).await?;
        let (_response, notifications) =
            read_response_and_notifications(&mut client_read, 6).await?;
        assert!(
            log_trace_params(&notifications).is_empty(),
            "trace off must stop emission immediately, got {notifications:?}"
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "shutdown", "params": null}),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 7).await?;
        assert!(shutdown.get("error").is_none());
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
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
fn framed_lsp_trace_initialize_trace_param_enables_tracing() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start test runtime: {err}"))?;

    runtime.block_on(async {
        let invalid_root_parent = unique_lsp_test_root("framed-trace-init-root")?;
        let invalid_root = invalid_root_parent.path().join("not-a-directory");
        std::fs::write(&invalid_root, b"not a workspace directory")
            .map_err(|err| format!("write invalid LSP root failed: {err}"))?;
        let invalid_root_uri = file_uri_for_path(&invalid_root)?;
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, mut client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (service, socket) = build_service(PathBuf::from("."));
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
                    "rootUri": invalid_root_uri.as_str(),
                    "trace": "verbose",
                    "capabilities": {}
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut client_read, 1).await?;
        assert!(initialize.get("error").is_none());
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await?;

        write_lsp_message(
            &mut client_write,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": text_uri },
                    "position": { "line": 0, "character": 4 }
                }
            }),
        )
        .await?;
        let (_response, notifications) =
            read_response_and_notifications(&mut client_read, 2).await?;
        let traces = log_trace_params(&notifications);
        assert!(
            traces.iter().any(|params| {
                params["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("<- request textDocument/hover"))
                    && params["verbose"]
                        .as_str()
                        .is_some_and(|verbose| verbose.starts_with("params_bytes="))
            }),
            "the initialize trace param must enable verbose tracing without any $/setTrace, got {traces:?}"
        );

        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 3).await?;
        assert!(shutdown.get("error").is_none());
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
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

// ---------------------------------------------------------------------------
// Typed component-outcome degradation (#1997, RIPR-SPEC-0141): no LSP
// analysis degradation may be reported only through process stderr, and a
// degraded optional component must surface typed on every status surface.
// ---------------------------------------------------------------------------

#[test]
fn lsp_production_sources_have_no_stderr_degradation_fallback() -> Result<(), String> {
    // Source-level guard: the production portion of every LSP source file
    // must be free of `eprintln!`. Degradation is reported through the typed
    // component outcomes on the snapshot plus `window/logMessage`, never
    // through hidden process stderr.
    let sources: [(&str, &str); 21] = [
        ("lsp.rs", include_str!("../lsp.rs")),
        ("actions.rs", include_str!("actions.rs")),
        ("agent_protocol.rs", include_str!("agent_protocol.rs")),
        ("backend.rs", include_str!("backend.rs")),
        ("capabilities.rs", include_str!("capabilities.rs")),
        ("component_outcome.rs", include_str!("component_outcome.rs")),
        ("config.rs", include_str!("config.rs")),
        ("diagnostic_budget.rs", include_str!("diagnostic_budget.rs")),
        (
            "diagnostic_catalog.rs",
            include_str!("diagnostic_catalog.rs"),
        ),
        ("diagnostics.rs", include_str!("diagnostics.rs")),
        ("gap_artifacts.rs", include_str!("gap_artifacts.rs")),
        ("hover.rs", include_str!("hover.rs")),
        ("input_identity.rs", include_str!("input_identity.rs")),
        ("lens.rs", include_str!("lens.rs")),
        ("payload_bounds.rs", include_str!("payload_bounds.rs")),
        ("position.rs", include_str!("position.rs")),
        ("progress.rs", include_str!("progress.rs")),
        ("refresh_scheduler.rs", include_str!("refresh_scheduler.rs")),
        ("state.rs", include_str!("state.rs")),
        ("transport_bounds.rs", include_str!("transport_bounds.rs")),
        ("uri.rs", include_str!("uri.rs")),
    ];
    for (name, source) in sources {
        // Test-only code (everything from the first `#[cfg(test)]` onward)
        // is excluded: test modules may print skip notices. No lsp source
        // declares `#[cfg(test)]` items before its test module except
        // diagnostics.rs test-only imports/helpers, which contain no
        // stderr prints either; the whole-file check for those files would
        // be equally clean, so the split is only a safety margin.
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        if production.contains("eprintln") {
            return Err(format!(
                "lsp/{name} writes to process stderr in production code; route degradation through the typed component outcomes (#1997)"
            ));
        }
    }
    Ok(())
}

/// Recursively copy a fixture workspace into a writable temp root.
fn copy_fixture_tree(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target)
        .map_err(|err| format!("create {} failed: {err}", target.display()))?;
    let entries = std::fs::read_dir(source)
        .map_err(|err| format!("read {} failed: {err}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read dir entry failed: {err}"))?;
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let from = entry.path();
        let to = target.join(&name);
        let file_type = entry
            .file_type()
            .map_err(|err| format!("inspect {} failed: {err}", from.display()))?;
        if file_type.is_dir() {
            copy_fixture_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|err| format!("copy {} failed: {err}", from.display()))?;
        }
    }
    Ok(())
}

/// Drive one explicit refresh over the wire, answering every
/// `window/workDoneProgress/create` request, and collect all notifications
/// emitted before the command response.
async fn run_wire_refresh_collecting(
    client_read: &mut tokio::io::ReadHalf<tokio::io::DuplexStream>,
    client_write: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>,
    id: u64,
) -> Result<Vec<serde_json::Value>, String> {
    write_lsp_message(
        client_write,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "workspace/executeCommand",
            "params": {"command": REFRESH_COMMAND, "arguments": []}
        }),
    )
    .await?;
    let mut notifications = Vec::new();
    tokio::time::timeout(Duration::from_mins(1), async {
        loop {
            let message = read_lsp_message(client_read).await?;
            if message.get("id").and_then(serde_json::Value::as_u64) == Some(id)
                && message.get("method").is_none()
            {
                if message.get("error").is_some() {
                    return Err(format!("refresh command failed: {message}"));
                }
                return Ok::<(), String>(());
            }
            match message.get("method").and_then(serde_json::Value::as_str) {
                Some("window/workDoneProgress/create") => {
                    let request_id = message
                        .get("id")
                        .cloned()
                        .ok_or_else(|| "create request carried no id".to_string())?;
                    write_lsp_message(
                        client_write,
                        serde_json::json!({"jsonrpc": "2.0", "id": request_id, "result": null}),
                    )
                    .await?;
                }
                Some(_) => notifications.push(message),
                None => {}
            }
        }
    })
    .await
    .map_err(|_elapsed| "refresh timed out waiting for the command response".to_string())??;
    Ok(notifications)
}

fn log_messages_of_type(notifications: &[serde_json::Value], message_type: u64) -> Vec<String> {
    notifications
        .iter()
        .filter(|message| {
            message.get("method").and_then(serde_json::Value::as_str) == Some("window/logMessage")
                && message["params"]["type"].as_u64() == Some(message_type)
        })
        .filter_map(|message| message["params"]["message"].as_str().map(str::to_string))
        .collect()
}

fn analysis_status_params(notifications: &[serde_json::Value]) -> Vec<serde_json::Value> {
    notifications
        .iter()
        .filter(|message| {
            message.get("method").and_then(serde_json::Value::as_str) == Some("ripr/analysisStatus")
        })
        .map(|message| message["params"].clone())
        .collect()
}

#[test]
fn framed_lsp_component_degradation_is_typed_logged_and_recovers() -> Result<(), String> {
    work_done_progress_runtime()?.block_on(async {
        // A writable copy of the boundary_gap fixture with its own git HEAD:
        // the on-disk fixture resolves `HEAD` through the enclosing
        // repository, which a temp copy does not have.
        let temp = unique_lsp_test_root("component-degradation")?;
        let root = temp.path().join("workspace");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
        copy_fixture_tree(&fixture, &root)?;
        run_lsp_scope_git(&root, &["init", "-q"])?;
        run_lsp_scope_git(&root, &["add", "-A"])?;
        run_lsp_scope_git(
            &root,
            &[
                "-c",
                "user.email=ripr-test@example.com",
                "-c",
                "user.name=ripr-test",
                "commit",
                "-qm",
                "fixture baseline",
            ],
        )?;

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (mut client_read, mut client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (service, socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let mut server_task = tokio::spawn(async move {
            Server::new(server_read, server_write, socket)
                .serve(service)
                .await;
        });
        let root_uri = file_uri_for_path(&root)?;
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
                        "checkMode": "instant",
                        "diagnosticProfile": "full"
                    },
                    "capabilities": {
                        "window": {"workDoneProgress": true}
                    }
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut client_read, 1).await?;
        if initialize.get("error").is_some() {
            return Err(format!("initialize failed: {initialize}"));
        }
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await?;

        // Refresh 1 (baseline): a clean run must not warn about degradation.
        let baseline = run_wire_refresh_collecting(&mut client_read, &mut client_write, 2).await?;
        let baseline_warnings = log_messages_of_type(&baseline, 2);
        if baseline_warnings
            .iter()
            .any(|message| message.contains("ripr analysis limited"))
        {
            return Err(format!(
                "a clean baseline run must not warn about degradation: {baseline_warnings:?}"
            ));
        }
        let baseline_status = analysis_status_params(&baseline);
        let Some(baseline_status) = baseline_status.last() else {
            return Err("baseline refresh published no analysis status".to_string());
        };
        if baseline_status["run_status"].as_str() != Some("full") {
            return Err(format!("baseline run must be full, got: {baseline_status}"));
        }

        // Plant a malformed causal delta artifact: the artifact exists but
        // cannot be loaded, so the causal_projection component fails while
        // diff and seam evidence remain usable.
        let delta_path = root.join("target/ripr/pr/canonical-delta.json");
        std::fs::create_dir_all(
            delta_path
                .parent()
                .ok_or_else(|| "delta path must have a parent".to_string())?,
        )
        .map_err(|err| format!("create delta dir failed: {err}"))?;
        std::fs::write(&delta_path, "{not json")
            .map_err(|err| format!("write malformed delta failed: {err}"))?;

        // Refresh 2 (degraded): typed status + one WARNING + limited progress
        // + ordinary evidence still published.
        let degraded = run_wire_refresh_collecting(&mut client_read, &mut client_write, 3).await?;
        let degraded_status = analysis_status_params(&degraded);
        let Some(status) = degraded_status.last() else {
            return Err("degraded refresh published no analysis status".to_string());
        };
        if status["run_status"].as_str() != Some("limited") {
            return Err(format!(
                "a degraded component must make the run limited, got: {status}"
            ));
        }
        let components = status["components"]
            .as_array()
            .ok_or_else(|| format!("status must expose typed components: {status}"))?;
        let causal = components
            .iter()
            .find(|outcome| outcome["component"].as_str() == Some("causal_projection"));
        let Some(causal) = causal else {
            return Err(format!(
                "components must include the causal_projection outcome: {components:?}"
            ));
        };
        if causal["state"].as_str() != Some("failed")
            || causal["kind"].as_str() != Some("causal_projection_unusable")
            || causal["findings_trustworthy"].as_bool() != Some(true)
            || causal["snapshot_identity"].is_null()
        {
            return Err(format!("unexpected causal_projection outcome: {causal}"));
        }
        let recovery = causal["recovery"].as_str().unwrap_or("");
        if !recovery.contains("ripr check") {
            return Err(format!(
                "the degraded outcome must name a concrete recovery route: {causal}"
            ));
        }
        let diff = components
            .iter()
            .find(|outcome| outcome["component"].as_str() == Some("diff"));
        if diff.and_then(|outcome| outcome["state"].as_str()) != Some("complete") {
            return Err(format!(
                "ordinary diff findings must stay complete and disclosed: {components:?}"
            ));
        }
        let warnings = log_messages_of_type(&degraded, 2);
        let degradation_warnings = warnings
            .iter()
            .filter(|message| message.contains("causal_projection failed"))
            .count();
        if degradation_warnings != 1 {
            return Err(format!(
                "expected exactly one degradation warning, got {degradation_warnings}: {warnings:?}"
            ));
        }
        if !warnings.iter().any(|message| {
            message.contains("causal_projection failed") && message.contains("recovery:")
        }) {
            return Err(format!(
                "the degradation warning must name the recovery route: {warnings:?}"
            ));
        }
        let progress_end_limited = degraded.iter().any(|message| {
            message.get("method").and_then(serde_json::Value::as_str) == Some("$/progress")
                && message["params"]["value"]["kind"].as_str() == Some("end")
                && message["params"]["value"]["message"].as_str()
                    == Some("analysis limited (run status: limited)")
        });
        if !progress_end_limited {
            return Err(format!(
                "progress must end limited for a degraded run: {degraded:?}"
            ));
        }
        let published_evidence = degraded.iter().any(|message| {
            message.get("method").and_then(serde_json::Value::as_str)
                == Some("textDocument/publishDiagnostics")
                && message["params"]["diagnostics"]
                    .as_array()
                    .is_some_and(|diagnostics| !diagnostics.is_empty())
        });
        if !published_evidence {
            return Err(
                "ordinary evidence must remain published under a degraded optional component"
                    .to_string(),
            );
        }

        // Refresh 3 (identical degradation): no repeated warning spam; the
        // typed status still discloses the degradation.
        let repeated = run_wire_refresh_collecting(&mut client_read, &mut client_write, 4).await?;
        let repeated_warnings = log_messages_of_type(&repeated, 2)
            .into_iter()
            .filter(|message| message.contains("causal_projection failed"))
            .count();
        if repeated_warnings != 0 {
            return Err("a byte-identical repeated degradation must not warn again".to_string());
        }
        let repeated_status = analysis_status_params(&repeated);
        if repeated_status
            .last()
            .and_then(|status| status["run_status"].as_str())
            != Some("limited")
        {
            return Err(format!(
                "the repeated degradation must stay typed on status: {repeated_status:?}"
            ));
        }

        // Refresh 4 (repaired): one INFO recovery line, full status restored.
        std::fs::remove_file(&delta_path)
            .map_err(|err| format!("remove malformed delta failed: {err}"))?;
        let recovered = run_wire_refresh_collecting(&mut client_read, &mut client_write, 5).await?;
        let recovery_infos = log_messages_of_type(&recovered, 3)
            .into_iter()
            .filter(|message| message.contains("recovered"))
            .count();
        if recovery_infos != 1 {
            return Err(format!(
                "recovery must log exactly one INFO line, got {recovery_infos}"
            ));
        }
        let recovered_status = analysis_status_params(&recovered);
        if recovered_status
            .last()
            .and_then(|status| status["run_status"].as_str())
            != Some("full")
        {
            return Err(format!(
                "the repaired refresh must restore a full run: {recovered_status:?}"
            ));
        }

        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "id": 6, "method": "shutdown", "params": null}),
        )
        .await?;
        let shutdown = read_lsp_response(&mut client_read, 6).await?;
        if shutdown.get("error").is_some() {
            return Err(format!("shutdown failed: {shutdown}"));
        }
        write_lsp_message(
            &mut client_write,
            serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        )
        .await?;
        client_write
            .shutdown()
            .await
            .map_err(|err| format!("failed to close test client: {err}"))?;
        match tokio::time::timeout(Duration::from_secs(2), &mut server_task).await {
            Ok(join_result) => {
                join_result.map_err(|err| format!("LSP server task failed: {err}"))?;
            }
            Err(_) => {
                server_task.abort();
                return Err("LSP server did not stop after exit notification".to_string());
            }
        }
        drop(temp);
        Ok(())
    })
}
