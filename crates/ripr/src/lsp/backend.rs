use super::AnalysisStatusNotification;
use super::actions::code_action_response;
use super::capabilities::{
    WorkspaceRootResolution, client_supports_diagnostic_refresh, client_supports_pull_diagnostics,
    initialize_result_for_client, root_from_initialize_params,
};
use super::config::LspAnalysisConfig;
use super::diagnostics::{
    DiagnosticBatch, DiagnosticRefreshPlan, DiagnosticResultIdCache, WorkspaceDiagnostics,
    diagnostic_refresh_plan, take_all_uris,
};
use super::hover::{
    classified_seam_hover_response, diagnostic_at_position, diagnostic_covers_position,
    diagnostic_hover_response, finding_hover_response, hover_response, hover_with_snapshot_status,
};
use super::lens::code_lens_response;
use super::refresh_scheduler::{
    RefreshAttemptOutcome, RefreshDecision, RefreshReason, RefreshRequest, RefreshScheduler,
    RefreshScope,
};
use super::state::{
    AnalysisAttemptState, AnalysisFailure, AnalysisHealth, AnalysisSnapshot, DocumentStore,
    WorkspaceRootAuthority, WorkspaceRootState, format_duration,
};
use super::uri::{
    file_uri_for_path, file_uri_is_within_root, file_uris_match, path_from_file_uri,
    path_is_within_root,
};
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, REFRESH_COMMAND,
};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::analysis::cancellation::{AnalysisAbortKind, is_cancellation_error};
use crate::config::LspDiagnosticProfile;
use crate::domain::context_packet::ContextPacket;
use crate::domain::{StageEvidence, StageState};
use crate::output::agent_seam_packets::{
    render_agent_gap_record_packet_json, suggested_assertion_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam, validate_agent_gap_record_packet,
};
use crate::output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT;
use crate::output::gap_decision_ledger::{
    DEFAULT_GAP_DECISION_LEDGER_OUT, GapRecord, parse_gap_records_json,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, Diagnostic,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, ExecuteCommandParams, FileEvent, Hover, HoverParams,
    InitializeParams, InitializeResult, InitializedParams, LSPAny, MessageType, Registration,
    RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport,
    UnchangedDocumentDiagnosticReport, Uri, WorkspaceDiagnosticParams, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport, WorkspaceUnchangedDocumentDiagnosticReport,
};
use tower_lsp_server::{Client, LanguageServer};

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    workspace_root: Mutex<WorkspaceRootAuthority>,
    workspace_root_epoch: AtomicU64,
    workspace_root_transition: AsyncMutex<()>,
    documents: Mutex<DocumentStore>,
    analysis_config: Mutex<LspAnalysisConfig>,
    configuration_failure: Mutex<Option<AnalysisFailure>>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    last_diagnostics: Mutex<BTreeMap<Uri, Vec<Diagnostic>>>,
    latest_analysis: Mutex<Option<Arc<AnalysisSnapshot>>>,
    diagnostic_result_ids: Mutex<Option<Arc<DiagnosticResultIdCache>>>,
    analysis_health: Mutex<AnalysisHealth>,
    pull_diagnostics: Mutex<bool>,
    diagnostic_refresh_support: Mutex<bool>,
    dynamic_file_watch_registration: Mutex<bool>,
    refresh_scheduler: RefreshScheduler,
    workspace_revision: Mutex<u64>,
    refresh_idle: Notify,
}

