use super::actions::code_action_response;
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, DiagnosticRefreshPlan, WorkspaceDiagnostics, diagnostic_refresh_plan,
    take_all_uris, workspace_diagnostics_with_config,
};
use super::hover::{
    classified_seam_hover_response, diagnostic_at_position, diagnostic_covers_position,
    diagnostic_hover_response, finding_hover_response, hover_response, hover_with_snapshot_status,
};
use super::state::{AnalysisSnapshot, DocumentStore, format_duration};
use super::{COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, REFRESH_COMMAND};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::domain::context_packet::ContextPacket;
use crate::domain::{StageEvidence, StageState};
use crate::output::agent_seam_packets::{
    render_agent_gap_record_packet_json, suggested_assertion_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam,
};
use crate::output::gap_decision_ledger::{
    DEFAULT_GAP_DECISION_LEDGER_OUT, GapRecord, parse_gap_records_json,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex as AsyncMutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionParams, CodeActionResponse, Diagnostic, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    ExecuteCommandParams, Hover, HoverParams, InitializeParams, InitializeResult, LSPAny,
    MessageType, Uri,
};
use tower_lsp_server::{Client, LanguageServer};

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    documents: Mutex<DocumentStore>,
    analysis_config: Mutex<LspAnalysisConfig>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    last_diagnostics: Mutex<BTreeMap<Uri, Vec<Diagnostic>>>,
    latest_analysis: Mutex<Option<AnalysisSnapshot>>,
    refresh_generation: Mutex<u64>,
    refresh_in_flight: AsyncMutex<()>,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root: Mutex::new(root),
            documents: Mutex::new(DocumentStore::default()),
            analysis_config: Mutex::new(LspAnalysisConfig::default()),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            last_diagnostics: Mutex::new(BTreeMap::new()),
            latest_analysis: Mutex::new(None),
            refresh_generation: Mutex::new(0),
            refresh_in_flight: AsyncMutex::new(()),
        }
    }

    pub(super) async fn refresh_diagnostics(&self) {
        let Some(generation) = self.next_refresh_generation() else {
            return;
        };
        self.log_refresh_queued(generation).await;
        let _refresh_guard = self.refresh_in_flight.lock().await;
        if !self.is_current_refresh_generation(generation) {
            return;
        }
        let Some(root) = self.root() else {
            return;
        };
        let Some(config) = self.analysis_config() else {
            return;
        };
        let enabled_languages = config.repo_config().languages().enabled().to_vec();
        let started = Instant::now();
        self.log_refresh_started(generation).await;
        let diagnostics = match tokio::task::spawn_blocking(move || {
            workspace_diagnostics_with_config(&root, &config)
        })
        .await
        {
            Ok(Ok(mut diagnostics)) => {
                diagnostics
                    .snapshot
                    .refresh
                    .record_duration(started.elapsed());
                diagnostics
            }
            Ok(Err(err)) => {
                self.report_refresh_failure_after(err, started.elapsed())
                    .await;
                return;
            }
            Err(err) => {
                self.report_refresh_failure_after(
                    format!("analysis task failed: {err}"),
                    started.elapsed(),
                )
                .await;
                return;
            }
        };
        if !self.is_current_refresh_generation(generation) {
            return;
        }
        let summary = RefreshLogSummary::from_snapshot(generation, &diagnostics.snapshot)
            .with_enabled_languages(&enabled_languages);
        let Some(refresh) = self.refresh_plan(diagnostics) else {
            self.report_refresh_failure_after(
                "diagnostic snapshot was inconsistent with publish batches".to_string(),
                started.elapsed(),
            )
            .await;
            return;
        };
        let published_uri_count = refresh.publish_batches.len();
        let cleared_uri_count = refresh.clear_uris.len();
        for batch in refresh.publish_batches {
            self.client
                .publish_diagnostics(batch.uri, batch.diagnostics, None)
                .await;
        }
        for uri in refresh.clear_uris {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
        self.log_refresh_completed(summary, published_uri_count, cleared_uri_count)
            .await;
    }

    pub(super) async fn report_refresh_failure_after(&self, message: String, duration: Duration) {
        self.client
            .log_message(
                MessageType::WARNING,
                refresh_failed_log_message(&message, duration),
            )
            .await;
        for uri in self.clear_all_diagnostic_uris() {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    pub(super) fn refresh_plan(
        &self,
        diagnostics: WorkspaceDiagnostics,
    ) -> Option<DiagnosticRefreshPlan> {
        let WorkspaceDiagnostics { snapshot, batches } = diagnostics;
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return None;
        };
        let Ok(mut last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        let Ok(mut latest_analysis) = self.latest_analysis.lock() else {
            return None;
        };
        if snapshot.diagnostics_by_uri != diagnostics_by_uri_from_batches(&batches) {
            return None;
        }
        let refresh = diagnostic_refresh_plan(&last_diagnostic_uris, batches);
        debug_assert!(snapshot.is_consistent());
        *last_diagnostics = refresh
            .publish_batches
            .iter()
            .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
            .collect();
        *last_diagnostic_uris = refresh.current_uris.clone();
        *latest_analysis = Some(snapshot);
        Some(refresh)
    }

    pub(super) fn clear_all_diagnostic_uris(&self) -> Vec<Uri> {
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return Vec::new();
        };
        if let Ok(mut last_diagnostics) = self.last_diagnostics.lock() {
            last_diagnostics.clear();
        }
        if let Ok(mut latest_analysis) = self.latest_analysis.lock() {
            *latest_analysis = None;
        }
        take_all_uris(&mut last_diagnostic_uris)
    }

    pub(super) fn next_refresh_generation(&self) -> Option<u64> {
        let Ok(mut generation) = self.refresh_generation.lock() else {
            return None;
        };
        *generation = generation.saturating_add(1);
        Some(*generation)
    }

    pub(super) fn is_current_refresh_generation(&self, generation: u64) -> bool {
        let Ok(current) = self.refresh_generation.lock() else {
            return false;
        };
        *current == generation
    }

    fn root(&self) -> Option<PathBuf> {
        let Ok(root) = self.root.lock() else {
            return None;
        };
        Some(root.clone())
    }

    pub(super) fn analysis_config(&self) -> Option<LspAnalysisConfig> {
        let Ok(config) = self.analysis_config.lock() else {
            return None;
        };
        Some(config.clone())
    }

    #[cfg(test)]
    pub(super) fn latest_analysis_snapshot(&self) -> Option<AnalysisSnapshot> {
        let Ok(snapshot) = self.latest_analysis.lock() else {
            return None;
        };
        snapshot.clone()
    }

    fn set_root(&self, root: PathBuf) {
        let Ok(mut current_root) = self.root.lock() else {
            return;
        };
        *current_root = root;
    }

    fn set_analysis_config(&self, config: LspAnalysisConfig) {
        let Ok(mut current_config) = self.analysis_config.lock() else {
            return;
        };
        *current_config = config;
    }

    fn open_document(&self, params: DidOpenTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.open(params);
    }

    fn change_document(&self, params: DidChangeTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.change(params);
    }

    fn close_document(&self, params: DidCloseTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.close(params);
    }

    pub(super) fn hover_for_position(&self, params: &HoverParams) -> Option<Hover> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;
        if let Ok(snapshot) = self.latest_analysis.lock()
            && let Some(snapshot) = snapshot.as_ref()
            && let Some(diagnostics) = snapshot.diagnostics_for_uri(uri)
        {
            // Walk every diagnostic that covers the cursor, not just
            // the first. When seamDiagnostics is enabled a Finding
            // diagnostic can overlap a seam diagnostic on the same
            // line, and findings are pushed before seams in the
            // diagnostic batch — first-match scanning would silently
            // shadow the new seam-evidence hover. Prefer the
            // seam-bearing diagnostic, then the finding-bearing one.
            // Caught by chatgpt-codex on PR #242.
            let overlapping: Vec<&Diagnostic> = diagnostics
                .iter()
                .filter(|d| diagnostic_covers_position(d, position))
                .collect();
            for diagnostic in &overlapping {
                if let Some(seam) = snapshot.classified_seam_for_diagnostic(diagnostic) {
                    return Some(hover_with_snapshot_status(
                        classified_seam_hover_response(seam, diagnostic, Some(snapshot)),
                        snapshot,
                    ));
                }
            }
            for diagnostic in &overlapping {
                if let Some(finding) = snapshot.finding_for_diagnostic(diagnostic) {
                    return Some(hover_with_snapshot_status(
                        finding_hover_response(finding, diagnostic),
                        snapshot,
                    ));
                }
            }
            if let Some(diagnostic) = overlapping.first() {
                return Some(hover_with_snapshot_status(
                    diagnostic_hover_response(diagnostic),
                    snapshot,
                ));
            }
        }

        let Ok(last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        let diagnostics = last_diagnostics.get(uri)?;
        diagnostic_at_position(diagnostics, position).map(diagnostic_hover_response)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RefreshLogSummary {
    generation: u64,
    duration: Duration,
    diagnostics: usize,
    files: usize,
    findings: usize,
    preview_findings: usize,
    static_limits: usize,
    seam_diagnostics: usize,
    gap_artifacts: usize,
    actionable_gap_artifacts: usize,
    preview_gap_artifacts: usize,
    no_action_gap_artifacts: usize,
    gap_static_limits: usize,
    gap_artifact_rejections: usize,
    gap_artifact_rejection_kinds: Vec<&'static str>,
    enabled_languages: usize,
    enabled_language_names: Vec<&'static str>,
}

impl RefreshLogSummary {
    pub(super) fn from_snapshot(generation: u64, snapshot: &AnalysisSnapshot) -> Self {
        let duration = match snapshot.refresh.duration {
            Some(duration) => duration,
            None => Duration::ZERO,
        };
        Self {
            generation,
            duration,
            diagnostics: snapshot.diagnostic_count(),
            files: snapshot.diagnostic_uri_count(),
            findings: snapshot.finding_count(),
            preview_findings: snapshot
                .findings
                .iter()
                .filter(|finding| {
                    finding
                        .language_status
                        .as_ref()
                        .is_some_and(|status| status.as_str() == "preview")
                })
                .count(),
            static_limits: snapshot
                .findings
                .iter()
                .filter(|finding| finding.static_limit_kind.is_some())
                .count(),
            seam_diagnostics: snapshot.seam_diagnostic_count(),
            gap_artifacts: snapshot.gap_artifacts.len(),
            actionable_gap_artifacts: snapshot
                .gap_artifacts
                .iter()
                .filter(|artifact| artifact.is_actionable_gap())
                .count(),
            preview_gap_artifacts: snapshot
                .gap_artifacts
                .iter()
                .filter(|artifact| artifact.is_preview())
                .count(),
            no_action_gap_artifacts: snapshot
                .gap_artifacts
                .iter()
                .filter(|artifact| artifact.is_no_action_gap())
                .count(),
            gap_static_limits: snapshot
                .gap_artifacts
                .iter()
                .filter(|artifact| artifact.has_static_limit())
                .count(),
            gap_artifact_rejections: snapshot.gap_artifact_rejections.len(),
            gap_artifact_rejection_kinds: snapshot
                .gap_artifact_rejections
                .iter()
                .map(|rejection| rejection.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            enabled_languages: 1,
            enabled_language_names: vec!["rust"],
        }
    }

    pub(super) fn with_enabled_languages(
        mut self,
        enabled_languages: &[crate::domain::LanguageId],
    ) -> Self {
        self.enabled_languages = enabled_languages.len();
        self.enabled_language_names = enabled_languages
            .iter()
            .map(crate::domain::LanguageId::as_str)
            .collect();
        self
    }
}

impl Backend {
    async fn log_refresh_queued(&self, generation: u64) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("ripr analysis refresh queued: generation={generation}"),
            )
            .await;
    }

    async fn log_refresh_started(&self, generation: u64) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("ripr analysis refresh started: generation={generation}"),
            )
            .await;
    }

    async fn log_refresh_completed(
        &self,
        summary: RefreshLogSummary,
        published_uri_count: usize,
        cleared_uri_count: usize,
    ) {
        self.client
            .log_message(
                MessageType::INFO,
                refresh_completed_log_message(&summary, published_uri_count, cleared_uri_count),
            )
            .await;
    }
}

