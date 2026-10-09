//! Map shared producer-owned analysis stages onto standard LSP work-done
//! progress (#4811).
//!
//! The producer vocabulary (`crate::app::AnalysisProgressStage`) is the only
//! stage authority. This module is a pure projection: it invents no stage,
//! percentage, or counter. Terminal stages are never reported through the
//! mapping — the terminal disposition stays derived from the attempt
//! outcome (`AnalysisProgressEnd`), which carries the typed lifecycle
//! granularity (limited/deferred, deadline, supersession) the producer
//! boundary alone cannot express. Cancellation, timeout, supersession and
//! failure therefore never publish a fake completed stage.
//!
//! [`StageReportBridge`] exists because the producer sink is synchronous
//! (`AnalysisProgressSink::emit`) while the tracker is async: the blocking
//! analysis closure pushes events into the bridge, and a drain task on the
//! async side forwards them to the accepted generation's token as bounded
//! `$/progress` reports. The bridge is best-effort: a stalled drain or a
//! transport failure cannot change the analysis result.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Notify;

use super::progress::AnalysisProgressTracker;
use crate::app::{AnalysisProgressEvent, AnalysisProgressSink, AnalysisProgressStage};

/// Client-appropriate bounded wording for one shared producer stage.
///
/// Returns `None` for terminal stages: terminals are the outcome-derived
/// end's job, never a report. Every non-terminal producer stage has exactly
/// one mapping; removing or diluting this mapping breaks the cross-surface
/// parity oracle even though legacy begin/end traffic still emits text
/// (#4811 control 10).
pub(super) fn stage_report_message(stage: AnalysisProgressStage) -> Option<&'static str> {
    match stage {
        AnalysisProgressStage::LoadingInput => Some("loading input"),
        AnalysisProgressStage::Analyzing => Some("analyzing workspace"),
        AnalysisProgressStage::BuildingOutput => Some("building output"),
        AnalysisProgressStage::Completed
        | AnalysisProgressStage::Cancelled
        | AnalysisProgressStage::Failed => None,
    }
}

/// Synchronous producer-sink side of the stage bridge.
///
/// `emit` only appends to a bounded in-memory queue and wakes the drain; it
/// never blocks on transport and never touches analysis state, so progress
/// mapping failure cannot fail or strengthen the underlying analysis result
/// (#4811 mapping law).
pub(super) struct StageReportBridge {
    queue: Mutex<VecDeque<AnalysisProgressEvent>>,
    notify: Notify,
    finished: AtomicBool,
}

impl StageReportBridge {
    pub(super) fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            finished: AtomicBool::new(false),
        }
    }

    /// Pop the next queued producer event, if any.
    pub(super) fn pop(&self) -> Option<AnalysisProgressEvent> {
        match self.queue.lock() {
            Ok(mut queue) => queue.pop_front(),
            Err(poisoned) => poisoned.into_inner().pop_front(),
        }
    }

    /// True once the producer side has finished (the analysis closure has
    /// returned) and the drain may exit after one final drain pass.
    pub(super) fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    /// Stop the drain: flag first, then wake. `Notify::notify_one` stores a
    /// permit, so the drain cannot miss the wakeup even when it is between
    /// its final pop and `wait`.
    pub(super) fn finish(&self) {
        self.finished.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    /// Wait until an event may be queued or the bridge finished. A stale
    /// permit from an earlier emit/finish makes this return immediately,
    /// which the drain's pop-then-check loop tolerates.
    pub(super) async fn wait(&self) {
        self.notify.notified().await;
    }
}

impl Default for StageReportBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisProgressSink for StageReportBridge {
    fn emit(&self, event: AnalysisProgressEvent) {
        match self.queue.lock() {
            Ok(mut queue) => queue.push_back(event),
            Err(poisoned) => poisoned.into_inner().push_back(event),
        }
        self.notify.notify_one();
    }
}

/// Budget for forwarding advisory stage reports once the producer finished.
/// A stalled client may delay, never block, result handling and the next
/// refresh: the drain task and the backend's defensive final forward each
/// apply this bound, so the worst-case added delay is twice this budget.
/// Stage reports are progress-only, and the outcome-derived end (emitted
/// after this request resolves) still reports the terminal disposition on
/// the same bounded transport the legacy begin/end traffic already uses.
pub(super) const STAGE_DRAIN_BUDGET: Duration = Duration::from_secs(5);

