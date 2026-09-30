//! CLI projection of producer-owned analysis progress onto stderr.
//!
//! This module does not invent stages, percentages, or analysis counters. It
//! renders [`crate::app::AnalysisProgressEvent`] records as bounded stderr
//! lines, throttles heartbeats, and restores the terminal on drop.

use crate::app::{
    AnalysisProgressEvent, AnalysisProgressScope, AnalysisProgressSink, AnalysisProgressStage,
};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Documented heartbeat and visibility policy for CLI progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProgressPolicy {
    /// Interactive terminals hold the first update until this duration so a
    /// sub-threshold run does not flash. Non-TTY / CI emits immediately.
    pub min_visible: Duration,
    /// First heartbeat is eligible only after the current stage has been
    /// active this long.
    pub first_heartbeat: Duration,
    /// Minimum spacing between heartbeats. This wall-time throttle is the
    /// only bound STANDARD places on a long-running stage, so liveness
    /// spans multi-minute walks while line growth stays one line per
    /// `heartbeat_every` of stage activity (#4957).
    pub heartbeat_every: Duration,
    /// Count ceiling for embedders that need one. STANDARD disables the
    /// count ceiling (`u32::MAX`) and relies on `heartbeat_every` alone:
    /// a fixed count stopped the heartbeat a third of the way into a
    /// minutes-long repo walk and left the rest of the run silent.
    pub max_heartbeats: u32,
}

impl ProgressPolicy {
    pub(crate) const STANDARD: Self = Self {
        min_visible: Duration::from_millis(250),
        first_heartbeat: Duration::from_secs(2),
        heartbeat_every: Duration::from_secs(8),
        max_heartbeats: u32::MAX,
    };
}

/// Elapsed classes used in heartbeat lines. Raw millisecond counts never
/// appear in the projection.
pub(crate) fn elapsed_class(elapsed: Duration) -> Option<&'static str> {
    const CLASSES: &[(u64, &str)] = &[
        (2_000, "2s"),
        (5_000, "5s"),
        (10_000, "10s"),
        (30_000, "30s"),
        (60_000, "1m"),
        (120_000, "2m"),
        (300_000, "5m"),
        (600_000, "10m"),
    ];
    let ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
    CLASSES
        .iter()
        .rev()
        .find(|(floor, _)| ms >= *floor)
        .map(|(_, label)| *label)
}

pub(crate) const fn stage_token(stage: AnalysisProgressStage) -> &'static str {
    match stage {
        AnalysisProgressStage::LoadingInput => "loading_input",
        AnalysisProgressStage::Analyzing => "analyzing",
        AnalysisProgressStage::BuildingOutput => "building_output",
        AnalysisProgressStage::Completed => "completed",
        AnalysisProgressStage::Cancelled => "cancelled",
        AnalysisProgressStage::Failed => "failed",
    }
}

pub(crate) const fn scope_token(scope: AnalysisProgressScope) -> &'static str {
    match scope {
        AnalysisProgressScope::Diff => "diff",
        AnalysisProgressScope::Worktree => "worktree",
        AnalysisProgressScope::Repo => "repo",
    }
}

pub(crate) const fn stage_is_terminal(stage: AnalysisProgressStage) -> bool {
    matches!(
        stage,
        AnalysisProgressStage::Completed
            | AnalysisProgressStage::Cancelled
            | AnalysisProgressStage::Failed
    )
}

pub(crate) const fn stage_is_success_terminal(stage: AnalysisProgressStage) -> bool {
    matches!(stage, AnalysisProgressStage::Completed)
}

pub(crate) fn format_stage_line(
    stage: AnalysisProgressStage,
    scope: AnalysisProgressScope,
) -> String {
    format!(
        "ripr progress: {} [{}]",
        stage_token(stage),
        scope_token(scope)
    )
}

pub(crate) fn format_heartbeat_line(stage: AnalysisProgressStage, class: &str) -> String {
    format!(
        "ripr progress: {} still active after {class}",
        stage_token(stage)
    )
}

