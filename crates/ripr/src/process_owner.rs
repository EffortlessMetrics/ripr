//! Shared owned-subprocess authority for bounded commands (#3803).
//!
//! Every shipped bounded subprocess path — the git invocation helpers, the
//! Perl facts exporter probe, the cargo metadata probe, and the `xtask`
//! timed-command runners — spawns its child through [`OwnedProcess`] so one
//! typed owner holds the process, its termination, wait and cleanup. On
//! Windows the owner guarantees Job Object containment with
//! assign-before-execution:
//!
//! ```text
//! create primary process suspended (CREATE_SUSPENDED)
//! → create Job Object + assign the suspended process
//! → resume execution
//! → bounded observation by the caller
//! → on timeout/cancellation/error terminate the owned job
//! → wait/reap the primary process
//! → only then release ownership
//! ```
//!
//! Because the assignment happens before any child user code can run, every
//! descendant the child later spawns joins the job, and terminating the job
//! (or ending ownership) kills the whole tree. No orphaned descendant can
//! outlive the owner to hold Windows file or working-directory locks. This
//! replaces the previous `taskkill /PID <pid> /T /F` shell dependency, whose
//! snapshot-based tree walk was racy (#3096, #1022) and which silently
//! degraded to a direct-child kill whenever `taskkill` was missing or
//! reported success before the parent had exited.
//!
//! Ownership is never dropped without cleanup: [`Drop`] terminates the job
//! and reaps the direct child even on panic unwinding. Non-Windows targets
//! keep their existing behavior: a transparent `std::process::Child`
//! passthrough whose termination kills and reaps the direct child, with the
//! Unix process-group authority unchanged in its callers.

pub(crate) mod native_limits;
mod qualified_capture;
mod retained_stage;

#[doc(hidden)]
pub use qualified_capture::{
    CompleteByteCapture, CompleteCaptureBudget, CompleteCaptureError, CompleteCaptureReceipt,
    CompleteCapturedBytes,
};
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) use qualified_capture::{
    CompleteEnclosingCustodian, CompleteEnclosingDisposed, CompleteEnclosingFailure,
    CompleteEnclosingPhysicalClosure, CompleteFailedClosed, CompleteTerminalCustodian,
    CompleteTerminalFailure,
};

#[cfg(target_os = "linux")]
#[doc(hidden)]
pub use qualified_capture::{
    ObservedProcessIdentity, QualifiedGroupOwner, QualifiedGroupSettlement, QualifiedGroupWait,
};

#[doc(hidden)]
pub use native_limits::NativeLimitTuple;
#[doc(hidden)]
pub use retained_stage::{
    ParentStage, StageBudget, StageDirectoryBinding, StageEntry, StageEntryKind, StageInventory,
    StageInventoryClosure, StageRole, StageRoleBudget, StageRootBinding, StageUsage,
};

use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};
use std::time::{Duration, Instant};

#[cfg(windows)]
use process_wrap::std::{ChildWrapper, CommandWrap, JobObject};

/// First sleep of a child-exit poll. Short-lived probes (`git rev-parse`,
/// `git diff` on a small repository) exit within a few milliseconds, so the
/// first re-check must come back on that scale (#5348).
const POLL_BACKOFF_FIRST: Duration = Duration::from_millis(1);

/// Ceiling of the child-exit poll backoff. A long-running child is still
/// re-checked (and cancellation/deadline observed) at least this often.
pub(crate) const POLL_BACKOFF_CEILING: Duration = Duration::from_millis(50);

/// Exponential sleep schedule for `try_wait` poll loops: 1 ms, doubling, up
/// to [`POLL_BACKOFF_CEILING`].
///
/// A fixed 50 ms interval made every short subprocess cost at least 50 ms of
/// wall time because the first `try_wait` right after spawn practically
/// always sees a running child; `ripr check` on a tiny crate spent ~0.35 s of
/// its ~0.4 s sleeping on eight `git` probes that had each exited in ~3 ms
/// (#5348). The ceiling keeps the long-running cadence (and the number of
/// wakeups for a slow child) where it was.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PollBackoff {
    next: Duration,
}

impl PollBackoff {
    pub(crate) const fn new() -> Self {
        Self {
            next: POLL_BACKOFF_FIRST,
        }
    }

