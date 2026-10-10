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
    pub fn into_parts(self) -> (ExitStatus, Vec<u8>, Vec<u8>, Duration, bool, CompleteCaptureReceipt) {
        (self.status, self.stdout, self.stderr, self.duration, self.timed_out, self.receipt)
    }
}

/// Capture failure retaining custody even when combined cleanup is unconfirmed.
pub struct CompleteCaptureError {
    message: String,
    receipt: Option<CompleteCaptureReceipt>,
    observation: Option<(ExitStatus, Vec<u8>, Vec<u8>, Duration, bool)>,
    timeout_only: bool,
    lease: Arc<dyn Any + Send + Sync>,
}
impl CompleteCaptureError {
    /// Original capture failure and any cleanup diagnostics.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Read actual failed wait data, without admitting captured output.
    pub fn observed_outcome(&self) -> Option<(ExitStatus, Duration, bool)> {
        self.observation.as_ref().map(|parts| (parts.0, parts.3, parts.4))
    }
    /// Consume failed observation bytes at most once; this grants no cleanup or success authority.
    pub fn take_failed_observation(
        &mut self,
    ) -> Option<(ExitStatus, Vec<u8>, Vec<u8>, Duration, bool)> {
        self.observation.take()
    }
    /// Whether an actual timeout is the sole failure, for explicit legacy result conversion.
    pub fn is_timeout_only(&self) -> bool {
        self.timeout_only
    }
    /// Transfer confirmed combined cleanup at most once; this never admits output.
    pub fn take_cleanup_receipt(&mut self) -> Option<CompleteCaptureReceipt> {
        self.receipt.take()
    }
    /// Compare the actual retained custody identity.
    pub fn matches_lease<L: Any + Send + Sync>(&self, lease: &Arc<L>) -> bool {
        let erased: Arc<dyn Any + Send + Sync> = lease.clone();
        Arc::ptr_eq(&self.lease, &erased)
    }
}
impl std::fmt::Debug for CompleteCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CompleteCaptureError")
            .field("message", &self.message)
            .field("combined_cleanup_confirmed", &self.receipt.is_some())
            .finish()
    }
}
impl std::fmt::Display for CompleteCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for CompleteCaptureError {}

