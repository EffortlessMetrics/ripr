//! Complete-only capped byte capture with qualified group and endpoint ownership.

use std::any::Any;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Explicit execution budgets; these values confer no complete-analysis authority.
pub struct CompleteCaptureBudget {
    #[cfg(target_os = "linux")]
    timeout: Duration,
    #[cfg(target_os = "linux")]
    stdin_bytes: usize,
    #[cfg(target_os = "linux")]
    stdout_bytes: usize,
    #[cfg(target_os = "linux")]
    stderr_bytes: usize,
}
impl CompleteCaptureBudget {
    /// Supply the caller's finite input and output admissions.
    pub fn new(
        timeout: Duration,
        stdin_bytes: usize,
        stdout_bytes: usize,
        stderr_bytes: usize,
    ) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                timeout,
                stdin_bytes,
                stdout_bytes,
                stderr_bytes,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (timeout, stdin_bytes, stdout_bytes, stderr_bytes);
            Self {}
        }
    }
}

/// A one-shot combined observation; neither request data nor group settlement can create it.
pub struct CompleteCaptureReceipt {
    #[cfg(target_os = "linux")]
    group: super::QualifiedGroupSettlement,
    lease: Arc<dyn Any + Send + Sync>,
}
impl CompleteCaptureReceipt {
    /// Settled primary status data; this does not establish successful analysis.
    pub fn settled_status(&self) -> Option<ExitStatus> {
        #[cfg(target_os = "linux")]
        {
            Some(self.group.status())
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
    /// Initial observed worker identity data; this grants no signaling authority.
    #[cfg(target_os = "linux")]
    pub fn observed_worker(&self) -> super::ObservedProcessIdentity {
        self.group.observed_worker()
    }
    /// Initial observed parent identity data; this grants no signaling authority.
    #[cfg(target_os = "linux")]
    pub fn observed_parent(&self) -> super::ObservedProcessIdentity {
        self.group.observed_parent()
    }
    /// Compare actual custody identity, without treating a digest or path as a lease.
    pub fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let erased: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &erased)
    }
}

/// Complete captured bytes and actual process observations.
pub struct CompleteCapturedBytes {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    duration: Duration,
    timed_out: bool,
    receipt: CompleteCaptureReceipt,
}
impl CompleteCapturedBytes {
    /// Consume bytes and the unique combined receipt.
    pub fn into_parts(
        self,
    ) -> (
        ExitStatus,
        Vec<u8>,
        Vec<u8>,
        Duration,
        bool,
        CompleteCaptureReceipt,
    ) {
        (
            self.status,
            self.stdout,
            self.stderr,
            self.duration,
            self.timed_out,
            self.receipt,
        )
    }
}

type FailedCaptureObservation = (ExitStatus, Vec<u8>, Vec<u8>, Duration, bool);

struct CaptureFailureState {
    receipt: Option<CompleteCaptureReceipt>,
    observation: Option<FailedCaptureObservation>,
    timeout_only: bool,
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    terminal: Option<TerminalFailureEvidence>,
}

#[cfg(target_os = "linux")]
struct CaptureCustody {
    setup: Option<super::GroupSetupFailure>,
    group: Option<super::QualifiedGroupOwner>,
}
#[cfg(target_os = "linux")]
impl CaptureCustody {
    fn setup(failure: super::GroupSetupFailure) -> Self {
        Self {
            setup: Some(failure),
            group: None,
        }
    }

    fn group(owner: super::QualifiedGroupOwner) -> Self {
        Self {
            setup: None,
            group: Some(owner),
        }
    }

    fn retained_process_count(&self) -> usize {
        self.setup
            .as_ref()
            .map_or(0, super::GroupSetupFailure::retained_process_count)
            + self
                .group
                .as_ref()
                .map_or(0, super::QualifiedGroupOwner::retained_process_count)
    }
}

/// Capture failure retaining custody even when combined cleanup is unconfirmed.
pub struct CompleteCaptureError {
    message: String,
    // Exactly one pre-admitted state entry; no failure-time allocation is needed.
    state: Option<Box<[CaptureFailureState]>>,
    #[cfg(target_os = "linux")]
    custody: Vec<CaptureCustody>,
    lease: Arc<dyn Any + Send + Sync>,
}
impl CompleteCaptureError {
    fn state(&self) -> Option<&CaptureFailureState> {
        self.state.as_deref().and_then(<[_]>::first)
    }

    fn state_mut(&mut self) -> Option<&mut CaptureFailureState> {
        self.state.as_deref_mut().and_then(<[_]>::first_mut)
    }

    /// Original capture failure and any cleanup diagnostics.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Read actual failed wait data, without admitting captured output.
    pub fn observed_outcome(&self) -> Option<(ExitStatus, Duration, bool)> {
        self.state()
            .and_then(|state| state.observation.as_ref())
            .map(|parts| (parts.0, parts.3, parts.4))
    }
    /// Consume failed observation bytes at most once; this grants no cleanup or success authority.
    pub fn take_failed_observation(&mut self) -> Option<FailedCaptureObservation> {
        self.state_mut().and_then(|state| state.observation.take())
    }
    /// Whether an actual timeout is the sole failure, for explicit legacy result conversion.
    pub fn is_timeout_only(&self) -> bool {
        self.state().is_some_and(|state| state.timeout_only)
    }
    /// Transfer confirmed combined cleanup at most once; this never admits output.
    pub fn take_cleanup_receipt(&mut self) -> Option<CompleteCaptureReceipt> {
        self.state_mut().and_then(|state| state.receipt.take())
    }
    /// Compare the actual retained custody identity.
    pub fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let erased: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &erased)
    }
}
impl std::fmt::Debug for CompleteCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("CompleteCaptureError");
        debug.field("message", &self.message).field(
            "combined_cleanup_confirmed",
            &self.state().is_some_and(|state| state.receipt.is_some()),
        );
        #[cfg(target_os = "linux")]
        debug.field(
            "unconfirmed_process_handles",
            &self
                .custody
                .iter()
                .map(CaptureCustody::retained_process_count)
                .sum::<usize>(),
        );
        debug.finish()
    }
}
impl std::fmt::Display for CompleteCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for CompleteCaptureError {}

/// A checked negative disposition; it never admits output or successful analysis.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteFailedClosed {
    group: super::QualifiedGroupSettlement,
    lease: Arc<dyn Any + Send + Sync>,
    release: TerminalReleaseObservation,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl CompleteFailedClosed {
    pub(crate) fn original_deadline(&self) -> Instant {
        self.release.original_deadline
    }

    pub(crate) fn settled_status(&self) -> ExitStatus {
        self.group.status()
    }

    pub(crate) fn observed_worker(&self) -> super::ObservedProcessIdentity {
        self.group.observed_worker()
    }

    pub(crate) fn observed_parent(&self) -> super::ObservedProcessIdentity {
        self.group.observed_parent()
    }

    pub(crate) fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let erased: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &erased)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
