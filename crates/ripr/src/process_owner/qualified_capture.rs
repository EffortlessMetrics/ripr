//! Qualified cooperative Linux process-group ownership and complete byte capture.
//! A saved PID, profile, request, or JSON receipt grants no signaling authority.

mod bytes;
pub use bytes::{
    CompleteByteCapture, CompleteCaptureBudget, CompleteCaptureError, CompleteCaptureReceipt,
    CompleteCapturedBytes,
};

#[cfg(target_os = "linux")]
mod linux {
    use super::super::OwnedProcess;
    use std::fs;
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const POST_KILL_GROUP_CONFIRM_GRACE: Duration = Duration::from_secs(2);

    #[cfg(test)]
    thread_local! {
        static SPAWN_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static SIGNAL_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    #[cfg(test)]
    pub(super) fn spawn_attempts() -> usize {
        SPAWN_ATTEMPTS.with(std::cell::Cell::get)
    }

    /// Read-only process facts; this value does not authorize signaling.
    pub struct ObservedProcessIdentity(Identity);

    impl ObservedProcessIdentity {
        /// Observe one bounded native process record.
        pub fn read(pid: u32) -> Result<Self, String> {
            identity(pid).map(Self)
        }
        /// Observed PID.
        pub fn pid(&self) -> u32 {
            self.0.pid
        }
        /// Observed parent PID.
        pub fn parent(&self) -> u32 {
            self.0.parent
        }
        /// Observed process group.
        pub fn group(&self) -> u32 {
            self.0.group
        }
        /// Native process start ticks.
        pub fn start(&self) -> u64 {
            self.0.start
        }
        /// Native lifecycle state.
        pub fn state(&self) -> char {
            self.0.state
        }
    }

    /// Group-only settlement; it does not certify pipe EOF or helper release.
    pub struct QualifiedGroupSettlement {
        parent: Identity,
        leader: Identity,
        status: ExitStatus,
    }
    impl QualifiedGroupSettlement {
        /// Actual settled primary status, retained as data.
        pub fn status(&self) -> ExitStatus {
            self.status
        }
        /// Initial observed worker identity data, retained after settlement.
        pub fn observed_worker(&self) -> ObservedProcessIdentity {
            ObservedProcessIdentity(self.leader.clone())
        }
        /// Initial observed parent identity data, retained after settlement.
        pub fn observed_parent(&self) -> ObservedProcessIdentity {
            ObservedProcessIdentity(self.parent.clone())
        }
        pub(super) fn belongs_to_current_parent(&self) -> bool {
            self.parent.pid == std::process::id()
                && self.leader.parent == self.parent.pid
                && self.leader.pid == self.leader.group
        }
    }

    /// Result data from a qualified wait.
    pub struct QualifiedGroupWait {
        status: ExitStatus,
        duration: Duration,
        timed_out: bool,
    }
    impl QualifiedGroupWait {
        /// Consume the observation without manufacturing a combined receipt.
        pub fn into_parts(self) -> (ExitStatus, Duration, bool) {
            (self.status, self.duration, self.timed_out)
        }
    }

    const PROC_BYTES: u64 = 4096;
    const PROC_ENTRIES: usize = 4096;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Identity {
        pid: u32,
        parent: u32,
        group: u32,
        start: u64,
        state: char,
    }
    impl Identity {
        fn same_owner(&self, other: &Self) -> bool {
            self.pid == other.pid
                && self.parent == other.parent
                && self.group == other.group
                && self.start == other.start
        }
        fn live(&self) -> bool {
            !matches!(self.state, 'Z' | 'X' | 'x')
        }
    }
    fn parse_identity(text: &str, pid: u32) -> Result<Identity, String> {
        let (number, _) = text
            .split_once(' ')
            .ok_or_else(|| "owned capture process stat has no PID".to_string())?;
        if number.parse::<u32>().ok() != Some(pid) {
            return Err("owned capture process stat PID mismatch".to_string());
        }
        let close = text
            .rfind(')')
            .ok_or_else(|| "owned capture process stat has no comm terminator".to_string())?;
        let fields: Vec<_> = text[close + 1..].split_whitespace().collect();
        let state = fields
            .first()
            .and_then(|value| {
                let mut chars = value.chars();
                let state = chars.next()?;
                chars.next().is_none().then_some(state)
            })
            .ok_or_else(|| "owned capture process state unavailable".to_string())?;
        if !matches!(
            state,
            'R' | 'S' | 'D' | 'Z' | 'T' | 't' | 'X' | 'x' | 'K' | 'W' | 'P' | 'I'
        ) {
            return Err("owned capture process state unsupported; ownership unavailable".to_string());
        }
        let number = |index: usize| {
            fields
                .get(index)
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| "owned capture process identity unavailable".to_string())
        };
        Ok(Identity {
            pid,
            parent: u32::try_from(number(1)?)
                .map_err(|err| format!("owned capture parent PID: {err}"))?,
            group: u32::try_from(number(2)?).map_err(|err| format!("owned capture PGID: {err}"))?,
            start: number(19)?,
            state,
        })
    }
    fn identity(pid: u32) -> Result<Identity, String> {
        let mut text = String::new();
        fs::File::open(format!("/proc/{pid}/stat"))
            .map_err(|err| format!("owned capture process {pid} unavailable: {err}"))?
            .take(PROC_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|err| format!("owned capture process {pid} stat: {err}"))?;
        if text.len() as u64 > PROC_BYTES {
            return Err("owned capture process stat exceeds its bound".to_string());
        }
        parse_identity(&text, pid)
    }
    fn require_owner(expected: &Identity, observed: &Identity) -> Result<(), String> {
        if !expected.same_owner(observed) || expected.pid != expected.group {
            return Err("owned capture leader identity mismatch; group signal refused".to_string());
        }
        Ok(())
    }
    fn signal_authority(
        expected: &Identity,
        observed: Result<Identity, String>,
        members: Result<Vec<u32>, String>,
    ) -> Result<Vec<u32>, String> {
        require_owner(expected, &observed?)?;
        members
    }
    fn read_scanned_identity(reader: impl Read, pid: u32) -> Result<Option<Identity>, String> {
        // An opened /proc task descriptor can return ESRCH after that task exits.
        // Keep raw partial bytes: read_to_string can discard invalid UTF-8 while
        // retaining the I/O error, so an empty String cannot establish disappearance.
        let mut bytes = Vec::new();
        if let Err(err) = reader.take(PROC_BYTES + 1).read_to_end(&mut bytes) {
            if err.raw_os_error() == Some(3) && bytes.is_empty() {
                // One absent entry grants no leader lease or settlement credit.
                return Ok(None);
            }
            return Err(format!("owned capture /proc/{pid}/stat: {err}"));
        }
        // Preserve successful-read UTF-8 error precedence over the size guard.
        let text = std::str::from_utf8(&bytes).map_err(|_invalid_utf8| {
            format!("owned capture /proc/{pid}/stat: stream did not contain valid UTF-8")
        })?;
        if bytes.len() as u64 > PROC_BYTES {
            return Err("owned capture scanned stat exceeds its bound".to_string());
        }
        parse_identity(text, pid).map(Some)
    }
    fn scan_group(group: u32, deadline: Instant) -> Result<Vec<u32>, String> {
        let entries = fs::read_dir("/proc")
            .map_err(|err| format!("owned capture complete group scan unavailable: {err}"))?;
        let mut live = Vec::new();
        for (index, entry) in entries.enumerate() {
            if Instant::now() >= deadline {
                return Err("owned capture complete group scan exceeded settlement bound".to_string());
            }
            if index >= PROC_ENTRIES {
                return Err("owned capture complete group scan exceeded entry bound".to_string());
            }
            let entry = entry.map_err(|err| format!("owned capture /proc entry: {err}"))?;
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else {
                continue;
            };
            // Disappeared tasks are not survivors. Every other unavailable numeric
            // entry prevents a complete scan and therefore prevents group signals.
            match fs::File::open(entry.path().join("stat")) {
                Ok(file) => {
                    if let Some(process) = read_scanned_identity(file, pid)?
                        && process.group == group
                        && process.live()
                    {
                        live.push(pid);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(format!("owned capture complete group scan refused: {err}")),
            }
        }
        Ok(live)
    }

    /// Owns a fresh unreaped direct-child group until qualified settlement.
    pub struct QualifiedGroupOwner {
        child: OwnedProcess,
        leader: Identity,
        settled: bool,
        refused: bool,
        parent: Identity,
        settlement: Option<QualifiedGroupSettlement>,
        settled_status: Option<ExitStatus>,
        held_deadline: Option<Instant>,
    }
    impl QualifiedGroupOwner {
        /// Spawn and immediately lease a fresh direct-child Linux group.
        pub fn spawn(mut command: Command) -> Result<Self, String> {
            let parent = identity(std::process::id())?;
            if !parent.live() {
                return Err("owned capture parent identity is not live".to_string());
            }
            command.process_group(0);
            #[cfg(test)]
            SPAWN_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
            let child = OwnedProcess::spawn_with_bounded_drop(command)
                .map_err(|error| error.to_string())?;
            let leader = identity(child.id())?;
            if leader.pid != leader.group || leader.parent != parent.pid {
                return Err("owned capture did not create its own direct-child group".to_string());
            }
            Ok(Self {
                child,
                leader,
                settled: false,
                refused: false,
                parent,
                settlement: None,
                settled_status: None,
                held_deadline: None,
            })
        }

        /// Strict capture keeps its original total ceiling through every fallback.
        pub(crate) fn spawn_with_deadline(
            mut command: Command,
            held_deadline: Instant,
        ) -> Result<Self, String> {
            held_time(held_deadline)?;
            let parent = identity(std::process::id())?;
            held_time(held_deadline)?;
            if !parent.live() {
                return Err("owned capture parent identity is not live".to_string());
            }
            command.process_group(0);
            held_time(held_deadline)?;
            #[cfg(test)]
            SPAWN_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
            let child = OwnedProcess::spawn_with_bounded_drop_until(command, held_deadline)
                .map_err(|error| error.to_string())?;
            let leader = identity(child.id())?;
            held_time(held_deadline)?;
            if leader.pid != leader.group || leader.parent != parent.pid {
                return Err("owned capture did not create its own direct-child group".to_string());
            }
            Ok(Self {
                child, leader, settled: false, refused: false, parent,
                settlement: None, settled_status: None,
                held_deadline: Some(held_deadline),
            })
        }

        fn check_held(&mut self) -> Result<(), String> {
            if let Some(deadline) = self.held_deadline
                && let Err(error) = held_time(deadline)
            {
                self.refused = true;
                self.settlement = None;
                return Err(error);
            }
            Ok(())
        }

        fn after_held<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
            match (result, self.check_held()) {
                (Err(primary), Err(clock)) => Err(format!("{primary}; {clock}")),
                (Err(primary), Ok(())) => Err(primary),
                (Ok(_), Err(clock)) => Err(clock),
                (Ok(value), Ok(())) => Ok(value),
            }
        }

        fn check_phase(&mut self, deadline: Instant) -> Result<(), String> {
            self.check_held()?;
            if self.held_deadline.is_some() && Instant::now() >= deadline {
                self.refused = true;
                self.settlement = None;
                return Err("owned capture settlement phase expired; cleanup unconfirmed".to_string());
            }
            Ok(())
        }

        fn after_phase<T>(
            &mut self,
            result: Result<T, String>,
            deadline: Instant,
        ) -> Result<T, String> {
            match (result, self.check_phase(deadline)) {
                (Err(primary), Err(clock)) => Err(format!("{primary}; {clock}")),
                (Err(primary), Ok(())) => Err(primary),
                (Ok(_), Err(clock)) => Err(clock),
                (Ok(value), Ok(())) => Ok(value),
            }
        }

        fn phase_deadline(&self, deadline: Instant) -> Instant {
            self.held_deadline.map_or(deadline, |held| deadline.min(held))
        }

        fn pause(&self, duration: Duration, deadline: Instant) {
            let duration = if self.held_deadline.is_some() {
                duration.min(deadline.saturating_duration_since(Instant::now()))
            } else {
                duration
            };
            thread::sleep(duration);
        }

        /// The actual unreaped primary PID.
        pub fn id(&self) -> u32 {
            self.child.id()
        }

        /// Transfer the child's stdin pipe without exposing reaping authority.
        pub fn take_stdin_pipe(&mut self) -> Option<ChildStdin> {
            self.child.stdin_pipe().take()
        }

        /// Transfer the child's stdout pipe without exposing reaping authority.
        pub fn take_stdout_pipe(&mut self) -> Option<ChildStdout> {
            self.child.stdout_pipe().take()
        }

        /// Transfer the child's stderr pipe without exposing reaping authority.
        pub fn take_stderr_pipe(&mut self) -> Option<ChildStderr> {
            self.child.stderr_pipe().take()
        }

        /// Cached status data is available only after qualified settlement and reap.
        pub fn settled_status(&self) -> Option<ExitStatus> {
            self.settled_status
        }

        /// Transfer the one-shot group-only settlement observation.
        pub fn take_settlement(&mut self) -> Option<QualifiedGroupSettlement> {
            if self.check_held().is_err() {
                return None;
            }
            self.settlement.take()
        }

        fn qualified(&mut self) -> Result<Identity, String> {
            self.check_held()?;
            if self.refused {
                return Err("owned capture group settlement previously refused".to_string());
            }
            let result = identity(self.leader.pid).and_then(|observed| {
                require_owner(&self.leader, &observed)?;
                Ok(observed)
            });
            if result.is_err() {
                self.refused = true;
            }
            self.after_held(result)
        }
        fn members(&mut self, deadline: Instant) -> Result<Vec<u32>, String> {
            let deadline = self.phase_deadline(deadline);
            self.check_phase(deadline)?;
            let result = scan_group(self.leader.group, deadline);
            if result.is_err() {
                self.refused = true;
            }
            self.after_phase(result, deadline)
        }
        fn signal(&mut self, deadline: Instant) -> Result<(), String> {
            self.check_held()?;
            if self.refused {
                return Err("owned capture group settlement previously refused".to_string());
            }
            let observed = self.qualified();
            let members = self.members(deadline);
            signal_authority(&self.leader, observed, members)?;
            self.qualified()?;
            let result = bounded_group_signal_with_held(
                self.leader.group, self.phase_deadline(deadline), self.held_deadline,
            );
            self.after_held(result)
        }
        fn settle(&mut self, terminate: bool) -> Result<(ExitStatus, bool), String> {
            let result = self.settle_qualified(terminate);
            if result.is_err() && !self.settled {
                self.refused = true;
            }
            result
        }
        fn settle_qualified(&mut self, terminate: bool) -> Result<(ExitStatus, bool), String> {
            self.check_held()?;
            if self.refused {
                return Err("owned capture group settlement refused; cleanup unconfirmed".to_string());
            }
            let deadline = self.phase_deadline(Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE);
            self.check_phase(deadline)?;
            let initial = self.qualified()?;
            let initial_members = self.members(deadline)?;
            let unexpected_survivor = !initial.live() && !initial_members.is_empty();
            let mut signaled = false;
            if (terminate || unexpected_survivor) && !initial_members.is_empty() {
                self.signal(deadline)?;
                signaled = true;
            }
            loop {
                self.qualified()?;
                if self.members(deadline)?.is_empty() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("owned capture group remains live; cleanup unconfirmed".to_string());
                }
                self.signal(deadline)?;
                signaled = true;
                self.pause(Duration::from_millis(10), deadline);
            }
            // The numeric group lease remains pinned by this actual unreaped child
            // until all live members are gone. Only now may try_wait reap it.
            let status = loop {
                self.check_phase(deadline)?;
                let observed = self.child.try_wait()
                    .map_err(|err| format!("owned capture bounded reap: {err}"));
                if let Some(status) = self.after_phase(observed, deadline)? {
                    break status;
                }
                if Instant::now() >= deadline {
                    return Err("owned capture primary reap exceeded settlement bound".to_string());
                }
                self.pause(Duration::from_millis(10), deadline);
            };
            self.check_phase(deadline)?;
            self.settled = true;
            self.settled_status = Some(status);
            self.settlement = Some(QualifiedGroupSettlement {
                parent: self.parent.clone(),
                leader: self.leader.clone(),
                status,
            });
            if unexpected_survivor {
                return Err("owned capture primary exited with live group members; cleanup failure despite settlement".to_string());
            }
            Ok((status, signaled))
        }
        pub fn wait(
            &mut self,
            started: Instant,
            timeout: Duration,
        ) -> Result<QualifiedGroupWait, String> {
            self.wait_supervised(started, timeout, || Ok(()))
        }

        pub fn wait_supervised(
            &mut self,
            started: Instant,
            timeout: Duration,
            mut monitor: impl FnMut() -> Result<(), String>,
        ) -> Result<QualifiedGroupWait, String> {
            loop {
                self.check_held()?;
                let monitored = monitor();
                self.after_held(monitored)?;
                let leader = self.qualified()?;
                if !leader.live() || started.elapsed() >= timeout {
                    let expired = leader.live() && started.elapsed() >= timeout;
                    let (status, signaled) = self.settle(expired)?;
                    return Ok(QualifiedGroupWait {
                        status,
                        duration: started.elapsed(),
                        timed_out: expired && signaled,
                    });
                }
                if let Some(held) = self.held_deadline {
                    let deadline = started.checked_add(timeout).map_or(held, |phase| phase.min(held));
                    self.pause(Duration::from_millis(100), deadline);
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        pub fn abort(&mut self) -> Result<(), String> {
            self.check_held()?;
            if self.settled {
                return Ok(());
            }
            self.settle(true).map(|_| ())
        }
    }
    impl Drop for QualifiedGroupOwner {
        fn drop(&mut self) {
            if !self.settled && !self.refused {
                let _ = self.abort();
            }
            // Unconfirmed scope is never retried as a numeric group kill. The
            // underlying OwnedProcess performs direct-handle bounded fallback only.
        }
    }

    fn held_time(deadline: Instant) -> Result<(), String> {
        if Instant::now() >= deadline {
            Err("owned capture held custody deadline expired; cleanup unconfirmed".to_string())
        } else {
            Ok(())
        }
    }

    fn bounded_group_signal_with_held(
        group: u32,
        deadline: Instant,
        held_deadline: Option<Instant>,
    ) -> Result<(), String> {
        let deadline = held_deadline.map_or(deadline, |held| deadline.min(held));
        if held_deadline.is_some() {
            held_time(deadline)?;
        }
        if Instant::now() >= deadline {
            return Err("owned group signal has no remaining settlement budget".to_string());
        }
        #[cfg(test)]
        SIGNAL_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
        let mut command = Command::new("/usr/bin/kill");
        command
            .args(["-KILL", "--", &format!("-{group}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if held_deadline.is_some() {
            held_time(deadline)?;
        }
        let mut child = match held_deadline {
            Some(held) => OwnedProcess::spawn_with_bounded_drop_until(command, deadline.min(held)),
            None => OwnedProcess::spawn_with_bounded_drop(command),
        }.map_err(|err| format!("owned group signal launch: {err}"))?;
        loop {
            if held_deadline.is_some() {
                held_time(deadline)?;
            }
            let observed = child.try_wait()
                .map_err(|err| format!("owned group signal poll: {err}"))?;
            if held_deadline.is_some() {
                held_time(deadline)?;
            }
            if let Some(status) = observed {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "owned group signal exited {status}; cleanup unconfirmed"
                    ))
                };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err(
                    "owned group signal exceeded settlement bound; cleanup unconfirmed".to_string(),
                );
            }
            let delay = if held_deadline.is_some() {
                Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now()))
            } else {
                Duration::from_millis(5)
            };
            thread::sleep(delay);
        }
    }