pub(super) struct RefreshTransaction {
    pub(super) plan: DiagnosticRefreshPlan,
    pub(super) snapshot: AnalysisSnapshot,
    pub(super) previous_diagnostics: BTreeMap<Uri, Vec<Diagnostic>>,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root: Mutex::new(root.clone()),
            workspace_root: Mutex::new(WorkspaceRootAuthority::unavailable(
                "workspace root authority is awaiting initialization",
            )),
            workspace_root_epoch: AtomicU64::new(0),
            workspace_root_transition: AsyncMutex::new(()),
            documents: Mutex::new(DocumentStore::default()),
            analysis_config: Mutex::new(LspAnalysisConfig::default()),
            configuration_failure: Mutex::new(None),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            last_diagnostics: Mutex::new(BTreeMap::new()),
            latest_analysis: Mutex::new(None),
            diagnostic_result_ids: Mutex::new(None),
            analysis_health: Mutex::new(AnalysisHealth::default()),
            pull_diagnostics: Mutex::new(false),
            diagnostic_refresh_support: Mutex::new(false),
            dynamic_file_watch_registration: Mutex::new(false),
            refresh_scheduler: RefreshScheduler::default(),
            workspace_revision: Mutex::new(0),
            refresh_idle: Notify::new(),
        }
    }

    /// Run a diagnostic refresh.
    ///
    /// `defer_seam_inventory` controls whether the expensive full-repo seam
    /// inventory is included in this refresh:
    ///
    /// - `true` (interactive path: `did_open`/`did_save`/`did_close`): only
    ///   the fast diff-scoped check runs. The snapshot carries complete findings
    ///   and is marked `seams_deferred` (run_status = `"seams_deferred"`).
    ///   This typically completes in 33ms–11s instead of 336s cold.
    ///
    /// - `false` (explicit `ripr.refreshDiagnostics` command): the full seam
    ///   inventory also runs, transitioning the snapshot to `full` (or
    ///   `limited`/`stale`/`cache_limited` per existing rules) with seam
    ///   diagnostics present.
    ///
    /// See RIPR-SPEC-0105 for the design rationale.
    pub(super) async fn refresh_diagnostics(&self, scope: RefreshScope, reason: RefreshReason) {
        if self.configuration_failure().is_some() {
            return;
        }
        let authority = self.workspace_root_authority();
        let Some(root) = authority.effective_root.clone() else {
            return;
        };
        if !authority.allows_analysis() {
            if matches!(
                (&authority.state, scope),
                (WorkspaceRootState::RootChanged, RefreshScope::Full)
            ) {
                self.apply_workspace_root_authority(WorkspaceRootAuthority::selected(root.clone()))
                    .await;
            } else {
                return;
            }
        }
        let Some(config) = self.analysis_config() else {
            return;
        };
        let workspace_revision = self.workspace_revision();
        let authority_epoch = self.workspace_root_epoch.load(Ordering::SeqCst);
        let decision = self.refresh_scheduler.request(
            root,
            config,
            workspace_revision,
            authority_epoch,
            scope,
            reason,
        );
        let mut request = match decision {
            RefreshDecision::Start(request) => {
                let request = *request;
                self.mark_attempt_queued(&request);
                self.publish_analysis_status().await;
                request
            }
            RefreshDecision::Queued { generation } => {
                if let Some(request) = self.refresh_scheduler.pending_request(generation) {
                    self.mark_pending_attempt(&request);
                    self.publish_analysis_status().await;
                }
                self.log_refresh_queued(generation).await;
                loop {
                    if self.refresh_scheduler.is_idle() {
                        break;
                    }
                    let notified = self.refresh_idle.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    if self.refresh_scheduler.is_idle() {
                        break;
                    }
                    notified.await;
                }
                return;
            }
            RefreshDecision::Deduplicated | RefreshDecision::Stopped => return,
        };
        let mut cancellation_guard = RefreshCancellationGuard::new(
            self,
            &self.refresh_scheduler,
            &self.refresh_idle,
            request.clone(),
        );

        loop {
            let attempt_started = Instant::now();
            let outcome = self.run_refresh_request(&request).await;
            let attempt_duration = attempt_started.elapsed();
            self.refresh_scheduler
                .record_attempt_outcome(outcome, attempt_duration);
            self.record_health_outcome(&request, outcome);
            self.publish_analysis_status().await;
            self.log_refresh_attempt_outcome(outcome, attempt_duration)
                .await;
            let Some(next) = self
                .refresh_scheduler
                .finish(&request, outcome == RefreshAttemptOutcome::Published)
            else {
                cancellation_guard.disarm();
                self.refresh_idle.notify_waiters();
                return;
            };
            request = next;
            cancellation_guard.update(request.clone());
            self.mark_attempt_running(&request);
            self.publish_analysis_status().await;
        }
    }

    async fn run_refresh_request(&self, request: &RefreshRequest) -> RefreshAttemptOutcome {
        let generation = request.generation;
        self.mark_attempt_running(request);
        self.publish_analysis_status().await;
        self.log_refresh_queued(generation).await;
        if !self.refresh_request_is_current(request) {
            return RefreshAttemptOutcome::NotStarted;
        }
        let enabled_languages = request.config.repo_config().languages().enabled().to_vec();
        let started = Instant::now();
        self.log_refresh_started(generation).await;
        let root = request.root.clone();
        let config = request.config.clone();
        let defer_seam_inventory = request.scope.defer_seam_inventory();
        let cancellation = request.cancellation.clone();
        let execution_gate = self.refresh_scheduler.execution_gate();
        let analysis_root = root.clone();
        let diagnostics = match tokio::task::spawn_blocking(move || {
            let _execution = match execution_gate.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            cancellation
                .checkpoint()
                .map_err(|error| error.to_string())?;
            super::diagnostics::workspace_diagnostics_with_config_and_cancellation(
                &analysis_root,
                &config,
                defer_seam_inventory,
                &cancellation,
            )
        })
        .await
        {
            Ok(Ok(mut diagnostics)) => {
                diagnostics.snapshot.input_identity = Some(request.input_identity.clone());
                diagnostics
                    .snapshot
                    .refresh
                    .record_duration(started.elapsed());
                diagnostics
            }
            Ok(Err(err)) => {
                if self.refresh_request_is_current(request) && !is_cancellation_error(&err) {
                    self.report_refresh_failure_after(
                        request,
                        err,
                        started.elapsed(),
                        "analysis_error",
                    )
                    .await;
                    return RefreshAttemptOutcome::Failed;
                }
                if !is_cancellation_error(&err) {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            format!(
                                "ripr analysis error on superseded generation {generation}: {err}"
                            ),
                        )
                        .await;
                }
                return cancellation_outcome(request);
            }
            Err(err) => {
                if self.refresh_request_is_current(request) {
                    self.report_refresh_failure_after(
                        request,
                        format!("analysis task failed: {err}"),
                        started.elapsed(),
                        "task_failure",
                    )
                    .await;
                    return RefreshAttemptOutcome::Failed;
                }
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!(
                            "ripr analysis task failed on superseded generation {generation}: {err}"
                        ),
                    )
                    .await;
                return cancellation_outcome(request);
            }
        };
        if !self.refresh_request_is_current(request) {
            return cancellation_outcome(request);
        }
        if !workspace_diagnostics_are_root_contained(&root, &diagnostics) {
            self.report_refresh_failure_after(
                request,
                "diagnostic projection escaped the selected workspace root".to_string(),
                started.elapsed(),
                "root_projection",
            )
            .await;
            return RefreshAttemptOutcome::Failed;
        }
        let summary = RefreshLogSummary::from_snapshot(generation, &diagnostics.snapshot)
            .with_enabled_languages(&enabled_languages);
        let Some(transaction) = self.prepare_refresh_transaction(diagnostics) else {
            self.report_refresh_failure_after(
                request,
                "diagnostic snapshot was inconsistent with publish batches".to_string(),
                started.elapsed(),
                "analysis_error",
            )
            .await;
            return RefreshAttemptOutcome::Failed;
        };
        let RefreshTransaction {
            plan,
            snapshot,
            previous_diagnostics,
        } = transaction;
        let push_delivery = !self.pull_diagnostics_enabled();
        let published_uri_count = if push_delivery {
            plan.publish_batches.len()
        } else {
            0
        };
        let cleared_uri_count = if push_delivery {
            plan.clear_uris.len()
        } else {
            0
        };
        let unchanged_uri_count = plan.unchanged_uri_count;
        let published_payload_bytes = if push_delivery {
            plan.published_payload_bytes
        } else {
            0
        };
        let suppressed_payload_bytes = if push_delivery {
            plan.suppressed_payload_bytes
        } else {
            plan.published_payload_bytes
                .saturating_add(plan.suppressed_payload_bytes)
        };
        // Serialize the final authority check with root transitions and the
        // diagnostic publication/commit sequence.  An epoch check alone can
        // still race between the check and the first publish; holding this
        // guard makes a transition either complete before this block or wait
        // until the current snapshot has committed.
        let _root_transition = self.workspace_root_transition.lock().await;
        if !self.refresh_request_is_current(request) {
            self.rollback_refresh_transaction_if_authority_is_current(
                request,
                &previous_diagnostics,
                &plan,
            )
            .await;
            return cancellation_outcome(request);
        }
        if !self.pull_diagnostics_enabled() {
            for batch in &plan.publish_batches {
                if !self.refresh_request_is_current(request) {
                    self.rollback_refresh_transaction_if_authority_is_current(
                        request,
                        &previous_diagnostics,
                        &plan,
                    )
                    .await;
                    return cancellation_outcome(request);
                }
                self.client
                    .publish_diagnostics(batch.uri.clone(), batch.diagnostics.clone(), None)
                    .await;
            }
            for uri in &plan.clear_uris {
                if !self.refresh_request_is_current(request) {
                    self.rollback_refresh_transaction(&previous_diagnostics, &plan)
                        .await;
                    return cancellation_outcome(request);
                }
                self.client
                    .publish_diagnostics(uri.clone(), Vec::new(), None)
                    .await;
            }
        }
        if !self.refresh_request_is_current(request) {
            self.rollback_refresh_transaction_if_authority_is_current(
                request,
                &previous_diagnostics,
                &plan,
            )
            .await;
            return cancellation_outcome(request);
        }
        if !self.commit_refresh_snapshot(snapshot, &plan) {
            self.rollback_refresh_transaction_if_authority_is_current(
                request,
                &previous_diagnostics,
                &plan,
            )
            .await;
            self.report_refresh_failure_after(
                request,
                "could not commit the completed diagnostic snapshot".to_string(),
                started.elapsed(),
                "snapshot_commit_failure",
            )
            .await;
            return RefreshAttemptOutcome::Failed;
        }
        if self.pull_diagnostics_enabled()
            && self.diagnostic_refresh_support_enabled()
            && let Err(error) = self.client.workspace_diagnostic_refresh().await
        {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("pull diagnostic refresh request failed: {error}"),
                )
                .await;
        }
        self.log_refresh_completed(
            summary,
            published_uri_count,
            unchanged_uri_count,
            cleared_uri_count,
            published_payload_bytes,
            suppressed_payload_bytes,
        )
        .await;
        RefreshAttemptOutcome::Published
    }

    pub(super) async fn report_refresh_failure_after(
        &self,
        request: &RefreshRequest,
        message: String,
        duration: Duration,
        kind: &str,
    ) {
        self.client
            .log_message(
                MessageType::WARNING,
                refresh_failed_log_message(&message, duration),
            )
            .await;
        self.mark_attempt_failed(request, kind, &message);
        self.publish_analysis_status().await;
    }

    #[cfg(test)]
    pub(super) fn refresh_plan(
        &self,
        mut diagnostics: WorkspaceDiagnostics,
    ) -> Option<DiagnosticRefreshPlan> {
        if diagnostics.snapshot.input_identity.is_none() {
            let config = self.analysis_config().unwrap_or_default();
            diagnostics.snapshot.input_identity = Some(
                super::input_identity::LspAnalysisInputIdentity::from_refresh_inputs(
                    diagnostics.snapshot.root.clone(),
                    self.workspace_revision(),
                    &config,
                ),
            );
        }
        let transaction = self.prepare_refresh_transaction(diagnostics)?;
        let RefreshTransaction { plan, snapshot, .. } = transaction;
        self.commit_refresh_snapshot(snapshot, &plan)
            .then_some(plan)
    }

    #[cfg(test)]
    pub(super) fn initialize_test_workspace_root(&self) {
        let root = self
            .root
            .lock()
            .map(|root| root.clone())
            .unwrap_or_else(|_| PathBuf::from("."));
        self.set_workspace_root_authority(WorkspaceRootAuthority::selected(root));
    }

    pub(super) fn prepare_refresh_transaction(
        &self,
        diagnostics: WorkspaceDiagnostics,
    ) -> Option<RefreshTransaction> {
        let WorkspaceDiagnostics { snapshot, batches } = diagnostics;
        let Ok(last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        if snapshot.diagnostics_by_uri != diagnostics_by_uri_from_batches(&batches) {
            return None;
        }
        let plan = diagnostic_refresh_plan(&last_diagnostics, batches);
        debug_assert!(snapshot.is_consistent());
        Some(RefreshTransaction {
            plan,
            snapshot,
            previous_diagnostics: last_diagnostics.clone(),
        })
    }

    pub(super) fn commit_refresh_snapshot(
        &self,
        snapshot: AnalysisSnapshot,
        plan: &DiagnosticRefreshPlan,
    ) -> bool {
        if snapshot.input_identity.is_none() {
            return false;
        }
        let snapshot = Arc::new(snapshot);
        let diagnostic_result_ids =
            Arc::new(DiagnosticResultIdCache::for_snapshot(Arc::clone(&snapshot)));
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return false;
        };
        let Ok(mut last_diagnostics) = self.last_diagnostics.lock() else {
            return false;
        };
        let Ok(mut latest_analysis) = self.latest_analysis.lock() else {
            return false;
        };
        let Ok(mut stored_result_ids) = self.diagnostic_result_ids.lock() else {
            return false;
        };
        *last_diagnostics = snapshot.diagnostics_by_uri.clone();
        *last_diagnostic_uris = plan.current_uris.clone();
        *latest_analysis = Some(snapshot);
        *stored_result_ids = Some(diagnostic_result_ids);
        true
    }

    async fn rollback_refresh_transaction(
        &self,
        previous_diagnostics: &BTreeMap<Uri, Vec<Diagnostic>>,
        plan: &DiagnosticRefreshPlan,
    ) {
        if self.pull_diagnostics_enabled() {
            return;
        }
        let mut uris = previous_diagnostics
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        uris.extend(plan.current_uris.iter().cloned());
        for uri in uris {
            let diagnostics = previous_diagnostics.get(&uri).cloned().unwrap_or_default();
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn rollback_refresh_transaction_if_authority_is_current(
        &self,
        request: &RefreshRequest,
        previous_diagnostics: &BTreeMap<Uri, Vec<Diagnostic>>,
        plan: &DiagnosticRefreshPlan,
    ) {
        if self.refresh_authority_is_unchanged(request) {
            self.rollback_refresh_transaction(previous_diagnostics, plan)
                .await;
        }
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
        if let Ok(mut diagnostic_result_ids) = self.diagnostic_result_ids.lock() {
            *diagnostic_result_ids = None;
        }
        take_all_uris(&mut last_diagnostic_uris)
    }

    #[cfg(test)]
    pub(super) fn next_refresh_generation(&self) -> Option<u64> {
        self.refresh_scheduler.next_generation_for_test()
    }

    #[cfg(test)]
    pub(super) fn is_current_refresh_generation(&self, generation: u64) -> bool {
        self.refresh_scheduler.is_current_generation(generation)
    }

    fn workspace_revision(&self) -> u64 {
        self.workspace_revision
            .lock()
            .map(|revision| *revision)
            .unwrap_or(0)
    }

    fn advance_workspace_revision(&self) {
        if let Ok(mut revision) = self.workspace_revision.lock() {
            *revision = revision.saturating_add(1);
        }
    }

    fn effective_root(&self) -> Option<PathBuf> {
        self.workspace_root
            .lock()
            .ok()
            .and_then(|authority| authority.effective_root.clone())
    }

    fn refresh_request_is_current(&self, request: &RefreshRequest) -> bool {
        if !self
            .refresh_scheduler
            .is_current_generation(request.generation)
        {
            return false;
        }
        if self.workspace_root_epoch.load(Ordering::SeqCst) != request.authority_epoch {
            return false;
        }
        let authority = self.workspace_root_authority();
        authority.allows_analysis() && authority.effective_root.as_ref() == Some(&request.root)
    }

    pub(super) fn refresh_authority_is_unchanged(&self, request: &RefreshRequest) -> bool {
        self.workspace_root_epoch.load(Ordering::SeqCst) == request.authority_epoch
    }

    #[cfg(test)]
    pub(super) async fn invalidate_workspace_root_for_test(&self) {
        self.apply_workspace_root_authority(WorkspaceRootAuthority::unavailable(
            "test root transition",
        ))
        .await;
    }

    fn workspace_root_authority(&self) -> WorkspaceRootAuthority {
        self.workspace_root
            .lock()
            .map(|authority| authority.clone())
            .unwrap_or_else(|_| WorkspaceRootAuthority::unavailable("root state unavailable"))
    }

    pub(super) fn analysis_config(&self) -> Option<LspAnalysisConfig> {
        let Ok(config) = self.analysis_config.lock() else {
            return None;
        };
        Some(config.clone())
    }

    fn pull_diagnostics_enabled(&self) -> bool {
        self.pull_diagnostics
            .lock()
            .map(|supported| *supported)
            .unwrap_or(false)
    }

    fn diagnostic_refresh_support_enabled(&self) -> bool {
        self.diagnostic_refresh_support
            .lock()
            .map(|supported| *supported)
            .unwrap_or(false)
    }

    fn latest_pull_snapshot(
        &self,
    ) -> Option<(Arc<AnalysisSnapshot>, Arc<DiagnosticResultIdCache>)> {
        let latest_analysis = self.latest_analysis.lock().ok()?;
        let snapshot = latest_analysis.as_ref()?.clone();
        let stored_result_ids = self.diagnostic_result_ids.lock().ok()?;
        let result_ids = stored_result_ids
            .as_ref()
            .filter(|result_ids| result_ids.matches_snapshot(&snapshot))
            .cloned()
            .unwrap_or_else(|| {
                Arc::new(DiagnosticResultIdCache::for_snapshot(Arc::clone(&snapshot)))
            });
        Some((snapshot, result_ids))
    }

    #[cfg(test)]
    pub(super) fn latest_analysis_snapshot(&self) -> Option<AnalysisSnapshot> {
        let Ok(snapshot) = self.latest_analysis.lock() else {
            return None;
        };
        snapshot.as_ref().map(|value| value.as_ref().clone())
    }

    fn analysis_health_snapshot(&self) -> AnalysisHealth {
        self.analysis_health
            .lock()
            .map(|health| health.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn set_snapshot_run_status_for_test(&self, run_status: &str) {
        if let Ok(mut health) = self.analysis_health.lock() {
            health.snapshot_run_status = Some(run_status.to_string());
        }
    }

    #[cfg(test)]
    pub(super) fn set_analysis_attempt_state_for_test(&self, state: AnalysisAttemptState) {
        if let Ok(mut health) = self.analysis_health.lock() {
            health.attempt_id = Some(2);
            health.state = state;
            health.snapshot_id = Some("snapshot:test".to_string());
            health.last_success_snapshot_id = Some("snapshot:test".to_string());
        }
    }

    fn effective_health_for_snapshot(
        &self,
        mut health: AnalysisHealth,
        snapshot: &AnalysisSnapshot,
    ) -> AnalysisHealth {
        // `refresh_plan` is also a focused test seam and may be queried before
        // the async refresh loop records its final health event. Do not expose
        // an internally contradictory `latest_analysis != None` plus
        // `run_status = no_snapshot` state during that short handoff.
        if health.snapshot_id.is_none() && health.attempt_id.is_none() {
            let snapshot_id = "snapshot:legacy".to_string();
            health.state = AnalysisAttemptState::Succeeded;
            health.snapshot_id = Some(snapshot_id.clone());
            health.last_success_snapshot_id = Some(snapshot_id);
            health.last_success_at = Some(snapshot.refresh.generated_at);
            health.snapshot_run_status = Some(workspace_status_run_status(snapshot).to_string());
            health.last_success_input_identity = snapshot.input_identity_id();
        }
        health
    }

    fn mark_attempt_queued(&self, request: &RefreshRequest) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.state = AnalysisAttemptState::Queued;
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        health.current_input_identity = Some(request.input_identity_id());
        health.failure = None;
    }

    fn mark_attempt_running(&self, request: &RefreshRequest) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.state = AnalysisAttemptState::Running;
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        health.current_input_identity = Some(request.input_identity_id());
        if health.pending_attempt_id == Some(request.generation) {
            health.pending_attempt_id = None;
            health.pending_reason = None;
            health.pending_scope = None;
        }
    }

    fn mark_pending_attempt(&self, request: &RefreshRequest) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.pending_attempt_id = Some(request.generation);
        health.pending_reason = Some(request.reason.as_str().to_string());
        health.pending_scope = Some(request.scope.as_str().to_string());
    }

    fn mark_attempt_failed(&self, request: &RefreshRequest, kind: &str, message: &str) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.state = AnalysisAttemptState::Failed;
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        health.failure = Some(AnalysisFailure {
            kind: kind.to_string(),
            message: bounded_failure_message(message),
        });
    }

    fn mark_attempt_cancelled(&self, request: &RefreshRequest) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.state = AnalysisAttemptState::Cancelled;
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        health.pending_attempt_id = None;
        health.pending_reason = None;
        health.pending_scope = None;
    }

    pub(super) fn record_health_outcome(
        &self,
        request: &RefreshRequest,
        outcome: RefreshAttemptOutcome,
    ) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        match outcome {
            RefreshAttemptOutcome::Published => {
                health.state = AnalysisAttemptState::Succeeded;
                health.failure = None;
                let snapshot_id = format!("snapshot:{}", request.generation);
                health.snapshot_id = Some(snapshot_id.clone());
                health.last_success_snapshot_id = Some(snapshot_id);
                health.last_success_at =
                    self.latest_analysis.lock().ok().and_then(|snapshot| {
                        snapshot.as_ref().map(|value| value.refresh.generated_at)
                    });
                health.snapshot_run_status = self
                    .latest_analysis
                    .lock()
                    .ok()
                    .and_then(|snapshot| {
                        snapshot
                            .as_ref()
                            .map(|snapshot| workspace_status_run_status(snapshot))
                    })
                    .map(str::to_string);
                health.current_input_identity = Some(request.input_identity_id());
                health.last_success_input_identity =
                    self.latest_analysis.lock().ok().and_then(|snapshot| {
                        snapshot
                            .as_ref()
                            .and_then(|snapshot| snapshot.input_identity_id())
                    });
            }
            RefreshAttemptOutcome::Failed => {
                // `mark_attempt_failed` records the bounded error and preserves
                // the completed snapshot. Keep that richer state here.
            }
            RefreshAttemptOutcome::Cancelled => health.state = AnalysisAttemptState::Cancelled,
            RefreshAttemptOutcome::Superseded => health.state = AnalysisAttemptState::Superseded,
            RefreshAttemptOutcome::NotStarted => health.state = AnalysisAttemptState::Stopped,
        }
    }

    async fn publish_analysis_status(&self) {
        self.client
            .send_notification::<AnalysisStatusNotification>(self.analysis_status_payload())
            .await;
    }

    fn analysis_status_payload(&self) -> LSPAny {
        let health = self.analysis_health_snapshot();
        self.analysis_status_payload_for_health(&health)
    }

    fn analysis_status_payload_for_health(&self, health: &AnalysisHealth) -> LSPAny {
        let root = self.workspace_root_authority();
        let last_success_age_ms = health
            .last_success_at
            .and_then(|generated_at| generated_at.elapsed().ok())
            .map(|duration| duration.as_millis() as u64);
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "analysis_status",
            "attempt_id": health.attempt_id.map(|id| id.to_string()),
            "state": health.state.as_str(),
            "reason": health.reason,
            "requested_scope": health.requested_scope,
            "snapshot_id": health.snapshot_id,
            "last_success_snapshot_id": health.last_success_snapshot_id,
            "current_input_identity": health.current_input_identity,
            "last_success_input_identity": health.last_success_input_identity,
            "last_success_age_ms": last_success_age_ms,
            "run_status": health.run_status(),
            "diagnostic_profile": self
                .analysis_config()
                .map(|config| config.diagnostic_profile.as_str())
                .unwrap_or("actionable"),
            "failure": health.failure.clone().map(|failure| serde_json::json!({
                "kind": failure.kind,
                "message": failure.message,
            })),
            "pending": health.pending(),
            "pending_attempt_id": health.pending_attempt_id.map(|id| id.to_string()),
            "pending_reason": health.pending_reason,
            "pending_scope": health.pending_scope,
            "retry_command": REFRESH_COMMAND,
            "repair_actions_available": health.allows_current_repairs()
                && root.allows_analysis(),
            "root_state": root.state.as_str(),
            "effective_root": root
                .effective_root
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            "candidate_roots": root
                .candidate_roots
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            "root_input_identity": root.input_identity(),
            "root_detail": root.detail,
            "root_recovery_route": root_recovery_route(&root.state),
        })
    }

    fn set_root(&self, root: PathBuf) {
        let Ok(mut current_root) = self.root.lock() else {
            return;
        };
        *current_root = root;
    }

    async fn apply_workspace_root_resolution(&self, resolution: WorkspaceRootResolution) {
        let authority = match resolution {
            WorkspaceRootResolution::Selected(root) => {
                if !root.is_absolute() {
                    WorkspaceRootAuthority::unavailable(format!(
                        "workspace root is not absolute: {}",
                        root.display()
                    ))
                } else if !root.is_dir() {
                    WorkspaceRootAuthority::unavailable(format!(
                        "workspace root is inaccessible or not a directory: {}",
                        root.display()
                    ))
                } else {
                    WorkspaceRootAuthority::selected(root)
                }
            }
            WorkspaceRootResolution::Ambiguous(candidates) => {
                if candidates
                    .iter()
                    .any(|root| !root.is_absolute() || !root.is_dir())
                {
                    WorkspaceRootAuthority::unavailable(
                        "one or more workspace folders are inaccessible or not directories",
                    )
                } else {
                    WorkspaceRootAuthority::ambiguous(candidates)
                }
            }
            WorkspaceRootResolution::Unavailable(detail) => {
                WorkspaceRootAuthority::unavailable(detail)
            }
        };

        self.apply_workspace_root_authority(authority).await;
    }

    async fn apply_workspace_root_authority(&self, authority: WorkspaceRootAuthority) {
        let _transition = self.workspace_root_transition.lock().await;
        let authority_epoch = self.workspace_root_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let previous = self.workspace_root_authority();
        let changed = previous.state != authority.state
            || previous.effective_root != authority.effective_root
            || previous.candidate_roots != authority.candidate_roots;
        if changed {
            self.refresh_scheduler.invalidate_input();
            let uris = self.clear_all_diagnostic_uris();
            if !self.pull_diagnostics_enabled() {
                for uri in uris {
                    self.client.publish_diagnostics(uri, Vec::new(), None).await;
                }
            }
            self.reset_health_for_input_change();
        }

        if self.workspace_root_epoch.load(Ordering::SeqCst) != authority_epoch {
            return;
        }

        let final_authority = if changed
            && matches!(authority.state, WorkspaceRootState::SelectedSingleRoot)
            && previous.effective_root != authority.effective_root
            && previous.effective_root.is_some()
        {
            WorkspaceRootAuthority::changed(
                previous.effective_root.clone(),
                authority.effective_root.clone(),
            )
        } else {
            authority
        };

        if let Some(root) = final_authority.effective_root.clone() {
            self.set_root(root);
        }
        self.set_workspace_root_authority(final_authority);
        self.publish_analysis_status().await;
        self.refresh_idle.notify_waiters();
    }

    fn set_workspace_root_authority(&self, authority: WorkspaceRootAuthority) {
        if let Ok(mut current) = self.workspace_root.lock() {
            *current = authority;
        }
    }

    fn reset_health_for_input_change(&self) {
        if let Ok(mut failure) = self.configuration_failure.lock() {
            *failure = None;
        }
        if let Ok(mut health) = self.analysis_health.lock() {
            health.state = AnalysisAttemptState::Stopped;
            health.attempt_id = None;
            health.snapshot_id = None;
            health.last_success_snapshot_id = None;
            health.last_success_at = None;
            health.snapshot_run_status = None;
            health.current_input_identity = None;
            health.last_success_input_identity = None;
            health.failure = None;
            health.pending_attempt_id = None;
            health.pending_reason = None;
            health.pending_scope = None;
        }
    }

    fn invalidate_analysis_input(&self, reason: &str) {
        self.refresh_scheduler.invalidate_input();
        if let Ok(mut health) = self.analysis_health.lock() {
            health.state = AnalysisAttemptState::Stopped;
            health.reason = Some(reason.to_string());
            health.requested_scope = Some(RefreshScope::Interactive.as_str().to_string());
            health.failure = None;
            health.current_input_identity = None;
            health.pending_attempt_id = None;
            health.pending_reason = None;
            health.pending_scope = None;
        }
    }

    fn set_configuration_failure(&self, message: impl Into<String>) {
        let failure = AnalysisFailure {
            kind: "config_invalid".to_string(),
            message: bounded_failure_message(&message.into()),
        };
        if let Ok(mut current) = self.configuration_failure.lock() {
            *current = Some(failure.clone());
        }
        if let Ok(mut health) = self.analysis_health.lock() {
            health.state = AnalysisAttemptState::Failed;
            health.reason = Some(RefreshReason::ConfigReload.as_str().to_string());
            health.requested_scope = Some(RefreshScope::Interactive.as_str().to_string());
            health.failure = Some(failure);
            health.current_input_identity = None;
        }
    }

    fn clear_configuration_failure(&self) {
        if let Ok(mut current) = self.configuration_failure.lock() {
            *current = None;
        }
        if let Ok(mut health) = self.analysis_health.lock()
            && health
                .failure
                .as_ref()
                .is_some_and(|failure| failure.kind == "config_invalid")
        {
            health.failure = None;
            health.state = AnalysisAttemptState::Stopped;
        }
    }

    pub(super) fn configuration_failure(&self) -> Option<AnalysisFailure> {
        self.configuration_failure
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().cloned())
    }

    async fn reload_repository_config(&self) {
        let Some(root) = self.effective_root() else {
            return;
        };
        let current = self.analysis_config().unwrap_or_default();
        let had_configuration_failure = self.configuration_failure().is_some();
        match crate::config::load_for_root(&root) {
            Ok(repo_config) => {
                let next = current.reload_repo_config(repo_config);
                self.clear_configuration_failure();
                if next != current || had_configuration_failure {
                    self.set_analysis_config(next);
                    self.invalidate_analysis_input(RefreshReason::ConfigReload.as_str());
                    self.publish_analysis_status().await;
                    self.refresh_diagnostics(
                        RefreshScope::Interactive,
                        RefreshReason::ConfigReload,
                    )
                    .await;
                } else {
                    self.publish_analysis_status().await;
                }
            }
            Err(error) => {
                self.invalidate_analysis_input(RefreshReason::ConfigReload.as_str());
                self.set_configuration_failure(error);
                self.publish_analysis_status().await;
            }
        }
    }

    async fn apply_session_configuration_change(&self, settings: &LSPAny) {
        if !LspAnalysisConfig::has_session_option_changes(settings) {
            return;
        }
        let Some(current) = self.analysis_config() else {
            return;
        };
        let Some(next) = current.with_changed_session_options(settings) else {
            return;
        };
        if next == current {
            return;
        }
        self.set_analysis_config(next);
        if self.configuration_failure().is_some() {
            // Keep the config_invalid signal visible while repository
            // configuration remains broken. The stored session override is
            // retained and will be reapplied when the repository reload
            // succeeds; it cannot authorize a refresh on its own.
            self.publish_analysis_status().await;
            return;
        }
        self.invalidate_analysis_input(RefreshReason::ConfigReload.as_str());
        self.publish_analysis_status().await;
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::ConfigReload)
            .await;
    }

    fn file_event_is_repository_config(&self, event: &FileEvent) -> bool {
        let Some(root) = self.effective_root() else {
            return false;
        };
        let expected = root.join(crate::config::CONFIG_FILE_NAME);
        file_uri_for_path(&expected)
            .ok()
            .is_some_and(|uri| file_uris_match(&uri, &event.uri))
    }

    fn file_event_is_workspace_manifest_or_lockfile(&self, event: &FileEvent) -> bool {
        let Some(root) = self.effective_root() else {
            return false;
        };
        let Some(path) = path_from_file_uri(&event.uri) else {
            return false;
        };
        workspace_input_path_is_relevant(&root, &path)
    }

    pub(super) fn watched_file_change_kinds(&self, changes: &[FileEvent]) -> (bool, bool) {
        let config_changed = changes
            .iter()
            .any(|event| self.file_event_is_repository_config(event));
        let workspace_graph_changed = changes
            .iter()
            .any(|event| self.file_event_is_workspace_manifest_or_lockfile(event));
        (config_changed, workspace_graph_changed)
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

pub(super) fn workspace_input_path_is_relevant(root: &Path, path: &Path) -> bool {
    path_is_within_root(root, path)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, "Cargo.toml" | "Cargo.lock"))
}