/// Source-single complete capture adapter; ordinary capture helpers are unchanged.
pub struct CompleteByteCapture;
impl CompleteByteCapture {
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
                receipt: None,
                observation: None,
                timeout_only: false,
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
                command, source, env_remove, execution, Some(held_deadline), lease, settled,
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
                receipt: None, observation: None, timeout_only: false, lease,
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
        let (parent, child) = UnixStream::pair()
            .map_err(|error| format!("prepare {name} socket: {error}"))?;
        // The endpoints have independent file descriptions. The worker endpoint
        // retains blocking stdio; only the controller endpoint is nonblocking.
        parent.set_nonblocking(true)
            .map_err(|error| format!("prepare nonblocking {name}: {error}"))?;
        #[cfg(test)]
        HOOKS.with(|hooks| {
            let mut hooks = hooks.borrow_mut();
            if hooks.hold_writer == Some(name) {
                hooks.hold_writer = None;
                hooks.held_writer = Some(child.try_clone()
                    .map_err(|error| format!("retain native {name} writer control: {error}"))?);
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
                Err(error) if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {
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
            let end = self.offset.saturating_add(CHUNK_BYTES).min(self.bytes.len());
            match stream.write(&self.bytes[self.offset..end]) {
                Ok(0) => {
                    self.error = Some(std::io::Error::new(
                        ErrorKind::WriteZero, "bounded stdin write made no progress"
                    ));
                    drop(self.stream.take());
                    false
                }
                Ok(count) => {
                    self.offset += count;
                    true
                }
                Err(error) if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {
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
                && self.input.as_ref().is_none_or(|input| input.stream.is_none())
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
                            &mut self.stdout, "stdout", self.stdout_limit, &mut scratch,
                        ),
                        1 => self.stderr_capture.step(
                            &mut self.stderr, "stderr", self.stderr_limit, &mut scratch,
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

    fn failure(
        message: String,
        receipt: Option<CompleteCaptureReceipt>,
        lease: Arc<dyn Any + Send + Sync>,
    ) -> CompleteCaptureError {
        CompleteCaptureError {
            message, receipt, observation: None, timeout_only: false, lease,
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

    pub(super) fn capture_with_held(
        command: (&Path, &[String]),
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        execution: (CompleteCaptureBudget, &str),
        held_deadline: Option<Instant>,
        lease: Arc<dyn Any + Send + Sync>,
        mut settled: impl FnMut(ExitStatus),
    ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
        // None preserves ordinary validation-before-clock ordering.
        let strict_started = held_deadline.map(|_| Instant::now());
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        let (program, args) = command;
        let (budget, error_context) = execution;
        let (cwd, input) = source;
        // Validate the complete execution admission before copying stdin or
        // preparing endpoints. These ceilings do not alter ordinary capture.
        if budget.timeout.is_zero() {
            return Err(failure(
                format!("complete byte capture for {error_context} has no deadline"),
                None, lease,
            ));
        }
        for (name, limit) in [
            ("stdin", budget.stdin_bytes),
            ("stdout", budget.stdout_bytes),
            ("stderr", budget.stderr_bytes),
        ] {
            if limit > MAX_STREAM_BYTES {
                return Err(failure(
                    format!("{name} byte budget exceeds the {MAX_STREAM_BYTES}-byte stream ceiling"),
                    None, lease,
                ));
            }
        }
        for (name, limit) in [("stdout", budget.stdout_bytes), ("stderr", budget.stderr_bytes)] {
            if limit.checked_add(1).is_none() {
                return Err(failure(
                    format!("{name} byte budget cannot admit its overflow sentinel"),
                    None, lease,
                ));
            }
        }
        let started = strict_started.unwrap_or_else(Instant::now);
        let worker_deadline = started.checked_add(budget.timeout).ok_or_else(|| failure(
            format!("complete byte capture for {error_context} deadline overflow"),
            None, lease.clone(),
        ))?;
        let worker_deadline = held_deadline.map_or(worker_deadline, |held| worker_deadline.min(held));
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        let input = input.map(|bytes| {
            if bytes.len() > budget.stdin_bytes {
                return Err(format!(
                    "stdin exceeds its {}-byte input budget", budget.stdin_bytes
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
        }).transpose().map_err(|error| failure(error, None, lease.clone()))?;
        #[cfg(test)]
        if held_deadline.is_some()
            && let Some(delay) = HOOKS.with(|hooks| hooks.borrow_mut().delay_admission_once.take())
        {
            thread::sleep(delay);
        }
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        if Instant::now() >= worker_deadline {
            return Err(failure(
                format!("complete byte capture for {error_context} admission exceeded its deadline"),
                None, lease,
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
                    (Some(InputCapture {
                        stream: Some(stream), bytes, offset: 0, complete: false, error: None,
                    }), child)
                }
                None => (None, Stdio::null()),
            };
            Ok::<_, String>((stdout, stderr, input, stdout_child, stderr_child, stdin_child))
        })().map_err(|error| failure(error, None, lease.clone()))?;
        let (stdout, stderr, input, stdout_child, stderr_child, stdin_child) = prepared;
        let mut transport = Transport {
            stdout, stderr, input,
            stdout_capture: OutputCapture::new(),
            stderr_capture: OutputCapture::new(),
            stdout_limit: budget.stdout_bytes,
            stderr_limit: budget.stderr_bytes,
            next_endpoint: 0,
            late_action: false,
        };
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd);
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        for name in env_remove {
            strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
            command.env_remove(name);
            strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        }
        command.stdout(stdout_child).stderr(stderr_child).stdin(stdin_child);
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
        // Consuming spawn drops the Command and every parent copy of its child
        // endpoints before it returns. No parent I/O helper is ever started.
        if Instant::now() >= worker_deadline {
            return Err(failure(
                format!("complete byte capture for {error_context} setup exceeded its deadline; spawn refused"),
                None, lease,
            ));
        }
        let mut owner = match held_deadline {
            Some(held) => QualifiedGroupOwner::spawn_with_deadline(command, held),
            None => QualifiedGroupOwner::spawn(command),
        }.map_err(|error| failure(format!("failed to run {error_context}: {error}"), None, lease.clone()))?;
        strict_time(held_deadline).map_err(|error| failure(error, None, lease.clone()))?;
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
                return Err(failure(
                    "late primary control did not establish actual output EOF".to_string(),
                    None, lease,
                ));
            }
            thread::sleep(delay);
        }
        let outcome = owner.wait_supervised(started, budget.timeout, || {
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
                if parts.1 >= budget.timeout && !parts.2 {
                    errors.push("primary observation exceeded its held execution deadline".to_string());
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
            errors.push("stdout actual EOF is unconfirmed within shared post-kill grace".to_string());
        }
        if let Some(error) = transport.stderr_capture.error.take() {
            errors.push(error);
        }
        if !stderr_eof {
            errors.push("stderr actual EOF is unconfirmed within shared post-kill grace".to_string());
        }
        if let Some(input) = transport.input.as_ref() {
            if let Some(error) = &input.error {
                if !(observation.as_ref().is_some_and(|parts| parts.2)
                    && error.kind() == ErrorKind::BrokenPipe)
                {
                    errors.push(format!("write stdin for {error_context}: {error}"));
                }
            } else if !input.complete {
                errors.push(format!("stdin completion for {error_context} is unconfirmed"));
            }
        }
        if transport.late_action {
            errors.push("complete endpoint action exceeded its held clock boundary".to_string());
        }
        let before_release = Instant::now() < deadline;
        let late_action = transport.late_action;
        let Transport { stdout, stderr, input, stdout_capture, stderr_capture, .. } = transport;
        // Local close is ownership release, never an EOF observation. An input
        // failure may authorize cleanup only after actual output EOF/group settle;
        // it can never turn failed capture into successful output.
        drop(stdout);
        drop(stderr);
        drop(input);
        let released_in_time = before_release && Instant::now() < deadline && !late_action;
        let group = owner.take_settlement();
        let mut receipt = if stdout_eof && stderr_eof && released_in_time {
            group.filter(|group| group.belongs_to_current_parent())
                .map(|group| CompleteCaptureReceipt { group, lease: lease.clone() })
        } else {
            None
        };
        if let Err(clock) = strict_time(held_deadline) {
            receipt = None;
            errors.push(clock);
        }
        let timed_out = observation.as_ref().is_some_and(|parts| parts.2);
        let mut timeout_only = timed_out && errors.is_empty() && receipt.is_some();
        if timed_out {
            errors.push(format!("complete worker timed out for {error_context}"));
        }
        let observation = observation.map(|(status, _, timed_out)| (
            status, stdout_capture.bytes, stderr_capture.bytes, started.elapsed(), timed_out,
        ));
        if let Err(clock) = strict_time(held_deadline) {
            receipt = None;
            timeout_only = false;
            errors.push(clock);
        }
        if !errors.is_empty() {
            return Err(CompleteCaptureError {
                message: format!("{error_context}: {}", errors.join("; ")),
                receipt, observation, timeout_only, lease,
            });
        }
        let receipt = match receipt {
            Some(receipt) => receipt,
            None => return Err(CompleteCaptureError {
                message: format!("{error_context}: combined group, actual EOF and controller cleanup is unconfirmed"),
                receipt: None, observation, timeout_only: false, lease,
            }),
        };
        let (status, stdout, stderr, duration, timed_out) = observation.ok_or_else(|| failure(
            format!("{error_context}: primary observation is unavailable"), None, lease.clone(),
        ))?;
        Ok(CompleteCapturedBytes { status, stdout, stderr, duration, timed_out, receipt })
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
                    std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
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
                (Path::new("/bin/sh"), args), (Path::new("/"), input), &[],
                (budget, "native complete capture control"), lease, settled,
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

        fn run_held(
            args: &[String],
            input: Option<&[u8]>,
            budget: CompleteCaptureBudget,
            held_deadline: Instant,
            lease: Arc<()>,
            settled: impl FnMut(ExitStatus),
        ) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
            CompleteByteCapture::capture_with_deadline(
                (Path::new("/bin/sh"), args), (Path::new("/"), input), &[],
                (budget, "native complete capture control"), held_deadline, lease, settled,
            )
        }

        #[test]
        fn original_expired_clock_precedes_missing_executable_and_transport_setup() -> Result<(), String> {
            let lease = Arc::new(());
            let foreign = Arc::new(());
            let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
            let attempts = super::super::super::linux::spawn_attempts();
            let mut error = refused(
                CompleteByteCapture::capture_with_deadline(
                    (Path::new("/ripr-deliberately-missing-held-worker"), &[]),
                    (Path::new("/"), None), &[],
                    (budget(0, 64), "expired held control"),
                    Instant::now(), lease.clone(), |_| {},
                ),
                "held custody deadline",
            )?;
            assert_eq!(HOOKS.with(|hooks| hooks.borrow().pair_attempts), pairs);
            assert_eq!(super::super::super::linux::spawn_attempts(), attempts);
            if !error.matches_lease(&lease) || error.matches_lease(&foreign)
                || error.take_cleanup_receipt().is_some()
                || error.take_failed_observation().is_some()
                || error.is_timeout_only()
            {
                return Err("expired preflight manufactured observation or lost custody".to_string());
            }
            drop(error);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn original_clock_expiring_during_real_input_admission_does_not_restart() -> Result<(), String> {
            let lease = Arc::new(());
            let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
            let attempts = super::super::super::linux::spawn_attempts();
            HOOKS.with(|hooks| {
                hooks.borrow_mut().delay_admission_once = Some(Duration::from_millis(40));
            });
            let input = [19_u8; 64];
            let output = run_held(
                &shell("cat"), Some(&input), budget(input.len(), 64),
                Instant::now() + Duration::from_millis(20), lease.clone(), |_| {},
            );
            HOOKS.with(|hooks| hooks.borrow_mut().delay_admission_once = None);
            let mut error = refused(output, "held custody deadline")?;
            assert_eq!(HOOKS.with(|hooks| hooks.borrow().pair_attempts), pairs);
            assert_eq!(super::super::super::linux::spawn_attempts(), attempts);
            if error.take_cleanup_receipt().is_some() || error.observed_outcome().is_some()
                || !error.matches_lease(&lease)
            {
                return Err("late admission acquired worker authority".to_string());
            }
            Ok(())
        }

        #[test]
        fn strict_timely_capture_keeps_none_empty_binary_and_ceiling_parity() -> Result<(), String> {
            for input in [None, Some(&[][..]), Some(&b"\0owned\xff"[..])] {
                for output_limit in [64, MAX_STREAM_BYTES] {
                    let args = shell("cat; printf '\\377\\200' >&2; exit 7");
                    let input_limit = input.map_or(0, <[u8]>::len);
                    let ordinary_lease = Arc::new(());
                    let ordinary = run(
                        &args, input, budget(input_limit, output_limit), ordinary_lease.clone(), |_| {},
                    ).map_err(|error| error.to_string())?.into_parts();
                    let strict_lease = Arc::new(());
                    let strict = run_held(
                        &args, input, budget(input_limit, output_limit),
                        Instant::now() + Duration::from_secs(2), strict_lease.clone(), |_| {},
                    ).map_err(|error| error.to_string())?.into_parts();
                    assert_eq!((strict.0, &strict.1, &strict.2, strict.4),
                        (ordinary.0, &ordinary.1, &ordinary.2, ordinary.4));
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
        fn strict_overflow_and_timeout_keep_failed_data_and_one_shot_cleanup_parity() -> Result<(), String> {
            for timeout in [false, true] {
                let args = if timeout { shell("sleep 30") } else { shell("printf abc") };
                let make_budget = || {
                    CompleteCaptureBudget::new(
                        if timeout { Duration::from_millis(50) } else { Duration::from_secs(2) },
                        0, if timeout { 64 } else { 1 }, 64,
                    )
                };
                let expected = if timeout { "timed out" } else { "stdout exceeds" };
                let mut ordinary = refused(run(&args, None, make_budget(), Arc::new(()), |_| {}), expected)?;
                let lease = Arc::new(());
                let mut strict = refused(run_held(
                    &args, None, make_budget(), Instant::now() + Duration::from_secs(2),
                    lease.clone(), |_| {},
                ), expected)?;
                assert_eq!(strict.is_timeout_only(), ordinary.is_timeout_only());
                let ordinary_data = ordinary.take_failed_observation()
                    .ok_or_else(|| "ordinary failure lost actual observation".to_string())?;
                let strict_data = strict.take_failed_observation()
                    .ok_or_else(|| "strict failure lost actual observation".to_string())?;
                assert_eq!((strict_data.0, &strict_data.1, &strict_data.2, strict_data.4),
                    (ordinary_data.0, &ordinary_data.1, &ordinary_data.2, ordinary_data.4));
                if strict.take_failed_observation().is_some() {
                    return Err("strict failed DATA was reused".to_string());
                }
                let receipt = strict.take_cleanup_receipt()
                    .ok_or_else(|| "timely failed capture lost real cleanup settlement".to_string())?;
                if !receipt.matches_lease(&lease) || strict.take_cleanup_receipt().is_some()
                    || ordinary.take_cleanup_receipt().is_none()
                {
                    return Err("failed parity capture fabricated or reused cleanup custody".to_string());
                }
            }
            Ok(())
        }

        #[test]
        fn actual_settled_callback_crossing_original_clock_cannot_mint_cleanup() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let lease = Arc::new(());
            let mut observed = None;
            let output = run_held(
                &shell("printf owned"), None, budget(0, 64), held, lease.clone(),
                |status| {
                    observed = Some(status);
                    thread::sleep(held.saturating_duration_since(Instant::now()) + Duration::from_millis(10));
                },
            );
            let mut error = refused(output, "held custody deadline")?;
            let observed = observed.ok_or_else(|| "real timely group never reached settled callback".to_string())?;
            if !observed.success() || !error.matches_lease(&lease)
                || error.take_cleanup_receipt().is_some() || error.is_timeout_only()
            {
                return Err("late callback manufactured cleanup or lost actual status".to_string());
            }
            let data = error.take_failed_observation()
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
            let output = run(&args, Some(input), budget(input.len(), 64), lease.clone(), |_| {})
                .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, receipt) = output.into_parts();
            if !status.success() || timeout {
                return Err("native positive child failed or timed out".to_string());
            }
            assert_eq!(stdout, input);
            assert_eq!(stderr, vec![255, 128]);
            if !receipt.matches_lease(&lease) || receipt.matches_lease(&foreign) {
                return Err("combined receipt accepted a different actual custody lease".to_string());
            }
            assert_eq!(receipt.settled_status(), Some(status));
            let worker = receipt.observed_worker();
            let parent = receipt.observed_parent();
            let actual_parent = super::super::super::ObservedProcessIdentity::read(std::process::id())?;
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
                let output = run(&shell("cat; printf eof"), input, budget(0, 64), Arc::new(()), |_| {})
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
            reader.set_nonblocking(true).map_err(|error| error.to_string())?;
            writer.write_all(b"sent-before-block").map_err(|error| error.to_string())?;
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
                run(&shell("printf abc"), None, budget(0, 1), lease.clone(), |status| observed = Some(status)),
                "stdout exceeds its 1-byte output budget",
            )?;
            if !observed.is_some_and(|status| status.success()) {
                return Err("overflow control lacks actual exit0 primary".to_string());
            }
            let receipt = failure.take_cleanup_receipt().ok_or("overflow never reached actual EOF/group cleanup")?;
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
            if capture.eof || !capture.failed || !capture.error.as_deref().is_some_and(|error| {
                error.contains("intentional opened-stream failure")
            }) {
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
                    run(&shell("cat"), Some(b"owned"), budget(5, 64), lease.clone(), |_| observed = true),
                    &format!("prepare {name} socket: injected pair failure"),
                )?;
                if observed || failure.take_cleanup_receipt().is_some()
                    || super::super::super::linux::spawn_attempts() != attempts
                    || !failure.matches_lease(&lease)
                {
                    return Err("partial pair preparation spawned a worker or dropped custody".to_string());
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
                run(&shell("printf sent"), None, budget(0, 64), lease.clone(), |observed| status = Some(observed)),
                "stdout actual EOF is unconfirmed",
            )?;
            // Release the owned discriminator before checking assertions.
            HOOKS.with(|hooks| drop(hooks.borrow_mut().held_writer.take()));
            if !status.is_some_and(|status| status.success())
                || failure.take_cleanup_receipt().is_some() || !failure.matches_lease(&lease)
                || started.elapsed() < POST_KILL_DRAIN_GRACE
            {
                return Err("retained writer manufactured EOF, cleanup or status".to_string());
            }
            drop(failure);
            let output = run(&shell("printf recovered"), None, budget(0, 64), lease, |_| {})
                .map_err(|error| error.to_string())?;
            assert_eq!(output.into_parts().1, b"recovered");
            Ok(())
        }

        #[test]
        fn inherited_descendant_writer_retains_group_failure_after_cleanup() -> Result<(), String> {
            let lease = Arc::new(());
            let mut status = None;
            let mut failure = refused(
                run(&shell("printf parent; /usr/bin/sleep 30 &"), None, budget(0, 64), lease.clone(), |observed| status = Some(observed)),
                "primary exited with live group members",
            )?;
            if !status.is_some_and(|status| status.success()) {
                return Err("descendant writer premise lacks actual exit0 primary".to_string());
            }
            let receipt = failure.take_cleanup_receipt().ok_or("owned descendant did not settle and drain")?;
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
                &shell("printf prefix; printf stderr >&2; cat"), Some(input),
                budget(input.len(), 64), Arc::new(()), |_| {},
            ).map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timeout, _) = output.into_parts();
            if !status.success() || timeout
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
                    &shell("printf held; printf deadline >&2"), None,
                    CompleteCaptureBudget::new(Duration::from_millis(250), 0, 64, 64),
                    lease.clone(), |_| {},
                ),
                "complete endpoint action exceeded its held clock boundary",
            )?;
            if HOOKS.with(|hooks| hooks.borrow().delayed_endpoints) != before + 1
                || !failure.matches_lease(&lease) || failure.is_timeout_only()
                || failure.take_cleanup_receipt().is_some()
            {
                return Err("actual endpoint clock overrun yielded success or cleanup authority".to_string());
            }
            let (_, stdout, stderr, _, _) = failure.take_failed_observation()
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
                    lease.clone(), |status| actual_status = Some(status),
                ),
                "primary observation exceeded its held execution deadline",
            )?;
            let (status, duration, timed_out) = failure.observed_outcome()
                .ok_or("late primary control lost its actual wait data")?;
            if !status.success() || actual_status != Some(status)
                || duration < Duration::from_millis(250) || timed_out
                || failure.is_timeout_only()
            {
                return Err("late exit0 was not preserved as failed non-timeout data".to_string());
            }
            let receipt = failure.take_cleanup_receipt()
                .ok_or("late observed exit0 did not settle and release actual EOF endpoints")?;
            if !receipt.matches_lease(&lease) {
                return Err("late primary cleanup has another custody lease".to_string());
            }
            let (_, stdout, stderr, _, _) = failure.take_failed_observation()
                .ok_or("late primary failure lost its empty actual output observations")?;
            assert_eq!(stdout, Vec::<u8>::new());
            assert_eq!(stderr, Vec::<u8>::new());
            Ok(())
        }

        #[test]
        fn blocked_stdin_is_bounded_and_early_close_is_not_success() -> Result<(), String> {
            let input = vec![0xffu8; 2 * 1024 * 1024];
            let mut failure = refused(
                run(&shell("exec 0<&-; printf closed"), Some(&input), budget(input.len(), 64), Arc::new(()), |_| {}),
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
                    lease.clone(), |_| {},
                ),
                "complete worker timed out",
            )?;
            let (observed_status, duration, timed_out) = failure.observed_outcome()
                .ok_or("blocked input lost its actual failed wait observation")?;
            if !timed_out || duration > Duration::from_secs(8) || !failure.is_timeout_only() {
                return Err("blocked input evaded the admitted failure/execution/drain bounds".to_string());
            }
            let receipt = failure.take_cleanup_receipt()
                .ok_or("timeout did not settle the group and actual endpoints")?;
            if !receipt.matches_lease(&lease) {
                return Err("timeout cleanup has another custody lease".to_string());
            }
            assert_eq!(receipt.settled_status(), Some(observed_status));
            let (status, stdout, stderr, failed_duration, failed_timeout) = failure.take_failed_observation()
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
        fn large_binary_backpressure_preserves_both_outputs_and_input_offset() -> Result<(), String> {
            let input: Vec<u8> = (0u8..251).cycle().take(2 * 1024 * 1024).collect();
            let script = "head -c 1048576 /dev/zero; head -c 1048576 /dev/zero >&2; cat";
            let output = run(
                &shell(script), Some(&input),
                CompleteCaptureBudget::new(Duration::from_secs(120), input.len(), 3 * 1024 * 1024, 1024 * 1024),
                Arc::new(()), |_| {},
            ).map_err(|error| error.to_string())?;
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
        fn continuously_readable_excess_remains_failed_and_deadline_bounded() -> Result<(), String> {
            let mut failure = refused(
                run(&shell("exec cat /dev/zero"), None,
                    CompleteCaptureBudget::new(Duration::from_millis(250), 0, 3, 64),
                    Arc::new(()), |_| {}),
                "stdout exceeds its 3-byte output budget",
            )?;
            if failure.is_timeout_only() {
                return Err("overflow plus timeout was classified as a pure legacy timeout".to_string());
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
        fn actual_cargo_json_and_native_eight_mib_stream_keep_existing_admission() -> Result<(), String> {
            let fixture = Fixture::new()?;
            std::fs::write(fixture.0.join("Cargo.toml"),
                "[package]\nname=\"complete_capture_json_control\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n")
                .map_err(|error| error.to_string())?;
            std::fs::create_dir(fixture.0.join("src")).map_err(|error| error.to_string())?;
            std::fs::write(fixture.0.join("src/main.rs"), "fn main() {}\n")
                .map_err(|error| error.to_string())?;
            let args = vec!["build".to_string(), "--offline".to_string(), "--manifest-path".to_string(),
                fixture.0.join("Cargo.toml").display().to_string(), "--message-format=json".to_string()];
            let output = CompleteByteCapture::capture(
                (Path::new("cargo"), &args), (&fixture.0, None), &[],
                (CompleteCaptureBudget::new(Duration::from_secs(120), 0, 8 * 1024 * 1024, 64 * 1024), "actual Cargo JSON control"),
                Arc::new(()), |_| {},
            ).map_err(|error| error.to_string())?;
            let (status, stdout, _, _, timeout, _) = output.into_parts();
            let text = std::str::from_utf8(&stdout).map_err(|error| error.to_string())?;
            if !status.success() || timeout || !text.contains("\"reason\":\"compiler-artifact\"")
                || !text.contains("\"reason\":\"build-finished\",\"success\":true")
            {
                return Err("actual Cargo JSON did not survive qualified socket capture".to_string());
            }
            let output = run(
                &shell("head -c 8388608 /dev/zero"), None,
                CompleteCaptureBudget::new(Duration::from_secs(120), 0, 8 * 1024 * 1024, 64 * 1024),
                Arc::new(()), |_| {},
            ).map_err(|error| error.to_string())?;
            let (status, stdout, _, _, timeout, _) = output.into_parts();
            if !status.success() || timeout || stdout.len() != 8 * 1024 * 1024 || stdout.iter().any(|byte| *byte != 0) {
                return Err("native 8MiB output was changed, truncated or timed out".to_string());
            }
            Ok(())
        }

        #[test]
        fn stream_ceilings_and_invalid_deadlines_refuse_before_transport_setup() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let marker = fixture.0.join("started");
            let args = vec!["-c".to_string(), "printf ran > \"$1\"".to_string(),
                "admission-proof".to_string(), marker.display().to_string()];
            let cases = [
                (CompleteCaptureBudget::new(Duration::from_secs(1), MAX_STREAM_BYTES + 1, 64, 64), "stdin byte budget exceeds"),
                (CompleteCaptureBudget::new(Duration::from_secs(1), 1, MAX_STREAM_BYTES + 1, 64), "stdout byte budget exceeds"),
                (CompleteCaptureBudget::new(Duration::from_secs(1), 1, 64, MAX_STREAM_BYTES + 1), "stderr byte budget exceeds"),
                (CompleteCaptureBudget::new(Duration::from_secs(1), 1, 1024 * 1024 * 1024, 64), "stdout byte budget exceeds"),
                (CompleteCaptureBudget::new(Duration::from_secs(1), 1, 64, 1024 * 1024 * 1024), "stderr byte budget exceeds"),
                (CompleteCaptureBudget::new(Duration::ZERO, 1, 64, 64), "has no deadline"),
                (CompleteCaptureBudget::new(Duration::MAX, 1, 64, 64), "deadline overflow"),
            ];
            for (budget, expected) in cases {
                let attempts = super::super::super::linux::spawn_attempts();
                let pairs = HOOKS.with(|hooks| hooks.borrow().pair_attempts);
                let mut observed = false;
                // This input would independently fail its one-byte admission.
                // The execution preflight must run before input copy or sockets.
                let mut failure = refused(
                    run(&args, Some(b"too-large"), budget, Arc::new(()), |_| observed = true),
                    expected,
                )?;
                if observed || marker.exists() || failure.take_cleanup_receipt().is_some()
                    || failure.observed_outcome().is_some() || failure.is_timeout_only()
                    || failure.take_failed_observation().is_some()
                    || super::super::super::linux::spawn_attempts() != attempts
                    || HOOKS.with(|hooks| hooks.borrow().pair_attempts) != pairs
                {
                    return Err("invalid admission allocated transport, spawned or fabricated observations".to_string());
                }
            }
            Ok(())
        }

        #[test]
        fn exact_stream_ceiling_preserves_small_native_capture() -> Result<(), String> {
            let output = run(
                &shell("printf ceiling; printf accepted >&2"), None,
                CompleteCaptureBudget::new(
                    Duration::from_secs(2), MAX_STREAM_BYTES, MAX_STREAM_BYTES, MAX_STREAM_BYTES,
                ),
                Arc::new(()), |_| {},
            ).map_err(|error| error.to_string())?;
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
            let args = vec!["-c".to_string(), "printf ran > \"$1\"".to_string(),
                "input-proof".to_string(), marker.display().to_string()];
            let lease = Arc::new(());
            let attempts = super::super::super::linux::spawn_attempts();
            let mut observed = false;
            let mut failure = refused(
                run(&args, Some(b"too-large"), budget(1, 64), lease, |_| observed = true),
                "stdin exceeds its 1-byte input budget",
            )?;
            if observed || marker.exists() || failure.take_cleanup_receipt().is_some()
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
            (CompleteCaptureBudget::new(Duration::from_secs(1), 0, 1, 1), "unsupported control"),
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
            },
            Ok(_) => Err("unsupported platform launched a complete capture".to_string()),
        }
    }
}