/// Forward every queued stage report to the tracker's accepted generation,
/// waiting for in-flight forwards at most [`STAGE_DRAIN_BUDGET`]. Returns the
/// number of reports forwarded. On budget exhaustion the wait is abandoned:
/// reports are advisory, and the outcome-derived end still reports the
/// terminal disposition.
pub(super) async fn drain_stage_reports(
    bridge: &StageReportBridge,
    tracker: &AnalysisProgressTracker,
    generation: u64,
) -> usize {
    let outcome = tokio::time::timeout(STAGE_DRAIN_BUDGET, async {
        let mut forwarded = 0usize;
        loop {
            while let Some(event) = bridge.pop() {
                tracker.report_stage(generation, event.stage).await;
                forwarded += 1;
            }
            if bridge.is_finished() {
                break forwarded;
            }
            bridge.wait().await;
        }
    })
    .await;
    outcome.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::progress::{
        AnalysisProgressEnd, AnalysisProgressPhase, AnalysisProgressTracker, BoxFuture,
        ProgressNotificationValue, ProgressSink,
    };
    use super::super::refresh_scheduler::{RefreshReason, RefreshScope};
    use super::*;
    use crate::app::{AnalysisProgressScope, AnalysisProgressStage};
    use crate::cli::progress::{CliProgressSink, ProgressPolicy};
    use crate::lsp::config::LspAnalysisConfig;
    use crate::lsp::git_inputs::ResolvedGitInputs;
    use crate::lsp::input_identity::LspAnalysisInputIdentity;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use tower_lsp_server::jsonrpc::Result as LspResult;
    use tower_lsp_server::ls_types::{InitializeParams, InitializeResult};
    use tower_lsp_server::{LanguageServer, LspService};

    /// Semantic terminal both surfaces must agree on for one producer
    /// trace. `SuccessLimited` keeps typed degradation honest: a disclosed
    /// limitation reads as limited, never as plain success.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum SemanticTerminal {
        Success,
        SuccessLimited,
        Cancelled,
        Failed,
    }

    /// One cross-surface parity projection of a single normalized producer
    /// event trace (#4811): producer identity and order, the CLI semantic
    /// stage projection, the LSP semantic stage projection, the
    /// known/unknown denominator posture, the terminal disposition, the
    /// selected/total progress records, and the disclosed limitations.
    #[derive(Debug)]
    struct ProgressParityReport {
        producer_stages: Vec<AnalysisProgressStage>,
        cli_stage_tokens: Vec<&'static str>,
        lsp_stage_messages: Vec<String>,
        denominator_known: bool,
        cli_terminal: &'static str,
        lsp_terminal: SemanticTerminal,
        cli_selected_records: usize,
        lsp_selected_records: usize,
        total_producer_events: usize,
        limitations: Vec<&'static str>,
    }

    #[derive(Clone)]
    struct Buffer(Arc<StdMutex<Vec<u8>>>);

    impl Buffer {
        fn new() -> Self {
            Self(Arc::new(StdMutex::new(Vec::new())))
        }

        fn text(&self) -> String {
            let bytes = match self.0.lock() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            String::from_utf8_lossy(&bytes).into_owned()
        }
    }

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self.0.lock() {
                Ok(mut guard) => guard.extend_from_slice(buf),
                Err(poisoned) => poisoned.into_inner().extend_from_slice(buf),
            };
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct ClientOnly(tower_lsp_server::Client);

    impl LanguageServer for ClientOnly {
        async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
            Ok(InitializeResult::default())
        }

        async fn shutdown(&self) -> tower_lsp_server::jsonrpc::Result<()> {
            Ok(())
        }
    }

    fn test_client() -> tower_lsp_server::Client {
        let (service, _socket) = LspService::new(ClientOnly);
        service.inner().0.clone()
    }

    fn parity_request(generation: u64) -> super::super::refresh_scheduler::RefreshRequest {
        let root = PathBuf::from("/workspace");
        let config = LspAnalysisConfig::default();
        super::super::refresh_scheduler::RefreshRequest {
            generation,
            authority_epoch: 0,
            input_identity: LspAnalysisInputIdentity::from_refresh_inputs(
                root.clone(),
                generation,
                &config,
            ),
            git_inputs: ResolvedGitInputs::resolve(&root, config.base_ref.as_deref(), None),
            root,
            config,
            workspace_revision: generation,
            scope: RefreshScope::Interactive,
            reason: RefreshReason::DidSave,
            cancellation: crate::analysis::cancellation::AnalysisCancellationToken::new(),
        }
    }

    fn runtime() -> Result<tokio::runtime::Runtime, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))
    }

    fn event(
        stage: AnalysisProgressStage,
        scope: AnalysisProgressScope,
        completed_units: Option<u64>,
        total_units: Option<u64>,
    ) -> AnalysisProgressEvent {
        AnalysisProgressEvent {
            stage,
            scope,
            completed_units,
            total_units,
            elapsed_ms: 0,
        }
    }

    fn non_tty_policy() -> ProgressPolicy {
        ProgressPolicy {
            min_visible: Duration::ZERO,
            first_heartbeat: Duration::from_secs(30),
            heartbeat_every: Duration::from_secs(30),
            max_heartbeats: 1,
        }
    }

    fn cli_stage_token(stage: AnalysisProgressStage) -> &'static str {
        crate::cli::progress::stage_token(stage)
    }

    fn stage_is_terminal(stage: AnalysisProgressStage) -> bool {
        crate::cli::progress::stage_is_terminal(stage)
    }

    fn semantic_terminal_for_producer(
        stage: AnalysisProgressStage,
        limitation: Option<&'static str>,
    ) -> SemanticTerminal {
        match stage {
            AnalysisProgressStage::Completed => match limitation {
                Some(_) => SemanticTerminal::SuccessLimited,
                None => SemanticTerminal::Success,
            },
            AnalysisProgressStage::Cancelled => SemanticTerminal::Cancelled,
            AnalysisProgressStage::Failed => SemanticTerminal::Failed,
            _ => panic_free_unreachable_terminal(stage),
        }
    }

    fn panic_free_unreachable_terminal(stage: AnalysisProgressStage) -> SemanticTerminal {
        // Non-terminal stages never define a terminal disposition; keep the
        // mapping total without panicking.
        let _ = stage;
        SemanticTerminal::Failed
    }

    /// Project one normalized producer trace through BOTH real projections
    /// and retain the parity DTO. `lsp_mapping_removed` simulates the
    /// removal experiment (#4811 control 10): legacy begin/end traffic still
    /// emits, but the shared stage mapping is gone, so the LSP projection
    /// loses its stage evidence.
    fn project_trace(
        trace: &[AnalysisProgressEvent],
        limitation: Option<&'static str>,
        lsp_mapping_removed: bool,
    ) -> Result<ProgressParityReport, String> {
        let denominator_known = trace
            .iter()
            .any(|event| event.completed_units.is_some() && event.total_units.is_some());
        let producer_stages: Vec<AnalysisProgressStage> =
            trace.iter().map(|event| event.stage).collect();
        let Some(terminal_stage) = producer_stages.last().copied() else {
            return Err("parity trace must be non-empty".to_string());
        };
        if !stage_is_terminal(terminal_stage) {
            return Err("parity trace must end on a producer terminal stage".to_string());
        }

        // CLI projection: the real stderr sink over a buffer, non-TTY policy.
        let buffer = Buffer::new();
        let cli_sink =
            CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        for event in trace {
            cli_sink.emit(*event);
        }
        drop(cli_sink);
        let cli_text = buffer.text();
        // Parse the emitted stage lines IN ORDER: an unordered substring
        // check cannot detect a reordered or duplicated stage line, and any
        // token outside the shared producer vocabulary is a projection
        // drift, not a parse miss.
        let known_stage_tokens: Vec<(&'static str, AnalysisProgressStage)> = [
            AnalysisProgressStage::LoadingInput,
            AnalysisProgressStage::Analyzing,
            AnalysisProgressStage::BuildingOutput,
            AnalysisProgressStage::Completed,
            AnalysisProgressStage::Cancelled,
            AnalysisProgressStage::Failed,
        ]
        .into_iter()
        .map(|stage| (cli_stage_token(stage), stage))
        .collect();
        let mut cli_parsed_tokens: Vec<(&'static str, AnalysisProgressStage)> = Vec::new();
        for line in cli_text.lines() {
            let Some(rest) = line.strip_prefix("ripr progress: ") else {
                continue;
            };
            let Some(raw_token) = rest.split('[').next() else {
                continue;
            };
            let raw_token = raw_token.trim_end();
            let Some((token, stage)) = known_stage_tokens
                .iter()
                .find(|(text, _)| *text == raw_token)
                .copied()
            else {
                return Err(format!(
                    "CLI projection emitted an unknown stage token '{raw_token}': {cli_text}"
                ));
            };
            cli_parsed_tokens.push((token, stage));
        }
        let cli_stage_tokens: Vec<&'static str> = cli_parsed_tokens
            .iter()
            .filter(|(_, stage)| !stage_is_terminal(*stage))
            .map(|(token, _)| *token)
            .collect();
        let Some((_, cli_terminal_stage)) = cli_parsed_tokens.last().copied() else {
            return Err(format!("CLI projection emitted no records: {cli_text}"));
        };
        if cli_terminal_stage != terminal_stage {
            return Err(format!(
                "CLI terminal drifted from the producer trace: {cli_terminal_stage:?} vs {terminal_stage:?}"
            ));
        }

        // LSP projection: the real tracker over a recording transport.
        let tracker = Arc::new(AnalysisProgressTracker::new(test_client()));
        let recording = Arc::new(super::super::progress::RecordingSink::default());
        tracker.install_recorder(Arc::clone(&recording));
        runtime()?.block_on(async {
            let request = parity_request(1);
            tracker
                .begin(&request, AnalysisProgressPhase::Analyzing)
                .await;
            for event in trace {
                if lsp_mapping_removed {
                    // Removal experiment: the mapping is gone, so stage
                    // reports never fire; only legacy begin/end remain.
                    continue;
                }
                tracker.report_stage(1, event.stage).await;
            }
            let end = match semantic_terminal_for_producer(terminal_stage, limitation) {
                SemanticTerminal::Success => AnalysisProgressEnd::Complete,
                SemanticTerminal::SuccessLimited => AnalysisProgressEnd::Limited,
                SemanticTerminal::Cancelled => AnalysisProgressEnd::Cancelled,
                SemanticTerminal::Failed => AnalysisProgressEnd::Failed(None),
            };
            tracker.end(1, end).await;
            Ok::<(), String>(())
        })?;

        let lsp_events = recording.events();
        let lsp_stage_messages: Vec<String> = lsp_events
            .iter()
            .filter_map(|recorded| match recorded {
                super::super::progress::ProgressEvent::Report { message, .. } => {
                    Some(message.clone())
                }
                _ => None,
            })
            .collect();
        let lsp_ends: Vec<&super::super::progress::ProgressEvent> = lsp_events
            .iter()
            .filter(|recorded| {
                matches!(recorded, super::super::progress::ProgressEvent::End { .. })
            })
            .collect();
        if lsp_ends.len() != 1 {
            return Err(format!(
                "LSP projection must end exactly once: {lsp_events:?}"
            ));
        }

        let cli_selected_records = cli_text.matches("ripr progress:").count();
        Ok(ProgressParityReport {
            producer_stages,
            cli_stage_tokens,
            lsp_stage_messages,
            denominator_known,
            cli_terminal: cli_stage_token(terminal_stage),
            lsp_terminal: semantic_terminal_for_producer(terminal_stage, limitation),
            cli_selected_records,
            lsp_selected_records: lsp_events.len(),
            total_producer_events: trace.len(),
            limitations: limitation.into_iter().collect(),
        })
    }

    /// The parity oracle: stage identity and order, denominator honesty,
    /// and terminal disposition must agree across the two projections.
    fn assert_parity(report: &ProgressParityReport) -> Result<(), String> {
        let expected_lsp: Vec<String> = report
            .producer_stages
            .iter()
            .copied()
            .filter(|stage| !stage_is_terminal(*stage))
            .filter_map(|stage| stage_report_message(stage).map(str::to_string))
            .collect();
        if report.lsp_stage_messages != expected_lsp {
            return Err(format!(
                "LSP stage projection diverged from the producer trace: {:?} vs {expected_lsp:?}",
                report.lsp_stage_messages
            ));
        }
        let producer_non_terminal: Vec<&'static str> = report
            .producer_stages
            .iter()
            .copied()
            .filter(|stage| !stage_is_terminal(*stage))
            .map(cli_stage_token)
            .collect();
        if report.cli_stage_tokens != producer_non_terminal {
            return Err(format!(
                "CLI stage order diverged from the producer trace: {:?} vs {producer_non_terminal:?}",
                report.cli_stage_tokens
            ));
        }
        if report.cli_stage_tokens.len() != report.lsp_stage_messages.len() {
            return Err(format!(
                "CLI/LSP stage record counts diverged: {:?} vs {:?}",
                report.cli_stage_tokens, report.lsp_stage_messages
            ));
        }
        let non_terminal_count = report
            .producer_stages
            .iter()
            .filter(|stage| !stage_is_terminal(**stage))
            .count();
        if report.lsp_stage_messages.len() != non_terminal_count {
            return Err(format!(
                "LSP projection must carry every non-terminal producer stage: {} reports for {non_terminal_count} stages",
                report.lsp_stage_messages.len()
            ));
        }
        if report.lsp_selected_records != report.lsp_stage_messages.len().saturating_add(3) {
            return Err(format!(
                "LSP record accounting drifted: {} records for {} stage reports + create + begin + end",
                report.lsp_selected_records,
                report.lsp_stage_messages.len()
            ));
        }
        if report.cli_selected_records < report.cli_stage_tokens.len() {
            return Err(format!(
                "CLI selected fewer records than stages: {}",
                report.cli_selected_records
            ));
        }
        if report.total_producer_events != report.producer_stages.len() {
            return Err("producer event accounting drifted".to_string());
        }
        Ok(())
    }

    fn success_trace(scope: AnalysisProgressScope) -> Vec<AnalysisProgressEvent> {
        vec![
            event(AnalysisProgressStage::LoadingInput, scope, None, None),
            event(AnalysisProgressStage::Analyzing, scope, None, None),
            event(
                AnalysisProgressStage::BuildingOutput,
                scope,
                Some(3),
                Some(10),
            ),
            event(AnalysisProgressStage::Completed, scope, Some(10), Some(10)),
        ]
    }

    #[test]
    fn stage_mapping_covers_every_non_terminal_producer_stage() {
        // Control 3: every shared non-terminal stage has exactly one bounded
        // client wording; terminals are the outcome-derived end's job.
        assert_eq!(
            stage_report_message(AnalysisProgressStage::LoadingInput),
            Some("loading input")
        );
        assert_eq!(
            stage_report_message(AnalysisProgressStage::Analyzing),
            Some("analyzing workspace")
        );
        assert_eq!(
            stage_report_message(AnalysisProgressStage::BuildingOutput),
            Some("building output")
        );
        assert_eq!(stage_report_message(AnalysisProgressStage::Completed), None);
        assert_eq!(stage_report_message(AnalysisProgressStage::Cancelled), None);
        assert_eq!(stage_report_message(AnalysisProgressStage::Failed), None);
    }

    #[test]
    fn cli_and_lsp_project_one_success_trace_with_identical_stage_identity() -> Result<(), String> {
        let trace = success_trace(AnalysisProgressScope::Worktree);
        let report = project_trace(&trace, None, false)?;
        assert_parity(&report)?;
        if report.cli_terminal != "completed" {
            return Err(format!("CLI terminal drifted: {}", report.cli_terminal));
        }
        if report.lsp_terminal != SemanticTerminal::Success {
            return Err(format!("LSP terminal drifted: {:?}", report.lsp_terminal));
        }
        if report.limitations.is_empty() != (report.lsp_terminal == SemanticTerminal::Success) {
            return Err(
                "limitations must be empty exactly when the run is not limited".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn disclosed_limitation_stays_limited_on_the_lsp_surface() -> Result<(), String> {
        // Degradation honesty: a limited/deferred run reads as limited on
        // the LSP surface while the producer terminal stays `completed`;
        // the limitation is recorded, never upgraded to plain success.
        let trace = success_trace(AnalysisProgressScope::Worktree);
        let report = project_trace(&trace, Some("seams_deferred"), false)?;
        assert_parity(&report)?;
        if report.lsp_terminal != SemanticTerminal::SuccessLimited {
            return Err(format!(
                "limited run must end limited, not {:?}",
                report.lsp_terminal
            ));
        }
        if report.limitations != vec!["seams_deferred"] {
            return Err(format!("limitations drifted: {:?}", report.limitations));
        }
        Ok(())
    }

    #[test]
    fn cancelled_and_failed_traces_stay_non_successful_on_both_surfaces() -> Result<(), String> {
        let cancelled = vec![
            event(
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressScope::Diff,
                None,
                None,
            ),
            event(
                AnalysisProgressStage::Cancelled,
                AnalysisProgressScope::Diff,
                None,
                None,
            ),
        ];
        let report = project_trace(&cancelled, None, false)?;
        assert_parity(&report)?;
        if report.cli_terminal != "cancelled" || report.lsp_terminal != SemanticTerminal::Cancelled
        {
            return Err(format!("cancelled trace drifted: {report:?}"));
        }

        let failed = vec![
            event(
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressScope::Diff,
                None,
                None,
            ),
            event(
                AnalysisProgressStage::Analyzing,
                AnalysisProgressScope::Diff,
                None,
                None,
            ),
            event(
                AnalysisProgressStage::Failed,
                AnalysisProgressScope::Diff,
                None,
                None,
            ),
        ];
        let report = project_trace(&failed, None, false)?;
        assert_parity(&report)?;
        if report.cli_terminal != "failed" || report.lsp_terminal != SemanticTerminal::Failed {
            return Err(format!("failed trace drifted: {report:?}"));
        }
        Ok(())
    }

    #[test]
    fn known_and_unknown_denominators_never_become_percentages() -> Result<(), String> {
        // Control 4/5: the mixed trace carries both unknown and known
        // finite totals; neither projection may render a percentage, and
        // both must stay consistent (message-only stage records).
        let trace = success_trace(AnalysisProgressScope::Repo);
        if !trace.iter().any(|event| event.total_units.is_none()) {
            return Err("trace must include unknown totals".to_string());
        }
        let report = project_trace(&trace, None, false)?;
        assert_parity(&report)?;
        if !report.denominator_known {
            return Err("trace with 3/10 totals must record a known denominator".to_string());
        }
        for message in &report.lsp_stage_messages {
            if message.contains('%') {
                return Err(format!("LSP report fabricated a percentage: {message}"));
            }
        }
        Ok(())
    }

    #[test]
    fn removing_the_shared_mapping_breaks_parity_even_with_legacy_progress() -> Result<(), String> {
        // Control 10: with the stage mapping removed, the LSP projection
        // keeps its legacy begin/end traffic but loses every stage report,
        // and the parity oracle must reject that.
        let trace = success_trace(AnalysisProgressScope::Worktree);
        let report = project_trace(&trace, None, true)?;
        if !report.lsp_stage_messages.is_empty() {
            return Err("removal experiment must drop all stage reports".to_string());
        }
        match assert_parity(&report) {
            Ok(()) => Err(
                "parity oracle accepted a projection without the shared stage mapping".to_string(),
            ),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn bridge_forwards_events_without_blocking_the_producer() -> Result<(), String> {
        let bridge = StageReportBridge::new();
        let trace = success_trace(AnalysisProgressScope::Diff);
        for event in &trace {
            bridge.emit(*event);
        }
        bridge.finish();
        let drained: Vec<AnalysisProgressEvent> = std::iter::from_fn(|| bridge.pop()).collect();
        if drained != trace {
            return Err(format!("bridge reordered or dropped events: {drained:?}"));
        }
        if bridge.pop().is_some() {
            return Err("bridge must stay drained".to_string());
        }
        Ok(())
    }

    /// A client that accepted the token and begin, then stopped reading:
    /// every stage report send parks forever.
    struct StalledReportsSink;

    impl ProgressSink for StalledReportsSink {
        fn create(&self, _token: String) -> BoxFuture<'_, Result<(), String>> {
            Box::pin(async { Ok(()) })
        }

        fn notify(&self, _token: String, value: ProgressNotificationValue) -> BoxFuture<'_, ()> {
            Box::pin(async move {
                if matches!(value, ProgressNotificationValue::Report { .. }) {
                    std::future::pending::<()>().await;
                }
            })
        }
    }

    #[test]
    fn stalled_progress_sink_cannot_block_the_bounded_stage_drain() -> Result<(), String> {
        // The drain budget is the guarantee that advisory progress can never
        // delay result handling or the next refresh once the client stops
        // reading mid-session (#4811 review hardening).
        let bridge = StageReportBridge::new();
        let tracker = Arc::new(AnalysisProgressTracker::new(test_client()));
        tracker.install_test_sink(Arc::new(StalledReportsSink));
        let rt = runtime()?;
        rt.block_on(async {
            tracker
                .begin(&parity_request(1), AnalysisProgressPhase::Analyzing)
                .await;
        });
        for event in success_trace(AnalysisProgressScope::Worktree) {
            bridge.emit(event);
        }
        bridge.finish();
        let started = std::time::Instant::now();
        let forwarded = rt.block_on(drain_stage_reports(&bridge, &tracker, 1));
        if forwarded != 0 {
            return Err(format!(
                "stalled sink must forward no stage reports, got {forwarded}"
            ));
        }
        if started.elapsed() > STAGE_DRAIN_BUDGET + Duration::from_secs(2) {
            return Err("bounded stage drain blocked by a stalled progress sink".to_string());
        }
        Ok(())
    }
}
