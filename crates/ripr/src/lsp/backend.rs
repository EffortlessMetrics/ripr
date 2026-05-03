use super::REFRESH_COMMAND;
use super::actions::code_action_response;
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, DiagnosticRefreshPlan, WorkspaceDiagnostics, diagnostic_refresh_plan,
    take_all_uris, workspace_diagnostics_with_config,
};
use super::hover::{diagnostic_at_position, diagnostic_hover_response, hover_response};
use super::state::{AnalysisSnapshot, DocumentStore};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
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
        let diagnostics = match tokio::task::spawn_blocking(move || {
            workspace_diagnostics_with_config(&root, &config)
        })
        .await
        {
            Ok(Ok(diagnostics)) => diagnostics,
            Ok(Err(err)) => {
                self.report_refresh_failure(err).await;
                return;
            }
            Err(err) => {
                self.report_refresh_failure(format!("analysis task failed: {err}"))
                    .await;
                return;
            }
        };
        if !self.is_current_refresh_generation(generation) {
            return;
        }
        let Some(refresh) = self.refresh_plan(diagnostics) else {
            return;
        };
        for batch in refresh.publish_batches {
            self.client
                .publish_diagnostics(batch.uri, batch.diagnostics, None)
                .await;
        }
        for uri in refresh.clear_uris {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    pub(super) async fn report_refresh_failure(&self, message: String) {
        self.client
            .log_message(
                MessageType::WARNING,
                format!("ripr analysis refresh failed: {message}"),
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
            && let Some(diagnostic) = diagnostic_at_position(diagnostics, position)
            && snapshot.finding_for_diagnostic(diagnostic).is_some()
        {
            return Some(diagnostic_hover_response(diagnostic));
        }

        let Ok(last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        let diagnostics = last_diagnostics.get(uri)?;
        diagnostic_at_position(diagnostics, position).map(diagnostic_hover_response)
    }
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
        self.set_root(root_from_initialize_params(&params, &fallback_root));
        self.set_analysis_config(LspAnalysisConfig::from_initialize_params(&params));
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
        Ok(Some(code_action_response(&params)))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command == REFRESH_COMMAND {
            self.refresh_diagnostics().await;
        }
        Ok(None)
    }
}