struct RefreshCancellationGuard<'a> {
    backend: &'a Backend,
    scheduler: &'a RefreshScheduler,
    idle: &'a Notify,
    request: RefreshRequest,
    armed: bool,
}

impl<'a> RefreshCancellationGuard<'a> {
    fn new(
        backend: &'a Backend,
        scheduler: &'a RefreshScheduler,
        idle: &'a Notify,
        request: RefreshRequest,
    ) -> Self {
        Self {
            backend,
            scheduler,
            idle,
            request,
            armed: true,
        }
    }

    fn update(&mut self, request: RefreshRequest) {
        self.request = request;
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RefreshCancellationGuard<'_> {
    fn drop(&mut self) {
        if self.armed && self.scheduler.cancel(&self.request) {
            self.backend.mark_attempt_cancelled(&self.request);
            self.idle.notify_waiters();
        }
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

    async fn log_refresh_attempt_outcome(
        &self,
        outcome: RefreshAttemptOutcome,
        duration: Duration,
    ) {
        let telemetry = self.refresh_scheduler.telemetry();
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "ripr analysis refresh attempt outcome={}, duration={}, coalesced={}, cancelled={}, superseded={}, published={}, failed={}, queue_high_water={}, last_superseded_ms={}",
                    outcome.as_str(),
                    format_duration(duration),
                    telemetry.requests_coalesced,
                    telemetry.active_attempts_cooperatively_cancelled,
                    telemetry.completed_but_superseded,
                    telemetry.snapshots_published,
                    telemetry.failed_attempts,
                    telemetry.pending_queue_high_water,
                    telemetry
                        .last_superseded_attempt_ms
                        .map_or_else(|| "none".to_string(), |value| value.to_string()),
                ),
            )
            .await;
    }