/// TTY in-place overwrite: pad to the longest line shown so far so a shorter
/// successor cannot leave a stale suffix after CR.
pub(crate) fn tty_overwrite(line: &str, previous_width: usize) -> (String, usize) {
    let width = line.chars().count().max(previous_width);
    let mut rendered = String::from('\r');
    rendered.push_str(line);
    if let Some(pad) = width.checked_sub(line.chars().count()) {
        rendered.push_str(&" ".repeat(pad));
    }
    (rendered, width)
}

/// Fail closed: a projected line may only carry closed tokens and elapsed
/// classes. Absolute paths, percents, ETAs, and source text are rejected.
pub(crate) fn progress_line_is_safe(line: &str) -> bool {
    if !line.starts_with("ripr progress: ") {
        return false;
    }
    if line.len() > 80 {
        return false;
    }
    if line.contains('/') || line.contains('\\') {
        return false;
    }
    let lowered = line.to_ascii_lowercase();
    if lowered.contains('%') || lowered.contains("eta") {
        return false;
    }
    let payload = &line["ripr progress: ".len()..];
    payload.chars().all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | ' ' | '[' | ']' | ':')
    })
}

/// Wall-clock source for the projection. Production passes [`Instant::now`];
/// tests inject a fake clock so multi-minute heartbeat cadence can be
/// discriminated without real sleeps.
type NowFn = Arc<dyn Fn() -> Instant + Send + Sync>;

struct Projection {
    writer: Box<dyn Write + Send>,
    tty: bool,
    policy: ProgressPolicy,
    current_stage: Option<AnalysisProgressStage>,
    current_scope: Option<AnalysisProgressScope>,
    terminal: bool,
    visible: bool,
    started: Instant,
    stage_started: Instant,
    last_heartbeat: Option<Instant>,
    heartbeat_count: u32,
    in_place: bool,
    tty_width: usize,
    hold_success: bool,
    pending_success: Option<AnalysisProgressEvent>,
    now: NowFn,
}

struct SinkInner {
    state: Mutex<Projection>,
    stop: Arc<AtomicBool>,
    heartbeat: Mutex<Option<JoinHandle<()>>>,
}

pub(crate) struct CliProgressSink {
    inner: Arc<SinkInner>,
}

impl CliProgressSink {
    pub(crate) fn for_stderr(tty: bool, policy: ProgressPolicy) -> Self {
        let sink = Self::with_writer(Box::new(io::stderr()), tty, policy);
        sink.hold_success_terminal();
        sink
    }

    /// CLI command success is later than producer `completed`. Hold that
    /// terminal until [`Self::commit_success`] so a later artifact/stdout
    /// failure can still project `failed`.
    pub(crate) fn hold_success_terminal(&self) {
        Self::lock_state(&self.inner).hold_success = true;
    }

    pub(crate) fn commit_success(&self) {
        let mut projection = Self::lock_state(&self.inner);
        let Some(event) = projection.pending_success.take() else {
            return;
        };
        Self::project_terminal(&mut projection, event);
    }

    pub(crate) fn with_writer(
        writer: Box<dyn Write + Send>,
        tty: bool,
        policy: ProgressPolicy,
    ) -> Self {
        Self::with_writer_and_clock(writer, tty, policy, Arc::new(Instant::now))
    }

    fn with_writer_and_clock(
        writer: Box<dyn Write + Send>,
        tty: bool,
        policy: ProgressPolicy,
        now: NowFn,
    ) -> Self {
        let started = now();
        Self {
            inner: Arc::new(SinkInner {
                state: Mutex::new(Projection {
                    writer,
                    tty,
                    policy,
                    current_stage: None,
                    current_scope: None,
                    terminal: false,
                    visible: !tty,
                    started,
                    stage_started: started,
                    last_heartbeat: None,
                    heartbeat_count: 0,
                    in_place: false,
                    tty_width: 0,
                    hold_success: false,
                    pending_success: None,
                    now,
                }),
                stop: Arc::new(AtomicBool::new(false)),
                heartbeat: Mutex::new(None),
            }),
        }
    }

