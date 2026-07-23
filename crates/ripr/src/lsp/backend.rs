use super::AnalysisStatusNotification;
use super::actions::code_action_response;
use super::capabilities::{
    ConfigurationMode, WorkspaceRootResolution, initialize_result_for_client,
    root_from_initialize_params, workspace_folder_set_from_initialize_params,
};
use super::client_features::ClientFeatureProfile;
use super::config::{LspAnalysisConfig, validated_pulled_options};
use super::diagnostics::{
    DiagnosticBatch, DiagnosticRefreshPlan, DiagnosticResultIdCache, WorkspaceDiagnostics,
    diagnostic_refresh_plan, take_all_uris,
};
use super::hover::{
    classified_seam_hover_response, diagnostic_at_position, diagnostic_covers_position,
    diagnostic_hover_response, finding_hover_response, hover_response, hover_with_snapshot_status,
};
use super::lens::{LensViewIdentity, code_lens_response, lens_view_identity};
use super::payload_bounds::{
    check_execute_command_arguments, check_initialization_options, check_previous_result_ids,
};
use super::progress::{AnalysisProgressEnd, AnalysisProgressPhase, AnalysisProgressTracker};
use super::refresh_scheduler::{
    RefreshAttemptOutcome, RefreshDecision, RefreshReason, RefreshRequest, RefreshScheduler,
    RefreshScope,
};
use super::state::{
    AnalysisAttemptState, AnalysisFailure, AnalysisHealth, AnalysisSnapshot, ConfigPullState,
    DocumentStalenessReason, DocumentStore, QuarantineTransition, WorkspaceFolderEventRejection,
    WorkspaceFolderSelection, WorkspaceFolderSet, WorkspaceRootAuthority, WorkspaceRootState,
    content_digest, format_duration,
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
use crate::lsp::diagnostic_budget::DiagnosticDeliveryOutcome;
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
use std::time::{Duration, Instant};

/// Debounce window for interactive refresh triggers (did_open, did_save,
/// did_close). Rapid saves within this window collapse into a single
/// analysis run instead of canceling and re-queuing on every keystroke.
/// Explicit refresh and config reload bypass the debounce (#1908).
const INTERACTIVE_REFRESH_DEBOUNCE: Duration = Duration::from_millis(200);
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::notification::LogTrace;
use tower_lsp_server::ls_types::{
    CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, ConfigurationItem, Diagnostic,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, ExecuteCommandParams, FileEvent, Hover, HoverParams,
    InitializeParams, InitializeResult, InitializedParams, LSPAny, LogTraceParams, MessageType,
    Registration, RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport,
    TraceValue, UnchangedDocumentDiagnosticReport, Uri, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport, WorkspaceUnchangedDocumentDiagnosticReport,
};
use tower_lsp_server::{Client, LanguageServer};

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    workspace_root: Mutex<WorkspaceRootAuthority>,
    workspace_root_epoch: AtomicU64,
    workspace_root_transition: AsyncMutex<()>,
    /// The stored workspace-folder-set identity (#2036, RIPR-SPEC-0139):
    /// `didChangeWorkspaceFolders` deltas apply to this set, and the
    /// optional full-list reconciliation query is a separately versioned
    /// confirmation step bound to the folder-set epoch.
    workspace_folders: Mutex<WorkspaceFolderSet>,
    documents: Mutex<DocumentStore>,
    saved_content_digests: Mutex<BTreeMap<tower_lsp_server::ls_types::Uri, String>>,
    analysis_config: Mutex<LspAnalysisConfig>,
    configuration_failure: Mutex<Option<AnalysisFailure>>,
    /// Negotiated session-configuration transport (#2031, RIPR-SPEC-0136).
    configuration_mode: Mutex<ConfigurationMode>,
    /// The immutable typed client-feature profile (#1987, RIPR-SPEC-0143):
    /// parsed exactly once at `initialize` and the one authority for what
    /// the client advertised. The negotiated session fields below are
    /// populated from it; the bounded status projection is rendered from it.
    client_features: Mutex<ClientFeatureProfile>,
    /// Epoch guarding pull responses: `workspace/didChangeConfiguration` in
    /// pull mode bumps it, and a response arriving for an older epoch is
    /// dropped. Mirrors `workspace_root_epoch`.
    config_pull_epoch: AtomicU64,
    config_pull_state: Mutex<ConfigPullState>,
    /// Coalesces pull scheduling to one in-flight request plus at most one
    /// queued re-pull for the latest epoch.
    config_pull_coordinator: Mutex<ConfigPullCoordinator>,
    /// The root the currently retained pulled layer was scoped to. Pulled
    /// settings are valid only for that root; any transition landing on a
    /// different analysis-capable root must re-pull. Compared rather than
    /// derived from transition deltas so a direct A -> B switch (which
    /// rewrites to a non-analyzable `RootChanged` state and is re-selected
    /// later with `root_changed == false`) is still caught.
    config_pull_scope_root: Mutex<Option<PathBuf>>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    last_diagnostics: Mutex<BTreeMap<Uri, Vec<Diagnostic>>>,
    latest_analysis: Mutex<Option<Arc<AnalysisSnapshot>>>,
    diagnostic_result_ids: Mutex<Option<Arc<DiagnosticResultIdCache>>>,
    analysis_health: Mutex<AnalysisHealth>,
    pull_diagnostics: Mutex<bool>,
    /// Session-local standard trace level (`$/setTrace`, #2035,
    /// RIPR-SPEC-0137). Volatile observability state only: it never enters
    /// snapshot, input-identity, diagnostic, action, command, status, or
    /// receipt state, and it is never read from refresh or identity paths.
    trace: Mutex<TraceValue>,
    diagnostic_refresh_support: Mutex<bool>,
    /// Negotiated `workspace.codeLens.refreshSupport` (#2032, RIPR-SPEC-0138).
    code_lens_refresh_support: Mutex<bool>,
    /// The lens-view identity covered by the last `workspace/codeLens/refresh`
    /// request. Compared per committed snapshot; a request is sent only when
    /// the visible lens view changed. `None` until the first commit.
    last_lens_view_identity: Mutex<Option<LensViewIdentity>>,
    dynamic_file_watch_registration: Mutex<bool>,
    /// The degradation signature covered by the last `window/logMessage`
    /// component warning (#1997, RIPR-SPEC-0141). Compared per committed
    /// snapshot: a byte-identical repeated degradation warns once, a new
    /// signature warns again, and a cleared signature logs one recovery line.
    last_component_degradation: Mutex<Option<String>>,
    refresh_scheduler: RefreshScheduler,
    workspace_revision: Mutex<u64>,
    refresh_idle: Notify,
    pub(super) progress: Arc<AnalysisProgressTracker>,
}

#[derive(Default)]
struct ConfigPullCoordinator {
    in_flight: bool,
    queued: bool,
}