#[derive(Clone, Copy)]
struct TerminalReleaseObservation {
    original_deadline: Instant,
    before: Instant,
    after: Instant,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl TerminalReleaseObservation {
    fn valid_for(self, original_deadline: Instant) -> bool {
        self.original_deadline == original_deadline
            && self.before <= self.after
            && self.before < original_deadline
            && self.after < original_deadline
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
struct TerminalFailureEvidence {
    group_only: Option<super::QualifiedGroupSettlement>,
    release: Option<TerminalReleaseObservation>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
enum TakenTerminalEvidence {
    Combined(CompleteCaptureReceipt),
    GroupOnly(super::QualifiedGroupSettlement),
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
thread_local! {
    static TERMINAL_TAKE_DEADLINE_BARRIER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn with_terminal_take_deadline_barrier<T>(work: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            TERMINAL_TAKE_DEADLINE_BARRIER.with(|barrier| barrier.set(self.0));
        }
    }
    let previous = TERMINAL_TAKE_DEADLINE_BARRIER.with(|barrier| barrier.replace(true));
    let _restore = Restore(previous);
    work()
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn take_closed_failure(
    failure: &mut CompleteCaptureError,
    original_deadline: Instant,
) -> Option<CompleteFailedClosed> {
    if Instant::now() >= original_deadline {
        return None;
    }
    let lease = failure.lease.clone();
    let state = failure.state_mut()?;
    let release = state.terminal.as_ref()?.release?;
    if !release.valid_for(original_deadline) {
        return None;
    }
    let taken = match state.receipt.take() {
        Some(receipt) => TakenTerminalEvidence::Combined(receipt),
        None => TakenTerminalEvidence::GroupOnly(state.terminal.as_mut()?.group_only.take()?),
    };
    if TERMINAL_TAKE_DEADLINE_BARRIER.with(|barrier| barrier.replace(false)) {
        while Instant::now() < original_deadline {
            std::thread::yield_now();
        }
    }
    let belongs = match &taken {
        TakenTerminalEvidence::Combined(receipt) => receipt.group.belongs_to_current_parent(),
        TakenTerminalEvidence::GroupOnly(group) => group.belongs_to_current_parent(),
    };
    if Instant::now() >= original_deadline || !belongs {
        match taken {
            TakenTerminalEvidence::Combined(receipt) => state.receipt = Some(receipt),
            TakenTerminalEvidence::GroupOnly(group) => {
                state.terminal.as_mut()?.group_only = Some(group);
            }
        }
        return None;
    }
    let (group, lease) = match taken {
        TakenTerminalEvidence::Combined(receipt) => (receipt.group, receipt.lease),
        TakenTerminalEvidence::GroupOnly(group) => (group, lease),
    };
    Some(CompleteFailedClosed {
        group,
        lease,
        release,
    })
}

/// One pre-existing, invocation-local owner for a capture's terminal failure.
///
/// Reports borrow this owner. A discarded report therefore cannot discard the
/// actual child, group, signal helper or caller lease. The final owner still
/// requires a genuine terminal endpoint; dropping it is not confirmed cleanup.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteTerminalCustodian<L: Any + Send + Sync> {
    held_deadline: Instant,
    lease: Arc<L>,
    attempted: bool,
    failure: Option<CompleteCaptureError>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> CompleteTerminalCustodian<L> {
    pub(crate) fn new(held_deadline: Instant, lease: Arc<L>) -> Result<Self, String> {
        if Instant::now() >= held_deadline {
            return Err("terminal custody admission reached its original deadline".to_string());
        }
        Ok(Self {
            held_deadline,
            lease,
            attempted: false,
            failure: None,
        })
    }

    pub(crate) fn original_deadline(&self) -> Instant {
        self.held_deadline
    }

    pub(crate) fn failure(&self) -> Option<&CompleteCaptureError> {
        self.failure.as_ref()
    }

    pub(crate) fn matches_lease(&self, lease: &Arc<L>) -> bool {
        Arc::ptr_eq(&self.lease, lease)
    }

    pub(crate) fn retained_process_count(&self) -> usize {
        self.failure.as_ref().map_or(0, |failure| {
            failure
                .custody
                .iter()
                .map(CaptureCustody::retained_process_count)
                .sum()
        })
    }

    pub(crate) fn take_cleanup_receipt(&mut self) -> Option<CompleteCaptureReceipt> {
        self.failure
            .as_mut()
            .and_then(|failure| take_timely_terminal_receipt(failure, self.held_deadline))
    }

    /// Consume only a genuine timely negative disposition, retaining the first error.
    pub(crate) fn try_closeout_failure(
        &mut self,
    ) -> Result<CompleteFailedClosed, CompleteTerminalFailure<'_>> {
        if let Some(closed) = self
            .failure
            .as_mut()
            .and_then(|failure| take_closed_failure(failure, self.held_deadline))
        {
            return Ok(closed);
        }
        let reason = self.failure.is_none().then_some(
            "no failed capture has a checked terminal disposition; custody unconfirmed",
        );
        Err(CompleteTerminalFailure {
            reason,
            capture: self.failure.as_mut(),
            original_deadline: self.held_deadline,
        })
    }

    pub(crate) fn take_failed_observation(&mut self) -> Option<FailedCaptureObservation> {
        self.failure
            .as_mut()
            .and_then(CompleteCaptureError::take_failed_observation)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
fn take_timely_terminal_receipt(
    failure: &mut CompleteCaptureError,
    original_deadline: Instant,
) -> Option<CompleteCaptureReceipt> {
    if Instant::now() >= original_deadline {
        return None;
    }
    let state = failure.state_mut()?;
    let receipt = state.receipt.take()?;
    if Instant::now() >= original_deadline {
        state.receipt = Some(receipt);
        return None;
    }
    Some(receipt)
}

/// Borrowed reporting only; this value contains no owned process custody.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteTerminalFailure<'a> {
    reason: Option<&'static str>,
    capture: Option<&'a mut CompleteCaptureError>,
    original_deadline: Instant,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl CompleteTerminalFailure<'_> {
    pub(crate) fn message(&self) -> &str {
        self.reason.unwrap_or_else(|| {
            self.capture.as_deref().map_or(
                "terminal capture failure unavailable; custody unconfirmed",
                CompleteCaptureError::message,
            )
        })
    }

    pub(crate) fn capture_error(&self) -> Option<&CompleteCaptureError> {
        self.capture.as_deref()
    }

    pub(crate) fn observed_outcome(&self) -> Option<(ExitStatus, Duration, bool)> {
        self.capture
            .as_deref()
            .and_then(CompleteCaptureError::observed_outcome)
    }

    pub(crate) fn take_cleanup_receipt(&mut self) -> Option<CompleteCaptureReceipt> {
        self.capture
            .as_deref_mut()
            .and_then(|failure| take_timely_terminal_receipt(failure, self.original_deadline))
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl std::fmt::Debug for CompleteTerminalFailure<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompleteTerminalFailure")
            .field("message", &self.message())
            .field("capture", &self.capture)
            .finish()
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl std::fmt::Display for CompleteTerminalFailure<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl std::error::Error for CompleteTerminalFailure<'_> {}

/// Source-single complete capture adapter; ordinary capture helpers are unchanged.
pub struct CompleteByteCapture;
impl CompleteByteCapture {
    /// Run once, retaining any actual refusal in an owner outside the failing call.
    ///
    /// The fixed internal callback cannot unwind caller code before the transfer.
    /// Neither the report nor this test-only boundary grants execution or cleanup.
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    pub(crate) fn capture_with_terminal_custody<'a, L: Any + Send + Sync>(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        custodian: &'a mut CompleteTerminalCustodian<L>,
    ) -> Result<CompleteCapturedBytes, CompleteTerminalFailure<'a>> {
        if custodian.attempted {
            return Err(CompleteTerminalFailure {
                reason: Some("terminal capture was already attempted; custody retained"),
                capture: custodian.failure.as_mut(),
                original_deadline: custodian.held_deadline,
            });
        }
        // Claim the sole slot before any fallible capture admission or spawn.
        custodian.attempted = true;
        match Self::capture_with_deadline(
            command,
            source,
            env_remove,
            execution,
            custodian.held_deadline,
            custodian.lease.clone(),
            |_| {},
        ) {
            Ok(output) => Ok(output),
            Err(error) => {
                // A by-value slot: this transfer allocates nothing and never
                // reconstructs an owner from a PID, receipt, path or saved data.
                custodian.failure = Some(error);
                Err(CompleteTerminalFailure {
                    reason: None,
                    capture: custodian.failure.as_mut(),
                    original_deadline: custodian.held_deadline,
                })
            }
        }
    }

    /// Use the caller's already-held execution start and absolute cutoff.
    ///
    /// This tighter test-only boundary cannot reset the original custody clock.
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    pub(crate) fn capture_with_terminal_custody_until<'a, L: Any + Send + Sync>(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        started: Instant,
        execution_deadline: Instant,
        custodian: &'a mut CompleteTerminalCustodian<L>,
    ) -> Result<CompleteCapturedBytes, CompleteTerminalFailure<'a>> {
        if custodian.attempted {
            return Err(CompleteTerminalFailure {
                reason: Some("terminal capture was already attempted; custody retained"),
                capture: custodian.failure.as_mut(),
                original_deadline: custodian.held_deadline,
            });
        }
        custodian.attempted = true;
        let lease: Arc<dyn Any + Send + Sync> = custodian.lease.clone();
        match linux::capture_with_execution_window(
            command,
            source,
            env_remove,
            execution,
            (custodian.held_deadline, started, execution_deadline),
            lease,
        ) {
            Ok(output) => Ok(output),
            Err(error) => {
                custodian.failure = Some(error);
                Err(CompleteTerminalFailure {
                    reason: None,
                    capture: custodian.failure.as_mut(),
                    original_deadline: custodian.held_deadline,
                })
            }
        }
    }

    /// Run a qualified capture while its owner retains the supplied custody lease.
    /// The caller authenticates that lease and binds the worker invocation.
    pub fn capture<L: Any + Send + Sync>(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        lease: Arc<L>,
        settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        let lease: Arc<dyn Any + Send + Sync> = lease;
        #[cfg(target_os = "linux")]
        {
            linux::capture(command, source, env_remove, execution, lease, settled)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let (_, error_context) = execution;
            let _ = (command, source, env_remove, settled);
            Err(CompleteCaptureError {
                message: format!(
                    "complete byte capture for {error_context} requires qualified Linux group ownership; spawn refused"
                ),
                state: None,
                lease,
            })
        }
    }
    /// Capture under the caller's original total custody ceiling.
    /// The execution timeout remains an independent, possibly tighter phase bound.
    /// Checks are cooperative and cannot preempt a blocked syscall or callback.
    pub fn capture_with_deadline<L: Any + Send + Sync>(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        held_deadline: Instant,
        lease: Arc<L>,
        settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        let lease: Arc<dyn Any + Send + Sync> = lease;
        #[cfg(target_os = "linux")]
        {
            linux::capture_with_held(
                command,
                source,
                env_remove,
                execution,
                Some(held_deadline),
                lease,
                settled,
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            let (_, error_context) = execution;
            let _ = (command, source, env_remove, held_deadline, settled);
            Err(CompleteCaptureError {
                message: format!(
                    "complete byte capture for {error_context} requires qualified Linux group ownership; spawn refused"
                ),
                state: None,
                lease,
            })
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::process_owner::qualified_capture::QualifiedGroupOwner;
    use std::io::{ErrorKind, Read, Write};
    use std::net::Shutdown;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Instant;

    const POST_KILL_DRAIN_GRACE: Duration = Duration::from_secs(5);
    const MAX_STREAM_BYTES: usize = 256 * 1024 * 1024;
    const PUMP_SLICE: Duration = Duration::from_millis(5);
    const PUMP_ROUNDS: usize = 64;
    const CHUNK_BYTES: usize = 4096;

    #[cfg(test)]
    #[derive(Default)]
    struct Hooks {
        fail_pair: Option<&'static str>,
        pair_attempts: usize,
        delay_first_wait: Option<Duration>,
        delay_admission_once: Option<Duration>,
        delay_endpoint_once: Option<Duration>,
        delayed_endpoints: usize,
        hold_writer: Option<&'static str>,
        held_writer: Option<UnixStream>,
    }
    #[cfg(test)]
    thread_local! {
        static HOOKS: std::cell::RefCell<Hooks> = std::cell::RefCell::new(Hooks::default());
    }

    fn pair(name: &'static str) -> Result<(UnixStream, Stdio), String> {
        #[cfg(test)]
        HOOKS.with(|hooks| {
            let mut hooks = hooks.borrow_mut();
            hooks.pair_attempts += 1;
            if hooks.fail_pair == Some(name) {
                hooks.fail_pair = None;
                return Err(format!("prepare {name} socket: injected pair failure"));
            }
            Ok(())
        })?;
        let (parent, child) =
            UnixStream::pair().map_err(|error| format!("prepare {name} socket: {error}"))?;
        // The endpoints have independent file descriptions. The worker endpoint
        // retains blocking stdio; only the controller endpoint is nonblocking.
        parent
            .set_nonblocking(true)
            .map_err(|error| format!("prepare nonblocking {name}: {error}"))?;
        #[cfg(test)]
        HOOKS.with(|hooks| {
            let mut hooks = hooks.borrow_mut();
            if hooks.hold_writer == Some(name) {
                hooks.hold_writer = None;
                hooks.held_writer =
                    Some(child.try_clone().map_err(|error| {
                        format!("retain native {name} writer control: {error}")
                    })?);
            }
            Ok::<_, String>(())
        })?;
        Ok((parent, Stdio::from(OwnedFd::from(child))))
    }

    struct OutputCapture {
        bytes: Vec<u8>,
        eof: bool,
        failed: bool,
        retain: bool,
        error: Option<String>,
    }
    impl OutputCapture {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                eof: false,
                failed: false,
                retain: true,
                error: None,
            }
        }

        fn record_failure(&mut self, error: String) {
            self.error = Some(match self.error.take() {
                Some(primary) => format!("{primary}; {error}"),
                None => error,
            });
        }

        // Exactly one read; Interrupted and WouldBlock yield to the fair pump.
        fn step(
            &mut self,
            stream: &mut impl Read,
            name: &str,
            limit: usize,
            scratch: &mut [u8; CHUNK_BYTES],
        ) -> bool {
            if self.eof || self.failed {
                return false;
            }
            match stream.read(scratch) {
                Ok(0) => {
                    self.eof = true;
                    true
                }
                Ok(count) => {
                    // Admission checked cap+1 before spawn. After overflow retain
                    // its original failure and drain, without further growth.
                    let retain = if self.retain {
                        count.min((limit + 1).saturating_sub(self.bytes.len()))
                    } else {
                        0
                    };
                    if retain != 0 {
                        match self.bytes.try_reserve_exact(retain) {
                            Ok(()) => self.bytes.extend_from_slice(&scratch[..retain]),
                            Err(error) => {
                                self.retain = false;
                                self.record_failure(format!("reserve bounded {name}: {error}"));
                            }
                        }
                    }
                    if self.bytes.len() > limit && self.error.is_none() {
                        self.record_failure(format!(
                            "{name} exceeds its {limit}-byte output budget"
                        ));
                    }
                    true
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) =>
                {
                    false
                }
                Err(error) => {
                    self.failed = true;
                    self.record_failure(format!("read bounded {name}: {error}"));
                    false
                }
            }
        }
    }

    struct InputCapture {
        stream: Option<UnixStream>,
        bytes: Vec<u8>,
        offset: usize,
        complete: bool,
        error: Option<std::io::Error>,
    }
    impl InputCapture {
        // One bounded write, or one final half-close. Never restart a partial write.
        fn step(&mut self) -> bool {
            let Some(stream) = self.stream.as_mut() else {
                return false;
            };
            if self.offset == self.bytes.len() {
                let result = stream.shutdown(Shutdown::Write);
                self.complete = result.is_ok();
                if let Err(error) = result {
                    self.error = Some(error);
                }
                drop(self.stream.take());
                return true;
            }
            let end = self
                .offset
                .saturating_add(CHUNK_BYTES)
                .min(self.bytes.len());
            match stream.write(&self.bytes[self.offset..end]) {
                Ok(0) => {
                    self.error = Some(std::io::Error::new(
                        ErrorKind::WriteZero,
                        "bounded stdin write made no progress",
                    ));
                    drop(self.stream.take());
                    false
                }
                Ok(count) => {
                    self.offset += count;
                    true
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) =>
                {
                    false
                }
                Err(error) => {
                    self.error = Some(error);
                    drop(self.stream.take());
                    false
                }
            }
        }
    }

    struct Transport {
        stdout: UnixStream,
        stderr: UnixStream,
        input: Option<InputCapture>,
        stdout_capture: OutputCapture,
        stderr_capture: OutputCapture,
        stdout_limit: usize,
        stderr_limit: usize,
        next_endpoint: usize,
        late_action: bool,
    }
    impl Transport {
        fn terminal(&self) -> bool {
            (self.stdout_capture.eof || self.stdout_capture.failed)
                && (self.stderr_capture.eof || self.stderr_capture.failed)
                && self
                    .input
                    .as_ref()
                    .is_none_or(|input| input.stream.is_none())
        }

        // The slice is a scheduling yield, while the supplied held deadline is
        // an admission boundary. A soft yield retains the next endpoint.
        fn can_step(&mut self, slice: Instant, deadline: Instant) -> bool {
            let now = Instant::now();
            if now >= deadline {
                self.late_action = true;
                return false;
            }
            now < slice
        }

        // At most 64 fair rounds / 192 bounded actions. No endpoint can
        // monopolize supervision or restart a partial input write.
        fn pump(&mut self, deadline: Instant) -> bool {
            let slice = Instant::now() + PUMP_SLICE;
            let mut progress = false;
            let mut scratch = [0u8; CHUNK_BYTES];
            for _ in 0..PUMP_ROUNDS {
                let mut round_progress = false;
                for _ in 0..3 {
                    if !self.can_step(slice, deadline) {
                        return progress;
                    }
                    let endpoint = self.next_endpoint;
                    self.next_endpoint = (self.next_endpoint + 1) % 3;
                    let action_progress = match endpoint {
                        0 => self.stdout_capture.step(
                            &mut self.stdout,
                            "stdout",
                            self.stdout_limit,
                            &mut scratch,
                        ),
                        1 => self.stderr_capture.step(
                            &mut self.stderr,
                            "stderr",
                            self.stderr_limit,
                            &mut scratch,
                        ),
                        _ => self.input.as_mut().is_some_and(|input| input.step()),
                    };
                    #[cfg(test)]
                    if let Some(delay) = HOOKS.with(|hooks| {
                        let mut hooks = hooks.borrow_mut();
                        let delay = hooks.delay_endpoint_once.take();
                        if delay.is_some() {
                            hooks.delayed_endpoints += 1;
                        }
                        delay
                    }) {
                        thread::sleep(delay);
                    }
                    round_progress |= action_progress;
                    progress |= action_progress;
                    if !self.can_step(slice, deadline) {
                        return progress;
                    }
                }
                if !round_progress {
                    break;
                }
            }
            progress
        }
    }

    struct FailureStorage {
        state: Option<Box<[CaptureFailureState]>>,
        custody: Vec<CaptureCustody>,
    }
    impl FailureStorage {
        fn new() -> Self {
            Self {
                state: None,
                custody: Vec::new(),
            }
        }

        fn failure(
            &mut self,
            message: String,
            receipt: Option<CompleteCaptureReceipt>,
            lease: Arc<dyn Any + Send + Sync>,
        ) -> CompleteCaptureError {
            let mut state = self.state.take();
            if let Some(entry) = state.as_deref_mut().and_then(<[_]>::first_mut) {
                entry.receipt = receipt;
            }
            CompleteCaptureError {
                message,
                state,
                custody: std::mem::take(&mut self.custody),
                lease,
            }
        }

        fn admit(&mut self, strict: bool) -> Result<(), String> {
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(1)
                .map_err(|error| format!("reserve complete capture failure state: {error}"))?;
            entries.push(CaptureFailureState {
                receipt: None,
                observation: None,
                timeout_only: false,
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                terminal: None,
            });
            self.state = Some(entries.into_boxed_slice());
            if strict {
                // One possible setup OR group owner, containing at most one signal helper.
                self.custody
                    .try_reserve_exact(1)
                    .map_err(|error| format!("reserve complete capture custody slot: {error}"))?;
            }
            Ok(())
        }
    }

    pub(super) fn capture(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        lease: Arc<dyn Any + Send + Sync>,
        settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        capture_with_held(command, source, env_remove, execution, None, lease, settled)
    }

    fn strict_time(held: Option<Instant>) -> Result<(), String> {
        if held.is_some_and(|deadline| Instant::now() >= deadline) {
            Err("complete capture held custody deadline expired; cleanup unconfirmed".to_string())
        } else {
            Ok(())
        }
    }

    struct CaptureClocks {
        held_deadline: Option<Instant>,
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        execution_window: Option<(Instant, Instant)>,
    }

    pub(super) fn capture_with_held(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        held_deadline: Option<Instant>,
        lease: Arc<dyn Any + Send + Sync>,
        settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        capture_with_clocks(
            command,
            source,
            env_remove,
            execution,
            CaptureClocks {
                held_deadline,
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                execution_window: None,
            },
            lease,
            settled,
        )
    }

    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    pub(super) fn capture_with_execution_window(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        clocks: (Instant, Instant, Instant),
        lease: Arc<dyn Any + Send + Sync>,
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        capture_with_clocks(
            command,
            source,
            env_remove,
            execution,
            CaptureClocks {
                held_deadline: Some(clocks.0),
                execution_window: Some((clocks.1, clocks.2)),
            },
            lease,
            |_| {},
        )
    }

    fn capture_with_clocks(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        clocks: CaptureClocks,
        lease: Arc<dyn Any + Send + Sync>,
        mut settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        let held_deadline = clocks.held_deadline;
        let mut failure_storage = FailureStorage::new();
        // None preserves ordinary validation-before-clock ordering.
        let strict_started = held_deadline.map(|_| Instant::now());
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let strict_started = clocks
            .execution_window
            .map(|window| window.0)
            .or(strict_started);
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        let (program, args) = command;
        let (budget, error_context) = execution;
        let (cwd, input) = source;
        // Validate the complete execution admission before copying stdin or
        // preparing endpoints. These ceilings do not alter ordinary capture.
        if budget.timeout.is_zero() {
            return Err(failure_storage.failure(
                format!("complete byte capture for {error_context} has no deadline"),
                None,
                lease,
            ));
        }
        for (name, limit) in [
            ("stdin", budget.stdin_bytes),
            ("stdout", budget.stdout_bytes),
            ("stderr", budget.stderr_bytes),
        ] {
            if limit > MAX_STREAM_BYTES {
                return Err(failure_storage.failure(
                    format!(
                        "{name} byte budget exceeds the {MAX_STREAM_BYTES}-byte stream ceiling"
                    ),
                    None,
                    lease,
                ));
            }
        }
        for (name, limit) in [
            ("stdout", budget.stdout_bytes),
            ("stderr", budget.stderr_bytes),
        ] {
            if limit.checked_add(1).is_none() {
                return Err(failure_storage.failure(
                    format!("{name} byte budget cannot admit its overflow sentinel"),
                    None,
                    lease,
                ));
            }
        }
        let started = strict_started.unwrap_or_else(Instant::now);
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        if let Some((original_started, execution_deadline)) = clocks.execution_window {
            let now = Instant::now();
            let reserve_end = execution_deadline
                .checked_add(super::super::linux::terminal_settlement_grace())
                .and_then(|end| end.checked_add(POST_KILL_DRAIN_GRACE));
            if original_started > now
                || execution_deadline <= original_started
                || now >= execution_deadline
                || !reserve_end.is_some_and(|end| {
                    held_deadline.is_some_and(|held| end <= held)
                })
            {
                return Err(failure_storage.failure(
                    "absolute execution window is invalid or lacks original settlement and drain reserve"
                        .to_string(),
                    None,
                    lease,
                ));
            }
        }
        let worker_deadline = started.checked_add(budget.timeout).ok_or_else(|| {
            failure_storage.failure(
                format!("complete byte capture for {error_context} deadline overflow"),
                None,
                lease.clone(),
            )
        })?;
        let worker_deadline =
            held_deadline.map_or(worker_deadline, |held| worker_deadline.min(held));
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let worker_deadline = clocks
            .execution_window
            .map_or(worker_deadline, |window| worker_deadline.min(window.1));
        let wait_timeout = budget.timeout;
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let wait_timeout = clocks.execution_window.map_or(wait_timeout, |_| {
            worker_deadline.saturating_duration_since(started)
        });
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        let input = input
            .map(|bytes| {
                if bytes.len() > budget.stdin_bytes {
                    return Err(format!(
                        "stdin exceeds its {}-byte input budget",
                        budget.stdin_bytes
                    ));
                }
                strict_time(held_deadline)?;
                let mut copy = Vec::new();
                copy.try_reserve_exact(bytes.len())
                    .map_err(|error| format!("reserve stdin for {error_context}: {error}"))?;
                strict_time(held_deadline)?;
                copy.extend_from_slice(bytes);
                strict_time(held_deadline)?;
                Ok(copy)
            })
            .transpose()
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        #[cfg(test)]
        if held_deadline.is_some()
            && let Some(delay) = HOOKS.with(|hooks| hooks.borrow_mut().delay_admission_once.take())
        {
            thread::sleep(delay);
        }
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        if Instant::now() >= worker_deadline {
            return Err(failure_storage.failure(
                format!(
                    "complete byte capture for {error_context} admission exceeded its deadline"
                ),
                None,
                lease,
            ));
        }
        // All fallible pair preparation happens before spawning a worker.
        let prepared = (|| {
            strict_time(held_deadline)?;
            let (stdout, stdout_child) = pair("stdout")?;
            strict_time(held_deadline)?;
            let (stderr, stderr_child) = pair("stderr")?;
            strict_time(held_deadline)?;
            let (input, stdin_child) = match input {
                Some(bytes) => {
                    let (stream, child) = pair("stdin")?;
                    strict_time(held_deadline)?;
                    (
                        Some(InputCapture {
                            stream: Some(stream),
                            bytes,
                            offset: 0,
                            complete: false,
                            error: None,
                        }),
                        child,
                    )
                }
                None => (None, Stdio::null()),
            };
            Ok::<_, String>((
                stdout,
                stderr,
                input,
                stdout_child,
                stderr_child,
                stdin_child,
            ))
        })()
        .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        let (stdout, stderr, input, stdout_child, stderr_child, stdin_child) = prepared;
        let mut transport = Transport {
            stdout,
            stderr,
            input,
            stdout_capture: OutputCapture::new(),
            stderr_capture: OutputCapture::new(),
            stdout_limit: budget.stdout_bytes,
            stderr_limit: budget.stderr_bytes,
            next_endpoint: 0,
            late_action: false,
        };
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd);
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        for name in env_remove {
            strict_time(held_deadline)
                .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
            command.env_remove(name);
            strict_time(held_deadline)
                .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        }
        command
            .stdout(stdout_child)
            .stderr(stderr_child)
            .stdin(stdin_child);
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        // Preserve existing input/setup refusal precedence before admitting the
        // fixed error/retention storage. Nothing can spawn without this storage.
        failure_storage
            .admit(held_deadline.is_some())
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        strict_time(held_deadline)
            .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        // Consuming spawn drops the Command and every parent copy of its child
        // endpoints before it returns. No parent I/O helper is ever started.
        if Instant::now() >= worker_deadline {
            return Err(failure_storage.failure(
                format!(
                    "complete byte capture for {error_context} setup exceeded its deadline; spawn refused"
                ),
                None,
                lease,
            ));
        }
        let mut owner = match held_deadline {
            Some(held) => match QualifiedGroupOwner::spawn_with_deadline(command, held) {
                Ok(owner) => owner,
                Err(error) => {
                    let message = format!("failed to run {error_context}: {error}");
                    failure_storage.custody.push(CaptureCustody::setup(error));
                    return Err(failure_storage.failure(message, None, lease));
                }
            },
            None => QualifiedGroupOwner::spawn(command).map_err(|error| {
                failure_storage.failure(
                    format!("failed to run {error_context}: {error}"),
                    None,
                    lease.clone(),
                )
            })?,
        };
        if let Err(error) = strict_time(held_deadline) {
            failure_storage.custody.push(CaptureCustody::group(owner));
            return Err(failure_storage.failure(error, None, lease));
        }
        #[cfg(test)]
        if let Some(delay) = HOOKS.with(|hooks| hooks.borrow_mut().delay_first_wait.take()) {
            // Establish actual EOF before delaying the first native status
            // observation. This exercises the legacy wait's post-deadline
            // nonlive branch without granting complete success from it.
            while !(transport.stdout_capture.eof && transport.stderr_capture.eof)
                && Instant::now() < worker_deadline
            {
                transport.pump(worker_deadline);
                thread::sleep(Duration::from_millis(1));
            }
            if !(transport.stdout_capture.eof && transport.stderr_capture.eof) {
                if held_deadline.is_some() {
                    failure_storage.custody.push(CaptureCustody::group(owner));
                }
                return Err(failure_storage.failure(
                    "late primary control did not establish actual output EOF".to_string(),
                    None,
                    lease,
                ));
            }
            thread::sleep(delay);
        }
        let outcome = owner.wait_supervised(started, wait_timeout, || {
            if Instant::now() < worker_deadline {
                transport.pump(worker_deadline);
            }
            // Keep ordinary group wait/refusal precedence. Endpoint failures are
            // retained and reported after its actual outcome, never as success.
            Ok(())
        });
        let mut errors = Vec::new();
        let observation = match outcome {
            Ok(observation) => {
                let parts = observation.into_parts();
                match strict_time(held_deadline) {
                    Ok(()) => {
                        settled(parts.0);
                        if let Err(clock) = strict_time(held_deadline) {
                            errors.push(clock);
                        }
                    }
                    Err(clock) => errors.push(clock),
                }
                if parts.1 >= wait_timeout && !parts.2 {
                    errors.push(
                        "primary observation exceeded its held execution deadline".to_string(),
                    );
                }
                Some(parts)
            }
            Err(primary) => {
                match owner.abort() {
                    Ok(()) => {
                        let mut callback_clock = None;
                        if let Some(status) = owner.settled_status() {
                            match strict_time(held_deadline) {
                                Ok(()) => {
                                    settled(status);
                                    callback_clock = strict_time(held_deadline).err();
                                }
                                Err(clock) => callback_clock = Some(clock),
                            }
                        }
                        errors.push(primary);
                        if let Some(clock) = callback_clock {
                            errors.push(clock);
                        }
                    }
                    Err(cleanup) => errors.push(format!(
                        "{primary}; qualified group cleanup unconfirmed: {cleanup}"
                    )),
                }
                None
            }
        };
        // One shared grace for all controller endpoints. There are no helpers,
        // completion channels, native TLS assumptions, or joins in this route.
        let deadline = Instant::now() + POST_KILL_DRAIN_GRACE;
        let deadline = held_deadline.map_or(deadline, |held| deadline.min(held));
        while !transport.terminal() && Instant::now() < deadline {
            let progress = transport.pump(deadline);
            if !progress && !transport.terminal() {
                thread::sleep(PUMP_SLICE.min(deadline.saturating_duration_since(Instant::now())));
            }
        }
        let stdout_eof = transport.stdout_capture.eof;
        let stderr_eof = transport.stderr_capture.eof;
        if let Some(error) = transport.stdout_capture.error.take() {
            errors.push(error);
        }
        if !stdout_eof {
            errors
                .push("stdout actual EOF is unconfirmed within shared post-kill grace".to_string());
        }
        if let Some(error) = transport.stderr_capture.error.take() {
            errors.push(error);
        }
        if !stderr_eof {
            errors
                .push("stderr actual EOF is unconfirmed within shared post-kill grace".to_string());
        }
        if let Some(input) = transport.input.as_ref() {
            if let Some(error) = &input.error {
                if !(observation.as_ref().is_some_and(|parts| parts.2)
                    && error.kind() == ErrorKind::BrokenPipe)
                {
                    errors.push(format!("write stdin for {error_context}: {error}"));
                }
            } else if !input.complete {
                errors.push(format!(
                    "stdin completion for {error_context} is unconfirmed"
                ));
            }
        }
        if transport.late_action {
            errors.push("complete endpoint action exceeded its held clock boundary".to_string());
        }
        let before_release = Instant::now() < deadline;
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let terminal_release_before = Instant::now();
        let late_action = transport.late_action;
        let Transport {
            stdout,
            stderr,
            input,
            stdout_capture,
            stderr_capture,
            ..
        } = transport;
        // Local close is ownership release, never an EOF observation. An input
        // failure may authorize cleanup only after actual output EOF/group settle;
        // it can never turn failed capture into successful output.
        drop(stdout);
        drop(stderr);
        drop(input);
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let terminal_release_after = Instant::now();
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let terminal_release = held_deadline.and_then(|original_deadline| {
            let release = TerminalReleaseObservation {
                original_deadline,
                before: terminal_release_before,
                after: terminal_release_after,
            };
            release.valid_for(original_deadline).then_some(release)
        });
        let released_in_time = before_release && Instant::now() < deadline && !late_action;
        let group = owner.take_settlement();
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let mut group_only = group;
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        let mut receipt = if stdout_eof
            && stderr_eof
            && released_in_time
            && group_only
                .as_ref()
                .is_some_and(|group| group.belongs_to_current_parent())
        {
            group_only.take().map(|group| CompleteCaptureReceipt {
                group,
                lease: lease.clone(),
            })
        } else {
            None
        };
        #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
        let mut receipt = if stdout_eof && stderr_eof && released_in_time {
            group
                .filter(|group| group.belongs_to_current_parent())
                .map(|group| CompleteCaptureReceipt {
                    group,
                    lease: lease.clone(),
                })
        } else {
            None
        };
        if let Err(clock) = strict_time(held_deadline) {
            #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
            if let Some(actual) = receipt.take() {
                group_only = Some(actual.group);
            }
            receipt = None;
            errors.push(clock);
        }
        let timed_out = observation.as_ref().is_some_and(|parts| parts.2);
        let mut timeout_only = timed_out && errors.is_empty() && receipt.is_some();
        if timed_out {
            errors.push(format!("complete worker timed out for {error_context}"));
        }
        let observation = observation.map(|(status, _, timed_out)| {
            (
                status,
                stdout_capture.bytes,
                stderr_capture.bytes,
                started.elapsed(),
                timed_out,
            )
        });
        if let Err(clock) = strict_time(held_deadline) {
            #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
            if let Some(actual) = receipt.take() {
                group_only = Some(actual.group);
            }
            receipt = None;
            timeout_only = false;
            errors.push(clock);
        }
        if !errors.is_empty() {
            if held_deadline.is_some() && owner.retained_process_count() != 0 {
                failure_storage.custody.push(CaptureCustody::group(owner));
            }
            let mut error = failure_storage.failure(
                format!("{error_context}: {}", errors.join("; ")),
                receipt,
                lease,
            );
            if let Some(state) = error.state_mut() {
                state.observation = observation;
                state.timeout_only = timeout_only;
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                {
                    state.terminal = Some(TerminalFailureEvidence {
                        group_only,
                        release: terminal_release,
                    });
                }
            }
            return Err(error);
        }
        let receipt = match receipt {
            Some(receipt) => receipt,
            None => {
                if held_deadline.is_some() && owner.retained_process_count() != 0 {
                    failure_storage.custody.push(CaptureCustody::group(owner));
                }
                let mut error = failure_storage.failure(
                    format!(
                        "{error_context}: combined group, actual EOF and controller cleanup is unconfirmed"
                    ),
                    None,
                    lease);
                if let Some(state) = error.state_mut() {
                    state.observation = observation;
                    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                    {
                        state.terminal = Some(TerminalFailureEvidence {
                            group_only,
                            release: terminal_release,
                        });
                    }
                }
                return Err(error);
            }
        };
        let (status, stdout, stderr, duration, timed_out) = match observation {
            Some(observation) => observation,
            None => {
                if held_deadline.is_some() && owner.retained_process_count() != 0 {
                    failure_storage.custody.push(CaptureCustody::group(owner));
                }
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                {
                    let mut error = failure_storage.failure(
                        format!("{error_context}: primary observation is unavailable"),
                        held_deadline.map(|_| receipt),
                        lease,
                    );
                    if let Some(state) = error.state_mut() {
                        state.terminal = Some(TerminalFailureEvidence {
                            group_only,
                            release: terminal_release,
                        });
                    }
                    return Err(error);
                }
                #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
                return Err(failure_storage.failure(
                    format!("{error_context}: primary observation is unavailable"),
                    None,
                    lease,
                ));
            }
        };
        Ok(CompleteCapturedBytes {
            status,
            stdout,
            stderr,
            duration,
            timed_out,
            receipt,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);
        struct Fixture(std::path::PathBuf);
        impl Fixture {
            fn new() -> Result<Self, String> {
                let path = std::env::temp_dir().join(format!(
                    "ripr-complete-capture-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed),
                ));
                std::fs::create_dir(&path).map_err(|error| error.to_string())?;
                Ok(Self(path))
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _cleanup = std::fs::remove_dir_all(&self.0);
            }
        }
        fn budget(input: usize, output: usize) -> CompleteCaptureBudget {
            CompleteCaptureBudget::new(Duration::from_secs(2), input, output, output)
        }
        fn shell(script: &str) -> Vec<String> {
            vec!["-c".to_string(), script.to_string()]
        }
        fn run(
            args: &[String],
            input: Option<&[u8]>,
            budget: CompleteCaptureBudget,
            lease: Arc<()>,
            settled: impl FnMut(ExitStatus),
        ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
            CompleteByteCapture::capture(
                (Path::new("/bin/sh"), args),
                (Path::new("/"), input),
                &[],
                (budget, "native complete capture control"),
                lease,
                settled,
            )
        }
        fn refused(
            result: Result<CompleteCapturedBytes, CompleteCaptureError>,
            expected: &str,
        ) -> Result<CompleteCaptureError, String> {
            match result {
                Err(error) if error.message().contains(expected) => Ok(error),
                Err(error) => Err(format!("expected {expected:?}, observed {error}")),
                Ok(_) => Err(format!("capture admitted expected refusal {expected:?}")),
            }
        }

        #[cfg(feature = "lang-rust")]
        mod terminal {
            use super::*;
            use crate::process_owner::{
                CompleteFailedClosed, CompleteTerminalCustodian, CompleteTerminalFailure,
            };
            use std::sync::atomic::{AtomicBool, Ordering};

            struct Lease {
                _file: std::fs::File,
                _fixture: Fixture,
                dropped: Arc<AtomicBool>,
            }

            impl Drop for Lease {
                fn drop(&mut self) {
                    self.dropped.store(true, Ordering::SeqCst);
                }
            }

            fn lease() -> Result<(Arc<Lease>, Arc<AtomicBool>), String> {
                let fixture = Fixture::new()?;
                let path = fixture.0.join("held-image");
                std::fs::write(&path, b"owned image bytes").map_err(|error| error.to_string())?;
                let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                let dropped = Arc::new(AtomicBool::new(false));
                Ok((
                    Arc::new(Lease {
                        _file: file,
                        _fixture: fixture,
                        dropped: dropped.clone(),
                    }),
                    dropped,
                ))
            }

            fn report_to_string(report: CompleteTerminalFailure<'_>) -> String {
                report.to_string()
            }

            fn capture<L: Any + Send + Sync>(
                script: &str,
                input: Option<&[u8]>,
                limits: CompleteCaptureBudget,
                custodian: &mut CompleteTerminalCustodian<L>,
            ) -> Result<CompleteCapturedBytes, String> {
                let args = shell(script);
                CompleteByteCapture::capture_with_terminal_custody(
                    (Path::new("/bin/sh"), &args),
                    (Path::new("/"), input),
                    &[],
                    (limits, "terminal custody control"),
                    custodian,
                )
                .map_err(report_to_string)
            }

            fn retained_child<L: Any + Send + Sync>(
                custodian: &mut CompleteTerminalCustodian<L>,
            ) -> Result<&mut crate::process_owner::OwnedProcess, String> {
                let error = custodian
                    .failure
                    .as_mut()
                    .ok_or("outside custodian lost the actual capture error")?;
                let custody = error
                    .custody
                    .first_mut()
                    .ok_or("outside custodian lost the actual process slot")?;
                if let Some(failure) = custody.setup.as_mut() {
                    return failure
                        .fixture_child()
                        .ok_or_else(|| "setup custody lost the actual child".to_string());
                }
                custody
                    .group
                    .as_mut()
                    .map(super::super::super::super::QualifiedGroupOwner::fixture_child)
                    .ok_or_else(|| "outside custodian lost the actual group owner".to_string())
            }

            fn fixture_closeout<L: Any + Send + Sync>(
                custodian: &mut CompleteTerminalCustodian<L>,
                checks: Result<(), String>,
            ) -> Result<(), String> {
                // Dispose only the controlled fixtures. This independent fixture
                // clock is never capture settlement or native caller qualification.
                let closeout = match custodian.failure.as_mut() {
                    Some(error) => close_retained_fixture(error),
                    None => Ok(()),
                };
                match (checks, closeout) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(primary), Ok(())) => Err(primary),
                    (Ok(()), Err(closeout)) => Err(format!(
                        "fixture-only closeout unconfirmed; custody remains in caller: {closeout}"
                    )),
                    (Err(primary), Err(closeout)) => Err(format!(
                        "{primary}; fixture-only closeout unconfirmed; custody remains in caller: {closeout}"
                    )),
                }
            }

            #[test]
            fn dropped_string_report_keeps_post_spawn_child_and_owned_lease() -> Result<(), String>
            {
                let admitted = Instant::now();
                let held = admitted + Duration::from_millis(500);
                let (lease, dropped) = lease()?;
                let weak = Arc::downgrade(&lease);
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                std::thread::sleep(Duration::from_millis(25));
                if Instant::now() >= held || admitted.elapsed() < Duration::from_millis(25) {
                    return Err(
                        "elapsed original admission was not still live before spawn".to_string()
                    );
                }
                let report = crate::process_owner::with_post_spawn_deadline_barrier(|| {
                    capture(
                        "exec /usr/bin/sleep 30",
                        None,
                        budget(0, 64),
                        &mut custodian,
                    )
                });
                drop(lease);
                let checks = (|| {
                    let message = match report {
                        Err(message) => message,
                        Ok(_) => return Err("terminal control admitted expired spawn".to_string()),
                    };
                    if !message.contains("spawn crossed its held deadline")
                        || dropped.load(Ordering::SeqCst)
                        || weak.upgrade().is_none()
                        || custodian.original_deadline() != held
                        || custodian.retained_process_count() != 1
                        || custodian.take_cleanup_receipt().is_some()
                        || custodian.take_failed_observation().is_some()
                    {
                        return Err("report disposal lost custody or minted evidence".to_string());
                    }
                    drop(message);
                    require_retained_identity(retained_child(&mut custodian)?, held)
                })();
                fixture_closeout(&mut custodian, checks)?;
                drop(custodian);
                if !dropped.load(Ordering::SeqCst) || weak.upgrade().is_some() {
                    return Err(
                        "observed fixture closeout failed to release owned lease".to_string()
                    );
                }
                Ok(())
            }

            #[test]
            fn dropped_report_keeps_constructed_group_under_original_clock() -> Result<(), String> {
                let held = Instant::now() + Duration::from_millis(500);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                HOOKS.with(|hooks| {
                    hooks.borrow_mut().delay_endpoint_once = Some(Duration::from_millis(600));
                });
                let report = capture(
                    "printf owned; exec /usr/bin/sleep 30",
                    None,
                    budget(0, 64),
                    &mut custodian,
                );
                HOOKS.with(|hooks| hooks.borrow_mut().delay_endpoint_once = None);
                let checks = (|| {
                    let message = match report {
                        Err(message) => message,
                        Ok(_) => return Err("terminal group expiry was admitted".to_string()),
                    };
                    if !message.contains("held custody deadline")
                        || !custodian.matches_lease(&lease)
                        || custodian.retained_process_count() != 1
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err("group report disposal discarded real custody".to_string());
                    }
                    let error = custodian
                        .failure()
                        .ok_or("group failure is absent from its outside owner")?;
                    if !error.matches_lease(&lease) || error.is_timeout_only() {
                        return Err("terminal group failure changed lease or meaning".to_string());
                    }
                    require_retained_identity(retained_child(&mut custodian)?, held)
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn report_unwind_keeps_outside_custodian_until_fixture_closeout() -> Result<(), String>
            {
                let held = Instant::now() + Duration::from_millis(500);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease)?;
                let report_checks = {
                    let args = shell("exec /usr/bin/sleep 30");
                    let output = crate::process_owner::with_post_spawn_deadline_barrier(|| {
                        CompleteByteCapture::capture_with_terminal_custody(
                            (Path::new("/bin/sh"), &args),
                            (Path::new("/"), None),
                            &[],
                            (budget(0, 64), "report unwind control"),
                            &mut custodian,
                        )
                    });
                    match output {
                        Ok(_) => Err("report unwind control admitted expired spawn".to_string()),
                        Err(report) => {
                            if !report.message().contains("spawn crossed its held deadline")
                                || report.capture_error().is_none()
                            {
                                Err("borrowed report lost its actual primary failure".to_string())
                            } else {
                                let unwound = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(move || {
                                        let _reported = report.to_string();
                                        std::panic::resume_unwind(Box::new(
                                            "borrowed report control",
                                        ));
                                    }),
                                );
                                match unwound {
                                    Ok(()) => Err("report control did not unwind".to_string()),
                                    Err(payload) => {
                                        drop(payload);
                                        Ok(())
                                    }
                                }
                            }
                        }
                    }
                };
                let checks = (|| {
                    report_checks?;
                    require_retained_identity(retained_child(&mut custodian)?, held)
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn terminal_reuse_preserves_first_error_child_clock_and_lease() -> Result<(), String> {
                let held = Instant::now() + Duration::from_millis(500);
                let lease = Arc::new(());
                let foreign = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let first = crate::process_owner::with_post_spawn_deadline_barrier(|| {
                    capture(
                        "exec /usr/bin/sleep 30",
                        None,
                        budget(0, 64),
                        &mut custodian,
                    )
                });
                let checks = (|| {
                    match first {
                        Err(message) if message.contains("spawn crossed its held deadline") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("first terminal capture was admitted".to_string()),
                    }
                    let before = super::super::super::super::ObservedProcessIdentity::read(
                        retained_child(&mut custodian)?.id(),
                    )?;
                    let attempts = super::super::super::super::linux::spawn_attempts();
                    let refusal = capture("exit 0", None, budget(0, 64), &mut custodian);
                    match refusal {
                        Err(message) if message.contains("already attempted") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("terminal owner was reused".to_string()),
                    }
                    let after = super::super::super::super::ObservedProcessIdentity::read(
                        retained_child(&mut custodian)?.id(),
                    )?;
                    if before.pid() != after.pid()
                        || before.start() != after.start()
                        || !custodian.matches_lease(&lease)
                        || custodian.matches_lease(&foreign)
                        || custodian.original_deadline() != held
                        || super::super::super::super::linux::spawn_attempts() != attempts
                    {
                        return Err("retry replaced the actual terminal custody".to_string());
                    }
                    require_retained_identity(retained_child(&mut custodian)?, held)
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn expiry_before_dispatch_has_no_child_or_cleanup_receipt() -> Result<(), String> {
                let held = Instant::now() + Duration::from_millis(20);
                let mut custodian = CompleteTerminalCustodian::new(held, Arc::new(()))?;
                while Instant::now() < held {
                    std::thread::yield_now();
                }
                let attempts = super::super::super::super::linux::spawn_attempts();
                match capture(
                    "exec /missing/terminal-worker",
                    None,
                    budget(0, 64),
                    &mut custodian,
                ) {
                    Err(message) if message.contains("held custody deadline expired") => {}
                    Err(message) => return Err(message),
                    Ok(_) => return Err("expired terminal dispatch was admitted".to_string()),
                }
                if custodian.retained_process_count() != 0
                    || custodian.take_cleanup_receipt().is_some()
                    || custodian.take_failed_observation().is_some()
                    || super::super::super::super::linux::spawn_attempts() != attempts
                {
                    return Err("pre-dispatch refusal acquired a child or authority".to_string());
                }
                Ok(())
            }

            #[test]
            fn timely_terminal_capture_returns_unchanged_real_receipt_and_bytes()
            -> Result<(), String> {
                let held = Instant::now() + Duration::from_secs(5);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let input = b"terminal-input\0\xff";
                let output = capture(
                    "printf prefix; printf stderr >&2; cat",
                    Some(input),
                    budget(input.len(), 64),
                    &mut custodian,
                )?;
                let (status, stdout, stderr, _, timed_out, receipt) = output.into_parts();
                let mut expected = b"prefix".to_vec();
                expected.extend_from_slice(input);
                if !status.success()
                    || timed_out
                    || !receipt.matches_lease(&lease)
                    || custodian.failure().is_some()
                    || custodian.take_cleanup_receipt().is_some()
                    || custodian.retained_process_count() != 0
                    || custodian.original_deadline() != held
                {
                    return Err("terminal success changed actual capture semantics".to_string());
                }
                assert_eq!(stdout, expected);
                assert_eq!(stderr, b"stderr");
                match capture("exit 0", None, budget(0, 64), &mut custodian) {
                    Err(message) if message.contains("already attempted") => Ok(()),
                    Err(message) => Err(message),
                    Ok(_) => Err("successful terminal owner was reused".to_string()),
                }
            }

            #[test]
            fn terminal_failure_keeps_real_settled_negative_without_success() -> Result<(), String>
            {
                let held = Instant::now() + Duration::from_secs(5);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                {
                    let args = shell("printf overflow");
                    let mut report = match CompleteByteCapture::capture_with_terminal_custody(
                        (Path::new("/bin/sh"), &args),
                        (Path::new("/"), None),
                        &[],
                        (budget(0, 3), "terminal settled negative"),
                        &mut custodian,
                    ) {
                        Err(report) => report,
                        Ok(_) => return Err("terminal overflow became success".to_string()),
                    };
                    if !report
                        .message()
                        .contains("stdout exceeds its 3-byte output budget")
                    {
                        return Err(format!("unexpected terminal failure: {report}"));
                    }
                    let error = report
                        .capture_error()
                        .ok_or("terminal overflow lost actual capture error")?;
                    if error.is_timeout_only() || !error.matches_lease(&lease) {
                        return Err("terminal overflow changed failure or lease".to_string());
                    }
                    let (status, _, timed_out) = report
                        .observed_outcome()
                        .ok_or("settled terminal negative lost its native observation")?;
                    if !status.success() || timed_out {
                        return Err("controlled negative changed its actual exit data".to_string());
                    }
                    let receipt = report
                        .take_cleanup_receipt()
                        .ok_or("settled terminal negative lost actual combined receipt")?;
                    if !receipt.matches_lease(&lease)
                        || report.take_cleanup_receipt().is_some()
                        || Instant::now() >= held
                    {
                        return Err(
                            "settled terminal negative changed receipt or clock".to_string()
                        );
                    }
                }
                let (_, stdout, stderr, _, timed_out) = custodian
                    .take_failed_observation()
                    .ok_or("terminal failure lost actual rejected output bytes")?;
                assert_eq!(stdout, b"over");
                assert_eq!(stderr, Vec::<u8>::new());
                if timed_out || custodian.take_failed_observation().is_some() {
                    return Err("settled negative was fabricated or replayed".to_string());
                }
                Ok(())
            }

            fn capture_until<L: Any + Send + Sync>(
                script: &str,
                limits: CompleteCaptureBudget,
                started: Instant,
                execution_deadline: Instant,
                custodian: &mut CompleteTerminalCustodian<L>,
            ) -> Result<CompleteCapturedBytes, String> {
                let args = shell(script);
                CompleteByteCapture::capture_with_terminal_custody_until(
                    (Path::new("/bin/sh"), &args),
                    (Path::new("/"), None),
                    &[],
                    (limits, "absolute terminal custody control"),
                    started,
                    execution_deadline,
                    custodian,
                )
                .map_err(report_to_string)
            }

            fn require_closed<L: Any + Send + Sync>(
                closed: &CompleteFailedClosed,
                lease: &Arc<L>,
                held: Instant,
            ) -> Result<(), String> {
                let worker = closed.observed_worker();
                let parent = closed.observed_parent();
                if closed.original_deadline() != held
                    || !closed.matches_lease(lease)
                    || worker.pid() == 0
                    || worker.start() == 0
                    || worker.group() != worker.pid()
                    || worker.parent() != parent.pid()
                    || parent.pid() != std::process::id()
                    || parent.start() == 0
                    || Instant::now() >= held
                {
                    return Err("negative disposition changed actual identity, lease or clock".to_string());
                }
                Ok(())
            }

            #[test]
            fn missing_eof_retains_real_group_only_negative_and_refuses_replay() -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_secs(12);
                let execution_deadline = started + Duration::from_secs(4);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                HOOKS.with(|hooks| hooks.borrow_mut().hold_writer = Some("stdout"));
                let result = capture_until(
                    "printf missing-eof",
                    budget(0, 64),
                    started,
                    execution_deadline,
                    &mut custodian,
                );
                // The discriminator owns an extra real writer, outside transport.
                // Close it before asking to dispose this fixture's failed capture.
                HOOKS.with(|hooks| drop(hooks.borrow_mut().held_writer.take()));
                let checks = (|| {
                    let message = match result {
                        Err(message) if message.contains("stdout actual EOF is unconfirmed") => message,
                        Err(message) => return Err(message),
                        Ok(_) => return Err("missing actual EOF became captured success".to_string()),
                    };
                    if started.elapsed() < POST_KILL_DRAIN_GRACE
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err("missing EOF fabricated a combined receipt or reset drain".to_string());
                    }
                    let expected = custodian
                        .failure()
                        .and_then(CompleteCaptureError::state)
                        .and_then(|state| state.terminal.as_ref())
                        .and_then(|terminal| terminal.group_only.as_ref())
                        .ok_or("actual settled group was lost at missing EOF")?
                        .observed_worker();
                    let closed = custodian.try_closeout_failure().map_err(report_to_string)?;
                    require_closed(&closed, &lease, held)?;
                    if !closed.settled_status().success()
                        || closed.observed_worker().pid() != expected.pid()
                        || closed.observed_worker().start() != expected.start()
                        || custodian.failure().map(CompleteCaptureError::message) != Some(message.as_str())
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err("negative closeout changed group proof or first refusal".to_string());
                    }
                    match custodian.try_closeout_failure() {
                        Err(report) if report.message() == message => Ok(()),
                        Err(report) => Err(format!("negative replay changed the first error: {report}")),
                        Ok(_) => Err("group-only negative disposition replayed".to_string()),
                    }
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn combined_negative_consumes_real_receipt_without_output_success_and_recovers()
            -> Result<(), String> {
                let held = Instant::now() + Duration::from_secs(5);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let message = match capture("printf overflow", None, budget(0, 3), &mut custodian) {
                    Err(message) if message.contains("stdout exceeds its 3-byte output budget") => message,
                    Err(message) => return fixture_closeout(&mut custodian, Err(message)),
                    Ok(_) => return Err("overflow became successful captured output".to_string()),
                };
                let checks = (|| {
                    let expected = custodian
                        .failure()
                        .and_then(CompleteCaptureError::state)
                        .and_then(|state| state.receipt.as_ref())
                        .ok_or("overflow lost actual combined EOF receipt")?
                        .observed_worker();
                    let closed = custodian.try_closeout_failure().map_err(report_to_string)?;
                    require_closed(&closed, &lease, held)?;
                    if !closed.settled_status().success()
                        || closed.observed_worker().pid() != expected.pid()
                        || closed.observed_worker().start() != expected.start()
                        || custodian.take_cleanup_receipt().is_some()
                        || custodian.failure().map(CompleteCaptureError::message) != Some(message.as_str())
                    {
                        return Err("combined negative changed actual observation or first error".to_string());
                    }
                    let (_, stdout, stderr, _, timed_out) = custodian
                        .take_failed_observation()
                        .ok_or("combined negative lost rejected byte observations")?;
                    assert_eq!(stdout, b"over");
                    assert_eq!(stderr, Vec::<u8>::new());
                    if timed_out {
                        return Err("overflow was recategorized as timeout".to_string());
                    }
                    match custodian.try_closeout_failure() {
                        Err(report) if report.message() == message => {}
                        Err(report) => return Err(report.to_string()),
                        Ok(_) => return Err("combined negative disposition replayed".to_string()),
                    }
                    let recovery_lease = Arc::new(());
                    let mut recovery = CompleteTerminalCustodian::new(held, recovery_lease.clone())?;
                    let output = capture("printf recovered", None, budget(0, 64), &mut recovery)?;
                    let (status, stdout, stderr, _, timed_out, receipt) = output.into_parts();
                    if !status.success() || timed_out || !receipt.matches_lease(&recovery_lease) {
                        return Err("independent recovery lost genuine capture semantics".to_string());
                    }
                    assert_eq!(stdout, b"recovered");
                    assert_eq!(stderr, Vec::<u8>::new());
                    Ok(())
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn negative_extraction_crossing_original_clock_restores_same_real_receipt()
            -> Result<(), String> {
                let held = Instant::now() + Duration::from_secs(1);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let message = match capture("printf overflow", None, budget(0, 3), &mut custodian) {
                    Err(message) if message.contains("stdout exceeds its 3-byte output budget") => message,
                    Err(message) => return fixture_closeout(&mut custodian, Err(message)),
                    Ok(_) => return Err("late extraction control admitted overflow".to_string()),
                };
                let checks = (|| {
                    let expected = custodian
                        .failure()
                        .and_then(CompleteCaptureError::state)
                        .and_then(|state| state.receipt.as_ref())
                        .ok_or("late extraction control lacks actual combined receipt")?
                        .observed_worker();
                    with_terminal_take_deadline_barrier(|| {
                        match custodian.try_closeout_failure() {
                            Err(report) if report.message() == message => Ok(()),
                            Err(report) => Err(report.to_string()),
                            Ok(_) => Err("late extraction minted a negative disposition".to_string()),
                        }
                    })?;
                    let restored = custodian
                        .failure()
                        .and_then(CompleteCaptureError::state)
                        .and_then(|state| state.receipt.as_ref())
                        .ok_or("late extraction lost the taken real receipt")?;
                    if Instant::now() < held
                        || restored.observed_worker().pid() != expected.pid()
                        || restored.observed_worker().start() != expected.start()
                        || !restored.matches_lease(&lease)
                        || custodian.original_deadline() != held
                        || custodian.failure().map(CompleteCaptureError::message) != Some(message.as_str())
                    {
                        return Err("late extraction restored a different receipt, clock or lease".to_string());
                    }
                    if custodian.take_cleanup_receipt().is_some() {
                        return Err("expired custody transferred cleanup authority".to_string());
                    }
                    Ok(())
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn elapsed_live_absolute_execution_cutoff_reaches_actual_wait_without_rebase()
            -> Result<(), String> {
                let started = Instant::now();
                let execution_deadline = started + Duration::from_millis(700);
                let held = started + Duration::from_secs(10);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                thread::sleep(Duration::from_millis(100));
                if Instant::now() >= execution_deadline {
                    return Err("elapsed execution control did not retain a live cutoff".to_string());
                }
                let result = capture_until(
                    "printf actual-progress; exec /usr/bin/sleep 30",
                    budget(0, 64),
                    started,
                    execution_deadline,
                    &mut custodian,
                );
                let checks = (|| {
                    match result {
                        Err(message) if message.contains("complete worker timed out") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("absolute execution timeout became capture success".to_string()),
                    }
                    let observed = super::super::super::super::linux::last_wait_window()
                        .ok_or("actual group wait did not record its supplied window")?;
                    assert_eq!(observed.0, started);
                    assert_eq!(observed.1, execution_deadline.duration_since(started));
                    let closed = custodian.try_closeout_failure().map_err(report_to_string)?;
                    require_closed(&closed, &lease, held)?;
                    let (_, stdout, stderr, _, timed_out) = custodian
                        .take_failed_observation()
                        .ok_or("actual timeout lost failed byte observations")?;
                    assert_eq!(stdout, b"actual-progress");
                    assert_eq!(stderr, Vec::<u8>::new());
                    if !timed_out || Instant::now() < execution_deadline
                        || custodian.original_deadline() != held
                    {
                        return Err("actual wait rebased the admitted absolute window".to_string());
                    }
                    Ok(())
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn absolute_execution_keeps_tighter_existing_relative_budget() -> Result<(), String> {
                let started = Instant::now();
                let execution_deadline = started + Duration::from_secs(2);
                let held = started + Duration::from_secs(10);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let timeout = Duration::from_millis(300);
                thread::sleep(Duration::from_millis(50));
                let result = capture_until(
                    "printf relative-progress; exec /usr/bin/sleep 30",
                    CompleteCaptureBudget::new(timeout, 0, 64, 64),
                    started,
                    execution_deadline,
                    &mut custodian,
                );
                let checks = (|| {
                    match result {
                        Err(message) if message.contains("complete worker timed out") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("tighter relative timeout became success".to_string()),
                    }
                    let observed = super::super::super::super::linux::last_wait_window()
                        .ok_or("actual tighter wait did not record its supplied window")?;
                    assert_eq!(observed.0, started);
                    assert_eq!(observed.1, timeout);
                    let closed = custodian.try_closeout_failure().map_err(report_to_string)?;
                    require_closed(&closed, &lease, held)
                })();
                fixture_closeout(&mut custodian, checks)
            }

            #[test]
            fn invalid_absolute_windows_refuse_before_pairs_or_spawn_without_negative_grant()
            -> Result<(), String> {
                for variant in 0..4 {
                    let started = Instant::now();
                    let held = started + Duration::from_secs(10);
                    let (admitted_started, execution_deadline) = match variant {
                        0 => (started, held - Duration::from_secs(6)),
                        1 => (started + Duration::from_secs(1), started + Duration::from_secs(2)),
                        2 => (started, started),
                        _ => (started - Duration::from_secs(1), started - Duration::from_millis(1)),
                    };
                    let lease = Arc::new(());
                    let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                    let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
                    let spawns = super::super::super::super::linux::spawn_attempts();
                    match capture_until(
                        "exec /missing/absolute-worker",
                        budget(0, 64),
                        admitted_started,
                        execution_deadline,
                        &mut custodian,
                    ) {
                        Err(message) if message.contains("absolute execution window") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("invalid absolute window reached capture".to_string()),
                    }
                    if HOOKS.with(|hooks| hooks.borrow().pair_attempts) != pairs
                        || super::super::super::super::linux::spawn_attempts() != spawns
                        || custodian.take_cleanup_receipt().is_some()
                        || !custodian.matches_lease(&lease)
                        || custodian.original_deadline() != held
                    {
                        return Err("invalid window allocated endpoints, spawned or changed custody".to_string());
                    }
                    match custodian.try_closeout_failure() {
                        Err(report) if report.capture_error().is_some() => {}
                        Err(report) => return Err(report.to_string()),
                        Ok(_) => return Err("count-free preflight minted negative disposition".to_string()),
                    }
                }
                Ok(())
            }

            #[test]
            fn unqualified_post_spawn_owner_never_gets_checked_negative_disposition()
            -> Result<(), String> {
                let held = Instant::now() + Duration::from_millis(500);
                let lease = Arc::new(());
                let mut custodian = CompleteTerminalCustodian::new(held, lease.clone())?;
                let result = crate::process_owner::with_post_spawn_deadline_barrier(|| {
                    capture("exec /usr/bin/sleep 30", None, budget(0, 64), &mut custodian)
                });
                let checks = (|| {
                    match result {
                        Err(message) if message.contains("spawn crossed its held deadline") => {}
                        Err(message) => return Err(message),
                        Ok(_) => return Err("post-spawn uncertainty became success".to_string()),
                    }
                    let expected = super::super::super::super::ObservedProcessIdentity::read(
                        retained_child(&mut custodian)?.id(),
                    )?;
                    match custodian.try_closeout_failure() {
                        Err(report) if report.capture_error().is_some() => {}
                        Err(report) => return Err(report.to_string()),
                        Ok(_) => return Err("unqualified direct custody became checked negative".to_string()),
                    }
                    let retained = super::super::super::super::ObservedProcessIdentity::read(
                        retained_child(&mut custodian)?.id(),
                    )?;
                    if retained.pid() != expected.pid()
                        || retained.start() != expected.start()
                        || !custodian.matches_lease(&lease)
                        || custodian.original_deadline() != held
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err("negative refusal lost real unqualified child or original ceiling".to_string());
                    }
                    require_retained_identity(retained_child(&mut custodian)?, held)
                })();
                fixture_closeout(&mut custodian, checks)
            }
        }

        fn run_held(
            args: &[String],
            input: Option<&[u8]>,
            budget: CompleteCaptureBudget,
            held_deadline: Instant,
            lease: Arc<()>,
            settled: impl FnMut(ExitStatus),
        ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
            CompleteByteCapture::capture_with_deadline(
                (Path::new("/bin/sh"), args),
                (Path::new("/"), input),
                &[],
                (budget, "native complete capture control"),
                held_deadline,
                lease,
                settled,
            )
        }

        fn close_retained_fixture(error: &mut CompleteCaptureError) -> Result<(), String> {
            // Only fixtures use this independent clock; no production receipt is created.
            for custody in &mut error.custody {
                if let Some(failure) = custody.setup.as_mut()
                    && let Some(child) = failure.fixture_child()
                {
                    child.request_kill().map_err(|error| error.to_string())?;
                    if !child.reap_within(Duration::from_secs(5)) {
                        return Err("retained setup fixture reap was unconfirmed".to_string());
                    }
                }
                if let Some(owner) = custody.group.as_mut() {
                    owner.close_fixture_custody()?;
                }
            }
            Ok(())
        }

        fn require_retained_identity(
            child: &mut crate::process_owner::OwnedProcess,
            held: Instant,
        ) -> Result<(), String> {
            let before = super::super::super::ObservedProcessIdentity::read(child.id())?;
            if before.pid() != child.id()
                || before.parent() != std::process::id()
                || before.group() != child.id()
                || before.start() == 0
                || child.bounded_drop_until != Some(held)
                || !matches!(before.state(), 'R' | 'S' | 'D' | 'T' | 't' | 'I')
            {
                return Err(
                    "capture refusal lost actual live direct-child identity or clock".to_string(),
                );
            }
            if child.reap_until(held) {
                return Err("retained capture restarted an expired direct reap clock".to_string());
            }
            let after = super::super::super::ObservedProcessIdentity::read(child.id())?;
            if before.pid() != after.pid()
                || before.start() != after.start()
                || before.parent() != after.parent()
                || before.group() != after.group()
                || !matches!(after.state(), 'R' | 'S' | 'D' | 'T' | 't' | 'I')
            {
                return Err(
                    "retained capture child changed before fixture-only cleanup".to_string()
                );
            }
            Ok(())
        }

        #[test]
        fn actual_post_spawn_clock_crossing_keeps_same_lease_and_unqualified_child()
        -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let lease = Arc::new(());
            let foreign = Arc::new(());
            let mut settled = false;
            let output = crate::process_owner::with_post_spawn_deadline_barrier(|| {
                run_held(
                    &shell("exec /usr/bin/sleep 30"),
                    None,
                    budget(0, 64),
                    held,
                    lease.clone(),
                    |_| settled = true,
                )
            });
            let mut error = match output {
                Err(error) => error,
                Ok(_) => return Err("post-spawn capture clock crossing was admitted".to_string()),
            };
            let checks = (|| {
                if !error.message().contains("spawn crossed its held deadline")
                    || !error.matches_lease(&lease)
                    || error.matches_lease(&foreign)
                    || settled
                    || Instant::now() < held
                    || error.is_timeout_only()
                    || error.take_cleanup_receipt().is_some()
                    || error.take_failed_observation().is_some()
                    || error.custody.len() != 1
                    || Arc::strong_count(&lease) != 2
                {
                    return Err(
                        "post-spawn failure lost primary custody or fabricated evidence"
                            .to_string(),
                    );
                }
                let custody = error.custody.first_mut().ok_or_else(|| {
                    "failed qualification has no retained custody slot".to_string()
                })?;
                if custody.group.is_some() {
                    return Err(
                        "failed qualification was converted to a qualified group".to_string()
                    );
                }
                let failure = custody.setup.as_mut().ok_or_else(|| {
                    "failed qualification discarded its setup custody".to_string()
                })?;
                let child = failure
                    .fixture_child()
                    .ok_or_else(|| "actual post-spawn child handle was discarded".to_string())?;
                require_retained_identity(child, held)
            })();
            close_retained_fixture(&mut error)?;
            checks?;
            drop(error);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn actual_io_clock_failure_keeps_constructed_group_custody_without_receipt()
        -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let lease = Arc::new(());
            HOOKS.with(|hooks| {
                hooks.borrow_mut().delay_endpoint_once = Some(Duration::from_millis(600));
            });
            let output = run_held(
                &shell("printf owned; exec /usr/bin/sleep 30"),
                None,
                budget(0, 64),
                held,
                lease.clone(),
                |_| {},
            );
            HOOKS.with(|hooks| hooks.borrow_mut().delay_endpoint_once = None);
            let mut error = match output {
                Err(error) => error,
                Ok(_) => return Err("actual I/O crossed its custody clock as success".to_string()),
            };
            let checks = (|| {
                if !error.message().contains("held custody deadline")
                    || !error.matches_lease(&lease)
                    || error.take_cleanup_receipt().is_some()
                    || error.observed_outcome().is_some()
                    || error.is_timeout_only()
                    || error.custody.len() != 1
                {
                    return Err("late I/O lost group custody or fabricated settlement".to_string());
                }
                let custody = error.custody.first_mut().ok_or_else(|| {
                    "actual constructed group has no retained custody slot".to_string()
                })?;
                if custody.setup.is_some() {
                    return Err("actual constructed group was reduced to setup custody".to_string());
                }
                let owner = custody.group.as_mut().ok_or_else(|| {
                    "actual constructed group custody was not retained".to_string()
                })?;
                require_retained_identity(owner.fixture_child(), held)
            })();
            close_retained_fixture(&mut error)?;
            checks?;
            drop(error);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn original_expired_clock_precedes_missing_executable_and_transport_setup()
        -> Result<(), String> {
            let lease = Arc::new(());
            let foreign = Arc::new(());
            let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
            let attempts = super::super::super::linux::spawn_attempts();
            let mut error = refused(
                CompleteByteCapture::capture_with_deadline(
                    (Path::new("/ripr-deliberately-missing-held-worker"), &[]),
                    (Path::new("/"), None),
                    &[],
                    (budget(0, 64), "expired held control"),
                    Instant::now(),
                    lease.clone(),
                    |_| {},
                ),
                "held custody deadline",
            )?;
            assert_eq!(HOOKS.with(|hooks| hooks.borrow().pair_attempts), pairs);
            assert_eq!(super::super::super::linux::spawn_attempts(), attempts);
            if !error.matches_lease(&lease)
                || error.matches_lease(&foreign)
                || error.take_cleanup_receipt().is_some()
                || error.take_failed_observation().is_some()
                || error.is_timeout_only()
            {
                return Err(
                    "expired preflight manufactured observation or lost custody".to_string()
                );
            }
            drop(error);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn original_clock_expiring_during_real_input_admission_does_not_restart()
        -> Result<(), String> {
            let lease = Arc::new(());
            let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
            let attempts = super::super::super::linux::spawn_attempts();
            HOOKS.with(|hooks| {
                hooks.borrow_mut().delay_admission_once = Some(Duration::from_millis(40));
            });
            let input = [19_u8; 64];
            let output = run_held(
                &shell("cat"),
                Some(&input),
                budget(input.len(), 64),
                Instant::now() + Duration::from_millis(20),
                lease.clone(),
                |_| {},
            );
            HOOKS.with(|hooks| hooks.borrow_mut().delay_admission_once = None);
            let mut error = refused(output, "held custody deadline")?;
            assert_eq!(HOOKS.with(|hooks| hooks.borrow().pair_attempts), pairs);
            assert_eq!(super::super::super::linux::spawn_attempts(), attempts);
            if error.take_cleanup_receipt().is_some()
                || error.observed_outcome().is_some()
                || !error.matches_lease(&lease)
            {
                return Err("late admission acquired worker authority".to_string());
            }
            Ok(())
        }

        #[test]
        fn strict_timely_capture_keeps_none_empty_binary_and_ceiling_parity() -> Result<(), String>
        {
            for input in [None, Some(&[][..]), Some(&b"\0owned\xff"[..])] {
                for output_limit in [64, MAX_STREAM_BYTES] {
                    let args = shell("cat; printf '\\377\\200' >&2; exit 7");
                    let input_limit = input.map_or(0, <[u8]>::len);
                    let ordinary_lease = Arc::new(());
                    let ordinary = run(
                        &args,
                        input,
                        budget(input_limit, output_limit),
                        ordinary_lease.clone(),
                        |_| {},
                    )
                    .map_err(|error| error.to_string())?
                    .into_parts();
                    let strict_lease = Arc::new(());
                    let strict = run_held(
                        &args,
                        input,
                        budget(input_limit, output_limit),
                        Instant::now() + Duration::from_secs(2),
                        strict_lease.clone(),
                        |_| {},
                    )
                    .map_err(|error| error.to_string())?
                    .into_parts();
                    assert_eq!(
                        (strict.0, &strict.1, &strict.2, strict.4),
                        (ordinary.0, &ordinary.1, &ordinary.2, ordinary.4)
                    );
                    assert_eq!(strict.1, input.unwrap_or(&[]));
                    assert_eq!(strict.2, vec![255, 128]);
                    if !strict.5.matches_lease(&strict_lease)
                        || strict.5.matches_lease(&ordinary_lease)
                        || !ordinary.5.matches_lease(&ordinary_lease)
                    {
                        return Err("parity output confused actual invocation leases".to_string());
                    }
                }
            }
            Ok(())
        }

        #[test]
        fn strict_overflow_and_timeout_keep_failed_data_and_one_shot_cleanup_parity()
        -> Result<(), String> {
            for timeout in [false, true] {
                let args = if timeout {
                    shell("sleep 30")
                } else {
                    shell("printf abc")
                };
                let make_budget = || {
                    CompleteCaptureBudget::new(
                        if timeout {
                            Duration::from_millis(50)
                        } else {
                            Duration::from_secs(2)
                        },
                        0,
                        if timeout { 64 } else { 1 },
                        64,
                    )
                };
                let expected = if timeout {
                    "timed out"
                } else {
                    "stdout exceeds"
                };
                let mut ordinary = refused(
                    run(&args, None, make_budget(), Arc::new(()), |_| {}),
                    expected,
                )?;
                let lease = Arc::new(());
                let mut strict = refused(
                    run_held(
                        &args,
                        None,
                        make_budget(),
                        Instant::now() + Duration::from_secs(2),
                        lease.clone(),
                        |_| {},
                    ),
                    expected,
                )?;
                assert_eq!(strict.is_timeout_only(), ordinary.is_timeout_only());
                let ordinary_data = ordinary
                    .take_failed_observation()
                    .ok_or_else(|| "ordinary failure lost actual observation".to_string())?;
                let strict_data = strict
                    .take_failed_observation()
                    .ok_or_else(|| "strict failure lost actual observation".to_string())?;
                assert_eq!(
                    (strict_data.0, &strict_data.1, &strict_data.2, strict_data.4),
                    (
                        ordinary_data.0,
                        &ordinary_data.1,
                        &ordinary_data.2,
                        ordinary_data.4
                    )
                );
                if strict.take_failed_observation().is_some() {
                    return Err("strict failed DATA was reused".to_string());
                }
                let receipt = strict.take_cleanup_receipt().ok_or_else(|| {
                    "timely failed capture lost real cleanup settlement".to_string()
                })?;
                if !receipt.matches_lease(&lease)
                    || strict.take_cleanup_receipt().is_some()
                    || ordinary.take_cleanup_receipt().is_none()
                {
                    return Err(
                        "failed parity capture fabricated or reused cleanup custody".to_string()
                    );
                }
            }
            Ok(())
        }

        #[test]
        fn actual_settled_callback_crossing_original_clock_cannot_mint_cleanup()
        -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let lease = Arc::new(());
            let mut observed = None;
            let output = run_held(
                &shell("printf owned"),
                None,
                budget(0, 64),
                held,
                lease.clone(),
                |status| {
                    observed = Some(status);
                    thread::sleep(
                        held.saturating_duration_since(Instant::now()) + Duration::from_millis(10),
                    );
                },
            );
            let mut error = refused(output, "held custody deadline")?;
            let observed = observed
                .ok_or_else(|| "real timely group never reached settled callback".to_string())?;
            if !observed.success()
                || !error.matches_lease(&lease)
                || error.take_cleanup_receipt().is_some()
                || error.is_timeout_only()
            {
                return Err("late callback manufactured cleanup or lost actual status".to_string());
            }
            let data = error
                .take_failed_observation()
                .ok_or_else(|| "late callback lost actual failed capture DATA".to_string())?;
            assert_eq!(data.0, observed);
            assert_eq!(data.1, b"owned");
            Ok(())
        }

        #[test]
        fn complete_bytes_require_actual_eof_and_matching_lease() -> Result<(), String> {
            let lease = Arc::new(());
            let foreign = Arc::new(());
            let input = b"\0owned\xff";
            let args = shell("cat; printf '\\377\\200' >&2");
            let output = run(
                &args,
                Some(input),
                budget(input.len(), 64),
                lease.clone(),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, receipt) = output.into_parts();
            if !status.success() || timeout {
                return Err("native positive child failed or timed out".to_string());
            }
            assert_eq!(stdout, input);
            assert_eq!(stderr, vec![255, 128]);
            if !receipt.matches_lease(&lease) || receipt.matches_lease(&foreign) {
                return Err(
                    "combined receipt accepted a different actual custody lease".to_string()
                );
            }
            assert_eq!(receipt.settled_status(), Some(status));
            let worker = receipt.observed_worker();
            let parent = receipt.observed_parent();
            let actual_parent =
                super::super::super::ObservedProcessIdentity::read(std::process::id())?;
            assert_eq!(parent.pid(), actual_parent.pid());
            assert_eq!(parent.parent(), actual_parent.parent());
            assert_eq!(parent.group(), actual_parent.group());
            assert_eq!(parent.start(), actual_parent.start());
            assert_eq!(worker.parent(), parent.pid());
            assert_eq!(worker.group(), worker.pid());
            if worker.start() == 0 {
                return Err("settled receipt lost its actual worker start identity".to_string());
            }
            drop(receipt);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn some_empty_half_closes_but_none_retains_null_input() -> Result<(), String> {
            for input in [None, Some(&[][..])] {
                let output = run(
                    &shell("cat; printf eof"),
                    input,
                    budget(0, 64),
                    Arc::new(()),
                    |_| {},
                )
                .map_err(|error| error.to_string())?;
                let (status, stdout, _, _, timeout, _) = output.into_parts();
                if !status.success() || timeout || stdout != b"eof" {
                    return Err("empty/null input did not reach worker EOF".to_string());
                }
            }
            Ok(())
        }

        #[test]
        fn native_sender_bytes_and_would_block_are_not_eof() -> Result<(), String> {
            let (mut reader, mut writer) = UnixStream::pair().map_err(|error| error.to_string())?;
            reader
                .set_nonblocking(true)
                .map_err(|error| error.to_string())?;
            writer
                .write_all(b"sent-before-block")
                .map_err(|error| error.to_string())?;
            let mut capture = OutputCapture::new();
            let mut scratch = [0u8; CHUNK_BYTES];
            capture.step(&mut reader, "stdout", 64, &mut scratch);
            assert_eq!(capture.bytes, b"sent-before-block");
            capture.step(&mut reader, "stdout", 64, &mut scratch);
            if capture.eof || capture.failed || capture.error.is_some() {
                return Err("sender bytes or WouldBlock fabricated EOF/failure".to_string());
            }
            drop(writer);
            capture.step(&mut reader, "stdout", 64, &mut scratch);
            if !capture.eof {
                return Err("actual final read(0) was not observed".to_string());
            }
            Ok(())
        }

        #[test]
        fn overflow_retains_failure_and_requires_actual_eof_before_cleanup() -> Result<(), String> {
            let lease = Arc::new(());
            let mut observed = None;
            let mut failure = refused(
                run(
                    &shell("printf abc"),
                    None,
                    budget(0, 1),
                    lease.clone(),
                    |status| observed = Some(status),
                ),
                "stdout exceeds its 1-byte output budget",
            )?;
            if !observed.is_some_and(|status| status.success()) {
                return Err("overflow control lacks actual exit0 primary".to_string());
            }
            let receipt = failure
                .take_cleanup_receipt()
                .ok_or("overflow never reached actual EOF/group cleanup")?;
            if !receipt.matches_lease(&lease) || failure.take_cleanup_receipt().is_some() {
                return Err("cleanup receipt mismatched or transferable twice".to_string());
            }
            let mut capture = OutputCapture::new();
            let mut input = std::io::Cursor::new(vec![1u8; 64]);
            let mut scratch = [0u8; CHUNK_BYTES];
            capture.step(&mut input, "stdout", 3, &mut scratch);
            assert_eq!(capture.bytes.len(), 4);
            if capture.eof || capture.error.is_none() {
                return Err("overflow fabricated EOF or successful output".to_string());
            }
            capture.step(&mut input, "stdout", 3, &mut scratch);
            if !capture.eof {
                return Err("overflow did not drain to actual read(0)".to_string());
            }
            Ok(())
        }

        struct BrokenReader;
        impl Read for BrokenReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("intentional opened-stream failure"))
            }
        }
        #[test]
        fn opened_stream_error_is_not_eof() -> Result<(), String> {
            let mut capture = OutputCapture::new();
            capture.step(&mut BrokenReader, "stdout", 3, &mut [0u8; CHUNK_BYTES]);
            if capture.eof
                || !capture.failed
                || !capture
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("intentional opened-stream failure"))
            {
                return Err("read error was converted to EOF or its failure was lost".to_string());
            }
            Ok(())
        }

        #[test]
        fn partial_pair_failure_occurs_before_actual_worker_start() -> Result<(), String> {
            for name in ["stderr", "stdin"] {
                HOOKS.with(|hooks| hooks.borrow_mut().fail_pair = Some(name));
                let attempts = super::super::super::linux::spawn_attempts();
                let lease = Arc::new(());
                let mut observed = false;
                let mut failure = refused(
                    run(
                        &shell("cat"),
                        Some(b"owned"),
                        budget(5, 64),
                        lease.clone(),
                        |_| observed = true,
                    ),
                    &format!("prepare {name} socket: injected pair failure"),
                )?;
                if observed
                    || failure.take_cleanup_receipt().is_some()
                    || super::super::super::linux::spawn_attempts() != attempts
                    || !failure.matches_lease(&lease)
                {
                    return Err(
                        "partial pair preparation spawned a worker or dropped custody".to_string(),
                    );
                }
            }
            Ok(())
        }

        #[test]
        fn retained_native_writer_prevents_combined_receipt_and_recovers() -> Result<(), String> {
            HOOKS.with(|hooks| hooks.borrow_mut().hold_writer = Some("stdout"));
            let lease = Arc::new(());
            let mut status = None;
            let started = Instant::now();
            let mut failure = refused(
                run(
                    &shell("printf sent"),
                    None,
                    budget(0, 64),
                    lease.clone(),
                    |observed| status = Some(observed),
                ),
                "stdout actual EOF is unconfirmed",
            )?;
            // Release the owned discriminator before checking assertions.
            HOOKS.with(|hooks| drop(hooks.borrow_mut().held_writer.take()));
            if !status.is_some_and(|status| status.success())
                || failure.take_cleanup_receipt().is_some()
                || !failure.matches_lease(&lease)
                || started.elapsed() < POST_KILL_DRAIN_GRACE
            {
                return Err("retained writer manufactured EOF, cleanup or status".to_string());
            }
            drop(failure);
            let output = run(
                &shell("printf recovered"),
                None,
                budget(0, 64),
                lease,
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            assert_eq!(output.into_parts().1, b"recovered");
            Ok(())
        }

        #[test]
        fn inherited_descendant_writer_retains_group_failure_after_cleanup() -> Result<(), String> {
            let lease = Arc::new(());
            let mut status = None;
            let mut failure = refused(
                run(
                    &shell("printf parent; /usr/bin/sleep 30 &"),
                    None,
                    budget(0, 64),
                    lease.clone(),
                    |observed| status = Some(observed),
                ),
                "primary exited with live group members",
            )?;
            if !status.is_some_and(|status| status.success()) {
                return Err("descendant writer premise lacks actual exit0 primary".to_string());
            }
            let receipt = failure
                .take_cleanup_receipt()
                .ok_or("owned descendant did not settle and drain")?;
            if !receipt.matches_lease(&lease) {
                return Err("descendant cleanup receipt has another custody lease".to_string());
            }
            Ok(())
        }

        #[test]
        fn soft_slice_yield_preserves_real_bidirectional_capture() -> Result<(), String> {
            let before = HOOKS.with(|hooks| {
                let mut hooks = hooks.borrow_mut();
                hooks.delay_endpoint_once = Some(Duration::from_millis(10));
                hooks.delayed_endpoints
            });
            let input = b"slice-input\0\xff";
            let output = run(
                &shell("printf prefix; printf stderr >&2; cat"),
                Some(input),
                budget(input.len(), 64),
                Arc::new(()),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, _) = output.into_parts();
            if !status.success()
                || timeout
                || HOOKS.with(|hooks| hooks.borrow().delayed_endpoints) != before + 1
            {
                return Err("soft scheduling slice did not yield once and recover".to_string());
            }
            let mut expected = b"prefix".to_vec();
            expected.extend_from_slice(input);
            assert_eq!(stdout, expected);
            assert_eq!(stderr, b"stderr");
            Ok(())
        }

        #[test]
        fn actual_endpoint_deadline_overrun_cannot_mint_a_receipt() -> Result<(), String> {
            let before = HOOKS.with(|hooks| {
                let mut hooks = hooks.borrow_mut();
                hooks.delay_endpoint_once = Some(Duration::from_millis(400));
                hooks.delayed_endpoints
            });
            let lease = Arc::new(());
            let mut failure = refused(
                run(
                    &shell("printf held; printf deadline >&2"),
                    None,
                    CompleteCaptureBudget::new(Duration::from_millis(250), 0, 64, 64),
                    lease.clone(),
                    |_| {},
                ),
                "complete endpoint action exceeded its held clock boundary",
            )?;
            if HOOKS.with(|hooks| hooks.borrow().delayed_endpoints) != before + 1
                || !failure.matches_lease(&lease)
                || failure.is_timeout_only()
                || failure.take_cleanup_receipt().is_some()
            {
                return Err(
                    "actual endpoint clock overrun yielded success or cleanup authority"
                        .to_string(),
                );
            }
            let (_, stdout, stderr, _, _) = failure
                .take_failed_observation()
                .ok_or("held-clock failure lost its actual failed observations")?;
            assert_eq!(stdout, b"held");
            assert_eq!(stderr, b"deadline");
            Ok(())
        }

        #[test]
        fn already_eof_child_observed_after_deadline_cannot_pass_capture() -> Result<(), String> {
            HOOKS.with(|hooks| {
                hooks.borrow_mut().delay_first_wait = Some(Duration::from_millis(250));
            });
            let lease = Arc::new(());
            let mut actual_status = None;
            let mut failure = refused(
                run(
                    &shell("exec 1>&- 2>&-; /usr/bin/sleep 0.15"),
                    None,
                    CompleteCaptureBudget::new(Duration::from_millis(100), 0, 64, 64),
                    lease.clone(),
                    |status| actual_status = Some(status),
                ),
                "primary observation exceeded its held execution deadline",
            )?;
            let (status, duration, timed_out) = failure
                .observed_outcome()
                .ok_or("late primary control lost its actual wait data")?;
            if !status.success()
                || actual_status != Some(status)
                || duration < Duration::from_millis(250)
                || timed_out
                || failure.is_timeout_only()
            {
                return Err("late exit0 was not preserved as failed non-timeout data".to_string());
            }
            let receipt = failure
                .take_cleanup_receipt()
                .ok_or("late observed exit0 did not settle and release actual EOF endpoints")?;
            if !receipt.matches_lease(&lease) {
                return Err("late primary cleanup has another custody lease".to_string());
            }
            let (_, stdout, stderr, _, _) = failure
                .take_failed_observation()
                .ok_or("late primary failure lost its empty actual output observations")?;
            assert_eq!(stdout, Vec::<u8>::new());
            assert_eq!(stderr, Vec::<u8>::new());
            Ok(())
        }

        #[test]
        fn blocked_stdin_is_bounded_and_early_close_is_not_success() -> Result<(), String> {
            let input = vec![0xffu8; 2 * 1024 * 1024];
            let mut failure = refused(
                run(
                    &shell("exec 0<&-; printf closed"),
                    Some(&input),
                    budget(input.len(), 64),
                    Arc::new(()),
                    |_| {},
                ),
                "write stdin",
            )?;
            if failure.take_cleanup_receipt().is_none() {
                return Err("early close did not release owned endpoint/group custody".to_string());
            }
            let lease = Arc::new(());
            let mut failure = refused(
                run(
                    &shell("printf timed-out; printf diagnostic >&2; /usr/bin/sleep 30"),
                    Some(&input),
                    CompleteCaptureBudget::new(Duration::from_millis(250), input.len(), 64, 64),
                    lease.clone(),
                    |_| {},
                ),
                "complete worker timed out",
            )?;
            let (observed_status, duration, timed_out) = failure
                .observed_outcome()
                .ok_or("blocked input lost its actual failed wait observation")?;
            if !timed_out || duration > Duration::from_secs(8) || !failure.is_timeout_only() {
                return Err(
                    "blocked input evaded the admitted failure/execution/drain bounds".to_string(),
                );
            }
            let receipt = failure
                .take_cleanup_receipt()
                .ok_or("timeout did not settle the group and actual endpoints")?;
            if !receipt.matches_lease(&lease) {
                return Err("timeout cleanup has another custody lease".to_string());
            }
            assert_eq!(receipt.settled_status(), Some(observed_status));
            let (status, stdout, stderr, failed_duration, failed_timeout) = failure
                .take_failed_observation()
                .ok_or("timeout lost retained failed stdout/stderr data")?;
            assert_eq!(status, observed_status);
            assert_eq!(failed_duration, duration);
            assert_eq!(failed_timeout, timed_out);
            assert_eq!(stdout, b"timed-out");
            assert_eq!(stderr, b"diagnostic");
            if failure.take_cleanup_receipt().is_some()
                || failure.take_failed_observation().is_some()
            {
                return Err("timeout cleanup or failed data was transferred twice".to_string());
            }
            Ok(())
        }

        #[test]
        fn large_binary_backpressure_preserves_both_outputs_and_input_offset() -> Result<(), String>
        {
            let input: Vec<u8> = (0u8..251).cycle().take(2 * 1024 * 1024).collect();
            let script = "head -c 1048576 /dev/zero; head -c 1048576 /dev/zero >&2; cat";
            let output = run(
                &shell(script),
                Some(&input),
                CompleteCaptureBudget::new(
                    Duration::from_mins(2),
                    input.len(),
                    3 * 1024 * 1024,
                    1024 * 1024,
                ),
                Arc::new(()),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, _) = output.into_parts();
            if !status.success() || timeout {
                return Err("large interleaved native worker failed or timed out".to_string());
            }
            if stdout.len() != 3 * 1024 * 1024 {
                return Err("large output length changed before byte comparison".to_string());
            }
            let zeros = vec![0u8; 1024 * 1024];
            assert_eq!(&stdout[..1024 * 1024], zeros.as_slice());
            assert_eq!(&stdout[1024 * 1024..], input.as_slice());
            assert_eq!(stderr, zeros);
            Ok(())
        }

        #[test]
        fn continuously_readable_excess_remains_failed_and_deadline_bounded() -> Result<(), String>
        {
            let mut failure = refused(
                run(
                    &shell("exec cat /dev/zero"),
                    None,
                    CompleteCaptureBudget::new(Duration::from_millis(250), 0, 3, 64),
                    Arc::new(()),
                    |_| {},
                ),
                "stdout exceeds its 3-byte output budget",
            )?;
            if failure.is_timeout_only() {
                return Err(
                    "overflow plus timeout was classified as a pure legacy timeout".to_string(),
                );
            }
            if !failure.observed_outcome().is_some_and(|parts| parts.2) {
                return Err("excess worker lost its actual timeout observation".to_string());
            }
            if failure.take_cleanup_receipt().is_none() {
                return Err("excess worker did not settle and drain after timeout".to_string());
            }
            Ok(())
        }

        #[test]
        fn actual_cargo_json_and_native_eight_mib_stream_keep_existing_admission()
        -> Result<(), String> {
            let fixture = Fixture::new()?;
            std::fs::write(fixture.0.join("Cargo.toml"),
                "[package]\nname=\"complete_capture_json_control\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n")
                .map_err(|error| error.to_string())?;
            std::fs::create_dir(fixture.0.join("src")).map_err(|error| error.to_string())?;
            std::fs::write(fixture.0.join("src/main.rs"), "fn main() {}\n")
                .map_err(|error| error.to_string())?;
            let args = vec![
                "build".to_string(),
                "--offline".to_string(),
                "--manifest-path".to_string(),
                fixture.0.join("Cargo.toml").display().to_string(),
                "--message-format=json".to_string(),
            ];
            let output = CompleteByteCapture::capture(
                (Path::new("cargo"), &args),
                (&fixture.0, None),
                &[],
                (
                    CompleteCaptureBudget::new(
                        Duration::from_mins(2),
                        0,
                        8 * 1024 * 1024,
                        64 * 1024,
                    ),
                    "actual Cargo JSON control",
                ),
                Arc::new(()),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, _, _, timeout, _) = output.into_parts();
            let text = std::str::from_utf8(&stdout).map_err(|error| error.to_string())?;
            if !status.success()
                || timeout
                || !text.contains("\"reason\":\"compiler-artifact\"")
                || !text.contains("\"reason\":\"build-finished\",\"success\":true")
            {
                return Err(
                    "actual Cargo JSON did not survive qualified socket capture".to_string()
                );
            }
            let output = run(
                &shell("head -c 8388608 /dev/zero"),
                None,
                CompleteCaptureBudget::new(Duration::from_mins(2), 0, 8 * 1024 * 1024, 64 * 1024),
                Arc::new(()),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, _, _, timeout, _) = output.into_parts();
            if !status.success()
                || timeout
                || stdout.len() != 8 * 1024 * 1024
                || stdout.iter().any(|byte| *byte != 0)
            {
                return Err("native 8MiB output was changed, truncated or timed out".to_string());
            }
            Ok(())
        }

        #[test]
        fn stream_ceilings_and_invalid_deadlines_refuse_before_transport_setup()
        -> Result<(), String> {
            let fixture = Fixture::new()?;
            let marker = fixture.0.join("started");
            let args = vec![
                "-c".to_string(),
                "printf ran > \"$1\"".to_string(),
                "admission-proof".to_string(),
                marker.display().to_string(),
            ];
            let cases = [
                (
                    CompleteCaptureBudget::new(
                        Duration::from_secs(1),
                        MAX_STREAM_BYTES + 1,
                        64,
                        64,
                    ),
                    "stdin byte budget exceeds",
                ),
                (
                    CompleteCaptureBudget::new(Duration::from_secs(1), 1, MAX_STREAM_BYTES + 1, 64),
                    "stdout byte budget exceeds",
                ),
                (
                    CompleteCaptureBudget::new(Duration::from_secs(1), 1, 64, MAX_STREAM_BYTES + 1),
                    "stderr byte budget exceeds",
                ),
                (
                    CompleteCaptureBudget::new(Duration::from_secs(1), 1, 1024 * 1024 * 1024, 64),
                    "stdout byte budget exceeds",
                ),
                (
                    CompleteCaptureBudget::new(Duration::from_secs(1), 1, 64, 1024 * 1024 * 1024),
                    "stderr byte budget exceeds",
                ),
                (
                    CompleteCaptureBudget::new(Duration::ZERO, 1, 64, 64),
                    "has no deadline",
                ),
                (
                    CompleteCaptureBudget::new(Duration::MAX, 1, 64, 64),
                    "deadline overflow",
                ),
            ];
            for (budget, expected) in cases {
                let attempts = super::super::super::linux::spawn_attempts();
                let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
                let mut observed = false;
                // This input would independently fail its one-byte admission.
                // The execution preflight must run before input copy or sockets.
                let mut failure = refused(
                    run(&args, Some(b"too-large"), budget, Arc::new(()), |_| {
                        observed = true
                    }),
                    expected,
                )?;
                if observed
                    || marker.exists()
                    || failure.take_cleanup_receipt().is_some()
                    || failure.observed_outcome().is_some()
                    || failure.is_timeout_only()
                    || failure.take_failed_observation().is_some()
                    || super::super::super::linux::spawn_attempts() != attempts
                    || HOOKS.with(|hooks| hooks.borrow().pair_attempts) != pairs
                {
                    return Err(
                        "invalid admission allocated transport, spawned or fabricated observations"
                            .to_string(),
                    );
                }
            }
            Ok(())
        }

        #[test]
        fn exact_stream_ceiling_preserves_small_native_capture() -> Result<(), String> {
            let output = run(
                &shell("printf ceiling; printf accepted >&2"),
                None,
                CompleteCaptureBudget::new(
                    Duration::from_secs(2),
                    MAX_STREAM_BYTES,
                    MAX_STREAM_BYTES,
                    MAX_STREAM_BYTES,
                ),
                Arc::new(()),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, _) = output.into_parts();
            if !status.success() || timeout {
                return Err("exact existing stream ceiling refused native small output".to_string());
            }
            assert_eq!(stdout, b"ceiling");
            assert_eq!(stderr, b"accepted");
            Ok(())
        }

        #[test]
        fn oversized_stdin_is_refused_before_actual_worker_start() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let marker = fixture.0.join("started");
            let args = vec![
                "-c".to_string(),
                "printf ran > \"$1\"".to_string(),
                "input-proof".to_string(),
                marker.display().to_string(),
            ];
            let lease = Arc::new(());
            let attempts = super::super::super::linux::spawn_attempts();
            let mut observed = false;
            let mut failure = refused(
                run(&args, Some(b"too-large"), budget(1, 64), lease, |_| {
                    observed = true
                }),
                "stdin exceeds its 1-byte input budget",
            )?;
            if observed
                || marker.exists()
                || failure.take_cleanup_receipt().is_some()
                || super::super::super::linux::spawn_attempts() != attempts
            {
                return Err("oversized input spawned a child or fabricated cleanup".to_string());
            }
            Ok(())
        }
    }
}

#[cfg(all(test, not(target_os = "linux")))]
mod unsupported_tests {
    use super::*;
    #[test]
    fn unsupported_capture_refuses_before_spawn() -> Result<(), String> {
        let lease = Arc::new(());
        let result = CompleteByteCapture::capture(
            (Path::new("must-never-be-launched"), &[]),
            (Path::new("."), None),
            &[],
            (
                CompleteCaptureBudget::new(Duration::from_secs(1), 0, 1, 1),
                "unsupported control",
            ),
            lease,
            |_| {},
        );
        match result {
            Err(mut error) => {
                if error.message().contains("spawn refused")
                    && error.take_cleanup_receipt().is_none()
                {
                    Ok(())
                } else {
                    Err(error.to_string())
                }
            }
            Ok(_) => Err("unsupported platform launched a complete capture".to_string()),
        }
    }
}