    async fn log_refresh_completed(
        &self,
        summary: RefreshLogSummary,
        published_uri_count: usize,
        unchanged_uri_count: usize,
        cleared_uri_count: usize,
        published_payload_bytes: usize,
        suppressed_payload_bytes: usize,
    ) {
        self.client
            .log_message(
                MessageType::INFO,
                refresh_completed_log_message_with_telemetry(
                    &summary,
                    published_uri_count,
                    unchanged_uri_count,
                    cleared_uri_count,
                    published_payload_bytes,
                    suppressed_payload_bytes,
                ),
            )
            .await;
    }
}

#[cfg(test)]
pub(super) fn refresh_completed_log_message(
    summary: &RefreshLogSummary,
    published_uri_count: usize,
    cleared_uri_count: usize,
) -> String {
    refresh_completed_log_message_with_telemetry(
        summary,
        published_uri_count,
        0,
        cleared_uri_count,
        0,
        0,
    )
}

fn refresh_completed_log_message_with_telemetry(
    summary: &RefreshLogSummary,
    published_uri_count: usize,
    unchanged_uri_count: usize,
    cleared_uri_count: usize,
    published_payload_bytes: usize,
    suppressed_payload_bytes: usize,
) -> String {
    let duration = format_duration(summary.duration);
    format!(
        "ripr analysis refresh completed in {duration}: generation={}, diagnostics={}, files={}, findings={}, preview_findings={}, static_limits={}, seam_diagnostics={}, gap_artifacts={}, actionable_gap_artifacts={}, preview_gap_artifacts={}, no_action_gap_artifacts={}, gap_static_limits={}, gap_artifact_rejections={}, gap_artifact_rejection_kinds={}, enabled_languages={}, enabled_language_names={}, computed_files={}, published_files={}, unchanged_files={}, cleared_files={}, published_payload_bytes={}, suppressed_payload_bytes={}",
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
        summary.files,
        published_uri_count,
        unchanged_uri_count,
        cleared_uri_count,
        published_payload_bytes,
        suppressed_payload_bytes
    )
}

pub(super) fn refresh_failed_log_message(message: &str, duration: Duration) -> String {
    format!(
        "ripr analysis refresh failed after {}: {message}",
        format_duration(duration)
    )
}

fn bounded_failure_message(message: &str) -> String {
    let normalized = message
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            character => character,
        })
        .collect::<String>();
    let path_safe = normalized
        .split_whitespace()
        .map(|token| {
            if token.starts_with('/')
                || token.starts_with('\\')
                || token
                    .as_bytes()
                    .get(1)
                    .is_some_and(|character| *character == b':')
            {
                "<path>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut bounded = path_safe.chars().take(240).collect::<String>();
    if path_safe.chars().count() > 240 {
        bounded.push('…');
    }
    bounded
}

fn cancellation_outcome(request: &RefreshRequest) -> RefreshAttemptOutcome {
    match request.cancellation.checkpoint() {
        Err(error) if error.kind == AnalysisAbortKind::Cancelled => {
            RefreshAttemptOutcome::Cancelled
        }
        Err(_) => RefreshAttemptOutcome::Superseded,
        Ok(()) => RefreshAttemptOutcome::Superseded,
    }
}

fn diagnostics_by_uri_from_batches(batches: &[DiagnosticBatch]) -> BTreeMap<Uri, Vec<Diagnostic>> {
    batches
        .iter()
        .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
        .collect()
}

fn workspace_diagnostics_are_root_contained(
    root: &Path,
    diagnostics: &WorkspaceDiagnostics,
) -> bool {
    let snapshot_root_matches = super::uri::path_is_within_root(root, &diagnostics.snapshot.root)
        && super::uri::path_is_within_root(&diagnostics.snapshot.root, root);
    snapshot_root_matches
        && diagnostics.batches.iter().all(|batch| {
            file_uri_is_within_root(root, &batch.uri)
                && batch.diagnostics.iter().all(|diagnostic| {
                    diagnostic
                        .related_information
                        .as_ref()
                        .is_none_or(|related| {
                            related
                                .iter()
                                .all(|item| file_uri_is_within_root(root, &item.location.uri))
                        })
                })
        })
}

fn root_recovery_route(state: &WorkspaceRootState) -> &'static str {
    match state {
        WorkspaceRootState::SelectedSingleRoot => "refresh",
        WorkspaceRootState::WorkspaceAmbiguous => "select_root_and_restart",
        WorkspaceRootState::RootUnavailable => "select_root_and_restart",
        WorkspaceRootState::RootRemoved => "select_root_and_restart",
        WorkspaceRootState::RootChanged => "refresh",
    }
}

fn root_authority_block_reason(state: &WorkspaceRootState) -> &'static str {
    match state {
        WorkspaceRootState::WorkspaceAmbiguous => "workspace_root_ambiguous",
        WorkspaceRootState::RootUnavailable => "workspace_root_unavailable",
        WorkspaceRootState::RootRemoved => "workspace_root_removed",
        WorkspaceRootState::RootChanged => "workspace_root_changed",
        WorkspaceRootState::SelectedSingleRoot => "analysis_snapshot_stale",
    }
}

fn root_authority_receipt_status(state: &WorkspaceRootState) -> LSPAny {
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "receipt_status",
        "status": "workspace_root_blocked",
        "receipt_status": "not_available",
        "missing_receipt_reason": root_authority_block_reason(state),
        "copy_receipt_command": "not_available",
        "open_attempt_ledger": "not_available",
        "latest_attempt_outcome": "not_available",
        "route_quality_summary": "not_available",
        "limits_note": "Static evidence only; advisory, not a gate decision.",
    })
}