pub(super) struct RefreshTransaction {
    pub(super) plan: DiagnosticRefreshPlan,
    pub(super) snapshot: AnalysisSnapshot,
    pub(super) previous_diagnostics: BTreeMap<Uri, Vec<Diagnostic>>,
    /// The analyzed saved-content identity this transaction would record
    /// for each open document, computed at prepare time from the persisted
    /// bytes the analysis read (#1970). Applied to the document store only
    /// by `commit_refresh_snapshot`, so a superseded or failed transaction
    /// never advances document identities. Publication reads it to decide
    /// quarantine against the identity this snapshot will carry.
    pub(super) pending_analyzed: BTreeMap<Uri, Option<String>>,
    /// Documents that are currently clean but enter quarantine under the
    /// pending identity. They must be withdrawn during publication even when
    /// the plan considers their diagnostics unchanged.
    pub(super) pending_entered: Vec<Uri>,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            root: Mutex::new(root.clone()),
            workspace_root: Mutex::new(WorkspaceRootAuthority::unavailable(
                "workspace root authority is awaiting initialization",
            )),
            workspace_root_epoch: AtomicU64::new(0),
            workspace_root_transition: AsyncMutex::new(()),
            workspace_folders: Mutex::new(WorkspaceFolderSet::default()),
            documents: Mutex::new(DocumentStore::default()),
            saved_content_digests: Mutex::new(BTreeMap::new()),
            analysis_config: Mutex::new(LspAnalysisConfig::default()),
            configuration_failure: Mutex::new(None),
            configuration_mode: Mutex::new(ConfigurationMode::InitializationOnly),
            client_features: Mutex::new(ClientFeatureProfile::unsupported()),
            config_pull_epoch: AtomicU64::new(0),
            config_pull_state: Mutex::new(ConfigPullState::NotApplicable),
            config_pull_coordinator: Mutex::new(ConfigPullCoordinator::default()),
            config_pull_scope_root: Mutex::new(None),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            last_diagnostics: Mutex::new(BTreeMap::new()),
            latest_analysis: Mutex::new(None),
            diagnostic_result_ids: Mutex::new(None),
            analysis_health: Mutex::new(AnalysisHealth::default()),
            pull_diagnostics: Mutex::new(false),
            trace: Mutex::new(TraceValue::Off),
            diagnostic_refresh_support: Mutex::new(false),
            code_lens_refresh_support: Mutex::new(false),
            last_lens_view_identity: Mutex::new(None),
            dynamic_file_watch_registration: Mutex::new(false),
            last_component_degradation: Mutex::new(None),
            refresh_scheduler: RefreshScheduler::default(),
            workspace_revision: Mutex::new(0),
            refresh_idle: Notify::new(),
            progress: Arc::new(AnalysisProgressTracker::new(client.clone())),
            client,
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
        // Debounce interactive triggers (did_open/did_save/did_close) so
        // rapid saves collapse into one analysis run. Explicit refresh and
        // config reload bypass the debounce (#1908).
        //
        // The collapse itself is handled by the refresh scheduler:
        // `refresh_scheduler.request()` deduplicates matching active/pending
        // work before incrementing `next_generation`, so rapid equivalent
        // saves do not each obtain a new generation. The sleep here just
        // delays the request so a second save arriving within the window
        // can coalesce with the first rather than triggering a cancel +
        // re-queue cycle on the scheduler.
        //
        // The earlier version of this block used a `tokio::select!` arm on
        // `refresh_idle.notified()` to "cancel" when a superseding
        // notification arrived. That arm was dead code for the described
        // race: `notify_waiters` is called when an in-flight analysis
        // finishes and when a root-authority transition completes — not
        // when a new save arrives. The dedup never depended on it.
        // Removed in the post-merge review of #2041.
        let is_interactive = matches!(
            reason,
            RefreshReason::DidOpen | RefreshReason::DidSave | RefreshReason::DidClose
        );
        if is_interactive {
            tokio::time::sleep(INTERACTIVE_REFRESH_DEBOUNCE).await;
            // Re-check preconditions after the debounce window — config or
            // authority may have changed during the wait.
            if self.configuration_failure().is_some() {
                return;
            }
            let post_authority = self.workspace_root_authority();
            if !post_authority.allows_analysis() {
                return;
            }
        }
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
        self.emit_progress_for_decision(&decision).await;
        let mut request = match decision {
            RefreshDecision::Start(request) => {
                let request = *request;
                self.mark_attempt_queued(&request);
                self.publish_analysis_status().await;
                request
            }
            RefreshDecision::Queued { generation, .. } => {
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
            self.end_progress_for_attempt(&request, outcome).await;
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
            self.progress
                .transition_to_analyzing(request.generation)
                .await;
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
        self.log_refresh_started(request).await;
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
            pending_analyzed,
            pending_entered,
        } = transaction;
        // Bounded report at a real phase boundary: analysis produced a
        // consistent snapshot and diagnostic publication is next.
        self.progress.report_publishing(request.generation).await;
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
            // Read the stored delivery selection computed at refresh-transaction
            // prepare time (#1973). The push path no longer evaluates the
            // budget at publish time; it applies the same stored outcome the
            // pull handlers serve. The full budget result is retained: when
            // the budget trims diagnostics, the client is told the
            // publication is partial (bounded omission summary); when the
            // budget itself failed, the unfiltered fallback is named as a
            // partial state instead of passing silently.
            let selection = match &snapshot.delivery_selection {
                Some(selection) => Arc::clone(selection),
                // Defensive only: prepare always computes the selection
                // before publication. Never publish without one.
                None => compute_delivery_selection(&snapshot),
            };
            // Distinguish "budget computation failed" (None → publish
            // everything, no honest budget to apply) from "computation
            // succeeded with an empty selection" (Some(empty) → publish
            // nothing: the budget legitimately dropped every item).
            match &selection.outcome {
                DiagnosticDeliveryOutcome::Applied {
                    result,
                    document_by_canonical_id,
                    ..
                } => {
                    if let Some(disclosure) = push_budget_omission_disclosure(
                        result,
                        &selection.budget,
                        document_by_canonical_id,
                    ) {
                        self.client
                            .log_message(MessageType::WARNING, disclosure)
                            .await;
                    }
                    if result.selected.is_empty() && result.total_canonical_items > 0 {
                        // The publish-everything fallback rule fires when the
                        // selection collapses; name it instead of silently
                        // presenting an unenforced delivery as budgeted.
                        self.client
                            .log_message(
                                MessageType::WARNING,
                                push_budget_zero_selection_log_message(result),
                            )
                            .await;
                    }
                }
                DiagnosticDeliveryOutcome::Unavailable { detail, .. } => {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            push_budget_unavailable_log_message(detail),
                        )
                        .await;
                }
            }

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
                // The stored selection is the membership authority: an
                // unavailable budget publishes the batch unfiltered (named by
                // the fallback disclosure); a computed selection is applied
                // strictly, so zero selected means zero published. A
                // document quarantined under this transaction's pending
                // analyzed identity is withdrawn instead (#1970): its
                // saved-state line identity no longer matches the client's
                // buffer.
                let diagnostics_to_publish =
                    selection.diagnostics_for_document(batch.uri.as_str(), &batch.diagnostics);
                self.publish_served_diagnostics_for_transaction(
                    &batch.uri,
                    diagnostics_to_publish,
                    &pending_analyzed,
                )
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
            // Documents that enter quarantine under this transaction's
            // pending identity are withdrawn even when the plan considers
            // their diagnostics unchanged: the analyzed saved content moved
            // under them. Their withdrawal is disclosed directly — the
            // quarantine episode only exists once the snapshot commits, and
            // the commit marks these URIs as already disclosed.
            for uri in &pending_entered {
                if plan
                    .publish_batches
                    .iter()
                    .any(|batch| file_uris_match(&batch.uri, uri))
                {
                    continue;
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
                if !snapshot.served_diagnostics_for_uri(uri).is_empty()
                    && let Some((path, reason)) =
                        self.document_quarantine_for_pending(uri, &pending_analyzed)
                {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            quarantine_withdrawal_log_message(&path, reason),
                        )
                        .await;
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
        // Compute the lens-view identity from the completed snapshot before
        // the commit consumes it; the refresh request below compares it
        // against the last requested view (#2032, RIPR-SPEC-0138).
        let lens_view = lens_view_identity(&snapshot);
        // Retain the typed component outcomes for the deduplicated
        // degradation warning emitted after a successful commit (#1997).
        let component_outcomes = snapshot.component_outcomes.clone();
        let Some(quarantine_edges) =
            self.commit_refresh_snapshot(snapshot, &plan, &pending_analyzed, &pending_entered)
        else {
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
        };
        // Disclose lifted quarantines symmetrically (#1970): the fresh
        // publication (push) or the next pull re-serves the document against
        // the newly analyzed saved content.
        for (uri, was_disclosed) in &quarantine_edges.exited {
            if *was_disclosed && let Some(path) = self.document_path_for_uri(uri) {
                self.client
                    .log_message(MessageType::INFO, quarantine_restored_log_message(&path))
                    .await;
            }
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
        self.request_code_lens_refresh_if_view_changed(lens_view)
            .await;
        self.log_component_degradations(&component_outcomes).await;
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
        let RefreshTransaction {
            plan,
            snapshot,
            pending_analyzed,
            pending_entered,
            ..
        } = transaction;
        self.commit_refresh_snapshot(snapshot, &plan, &pending_analyzed, &pending_entered)
            .map(|_| plan)
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
        let WorkspaceDiagnostics {
            mut snapshot,
            batches,
        } = diagnostics;
        let Ok(last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        if snapshot.diagnostics_by_uri != diagnostics_by_uri_from_batches(&batches) {
            return None;
        }
        // Compute the one delivery selection before any publication (#1973):
        // push publication and both pull handlers read this stored outcome
        // instead of re-evaluating the budget per transport.
        if snapshot.delivery_selection.is_none() {
            snapshot.delivery_selection = Some(compute_delivery_selection(&snapshot));
        }
        let plan = diagnostic_refresh_plan(&last_diagnostics, batches);
        // Saved-workspace authority (#1970): compute the analyzed
        // saved-content identity this transaction will record — from the
        // persisted bytes the analysis read, not the didSave-tracked digest —
        // without mutating the document store. The store only advances when
        // the snapshot commits; publication filters against this pending
        // identity so a just-saved document is re-served and a document whose
        // buffer diverges from the freshly analyzed bytes is withdrawn.
        let Ok(documents) = self.documents.lock() else {
            return None;
        };
        let (pending_analyzed, pending_entered) = documents.pending_analyzed_digests();
        drop(documents);
        debug_assert!(snapshot.is_consistent());
        Some(RefreshTransaction {
            plan,
            snapshot,
            previous_diagnostics: last_diagnostics.clone(),
            pending_analyzed,
            pending_entered,
        })
    }

    /// Commit a completed snapshot as the analysis authority. Document
    /// identities advance here and only here (#1970): the pending analyzed
    /// digests computed at prepare time are applied to the document store
    /// atomically with the snapshot commit, so a transaction that is
    /// superseded or fails before this point never leaves document state
    /// ahead of `latest_analysis`. Returns the quarantine edges the
    /// committed identities produced so the caller can disclose lifts;
    /// `None` when the commit could not complete.
    pub(super) fn commit_refresh_snapshot(
        &self,
        mut snapshot: AnalysisSnapshot,
        plan: &DiagnosticRefreshPlan,
        pending_analyzed: &BTreeMap<Uri, Option<String>>,
        pending_entered: &[Uri],
    ) -> Option<super::state::QuarantineEdges> {
        snapshot.input_identity.as_ref()?;
        // Final authority guard: a committed snapshot always carries its
        // delivery selection (#1973). The refresh-transaction prepare step
        // already computed it on the real path; this fills snapshots that
        // bypassed prepare so pull never re-evaluates the budget.
        if snapshot.delivery_selection.is_none() {
            snapshot.delivery_selection = Some(compute_delivery_selection(&snapshot));
        }
        let input_identity = snapshot.input_identity_id();
        let snapshot = Arc::new(snapshot);
        let diagnostic_result_ids =
            Arc::new(DiagnosticResultIdCache::for_snapshot(Arc::clone(&snapshot)));
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return None;
        };
        let Ok(mut last_diagnostics) = self.last_diagnostics.lock() else {
            return None;
        };
        let Ok(mut latest_analysis) = self.latest_analysis.lock() else {
            return None;
        };
        let Ok(mut stored_result_ids) = self.diagnostic_result_ids.lock() else {
            return None;
        };
        let Ok(mut documents) = self.documents.lock() else {
            return None;
        };
        // Advance document identities atomically with the snapshot commit.
        // Entries disclosed during publication keep their disclosed marker
        // so the new episode does not disclose a second time.
        let transitions =
            documents.note_refresh_analyzed(input_identity, pending_analyzed, pending_entered);
        // `last_diagnostics` mirrors the client-visible state: quarantined
        // documents were withdrawn (published empty), so the committed
        // baseline carries an empty set for them (#1970). The next plan then
        // re-publishes the full set when the quarantine lifts instead of
        // diffing against diagnostics the client never saw.
        let mut committed_diagnostics = snapshot.diagnostics_by_uri.clone();
        for (uri, diagnostics) in committed_diagnostics.iter_mut() {
            if documents
                .state_for_uri(uri)
                .is_some_and(|state| state.is_quarantined())
            {
                diagnostics.clear();
            }
        }
        drop(documents);
        *last_diagnostics = committed_diagnostics;
        *last_diagnostic_uris = plan.current_uris.clone();
        *latest_analysis = Some(snapshot);
        *stored_result_ids = Some(diagnostic_result_ids);
        Some(transitions)
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
    pub(super) fn install_progress_recorder(&self) -> Arc<super::progress::RecordingSink> {
        let sink = Arc::new(super::progress::RecordingSink::default());
        self.progress.install_recorder(Arc::clone(&sink));
        sink
    }

    #[cfg(test)]
    pub(super) fn refresh_scheduler_for_test(&self) -> &RefreshScheduler {
        &self.refresh_scheduler
    }

    #[cfg(test)]
    pub(super) fn is_current_refresh_generation(&self, generation: u64) -> bool {
        self.refresh_scheduler.is_current_generation(generation)
    }

    pub(super) fn workspace_revision(&self) -> u64 {
        self.workspace_revision
            .lock()
            .map(|revision| *revision)
            .unwrap_or(0)
    }

    pub(super) fn advance_workspace_revision(&self) {
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

    #[cfg(test)]
    pub(super) fn invalidate_analysis_input_for_test(&self, reason: &str) {
        self.invalidate_analysis_input(reason);
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

    fn code_lens_refresh_support_enabled(&self) -> bool {
        self.code_lens_refresh_support
            .lock()
            .map(|supported| *supported)
            .unwrap_or(false)
    }

    /// Record `new_identity` as the current lens view and report whether the
    /// visible view changed since the last recorded one (#2032,
    /// RIPR-SPEC-0138). Returns `false` without touching state when the
    /// client did not negotiate refresh support, so an unsupported client
    /// never records or attempts a refresh. The identity comparison is the
    /// coalescing: a byte-identical re-commit reports no change.
    pub(super) fn note_lens_view_for_refresh(&self, new_identity: LensViewIdentity) -> bool {
        if !self.code_lens_refresh_support_enabled() {
            return false;
        }
        match self.last_lens_view_identity.lock() {
            Ok(mut last) => {
                if last.as_ref() == Some(&new_identity) {
                    false
                } else {
                    *last = Some(new_identity);
                    true
                }
            }
            Err(_) => false,
        }
    }

    /// Request one `workspace/codeLens/refresh` after a snapshot commit when
    /// the visible lens view changed (#2032, RIPR-SPEC-0138). Failure is
    /// log-only with no retry (mirrors the pull-diagnostic refresh), and the
    /// request never triggers analysis, refresh scheduling, config reload,
    /// or source access by itself.
    async fn request_code_lens_refresh_if_view_changed(&self, new_identity: LensViewIdentity) {
        if !self.note_lens_view_for_refresh(new_identity) {
            return;
        }
        self.send_code_lens_refresh().await;
    }

    /// Bare `workspace/codeLens/refresh` send with log-only failure, shared
    /// by the view-changed and view-cleared paths.
    async fn send_code_lens_refresh(&self) {
        if let Err(error) = self.client.code_lens_refresh().await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("code lens refresh request failed: {error}"),
                )
                .await;
        }
    }

    /// Test-only view of the lens-view identity covered by the last refresh
    /// request, so in-process tests can pin gating and change detection.
    #[cfg(test)]
    pub(super) fn last_requested_lens_view_identity(&self) -> Option<LensViewIdentity> {
        self.last_lens_view_identity
            .lock()
            .ok()
            .and_then(|last| last.clone())
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

    #[cfg(test)]
    pub(super) fn document_state_for_test(&self, uri: &Uri) -> Option<super::state::DocumentState> {
        self.documents.lock().ok()?.state_for_uri(uri).cloned()
    }

    #[cfg(test)]
    pub(super) fn last_diagnostics_for_uri_for_test(&self, uri: &Uri) -> Option<Vec<Diagnostic>> {
        self.last_diagnostics.lock().ok()?.get(uri).cloned()
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

    /// Emit work-done progress lifecycle events for a scheduler decision
    /// (#1971). Only accepted requests create progress: `Start` begins as
    /// `analyzing`, `Queued` begins as `queued` (and first terminates the
    /// token of the pending generation it replaced, which will never run).
    /// `Deduplicated` and `Stopped` are not accepted work and emit nothing.
    pub(super) async fn emit_progress_for_decision(&self, decision: &RefreshDecision) {
        match decision {
            RefreshDecision::Start(request) => {
                self.progress
                    .begin(request, AnalysisProgressPhase::Analyzing)
                    .await;
            }
            RefreshDecision::Queued {
                generation,
                superseded_pending,
            } => {
                if let Some(superseded) = superseded_pending {
                    self.progress
                        .end(*superseded, AnalysisProgressEnd::Superseded)
                        .await;
                }
                if let Some(request) = self.refresh_scheduler.pending_request(*generation) {
                    self.progress
                        .begin(&request, AnalysisProgressPhase::Queued)
                        .await;
                }
            }
            RefreshDecision::Deduplicated | RefreshDecision::Stopped => {}
        }
    }

    /// Terminal end for one attempt, derived from the same outcome and
    /// recorded health as `ripr/analysisStatus`, so the progress end message
    /// agrees with the analysis status surface. The tracker guarantees the
    /// end is emitted exactly once per generation.
    pub(super) async fn end_progress_for_attempt(
        &self,
        request: &RefreshRequest,
        outcome: RefreshAttemptOutcome,
    ) {
        let end = self.progress_end_for(request, outcome);
        self.progress.end(request.generation, end).await;
    }

    pub(super) fn progress_end_for(
        &self,
        request: &RefreshRequest,
        outcome: RefreshAttemptOutcome,
    ) -> AnalysisProgressEnd {
        let health = self.analysis_health_snapshot();
        let health_is_for_request = health.attempt_id == Some(request.generation);
        match outcome {
            RefreshAttemptOutcome::Published => {
                // Agrees with the snapshot run status: `full` completes;
                // every disclosed limited state (`seams_deferred`,
                // `limited`, `cache_limited`, `stale`) ends as limited.
                if health_is_for_request && health.run_status() == "full" {
                    AnalysisProgressEnd::Complete
                } else {
                    AnalysisProgressEnd::Limited(health.run_status().to_string())
                }
            }
            RefreshAttemptOutcome::Failed => {
                let kind = if health_is_for_request {
                    health.failure.map(|failure| failure.kind)
                } else {
                    None
                };
                AnalysisProgressEnd::Failed(kind)
            }
            RefreshAttemptOutcome::Cancelled => AnalysisProgressEnd::Cancelled,
            RefreshAttemptOutcome::Superseded => AnalysisProgressEnd::Superseded,
            RefreshAttemptOutcome::NotStarted => AnalysisProgressEnd::NotStarted,
        }
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
        let config = self.analysis_config();
        let configuration_failure = self.configuration_failure();
        let snapshot_state = self.latest_analysis.lock().ok().and_then(|snapshot| {
            snapshot.as_ref().map(|value| {
                (
                    value.input_identity.clone(),
                    value
                        .component_outcomes
                        .iter()
                        .map(|outcome| outcome.status_payload(value.input_identity_id().as_deref()))
                        .collect::<Vec<_>>(),
                )
            })
        });
        let (snapshot_identity, components) = match snapshot_state {
            Some((identity, components)) => (identity, components),
            None => (None, Vec::new()),
        };
        let snapshot_input = snapshot_identity.map(|identity| identity.status_payload());
        let current_input = match (
            health.current_input_identity.as_deref(),
            snapshot_input.as_ref(),
        ) {
            (Some(current), Some(snapshot))
                if snapshot["input_identity"].as_str() == Some(current) =>
            {
                snapshot.clone()
            }
            (Some(input_identity), _) => serde_json::json!({"input_identity": input_identity}),
            (None, Some(snapshot)) if health.state == AnalysisAttemptState::Succeeded => {
                snapshot.clone()
            }
            (None, None) => serde_json::Value::Null,
            (None, Some(_)) => serde_json::Value::Null,
        };
        let last_success_input = snapshot_input
            .or_else(|| {
                health
                    .last_success_input_identity
                    .clone()
                    .map(|input_identity| serde_json::json!({"input_identity": input_identity}))
            })
            .unwrap_or(serde_json::Value::Null);
        let pull_state = self.config_pull_state();
        let input_authority = serde_json::json!({
            "current": if configuration_failure.is_some() {
                serde_json::Value::Null
            } else {
                current_input
            },
            "last_success": last_success_input,
            "configuration_state": configuration_failure
                .as_ref()
                .map_or("valid", |_| "invalid"),
            "repository_config_source": config
                .as_ref()
                .and_then(|value| value.repo_config().source_path())
                .map(|path| path.to_string_lossy().to_string()),
            "session_options_present": config
                .as_ref()
                .is_some_and(|value| value.session_options.is_some()),
            // Session-configuration transport disclosure (#2031,
            // RIPR-SPEC-0136): how governed values arrive, where each one
            // came from, and the last pull outcome so defaults never
            // masquerade as accepted requested settings.
            "configuration_mode": self.configuration_mode().as_str(),
            "session_value_sources": config
                .as_ref()
                .map(|value| serde_json::Value::Object(value.session_value_sources()))
                .unwrap_or(serde_json::Value::Null),
            "configuration_pull": {
                // Snapshot once: the status request handler runs concurrently
                // with pull tasks, and three separate locked reads could
                // interleave a state change into a torn payload (e.g. a
                // failure from a newer state beside an older state string).
                "state": pull_state.as_str(),
                "epoch": self.config_pull_epoch.load(Ordering::Relaxed),
                "failure": pull_state.failure().map(|failure| serde_json::json!({
                    "kind": failure.kind,
                    "message": failure.message,
                })),
                "recovery_route": pull_state.recovery_route(),
            },
        });
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
            // Typed bounded per-component outcomes for the committed snapshot
            // (#1997, RIPR-SPEC-0141): the single typed authority for
            // optional-component degradation. Empty until a snapshot commits.
            "components": components,
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
            "input_authority": input_authority,
            // Bounded negotiated-capability disclosure (#1987,
            // RIPR-SPEC-0143): the selected client-feature profile without
            // the raw capability document.
            "client_features": self.client_features_projection(),
        })
    }

    /// Typed bounded status for a rejected workspace-folder update (#2036,
    /// RIPR-SPEC-0139): the stored set was left unchanged, and analysis is
    /// blocked behind `workspace_root_unavailable` until a valid event
    /// repairs the set.
    async fn reject_workspace_folder_update(&self, rejection: WorkspaceFolderEventRejection) {
        let detail = bounded_failure_message(&format!(
            "workspace folder update rejected ({}): {}; the stored folder set was left unchanged",
            rejection.kind.as_str(),
            rejection.detail
        ));
        self.apply_workspace_root_authority(WorkspaceRootAuthority::unavailable(detail))
            .await;
    }

    fn set_root(&self, root: PathBuf) {
        let Ok(mut current_root) = self.root.lock() else {
            return;
        };
        *current_root = root;
    }

    async fn apply_workspace_root_resolution(&self, resolution: WorkspaceRootResolution) {
        self.apply_workspace_root_authority(Self::workspace_root_authority_for_resolution(
            resolution,
        ))
        .await;
    }

    fn workspace_root_authority_for_resolution(
        resolution: WorkspaceRootResolution,
    ) -> WorkspaceRootAuthority {
        match resolution {
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
        }
    }

    async fn apply_workspace_root_authority(&self, authority: WorkspaceRootAuthority) {
        self.apply_workspace_root_authority_inner(authority, None)
            .await;
    }

    /// Apply an authority derived from the stored workspace-folder set
    /// (#2036, RIPR-SPEC-0139). The application is bound to the folder-set
    /// epoch the authority was derived from: if a newer event advanced the
    /// set before the transition guard is acquired, the stale application is
    /// dropped and the newer event's handler owns the authority.
    async fn apply_workspace_folder_set_authority(
        &self,
        authority: WorkspaceRootAuthority,
        expected_folder_set_epoch: u64,
    ) {
        self.apply_workspace_root_authority_inner(authority, Some(expected_folder_set_epoch))
            .await;
    }

    async fn apply_workspace_root_authority_inner(
        &self,
        authority: WorkspaceRootAuthority,
        expected_folder_set_epoch: Option<u64>,
    ) {
        let (schedule_deferred_pull, lens_view_cleared) = self
            .apply_workspace_root_authority_locked(authority, expected_folder_set_epoch)
            .await;
        if lens_view_cleared {
            // Sent after the transition guard is released, same discipline as
            // the deferred configuration pull: a cleared analysis state means
            // every lens is stale (#2032, RIPR-SPEC-0138).
            self.send_code_lens_refresh().await;
        }
        if schedule_deferred_pull {
            // Scheduled after the transition guard is released: a deferred
            // configuration pull (#2031) becomes runnable once a single
            // workspace root is selected, and its apply path can reach the
            // refresh path, which re-locks `workspace_root_transition`
            // (`run_refresh_request`). Scheduling inline while holding the
            // guard would deadlock.
            // Indirection via Box::pin: root transitions are reachable from
            // the refresh path that a pull can schedule, so the direct call
            // would be a recursive async fn.
            Box::pin(self.schedule_configuration_pull()).await;
        }
    }

    async fn apply_workspace_root_authority_locked(
        &self,
        authority: WorkspaceRootAuthority,
        expected_folder_set_epoch: Option<u64>,
    ) -> (bool, bool) {
        let _transition = self.workspace_root_transition.lock().await;
        // Folder-set epoch binding (#2036): an authority derived from the
        // stored workspace-folder set is applied only when the set has not
        // advanced since derivation. The set lock is always the inner lock
        // (transition guard first), matching the handler's ordering.
        if let Some(expected) = expected_folder_set_epoch {
            let current = self
                .workspace_folders
                .lock()
                .ok()
                .map(|set| set.folder_set_epoch());
            if current != Some(expected) {
                return (false, false);
            }
        }
        let previous = self.workspace_root_authority();
        let changed = previous.state != authority.state
            || previous.effective_root != authority.effective_root
            || previous.candidate_roots != authority.candidate_roots
            || previous.detail != authority.detail;
        if !changed {
            // A byte-identical authority is no transition (#2036): no epoch
            // bump, no invalidation, no status publish. An equivalent
            // workspace-folder set (any order) therefore produces no epoch
            // bump and no transition.
            return (false, false);
        }
        let authority_epoch = self.workspace_root_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        // Any change away from a selected root affects root-scoped pulled
        // settings: bump the pull epoch so an in-flight response scoped to
        // the old root is dropped. A re-pull is scheduled below whenever the
        // transition lands on an analysis-capable root — including
        // A -> unavailable -> B, where the intermediate transition has no
        // root but the retained layer is still A-scoped; the epoch bump at
        // the A -> unavailable step already dropped A's in-flight responses
        // (#2211 review). Initial selection (None -> Some) does not bump:
        // nothing was ever scoped to a prior root.
        let root_changed = previous.effective_root != authority.effective_root;
        if previous.effective_root.is_some()
            && root_changed
            && self.configuration_mode() == ConfigurationMode::Pull
        {
            self.config_pull_epoch.fetch_add(1, Ordering::SeqCst);
        }
        if changed {
            self.refresh_scheduler.invalidate_input();
            self.progress
                .end_queued(AnalysisProgressEnd::Superseded)
                .await;
            let uris = self.clear_all_diagnostic_uris();
            if !self.pull_diagnostics_enabled() {
                for uri in uris {
                    self.client.publish_diagnostics(uri, Vec::new(), None).await;
                }
            }
            self.reset_health_for_input_change();
        }

        if self.workspace_root_epoch.load(Ordering::SeqCst) != authority_epoch {
            return (false, false);
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
        // A deferred configuration pull (#2031) becomes runnable once a
        // single workspace root is selected, and a root change after the
        // pull lifecycle has started needs one re-pull scoped to the new
        // root. Staleness is decided by comparing the retained layer's scope
        // root against the final effective root — NOT by transition deltas —
        // so a direct A -> B switch (rewritten to a non-analyzable
        // `RootChanged` state and re-selected later with no further root
        // change) is still caught when B is re-selected. Applied/Failed or
        // an in-flight pull makes the lifecycle restartable; a
        // pre-`initialized` Pending state must NOT schedule here — the
        // client rejects server->client requests before `initialized`
        // (-32002), and `initialized` owns the first pull. The wrapper
        // schedules after the transition guard is released.
        let pull_scope_stale = {
            let restartable = {
                let in_flight = self
                    .config_pull_coordinator
                    .lock()
                    .map(|coordinator| coordinator.in_flight)
                    .unwrap_or(false);
                in_flight
                    || matches!(
                        self.config_pull_state(),
                        ConfigPullState::Applied | ConfigPullState::Failed(_)
                    )
            };
            restartable
                && self
                    .config_pull_scope_root
                    .lock()
                    .ok()
                    .and_then(|scope_root| scope_root.clone())
                    != self.workspace_root_authority().effective_root
        };
        let schedule_deferred_pull = self.configuration_mode() == ConfigurationMode::Pull
            && (matches!(self.config_pull_state(), ConfigPullState::Deferred) || pull_scope_stale)
            && self.workspace_root_authority().allows_analysis();
        // Clearing analysis state (#2032): the visible lens view just became
        // empty. Record the cleared identity (coalesced — no-op when already
        // cleared or unsupported); the wrapper sends the refresh after the
        // guard is released.
        let lens_view_cleared =
            changed && self.note_lens_view_for_refresh(LensViewIdentity::cleared());
        (schedule_deferred_pull, lens_view_cleared)
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
        // The degradation-dedup state belongs to the old input/root context
        // (#1997 review): a signature suppressed there must re-warn once in
        // the new context instead of staying silently suppressed.
        if let Ok(mut last) = self.last_component_degradation.lock() {
            *last = None;
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

    /// Invalidate analysis input and terminate the progress token of any
    /// queued request the invalidation dropped before it could start (#1971).
    /// The active (analyzing) token is left for the refresh loop's
    /// outcome-based end.
    async fn invalidate_analysis_input_and_end_queued_progress(&self, reason: &str) {
        self.invalidate_analysis_input(reason);
        self.progress
            .end_queued(AnalysisProgressEnd::Superseded)
            .await;
    }

    pub(super) fn set_configuration_failure(&self, message: impl Into<String>) {
        self.record_blocking_failure("config_invalid", message);
    }

    /// Record a failure that pauses analysis until the session state is
    /// repaired: the typed failure lands in `configuration_failure` and the
    /// analysis health so the status payload discloses it instead of
    /// presenting an internally inconsistent session as normal.
    fn record_blocking_failure(&self, kind: &str, message: impl Into<String>) {
        let failure = AnalysisFailure {
            kind: kind.to_string(),
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
                    self.invalidate_analysis_input_and_end_queued_progress(
                        RefreshReason::ConfigReload.as_str(),
                    )
                    .await;
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
                self.invalidate_analysis_input_and_end_queued_progress(
                    RefreshReason::ConfigReload.as_str(),
                )
                .await;
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
        self.invalidate_analysis_input_and_end_queued_progress(
            RefreshReason::ConfigReload.as_str(),
        )
        .await;
        self.publish_analysis_status().await;
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::ConfigReload)
            .await;
    }

    fn configuration_mode(&self) -> ConfigurationMode {
        self.configuration_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or(ConfigurationMode::InitializationOnly)
    }

    /// The bounded status projection of the negotiated client-feature
    /// profile (#1987, RIPR-SPEC-0143): status-visible without dumping the
    /// raw capability document. Before `initialize` this projects the
    /// unsupported pre-initialize profile.
    fn client_features_projection(&self) -> serde_json::Value {
        self.client_features
            .lock()
            .map(|features| features.status_projection())
            .unwrap_or(serde_json::Value::Null)
    }

    /// Poison the profile store so tests can exercise the fail-closed
    /// surfacing at `initialize` (#1987 review). A std::sync::Mutex is
    /// poisoned only when a guard holder unwinds, so this helper triggers a
    /// runtime index failure while holding the guard; the index is derived
    /// at runtime so the compiler cannot const-prove it, and catch_unwind
    /// confines the unwind to this helper. No production path is involved.
    #[cfg(test)]
    pub(super) fn poison_client_features_for_test(&self) {
        let out_of_bounds = std::env::args_os().count() + 1;
        let slots = [0u8];
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = self.client_features.lock();
            let _tombstone = slots[out_of_bounds];
        }));
    }

    fn config_pull_state(&self) -> ConfigPullState {
        self.config_pull_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(ConfigPullState::NotApplicable)
    }

    fn set_config_pull_state(&self, state: ConfigPullState) {
        if let Ok(mut current) = self.config_pull_state.lock() {
            *current = state;
        }
    }

    /// Schedule one coalesced `workspace/configuration` pull (#2031). At most
    /// one request is in flight; a schedule request arriving while one is
    /// in flight collapses into a single queued re-pull for the latest epoch.
    /// The pull is a pure LSP round-trip: it never launches analysis, git,
    /// network beyond the LSP connection, or edits.
    async fn schedule_configuration_pull(&self) {
        {
            let Ok(mut coordinator) = self.config_pull_coordinator.lock() else {
                return;
            };
            if coordinator.in_flight {
                coordinator.queued = true;
                return;
            }
            coordinator.in_flight = true;
        }
        loop {
            let epoch = self.config_pull_epoch.load(Ordering::SeqCst);
            self.set_config_pull_state(ConfigPullState::Pending);
            self.publish_analysis_status().await;
            self.pull_and_apply_configuration(epoch).await;
            let queued = {
                let Ok(mut coordinator) = self.config_pull_coordinator.lock() else {
                    return;
                };
                if coordinator.queued {
                    coordinator.queued = false;
                    true
                } else {
                    coordinator.in_flight = false;
                    false
                }
            };
            if !queued {
                return;
            }
        }
    }

    async fn pull_and_apply_configuration(&self, epoch: u64) {
        let Some(root) = self.effective_root() else {
            // No single selected root at pull time (ambiguous or unavailable
            // workspace): defer the pull; analysis is already blocked there.
            self.set_config_pull_state(ConfigPullState::Deferred);
            self.publish_analysis_status().await;
            return;
        };
        let scope_uri = match file_uri_for_path(&root) {
            Ok(uri) => uri,
            Err(error) => {
                self.set_config_pull_state(ConfigPullState::Failed(AnalysisFailure {
                    kind: "config_pull_failed".to_string(),
                    message: bounded_failure_message(&format!(
                        "workspace/configuration scope URI for the selected root is invalid: {error}"
                    )),
                }));
                self.publish_analysis_status().await;
                return;
            }
        };
        let result = self
            .client
            .configuration(vec![ConfigurationItem {
                scope_uri: Some(scope_uri),
                section: Some("ripr".to_string()),
            }])
            .await;
        // Epoch guard: a response for an older epoch is dropped; the queued
        // re-pull for the current epoch owns the disclosed state.
        if self.config_pull_epoch.load(Ordering::SeqCst) != epoch {
            return;
        }
        match result {
            Ok(values) => match validated_pulled_options(&values) {
                Ok(pulled) => {
                    self.apply_pulled_configuration(pulled).await;
                    // The retained layer is scoped to the root it was pulled
                    // for; root transitions compare against this to decide
                    // whether a re-pull is needed.
                    if let Ok(mut scope_root) = self.config_pull_scope_root.lock() {
                        *scope_root = Some(root);
                    }
                }
                Err(message) => {
                    self.set_config_pull_state(ConfigPullState::Failed(AnalysisFailure {
                        kind: "config_pull_invalid".to_string(),
                        message: bounded_failure_message(&message),
                    }));
                    self.publish_analysis_status().await;
                }
            },
            Err(error) => {
                self.set_config_pull_state(ConfigPullState::Failed(AnalysisFailure {
                    kind: "config_pull_failed".to_string(),
                    message: bounded_failure_message(&format!(
                        "workspace/configuration request failed: {error}"
                    )),
                }));
                self.publish_analysis_status().await;
            }
        }
    }

    /// Apply a validated pulled layer. Semantically unchanged effective
    /// settings do not reschedule analysis (the `next == current` no-op guard
    /// pattern from `apply_session_configuration_change`, applied to the
    /// effective settings rather than the retained layer representation).
    async fn apply_pulled_configuration(&self, pulled: Option<LSPAny>) {
        let Some(current) = self.analysis_config() else {
            return;
        };
        let next = current.with_pulled_options(pulled.as_ref());
        self.set_config_pull_state(ConfigPullState::Applied);
        if next.effective_settings_eq(&current) {
            // Retain the pulled layer (it decides precedence and source
            // disclosure on later reloads) without rescheduling analysis:
            // semantically unchanged effective settings are a no-op.
            if next != current {
                self.set_analysis_config(next);
            }
            self.publish_analysis_status().await;
            return;
        }
        self.set_analysis_config(next);
        if self.configuration_failure().is_some() {
            // Mirror the session-change path: the pulled layer is retained
            // and re-applied once the repository configuration recovers; it
            // cannot authorize a refresh on its own.
            self.publish_analysis_status().await;
            return;
        }
        self.invalidate_analysis_input_and_end_queued_progress(
            RefreshReason::ConfigReload.as_str(),
        )
        .await;
        self.publish_analysis_status().await;
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::ConfigReload)
            .await;
    }

    /// `workspace/didChangeConfiguration` in pull mode: invalidate the cached
    /// pulled values by bumping the pull epoch (responses for older epochs
    /// are dropped) and schedule one coalesced re-pull. The retained pulled
    /// layer stays in effect as last-known-good until the re-pull resolves.
    async fn handle_pull_mode_configuration_change(&self) {
        self.config_pull_epoch.fetch_add(1, Ordering::SeqCst);
        self.schedule_configuration_pull().await;
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

    fn open_document(
        &self,
        params: DidOpenTextDocumentParams,
    ) -> Option<(Uri, QuarantineTransition)> {
        let uri = params.text_document.uri.clone();
        self.documents
            .lock()
            .ok()
            .map(|mut documents| (uri, documents.open(params)))
    }

    fn change_document(
        &self,
        params: DidChangeTextDocumentParams,
    ) -> Option<(Uri, QuarantineTransition)> {
        let uri = params.text_document.uri.clone();
        self.documents
            .lock()
            .ok()
            .map(|mut documents| (uri, documents.change(params)))
    }

    fn save_document(
        &self,
        uri: &Uri,
        saved_digest: Option<String>,
        text: Option<String>,
    ) -> Option<QuarantineTransition> {
        self.documents
            .lock()
            .ok()
            .map(|mut documents| documents.save(uri, saved_digest, text))
    }

    fn close_document(&self, params: DidCloseTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.close(params);
    }

    fn document_text(&self, uri: &tower_lsp_server::ls_types::Uri) -> Option<String> {
        self.documents
            .lock()
            .ok()?
            .documents
            .get(uri)
            .map(|state| state.text.clone())
    }

    /// The quarantine state of an open document, as `(path, reason)`.
    /// `None` means the document is unknown or clean: its buffer matches the
    /// saved content the committed snapshot analyzed.
    fn document_quarantine(&self, uri: &Uri) -> Option<(PathBuf, DocumentStalenessReason)> {
        let documents = self.documents.lock().ok()?;
        let state = documents.state_for_uri(uri)?;
        let quarantine = state.quarantine.as_ref()?;
        Some((state.path.clone(), quarantine.reason))
    }

    /// The quarantine state of an open document against a refresh
    /// transaction's pending analyzed identity (#1970): what the state will
    /// be once this snapshot commits. Publication filters on this so a
    /// document is withdrawn or re-served against the identity the
    /// in-flight snapshot carries, while the committed state only advances
    /// at commit time.
    fn document_quarantine_for_pending(
        &self,
        uri: &Uri,
        pending_analyzed: &BTreeMap<Uri, Option<String>>,
    ) -> Option<(PathBuf, DocumentStalenessReason)> {
        let documents = self.documents.lock().ok()?;
        let state = documents.state_for_uri(uri)?;
        let Some(analyzed) = pending_analyzed.get(&state.uri) else {
            return state
                .quarantine
                .as_ref()
                .map(|quarantine| (state.path.clone(), quarantine.reason));
        };
        state
            .staleness_for_analyzed(analyzed.as_ref())
            .map(|reason| (state.path.clone(), reason))
    }

    fn document_path_for_uri(&self, uri: &Uri) -> Option<PathBuf> {
        self.documents
            .lock()
            .ok()?
            .state_for_uri(uri)
            .map(|state| state.path.clone())
    }

    /// Mark the document's withdrawal as disclosed, returning `(path,
    /// reason)` only on the first disclosure of the episode so one episode
    /// emits exactly one disclosure.
    fn mark_withdrawal_disclosed(&self, uri: &Uri) -> Option<(PathBuf, DocumentStalenessReason)> {
        let mut documents = self.documents.lock().ok()?;
        let state = documents.state_for_uri_mut(uri)?;
        let quarantine = state.quarantine.as_mut()?;
        if quarantine.withdrawal_disclosed {
            return None;
        }
        quarantine.withdrawal_disclosed = true;
        Some((state.path.clone(), quarantine.reason))
    }

    async fn disclose_withdrawal_once(&self, uri: &Uri) {
        if let Some((path, reason)) = self.mark_withdrawal_disclosed(uri) {
            self.client
                .log_message(
                    MessageType::WARNING,
                    quarantine_withdrawal_log_message(&path, reason),
                )
                .await;
        }
    }

    /// The diagnostics the committed snapshot serves for one URI under the
    /// stored delivery selection (#1973).
    fn committed_served_diagnostics_for_uri(&self, uri: &Uri) -> Vec<Diagnostic> {
        self.latest_analysis
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .map(|snapshot| snapshot.served_diagnostics_for_uri(uri))
            .unwrap_or_default()
    }

    fn last_diagnostics_has_any(&self, uri: &Uri) -> bool {
        self.last_diagnostics
            .lock()
            .ok()
            .and_then(|last| last.get(uri).map(|diagnostics| !diagnostics.is_empty()))
            .unwrap_or(false)
    }

    fn set_last_diagnostics_for_uri(&self, uri: &Uri, diagnostics: Vec<Diagnostic>) {
        if let Ok(mut last) = self.last_diagnostics.lock() {
            last.insert(uri.clone(), diagnostics);
        }
    }

    /// Apply a quarantine edge from a document lifecycle event
    /// (open/change/save): entering withdraws the document's line-local
    /// diagnostics, exiting re-serves them from the committed snapshot.
    async fn handle_document_quarantine_transition(
        &self,
        uri: &Uri,
        transition: QuarantineTransition,
    ) {
        match transition {
            QuarantineTransition::Entered => self.withdraw_document_diagnostics(uri).await,
            QuarantineTransition::Exited { was_disclosed } => {
                self.restore_document_diagnostics(uri, was_disclosed).await;
            }
            QuarantineTransition::Unchanged => {}
        }
    }

    /// Withdraw a dirty document's line-local diagnostics (#1970): publish
    /// an empty set (push delivery) and disclose the withdrawal once per
    /// episode. A document with nothing served stays silent — there is no
    /// stale line identity to withdraw.
    async fn withdraw_document_diagnostics(&self, uri: &Uri) {
        let had_visible = !self.committed_served_diagnostics_for_uri(uri).is_empty()
            || self.last_diagnostics_has_any(uri);
        if !had_visible {
            return;
        }
        self.disclose_withdrawal_once(uri).await;
        if !self.pull_diagnostics_enabled() {
            self.client
                .publish_diagnostics(uri.clone(), Vec::new(), None)
                .await;
        }
        self.set_last_diagnostics_for_uri(uri, Vec::new());
    }

    /// Re-serve a document whose quarantine lifted (#1970): the buffer again
    /// matches the analyzed saved content, so the committed snapshot's
    /// line-local diagnostics are valid for the client's buffer.
    async fn restore_document_diagnostics(&self, uri: &Uri, was_disclosed: bool) {
        let diagnostics = self.committed_served_diagnostics_for_uri(uri);
        if !self.pull_diagnostics_enabled() {
            self.client
                .publish_diagnostics(uri.clone(), diagnostics.clone(), None)
                .await;
        }
        self.set_last_diagnostics_for_uri(uri, diagnostics);
        if was_disclosed && let Some(path) = self.document_path_for_uri(uri) {
            self.client
                .log_message(MessageType::INFO, quarantine_restored_log_message(&path))
                .await;
        }
    }

    /// Publish one document's selection-filtered diagnostics during a
    /// refresh transaction, honoring the dirty-buffer quarantine against the
    /// transaction's pending analyzed identity (#1970): a document
    /// quarantined under the pending identity is published empty and its
    /// withdrawal disclosed, instead of receiving saved-state line-local
    /// diagnostics whose line identity no longer matches the client's
    /// buffer. Does not touch `last_diagnostics`; the refresh transaction
    /// commit owns that baseline.
    async fn publish_served_diagnostics_for_transaction(
        &self,
        uri: &Uri,
        diagnostics: Vec<Diagnostic>,
        pending_analyzed: &BTreeMap<Uri, Option<String>>,
    ) {
        if self
            .document_quarantine_for_pending(uri, pending_analyzed)
            .is_some()
        {
            if !diagnostics.is_empty() {
                // When the quarantine episode is already registered the
                // once-per-episode marker applies; a pending-entered episode
                // is disclosed by the pending_entered publication pass (or
                // here directly when the batch covers it) and marked at
                // commit.
                if self.document_quarantine(uri).is_some() {
                    self.disclose_withdrawal_once(uri).await;
                } else if let Some((path, reason)) =
                    self.document_quarantine_for_pending(uri, pending_analyzed)
                {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            quarantine_withdrawal_log_message(&path, reason),
                        )
                        .await;
                }
            }
            self.client
                .publish_diagnostics(uri.clone(), Vec::new(), None)
                .await;
            return;
        }
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    fn saved_content_digest_matches(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
        digest: &str,
    ) -> bool {
        self.saved_content_digests
            .lock()
            .ok()
            .and_then(|digests| digests.get(uri).cloned())
            .is_some_and(|recorded| recorded == digest)
    }

    fn record_saved_content_digest(&self, uri: &tower_lsp_server::ls_types::Uri, digest: String) {
        if let Ok(mut digests) = self.saved_content_digests.lock() {
            digests.insert(uri.clone(), digest);
        }
    }

    pub(super) fn hover_for_position(&self, params: &HoverParams) -> Option<Hover> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;
        // A quarantined (dirty-buffer) document has no valid saved-state
        // line identity (#1970): do not hover stale line-local evidence.
        if self.document_quarantine(uri).is_some() {
            return None;
        }
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
            // The refresh loop is being dropped mid-flight (e.g. the
            // dispatcher cancelled the future), so its outcome-based end
            // never runs. Terminate this generation's token plus any queued
            // token the scheduler just cancelled. Best-effort: the send is
            // spawned because Drop is synchronous; the tracker's registry
            // keeps the end exactly-once.
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let progress = Arc::clone(&self.backend.progress);
                let generation = self.request.generation;
                runtime.spawn(async move {
                    progress
                        .end(generation, AnalysisProgressEnd::Cancelled)
                        .await;
                    progress.end_queued(AnalysisProgressEnd::Cancelled).await;
                });
            }
            // Invariant: guard drops outside a current tokio runtime (sync
            // teardown) skip the best-effort end entirely. That is accepted
            // rather than hidden: the tracker's registry never emits a second
            // end, and a skipped end only leaves the client without a terminal
            // for a generation that never produced diagnostics.
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

    async fn log_refresh_started(&self, request: &RefreshRequest) {
        // Name the one Git-input record this attempt consumes (#2000,
        // RIPR-SPEC-0142): the resolution state and requested/resolved base
        // are visible at the phase boundary, and an unresolved base is
        // observable before the analysis error path reports it.
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "ripr analysis refresh started: generation={}, git_input_resolution={}, requested_base={:?}, resolved_base={:?}",
                    request.generation,
                    request.git_inputs.resolution().as_str(),
                    request.git_inputs.requested_base(),
                    request.git_inputs.resolved_base(),
                ),
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

    /// Emit the deduplicated client-visible component-degradation warning for
    /// one committed snapshot (#1997, RIPR-SPEC-0141). A byte-identical
    /// repeated degradation warns once per distinct signature; a changed
    /// signature warns again; a cleared signature logs one recovery line.
    /// Routine partial evidence stays in this standard log channel —
    /// `window/showMessage` remains reserved for reviewed hard failures.
    async fn log_component_degradations(
        &self,
        outcomes: &[super::component_outcome::ComponentOutcome],
    ) {
        let signature = super::component_outcome::degradation_signature(outcomes);
        let previous = {
            let Ok(mut last) = self.last_component_degradation.lock() else {
                return;
            };
            std::mem::replace(&mut *last, signature.clone())
        };
        if previous == signature {
            return;
        }
        if let Some(message) = super::component_outcome::degradation_log_message(outcomes) {
            self.client.log_message(MessageType::WARNING, message).await;
        } else if previous.is_some() {
            self.client
                .log_message(
                    MessageType::INFO,
                    "ripr analysis recovered: all recorded analysis components are complete",
                )
                .await;
        }
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
    // Single-source bounding/redaction for LSP client-visible error text
    // (#1997): the implementation lives in the component-outcome module so
    // health failures and component outcomes are governed identically.
    super::component_outcome::bounded_message(message)
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

fn root_authority_receipt_status(state: &WorkspaceRootState, client_features: LSPAny) -> LSPAny {
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
        "client_features": client_features,
        "limits_note": "Static evidence only; advisory, not a gate decision.",
    })
}

/// Standard LSP protocol tracing (`$/setTrace` / `$/logTrace`, #2035,
/// RIPR-SPEC-0137).
///
/// Trace state is session-local volatile observability: it never enters
/// snapshot, input-identity, diagnostic, action, command, status, or receipt
/// state, and toggling it never triggers analysis, refresh, configuration
/// reload, or source access. Traces are never a semantic or evidence
/// authority.
///
/// Emission is STRUCTURALLY redacted by construction: a trace only ever
/// carries the direction, the method name, the message class, and — at
/// `verbose` — bounded numeric metadata (byte counts, outcome classes, error
/// codes). Source text, paths, configuration values, repair packets, command
/// arguments, and arbitrary client-provided strings never enter the trace
/// because the emission sites never receive them as free text; params are
/// serialized only to count bytes and the serialized form is dropped
/// immediately.
impl Backend {
    pub(super) fn trace_level(&self) -> TraceValue {
        self.trace
            .lock()
            .map(|level| *level)
            .unwrap_or(TraceValue::Off)
    }

    fn set_trace_level(&self, level: TraceValue) {
        if let Ok(mut current) = self.trace.lock() {
            *current = level;
        }
    }

    /// `$/setTrace` notification handler, registered through
    /// `LspServiceBuilder::custom_method` (tower-lsp-server has no native
    /// `$/setTrace`; unregistered notifications are silently dropped).
    /// Updates the session trace level immediately; it never triggers
    /// analysis, refresh, configuration reload, or source access.
    ///
    /// Params arrive as `LSPAny` rather than typed `SetTraceParams` so an
    /// unknown value is a handled rejection instead of a silently dropped
    /// parse failure: the current state is kept, and the rejection is
    /// observable through `$/logTrace` when tracing is enabled. The
    /// client-provided value is never reflected verbatim. Trace lifecycle
    /// notifications are never themselves traced, so the trace channel
    /// cannot recurse.
    pub(super) async fn set_trace(&self, params: LSPAny) {
        let level = params
            .get("value")
            .and_then(LSPAny::as_str)
            .and_then(|value| match value {
                "off" => Some(TraceValue::Off),
                "messages" => Some(TraceValue::Messages),
                "verbose" => Some(TraceValue::Verbose),
                _ => None,
            });
        match level {
            Some(level) => self.set_trace_level(level),
            None => {
                if self.trace_level() != TraceValue::Off {
                    self.emit_log_trace(
                        "ripr trace: $/setTrace rejected (class=unknown_value); trace state unchanged"
                            .to_string(),
                        None,
                    )
                    .await;
                }
            }
        }
    }

    /// Bounded params byte count for `verbose` traces, or `None` below
    /// `verbose`. The serialized form is used only for its length and is
    /// dropped immediately.
    fn verbose_params_bytes(&self, params: &impl serde::Serialize) -> Option<usize> {
        (self.trace_level() == TraceValue::Verbose).then(|| {
            serde_json::to_string(params)
                .map(|text| text.len())
                .unwrap_or(0)
        })
    }

    /// Emit a redacted `$/logTrace` for one inbound message: direction,
    /// method name, and message class at `messages`; a bounded byte count
    /// added at `verbose`.
    async fn trace_inbound(&self, class: &str, method: &str, params_bytes: Option<usize>) {
        if self.trace_level() == TraceValue::Off {
            return;
        }
        let verbose = params_bytes.map(|bytes| format!("params_bytes={bytes}"));
        self.emit_log_trace(format!("ripr trace <- {class} {method}"), verbose)
            .await;
    }

    /// Emit a redacted `$/logTrace` for one outbound result: direction,
    /// method name, and the response/error class at `messages`; the outcome
    /// class plus a bounded byte count (or the error code) added at
    /// `verbose`.
    async fn trace_response<T: serde::Serialize>(&self, method: &str, result: &LspResult<T>) {
        if self.trace_level() == TraceValue::Off {
            return;
        }
        let class = if result.is_ok() { "response" } else { "error" };
        let verbose = (self.trace_level() == TraceValue::Verbose).then(|| match result {
            Ok(value) => format!(
                "outcome=ok response_bytes={}",
                serde_json::to_string(value)
                    .map(|text| text.len())
                    .unwrap_or(0)
            ),
            Err(error) => format!("outcome=error code={}", error.code),
        });
        self.emit_log_trace(format!("ripr trace -> {class} {method}"), verbose)
            .await;
    }

    /// Fire-and-forget `$/logTrace` emission. Emission failure is non-fatal:
    /// `send_notification` never blocks analysis or shutdown, and it is
    /// suppressed by the library while the session is not initialized.
    async fn emit_log_trace(&self, message: String, verbose: Option<String>) {
        self.client
            .send_notification::<LogTrace>(LogTraceParams { message, verbose })
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialized(&self, _: InitializedParams) {
        self.trace_inbound("notification", "initialized", None)
            .await;
        let supports_dynamic_registration = self
            .dynamic_file_watch_registration
            .lock()
            .map(|value| *value)
            .unwrap_or(false);
        if supports_dynamic_registration {
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
        // First configuration pull (#2031). This runs in `initialized`, not
        // `initialize`: tower-lsp-server rejects client requests with -32002
        // before the session is initialized.
        if self.configuration_mode() == ConfigurationMode::Pull {
            self.schedule_configuration_pull().await;
        }
    }

    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        // Trace emission is suppressed by the transport until the session is
        // initialized, so the initialize handshake itself is never traced;
        // this call documents the lifecycle boundary.
        self.trace_inbound("request", "initialize", None).await;
        // The standard trace lifecycle (#2035, RIPR-SPEC-0137): honor the
        // initial trace value selected by the client. An unknown value fails
        // typed initialize-param parsing per the library contract. Trace
        // state is volatile session observability only; it never enters
        // snapshot, input-identity, diagnostic, or action state.
        if let Some(trace) = params.trace {
            self.set_trace_level(trace);
        }
        // Typed ingress bound (#2034): reject oversized client options before
        // any config load, root resolution, or capability state mutation.
        check_initialization_options(params.initialization_options.as_ref())?;
        // Parse the typed client-feature profile exactly once (#1987,
        // RIPR-SPEC-0143); every negotiated session field below is populated
        // from it, and downstream surfaces consume session state or the
        // profile — never raw `InitializeParams` capability trees.
        let profile = ClientFeatureProfile::from_initialize_params(&params);
        let supports_pull_diagnostics = profile.pull_diagnostics;
        let supports_diagnostic_refresh = profile.diagnostic_refresh;
        if let Ok(mut supported) = self.pull_diagnostics.lock() {
            *supported = supports_pull_diagnostics;
        }
        if let Ok(mut supported) = self.diagnostic_refresh_support.lock() {
            *supported = supports_diagnostic_refresh;
        }
        // CodeLens refresh negotiation (#2032, RIPR-SPEC-0138): from client
        // capabilities only, never inferred from the client name.
        let supports_code_lens_refresh = profile.code_lens_refresh;
        if let Ok(mut supported) = self.code_lens_refresh_support.lock() {
            *supported = supports_code_lens_refresh;
        }
        self.progress.set_supported(profile.work_done_progress);
        // Negotiate the session-configuration transport from capabilities
        // only (#2031, RIPR-SPEC-0136): pull when the client answers
        // `workspace/configuration`, push fallback when it advertises
        // `workspace/didChangeConfiguration`, otherwise initialization-only.
        let negotiated_mode = profile.configuration_mode;
        if let Ok(mut mode) = self.configuration_mode.lock() {
            *mode = negotiated_mode;
        }
        self.set_config_pull_state(match negotiated_mode {
            ConfigurationMode::Pull => ConfigPullState::Pending,
            _ => ConfigPullState::NotApplicable,
        });
        let supports_dynamic_registration = profile.watched_files_dynamic_registration;
        if let Ok(mut supported) = self.dynamic_file_watch_registration.lock() {
            *supported = supports_dynamic_registration;
        }
        let resolution = root_from_initialize_params(&params);
        // Retain the canonical workspace-folder set (#2036, RIPR-SPEC-0139)
        // so `didChangeWorkspaceFolders` deltas apply to stored state
        // instead of being discarded. The root resolution is unchanged.
        if let Ok(mut set) = self.workspace_folders.lock() {
            *set = workspace_folder_set_from_initialize_params(&params);
        }
        let (repo_config, config_error) = match &resolution {
            WorkspaceRootResolution::Selected(root) if root.is_dir() => {
                match crate::config::load_for_root(root) {
                    Ok(config) => (config, None),
                    Err(err) => (crate::config::RiprConfig::default(), Some(err)),
                }
            }
            _ => (crate::config::RiprConfig::default(), None),
        };
        let analysis_config =
            LspAnalysisConfig::from_initialize_params(&params, repo_config, &profile);
        // The profile is the sole owner of the position-encoding negotiation;
        // read the chosen encoding back so the initialize response advertises
        // exactly what the config will use.
        let position_encoding = analysis_config.position_encoding.clone();
        self.set_analysis_config(analysis_config);
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
        // The profile store lands after the root application and config
        // failure handling because applying a workspace-root authority
        // resets the failure channel for the new input context; a failure
        // recorded here is the last writer and stays disclosed.
        if let Ok(mut features) = self.client_features.lock() {
            *features = profile.clone();
        } else {
            // Fail closed, never silent (#1987 review): a poisoned profile
            // store would leave the pre-initialize `unsupported()` profile
            // beside sibling session fields just populated from the parsed
            // profile — an internally inconsistent session. Surface it
            // through the same blocking-failure channel as a config load
            // failure so the status payload discloses the state and analysis
            // stays paused instead of running on a torn negotiation.
            self.client
                .log_message(
                    MessageType::WARNING,
                    "ripr client feature profile could not be stored; analysis is paused",
                )
                .await;
            self.record_blocking_failure(
                "session_state_inconsistent",
                "client feature profile could not be stored after initialize negotiation; analysis is paused",
            );
            self.publish_analysis_status().await;
        }
        Ok(initialize_result_for_client(
            supports_pull_diagnostics,
            position_encoding,
        ))
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.trace_inbound(
            "notification",
            "workspace/didChangeConfiguration",
            self.verbose_params_bytes(&params),
        )
        .await;
        match self.configuration_mode() {
            // Pull mode (#2031): pushed values are not applied directly; the
            // notification invalidates the pulled layer and schedules one
            // coalesced re-pull.
            ConfigurationMode::Pull => self.handle_pull_mode_configuration_change().await,
            ConfigurationMode::PushFallback | ConfigurationMode::InitializationOnly => {
                self.apply_session_configuration_change(&params.settings)
                    .await;
            }
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        self.trace_inbound(
            "notification",
            "workspace/didChangeWatchedFiles",
            self.verbose_params_bytes(&params),
        )
        .await;
        let (config_changed, workspace_graph_changed) =
            self.watched_file_change_kinds(&params.changes);
        if config_changed {
            self.reload_repository_config().await;
        }
        if workspace_graph_changed {
            self.invalidate_analysis_input_and_end_queued_progress(
                "workspace_manifest_or_lockfile_changed",
            )
            .await;
            self.publish_analysis_status().await;
            self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::ConfigReload)
                .await;
        }
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        self.trace_inbound(
            "notification",
            "workspace/didChangeWorkspaceFolders",
            self.verbose_params_bytes(&params),
        )
        .await;
        // 1. Apply the event delta to the stored workspace-folder set (#2036,
        //    RIPR-SPEC-0139); the delta is never discarded. Validation runs
        //    before any mutation, so a rejected event leaves the set
        //    untouched and surfaces a typed bounded status instead of a
        //    silent fallback. The folder-set and root epochs are captured
        //    under the same lock acquisition so the reconciliation
        //    round-trip below binds to this exact set state.
        let delta = {
            let Ok(mut set) = self.workspace_folders.lock() else {
                return;
            };
            set.apply_event(&params.event.added, &params.event.removed)
                .map(|outcome| (outcome, self.workspace_root_epoch.load(Ordering::SeqCst)))
        };
        let (outcome, root_epoch) = match delta {
            Ok(pair) => pair,
            Err(rejection) => {
                self.reject_workspace_folder_update(rejection).await;
                return;
            }
        };
        // 2. Optional full-list reconciliation: a separately versioned
        //    confirmation step, never a substitute for the delta. A client
        //    that cannot answer keeps the delta-derived state.
        let queried = self.client.workspace_folders().await.unwrap_or(None);
        // 3. Drop the round-trip when a newer event or transition won the
        //    race, then apply at most one transition derived from the stored
        //    set.
        let action = {
            let Ok(mut set) = self.workspace_folders.lock() else {
                return;
            };
            if set.folder_set_epoch() != outcome.folder_set_epoch
                || set.last_applied_event() != Some(outcome.event_id)
                || self.workspace_root_epoch.load(Ordering::SeqCst) != root_epoch
            {
                return;
            }
            match queried {
                Some(folders) if outcome.changed => {
                    // The accepted delta is authoritative over the
                    // round-trip: the answer is consistency-checked but
                    // NEVER installed. A consistent answer (same path set)
                    // merely confirms the stored set; a lagging,
                    // contradictory answer (e.g. the pre-delta list) is
                    // dropped without mutating — installing it would undo
                    // the accepted delta and reintroduce the stale
                    // full-list race this handler exists to close. Either
                    // way the delta-derived authority is applied below.
                    let _contradictory_answer_dropped = !set.matches_folder_list_paths(&folders);
                    Some(Ok(set.folder_set_epoch()))
                }
                Some(folders) => {
                    // No accepted delta: the full-list answer is the
                    // drift-correction path and may replace the set.
                    match set.replace_from_folder_list(&folders) {
                        Ok(replaced) => {
                            if replaced {
                                Some(Ok(set.folder_set_epoch()))
                            } else {
                                None
                            }
                        }
                        Err(rejection) => Some(Err(rejection)),
                    }
                }
                None if outcome.changed => Some(Ok(set.folder_set_epoch())),
                None => None,
            }
        };
        let folder_set_epoch = match action {
            None => return,
            Some(Err(rejection)) => {
                self.reject_workspace_folder_update(rejection).await;
                return;
            }
            Some(Ok(folder_set_epoch)) => folder_set_epoch,
        };
        // Derive the authority from the stored set in one lock acquisition
        // (re-reading the epoch alongside) and apply it epoch-bound: a set
        // that advanced in the meantime makes the guard drop this
        // application, and the newer event's handler converges the
        // authority to the latest set.
        let derived = {
            let Ok(set) = self.workspace_folders.lock() else {
                return;
            };
            let resolution = match set.selection() {
                WorkspaceFolderSelection::NoFolders => None,
                WorkspaceFolderSelection::SingleFolder => set
                    .entries()
                    .first()
                    .map(|entry| WorkspaceRootResolution::Selected(entry.path.clone())),
                WorkspaceFolderSelection::AmbiguousFolders => {
                    Some(WorkspaceRootResolution::Ambiguous(
                        set.entries()
                            .iter()
                            .map(|entry| entry.path.clone())
                            .collect(),
                    ))
                }
            };
            (resolution, set.folder_set_epoch())
        };
        if derived.1 != folder_set_epoch {
            return;
        }
        let authority = match derived.0 {
            None => WorkspaceRootAuthority::removed(self.effective_root()),
            Some(resolution) => Self::workspace_root_authority_for_resolution(resolution),
        };
        self.apply_workspace_folder_set_authority(authority, folder_set_epoch)
            .await;
        self.reload_repository_config().await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        self.trace_inbound("request", "shutdown", None).await;
        self.refresh_scheduler.stop();
        self.progress.end_all(AnalysisProgressEnd::Cancelled).await;
        self.clear_all_diagnostic_uris();
        self.reset_health_for_input_change();
        self.publish_analysis_status().await;
        self.refresh_idle.notify_waiters();
        let result = Ok(());
        self.trace_response("shutdown", &result).await;
        result
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.trace_inbound(
            "notification",
            "textDocument/didOpen",
            self.verbose_params_bytes(&params),
        )
        .await;
        // No digest seeding from the didOpen text: it may carry unsaved or
        // recovered buffer text rather than persisted bytes (#2129). The
        // saved-content identity is read from the persisted bytes inside the
        // document store, and the dedup digest is still only recorded from
        // didSave, whose content is persisted by definition. Opening with a
        // buffer that diverges from the analyzed saved content quarantines
        // the document and withdraws its line-local diagnostics (#1970).
        let Some((uri, transition)) = self.open_document(params) else {
            // A failed store lock means the document was never registered;
            // skip every downstream mutation so the store, the workspace
            // revision, and the committed snapshot cannot diverge.
            self.client
                .log_message(
                    MessageType::ERROR,
                    "ripr didOpen dropped: the document store is unavailable; the document was not registered and no refresh was scheduled",
                )
                .await;
            return;
        };
        self.advance_workspace_revision();
        self.handle_document_quarantine_transition(&uri, transition)
            .await;
        // Interactive path: defer the seam inventory (RIPR-SPEC-0105).
        // Diff-scoped findings are complete; seams run on explicit refresh only.
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::DidOpen)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.trace_inbound(
            "notification",
            "textDocument/didChange",
            self.verbose_params_bytes(&params),
        )
        .await;
        // A buffer that starts or stops diverging from the analyzed saved
        // content crosses a quarantine edge (#1970): withdraw or re-serve
        // the document's line-local diagnostics immediately rather than
        // waiting for the next save-triggered refresh.
        let Some((uri, transition)) = self.change_document(params) else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    "ripr didChange dropped: the document store is unavailable; the buffer change was not registered",
                )
                .await;
            return;
        };
        self.handle_document_quarantine_transition(&uri, transition)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.trace_inbound(
            "notification",
            "textDocument/didClose",
            self.verbose_params_bytes(&params),
        )
        .await;
        if let Ok(mut digests) = self.saved_content_digests.lock() {
            digests.remove(&params.text_document.uri);
        }
        self.close_document(params);
        self.advance_workspace_revision();
        // Interactive path: defer the seam inventory (RIPR-SPEC-0105).
        self.refresh_diagnostics(RefreshScope::Interactive, RefreshReason::DidClose)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.trace_inbound(
            "notification",
            "textDocument/didSave",
            self.verbose_params_bytes(&params),
        )
        .await;
        // Deduplicate by saved-content input identity (#1908): a save whose
        // bytes did not change since the last recorded save cannot change
        // analysis input, so it neither advances the workspace revision nor
        // schedules a refresh. The save event is still disclosed.
        let uri = params.text_document.uri;
        let text = params.text.or_else(|| self.document_text(&uri));
        let digest = text.as_deref().map(|text| content_digest(text.as_bytes()));
        // Update the per-document saved-content identity and quarantine
        // state first (#1970): the dedup decision below gates the refresh,
        // never the quarantine lift. A save whose buffer again matches the
        // analyzed saved content re-serves the document even when no
        // refresh is scheduled. A failed store lock means the save was never
        // registered; skip every downstream mutation so the store, the
        // dedup ledger, and the committed snapshot cannot diverge.
        let Some(transition) = self.save_document(&uri, digest.clone(), text) else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    "ripr didSave dropped: the document store is unavailable; the save was not registered and no refresh was scheduled",
                )
                .await;
            return;
        };
        if let Some(digest) = &digest
            && self.saved_content_digest_matches(&uri, digest)
        {
            self.handle_document_quarantine_transition(&uri, transition)
                .await;
            self.client
                .log_message(
                    MessageType::INFO,
                    "ripr didSave deduplicated: saved content is unchanged since the last recorded save; no refresh scheduled",
                )
                .await;
            return;
        }
        if let Some(digest) = digest {
            self.record_saved_content_digest(&uri, digest);
        }
        self.handle_document_quarantine_transition(&uri, transition)
            .await;
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
        self.trace_inbound(
            "request",
            "textDocument/diagnostic",
            self.verbose_params_bytes(&params),
        )
        .await;
        let result = self.document_diagnostic_inner(params).await;
        self.trace_response("textDocument/diagnostic", &result)
            .await;
        result
    }

    async fn workspace_diagnostic(
        &self,
        params: WorkspaceDiagnosticParams,
    ) -> LspResult<WorkspaceDiagnosticReportResult> {
        self.trace_inbound(
            "request",
            "workspace/diagnostic",
            self.verbose_params_bytes(&params),
        )
        .await;
        let result = self.workspace_diagnostic_inner(params).await;
        self.trace_response("workspace/diagnostic", &result).await;
        result
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        self.trace_inbound(
            "request",
            "textDocument/hover",
            self.verbose_params_bytes(&params),
        )
        .await;
        let result = Ok(Some(
            self.hover_for_position(&params)
                .unwrap_or_else(hover_response),
        ));
        self.trace_response("textDocument/hover", &result).await;
        result
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        self.trace_inbound(
            "request",
            "textDocument/codeAction",
            self.verbose_params_bytes(&params),
        )
        .await;
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
        let result = Ok(Some(code_action_response(
            &params,
            action_snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
        )));
        self.trace_response("textDocument/codeAction", &result)
            .await;
        result
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
        self.trace_inbound(
            "request",
            "textDocument/codeLens",
            self.verbose_params_bytes(&params),
        )
        .await;
        let snapshot = self
            .latest_analysis
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let uri = &params.text_document.uri;
        let result = Ok(Some(code_lens_response(
            uri,
            snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
        )));
        self.trace_response("textDocument/codeLens", &result).await;
        result
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        self.trace_inbound(
            "request",
            "workspace/executeCommand",
            self.verbose_params_bytes(&params),
        )
        .await;
        let result = self.execute_command_inner(params).await;
        self.trace_response("workspace/executeCommand", &result)
            .await;
        result
    }
}