pub(super) fn refresh_completed_log_message(
    summary: &RefreshLogSummary,
    published_uri_count: usize,
    cleared_uri_count: usize,
) -> String {
    let duration = format_duration(summary.duration);
    format!(
        "ripr analysis refresh completed in {duration}: generation={}, diagnostics={}, files={}, findings={}, preview_findings={}, static_limits={}, seam_diagnostics={}, gap_artifacts={}, actionable_gap_artifacts={}, preview_gap_artifacts={}, no_action_gap_artifacts={}, gap_static_limits={}, gap_artifact_rejections={}, gap_artifact_rejection_kinds={}, enabled_languages={}, enabled_language_names={}, published_files={}, cleared_files={}",
        summary.generation,
        summary.diagnostics,
        summary.files,
        summary.findings,
        summary.preview_findings,
        summary.static_limits,
        summary.seam_diagnostics,
        summary.gap_artifacts,
        summary.actionable_gap_artifacts,
        summary.preview_gap_artifacts,
        summary.no_action_gap_artifacts,
        summary.gap_static_limits,
        summary.gap_artifact_rejections,
        summary.gap_artifact_rejection_kinds.join("|"),
        summary.enabled_languages,
        summary.enabled_language_names.join("|"),
        published_uri_count,
        cleared_uri_count
    )
}