impl LanguageServer for Backend {
    async fn initialized(&self, _: InitializedParams) {
        let supports_dynamic_registration = self
            .dynamic_file_watch_registration
            .lock()
            .map(|value| *value)
            .unwrap_or(false);
        if !supports_dynamic_registration {
            return;
        }
        let registration = Registration {
            id: "ripr-config-file-watch".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::json!({
                "watchers": [
                    {"globPattern": "**/ripr.toml"},
                    {"globPattern": "**/Cargo.toml"},
                    {"globPattern": "**/Cargo.lock"}
                ]
            })),
        };
        if let Err(error) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("ripr workspace input watching unavailable: {error}"),
                )
                .await;
        }
    }

    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let supports_pull_diagnostics = client_supports_pull_diagnostics(&params);
        let supports_diagnostic_refresh = client_supports_diagnostic_refresh(&params);
        if let Ok(mut supported) = self.pull_diagnostics.lock() {
            *supported = supports_pull_diagnostics;
        }
        if let Ok(mut supported) = self.diagnostic_refresh_support.lock() {
            *supported = supports_diagnostic_refresh;
        }
        let supports_dynamic_registration = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref())
            .and_then(|capability| capability.dynamic_registration)
            .unwrap_or(false);
        if let Ok(mut supported) = self.dynamic_file_watch_registration.lock() {
            *supported = supports_dynamic_registration;
        }
        let resolution = root_from_initialize_params(&params);
        let (repo_config, config_error) = match &resolution {
            WorkspaceRootResolution::Selected(root) if root.is_dir() => {
                match crate::config::load_for_root(root) {
                    Ok(config) => (config, None),
                    Err(err) => (crate::config::RiprConfig::default(), Some(err)),
                }
            }
            _ => (crate::config::RiprConfig::default(), None),
        };
        self.set_analysis_config(LspAnalysisConfig::from_initialize_params(
            &params,
            repo_config,
        ));
        self.set_workspace_root_authority(WorkspaceRootAuthority::unavailable(
            "initial workspace root resolution pending",
        ));
        self.apply_workspace_root_resolution(resolution).await;
        if let Some(error) = config_error {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("ripr config load failed; analysis is paused: {error}"),
                )
                .await;
            self.set_configuration_failure(error);
            self.publish_analysis_status().await;
        }
        Ok(initialize_result_for_client(supports_pull_diagnostics))
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.apply_session_configuration_change(&params.settings)
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let (config_changed, workspace_graph_changed) =
            self.watched_file_change_kinds(&params.changes);
        if config_changed {
            self.reload_repository_config().await;
        }
        if workspace_graph_changed {
            self.invalidate_analysis_input("workspace_manifest_or_lockfile_changed");
            self.publish_analysis_status().await;
            self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::ConfigReload)
                .await;
        }
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        let resolution = match self.client.workspace_folders().await {
            Ok(Some(folders)) => {
                if folders.is_empty() {
                    self.apply_workspace_root_authority(WorkspaceRootAuthority::removed(
                        self.effective_root(),
                    ))
                    .await;
                    return;
                }
                let params = InitializeParams {
                    workspace_folders: Some(folders),
                    ..InitializeParams::default()
                };
                root_from_initialize_params(&params)
            }
            Ok(None) => WorkspaceRootResolution::Unavailable(
                "client did not return workspace folders after a workspace change".to_string(),
            ),
            Err(err) => WorkspaceRootResolution::Unavailable(format!(
                "workspace folder query failed: {err}"
            )),
        };
        self.apply_workspace_root_resolution(resolution).await;
        self.reload_repository_config().await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        self.refresh_scheduler.stop();
        self.clear_all_diagnostic_uris();
        self.reset_health_for_input_change();
        self.publish_analysis_status().await;
        self.refresh_idle.notify_waiters();
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.open_document(params);
        self.advance_workspace_revision();
        // Interactive path: defer the seam inventory (RIPR-SPEC-0105).
        // Diff-scoped findings are complete; seams run on explicit refresh only.
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::DidOpen)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.change_document(params);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.close_document(params);
        self.advance_workspace_revision();
        // Interactive path: defer the seam inventory (RIPR-SPEC-0105).
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::DidClose)
            .await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.advance_workspace_revision();
        // Interactive path: defer the seam inventory (RIPR-SPEC-0105).
        // Diff-scoped findings are complete; seams run on explicit refresh only.
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::DidSave)
            .await;
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> LspResult<DocumentDiagnosticReportResult> {
        let Some((snapshot, result_ids)) = self.latest_pull_snapshot() else {
            return Ok(
                DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                    related_documents: None,
                    full_document_diagnostic_report:
                        tower_lsp_server::ls_types::FullDocumentDiagnosticReport {
                            result_id: None,
                            items: Vec::new(),
                        },
                })
                .into(),
            );
        };
        let uri = params.text_document.uri;
        let result_id = result_ids.document_id(&snapshot, &uri);
        if params.previous_result_id.as_deref() == Some(result_id.as_str()) {
            return Ok(DocumentDiagnosticReport::Unchanged(
                RelatedUnchangedDocumentDiagnosticReport {
                    related_documents: None,
                    unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                        result_id,
                    },
                },
            )
            .into());
        }
        let diagnostics = snapshot
            .diagnostics_for_uri(&uri)
            .map_or_else(Vec::new, |items| items.to_vec());
        Ok(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report:
                    tower_lsp_server::ls_types::FullDocumentDiagnosticReport {
                        result_id: Some(result_id),
                        items: diagnostics,
                    },
            })
            .into(),
        )
    }

    async fn workspace_diagnostic(
        &self,
        params: WorkspaceDiagnosticParams,
    ) -> LspResult<WorkspaceDiagnosticReportResult> {
        let Some((snapshot, result_ids)) = self.latest_pull_snapshot() else {
            return Ok(WorkspaceDiagnosticReport { items: Vec::new() }.into());
        };
        let mut uris = snapshot
            .diagnostics_by_uri
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        uris.extend(
            params
                .previous_result_ids
                .iter()
                .map(|entry| entry.uri.clone()),
        );
        let mut items = Vec::with_capacity(uris.len());
        for uri in uris {
            let result_id = result_ids.document_id(&snapshot, &uri);
            let previous = params
                .previous_result_ids
                .iter()
                .find(|entry| entry.uri == uri)
                .map(|entry| entry.value.as_str());
            if previous == Some(result_id.as_str()) {
                items.push(WorkspaceDocumentDiagnosticReport::Unchanged(
                    WorkspaceUnchangedDocumentDiagnosticReport {
                        uri,
                        version: None,
                        unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                            result_id,
                        },
                    },
                ));
                continue;
            }
            let diagnostics = snapshot
                .diagnostics_for_uri(&uri)
                .map_or_else(Vec::new, |items| items.to_vec());
            items.push(WorkspaceDocumentDiagnosticReport::Full(
                WorkspaceFullDocumentDiagnosticReport {
                    uri,
                    version: None,
                    full_document_diagnostic_report:
                        tower_lsp_server::ls_types::FullDocumentDiagnosticReport {
                            result_id: Some(result_id),
                            items: diagnostics,
                        },
                },
            ));
        }
        Ok(WorkspaceDiagnosticReport { items }.into())
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
        let health = self.analysis_health_snapshot();
        let root_allows_analysis = self.workspace_root_authority().allows_analysis();
        let action_snapshot = (health.allows_current_repairs() && root_allows_analysis)
            .then_some(snapshot)
            .flatten();
        Ok(Some(code_action_response(
            &params,
            action_snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
        )))
    }

    /// Advisory `textDocument/codeLens` handler (RIPR-SPEC-0099).
    ///
    /// Locks `latest_analysis` read-only exactly like `hover_for_position` and
    /// `code_action`, then delegates to the pure `code_lens_response` helper in
    /// `lsp/lens.rs`. Returns `Some([])` (not `None`) so the client removes any
    /// stale lenses from a previous snapshot. Returns an empty Vec when no
    /// snapshot is available — absence of analysis is not absence of tests, and
    /// we must not fabricate a 0-count lens.
    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        let snapshot = self
            .latest_analysis
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let uri = &params.text_document.uri;
        Ok(Some(code_lens_response(
            uri,
            snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
        )))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command == REFRESH_COMMAND {
            if self.configuration_failure().is_some() {
                // A retry must re-read the repository configuration. The
                // normal refresh guard intentionally refuses to run while a
                // config error is latched, so relying on a later file-watch
                // event would make the advertised retry route dishonest.
                self.reload_repository_config().await;
                return Ok(None);
            }
            // Explicit refresh: run the full seam inventory (RIPR-SPEC-0105).
            // This is the demand path that transitions a seams_deferred snapshot
            // to full/limited with complete seam evidence.
            self.refresh_diagnostics(RefreshScope::Full, RefreshReason::ExplicitRefresh)
                .await;
            return Ok(None);
        }
        if params.command == COLLECT_CONTEXT_COMMAND {
            return Ok(self.collect_context_packet(&params.arguments));
        }
        if params.command == COLLECT_EVIDENCE_CONTEXT_COMMAND {
            return Ok(self.collect_evidence_context_packet(&params.arguments));
        }
        if params.command == COLLECT_WORKSPACE_STATUS_COMMAND {
            return Ok(self.collect_workspace_status());
        }
        if params.command == COLLECT_REPAIR_PACKET_COMMAND {
            return Ok(self.collect_repair_packet(&params.arguments));
        }
        if params.command == COLLECT_TOP_LIMITATION_COMMAND {
            return Ok(self.collect_top_limitation());
        }
        if params.command == COLLECT_RECEIPT_STATUS_COMMAND {
            return Ok(self.collect_receipt_status());
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
            let (causal_projection, causal_projection_warning) =
                crate::app::causal_projection::CausalDeltaArtifact::load_optional(&snapshot.root);
            if let Some(warning) = causal_projection_warning {
                eprintln!("ripr lsp: {warning}");
            }
            let packet =
                crate::output::agent_seam_packets::render_agent_seam_packets_json_with_causal(
                    std::slice::from_ref(seam),
                    None,
                    causal_projection.as_ref(),
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

    fn collect_workspace_status(&self) -> Option<LSPAny> {
        let health = self.analysis_health_snapshot();
        let authority = self.workspace_root_authority();
        let snapshot = match self.latest_analysis.lock().ok()? {
            guard if guard.is_none() => {
                let top_limitation = top_limitation_dto(&health, None, &authority).into_json();
                return Some(serde_json::json!({
                    "schema_version": "0.1",
                    "tool": "ripr",
                    "kind": "workspace_status",
                    "run_status": health.run_status(),
                    "analysis_status": self.analysis_status_payload_for_health(&health),
                    "snapshot_age_ms": serde_json::Value::Null,
                    "snapshot_duration_ms": serde_json::Value::Null,
                    "diagnostics": serde_json::Value::Null,
                    "top_actionable_packet": serde_json::Value::Null,
                    "top_limitation": top_limitation,
                    "report_paths": workspace_status_report_paths(),
                    "refresh_command": REFRESH_COMMAND,
                    "limits_note": "Static evidence only; advisory, not a gate decision.",
                }));
            }
            guard => guard.clone()?,
        };
        let health = self.effective_health_for_snapshot(health, &snapshot);
        let analysis_status = self.analysis_status_payload_for_health(&health);

        let age_ms = snapshot
            .refresh
            .age()
            .map(|d| serde_json::Value::from(d.as_millis() as u64))
            .unwrap_or(serde_json::Value::Null);
        let duration_ms = snapshot
            .refresh
            .duration
            .map(|d| serde_json::Value::from(d.as_millis() as u64))
            .unwrap_or(serde_json::Value::Null);

        let total_diagnostics = snapshot.diagnostic_count();
        let files = snapshot.diagnostic_uri_count();
        let raw_signals = snapshot.finding_count();
        let canonical_items = snapshot.canonical_finding_count();
        let actionable_diagnostics = snapshot.actionable_diagnostic_count();
        let seam_diagnostics = snapshot.seam_diagnostic_count();
        let gap_artifacts = snapshot.gap_artifacts.len();
        let actionable_gap_artifacts = snapshot
            .gap_artifacts
            .iter()
            .filter(|a| a.is_actionable_gap())
            .count();
        let gap_artifact_rejections = snapshot.gap_artifact_rejections.len();

        let top_actionable_packet =
            if health.allows_current_repairs() && authority.allows_analysis() {
                workspace_status_top_actionable_packet(&snapshot)
            } else {
                serde_json::Value::Null
            };
        let top_limitation = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        let run_status = health.run_status();

        // Compact receipt/outcome summary — reuses the same artifact readers
        // as collect_receipt_status so the cockpit's single status call
        // surfaces receipt state without a second round-trip.
        let root = match self.root.lock().ok() {
            Some(r) => r.clone(),
            None => {
                return Some(serde_json::json!({
                    "schema_version": "0.1",
                    "tool": "ripr",
                    "kind": "workspace_status",
                    "run_status": run_status,
                    "analysis_status": analysis_status,
                    "snapshot_age_ms": age_ms,
                    "snapshot_duration_ms": duration_ms,
                    "diagnostics": {
                        "total": total_diagnostics,
                        "files": files,
                        "findings": raw_signals,
                        "raw_signals": raw_signals,
                        "canonical_items": canonical_items,
                        "actionable_diagnostics": actionable_diagnostics,
                        "seam_diagnostics": seam_diagnostics,
                        "gap_artifacts": gap_artifacts,
                        "actionable_gap_artifacts": actionable_gap_artifacts,
                        "gap_artifact_rejections": gap_artifact_rejections,
                    },
                    "top_actionable_packet": top_actionable_packet,
                    "top_limitation": top_limitation,
                    "receipt_status_summary": serde_json::Value::Null,
                    "report_paths": workspace_status_report_paths(),
                    "refresh_command": REFRESH_COMMAND,
                    "limits_note": "Static evidence only; advisory, not a gate decision.",
                }));
            }
        };
        let receipt_status_summary = workspace_status_receipt_summary(&root, &snapshot);

        Some(serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "workspace_status",
            "run_status": run_status,
            "analysis_status": analysis_status,
            "snapshot_age_ms": age_ms,
            "snapshot_duration_ms": duration_ms,
            "diagnostics": {
                "total": total_diagnostics,
                "files": files,
                "findings": raw_signals,
                "raw_signals": raw_signals,
                "canonical_items": canonical_items,
                "actionable_diagnostics": actionable_diagnostics,
                "seam_diagnostics": seam_diagnostics,
                "gap_artifacts": gap_artifacts,
                "actionable_gap_artifacts": actionable_gap_artifacts,
                "gap_artifact_rejections": gap_artifact_rejections,
            },
            "top_actionable_packet": top_actionable_packet,
            "top_limitation": top_limitation,
            "receipt_status_summary": receipt_status_summary,
            "report_paths": workspace_status_report_paths(),
            "refresh_command": REFRESH_COMMAND,
            "limits_note": "Static evidence only; advisory, not a gate decision.",
        }))
    }
}

fn workspace_status_report_paths() -> serde_json::Value {
    serde_json::json!({
        "actionable_gaps": "target/ripr/reports/actionable-gaps.json",
        "first_useful_action": DEFAULT_FIRST_USEFUL_ACTION_OUT,
        "gap_decision_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        "start_here": "target/ripr/reports/start-here.json",
    })
}

/// Compact receipt/outcome summary for `collect_workspace_status`.
/// Reuses the same artifact readers as `collect_receipt_status` so the
/// cockpit's single status call shows receipt state without a second
/// round-trip. Returns a JSON object or Null on any read/parse failure.
fn workspace_status_receipt_summary(
    root: &std::path::Path,
    snapshot: &AnalysisSnapshot,
) -> serde_json::Value {
    let top_gap = snapshot
        .gap_artifacts
        .iter()
        .find(|a| a.is_safe_projection_input() && a.is_actionable_gap());

    // receipt_status: movement from ledger.
    let ledger_path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
    let receipt_movement = top_gap
        .map(|artifact| receipt_status_from_ledger(&ledger_path, artifact).0)
        .unwrap_or_else(|| serde_json::Value::String("not_available".to_string()));

    // latest_attempt_outcome from swarm-attempt-ledger.json.
    let attempt_ledger_path = root.join("target/ripr/reports/swarm-attempt-ledger.json");
    let latest_attempt_outcome = read_latest_attempt_outcome(&attempt_ledger_path, top_gap);

    // route_quality_summary from route-quality.json.
    let route_quality_path = root.join("target/ripr/reports/route-quality.json");
    let route_quality_summary = read_route_quality_summary(&route_quality_path);

    serde_json::json!({
        "receipt_movement": receipt_movement,
        "latest_attempt_outcome": latest_attempt_outcome,
        "route_quality_summary": route_quality_summary,
    })
}

