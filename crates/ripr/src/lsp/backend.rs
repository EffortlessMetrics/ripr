use super::AnalysisStatusNotification;
use super::actions::{code_action_response, resolve_action};
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
    is_gap_diagnostic, markdown_hover,
};
use super::lens::{LensViewIdentity, code_lens_response, lens_view_identity};
use super::payload_bounds::{
    check_execute_command_arguments, check_initialization_options,
    check_list_actionable_items_params, check_previous_result_ids,
};
use super::progress::{AnalysisProgressEnd, AnalysisProgressPhase, AnalysisProgressTracker};
use super::refresh_scheduler::{
    RefreshAttemptOutcome, RefreshDecision, RefreshReason, RefreshRequest, RefreshScheduler,
    RefreshScope,
};
use super::state::{
    AnalysisAttemptState, AnalysisFailure, AnalysisFailureKind, AnalysisHealth, AnalysisSnapshot,
    ConfigPullState, DocumentStalenessReason, DocumentStore, QuarantineTransition,
    WorkspaceFolderEntry, WorkspaceFolderEventRejection, WorkspaceFolderSelection,
    WorkspaceFolderSet, WorkspaceRootAuthority, WorkspaceRootState, content_digest,
    format_duration,
};
use super::uri::{
    CappedArtifactRead, absolute_join, display_path, file_uri_for_path, file_uri_is_within_root,
    file_uris_match, path_from_file_uri, path_is_within_root, read_artifact_capped,
};
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, REFRESH_COMMAND,
};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::analysis::cancellation::{AnalysisAbortKind, is_cancellation_error};
use crate::config::{
    CONFIG_FILE_NAME, LspDiagnosticProfile, PYTHON_PROJECT_MARKERS, PYTHON_SOURCE_DIR_MARKERS,
    is_detectable_python_source_name, is_python_excluded_dir_everywhere,
    python_project_marker_name, python_source_dir_marker_name,
};
use crate::domain::context_packet::ContextPacket;
use crate::domain::{StageEvidence, StageState};
use crate::lsp::diagnostic_budget::DiagnosticDeliveryOutcome;
use crate::output::agent_seam_packets::{
    render_agent_gap_record_packet_json, suggested_assertion_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam, validate_agent_gap_record_packet,
};
use crate::output::evidence_record::repair_start_command_for;
use crate::output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT;
use crate::output::gap_decision_ledger::{
    DEFAULT_GAP_DECISION_LEDGER_OUT, GapRecord, parse_gap_records_json_for_root,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Debounce window for interactive refresh triggers (did_open, did_save,
/// did_close). Rapid saves within this window collapse into a single
/// analysis run instead of canceling and re-queuing on every keystroke.
/// Explicit refresh and config reload bypass the debounce (#1908).
const INTERACTIVE_REFRESH_DEBOUNCE: Duration = Duration::from_millis(200);

/// Upper bound on the optional client disclosures that follow an
/// initialize-time failure commit (#3802). The owning session failure is
/// recorded synchronously first, so a wedged or undriven peer can delay only
/// this advisory `window/logMessage` + status delivery window, never the
/// failure state transition itself. Healthy delivery completes in
/// microseconds; an expired window is recorded on the backend and the
/// failure state is re-disclosed by the next status publication.
const INITIALIZE_FAILURE_DISCLOSURE_BUDGET: Duration = Duration::from_secs(2);

const SAVED_WORKTREE_LIMITS_NOTE: &str = "Static evidence only; staged and unstaged tracked files are analyzed; untracked files remain out of scope until staged or supplied through an explicit diff.";
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tower_lsp_server::jsonrpc::{Error as LspError, Result as LspResult};
use tower_lsp_server::ls_types::notification::LogTrace;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, ConfigurationItem,
    Diagnostic, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, ExecuteCommandParams, FileEvent,
    Hover, HoverParams, InitializeParams, InitializeResult, InitializedParams, LSPAny,
    LogTraceParams, MessageType, Registration, RelatedFullDocumentDiagnosticReport,
    RelatedUnchangedDocumentDiagnosticReport, TraceValue, UnchangedDocumentDiagnosticReport, Uri,
    WorkspaceDiagnosticParams, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult,
    WorkspaceDocumentDiagnosticReport, WorkspaceFullDocumentDiagnosticReport,
    WorkspaceUnchangedDocumentDiagnosticReport,
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
    /// Set when the bounded disclosure window after an initialize-time
    /// failure commit expired without completing client delivery (#3802).
    /// The owning session failure stays committed; this records the delivery
    /// omission locally instead of dropping it silently. Reflects the most
    /// recent disclosure attempt.
    initialize_failure_disclosure_omitted: AtomicBool,
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

/// Aborts a spawned deadline timer on drop (#1972): the explicit `drop` after
/// the blocking await is the normal path, but if the refresh-worker future is
/// itself dropped mid-await (shutdown, supersede-adjacent paths), the timer
/// task must not outlive the attempt and fire into a dead token later.
struct AbortTimerOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortTimerOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
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
            initialize_failure_disclosure_omitted: AtomicBool::new(false),
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
        // Physical deadline (#1972): arm a timer for this attempt. On expiry
        // the attempt's token is cancelled with the named `DeadlineExceeded`
        // kind; the blocking closure exits cooperatively at its checkpoints
        // and the refresh is dropped fail-closed. First-cancel-wins means a
        // deadline cancel loses to an earlier supersede/client cancel. The
        // timer is aborted as soon as the blocking await returns, either way.
        let deadline = request.config.refresh_deadline;
        let deadline_token = request.cancellation.clone();
        let deadline_timer = AbortTimerOnDrop(tokio::spawn(async move {
            tokio::time::sleep(deadline).await;
            deadline_token.cancel(AnalysisAbortKind::DeadlineExceeded);
        }));
        let diagnostics_result = tokio::task::spawn_blocking(move || {
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
        .await;
        drop(deadline_timer);
        let diagnostics = match diagnostics_result {
            Ok(Ok(mut diagnostics)) => {
                diagnostics.snapshot.input_identity = Some(request.input_identity.clone());
                diagnostics.snapshot.refresh.snapshot_id =
                    Some(snapshot_id_for_generation(generation));
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
                        AnalysisFailureKind::AnalysisError,
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
                        AnalysisFailureKind::TaskFailure,
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
                AnalysisFailureKind::RootProjection,
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
                AnalysisFailureKind::AnalysisError,
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
                AnalysisFailureKind::SnapshotCommitFailure,
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
        kind: AnalysisFailureKind,
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
            mut batches,
        } = diagnostics;
        if snapshot.refresh.snapshot_id.is_none() {
            snapshot.refresh.snapshot_id = Some("snapshot:legacy".to_string());
        }
        bind_seam_evidence_identity(&mut snapshot, &mut batches);
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

    #[cfg(test)]
    pub(super) fn reset_analysis_health_for_test(&self) {
        self.reset_health_for_input_change();
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
            RefreshAttemptOutcome::DeadlineExceeded => AnalysisProgressEnd::DeadlineExceeded,
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

    fn mark_attempt_failed(
        &self,
        request: &RefreshRequest,
        kind: AnalysisFailureKind,
        message: &str,
    ) {
        let Ok(mut health) = self.analysis_health.lock() else {
            return;
        };
        health.attempt_id = Some(request.generation);
        health.state = AnalysisAttemptState::Failed;
        health.reason = Some(request.reason.as_str().to_string());
        health.requested_scope = Some(request.scope.as_str().to_string());
        health.failure = Some(AnalysisFailure {
            kind,
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
                let snapshot_id = snapshot_id_for_generation(request.generation);
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
            // A deadline-expired attempt is a fail-closed dropped refresh
            // (#1972); reuse the existing cancelled health state — no new
            // run-status or attempt-state string.
            RefreshAttemptOutcome::DeadlineExceeded => {
                health.state = AnalysisAttemptState::Cancelled;
            }
            RefreshAttemptOutcome::Superseded => health.state = AnalysisAttemptState::Superseded,
            RefreshAttemptOutcome::NotStarted => health.state = AnalysisAttemptState::Stopped,
        }
    }

    async fn publish_analysis_status(&self) {
        self.client
            .send_notification::<AnalysisStatusNotification>(self.analysis_status_payload())
            .await;
    }

    /// The selected root to keep when a folder event leaves several folders
    /// (#4459). Helix, and any client that shares one server across
    /// repositories, adds each newly opened repository as a workspace folder;
    /// treating that as ambiguous stopped analysis for the repository the
    /// user was already in. For clients without the `riprEditor`
    /// integration, a root that is selected, analyzable and still in the set
    /// stays the session root and the other folders go unanalyzed. The VS
    /// Code extension owns root selection, so it keeps the RIPR-SPEC-0139
    /// ambiguous transition. A set with no selected root (for example two
    /// folders at `initialize`) is still ambiguous: nothing is kept.
    fn kept_root_for_added_folders(&self, entries: &[WorkspaceFolderEntry]) -> Option<PathBuf> {
        let generic_client = self
            .client_features
            .lock()
            .map(|features| features.ripr_editor.is_none())
            .unwrap_or(false);
        if !generic_client {
            return None;
        }
        let current = self.workspace_root_authority();
        if !current.allows_analysis() {
            return None;
        }
        let root = current.effective_root?;
        entries
            .iter()
            .any(|entry| entry.path == root)
            .then_some(root)
    }

    /// Say which folders a kept root leaves unanalyzed, in the log and on
    /// screen, so a user who opened a second repository learns why it has no
    /// evidence.
    async fn disclose_kept_root(&self, root: &Path, others: &[PathBuf]) {
        let listed = others
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let message = format!(
            "ripr keeps analyzing {}. Not analyzed by this server: {listed}. ripr analyzes one workspace root per server; open another repository in its own editor session to analyze it.",
            root.display()
        );
        self.client
            .log_message(MessageType::INFO, message.clone())
            .await;
        self.client.show_message(MessageType::INFO, message).await;
    }

    /// Warn when a workspace-folder change moves the root into a blocked
    /// state the client has not been told about. Startup is covered by
    /// [`Self::disclose_blocked_startup_root`]; an unchanged blocked state is
    /// not repeated.
    async fn disclose_blocked_root_transition(&self, previous: &WorkspaceRootAuthority) {
        let current = self.workspace_root_authority();
        if current.state == previous.state
            && current.candidate_roots == previous.candidate_roots
            && current.detail == previous.detail
        {
            return;
        }
        self.disclose_blocked_root(&current).await;
    }

    /// Warn once at startup when the workspace root blocks analysis.
    async fn disclose_blocked_startup_root(&self) {
        let root = self.workspace_root_authority();
        self.disclose_blocked_root(&root).await;
    }

    /// The warning always goes to the log; clients without the `riprEditor`
    /// integration also get `window/showMessage`, because `ripr/analysisStatus`
    /// is the only other place the blocked state appears and generic editors
    /// do not render it. The VS Code extension renders its own root state.
    async fn disclose_blocked_root(&self, root: &WorkspaceRootAuthority) {
        let Some(message) = blocked_root_message(root) else {
            return;
        };
        let message = format!("ripr {message}");
        self.client
            .log_message(MessageType::WARNING, message.clone())
            .await;
        let generic_client = self
            .client_features
            .lock()
            .map(|features| features.ripr_editor.is_none())
            .unwrap_or(true);
        if generic_client {
            self.client
                .show_message(MessageType::WARNING, message)
                .await;
        }
    }

    /// Deliver the optional client disclosures that follow an
    /// initialize-time failure commit — one `window/logMessage` warning plus
    /// one analysis-status publication — within
    /// [`INITIALIZE_FAILURE_DISCLOSURE_BUDGET`] (#3802). The owning failure
    /// is already committed when this runs, so a wedged or undriven peer can
    /// never delay the state transition through these awaits: an expired
    /// window is recorded on the backend instead of being dropped silently,
    /// and the committed failure is re-disclosed by the next status
    /// publication.
    async fn deliver_initialize_failure_disclosures(&self, warning: String) {
        self.initialize_failure_disclosure_omitted
            .store(false, Ordering::Release);
        let delivery = tokio::time::timeout(INITIALIZE_FAILURE_DISCLOSURE_BUDGET, async {
            self.client.log_message(MessageType::WARNING, warning).await;
            self.publish_analysis_status().await;
        })
        .await;
        if delivery.is_err() {
            self.initialize_failure_disclosure_omitted
                .store(true, Ordering::Release);
        }
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
                    value.analysis_outcome.clone(),
                )
            })
        });
        let (snapshot_identity, components, analysis_outcome) = match snapshot_state {
            Some((identity, components, analysis_outcome)) => {
                (identity, components, analysis_outcome)
            }
            None => (None, Vec::new(), None),
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
                    "kind": failure.kind.as_str(),
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
            "analysis_outcome": analysis_outcome,
            // Typed bounded per-component outcomes for the committed snapshot
            // (#1997, RIPR-SPEC-0141): the single typed authority for
            // optional-component degradation. Empty until a snapshot commits.
            "components": components,
            "diagnostic_profile": self
                .analysis_config()
                .map(|config| config.diagnostic_profile.as_str())
                .unwrap_or("actionable"),
            "failure": health.failure.clone().map(|failure| serde_json::json!({
                "kind": failure.kind.as_str(),
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
        self.record_blocking_failure(AnalysisFailureKind::ConfigInvalid, message);
    }

    /// Record a failure that pauses analysis until the session state is
    /// repaired: the typed failure lands in `configuration_failure` and the
    /// analysis health so the status payload discloses it instead of
    /// presenting an internally inconsistent session as normal.
    fn record_blocking_failure(&self, kind: AnalysisFailureKind, message: impl Into<String>) {
        let failure = AnalysisFailure {
            kind,
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
                .is_some_and(|failure| failure.kind == AnalysisFailureKind::ConfigInvalid)
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

    /// Whether the most recent bounded disclosure window after an
    /// initialize-time failure commit expired without completing client
    /// delivery (#3802). Unreadable state fails closed: an unknown delivery
    /// outcome is reported as omitted rather than silently clean. Read
    /// locally by tests; production surfaces observe the committed failure
    /// itself, which the next status publication re-discloses.
    #[cfg(test)]
    pub(super) fn initialize_failure_disclosure_omitted(&self) -> bool {
        self.initialize_failure_disclosure_omitted
            .load(Ordering::Acquire)
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
                    kind: AnalysisFailureKind::ConfigPullFailed,
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
                        kind: AnalysisFailureKind::ConfigPullInvalid,
                        message: bounded_failure_message(&message),
                    }));
                    self.publish_analysis_status().await;
                }
            },
            Err(error) => {
                self.set_config_pull_state(ConfigPullState::Failed(AnalysisFailure {
                    kind: AnalysisFailureKind::ConfigPullFailed,
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

    fn file_event_workspace_input_kind(&self, event: &FileEvent) -> Option<WorkspaceInputKind> {
        let root = self.effective_root()?;
        let path = path_from_file_uri(&event.uri)?;
        if !workspace_input_path_is_relevant(&root, &path) {
            return None;
        }
        workspace_input_kind(&root, &path)
    }

    pub(super) fn watched_file_change_kinds(&self, changes: &[FileEvent]) -> (bool, bool) {
        let mut config_changed = changes
            .iter()
            .any(|event| self.file_event_is_repository_config(event));
        let mut workspace_graph_changed = false;
        for event in changes {
            let Some(kind) = self.file_event_workspace_input_kind(event) else {
                continue;
            };
            config_changed |= kind.reloads_repository_config();
            workspace_graph_changed |= kind.invalidates_workspace_graph();
        }
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

    /// The hover for a position with no evidence to show. A generic editor
    /// has no other place that says why ripr is quiet, so name a blocked
    /// root or an unsaved buffer before falling back to the CLI pointer.
    fn hover_without_evidence(&self, uri: &Uri) -> Hover {
        if let Some(message) = blocked_root_message(&self.workspace_root_authority()) {
            return markdown_hover(format!("**ripr** {message}"));
        }
        if let Some(root) = self.effective_root()
            && let Some(path) = path_from_file_uri(uri)
            && !path_is_within_root(&root, &path)
        {
            return markdown_hover(format!(
                "**ripr** this file is outside the analyzed workspace root {}. ripr analyzes one workspace root per server; open this file's repository in its own editor session to analyze it.",
                root.display()
            ));
        }
        if let Some((_, reason)) = self.document_quarantine(uri) {
            // Only a divergent buffer is fixed by saving; a file with no
            // analyzed saved content needs an analysis run.
            let route = match reason {
                DocumentStalenessReason::BufferDivergesFromAnalyzedSavedContent => {
                    "ripr analyzes saved files; save the file to refresh its evidence."
                }
                DocumentStalenessReason::NoAnalyzedSavedContent => {
                    "ripr has not analyzed this file's saved content yet; its evidence appears after the next refresh (`ripr.refresh`) completes. A new file must be saved first."
                }
            };
            return markdown_hover(format!(
                "**ripr** evidence for this file is paused: {}. {route}",
                reason.description()
            ));
        }
        hover_response()
    }

    /// The quarantine state of an open document, as `(path, reason)`.
    /// `None` means the document is unknown or clean: its buffer matches the
    /// saved content the committed snapshot analyzed.
    pub(super) fn document_quarantine(
        &self,
        uri: &Uri,
    ) -> Option<(PathBuf, DocumentStalenessReason)> {
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
            // A producer-backed gap decision is more specific than the
            // overlapping language finding: it carries the bounded repair,
            // verification, and receipt route the user asked to inspect.
            // Prefer it before the generic finding hover while retaining seam
            // evidence as the highest-priority structural projection.
            for diagnostic in &overlapping {
                if is_gap_diagnostic(diagnostic) {
                    return Some(hover_with_snapshot_status(
                        diagnostic_hover_response(diagnostic),
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
        // No published diagnostic covers the cursor, but the code lens on
        // this line may still show snapshot findings (route-less or exposed
        // findings the actionable profile does not publish). Describe those
        // instead of falling back to the generic CLI pointer.
        if let Ok(snapshot) = self.latest_analysis.lock()
            && let Some(snapshot) = snapshot.as_ref()
        {
            // A finding that already has a published diagnostic is described
            // by that diagnostic's hover; list only the unpublished ones.
            let published = snapshot
                .diagnostics_for_uri(uri)
                .into_iter()
                .flatten()
                .filter_map(|diagnostic| snapshot.finding_for_diagnostic(diagnostic))
                .map(|finding| finding.id.as_str())
                .collect::<BTreeSet<_>>();
            let findings = super::lens::lens_findings_at_line(uri, snapshot, position.line)
                .into_iter()
                .filter(|finding| !published.contains(finding.id.as_str()))
                .collect::<Vec<_>>();
            if !findings.is_empty() {
                return Some(hover_with_snapshot_status(
                    super::hover::line_findings_hover_response(
                        &findings,
                        snapshot.diagnostic_profile,
                    ),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceInputKind {
    CargoGraph,
    /// Canonical root Python project-marker file: the first no-config
    /// detection input class (#3503).
    PythonProjectMarker,
    /// Detectable Python source below a root `src`/`tests` directory: the
    /// second no-config detection input class (#3553). Watched through
    /// detectable-file events because watched-file globs cannot express
    /// directory presence.
    PythonSourcePresence,
}

impl WorkspaceInputKind {
    fn reloads_repository_config(self) -> bool {
        matches!(self, Self::PythonProjectMarker | Self::PythonSourcePresence)
    }

    fn invalidates_workspace_graph(self) -> bool {
        matches!(self, Self::CargoGraph)
    }
}

fn path_matches_root_file(root: &Path, path: &Path, file_name: &str) -> bool {
    let Ok(expected) = file_uri_for_path(&root.join(file_name)) else {
        return false;
    };
    let Ok(actual) = file_uri_for_path(path) else {
        return false;
    };
    file_uris_match(&expected, &actual)
}

fn workspace_input_kind(root: &Path, path: &Path) -> Option<WorkspaceInputKind> {
    if !path_is_within_root(root, path) {
        return None;
    }
    let name = path.file_name().and_then(|name| name.to_str())?;
    if matches!(name, "Cargo.toml" | "Cargo.lock") {
        return Some(WorkspaceInputKind::CargoGraph);
    }
    if let Some(marker) = python_project_marker_name(name)
        && path_matches_root_file(root, path, marker)
    {
        return Some(WorkspaceInputKind::PythonProjectMarker);
    }
    path_is_detectable_source_dir_python(root, path)
        .then_some(WorkspaceInputKind::PythonSourcePresence)
}

/// Whether `path` resolves below the workspace root to a detectable Python
/// source file directly under a root source-directory marker (`src` or
/// `tests`), classified with the canonical detector vocabulary —
/// source-directory marker, excluded directories, generated file names,
/// `.py` extension. Presence-only: the file is never read, so deleted files
/// classify the same as created ones.
fn path_is_detectable_source_dir_python(root: &Path, path: &Path) -> bool {
    let Some(relative) = root_relative_components(root, path) else {
        return false;
    };
    let Some((marker, below)) = relative.split_first() else {
        return false;
    };
    if python_source_dir_marker_name(marker).is_none() {
        return false;
    }
    let Some((file_name, directories)) = below.split_last() else {
        return false;
    };
    directories
        .iter()
        .all(|dir| !is_python_excluded_dir_everywhere(dir))
        && is_detectable_python_source_name(file_name)
}

/// Root-relative components of `path` when `path` sits below `root`. Root
/// components compare with the platform path rule (case-insensitive on
/// Windows), mirroring the LSP URI authority's containment rule. `.` and
/// `..` resolve on both sides first, including a root this process built
/// with a relative join (`crates/ripr/../..`). A collapsed file URI and
/// that uncollapsed root are the same directory.
fn root_relative_components(root: &Path, path: &Path) -> Option<Vec<String>> {
    let root = super::uri::normalize_path(root);
    let path = super::uri::normalize_path(path);
    fn component_names(path: &Path) -> Vec<Option<String>> {
        path.components()
            .filter(|component| {
                !matches!(
                    component,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                )
            })
            .map(|component| component.as_os_str().to_str().map(str::to_string))
            .collect()
    }

    let root_names = component_names(&root);
    let path_names = component_names(&path);
    if path_names.len() < root_names.len() {
        return None;
    }
    for (expected, actual) in root_names.iter().zip(path_names.iter()) {
        let (Some(expected), Some(actual)) = (expected, actual) else {
            return None;
        };
        let matched = if cfg!(windows) {
            expected.eq_ignore_ascii_case(actual)
        } else {
            expected == actual
        };
        if !matched {
            return None;
        }
    }
    let mut relative = Vec::new();
    for name in &path_names[root_names.len()..] {
        let Some(name) = name else {
            return None;
        };
        if name == "." {
            continue;
        }
        if name == ".." {
            relative.pop();
            continue;
        }
        relative.push(name.clone());
    }
    Some(relative)
}

/// Return whether a watched path can change the effective analysis input.
/// Cargo inputs remain recursive for workspace members. The Python inputs
/// are root-scoped — root marker presence and detectable `.py` source below
/// root `src`/`tests` are what this reload path tracks; Python outside those
/// roots is not a detection input and stays unwatched.
pub(super) fn workspace_input_path_is_relevant(root: &Path, path: &Path) -> bool {
    workspace_input_kind(root, path).is_some()
}

fn workspace_input_watchers() -> Vec<LSPAny> {
    std::iter::once(CONFIG_FILE_NAME)
        .chain(["Cargo.toml", "Cargo.lock"])
        .chain(PYTHON_PROJECT_MARKERS.iter().copied())
        .map(|name| serde_json::json!({"globPattern": format!("**/{name}")}))
        .chain(PYTHON_SOURCE_DIR_MARKERS.iter().map(|dir| {
            // Root-scoped relative glob: relative patterns resolve against
            // the workspace root and `**` matches zero or more segments, so
            // this covers `<root>/dir/**/*.py` only — never a workspace-wide
            // `**/*.py` watcher. Directory presence itself is not watchable;
            // detectable-file events are the transport hint, and the reload
            // path's config comparison stays the semantic authority.
            serde_json::json!({"globPattern": format!("{dir}/**/*.py")})
        }))
        .collect()
}

#[cfg(test)]
mod workspace_input_tests {
    use super::*;
    use crate::config::{PYTHON_EXCLUDED_DIRS, PYTHON_VENDOR_DIR};
    use std::collections::BTreeSet;

    #[test]
    fn python_project_markers_reload_configuration_and_stay_root_scoped() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-workspace-input-root");
        let outside = std::env::temp_dir().join("ripr-workspace-input-outside");

        for marker in PYTHON_PROJECT_MARKERS {
            let root_marker = root.join(marker);
            assert_eq!(
                workspace_input_kind(&root, &root_marker),
                Some(WorkspaceInputKind::PythonProjectMarker),
                "root marker {marker}"
            );
            assert!(workspace_input_path_is_relevant(&root, &root_marker));
            assert_eq!(
                workspace_input_kind(&root, &root.join("nested").join(marker)),
                None,
                "nested marker {marker} is not consumed by root detection"
            );
            assert_eq!(
                workspace_input_kind(&root, &outside.join(marker)),
                None,
                "outside marker {marker}"
            );
        }

        assert!(WorkspaceInputKind::PythonProjectMarker.reloads_repository_config());
        assert!(!WorkspaceInputKind::PythonProjectMarker.invalidates_workspace_graph());
        Ok(())
    }

    #[test]
    fn parent_segments_in_the_workspace_root_still_classify_root_python_sources()
    -> Result<(), String> {
        let repo =
            std::env::temp_dir().join(format!("ripr-root-parent-segments-{}", std::process::id()));
        let nested = repo.join("crates").join("pkg");
        std::fs::create_dir_all(&nested).map_err(|err| err.to_string())?;
        let root = nested.join("..").join("..");
        let source = repo.join("src").join("app.py");
        let dotted_source = repo.join("src").join("nested").join("..").join("app.py");
        let outside = repo.join("scripts").join("app.py");
        let generated = repo.join("src").join("client_pb2.py");
        let escaped = repo.join("src").join("..").join("..").join("outside.py");
        let source_kind = workspace_input_kind(&root, &source);
        let dotted_kind = workspace_input_kind(&root, &dotted_source);
        let outside_kind = workspace_input_kind(&root, &outside);
        let generated_kind = workspace_input_kind(&root, &generated);
        let escaped_kind = workspace_input_kind(&root, &escaped);
        let _ = std::fs::remove_dir_all(&repo);
        if source_kind != Some(WorkspaceInputKind::PythonSourcePresence) {
            return Err(format!(
                "src/app.py under a root with parent segments was {source_kind:?}"
            ));
        }
        if dotted_kind != Some(WorkspaceInputKind::PythonSourcePresence) {
            return Err(format!(
                "src/nested/../app.py was not collapsed before classification: {dotted_kind:?}"
            ));
        }
        if outside_kind.is_some() || generated_kind.is_some() || escaped_kind.is_some() {
            return Err(format!(
                "non-inputs were classified: outside={outside_kind:?} generated={generated_kind:?} escaped={escaped_kind:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn cargo_workspace_inputs_remain_recursive_and_graph_scoped() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-workspace-input-root");
        let outside = std::env::temp_dir().join("ripr-workspace-input-outside");

        for name in ["Cargo.toml", "Cargo.lock"] {
            let member_input = root.join("member").join(name);
            assert_eq!(
                workspace_input_kind(&root, &member_input),
                Some(WorkspaceInputKind::CargoGraph)
            );
            assert!(workspace_input_path_is_relevant(&root, &member_input));
            assert_eq!(workspace_input_kind(&root, &outside.join(name)), None);
        }

        assert!(!WorkspaceInputKind::CargoGraph.reloads_repository_config());
        assert!(WorkspaceInputKind::CargoGraph.invalidates_workspace_graph());
        assert_eq!(
            workspace_input_kind(&root, &root.join("package.json")),
            None
        );
        Ok(())
    }

    #[test]
    fn python_source_under_src_or_tests_is_a_configuration_input() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-workspace-input-root");
        let outside = std::env::temp_dir().join("ripr-workspace-input-outside");

        for dir in PYTHON_SOURCE_DIR_MARKERS {
            let source_file = root.join(dir).join("app.py");
            assert_eq!(
                workspace_input_kind(&root, &source_file),
                Some(WorkspaceInputKind::PythonSourcePresence),
                "{dir}/app.py drives no-config Python detection"
            );
            assert!(workspace_input_path_is_relevant(&root, &source_file));

            let nested = root.join(dir).join("pkg").join("mod.py");
            assert_eq!(
                workspace_input_kind(&root, &nested),
                Some(WorkspaceInputKind::PythonSourcePresence),
                "nested {dir} sources are covered by recursive detection"
            );

            for generated in [
                "client_pb2.py",
                "client_pb2_grpc.py",
                "client.generated.py",
                "client_generated.py",
                "generated_client.py",
            ] {
                assert_eq!(
                    workspace_input_kind(&root, &root.join(dir).join(generated)),
                    None,
                    "generated {dir}/{generated} never changes detection state"
                );
            }

            for excluded in PYTHON_EXCLUDED_DIRS
                .iter()
                .copied()
                .chain([PYTHON_VENDOR_DIR])
            {
                let path = root.join(dir).join(excluded).join("ignored.py");
                assert_eq!(
                    workspace_input_kind(&root, &path),
                    None,
                    "{dir}/{excluded}/ignored.py is excluded from detection traversal"
                );
            }

            assert_eq!(
                workspace_input_kind(&root, &root.join(dir).join("lib.rs")),
                None,
                "non-Python files under {dir} are not detection inputs"
            );
            assert_eq!(
                workspace_input_kind(&root, &outside.join(dir).join("app.py")),
                None,
                "outside-root {dir}/ sources fail closed"
            );
        }

        // Python that detection never reads: root level and non-source
        // directories are irrelevant to no-config detection.
        assert_eq!(workspace_input_kind(&root, &root.join("app.py")), None);
        assert_eq!(
            workspace_input_kind(&root, &root.join("scripts").join("app.py")),
            None
        );

        assert!(WorkspaceInputKind::PythonSourcePresence.reloads_repository_config());
        assert!(!WorkspaceInputKind::PythonSourcePresence.invalidates_workspace_graph());

        if cfg!(windows) {
            // Source-directory markers resolve through the Windows
            // filesystem, so differently-cased components classify the same.
            let upper = root.join("SRC").join("app.py");
            assert_eq!(
                workspace_input_kind(&root, &upper),
                Some(WorkspaceInputKind::PythonSourcePresence)
            );
        }
        Ok(())
    }

    #[test]
    fn dynamic_watchers_cover_the_canonical_python_input_set() {
        let actual = workspace_input_watchers()
            .into_iter()
            .filter_map(|watcher| {
                watcher
                    .get("globPattern")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .collect::<BTreeSet<_>>();
        let expected = std::iter::once(CONFIG_FILE_NAME)
            .chain(["Cargo.toml", "Cargo.lock"])
            .chain(PYTHON_PROJECT_MARKERS.iter().copied())
            .map(|name| format!("**/{name}"))
            .chain(
                PYTHON_SOURCE_DIR_MARKERS
                    .iter()
                    .map(|dir| format!("{dir}/**/*.py")),
            )
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }
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
        Err(error) if error.kind == AnalysisAbortKind::DeadlineExceeded => {
            RefreshAttemptOutcome::DeadlineExceeded
        }
        Err(_) => RefreshAttemptOutcome::Superseded,
        Ok(()) => RefreshAttemptOutcome::Superseded,
    }
}

pub(super) fn diagnostics_by_uri_from_batches(
    batches: &[DiagnosticBatch],
) -> BTreeMap<Uri, Vec<Diagnostic>> {
    batches
        .iter()
        .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
        .collect()
}

fn snapshot_id_for_generation(generation: u64) -> String {
    format!("snapshot:{generation}")
}

fn bind_seam_evidence_identity(snapshot: &mut AnalysisSnapshot, batches: &mut [DiagnosticBatch]) {
    let evidence_identity = snapshot.evidence_identity();
    for diagnostics in snapshot.diagnostics_by_uri.values_mut() {
        for diagnostic in diagnostics {
            bind_seam_diagnostic_identity(diagnostic, &evidence_identity);
        }
    }
    for batch in batches {
        for diagnostic in &mut batch.diagnostics {
            bind_seam_diagnostic_identity(diagnostic, &evidence_identity);
        }
    }
}

fn bind_seam_diagnostic_identity(
    diagnostic: &mut Diagnostic,
    evidence_identity: &serde_json::Value,
) {
    let Some(data) = diagnostic
        .data
        .as_mut()
        .and_then(|data| data.as_object_mut())
    else {
        return;
    };
    if data
        .get("seam_id")
        .and_then(|value| value.as_str())
        .is_some()
    {
        data.insert("evidence_identity".to_string(), evidence_identity.clone());
    }
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

/// The user-facing warning body (callers prefix `ripr`) for a root that
/// blocks analysis, or `None` when analysis may run. Names the folders for an
/// ambiguous set so the user can see which roots the editor sent.
fn blocked_root_message(root: &WorkspaceRootAuthority) -> Option<String> {
    if root.allows_analysis() {
        return None;
    }
    let detail = root.detail.as_deref().unwrap_or("no usable workspace root");
    let folders = if root.candidate_roots.is_empty() {
        String::new()
    } else {
        let listed = root
            .candidate_roots
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        // Removed and changed roots carry the previous root, not the
        // folders the client sent.
        let label = match root.state {
            WorkspaceRootState::RootRemoved | WorkspaceRootState::RootChanged => "Previous root",
            _ => "Folders",
        };
        format!(" {label}: {listed}.")
    };
    // The stored details already name the recovery for ambiguous, removed
    // and changed roots; an unavailable root only says what the client sent.
    let guidance = match root.state {
        WorkspaceRootState::WorkspaceAmbiguous => " ripr analyzes one workspace root per server.",
        WorkspaceRootState::RootUnavailable => {
            " Open the repository folder as the editor's workspace root, then restart the language server."
        }
        WorkspaceRootState::SelectedSingleRoot
        | WorkspaceRootState::RootRemoved
        | WorkspaceRootState::RootChanged => "",
    };
    Some(format!(
        "analysis is stopped ({}): {detail}.{folders}{guidance}",
        root.state.as_str()
    ))
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

    /// `ripr/listActionableItems` custom request handler (#1603).
    ///
    /// Returns the diagnostic-budget selected/omitted items as a typed
    /// envelope. The budget result already carries every field the response
    /// needs — this is a pure transform, no new analysis.
    pub(super) async fn ripr_list_actionable_items(
        &self,
        params: LSPAny,
    ) -> LspResult<Option<LSPAny>> {
        // Ingress payload bound (#2034) before any snapshot access or
        // early-return fast path; the handler reads no client-supplied
        // fields, so only the params blob size is enforced.
        check_list_actionable_items_params(&params)?;
        // A poisoned lock and an empty slot both fail closed to `no_snapshot`;
        // `and_then` flattens the stored `Option<Arc<AnalysisSnapshot>>`.
        let snapshot = self
            .latest_analysis
            .lock()
            .ok()
            .and_then(|guard| (*guard).clone());
        let Some(snapshot) = snapshot else {
            return Ok(Some(serde_json::json!({
                "error": {
                    "kind": "no_snapshot",
                    "message": "No analysis snapshot is available.",
                    "recovery_route": "refresh",
                }
            })));
        };
        let Some(delivery) = &snapshot.delivery_selection else {
            return Ok(Some(serde_json::json!({
                "error": {
                    "kind": "analysis_in_flight",
                    "message": "Analysis is in progress.",
                    "recovery_route": "refresh",
                }
            })));
        };
        let result = match &delivery.outcome {
            crate::lsp::diagnostic_budget::DiagnosticDeliveryOutcome::Applied {
                result, ..
            } => result,
            crate::lsp::diagnostic_budget::DiagnosticDeliveryOutcome::Unavailable { .. } => {
                return Ok(Some(serde_json::json!({
                    "error": {
                        "kind": "analysis_in_flight",
                        "message": "Diagnostic budget is unavailable for this snapshot.",
                        "recovery_route": "refresh",
                    }
                })));
            }
        };
        // The item lists make this route an answer rather than a loop: it
        // names itself as the continuation route, so counts alone left a
        // client with nothing new to inspect. `selected` is already bounded
        // by the workspace item budget; `omitted` is capped here.
        let selected = result
            .selected
            .iter()
            .map(|item| {
                serde_json::json!({
                    "canonical_id": item.canonical_id,
                    "document": item.document,
                })
            })
            .collect::<Vec<_>>();
        let omitted = result
            .omitted
            .iter()
            .take(LIST_ACTIONABLE_OMITTED_MAX)
            .map(|item| {
                serde_json::json!({
                    "canonical_id": item.canonical_id,
                    "reason": omitted_diagnostic_reason_name(item.reason),
                })
            })
            .collect::<Vec<_>>();
        // Gaps the actionable profile hides because they have no bounded repair
        // route (a new function no test calls is the common case). They are not
        // diagnostics by design (RIPR-SPEC-0069), so this list is the only
        // LSP surface that names them for an agent; each id is a valid
        // `ripr.collectContext` `finding_id`.
        // Filter canonical-group primaries, as publishing does, so a group
        // counts once and a published group's other members are not listed.
        // Only the three gap classes qualify: the `*_unknown` classes are
        // missing static evidence, not gaps, and must not be named as such.
        let hidden = if snapshot.diagnostic_profile == LspDiagnosticProfile::Actionable {
            super::diagnostics::canonical_finding_groups(&snapshot.findings)
                .into_iter()
                .map(|(primary, _)| primary)
                .filter(|finding| {
                    finding.is_candidate_actionable()
                        && matches!(
                            finding.class,
                            crate::domain::ExposureClass::WeaklyExposed
                                | crate::domain::ExposureClass::ReachableUnrevealed
                                | crate::domain::ExposureClass::NoStaticPath
                        )
                        && !super::diagnostics::finding_is_visible_in_profile(
                            LspDiagnosticProfile::Actionable,
                            finding,
                        )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let hidden_gaps = hidden
            .iter()
            .take(LIST_ACTIONABLE_HIDDEN_MAX)
            .map(|finding| {
                serde_json::json!({
                    "finding_id": finding.id,
                    "file": display_path(
                        finding
                            .probe
                            .location
                            .file
                            .strip_prefix(&snapshot.root)
                            .unwrap_or(&finding.probe.location.file),
                    ),
                    "line": finding.probe.location.line,
                    "class": finding.class.as_str(),
                })
            })
            .collect::<Vec<_>>();
        Ok(Some(serde_json::json!({
            "kind": "actionable_items",
            "status": "ok",
            // `snapshot_id` echoes the refresh-transaction generation
            // identity (`RefreshMetadata::snapshot_id`). This is an interim
            // compatibility state for the first #1603 slice — it is NOT
            // #1602's immutable snapshot-handle contract, and neither
            // responses nor docs may represent it as that contract. The
            // immutable handle binding lands with #1602.
            "snapshot_id": snapshot.refresh.snapshot_id,
            "selected_count": result.selected.len(),
            "omitted_count": result.omitted.len(),
            "total_count": result.total_canonical_items,
            "selected": selected,
            "omitted": omitted,
            "omitted_truncated": result.omitted.len() > LIST_ACTIONABLE_OMITTED_MAX,
            "hidden_gaps": hidden_gaps,
            "hidden_gap_count": hidden.len(),
            "budget_identity": result.snapshot_profile_budget_identity,
            "complete_evidence_identity": result.complete_evidence_identity,
            "continuation_or_inspect_route": result.continuation_or_inspect_route,
            "allowed_edit_surface": "read_only",
            "must_not_change": ["source_edits", "workspace_edit", "autonomous_repair"],
        })))
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
                    "watchers": workspace_input_watchers()
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
        // tower-lsp-server suppresses custom notifications until `initialize`
        // has returned, so the status published by the initialize-time root
        // and config transitions never reached the client. Publish the
        // startup state once the session is live, and name a root that
        // blocks analysis over a standard channel: without this, a generic
        // editor opened on two folders or on no folder sees nothing at all.
        self.publish_analysis_status().await;
        self.disclose_blocked_startup_root().await;
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
            // Commit the owning config failure before the optional client
            // disclosures (#3802): the state transition must not depend on a
            // client await, so a wedged or undriven peer can delay only the
            // bounded disclosure window, never the failure itself.
            let warning = format!("ripr config load failed; analysis is paused: {error}");
            self.set_configuration_failure(error);
            self.deliver_initialize_failure_disclosures(warning).await;
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
            // Commit the owning failure BEFORE any client delivery (#3802):
            // the state transition must not depend on an optional client
            // await, so a wedged or undriven peer can delay only the bounded
            // disclosure window and never leave the session hanging
            // half-initialized without its failure committed.
            self.record_blocking_failure(
                AnalysisFailureKind::SessionStateInconsistent,
                "client feature profile could not be stored after initialize negotiation; analysis is paused",
            );
            self.deliver_initialize_failure_disclosures(
                "ripr client feature profile could not be stored; analysis is paused".to_string(),
            )
            .await;
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
                let previous = self.workspace_root_authority();
                self.reject_workspace_folder_update(rejection).await;
                self.disclose_blocked_root_transition(&previous).await;
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
                let previous = self.workspace_root_authority();
                self.reject_workspace_folder_update(rejection).await;
                self.disclose_blocked_root_transition(&previous).await;
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
                    match self.kept_root_for_added_folders(set.entries()) {
                        Some(root) => Some(WorkspaceRootResolution::Selected(root)),
                        None => Some(WorkspaceRootResolution::Ambiguous(
                            set.entries()
                                .iter()
                                .map(|entry| entry.path.clone())
                                .collect(),
                        )),
                    }
                }
            };
            (resolution, set.folder_set_epoch())
        };
        // Reaching here means the stored set changed (an accepted delta or a
        // drift-correction replacement), so a kept root is announced once
        // per change.
        let kept_root = match &derived.0 {
            Some(WorkspaceRootResolution::Selected(root)) => {
                let others = {
                    let Ok(set) = self.workspace_folders.lock() else {
                        return;
                    };
                    set.entries()
                        .iter()
                        .filter(|entry| &entry.path != root)
                        .map(|entry| entry.path.clone())
                        .collect::<Vec<_>>()
                };
                (!others.is_empty()).then(|| (root.clone(), others))
            }
            _ => None,
        };
        if derived.1 != folder_set_epoch {
            return;
        }
        let authority = match derived.0 {
            None => WorkspaceRootAuthority::removed(self.effective_root()),
            Some(resolution) => Self::workspace_root_authority_for_resolution(resolution),
        };
        let previous = self.workspace_root_authority();
        self.apply_workspace_folder_set_authority(authority, folder_set_epoch)
            .await;
        self.disclose_blocked_root_transition(&previous).await;
        // Announce only a root that actually stayed selected: a kept root
        // that is no longer a directory, or an application dropped for a
        // newer event, must not claim it is still analyzed.
        if let Some((root, others)) = kept_root {
            let current = self.workspace_root_authority();
            if current.allows_analysis()
                && current.effective_root.as_deref() == Some(root.as_path())
            {
                self.disclose_kept_root(&root, &others).await;
            }
        }
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
        let result = Ok(Some(self.hover_for_position(&params).unwrap_or_else(
            || self.hover_without_evidence(&params.text_document_position_params.text_document.uri),
        )));
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
        // Negotiated client-command filter (#1776, RIPR-SPEC-0129): a
        // poisoned profile store fails closed to the unsupported profile,
        // which strips every client-executed command rather than offering
        // quick fixes the client may not have registered.
        let client_features = self
            .client_features
            .lock()
            .map(|features| features.clone())
            .unwrap_or_else(|_| ClientFeatureProfile::unsupported());
        let result = Ok(Some(code_action_response(
            &params,
            action_snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
            &client_features,
        )));
        self.trace_response("textDocument/codeAction", &result)
            .await;
        result
    }

    /// `codeAction/resolve` revalidation (#1751, RIPR-SPEC-0129): actions
    /// are emitted fully resolved, so resolve revalidates the versioned data
    /// payload, the current snapshot, and the negotiated client profile
    /// before the client executes. A malformed payload fails closed with
    /// `InvalidParams`; a lapsed action returns in the inert disabled form.
    async fn code_action_resolve(&self, params: CodeAction) -> LspResult<CodeAction> {
        self.trace_inbound(
            "request",
            "codeAction/resolve",
            self.verbose_params_bytes(&params),
        )
        .await;
        let result = self.code_action_resolve_inner(params);
        self.trace_response("codeAction/resolve", &result).await;
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

/// Parse-layer lookup for a context command's first argument object.
///
/// `None` means the arguments are unreadably shaped (missing first element,
/// or first element is not an object). That miss is a **bad request**, not
/// "no evidence found". Callers must convert it through
/// [`context_command_target`], which returns JSON-RPC `InvalidParams`
/// (`-32602`). Do not `?` this `None` into `Ok(None)` / `result: null`.
fn context_arguments(arguments: &[LSPAny]) -> Option<&serde_json::Map<String, serde_json::Value>> {
    let first = arguments.first()?;
    first.as_object()
}

/// Accepted argument shapes for `ripr.collectContext`, quoted in its errors.
pub(crate) const COLLECT_CONTEXT_ARGUMENT_SHAPES: &str = "one object: \
{\"finding_id\": \"probe:...\"}, {\"seam_id\": \"...\", \"evidence_identity\": {...}}, \
or {\"gap_id\": \"...\", \"gap_ledger\": \"target/ripr/reports/gap-decision-ledger.json\"} \
(gap_ledger optional); copy the ids and evidence_identity from a ripr diagnostic's data";

/// Accepted argument shape for `ripr.collectEvidenceContext`.
pub(crate) const COLLECT_EVIDENCE_CONTEXT_ARGUMENT_SHAPES: &str = "one object: \
{\"seam_id\": \"...\", \"evidence_identity\": {...}}; copy both from a ripr seam \
diagnostic's data";

/// Accepted argument shapes for `ripr.collectRepairPacket`, quoted in its errors.
pub(crate) const COLLECT_REPAIR_PACKET_ARGUMENT_SHAPES: &str = "no arguments for the top \
packet, or one object {\"gap_id\": \"...\"} for a specific gap";

/// Longest client-supplied id echoed back in an error message.
const ECHOED_ID_MAX_CHARS: usize = 120;

/// Resolves which target key a context command will look up, in the
/// handler's own precedence order, or rejects the arguments with the
/// accepted shapes.
fn context_command_target(
    command: &str,
    arguments: &[LSPAny],
    keys: &[&'static str],
    shapes: &str,
) -> LspResult<(&'static str, String)> {
    let Some(args) = context_arguments(arguments) else {
        return Err(LspError::invalid_params(format!(
            "`{command}` expects {shapes}"
        )));
    };
    // Every present target key is validated before one is chosen, so a
    // mistyped higher-precedence id (`{"gap_id": 42, "seam_id": "..."}`) is
    // reported under its own name instead of being skipped for a later key.
    // A blank id counts as not given, so the next key is used.
    let mut target = None;
    for key in keys {
        if let Some(id) = optional_id_argument(command, args, key, shapes)?
            && target.is_none()
        {
            target = Some((*key, id.to_string()));
        }
    }
    target.ok_or_else(|| {
        let one_of = if keys.len() > 1 { "one of " } else { "" };
        LspError::invalid_params(format!(
            "`{command}` needs a non-empty string {one_of}`{}`; it expects {shapes}",
            keys.join("`, `")
        ))
    })
}

/// Reads one optional id argument of an agent command. An absent key, a
/// JSON `null`, or a string that is empty after trimming is `Ok(None)`: the
/// id was not given (RIPR-SPEC-0077: an absent or empty `gap_id` selects the
/// top gap). Any other non-string value is rejected with InvalidParams naming
/// the field. A mistyped id must never fall through to a different target
/// (such as the top repair packet): answering a request for one gap with
/// another gap's packet is a wrong actionable signal.
fn optional_id_argument<'a>(
    command: &str,
    args: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    shapes: &str,
) -> LspResult<Option<&'a str>> {
    let found = match args.get(key) {
        None | Some(serde_json::Value::Null) => return Ok(None),
        Some(serde_json::Value::String(id)) if id.trim().is_empty() => return Ok(None),
        Some(serde_json::Value::String(id)) => return Ok(Some(id)),
        Some(serde_json::Value::Bool(_)) => "a boolean",
        Some(serde_json::Value::Number(_)) => "a number",
        Some(serde_json::Value::Array(_)) => "an array",
        Some(serde_json::Value::Object(_)) => "an object",
    };
    Err(LspError::invalid_params(format!(
        "`{command}`: `{key}` must be a string when present, got {found}; \
it expects {shapes}"
    )))
}

fn context_target_not_found(command: &str, key: &str, id: &str) -> LspError {
    let mut echoed: String = id.chars().take(ECHOED_ID_MAX_CHARS).collect();
    if echoed.len() < id.len() {
        echoed.push_str("...");
    }
    let source = if key == "gap_id" {
        "the gap ledger (`gap_ledger`, default target/ripr/reports/gap-decision-ledger.json)"
    } else {
        "the current analysis snapshot"
    };
    LspError::invalid_params(format!(
        "`{command}`: `{key}` `{echoed}` is not in {source}, or no analysis snapshot is \
available yet; run `ripr.refresh` and retry with an id from a current ripr diagnostic's data"
    ))
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
        // A context command that cannot answer returns a typed InvalidParams
        // error naming the accepted argument shapes, never a bare null: a
        // null gave editor and agent clients nothing to correct.
        if params.command == COLLECT_CONTEXT_COMMAND {
            let (key, id) = context_command_target(
                COLLECT_CONTEXT_COMMAND,
                &params.arguments,
                &["gap_id", "seam_id", "finding_id"],
                COLLECT_CONTEXT_ARGUMENT_SHAPES,
            )?;
            return match self.collect_context_packet(&params.arguments).await {
                Some(packet) => Ok(Some(packet)),
                None => Err(context_target_not_found(COLLECT_CONTEXT_COMMAND, key, &id)),
            };
        }
        if params.command == COLLECT_EVIDENCE_CONTEXT_COMMAND {
            let (key, id) = context_command_target(
                COLLECT_EVIDENCE_CONTEXT_COMMAND,
                &params.arguments,
                &["seam_id"],
                COLLECT_EVIDENCE_CONTEXT_ARGUMENT_SHAPES,
            )?;
            return match self.collect_evidence_context_packet(&params.arguments) {
                Some(packet) => Ok(Some(packet)),
                None => Err(context_target_not_found(
                    COLLECT_EVIDENCE_CONTEXT_COMMAND,
                    key,
                    &id,
                )),
            };
        }
        if params.command == COLLECT_WORKSPACE_STATUS_COMMAND {
            return Ok(self.collect_workspace_status());
        }
        if params.command == COLLECT_REPAIR_PACKET_COMMAND {
            let gap_id = match params.arguments.first() {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::Object(args)) => optional_id_argument(
                    COLLECT_REPAIR_PACKET_COMMAND,
                    args,
                    "gap_id",
                    COLLECT_REPAIR_PACKET_ARGUMENT_SHAPES,
                )?
                .map(str::trim),
                Some(_) => {
                    return Err(LspError::invalid_params(format!(
                        "`{COLLECT_REPAIR_PACKET_COMMAND}` expects \
{COLLECT_REPAIR_PACKET_ARGUMENT_SHAPES}"
                    )));
                }
            };
            return Ok(self.collect_repair_packet(gap_id));
        }
        if params.command == COLLECT_TOP_LIMITATION_COMMAND {
            return Ok(self.collect_top_limitation());
        }
        if params.command == COLLECT_RECEIPT_STATUS_COMMAND {
            return Ok(self.collect_receipt_status());
        }
        // Stable unsupported-command response (#1628): a command the server
        // does not execute — including client-registered `ripr.copy*` and
        // `ripr.openRelatedTest` commands, which run client-side — is
        // rejected with a determinate protocol error instead of a silent
        // no-op, so a client that forwards an unadvertised command learns
        // about it.
        Err(LspError::invalid_params(format!(
            "unsupported command `{}`: not a server-executed ripr command",
            params.command
        )))
    }

    /// Inner `codeAction/resolve` handler, wrapped by the trait method so
    /// redacted protocol tracing (#2035, RIPR-SPEC-0137) covers every exit
    /// point. Applies the same health/root gate and negotiated
    /// client-command policy as `code_action` (#1751, RIPR-SPEC-0129).
    fn code_action_resolve_inner(&self, action: CodeAction) -> LspResult<CodeAction> {
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
        // A poisoned profile store fails closed to the unsupported profile,
        // same as `code_action` (#1776).
        let client_features = self
            .client_features
            .lock()
            .map(|features| features.clone())
            .unwrap_or_else(|_| ClientFeatureProfile::unsupported());
        resolve_action(
            action,
            action_snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
            &client_features,
        )
        .map_err(LspError::invalid_params)
    }

    async fn collect_context_packet(&self, arguments: &[LSPAny]) -> Option<LSPAny> {
        let args = context_arguments(arguments)?;
        let snapshot = self.latest_analysis.lock().ok()?.clone();
        if let Some(gap_id) = args.get("gap_id").and_then(|v| v.as_str()) {
            let snapshot = snapshot.as_ref()?;
            return collect_gap_record_context_packet(&snapshot.root, args, gap_id);
        }
        if let Some(seam_id) = args.get("seam_id").and_then(|v| v.as_str()) {
            let Some(snapshot) = snapshot.as_ref() else {
                return Some(self.stale_seam_evidence_response(
                    args,
                    "the workspace input changed and no current analysis snapshot is available",
                ));
            };
            if let Some(reason) = self.seam_command_stale_reason(args, snapshot) {
                return Some(self.stale_seam_evidence_response(args, reason));
            }
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
        let snapshot = snapshot?;
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
        let snapshot = self.latest_analysis.lock().ok()?.clone();
        let seam_id = args.get("seam_id").and_then(|v| v.as_str())?;
        let Some(snapshot) = snapshot.as_ref() else {
            return Some(self.stale_seam_evidence_response(
                args,
                "the workspace input changed and no current analysis snapshot is available",
            ));
        };
        if let Some(reason) = self.seam_command_stale_reason(args, snapshot) {
            return Some(self.stale_seam_evidence_response(args, reason));
        }
        let seam = snapshot.classified_seam_by_id(seam_id)?;
        Some(evidence_context_packet(snapshot, seam))
    }

    fn seam_command_stale_reason(
        &self,
        args: &serde_json::Map<String, serde_json::Value>,
        snapshot: &AnalysisSnapshot,
    ) -> Option<&'static str> {
        let health = self.analysis_health_snapshot();
        let legacy_test_snapshot = health.snapshot_id.is_none()
            && health.attempt_id.is_none()
            && health.reason.is_none()
            && health.current_input_identity.is_none();
        if !legacy_test_snapshot && !health.allows_current_repairs() {
            return Some("analysis input is no longer current; refresh the workspace evidence");
        }
        if snapshot.seams_deferred {
            return Some(
                "the current deferred refresh does not retain the cited full-seam evidence",
            );
        }
        let cited_identity = args
            .get("evidence_identity")
            .filter(|value| !value.is_null());
        if let Some(cited_identity) = cited_identity {
            if !AnalysisSnapshot::evidence_identities_match(
                Some(cited_identity),
                Some(&snapshot.evidence_identity()),
            ) {
                return Some("the cited seam evidence belongs to an older snapshot");
            }
        } else if !legacy_test_snapshot {
            return Some("the cited seam action has no evidence identity");
        }
        let seam_id = args.get("seam_id").and_then(|v| v.as_str());
        if seam_id.is_some_and(|id| snapshot.classified_seam_by_id(id).is_none()) {
            return Some("the cited seam is absent from the current full-seam evidence");
        }
        None
    }

    fn stale_seam_evidence_response(
        &self,
        args: &serde_json::Map<String, serde_json::Value>,
        reason: &str,
    ) -> LSPAny {
        let snapshot = self
            .latest_analysis
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let health = self.analysis_health_snapshot();
        let stale_evidence_identity = args
            .get("evidence_identity")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| {
                serde_json::json!({
                    "seam_id": args.get("seam_id").and_then(|value| value.as_str()),
                })
            });
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "lsp_seam_evidence",
            "status": "stale",
            "seam_id": args.get("seam_id").and_then(|value| value.as_str()),
            "stale_evidence_identity": stale_evidence_identity,
            "current_snapshot_id": snapshot
                .as_ref()
                .and_then(|value| value.refresh.snapshot_id.clone())
                .or(health.snapshot_id),
            "current_input_identity": snapshot
                .as_ref()
                .and_then(|value| value.input_identity_id())
                .or(health.current_input_identity),
            "invalidation_reason": reason,
            "recovery_route": "ripr.refreshDiagnostics",
            "recovery_command": "ripr.refreshDiagnostics",
            "limits_note": "Static editor evidence only; refresh before using seam context or repair guidance.",
        })
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
                    "limits_note": SAVED_WORKTREE_LIMITS_NOTE,
                }));
            }
        };
        let receipt_status_summary = workspace_status_receipt_summary(&root, &snapshot);

        let mut status_json = serde_json::json!({
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
            "harness_registry": workspace_status_harness_summary(&snapshot),
            "open_documents": open_documents,
            "report_paths": workspace_status_report_paths(),
            "refresh_command": REFRESH_COMMAND,
            "limits_note": SAVED_WORKTREE_LIMITS_NOTE,
        });
        // Repositories without registrations keep the documented absent-field
        // contract: the key is removed rather than serialized as null.
        if matches!(
            snapshot.harness_facts,
            crate::lsp::state::HarnessFactsOnSnapshot::NotRegistered
        ) && let Some(object) = status_json.as_object_mut()
        {
            object.remove("harness_registry");
        }
        Some(status_json)
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

/// Most omitted items `ripr/listActionableItems` lists; `omitted_count`
/// still reports the full number.
const LIST_ACTIONABLE_OMITTED_MAX: usize = 200;

/// Most profile-hidden gaps `ripr/listActionableItems` lists;
/// `hidden_gap_count` still reports the full number.
const LIST_ACTIONABLE_HIDDEN_MAX: usize = 50;

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

/// Bounded harness-registry disclosure for `collect_workspace_status`.
/// Preserves the three documented states: Null when the repository has no
/// registrations, `available: false` for a limited run that did not
/// compute harness facts, and the registration summaries when facts are
/// complete (#3605).
fn workspace_status_harness_summary(snapshot: &AnalysisSnapshot) -> serde_json::Value {
    match &snapshot.harness_facts {
        crate::lsp::state::HarnessFactsOnSnapshot::NotRegistered => serde_json::Value::Null,
        crate::lsp::state::HarnessFactsOnSnapshot::Unknown => serde_json::json!({
            "available": false,
            "reason": "unknown_this_run",
        }),
        crate::lsp::state::HarnessFactsOnSnapshot::UnavailableLimitedRun => serde_json::json!({
            "available": false,
            "reason": "limited_run",
        }),
        crate::lsp::state::HarnessFactsOnSnapshot::Complete(projections) => {
            let registrations: Vec<serde_json::Value> = projections
                .iter()
                .map(|projection| {
                    serde_json::json!({
                        "registration_id": projection.registration_id,
                        "harness_kind": projection.harness_kind,
                        "adapter": projection.adapter,
                        "target": projection.target.to_string_lossy(),
                        "subject_count": projection.subjects.len(),
                        "limitation_count": projection.limitations.len(),
                    })
                })
                .collect();
            serde_json::json!({ "available": true, "registrations": registrations })
        }
    }
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
        .map(|artifact| receipt_status_from_ledger(root, &ledger_path, artifact).0)
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
    super::diagnostics::derive_run_status_with_outcome(
        &snapshot.findings,
        &snapshot.gap_artifact_rejections,
        &snapshot.gap_artifacts,
        snapshot.seams_deferred,
        snapshot.partial_scope.is_some(),
        &snapshot.component_outcomes,
        snapshot.analysis_outcome.as_ref(),
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
    analysis_outcome: Option<crate::analysis_outcome::AnalysisOutcome>,
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
            "analysis_outcome": self.analysis_outcome,
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
    let analysis_outcome = snapshot.and_then(|snapshot| snapshot.analysis_outcome.clone());
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
        analysis_outcome: analysis_outcome.clone(),
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
                .map(|failure| format!("{}: {}", failure.kind.as_str(), failure.message))
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
            {
                let uninspected = if scope.has_known_uninspected_scope() {
                    format!(
                        "at least {} changed file(s) and {} changed line(s) were not inspected",
                        scope.uninspected_files_lower_bound,
                        scope.uninspected_changed_lines_lower_bound,
                    )
                } else {
                    "every changed file ripr's language adapters read was selected, but the budget was exceeded, so the \
                     result stays partial"
                        .to_string()
                };
                format!(
                    "analysis inspected {} changed file(s) ({} changed line(s)) of the diff; \
                     {uninspected} (stop reason: {}); to widen the analyzed partition, {}",
                    scope.selected_files.len(),
                    scope.selected_changed_lines,
                    scope.stop_reason.as_str(),
                    scope.widen_instruction(),
                )
            },
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

    if let Some(outcome) = snapshot
        .analysis_outcome
        .as_ref()
        .filter(|outcome| !outcome.kind.is_complete())
    {
        let limitation = outcome.limitations.first();
        let recovery_route = limitation
            .map(|limitation| limitation.recovery.kind.as_str())
            .unwrap_or("inspect_failure");
        let sample_sources = limitation
            .and_then(|limitation| limitation.path.clone())
            .into_iter()
            .collect();
        let why_not_actionable = if outcome.counts.finding_count == 0 {
            format!(
                "the producer reported incomplete static analysis ({}) and zero findings are not a clean result",
                outcome.kind.as_str()
            )
        } else {
            format!(
                "the producer reported incomplete static analysis ({}); reported findings do not make the input complete",
                outcome.kind.as_str()
            )
        };
        let limitation_count = outcome.limitations.len().max(1);
        return common(
            "analysis_outcome_incomplete",
            outcome.kind.as_str(),
            "limited",
            why_not_actionable,
            recovery_route,
            sample_sources,
            1,
            limitation_count,
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
    use crate::analysis::{PartialDiffScope, PartialDiffStopReason};
    use crate::analysis_outcome::{
        AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
        AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
        AnalysisStage,
    };
    use crate::lsp::state::RefreshMetadata;
    use tower_lsp_server::LspService;

    fn incomplete_outcome(finding_count: u64) -> Result<AnalysisOutcome, String> {
        let limitation = AnalysisLimitation::new(
            AnalysisLimitationKind::UnresolvedConflictMarkers,
            AnalysisStage::DiffParse,
            AnalysisRecovery::new(
                AnalysisRecoveryKind::ResolveConflicts,
                "resolve conflict markers before retrying analysis",
            )?,
        )
        .with_path("src/lib.rs")?
        .with_affected_items(1)?;
        AnalysisOutcome::new(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisIdentity {
                input_identity: Some("sha256:lsp-fixture".to_string()),
                ..AnalysisIdentity::default()
            },
            AnalysisOutcomeCounts {
                candidate_line_count: finding_count,
                finding_count,
                ..AnalysisOutcomeCounts::default()
            },
            vec![limitation],
        )
    }

    fn snapshot_for_outcome(
        outcome: AnalysisOutcome,
        partial_scope: Option<PartialDiffScope>,
    ) -> AnalysisSnapshot {
        AnalysisSnapshot {
            root: PathBuf::from("C:").join("repo"),
            input_identity: None,
            base: Some("main".to_string()),
            mode: crate::app::Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            analysis_outcome: Some(outcome),
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            harness_facts: crate::lsp::state::HarnessFactsOnSnapshot::NotRegistered,
            diagnostics_by_uri: BTreeMap::new(),
            delivery_selection: None,
            seams_deferred: false,
            partial_scope,
            component_outcomes: Vec::new(),
            out_of_scope_test_file_findings: 0,
        }
    }

    fn partial_scope_fixture() -> PartialDiffScope {
        PartialDiffScope {
            run_status: PartialDiffScope::RUN_STATUS.to_string(),
            diff_identity: "sha256:diff".to_string(),
            file_budget: 1,
            line_budget: 10,
            budget_disclosures: Vec::new(),
            selected_files: vec!["src/lib.rs".to_string()],
            selected_changed_lines: 4,
            uninspected_files_lower_bound: 2,
            uninspected_changed_lines_lower_bound: 6,
            stop_reason: PartialDiffStopReason::FileBudget,
            next_file_changed_lines: Some(3),
            partition_identity: "sha256:partition".to_string(),
        }
    }

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

    #[test]
    fn top_limitation_retains_typed_incomplete_outcome() -> Result<(), String> {
        let repo_root = PathBuf::from("C:").join("repo");
        let repo_root_text = repo_root.to_string_lossy().to_string();
        let snapshot = snapshot_for_outcome(incomplete_outcome(0)?, None);
        let snapshot = AnalysisSnapshot {
            root: repo_root.clone(),
            ..snapshot
        };
        let health = AnalysisHealth {
            state: AnalysisAttemptState::Succeeded,
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some("limited_incomplete_input".to_string()),
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(repo_root.clone());
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        assert_eq!(value["status"], "analysis_outcome_incomplete");
        assert_eq!(value["selected_count"], 1);
        assert_eq!(value["total_count"], 1);
        assert_eq!(value["limitation_category"], "unsupported_input");
        assert_eq!(value["run_status"], "limited_incomplete_input");
        assert_eq!(value["analysis_outcome"]["kind"], "unsupported_input");
        assert_eq!(
            value["analysis_outcome"]["limitations"][0]["recovery"]["kind"],
            "resolve_conflicts"
        );
        assert!(
            !value["analysis_outcome"]
                .to_string()
                .contains(&repo_root_text)
        );

        let (service, _socket) = LspService::new(|client| Backend::new(client, repo_root));
        let backend = service.inner();
        *backend
            .latest_analysis
            .lock()
            .map_err(|error| format!("latest analysis lock poisoned: {error}"))? =
            Some(std::sync::Arc::new(snapshot));
        let analysis_status = backend.analysis_status_payload_for_health(&health);
        assert_eq!(analysis_status["run_status"], "limited_incomplete_input");
        assert_eq!(
            analysis_status["analysis_outcome"]["kind"],
            "unsupported_input"
        );
        Ok(())
    }

    #[test]
    fn partial_scope_precedes_incomplete_outcome_and_retains_it() -> Result<(), String> {
        let snapshot = snapshot_for_outcome(incomplete_outcome(0)?, Some(partial_scope_fixture()));
        let health = AnalysisHealth {
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some(PartialDiffScope::RUN_STATUS.to_string()),
            state: AnalysisAttemptState::Succeeded,
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(PathBuf::from("C:").join("repo"));
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        assert_eq!(value["status"], "limited_partial_scope");
        assert_eq!(value["run_status"], "limited_partial_scope");
        assert_eq!(value["analysis_outcome"]["kind"], "unsupported_input");
        assert!(
            value["why_not_actionable"]
                .as_str()
                .is_some_and(|text| text.contains("at least 2 changed file(s)"))
        );
        let why = value["why_not_actionable"].as_str().unwrap_or_default();
        assert!(
            why.contains("raise RIPR_PARTIAL_DIFF_FILE_BUDGET to at least 2, then re-run"),
            "the LSP limitation must name the stopping budget and its size: {why}"
        );
        assert!(!why.contains("and/or"), "no generic budget wording: {why}");
        Ok(())
    }

    #[test]
    fn partial_scope_with_no_known_uninspected_files_never_says_at_least_zero() -> Result<(), String>
    {
        let mut scope = partial_scope_fixture();
        scope.uninspected_files_lower_bound = 0;
        scope.uninspected_changed_lines_lower_bound = 0;
        scope.stop_reason = PartialDiffStopReason::LineBudgetExceededOnFirstFile;
        scope.selected_changed_lines = 14;
        scope.next_file_changed_lines = None;
        let snapshot = snapshot_for_outcome(incomplete_outcome(0)?, Some(scope));
        let health = AnalysisHealth {
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some(PartialDiffScope::RUN_STATUS.to_string()),
            state: AnalysisAttemptState::Succeeded,
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(PathBuf::from("C:").join("repo"));
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        assert_eq!(value["status"], "limited_partial_scope");
        let why = value["why_not_actionable"].as_str().unwrap_or_default();
        assert!(!why.contains("at least 0"), "{why}");
        assert!(
            why.contains("every changed file ripr's language adapters read was selected, but the budget was exceeded"),
            "{why}"
        );
        assert!(
            why.contains("raise RIPR_PARTIAL_DIFF_LINE_BUDGET to at least 14, then re-run"),
            "a first-file line stop names the line budget first: {why}"
        );
        Ok(())
    }

    #[test]
    fn failed_retained_snapshot_retains_typed_incomplete_outcome() -> Result<(), String> {
        let snapshot = snapshot_for_outcome(incomplete_outcome(0)?, None);
        let health = AnalysisHealth {
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some("limited_incomplete_input".to_string()),
            state: AnalysisAttemptState::Failed,
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(PathBuf::from("C:").join("repo"));
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        assert_eq!(value["status"], "analysis_failed_retained_snapshot");
        assert_eq!(value["analysis_outcome"]["kind"], "unsupported_input");
        Ok(())
    }

    #[test]
    fn stale_snapshot_retains_typed_incomplete_outcome() -> Result<(), String> {
        let snapshot = snapshot_for_outcome(incomplete_outcome(0)?, None);
        let health = AnalysisHealth {
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some("stale".to_string()),
            state: AnalysisAttemptState::Succeeded,
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(PathBuf::from("C:").join("repo"));
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();

        assert_eq!(value["status"], "snapshot_stale");
        assert_eq!(value["analysis_outcome"]["kind"], "unsupported_input");
        Ok(())
    }

    #[test]
    fn incomplete_outcome_with_findings_does_not_claim_zero_findings() -> Result<(), String> {
        let snapshot = snapshot_for_outcome(incomplete_outcome(2)?, None);
        let health = AnalysisHealth {
            snapshot_id: Some("snapshot:lsp-fixture".to_string()),
            snapshot_run_status: Some("limited_incomplete_input".to_string()),
            state: AnalysisAttemptState::Succeeded,
            ..AnalysisHealth::default()
        };
        let authority = WorkspaceRootAuthority::selected(PathBuf::from("C:").join("repo"));
        let value = top_limitation_dto(&health, Some(&snapshot), &authority).into_json();
        let why_not_actionable = value["why_not_actionable"]
            .as_str()
            .ok_or_else(|| "expected why_not_actionable text".to_string())?;

        assert!(!why_not_actionable.contains("zero findings"));
        assert!(why_not_actionable.contains("reported findings do not make the input complete"));
        assert_eq!(value["analysis_outcome"]["kind"], "unsupported_input");
        Ok(())
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
        GapArtifactRejection::StaleSubject(_) => (
            "regenerate_gap_artifacts",
            "gap artifacts describe other source file contents; regenerate them with ripr reports gap-ledger or cargo xtask lane1-evidence-audit",
        ),
        GapArtifactRejection::UnverifiableSubject(_) => (
            "regenerate_gap_artifacts",
            "gap artifacts carry no usable source_subject stamp; regenerate them with ripr reports gap-ledger or cargo xtask lane1-evidence-audit",
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
        | GapArtifactRejection::WrongRoot(s)
        | GapArtifactRejection::StaleSubject(s) => vec![s.clone()],
        GapArtifactRejection::UnverifiableSubject(reason) => vec![(*reason).to_string()],
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
        | "stale_subject"
        | "unverifiable_subject"
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
    let ledger_path = absolute_join(root, Path::new(ledger_arg));
    // Client-supplied path: confine reads to the workspace root so a client
    // message cannot make the long-running server read arbitrary files.
    // `absolute_join` passes absolute paths through unchanged, so an
    // out-of-root absolute path or a symlink escape is rejected here
    // (canonicalizing both sides, like `validate_agent_brief_diff_path`).
    if !path_is_within_root(root, &ledger_path) {
        return None;
    }
    let contents = match read_artifact_capped(&ledger_path) {
        CappedArtifactRead::Contents(contents) => contents,
        // Absent ledger: no packet is available, which is a normal state.
        CappedArtifactRead::Missing => return None,
        // Present but oversize/unreadable: typed sentinel, not a silent None.
        CappedArtifactRead::Unusable => {
            return Some(repair_packet_sentinel(MALFORMED_GAP_LEDGER_REASON));
        }
    };
    let records = match parse_gap_records_json_for_root(&contents, Some(root)) {
        Ok(records) => records,
        Err(_) => return Some(repair_packet_sentinel(MALFORMED_GAP_LEDGER_REASON)),
    };
    let record = records
        .iter()
        .find(|record| gap_record_matches(record, gap_id))?;
    if let Some(sentinel) = ledger_source_subject_sentinel(root, &contents) {
        return Some(sentinel);
    }
    let rendered = render_agent_gap_record_packet_json(&display_path(&ledger_path), record).ok()?;
    serde_json::from_str(&rendered).ok()
}

/// #4544: a command-time packet read re-checks the ledger's `source_subject`
/// stamp through the shared validator, so a packet computed for other file
/// contents is a typed stale disclosure instead of a current repair route.
fn ledger_source_subject_sentinel(root: &Path, contents: &str) -> Option<LSPAny> {
    let Ok(artifact) = serde_json::from_str::<serde_json::Value>(contents) else {
        return Some(repair_packet_sentinel(MALFORMED_GAP_LEDGER_REASON));
    };
    source_subject_sentinel(
        root,
        &artifact,
        super::gap_artifacts::GapArtifactKind::GapDecisionLedger,
        "gap-decision-ledger.json",
        "ripr reports gap-ledger",
    )
}

fn source_subject_sentinel(
    root: &Path,
    artifact: &serde_json::Value,
    kind: super::gap_artifacts::GapArtifactKind,
    artifact_name: &str,
    regenerate: &str,
) -> Option<LSPAny> {
    use super::gap_artifacts::GapArtifactRejection;
    match super::gap_artifacts::validate_source_subject(artifact, kind, root) {
        Ok(()) => None,
        Err(GapArtifactRejection::StaleSubject(path)) => Some(repair_packet_sentinel(&format!(
            "stale_subject: {path} changed since {artifact_name} was written; regenerate it for the current source with {regenerate}"
        ))),
        Err(GapArtifactRejection::UnverifiableSubject(reason)) => {
            Some(repair_packet_sentinel(&format!(
                "unverifiable_subject: {reason}: {artifact_name} cannot be matched to the current source files; regenerate it with {regenerate}"
            )))
        }
        Err(other) => Some(repair_packet_sentinel(other.as_str())),
    }
}

const DEFAULT_ACTIONABLE_GAPS_OUT: &str = "target/ripr/reports/actionable-gaps.json";

impl Backend {
    /// `gap_id` is already validated by `optional_id_argument`: `None` means
    /// the caller asked for the top packet, never that a malformed id was
    /// dropped.
    fn collect_repair_packet(&self, gap_id: Option<&str>) -> Option<LSPAny> {
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

        // Try actionable-gaps.json first (preferred: projection-validated).
        let actionable_path = absolute_join(&root, Path::new(DEFAULT_ACTIONABLE_GAPS_OUT));
        if let Some(result) =
            collect_repair_packet_from_actionable_gaps(&root, &actionable_path, gap_id)
        {
            return Some(result);
        }

        // Fallback: gap-decision-ledger.json using the existing GapRecord machinery.
        let ledger_path = absolute_join(&root, Path::new(DEFAULT_GAP_DECISION_LEDGER_OUT));
        collect_repair_packet_from_ledger(&root, &ledger_path, gap_id)
            // Neither source holds a packet: say so and name the route,
            // instead of a null the client can only render as "no response".
            .or_else(|| Some(repair_packet_sentinel(&no_repair_packet_reason(gap_id))))
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
            serde_json::Value::String(display_path(&attempt_ledger_path))
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
        receipt_status_from_ledger(root, &ledger_path, artifact);

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
    root: &std::path::Path,
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
    let records = match crate::output::gap_decision_ledger::parse_gap_records_json_for_root(
        &contents,
        Some(root),
    ) {
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
        // Another gap's attempt is not this gap's outcome.
        attempts.iter().find(|entry| {
            entry
                .get("canonical_gap_id")
                .and_then(|v| v.as_str())
                .is_some_and(|cid| cid == canonical_gap_id)
        })
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

fn collect_repair_packet_from_actionable_gaps(
    root: &Path,
    path: &Path,
    gap_id: Option<&str>,
) -> Option<LSPAny> {
    let contents = match read_artifact_capped(path) {
        CappedArtifactRead::Contents(contents) => contents,
        // Absent artifact: falling back to the next packet source is honest.
        CappedArtifactRead::Missing => return None,
        // Present but oversize/unreadable: typed sentinel so the client can
        // distinguish a corrupt source from "no packet"; no fallback.
        CappedArtifactRead::Unusable => {
            return Some(repair_packet_sentinel(MALFORMED_ACTIONABLE_GAPS_REASON));
        }
    };
    let report: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(report) => report,
        Err(_) => return Some(repair_packet_sentinel(MALFORMED_ACTIONABLE_GAPS_REASON)),
    };
    let packets = report.get("packets").and_then(|v| v.as_array())?;
    // A requested gap must match exactly. Answering an unknown id with the
    // first packet handed out another gap's repair instructions; `None` lets
    // the caller try the ledger, then say which gap has no packet.
    let packet = if let Some(id) = gap_id {
        packets.iter().find(|p| {
            p.get("canonical_gap_id")
                .and_then(|v| v.as_str())
                .is_some_and(|cid| cid == id)
        })?
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
    if let Some(sentinel) = source_subject_sentinel(
        root,
        &report,
        super::gap_artifacts::GapArtifactKind::ActionableGaps,
        "actionable-gaps.json",
        "cargo xtask lane1-evidence-audit",
    ) {
        return Some(sentinel);
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

fn collect_repair_packet_from_ledger(
    root: &Path,
    path: &Path,
    gap_id: Option<&str>,
) -> Option<LSPAny> {
    let contents = match read_artifact_capped(path) {
        CappedArtifactRead::Contents(contents) => contents,
        // Absent artifact: no packet is available, which is a normal state.
        CappedArtifactRead::Missing => return None,
        // Present but oversize/unreadable: typed sentinel, not a silent None.
        CappedArtifactRead::Unusable => {
            return Some(repair_packet_sentinel(MALFORMED_GAP_LEDGER_REASON));
        }
    };
    let records = match parse_gap_records_json_for_root(&contents, Some(root)) {
        Ok(records) => records,
        Err(_) => return Some(repair_packet_sentinel(MALFORMED_GAP_LEDGER_REASON)),
    };
    let record = if let Some(id) = gap_id {
        records
            .iter()
            .find(|r| r.gap_id == id || r.canonical_gap_id == id)?
    } else {
        records.iter().find(|r| r.gap_state == "actionable")?
    };
    if let Some(sentinel) = ledger_source_subject_sentinel(root, &contents) {
        return Some(sentinel);
    }

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

fn no_repair_packet_reason(gap_id: Option<&str>) -> String {
    let subject = match gap_id {
        Some(id) => {
            let mut echoed: String = id.chars().take(ECHOED_ID_MAX_CHARS).collect();
            if echoed.len() < id.len() {
                echoed.push_str("...");
            }
            format!("no repair packet for gap `{echoed}`")
        }
        None => "no repair packet".to_string(),
    };
    format!(
        "{subject}: neither {DEFAULT_ACTIONABLE_GAPS_OUT} nor {DEFAULT_GAP_DECISION_LEDGER_OUT} \
holds one; run `ripr pilot --root .` for seams with their repair commands, or write the gap \
ledger with `ripr reports gap-ledger --check-output PATH` from saved `ripr check --format json` \
output"
    )
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

/// Typed-degradation reasons for a present-but-unparseable packet source.
/// Vocabulary mirrors `limitation_non_claims("malformed_artifact")`: the
/// client must be able to tell "no packet" apart from "packet source corrupt;
/// artifact regeneration required before exposure can be assessed".
const MALFORMED_ACTIONABLE_GAPS_REASON: &str = "actionable-gaps.json is malformed; artifact regeneration required before exposure can be assessed";
const MALFORMED_GAP_LEDGER_REASON: &str = "gap-decision-ledger.json is malformed; artifact regeneration required before exposure can be assessed";

fn gap_record_matches(record: &GapRecord, gap_id: &str) -> bool {
    record.gap_id == gap_id || record.canonical_gap_id == gap_id
}

fn evidence_context_packet(snapshot: &AnalysisSnapshot, entry: &ClassifiedSeam) -> LSPAny {
    // #4001/#3999: loop commands bind the snapshot's selected workspace root,
    // not the language-server process working directory; the packet's
    // `root` field stays the portable role.
    let root = loop_commands::bound_root(&snapshot.root.to_string_lossy());
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
        "file": display_path(seam.file()),
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
            format!("{}::{}", display_path(&test.file), test.test_name)
        }),
        "related_test_location": related_test.map(|test| {
            serde_json::json!({
                "file": display_path(&test.file),
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
        // The repair start only for a seam `agent repair` would accept (the
        // fail-closed repair-packet flip, RIPR-SPEC-0087 §8, plus a
        // test-surface target); `null` otherwise (#3906).
        "repair_command": repair_start_command_for(entry, &root),
        "agent_packet_command": loop_commands::agent_packet_command(
            &root,
            seam_id,
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        ),
        "agent_brief_command": loop_commands::agent_brief_command(
            &root,
            seam_id,
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        ),
        "after_snapshot_command": loop_commands::check_repo_exposure_command_with_base(
            &root,
            snapshot.base.as_deref(),
            snapshot.mode.as_str(),
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        ),
        "verify_command": loop_commands::agent_verify_command(
            &root,
            loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
        ),
        "receipt_command": loop_commands::agent_receipt_command(
            &root,
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

#[cfg(test)]
mod gap_record_context_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    fn top_gap_named(canonical_gap_id: &str) -> super::super::gap_artifacts::ValidatedGapArtifact {
        use super::super::gap_artifacts::{
            GapArtifactIdentity, GapArtifactKind, ValidatedGapArtifact,
        };
        ValidatedGapArtifact {
            kind: GapArtifactKind::ActionableGaps,
            root: None,
            identities: vec![GapArtifactIdentity {
                canonical_gap_id: Some(canonical_gap_id.to_string()),
                seam_id: None,
                finding_id: None,
            }],
            language: None,
            language_status: None,
            gap_state: None,
            related_paths: Vec::new(),
            verify_commands: Vec::new(),
            receipt_commands: Vec::new(),
            verify_command_specs: Vec::new(),
            receipt_command_specs: Vec::new(),
            static_limit_kinds: Vec::new(),
            has_text_static_limit: false,
        }
    }

    #[test]
    fn latest_attempt_outcome_never_borrows_another_gaps_attempt() -> Result<(), String> {
        let root = temp_root()?;
        let ledger = root.join("target/ripr/reports/swarm-attempt-ledger.json");
        fs::write(
            &ledger,
            serde_json::json!({
                "latest_attempts": [
                    { "canonical_gap_id": "gap:rust:other", "outcome": "evidence_improved" },
                    { "canonical_gap_id": "gap:rust:top", "outcome": "no_movement" }
                ]
            })
            .to_string(),
        )
        .map_err(|err| format!("write attempt ledger failed: {err}"))?;

        // The top gap's own attempt is found even when it is not first.
        let top = top_gap_named("gap:rust:top");
        assert_eq!(
            read_latest_attempt_outcome(&ledger, Some(&top)),
            serde_json::json!("no_movement")
        );
        // A gap with no attempt of its own has no outcome; the first entry
        // belongs to another gap.
        let unattempted = top_gap_named("gap:rust:never-attempted");
        assert_eq!(
            read_latest_attempt_outcome(&ledger, Some(&unattempted)),
            serde_json::json!("not_available")
        );
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_context_packet_for_gap_id_reads_explicit_ledger() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
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
            "source_currentness": "candidate_current",
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
            "source_currentness": "candidate_current",
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
            display_path(&root.join(DEFAULT_GAP_DECISION_LEDGER_OUT))
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

    /// Writes the fixture ledger stamped against `root` as the producer does
    /// (#4544); the fixture's source files are absent, so the stamp is current.
    fn write_gap_ledger(root: &Path) -> Result<(), String> {
        let path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        let ledger = serde_json::from_str::<serde_json::Value>(gap_ledger_json())
            .map_err(|err| format!("parse fixture ledger failed: {err}"))?;
        let stamped = crate::output::gap_source_subject::with_source_subject_for_test(root, ledger);
        fs::write(path, stamped.to_string())
            .map_err(|err| format!("write gap ledger in {} failed: {err}", root.display()))
    }

    /// #4544: the command-time context and repair packet readers re-check
    /// the ledger stamp; an edited anchor file or a missing stamp returns a
    /// typed sentinel instead of a current-looking packet.
    #[test]
    fn gap_packets_disclose_stale_or_unverifiable_ledger_subject() -> Result<(), String> {
        let root = temp_root()?;
        let result = (|| {
            write_gap_ledger(&root)?;
            let args_value = serde_json::json!({
                "gap_id": "gap:pr:pricing:threshold-boundary",
                "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
            });
            let args = args_value
                .as_object()
                .ok_or_else(|| "expected object args".to_string())?;
            let ledger_path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
            let gap_id = "gap:pr:pricing:threshold-boundary";

            let current = collect_gap_record_context_packet(&root, args, gap_id)
                .ok_or_else(|| "expected a current packet".to_string())?;
            if current["status"] == "not_actionable_or_incomplete" {
                return Err(format!(
                    "a current ledger must render its packet: {current}"
                ));
            }

            // The anchor file appears after the ledger stamped it as absent.
            fs::create_dir_all(root.join("src")).map_err(|err| err.to_string())?;
            fs::write(root.join("src/pricing.rs"), "pub fn price() {}\n")
                .map_err(|err| err.to_string())?;
            for packet in [
                collect_gap_record_context_packet(&root, args, gap_id),
                collect_repair_packet_from_ledger(&root, &ledger_path, Some(gap_id)),
            ] {
                let packet = packet.ok_or_else(|| "expected a stale sentinel".to_string())?;
                let reason = packet["reason"].as_str().unwrap_or_default();
                if packet["status"] != "not_actionable_or_incomplete"
                    || !reason.starts_with("stale_subject: src/pricing.rs changed")
                    || !reason.contains("ripr reports gap-ledger")
                {
                    return Err(format!("unexpected stale packet: {packet}"));
                }
            }

            // A ledger without a stamp cannot be matched to the workspace.
            fs::write(&ledger_path, gap_ledger_json()).map_err(|err| err.to_string())?;
            let packet = collect_gap_record_context_packet(&root, args, gap_id)
                .ok_or_else(|| "expected an unverifiable sentinel".to_string())?;
            if !packet["reason"]
                .as_str()
                .unwrap_or_default()
                .starts_with("unverifiable_subject: source_subject_missing")
            {
                return Err(format!("unexpected unstamped packet: {packet}"));
            }
            Ok(())
        })();
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        result
    }

    fn gap_ledger_json() -> &'static str {
        r#"{
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "source_currentness": "candidate_current",
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
            "source_currentness": "candidate_current",
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
    fn collect_gap_record_context_packet_with_malformed_ledger_returns_sentinel()
    -> Result<(), String> {
        let root = temp_root()?;
        let path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        fs::write(path, "{ not valid json")
            .map_err(|err| format!("write malformed ledger in {} failed: {err}", root.display()))?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
            "gap_ledger": DEFAULT_GAP_DECISION_LEDGER_OUT,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .ok_or_else(|| "expected malformed-artifact sentinel, not null".to_string())?;
        assert_eq!(packet["kind"], "repair_packet");
        assert_eq!(packet["status"], "not_actionable_or_incomplete");
        assert_eq!(packet["reason"], MALFORMED_GAP_LEDGER_REASON);

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_rejects_out_of_root_ledger() -> Result<(), String> {
        let root = temp_root()?;
        // A client-supplied absolute path outside the workspace root must not
        // be read, even when the file exists and is a valid ledger.
        let outside = std::env::temp_dir().join(format!(
            "ripr-lsp-gap-record-outside-{}-{}",
            std::process::id(),
            TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&outside)
            .map_err(|err| format!("create outside dir {} failed: {err}", outside.display()))?;
        let outside_ledger = outside.join("gap-decision-ledger.json");
        fs::write(&outside_ledger, gap_ledger_json()).map_err(|err| {
            format!(
                "write outside ledger {} failed: {err}",
                outside_ledger.display()
            )
        })?;
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
            "gap_ledger": outside_ledger,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .is_none()
        );

        fs::remove_dir_all(&outside)
            .map_err(|err| format!("remove outside dir {} failed: {err}", outside.display()))?;
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_accepts_in_root_absolute_ledger() -> Result<(), String> {
        let root = temp_root()?;
        write_gap_ledger(&root)?;
        // An absolute path that stays at-or-below the root remains readable.
        let absolute_ledger = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
            "gap_ledger": absolute_ledger,
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        let packet =
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .ok_or_else(|| "expected gap packet".to_string())?;
        assert_eq!(packet["source"], "gap_decision_ledger");

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn collect_gap_record_context_packet_rejects_symlink_escape_ledger() -> Result<(), String> {
        let root = temp_root()?;
        let outside = std::env::temp_dir().join(format!(
            "ripr-lsp-gap-record-symlink-outside-{}-{}",
            std::process::id(),
            TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&outside)
            .map_err(|err| format!("create outside dir {} failed: {err}", outside.display()))?;
        let outside_ledger = outside.join("gap-decision-ledger.json");
        fs::write(&outside_ledger, gap_ledger_json()).map_err(|err| {
            format!(
                "write outside ledger {} failed: {err}",
                outside_ledger.display()
            )
        })?;
        let link = root.join("linked-ledger");
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&outside_ledger, &link);
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_file(&outside_ledger, &link);
        if let Err(err) = link_result {
            eprintln!("skipping symlink escape test: {err}");
            let _ = fs::remove_dir_all(&outside);
            let _ = fs::remove_dir_all(&root);
            return Ok(());
        }
        let args_value = serde_json::json!({
            "gap_id": "gap:pr:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
            "gap_ledger": "linked-ledger",
        });
        let args = args_value
            .as_object()
            .ok_or_else(|| "expected object args".to_string())?;

        assert!(
            collect_gap_record_context_packet(&root, args, "gap:pr:pricing:threshold-boundary")
                .is_none(),
            "symlink escape out of the workspace root must be rejected"
        );

        fs::remove_dir_all(&outside)
            .map_err(|err| format!("remove outside dir {} failed: {err}", outside.display()))?;
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
            "source_currentness": "candidate_current",
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
            "source_currentness": "candidate_current",
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
            display_path(&root.join(DEFAULT_GAP_DECISION_LEDGER_OUT))
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    /// Parse miss for unreadably shaped arguments: `None` here is a bad
    /// request, not "no evidence". Dispatch converts it to InvalidParams
    /// (#4358) instead of JSON-RPC `result: null`.
    fn assert_context_arguments_parse_miss_is_typed_invalid_params(
        arguments: &[LSPAny],
        command: &str,
        keys: &[&'static str],
        shapes: &str,
    ) -> Result<(), String> {
        if context_arguments(arguments).is_some() {
            return Err(
                "unreadably shaped args are a parse miss at `context_arguments`, not a packet"
                    .to_string(),
            );
        }
        let error = match context_command_target(command, arguments, keys, shapes) {
            Ok(target) => {
                return Err(format!(
                    "dispatch must convert a parse miss into InvalidParams, got target {target:?}"
                ));
            }
            Err(error) => error,
        };
        if error.code != tower_lsp_server::jsonrpc::ErrorCode::InvalidParams {
            return Err(format!(
                "expected InvalidParams, got code {:?}: {}",
                error.code, error.message
            ));
        }
        if !(error.message.contains(command) && error.message.contains("expects one object")) {
            return Err(format!(
                "error must name the command and accepted object shape: {}",
                error.message
            ));
        }
        Ok(())
    }

    #[test]
    fn context_arguments_returns_none_for_empty_argument_list() -> Result<(), String> {
        assert_context_arguments_parse_miss_is_typed_invalid_params(
            &[],
            COLLECT_CONTEXT_COMMAND,
            &["gap_id", "seam_id", "finding_id"],
            COLLECT_CONTEXT_ARGUMENT_SHAPES,
        )?;
        assert_context_arguments_parse_miss_is_typed_invalid_params(
            &[],
            COLLECT_EVIDENCE_CONTEXT_COMMAND,
            &["seam_id"],
            COLLECT_EVIDENCE_CONTEXT_ARGUMENT_SHAPES,
        )
    }

    #[test]
    fn context_arguments_returns_none_when_first_argument_is_not_an_object() -> Result<(), String> {
        let arg = serde_json::Value::String("not-an-object".to_string());
        let arguments = std::slice::from_ref(&arg);
        assert_context_arguments_parse_miss_is_typed_invalid_params(
            arguments,
            COLLECT_CONTEXT_COMMAND,
            &["gap_id", "seam_id", "finding_id"],
            COLLECT_CONTEXT_ARGUMENT_SHAPES,
        )?;
        assert_context_arguments_parse_miss_is_typed_invalid_params(
            arguments,
            COLLECT_EVIDENCE_CONTEXT_COMMAND,
            &["seam_id"],
            COLLECT_EVIDENCE_CONTEXT_ARGUMENT_SHAPES,
        )?;
        let null_arg = serde_json::Value::Null;
        assert_context_arguments_parse_miss_is_typed_invalid_params(
            std::slice::from_ref(&null_arg),
            COLLECT_CONTEXT_COMMAND,
            &["gap_id", "seam_id", "finding_id"],
            COLLECT_CONTEXT_ARGUMENT_SHAPES,
        )
    }

    #[test]
    fn gap_record_matches_compares_pr_local_and_canonical_ids() -> Result<(), String> {
        let records = crate::output::gap_decision_ledger::parse_gap_records_json(gap_ledger_json())
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
    fn absolute_join_keeps_absolute_paths_and_joins_relative_paths() -> Result<(), String> {
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

        assert_eq!(absolute_join(&root, &already_absolute), already_absolute);
        assert_eq!(
            absolute_join(&root, Path::new("nested/file.json")),
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
    fn display_path_normalizes_backslashes_to_forward_slashes() {
        let path = std::path::PathBuf::from(r"a\b\c.rs");
        assert_eq!(display_path(&path), "a/b/c.rs");
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
            analysis_outcome: None,
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            harness_facts: crate::lsp::state::HarnessFactsOnSnapshot::NotRegistered,
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
    fn hover_prefers_gap_diagnostic_over_overlapping_finding() -> Result<(), String> {
        let harness = parity_backend()?;
        let backend = harness.service.inner();
        let uri = parity_uri("pricing.rs")?;
        let mut finding = parity_diagnostic("finding:pricing", true);
        finding.range = tower_lsp_server::ls_types::Range::new(
            tower_lsp_server::ls_types::Position::new(0, 0),
            tower_lsp_server::ls_types::Position::new(0, 20),
        );
        let mut gap = finding.clone();
        gap.data = Some(serde_json::json!({
            "diagnostic_id": "gap:pricing",
            "source": "gap_decision_ledger",
            "gap_id": "gap:rust:pricing:threshold-boundary",
            "source_currentness": "candidate_current",
            "language": "rust",
            "language_status": "supported",
            "repairability": "actionable",
            "repair_route": "AddBoundaryAssertion"
        }));

        commit(
            backend,
            parity_workspace_diagnostics(vec![(uri.clone(), vec![finding, gap])]),
        )?;
        let params = HoverParams {
            text_document_position_params: tower_lsp_server::ls_types::TextDocumentPositionParams {
                text_document: tower_lsp_server::ls_types::TextDocumentIdentifier::new(uri),
                position: tower_lsp_server::ls_types::Position::new(0, 1),
            },
            work_done_progress_params: tower_lsp_server::ls_types::WorkDoneProgressParams::default(
            ),
        };

        let hover = backend
            .hover_for_position(&params)
            .ok_or_else(|| "overlapping gap diagnostic should provide a hover".to_string())?;
        let hover_text = serde_json::to_string(&hover).map_err(|error| error.to_string())?;
        if !hover_text.contains("gap:rust:pricing:threshold-boundary") {
            return Err(format!(
                "overlapping gap diagnostic did not own the hover content: {hover_text}"
            ));
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

    #[test]
    fn deadline_expiry_drops_refresh_with_named_outcome_and_one_progress_end() -> Result<(), String>
    {
        // #1972: a test-armed token stands in for the physical deadline timer
        // (a real millisecond-overrun through the full loop needs a selected
        // workspace-root authority, which no backend test drives today). The
        // terminal chain is pinned end to end: named outcome, distinct
        // progress-end message, exactly-once end, idle scheduler.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))?;
        runtime.block_on(async {
            let (service, _socket) =
                LspService::new(|client| Backend::new(client, PathBuf::from(".")));
            let backend = service.inner();
            let sink = backend.install_progress_recorder();

            let decision = backend.refresh_scheduler.request(
                PathBuf::from("/workspace"),
                LspAnalysisConfig::default(),
                1,
                0,
                RefreshScope::Interactive,
                RefreshReason::DidSave,
            );
            let RefreshDecision::Start(request) = &decision else {
                return Err(format!("expected Start decision, got {decision:?}"));
            };
            let request = request.as_ref().clone();
            backend.emit_progress_for_decision(&decision).await;

            // The deadline timer fired: the attempt token carries the named
            // DeadlineExceeded kind.
            if !request
                .cancellation
                .cancel(AnalysisAbortKind::DeadlineExceeded)
            {
                return Err("deadline cancel must win on a fresh attempt token".to_string());
            }
            let outcome = cancellation_outcome(&request);
            if outcome != RefreshAttemptOutcome::DeadlineExceeded {
                return Err(format!(
                    "expiry must produce the named DeadlineExceeded outcome, got {outcome:?}"
                ));
            }
            if outcome.as_str() != "deadline_exceeded" {
                return Err(format!(
                    "the outcome must be named deadline_exceeded, got {}",
                    outcome.as_str()
                ));
            }

            backend.end_progress_for_attempt(&request, outcome).await;
            for _ in 0..20 {
                tokio::task::yield_now().await;
                if sink
                    .events()
                    .iter()
                    .any(|event| matches!(event, ProgressEvent::End { .. }))
                {
                    break;
                }
            }
            let ends: Vec<String> = sink
                .events()
                .iter()
                .filter_map(|event| match event {
                    ProgressEvent::End { message, .. } => Some(message.clone()),
                    _ => None,
                })
                .collect();
            if ends != vec!["analysis deadline exceeded".to_string()] {
                return Err(format!(
                    "expiry must end progress exactly once with the deadline message: {ends:?}"
                ));
            }
            // Exactly-once: a repeated terminal end for the same attempt is a no-op.
            backend.end_progress_for_attempt(&request, outcome).await;
            let ends_after = sink
                .events()
                .iter()
                .filter(|event| matches!(event, ProgressEvent::End { .. }))
                .count();
            if ends_after != 1 {
                return Err("deadline end must stay exactly-once".to_string());
            }

            // Fail-closed drop: finishing the attempt frees the scheduler, so
            // the next request starts instead of queueing behind the corpse.
            if backend.refresh_scheduler.finish(&request, false).is_some() {
                return Err("no pending request should follow the dropped attempt".to_string());
            }
            let next = backend.refresh_scheduler.request(
                PathBuf::from("/workspace"),
                LspAnalysisConfig::default(),
                2,
                0,
                RefreshScope::Interactive,
                RefreshReason::DidSave,
            );
            if !matches!(next, RefreshDecision::Start(_)) {
                return Err(format!(
                    "scheduler must be idle after the dropped attempt, got {next:?}"
                ));
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

#[cfg(test)]
mod list_actionable_items_tests {
    use super::*;
    use crate::lsp::diagnostic_budget::{
        DiagnosticBudget, DiagnosticDeliverySelection, DiagnosticDeliveryUnavailableReason,
    };
    use crate::lsp::state::RefreshMetadata;
    use tower_lsp_server::LspService;
    use tower_lsp_server::ls_types::{Diagnostic, Uri};

    struct HandlerHarness {
        service: LspService<Backend>,
        // Keep the socket alive for the whole test, mirroring the other LSP
        // handler test harnesses in this crate.
        _socket: tower_lsp_server::ClientSocket,
        runtime: tokio::runtime::Runtime,
    }

    fn handler_harness() -> Result<HandlerHarness, String> {
        let (service, socket) = LspService::new(|client| Backend::new(client, PathBuf::from(".")));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to start test runtime: {error}"))?;
        Ok(HandlerHarness {
            service,
            _socket: socket,
            runtime,
        })
    }

    fn snapshot_with_selection(selection: Option<DiagnosticDeliverySelection>) -> AnalysisSnapshot {
        AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            input_identity: None,
            base: Some("main".to_string()),
            mode: crate::app::Mode::Draft,
            refresh: RefreshMetadata {
                snapshot_id: Some("snapshot:test-handler".to_string()),
                ..RefreshMetadata::default()
            },
            findings: Vec::new(),
            analysis_outcome: None,
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            harness_facts: crate::lsp::state::HarnessFactsOnSnapshot::NotRegistered,
            diagnostics_by_uri: BTreeMap::new(),
            delivery_selection: selection.map(Arc::new),
            seams_deferred: false,
            partial_scope: None,
            component_outcomes: Vec::new(),
            out_of_scope_test_file_findings: 0,
        }
    }

    fn install_snapshot(
        harness: &HandlerHarness,
        snapshot: AnalysisSnapshot,
    ) -> Result<(), String> {
        *harness
            .service
            .inner()
            .latest_analysis
            .lock()
            .map_err(|error| format!("latest analysis lock poisoned: {error}"))? =
            Some(Arc::new(snapshot));
        Ok(())
    }

    fn call_handler(harness: &HandlerHarness) -> Result<serde_json::Value, String> {
        harness
            .runtime
            .block_on(
                harness
                    .service
                    .inner()
                    .ripr_list_actionable_items(serde_json::Value::Null),
            )
            .map_err(|error| format!("handler returned an LSP error: {error}"))?
            .ok_or_else(|| "handler returned no payload".to_string())
    }

    /// A gap-ledger diagnostic whose budget eligibility is explicit, matching
    /// the delivery-selection parity fixtures.
    fn actionable_diagnostic(id: &str) -> Diagnostic {
        Diagnostic {
            message: format!("diagnostic {id}"),
            data: Some(serde_json::json!({
                "diagnostic_id": id,
                "gap_id": id,
                "headline_eligible": true,
            })),
            ..Default::default()
        }
    }

    /// A real `Applied` selection produced by the shared budget evaluator
    /// over one actionable diagnostic — no fabricated budget result.
    fn applied_selection() -> Result<DiagnosticDeliverySelection, String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<Uri>()
            .map_err(|error| format!("parse test URI: {error}"))?;
        let mut diagnostics_by_uri = BTreeMap::new();
        diagnostics_by_uri.insert(uri, vec![actionable_diagnostic("gap:1")]);
        Ok(DiagnosticDeliverySelection::evaluate(
            &diagnostics_by_uri,
            &DiagnosticBudget::default(),
            "snapshot:test-profile",
            "evidence:test",
        ))
    }

    #[test]
    fn list_actionable_items_without_snapshot_fails_closed() -> Result<(), String> {
        let harness = handler_harness()?;
        let response = call_handler(&harness)?;

        assert_eq!(response["error"]["kind"], "no_snapshot");
        assert_eq!(response["error"]["recovery_route"], "refresh");
        assert!(response["error"]["message"].is_string());
        // An error response must not present a snapshot identity.
        assert!(response.get("snapshot_id").is_none());
        assert!(response.get("kind").is_none());
        Ok(())
    }

    #[test]
    fn list_actionable_items_analysis_in_flight_fails_closed() -> Result<(), String> {
        let harness = handler_harness()?;
        install_snapshot(&harness, snapshot_with_selection(None))?;
        let response = call_handler(&harness)?;

        assert_eq!(response["error"]["kind"], "analysis_in_flight");
        assert_eq!(response["error"]["recovery_route"], "refresh");
        assert!(response["error"]["message"].is_string());
        assert!(response.get("snapshot_id").is_none());
        Ok(())
    }

    #[test]
    fn list_actionable_items_unavailable_delivery_fails_closed() -> Result<(), String> {
        let selection = DiagnosticDeliverySelection {
            budget: DiagnosticBudget::default(),
            outcome: DiagnosticDeliveryOutcome::Unavailable {
                reason: DiagnosticDeliveryUnavailableReason::Evaluation,
                detail: "test fixture: budget evaluation rejected".to_string(),
            },
        };
        let harness = handler_harness()?;
        install_snapshot(&harness, snapshot_with_selection(Some(selection)))?;
        let response = call_handler(&harness)?;

        assert_eq!(response["error"]["kind"], "analysis_in_flight");
        assert_eq!(response["error"]["recovery_route"], "refresh");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("unavailable"))
        );
        assert!(response.get("snapshot_id").is_none());
        Ok(())
    }

    #[test]
    fn list_actionable_items_applied_returns_bounded_envelope() -> Result<(), String> {
        let harness = handler_harness()?;
        install_snapshot(
            &harness,
            snapshot_with_selection(Some(applied_selection()?)),
        )?;
        let response = call_handler(&harness)?;

        assert_eq!(response["kind"], "actionable_items");
        assert_eq!(response["status"], "ok");
        // The interim generation identity propagates from the committed
        // snapshot (see the handler's interim-binding comment; the immutable
        // #1602 handle contract is a later slice).
        assert_eq!(response["snapshot_id"], "snapshot:test-handler");
        assert_eq!(response["selected_count"], 1);
        assert_eq!(response["omitted_count"], 0);
        assert_eq!(response["total_count"], 1);
        assert_eq!(
            response["selected"],
            serde_json::json!([{
                "canonical_id": "gap:1",
                "document": "file:///workspace/src/lib.rs",
            }])
        );
        assert_eq!(response["omitted"], serde_json::json!([]));
        assert_eq!(response["omitted_truncated"], false);
        assert!(
            response["budget_identity"]
                .as_str()
                .is_some_and(|identity| identity.contains("snapshot:test-profile"))
        );
        assert_eq!(response["complete_evidence_identity"], "evidence:test");
        assert_eq!(
            response["continuation_or_inspect_route"],
            "ripr/listActionableItems"
        );
        // Read-only invariants are part of every success envelope.
        assert_eq!(response["allowed_edit_surface"], "read_only");
        assert_eq!(
            response["must_not_change"],
            serde_json::json!(["source_edits", "workspace_edit", "autonomous_repair"])
        );
        assert!(response.get("error").is_none());
        Ok(())
    }

    /// Omitted items are listed with their reason and capped; the count
    /// still reports every omitted item.
    #[test]
    fn list_actionable_items_lists_omitted_items_up_to_the_cap() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<Uri>()
            .map_err(|error| format!("parse test URI: {error}"))?;
        let total = LIST_ACTIONABLE_OMITTED_MAX + 2;
        let diagnostics = (0..total)
            .map(|index| actionable_diagnostic(&format!("gap:{index:04}")))
            .collect::<Vec<_>>();
        let mut diagnostics_by_uri = BTreeMap::new();
        diagnostics_by_uri.insert(uri, diagnostics);
        let budget = DiagnosticBudget {
            max_items_per_document: 1,
            max_items_per_workspace_response: 1,
            ..DiagnosticBudget::default()
        };
        let selection = DiagnosticDeliverySelection::evaluate(
            &diagnostics_by_uri,
            &budget,
            "snapshot:test-profile",
            "evidence:test",
        );
        let harness = handler_harness()?;
        install_snapshot(&harness, snapshot_with_selection(Some(selection)))?;
        let response = call_handler(&harness)?;

        assert_eq!(response["selected_count"], 1);
        assert_eq!(response["omitted_count"], total - 1);
        let omitted = response["omitted"]
            .as_array()
            .ok_or("omitted must be an array")?;
        assert_eq!(omitted.len(), LIST_ACTIONABLE_OMITTED_MAX);
        assert_eq!(response["omitted_truncated"], true);
        assert!(
            omitted
                .iter()
                .all(|item| item["reason"] == "document_item_limit"
                    && item["canonical_id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("gap:"))),
            "every omitted item names its id and reason: {omitted:?}"
        );
        Ok(())
    }

    /// The actionable profile hides a candidate gap with no repair route (an
    /// new function no test calls is the common case); the list names it with an
    /// id `ripr.collectContext` takes. Exposed and base-side findings, and
    /// every finding under the full profile, are not listed.
    #[test]
    fn list_actionable_items_names_gaps_the_actionable_profile_hides() -> Result<(), String> {
        let hidden = crate::lsp::tests::sample_finding();
        let mut exposed = hidden.clone();
        exposed.id = "probe:pricing:90:predicate".to_string();
        exposed.class = crate::domain::ExposureClass::Exposed;
        let mut base_side = hidden.clone();
        base_side.id = "probe:pricing:92:predicate".to_string();
        base_side.source_currentness = crate::domain::SourceCurrentness::BaseDeleted;
        let mut unknown = hidden.clone();
        unknown.id = "probe:pricing:94:predicate".to_string();
        unknown.class = crate::domain::ExposureClass::StaticUnknown;
        // Two members of one canonical gap count once, as one diagnostic would.
        let mut grouped_first = hidden.clone();
        grouped_first.id = "probe:pricing:96:predicate".to_string();
        grouped_first.probe.location.line = 96;
        grouped_first.canonical_gap = Some(crate::lsp::tests::sample_canonical_gap());
        let mut grouped_second = grouped_first.clone();
        grouped_second.id = "probe:pricing:97:predicate".to_string();
        grouped_second.probe.location.line = 97;
        let mut snapshot = snapshot_with_selection(Some(applied_selection()?));
        snapshot.findings = vec![
            hidden,
            exposed,
            base_side,
            unknown,
            grouped_first,
            grouped_second,
        ];
        snapshot.diagnostic_profile = LspDiagnosticProfile::Actionable;
        let harness = handler_harness()?;
        install_snapshot(&harness, snapshot.clone())?;
        let response = call_handler(&harness)?;

        assert_eq!(response["hidden_gap_count"], 2, "{response:#}");
        let listed = response["hidden_gaps"]
            .as_array()
            .ok_or("hidden_gaps is not an array")?
            .iter()
            .map(|gap| gap["finding_id"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            listed.contains(&"probe:pricing:88:predicate".to_string()),
            "{response:#}"
        );
        assert!(
            !listed.iter().any(|id| id.ends_with(":94:predicate")),
            "a static_unknown finding is not a gap: {response:#}"
        );
        assert_eq!(
            listed
                .iter()
                .filter(|id| id.ends_with(":96:predicate") || id.ends_with(":97:predicate"))
                .count(),
            1,
            "one canonical gap is listed once: {response:#}"
        );
        let first = response["hidden_gaps"]
            .as_array()
            .and_then(|gaps| {
                gaps.iter()
                    .find(|gap| gap["finding_id"] == "probe:pricing:88:predicate")
            })
            .ok_or("probe 88 missing")?;
        assert_eq!(
            first,
            &serde_json::json!({
                "finding_id": "probe:pricing:88:predicate",
                "file": "src/pricing.rs",
                "line": 88,
                "class": "weakly_exposed",
            })
        );

        snapshot.diagnostic_profile = LspDiagnosticProfile::Full;
        let full_harness = handler_harness()?;
        install_snapshot(&full_harness, snapshot)?;
        let full = call_handler(&full_harness)?;
        assert_eq!(full["hidden_gap_count"], 0);
        assert_eq!(full["hidden_gaps"], serde_json::json!([]));
        Ok(())
    }

    /// Sort a JSON object's keys for a closed-shape comparison.
    fn sorted_keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// Pin the CURRENT interim response shape of `ripr/listActionableItems`
    /// as a closed key set — the same shape recorded in
    /// `docs/OUTPUT_SCHEMA.md`. This is the #1603 interim transform, NOT the
    /// full `ripr-agent-success.schema.json` envelope reserved for #3009;
    /// any field addition or removal is a deliberate wire change that must
    /// update the doc and this test together.
    #[test]
    fn list_actionable_items_interim_response_shape_is_closed() -> Result<(), String> {
        let harness = handler_harness()?;
        install_snapshot(
            &harness,
            snapshot_with_selection(Some(applied_selection()?)),
        )?;
        let response = call_handler(&harness)?;

        assert_eq!(
            sorted_keys(&response),
            vec![
                "allowed_edit_surface",
                "budget_identity",
                "complete_evidence_identity",
                "continuation_or_inspect_route",
                "hidden_gap_count",
                "hidden_gaps",
                "kind",
                "must_not_change",
                "omitted",
                "omitted_count",
                "omitted_truncated",
                "selected",
                "selected_count",
                "snapshot_id",
                "status",
                "total_count",
            ]
        );

        // Error fast paths carry only the closed `error` object; every
        // variant shares the same three-field error shape.
        let expected_error_keys = vec![
            "kind".to_string(),
            "message".to_string(),
            "recovery_route".to_string(),
        ];
        let cases = [(false, "no_snapshot"), (true, "analysis_in_flight")];
        for (install_empty_selection, expected_kind) in cases {
            let empty_harness = handler_harness()?;
            if install_empty_selection {
                install_snapshot(&empty_harness, snapshot_with_selection(None))?;
            }
            let response = call_handler(&empty_harness)?;
            assert_eq!(
                sorted_keys(&response),
                vec!["error".to_string()],
                "top-level shape for {expected_kind}"
            );
            assert_eq!(response["error"]["kind"], expected_kind);
            assert_eq!(
                sorted_keys(&response["error"]),
                expected_error_keys,
                "error shape for {expected_kind}"
            );
        }
        Ok(())
    }

    /// The #2034 ingress bound runs before any snapshot access or
    /// early-return fast path: an over-budget params blob is rejected with a
    /// bounded `-32602` even when a fully usable snapshot is installed.
    #[test]
    fn list_actionable_items_oversized_params_are_rejected_before_handler_work()
    -> Result<(), String> {
        let harness = handler_harness()?;
        install_snapshot(
            &harness,
            snapshot_with_selection(Some(applied_selection()?)),
        )?;
        let limit = super::super::payload_bounds::MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES;
        let oversized = serde_json::Value::String("x".repeat(limit + 1));
        let result = harness.runtime.block_on(
            harness
                .service
                .inner()
                .ripr_list_actionable_items(oversized),
        );
        let error = result
            .err()
            .ok_or("over-budget params must be rejected with an LSP error")?;
        assert_eq!(
            error.code,
            tower_lsp_server::jsonrpc::ErrorCode::InvalidParams
        );
        assert!(
            error
                .message
                .contains("ripr/listActionableItems params exceed 16384 bytes"),
            "rejection must name the bound, got: {}",
            error.message
        );
        Ok(())
    }
}