fn context_arguments(arguments: &[LSPAny]) -> Option<&serde_json::Map<String, serde_json::Value>> {
    let first = arguments.first()?;
    first.as_object()
}

impl Backend {
    /// Inner `textDocument/diagnostic` handler, wrapped by the trait
    /// method so redacted protocol tracing (#2035, RIPR-SPEC-0137)
    /// covers every exit point.
    async fn document_diagnostic_inner(
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
        // A quarantined (dirty-buffer) document is served an empty full
        // report under a distinct result id (#1970): saved-state line-local
        // diagnostics no longer match the client's buffer, and the distinct
        // id keeps the client from treating the withdrawn state as the
        // previously served one.
        let quarantine = self.document_quarantine(&uri);
        let mut result_id = result_ids.document_id(&snapshot, &uri);
        if quarantine.is_some() {
            result_id = quarantined_document_result_id(&result_id);
        }
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
        if quarantine.is_some() {
            if !snapshot.served_diagnostics_for_uri(&uri).is_empty() {
                self.disclose_withdrawal_once(&uri).await;
            }
            return Ok(
                DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                    related_documents: None,
                    full_document_diagnostic_report:
                        tower_lsp_server::ls_types::FullDocumentDiagnosticReport {
                            result_id: Some(result_id),
                            items: Vec::new(),
                        },
                })
                .into(),
            );
        }
        // Serve exactly the stored delivery selection's per-document set
        // (#1973): the same membership push publishes, never a re-evaluated
        // budget. A partial delivery state is disclosed, not hidden behind an
        // empty report.
        if let Some(disclosure) = pull_delivery_disclosure(&snapshot) {
            self.client
                .log_message(MessageType::WARNING, disclosure)
                .await;
        }
        let diagnostics = snapshot.served_diagnostics_for_uri(&uri);
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

    /// Inner `workspace/diagnostic` handler, wrapped by the trait
    /// method so redacted protocol tracing (#2035, RIPR-SPEC-0137)
    /// covers every exit point.
    async fn workspace_diagnostic_inner(
        &self,
        params: WorkspaceDiagnosticParams,
    ) -> LspResult<WorkspaceDiagnosticReportResult> {
        // Typed ingress bound (#2034): reject an oversized previousResultIds
        // set before the URI set clone and per-document scan, and before the
        // snapshot fast path so a missing snapshot cannot skip the bound.
        check_previous_result_ids(&params.previous_result_ids)?;
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
        // Serve the stored delivery selection (#1973). Disclose a partial
        // delivery state once per workspace report rather than once per
        // document; the per-document membership still comes from the stored
        // selection so push and pull agree.
        let mut disclosed = false;
        for uri in uris {
            // Quarantined (dirty-buffer) documents serve an empty set under
            // a distinct result id, same as the document pull handler
            // (#1970); other documents are unaffected.
            let quarantined = self.document_quarantine(&uri).is_some();
            let mut result_id = result_ids.document_id(&snapshot, &uri);
            if quarantined {
                result_id = quarantined_document_result_id(&result_id);
            }
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
            if !disclosed {
                if let Some(disclosure) = pull_delivery_disclosure(&snapshot) {
                    self.client
                        .log_message(MessageType::WARNING, disclosure)
                        .await;
                }
                disclosed = true;
            }
            let diagnostics = if quarantined {
                if !snapshot.served_diagnostics_for_uri(&uri).is_empty() {
                    self.disclose_withdrawal_once(&uri).await;
                }
                Vec::new()
            } else {
                snapshot.served_diagnostics_for_uri(&uri)
            };
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

    /// Inner `workspace/executeCommand` handler, wrapped by the trait
    /// method so redacted protocol tracing (#2035, RIPR-SPEC-0137)
    /// covers every exit point.
    async fn execute_command_inner(
        &self,
        params: ExecuteCommandParams,
    ) -> LspResult<Option<LSPAny>> {
        // Typed ingress bound (#2034): reject oversized argument payloads
        // before any command dispatch, refresh, or packet lookup.
        check_execute_command_arguments(&params.arguments)?;
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
            return Ok(self.collect_context_packet(&params.arguments).await);
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

    async fn collect_context_packet(&self, arguments: &[LSPAny]) -> Option<LSPAny> {
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
                // Client-visible warning, not hidden stderr (#1997): the
                // bounded/redacted message goes to the standard log channel.
                self.client
                    .log_message(
                        MessageType::WARNING,
                        super::component_outcome::bounded_message(&warning),
                    )
                    .await;
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

    /// Per-document dirty/quarantine projection for the workspace status
    /// payload (#1970). Names each open document's buffer state and the
    /// saved/analyzed content identities so the status surface shows the
    /// saved-workspace authority explicitly instead of presenting
    /// saved-state diagnostics as current for a dirty buffer. Identities
    /// are SHA-256 digests of saved content only; unsaved buffer text is
    /// never included.
    fn open_document_statuses_json(&self) -> serde_json::Value {
        let Ok(documents) = self.documents.lock() else {
            return serde_json::json!([]);
        };
        let statuses = documents
            .documents
            .values()
            .map(|state| {
                let quarantined = state.is_quarantined();
                serde_json::json!({
                    "uri": state.uri.as_str(),
                    "path": state.path.display().to_string(),
                    "version": state.version,
                    "state": if quarantined { "quarantined" } else { "clean" },
                    "diagnostics_authority": "saved_workspace",
                    "line_local_diagnostics": if quarantined { "withdrawn" } else { "served" },
                    "staleness_reason": state
                        .quarantine
                        .as_ref()
                        .map(|quarantine| quarantine.reason.as_str()),
                    "last_saved_content_identity": state.saved_digest,
                    "analyzed_saved_content_identity": state.analyzed_saved_digest,
                    "analyzed_input_identity": state.analyzed_input_identity,
                })
            })
            .collect::<Vec<_>>();
        serde_json::Value::Array(statuses)
    }

    fn collect_workspace_status(&self) -> Option<LSPAny> {
        let health = self.analysis_health_snapshot();
        let authority = self.workspace_root_authority();
        let latest_analysis = self.latest_analysis.lock().ok()?.clone();
        let open_documents = self.open_document_statuses_json();
        let snapshot = match latest_analysis {
            None => {
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
                    "diagnostic_budget": serde_json::Value::Null,
                    "diagnostic_budget_state": {
                        "status": "unavailable",
                        "reason": "no_snapshot",
                    },
                    "top_actionable_packet": serde_json::Value::Null,
                    "top_limitation": top_limitation,
                    "open_documents": open_documents,
                    "report_paths": workspace_status_report_paths(),
                    "refresh_command": REFRESH_COMMAND,
                    "limits_note": "Static evidence only; advisory, not a gate decision.",
                }));
            }
            Some(snapshot) => snapshot,
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

        // Project the stored delivery selection (#1973): the same outcome
        // push publication and both pull handlers serve, not a re-evaluated
        // budget. A snapshot without a committed selection (unit-test
        // fixture) reports the selection state honestly as not committed.
        let (diagnostic_budget_json, diagnostic_budget_state) = match &snapshot.delivery_selection {
            Some(selection) => match &selection.outcome {
                DiagnosticDeliveryOutcome::Applied { result, .. } => (
                    diagnostic_budget_result_json(result),
                    serde_json::json!({
                        "status": "available",
                        "inline_detail_measurement": "not_available",
                    }),
                ),
                DiagnosticDeliveryOutcome::Unavailable { reason, detail } => (
                    serde_json::Value::Null,
                    diagnostic_budget_unavailable_state(reason.as_status_reason(), detail),
                ),
            },
            None => (
                serde_json::Value::Null,
                serde_json::json!({
                    "status": "unavailable",
                    "reason": "selection_not_committed",
                }),
            ),
        };

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
                        "out_of_scope_test_file_findings": snapshot.out_of_scope_test_file_findings,
                    },
                    "diagnostic_budget": diagnostic_budget_json.clone(),
                    "diagnostic_budget_state": diagnostic_budget_state.clone(),
                    "top_actionable_packet": top_actionable_packet,
                    "top_limitation": top_limitation,
                    "receipt_status_summary": serde_json::Value::Null,
                    "open_documents": open_documents,
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
                "out_of_scope_test_file_findings": snapshot.out_of_scope_test_file_findings,
            },
            "diagnostic_budget": diagnostic_budget_json,
            "diagnostic_budget_state": diagnostic_budget_state,
            "top_actionable_packet": top_actionable_packet,
            "top_limitation": top_limitation,
            "receipt_status_summary": receipt_status_summary,
            "open_documents": open_documents,
            "report_paths": workspace_status_report_paths(),
            "refresh_command": REFRESH_COMMAND,
            "limits_note": "Static evidence only; advisory, not a gate decision.",
        }))
    }
}