    fn lock_state(inner: &SinkInner) -> std::sync::MutexGuard<'_, Projection> {
        match inner.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn write_line(projection: &mut Projection, line: &str) {
        if !progress_line_is_safe(line) {
            return;
        }
        let rendered = if projection.tty {
            projection.in_place = true;
            let (owned, width) = tty_overwrite(line, projection.tty_width);
            projection.tty_width = width;
            owned
        } else {
            let mut owned = line.to_string();
            owned.push('\n');
            owned
        };
        let _ = projection.writer.write_all(rendered.as_bytes());
        let _ = projection.writer.flush();
    }

    fn finish_in_place(projection: &mut Projection) {
        if projection.in_place {
            let _ = projection.writer.write_all(b"\n");
            let _ = projection.writer.flush();
            projection.in_place = false;
        }
    }

    fn emit_event(&self, event: AnalysisProgressEvent) {
        let mut projection = Self::lock_state(&self.inner);
        if projection.terminal {
            return;
        }
        let now = (projection.now)();
        if projection.current_stage.is_none() {
            projection.started = now;
        }
        if projection.current_stage != Some(event.stage) {
            projection.stage_started = now;
            projection.last_heartbeat = None;
        }
        projection.current_stage = Some(event.stage);
        projection.current_scope = Some(event.scope);

        if stage_is_terminal(event.stage) {
            if projection.hold_success && stage_is_success_terminal(event.stage) {
                projection.pending_success = Some(event);
                return;
            }
            Self::project_terminal(&mut projection, event);
            return;
        }

        if projection.tty && now.duration_since(projection.started) < projection.policy.min_visible
        {
            return;
        }
        projection.visible = true;
        let line = format_stage_line(event.stage, event.scope);
        Self::write_line(&mut projection, &line);
    }

    fn project_terminal(projection: &mut Projection, event: AnalysisProgressEvent) {
        projection.terminal = true;
        let too_short = projection.tty
            && stage_is_success_terminal(event.stage)
            && (projection.now)().duration_since(projection.started)
                < projection.policy.min_visible;
        if too_short {
            Self::finish_in_place(projection);
            return;
        }
        projection.visible = true;
        let line = format_stage_line(event.stage, event.scope);
        Self::write_line(projection, &line);
        Self::finish_in_place(projection);
    }

    fn project_held_failure(projection: &mut Projection) {
        let Some(event) = projection.pending_success.take() else {
            return;
        };
        Self::project_terminal(
            projection,
            AnalysisProgressEvent {
                stage: AnalysisProgressStage::Failed,
                scope: event.scope,
                completed_units: None,
                total_units: None,
                elapsed_ms: event.elapsed_ms,
            },
        );
    }

    fn heartbeat_tick(inner: &SinkInner) {
        let mut projection = Self::lock_state(inner);
        if projection.terminal || projection.pending_success.is_some() {
            return;
        }
        if projection.heartbeat_count >= projection.policy.max_heartbeats {
            return;
        }
        let Some(stage) = projection.current_stage else {
            return;
        };
        if stage_is_terminal(stage) {
            return;
        }
        let now = (projection.now)();
        if !projection.visible {
            if now.duration_since(projection.started) < projection.policy.min_visible {
                return;
            }
            let Some(scope) = projection.current_scope else {
                return;
            };
            projection.visible = true;
            let line = format_stage_line(stage, scope);
            Self::write_line(&mut projection, &line);
        }
        let stage_age = now.duration_since(projection.stage_started);
        if stage_age < projection.policy.first_heartbeat {
            return;
        }
        if let Some(last) = projection.last_heartbeat
            && now.duration_since(last) < projection.policy.heartbeat_every
        {
            return;
        }
        let Some(class) = elapsed_class(stage_age) else {
            return;
        };
        let line = format_heartbeat_line(stage, class);
        Self::write_line(&mut projection, &line);
        projection.last_heartbeat = Some(now);
        projection.heartbeat_count = projection.heartbeat_count.saturating_add(1);
    }