    #[cfg(test)]
    mod tests {
        use super::*;
        fn test_command(program: &str, args: &[&str], group: bool) -> Command {
            let mut command = Command::new(program);
            command
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if group {
                command.process_group(0);
            }
            command
        }
        fn require_error<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
            match result {
                Err(error) if error.contains(expected) => Ok(()),
                Err(error) => Err(format!("expected {expected:?}, observed {error:?}")),
                Ok(_) => Err(format!("expected refusal containing {expected:?}")),
            }
        }
        fn require_not_live(pid: u32) -> Result<(), String> {
            match identity(pid) {
                Ok(process) if process.live() => Err(format!("owned process {pid} remains live")),
                Ok(_) => Ok(()),
                Err(error) => {
                    // Only actual absence is gone; unreadable/invalid stat is not evidence.
                    match fs::symlink_metadata(format!("/proc/{pid}")) {
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        _ => Err(error),
                    }
                }
            }
        }

        #[test]
        fn scanned_stat_opened_descriptor_disappearance_is_not_a_survivor() -> Result<(), String> {
            let mut child =
                OwnedProcess::spawn_with_bounded_drop(test_command("/usr/bin/sleep", &["30"], true))
                    .map_err(|err| format!("opened stat owned child: {err}"))?;
            let pid = child.id();
            let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
            let before = loop {
                let observed = identity(pid)?;
                if observed.parent != std::process::id() || observed.group != pid || !observed.live() {
                    return Err("opened stat child has no live direct-owner identity".to_string());
                }
                if observed.state == 'S' {
                    break observed;
                }
                if Instant::now() >= deadline {
                    return Err("opened stat child did not reach its sleeping premise".to_string());
                }
                thread::sleep(Duration::from_millis(5));
            };
            // Independent open calls are essential: dup/try_clone shares the offset,
            // which could turn the second observation into an empty EOF.
            let path = format!("/proc/{pid}/stat");
            let witness = fs::File::open(&path)
                .map_err(|err| format!("opened stat witness descriptor: {err}"))?;
            let scanned = fs::File::open(&path)
                .map_err(|err| format!("opened stat production descriptor: {err}"))?;
            let still_live = identity(pid)?;
            if !before.same_owner(&still_live) || !still_live.live() {
                return Err("opened stat identity changed before direct-owner termination".to_string());
            }
            child
                .kill()
                .map_err(|err| format!("opened stat direct-owner termination: {err}"))?;
            let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
            loop {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|err| format!("opened stat bounded reap: {err}"))?
                {
                    if status.success() {
                        return Err("opened stat owned child exited without termination".to_string());
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("opened stat owned child reap unconfirmed".to_string());
                }
                thread::sleep(Duration::from_millis(5));
            }
            let mut bytes = Vec::new();
            match witness.take(PROC_BYTES + 1).read_to_end(&mut bytes) {
                Err(err) if err.raw_os_error() == Some(3) && bytes.is_empty() => {}
                observed => {
                    return Err(format!(
                        "native unread stat disappearance did not produce empty ESRCH: {observed:?}"
                    ));
                }
            }
            // This is the same bounded reader called by scan_group, after the
            // independent descriptor established the native Linux lifecycle.
            match read_scanned_identity(scanned, pid) {
                Ok(None) => Ok(()),
                observed => Err(format!(
                    "opened stat disappearance must be absent after native ESRCH: {observed:?}"
                )),
            }
        }