fn diagnostic_budget_unavailable_state(
    reason: &'static str,
    error: impl std::fmt::Display,
) -> serde_json::Value {
    serde_json::json!({
        "status": "unavailable",
        "reason": reason,
        "detail": error.to_string(),
    })
}

fn diagnostic_budget_result_json(
    result: &crate::lsp::diagnostic_budget::DiagnosticBudgetResult,
) -> serde_json::Value {
    let selected = result
        .selected
        .iter()
        .map(|item| {
            serde_json::json!({
                "canonical_id": item.canonical_id,
                "document": item.document,
                "payload_bytes": item.payload_bytes,
                "inline_detail_omitted": item.inline_detail_omitted,
            })
        })
        .collect::<Vec<_>>();
    let omitted = result
        .omitted
        .iter()
        .map(|item| {
            serde_json::json!({
                "canonical_id": item.canonical_id,
                "reason": omitted_diagnostic_reason_name(item.reason),
            })
        })
        .collect::<Vec<_>>();
    let overflow_reasons = result
        .overflow_reasons
        .iter()
        .map(|reason| overflow_reason_name(*reason))
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": result.schema_version,
        "snapshot_profile_budget_identity": result.snapshot_profile_budget_identity,
        "complete_evidence_identity": result.complete_evidence_identity,
        "continuation_or_inspect_route": result.continuation_or_inspect_route,
        "selection_basis_version": result.selection_basis_version,
        "total_canonical_items": result.total_canonical_items,
        "eligible_items": result.eligible_items,
        "selected_count": result.selected.len(),
        "omitted_count": result.omitted.len(),
        "selected": selected,
        "omitted": omitted,
        "selected_bytes": result.selected_bytes,
        "complete_bytes": result.complete_bytes,
        "overflowed": result.overflowed,
        "overflow_reasons": overflow_reasons,
        "inline_detail_measurement": "not_available",
    })
}