    fn start_heartbeat_thread(&self) {
        let mut slot = match self.inner.heartbeat.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if slot.is_some() || self.inner.stop.load(Ordering::Relaxed) {
            return;
        }
        let inner = Arc::clone(&self.inner);
        *slot = Some(thread::spawn(move || {
            while !inner.stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(50));
                if inner.stop.load(Ordering::Relaxed) {
                    break;
                }
                CliProgressSink::heartbeat_tick(&inner);
            }
        }));
    }

    fn stop_heartbeat(&self) {
        self.inner.stop.store(true, Ordering::Relaxed);
        let mut slot = match self.inner.heartbeat.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(handle) = slot.take() {
            let _ = handle.join();
        }
    }
}

impl AnalysisProgressSink for CliProgressSink {
    fn emit(&self, event: AnalysisProgressEvent) {
        self.emit_event(event);
        if stage_is_terminal(event.stage) {
            self.stop_heartbeat();
        } else {
            self.start_heartbeat_thread();
        }
    }
}

impl Drop for CliProgressSink {
    fn drop(&mut self) {
        self.stop_heartbeat();
        let mut projection = Self::lock_state(&self.inner);
        Self::project_held_failure(&mut projection);
        Self::finish_in_place(&mut projection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::AtomicU64;

    /// Milliseconds of fake time elapsed since construction. The produced
    /// `NowFn` lets a test drive `heartbeat_tick` through minutes of stage
    /// activity deterministically.
    #[derive(Clone)]
    struct FakeClock(Arc<AtomicU64>);

    impl FakeClock {
        fn start() -> (Self, NowFn) {
            let offset = Arc::new(AtomicU64::new(0));
            let base = Instant::now();
            let driven = Arc::clone(&offset);
            let now: NowFn =
                Arc::new(move || base + Duration::from_millis(driven.load(Ordering::Relaxed)));
            (Self(offset), now)
        }

        fn advance_millis(&self, millis: u64) {
            self.0.fetch_add(millis, Ordering::Relaxed);
        }

        fn millis(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
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

    struct BrokenWriter;

    impl Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("progress sink failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn tty_visible_line(text: &str) -> String {
        let body = text.strip_suffix('\n').unwrap_or(text);
        let last = body.rsplit('\n').next().unwrap_or("");
        let mut line = Vec::<u8>::new();
        let mut col = 0usize;
        for byte in last.bytes() {
            match byte {
                b'\r' => col = 0,
                other => {
                    if col < line.len() {
                        line[col] = other;
                    } else {
                        line.push(other);
                    }
                    col = col.saturating_add(1);
                }
            }
        }
        String::from_utf8_lossy(&line).trim_end().to_owned()
    }

    fn event(stage: AnalysisProgressStage) -> AnalysisProgressEvent {
        AnalysisProgressEvent {
            stage,
            scope: AnalysisProgressScope::Diff,
            completed_units: None,
            total_units: None,
            elapsed_ms: 0,
        }
    }

    fn non_tty_policy() -> ProgressPolicy {
        ProgressPolicy {
            min_visible: Duration::ZERO,
            first_heartbeat: Duration::from_secs(2),
            heartbeat_every: Duration::from_millis(50),
            max_heartbeats: 4,
        }
    }

    #[test]
    fn non_tty_stage_lines_are_newline_delimited_and_path_free() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.emit(event(AnalysisProgressStage::LoadingInput));
        sink.emit(event(AnalysisProgressStage::Analyzing));
        sink.emit(event(AnalysisProgressStage::BuildingOutput));
        sink.emit(event(AnalysisProgressStage::Completed));
        let text = buffer.text();
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\r'));
        assert!(!text.contains('%'));
        assert!(!text.to_ascii_lowercase().contains("eta"));
        assert!(text.contains("ripr progress: loading_input [diff]"));
        assert!(text.contains("ripr progress: analyzing [diff]"));
        assert!(text.contains("ripr progress: building_output [diff]"));
        assert!(text.contains("ripr progress: completed [diff]"));
        assert_eq!(text.matches("ripr progress: completed").count(), 1);
        for line in text.lines() {
            assert!(progress_line_is_safe(line), "unsafe progress line: {line}");
        }
    }

    #[test]
    fn unknown_totals_never_render_as_percentage_or_eta() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.emit(AnalysisProgressEvent {
            stage: AnalysisProgressStage::Analyzing,
            scope: AnalysisProgressScope::Diff,
            completed_units: Some(3),
            total_units: None,
            elapsed_ms: 4_000,
        });
        let text = buffer.text();
        assert!(!text.contains('%'));
        assert!(!text.contains("3/"));
        assert!(!text.to_ascii_lowercase().contains("eta"));
        assert!(text.contains("analyzing [diff]"));
    }

    #[test]
    fn tty_short_run_does_not_flash_completed() {
        let buffer = Buffer::new();
        let policy = ProgressPolicy {
            min_visible: Duration::from_secs(30),
            first_heartbeat: Duration::from_secs(30),
            heartbeat_every: Duration::from_secs(30),
            max_heartbeats: 1,
        };
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), true, policy);
        sink.emit(event(AnalysisProgressStage::LoadingInput));
        sink.emit(event(AnalysisProgressStage::Completed));
        drop(sink);
        assert!(
            !buffer.text().contains("ripr progress:"),
            "sub-threshold TTY run must not spray stages: {}",
            buffer.text()
        );
    }

