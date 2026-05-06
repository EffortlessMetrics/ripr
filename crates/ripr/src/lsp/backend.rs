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
use super::{COLLECT_CONTEXT_COMMAND, REFRESH_COMMAND};
use crate::domain::context_packet::ContextPacket;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
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
        let summary = RefreshLogSummary::from_snapshot(generation, &diagnostics.snapshot);
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
                        classified_seam_hover_response(seam, diagnostic),
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
    seam_diagnostics: usize,
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
            seam_diagnostics: snapshot.seam_diagnostic_count(),
        }
    }
}

impl Backend {
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
        "ripr analysis refresh completed in {duration}: generation={}, diagnostics={}, files={}, findings={}, seam_diagnostics={}, published_files={}, cleared_files={}",
        summary.generation,
        summary.diagnostics,
        summary.files,
        summary.findings,
        summary.seam_diagnostics,
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
}