fn omitted_diagnostic_reason_name(
    reason: crate::lsp::diagnostic_budget::OmittedDiagnosticReason,
) -> &'static str {
    match reason {
        crate::lsp::diagnostic_budget::OmittedDiagnosticReason::ProfileFiltered => {
            "profile_filtered"
        }
        crate::lsp::diagnostic_budget::OmittedDiagnosticReason::DocumentItemLimit => {
            "document_item_limit"
        }
        crate::lsp::diagnostic_budget::OmittedDiagnosticReason::WorkspaceItemLimit => {
            "workspace_item_limit"
        }
        crate::lsp::diagnostic_budget::OmittedDiagnosticReason::SerializedByteLimit => {
            "serialized_byte_limit"
        }
    }
}

fn overflow_reason_name(
    reason: crate::lsp::diagnostic_budget::DiagnosticOverflowReason,
) -> &'static str {
    match reason {
        crate::lsp::diagnostic_budget::DiagnosticOverflowReason::DocumentItemLimit => {
            "document_item_limit"
        }
        crate::lsp::diagnostic_budget::DiagnosticOverflowReason::WorkspaceItemLimit => {
            "workspace_item_limit"
        }
        crate::lsp::diagnostic_budget::DiagnosticOverflowReason::SerializedByteLimit => {
            "serialized_byte_limit"
        }
        crate::lsp::diagnostic_budget::DiagnosticOverflowReason::InlineDetailLimit => {
            "inline_detail_limit"
        }
    }
}

