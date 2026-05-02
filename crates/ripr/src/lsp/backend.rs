use super::REFRESH_COMMAND;
use super::actions::code_action_response;
use super::capabilities::{initialize_result, root_from_initialize_params};
use super::diagnostics::{
    DiagnosticBatch, DiagnosticRefreshPlan, diagnostic_refresh_plan, take_all_uris,
    workspace_diagnostic_batches,
};
use super::hover::hover_response;
use super::state::DocumentStore;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, ExecuteCommandParams, Hover, HoverParams,
    InitializeParams, InitializeResult, LSPAny, MessageType, Uri,
};
use tower_lsp_server::{Client, LanguageServer};

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    documents: Mutex<DocumentStore>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root: Mutex::new(root),
            documents: Mutex::new(DocumentStore::default()),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
        }
    }

    async fn refresh_diagnostics(&self) {
        let Some(root) = self.root() else {
            return;
        };
        let batches =
            match tokio::task::spawn_blocking(move || workspace_diagnostic_batches(&root)).await {
                Ok(Ok(batches)) => batches,
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
        let Some(refresh) = self.refresh_plan(batches) else {
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
        batches: Vec<DiagnosticBatch>,
    ) -> Option<DiagnosticRefreshPlan> {
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return None;
        };
        let refresh = diagnostic_refresh_plan(&last_diagnostic_uris, batches);
        *last_diagnostic_uris = refresh.current_uris.clone();
        Some(refresh)
    }

    pub(super) fn clear_all_diagnostic_uris(&self) -> Vec<Uri> {
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return Vec::new();
        };
        take_all_uris(&mut last_diagnostic_uris)
    }

    fn root(&self) -> Option<PathBuf> {
        let Ok(root) = self.root.lock() else {
            return None;
        };
        Some(root.clone())
    }

    fn set_root(&self, root: PathBuf) {
        let Ok(mut current_root) = self.root.lock() else {
            return;
        };
        *current_root = root;
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
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let fallback_root = self
            .root()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        self.set_root(root_from_initialize_params(&params, &fallback_root));
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

    async fn hover(&self, _: HoverParams) -> LspResult<Option<Hover>> {
        Ok(Some(hover_response()))
    }

    async fn code_action(
        &self,
        _: tower_lsp_server::ls_types::CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        Ok(Some(code_action_response()))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command == REFRESH_COMMAND {
            self.refresh_diagnostics().await;
        }
        Ok(None)
    }
}
