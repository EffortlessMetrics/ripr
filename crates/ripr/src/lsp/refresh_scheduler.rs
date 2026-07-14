use super::config::LspAnalysisConfig;
use crate::analysis::cancellation::{AnalysisAbortKind, AnalysisCancellationToken};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshScope {
    Interactive,
    Full,
}

impl RefreshScope {
    pub(super) fn defer_seam_inventory(self) -> bool {
        matches!(self, Self::Interactive)
    }

    fn covers(self, requested: Self) -> bool {
        matches!(self, Self::Full) || self == requested
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshReason {
    DidOpen,
    DidSave,
    DidClose,
    ExplicitRefresh,
}

impl RefreshReason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::DidOpen => "did_open",
            Self::DidSave => "did_save",
            Self::DidClose => "did_close",
            Self::ExplicitRefresh => "explicit_refresh",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshAttemptOutcome {
    Published,
    Failed,
    Cancelled,
    Superseded,
    NotStarted,
}

impl RefreshAttemptOutcome {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::NotStarted => "not_started",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RefreshTelemetrySnapshot {
    pub(super) did_open_requests: u64,
    pub(super) did_save_requests: u64,
    pub(super) did_close_requests: u64,
    pub(super) explicit_refresh_requests: u64,
    pub(super) analyses_started: u64,
    pub(super) requests_coalesced: u64,
    pub(super) active_attempts_cooperatively_cancelled: u64,
    pub(super) completed_but_superseded: u64,
    pub(super) snapshots_published: u64,
    pub(super) failed_attempts: u64,
    pub(super) pending_queue_high_water: u64,
    pub(super) latest_save_to_snapshot_ms: Option<u128>,
    pub(super) last_superseded_attempt_ms: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RefreshInputIdentity {
    root: PathBuf,
    config: LspAnalysisConfig,
    workspace_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RefreshRequest {
    pub(super) generation: u64,
    pub(super) root: PathBuf,
    pub(super) config: LspAnalysisConfig,
    pub(super) workspace_revision: u64,
    pub(super) scope: RefreshScope,
    pub(super) reason: RefreshReason,
    pub(super) cancellation: AnalysisCancellationToken,
}

impl RefreshRequest {
    fn identity(&self) -> RefreshInputIdentity {
        RefreshInputIdentity {
            root: self.root.clone(),
            config: self.config.clone(),
            workspace_revision: self.workspace_revision,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum RefreshDecision {
    Start(Box<RefreshRequest>),
    Queued { generation: u64 },
    Deduplicated,
    Stopped,
}

#[derive(Default)]
struct SchedulerState {
    next_generation: u64,
    active: Option<RefreshRequest>,
    pending_latest: Option<RefreshRequest>,
    last_completed: Option<(RefreshInputIdentity, RefreshScope)>,
    stopping: bool,
    telemetry: RefreshTelemetrySnapshot,
    latest_save_requested_at: Option<Instant>,
}

pub(super) struct RefreshScheduler {
    state: Mutex<SchedulerState>,
    execution_gate: Arc<Mutex<()>>,
}

impl Default for RefreshScheduler {
    fn default() -> Self {
        Self {
            state: Mutex::new(SchedulerState::default()),
            execution_gate: Arc::new(Mutex::new(())),
        }
    }
}

impl RefreshScheduler {
    pub(super) fn request(
        &self,
        root: PathBuf,
        config: LspAnalysisConfig,
        workspace_revision: u64,
        scope: RefreshScope,
        reason: RefreshReason,
    ) -> RefreshDecision {
        let Ok(mut state) = self.state.lock() else {
            return RefreshDecision::Stopped;
        };
        if state.stopping {
            return RefreshDecision::Stopped;
        }

        match reason {
            RefreshReason::DidOpen => state.telemetry.did_open_requests += 1,
            RefreshReason::DidSave => {
                state.telemetry.did_save_requests += 1;
                state.latest_save_requested_at = Some(Instant::now());
            }
            RefreshReason::DidClose => state.telemetry.did_close_requests += 1,
            RefreshReason::ExplicitRefresh => state.telemetry.explicit_refresh_requests += 1,
        }

        let identity = RefreshInputIdentity {
            root: root.clone(),
            config: config.clone(),
            workspace_revision,
        };

        if reason != RefreshReason::ExplicitRefresh {
            if let Some(active) = state.active.as_ref()
                && active.identity() == identity
                && active.scope.covers(scope)
            {
                return RefreshDecision::Deduplicated;
            }
            if let Some(pending) = state.pending_latest.as_ref()
                && pending.identity() == identity
                && pending.scope.covers(scope)
            {
                return RefreshDecision::Deduplicated;
            }
            if state.active.is_none()
                && state
                    .last_completed
                    .as_ref()
                    .is_some_and(|(completed, completed_scope)| {
                        *completed == identity && completed_scope.covers(scope)
                    })
            {
                return RefreshDecision::Deduplicated;
            }
        }

        state.next_generation = state.next_generation.saturating_add(1);
        let request = RefreshRequest {
            generation: state.next_generation,
            root,
            config,
            workspace_revision,
            scope,
            reason,
            cancellation: AnalysisCancellationToken::new(),
        };
        if state.active.is_none() {
            state.active = Some(request.clone());
            state.telemetry.analyses_started += 1;
            RefreshDecision::Start(Box::new(request))
        } else {
            if let Some(active) = state.active.as_ref()
                && !(active.scope == RefreshScope::Full && scope == RefreshScope::Interactive)
            {
                active.cancellation.cancel(AnalysisAbortKind::Superseded);
            }
            if let Some(pending) = state.pending_latest.as_ref() {
                pending.cancellation.cancel(AnalysisAbortKind::Superseded);
            }
            let generation = request.generation;
            state.telemetry.requests_coalesced += 1;
            state.pending_latest = Some(request);
            state.telemetry.pending_queue_high_water =
                state.telemetry.pending_queue_high_water.max(1);
            RefreshDecision::Queued { generation }
        }
    }

    pub(super) fn finish(
        &self,
        request: &RefreshRequest,
        completed_authoritatively: bool,
    ) -> Option<RefreshRequest> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        let request_is_active = state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == request.generation);
        if !request_is_active {
            return None;
        }
        if state.stopping {
            request.cancellation.cancel(AnalysisAbortKind::Cancelled);
            state.active = None;
            state.pending_latest = None;
            return None;
        }
        if let Some(next) = state.pending_latest.take() {
            state.active = Some(next.clone());
            state.telemetry.analyses_started += 1;
            return Some(next);
        }
        if completed_authoritatively {
            state.last_completed = Some((request.identity(), request.scope));
        }
        state.active = None;
        None
    }

    pub(super) fn cancel(&self, request: &RefreshRequest) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let request_is_active = state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == request.generation);
        if !request_is_active {
            return false;
        }
        request.cancellation.cancel(AnalysisAbortKind::Cancelled);
        if let Some(pending) = state.pending_latest.as_ref() {
            pending.cancellation.cancel(AnalysisAbortKind::Cancelled);
        }
        state.active = None;
        state.pending_latest = None;
        true
    }

    pub(super) fn stop(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.stopping = true;
        state.next_generation = state.next_generation.saturating_add(1);
        if let Some(active) = state.active.as_ref() {
            active.cancellation.cancel(AnalysisAbortKind::Cancelled);
        }
        if let Some(pending) = state.pending_latest.as_ref() {
            pending.cancellation.cancel(AnalysisAbortKind::Cancelled);
        }
        state.pending_latest = None;
    }

    pub(super) fn is_current_generation(&self, generation: u64) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        !state.stopping && state.next_generation == generation
    }

    pub(super) fn is_idle(&self) -> bool {
        let Ok(state) = self.state.lock() else {
            return true;
        };
        state.active.is_none() && state.pending_latest.is_none()
    }

    pub(super) fn execution_gate(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.execution_gate)
    }

    pub(super) fn pending_request(&self, generation: u64) -> Option<RefreshRequest> {
        let state = self.state.lock().ok()?;
        state
            .pending_latest
            .as_ref()
            .filter(|request| request.generation == generation)
            .cloned()
    }

    pub(super) fn record_attempt_outcome(
        &self,
        outcome: RefreshAttemptOutcome,
        duration: std::time::Duration,
    ) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        match outcome {
            RefreshAttemptOutcome::Published => {
                state.telemetry.snapshots_published += 1;
                if state.latest_save_requested_at.is_some() {
                    state.telemetry.latest_save_to_snapshot_ms = state
                        .latest_save_requested_at
                        .take()
                        .map(|started| started.elapsed().as_millis());
                }
            }
            RefreshAttemptOutcome::Cancelled => {
                state.telemetry.active_attempts_cooperatively_cancelled += 1;
            }
            RefreshAttemptOutcome::Superseded => {
                state.telemetry.completed_but_superseded += 1;
                state.telemetry.last_superseded_attempt_ms = Some(duration.as_millis());
            }
            RefreshAttemptOutcome::Failed => state.telemetry.failed_attempts += 1,
            RefreshAttemptOutcome::NotStarted => {}
        }
    }

    pub(super) fn telemetry(&self) -> RefreshTelemetrySnapshot {
        match self.state.lock() {
            Ok(state) => state.telemetry,
            Err(_) => RefreshTelemetrySnapshot::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn next_generation_for_test(&self) -> Option<u64> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        state.next_generation = state.next_generation.saturating_add(1);
        Some(state.next_generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LspAnalysisConfig {
        LspAnalysisConfig::default()
    }

    fn request(
        scheduler: &RefreshScheduler,
        revision: u64,
        scope: RefreshScope,
    ) -> RefreshDecision {
        scheduler.request(
            PathBuf::from("/workspace"),
            config(),
            revision,
            scope,
            RefreshReason::DidSave,
        )
    }

    #[test]
    fn keeps_one_active_and_one_latest_pending_request() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Interactive)
        else {
            return Err("first request should start".to_string());
        };
        let active = *active;
        for revision in 2..=10 {
            if !matches!(
                request(&scheduler, revision, RefreshScope::Interactive),
                RefreshDecision::Queued { .. }
            ) {
                return Err("new input should replace the pending request".to_string());
            }
        }

        let Some(next) = scheduler.finish(&active, false) else {
            return Err("pending request should become active".to_string());
        };
        if next.workspace_revision != 10 {
            return Err("pending request should be the newest input".to_string());
        }
        if scheduler.finish(&next, true).is_some() {
            return Err("no request should remain after the pending request".to_string());
        }
        Ok(())
    }

    #[test]
    fn deduplicates_same_input_and_full_scope_covers_interactive_scope() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Full) else {
            return Err("full request should start".to_string());
        };
        let active = *active;
        if request(&scheduler, 1, RefreshScope::Interactive) != RefreshDecision::Deduplicated {
            return Err("interactive request should be covered by active full request".to_string());
        }
        if scheduler.finish(&active, true).is_some() {
            return Err("no request should remain after the active request".to_string());
        }
        if request(&scheduler, 1, RefreshScope::Interactive) != RefreshDecision::Deduplicated {
            return Err("completed full request should cover interactive retry".to_string());
        }
        Ok(())
    }