        #[test]
        fn scanned_stat_retains_exact_live_dead_and_foreign_identities() -> Result<(), String> {
            for (state, group, live) in [
                ('S', 9, true),
                ('Z', 9, false),
                ('X', 12, false),
                ('R', 12, true),
            ] {
                let text = format!("9 (owned) {state} 1 {group} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17");
                let expected = Identity {
                    pid: 9,
                    parent: 1,
                    group,
                    start: 17,
                    state,
                };
                let observed = read_scanned_identity(text.as_bytes(), 9)?
                    .ok_or_else(|| "valid scanned identity was discarded".to_string())?;
                assert_eq!(observed, expected);
                assert_eq!(observed.live(), live);
            }
            Ok(())
        }

        #[test]
        fn scanned_stat_refuses_incomplete_invalid_and_oversized_records() -> Result<(), String> {
            for (text, expected) in [
                ("", "no PID"),
                ("9 (bad) S 1 9", "identity unavailable"),
                (
                    "8 (owned) S 1 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17",
                    "PID mismatch",
                ),
                (
                    "9 (owned) Q 1 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17",
                    "state unsupported",
                ),
            ] {
                require_error(read_scanned_identity(text.as_bytes(), 9), expected)?;
            }
            require_error(
                read_scanned_identity(&[0xff][..], 9),
                "stream did not contain valid UTF-8",
            )?;
            let oversized = "a".repeat((PROC_BYTES + 1) as usize);
            require_error(
                read_scanned_identity(oversized.as_bytes(), 9),
                "exceeds its bound",
            )?;
            Ok(())
        }

