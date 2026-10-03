//! Standard LSP work-done progress for accepted analysis requests (#1971).
//!
//! Every accepted refresh generation gets one progress token
//! (`ripr-analysis-{generation}`) created via `window/workDoneProgress/create`
//! and driven by `$/progress` notifications:
//!
//! - begin (`queued` for a coalesced request, `analyzing` for an immediately
//!   started request) -> optional bounded reports at real phase boundaries
//!   (queued -> analyzing, analyzing -> publishing) -> exactly one terminal
//!   end (complete | limited | failed | cancelled | superseded | not_started).
//!
//! Rules enforced here:
//!
//! - deduplicated requests never reach this module, so they create no token;
//! - the denominator for real percentages is unknown, so no percentage is
//!   ever emitted — title/message + phase only;
//! - a terminal end is sent exactly once per token: `end*` removes the token
//!   from the registry before notifying, so every later end for the same
//!   generation is a no-op;
//! - progress is best-effort: a client error on
//!   `window/workDoneProgress/create` is logged and the token is dropped
//!   (per the spec no progress notifications may follow a failed create);
//!   analysis state is never touched;
//! - clients without `window.workDoneProgress` support see no progress
//!   traffic at all (`supported` stays false and every entry point no-ops);
//! - generic messages only: no file paths, source excerpts, or config
//!   values. Reasons, scopes, failure kinds, and run statuses are internal
//!   enum tags.
//!
//! The richer RIPR-specific `ripr/analysisStatus` notification remains the
//! authoritative status channel; the terminal end message is derived from
//! the same attempt outcome/run status so the two surfaces agree.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tower_lsp_server::Client;
use tower_lsp_server::ls_types::{
    MessageType, ProgressParams, ProgressParamsValue, ProgressToken, WorkDoneProgress,
    WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
    notification::Progress as ProgressNotification,
};

use super::progress_stages::stage_report_message;
use super::refresh_scheduler::RefreshRequest;
use super::state::AnalysisFailureKind;

use crate::app::AnalysisProgressStage;

/// Lifecycle phase of an accepted analysis request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AnalysisProgressPhase {
    /// Accepted while another analysis is active; waiting to start.
    Queued,
    /// Actively running workspace analysis.
    Analyzing,
}

impl AnalysisProgressPhase {
    fn begin_message(self, request: &RefreshRequest) -> String {
        match self {
            Self::Queued => format!(
                "queued; waiting for the active analysis ({})",
                request.reason.as_str()
            ),
            Self::Analyzing => format!("analyzing workspace ({})", request.reason.as_str()),
        }
    }
}

/// Terminal state for one progress token. Every accepted generation ends
/// with exactly one of these on every path — completion, failure,
/// cancellation, deadline expiry, supersession, root invalidation, or a
/// pre-start stop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AnalysisProgressEnd {
    /// Snapshot published with run status `full`.
    Complete,
    /// Snapshot published with a limited run status. Carries no run-status
    /// tag: the tag is machine-only vocabulary and stays on the typed
    /// `ripr/analysisStatus` payload; the client-facing end message is a
    /// pinned human phrase (see `Self::message`).
    Limited,
    /// Analysis or analysis task failed (carries the internal failure kind
    /// tag, e.g. `analysis_error`/`task_failure`; never the raw error, which
    /// may contain paths).
    Failed(Option<AnalysisFailureKind>),
    Cancelled,
    /// The physical refresh deadline (#1972) expired mid-attempt; the
    /// refresh was dropped fail-closed with no committed limited snapshot.
    DeadlineExceeded,
    Superseded,
    NotStarted,
}

impl AnalysisProgressEnd {
    fn message(&self) -> String {
        match self {
            Self::Complete => "analysis complete".to_string(),
            Self::Limited => "analysis completed with limited evidence".to_string(),
            Self::Failed(Some(kind)) => format!("analysis failed ({})", kind.as_str()),
            Self::Failed(None) => "analysis failed".to_string(),
            Self::Cancelled => "analysis cancelled".to_string(),
            Self::DeadlineExceeded => "analysis deadline exceeded".to_string(),
            Self::Superseded => "analysis superseded by a newer request".to_string(),
            Self::NotStarted => "analysis did not start".to_string(),
        }
    }
}