pub(super) fn refresh_failed_log_message(message: &str, duration: Duration) -> String {
    format!(
        "ripr analysis refresh failed after {}: {message}",
        format_duration(duration)
    )
}

fn diagnostics_by_uri_from_batches(batches: &[DiagnosticBatch]) -> BTreeMap<Uri, Vec<Diagnostic>> {
    batches
        .iter()
        .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
        .collect()
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let fallback_root = self
            .root()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let root = root_from_initialize_params(&params, &fallback_root);
        let repo_config = match crate::config::load_for_root(&root) {
            Ok(config) => config,
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("ripr config load failed; using defaults: {err}"),
                    )
                    .await;
                crate::config::RiprConfig::default()
            }
        };
        self.set_root(root);
        self.set_analysis_config(LspAnalysisConfig::from_initialize_params(
            &params,
            repo_config,
        ));
        Ok(initialize_result())
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.open_document(params);
        self.refresh_diagnostics().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.change_document(params);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.close_document(params);
        self.refresh_diagnostics().await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.refresh_diagnostics().await;
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        Ok(Some(
            self.hover_for_position(&params)
                .unwrap_or_else(hover_response),
        ))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let snapshot = self
            .latest_analysis
            .lock()
            .ok()
            .and_then(|value| value.clone());
        Ok(Some(code_action_response(&params, snapshot.as_ref())))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command == REFRESH_COMMAND {
            self.refresh_diagnostics().await;
            return Ok(None);
        }
        if params.command == COLLECT_CONTEXT_COMMAND {
            return Ok(self.collect_context_packet(&params.arguments));
        }
        if params.command == COLLECT_EVIDENCE_CONTEXT_COMMAND {
            return Ok(self.collect_evidence_context_packet(&params.arguments));
        }
        Ok(None)
    }
}

