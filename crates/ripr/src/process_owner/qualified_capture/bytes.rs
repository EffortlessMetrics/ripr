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
    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    spawn_attempted: bool,
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
enum TerminalAttempt {
    Unattempted,
    Claimed,
    NoSpawn,
    NoChild,
    AcceptedClosed,
    OwnedFailure,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteTerminalCustodian<L: Any + Send + Sync> {
    held_deadline: Instant,
    lease: Arc<L>,
    attempted: bool,
    failure: Option<CompleteCaptureError>,
    outcome: TerminalAttempt,
    enclosing_deadline: Option<Instant>,
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
            outcome: TerminalAttempt::Unattempted,
            enclosing_deadline: None,
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
        let reason = self
            .failure
            .is_none()
            .then_some("no failed capture has a checked terminal disposition; custody unconfirmed");
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

/// A separately admitted outer owner. Moving custody here grants no capture
/// success, settlement, EOF, source, cleanup or publication authority.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteEnclosingCustodian<L: Any + Send + Sync> {
    inner: Option<CompleteTerminalCustodian<L>>,
    original_deadline: Instant,
    enclosing_deadline: Instant,
    entered: bool,
    disposal_attempted: bool,
    disposal_deadline: Option<Instant>,
    disposal_error: Option<String>,
    progress: super::EnclosingDispositionProgress,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
struct EnclosingReturnGuard<'a, L: Any + Send + Sync> {
    destination: &'a mut Option<CompleteTerminalCustodian<L>>,
    inner: Option<CompleteTerminalCustodian<L>>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> Drop for EnclosingReturnGuard<'_, L> {
    fn drop(&mut self) {
        // The exclusive destination was emptied before the callback. Restore
        // the same owner by value without allocation, reporting or acceptance.
        *self.destination = self.inner.take();
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> CompleteEnclosingCustodian<L> {
    pub(crate) fn admit(
        inner_slot: &mut Option<CompleteTerminalCustodian<L>>,
        enclosing_deadline: Instant,
    ) -> Result<Self, &'static str> {
        let inner = inner_slot
            .as_ref()
            .ok_or("enclosing admission has no original custodian")?;
        let now = Instant::now();
        if inner.attempted
            || inner.failure.is_some()
            || now >= inner.held_deadline
            || inner.held_deadline >= enclosing_deadline
        {
            // The rejected caller still owns exactly the original inner slot.
            return Err("enclosing admission requires an unattempted live original T below U");
        }
        let original_deadline = inner.held_deadline;
        let mut admitted = inner_slot.take();
        if let Some(inner) = admitted.as_mut() {
            // Install the separate clock only during original pre-spawn admission.
            inner.enclosing_deadline = Some(enclosing_deadline);
        }
        Ok(Self {
            inner: admitted,
            original_deadline,
            enclosing_deadline,
            entered: false,
            disposal_attempted: false,
            disposal_deadline: None,
            disposal_error: None,
            progress: super::EnclosingDispositionProgress::new(),
        })
    }

    pub(crate) fn with_inner<R>(
        &mut self,
        work: impl FnOnce(&mut CompleteTerminalCustodian<L>) -> R,
    ) -> Result<R, &'static str> {
        if self.entered || self.disposal_attempted {
            return Err("enclosing inner boundary was already entered; custody retained");
        }
        if Instant::now() >= self.original_deadline {
            return Err("enclosing inner boundary reached the original capture deadline");
        }
        self.entered = true;
        let inner = self
            .inner
            .take()
            .ok_or("enclosing inner owner is unavailable; custody retained")?;
        let mut guard = EnclosingReturnGuard {
            destination: &mut self.inner,
            inner: Some(inner),
        };
        let inner = guard
            .inner
            .as_mut()
            .ok_or("enclosing transfer did not retain its original owner")?;
        Ok(work(inner))
    }

    pub(crate) fn inner(&self) -> Option<&CompleteTerminalCustodian<L>> {
        self.inner.as_ref()
    }

    pub(crate) fn original_deadline(&self) -> Instant {
        self.original_deadline
    }

    pub(crate) fn enclosing_deadline(&self) -> Instant {
        self.enclosing_deadline
    }

    pub(crate) fn matches_lease(&self, lease: &Arc<L>) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|inner| inner.matches_lease(lease))
    }

    pub(crate) fn observed_retained_worker(
        &self,
    ) -> Result<super::ObservedProcessIdentity, String> {
        crate::process_owner::enclosing_time(self.enclosing_deadline)?;
        let inner = self
            .inner
            .as_ref()
            .ok_or("enclosing original owner is unavailable")?;
        let failure = inner
            .failure
            .as_ref()
            .ok_or("enclosing owner has no failed capture")?;
        if inner.held_deadline != self.original_deadline
            || !failure.matches_lease(&inner.lease)
            || failure.custody.len() != 1
        {
            return Err("enclosing retained worker binding mismatched".to_string());
        }
        let custody = &failure.custody[0];
        match (&custody.setup, &custody.group) {
            (Some(setup), None) => setup.observe_enclosing_worker(self.enclosing_deadline),
            (None, Some(group)) => group.observe_enclosing_worker(self.enclosing_deadline),
            _ => Err("enclosing actual primary custody is unavailable".to_string()),
        }
    }

    fn require_actual_physical_closure(&self) -> Result<(), &'static str> {
        let inner = self
            .inner
            .as_ref()
            .ok_or("actual enclosing owner is absent")?;
        let failure = inner
            .failure
            .as_ref()
            .ok_or("actual first capture failure is absent")?;
        if inner.held_deadline != self.original_deadline
            || !failure.matches_lease(&inner.lease)
            || failure.custody.len() != 1
        {
            return Err("actual enclosing identity or first failure changed; custody retained");
        }
        let custody = &failure.custody[0];
        match (&custody.setup, &custody.group) {
            (Some(setup), None) => setup.require_actual_enclosing_physical_closure(&self.progress),
            (None, Some(group)) => group.require_actual_enclosing_physical_closure(&self.progress),
            _ => Err("actual enclosing primary custody is absent; no physical closure"),
        }
    }

    pub(crate) fn take_physically_closed(
        slot: &mut Option<Self>,
    ) -> Result<CompleteEnclosingPhysicalClosure<L>, &'static str> {
        // Borrow first. Refusal never takes, replaces or destroys the owner.
        // These are historical actual physical observations, not timely
        // capture/U admission. No clock, new I/O or deadline is introduced.
        slot.as_ref()
            .ok_or("actual enclosing slot is absent")?
            .require_actual_physical_closure()?;
        match slot.take() {
            Some(enclosing) => Ok(CompleteEnclosingPhysicalClosure { enclosing }),
            None => Err("actual enclosing slot is absent"),
        }
    }

    /// Continue actual physical closeout without renewing capture eligibility.
    /// The first capture error and the original T/U remain in the same owner.
    pub(crate) fn continue_physical_closeout(&mut self) -> super::PhysicalStep {
        let Some(inner) = self.inner.as_mut() else {
            return super::PhysicalStep::Retained("physical enclosing owner is absent".to_string());
        };
        if matches!(inner.outcome, TerminalAttempt::Unattempted) && !inner.attempted
            || matches!(
                inner.outcome,
                TerminalAttempt::NoSpawn
                    | TerminalAttempt::NoChild
                    | TerminalAttempt::AcceptedClosed
            )
        {
            return super::PhysicalStep::NoProcess;
        }
        let Some(failure) = inner.failure.as_mut() else {
            return super::PhysicalStep::Retained(
                "claimed physical capture has no confirmed result; custody retained".to_string(),
            );
        };
        if inner.held_deadline != self.original_deadline
            || !failure.matches_lease(&inner.lease)
            || failure.custody.len() != 1
        {
            return super::PhysicalStep::Retained(
                "physical original custody binding changed".to_string(),
            );
        }
        let custody = &mut failure.custody[0];
        let step = match (&mut custody.setup, &mut custody.group) {
            (Some(setup), None) => setup.continue_enclosing_physical(&mut self.progress),
            (None, Some(group)) => group.continue_enclosing_physical(&mut self.progress),
            _ => super::PhysicalStep::Retained(
                "physical original process owner is absent".to_string(),
            ),
        };
        if let super::PhysicalStep::Retained(error) = &step
            && self.disposal_error.is_none()
        {
            self.disposal_error = Some(error.clone());
        }
        step
    }

    pub(crate) fn try_dispose_failure(
        &mut self,
    ) -> Result<CompleteEnclosingDisposed, CompleteEnclosingFailure<'_, L>> {
        if self.disposal_attempted {
            return Err(CompleteEnclosingFailure { enclosing: self });
        }
        self.disposal_attempted = true;
        let result = self.dispose_failure();
        match result {
            Ok(disposed) => Ok(disposed),
            Err(error) => {
                self.disposal_error = Some(error);
                Err(CompleteEnclosingFailure { enclosing: self })
            }
        }
    }

    fn dispose_failure(&mut self) -> Result<CompleteEnclosingDisposed, String> {
        crate::process_owner::enclosing_time(self.enclosing_deadline)?;
        // Create this shared terminal cutoff once. Helpers cannot restart it.
        let terminal_started = Instant::now();
        let deadline = terminal_started
            .checked_add(crate::process_owner::enclosing_disposal_grace())
            .map_or(self.enclosing_deadline, |phase| {
                phase.min(self.enclosing_deadline)
            });
        self.disposal_deadline = Some(deadline);
        let group_deadline = terminal_started
            .checked_add(super::linux::terminal_settlement_grace())
            .map_or(deadline, |phase| phase.min(deadline));
        self.progress.admit_group_deadline(group_deadline);
        let inner = self
            .inner
            .as_mut()
            .ok_or("enclosing original owner is unavailable")?;
        if inner.held_deadline != self.original_deadline {
            return Err("enclosing original deadline changed; custody retained".to_string());
        }
        let failure = inner
            .failure
            .as_mut()
            .ok_or("enclosing owner has no failed capture; no disposition")?;
        if !failure.matches_lease(&inner.lease) || failure.custody.len() != 1 {
            return Err(
                "enclosing actual lease or custody slot mismatched; custody retained".to_string(),
            );
        }
        let custody = &mut failure.custody[0];
        match (&mut custody.setup, &mut custody.group) {
            (Some(setup), None) => {
                setup.dispose_enclosing_until(deadline, &mut self.progress)?;
            }
            (None, Some(group)) => {
                group.dispose_enclosing_until(deadline, &mut self.progress)?;
            }
            _ => {
                return Err(
                    "enclosing actual process owner is unavailable; custody retained".to_string(),
                );
            }
        }
        crate::process_owner::enclosing_time(deadline)?;
        if !self.progress.no_live_members() {
            return Err("enclosing primary group observation is incomplete".to_string());
        }
        let worker = self
            .progress
            .observed_worker()
            .ok_or("enclosing actual worker observation is unavailable")?;
        let parent = self
            .progress
            .observed_parent()
            .ok_or("enclosing actual parent observation is unavailable")?;
        let status = self
            .progress
            .status()
            .ok_or("enclosing actual direct reap status is unavailable")?;
        let lease: Arc<dyn Any + Send + Sync> = inner.lease.clone();
        crate::process_owner::enclosing_time(deadline)?;
        let disposed = CompleteEnclosingDisposed {
            worker,
            parent,
            status,
            original_deadline: self.original_deadline,
            enclosing_deadline: self.enclosing_deadline,
            lease,
        };
        crate::process_owner::enclosing_time(deadline)?;
        Ok(disposed)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteEnclosingPhysicalClosure<L: Any + Send + Sync> {
    enclosing: CompleteEnclosingCustodian<L>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> CompleteEnclosingPhysicalClosure<L> {
    pub(crate) fn enclosing(&self) -> &CompleteEnclosingCustodian<L> {
        &self.enclosing
    }

    pub(crate) fn message(&self) -> &str {
        self.enclosing
            .inner()
            .and_then(CompleteTerminalCustodian::failure)
            .map_or(
                "actual capture failure is absent",
                CompleteCaptureError::message,
            )
    }
}

/// Negative observed disposition DATA, never a capture/group/EOF receipt.
/// Unknown or live descendants cause refusal instead of this observation.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteEnclosingDisposed {
    worker: super::ObservedProcessIdentity,
    parent: super::ObservedProcessIdentity,
    status: ExitStatus,
    original_deadline: Instant,
    enclosing_deadline: Instant,
    lease: Arc<dyn Any + Send + Sync>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl CompleteEnclosingDisposed {
    pub(crate) fn observed_worker(&self) -> &super::ObservedProcessIdentity {
        &self.worker
    }

    pub(crate) fn observed_parent(&self) -> &super::ObservedProcessIdentity {
        &self.parent
    }

    pub(crate) fn status(&self) -> ExitStatus {
        self.status
    }

    pub(crate) fn original_deadline(&self) -> Instant {
        self.original_deadline
    }

    pub(crate) fn enclosing_deadline(&self) -> Instant {
        self.enclosing_deadline
    }

    pub(crate) fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let supplied: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &supplied)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteEnclosingFailure<'a, L: Any + Send + Sync> {
    enclosing: &'a CompleteEnclosingCustodian<L>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> CompleteEnclosingFailure<'_, L> {
    pub(crate) fn message(&self) -> &str {
        self.enclosing
            .disposal_error
            .as_deref()
            .unwrap_or("enclosing disposition was already attempted; actual custody retained")
    }

    pub(crate) fn enclosing(&self) -> &CompleteEnclosingCustodian<L> {
        self.enclosing
    }
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

/// Concrete externally held bootstrap transport; no terminal Drop proof.
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteControllerTransport<L: Any + Send + Sync>(linux::ControllerTransport<L>);

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl<L: Any + Send + Sync> CompleteControllerTransport<L> {
    pub(crate) fn admit(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        budget: CompleteCaptureBudget,
        clocks: (Instant, Instant, Instant, Instant),
        lease: Arc<L>,
    ) -> Result<Self, String> {
        linux::ControllerTransport::admit(command, source, env_remove, budget, clocks, lease)
            .map(Self)
    }

    pub(crate) fn launch(&mut self) -> Result<(), String> {
        self.0.launch()
    }

    pub(crate) fn step_to_terminal(&mut self) -> super::PhysicalStep {
        self.0.step_to_terminal()
    }

    pub(crate) fn take_terminal(&mut self) -> Result<CompleteControllerTerminal, String> {
        self.0.take_terminal()
    }

    pub(crate) fn matches_lease(&self, lease: &Arc<L>) -> bool {
        Arc::ptr_eq(&self.0.lease, lease)
    }
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) enum CompleteControllerTerminal {
    Accepted(CompleteCapturedBytes),
    Failed(CompleteControllerFailure),
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) struct CompleteControllerFailure {
    message: String,
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    lease: Arc<dyn Any + Send + Sync>,
}

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
impl CompleteControllerFailure {
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
    pub(crate) fn observed_status(&self) -> Option<ExitStatus> {
        self.status
    }
    pub(crate) fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub(crate) fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    pub(crate) fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let erased: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &erased)
    }
}

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
        custodian.outcome = TerminalAttempt::Claimed;
        linux::after_capture_claim();
        let result = match custodian.enclosing_deadline {
            Some(enclosing) => {
                let lease: Arc<dyn Any + Send + Sync> = custodian.lease.clone();
                linux::capture_with_enclosing(
                    command,
                    source,
                    env_remove,
                    execution,
                    (custodian.held_deadline, enclosing),
                    None,
                    lease,
                )
            }
            None => Self::capture_with_deadline(
                command,
                source,
                env_remove,
                execution,
                custodian.held_deadline,
                custodian.lease.clone(),
                |_| {},
            ),
        };
        match result {
            Ok(output) => {
                custodian.outcome = TerminalAttempt::AcceptedClosed;
                Ok(output)
            }
            Err(error) => {
                custodian.outcome = if !error.spawn_attempted {
                    TerminalAttempt::NoSpawn
                } else if error.custody.first().is_some_and(|custody| {
                    custody.group.is_none()
                        && custody
                            .setup
                            .as_ref()
                            .is_some_and(super::GroupSetupFailure::failed_before_child_creation)
                }) {
                    TerminalAttempt::NoChild
                } else {
                    TerminalAttempt::OwnedFailure
                };
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
        custodian.outcome = TerminalAttempt::Claimed;
        linux::after_capture_claim();
        let lease: Arc<dyn Any + Send + Sync> = custodian.lease.clone();
        let result = match custodian.enclosing_deadline {
            Some(enclosing) => linux::capture_with_enclosing(
                command,
                source,
                env_remove,
                execution,
                (custodian.held_deadline, enclosing),
                Some((started, execution_deadline)),
                lease,
            ),
            None => linux::capture_with_execution_window(
                command,
                source,
                env_remove,
                execution,
                (custodian.held_deadline, started, execution_deadline),
                lease,
            ),
        };
        match result {
            Ok(output) => {
                custodian.outcome = TerminalAttempt::AcceptedClosed;
                Ok(output)
            }
            Err(error) => {
                custodian.outcome = if !error.spawn_attempted {
                    TerminalAttempt::NoSpawn
                } else if error.custody.first().is_some_and(|custody| {
                    custody.group.is_none()
                        && custody
                            .setup
                            .as_ref()
                            .is_some_and(super::GroupSetupFailure::failed_before_child_creation)
                }) {
                    TerminalAttempt::NoChild
                } else {
                    TerminalAttempt::OwnedFailure
                };
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
        #[cfg(feature = "lang-rust")]
        unwind_capture_claim: Option<Box<dyn Any + Send>>,
        hold_writer: Option<&'static str>,
        held_writer: Option<UnixStream>,
    }
    #[cfg(test)]
    thread_local! {
        static HOOKS: std::cell::RefCell<Hooks> = std::cell::RefCell::new(Hooks::default());
    }

    #[cfg(all(test, feature = "lang-rust"))]
    pub(super) fn after_capture_claim() {
        if let Some(payload) = HOOKS.with(|hooks| hooks.borrow_mut().unwind_capture_claim.take()) {
            std::panic::resume_unwind(payload);
        }
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
        fn can_step(&mut self, slice: Instant, deadline: Option<Instant>) -> bool {
            let now = Instant::now();
            if deadline.is_some_and(|deadline| now >= deadline) {
                self.late_action = true;
                return false;
            }
            now < slice
        }

        // At most 64 fair rounds / 192 bounded actions. No endpoint can
        // monopolize supervision or restart a partial input write.
        fn pump(&mut self, deadline: Instant) -> bool {
            self.pump_with_clock(Some(deadline))
        }

        #[cfg(all(test, feature = "lang-rust"))]
        fn pump_physical(&mut self) -> bool {
            self.pump_with_clock(None)
        }

        fn pump_with_clock(&mut self, deadline: Option<Instant>) -> bool {
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

    fn prepare_transport(
        input: Option<Vec<u8>>,
        budget: &CompleteCaptureBudget,
        held_deadline: Option<Instant>,
    ) -> Result<(Transport, Stdio, Stdio, Stdio), String> {
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
        })()?;
        let (stdout, stderr, input, stdout_child, stderr_child, stdin_child) = prepared;
        let transport = Transport {
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
        Ok((transport, stdout_child, stderr_child, stdin_child))
    }

    fn prepare_capture_command(
        command: (&Path, &[String]),
        cwd: &Path,
        env_remove: &[&str],
        endpoints: (Stdio, Stdio, Stdio),
        held_deadline: Option<Instant>,
    ) -> Result<Command, String> {
        let (program, args) = command;
        let (stdout_child, stderr_child, stdin_child) = endpoints;
        strict_time(held_deadline)?;
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd);
        strict_time(held_deadline)?;
        for name in env_remove {
            strict_time(held_deadline)?;
            command.env_remove(name);
            strict_time(held_deadline)?;
        }
        command
            .stdout(stdout_child)
            .stderr(stderr_child)
            .stdin(stdin_child);
        strict_time(held_deadline)?;
        Ok(command)
    }

    struct FailureStorage {
        state: Option<Box<[CaptureFailureState]>>,
        custody: Vec<CaptureCustody>,
        #[cfg(all(test, feature = "lang-rust"))]
        spawn_attempted: bool,
    }
    impl FailureStorage {
        fn new() -> Self {
            Self {
                state: None,
                custody: Vec::new(),
                #[cfg(all(test, feature = "lang-rust"))]
                spawn_attempted: false,
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
                #[cfg(all(test, feature = "lang-rust"))]
                spawn_attempted: self.spawn_attempted,
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

    #[cfg(all(test, feature = "lang-rust"))]
    pub(super) struct ControllerTransport<L: Any + Send + Sync> {
        command: Option<Command>,
        transport: Option<Transport>,
        owner: Option<QualifiedGroupOwner>,
        setup: Option<super::super::GroupSetupFailure>,
        progress: super::super::EnclosingDispositionProgress,
        pub(super) lease: Arc<L>,
        started: Instant,
        execution_deadline: Instant,
        held_deadline: Instant,
        enclosing_deadline: Instant,
        launched: bool,
        no_spawn: bool,
        closed: bool,
        taken: bool,
        failure: Option<String>,
        status: Option<ExitStatus>,
        group_settlement: Option<super::super::QualifiedGroupSettlement>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    #[cfg(all(test, feature = "lang-rust"))]
    impl<L: Any + Send + Sync> ControllerTransport<L> {
        pub(super) fn admit(
            command: (&Path, &[String]),
            source: (&Path, Option<&[u8]>),
            env_remove: &[&str],
            budget: CompleteCaptureBudget,
            clocks: (Instant, Instant, Instant, Instant),
            lease: Arc<L>,
        ) -> Result<Self, String> {
            let (started, execution_deadline, held_deadline, enclosing_deadline) = clocks;
            let now = Instant::now();
            let reserved = execution_deadline
                .checked_add(super::super::linux::terminal_settlement_grace())
                .and_then(|end| end.checked_add(POST_KILL_DRAIN_GRACE));
            if budget.timeout.is_zero()
                || started > now
                || now >= execution_deadline
                || execution_deadline <= started
                || reserved.is_none_or(|end| end > held_deadline)
                || held_deadline >= enclosing_deadline
            {
                return Err(
                    "controller original windows lack admitted execution and closeout reserve"
                        .to_string(),
                );
            }
            for limit in [budget.stdin_bytes, budget.stdout_bytes, budget.stderr_bytes] {
                if limit > MAX_STREAM_BYTES {
                    return Err(
                        "controller stream exceeds existing complete capture ceiling".to_string(),
                    );
                }
            }
            let relative_deadline = started
                .checked_add(budget.timeout)
                .ok_or("controller relative execution deadline overflow")?;
            let execution_deadline = execution_deadline.min(relative_deadline).min(held_deadline);
            strict_time(Some(execution_deadline))?;
            let input = source
                .1
                .map(|bytes| {
                    if bytes.len() > budget.stdin_bytes {
                        return Err("controller stdin exceeds its admitted byte budget".to_string());
                    }
                    let mut copy = Vec::new();
                    copy.try_reserve_exact(bytes.len())
                        .map_err(|error| format!("reserve controller stdin: {error}"))?;
                    strict_time(Some(execution_deadline))?;
                    copy.extend_from_slice(bytes);
                    strict_time(Some(execution_deadline))?;
                    Ok(copy)
                })
                .transpose()?;
            let (transport, stdout, stderr, stdin) =
                prepare_transport(input, &budget, Some(execution_deadline))?;
            let command = prepare_capture_command(
                command,
                source.0,
                env_remove,
                (stdout, stderr, stdin),
                Some(execution_deadline),
            )?;
            strict_time(Some(execution_deadline))?;
            Ok(Self {
                command: Some(command),
                transport: Some(transport),
                owner: None,
                setup: None,
                progress: super::super::EnclosingDispositionProgress::new(),
                lease,
                started,
                execution_deadline,
                held_deadline,
                enclosing_deadline,
                launched: false,
                no_spawn: false,
                closed: false,
                taken: false,
                failure: None,
                status: None,
                group_settlement: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }

        fn record_failure(&mut self, error: String) {
            if self.failure.is_none() {
                self.failure = Some(error);
            }
            if let Some(input) = self
                .transport
                .as_mut()
                .and_then(|transport| transport.input.as_mut())
            {
                drop(input.stream.take());
            }
        }

        pub(super) fn launch(&mut self) -> Result<(), String> {
            if self.launched {
                return Err("controller transport launch is one-shot".to_string());
            }
            self.launched = true;
            if let Err(error) = strict_time(Some(self.execution_deadline)) {
                self.no_spawn = true;
                drop(self.command.take());
                self.record_failure(error.clone());
                return Err(error);
            }
            let Some(command) = self.command.take() else {
                self.no_spawn = true;
                let error = "controller prepared command is absent".to_string();
                self.record_failure(error.clone());
                return Err(error);
            };
            match QualifiedGroupOwner::spawn_with_deadline(command, self.held_deadline) {
                Ok(owner) => self.owner = Some(owner),
                Err(error) => {
                    self.no_spawn = error.failed_before_child_creation();
                    let message = error.to_string();
                    self.setup = Some(error);
                    self.record_failure(message.clone());
                    return Err(message);
                }
            }
            let admission = self
                .owner
                .as_mut()
                .ok_or("controller actual owner was not stored")?
                .admit_enclosing_scope(self.enclosing_deadline);
            if let Err(error) = admission {
                self.record_failure(error.clone());
                return Err(error);
            }
            if let Err(error) = strict_time(Some(self.execution_deadline)) {
                self.record_failure(error.clone());
                return Err(error);
            }
            Ok(())
        }

        fn eligibility(&mut self) {
            let now = Instant::now();
            if now >= self.execution_deadline
                || now >= self.held_deadline
                || now >= self.enclosing_deadline
            {
                self.record_failure(
                    "controller original capture eligibility expired; physical owner retained"
                        .to_string(),
                );
            }
        }

        pub(super) fn step_to_terminal(&mut self) -> super::super::PhysicalStep {
            if self.closed {
                return super::super::PhysicalStep::Closed;
            }
            if !self.launched {
                return super::super::PhysicalStep::Retained(
                    "controller transport was not launched".to_string(),
                );
            }
            self.eligibility();
            if let Some(transport) = self.transport.as_mut() {
                if self.failure.is_none() {
                    transport.pump(self.execution_deadline);
                } else {
                    transport.pump_physical();
                }
            }
            self.eligibility();
            let transport_error = self.transport.as_ref().and_then(|transport| {
                transport
                    .stdout_capture
                    .error
                    .as_ref()
                    .or(transport.stderr_capture.error.as_ref())
                    .cloned()
            });
            if let Some(error) = transport_error {
                self.record_failure(error);
            }
            let step = if self.no_spawn {
                super::super::PhysicalStep::Closed
            } else if let Some(owner) = self.owner.as_mut() {
                owner.controller_natural_step(&mut self.progress)
            } else if let Some(setup) = self.setup.as_mut() {
                setup.continue_enclosing_physical(&mut self.progress)
            } else {
                super::super::PhysicalStep::Retained(
                    "controller actual custody is absent".to_string(),
                )
            };
            match step {
                super::super::PhysicalStep::NoProcess => super::super::PhysicalStep::Retained(
                    "controller spawned path returned no-process DATA".to_string(),
                ),
                super::super::PhysicalStep::Pending => super::super::PhysicalStep::Pending,
                super::super::PhysicalStep::Retained(error) => {
                    self.record_failure(error.clone());
                    super::super::PhysicalStep::Retained(error)
                }
                super::super::PhysicalStep::Closed => {
                    self.eligibility();
                    if let Some(owner) = self.owner.as_mut() {
                        self.status = owner.physical_status();
                        if self.group_settlement.is_none() {
                            self.group_settlement = owner.take_settlement();
                        }
                    } else {
                        self.status = self.progress.status();
                    }
                    if let Some(transport) = self.transport.as_mut() {
                        transport.pump_physical();
                        if !(transport.stdout_capture.eof || transport.stdout_capture.failed)
                            || !(transport.stderr_capture.eof || transport.stderr_capture.failed)
                        {
                            return super::super::PhysicalStep::Pending;
                        }
                    }
                    if let Some(transport) = self.transport.take() {
                        let valid = transport.stdout_capture.eof
                            && transport.stderr_capture.eof
                            && !transport.stdout_capture.failed
                            && !transport.stderr_capture.failed
                            && transport.stdout_capture.error.is_none()
                            && transport.stderr_capture.error.is_none()
                            && !transport.late_action
                            && transport
                                .input
                                .as_ref()
                                .is_none_or(|input| input.complete && input.error.is_none());
                        if !valid {
                            self.record_failure(
                                "controller transport lacked error-free input and real EOF"
                                    .to_string(),
                            );
                        }
                        self.stdout = transport.stdout_capture.bytes;
                        self.stderr = transport.stderr_capture.bytes;
                        drop(transport.stdout);
                        drop(transport.stderr);
                        drop(transport.input);
                    }
                    self.eligibility();
                    self.closed = true;
                    super::super::PhysicalStep::Closed
                }
            }
        }

        pub(super) fn take_terminal(&mut self) -> Result<CompleteControllerTerminal, String> {
            if !self.closed || self.taken {
                return Err(
                    "controller terminal extraction requires actual closure and is one-shot"
                        .to_string(),
                );
            }
            self.eligibility();
            self.taken = true;
            let status = self.status;
            let stdout = std::mem::take(&mut self.stdout);
            let stderr = std::mem::take(&mut self.stderr);
            if self.failure.is_none()
                && status.is_some_and(|status| status.success())
                && let (Some(status), Some(group)) = (status, self.group_settlement.take())
            {
                let receipt = CompleteCaptureReceipt {
                    group,
                    lease: self.lease.clone(),
                };
                let captured = CompleteCapturedBytes {
                    status,
                    stdout,
                    stderr,
                    duration: self.started.elapsed(),
                    timed_out: false,
                    receipt,
                };
                self.eligibility();
                if self.failure.is_none() {
                    return Ok(CompleteControllerTerminal::Accepted(captured));
                }
                let (status, stdout, stderr, _, _, _) = captured.into_parts();
                return Ok(CompleteControllerTerminal::Failed(
                    CompleteControllerFailure {
                        message: self
                            .failure
                            .take()
                            .ok_or("controller late refusal was not retained")?,
                        status: Some(status),
                        stdout,
                        stderr,
                        lease: self.lease.clone(),
                    },
                ));
            }
            Ok(CompleteControllerTerminal::Failed(
                CompleteControllerFailure {
                    message: self.failure.take().unwrap_or_else(|| {
                        "controller failed without timely capture acceptance".to_string()
                    }),
                    status,
                    stdout,
                    stderr,
                    lease: self.lease.clone(),
                },
            ))
        }
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
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        enclosing_deadline: Option<Instant>,
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
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                enclosing_deadline: None,
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
                enclosing_deadline: None,
            },
            lease,
            |_| {},
        )
    }

    #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
    pub(super) fn capture_with_enclosing(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        deadlines: (Instant, Instant),
        execution_window: Option<(Instant, Instant)>,
        lease: Arc<dyn Any + Send + Sync>,
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        capture_with_clocks(
            command,
            source,
            env_remove,
            execution,
            CaptureClocks {
                held_deadline: Some(deadlines.0),
                execution_window,
                enclosing_deadline: Some(deadlines.1),
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
                || reserve_end.is_none_or(|end| held_deadline.is_none_or(|held| end > held))
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
        let (mut transport, stdout_child, stderr_child, stdin_child) =
            prepare_transport(input, &budget, held_deadline)
                .map_err(|error| failure_storage.failure(error, None, lease.clone()))?;
        let command = prepare_capture_command(
            (program, args),
            cwd,
            env_remove,
            (stdout_child, stderr_child, stdin_child),
            held_deadline,
        )
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
        #[cfg(all(test, feature = "lang-rust"))]
        {
            failure_storage.spawn_attempted = true;
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
        #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
        if let Some(enclosing) = clocks.enclosing_deadline
            && let Err(error) = owner.admit_enclosing_scope(enclosing)
        {
            // Header/input remains withheld. The actual owner enters exactly
            // the existing pre-admitted error slot on failed initial minting.
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
            #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
            if held_deadline.is_some()
                && (owner.retained_process_count() != 0 || owner.has_enclosing_scope())
            {
                failure_storage.custody.push(CaptureCustody::group(owner));
            }
            #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
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
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                if held_deadline.is_some()
                    && (owner.retained_process_count() != 0 || owner.has_enclosing_scope())
                {
                    failure_storage.custody.push(CaptureCustody::group(owner));
                }
                #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
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
                #[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
                if held_deadline.is_some()
                    && (owner.retained_process_count() != 0 || owner.has_enclosing_scope())
                {
                    failure_storage.custody.push(CaptureCustody::group(owner));
                }
                #[cfg(not(all(test, target_os = "linux", feature = "lang-rust")))]
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
                CompleteEnclosingCustodian, CompleteEnclosingDisposed, CompleteEnclosingFailure,
                CompleteEnclosingPhysicalClosure, CompleteFailedClosed, CompleteTerminalCustodian,
                CompleteTerminalFailure,
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

            fn enclosing_report_to_string(report: CompleteEnclosingFailure<'_, Lease>) -> String {
                // This typed report owns only a borrow; the actual error, child
                // and composite lease remain in the pre-existing outer owner.
                report.message().to_string()
            }

            fn physically_closed_report_to_string(
                closed: CompleteEnclosingPhysicalClosure<Lease>,
            ) -> String {
                closed.message().to_string()
            }

            fn enclosing(
                held: Instant,
                outer: Instant,
                lease: Arc<Lease>,
            ) -> Result<CompleteEnclosingCustodian<Lease>, String> {
                let mut slot = Some(CompleteTerminalCustodian::new(held, lease)?);
                let admitted =
                    CompleteEnclosingCustodian::admit(&mut slot, outer).map_err(str::to_string)?;
                if slot.is_some() {
                    return Err("enclosing admission duplicated the original owner".to_string());
                }
                Ok(admitted)
            }

            fn expire_inner(
                enclosing: &mut CompleteEnclosingCustodian<Lease>,
                script: &str,
            ) -> Result<String, String> {
                enclosing
                    .with_inner(|inner| {
                        crate::process_owner::with_post_spawn_deadline_barrier(|| {
                            match capture(script, None, budget(0, 64), inner) {
                                Err(report) => Ok(report),
                                Ok(_) => Err("post-spawn crossing admitted capture".to_string()),
                            }
                        })
                    })
                    .map_err(str::to_string)?
            }

            fn require_enclosing_binding(
                enclosing: &CompleteEnclosingCustodian<Lease>,
                lease: &Arc<Lease>,
                held: Instant,
                outer: Instant,
            ) -> Result<(), String> {
                let inner = enclosing
                    .inner()
                    .ok_or("enclosing original owner was lost")?;
                let failure = inner.failure().ok_or("enclosing actual error was lost")?;
                if inner.original_deadline() != held
                    || enclosing.original_deadline() != held
                    || enclosing.enclosing_deadline() != outer
                    || !enclosing.matches_lease(lease)
                    || !failure.matches_lease(lease)
                    || !failure
                        .message()
                        .contains("spawn crossed its held deadline")
                    || !inner.attempted
                    || inner.retained_process_count() != 1
                {
                    return Err(
                        "enclosing transfer changed the original capture or lease".to_string()
                    );
                }
                Ok(())
            }

            fn require_enclosing_disposed(
                disposed: &CompleteEnclosingDisposed,
                before: &super::super::super::super::ObservedProcessIdentity,
                lease: &Arc<Lease>,
                held: Instant,
                outer: Instant,
                expected_success: bool,
            ) -> Result<(), String> {
                let actual = disposed.observed_worker();
                let parent = disposed.observed_parent();
                if actual.pid() != before.pid()
                    || actual.start() != before.start()
                    || actual.group() != before.group()
                    || actual.parent() != before.parent()
                    || parent.pid() != std::process::id()
                    || parent.start() == 0
                    || actual.parent() != parent.pid()
                    || disposed.status().success() != expected_success
                    || disposed.original_deadline() != held
                    || disposed.enclosing_deadline() != outer
                    || !disposed.matches_lease(lease)
                    || Instant::now() >= outer
                {
                    return Err(
                        "enclosing direct disposition changed actual identity or clocks"
                            .to_string(),
                    );
                }
                Ok(())
            }

            #[test]
            fn enclosing_by_value_return_disposes_real_expired_child_under_pre_admitted_u()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(500);
                let outer = started + Duration::from_secs(5);
                let (lease, dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                std::thread::sleep(Duration::from_millis(25));
                let report = expire_inner(&mut owner, "exec /usr/bin/sleep 30")?;
                if !report.contains("spawn crossed its held deadline") {
                    return Err(format!("unexpected inner report: {report}"));
                }
                let before = owner.observed_retained_worker()?;
                let checks = require_enclosing_binding(&owner, &lease, held, outer);
                let disposed = owner
                    .try_dispose_failure()
                    .map_err(enclosing_report_to_string)?;
                checks?;
                require_enclosing_disposed(&disposed, &before, &lease, held, outer, false)?;
                let inner = owner.inner.as_mut().ok_or("original inner slot lost")?;
                let child = retained_child(inner)?;
                if child.bounded_drop_until != Some(held)
                    || child
                        .try_wait()
                        .map_err(|error| error.to_string())?
                        .is_none()
                    || inner.take_cleanup_receipt().is_some()
                    || inner.try_closeout_failure().is_ok()
                    || dropped.load(Ordering::SeqCst)
                {
                    return Err(
                        "enclosing disposal rebased capture or fabricated its receipt".to_string(),
                    );
                }
                match owner.try_dispose_failure() {
                    Err(error) if error.enclosing().matches_lease(&lease) => Ok(()),
                    Err(error) => Err(error.message().to_string()),
                    Ok(_) => Err("enclosing negative disposition was replayed".to_string()),
                }
            }

            #[test]
            fn enclosing_by_value_guard_restores_real_owner_after_report_unwind()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(100);
                let outer = started + Duration::from_secs(5);
                let (lease, dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let panic_payload: Box<dyn Any + Send> = Box::new("inner borrowed report");
                let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _outcome = owner.with_inner(|inner| {
                        crate::process_owner::with_post_spawn_deadline_barrier(|| {
                            let args = shell("exec /usr/bin/sleep 30");
                            if let Err(report) = CompleteByteCapture::capture_with_terminal_custody(
                                (Path::new("/bin/sh"), &args),
                                (Path::new("/"), None),
                                &[],
                                (budget(0, 64), "enclosing real unwind"),
                                inner,
                            ) {
                                let _text = report.to_string();
                                std::panic::resume_unwind(panic_payload);
                            }
                        });
                    });
                }));
                let before = owner.observed_retained_worker()?;
                let checks = require_enclosing_binding(&owner, &lease, held, outer);
                let disposed = owner
                    .try_dispose_failure()
                    .map_err(enclosing_report_to_string)?;
                checks?;
                if unwound.is_ok() || dropped.load(Ordering::SeqCst) {
                    return Err(
                        "unwind lost the actual outside owner or resource lease".to_string()
                    );
                }
                require_enclosing_disposed(&disposed, &before, &lease, held, outer, false)
            }

            #[test]
            fn enclosing_admission_and_reentry_never_replace_the_original_slot()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_secs(1);
                let outer = started + Duration::from_secs(3);
                let (lease, _dropped) = lease()?;
                let mut slot = Some(CompleteTerminalCustodian::new(held, lease.clone())?);
                match CompleteEnclosingCustodian::admit(&mut slot, held) {
                    Err(_) => {}
                    Ok(_) => return Err("enclosing admission accepted T equal to U".to_string()),
                }
                let rejected = slot
                    .as_ref()
                    .ok_or("rejected admission dropped original slot")?;
                if rejected.original_deadline() != held || !rejected.matches_lease(&lease) {
                    return Err("rejected admission replaced original state".to_string());
                }
                let mut owner =
                    CompleteEnclosingCustodian::admit(&mut slot, outer).map_err(str::to_string)?;
                owner
                    .with_inner(|inner| {
                        inner.attempted = true;
                    })
                    .map_err(str::to_string)?;
                match owner.with_inner(|_| ()) {
                    Err(_) if owner.matches_lease(&lease) => {}
                    Err(error) => return Err(error.to_string()),
                    Ok(()) => {
                        return Err("enclosing boundary accepted a second attempt".to_string());
                    }
                }
                let mut used = owner.inner.take();
                match CompleteEnclosingCustodian::admit(&mut used, outer) {
                    Err(_) if used.as_ref().is_some_and(|inner| inner.attempted) => Ok(()),
                    Err(error) => Err(error.to_string()),
                    Ok(_) => Err("enclosing admission replaced an attempted owner".to_string()),
                }
            }

            #[test]
            fn enclosing_real_live_descendant_refuses_before_primary_reap() -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(100);
                let outer = started + Duration::from_secs(5);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let _report =
                    expire_inner(&mut owner, "/usr/bin/sleep 0.5 & exec /usr/bin/sleep 30")?;
                let before = owner.observed_retained_worker()?;
                let refused = match owner.try_dispose_failure() {
                    Err(error) => error.message().contains("live descendants remain"),
                    Ok(_) => false,
                };
                let inner = owner
                    .inner
                    .as_mut()
                    .ok_or("descendant refusal lost inner")?;
                let child = retained_child(inner)?;
                let original = child.bounded_drop_until;
                // Controlled finite descendant exits naturally. This fixture
                // teardown uses the ALREADY admitted U and grants no disposition.
                std::thread::sleep(Duration::from_millis(600));
                let mut status = None;
                child.enclosing_reap_until(outer, &mut status)?;
                if !refused
                    || original != Some(held)
                    || status.is_none()
                    || before.pid() != child.id()
                    || owner.progress.status().is_some()
                    || owner.progress.no_live_members()
                    || !owner.matches_lease(&lease)
                {
                    return Err(
                        "live-descendant refusal reaped, replaced or certified custody".to_string(),
                    );
                }
                Ok(())
            }

            #[test]
            fn enclosing_late_after_real_reap_retains_status_without_disposition()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(50);
                let outer = started + Duration::from_millis(250);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let _report = expire_inner(&mut owner, "exec /usr/bin/sleep 30")?;
                let failed = crate::process_owner::with_enclosing_after_reap_delay(
                    Duration::from_millis(300),
                    || match owner.try_dispose_failure() {
                        Err(error) => error.message().contains("admitted ceiling"),
                        Ok(_) => false,
                    },
                );
                let inner = owner.inner.as_mut().ok_or("late disposition lost inner")?;
                let child = retained_child(inner)?;
                let actual = child.try_wait().map_err(|error| error.to_string())?;
                if !failed
                    || actual.is_none()
                    || owner.progress.status() != actual
                    || owner.original_deadline() != held
                    || owner.enclosing_deadline() != outer
                    || !owner.matches_lease(&lease)
                    || !owner.progress.no_live_members()
                {
                    return Err(
                        "late actual reap discarded status or granted disposition".to_string()
                    );
                }
                match owner.try_dispose_failure() {
                    Err(_) => Ok(()),
                    Ok(_) => Err("late enclosing disposition was retried".to_string()),
                }
            }

            #[test]
            fn enclosing_qualified_refusal_keeps_original_flags_and_deadline() -> Result<(), String>
            {
                let started = Instant::now();
                let held = started + Duration::from_millis(100);
                let outer = started + Duration::from_secs(5);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                HOOKS.with(|hooks| {
                    hooks.borrow_mut().delay_first_wait = Some(Duration::from_millis(150));
                });
                let report = owner
                    .with_inner(|inner| {
                        capture(
                            "exec 1>&- 2>&-; exec /usr/bin/sleep 30",
                            None,
                            budget(0, 64),
                            inner,
                        )
                    })
                    .map_err(str::to_string)?;
                if report.is_ok() {
                    return Err("qualified held expiry admitted capture".to_string());
                }
                let before = owner.observed_retained_worker()?;
                let disposed = owner
                    .try_dispose_failure()
                    .map_err(enclosing_report_to_string)?;
                require_enclosing_disposed(&disposed, &before, &lease, held, outer, false)?;
                let inner = owner
                    .inner
                    .as_mut()
                    .ok_or("qualified disposal lost inner")?;
                let failure = inner
                    .failure
                    .as_mut()
                    .ok_or("qualified actual error lost")?;
                let group = failure.custody[0]
                    .group
                    .as_ref()
                    .ok_or("qualified owner lost")?;
                if group.enclosing_failure_state() != (Some(held), true, false, false, false) {
                    return Err(
                        "enclosing endpoint requalified or reset failed capture".to_string()
                    );
                }
                Ok(())
            }

            #[test]
            fn enclosing_initial_scope_disposes_real_qualified_descendants() -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(100);
                let outer = started + Duration::from_secs(5);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                HOOKS.with(|hooks| {
                    hooks.borrow_mut().delay_first_wait = Some(Duration::from_millis(150));
                });
                let report = owner
                    .with_inner(|inner| {
                        capture(
                            "exec 1>&- 2>&-; /usr/bin/sleep 30 & exec /usr/bin/sleep 30",
                            None,
                            budget(0, 64),
                            inner,
                        )
                    })
                    .map_err(str::to_string)?;
                let primary = match report {
                    Err(message) => message,
                    Ok(_) => return Err("qualified descendant expiry admitted capture".to_string()),
                };
                let before = owner.observed_retained_worker()?;
                let checks = (|| {
                    let disposed = owner
                        .try_dispose_failure()
                        .map_err(enclosing_report_to_string)?;
                    require_enclosing_disposed(&disposed, &before, &lease, held, outer, false)?;
                    let inner = owner
                        .inner
                        .as_mut()
                        .ok_or("descendant original owner lost")?;
                    let failure = inner
                        .failure
                        .as_mut()
                        .ok_or("descendant first failure lost")?;
                    let group = failure.custody[0]
                        .group
                        .as_ref()
                        .ok_or("descendant group lost")?;
                    if failure.message() != primary
                        || group.enclosing_failure_state()
                            != (Some(held), true, false, false, false)
                        || failure.take_cleanup_receipt().is_some()
                        || inner.try_closeout_failure().is_ok()
                    {
                        return Err(
                            "negative group disposal altered failure or minted receipt".to_string()
                        );
                    }
                    // This is the endpoint's actual no-live-before-reap audit,
                    // not a receipt inferred from a retained handle count.
                    if !owner.progress.no_live_members() || owner.progress.status().is_none() {
                        return Err(
                            "descendant negative disposition lost actual observations".to_string()
                        );
                    }
                    Ok(())
                })();
                let inner = owner
                    .inner
                    .as_mut()
                    .ok_or("descendant fixture owner lost")?;
                fixture_closeout(inner, checks)
            }

            #[test]
            fn enclosing_uncertain_new_signal_helper_preserves_unreaped_primary()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(100);
                let outer = started + Duration::from_secs(5);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                HOOKS.with(|hooks| {
                    hooks.borrow_mut().delay_first_wait = Some(Duration::from_millis(150));
                });
                let report = owner
                    .with_inner(|inner| {
                        capture(
                            "exec 1>&- 2>&-; /usr/bin/sleep 30 & exec /usr/bin/sleep 30",
                            None,
                            budget(0, 64),
                            inner,
                        )
                    })
                    .map_err(str::to_string)?;
                let primary = match report {
                    Err(message) => message,
                    Ok(_) => return Err("signal uncertainty control admitted capture".to_string()),
                };
                let before = owner.observed_retained_worker()?;
                let message =
                    crate::process_owner::with_post_spawn_deadline_barrier(|| {
                        match owner.try_dispose_failure() {
                            Err(report) => {
                                if !report.enclosing().matches_lease(&lease) {
                                    return Err(
                                        "helper refusal transferred the actual lease".to_string()
                                    );
                                }
                                Ok(report.message().to_string())
                            }
                            Ok(_) => Err("unknown helper minted negative disposition".to_string()),
                        }
                    })?;
                let checks = (|| {
                    if !message.contains("spawn crossed its held deadline")
                        || owner.original_deadline() != held
                        || owner.enclosing_deadline() != outer
                        || owner.progress.status().is_some()
                        || owner.progress.no_live_members()
                    {
                        return Err(
                            "helper uncertainty released primary pin or rebased clocks".to_string()
                        );
                    }
                    let after = owner.observed_retained_worker()?;
                    if after.pid() != before.pid()
                        || after.start() != before.start()
                        || after.group() != before.group()
                        || after.parent() != before.parent()
                    {
                        return Err("helper uncertainty replaced the pinned primary".to_string());
                    }
                    let inner = owner
                        .inner
                        .as_mut()
                        .ok_or("helper uncertainty original owner lost")?;
                    let failure = inner
                        .failure
                        .as_mut()
                        .ok_or("helper uncertainty first error lost")?;
                    let group = failure.custody[0]
                        .group
                        .as_ref()
                        .ok_or("helper uncertainty group lost")?;
                    let (helper, helper_clock) =
                        group.observe_pending_enclosing_helper_for_test(outer)?;
                    if helper.pid() == before.pid()
                        || helper.parent() != std::process::id()
                        || helper_clock != owner.progress.group_deadline()
                        || inner.held_deadline != held
                        || failure.message() != primary
                        || group.retained_process_count() != 2
                        || group.enclosing_failure_state()
                            != (Some(held), true, false, false, false)
                        || failure.take_cleanup_receipt().is_some()
                    {
                        return Err(
                            "actual pending helper or original refusal was discarded".to_string()
                        );
                    }
                    match owner.try_dispose_failure() {
                        Err(_) => Ok(()),
                        Ok(_) => Err("helper refusal reset its one-shot cutoff".to_string()),
                    }
                })();
                let mut slot = Some(owner);
                let checks =
                    checks.and_then(
                        |()| match CompleteEnclosingCustodian::take_physically_closed(&mut slot) {
                            Err(_) => {
                                let actual =
                                    slot.as_ref().ok_or("live helper moved actual owner")?;
                                let inner = actual
                                    .inner()
                                    .ok_or("live helper lost original custodian")?;
                                if inner.retained_process_count() != 2
                                    || actual.original_deadline() != held
                                    || actual.enclosing_deadline() != outer
                                    || !actual.matches_lease(&lease)
                                {
                                    return Err(
                                        "physical refusal discarded live helper or primary"
                                            .to_string(),
                                    );
                                }
                                Ok(())
                            }
                            Ok(_) => {
                                Err("unreaped helper manufactured physical closure".to_string())
                            }
                        },
                    );
                let owner = slot
                    .as_mut()
                    .ok_or("helper uncertainty actual fixture owner lost")?;
                let inner = owner
                    .inner
                    .as_mut()
                    .ok_or("helper uncertainty fixture owner lost")?;
                fixture_closeout(inner, checks)
            }

            fn late_reap_control(remove_actual_audit: bool) -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(200);
                let outer = started + Duration::from_secs(5);
                let (lease, _dropped) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let report = crate::process_owner::with_enclosing_after_try_wait_delay(
                    Duration::from_millis(300),
                    || {
                        owner
                            .with_inner(|inner| capture("exit 0", None, budget(0, 64), inner))
                            .map_err(str::to_string)
                    },
                )?;
                let primary = match report {
                    Err(message) => message,
                    Ok(_) => return Err("late actual reap admitted capture".to_string()),
                };
                let before = {
                    let inner = owner
                        .inner
                        .as_mut()
                        .ok_or("late reap original owner lost")?;
                    let failure = inner.failure.as_mut().ok_or("late reap first error lost")?;
                    let group = failure.custody[0]
                        .group
                        .as_mut()
                        .ok_or("late reap group lost")?;
                    let observed = group.initial_enclosing_worker_for_test()?;
                    if remove_actual_audit {
                        // This destructive negative control removes real DATA.
                        // It never fabricates an empty audit or a positive grant.
                        group.remove_enclosing_empty_audit_for_test()?;
                    }
                    observed
                };
                let checks = (|| {
                    match owner.observed_retained_worker() {
                        Err(error) if error.contains("actually reaped; no PID lookup") => {}
                        Err(error) => return Err(format!("wrong reaped-worker refusal: {error}")),
                        Ok(_) => {
                            return Err("reaped worker getter revisited numeric PID".to_string());
                        }
                    }
                    let result = owner.try_dispose_failure();
                    if remove_actual_audit {
                        let message = match result {
                            Err(report) => report.message().to_string(),
                            Ok(_) => {
                                return Err(
                                    "missing empty audit admitted U disposition".to_string()
                                );
                            }
                        };
                        if !message.contains("lacks actual before-reap empty audit") {
                            return Err(format!("wrong missing-audit refusal: {message}"));
                        }
                    } else {
                        let disposed = result.map_err(enclosing_report_to_string)?;
                        require_enclosing_disposed(&disposed, &before, &lease, held, outer, true)?;
                        if !disposed.status().success() {
                            return Err(
                                "retained actual successful native reap was altered".to_string()
                            );
                        }
                    }
                    let same_lease = owner.matches_lease(&lease);
                    let inner = owner
                        .inner
                        .as_mut()
                        .ok_or("late reap original owner lost")?;
                    let failure = inner.failure.as_mut().ok_or("late reap first error lost")?;
                    let group = failure.custody[0]
                        .group
                        .as_mut()
                        .ok_or("late reap group lost")?;
                    let original_flags = group.enclosing_failure_state();
                    let child = group.fixture_child();
                    let actual_reap = child.enclosing_observed_reap;
                    let actual_drop_clock = child.bounded_drop_until;
                    if failure.message() != primary
                        || actual_reap.is_none_or(|(status, at)| !status.success() || at >= outer)
                        || actual_drop_clock != Some(held)
                        || original_flags != (Some(held), true, false, false, false)
                        || (remove_actual_audit && owner.progress.status().is_some())
                        || (remove_actual_audit && owner.progress.no_live_members())
                        || !same_lease
                        || failure.take_cleanup_receipt().is_some()
                    {
                        return Err(
                            "late real reap changed old refusal or inferred a missing audit"
                                .to_string(),
                        );
                    }
                    Ok(())
                })();
                let mut slot = Some(owner);
                let checks = checks.and_then(|()| {
                    if remove_actual_audit {
                        match CompleteEnclosingCustodian::take_physically_closed(&mut slot) {
                            Err(_) => {
                                let actual =
                                    slot.as_ref().ok_or("missing audit moved actual owner")?;
                                if actual.original_deadline() != held
                                    || actual.enclosing_deadline() != outer
                                    || !actual.matches_lease(&lease)
                                {
                                    return Err(
                                        "physical refusal replaced original missing-audit owner"
                                            .to_string(),
                                    );
                                }
                                Ok(())
                            }
                            Ok(_) => Err("missing audit manufactured physical closure".to_string()),
                        }
                    } else {
                        Ok(())
                    }
                });
                let owner = slot.as_mut().ok_or("late reap actual fixture owner lost")?;
                let inner = owner.inner.as_mut().ok_or("late reap fixture owner lost")?;
                fixture_closeout(inner, checks)
            }

            #[test]
            fn enclosing_recorded_reap_before_late_t_check_preserves_negative_closeout()
            -> Result<(), String> {
                late_reap_control(false)
            }

            #[test]
            fn enclosing_late_reap_without_actual_empty_audit_stays_retained() -> Result<(), String>
            {
                late_reap_control(true)
            }

            #[test]
            fn enclosing_native_reap_observed_after_u_allows_only_physical_extraction()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(500);
                let outer = started + Duration::from_millis(700);
                let (lease, dropped) = lease()?;
                let weak = Arc::downgrade(&lease);
                let mut owner = enclosing(held, outer, lease.clone())?;
                let report = crate::process_owner::with_enclosing_before_try_wait_delay(
                    Duration::from_millis(800),
                    || {
                        owner
                            .with_inner(|inner| capture("exit 0", None, budget(0, 64), inner))
                            .map_err(str::to_string)
                    },
                )?;
                let primary = match report {
                    Err(message) => message,
                    Ok(_) => return Err("after-U native reap admitted capture".to_string()),
                };
                let (before, status, reaped_at, flags) = {
                    let inner = owner.inner.as_mut().ok_or("after-U original owner lost")?;
                    let failure = inner.failure.as_mut().ok_or("after-U first error lost")?;
                    let group = failure.custody[0]
                        .group
                        .as_mut()
                        .ok_or("after-U actual group lost")?;
                    let before = group.initial_enclosing_worker_for_test()?;
                    let flags = group.enclosing_failure_state();
                    let (status, reaped_at) = group
                        .fixture_child()
                        .enclosing_observed_reap
                        .ok_or("after-U control did not record actual native Some")?;
                    (before, status, reaped_at, flags)
                };
                let timely = match owner.try_dispose_failure() {
                    Err(report) => report.message().to_string(),
                    Ok(_) => return Err("after-U reap minted timely disposition".to_string()),
                };
                let mut slot = Some(owner);
                let checks = (|| {
                    if reaped_at < outer
                        || !status.success()
                        || flags != (Some(held), true, false, false, false)
                        || !timely.contains("admitted ceiling")
                    {
                        return Err(
                            "actual native reap did not cross U while preserving T refusal"
                                .to_string(),
                        );
                    }
                    let closed = CompleteEnclosingCustodian::take_physically_closed(&mut slot)
                        .map_err(str::to_string)?;
                    let owner = closed.enclosing();
                    let inner = owner
                        .inner()
                        .ok_or("after-U physical move lost original owner")?;
                    let failure = inner
                        .failure()
                        .ok_or("after-U physical move lost first error")?;
                    if slot.is_some()
                        || closed.message() != primary
                        || owner.original_deadline() != held
                        || owner.enclosing_deadline() != outer
                        || !owner.matches_lease(&lease)
                        || failure.state().is_some_and(|state| {
                            state.receipt.is_some()
                                || state.terminal.as_ref().is_some_and(|evidence| {
                                    evidence.group_only.is_some() || evidence.release.is_some()
                                })
                        })
                        || dropped.load(Ordering::SeqCst)
                    {
                        return Err(
                            "after-U physical move changed ownership or granted timely evidence"
                                .to_string(),
                        );
                    }
                    if before.start() == 0 || before.pid() != before.group() {
                        return Err("after-U initial owned identity was not genuine".to_string());
                    }
                    drop(lease);
                    let message = physically_closed_report_to_string(closed);
                    if message != primary
                        || !dropped.load(Ordering::SeqCst)
                        || weak.upgrade().is_some()
                    {
                        return Err(
                            "after-U physical report retained or dropped the wrong lease"
                                .to_string(),
                        );
                    }
                    match CompleteEnclosingCustodian::take_physically_closed(&mut slot) {
                        Err(_) => Ok(()),
                        Ok(_) => Err("after-U physical closure replayed".to_string()),
                    }
                })();
                if let Some(owner) = slot.as_mut()
                    && let Some(inner) = owner.inner.as_mut()
                {
                    fixture_closeout(inner, checks)
                } else {
                    checks
                }
            }

            #[test]
            fn enclosing_late_physical_reap_moves_same_owner_without_timely_authority()
            -> Result<(), String> {
                let started = Instant::now();
                let held = started + Duration::from_millis(50);
                let outer = started + Duration::from_millis(250);
                let (lease, dropped) = lease()?;
                let weak = Arc::downgrade(&lease);
                let mut owner = enclosing(held, outer, lease.clone())?;
                let primary = expire_inner(&mut owner, "exec /usr/bin/sleep 30")?;
                let before = owner.observed_retained_worker()?;
                let timely = crate::process_owner::with_enclosing_after_reap_delay(
                    Duration::from_millis(300),
                    || match owner.try_dispose_failure() {
                        Err(report) => Ok(report.message().to_string()),
                        Ok(_) => {
                            Err("late physical control minted timely U disposition".to_string())
                        }
                    },
                )?;
                let mut slot = Some(owner);
                let checks = (|| {
                    if !timely.contains("admitted ceiling") || Instant::now() < outer {
                        return Err("real post-reap U crossing was not exercised".to_string());
                    }
                    let closed = CompleteEnclosingCustodian::take_physically_closed(&mut slot)
                        .map_err(str::to_string)?;
                    let owner = closed.enclosing();
                    let inner = owner
                        .inner()
                        .ok_or("physical closure lost original owner")?;
                    let failure = inner
                        .failure()
                        .ok_or("physical closure lost actual first error")?;
                    if slot.is_some()
                        || closed.message() != primary
                        || owner.original_deadline() != held
                        || owner.enclosing_deadline() != outer
                        || !owner.matches_lease(&lease)
                        || owner.progress.observed_worker().is_none_or(|actual| {
                            actual.pid() != before.pid()
                                || actual.start() != before.start()
                                || actual.group() != before.group()
                                || actual.parent() != before.parent()
                        })
                        || owner.progress.status().is_none()
                        || !owner.progress.no_live_members()
                        || failure.state().is_some_and(|state| {
                            state.receipt.is_some() || state.terminal.is_some()
                        })
                        || dropped.load(Ordering::SeqCst)
                        || weak.upgrade().is_none()
                    {
                        return Err(
                            "physical move changed ownership or fabricated timely authority"
                                .to_string(),
                        );
                    }
                    drop(lease);
                    if physically_closed_report_to_string(closed) != primary
                        || !dropped.load(Ordering::SeqCst)
                        || weak.upgrade().is_some()
                    {
                        return Err(
                            "physical-only terminal report failed actual closed-owner disposition"
                                .to_string(),
                        );
                    }
                    match CompleteEnclosingCustodian::take_physically_closed(&mut slot) {
                        Err(_) => Ok(()),
                        Ok(_) => {
                            Err("physical closure was replayed from an empty slot".to_string())
                        }
                    }
                })();
                if let Some(owner) = slot.as_mut() {
                    let inner = owner.inner.as_mut().ok_or("physical fixture owner lost")?;
                    fixture_closeout(inner, checks)
                } else {
                    checks
                }
            }

            fn finish_enclosing_physical(
                owner: &mut CompleteEnclosingCustodian<Lease>,
            ) -> Result<(), String> {
                let fixture_limit = Instant::now() + Duration::from_secs(5);
                loop {
                    match owner.continue_physical_closeout() {
                        crate::process_owner::PhysicalStep::Closed => return Ok(()),
                        crate::process_owner::PhysicalStep::NoProcess => {
                            return Err("spawned control returned no-process DATA".to_string());
                        }
                        crate::process_owner::PhysicalStep::Retained(error) => return Err(error),
                        crate::process_owner::PhysicalStep::Pending => {}
                    }
                    if Instant::now() >= fixture_limit {
                        return Err("physical fixture progress did not complete".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }

            #[test]
            fn physical_post_u_report_discard_and_unwind_keep_same_actual_custody()
            -> Result<(), String> {
                for unwind in [false, true] {
                    let started = Instant::now();
                    let held = started + Duration::from_millis(150);
                    let outer = started + Duration::from_millis(350);
                    let (lease, dropped) = lease()?;
                    let mut slot = Some(enclosing(held, outer, lease.clone())?);
                    let checks = (|| {
                        let owner = slot.as_mut().ok_or("physical enclosing owner absent")?;
                        let primary = if unwind {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let _attempt = owner.with_inner(|inner| {
                                        crate::process_owner::with_post_spawn_deadline_barrier(
                                            || match capture(
                                                "exec /usr/bin/sleep 30",
                                                None,
                                                budget(0, 64),
                                                inner,
                                            ) {
                                                Err(message) => {
                                                    std::panic::resume_unwind(Box::new(message))
                                                }
                                                Ok(_) => Err::<(), String>(
                                                    "expired spawn admitted capture".to_string(),
                                                ),
                                            },
                                        )
                                    });
                                }));
                            match result {
                                Err(payload) => *payload.downcast::<String>().map_err(|_| {
                                    "actual report unwind lost its diagnostic".to_string()
                                })?,
                                Ok(()) => return Err("actual report did not unwind".to_string()),
                            }
                        } else {
                            expire_inner(owner, "exec /usr/bin/sleep 30")?
                        };
                        let before = owner.observed_retained_worker()?;
                        std::thread::sleep(
                            outer.saturating_duration_since(Instant::now())
                                + Duration::from_millis(20),
                        );
                        require_enclosing_binding(owner, &lease, held, outer)?;
                        if !primary.contains("spawn crossed its held deadline")
                            || dropped.load(Ordering::SeqCst)
                        {
                            return Err(
                                "physical report boundary changed the real failure".to_string()
                            );
                        }
                        finish_enclosing_physical(owner)?;
                        if owner.progress.observed_worker().is_none_or(|actual| {
                            actual.pid() != before.pid()
                                || actual.start() != before.start()
                                || actual.parent() != before.parent()
                                || actual.group() != before.group()
                        }) || owner
                            .inner()
                            .and_then(CompleteTerminalCustodian::failure)
                            .is_none_or(|failure| {
                                failure.state().is_some_and(|state| state.receipt.is_some())
                            })
                        {
                            return Err("physical completion changed identity or made a receipt"
                                .to_string());
                        }
                        let mut closed =
                            CompleteEnclosingCustodian::take_physically_closed(&mut slot)
                                .map_err(str::to_string)?;
                        if !closed.enclosing().matches_lease(&lease)
                            || closed.enclosing().original_deadline() != held
                            || closed.enclosing().enclosing_deadline() != outer
                            || closed.message() != primary
                        {
                            return Err(
                                "physical extraction replaced original resources".to_string()
                            );
                        }
                        let child = retained_child(
                            closed
                                .enclosing
                                .inner
                                .as_mut()
                                .ok_or("physical closed inner absent")?,
                        )?;
                        if child.bounded_drop_until != Some(held)
                            || child.enclosing_observed_reap.is_none()
                        {
                            return Err(
                                "physical native observation changed original drop ceiling"
                                    .to_string(),
                            );
                        }
                        drop(closed);
                        Ok(())
                    })();
                    if let Some(owner) = slot.as_mut()
                        && let Some(inner) = owner.inner.as_mut()
                    {
                        fixture_closeout(inner, checks)?;
                    } else {
                        checks?;
                    }
                }
                Ok(())
            }

            #[test]
            fn physical_unattempted_prelaunch_refusal_is_only_no_process_data() -> Result<(), String>
            {
                let held = Instant::now() + Duration::from_secs(2);
                let outer = held + Duration::from_secs(1);
                let (lease, _) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let rejected: Result<(), String> = owner
                    .with_inner(
                        |_inner| Err("prelaunch binding refused before capture".to_string()),
                    )
                    .map_err(str::to_string)?;
                if rejected.is_ok()
                    || !matches!(
                        owner.continue_physical_closeout(),
                        crate::process_owner::PhysicalStep::NoProcess
                    )
                    || owner
                        .inner()
                        .is_none_or(|inner| inner.attempted || inner.failure().is_some())
                    || !owner.matches_lease(&lease)
                {
                    return Err("prelaunch refusal fabricated attempted closure".to_string());
                }
                let mut slot = Some(owner);
                if CompleteEnclosingCustodian::take_physically_closed(&mut slot).is_ok()
                    || slot.is_none()
                {
                    return Err("no-process DATA became a physical wrapper".to_string());
                }
                Ok(())
            }

            #[test]
            fn physical_invalid_cap_records_actual_no_spawn_without_receipt() -> Result<(), String>
            {
                let held = Instant::now() + Duration::from_secs(2);
                let outer = held + Duration::from_secs(1);
                let (lease, _) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let result = owner
                    .with_inner(|inner| {
                        capture(
                            "exec /usr/bin/sleep 30",
                            None,
                            CompleteCaptureBudget::new(
                                Duration::from_secs(1),
                                0,
                                MAX_STREAM_BYTES + 1,
                                64,
                            ),
                            inner,
                        )
                    })
                    .map_err(str::to_string)?;
                if result.is_ok()
                    || !matches!(
                        owner.continue_physical_closeout(),
                        crate::process_owner::PhysicalStep::NoProcess
                    )
                    || owner.inner().is_none_or(|inner| {
                        !matches!(inner.outcome, TerminalAttempt::NoSpawn)
                            || inner.failure().is_none_or(|error| {
                                error.spawn_attempted
                                    || !error.matches_lease(&lease)
                                    || error.state().is_some_and(|state| state.receipt.is_some())
                            })
                    })
                {
                    return Err("invalid cap did not preserve actual pre-spawn refusal".to_string());
                }
                Ok(())
            }

            #[test]
            fn physical_native_spawn_error_records_actual_no_child_data() -> Result<(), String> {
                let held = Instant::now() + Duration::from_secs(2);
                let outer = held + Duration::from_secs(1);
                let (lease, _) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let result = owner
                    .with_inner(|inner| {
                        CompleteByteCapture::capture_with_terminal_custody(
                            (Path::new("/ripr-missing-physical-executable"), &[]),
                            (Path::new("/"), None),
                            &[],
                            (budget(0, 64), "no-child control"),
                            inner,
                        )
                        .map_err(report_to_string)
                    })
                    .map_err(str::to_string)?;
                if result.is_ok()
                    || !matches!(
                        owner.continue_physical_closeout(),
                        crate::process_owner::PhysicalStep::NoProcess
                    )
                    || owner.inner().is_none_or(|inner| {
                        !matches!(inner.outcome, TerminalAttempt::NoChild)
                            || inner.failure().is_none_or(|error| {
                                !error.spawn_attempted
                                    || error.custody.first().is_none_or(|custody| {
                                        custody.group.is_some()
                                            || custody.setup.as_ref().is_none_or(|setup| {
                                                !setup.failed_before_child_creation()
                                            })
                                    })
                            })
                    })
                {
                    return Err(
                        "native spawn error did not retain typed no-child provenance".to_string(),
                    );
                }
                Ok(())
            }

            #[test]
            fn physical_accepted_capture_is_no_process_data_with_real_receipt() -> Result<(), String>
            {
                let held = Instant::now() + Duration::from_secs(3);
                let outer = held + Duration::from_secs(1);
                let (lease, _) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                let checks = (|| {
                    let captured = owner
                        .with_inner(|inner| capture("printf actual", None, budget(0, 64), inner))
                        .map_err(str::to_string)??;
                    let (status, stdout, stderr, _, timed_out, receipt) = captured.into_parts();
                    if !status.success()
                        || stdout != b"actual"
                        || !stderr.is_empty()
                        || timed_out
                        || !receipt.matches_lease(&lease)
                        || !matches!(
                            owner.continue_physical_closeout(),
                            crate::process_owner::PhysicalStep::NoProcess
                        )
                        || owner.inner().is_none_or(|inner| {
                            !matches!(inner.outcome, TerminalAttempt::AcceptedClosed)
                                || inner.failure().is_some()
                        })
                    {
                        return Err(
                            "accepted result lost real receipt or became physical authority"
                                .to_string(),
                        );
                    }
                    Ok(())
                })();
                let inner = owner
                    .inner
                    .as_mut()
                    .ok_or("actual accepted-control owner absent")?;
                fixture_closeout(inner, checks)
            }

            #[test]
            fn physical_claim_unwind_stays_retained_without_no_process_classification()
            -> Result<(), String> {
                let held = Instant::now() + Duration::from_secs(2);
                let outer = held + Duration::from_secs(1);
                let (lease, _) = lease()?;
                let mut owner = enclosing(held, outer, lease.clone())?;
                HOOKS.with(|hooks| {
                    hooks.borrow_mut().unwind_capture_claim =
                        Some(Box::new("actual claimed interruption".to_string()))
                });
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _attempt = owner
                        .with_inner(|inner| capture("printf unused", None, budget(0, 64), inner));
                }));
                if result.is_ok()
                    || !owner.matches_lease(&lease)
                    || owner.inner().is_none_or(|inner| {
                        !inner.attempted || !matches!(inner.outcome, TerminalAttempt::Claimed)
                    })
                    || !matches!(
                        owner.continue_physical_closeout(),
                        crate::process_owner::PhysicalStep::Retained(_)
                    )
                {
                    return Err(
                        "claimed unwind was reclassified as untouched no-process DATA".to_string(),
                    );
                }
                Ok(())
            }

            fn controller_failure_message(
                failure: &crate::process_owner::CompleteControllerFailure,
            ) -> String {
                failure.message().to_string()
            }

            fn finish_controller<L: Any + Send + Sync>(
                transport: &mut crate::process_owner::CompleteControllerTransport<L>,
            ) -> Result<(), String> {
                let fixture_limit = Instant::now() + Duration::from_secs(5);
                loop {
                    match transport.step_to_terminal() {
                        crate::process_owner::PhysicalStep::Closed => return Ok(()),
                        crate::process_owner::PhysicalStep::Retained(error) => return Err(error),
                        crate::process_owner::PhysicalStep::NoProcess => {
                            return Err("controller used unrelated no-process DATA".to_string());
                        }
                        crate::process_owner::PhysicalStep::Pending => {}
                    }
                    if Instant::now() >= fixture_limit {
                        return Err(
                            "controller fixture did not reach actual terminal state".to_string()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }

            fn controller_fixture_checks<L: Any + Send + Sync>(
                transport: &mut crate::process_owner::CompleteControllerTransport<L>,
                checks: Result<(), String>,
            ) -> Result<(), String> {
                if checks.is_ok() {
                    return checks;
                }
                // Independent controlled-fixture teardown is not a terminal
                // transition. Retain the same outside object and first error.
                drop(transport.0.transport.take());
                let cleanup: Result<(), String> = (|| {
                    if let Some(owner) = transport.0.owner.as_mut() {
                        owner.close_controller_fixture_custody()?;
                    }
                    if let Some(setup) = transport.0.setup.as_mut() {
                        setup.close_controller_fixture_custody()?;
                    }
                    Ok(())
                })();
                match (checks, cleanup) {
                    (Err(primary), Err(cleanup)) => {
                        Err(format!("{primary}; fixture disposal: {cleanup}"))
                    }
                    (Err(primary), Ok(())) => Err(primary),
                    (Ok(()), _) => Ok(()),
                }
            }

            fn controller<L: Any + Send + Sync>(
                script: &str,
                input: Option<&[u8]>,
                output_limit: usize,
                execution_window: Duration,
                lease: Arc<L>,
            ) -> Result<crate::process_owner::CompleteControllerTransport<L>, String> {
                let started = Instant::now();
                let execution = started + execution_window;
                let held = execution + Duration::from_secs(7);
                let outer = held + Duration::from_secs(1);
                crate::process_owner::CompleteControllerTransport::admit(
                    (Path::new("/bin/sh"), &shell(script)),
                    (Path::new("/"), input),
                    &[],
                    CompleteCaptureBudget::new(
                        execution_window,
                        input.map_or(0, <[u8]>::len),
                        output_limit,
                        output_limit,
                    ),
                    (started, execution, held, outer),
                    lease,
                )
            }

            #[test]
            fn controller_timely_binary_transport_has_real_eof_receipt_and_same_lease()
            -> Result<(), String> {
                let lease = Arc::new(());
                let input = b"input\0\xff";
                let mut transport = controller(
                    "cat; printf 'stderr\\000\\376' >&2",
                    Some(input),
                    64,
                    Duration::from_secs(2),
                    lease.clone(),
                )?;
                let checks = (|| {
                    transport.launch()?;
                    finish_controller(&mut transport)?;
                    match transport.take_terminal()? {
                        crate::process_owner::CompleteControllerTerminal::Accepted(captured) => {
                            let (status, stdout, stderr, _, timed_out, receipt) =
                                captured.into_parts();
                            if !status.success()
                                || timed_out
                                || stdout != input
                                || stderr != b"stderr\0\xfe"
                                || !receipt.matches_lease(&lease)
                                || !transport.matches_lease(&lease)
                            {
                                return Err(
                                    "controller timely receipt or binary transport changed"
                                        .to_string(),
                                );
                            }
                        }
                        crate::process_owner::CompleteControllerTerminal::Failed(error) => {
                            return Err(format!(
                                "timely controller refused: {}",
                                controller_failure_message(&error)
                            ));
                        }
                    }
                    if transport.take_terminal().is_ok() {
                        return Err("controller terminal observation replayed".to_string());
                    }
                    Ok(())
                })();
                controller_fixture_checks(&mut transport, checks)
            }

            #[test]
            fn controller_partial_drain_reentry_preserves_same_real_receipt_until_eof()
            -> Result<(), String> {
                let lease = Arc::new(());
                HOOKS.with(|hooks| hooks.borrow_mut().hold_writer = Some("stdout"));
                let mut transport = match controller(
                    "printf drained",
                    None,
                    64,
                    Duration::from_secs(4),
                    lease.clone(),
                ) {
                    Ok(transport) => transport,
                    Err(error) => {
                        HOOKS.with(|hooks| {
                            let mut hooks = hooks.borrow_mut();
                            hooks.hold_writer = None;
                            drop(hooks.held_writer.take());
                        });
                        return Err(error);
                    }
                };
                let checks = (|| {
                    transport.launch()?;
                    let fixture_limit = Instant::now() + Duration::from_secs(2);
                    loop {
                        match transport.step_to_terminal() {
                            crate::process_owner::PhysicalStep::Pending => {}
                            crate::process_owner::PhysicalStep::Retained(error) => {
                                return Err(error);
                            }
                            _ => {
                                return Err(
                                    "actual held writer did not keep drain pending".to_string()
                                );
                            }
                        }
                        if transport.0.group_settlement.is_some() {
                            break;
                        }
                        if Instant::now() >= fixture_limit {
                            return Err(
                                "actual group did not settle before the live drain control"
                                    .to_string(),
                            );
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    let initial = transport
                        .0
                        .group_settlement
                        .as_ref()
                        .ok_or("actual first group settlement absent")?
                        .observed_worker();
                    if !matches!(
                        transport.step_to_terminal(),
                        crate::process_owner::PhysicalStep::Pending
                    ) || !transport.matches_lease(&lease)
                        || transport.0.group_settlement.as_ref().is_none_or(|receipt| {
                            let observed = receipt.observed_worker();
                            observed.pid() != initial.pid()
                                || observed.start() != initial.start()
                                || observed.group() != initial.group()
                                || observed.parent() != initial.parent()
                        })
                    {
                        return Err(
                            "partial drain reentry discarded or replaced real settlement"
                                .to_string(),
                        );
                    }
                    HOOKS.with(|hooks| {
                        drop(hooks.borrow_mut().held_writer.take());
                    });
                    finish_controller(&mut transport)?;
                    match transport.take_terminal()? {
                        crate::process_owner::CompleteControllerTerminal::Accepted(captured) => {
                            let (status, stdout, stderr, _, timed_out, receipt) =
                                captured.into_parts();
                            let observed = receipt.observed_worker();
                            if !status.success()
                                || stdout != b"drained"
                                || !stderr.is_empty()
                                || timed_out
                                || observed.pid() != initial.pid()
                                || observed.start() != initial.start()
                                || observed.group() != initial.group()
                                || observed.parent() != initial.parent()
                                || !receipt.matches_lease(&lease)
                            {
                                return Err(
                                    "actual EOF did not preserve original captured settlement"
                                        .to_string(),
                                );
                            }
                            Ok(())
                        }
                        crate::process_owner::CompleteControllerTerminal::Failed(error) => {
                            Err(controller_failure_message(&error))
                        }
                    }
                })();
                HOOKS.with(|hooks| {
                    let mut hooks = hooks.borrow_mut();
                    hooks.hold_writer = None;
                    drop(hooks.held_writer.take());
                });
                controller_fixture_checks(&mut transport, checks)
            }

            #[test]
            fn controller_overflow_drains_without_more_retained_growth() -> Result<(), String> {
                let lease = Arc::new(());
                let mut transport = controller(
                    "head -c 1048576 /dev/zero; printf done >&2",
                    None,
                    32,
                    Duration::from_secs(2),
                    lease.clone(),
                )?;
                let checks = (|| {
                    transport.launch()?;
                    finish_controller(&mut transport)?;
                    match transport.take_terminal()? {
                        crate::process_owner::CompleteControllerTerminal::Failed(error) => {
                            if !error.message().contains("budget")
                                || error.stdout().len() != 33
                                || error.stderr() != b"done"
                                || !error
                                    .observed_status()
                                    .is_some_and(|status| status.success())
                                || !error.matches_lease(&lease)
                            {
                                return Err("controller overflow lost bounded drain observations"
                                    .to_string());
                            }
                        }
                        crate::process_owner::CompleteControllerTerminal::Accepted(_) => {
                            return Err("controller overflow became accepted output".to_string());
                        }
                    }
                    Ok(())
                })();
                controller_fixture_checks(&mut transport, checks)
            }

            #[test]
            fn controller_expired_execution_waits_for_real_natural_exit_without_kill()
            -> Result<(), String> {
                let lease = Arc::new(());
                let mut transport = controller(
                    "exec /usr/bin/sleep 0.75",
                    None,
                    64,
                    Duration::from_millis(75),
                    lease.clone(),
                )?;
                let checks = (|| {
                    transport.launch()?;
                    let before = transport
                        .0
                        .owner
                        .as_ref()
                        .ok_or("actual controller owner missing")?
                        .initial_enclosing_worker_for_test()?;
                    std::thread::sleep(Duration::from_millis(100));
                    if !matches!(
                        transport.step_to_terminal(),
                        crate::process_owner::PhysicalStep::Pending
                    ) || transport
                        .0
                        .owner
                        .as_ref()
                        .and_then(QualifiedGroupOwner::physical_status)
                        .is_some()
                    {
                        return Err("expired execution reaped or terminated the live controller"
                            .to_string());
                    }
                    let after =
                        super::super::super::super::ObservedProcessIdentity::read(before.pid())?;
                    if after.start() != before.start()
                        || after.group() != before.group()
                        || after.parent() != before.parent()
                    {
                        return Err(
                            "late controller observation changed native identity".to_string()
                        );
                    }
                    finish_controller(&mut transport)?;
                    match transport.take_terminal()? {
                        crate::process_owner::CompleteControllerTerminal::Failed(error)
                            if error
                                .observed_status()
                                .is_some_and(|status| status.success())
                                && error.matches_lease(&lease) =>
                        {
                            Ok(())
                        }
                        _ => Err(
                            "late natural controller exit gained acceptance or lost status"
                                .to_string(),
                        ),
                    }
                })();
                controller_fixture_checks(&mut transport, checks)
            }

            #[test]
            fn controller_caught_unwind_keeps_external_live_owner_serviceable() -> Result<(), String>
            {
                let lease = Arc::new(());
                let mut transport = controller(
                    "printf prior; exec /usr/bin/sleep 0.5",
                    None,
                    64,
                    Duration::from_secs(2),
                    lease.clone(),
                )?;
                let checks = (|| {
                    transport.launch()?;
                    let before = transport
                        .0
                        .owner
                        .as_ref()
                        .ok_or("actual external controller owner missing")?
                        .initial_enclosing_worker_for_test()?;
                    let interrupted =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let _step = transport.step_to_terminal();
                            std::panic::resume_unwind(Box::new(
                                "bootstrap operation interrupted".to_string(),
                            ));
                        }));
                    if interrupted.is_ok()
                        || !transport.matches_lease(&lease)
                        || transport
                            .0
                            .owner
                            .as_mut()
                            .is_none_or(|owner| owner.fixture_child().id() != before.pid())
                    {
                        return Err(
                            "bootstrap unwind lost the actual external controller".to_string()
                        );
                    }
                    finish_controller(&mut transport)?;
                    match transport.take_terminal()? {
                        crate::process_owner::CompleteControllerTerminal::Accepted(captured) => {
                            let (status, stdout, _, _, _, receipt) = captured.into_parts();
                            if !status.success()
                                || stdout != b"prior"
                                || !receipt.matches_lease(&lease)
                            {
                                return Err(
                                    "external unwind recovery changed actual capture".to_string()
                                );
                            }
                            Ok(())
                        }
                        crate::process_owner::CompleteControllerTerminal::Failed(error) => {
                            Err(error.message().to_string())
                        }
                    }
                })();
                controller_fixture_checks(&mut transport, checks)
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
                    return Err(
                        "negative disposition changed actual identity, lease or clock".to_string(),
                    );
                }
                Ok(())
            }

            #[test]
            fn missing_eof_retains_real_group_only_negative_and_refuses_replay()
            -> Result<(), String> {
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
                        Err(message) if message.contains("stdout actual EOF is unconfirmed") => {
                            message
                        }
                        Err(message) => return Err(message),
                        Ok(_) => {
                            return Err("missing actual EOF became captured success".to_string());
                        }
                    };
                    if started.elapsed() < POST_KILL_DRAIN_GRACE
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err(
                            "missing EOF fabricated a combined receipt or reset drain".to_string()
                        );
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
                        || custodian.failure().map(CompleteCaptureError::message)
                            != Some(message.as_str())
                        || custodian.take_cleanup_receipt().is_some()
                    {
                        return Err(
                            "negative closeout changed group proof or first refusal".to_string()
                        );
                    }
                    match custodian.try_closeout_failure() {
                        Err(report) if report.message() == message => Ok(()),
                        Err(report) => {
                            Err(format!("negative replay changed the first error: {report}"))
                        }
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
                    Err(message) if message.contains("stdout exceeds its 3-byte output budget") => {
                        message
                    }
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
                        || custodian.failure().map(CompleteCaptureError::message)
                            != Some(message.as_str())
                    {
                        return Err(
                            "combined negative changed actual observation or first error"
                                .to_string(),
                        );
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
                    let mut recovery =
                        CompleteTerminalCustodian::new(held, recovery_lease.clone())?;
                    let output = capture("printf recovered", None, budget(0, 64), &mut recovery)?;
                    let (status, stdout, stderr, _, timed_out, receipt) = output.into_parts();
                    if !status.success() || timed_out || !receipt.matches_lease(&recovery_lease) {
                        return Err(
                            "independent recovery lost genuine capture semantics".to_string()
                        );
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
                    Err(message) if message.contains("stdout exceeds its 3-byte output budget") => {
                        message
                    }
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
                            Ok(_) => {
                                Err("late extraction minted a negative disposition".to_string())
                            }
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
                        || custodian.failure().map(CompleteCaptureError::message)
                            != Some(message.as_str())
                    {
                        return Err(
                            "late extraction restored a different receipt, clock or lease"
                                .to_string(),
                        );
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
                    return Err(
                        "elapsed execution control did not retain a live cutoff".to_string()
                    );
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
                        Ok(_) => {
                            return Err(
                                "absolute execution timeout became capture success".to_string()
                            );
                        }
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
                    if !timed_out
                        || Instant::now() < execution_deadline
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
                        1 => (
                            started + Duration::from_secs(1),
                            started + Duration::from_secs(2),
                        ),
                        2 => (started, started),
                        _ => (
                            started - Duration::from_secs(1),
                            started - Duration::from_millis(1),
                        ),
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
                        return Err(
                            "invalid window allocated endpoints, spawned or changed custody"
                                .to_string(),
                        );
                    }
                    match custodian.try_closeout_failure() {
                        Err(report) if report.capture_error().is_some() => {}
                        Err(report) => return Err(report.to_string()),
                        Ok(_) => {
                            return Err(
                                "count-free preflight minted negative disposition".to_string()
                            );
                        }
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
                    capture(
                        "exec /usr/bin/sleep 30",
                        None,
                        budget(0, 64),
                        &mut custodian,
                    )
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
                        Ok(_) => {
                            return Err(
                                "unqualified direct custody became checked negative".to_string()
                            );
                        }
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
                        return Err(
                            "negative refusal lost real unqualified child or original ceiling"
                                .to_string(),
                        );
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