    /// The delay to sleep before the next poll; advances the schedule.
    pub(crate) fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(POLL_BACKOFF_CEILING);
        delay
    }

    /// Sleep for the next delay, never past `deadline` when one is given.
    pub(crate) fn sleep(&mut self, deadline: Option<Instant>) {
        let mut delay = self.next_delay();
        if let Some(deadline) = deadline {
            delay = delay.min(deadline.saturating_duration_since(Instant::now()));
        }
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
    }
}

/// Budget for the best-effort primary reap after a failed tree-termination
/// request. Long enough to observe a dying primary on a loaded host, short
/// enough that the returned cleanup-failure error is never delayed by a
/// process that refuses to die.
const FALLBACK_REAP_BUDGET: Duration = Duration::from_secs(5);

/// One owned child process for a bounded command path.
///
/// Windows: the child is created suspended, assigned to a Job Object before
/// any child user code executes, and only then resumed. Any descendant it
/// spawns joins the job, so [`OwnedProcess::terminate_tree`] (or ending
/// ownership) kills the entire assigned tree. Other platforms: a transparent
/// wrapper over `std::process::Child`; termination kills and reaps the
/// direct child exactly as the pre-#3803 helpers did.
#[derive(Debug)]
pub struct OwnedProcess {
    #[cfg(windows)]
    child: Box<dyn ChildWrapper>,
    #[cfg(not(windows))]
    child: std::process::Child,
    #[cfg(not(windows))]
    bounded_drop: bool,
    #[cfg(target_os = "linux")]
    bounded_drop_until: Option<Instant>,
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    enclosing_observed_reap: Option<(ExitStatus, Instant)>,
}

/// A strict spawn refusal retains any actual child created before the clock crossed.
/// This private custody is not a process-group or termination observation.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(crate) struct OwnedSpawnFailure {
    error: std::io::Error,
    child: Option<OwnedProcess>,
}
#[cfg(target_os = "linux")]
impl OwnedSpawnFailure {
    fn before_spawn(error: std::io::Error) -> Self {
        Self { error, child: None }
    }

    pub(crate) fn into_parts(self) -> (std::io::Error, Option<OwnedProcess>) {
        (self.error, self.child)
    }
}
#[cfg(target_os = "linux")]
impl std::fmt::Display for OwnedSpawnFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, formatter)
    }
}