fn context_arguments(arguments: &[LSPAny]) -> Option<&serde_json::Map<String, serde_json::Value>> {
    let first = arguments.first()?;
    first.as_object()
}

impl Backend {
    fn collect_context_packet(&self, arguments: &[LSPAny]) -> Option<LSPAny> {
        let args = context_arguments(arguments)?;
        let snapshot = self.latest_analysis.lock().ok()?.clone()?;
        if let Some(gap_id) = args.get("gap_id").and_then(|v| v.as_str()) {
            return collect_gap_record_context_packet(&snapshot.root, args, gap_id);
        }
        if let Some(seam_id) = args.get("seam_id").and_then(|v| v.as_str()) {
            let seam = snapshot.classified_seam_by_id(seam_id)?;
            let packet = crate::output::agent_seam_packets::render_agent_seam_packets_json(
                std::slice::from_ref(seam),
            );
            return serde_json::from_str(&packet).ok();
        }
        let finding_id = args.get("finding_id").and_then(|v| v.as_str())?;
        let finding = snapshot.finding_by_id(finding_id)?;
        let max_related_tests = self
            .analysis_config()
            .map(|config| config.repo_config().reports().max_related_tests())
            .unwrap_or(crate::config::DEFAULT_CONTEXT_RELATED_TESTS);
        let stop_reasons = finding
            .effective_stop_reasons()
            .iter()
            .map(|reason| reason.as_str().to_string())
            .collect();
        let packet = ContextPacket::from_finding(finding, max_related_tests, stop_reasons);
        let rendered = crate::output::json::render_context_packet_dto(&packet);
        serde_json::from_str(&rendered).ok()
    }

