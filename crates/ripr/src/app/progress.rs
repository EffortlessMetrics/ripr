//! Passive progress events for the shared check application pipeline.
//!
//! The producer reports only boundaries it has reached. Elapsed time is
//! observational and never becomes part of analysis identity or output.
//! Unknown work totals stay unknown: this layer does not invent percentages.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use crate::analysis::cancellation::AnalysisAbortKind;

/// Closed stage vocabulary for one check invocation.
///
/// Substages such as indexing or classification are omitted until those
/// producers own trustworthy counters. CLI stderr and LSP work-done mapping
/// are later projections of these same identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisProgressStage {
    LoadingInput,
    Analyzing,
    BuildingOutput,
    Completed,
    Cancelled,
    Failed,
}

/// Mode identity without a checkout path or source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisProgressScope {
    Diff,
    Worktree,
    Repo,
}

/// One bounded, path-free progress observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AnalysisProgressEvent {
    pub(crate) stage: AnalysisProgressStage,
    pub(crate) scope: AnalysisProgressScope,
    pub(crate) completed_units: Option<u64>,
    pub(crate) total_units: Option<u64>,
    pub(crate) elapsed_ms: u64,
}

impl AnalysisProgressEvent {
    fn boundary(
        stage: AnalysisProgressStage,
        scope: AnalysisProgressScope,
        elapsed_ms: u64,
    ) -> Self {
        Self {
            stage,
            scope,
            completed_units: None,
            total_units: None,
            elapsed_ms,
        }
    }

    #[cfg(test)]
    fn is_terminal(self) -> bool {
        matches!(
            self.stage,
            AnalysisProgressStage::Completed
                | AnalysisProgressStage::Cancelled
                | AnalysisProgressStage::Failed
        )
    }
}

/// Optional, best-effort observer. Implementations must return quickly.
/// A sink failure, including a panic, cannot change the check result.
pub(crate) trait AnalysisProgressSink {
    fn emit(&self, event: AnalysisProgressEvent);
}

pub(crate) struct ProgressRun<'a> {
    sink: Option<&'a dyn AnalysisProgressSink>,
    scope: AnalysisProgressScope,
    started: Instant,
    terminal: bool,
}

impl<'a> ProgressRun<'a> {
    pub(crate) fn new(
        sink: Option<&'a dyn AnalysisProgressSink>,
        scope: AnalysisProgressScope,
    ) -> Self {
        Self {
            sink,
            scope,
            started: Instant::now(),
            terminal: false,
        }
    }

    pub(crate) fn emit(&mut self, stage: AnalysisProgressStage) {
        if self.terminal {
            return;
        }
        self.emit_now(stage);
    }

    pub(crate) fn complete(&mut self) {
        self.finish(AnalysisProgressStage::Completed);
    }

    fn finish(&mut self, stage: AnalysisProgressStage) {
        if self.terminal {
            return;
        }
        self.terminal = true;
        self.emit_now(stage);
    }

    fn emit_now(&self, stage: AnalysisProgressStage) {
        let Some(sink) = self.sink else {
            return;
        };
        let event = AnalysisProgressEvent::boundary(stage, self.scope, elapsed_ms(self.started));
        let _ = catch_unwind(AssertUnwindSafe(|| sink.emit(event)));
    }
}