    #[test]
    fn full_request_is_not_downgraded_by_new_interactive_input() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Full) else {
            return Err("full request should start".to_string());
        };
        let active = *active;
        let RefreshDecision::Queued { .. } = request(&scheduler, 2, RefreshScope::Interactive)
        else {
            return Err("new saved input should remain pending".to_string());
        };
        if active.cancellation.checkpoint().is_err() {
            return Err("interactive input must not cancel an explicit full refresh".to_string());
        }
        let Some(next) = scheduler.finish(&active, false) else {
            return Err("pending request should become active".to_string());
        };
        if next.scope != RefreshScope::Interactive || next.workspace_revision != 2 {
            return Err("newest saved input should retain its interactive scope".to_string());
        }
        Ok(())
    }

    #[test]
    fn shutdown_invalidates_active_and_drops_pending_work() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Interactive)
        else {
            return Err("request should start".to_string());
        };
        let active = *active;
        if !matches!(
            request(&scheduler, 2, RefreshScope::Full),
            RefreshDecision::Queued { .. }
        ) {
            return Err("new request should be pending".to_string());
        }
        scheduler.stop();
        if scheduler.is_current_generation(active.generation) {
            return Err("shutdown should invalidate active work".to_string());
        }
        if scheduler.finish(&active, true).is_some() {
            return Err("shutdown should drop pending work".to_string());
        }
        if request(&scheduler, 3, RefreshScope::Interactive) != RefreshDecision::Stopped {
            return Err("shutdown should reject new work".to_string());
        }
        Ok(())
    }

    #[test]
    fn cancellation_clears_active_and_pending_work() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Interactive)
        else {
            return Err("request should start".to_string());
        };
        let active = *active;
        if !matches!(
            request(&scheduler, 2, RefreshScope::Interactive),
            RefreshDecision::Queued { .. }
        ) {
            return Err("new request should be pending".to_string());
        }
        if !scheduler.cancel(&active) || !scheduler.is_idle() {
            return Err("cancellation should clear active and pending work".to_string());
        }
        if !matches!(
            request(&scheduler, 3, RefreshScope::Interactive),
            RefreshDecision::Start(_)
        ) {
            return Err("a later request should be able to start".to_string());
        }
        if !active.cancellation.checkpoint().is_err_and(|error| {
            matches!(
                error.kind,
                AnalysisAbortKind::Cancelled | AnalysisAbortKind::Superseded
            )
        }) {
            return Err("cancellation should signal the active token".to_string());
        }
        Ok(())
    }

    #[test]
    fn cancelled_blocking_analysis_cannot_overlap_the_next_analysis() -> Result<(), String> {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let scheduler = RefreshScheduler::default();
        let gate = scheduler.execution_gate();
        let first_token = AnalysisCancellationToken::new();
        let second_token = AnalysisCancellationToken::new();
        let counters = Arc::new(Mutex::new((0usize, 0usize)));
        let (first_started_tx, first_started_rx) = mpsc::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();
        let (first_cancelled_tx, first_cancelled_rx) = mpsc::channel();
        let (second_started_tx, second_started_rx) = mpsc::channel();

        let first_counters = Arc::clone(&counters);
        let first_gate = Arc::clone(&gate);
        let first_thread_token = first_token.clone();
        let first_thread = thread::spawn(move || -> Result<(), String> {
            let _execution = first_gate
                .lock()
                .map_err(|error| format!("first execution gate poisoned: {error}"))?;
            {
                let mut counts = first_counters
                    .lock()
                    .map_err(|error| format!("first counter poisoned: {error}"))?;
                counts.0 += 1;
                counts.1 = counts.1.max(counts.0);
            }
            first_started_tx
                .send(())
                .map_err(|error| format!("first start signal failed: {error}"))?;
            first_release_rx
                .recv()
                .map_err(|error| format!("first release signal failed: {error}"))?;
            let cancelled = first_thread_token.checkpoint().is_err();
            first_cancelled_tx
                .send(cancelled)
                .map_err(|error| format!("first cancellation signal failed: {error}"))?;
            let mut counts = first_counters
                .lock()
                .map_err(|error| format!("first counter poisoned on exit: {error}"))?;
            counts.0 -= 1;
            Ok(())
        });

        first_started_rx
            .recv()
            .map_err(|error| format!("first analysis did not start: {error}"))?;
        if !first_token.cancel(AnalysisAbortKind::Superseded) {
            return Err("first analysis should accept supersession".to_string());
        }

        let second_counters = Arc::clone(&counters);
        let second_gate = Arc::clone(&gate);
        let second_thread = thread::spawn(move || -> Result<(), String> {
            let _execution = second_gate
                .lock()
                .map_err(|error| format!("second execution gate poisoned: {error}"))?;
            {
                let mut counts = second_counters
                    .lock()
                    .map_err(|error| format!("second counter poisoned: {error}"))?;
                counts.0 += 1;
                counts.1 = counts.1.max(counts.0);
            }
            second_started_tx
                .send(())
                .map_err(|error| format!("second start signal failed: {error}"))?;
            let mut counts = second_counters
                .lock()
                .map_err(|error| format!("second counter poisoned on exit: {error}"))?;
            counts.0 -= 1;
            let _ = second_token;
            Ok(())
        });

        if second_started_rx
            .recv_timeout(Duration::from_millis(50))
            .is_ok()
        {
            return Err("second analysis overlapped the cancelled analysis".to_string());
        }
        first_release_tx
            .send(())
            .map_err(|error| format!("failed to release first analysis: {error}"))?;
        if !first_cancelled_rx
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| format!("first analysis did not observe cancellation: {error}"))?
        {
            return Err("first analysis should observe supersession".to_string());
        }
        second_started_rx
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| format!("second analysis did not start after release: {error}"))?;
        first_thread
            .join()
            .map_err(|error| format!("first analysis thread panicked: {error:?}"))??;
        second_thread
            .join()
            .map_err(|error| format!("second analysis thread panicked: {error:?}"))??;

        let max_executing = counters
            .lock()
            .map_err(|error| format!("final counter poisoned: {error}"))?
            .1;
        if max_executing != 1 {
            return Err(format!(
                "expected one executing analysis, saw {max_executing}"
            ));
        }
        Ok(())
    }

    #[test]
    fn telemetry_keeps_request_and_attempt_denominators_separate() -> Result<(), String> {
        let scheduler = RefreshScheduler::default();
        let RefreshDecision::Start(active) = request(&scheduler, 1, RefreshScope::Interactive)
        else {
            return Err("first request should start".to_string());
        };
        let active = *active;
        let RefreshDecision::Queued { .. } = request(&scheduler, 2, RefreshScope::Interactive)
        else {
            return Err("second request should be coalesced".to_string());
        };
        scheduler.record_attempt_outcome(
            RefreshAttemptOutcome::Superseded,
            std::time::Duration::from_millis(7),
        );
        let Some(next) = scheduler.finish(&active, false) else {
            return Err("latest request should become active".to_string());
        };
        scheduler.record_attempt_outcome(
            RefreshAttemptOutcome::Published,
            std::time::Duration::from_millis(2),
        );
        if scheduler.finish(&next, true).is_some() {
            return Err("no request should remain after publication".to_string());
        }

        let telemetry = scheduler.telemetry();
        if telemetry.did_save_requests != 2
            || telemetry.analyses_started != 2
            || telemetry.requests_coalesced != 1
            || telemetry.completed_but_superseded != 1
            || telemetry.snapshots_published != 1
            || telemetry.pending_queue_high_water != 1
            || telemetry.latest_save_to_snapshot_ms.is_none()
            || telemetry.last_superseded_attempt_ms != Some(7)
        {
            return Err(format!("unexpected telemetry: {telemetry:?}"));
        }
        Ok(())
    }
}