    fn collect_evidence_context_packet(&self, arguments: &[LSPAny]) -> Option<LSPAny> {
        let args = context_arguments(arguments)?;
        let snapshot = self.latest_analysis.lock().ok()?.clone()?;
        let seam_id = args.get("seam_id").and_then(|v| v.as_str())?;
        let seam = snapshot.classified_seam_by_id(seam_id)?;
        Some(evidence_context_packet(&snapshot, seam))
    }
}

fn collect_gap_record_context_packet(
    root: &Path,
    args: &serde_json::Map<String, serde_json::Value>,
    gap_id: &str,
) -> Option<LSPAny> {
    let gap_id = gap_id.trim();
    if gap_id.is_empty() {
        return None;
    }
    let ledger_arg = args
        .get("gap_ledger")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_GAP_DECISION_LEDGER_OUT);
    let ledger_path = absolute_context_path(root, Path::new(ledger_arg));
    let contents = fs::read_to_string(&ledger_path).ok()?;
    let records = parse_gap_records_json(&contents).ok()?;
    let record = records
        .iter()
        .find(|record| gap_record_matches(record, gap_id))?;
    let rendered =
        render_agent_gap_record_packet_json(&display_lsp_path(&ledger_path), record).ok()?;
    serde_json::from_str(&rendered).ok()
}

fn absolute_context_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn gap_record_matches(record: &GapRecord, gap_id: &str) -> bool {
    record.gap_id == gap_id || record.canonical_gap_id == gap_id
}

fn evidence_context_packet(snapshot: &AnalysisSnapshot, entry: &ClassifiedSeam) -> LSPAny {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let seam_id = seam.id().as_str();
    let outline = targeted_test_brief_outline_for_classified_seam(entry);
    let related_test = evidence.related_tests.first();
    let missing_discriminator = evidence
        .missing_discriminators
        .first()
        .map(|missing| missing.value.as_str());
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "root": ".",
        "base": snapshot.base.as_deref(),
        "mode": snapshot.mode.as_str(),
        "seam_id": seam_id,
        "file": display_lsp_path(seam.file()),
        "range": {
            "start": seam.display_line(),
            "end": seam.display_line(),
        },
        "class": entry.class.as_str(),
        "seam_kind": seam.kind().as_str(),
        "owner": seam.owner(),
        "expression": seam.expression(),
        "required_discriminator": seam.required_discriminator().as_str(),
        "expected_sink": seam.expected_sink().as_str(),
        "evidence_path": {
            "reach": evidence_stage_status(&evidence.reach),
            "activate": evidence_stage_status(&evidence.activate),
            "propagate": evidence_stage_status(&evidence.propagate),
            "observe": evidence_stage_status(&evidence.observe),
            "discriminate": evidence_stage_status(&evidence.discriminate),
        },
        "evidence_summaries": {
            "reach": evidence.reach.summary.as_str(),
            "activate": evidence.activate.summary.as_str(),
            "propagate": evidence.propagate.summary.as_str(),
            "observe": evidence.observe.summary.as_str(),
            "discriminate": evidence.discriminate.summary.as_str(),
        },
        "missing_discriminator": missing_discriminator,
        "missing_discriminators": evidence.missing_discriminators.iter().map(|missing| {
            serde_json::json!({
                "value": missing.value.as_str(),
                "reason": missing.reason.as_str(),
            })
        }).collect::<Vec<_>>(),
        "related_test": related_test.map(|test| {
            format!("{}::{}", display_lsp_path(&test.file), test.test_name)
        }),
        "related_test_location": related_test.map(|test| {
            serde_json::json!({
                "file": display_lsp_path(&test.file),
                "line": test.line,
                "test_name": test.test_name.as_str(),
                "oracle_kind": test.oracle_kind.as_str(),
                "oracle_strength": test.oracle_strength.as_str(),
            })
        }),
        "suggested_assertion": suggested_assertion_for_classified_seam(entry),
        "suggested_test": {
            "file": outline.suggested_file,
            "name": outline.suggested_name,
            "candidate_value": outline.candidate_value,
            "assertion_shape": outline.assertion_shape,
        },
        "agent_packet_command": loop_commands::agent_packet_command(
            ".",
            seam_id,
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        ),
        "agent_brief_command": loop_commands::agent_brief_command(
            ".",
            seam_id,
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        ),
        "after_snapshot_command": loop_commands::check_repo_exposure_command_with_base(
            ".",
            snapshot.base.as_deref(),
            snapshot.mode.as_str(),
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        ),
        "verify_command": loop_commands::agent_verify_command(
            ".",
            loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
        ),
        "receipt_command": loop_commands::agent_receipt_command(
            ".",
            loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
            seam_id,
            Some(loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
        ),
        "limits_note": "Static evidence only; no runtime mutation execution.",
    })
}