#[cfg(all(test, target_os = "linux"))]
thread_local! {
    static POST_SPAWN_DEADLINE_BARRIER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn with_post_spawn_deadline_barrier<T>(work: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            POST_SPAWN_DEADLINE_BARRIER.with(|barrier| barrier.set(self.0));
        }
    }
    let previous = POST_SPAWN_DEADLINE_BARRIER.with(|barrier| barrier.replace(true));
    let _restore = Restore(previous);
    work()
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
thread_local! {
    static ENCLOSING_AFTER_REAP_DELAY: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
    static ENCLOSING_AFTER_TRY_WAIT_DELAY: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
    static ENCLOSING_BEFORE_TRY_WAIT_DELAY: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn with_enclosing_after_reap_delay<T>(delay: Duration, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENCLOSING_AFTER_REAP_DELAY.with(|stored| stored.set(self.0));
        }
    }
    let previous = ENCLOSING_AFTER_REAP_DELAY.with(|stored| stored.replace(Some(delay)));
    let _restore = Restore(previous);
    work()
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn with_enclosing_after_try_wait_delay<T>(delay: Duration, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENCLOSING_AFTER_TRY_WAIT_DELAY.with(|stored| stored.set(self.0));
        }
    }
    let previous = ENCLOSING_AFTER_TRY_WAIT_DELAY.with(|stored| stored.replace(Some(delay)));
    let _restore = Restore(previous);
    work()
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn with_enclosing_before_try_wait_delay<T>(delay: Duration, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENCLOSING_BEFORE_TRY_WAIT_DELAY.with(|stored| stored.set(self.0));
        }
    }
    let previous = ENCLOSING_BEFORE_TRY_WAIT_DELAY.with(|stored| stored.replace(Some(delay)));
    let _restore = Restore(previous);
    work()
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn enclosing_time(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("enclosing failure disposition reached its admitted ceiling".to_string())
    } else {
        Ok(())
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn enclosing_disposal_grace() -> Duration {
    FALLBACK_REAP_BUDGET
}

impl OwnedProcess {
    /// Spawn the fully prepared `command` under owned containment.
    ///
    /// Stdio redirection, environment, working directory, arguments and any
    /// platform process-group configuration must be applied to `command`
    /// before this call; they are preserved exactly. On Windows the process
    /// is created with `CREATE_SUSPENDED`, assigned to a fresh Job Object,
    /// and resumed inside this call. A setup failure (job creation,
    /// assignment, or resume) is returned as a typed `std::io::Error` and
    /// the suspended or partially set-up child is terminated and reaped
    /// before the error surfaces, so no half-owned process escapes.
    pub fn spawn(command: Command) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            let mut wrapped = CommandWrap::from(command);
            wrapped.wrap(JobObject);
            let child = wrapped.spawn()?;
            Ok(Self { child })
        }
        #[cfg(not(windows))]
        {
            // The passthrough spawn configures the prepared command in
            // place; the owned binding keeps the call site identical to
            // the Windows arm's by-value consumption.
            let mut command = command;
            let child = command.spawn()?;
            Ok(Self {
                child,
                bounded_drop: false,
                #[cfg(target_os = "linux")]
                bounded_drop_until: None,
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                enclosing_observed_reap: None,
            })
        }
    }

    /// Spawn with bounded direct-child fallback on ownership release.
    ///
    /// This is an explicit opt-in for callers with a separate qualified
    /// process-group settlement authority. Ordinary spawn/Drop behavior is
    /// unchanged. A refused settlement cannot fall into an unbounded direct
    /// child wait; bounded fallback never certifies descendant termination.
    /// Windows retains the existing owned Job Object behavior.
    pub fn spawn_with_bounded_drop(command: Command) -> std::io::Result<Self> {
        let owned = Self::spawn(command)?;
        #[cfg(not(windows))]
        {
            let mut owned = owned;
            owned.bounded_drop = true;
            Ok(owned)
        }
        #[cfg(windows)]
        {
            Ok(owned)
        }
    }

    /// Bind direct-child fallback to an already admitted absolute ceiling.
    /// This remains cooperative; a blocked syscall cannot be preempted here.
    #[cfg(target_os = "linux")]
    pub(crate) fn spawn_with_bounded_drop_until(
        command: Command,
        deadline: Instant,
    ) -> Result<Self, OwnedSpawnFailure> {
        if Instant::now() >= deadline {
            return Err(OwnedSpawnFailure::before_spawn(std::io::Error::other(
                "owned process held deadline expired before spawn",
            )));
        }
        let mut owned = Self::spawn(command).map_err(OwnedSpawnFailure::before_spawn)?;
        // Store the original ceiling before any fallible post-spawn observation.
        owned.bounded_drop = true;
        owned.bounded_drop_until = Some(deadline);
        #[cfg(test)]
        if POST_SPAWN_DEADLINE_BARRIER.with(|barrier| barrier.replace(false)) {
            // A real finite clock barrier after real spawn, not a synthetic clock.
            while Instant::now() < deadline {
                std::thread::yield_now();
            }
        }
        if Instant::now() >= deadline {
            return Err(OwnedSpawnFailure {
                error: std::io::Error::other("owned process spawn crossed its held deadline"),
                child: Some(owned),
            });
        }
        Ok(owned)
    }

    #[cfg(target_os = "linux")]
    fn reap_until(&mut self, deadline: Instant) -> bool {
        let mut backoff = PollBackoff::new();
        loop {
            if Instant::now() >= deadline {
                return false;
            }
            let observed = self.child.try_wait();
            if Instant::now() >= deadline {
                return false;
            }
            match observed {
                Ok(Some(_)) => return true,
                Ok(None) => backoff.sleep(Some(deadline)),
                Err(_) => return false,
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn drop_until(&mut self, held_deadline: Instant) {
        let deadline = Instant::now()
            .checked_add(FALLBACK_REAP_BUDGET)
            .map_or(held_deadline, |phase| phase.min(held_deadline));
        if Instant::now() >= deadline {
            return;
        }
        let observed = self.child.try_wait();
        if Instant::now() >= deadline || observed.is_ok_and(|status| status.is_some()) {
            return;
        }
        let killed = self.request_kill();
        if Instant::now() >= deadline {
            return;
        }
        if killed.is_ok() {
            let _reaped = self.reap_until(deadline);
        }
        // Failure/expiry leaves cleanup unconfirmed; Drop grants no receipt.
    }

    // Enclosing failure disposition is separately admitted before capture.
    // It never changes the failed capture's stored drop ceiling or grants a
    // capture/group/EOF observation.
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    fn enclosing_kill_until(&mut self, deadline: Instant) -> Result<(), String> {
        enclosing_time(deadline)?;
        let killed = self.request_kill();
        enclosing_time(deadline)?;
        killed.map_err(|error| format!("enclosing direct kill: {error}"))
    }

    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    fn enclosing_reap_until(
        &mut self,
        deadline: Instant,
        status: &mut Option<ExitStatus>,
    ) -> Result<(), String> {
        let mut backoff = PollBackoff::new();
        loop {
            enclosing_time(deadline)?;
            let observed = self.child.try_wait();
            if let Ok(Some(actual)) = observed.as_ref() {
                // Preserve actual progress even when the following check is late.
                *status = Some(*actual);
                self.enclosing_observed_reap.get_or_insert((*actual, Instant::now()));
                if let Some(delay) = ENCLOSING_AFTER_REAP_DELAY.with(|stored| stored.take()) {
                    std::thread::sleep(delay);
                }
            }
            enclosing_time(deadline)?;
            match observed {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => backoff.sleep(Some(deadline)),
                Err(error) => return Err(format!("enclosing direct reap: {error}")),
            }
        }
    }

    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    fn enclosing_dispose_helper_until(
        &mut self,
        deadline: Instant,
        status: &mut Option<ExitStatus>,
    ) -> Result<(), String> {
        enclosing_time(deadline)?;
        let observed = self.child.try_wait();
        if let Ok(Some(actual)) = observed.as_ref() {
            *status = Some(*actual);
            self.enclosing_observed_reap.get_or_insert((*actual, Instant::now()));
        }
        enclosing_time(deadline)?;
        match observed {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                self.enclosing_kill_until(deadline)?;
                self.enclosing_reap_until(deadline, status)
            }
            Err(error) => Err(format!("enclosing helper probe: {error}")),
        }
    }

    /// The child's process id.
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Take the piped stdin declared on the command before spawning.
    pub fn stdin_pipe(&mut self) -> &mut Option<ChildStdin> {
        #[cfg(windows)]
        {
            self.child.stdin()
        }
        #[cfg(not(windows))]
        {
            &mut self.child.stdin
        }
    }

    /// Take the piped stdout declared on the command before spawning.
    pub fn stdout_pipe(&mut self) -> &mut Option<ChildStdout> {
        #[cfg(windows)]
        {
            self.child.stdout()
        }
        #[cfg(not(windows))]
        {
            &mut self.child.stdout
        }
    }

    /// Take the piped stderr declared on the command before spawning.
    pub fn stderr_pipe(&mut self) -> &mut Option<ChildStderr> {
        #[cfg(windows)]
        {
            self.child.stderr()
        }
        #[cfg(not(windows))]
        {
            &mut self.child.stderr
        }
    }

    /// Non-blocking exit check for the direct child.
    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        {
            if let Some(delay) = ENCLOSING_BEFORE_TRY_WAIT_DELAY.with(|stored| stored.take()) {
                std::thread::sleep(delay);
            }
            let observed = self.child.try_wait();
            if let Ok(Some(status)) = observed.as_ref() {
                // Preserve the actual reap even if a following caller clock
                // check fails before its own settled status can be stored.
                self.enclosing_observed_reap.get_or_insert((*status, Instant::now()));
                if let Some(delay) = ENCLOSING_AFTER_TRY_WAIT_DELAY.with(|stored| stored.take()) {
                    std::thread::sleep(delay);
                }
            }
            observed
        }
        #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
        {
            self.child.try_wait()
        }
    }

    /// Reap the direct child to completion.
    ///
    /// This polls the child instead of blocking on a job completion port so
    /// the wait stays bounded by the caller's termination decisions and can
    /// never hang on queued job notifications.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let mut backoff = PollBackoff::new();
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            backoff.sleep(None);
        }
    }

    /// Request termination of the owned primary process without waiting.
    ///
    /// Windows: the Job Object kill request. Other platforms:
    /// `std::process::Child::kill`, which — like the Job Object request —
    /// returns `Ok(())` for an already-exited child and never waits, so the
    /// owner's terminate-then-reap sequence stays the single reaping
    /// authority.
    fn request_kill(&mut self) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            self.child.start_kill()
        }
        #[cfg(not(windows))]
        {
            self.child.kill()
        }
    }

    /// Terminate the whole owned tree and reap the direct child before
    /// returning. Windows: one termination request kills every process in
    /// the job. Other platforms: the direct child kill and reap.
    ///
    /// An already-exited child is reaped and — on Windows — its job is
    /// still terminated, because `KILL_ON_JOB_CLOSE` is deliberately off
    /// and descendants the child spawned may outlive it in the job. A
    /// FAILED termination request falls back to a bounded best-effort
    /// direct-child kill and reap and is returned as an error that reports
    /// the incomplete tree cleanup: waiting on a process that refused to
    /// die would hang the caller on the unbounded poll, and the fallback
    /// can reap the direct child without certifying that every descendant
    /// stopped. The reap error is reported only when termination
    /// succeeded (a successful kill makes a failed reap a real zombie).
    pub fn terminate_tree(&mut self) -> Result<(), String> {
        if let Some(status) = self
            .child
            .try_wait()
            .map_err(|err| format!("failed to probe owned process: {err}"))?
        {
            let _ = status;
            // The primary exited on its own, but the job it was assigned
            // does not kill on close: any descendant still in the job would
            // survive ownership unless it is terminated here. The request
            // is a no-op for the exited primary and kills the remaining
            // tree; its wait returns immediately on the reaped primary.
            #[cfg(windows)]
            {
                self.request_kill().map_err(|err| {
                    format!(
                        "incomplete tree cleanup: failed to terminate the owned job after the \
                         primary exited: {err}"
                    )
                })?;
                self.wait()
                    .map_err(|err| format!("failed to reap owned process: {err}"))?;
            }
            return Ok(());
        }
        let termination = self.request_kill().map_err(|err| err.to_string());
        if let Err(termination_err) = termination {
            return Err(self.failed_termination_fallback(termination_err));
        }
        let reap = self
            .wait()
            .map_err(|err| format!("failed to reap owned process: {err}"));
        reap.map(|_| ())
    }

    /// Best-effort cleanup after the tree-termination request was refused:
    /// re-issue the kill through the direct-child fallback path and reap
    /// the primary within a fixed budget. The fallback reaps the direct
    /// child but cannot certify that every descendant stopped, so the
    /// caller always receives the `incomplete tree cleanup` error carrying
    /// the fallback evidence — never a plain termination failure and never
    /// an `Ok` that would imply confirmed containment.
    fn failed_termination_fallback(&mut self, termination_err: String) -> String {
        let fallback_kill = self.kill().map_err(|err| err.to_string());
        let reaped = match &fallback_kill {
            // The fallback kill reaps the direct child itself; the bounded
            // poll only confirms it. Without a successful kill request only
            // a natural exit counts, so a single probe avoids stalling the
            // caller on a process nothing has asked to die.
            Ok(()) => self.reap_within(FALLBACK_REAP_BUDGET),
            Err(_) => matches!(self.try_wait(), Ok(Some(_))),
        };
        let fallback = match fallback_kill {
            Ok(()) => "succeeded".to_string(),
            Err(err) => format!("failed: {err}"),
        };
        format!(
            "incomplete tree cleanup: failed to terminate owned process: {termination_err}; \
             direct-child fallback kill {fallback}; primary reaped within budget: {reaped}"
        )
    }

    /// Bounded reap: poll the direct child until it exits or the budget
    /// expires. Never blocks past the budget, so a process that ignores
    /// the termination cannot hang the cleanup reporter.
    fn reap_within(&mut self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        let mut backoff = PollBackoff::new();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    backoff.sleep(Some(deadline));
                }
                Err(_) => return false,
            }
        }
    }

    /// Direct-child kill request without tree scope or reaping.
    ///
    /// This is only the narrow fallback for a failed tree-termination
    /// request; ownership paths must prefer [`OwnedProcess::terminate_tree`].
    /// On Windows the wrapper stack re-issues the Job Object termination
    /// request and then waits the tree out; on other platforms this kills
    /// and reaps the direct child.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
}