impl Drop for ProgressRun<'_> {
    fn drop(&mut self) {
        let stage = match crate::analysis::cancellation::current_abort_kind() {
            Some(
                AnalysisAbortKind::Cancelled
                | AnalysisAbortKind::Superseded
                | AnalysisAbortKind::DeadlineExceeded,
            ) => AnalysisProgressStage::Cancelled,
            None => AnalysisProgressStage::Failed,
        };
        self.finish(stage);
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Bracket one full-repo inventory walk and its artifact assembly with
/// repo-scope progress boundaries (#4945).
///
/// The repo-scoped formats drive the seam walkers from the render layer, so
/// this is the repo-scope analog of [`crate::app::check_with_progress`]: the same closed
/// stage vocabulary at the same [`AnalysisProgressScope::Repo`] scope, emitted
/// around the walk the caller owns. `LoadingInput` opens the run at the
/// diff-path's entry boundary and is followed immediately by `Analyzing`:
/// the inventory closures own corpus discovery and the walk/classification
/// compute together, and no separate discovery/compute boundary exists
/// without re-plumbing the inventory signatures, so `Analyzing` is the one
/// honest coarse stage for the whole walk (heartbeats report under it).
/// `BuildingOutput` covers the assembly callback and `Completed` the producer
/// boundary. The CLI projection holds `Completed` until the command has
/// committed stdout. An early return through either closure drops the run,
/// which fail-closes as `failed` (or `cancelled`) exactly like the
/// diff-scoped path.
pub(crate) fn repo_inventory_with_progress<T, R>(
    sink: Option<&dyn AnalysisProgressSink>,
    inventory: impl FnOnce() -> Result<T, String>,
    assemble: impl FnOnce(T) -> Result<R, String>,
) -> Result<R, String> {
    let mut run = ProgressRun::new(sink, AnalysisProgressScope::Repo);
    run.emit(AnalysisProgressStage::LoadingInput);
    run.emit(AnalysisProgressStage::Analyzing);
    let value = inventory()?;
    run.emit(AnalysisProgressStage::BuildingOutput);
    let assembled = assemble(value)?;
    run.complete();
    Ok(assembled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cancellation::{self, AnalysisAbortKind, AnalysisCancellationToken};
    use crate::app::check::check_with_progress;
    use crate::app::{CheckInput, Mode, OutputFormat};
    use crate::config::RiprConfig;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;

    struct ProgressRecorder {
        events: Mutex<Vec<AnalysisProgressEvent>>,
    }

    impl ProgressRecorder {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<AnalysisProgressEvent> {
            match self.events.lock() {
                Ok(events) => events.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    impl AnalysisProgressSink for ProgressRecorder {
        fn emit(&self, event: AnalysisProgressEvent) {
            match self.events.lock() {
                Ok(mut events) => events.push(event),
                Err(poisoned) => poisoned.into_inner().push(event),
            }
        }
    }

    fn sample_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample")
    }

    fn sample_diff_input() -> CheckInput {
        let root = sample_root();
        CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("example.diff")),
            mode: Mode::Draft,
            format: OutputFormat::Json,
            ..CheckInput::default()
        }
    }

    fn stages(events: &[AnalysisProgressEvent]) -> Vec<AnalysisProgressStage> {
        events.iter().map(|event| event.stage).collect()
    }

    fn terminal_count(events: &[AnalysisProgressEvent]) -> usize {
        events.iter().filter(|event| event.is_terminal()).count()
    }

    fn require_check_failure<T>(result: Result<T, String>, why: &str) -> Result<(), String> {
        match result {
            Err(_) => Ok(()),
            Ok(_) => Err(why.to_string()),
        }
    }

    fn assert_honest_events(
        events: &[AnalysisProgressEvent],
        scope: AnalysisProgressScope,
        secret: &str,
    ) {
        assert!(
            !events.is_empty(),
            "producer must emit at least one progress boundary"
        );
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].elapsed_ms <= pair[1].elapsed_ms),
            "elapsed_ms must be monotonic"
        );
        for event in events {
            assert_eq!(event.scope, scope);
            assert_eq!(event.completed_units, None);
            assert_eq!(event.total_units, None);
            let rendered = format!("{event:?}");
            assert!(
                !rendered.contains(secret),
                "progress event leaked private text `{secret}`: {rendered}"
            );
        }
        assert_eq!(terminal_count(events), 1);
    }

    #[test]
    fn check_progress_reports_real_boundaries_without_invented_totals() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let input = sample_diff_input();
        let secret = input.root.display().to_string();
        let output = check_with_progress(
            input,
            &RiprConfig::default(),
            AnalysisProgressScope::Diff,
            Some(&recorder),
        )?;
        assert!(
            !output.findings.is_empty(),
            "sample diff must execute analysis"
        );
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::BuildingOutput,
                AnalysisProgressStage::Completed,
            ]
        );
        assert_honest_events(&events, AnalysisProgressScope::Diff, &secret);
        Ok(())
    }

    #[test]
    fn progress_sink_cannot_change_analysis_identity() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let input = sample_diff_input();
        let quiet = crate::app::check_workspace_with_config(input.clone(), &RiprConfig::default())?;
        let observed = check_with_progress(
            input,
            &RiprConfig::default(),
            AnalysisProgressScope::Diff,
            Some(&recorder),
        )?;
        assert_eq!(observed.findings, quiet.findings);
        assert_eq!(observed.summary, quiet.summary);
        assert_eq!(observed.schema_version, quiet.schema_version);
        assert!(
            !recorder.events().is_empty(),
            "observed run must actually emit progress"
        );
        Ok(())
    }

    #[test]
    fn missing_diff_fails_after_loading_input_without_false_completion() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let mut input = sample_diff_input();
        let secret = input.root.display().to_string();
        input.diff_file = Some(input.root.join("absent-progress-input.diff"));
        require_check_failure(
            check_with_progress(
                input,
                &RiprConfig::default(),
                AnalysisProgressScope::Diff,
                Some(&recorder),
            ),
            "missing diff must fail the check",
        )?;
        let events = recorder.events();
        assert_eq!(events[0].stage, AnalysisProgressStage::LoadingInput);
        assert_eq!(
            events.last().map(|event| event.stage),
            Some(AnalysisProgressStage::Failed)
        );
        assert!(!stages(&events).contains(&AnalysisProgressStage::Completed));
        assert!(!stages(&events).contains(&AnalysisProgressStage::BuildingOutput));
        assert_honest_events(&events, AnalysisProgressScope::Diff, &secret);
        Ok(())
    }

    #[test]
    fn suppression_policy_failure_after_analysis_is_failed_not_completed() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let mut input = sample_diff_input();
        input.suppression_policy = Some(input.root.join("absent-progress-suppression.toml"));
        require_check_failure(
            check_with_progress(
                input,
                &RiprConfig::default(),
                AnalysisProgressScope::Diff,
                Some(&recorder),
            ),
            "absent suppression policy must fail the check",
        )?;
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::BuildingOutput,
                AnalysisProgressStage::Failed,
            ]
        );
        assert_eq!(terminal_count(&events), 1);
        Ok(())
    }

    #[test]
    fn git_candidate_bind_failure_never_claims_analysis_or_completion() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let temp = std::env::temp_dir().join("ripr-2608-progress-not-a-repo");
        std::fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
        let tree = crate::domain::GitObjectId::parse(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .map_err(|error| error.to_string())?;
        let subject = crate::domain::GitCandidateSubject::new(
            &temp,
            crate::domain::GitCandidateBase::EmptyTree,
            tree,
        );
        let mut input = sample_diff_input();
        input.root = temp;
        input.diff_file = None;
        input.base = None;
        input.git_candidate = Some(subject);
        require_check_failure(
            check_with_progress(
                input,
                &RiprConfig::default(),
                AnalysisProgressScope::Diff,
                Some(&recorder),
            ),
            "unbound git candidate must fail the check",
        )?;
        let events = recorder.events();
        assert_eq!(events[0].stage, AnalysisProgressStage::LoadingInput);
        assert!(stages(&events).contains(&AnalysisProgressStage::Analyzing));
        assert_eq!(
            events.last().map(|event| event.stage),
            Some(AnalysisProgressStage::Failed)
        );
        assert!(!stages(&events).contains(&AnalysisProgressStage::BuildingOutput));
        assert!(!stages(&events).contains(&AnalysisProgressStage::Completed));
        assert_eq!(terminal_count(&events), 1);
        Ok(())
    }

    #[test]
    fn worktree_scope_survives_an_early_failure() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let mut input = sample_diff_input();
        input.diff_file = None;
        input.root = std::env::temp_dir().join("ripr-2608-progress-worktree-missing");
        require_check_failure(
            check_with_progress(
                input,
                &RiprConfig::default(),
                AnalysisProgressScope::Worktree,
                Some(&recorder),
            ),
            "missing worktree root must fail the check",
        )?;
        let events = recorder.events();
        assert!(
            events
                .iter()
                .all(|event| event.scope == AnalysisProgressScope::Worktree)
        );
        assert_eq!(
            events.last().map(|event| event.stage),
            Some(AnalysisProgressStage::Failed)
        );
        assert!(!stages(&events).contains(&AnalysisProgressStage::Completed));
        Ok(())
    }

    #[test]
    fn repo_scope_uses_the_same_producer_boundaries() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let input = CheckInput {
            root: sample_root(),
            mode: Mode::Draft,
            format: OutputFormat::Json,
            ..CheckInput::default()
        };
        let output = check_with_progress(
            input,
            &RiprConfig::default(),
            AnalysisProgressScope::Repo,
            Some(&recorder),
        )?;
        assert!(
            !output.findings.is_empty(),
            "sample repo check must execute analysis"
        );
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::BuildingOutput,
                AnalysisProgressStage::Completed,
            ]
        );
        assert_honest_events(
            &events,
            AnalysisProgressScope::Repo,
            &sample_root().display().to_string(),
        );
        Ok(())
    }

    #[test]
    fn cancelled_token_through_check_emits_cancelled_not_completed() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let token = AnalysisCancellationToken::new();
        assert!(token.cancel(AnalysisAbortKind::Cancelled));
        let result = cancellation::with_token(&token, || {
            check_with_progress(
                sample_diff_input(),
                &RiprConfig::default(),
                AnalysisProgressScope::Diff,
                Some(&recorder),
            )
        });
        require_check_failure(result, "cancelled token must fail the check")?;
        let events = recorder.events();
        assert_eq!(
            events.last().map(|event| event.stage),
            Some(AnalysisProgressStage::Cancelled)
        );
        assert!(!stages(&events).contains(&AnalysisProgressStage::Completed));
        assert_eq!(terminal_count(&events), 1);
        Ok(())
    }

    #[test]
    fn deadline_abort_is_cancelled_not_completed() -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let token = AnalysisCancellationToken::new();
        assert!(token.cancel(AnalysisAbortKind::DeadlineExceeded));
        let result = cancellation::with_token(&token, || {
            check_with_progress(
                sample_diff_input(),
                &RiprConfig::default(),
                AnalysisProgressScope::Diff,
                Some(&recorder),
            )
        });
        require_check_failure(result, "deadline abort must fail the check")?;
        let events = recorder.events();
        assert_eq!(
            events.last().map(|event| event.stage),
            Some(AnalysisProgressStage::Cancelled)
        );
        assert!(!stages(&events).contains(&AnalysisProgressStage::Completed));
        Ok(())
    }

    #[test]
    fn drop_does_not_invent_a_deadline_by_calling_checkpoint() {
        let recorder = ProgressRecorder::new();
        let started = Instant::now();
        let token = AnalysisCancellationToken::with_budget(
            started,
            Duration::ZERO,
            std::sync::Arc::new(Instant::now),
        );
        cancellation::with_token(&token, || {
            let mut progress = ProgressRun::new(Some(&recorder), AnalysisProgressScope::Diff);
            progress.emit(AnalysisProgressStage::LoadingInput);
        });
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Failed,
            ]
        );
        assert!(token.abort_kind().is_none());
    }

    #[test]
    fn complete_then_drop_emits_one_completed_terminal() {
        let recorder = ProgressRecorder::new();
        {
            let mut progress = ProgressRun::new(Some(&recorder), AnalysisProgressScope::Diff);
            progress.emit(AnalysisProgressStage::LoadingInput);
            progress.complete();
            progress.emit(AnalysisProgressStage::Analyzing);
            progress.complete();
        }
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Completed,
            ]
        );
        assert_eq!(terminal_count(&events), 1);
    }

    #[test]
    fn progress_sink_panic_cannot_change_analysis_result() -> Result<(), String> {
        struct BrokenSink;
        impl AnalysisProgressSink for BrokenSink {
            fn emit(&self, _: AnalysisProgressEvent) {
                let slots = [0u8];
                let out_of_bounds = std::env::args_os().count() + 1;
                let _tombstone = slots[out_of_bounds];
            }
        }
        let input = sample_diff_input();
        let baseline =
            crate::app::check_workspace_with_config(input.clone(), &RiprConfig::default())?;
        let observed = check_with_progress(
            input,
            &RiprConfig::default(),
            AnalysisProgressScope::Diff,
            Some(&BrokenSink),
        )?;
        assert_eq!(observed.findings, baseline.findings);
        assert_eq!(observed.summary, baseline.summary);
        Ok(())
    }

    #[test]
    fn no_sink_path_stays_available() -> Result<(), String> {
        let output = check_with_progress(
            sample_diff_input(),
            &RiprConfig::default(),
            AnalysisProgressScope::Diff,
            None,
        )?;
        assert!(
            !output.findings.is_empty(),
            "optional sink must not be required for analysis"
        );
        Ok(())
    }

    #[test]
    fn repo_inventory_with_progress_reports_the_diff_stage_shape_at_repo_scope()
    -> Result<(), String> {
        let recorder = ProgressRecorder::new();
        let assembled =
            repo_inventory_with_progress(Some(&recorder), || Ok(41usize), |value| Ok(value + 1))?;
        assert_eq!(assembled, 42);
        let events = recorder.events();
        assert_eq!(
            stages(&events),
            [
                AnalysisProgressStage::LoadingInput,
                AnalysisProgressStage::Analyzing,
                AnalysisProgressStage::BuildingOutput,
                AnalysisProgressStage::Completed,
            ],
            "repo inventory must reuse the diff-scoped stage shape"
        );
        assert_honest_events(&events, AnalysisProgressScope::Repo, "private-root-text");
        Ok(())
    }

    #[test]
    fn repo_inventory_progress_fail_closes_without_completed_on_either_closure_error() {
        for (name, expected, failing) in [
            (
                "inventory",
                vec![
                    AnalysisProgressStage::LoadingInput,
                    AnalysisProgressStage::Analyzing,
                    AnalysisProgressStage::Failed,
                ],
                true,
            ),
            (
                "assembly",
                vec![
                    AnalysisProgressStage::LoadingInput,
                    AnalysisProgressStage::Analyzing,
                    AnalysisProgressStage::BuildingOutput,
                    AnalysisProgressStage::Failed,
                ],
                false,
            ),
        ] {
            let recorder = ProgressRecorder::new();
            let result: Result<(), String> = repo_inventory_with_progress(
                Some(&recorder),
                || {
                    if failing {
                        Err("inventory failed".to_string())
                    } else {
                        Ok(())
                    }
                },
                |_| Err("assembly failed".to_string()),
            );
            let expected_error = if failing {
                "inventory failed"
            } else {
                "assembly failed"
            };
            assert_eq!(
                result,
                Err(expected_error.to_string()),
                "{name} closure error must propagate verbatim"
            );
            let events = recorder.events();
            assert_eq!(stages(&events), expected, "{name} must fail-close");
            assert!(
                !stages(&events).contains(&AnalysisProgressStage::Completed),
                "{name} failure must never project completed"
            );
        }
    }
}