/// Bound on omitted canonical identities embedded in the push-delivery
/// omission disclosure. The full omission list stays available through the
/// continuation route and the workspace-status budget projection.
const PUSH_BUDGET_DISCLOSURE_MAX_OMITTED_ITEMS: usize = 20;

/// Compute the one delivery selection for a snapshot (#1973). Called once at
/// refresh-transaction prepare time (before any publication); the result is
/// stored on the snapshot and read by push publication and both pull
/// handlers, so no transport re-evaluates the budget with a different
/// result.
fn compute_delivery_selection(
    snapshot: &AnalysisSnapshot,
) -> Arc<crate::lsp::diagnostic_budget::DiagnosticDeliverySelection> {
    let complete_evidence_identity =
        super::diagnostics::complete_diagnostic_evidence_identity(snapshot);
    let snapshot_profile_identity = snapshot
        .input_identity_id()
        .unwrap_or_else(|| complete_evidence_identity.clone());
    Arc::new(
        crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &snapshot.diagnostics_by_uri,
            &crate::lsp::diagnostic_budget::DiagnosticBudget::default(),
            &snapshot_profile_identity,
            &complete_evidence_identity,
        ),
    )
}

/// Bounded pull-side disclosure for a partial delivery state (#1973).
/// Returns `None` when the stored selection is complete — profile filtering
/// alone does not make a delivery partial — mirroring the push omission
/// disclosure. When the pull transport serves a partial, collapsed, or
/// unfiltered-fallback delivery, the state is named and the retrieval route
/// for the complete evidence is given; an empty or unchanged report never
/// hides the overflow.
fn pull_delivery_disclosure(snapshot: &AnalysisSnapshot) -> Option<String> {
    let selection = snapshot.delivery_selection.as_ref()?;
    match &selection.outcome {
        DiagnosticDeliveryOutcome::Applied { result, .. } => {
            let route = result.continuation_or_inspect_route.as_str();
            if result.selected.is_empty() && result.total_canonical_items > 0 {
                return Some(format!(
                    "ripr pull diagnostic delivery served zero of {} items: every item was omitted by the delivery budget (profile filtering or delivery limits); retrieve the complete evidence via {route}",
                    result.total_canonical_items
                ));
            }
            if result.overflowed {
                let overflow_reasons = result
                    .overflow_reasons
                    .iter()
                    .map(|reason| overflow_reason_name(*reason))
                    .collect::<Vec<_>>()
                    .join(",");
                return Some(format!(
                    "ripr pull diagnostic delivery is partial: {} of {} items omitted by the delivery budget (overflow: {overflow_reasons}); retrieve the omitted evidence via {route}",
                    result.omitted.len(),
                    result.total_canonical_items
                ));
            }
            None
        }
        DiagnosticDeliveryOutcome::Unavailable { detail, .. } => Some(format!(
            "ripr pull diagnostic delivery budget unavailable ({detail}); served all diagnostics unfiltered: budget enforcement was not applied, delivery completeness is unknown"
        )),
    }
}

/// Bounded machine-readable omission disclosure for one push publication
/// round. Returns `None` when the budget did not overflow — profile filtering
/// alone does not make a publication partial — so non-overflowed publications
/// stay silent. Field names mirror `diagnostic_budget_result_json` so the log
/// payload and the workspace-status budget projection agree.
fn push_budget_omission_disclosure(
    result: &crate::lsp::diagnostic_budget::DiagnosticBudgetResult,
    budget: &crate::lsp::diagnostic_budget::DiagnosticBudget,
    document_by_canonical_id: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    if !result.overflowed {
        return None;
    }
    let mut omitted_by_document: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for item in &result.omitted {
        if let Some(document) = document_by_canonical_id.get(&item.canonical_id) {
            *omitted_by_document.entry(document.clone()).or_insert(0) += 1;
        }
    }
    let omitted_items = result
        .omitted
        .iter()
        .take(PUSH_BUDGET_DISCLOSURE_MAX_OMITTED_ITEMS)
        .map(|item| {
            serde_json::json!({
                "canonical_id": item.canonical_id,
                "reason": omitted_diagnostic_reason_name(item.reason),
            })
        })
        .collect::<Vec<_>>();
    let overflow_reasons = result
        .overflow_reasons
        .iter()
        .map(|reason| overflow_reason_name(*reason))
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "kind": "push_delivery_budget_omission",
        "schema_version": result.schema_version,
        "budget": {
            "max_items_per_document": budget.max_items_per_document,
            "max_items_per_workspace_response": budget.max_items_per_workspace_response,
            "max_serialized_bytes": budget.max_serialized_bytes,
            "max_inline_detail_bytes": budget.max_inline_detail_bytes,
        },
        "total_canonical_items": result.total_canonical_items,
        "eligible_items": result.eligible_items,
        "complete_bytes": result.complete_bytes,
        "selected_count": result.selected.len(),
        "selected_bytes": result.selected_bytes,
        "omitted_count": result.omitted.len(),
        "overflow_reasons": overflow_reasons,
        "omitted_by_document": omitted_by_document,
        "omitted_items": omitted_items,
        "omitted_items_total": result.omitted.len(),
        "omitted_items_truncated": result.omitted.len() > PUSH_BUDGET_DISCLOSURE_MAX_OMITTED_ITEMS,
        "continuation_or_inspect_route": result.continuation_or_inspect_route,
    });
    Some(format!(
        "ripr push diagnostic delivery budget overflowed; publication is partial: {payload}"
    ))
}

/// Disclosure for the budget-error fallback: every diagnostic is published
/// unfiltered, so no delivery limit was enforced — a partial state that must
/// be named rather than presented as a normal complete publication.
fn push_budget_zero_selection_log_message(
    result: &crate::lsp::diagnostic_budget::DiagnosticBudgetResult,
) -> String {
    format!(
        "ripr push diagnostic delivery budget selected zero of {} items; published nothing for this round: every item was omitted by the budget (profile filtering or delivery limits)",
        result.total_canonical_items
    )
}

fn push_budget_unavailable_log_message(detail: &str) -> String {
    format!(
        "ripr push diagnostic delivery budget unavailable ({detail}); published all diagnostics unfiltered: budget enforcement was not applied, delivery completeness is unknown"
    )
}

/// The pull result id for a quarantined document (#1970). Distinct from the
/// served-set id so a client cannot treat the withdrawn (empty) report as
/// unchanged relative to the previously served diagnostics.
fn quarantined_document_result_id(base: &str) -> String {
    format!("{base}:quarantined")
}

/// Disclosure emitted once per quarantine episode when a dirty document's
/// line-local diagnostics are withdrawn (#1970).
fn quarantine_withdrawal_log_message(path: &Path, reason: DocumentStalenessReason) -> String {
    format!(
        "ripr: line-local diagnostics withdrawn for {}: {} ({}); save the file to analyze the new saved state",
        path.display(),
        reason.description(),
        reason.as_str(),
    )
}

/// Disclosure emitted when a document's quarantine lifts and its line-local
/// diagnostics are served again (#1970).
fn quarantine_restored_log_message(path: &Path) -> String {
    format!(
        "ripr: line-local diagnostics restored for {}: the buffer matches the analyzed saved content again",
        path.display(),
    )
}

#[cfg(test)]
mod diagnostic_budget_projection_tests {
    use super::*;