/// One `$/progress` notification payload.
pub(super) enum ProgressNotificationValue {
    Begin { title: String, message: String },
    Report { message: String },
    End { message: String },
}

pub(super) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Transport for progress traffic. Abstracted so unit tests can record the
/// wire sequence without a live client.
pub(super) trait ProgressSink: Send + Sync {
    fn create(&self, token: String) -> BoxFuture<'_, Result<(), String>>;
    fn notify(&self, token: String, value: ProgressNotificationValue) -> BoxFuture<'_, ()>;
}

impl ProgressSink for Client {
    fn create(&self, token: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            self.create_work_done_progress(ProgressToken::String(token))
                .await
                .map_err(|error| error.to_string())
        })
    }

    fn notify(&self, token: String, value: ProgressNotificationValue) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let value = match value {
                ProgressNotificationValue::Begin { title, message } => {
                    ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
                        title,
                        // Server-driven analysis is not cancelled through this
                        // token, so no cancel button is advertised.
                        cancellable: Some(false),
                        message: Some(message),
                        // No fabricated percentages: the denominator is
                        // unknown, so progress stays unbounded.
                        percentage: None,
                    }))
                }
                ProgressNotificationValue::Report { message } => ProgressParamsValue::WorkDone(
                    WorkDoneProgress::Report(WorkDoneProgressReport {
                        message: Some(message),
                        ..WorkDoneProgressReport::default()
                    }),
                ),
                ProgressNotificationValue::End { message } => {
                    ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                        message: Some(message),
                    }))
                }
            };
            self.send_notification::<ProgressNotification>(ProgressParams {
                token: ProgressToken::String(token),
                value,
            })
            .await;
        })
    }
}

struct TokenRecord {
    token: String,
    phase: AnalysisProgressPhase,
    /// True once the `$/progress` begin notification has actually been sent.
    /// A token ended while `started == false` is removed silently: per the LSP
    /// spec no progress notifications may precede a begin, and none may follow
    /// a failed create.
    started: bool,
    /// Last producer stage forwarded as a report on this token (#4811).
    /// Consecutive duplicate stage events are suppressed so repeated
    /// equivalent events stay within the declared record ceiling.
    last_stage: Option<AnalysisProgressStage>,
}

fn progress_token(generation: u64) -> String {
    format!("ripr-analysis-{generation}")
}

/// Owns the progress-token registry for accepted analysis generations.
///
/// Exactly-once terminal semantics come from the registry: a token is only
/// ended after it has been removed, so concurrent or repeated terminal
/// paths (outcome end, cancellation guard, invalidation drain, shutdown
/// drain) cannot emit two ends for one token.
pub(super) struct AnalysisProgressTracker {
    client: Client,
    sink: Mutex<Arc<dyn ProgressSink>>,
    supported: AtomicBool,
    tokens: Mutex<BTreeMap<u64, TokenRecord>>,
}

impl AnalysisProgressTracker {
    pub(super) fn new(client: Client) -> Self {
        let sink: Arc<dyn ProgressSink> = Arc::new(client.clone());
        Self {
            client,
            sink: Mutex::new(sink),
            supported: AtomicBool::new(false),
            tokens: Mutex::new(BTreeMap::new()),
        }
    }