fn evidence_stage_status(evidence: &StageEvidence) -> &'static str {
    match evidence.state {
        StageState::Yes => "present",
        StageState::Weak => "weak",
        StageState::No => "missing",
        StageState::Unknown => "unknown",
        StageState::Opaque => "opaque",
        StageState::NotApplicable => "not_applicable",
    }
}

fn display_lsp_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod gap_record_context_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn collect_context_packet_for_gap_id_reads_explicit_ledger() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .ok_or_else(|| "expected gap packet".to_string())?;

        assert_eq!(packet["source"], "gap_decision_ledger");
        let gap_packet = &packet["packets"][0];
        assert_eq!(gap_packet["gap_id"], "gap:pr:pricing:threshold-boundary");
        assert_eq!(
            gap_packet["repair_route"]["route_kind"],
            "AddBoundaryAssertion"
        );
        assert_eq!(
            gap_packet["verification_commands"][0],
            "cargo xtask fixtures boundary_gap"
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_context_packet_for_gap_id_matches_canonical_gap_id() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:rust:pricing:threshold-boundary",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:rust:pricing:threshold-boundary")
                .ok_or_else(|| "expected gap packet".to_string())?;

        assert_eq!(
            packet["packets"][0]["gap_id"],
            "gap:pr:pricing:threshold-boundary"
        );
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_context_packet_for_gap_id_uses_default_ledger_path() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .ok_or_else(|| "expected gap packet".to_string())?;

        assert_eq!(packet["source"], "gap_decision_ledger");
        assert_eq!(
            packet["packets"][0]["repair_card"]["source_artifact"],
            display_lsp_path(&root.join(DEFAULT_GAP_DECISION_LEDGER_OUT))
        );
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    fn temp_root() -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-lsp-gap-record-context-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("target/ripr/reports"))
            .map_err(|err| format!("create temp root {} failed: {err}", root.display()))?;
        Ok(root)
    }

    fn write_gap_ledger(root: &Path) -> Result<(), String> {
        let path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        fs::write(path, gap_ledger_json())
            .map_err(|err| format!("write gap ledger in {} failed: {err}", root.display()))
    }

    fn gap_ledger_json() -> &'static str {
        r#"{
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "static_exposure",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "target_line": 33,
        "related_test": "tests/pricing.rs::discount_threshold",
        "assertion_shape": "assert_eq!(price(threshold), expected)",
        "changed_behavior": "amount >= threshold",
        "stop_conditions": ["Stop if the target owner moved."]
      },
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "owner": "pricing::discounted_total",
        "dedupe_fingerprint": "gap:rust:pricing:threshold-boundary"
      },
      "evidence_ids": ["evidence:pricing"],
      "projection_eligibility": {
        "agent_packet": { "eligible": true, "reason": "bounded_repair_route" }
      },
      "verification_commands": ["cargo xtask fixtures boundary_gap"],
      "authority_boundary": "advisory"
    }
  ]
}"#
    }

    // -- coverage-gap tests --
    //
    // These pin the previously-uncovered None branches of
    // `collect_gap_record_context_packet`, `context_arguments`,
    // `gap_record_matches`, and the missing match arms of
    // `evidence_stage_status`. Region coverage on this file was 85.35%
    // before; the branches below are pure-function paths with no
    // production-code change.

    #[test]
    fn collect_gap_record_context_packet_with_blank_gap_id_returns_none() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "   ",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(collect_gap_record_context_packet(&root, args, "   ").is_none());

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_with_missing_ledger_file_returns_none()
    -> Result<(), String> {
        let root = temp_root()?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .is_none()
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_with_malformed_ledger_returns_none() -> Result<(), String>
    {
        let root = temp_root()?;
        let path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        fs::write(path, "{ not valid json")
            .map_err(|err| format!("write malformed ledger in {} failed: {err}", root.display()))?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .is_none()
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_with_unknown_gap_id_returns_none() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:unknown:missing",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(collect_gap_record_context_packet(&root, args, "gap:pr:unknown:missing").is_none());

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_with_blank_ledger_arg_falls_back_to_default()
    -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "gap_ledger": "   ",
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .ok_or_else(|| "expected gap packet".to_string())?;
        assert_eq!(
            packet["packets"][0]["repair_card"]["source_artifact"],
            display_lsp_path(&root.join(DEFAULT_GAP_DECISION_LEDGER_OUT))
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn context_arguments_returns_none_for_empty_argument_list() {
        assert!(context_arguments(&[]).is_none());
    }

    #[test]
    fn context_arguments_returns_none_when_first_argument_is_not_an_object() {
        let arg = serde_json::Value::String("not-an-object".to_string());
        assert!(context_arguments(std::slice::from_ref(&arg)).is_none());
    }

    #[test]
    fn gap_record_matches_compares_pr_local_and_canonical_ids() -> Result<(), String> {
        let records = parse_gap_records_json(gap_ledger_json())
            .map_err(|err| format!("parse fixture ledger failed: {err}"))?;
        let record = records
            .first()
            .ok_or_else(|| "expected fixture to contain one record".to_string())?;

        assert!(gap_record_matches(
            record,
            "gap:pr:pricing:threshold-boundary"
        ));
        assert!(gap_record_matches(
            record,
            "gap:rust:pricing:threshold-boundary"
        ));
        assert!(!gap_record_matches(record, "gap:pr:other:missing"));
        Ok(())
    }

    #[test]
    fn absolute_context_path_keeps_absolute_paths_and_joins_relative_paths() {
        let root = Path::new("/workspace/root");
        let already_absolute = if cfg!(windows) {
            PathBuf::from(r"C:\already\absolute.json")
        } else {
            PathBuf::from("/already/absolute.json")
        };

        assert_eq!(
            absolute_context_path(root, &already_absolute),
            already_absolute
        );
        assert_eq!(
            absolute_context_path(root, Path::new("nested/file.json")),
            root.join("nested/file.json")
        );
    }

    #[test]
    fn evidence_stage_status_maps_every_stage_state_variant() {
        use crate::domain::Confidence;

        let cases = [
            (StageState::Yes, "present"),
            (StageState::Weak, "weak"),
            (StageState::No, "missing"),
            (StageState::Unknown, "unknown"),
            (StageState::Opaque, "opaque"),
            (StageState::NotApplicable, "not_applicable"),
        ];
        for (state, expected) in cases {
            let label = format!("{state:?}");
            let evidence = StageEvidence::new(state, Confidence::Medium, "");
            assert_eq!(
                evidence_stage_status(&evidence),
                expected,
                "unexpected status for stage state {label}"
            );
        }
    }

    #[test]
    fn display_lsp_path_normalizes_backslashes_to_forward_slashes() {
        let path = std::path::PathBuf::from(r"a\b\c.rs");
        assert_eq!(display_lsp_path(&path), "a/b/c.rs");
    }

    #[test]
    fn refresh_failed_log_message_formats_actionable_duration_and_message() {
        let formatted =
            refresh_failed_log_message("analysis crashed", std::time::Duration::from_millis(420));
        assert!(
            formatted.starts_with("ripr analysis refresh failed after "),
            "missing prefix in {formatted}"
        );
        assert!(
            formatted.ends_with(": analysis crashed"),
            "missing message suffix in {formatted}"
        );
    }
}