    #[test]
    fn tty_and_non_tty_share_stage_tokens() {
        let tty_buf = Buffer::new();
        let ci_buf = Buffer::new();
        let policy = ProgressPolicy {
            min_visible: Duration::ZERO,
            first_heartbeat: Duration::from_secs(30),
            heartbeat_every: Duration::from_secs(30),
            max_heartbeats: 1,
        };
        let tty = CliProgressSink::with_writer(Box::new(tty_buf.clone()), true, policy);
        let ci = CliProgressSink::with_writer(Box::new(ci_buf.clone()), false, policy);
        for sink in [&tty, &ci] {
            sink.emit(event(AnalysisProgressStage::LoadingInput));
            sink.emit(event(AnalysisProgressStage::Analyzing));
            sink.emit(event(AnalysisProgressStage::Completed));
        }
        drop(tty);
        drop(ci);
        for token in ["loading_input", "analyzing", "completed"] {
            assert!(tty_buf.text().contains(token));
            assert!(ci_buf.text().contains(token));
        }
        assert!(tty_buf.text().contains('\r'));
        assert!(!ci_buf.text().contains('\r'));
    }

    #[test]
    fn failure_terminal_never_emits_completed() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.emit(event(AnalysisProgressStage::LoadingInput));
        sink.emit(event(AnalysisProgressStage::Failed));
        sink.emit(event(AnalysisProgressStage::Completed));
        let text = buffer.text();
        assert!(text.contains("ripr progress: failed [diff]"));
        assert!(!text.contains("completed"));
        assert_eq!(text.matches("ripr progress: failed").count(), 1);
    }

    #[test]
    fn tty_suppressed_stage_becomes_visible_once_min_visible_elapses() {
        let buffer = Buffer::new();
        let policy = ProgressPolicy {
            min_visible: Duration::from_millis(40),
            first_heartbeat: Duration::from_secs(2),
            heartbeat_every: Duration::from_millis(40),
            max_heartbeats: 2,
        };
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), true, policy);
        sink.emit(event(AnalysisProgressStage::Analyzing));
        assert!(
            !buffer.text().contains("ripr progress:"),
            "TTY must stay silent before min_visible: {}",
            buffer.text()
        );
        thread::sleep(Duration::from_millis(50));
        CliProgressSink::heartbeat_tick(&sink.inner);
        let revealed = buffer.text();
        assert!(
            revealed.contains("analyzing"),
            "active TTY stage must appear after min_visible: {revealed}"
        );
        assert!(
            !revealed.contains("still active"),
            "heartbeat must wait for first_heartbeat: {revealed}"
        );
        thread::sleep(Duration::from_secs(2));
        CliProgressSink::heartbeat_tick(&sink.inner);
        let beating = buffer.text();
        assert!(
            beating.contains("still active after"),
            "blocked TTY stage must heartbeat after first_heartbeat: {beating}"
        );
        sink.emit(event(AnalysisProgressStage::Completed));
    }

    #[test]
    fn tty_overwrite_clears_a_longer_previous_line() {
        let buffer = Buffer::new();
        let policy = ProgressPolicy {
            min_visible: Duration::ZERO,
            first_heartbeat: Duration::from_secs(30),
            heartbeat_every: Duration::from_secs(30),
            max_heartbeats: 1,
        };
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), true, policy);
        sink.emit(event(AnalysisProgressStage::BuildingOutput));
        {
            let mut projection = CliProgressSink::lock_state(&sink.inner);
            let long = format_heartbeat_line(AnalysisProgressStage::BuildingOutput, "10m");
            assert!(
                long.chars().count()
                    > format_stage_line(
                        AnalysisProgressStage::Completed,
                        AnalysisProgressScope::Diff
                    )
                    .chars()
                    .count(),
                "control requires a longer heartbeat than completed: {long}"
            );
            CliProgressSink::write_line(&mut projection, &long);
        }
        sink.emit(event(AnalysisProgressStage::Completed));
        drop(sink);
        let visible = tty_visible_line(&buffer.text());
        assert!(
            visible.contains("completed"),
            "TTY successor must show completed: {visible:?}"
        );
        assert!(
            !visible.contains("still active"),
            "longer heartbeat suffix must not remain: {visible:?}"
        );
        assert!(
            !visible.contains("10m"),
            "elapsed class from the prior line must not remain: {visible:?}"
        );
    }

    #[test]
    fn held_completed_waits_for_command_commit() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.hold_success_terminal();
        sink.emit(event(AnalysisProgressStage::Analyzing));
        sink.emit(event(AnalysisProgressStage::Completed));
        let held = buffer.text();
        assert!(
            held.contains("analyzing"),
            "non-terminal stages still project while success is held: {held}"
        );
        assert!(
            !held.contains("completed"),
            "producer completed must not render before command commit: {held}"
        );
        sink.commit_success();
        let committed = buffer.text();
        assert!(
            committed.contains("ripr progress: completed [diff]"),
            "commit must project completed: {committed}"
        );
        assert!(!committed.contains("failed"));
    }

    #[test]
    fn drop_without_commit_converts_held_completed_to_failed() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.hold_success_terminal();
        sink.emit(event(AnalysisProgressStage::Analyzing));
        sink.emit(event(AnalysisProgressStage::Completed));
        drop(sink);
        let text = buffer.text();
        assert!(
            text.contains("ripr progress: failed [diff]"),
            "Drop must fail-close a held success terminal: {text}"
        );
        assert!(!text.contains("completed"));
    }

    #[test]
    fn tty_overwrite_pads_to_the_longest_prior_line() {
        let long = format_heartbeat_line(AnalysisProgressStage::BuildingOutput, "10m");
        let short = format_stage_line(
            AnalysisProgressStage::Completed,
            AnalysisProgressScope::Diff,
        );
        let (first, width) = tty_overwrite(&long, 0);
        let (second, _) = tty_overwrite(&short, width);
        let visible = tty_visible_line(&format!("{first}{second}"));
        assert!(visible.contains("completed"));
        assert!(!visible.contains("still active"));
        assert!(!visible.contains("10m"));
        assert_eq!(width, long.chars().count());
    }

    #[test]
    fn rendering_failure_is_isolated() {
        let sink = CliProgressSink::with_writer(Box::new(BrokenWriter), false, non_tty_policy());
        sink.emit(event(AnalysisProgressStage::Analyzing));
        sink.emit(event(AnalysisProgressStage::Completed));
    }

    #[test]
    fn heartbeat_is_throttled_and_bounded() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.emit(event(AnalysisProgressStage::Analyzing));
        let deadline = Instant::now() + Duration::from_millis(2500);
        while Instant::now() < deadline {
            if buffer.text().matches("still active after").count() >= 2 {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let during = buffer.text();
        assert!(
            during.contains("still active after"),
            "blocked stage must heartbeat, got: {during:?}"
        );
        sink.emit(event(AnalysisProgressStage::Completed));
        drop(sink);
        let heartbeats = during.matches("still active after").count();
        assert!(heartbeats <= 4, "heartbeat ceiling exceeded: {heartbeats}");
        assert!(!during.contains('%'));
        assert!(!during.contains('\u{1b}'));
    }

    #[test]
    fn standard_heartbeat_spans_long_repo_walks_without_a_ten_second_gap() {
        // #4957: the STANDARD policy is time-throttled, not count-capped. A
        // repo stage held open far past the old 16-heartbeat stop (~32s) must
        // keep emitting, and no 10s window of stage time may stay silent.
        let buffer = Buffer::new();
        let (clock, now) = FakeClock::start();
        let sink = CliProgressSink::with_writer_and_clock(
            Box::new(buffer.clone()),
            false,
            ProgressPolicy::STANDARD,
            now,
        );
        sink.emit(AnalysisProgressEvent {
            stage: AnalysisProgressStage::Analyzing,
            scope: AnalysisProgressScope::Repo,
            completed_units: None,
            total_units: None,
            elapsed_ms: 0,
        });
        // Take over the tick loop so emission times are exactly the fake
        // instants this test drives.
        sink.stop_heartbeat();
        let heartbeat_count = |buffer: &Buffer| buffer.text().matches("still active after").count();
        let mut emitted_at_ms: Vec<u64> = Vec::new();
        let mut previous = heartbeat_count(&buffer);
        // Ten minutes of a cold full-repo walk, ticked once per fake second.
        for _ in 0..600 {
            clock.advance_millis(1_000);
            CliProgressSink::heartbeat_tick(&sink.inner);
            let current = heartbeat_count(&buffer);
            if current > previous {
                emitted_at_ms.push(clock.millis());
                previous = current;
            }
        }
        // Liveness continues far past where the old count cap went silent.
        assert!(
            emitted_at_ms.len() > 16,
            "heartbeat must outlive the old 16-line cap: {} lines in 10m",
            emitted_at_ms.len()
        );
        // First heartbeat keeps the pinned 2s first_heartbeat contract.
        assert_eq!(
            emitted_at_ms.first(),
            Some(&2_000),
            "first heartbeat must land at first_heartbeat: {emitted_at_ms:?}"
        );
        // No silence gap between consecutive heartbeats (or before the first)
        // may exceed 10 seconds of stage time.
        let mut last = 0u64;
        let mut worst_gap_ms = 0u64;
        for at in &emitted_at_ms {
            worst_gap_ms = worst_gap_ms.max(at - last);
            last = *at;
        }
        assert!(
            worst_gap_ms <= 10_000,
            "silence gap exceeded 10s of stage time: {worst_gap_ms}ms"
        );
        // The elapsed class keeps advancing on the long walk.
        let text = buffer.text();
        assert!(
            text.contains("still active after 5m"),
            "heartbeat class must advance on a minutes-long walk: {text}"
        );
        for line in text.lines() {
            assert!(progress_line_is_safe(line), "unsafe progress line: {line}");
        }
        sink.emit(event(AnalysisProgressStage::Completed));
        drop(sink);
    }

    #[test]
    fn unsafe_constructed_lines_are_rejected() {
        assert!(!progress_line_is_safe(
            "ripr progress: analyzing /tmp/secret.rs"
        ));
        assert!(!progress_line_is_safe("ripr progress: analyzing 40%"));
        assert!(!progress_line_is_safe("ripr progress: analyzing eta 12s"));
        assert!(!progress_line_is_safe("warning: something else"));
        assert!(progress_line_is_safe("ripr progress: analyzing [diff]"));
        assert!(progress_line_is_safe(
            "ripr progress: analyzing still active after 2s"
        ));
    }

    #[test]
    fn elapsed_class_is_bucketed_not_raw_millis() {
        assert_eq!(elapsed_class(Duration::from_millis(1999)), None);
        assert_eq!(elapsed_class(Duration::from_secs(2)), Some("2s"));
        assert_eq!(elapsed_class(Duration::from_millis(4999)), Some("2s"));
        assert_eq!(elapsed_class(Duration::from_secs(5)), Some("5s"));
        assert_eq!(elapsed_class(Duration::from_secs(90)), Some("1m"));
    }

    #[test]
    fn projection_tokens_cover_the_closed_producer_vocabulary() {
        assert_eq!(
            stage_token(AnalysisProgressStage::LoadingInput),
            "loading_input"
        );
        assert_eq!(stage_token(AnalysisProgressStage::Analyzing), "analyzing");
        assert_eq!(
            stage_token(AnalysisProgressStage::BuildingOutput),
            "building_output"
        );
        assert_eq!(stage_token(AnalysisProgressStage::Completed), "completed");
        assert_eq!(stage_token(AnalysisProgressStage::Cancelled), "cancelled");
        assert_eq!(stage_token(AnalysisProgressStage::Failed), "failed");
        assert_eq!(scope_token(AnalysisProgressScope::Diff), "diff");
        assert_eq!(scope_token(AnalysisProgressScope::Worktree), "worktree");
        assert_eq!(scope_token(AnalysisProgressScope::Repo), "repo");
        assert!(!stage_is_terminal(AnalysisProgressStage::Analyzing));
        assert!(stage_is_terminal(AnalysisProgressStage::Cancelled));
        assert!(stage_is_success_terminal(AnalysisProgressStage::Completed));
        assert!(!stage_is_success_terminal(AnalysisProgressStage::Failed));
        assert!(!stage_is_success_terminal(AnalysisProgressStage::Cancelled));
        assert_eq!(
            format_stage_line(
                AnalysisProgressStage::Analyzing,
                AnalysisProgressScope::Worktree
            ),
            "ripr progress: analyzing [worktree]"
        );
        assert_eq!(
            format_stage_line(
                AnalysisProgressStage::Completed,
                AnalysisProgressScope::Repo
            ),
            "ripr progress: completed [repo]"
        );
    }

    #[test]
    fn cancelled_terminal_never_emits_completed() {
        let buffer = Buffer::new();
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), false, non_tty_policy());
        sink.emit(event(AnalysisProgressStage::Analyzing));
        sink.emit(event(AnalysisProgressStage::Cancelled));
        sink.emit(event(AnalysisProgressStage::Completed));
        let text = buffer.text();
        assert!(text.contains("ripr progress: cancelled [diff]"));
        assert!(!text.contains("completed"));
        assert_eq!(text.matches("ripr progress: cancelled").count(), 1);
    }

    #[test]
    fn tty_short_failure_still_projects_failed() {
        let buffer = Buffer::new();
        let policy = ProgressPolicy {
            min_visible: Duration::from_secs(30),
            first_heartbeat: Duration::from_secs(30),
            heartbeat_every: Duration::from_secs(30),
            max_heartbeats: 1,
        };
        let sink = CliProgressSink::with_writer(Box::new(buffer.clone()), true, policy);
        sink.emit(event(AnalysisProgressStage::LoadingInput));
        sink.emit(event(AnalysisProgressStage::Failed));
        drop(sink);
        let text = buffer.text();
        assert!(
            text.contains("failed"),
            "non-success terminals must remain visible on a short TTY run: {text}"
        );
        assert!(!text.contains("completed"));
    }
}