    pub(super) fn set_supported(&self, supported: bool) {
        self.supported.store(supported, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn is_supported(&self) -> bool {
        self.supported.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    fn set_sink(&self, sink: Arc<dyn ProgressSink>) {
        if let Ok(mut current) = self.sink.lock() {
            *current = sink;
        }
    }

    #[cfg(test)]
    pub(super) fn install_recorder(&self, sink: Arc<RecordingSink>) {
        let recorder: Arc<dyn ProgressSink> = sink;
        self.set_sink(recorder);
        self.set_supported(true);
    }

    /// Install an arbitrary test transport (for example a sink whose sends
    /// never resolve) so bounded-drain guarantees can be exercised without a
    /// live client.
    #[cfg(test)]
    pub(super) fn install_test_sink(&self, sink: Arc<dyn ProgressSink>) {
        self.set_sink(sink);
        self.set_supported(true);
    }

    fn sink(&self) -> Option<Arc<dyn ProgressSink>> {
        self.sink.lock().ok().map(|sink| Arc::clone(&sink))
    }

    /// Begin progress for an accepted generation. A create failure is
    /// logged and the token is dropped: per the spec no progress
    /// notifications may follow a failed create, and analysis state is
    /// unaffected (best-effort).
    pub(super) async fn begin(&self, request: &RefreshRequest, phase: AnalysisProgressPhase) {
        if !self.supported.load(Ordering::SeqCst) {
            return;
        }
        {
            // Register BEFORE the sink awaits so a concurrent end() always
            // finds the generation and terminates it cleanly instead of
            // leaking a begin without an end.
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            if tokens.contains_key(&request.generation) {
                return;
            }
            tokens.insert(
                request.generation,
                TokenRecord {
                    token: progress_token(request.generation),
                    phase,
                    started: false,
                    last_stage: None,
                },
            );
        }
        let Some(sink) = self.sink() else {
            return;
        };
        let token = progress_token(request.generation);
        if let Err(error) = sink.create(token.clone()).await {
            if let Ok(mut tokens) = self.tokens.lock() {
                tokens.remove(&request.generation);
            }
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "ripr work-done progress unavailable for generation {}: {error}",
                        request.generation
                    ),
                )
                .await;
            return;
        }
        sink.notify(
            token.clone(),
            ProgressNotificationValue::Begin {
                title: "ripr analysis".to_string(),
                message: phase.begin_message(request),
            },
        )
        .await;
        if let Ok(mut tokens) = self.tokens.lock()
            && let Some(record) = tokens.get_mut(&request.generation)
        {
            record.started = true;
        }
    }

    /// Move a queued token to `analyzing` on the SAME token when the queued
    /// request starts. No-op for tokens already analyzing or unknown.
    pub(super) async fn transition_to_analyzing(&self, generation: u64) {
        let token = {
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            let Some(record) = tokens.get_mut(&generation) else {
                return;
            };
            if !record.started || record.phase != AnalysisProgressPhase::Queued {
                return;
            }
            record.phase = AnalysisProgressPhase::Analyzing;
            record.token.clone()
        };
        let Some(sink) = self.sink() else {
            return;
        };
        sink.notify(
            token,
            ProgressNotificationValue::Report {
                message: "analyzing workspace".to_string(),
            },
        )
        .await;
    }

    /// Bounded report at the real phase boundary where analysis results are
    /// about to be published.
    pub(super) async fn report_publishing(&self, generation: u64) {
        let token = {
            let Ok(tokens) = self.tokens.lock() else {
                return;
            };
            let Some(record) = tokens.get(&generation) else {
                return;
            };
            if !record.started || record.phase != AnalysisProgressPhase::Analyzing {
                return;
            }
            record.token.clone()
        };
        let Some(sink) = self.sink() else {
            return;
        };
        sink.notify(
            token,
            ProgressNotificationValue::Report {
                message: "publishing diagnostics".to_string(),
            },
        )
        .await;
    }

    /// Bounded report carrying one shared producer stage identity (#4811).
    ///
    /// This is the standard work-done consumption of the same
    /// `AnalysisProgressStage` vocabulary the CLI projects onto stderr:
    /// `loading_input`, `analyzing`, and `building_output` each become one
    /// client-appropriate bounded report on the accepted generation's token.
    /// Terminal stages are never reported here — the terminal disposition
    /// stays derived from the attempt outcome so cancellation, timeout,
    /// supersession, failure, and limited/deferred states keep their typed
    /// honesty and can never publish a fake completed stage.
    ///
    /// Reports are message-only: no percentage is ever attached, so unknown
    /// totals stay unknown. No-ops (never an error, never analysis-relevant):
    /// unsupported clients, unknown generations, unstarted tokens, tokens
    /// still queued, terminal stages, and consecutive duplicate stages.
    pub(super) async fn report_stage(&self, generation: u64, stage: AnalysisProgressStage) {
        if !self.supported.load(Ordering::SeqCst) {
            return;
        }
        let Some(message) = stage_report_message(stage) else {
            return;
        };
        let token = {
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            let Some(record) = tokens.get_mut(&generation) else {
                return;
            };
            if !record.started || record.phase != AnalysisProgressPhase::Analyzing {
                return;
            }
            if record.last_stage == Some(stage) {
                return;
            }
            record.last_stage = Some(stage);
            record.token.clone()
        };
        let Some(sink) = self.sink() else {
            return;
        };
        sink.notify(
            token,
            ProgressNotificationValue::Report {
                message: message.to_string(),
            },
        )
        .await;
    }

    /// Terminate one generation's token. Exactly-once: the token is removed
    /// before the end notification, so later ends for the same generation
    /// are no-ops.
    pub(super) async fn end(&self, generation: u64, end: AnalysisProgressEnd) {
        let record = {
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            tokens.remove(&generation)
        };
        let Some(record) = record else {
            return;
        };
        if !record.started {
            // Removed cleanly without a notification: no begin was ever sent.
            return;
        }
        let token = record.token;
        let Some(sink) = self.sink() else {
            return;
        };
        sink.notify(
            token,
            ProgressNotificationValue::End {
                message: end.message(),
            },
        )
        .await;
    }

    /// Terminate every token still in `queued` phase. Used by root/config
    /// invalidation and cancellation paths where a queued request is dropped
    /// without ever starting; the active (analyzing) token is left for the
    /// refresh loop's outcome-based end.
    pub(super) async fn end_queued(&self, end: AnalysisProgressEnd) {
        let drained = {
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            let queued: Vec<u64> = tokens
                .iter()
                .filter(|(_, record)| record.phase == AnalysisProgressPhase::Queued)
                .map(|(generation, _)| *generation)
                .collect();
            queued
                .into_iter()
                .filter_map(|generation| tokens.remove(&generation))
                .filter(|record| record.started)
                .map(|record| record.token)
                .collect::<Vec<_>>()
        };
        let Some(sink) = self.sink() else {
            return;
        };
        for token in drained {
            sink.notify(
                token,
                ProgressNotificationValue::End {
                    message: end.message(),
                },
            )
            .await;
        }
    }

    /// Terminate every open token. Used on shutdown when the scheduler
    /// stops accepting and running work.
    pub(super) async fn end_all(&self, end: AnalysisProgressEnd) {
        let drained = {
            let Ok(mut tokens) = self.tokens.lock() else {
                return;
            };
            std::mem::take(&mut *tokens)
                .into_values()
                .filter(|record| record.started)
                .map(|record| record.token)
                .collect::<Vec<_>>()
        };
        let Some(sink) = self.sink() else {
            return;
        };
        for token in drained {
            sink.notify(
                token,
                ProgressNotificationValue::End {
                    message: end.message(),
                },
            )
            .await;
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ProgressEvent {
    Create {
        token: String,
    },
    Begin {
        token: String,
        title: String,
        message: String,
    },
    Report {
        token: String,
        message: String,
    },
    End {
        token: String,
        message: String,
    },
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct RecordingSink {
    events: Mutex<Vec<ProgressEvent>>,
    fail_create: AtomicBool,
}

#[cfg(test)]
impl RecordingSink {
    pub(super) fn events(&self) -> Vec<ProgressEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    pub(super) fn set_fail_create(&self, fail: bool) {
        self.fail_create.store(fail, Ordering::SeqCst);
    }

    fn push(&self, event: ProgressEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

#[cfg(test)]
impl ProgressSink for RecordingSink {
    fn create(&self, token: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            self.push(ProgressEvent::Create { token });
            if self.fail_create.load(Ordering::SeqCst) {
                return Err("client rejected work-done progress create".to_string());
            }
            Ok(())
        })
    }

    fn notify(&self, token: String, value: ProgressNotificationValue) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            match value {
                ProgressNotificationValue::Begin { title, message } => {
                    self.push(ProgressEvent::Begin {
                        token,
                        title,
                        message,
                    });
                }
                ProgressNotificationValue::Report { message } => {
                    self.push(ProgressEvent::Report { token, message });
                }
                ProgressNotificationValue::End { message } => {
                    self.push(ProgressEvent::End { token, message });
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::LspAnalysisConfig;
    use super::super::input_identity::LspAnalysisInputIdentity;
    use super::super::refresh_scheduler::{RefreshReason, RefreshScope};
    use super::*;
    use crate::analysis::cancellation::AnalysisCancellationToken;
    use std::path::PathBuf;
    use tower_lsp_server::jsonrpc::Result as LspResult;
    use tower_lsp_server::ls_types::{InitializeParams, InitializeResult};
    use tower_lsp_server::{LanguageServer, LspService};

    struct ClientOnly(Client);

    impl LanguageServer for ClientOnly {
        async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
            Ok(InitializeResult::default())
        }

        async fn shutdown(&self) -> LspResult<()> {
            Ok(())
        }
    }

    fn test_client() -> Client {
        let (service, _socket) = LspService::new(ClientOnly);
        service.inner().0.clone()
    }

    fn tracker_with_recorder() -> (Arc<AnalysisProgressTracker>, Arc<RecordingSink>) {
        let tracker = Arc::new(AnalysisProgressTracker::new(test_client()));
        let sink = Arc::new(RecordingSink::default());
        tracker.install_recorder(Arc::clone(&sink));
        (tracker, sink)
    }

    fn test_request(generation: u64) -> RefreshRequest {
        let root = PathBuf::from("/workspace");
        let config = LspAnalysisConfig::default();
        RefreshRequest {
            generation,
            authority_epoch: 0,
            input_identity: LspAnalysisInputIdentity::from_refresh_inputs(
                root.clone(),
                generation,
                &config,
            ),
            git_inputs: crate::lsp::git_inputs::ResolvedGitInputs::resolve(
                &root,
                config.base_ref.as_deref(),
                None,
            ),
            root,
            config,
            workspace_revision: generation,
            scope: RefreshScope::Interactive,
            reason: RefreshReason::DidSave,
            cancellation: AnalysisCancellationToken::new(),
        }
    }

    fn runtime() -> Result<tokio::runtime::Runtime, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))
    }

    #[test]
    fn success_run_begins_analyzing_and_ends_complete_exactly_once() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            tracker.report_publishing(1).await;
            tracker.end(1, AnalysisProgressEnd::Complete).await;
            tracker.end(1, AnalysisProgressEnd::Complete).await;

            let events = sink.events();
            let expected = vec![
                ProgressEvent::Create {
                    token: "ripr-analysis-1".to_string(),
                },
                ProgressEvent::Begin {
                    token: "ripr-analysis-1".to_string(),
                    title: "ripr analysis".to_string(),
                    message: "analyzing workspace (did_save)".to_string(),
                },
                ProgressEvent::Report {
                    token: "ripr-analysis-1".to_string(),
                    message: "publishing diagnostics".to_string(),
                },
                ProgressEvent::End {
                    token: "ripr-analysis-1".to_string(),
                    message: "analysis complete".to_string(),
                },
            ];
            if events != expected {
                return Err(format!("unexpected event sequence: {events:?}"));
            }
            Ok(())
        })
    }