/// Stop a rustup proxy (`cargo`, `rustc`) from installing a toolchain while
/// ripr probes it (#4734). rustup 1.28.1+ auto-installs the toolchain a
/// `rust-toolchain.toml` pins when it is missing, so a bare `cargo --version`
/// or `cargo metadata --offline` in such a checkout downloads a whole
/// toolchain. With auto-install off, rustup fails fast and names the missing
/// toolchain; the probe then reports that instead of installing software.
/// Older rustup releases ignore the variable.
pub(crate) fn forbid_rustup_auto_install(command: &mut Command) {
    command.env("RUSTUP_AUTO_INSTALL", "0");
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(deadline) = self.bounded_drop_until {
            self.drop_until(deadline);
            return;
        }
        // Kill-on-close semantics live at this boundary: whichever path
        // releases the owner — normal completion, timeout, cancellation,
        // wait failure, or panic unwinding — terminates the owned tree and
        // reaps the direct child first. Both steps are best-effort here;
        // typed evidence belongs to `terminate_tree`, which every explicit
        // termination path uses. An already-reaped child is left alone on
        // non-Windows targets, and a failed kill is not followed by an
        // unbounded wait on a process that refused to die.
        if self.child.try_wait().is_ok_and(|status| status.is_some()) {
            // Even though the primary exited, the job does not kill on
            // close: descendants it spawned may still hold the job, so
            // ownership release still issues the termination request and
            // reaps the (already exited) primary before dropping the job
            // handle.
            #[cfg(windows)]
            {
                if self.request_kill().is_ok() {
                    let _ = self.wait();
                }
            }
            return;
        }
        if self.request_kill().is_ok() {
            #[cfg(not(windows))]
            if self.bounded_drop {
                let _ = self.reap_within(FALLBACK_REAP_BUDGET);
                return;
            }
            let _ = self.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::process::Stdio;

    #[cfg(windows)]
    fn platform_sleep_command(seconds: u32) -> Command {
        #[cfg(windows)]
        {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-Command",
                &format!("Start-Sleep -Seconds {seconds}"),
            ]);
            command
        }
        #[cfg(not(windows))]
        {
            let mut command = Command::new("sleep");
            command.arg(seconds.to_string());
            command
        }
    }

    #[cfg(windows)]
    fn alive_on_windows(pid: u32) -> bool {
        #[cfg(windows)]
        {
            Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
                    ),
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            false
        }
    }

    /// #5348: the child-exit poll starts at 1 ms and doubles to the 50 ms
    /// ceiling. Negative experiment: a constant 50 ms schedule (the
    /// pre-#5348 `WAIT_POLL_INTERVAL`) fails the first-delay assertion.
    #[test]
    fn poll_backoff_starts_at_one_millisecond_and_caps_at_the_ceiling() {
        let mut backoff = PollBackoff::new();
        let delays: Vec<Duration> = (0..10).map(|_| backoff.next_delay()).collect();
        assert_eq!(delays[0], Duration::from_millis(1));
        assert_eq!(
            &delays[..6],
            &[1, 2, 4, 8, 16, 32].map(Duration::from_millis),
            "schedule must double from 1 ms"
        );
        assert!(
            delays[6..]
                .iter()
                .all(|delay| *delay == POLL_BACKOFF_CEILING)
        );
    }

    /// A deadline already in the past makes the backoff sleep return at
    /// once instead of sleeping the scheduled delay past it.
    #[test]
    fn poll_backoff_sleep_never_passes_the_deadline() {
        let mut backoff = PollBackoff::new();
        for _ in 0..8 {
            let _ = backoff.next_delay();
        }
        let started = Instant::now();
        backoff.sleep(Some(started));
        assert!(started.elapsed() < POLL_BACKOFF_CEILING / 2);
    }

    /// Normal completion, captured output and pipe teardown through the
    /// owner (native control 1: normal exit and complete output).
    #[test]
    fn owned_process_completes_and_captures_output() -> Result<(), String> {
        let mut command = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        if cfg!(windows) {
            command.args(["/C", "echo ripr-owned-ok"]);
        } else {
            command.args(["-c", "echo ripr-owned-ok"]);
        }
        command.stdin(Stdio::null()).stdout(Stdio::piped());
        let mut owned = OwnedProcess::spawn(command).map_err(|err| format!("spawn: {err}"))?;
        let mut stdout = owned
            .stdout_pipe()
            .take()
            .ok_or_else(|| "stdout pipe missing".to_string())?;
        let status = owned.wait().map_err(|err| format!("wait: {err}"))?;
        let mut captured = String::new();
        stdout
            .read_to_string(&mut captured)
            .map_err(|err| format!("read: {err}"))?;
        drop(owned);
        if !status.success() {
            return Err(format!("child should exit successfully, got {status}"));
        }
        if !captured.contains("ripr-owned-ok") {
            return Err(format!(
                "captured stdout should carry the marker: {captured}"
            ));
        }
        Ok(())
    }

    /// Environment and working-directory configuration survive the owned
    /// spawn (native control 11: arguments/environment fidelity).
    #[test]
    fn owned_process_preserves_environment_and_directory() -> Result<(), String> {
        let mut command = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        if cfg!(windows) {
            command.args(["/C", "echo %RIPR_OWNER_TEST_MARKER%"]);
        } else {
            command.args(["-c", "echo $RIPR_OWNER_TEST_MARKER"]);
        }
        command
            .env("RIPR_OWNER_TEST_MARKER", "env-fidelity-ok")
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        let mut owned = OwnedProcess::spawn(command).map_err(|err| format!("spawn: {err}"))?;
        let mut stdout = owned
            .stdout_pipe()
            .take()
            .ok_or_else(|| "stdout pipe missing".to_string())?;
        let status = owned.wait().map_err(|err| format!("wait: {err}"))?;
        let mut captured = String::new();
        stdout
            .read_to_string(&mut captured)
            .map_err(|err| format!("read: {err}"))?;
        drop(owned);
        if !status.success() || !captured.contains("env-fidelity-ok") {
            return Err(format!(
                "owned spawn should preserve child environment (status {status}): {captured}"
            ));
        }
        Ok(())
    }

    /// A still-running owned child is terminated and reaped when the owner
    /// is dropped without an explicit wait — the kill-on-close owner
    /// boundary (Windows host-verified containment).
    #[cfg(windows)]
    #[test]
    fn owner_drop_terminates_a_still_running_child() -> Result<(), String> {
        let command = platform_sleep_command(60);
        let pid;
        {
            let owned = OwnedProcess::spawn(command).map_err(|err| format!("spawn: {err}"))?;
            pid = owned.id();
            // Give the child one poll window to reach Start-Sleep so the
            // later liveness check observes the terminated process, not a
            // process that had not fully started.
            std::thread::sleep(Duration::from_millis(500));
            drop(owned);
        }
        if alive_on_windows(pid) {
            return Err(format!("owned child {pid} persisted past owner drop"));
        }
        Ok(())
    }

    /// A descendant created by the owned child is terminated by
    /// `terminate_tree` even when it holds inherited pipe handles, and the
    /// call returns after the direct child is reaped (native controls 3-5).
    #[cfg(windows)]
    #[test]
    fn terminate_tree_kills_pipe_inheriting_descendants() -> Result<(), String> {
        let marker_path =
            std::env::temp_dir().join(format!("ripr-owner-descendant-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&marker_path);
        let marker_path_text = marker_path.display().to_string().replace('\'', "''");
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            &format!(
                "$p = Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 60') -NoNewWindow -PassThru; Set-Content -LiteralPath '{marker_path_text}' -Value $p.Id; Wait-Process -Id $p.Id"
            ),
        ]);
        command.stdin(Stdio::null()).stdout(Stdio::piped());
        let mut owned = OwnedProcess::spawn(command).map_err(|err| format!("spawn: {err}"))?;
        // Wait for the marker so the descendant demonstrably exists before
        // termination; a missing marker after a generous window is a setup
        // failure, not a containment success.
        let descendant_pid = wait_for_descendant_marker(&marker_path)?;
        let termination = owned.terminate_tree();
        let _ = std::fs::remove_file(&marker_path);
        termination?;
        if alive_on_windows(descendant_pid) {
            return Err(format!(
                "descendant {descendant_pid} persisted past terminate_tree; tree containment not established"
            ));
        }
        Ok(())
    }

    /// Termination of an owned tree leaves unrelated same-name processes
    /// alone (native control 10).
    #[cfg(windows)]
    #[test]
    fn terminate_tree_leaves_unrelated_processes_alive() -> Result<(), String> {
        let mut unrelated_command = platform_sleep_command(5);
        let mut unrelated = unrelated_command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("spawn unrelated: {err}"))?;
        let unrelated_pid = unrelated.id();

        let owned_result = OwnedProcess::spawn(platform_sleep_command(60));
        let mut owned = match owned_result {
            Ok(owned) => owned,
            Err(err) => {
                let _ = unrelated.kill();
                let _ = unrelated.wait();
                return Err(format!("spawn owned: {err}"));
            }
        };
        std::thread::sleep(Duration::from_millis(500));
        let termination = owned.terminate_tree();

        let unrelated_alive = alive_on_windows(unrelated_pid);
        let _ = unrelated.kill();
        let _ = unrelated.wait();
        termination?;
        if !unrelated_alive {
            return Err("terminate_tree terminated an unrelated same-name process".to_string());
        }
        Ok(())
    }

    /// Poll until the marker holds a whole PID. `Set-Content` creates the
    /// file before writing it, so an early read can see it empty or partial;
    /// that is not yet a marker, and stopping on it failed the setup rather
    /// than the containment under test. The window covers two cold
    /// PowerShell starts on a loaded runner; the last observation is kept so
    /// a setup failure still says what was seen.
    #[cfg(windows)]
    fn wait_for_descendant_marker(marker_path: &std::path::Path) -> Result<u32, String> {
        let mut last = "marker not written".to_string();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match std::fs::read_to_string(marker_path) {
                Ok(text) => match text.trim().parse::<u32>() {
                    Ok(pid) => return Ok(pid),
                    Err(err) => last = format!("marker {:?} not a PID yet: {err}", text.trim()),
                },
                Err(err) => last = format!("marker not written: {err}"),
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!("descendant PID marker after 30s: {last}"))
    }

    /// Spawn an owned primary that starts a long-lived descendant, records
    /// the descendant PID, and exits immediately on its own. The returned
    /// descendant is demonstrably alive while the owned primary has already
    /// exited — the early-exit containment setup under review.
    #[cfg(windows)]
    fn spawn_exited_primary_with_descendant(
        marker_path: &std::path::Path,
    ) -> Result<(OwnedProcess, u32), String> {
        let marker_path_text = marker_path.display().to_string().replace('\'', "''");
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            &format!(
                "$p = Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 60') -NoNewWindow -PassThru; \
                 Set-Content -LiteralPath '{marker_path_text}' -Value $p.Id; exit 0"
            ),
        ]);
        command.stdin(Stdio::null()).stdout(Stdio::null());
        let mut owned = OwnedProcess::spawn(command).map_err(|err| format!("spawn: {err}"))?;
        let descendant_pid = wait_for_descendant_marker(marker_path)?;
        // Observe the primary's own exit before the containment claim: the
        // descendant must outlive a primary that already exited.
        let mut primary_exited = false;
        for _ in 0..100 {
            if matches!(owned.try_wait(), Ok(Some(_))) {
                primary_exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !primary_exited {
            return Err("primary did not exit after writing the descendant marker".to_string());
        }
        if !alive_on_windows(descendant_pid) {
            return Err(format!(
                "setup failure: descendant {descendant_pid} was not alive after the primary exited"
            ));
        }
        Ok((owned, descendant_pid))
    }

    /// Bounded liveness confirmation: the descendant must disappear within
    /// the window after ownership release; a survivor past it is the
    /// containment failure under review, not an observation lag.
    #[cfg(windows)]
    fn assert_descendant_terminated(descendant_pid: u32, boundary: &str) -> Result<(), String> {
        for _ in 0..30 {
            if !alive_on_windows(descendant_pid) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "descendant {descendant_pid} persisted past {boundary}; early-exit tree containment not established"
        ))
    }

    /// The primary exits successfully on its own while a descendant keeps
    /// running: ending ownership must still terminate the descendant even
    /// though no explicit termination was requested — the early-exit
    /// `Drop` arm (KILL_ON_JOB_CLOSE is deliberately off).
    #[cfg(windows)]
    #[test]
    fn owner_drop_kills_descendants_after_the_primary_exits() -> Result<(), String> {
        let marker_path =
            std::env::temp_dir().join(format!("ripr-owner-exited-drop-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&marker_path);
        let (owned, descendant_pid) = spawn_exited_primary_with_descendant(&marker_path)?;
        let _ = std::fs::remove_file(&marker_path);
        drop(owned);
        assert_descendant_terminated(descendant_pid, "owner drop after the primary exited")
    }

    /// `terminate_tree` on a tree whose primary already exited still
    /// terminates the surviving descendant and reports success — the
    /// early-exit `terminate_tree` arm.
    #[cfg(windows)]
    #[test]
    fn terminate_tree_after_primary_exit_kills_descendants() -> Result<(), String> {
        let marker_path =
            std::env::temp_dir().join(format!("ripr-owner-exited-term-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&marker_path);
        let (mut owned, descendant_pid) = spawn_exited_primary_with_descendant(&marker_path)?;
        let _ = std::fs::remove_file(&marker_path);
        owned.terminate_tree()?;
        assert_descendant_terminated(descendant_pid, "terminate_tree after the primary exited")
    }
}
