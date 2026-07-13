use super::config::LspAnalysisConfig;
use std::path::PathBuf;
use std::sync::Mutex;

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshReason {
    DidOpen,
    DidSave,
    DidClose,
    ExplicitRefresh,
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
}

#[derive(Default)]
pub(super) struct RefreshScheduler {
    state: Mutex<SchedulerState>,
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
        };
        if state.active.is_none() {
            state.active = Some(request.clone());
            RefreshDecision::Start(Box::new(request))
        } else {
            let generation = request.generation;
            state.pending_latest = Some(request);
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
            state.active = None;
            state.pending_latest = None;
            return None;
        }
        if let Some(next) = state.pending_latest.take() {
            state.active = Some(next.clone());
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
        Ok(())
    }
}