    #[test]
    fn queued_request_transitions_to_analyzing_on_the_same_token() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            let request = test_request(2);
            tracker.begin(&request, AnalysisProgressPhase::Queued).await;
            tracker.transition_to_analyzing(2).await;
            tracker.end(2, AnalysisProgressEnd::Limited).await;

            let events = sink.events();
            let token = "ripr-analysis-2".to_string();
            let expected = vec![
                ProgressEvent::Create {
                    token: token.clone(),
                },
                ProgressEvent::Begin {
                    token: token.clone(),
                    title: "ripr analysis".to_string(),
                    message: "queued; waiting for the active analysis (did_save)".to_string(),
                },
                ProgressEvent::Report {
                    token: token.clone(),
                    message: "analyzing workspace".to_string(),
                },
                ProgressEvent::End {
                    token: token.clone(),
                    message: "analysis completed with limited evidence".to_string(),
                },
            ];
            if events != expected {
                return Err(format!("queued lifecycle drifted: {events:?}"));
            }
            Ok(())
        })
    }

    #[test]
    fn capability_absent_client_sees_no_progress_traffic() -> Result<(), String> {
        runtime()?.block_on(async {
            let (service, _socket) = LspService::new(ClientOnly);
            let tracker = AnalysisProgressTracker::new(service.inner().0.clone());
            let sink = Arc::new(RecordingSink::default());
            tracker.set_sink(sink.clone());
            // `supported` stays false: the client never advertised
            // window.workDoneProgress.
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            tracker.transition_to_analyzing(1).await;
            tracker.report_publishing(1).await;
            tracker.end(1, AnalysisProgressEnd::Complete).await;
            tracker.end_queued(AnalysisProgressEnd::Cancelled).await;
            tracker.end_all(AnalysisProgressEnd::Cancelled).await;

            if !sink.events().is_empty() {
                return Err(format!(
                    "capability-absent client received progress traffic: {:?}",
                    sink.events()
                ));
            }
            if tracker.is_supported() {
                return Err("tracker must stay unsupported without the client capability".into());
            }
            Ok(())
        })
    }

    #[test]
    fn end_before_begin_notification_removes_token_silently() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            // Simulate the mid-create window: a registered but unstarted token.
            {
                let Ok(mut tokens) = tracker.tokens.lock() else {
                    return Err("tracker lock poisoned".to_string());
                };
                tokens.insert(
                    7,
                    TokenRecord {
                        token: progress_token(7),
                        phase: AnalysisProgressPhase::Queued,
                        started: false,
                        last_stage: None,
                    },
                );
            }
            tracker.end(7, AnalysisProgressEnd::Cancelled).await;
            if !sink.events().is_empty() {
                return Err(format!(
                    "an unstarted token emitted progress traffic: {:?}",
                    sink.events()
                ));
            }
            let Ok(tokens) = tracker.tokens.lock() else {
                return Err("tracker lock poisoned".to_string());
            };
            if tokens.contains_key(&7) {
                return Err("unstarted token was not removed by end".to_string());
            }
            Ok(())
        })
    }

    #[test]
    fn create_failure_drops_placeholder_and_allows_retry() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            sink.set_fail_create(true);
            let request = test_request(9);
            tracker.begin(&request, AnalysisProgressPhase::Analyzing).await;
            {
                let Ok(tokens) = tracker.tokens.lock() else {
                    return Err("tracker lock poisoned".to_string());
                };
                if tokens.contains_key(&9) {
                    return Err(
                        "failed create left a placeholder token that would leak or block retry"
                            .to_string(),
                    );
                }
            }
            sink.set_fail_create(false);
            tracker.begin(&request, AnalysisProgressPhase::Analyzing).await;
            let events = sink.events();
            let begins = events
                .iter()
                .filter(|event| matches!(event, ProgressEvent::Begin { token, .. } if token == &progress_token(9)))
                .count();
            if begins != 1 {
                return Err(format!(
                    "retry after create failure did not begin exactly once: {events:?}"
                ));
            }
            Ok(())
        })
    }

    #[test]
    fn create_failure_is_best_effort_and_never_sends_notifications() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            sink.set_fail_create(true);
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            // The failed create drops the token: every later lifecycle call
            // for the generation is a no-op and cannot corrupt analysis.
            tracker.transition_to_analyzing(1).await;
            tracker.report_publishing(1).await;
            tracker.end(1, AnalysisProgressEnd::Complete).await;

            let events = sink.events();
            if events
                != vec![ProgressEvent::Create {
                    token: "ripr-analysis-1".to_string(),
                }]
            {
                return Err(format!(
                    "failed create must not emit progress notifications: {events:?}"
                ));
            }

            // A later generation recovers once the client accepts creates.
            sink.set_fail_create(false);
            let request = test_request(2);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            tracker.end(2, AnalysisProgressEnd::Complete).await;
            let events = sink.events();
            if !events
                .iter()
                .any(|event| matches!(event, ProgressEvent::Begin { token, .. } if token == "ripr-analysis-2"))
            {
                return Err(format!("recovered create did not begin: {events:?}"));
            }
            Ok(())
        })
    }

    #[test]
    fn end_queued_only_ends_queued_tokens_and_end_all_ends_everything() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            tracker
                .begin(&test_request(1), AnalysisProgressPhase::Analyzing)
                .await;
            tracker
                .begin(&test_request(2), AnalysisProgressPhase::Queued)
                .await;

            tracker.end_queued(AnalysisProgressEnd::Superseded).await;
            let events = sink.events();
            let ends: Vec<&ProgressEvent> = events
                .iter()
                .filter(|event| matches!(event, ProgressEvent::End { .. }))
                .collect();
            if ends.len() != 1 {
                return Err(format!(
                    "end_queued must end exactly the queued token: {events:?}"
                ));
            }
            if !matches!(
                ends.first(),
                Some(ProgressEvent::End { token, message })
                    if token == "ripr-analysis-2" && message.contains("superseded")
            ) {
                return Err(format!("wrong queued end: {events:?}"));
            }

            tracker.end_all(AnalysisProgressEnd::Cancelled).await;
            let events = sink.events();
            let end_count = events
                .iter()
                .filter(|event| matches!(event, ProgressEvent::End { .. }))
                .count();
            if end_count != 2 {
                return Err(format!("end_all must end the remaining token: {events:?}"));
            }
            // Everything is drained: further drains emit nothing.
            tracker.end_all(AnalysisProgressEnd::Cancelled).await;
            if sink.events().len() != events.len() {
                return Err("drained registry must stay drained".to_string());
            }
            Ok(())
        })
    }

    #[test]
    fn terminal_messages_carry_no_paths_or_source_excerpts() -> Result<(), String> {
        let ends = vec![
            AnalysisProgressEnd::Complete,
            AnalysisProgressEnd::Limited,
            AnalysisProgressEnd::Failed(Some(AnalysisFailureKind::TaskFailure)),
            AnalysisProgressEnd::Failed(None),
            AnalysisProgressEnd::Cancelled,
            AnalysisProgressEnd::Superseded,
            AnalysisProgressEnd::NotStarted,
        ];
        for end in ends {
            let message = end.message();
            if message.contains('/') || message.contains('\\') || message.contains(".rs") {
                return Err(format!("terminal message leaks path content: {message}"));
            }
        }
        Ok(())
    }

    #[test]
    fn limited_end_message_is_pinned_human_phrase_without_run_status_tag() -> Result<(), String> {
        // #5003: the raw run-status tag (`seams_deferred`, `cache_limited`,
        // ...) is machine-only vocabulary. The terminal work-done end message
        // must stay a human phrase; the tag remains available on the typed
        // `ripr/analysisStatus` payload.
        let message = AnalysisProgressEnd::Limited.message();
        if message != "analysis completed with limited evidence" {
            return Err(format!("limited end message drifted: {message:?}"));
        }
        let leaked_tags = [
            "run status",
            "seams_deferred",
            "cache_limited",
            "limited_partial_scope",
        ];
        for leaked in leaked_tags {
            if message.contains(leaked) {
                return Err(format!(
                    "limited end message leaks the internal run-status tag {leaked:?}: {message:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn reports_and_ends_for_unknown_generations_are_no_ops() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            tracker.transition_to_analyzing(99).await;
            tracker.report_publishing(99).await;
            tracker.end(99, AnalysisProgressEnd::Complete).await;
            if !sink.events().is_empty() {
                return Err(format!(
                    "unknown generations must not emit traffic: {:?}",
                    sink.events()
                ));
            }
            Ok(())
        })
    }

    #[test]
    fn report_stage_emits_one_bounded_report_per_stage_in_producer_order() -> Result<(), String> {
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            for stage in [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::BuildingOutput,
            ] {
                tracker.report_stage(1, stage).await;
            }
            tracker.end(1, AnalysisProgressEnd::Complete).await;

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
                ProgressEvent::Report {
                    token: token.clone(),
                    message: "loading input".to_string(),
                },
                ProgressEvent::Report {
                    token: token.clone(),
                    message: "analyzing workspace".to_string(),
                },
                ProgressEvent::Report {
                    token: token.clone(),
                    message: "building output".to_string(),
                },
                ProgressEvent::End {
                    token: token.clone(),
                    message: "analysis complete".to_string(),
                },
            ];
            if events != expected {
                return Err(format!("stage-mapped reports drifted: {events:?}"));
            }
            Ok(())
        })
    }

    #[test]
    fn report_stage_suppresses_consecutive_duplicate_stages() -> Result<(), String> {
        // Control 7: repeated equivalent stage events stay within the
        // declared record ceiling — one report per distinct stage.
        runtime()?.block_on(async {
            let (tracker, sink) = tracker_with_recorder();
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            for stage in [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::Analyzing,
            ] {
                tracker.report_stage(1, stage).await;
            }
            tracker.end(1, AnalysisProgressEnd::Complete).await;

            let reports = sink
                .events()
                .iter()
                .filter(|event| matches!(event, ProgressEvent::Report { .. }))
                .count();
            if reports != 2 {
                return Err(format!(
                    "duplicate stage events must collapse to one report each: {:?}",
                    sink.events()
                ));
            }
            Ok(())
        })
    }

    #[test]
    fn report_stage_is_silent_for_terminals_queued_unstarted_and_unknown() -> Result<(), String> {
        runtime()?.block_on(async {
            // Terminal stages never become reports: the outcome-derived end
            // owns the terminal disposition.
            let (tracker, sink) = tracker_with_recorder();
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            for stage in [
                AnalysisProgressStage::Completed,
                AnalysisProgressStage::Cancelled,
                AnalysisProgressStage::Failed,
            ] {
                tracker.report_stage(1, stage).await;
            }
            let mut reports = sink
                .events()
                .iter()
                .filter(|event| matches!(event, ProgressEvent::Report { .. }))
                .count();
            if reports != 0 {
                return Err(format!(
                    "terminal stages must never be reported: {:?}",
                    sink.events()
                ));
            }

            // Unknown generation is a no-op.
            tracker
                .report_stage(99, AnalysisProgressStage::LoadingInput)
                .await;

            // A queued (started) token must not carry stage reports: only
            // the active analyzing attempt may.
            let queued = test_request(2);
            tracker.begin(&queued, AnalysisProgressPhase::Queued).await;
            tracker
                .report_stage(2, AnalysisProgressStage::LoadingInput)
                .await;

            reports = sink
                .events()
                .iter()
                .filter(|event| matches!(event, ProgressEvent::Report { .. }))
                .count();
            if reports != 0 {
                return Err(format!(
                    "queued/unknown generations must not receive stage reports: {:?}",
                    sink.events()
                ));
            }
            Ok(())
        })
    }

    #[test]
    fn report_stage_is_silent_without_client_capability() -> Result<(), String> {
        runtime()?.block_on(async {
            let (service, _socket) = LspService::new(ClientOnly);
            let tracker = AnalysisProgressTracker::new(service.inner().0.clone());
            let sink = Arc::new(RecordingSink::default());
            tracker.set_sink(sink.clone());
            // `supported` stays false: no stage traffic may reach the client.
            let request = test_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            tracker
                .report_stage(1, AnalysisProgressStage::LoadingInput)
                .await;
            tracker
                .report_stage(1, AnalysisProgressStage::BuildingOutput)
                .await;
            if !sink.events().is_empty() {
                return Err(format!(
                    "capability-absent client received stage reports: {:?}",
                    sink.events()
                ));
            }
            Ok(())
        })
    }
}