fn workspace_status_run_status(snapshot: &AnalysisSnapshot) -> &'static str {
    if snapshot
        .gap_artifact_rejections
        .iter()
        .any(|r| matches!(r, super::gap_artifacts::GapArtifactRejection::StaleArtifact))
    {
        return "stale";
    }
    if !snapshot.gap_artifact_rejections.is_empty() {
        return "cache_limited";
    }
    let has_static_limit = snapshot
        .findings
        .iter()
        .any(|f| f.static_limit_kind.is_some())
        || snapshot.gap_artifacts.iter().any(|a| a.has_static_limit());
    if has_static_limit {
        return "limited";
    }
    // Seam inventory was deferred on this interactive refresh (RIPR-SPEC-0105).
    // Diff-scoped findings are complete but seam evidence is absent. The
    // cockpit must NOT present this as "full" — use the disclosed deferral
    // status so the refresh_command affordance is shown instead.
    if snapshot.seams_deferred {
        return "seams_deferred";
    }
    "full"
}

fn workspace_status_top_actionable_packet(snapshot: &AnalysisSnapshot) -> serde_json::Value {
    let artifact = snapshot
        .gap_artifacts
        .iter()
        .find(|a| a.is_safe_projection_input() && a.is_actionable_gap());
    let Some(artifact) = artifact else {
        return serde_json::Value::Null;
    };
    let canonical_gap_id = artifact
        .identities
        .first()
        .and_then(|id| id.canonical_gap_id.as_deref())
        .unwrap_or("");
    let verify_command = artifact
        .verify_commands
        .first()
        .map(String::as_str)
        .unwrap_or("");
    let receipt_command = artifact
        .receipt_commands
        .first()
        .map(String::as_str)
        .unwrap_or("");
    let file = artifact
        .related_paths
        .first()
        .map(String::as_str)
        .unwrap_or("");
    let repair_kind = artifact.gap_state.as_deref().unwrap_or("actionable");
    serde_json::json!({
        "canonical_gap_id": canonical_gap_id,
        "file": file,
        "line": serde_json::Value::Null,
        "repair_kind": repair_kind,
        "verify_command": verify_command,
        "receipt_command": receipt_command,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct TopLimitationDto {
    status: &'static str,
    limitation_category: &'static str,
    run_status: &'static str,
    snapshot_id: Option<String>,
    input_identity: Option<String>,
    scope: &'static str,
    completeness: &'static str,
    why_not_actionable: String,
    recovery_route: &'static str,
    sample_sources: Vec<String>,
    selected_count: usize,
    total_count: usize,
    non_claims: Vec<&'static str>,
}

impl TopLimitationDto {
    fn into_json(self) -> serde_json::Value {
        let repair_route = self.recovery_route;
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "top_limitation",
            "status": self.status,
            "category": self.limitation_category,
            "limitation_category": self.limitation_category,
            "run_status": self.run_status,
            "snapshot_id": self.snapshot_id,
            "input_identity": self.input_identity,
            "scope": self.scope,
            "completeness": self.completeness,
            "repair_route": repair_route,
            "recovery_route": repair_route,
            "why_not_actionable": self.why_not_actionable,
            "sample_sources": self.sample_sources,
            "selected_count": self.selected_count,
            "total_count": self.total_count,
            "unlock_condition": repair_route,
            "non_claims": self.non_claims,
            "limits_note": "Static evidence only; advisory, not a gate decision.",
        })
    }
}

fn top_limitation_dto(
    health: &AnalysisHealth,
    snapshot: Option<&AnalysisSnapshot>,
    authority: &WorkspaceRootAuthority,
) -> TopLimitationDto {
    let snapshot_id = health.snapshot_id.clone();
    let input_identity = authority.input_identity();
    let common = |status: &'static str,
                  category: &'static str,
                  completeness: &'static str,
                  why_not_actionable: String,
                  recovery_route: &'static str,
                  sample_sources: Vec<String>,
                  selected_count: usize,
                  total_count: usize,
                  non_claims: Vec<&'static str>| TopLimitationDto {
        status,
        limitation_category: category,
        run_status: health.run_status(),
        snapshot_id: snapshot_id.clone(),
        input_identity: input_identity.clone(),
        scope: "workspace",
        completeness,
        why_not_actionable,
        recovery_route,
        sample_sources,
        selected_count,
        total_count,
        non_claims,
    };

    if !authority.allows_analysis() {
        let (status, why_not_actionable, recovery_route) = match authority.state {
            WorkspaceRootState::WorkspaceAmbiguous => (
                "workspace_ambiguous",
                "workspace root authority is ambiguous; analysis input is not selected",
                "select_root_and_restart",
            ),
            WorkspaceRootState::RootUnavailable => (
                "input_invalid",
                "workspace root authority is unavailable; analysis input is not selected",
                "select_root_and_restart",
            ),
            WorkspaceRootState::RootRemoved => (
                "input_invalid",
                "the selected workspace root was removed; analysis input is invalid",
                "select_root_and_restart",
            ),
            WorkspaceRootState::RootChanged => (
                "snapshot_stale",
                "workspace root changed; the retained analysis input is stale",
                "refresh",
            ),
            WorkspaceRootState::SelectedSingleRoot => (
                "input_invalid",
                "workspace root authority is not available for analysis",
                "refresh",
            ),
        };
        return common(
            status,
            status,
            "incomplete",
            why_not_actionable.to_string(),
            recovery_route,
            Vec::new(),
            1,
            1,
            vec![
                "not a repository-clean signal",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    let Some(snapshot) = snapshot else {
        let (status, why_not_actionable, recovery_route, completeness) = match health.state {
            AnalysisAttemptState::Queued => (
                "analysis_queued",
                "RIPR has queued an analysis attempt but has not produced a snapshot",
                "wait_for_analysis",
                "pending",
            ),
            AnalysisAttemptState::Running => (
                "analysis_running",
                "RIPR is analyzing the workspace and has not produced a snapshot",
                "wait_for_analysis",
                "pending",
            ),
            AnalysisAttemptState::Failed => (
                "analysis_failed",
                "analysis failed before a snapshot was available",
                "refresh",
                "incomplete",
            ),
            _ => (
                "no_snapshot",
                "RIPR has not completed an analysis snapshot yet",
                "refresh",
                "none",
            ),
        };
        return common(
            status,
            status,
            completeness,
            health
                .failure
                .as_ref()
                .map(|failure| format!("{}: {}", failure.kind, failure.message))
                .unwrap_or_else(|| why_not_actionable.to_string()),
            recovery_route,
            Vec::new(),
            1,
            1,
            vec![
                "not a repository-clean signal",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    };

    if health.state == AnalysisAttemptState::Failed {
        let why = health
            .failure
            .as_ref()
            .map(|failure| format!("the latest analysis failed: {}", failure.message))
            .unwrap_or_else(|| "the latest analysis failed; the retained snapshot is stale".into());
        return common(
            "analysis_failed_retained_snapshot",
            "analysis_failed_retained_snapshot",
            "stale",
            why,
            "refresh",
            snapshot_id.clone().into_iter().collect(),
            1,
            1,
            vec![
                "retained evidence is stale",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    if matches!(
        health.state,
        AnalysisAttemptState::Queued | AnalysisAttemptState::Running
    ) {
        let (status, why_not_actionable) = if health.state == AnalysisAttemptState::Queued {
            (
                "analysis_queued",
                "RIPR has queued a new analysis attempt; the retained snapshot is not current",
            )
        } else {
            (
                "analysis_running",
                "RIPR is analyzing the workspace; the retained snapshot is not current",
            )
        };
        return common(
            status,
            status,
            "pending",
            why_not_actionable.to_string(),
            "wait_for_analysis",
            snapshot_id.clone().into_iter().collect(),
            1,
            1,
            vec![
                "retained evidence is not current",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    let run_status = health.run_status();
    if run_status == "stale" {
        return common(
            "snapshot_stale",
            "snapshot_stale",
            "stale",
            "the retained analysis snapshot is not current for this workspace".to_string(),
            "refresh",
            snapshot_id.clone().into_iter().collect(),
            1,
            1,
            vec![
                "retained evidence is stale",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }
    if run_status == "seams_deferred" {
        return common(
            "seams_deferred",
            "seams_deferred",
            "deferred",
            "interactive analysis deferred the expensive seam inventory".to_string(),
            "refresh",
            Vec::new(),
            1,
            1,
            vec![
                "seam evidence is incomplete",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    if let Some(rejection) = top_gap_artifact_rejection(&snapshot.gap_artifact_rejections) {
        let category = rejection.as_str();
        let (repair_route, why_not_actionable) = workspace_status_rejection_repair(rejection);
        return common(
            "artifact_rejected",
            category,
            "limited",
            why_not_actionable.to_string(),
            repair_route,
            limitation_sample_sources(rejection),
            1,
            snapshot.gap_artifact_rejections.len(),
            limitation_non_claims(category),
        );
    }

    let static_limit_count = snapshot
        .findings
        .iter()
        .filter(|finding| finding.static_limit_kind.is_some())
        .count()
        + snapshot
            .gap_artifacts
            .iter()
            .filter(|artifact| artifact.has_static_limit())
            .count();
    if static_limit_count > 0 {
        return common(
            "canonical_static_limitation",
            "canonical_static_limitation",
            "limited",
            "producer-owned static evidence is limited; inspect the named limitation".to_string(),
            "inspect_full_evidence",
            snapshot
                .findings
                .iter()
                .filter(|finding| finding.static_limit_kind.is_some())
                .min_by_key(|finding| finding.id.as_str())
                .map(|finding| vec![finding.id.clone()])
                .unwrap_or_default(),
            static_limit_count,
            static_limit_count,
            vec![
                "not a repair packet",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    if run_status == "limited" || run_status == "cache_limited" {
        return common(
            "run_limited",
            "run_limited",
            "limited",
            "the current analysis run is limited and does not represent complete scope".to_string(),
            "refresh",
            Vec::new(),
            1,
            1,
            vec![
                "not a repository-clean signal",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    if snapshot.diagnostic_profile == LspDiagnosticProfile::Actionable
        && snapshot.finding_count() > 0
        && snapshot.actionable_diagnostic_count() == 0
    {
        return common(
            "no_actionable_item",
            "no_actionable_item",
            "complete",
            "the current actionable profile filtered findings without a bounded next action; inspect full evidence for detail".to_string(),
            "inspect_full_evidence",
            Vec::new(),
            snapshot.finding_count(),
            snapshot.finding_count(),
            vec![
                "not a repository-clean signal",
                "not test adequacy",
                "not runtime evidence",
            ],
        );
    }

    common(
        "no_active_limitation_in_current_scope",
        "no_active_limitation_in_current_scope",
        "complete",
        "no active limitation was reported in the current RIPR analysis scope".to_string(),
        "inspect_full_evidence",
        Vec::new(),
        0,
        0,
        vec![
            "not a repository-clean signal",
            "not test adequacy",
            "not runtime evidence",
        ],
    )
}

fn top_gap_artifact_rejection(
    rejections: &[super::gap_artifacts::GapArtifactRejection],
) -> Option<&super::gap_artifacts::GapArtifactRejection> {
    rejections.iter().min_by_key(|rejection| {
        format!(
            "{}:{:?}",
            rejection.as_str(),
            limitation_sample_sources(rejection)
        )
    })
}

#[cfg(test)]
mod top_limitation_selection_tests {
    use super::*;

    #[test]
    fn top_gap_artifact_rejection_is_order_independent() {
        use super::super::gap_artifacts::GapArtifactRejection;

        let first_order = vec![
            GapArtifactRejection::WrongRoot("/workspace/other".to_string()),
            GapArtifactRejection::DisabledLanguage("typescript".to_string()),
        ];
        let second_order = vec![
            GapArtifactRejection::DisabledLanguage("typescript".to_string()),
            GapArtifactRejection::WrongRoot("/workspace/other".to_string()),
        ];

        let selected = |rejections: &[GapArtifactRejection]| {
            top_gap_artifact_rejection(rejections)
                .map(|rejection| (rejection.as_str(), limitation_sample_sources(rejection)))
        };
        let expected = ("disabled_language", vec!["typescript".to_string()]);

        assert_eq!(selected(&first_order), Some(expected.clone()));
        assert_eq!(selected(&second_order), Some(expected));
    }
}

fn workspace_status_rejection_repair(
    rejection: &super::gap_artifacts::GapArtifactRejection,
) -> (&'static str, &'static str) {
    use super::gap_artifacts::GapArtifactRejection;
    match rejection {
        GapArtifactRejection::StaleArtifact => (
            "regenerate_gap_artifacts",
            "gap artifacts are stale; rerun ripr check to refresh",
        ),
        GapArtifactRejection::WrongRoot(_) => (
            "verify_workspace_root",
            "gap artifact root does not match workspace root",
        ),
        GapArtifactRejection::UnsupportedSchema(_) => (
            "upgrade_ripr",
            "gap artifact schema version is not supported by this ripr version",
        ),
        GapArtifactRejection::MalformedArtifact(_) => (
            "regenerate_gap_artifacts",
            "gap artifact is malformed; rerun ripr check to regenerate",
        ),
        GapArtifactRejection::MissingIdentity => (
            "regenerate_gap_artifacts",
            "gap artifact is missing a canonical identity; rerun ripr check",
        ),
        GapArtifactRejection::MalformedCommandPayload(_) => (
            "regenerate_gap_artifacts",
            "gap artifact command payload is malformed; rerun ripr check",
        ),
        GapArtifactRejection::OutOfWorkspacePath(_) => (
            "verify_workspace_root",
            "gap artifact references a path outside the workspace",
        ),
        GapArtifactRejection::DisabledLanguage(_) => (
            "enable_language_in_config",
            "gap artifact language is not enabled in ripr config",
        ),
        GapArtifactRejection::UnavailableLanguage(_) => (
            "upgrade_ripr",
            "gap artifact language is not available in this ripr build",
        ),
        GapArtifactRejection::UnsupportedStaticLimitKind(_) => (
            "upgrade_ripr",
            "gap artifact static_limit_kind is not recognized by this ripr version",
        ),
        GapArtifactRejection::UnsupportedKind(_) => (
            "upgrade_ripr",
            "gap artifact kind is not supported by this ripr version",
        ),
    }
}

/// Extract the String payload from rejection variants that carry one.
/// Unit and `&str`-reason variants emit an empty list.
/// gap_id-bearing sample sources are a deferred follow-up — the rejection does
/// not carry a gap_id.
fn limitation_sample_sources(
    rejection: &super::gap_artifacts::GapArtifactRejection,
) -> Vec<String> {
    use super::gap_artifacts::GapArtifactRejection;
    match rejection {
        GapArtifactRejection::DisabledLanguage(s)
        | GapArtifactRejection::MalformedCommandPayload(s)
        | GapArtifactRejection::OutOfWorkspacePath(s)
        | GapArtifactRejection::UnavailableLanguage(s)
        | GapArtifactRejection::UnsupportedKind(s)
        | GapArtifactRejection::UnsupportedSchema(s)
        | GapArtifactRejection::UnsupportedStaticLimitKind(s)
        | GapArtifactRejection::WrongRoot(s) => vec![s.clone()],
        GapArtifactRejection::MalformedArtifact(_)
        | GapArtifactRejection::MissingIdentity
        | GapArtifactRejection::StaleArtifact => vec![],
    }
}

/// Per-category static non-claims table.
/// Vocabulary: approved static-exposure terms only (exposed/weakly_exposed/etc.).
fn limitation_non_claims(category: &str) -> Vec<&'static str> {
    match category {
        "disabled_language" | "unavailable_language" => vec![
            "not a Rust repair packet",
            "does not indicate the behavior is reachable",
            "does not indicate tests are absent",
        ],
        "wrong_root" | "out_of_workspace_path" => vec![
            "not a repair packet",
            "path resolution required before exposure can be assessed",
        ],
        "stale_artifact"
        | "missing_identity"
        | "malformed_artifact"
        | "malformed_command_payload" => vec![
            "not a repair packet",
            "artifact regeneration required before exposure can be assessed",
        ],
        "unsupported_schema" | "unsupported_kind" | "unsupported_static_limit_kind" => vec![
            "not a repair packet",
            "ripr upgrade required before exposure can be assessed",
        ],
        _ => vec!["not a repair packet"],
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

const DEFAULT_ACTIONABLE_GAPS_OUT: &str = "target/ripr/reports/actionable-gaps.json";

impl Backend {
    fn collect_repair_packet(&self, arguments: &[LSPAny]) -> Option<LSPAny> {
        let health = self.analysis_health_snapshot();
        if !health.allows_current_repairs() {
            return Some(repair_packet_sentinel("analysis_snapshot_stale"));
        }
        let authority = self.workspace_root_authority();
        if !authority.allows_analysis() {
            return Some(repair_packet_sentinel(root_authority_block_reason(
                &authority.state,
            )));
        }
        let root = self.root.lock().ok()?.clone();
        let gap_id_arg = arguments
            .first()
            .and_then(|v| v.as_object())
            .and_then(|obj| obj.get("gap_id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        // Try actionable-gaps.json first (preferred: projection-validated).
        let actionable_path = absolute_context_path(&root, Path::new(DEFAULT_ACTIONABLE_GAPS_OUT));
        if let Some(result) =
            collect_repair_packet_from_actionable_gaps(&actionable_path, gap_id_arg.as_deref())
        {
            return Some(result);
        }

        // Fallback: gap-decision-ledger.json using the existing GapRecord machinery.
        let ledger_path = absolute_context_path(&root, Path::new(DEFAULT_GAP_DECISION_LEDGER_OUT));
        collect_repair_packet_from_ledger(&ledger_path, gap_id_arg.as_deref())
    }

    fn collect_top_limitation(&self) -> Option<LSPAny> {
        let health = self.analysis_health_snapshot();
        let snapshot = self.latest_analysis.lock().ok()?.clone();
        let health = snapshot
            .as_ref()
            .map(|snapshot| self.effective_health_for_snapshot(health.clone(), snapshot))
            .unwrap_or(health);
        let authority = self.workspace_root_authority();
        Some(top_limitation_dto(&health, snapshot.as_ref(), &authority).into_json())
    }

    fn collect_receipt_status(&self) -> Option<LSPAny> {
        let authority = self.workspace_root_authority();
        if !authority.allows_analysis() {
            return Some(root_authority_receipt_status(&authority.state));
        }
        let root = self.root.lock().ok()?.clone();
        let snapshot = match self.latest_analysis.lock().ok()? {
            guard if guard.is_none() => {
                // No snapshot yet — all fields not_available.
                return Some(serde_json::json!({
                    "schema_version": "0.1",
                    "tool": "ripr",
                    "kind": "receipt_status",
                    "status": "no_snapshot",
                    "receipt_status": "not_available",
                    "missing_receipt_reason": "not_available",
                    "copy_receipt_command": "not_available",
                    "open_attempt_ledger": "not_available",
                    "latest_attempt_outcome": "not_available",
                    "route_quality_summary": "not_available",
                    "limits_note": "Static evidence only; advisory, not a gate decision.",
                }));
            }
            guard => guard.clone()?,
        };

        // Derive receipt_status fields from the gap-decision-ledger via the
        // top actionable gap artifact in the snapshot. All counts and outcomes
        // come from real artifact reads — never fabricated.
        let top_gap = snapshot
            .gap_artifacts
            .iter()
            .find(|a| a.is_safe_projection_input() && a.is_actionable_gap());

        // receipt_status: real movement from the top gap's receipt object
        // (sourced from GapRecord.receipt). movement is the only field
        // populated in production v0.1; state is best-effort.
        let (receipt_status_val, missing_receipt_reason_val, copy_receipt_cmd_val) =
            collect_receipt_status_fields(&root, top_gap, &snapshot);

        // open_attempt_ledger: path to swarm-attempt-ledger.json if it exists.
        let attempt_ledger_path = root.join("target/ripr/reports/swarm-attempt-ledger.json");
        let open_attempt_ledger_val = if attempt_ledger_path.is_file() {
            serde_json::Value::String(display_lsp_path(&attempt_ledger_path))
        } else {
            serde_json::Value::String("not_available".to_string())
        };

        // latest_attempt_outcome: read swarm-attempt-ledger.json and surface
        // the latest attempt's outcome for the top actionable gap. When the
        // artifact is absent → not_available (absence != no outcome).
        let latest_attempt_outcome_val = read_latest_attempt_outcome(&attempt_ledger_path, top_gap);

        // route_quality_summary: read route-quality.json (RIPR-SPEC-0080
        // output), surface compact summary. not_available when absent or
        // when status = "blocked".
        let route_quality_path = root.join("target/ripr/reports/route-quality.json");
        let route_quality_summary_val = read_route_quality_summary(&route_quality_path);

        Some(serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "receipt_status",
            "receipt_status": receipt_status_val,
            "missing_receipt_reason": missing_receipt_reason_val,
            "copy_receipt_command": copy_receipt_cmd_val,
            "open_attempt_ledger": open_attempt_ledger_val,
            "latest_attempt_outcome": latest_attempt_outcome_val,
            "route_quality_summary": route_quality_summary_val,
            "report_paths": workspace_receipt_status_report_paths(),
            "limits_note": "Static evidence only; advisory, not a gate decision.",
        }))
    }
}

/// Derive `receipt_status`, `missing_receipt_reason`, and `copy_receipt_command`
/// from the top actionable gap artifact plus the snapshot's first-safe-receipt-command
/// path.
///
/// Honesty rules:
/// - `copy_receipt_command` is only emitted when the packet is COMPLETE
///   (verify_command + receipt_command both present). A limitation or
///   incomplete packet MUST NOT surface a repair receipt command.
/// - `receipt_status` reports the real movement value (or "not_available"
///   when no gap artifact or receipt object exists).
fn collect_receipt_status_fields(
    root: &std::path::Path,
    top_gap: Option<&super::gap_artifacts::ValidatedGapArtifact>,
    snapshot: &super::state::AnalysisSnapshot,
) -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let not_available = serde_json::Value::String("not_available".to_string());

    // Top-level limitation present → no repair receipt command (RIPR-SPEC-0076
    // harmonization: limitations must never show a repair receipt command).
    let has_limitation = !snapshot.gap_artifact_rejections.is_empty();

    let Some(artifact) = top_gap else {
        return (not_available.clone(), not_available.clone(), not_available);
    };

    // receipt_status: from the gap-decision-ledger receipt.movement field.
    // Read directly from the artifact's receipt object via the ledger.
    // For LSP we can derive from the artifact's receipt_commands presence
    // as a proxy; the real movement comes from the ledger receipt record.
    let ledger_path =
        root.join(crate::output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_OUT);
    let (receipt_status_val, missing_receipt_reason_val) =
        receipt_status_from_ledger(&ledger_path, artifact);

    // copy_receipt_command: only for complete packets (verify + receipt
    // commands both present). Incomplete packets → not_available.
    let copy_receipt_cmd_val = if has_limitation {
        // Limitation surface — never show receipt command.
        not_available
    } else {
        let has_verify = !artifact.verify_commands.is_empty();
        let has_receipt = !artifact.receipt_commands.is_empty();
        if has_verify && has_receipt {
            // Use the existing first_safe_receipt_command path from actions.rs
            // by reading it directly from the artifact.
            let cmd = artifact
                .receipt_commands
                .first()
                .map(String::as_str)
                .unwrap_or("");
            if super::gap_artifacts::command_payload_is_safe(root, cmd) {
                serde_json::Value::String(cmd.to_string())
            } else {
                serde_json::Value::String("not_available".to_string())
            }
        } else {
            // Incomplete packet — no receipt command shown.
            serde_json::Value::String("not_available".to_string())
        }
    };

    (
        receipt_status_val,
        missing_receipt_reason_val,
        copy_receipt_cmd_val,
    )
}

/// Read the gap-decision-ledger to get the real receipt movement + missing_reason
/// for the given gap artifact. Falls back to not_available on any read/parse error.
fn receipt_status_from_ledger(
    ledger_path: &std::path::Path,
    artifact: &super::gap_artifacts::ValidatedGapArtifact,
) -> (serde_json::Value, serde_json::Value) {
    let not_available = "not_available".to_string();

    let contents = match fs::read_to_string(ledger_path) {
        Ok(c) => c,
        Err(_) => {
            return (
                serde_json::Value::String(not_available.clone()),
                serde_json::Value::String(not_available),
            );
        }
    };
    let records = match crate::output::gap_decision_ledger::parse_gap_records_json(&contents) {
        Ok(r) => r,
        Err(_) => {
            return (
                serde_json::Value::String(not_available.clone()),
                serde_json::Value::String(not_available),
            );
        }
    };

    // Match the ledger record to the top artifact using canonical_gap_id.
    let canonical_gap_id = artifact
        .identities
        .first()
        .and_then(|id| id.canonical_gap_id.as_deref())
        .unwrap_or("");

    let record = records.iter().find(|r| {
        r.canonical_gap_id == canonical_gap_id
            || (!canonical_gap_id.is_empty() && r.gap_id == canonical_gap_id)
    });

    let Some(record) = record else {
        // The artifact exists in the snapshot but the ledger doesn't have a
        // matching record — movement is genuinely unknown, not zero.
        return (
            serde_json::Value::String(not_available.clone()),
            serde_json::Value::String(not_available),
        );
    };

    // receipt.movement is the only reliably-populated field in production.
    let movement_val = match record.receipt.as_ref().and_then(|r| r.movement.as_deref()) {
        Some(m) => serde_json::Value::String(m.to_string()),
        None => serde_json::Value::String(not_available.clone()),
    };

    // missing_receipt_reason: only meaningful when the gap is actionable
    // but has no receipt. Otherwise not_available.
    let missing_receipt_reason_val = if record.gap_state == "actionable"
        && record
            .receipt
            .as_ref()
            .and_then(|r| r.movement.as_deref())
            .is_none()
        && record.receipt_command.is_some()
    {
        // Receipt command exists but no movement yet — emit honest "no_attempt_recorded".
        serde_json::Value::String("no_attempt_recorded".to_string())
    } else {
        serde_json::Value::String(not_available)
    };

    (movement_val, missing_receipt_reason_val)
}

/// Read swarm-attempt-ledger.json and surface the latest attempt's outcome
/// for the canonical_gap_id of the top actionable gap. Returns not_available
/// when the artifact is absent (absence != no outcome) or when no matching
/// attempt entry is found.
fn read_latest_attempt_outcome(
    ledger_path: &std::path::Path,
    top_gap: Option<&super::gap_artifacts::ValidatedGapArtifact>,
) -> serde_json::Value {
    let not_available = serde_json::Value::String("not_available".to_string());

    // Artifact absent → not_available (do not fabricate).
    let contents = match fs::read_to_string(ledger_path) {
        Ok(c) => c,
        Err(_) => return not_available,
    };
    let report: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return not_available,
    };

    let canonical_gap_id = top_gap
        .and_then(|a| a.identities.first())
        .and_then(|id| id.canonical_gap_id.as_deref())
        .unwrap_or("");

    // latest_attempts is the preferred section; fall back to attempts.
    let attempts_val = report
        .get("latest_attempts")
        .and_then(|v| v.as_array())
        .or_else(|| report.get("attempts").and_then(|v| v.as_array()));

    let Some(attempts) = attempts_val else {
        return not_available;
    };

    let entry = if canonical_gap_id.is_empty() {
        attempts.first()
    } else {
        attempts
            .iter()
            .find(|entry| {
                entry
                    .get("canonical_gap_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|cid| cid == canonical_gap_id)
            })
            .or_else(|| attempts.first())
    };

    entry
        .and_then(|e| e.get("outcome"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::Value::String(s.to_string()))
        .unwrap_or(not_available)
}

/// Read route-quality.json (RIPR-SPEC-0080 output) and surface a compact
/// summary. Returns not_available when the artifact is absent OR when
/// status = "blocked" (no real data to summarize).
fn read_route_quality_summary(path: &std::path::Path) -> serde_json::Value {
    let not_available = serde_json::Value::String("not_available".to_string());

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return not_available,
    };
    let report: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return not_available,
    };

    // If status = "blocked", the rows are empty and there's nothing real to
    // summarize — emit not_available to be honest.
    if report.get("status").and_then(|v| v.as_str()) == Some("blocked") {
        return not_available;
    }

    // Produce a compact summary: top repair_kind rows from the latest array.
    let rows = report
        .get("repair_route_quality_latest")
        .and_then(|v| v.as_array());

    let Some(rows) = rows else {
        return not_available;
    };

    if rows.is_empty() {
        return not_available;
    }

    // Emit up to 3 top rows, carrying repair_kind and success_rate.
    let summary_rows: Vec<serde_json::Value> = rows
        .iter()
        .take(3)
        .map(|row| {
            let repair_kind = row
                .get("repair_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let success_rate = row
                .get("repair_kind_success_rate")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let attempted = row
                .get("repair_kind_attempted")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "repair_kind": repair_kind,
                "attempted": attempted,
                "success_rate": success_rate,
            })
        })
        .collect();

    serde_json::json!({
        "report": "route-quality",
        "status": report.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "top_repair_kind_rows": summary_rows,
    })
}

fn workspace_receipt_status_report_paths() -> serde_json::Value {
    serde_json::json!({
        "gap_decision_ledger": crate::output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_OUT,
        "swarm_attempt_ledger": "target/ripr/reports/swarm-attempt-ledger.json",
        "route_quality": "target/ripr/reports/route-quality.json",
    })
}

fn collect_repair_packet_from_actionable_gaps(path: &Path, gap_id: Option<&str>) -> Option<LSPAny> {
    let contents = fs::read_to_string(path).ok()?;
    let report: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let packets = report.get("packets").and_then(|v| v.as_array())?;
    let packet = if let Some(id) = gap_id {
        packets
            .iter()
            .find(|p| {
                p.get("canonical_gap_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|cid| cid == id)
            })
            .or_else(|| packets.first())?
    } else {
        packets
            .iter()
            .find(|p| {
                p.get("gap_state")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == "actionable")
            })
            .or_else(|| packets.first())?
    };

    // Require the packet to be actionable to emit a complete repair packet.
    if packet
        .get("gap_state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        != "actionable"
    {
        return Some(repair_packet_sentinel(
            "gap is not actionable in actionable-gaps.json",
        ));
    }

    validate_and_render_actionable_gap_packet(packet)
}

fn validate_and_render_actionable_gap_packet(packet: &serde_json::Value) -> Option<LSPAny> {
    let str_field = |key: &str| -> Option<String> {
        packet
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    };

    let canonical_gap_id = match str_field("canonical_gap_id") {
        Some(v) => v,
        None => {
            return Some(repair_packet_sentinel(
                "actionable packet is missing canonical_gap_id",
            ));
        }
    };
    let repair_kind = match str_field("repair_kind") {
        Some(v) => v,
        None => {
            return Some(repair_packet_sentinel(
                "actionable packet is missing repair_kind",
            ));
        }
    };
    let verify_command = match str_field("verify_command") {
        Some(v) => v,
        None => {
            return Some(repair_packet_sentinel(
                "actionable packet is missing verify_command",
            ));
        }
    };
    let receipt_command = match str_field("receipt_command") {
        Some(v) => v,
        None => {
            return Some(repair_packet_sentinel(
                "actionable packet is missing receipt_command",
            ));
        }
    };

    let allowed_edit_surface: Vec<serde_json::Value> = packet
        .get("allowed_edit_surface")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if allowed_edit_surface.is_empty() {
        return Some(repair_packet_sentinel(
            "actionable packet is missing allowed_edit_surface",
        ));
    }

    let must_not_change: Vec<serde_json::Value> = packet
        .get("must_not_change")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if must_not_change.is_empty() {
        return Some(repair_packet_sentinel(
            "actionable packet is missing must_not_change",
        ));
    }

    let raw_evidence_refs: Vec<serde_json::Value> = packet
        .get("raw_evidence_refs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if raw_evidence_refs.is_empty() {
        return Some(repair_packet_sentinel(
            "actionable packet is missing raw_evidence_refs",
        ));
    }

    let confidence = packet
        .get("confidence_basis")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("static_only")
        .to_owned();

    // Derive language from raw_evidence_refs or canonical_gap_id.
    let language = raw_evidence_refs
        .iter()
        .find_map(|r| {
            r.get("language")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default();

    // Resolve source location from primary_anchor — never fabricate.
    let anchor = packet.get("primary_anchor");
    let anchor_file = anchor
        .and_then(|a| a.get("file"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let anchor_line = anchor
        .and_then(|a| a.get("line"))
        .and_then(|v| v.as_u64())
        .filter(|&n| n > 0);

    let source_location = match (anchor_file, anchor_line) {
        (Some(file), Some(line)) => serde_json::json!({ "file": file, "line": line }),
        (Some(file), None) => serde_json::json!({
            "status": "source_location_unresolved",
            "file": file,
        }),
        _ => serde_json::json!({ "status": "source_location_unresolved" }),
    };

    let result = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "repair_packet",
        "canonical_gap_id": canonical_gap_id,
        "language": language,
        "repair_kind": repair_kind,
        "source_location": source_location,
        "allowed_edit_surface": allowed_edit_surface,
        "verify_command": verify_command,
        "receipt_command": receipt_command,
        "must_not_change": must_not_change,
        "raw_evidence_refs": raw_evidence_refs,
        "confidence": confidence,
        "limits_note": "Static evidence only; advisory, not a gate decision.",
    });
    serde_json::from_value(result).ok()
}

fn collect_repair_packet_from_ledger(path: &Path, gap_id: Option<&str>) -> Option<LSPAny> {
    let contents = fs::read_to_string(path).ok()?;
    let records = parse_gap_records_json(&contents).ok()?;
    let record = if let Some(id) = gap_id {
        records
            .iter()
            .find(|r| r.gap_id == id || r.canonical_gap_id == id)?
    } else {
        records.iter().find(|r| r.gap_state == "actionable")?
    };

    // Use the existing validator for completeness gate.
    if let Err(reason) = validate_agent_gap_record_packet(record) {
        return Some(serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "repair_packet",
            "status": "not_actionable_or_incomplete",
            "reason": reason,
        }));
    }

    let route = record.repair_route.as_ref()?;
    let verify_command = record.verification_commands.first()?.clone();
    let receipt_command = record.receipt_command.as_deref().map(ToOwned::to_owned)?;
    let allowed_edit_surface =
        crate::output::agent_seam_packets::allowed_edit_surface_for_gap_route(route);
    let must_not_change: Vec<String> =
        crate::output::agent_seam_packets::gap_record_packet_do_not_do(record);

    let anchor = record.anchor.as_ref();
    let anchor_file = anchor
        .and_then(|a| a.file.as_deref())
        .filter(|s| !s.is_empty())
        .map(crate::output::path::display_path_text);
    let anchor_line = anchor.and_then(|a| a.line).filter(|&n| n > 0);
    let source_location = match (anchor_file, anchor_line) {
        (Some(file), Some(line)) => serde_json::json!({ "file": file, "line": line }),
        (Some(file), None) => serde_json::json!({
            "status": "source_location_unresolved",
            "file": file,
        }),
        _ => serde_json::json!({ "status": "source_location_unresolved" }),
    };

    let canonical_gap_id = if record.canonical_gap_id.trim().is_empty() {
        &record.gap_id
    } else {
        &record.canonical_gap_id
    };

    let result = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "repair_packet",
        "canonical_gap_id": canonical_gap_id,
        "language": &record.language,
        "repair_kind": route.route_kind.as_str(),
        "source_location": source_location,
        "allowed_edit_surface": allowed_edit_surface,
        "verify_command": verify_command,
        "receipt_command": receipt_command,
        "must_not_change": must_not_change,
        "raw_evidence_refs": &record.evidence_ids,
        "confidence": "static_only",
        "limits_note": "Static evidence only; advisory, not a gate decision.",
    });
    serde_json::from_value(result).ok()
}

fn repair_packet_sentinel(reason: &str) -> LSPAny {
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "repair_packet",
        "status": "not_actionable_or_incomplete",
        "reason": reason,
    })
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

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
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let process_id = std::process::id();
        let root = std::env::temp_dir().join(format!(
            "ripr-lsp-gap-record-context-{process_id}-{stamp}-{sequence}"
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
      "receipt_command": "ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json",
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
    fn absolute_context_path_keeps_absolute_paths_and_joins_relative_paths() -> Result<(), String> {
        // Use the host platform's temp_dir to produce an absolute path
        // without embedding a platform-specific literal — the policy gate
        // rejects literal Windows-drive paths committed to repository docs.
        let root = std::env::temp_dir();
        let already_absolute = root.join("already-absolute.json");
        if !already_absolute.is_absolute() {
            return Err(format!(
                "expected temp_dir-derived path to be absolute, got {}",
                already_absolute.display()
            ));
        }

        assert_eq!(
            absolute_context_path(&root, &already_absolute),
            already_absolute
        );
        assert_eq!(
            absolute_context_path(&root, Path::new("nested/file.json")),
            root.join("nested/file.json")
        );
        Ok(())
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