        struct ReadFailure {
            prefix: std::io::Cursor<Vec<u8>>,
            code: i32,
        }
        impl Read for ReadFailure {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let count = self.prefix.read(buffer)?;
                if count == 0 {
                    Err(std::io::Error::from_raw_os_error(self.code))
                } else {
                    Ok(count)
                }
            }
        }

        #[test]
        fn scanned_stat_refuses_other_io_errors_and_partial_esrch() -> Result<(), String> {
            for (prefix, code) in [
                (Vec::new(), 13),
                (Vec::new(), 5),
                (Vec::new(), 2),
                (b"9 (owned)".to_vec(), 3),
                (
                    b"9 (owned) S 1 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17".to_vec(),
                    3,
                ),
                (vec![0xff], 3),
            ] {
                let failure = ReadFailure {
                    prefix: std::io::Cursor::new(prefix),
                    code,
                };
                let expected = format!(
                    "owned capture /proc/9/stat: {}",
                    std::io::Error::from_raw_os_error(code)
                );
                match read_scanned_identity(failure, 9) {
                    Err(observed) => assert_eq!(observed, expected),
                    Ok(_) => return Err("scanned stat discarded an original I/O failure".to_string()),
                }
            }
            Ok(())
        }
        #[test]
        fn scanned_stat_accepts_only_zero_byte_esrch_as_absent() -> Result<(), String> {
            let failure = ReadFailure {
                prefix: std::io::Cursor::new(Vec::new()),
                code: 3,
            };
            assert_eq!(read_scanned_identity(failure, 9)?, None);
            // Successful empty EOF is an invalid stat, not a disappearance receipt.
            require_error(read_scanned_identity(&[][..], 9), "no PID")?;
            Ok(())
        }

        #[test]
        fn scanned_stat_preserves_exact_byte_bound_and_utf8_error_precedence() -> Result<(), String> {
            let mut text = "9 (owned) S 1 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17".to_string();
            text.extend(std::iter::repeat_n(' ', PROC_BYTES as usize - text.len()));
            let observed = read_scanned_identity(text.as_bytes(), 9)?
                .ok_or_else(|| "exact-bound identity was discarded".to_string())?;
            assert_eq!(
                observed,
                Identity {
                    pid: 9,
                    parent: 1,
                    group: 9,
                    start: 17,
                    state: 'S',
                }
            );
            text.push(' ');
            require_error(
                read_scanned_identity(text.as_bytes(), 9),
                "exceeds its bound",
            )?;
            let mut bytes = text.into_bytes();
            bytes[0] = 0xff;
            require_error(
                read_scanned_identity(bytes.as_slice(), 9),
                "stream did not contain valid UTF-8",
            )?;
            Ok(())
        }

        #[test]
        fn invalid_live_lease_refuses_numeric_signal_and_bounded_drop_remains_separate() -> Result<(), String> {
            // An altered lease makes the actual live guard refuse without group kill.
            let mut guard = QualifiedGroupOwner::spawn(test_command("sleep", &["30"], true))?;
            let pid = guard.id();
            guard.leader.start += 1;
            let started = Instant::now();
            require_error(guard.abort(), "identity mismatch")?;
            if !identity(pid)?.live() {
                return Err("invalid lease dispatched a group kill".to_string());
            }
            // A refused guard cannot retry its numeric group on drop. Direct-handle
            // fallback remains separate, bounded, and makes no family certificate.
            drop(guard);
            if started.elapsed() > Duration::from_secs(6) {
                return Err("refusal fallback exceeded existing direct reap budget".to_string());
            }
            require_not_live(pid)?;
            Ok(())
        }

        // Explicit fixture closeout through the actual original direct handle.
        // This is not a group/capture receipt and does not extend production clocks.
        fn close_fixture(child: &mut OwnedProcess) -> Result<(), String> {
            child.request_kill().map_err(|error| error.to_string())?;
            if !child.reap_within(Duration::from_secs(5)) {
                return Err("strict-clock fixture direct reap was unconfirmed".to_string());
            }
            Ok(())
        }

        #[test]
        fn strict_expired_clock_precedes_missing_worker_and_fixed_signal() -> Result<(), String> {
            let attempts = spawn_attempts();
            require_error(
                QualifiedGroupOwner::spawn_with_deadline(
                    test_command("/ripr-deliberately-missing-held-worker", &[], true),
                    Instant::now(),
                ),
                "held custody deadline",
            )?;
            assert_eq!(spawn_attempts(), attempts, "expired clock must not attempt a worker spawn");

            let mut fixture = OwnedProcess::spawn_with_bounded_drop(
                test_command("/usr/bin/sleep", &["30"], true),
            ).map_err(|error| error.to_string())?;
            let pid = fixture.id();
            let attempts = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            let result = bounded_group_signal_with_held(
                pid, Instant::now() + Duration::from_secs(2), Some(Instant::now()),
            );
            let still_live = identity(pid)?.live();
            let final_attempts = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            close_fixture(&mut fixture)?;
            require_error(result, "held custody deadline")?;
            assert_eq!(final_attempts, attempts, "expired clock must not launch fixed kill");
            if !still_live {
                return Err("expired clock dispatched a signal to its controlled fixture".to_string());
            }
            Ok(())
        }

        #[test]
        fn strict_monitor_expiry_poison_is_shared_with_abort_and_drop_owner() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let mut guard = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/usr/bin/sleep", &["30"], true), held,
            )?;
            assert_eq!(guard.held_deadline, Some(held));
            assert_eq!(guard.child.bounded_drop_until, Some(held));
            let signals = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            let result = guard.wait_supervised(Instant::now(), Duration::from_secs(2), || {
                thread::sleep(held.saturating_duration_since(Instant::now()) + Duration::from_millis(10));
                Ok(())
            });
            let abort = guard.abort();
            let refused = guard.refused;
            let settlement = guard.take_settlement();
            let final_signals = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            close_fixture(&mut guard.child)?;
            require_error(result, "held custody deadline")?;
            require_error(abort, "held custody deadline")?;
            assert_eq!(final_signals, signals, "expired abort must not reset the signal clock");
            if !refused || settlement.is_some() || guard.settled_status().is_some() {
                return Err("late monitor manufactured settlement or lost sticky refusal".to_string());
            }
            Ok(())
        }

        #[test]
        fn strict_direct_drop_and_reap_cannot_restart_expired_grace() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let mut child = OwnedProcess::spawn_with_bounded_drop_until(
                test_command("/usr/bin/sleep", &["30"], true), held,
            ).map_err(|error| error.to_string())?;
            assert_eq!(child.bounded_drop_until, Some(held));
            thread::sleep(held.saturating_duration_since(Instant::now()) + Duration::from_millis(10));
            let started = Instant::now();
            let reaped = child.reap_until(held);
            child.drop_until(held);
            let elapsed = started.elapsed();
            let still_live = identity(child.id())?.live();
            close_fixture(&mut child)?;
            if reaped || !still_live || elapsed >= Duration::from_millis(500) {
                return Err(format!("expired direct fallback reset its grace: reaped={reaped}, live={still_live}, elapsed={elapsed:?}"));
            }
            Ok(())
        }

        #[test]
        fn strict_actual_group_settlement_retains_original_clock_and_status() -> Result<(), String> {
            let held = Instant::now() + Duration::from_secs(2);
            let mut guard = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/bin/sh", &["-c", "exit 7"], true), held,
            )?;
            let outcome = guard.wait(Instant::now(), Duration::from_secs(2))?;
            let (status, _, timed_out) = outcome.into_parts();
            if timed_out || status.code() != Some(7) {
                return Err("strict actual primary status changed".to_string());
            }
            let receipt = guard.take_settlement()
                .ok_or_else(|| "strict timely group lost its real settlement".to_string())?;
            assert_eq!(receipt.status(), status);
            assert_eq!(guard.held_deadline, Some(held));
            if guard.take_settlement().is_some() {
                return Err("strict group settlement was cloned".to_string());
            }
            Ok(())
        }

        #[test]
        fn reaped_primary_cannot_supply_a_group_settlement() -> Result<(), String> {
            let mut guard = QualifiedGroupOwner::spawn(test_command("sh", &["-c", "exit 0"], true))?;
            let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
            loop {
                if guard.child.try_wait().map_err(|error| error.to_string())?.is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("premature reap control did not observe actual primary exit".to_string());
                }
                thread::sleep(Duration::from_millis(5));
            }
            require_error(guard.abort(), "unavailable")?;
            if guard.take_settlement().is_some() || guard.settled_status().is_some() {
                return Err("premature direct reap manufactured a group settlement".to_string());
            }
            Ok(())
        }
    }

}
#[cfg(target_os = "linux")]
pub use linux::{
    ObservedProcessIdentity, QualifiedGroupOwner, QualifiedGroupSettlement, QualifiedGroupWait,
};