    #[test]
    fn workspace_status_budget_json_preserves_all_reason_variants() {
        use crate::lsp::diagnostic_budget::{
            DiagnosticBudgetResult, DiagnosticOverflowReason, OmittedDiagnosticItem,
            OmittedDiagnosticReason, SelectedDiagnosticItem,
        };

        let result = DiagnosticBudgetResult {
            schema_version: crate::lsp::diagnostic_budget::DIAGNOSTIC_BUDGET_SCHEMA_VERSION,
            snapshot_profile_budget_identity: "profile:workspace".to_string(),
            complete_evidence_identity: "evidence:complete".to_string(),
            continuation_or_inspect_route: "ripr/inspect".to_string(),
            selection_basis_version:
                crate::lsp::diagnostic_budget::DIAGNOSTIC_BUDGET_SELECTION_VERSION,
            total_canonical_items: 5,
            eligible_items: 1,
            selected: vec![SelectedDiagnosticItem {
                canonical_id: "finding:selected".to_string(),
                document: "file:///workspace/src/lib.rs".to_string(),
                payload_bytes: 17,
                inline_detail_omitted: true,
            }],
            omitted: vec![
                OmittedDiagnosticItem {
                    canonical_id: "finding:profile".to_string(),
                    reason: OmittedDiagnosticReason::ProfileFiltered,
                },
                OmittedDiagnosticItem {
                    canonical_id: "finding:document".to_string(),
                    reason: OmittedDiagnosticReason::DocumentItemLimit,
                },
                OmittedDiagnosticItem {
                    canonical_id: "finding:workspace".to_string(),
                    reason: OmittedDiagnosticReason::WorkspaceItemLimit,
                },
                OmittedDiagnosticItem {
                    canonical_id: "finding:bytes".to_string(),
                    reason: OmittedDiagnosticReason::SerializedByteLimit,
                },
            ],
            selected_bytes: 17,
            complete_bytes: 100,
            overflowed: true,
            overflow_reasons: std::collections::BTreeSet::from([
                DiagnosticOverflowReason::DocumentItemLimit,
                DiagnosticOverflowReason::WorkspaceItemLimit,
                DiagnosticOverflowReason::SerializedByteLimit,
                DiagnosticOverflowReason::InlineDetailLimit,
            ]),
        };

        let json = diagnostic_budget_result_json(&result);
        assert_eq!(json["schema_version"], "lsp-diagnostic-budget-v1");
        assert_eq!(json["selected"][0]["canonical_id"], "finding:selected");
        assert_eq!(json["selected"][0]["payload_bytes"], 17);
        assert_eq!(json["selected"][0]["inline_detail_omitted"], true);
        assert_eq!(json["omitted"][0]["reason"], "profile_filtered");
        assert_eq!(json["omitted"][1]["reason"], "document_item_limit");
        assert_eq!(json["omitted"][2]["reason"], "workspace_item_limit");
        assert_eq!(json["omitted"][3]["reason"], "serialized_byte_limit");
        assert_eq!(
            json["overflow_reasons"],
            serde_json::json!([
                "document_item_limit",
                "workspace_item_limit",
                "serialized_byte_limit",
                "inline_detail_limit"
            ])
        );
        assert_eq!(json["selected_bytes"], 17);
        assert_eq!(json["complete_bytes"], 100);
        assert_eq!(json["overflowed"], true);
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
    super::diagnostics::derive_run_status(
        &snapshot.findings,
        &snapshot.gap_artifact_rejections,
        &snapshot.gap_artifacts,
        snapshot.seams_deferred,
        snapshot.partial_scope.is_some(),
        &snapshot.component_outcomes,
    )
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

    // RIPR-PROP-0019 (#1999): a `limited_partial_scope` run analyzed a
    // deterministic bounded partition of an over-budget diff. Surface the
    // exact selected/uninspected accounting and the only continuation route
    // (raising the explicit budget overrides) — the partial denominator is
    // never presented as complete scope. Ranked above per-finding static
    // limitations, matching `derive_run_status` precedence.
    if let Some(scope) = snapshot.partial_scope.as_ref() {
        return common(
            "limited_partial_scope",
            "limited_partial_scope",
            "limited",
            format!(
                "analysis inspected {} changed file(s) ({} changed line(s)) of the diff; \
                 at least {} changed file(s) and {} changed line(s) were not inspected \
                 (stop reason: {}); raise RIPR_PARTIAL_DIFF_FILE_BUDGET and/or \
                 RIPR_PARTIAL_DIFF_LINE_BUDGET to widen the analyzed partition",
                scope.selected_files.len(),
                scope.selected_changed_lines,
                scope.uninspected_files_lower_bound,
                scope.uninspected_changed_lines_lower_bound,
                scope.stop_reason.as_str(),
            ),
            "analysis/diff-scope-budget",
            scope.selected_files.iter().take(3).cloned().collect(),
            scope.selected_files.len(),
            scope
                .selected_files
                .len()
                .saturating_add(scope.uninspected_files_lower_bound),
            vec![
                "not a gate, baseline, badge, or RIPR Zero input",
                "not test adequacy",
                "not runtime evidence",
            ],
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
        Some(top_limitation_dto(&health, snapshot.as_deref(), &authority).into_json())
    }

    fn collect_receipt_status(&self) -> Option<LSPAny> {
        let authority = self.workspace_root_authority();
        if !authority.allows_analysis() {
            return Some(root_authority_receipt_status(
                &authority.state,
                self.client_features_projection(),
            ));
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
                    "client_features": self.client_features_projection(),
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
            "client_features": self.client_features_projection(),
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
    use super::gap_artifacts::{GapArtifactRejection, require_actionable_packet_render_fields};

    // The render-field contract is owned by the ingest boundary
    // (`lsp::gap_artifacts::require_actionable_packet_render_fields`,
    // RIPR-SPEC-0087 §8); this command-time re-check calls the same shared
    // validator instead of re-walking packet fields.
    if let Err(rejection) = require_actionable_packet_render_fields(packet) {
        let reason = match rejection {
            GapArtifactRejection::MalformedArtifact(message) => message.to_string(),
            other => other.as_str().to_string(),
        };
        return Some(repair_packet_sentinel(&reason));
    }

    let str_field = |key: &str| -> Option<String> {
        packet
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    };

    // The shared validator above has already established that these fields
    // are present and well-formed; the let-else fallbacks are defensive only.
    let Some(canonical_gap_id) = str_field("canonical_gap_id") else {
        return Some(repair_packet_sentinel(
            "actionable packet is missing canonical_gap_id",
        ));
    };
    let Some(repair_kind) = str_field("repair_kind") else {
        return Some(repair_packet_sentinel(
            "actionable packet is missing repair_kind",
        ));
    };
    let Some(verify_command) = str_field("verify_command") else {
        return Some(repair_packet_sentinel(
            "actionable packet is missing verify_command",
        ));
    };
    let Some(receipt_command) = str_field("receipt_command") else {
        return Some(repair_packet_sentinel(
            "actionable packet is missing receipt_command",
        ));
    };

    let allowed_edit_surface: Vec<serde_json::Value> = packet
        .get("allowed_edit_surface")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let must_not_change: Vec<serde_json::Value> = packet
        .get("must_not_change")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let raw_evidence_refs: Vec<serde_json::Value> = packet
        .get("raw_evidence_refs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

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

#[cfg(test)]
mod push_budget_disclosure_tests {
    use super::*;

    fn push_test_uri() -> Result<tower_lsp_server::ls_types::Uri, String> {
        "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))
    }

    fn headline_diagnostic(id: &str, eligible: bool) -> tower_lsp_server::ls_types::Diagnostic {
        tower_lsp_server::ls_types::Diagnostic {
            message: format!("diagnostic {id}"),
            data: Some(serde_json::json!({
                "diagnostic_id": id,
                "headline_eligible": eligible,
            })),
            ..Default::default()
        }
    }

    fn single_document_batches(
        diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
    ) -> Result<
        std::collections::BTreeMap<
            tower_lsp_server::ls_types::Uri,
            Vec<tower_lsp_server::ls_types::Diagnostic>,
        >,
        String,
    > {
        Ok(std::collections::BTreeMap::from([(
            push_test_uri()?,
            diagnostics,
        )]))
    }

    fn disclosure_payload(message: &str) -> Result<serde_json::Value, String> {
        let json_start = message
            .find('{')
            .ok_or_else(|| format!("disclosure is missing a JSON payload: {message}"))?;
        serde_json::from_str(&message[json_start..])
            .map_err(|err| format!("parse disclosure payload failed: {err}"))
    }

    #[test]
    fn overflowed_push_publication_discloses_bounded_omission_summary() -> Result<(), String> {
        let diagnostics = (0..25)
            .map(|index| headline_diagnostic(&format!("diag:{index:02}"), true))
            .collect::<Vec<_>>();
        let batches = single_document_batches(diagnostics.clone())?;
        let budget = crate::lsp::diagnostic_budget::DiagnosticBudget {
            max_items_per_document: 3,
            max_items_per_workspace_response: 100,
            max_serialized_bytes: 1 << 20,
            max_inline_detail_bytes: 4096,
        };

        let selection = crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &batches,
            &budget,
            "test-snapshot-profile",
            "test-complete-evidence",
        );
        let DiagnosticDeliveryOutcome::Applied {
            result,
            document_by_canonical_id,
            ..
        } = &selection.outcome
        else {
            return Err(format!(
                "expected applied budget outcome, got {:?}",
                selection.outcome
            ));
        };
        assert!(result.overflowed, "budget must report overflow");

        let disclosure = push_budget_omission_disclosure(result, &budget, document_by_canonical_id)
            .ok_or_else(|| "overflowed publication must emit an omission disclosure".to_string())?;
        assert!(
            disclosure.starts_with(
                "ripr push diagnostic delivery budget overflowed; publication is partial: "
            ),
            "disclosure must name the partial publication state: {disclosure}"
        );
        let payload = disclosure_payload(&disclosure)?;
        assert_eq!(payload["kind"], "push_delivery_budget_omission");
        assert_eq!(payload["total_canonical_items"], 25);
        assert_eq!(payload["eligible_items"], 25);
        assert_eq!(payload["selected_count"], 3);
        assert_eq!(payload["omitted_count"], 22);
        assert_eq!(
            payload["overflow_reasons"],
            serde_json::json!(["document_item_limit"])
        );
        assert_eq!(payload["budget"]["max_items_per_document"], 3);
        assert_eq!(payload["budget"]["max_items_per_workspace_response"], 100);
        assert_eq!(
            payload["omitted_by_document"]["file:///workspace/src/lib.rs"], 22,
            "per-document omitted count must cover every omitted identity: {payload}"
        );
        assert!(
            payload["complete_bytes"].as_u64().unwrap_or(0) > 0,
            "complete byte evidence must be present: {payload}"
        );
        let omitted_items = payload["omitted_items"]
            .as_array()
            .ok_or_else(|| format!("omitted_items must be an array: {payload}"))?;
        assert_eq!(
            omitted_items.len(),
            PUSH_BUDGET_DISCLOSURE_MAX_OMITTED_ITEMS,
            "omitted identities must be capped at the disclosure bound: {payload}"
        );
        assert_eq!(payload["omitted_items_total"], 22);
        assert_eq!(payload["omitted_items_truncated"], true);
        assert_eq!(omitted_items[0]["canonical_id"], "diag:03");
        assert_eq!(omitted_items[0]["reason"], "document_item_limit");

        // Selection behavior is unchanged: only budget-selected diagnostics
        // are published for the batch.
        let published =
            selection.diagnostics_for_document("file:///workspace/src/lib.rs", &diagnostics);
        assert_eq!(published.len(), 3);
        assert_eq!(result.selected_ids().count(), 3);
        Ok(())
    }

    #[test]
    fn delivery_filter_reuses_budget_ids_without_reserializing() -> Result<(), String> {
        let mut batches = single_document_batches(vec![
            headline_diagnostic("diag:actionable-a", true),
            headline_diagnostic("diag:actionable-b", true),
            headline_diagnostic("diag:actionable-c", true),
        ])?;
        let budget = crate::lsp::diagnostic_budget::DiagnosticBudget {
            max_items_per_document: 2,
            ..crate::lsp::diagnostic_budget::DiagnosticBudget::default()
        };
        let selection = crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &batches,
            &budget,
            "test-snapshot-profile",
            "test-complete-evidence",
        );
        let DiagnosticDeliveryOutcome::Applied {
            result,
            ids_by_document,
            ..
        } = &selection.outcome
        else {
            return Err(format!(
                "expected applied budget outcome, got {:?}",
                selection.outcome
            ));
        };
        let selected_ids = result
            .selected_ids()
            .map(str::to_string)
            .collect::<std::collections::BTreeSet<_>>();
        if selected_ids.len() != 2 {
            return Err(format!("expected two selected ids, got {selected_ids:?}"));
        }
        let document = batches
            .keys()
            .next()
            .ok_or("missing document")?
            .as_str()
            .to_string();
        let diagnostics = batches.values_mut().next().ok_or("missing batch")?.clone();
        let ordered_ids = ids_by_document.get(document.as_str()).map(Vec::as_slice);
        if ordered_ids.map(<[String]>::len) != Some(diagnostics.len()) {
            return Err(format!(
                "ids_by_document must cover the document in batch order: {ordered_ids:?}"
            ));
        }
        let served = selection.diagnostics_for_document(&document, &diagnostics);
        // Independently reserialize every diagnostic and re-derive canonical
        // identities: the stored-selection filter must agree exactly.
        let reserialized = diagnostics
            .iter()
            .filter(|diagnostic| {
                let payload = serde_json::to_vec(diagnostic).unwrap_or_default();
                let id = crate::lsp::diagnostic_budget::diagnostic_canonical_id(
                    diagnostic, &document, &payload,
                );
                selected_ids.contains(id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        if served != reserialized || served.len() != 2 {
            return Err(format!(
                "stored-selection filter diverged from reserialization: served={} reserialized={}",
                served.len(),
                reserialized.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn zero_selection_serves_nothing_and_names_the_fallback() -> Result<(), String> {
        let batches = single_document_batches(vec![
            headline_diagnostic("diag:advisory-a", false),
            headline_diagnostic("diag:advisory-b", false),
        ])?;
        let selection = crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &batches,
            &crate::lsp::diagnostic_budget::DiagnosticBudget::default(),
            "test-snapshot-profile",
            "test-complete-evidence",
        );
        let DiagnosticDeliveryOutcome::Applied { result, .. } = &selection.outcome else {
            return Err(format!(
                "expected applied budget outcome, got {:?}",
                selection.outcome
            ));
        };
        if !result.selected.is_empty() || result.total_canonical_items != 2 {
            return Err(format!(
                "expected collapsed selection over two items, got {result:?}"
            ));
        }
        let published = selection.diagnostics_for_document(
            "file:///workspace/src/lib.rs",
            batches.values().next().ok_or("missing batch")?,
        );
        if !published.is_empty() {
            return Err(format!(
                "a legitimately empty selection must publish nothing, got {}",
                published.len()
            ));
        }
        let message = push_budget_zero_selection_log_message(result);
        for expected in [
            "selected zero of 2 items",
            "published nothing for this round",
            "every item was omitted by the budget",
        ] {
            if !message.contains(expected) {
                return Err(format!(
                    "zero-selection message missing `{expected}`: {message}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn non_overflowed_push_publication_emits_no_omission_disclosure() -> Result<(), String> {
        let batches = single_document_batches(vec![
            headline_diagnostic("diag:actionable", true),
            headline_diagnostic("diag:other", true),
            headline_diagnostic("diag:advisory", false),
        ])?;
        let selection = crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &batches,
            &crate::lsp::diagnostic_budget::DiagnosticBudget::default(),
            "test-snapshot-profile",
            "test-complete-evidence",
        );
        let DiagnosticDeliveryOutcome::Applied {
            result,
            document_by_canonical_id,
            ..
        } = &selection.outcome
        else {
            return Err(format!(
                "expected applied budget outcome, got {:?}",
                selection.outcome
            ));
        };
        assert!(
            !result.overflowed,
            "profile filtering alone must not count as overflow"
        );
        assert_eq!(
            result.omitted.len(),
            1,
            "the profile-filtered diagnostic is still recorded as omitted"
        );
        assert!(
            push_budget_omission_disclosure(
                result,
                &crate::lsp::diagnostic_budget::DiagnosticBudget::default(),
                document_by_canonical_id,
            )
            .is_none(),
            "non-overflowed publication must stay silent"
        );
        Ok(())
    }

    #[test]
    fn budget_error_fallback_publishes_everything_and_names_the_partial_state() -> Result<(), String>
    {
        let diagnostics = vec![
            headline_diagnostic("diag:first", true),
            headline_diagnostic("diag:second", true),
        ];
        let batches = single_document_batches(diagnostics.clone())?;
        let invalid_budget = crate::lsp::diagnostic_budget::DiagnosticBudget {
            max_items_per_document: 0,
            ..crate::lsp::diagnostic_budget::DiagnosticBudget::default()
        };

        let selection = crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
            &batches,
            &invalid_budget,
            "test-snapshot-profile",
            "test-complete-evidence",
        );
        let DiagnosticDeliveryOutcome::Unavailable { detail, .. } = &selection.outcome else {
            return Err(format!(
                "expected unavailable budget outcome, got {:?}",
                selection.outcome
            ));
        };
        assert!(
            detail.contains("max_items_per_document"),
            "fallback detail must name the failing limit: {detail}"
        );

        // The fallback keeps the #1911 semantics at the call site: an
        // unavailable budget serves the batch unfiltered instead of applying
        // the strict filter.
        let published =
            selection.diagnostics_for_document("file:///workspace/src/lib.rs", &diagnostics);
        assert_eq!(
            published.len(),
            diagnostics.len(),
            "fallback must publish everything unfiltered"
        );

        let message = push_budget_unavailable_log_message(detail);
        assert!(
            message.contains("push diagnostic delivery budget unavailable"),
            "fallback disclosure must name the unavailable budget: {message}"
        );
        assert!(
            message.contains("unfiltered"),
            "fallback disclosure must state delivery was unfiltered: {message}"
        );
        assert!(
            message.contains("budget enforcement was not applied"),
            "fallback disclosure must name the partial state explicitly: {message}"
        );
        Ok(())
    }
}

#[cfg(test)]
mod delivery_selection_parity_tests {
    use super::*;
    use tower_lsp_server::LspService;
    use tower_lsp_server::ls_types::{
        DocumentDiagnosticParams, PartialResultParams, TextDocumentIdentifier,
        WorkspaceDiagnosticParams,
    };

    fn parity_uri(name: &str) -> Result<tower_lsp_server::ls_types::Uri, String> {
        format!("file:///workspace/src/{name}")
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))
    }

    /// A gap-ledger diagnostic whose budget eligibility is explicit. The
    /// `gap_id` data key keeps the snapshot consistency invariant
    /// (findings + seams + gap diagnostics == diagnostic count) satisfiable
    /// without fabricating findings.
    fn parity_diagnostic(id: &str, eligible: bool) -> tower_lsp_server::ls_types::Diagnostic {
        tower_lsp_server::ls_types::Diagnostic {
            message: format!("diagnostic {id}"),
            data: Some(serde_json::json!({
                "diagnostic_id": id,
                "gap_id": id,
                "headline_eligible": eligible,
            })),
            ..Default::default()
        }
    }

    fn parity_workspace_diagnostics(
        documents: Vec<(
            tower_lsp_server::ls_types::Uri,
            Vec<tower_lsp_server::ls_types::Diagnostic>,
        )>,
    ) -> WorkspaceDiagnostics {
        let diagnostics_by_uri = documents.iter().cloned().collect::<BTreeMap<_, _>>();
        let batches = documents
            .into_iter()
            .map(|(uri, diagnostics)| DiagnosticBatch { uri, diagnostics })
            .collect();
        let snapshot = AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            input_identity: Some(
                crate::lsp::input_identity::LspAnalysisInputIdentity::from_refresh_inputs(
                    PathBuf::from("/workspace"),
                    1,
                    &LspAnalysisConfig::default(),
                ),
            ),
            base: Some("origin/main".to_string()),
            mode: crate::app::Mode::Draft,
            refresh: crate::lsp::state::RefreshMetadata::default(),
            findings: Vec::new(),
            diagnostic_profile: LspDiagnosticProfile::Full,
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
        WorkspaceDiagnostics { snapshot, batches }
    }

    fn parity_backend() -> Result<ParityHarness, String> {
        let (service, socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))?;
        Ok(ParityHarness {
            service,
            _socket: socket,
            runtime,
        })
    }

    struct ParityHarness {
        service: tower_lsp_server::LspService<Backend>,
        // Keep the socket alive for the whole test so client log-message
        // disclosures have an open channel, mirroring the other LSP handler
        // tests in this crate.
        _socket: tower_lsp_server::ClientSocket,
        runtime: tokio::runtime::Runtime,
    }

    fn commit(
        backend: &Backend,
        diagnostics: WorkspaceDiagnostics,
    ) -> Result<AnalysisSnapshot, String> {
        backend
            .refresh_plan(diagnostics)
            .ok_or_else(|| "expected committed snapshot".to_string())?;
        backend
            .latest_analysis_snapshot()
            .ok_or_else(|| "expected latest analysis snapshot".to_string())
    }

    fn stored_selection(
        snapshot: &AnalysisSnapshot,
    ) -> Result<&crate::lsp::diagnostic_budget::DiagnosticDeliverySelection, String> {
        snapshot
            .delivery_selection
            .as_deref()
            .ok_or_else(|| "committed snapshot must carry a delivery selection".to_string())
    }

    fn applied_result(
        selection: &crate::lsp::diagnostic_budget::DiagnosticDeliverySelection,
    ) -> Result<&crate::lsp::diagnostic_budget::DiagnosticBudgetResult, String> {
        match &selection.outcome {
            DiagnosticDeliveryOutcome::Applied { result, .. } => Ok(result),
            DiagnosticDeliveryOutcome::Unavailable { detail, .. } => Err(format!(
                "expected applied selection, got unavailable: {detail}"
            )),
        }
    }

    fn diagnostic_ids(
        diagnostics: &[tower_lsp_server::ls_types::Diagnostic],
    ) -> Result<Vec<String>, String> {
        diagnostics
            .iter()
            .map(|diagnostic| {
                diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("diagnostic_id"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| format!("diagnostic missing diagnostic_id: {diagnostic:?}"))
            })
            .collect()
    }

    /// Pull one document through the real async `textDocument/diagnostic`
    /// handler and return the served canonical ids plus the result id.
    fn pulled_document(
        backend: &Backend,
        runtime: &tokio::runtime::Runtime,
        uri: &tower_lsp_server::ls_types::Uri,
        previous_result_id: Option<String>,
    ) -> Result<(String, Vec<String>), String> {
        let report = runtime
            .block_on(backend.diagnostic(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id,
                work_done_progress_params: Default::default(),
                partial_result_params: PartialResultParams::default(),
            }))
            .map_err(|err| format!("document pull failed: {err}"))?;
        let json = serde_json::to_value(report)
            .map_err(|err| format!("serialize document report failed: {err}"))?;
        if json.get("kind").and_then(serde_json::Value::as_str) != Some("full") {
            return Err(format!("expected full document report: {json}"));
        }
        let result_id = json
            .get("resultId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "full document report did not carry resultId".to_string())?
            .to_string();
        let ids = json
            .get("items")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("full document report had no items array: {json}"))?
            .iter()
            .map(|item| {
                item.get("data")
                    .and_then(|data| data.get("diagnostic_id"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| format!("pulled item missing diagnostic_id: {item}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((result_id, ids))
    }

    /// The push-side projection for one snapshot: exactly the computation the
    /// publish loop performs per batch, read from the stored selection.
    fn push_projection(
        snapshot: &AnalysisSnapshot,
    ) -> Result<BTreeMap<String, Vec<String>>, String> {
        let selection = stored_selection(snapshot)?;
        snapshot
            .diagnostics_by_uri
            .iter()
            .map(|(uri, diagnostics)| {
                diagnostic_ids(&selection.diagnostics_for_document(uri.as_str(), diagnostics))
                    .map(|ids| (uri.as_str().to_string(), ids))
            })
            .collect()
    }

    fn sorted(ids: &[String]) -> Vec<String> {
        let mut sorted = ids.to_vec();
        sorted.sort();
        sorted
    }

    #[test]
    fn input_change_reset_clears_component_degradation_dedup() -> Result<(), String> {
        // #1997 review: the degradation-dedup state belongs to the old
        // input/root context; a suppressed signature must re-warn once in the
        // new context instead of staying silently suppressed.
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        {
            let Ok(mut last) = backend.last_component_degradation.lock() else {
                return Err("dedup lock poisoned".to_string());
            };
            *last = Some("seam_inventory=failed:seam_inventory_failed:walk failed".to_string());
        }
        backend.reset_health_for_input_change();
        let cleared = backend
            .last_component_degradation
            .lock()
            .map(|last| last.is_none())
            .unwrap_or(false);
        if !cleared {
            return Err("input-change reset must clear the degradation dedup state".to_string());
        }
        Ok(())
    }

    #[test]
    fn push_and_pull_serve_identical_selected_sets_for_one_snapshot() -> Result<(), String> {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let runtime = &harness.runtime;
        let uri_a = parity_uri("a.rs")?;
        let uri_b = parity_uri("b.rs")?;
        let snapshot = commit(
            backend,
            parity_workspace_diagnostics(vec![
                (
                    uri_a.clone(),
                    vec![
                        parity_diagnostic("gap:a-1", true),
                        parity_diagnostic("gap:a-2", true),
                        parity_diagnostic("gap:a-3", true),
                    ],
                ),
                (
                    uri_b.clone(),
                    vec![
                        parity_diagnostic("gap:b-1", true),
                        parity_diagnostic("gap:b-2", true),
                        parity_diagnostic("gap:b-advisory", false),
                    ],
                ),
            ]),
        )?;
        let selection = stored_selection(&snapshot)?;
        let DiagnosticDeliveryOutcome::Applied {
            selected_ids_by_document,
            ..
        } = &selection.outcome
        else {
            return Err("expected applied selection".to_string());
        };
        let push = push_projection(&snapshot)?;

        // Item-level and per-document parity: the pull handler, the push
        // projection, and the stored per-document selected sets all agree.
        for uri in [&uri_a, &uri_b] {
            let (_, pull_ids) = pulled_document(backend, runtime, uri, None)?;
            let push_ids = push
                .get(uri.as_str())
                .ok_or_else(|| format!("push projection missing {}", uri.as_str()))?;
            if pull_ids != *push_ids {
                return Err(format!(
                    "push/pull item parity failed for {}: pull={pull_ids:?} push={push_ids:?}",
                    uri.as_str()
                ));
            }
            let stored = selected_ids_by_document
                .get(uri.as_str())
                .ok_or_else(|| format!("stored selection missing {}", uri.as_str()))?;
            if sorted(&pull_ids) != sorted(stored) {
                return Err(format!(
                    "per-document membership parity failed for {}: pull={pull_ids:?} stored={stored:?}",
                    uri.as_str()
                ));
            }
        }

        // The workspace pull handler agrees per document as well.
        let workspace = runtime
            .block_on(backend.workspace_diagnostic(WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: Vec::new(),
                work_done_progress_params: Default::default(),
                partial_result_params: PartialResultParams::default(),
            }))
            .map_err(|err| format!("workspace pull failed: {err}"))?;
        let workspace_json = serde_json::to_value(workspace)
            .map_err(|err| format!("serialize workspace report failed: {err}"))?;
        let items = workspace_json
            .get("items")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("workspace report had no items: {workspace_json}"))?;
        for item in items {
            let uri = item
                .get("uri")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("workspace item missing uri: {item}"))?;
            let ids = item
                .get("items")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| format!("workspace item missing diagnostics: {item}"))?
                .iter()
                .filter_map(|diagnostic| {
                    diagnostic
                        .get("data")
                        .and_then(|data| data.get("diagnostic_id"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .collect::<Vec<_>>();
            let push_ids = push
                .get(uri)
                .ok_or_else(|| format!("push projection missing {uri}"))?;
            if ids != *push_ids {
                return Err(format!(
                    "workspace pull diverged from push for {uri}: pull={ids:?} push={push_ids:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn omitted_identities_and_reasons_are_shared_by_both_transports() -> Result<(), String> {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let runtime = &harness.runtime;
        let uri = parity_uri("omitted.rs")?;
        let complete = vec![
            parity_diagnostic("gap:keep-1", true),
            parity_diagnostic("gap:drop-advisory", false),
            parity_diagnostic("gap:keep-2", true),
        ];
        let snapshot = commit(
            backend,
            parity_workspace_diagnostics(vec![(uri.clone(), complete.clone())]),
        )?;
        let selection = stored_selection(&snapshot)?;
        let result = applied_result(selection)?;
        let stored_omitted = result
            .omitted
            .iter()
            .map(|item| (item.canonical_id.clone(), item.reason))
            .collect::<BTreeMap<_, _>>();
        if stored_omitted.len() != 1
            || stored_omitted.get("gap:drop-advisory")
                != Some(&crate::lsp::diagnostic_budget::OmittedDiagnosticReason::ProfileFiltered)
        {
            return Err(format!(
                "expected one profile-filtered omission, got {stored_omitted:?}"
            ));
        }

        // Both transports leave exactly the stored omitted identities
        // unserved, with the stored reasons.
        let (_, pull_ids) = pulled_document(backend, runtime, &uri, None)?;
        let push = push_projection(&snapshot)?;
        let push_ids = push
            .get(uri.as_str())
            .ok_or_else(|| "push projection missing document".to_string())?;
        let complete_ids = diagnostic_ids(&complete)?;
        for (transport, served) in [("pull", &pull_ids), ("push", push_ids)] {
            let unserved = complete_ids
                .iter()
                .filter(|id| !served.contains(id))
                .cloned()
                .collect::<Vec<_>>();
            let stored = stored_omitted.keys().cloned().collect::<Vec<_>>();
            if unserved != stored {
                return Err(format!(
                    "{transport} unserved identities diverge from stored omissions: unserved={unserved:?} stored={stored:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn pull_after_budget_limits_discloses_the_push_overflow_state_and_route() -> Result<(), String>
    {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let runtime = &harness.runtime;
        let uri = parity_uri("overflow.rs")?;
        let diagnostics = (0..60)
            .map(|index| parity_diagnostic(&format!("gap:overflow-{index:02}"), true))
            .collect::<Vec<_>>();
        let snapshot = commit(
            backend,
            parity_workspace_diagnostics(vec![(uri.clone(), diagnostics)]),
        )?;
        let selection = stored_selection(&snapshot)?;
        let result = applied_result(selection)?;
        if !result.overflowed
            || !result.overflow_reasons.contains(
                &crate::lsp::diagnostic_budget::DiagnosticOverflowReason::DocumentItemLimit,
            )
        {
            return Err(format!("expected document-limit overflow, got {result:?}"));
        }
        let route = result.continuation_or_inspect_route.clone();

        // Push disclosure state.
        let DiagnosticDeliveryOutcome::Applied {
            document_by_canonical_id,
            ..
        } = &selection.outcome
        else {
            return Err("expected applied selection".to_string());
        };
        let push_disclosure =
            push_budget_omission_disclosure(result, &selection.budget, document_by_canonical_id)
                .ok_or_else(|| "overflowed push publication must disclose".to_string())?;
        let push_payload: serde_json::Value = serde_json::from_str(
            &push_disclosure[push_disclosure
                .find('{')
                .ok_or_else(|| "push disclosure missing JSON payload".to_string())?..],
        )
        .map_err(|err| format!("parse push disclosure failed: {err}"))?;
        if push_payload["overflow_reasons"] != serde_json::json!(["document_item_limit"])
            || push_payload["continuation_or_inspect_route"] != serde_json::json!(route)
        {
            return Err(format!(
                "push disclosure overflow state mismatch: {push_payload}"
            ));
        }

        // Pull serves the same bounded set and discloses the same state and
        // retrieval route.
        let (result_id, pull_ids) = pulled_document(backend, runtime, &uri, None)?;
        if pull_ids.len() != result.selected.len() {
            return Err(format!(
                "pull must serve the stored selected set: pull={} selected={}",
                pull_ids.len(),
                result.selected.len()
            ));
        }
        let pull_disclosure = pull_delivery_disclosure(&snapshot)
            .ok_or_else(|| "overflowed pull delivery must disclose".to_string())?;
        for expected in ["partial", route.as_str(), "document_item_limit"] {
            if !pull_disclosure.contains(expected) {
                return Err(format!(
                    "pull disclosure missing `{expected}`: {pull_disclosure}"
                ));
            }
        }

        // The workspace-status projection mirrors the same stored overflow
        // state and retrieval route.
        let status = backend
            .collect_workspace_status()
            .ok_or_else(|| "expected workspace status".to_string())?;
        if status["diagnostic_budget"]["overflowed"] != serde_json::json!(true)
            || status["diagnostic_budget"]["overflow_reasons"]
                != serde_json::json!(["document_item_limit"])
            || status["diagnostic_budget"]["continuation_or_inspect_route"]
                != serde_json::json!(route)
        {
            return Err(format!(
                "workspace-status budget projection diverged from the stored selection: {status}"
            ));
        }

        // The overflow is never hidden by an unchanged report: a repeated
        // equivalent pull returns `unchanged` against the served identity.
        let second = runtime
            .block_on(backend.diagnostic(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: Some(result_id),
                work_done_progress_params: Default::default(),
                partial_result_params: PartialResultParams::default(),
            }))
            .map_err(|err| format!("second pull failed: {err}"))?;
        let second_json = serde_json::to_value(second)
            .map_err(|err| format!("serialize second report failed: {err}"))?;
        if second_json.get("kind").and_then(serde_json::Value::as_str) != Some("unchanged") {
            return Err(format!(
                "expected unchanged report for the same selection: {second_json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn zero_selection_snapshot_serves_nothing_on_both_transports_and_discloses()
    -> Result<(), String> {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let runtime = &harness.runtime;
        let uri = parity_uri("zero.rs")?;
        let snapshot = commit(
            backend,
            parity_workspace_diagnostics(vec![(
                uri.clone(),
                vec![
                    parity_diagnostic("gap:advisory-1", false),
                    parity_diagnostic("gap:advisory-2", false),
                ],
            )]),
        )?;
        let selection = stored_selection(&snapshot)?;
        let result = applied_result(selection)?;
        if !result.selected.is_empty() || result.total_canonical_items != 2 {
            return Err(format!(
                "expected a legitimately empty selection over two items, got {result:?}"
            ));
        }

        // Pull serves nothing.
        let (_, pull_ids) = pulled_document(backend, runtime, &uri, None)?;
        if !pull_ids.is_empty() {
            return Err(format!(
                "zero-selection pull must serve nothing: {pull_ids:?}"
            ));
        }
        // Push publishes nothing.
        let push = push_projection(&snapshot)?;
        let push_ids = push
            .get(uri.as_str())
            .ok_or_else(|| "push projection missing document".to_string())?;
        if !push_ids.is_empty() {
            return Err(format!(
                "zero-selection push must publish nothing: {push_ids:?}"
            ));
        }
        // Both transports disclose the collapsed selection with the retrieval
        // route.
        let pull_disclosure = pull_delivery_disclosure(&snapshot)
            .ok_or_else(|| "zero-selection pull delivery must disclose".to_string())?;
        for expected in [
            "served zero of 2 items",
            result.continuation_or_inspect_route.as_str(),
        ] {
            if !pull_disclosure.contains(expected) {
                return Err(format!(
                    "pull zero-selection disclosure missing `{expected}`: {pull_disclosure}"
                ));
            }
        }
        let push_message = push_budget_zero_selection_log_message(result);
        if !push_message.contains("selected zero of 2 items") {
            return Err(format!(
                "push zero-selection disclosure mismatch: {push_message}"
            ));
        }
        Ok(())
    }

    #[test]
    fn pull_reads_the_stored_selection_without_reevaluating_the_budget() -> Result<(), String> {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let runtime = &harness.runtime;
        let uri = parity_uri("stored.rs")?;
        let mut workspace = parity_workspace_diagnostics(vec![(
            uri.clone(),
            vec![
                parity_diagnostic("gap:stored-1", true),
                parity_diagnostic("gap:stored-2", true),
            ],
        )]);
        // A selection computed under a tighter budget than the default: one
        // selected item instead of two. If the pull path re-evaluated the
        // budget with the default limits it would serve both items.
        let tight_budget = crate::lsp::diagnostic_budget::DiagnosticBudget {
            max_items_per_document: 1,
            ..crate::lsp::diagnostic_budget::DiagnosticBudget::default()
        };
        let injected = Arc::new(
            crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
                &workspace.snapshot.diagnostics_by_uri,
                &tight_budget,
                "test-tight-profile",
                "test-tight-evidence",
            ),
        );
        let injected_result = applied_result(&injected)?;
        if injected_result.selected.len() != 1 {
            return Err(format!(
                "tight budget must select exactly one item: {injected_result:?}"
            ));
        }
        let expected_served = injected_result.selected[0].canonical_id.clone();
        workspace.snapshot.delivery_selection = Some(Arc::clone(&injected));

        let snapshot = commit(backend, workspace)?;
        let committed = stored_selection(&snapshot)?;
        // The committed selection is the injected one — stored, not recomputed.
        let committed_arc = snapshot
            .delivery_selection
            .as_ref()
            .ok_or_else(|| "committed snapshot lost its selection".to_string())?;
        if !Arc::ptr_eq(committed_arc, &injected) {
            return Err("commit replaced the stored selection instead of retaining it".to_string());
        }
        let committed_result = applied_result(committed)?;
        if !committed_result
            .snapshot_profile_budget_identity
            .contains("test-tight-profile")
        {
            return Err(format!(
                "committed selection identity is not the injected one: {}",
                committed_result.snapshot_profile_budget_identity
            ));
        }

        let (_, pull_ids) = pulled_document(backend, runtime, &uri, None)?;
        if pull_ids != vec![expected_served.clone()] {
            return Err(format!(
                "pull must serve the stored selection, not a re-evaluated budget: pull={pull_ids:?} stored=[{expected_served}]"
            ));
        }

        // Sanity: the default budget would have served both items, so serving
        // exactly the stored one-item set is only possible by reading the
        // stored selection.
        let default_evaluation =
            crate::lsp::diagnostic_budget::DiagnosticDeliverySelection::evaluate(
                &snapshot.diagnostics_by_uri,
                &crate::lsp::diagnostic_budget::DiagnosticBudget::default(),
                "test-default-profile",
                "test-default-evidence",
            );
        if applied_result(&default_evaluation)?.selected.len() != 2 {
            return Err("default budget must select both items in this fixture".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod work_done_progress_guard_tests {
    use super::*;
    use crate::lsp::progress::ProgressEvent;
    use tower_lsp_server::LspService;

    #[test]
    fn dropped_refresh_guard_ends_active_and_queued_progress_exactly_once() -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))?;
        runtime.block_on(async {
            let (service, _socket) =
                LspService::new(|client| Backend::new(client, PathBuf::from(".")));
            let backend = service.inner();
            let sink = backend.install_progress_recorder();

            let first = backend.refresh_scheduler.request(
                PathBuf::from("/workspace"),
                LspAnalysisConfig::default(),
                1,
                0,
                RefreshScope::Interactive,
                RefreshReason::DidSave,
            );
            let RefreshDecision::Start(first) = &first else {
                return Err(format!("expected Start decision, got {first:?}"));
            };
            let first = first.as_ref().clone();
            backend
                .emit_progress_for_decision(&RefreshDecision::Start(Box::new(first.clone())))
                .await;
            let second = backend.refresh_scheduler.request(
                PathBuf::from("/workspace"),
                LspAnalysisConfig::default(),
                2,
                0,
                RefreshScope::Interactive,
                RefreshReason::DidSave,
            );
            if !matches!(second, RefreshDecision::Queued { .. }) {
                return Err(format!("expected Queued decision, got {second:?}"));
            }
            backend.emit_progress_for_decision(&second).await;

            // Dropping the armed guard simulates the refresh future being
            // cancelled mid-flight: it cancels the active and queued work and
            // must terminate both progress tokens.
            {
                let _guard = RefreshCancellationGuard::new(
                    backend,
                    &backend.refresh_scheduler,
                    &backend.refresh_idle,
                    first.clone(),
                );
            }
            // The end sends are spawned from Drop; let them run.
            for _ in 0..20 {
                tokio::task::yield_now().await;
                let ends = sink
                    .events()
                    .iter()
                    .filter(|event| matches!(event, ProgressEvent::End { .. }))
                    .count();
                if ends == 2 {
                    break;
                }
            }

            let events = sink.events();
            let ends: Vec<(String, String)> = events
                .iter()
                .filter_map(|event| match event {
                    ProgressEvent::End { token, message } => Some((token.clone(), message.clone())),
                    _ => None,
                })
                .collect();
            let expected = vec![
                (
                    "ripr-analysis-1".to_string(),
                    "analysis cancelled".to_string(),
                ),
                (
                    "ripr-analysis-2".to_string(),
                    "analysis cancelled".to_string(),
                ),
            ];
            if ends != expected {
                return Err(format!(
                    "guard drop must cancel both tokens exactly once: {ends:?} in {events:?}"
                ));
            }
            // The guard cancellation path is terminal: a later loop-style end
            // for the same generations must be a no-op.
            backend
                .end_progress_for_attempt(&first, RefreshAttemptOutcome::Cancelled)
                .await;
            let ends_after: usize = sink
                .events()
                .iter()
                .filter(|event| matches!(event, ProgressEvent::End { .. }))
                .count();
            if ends_after != 2 {
                return Err("terminal end must stay exactly-once after guard drop".to_string());
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod quarantine_message_tests {
    use super::*;

    #[test]
    fn quarantined_document_result_id_stays_distinct() {
        assert_eq!(quarantined_document_result_id("doc:1"), "doc:1:quarantined");
        assert_ne!(quarantined_document_result_id("doc:1"), "doc:1");
    }

    #[test]
    fn quarantine_withdrawal_log_message_names_path_reason_and_route() {
        let message = quarantine_withdrawal_log_message(
            Path::new("/workspace/src/lib.rs"),
            DocumentStalenessReason::BufferDivergesFromAnalyzedSavedContent,
        );
        assert!(message.contains("/workspace/src/lib.rs"));
        assert!(message.contains("buffer_diverges_from_analyzed_saved_content"));
        assert!(message.contains("save the file"));
    }

    #[test]
    fn quarantine_restored_log_message_names_path() {
        let message = quarantine_restored_log_message(Path::new("/workspace/src/lib.rs"));
        assert!(message.contains("/workspace/src/lib.rs"));
        assert!(message.contains("restored"));
    }
}
