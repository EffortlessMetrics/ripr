//! Qualified cooperative Linux process-group ownership and complete byte capture.
//! A saved PID, profile, request, or JSON receipt grants no signaling authority.

mod bytes;
pub use bytes::{
    CompleteByteCapture, CompleteCaptureBudget, CompleteCaptureError, CompleteCaptureReceipt,
    CompleteCapturedBytes,
};

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) use bytes::{
    CompleteEnclosingCustodian, CompleteEnclosingDisposed, CompleteEnclosingFailure,
    CompleteEnclosingPhysicalClosure, CompleteFailedClosed, CompleteTerminalCustodian,
    CompleteTerminalFailure,
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

    #[cfg(all(test, feature = "lang-rust"))]
    pub(super) fn terminal_settlement_grace() -> Duration {
        POST_KILL_GROUP_CONFIRM_GRACE
    }

    #[cfg(all(test, feature = "lang-rust"))]
    thread_local! {
        static LAST_WAIT_WINDOW: std::cell::Cell<Option<(Instant, Duration)>> = const { std::cell::Cell::new(None) };
    }

    #[cfg(all(test, feature = "lang-rust"))]
    pub(super) fn last_wait_window() -> Option<(Instant, Duration)> {
        LAST_WAIT_WINDOW.with(std::cell::Cell::get)
    }

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
            return Err(
                "owned capture process state unsupported; ownership unavailable".to_string(),
            );
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
                return Err(
                    "owned capture complete group scan exceeded settlement bound".to_string(),
                );
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
                Err(err) => {
                    return Err(format!("owned capture complete group scan refused: {err}"));
                }
            }
        }
        Ok(live)
    }

    /// Actual progress only; every field begins empty before any capture attempt.
    #[cfg(all(test, feature = "lang-rust"))]
    pub(crate) struct EnclosingDispositionProgress {
        leader: Option<Identity>,
        parent: Option<Identity>,
        primary_status: Option<ExitStatus>,
        helper_status: Option<ExitStatus>,
        no_live_members: bool,
        empty_before_reap_at: Option<Instant>,
        group_deadline: Option<Instant>,
    }

    #[cfg(all(test, feature = "lang-rust"))]
    impl EnclosingDispositionProgress {
        pub(super) fn new() -> Self {
            Self {
                leader: None,
                parent: None,
                primary_status: None,
                helper_status: None,
                no_live_members: false,
                empty_before_reap_at: None,
                group_deadline: None,
            }
        }

        pub(super) fn observed_worker(&self) -> Option<ObservedProcessIdentity> {
            self.leader.as_ref().cloned().map(ObservedProcessIdentity)
        }

        pub(super) fn observed_parent(&self) -> Option<ObservedProcessIdentity> {
            self.parent.as_ref().cloned().map(ObservedProcessIdentity)
        }

        pub(super) fn admit_group_deadline(&mut self, deadline: Instant) {
            self.group_deadline = Some(deadline);
        }

        pub(super) fn group_deadline(&self) -> Option<Instant> {
            self.group_deadline
        }

        pub(super) fn no_live_members(&self) -> bool {
            self.no_live_members
        }

        pub(super) fn status(&self) -> Option<ExitStatus> {
            self.primary_status
        }
    }

    #[cfg(all(test, feature = "lang-rust"))]
    fn observe_enclosing_worker(
        child: &OwnedProcess,
        expected: Option<&Identity>,
        deadline: Instant,
    ) -> Result<ObservedProcessIdentity, String> {
        super::super::enclosing_time(deadline)?;
        if child.enclosing_observed_reap.is_some() {
            return Err("enclosing retained worker was actually reaped; no PID lookup".to_string());
        }
        let observed = identity(child.id())?;
        super::super::enclosing_time(deadline)?;
        if observed.pid != child.id()
            || observed.pid != observed.group
            || observed.parent != std::process::id()
            || observed.start == 0
            || expected.is_some_and(|original| !original.same_owner(&observed))
        {
            return Err("enclosing retained worker identity audit refused".to_string());
        }
        Ok(ObservedProcessIdentity(observed))
    }

    #[cfg(all(test, feature = "lang-rust"))]
    fn dispose_enclosing_primary(
        child: &mut OwnedProcess,
        expected: Option<(&Identity, &Identity)>,
        deadline: Instant,
        signaled_group: bool,
        progress: &mut EnclosingDispositionProgress,
    ) -> Result<(), String> {
        // These identity observations audit a still-owned, unreaped handle.
        // They never authorize a numeric group signal or renewed qualification.
        super::super::enclosing_time(deadline)?;
        let parent = identity(std::process::id())?;
        super::super::enclosing_time(deadline)?;
        let observed = identity(child.id())?;
        super::super::enclosing_time(deadline)?;
        if parent.pid != std::process::id()
            || !parent.live()
            || observed.pid != child.id()
            || observed.pid != observed.group
            || observed.parent != parent.pid
            || observed.start == 0
        {
            return Err("enclosing direct identity audit refused; custody retained".to_string());
        }
        if let Some((leader, original_parent)) = expected
            && (!leader.same_owner(&observed) || !original_parent.same_owner(&parent))
        {
            return Err("enclosing original group identity changed; custody retained".to_string());
        }
        progress.leader = Some(observed.clone());
        progress.parent = Some(parent);
        child.enclosing_kill_until(deadline)?;
        loop {
            super::super::enclosing_time(deadline)?;
            let actual = identity(child.id())?;
            super::super::enclosing_time(deadline)?;
            if !observed.same_owner(&actual) {
                return Err("enclosing unreaped identity changed; custody retained".to_string());
            }
            let members = scan_group(observed.group, deadline)?;
            super::super::enclosing_time(deadline)?;
            if members.is_empty() {
                progress.no_live_members = true;
                progress.empty_before_reap_at = Some(Instant::now());
                break;
            }
            // Keep the primary unreaped. Only a real signal under the initial
            // U scope allows bounded polling for asynchronous descendant exit.
            // Direct-only custody cannot acquire that authority after failure.
            if !signaled_group && members.iter().any(|pid| *pid != observed.pid) {
                return Err("enclosing live descendants remain; custody retained".to_string());
            }
            thread::sleep(
                Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        child.enclosing_reap_until(deadline, &mut progress.primary_status)?;
        super::super::enclosing_time(deadline)
    }

    #[cfg(all(test, feature = "lang-rust"))]
    fn actual_direct_closure(
        child: &OwnedProcess,
        expected: Option<(&Identity, &Identity)>,
        progress: &EnclosingDispositionProgress,
    ) -> bool {
        let Some((status, reaped_at)) = child.enclosing_observed_reap else {
            return false;
        };
        let (Some(leader), Some(parent), Some(audited_at)) = (
            progress.leader.as_ref(),
            progress.parent.as_ref(),
            progress.empty_before_reap_at,
        ) else {
            return false;
        };
        progress.no_live_members
            && progress.primary_status == Some(status)
            && audited_at <= reaped_at
            && leader.pid == child.id()
            && leader.pid == leader.group
            && leader.parent == parent.pid
            && leader.start != 0
            && expected.is_none_or(|(expected_leader, expected_parent)| {
                expected_leader.same_owner(leader) && expected_parent.same_owner(parent)
            })
    }

    /// Setup or fixed-signal failure with actual unqualified direct-child custody.
    /// No identity, status, or settlement is fabricated from these handles.
    pub(crate) struct GroupSetupFailure {
        message: String,
        child: Option<OwnedProcess>,
    }
    impl GroupSetupFailure {
        fn before_spawn(message: String) -> Self {
            Self {
                message,
                child: None,
            }
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn observe_enclosing_worker(
            &self,
            deadline: Instant,
        ) -> Result<ObservedProcessIdentity, String> {
            let child = self
                .child
                .as_ref()
                .ok_or_else(|| "enclosing setup has no actual pinned child".to_string())?;
            observe_enclosing_worker(child, None, deadline)
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn dispose_enclosing_until(
            &mut self,
            deadline: Instant,
            progress: &mut EnclosingDispositionProgress,
        ) -> Result<(), String> {
            let child = self.child.as_mut().ok_or_else(|| {
                "enclosing setup has no actual pinned child; no disposition".to_string()
            })?;
            dispose_enclosing_primary(child, None, deadline, false, progress)
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn require_actual_enclosing_physical_closure(
            &self,
            progress: &EnclosingDispositionProgress,
        ) -> Result<(), &'static str> {
            if self
                .child
                .as_ref()
                .is_some_and(|child| actual_direct_closure(child, None, progress))
            {
                Ok(())
            } else {
                Err("enclosing setup still lacks genuine physical closure; actual custody retained")
            }
        }

        pub(crate) fn into_parts(self) -> (String, Option<OwnedProcess>) {
            (self.message, self.child)
        }

        pub(crate) fn retained_process_count(&self) -> usize {
            usize::from(self.child.is_some())
        }

        #[cfg(test)]
        pub(crate) fn fixture_child(&mut self) -> Option<&mut OwnedProcess> {
            self.child.as_mut()
        }
    }
    impl std::fmt::Display for GroupSetupFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.message)
        }
    }

    #[cfg(all(test, feature = "lang-rust"))]
    struct EnclosingGroupScope {
        original_deadline: Instant,
        enclosing_deadline: Instant,
        leader: Identity,
        parent: Identity,
        attempted: bool,
        pending_helper: Option<OwnedProcess>,
        empty_audit_before_reap: Option<Instant>,
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
        unconfirmed_signal: Option<OwnedProcess>,
        #[cfg(all(test, feature = "lang-rust"))]
        enclosing_scope: Option<EnclosingGroupScope>,
        #[cfg(all(test, feature = "lang-rust"))]
        enclosing_direct_reaped: bool,
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
                unconfirmed_signal: None,
                #[cfg(all(test, feature = "lang-rust"))]
                enclosing_scope: None,
                #[cfg(all(test, feature = "lang-rust"))]
                enclosing_direct_reaped: false,
            })
        }

        /// Strict capture keeps its original total ceiling through every fallback.
        pub(crate) fn spawn_with_deadline(
            mut command: Command,
            held_deadline: Instant,
        ) -> Result<Self, GroupSetupFailure> {
            held_time(held_deadline).map_err(GroupSetupFailure::before_spawn)?;
            let parent = identity(std::process::id()).map_err(GroupSetupFailure::before_spawn)?;
            held_time(held_deadline).map_err(GroupSetupFailure::before_spawn)?;
            if !parent.live() {
                return Err(GroupSetupFailure::before_spawn(
                    "owned capture parent identity is not live".to_string(),
                ));
            }
            command.process_group(0);
            held_time(held_deadline).map_err(GroupSetupFailure::before_spawn)?;
            #[cfg(test)]
            SPAWN_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
            let child = OwnedProcess::spawn_with_bounded_drop_until(command, held_deadline)
                .map_err(|failure| {
                    let (error, child) = failure.into_parts();
                    GroupSetupFailure {
                        message: error.to_string(),
                        child,
                    }
                })?;
            let observed = (|| {
                let leader = identity(child.id())?;
                held_time(held_deadline)?;
                if leader.pid != leader.group || leader.parent != parent.pid {
                    return Err(
                        "owned capture did not create its own direct-child group".to_string()
                    );
                }
                Ok(leader)
            })();
            let leader = match observed {
                Ok(leader) => leader,
                Err(message) => {
                    return Err(GroupSetupFailure {
                        message,
                        child: Some(child),
                    });
                }
            };
            Ok(Self {
                child,
                leader,
                settled: false,
                refused: false,
                parent,
                settlement: None,
                settled_status: None,
                held_deadline: Some(held_deadline),
                unconfirmed_signal: None,
                #[cfg(all(test, feature = "lang-rust"))]
                enclosing_scope: None,
                #[cfg(all(test, feature = "lang-rust"))]
                enclosing_direct_reaped: false,
            })
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn admit_enclosing_scope(
            &mut self,
            enclosing_deadline: Instant,
        ) -> Result<(), String> {
            let original_deadline = self.held_deadline.ok_or_else(|| {
                "enclosing group admission has no original capture deadline".to_string()
            })?;
            super::super::enclosing_time(original_deadline)?;
            super::super::enclosing_time(enclosing_deadline)?;
            if self.child.enclosing_observed_reap.is_some()
                || original_deadline >= enclosing_deadline
                || self.refused
                || self.settled
                || self.enclosing_scope.is_some()
            {
                return Err("enclosing group admission is not initial or pre-admitted".to_string());
            }
            let leader = identity(self.child.id())?;
            require_owner(&self.leader, &leader)?;
            let parent = identity(std::process::id())?;
            if !self.parent.same_owner(&parent) || !parent.live() {
                return Err("enclosing group initial parent identity changed".to_string());
            }
            super::super::enclosing_time(original_deadline)?;
            super::super::enclosing_time(enclosing_deadline)?;
            // This scope is issued only while the actual initial T lease is
            // live, before header/input pumping. It never resets that lease.
            self.enclosing_scope = Some(EnclosingGroupScope {
                original_deadline,
                enclosing_deadline,
                leader: self.leader.clone(),
                parent: self.parent.clone(),
                attempted: false,
                pending_helper: None,
                empty_audit_before_reap: None,
            });
            Ok(())
        }

        #[cfg(all(test, feature = "lang-rust"))]
        fn record_enclosing_empty_audit(&mut self, result: &Result<Vec<u32>, String>) {
            if let Some(scope) = self.enclosing_scope.as_mut() {
                let now = Instant::now();
                scope.empty_audit_before_reap = (result.as_ref().is_ok_and(Vec::is_empty)
                    && self.child.enclosing_observed_reap.is_none()
                    && self.unconfirmed_signal.is_none()
                    && scope.pending_helper.is_none()
                    && self.held_deadline == Some(scope.original_deadline)
                    && scope.leader.same_owner(&self.leader)
                    && scope.parent.same_owner(&self.parent))
                .then_some(now);
            }
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn initial_enclosing_worker_for_test(
            &self,
        ) -> Result<ObservedProcessIdentity, String> {
            self.enclosing_scope
                .as_ref()
                .map(|scope| ObservedProcessIdentity(scope.leader.clone()))
                .ok_or_else(|| "actual initial U observation is absent".to_string())
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn observe_pending_enclosing_helper_for_test(
            &self,
            deadline: Instant,
        ) -> Result<(ObservedProcessIdentity, Option<Instant>), String> {
            super::super::enclosing_time(deadline)?;
            let scope = self
                .enclosing_scope
                .as_ref()
                .ok_or("actual initial U scope is absent")?;
            let helper = scope
                .pending_helper
                .as_ref()
                .ok_or("actual pending U helper is absent")?;
            let observed = identity(helper.id())?;
            super::super::enclosing_time(deadline)?;
            if observed.pid != helper.id()
                || observed.parent != scope.parent.pid
                || observed.start == 0
            {
                return Err("actual pending U helper identity changed".to_string());
            }
            Ok((ObservedProcessIdentity(observed), helper.bounded_drop_until))
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn remove_enclosing_empty_audit_for_test(&mut self) -> Result<(), String> {
            let scope = self
                .enclosing_scope
                .as_mut()
                .ok_or("missing actual initial U scope")?;
            if scope.empty_audit_before_reap.take().is_none() {
                return Err("negative control had no real empty audit to remove".to_string());
            }
            Ok(())
        }

        #[cfg(all(test, feature = "lang-rust"))]
        fn record_reaped_enclosing_disposition(
            &mut self,
            deadline: Instant,
            progress: &mut EnclosingDispositionProgress,
        ) -> Result<(), String> {
            super::super::enclosing_time(deadline)?;
            let (status, reaped_at) = self
                .child
                .enclosing_observed_reap
                .ok_or("enclosing native reap observation is absent")?;
            let scope = self.enclosing_scope.as_mut().ok_or(
                "enclosing actual primary was already reaped without initial U scope; custody retained"
            )?;
            let audited_at = scope.empty_audit_before_reap.ok_or(
                "enclosing reaped primary lacks actual before-reap empty audit; custody retained",
            )?;
            if scope.attempted
                || audited_at > reaped_at
                || reaped_at >= scope.enclosing_deadline
                || scope.enclosing_deadline < deadline
                || self.held_deadline != Some(scope.original_deadline)
                || self.unconfirmed_signal.is_some()
                || scope.pending_helper.is_some()
                || !scope.leader.same_owner(&self.leader)
                || !scope.parent.same_owner(&self.parent)
            {
                return Err(
                    "enclosing reaped-primary observations mismatched; custody retained"
                        .to_string(),
                );
            }
            scope.attempted = true;
            let parent = identity(std::process::id())?;
            super::super::enclosing_time(deadline)?;
            if !scope.parent.same_owner(&parent) || !parent.live() {
                return Err("enclosing reaped-primary parent changed; custody retained".to_string());
            }
            // No /proc leader read or numeric signal occurs after the pin was
            // actually reaped. These are the retained native audit/reap DATA.
            progress.leader = Some(scope.leader.clone());
            progress.parent = Some(parent);
            progress.primary_status = Some(status);
            progress.no_live_members = true;
            progress.empty_before_reap_at = Some(audited_at);
            super::super::enclosing_time(deadline)
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn has_enclosing_scope(&self) -> bool {
            self.enclosing_scope.is_some()
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn enclosing_failure_state(&self) -> (Option<Instant>, bool, bool, bool, bool) {
            (
                self.held_deadline,
                self.refused,
                self.settled,
                self.settlement.is_some(),
                self.settled_status.is_some(),
            )
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn observe_enclosing_worker(
            &self,
            deadline: Instant,
        ) -> Result<ObservedProcessIdentity, String> {
            observe_enclosing_worker(&self.child, Some(&self.leader), deadline)
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn dispose_enclosing_until(
            &mut self,
            deadline: Instant,
            progress: &mut EnclosingDispositionProgress,
        ) -> Result<(), String> {
            if self.child.enclosing_observed_reap.is_some() {
                return self.record_reaped_enclosing_disposition(deadline, progress);
            }
            // Dispose the old numeric-signal helper while the primary remains
            // unreaped. A pending helper must never outlive the group's pin.
            if let Some(helper) = self.unconfirmed_signal.as_mut() {
                helper.enclosing_dispose_helper_until(deadline, &mut progress.helper_status)?;
            }
            let mut primary_deadline = deadline;
            let mut signaled_group = false;
            if let Some(scope) = self.enclosing_scope.as_mut() {
                super::super::enclosing_time(deadline)?;
                if scope.attempted
                    || self.held_deadline != Some(scope.original_deadline)
                    || scope.enclosing_deadline < deadline
                    || scope.pending_helper.is_some()
                    || !scope.leader.same_owner(&self.leader)
                    || !scope.parent.same_owner(&self.parent)
                {
                    return Err("enclosing group scope changed or was already used".to_string());
                }
                scope.attempted = true;
                let parent = identity(std::process::id())?;
                if !scope.parent.same_owner(&parent) || !parent.live() {
                    return Err("enclosing group parent changed; custody retained".to_string());
                }
                // The once-created two-second signal/scan window tightens the
                // same terminal cutoff; it never resets a helper's clock.
                primary_deadline = progress
                    .group_deadline
                    .ok_or("enclosing group subcutoff was not pre-admitted")?;
                super::super::enclosing_time(primary_deadline)?;
                let observed = identity(self.child.id());
                let members = scan_group(scope.leader.group, primary_deadline);
                let members = signal_authority(&scope.leader, observed, members)?;
                super::super::enclosing_time(primary_deadline)?;
                if !members.is_empty() {
                    let result = bounded_group_signal_with_held(
                        scope.leader.group,
                        primary_deadline,
                        Some(scope.enclosing_deadline),
                    );
                    if let Err(failure) = result {
                        let (message, helper) = failure.into_parts();
                        // This inline slot existed before initial capture.
                        // No old/new unknown helper is replaced or discarded.
                        scope.pending_helper = helper;
                        return Err(message);
                    }
                }
                super::super::enclosing_time(primary_deadline)?;
                signaled_group = true;
            }
            // Preserve refused/settled/held_deadline and every original fact.
            // No old abort/qualified/settle or receipt constructor is used.
            let result = dispose_enclosing_primary(
                &mut self.child,
                Some((&self.leader, &self.parent)),
                primary_deadline,
                signaled_group,
                progress,
            );
            if actual_direct_closure(&self.child, Some((&self.leader, &self.parent)), progress) {
                // Record only genuine physical closure from this endpoint,
                // including native reap followed by a late clock refusal.
                self.enclosing_direct_reaped = true;
            }
            result?;
            super::super::enclosing_time(deadline)
        }

        #[cfg(all(test, feature = "lang-rust"))]
        pub(super) fn require_actual_enclosing_physical_closure(
            &self,
            progress: &EnclosingDispositionProgress,
        ) -> Result<(), &'static str> {
            let helpers_closed = self
                .unconfirmed_signal
                .as_ref()
                .is_none_or(|helper| helper.enclosing_observed_reap.is_some())
                && self.enclosing_scope.as_ref().is_none_or(|scope| {
                    scope
                        .pending_helper
                        .as_ref()
                        .is_none_or(|helper| helper.enclosing_observed_reap.is_some())
                });
            let direct_closed =
                actual_direct_closure(&self.child, Some((&self.leader, &self.parent)), progress);
            let original_audit_closed = self.enclosing_scope.as_ref().is_some_and(|scope| {
                let Some((_, reaped_at)) = self.child.enclosing_observed_reap else {
                    return false;
                };
                scope
                    .empty_audit_before_reap
                    .is_some_and(|audited_at| audited_at <= reaped_at)
                    && self.held_deadline == Some(scope.original_deadline)
                    && scope.leader.same_owner(&self.leader)
                    && scope.parent.same_owner(&self.parent)
            });
            if helpers_closed && (direct_closed || original_audit_closed) {
                Ok(())
            } else {
                Err("enclosing group still lacks genuine physical closure; actual custody retained")
            }
        }

        pub(crate) fn retained_process_count(&self) -> usize {
            let count = usize::from(!self.settled) + usize::from(self.unconfirmed_signal.is_some());
            #[cfg(all(test, feature = "lang-rust"))]
            let count = count
                + usize::from(
                    self.enclosing_scope
                        .as_ref()
                        .is_some_and(|scope| scope.pending_helper.is_some()),
                );
            count
        }

        #[cfg(test)]
        pub(crate) fn fixture_child(&mut self) -> &mut OwnedProcess {
            &mut self.child
        }

        #[cfg(test)]
        pub(crate) fn close_fixture_custody(&mut self) -> Result<(), String> {
            #[cfg(all(test, feature = "lang-rust"))]
            if let Some(scope) = self.enclosing_scope.as_mut()
                && let Some(helper) = scope.pending_helper.as_mut()
            {
                // Fixture-only teardown is never a negative/capture grant.
                // Close the actual numeric helper before releasing the pin.
                helper.request_kill().map_err(|error| error.to_string())?;
                if !helper.reap_within(Duration::from_secs(5)) {
                    return Err("fixture enclosing helper reap was unconfirmed".to_string());
                }
            }
            #[cfg(all(test, feature = "lang-rust"))]
            if let Some(helper) = self.unconfirmed_signal.as_mut() {
                helper.request_kill().map_err(|error| error.to_string())?;
                if !helper.reap_within(Duration::from_secs(5)) {
                    return Err("fixture original signal reap was unconfirmed".to_string());
                }
            }
            // Explicit test-only closeout is not settlement and grants no receipt.
            self.child
                .request_kill()
                .map_err(|error| error.to_string())?;
            if !self.child.reap_within(Duration::from_secs(5)) {
                return Err("fixture retained direct reap was unconfirmed".to_string());
            }
            if let Some(child) = self.unconfirmed_signal.as_mut() {
                child.request_kill().map_err(|error| error.to_string())?;
                if !child.reap_within(Duration::from_secs(5)) {
                    return Err("fixture retained signal reap was unconfirmed".to_string());
                }
            }
            Ok(())
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
                return Err(
                    "owned capture settlement phase expired; cleanup unconfirmed".to_string(),
                );
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
            self.held_deadline
                .map_or(deadline, |held| deadline.min(held))
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
            #[cfg(all(test, feature = "lang-rust"))]
            self.record_enclosing_empty_audit(&result);
            if result.is_err() {
                self.refused = true;
            }
            self.after_phase(result, deadline)
        }
        fn signal(&mut self, deadline: Instant) -> Result<(), String> {
            self.check_held()?;
            if self.unconfirmed_signal.is_some() {
                self.refused = true;
                return Err("owned capture signal helper custody remains unconfirmed".to_string());
            }
            if self.refused {
                return Err("owned capture group settlement previously refused".to_string());
            }
            let observed = self.qualified();
            let members = self.members(deadline);
            signal_authority(&self.leader, observed, members)?;
            self.qualified()?;
            let result = bounded_group_signal_with_held(
                self.leader.group,
                self.phase_deadline(deadline),
                self.held_deadline,
            );
            let result = result.map_err(|failure| {
                let (message, child) = failure.into_parts();
                self.unconfirmed_signal = child;
                message
            });
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
                return Err(
                    "owned capture group settlement refused; cleanup unconfirmed".to_string(),
                );
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
                let observed = self
                    .child
                    .try_wait()
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
            #[cfg(all(test, feature = "lang-rust"))]
            LAST_WAIT_WINDOW.with(|window| window.set(Some((started, timeout))));
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
                    let deadline = started
                        .checked_add(timeout)
                        .map_or(held, |phase| phase.min(held));
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
            #[cfg(all(test, feature = "lang-rust"))]
            if (self.enclosing_scope.is_some() || self.enclosing_direct_reaped)
                && self.child.enclosing_observed_reap.is_some()
            {
                // Only an admitted enclosing scope or its actual endpoint
                // avoids revisiting a numerically unpinned PID during Drop.
                // Ordinary capture without that endpoint keeps old behavior.
                return;
            }
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
    ) -> Result<(), GroupSetupFailure> {
        let deadline = held_deadline.map_or(deadline, |held| deadline.min(held));
        if held_deadline.is_some() {
            held_time(deadline).map_err(GroupSetupFailure::before_spawn)?;
        }
        if Instant::now() >= deadline {
            return Err(GroupSetupFailure::before_spawn(
                "owned group signal has no remaining settlement budget".to_string(),
            ));
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
            held_time(deadline).map_err(GroupSetupFailure::before_spawn)?;
        }
        let mut child = match held_deadline {
            Some(held) => OwnedProcess::spawn_with_bounded_drop_until(command, deadline.min(held))
                .map_err(|failure| {
                    let (error, child) = failure.into_parts();
                    GroupSetupFailure {
                        message: format!("owned group signal launch: {error}"),
                        child,
                    }
                })?,
            None => OwnedProcess::spawn_with_bounded_drop(command).map_err(|error| {
                GroupSetupFailure::before_spawn(format!("owned group signal launch: {error}"))
            })?,
        };
        let result = (|| {
            loop {
                if held_deadline.is_some() {
                    held_time(deadline)?;
                }
                let observed = child
                    .try_wait()
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
                        "owned group signal exceeded settlement bound; cleanup unconfirmed"
                            .to_string(),
                    );
                }
                let delay = if held_deadline.is_some() {
                    Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now()))
                } else {
                    Duration::from_millis(5)
                };
                thread::sleep(delay);
            }
        })();
        match result {
            Ok(()) => Ok(()),
            Err(message) => Err(GroupSetupFailure {
                message,
                child: if held_deadline.is_some() {
                    Some(child)
                } else {
                    None
                },
            }),
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
            let mut child = OwnedProcess::spawn_with_bounded_drop(test_command(
                "/usr/bin/sleep",
                &["30"],
                true,
            ))
            .map_err(|err| format!("opened stat owned child: {err}"))?;
            let pid = child.id();
            let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
            let before = loop {
                let observed = identity(pid)?;
                if observed.parent != std::process::id()
                    || observed.group != pid
                    || !observed.live()
                {
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
                return Err(
                    "opened stat identity changed before direct-owner termination".to_string(),
                );
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
                        return Err(
                            "opened stat owned child exited without termination".to_string()
                        );
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
                let text =
                    format!("9 (owned) {state} 1 {group} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17");
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
                    Ok(_) => {
                        return Err("scanned stat discarded an original I/O failure".to_string());
                    }
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
        fn scanned_stat_preserves_exact_byte_bound_and_utf8_error_precedence() -> Result<(), String>
        {
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
        fn invalid_live_lease_refuses_numeric_signal_and_bounded_drop_remains_separate()
        -> Result<(), String> {
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

        fn close_owned_spawn_failure(failure: super::super::super::OwnedSpawnFailure) -> String {
            let (error, child) = failure.into_parts();
            let message = error.to_string();
            if let Some(mut child) = child
                && let Err(cleanup) = close_fixture(&mut child)
            {
                return format!("{message}; fixture closeout: {cleanup}");
            }
            message
        }

        fn close_setup_failure(failure: GroupSetupFailure) -> String {
            let (message, child) = failure.into_parts();
            if let Some(mut child) = child
                && let Err(cleanup) = close_fixture(&mut child)
            {
                return format!("{message}; fixture closeout: {cleanup}");
            }
            message
        }

        #[test]
        fn strict_expired_clock_precedes_missing_worker_and_fixed_signal() -> Result<(), String> {
            let attempts = spawn_attempts();
            require_error(
                QualifiedGroupOwner::spawn_with_deadline(
                    test_command("/ripr-deliberately-missing-held-worker", &[], true),
                    Instant::now(),
                )
                .map_err(close_setup_failure),
                "held custody deadline",
            )?;
            assert_eq!(
                spawn_attempts(),
                attempts,
                "expired clock must not attempt a worker spawn"
            );

            let mut fixture = OwnedProcess::spawn_with_bounded_drop(test_command(
                "/usr/bin/sleep",
                &["30"],
                true,
            ))
            .map_err(|error| error.to_string())?;
            let pid = fixture.id();
            let attempts = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            let result = bounded_group_signal_with_held(
                pid,
                Instant::now() + Duration::from_secs(2),
                Some(Instant::now()),
            );
            let still_live = identity(pid)?.live();
            let final_attempts = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            close_fixture(&mut fixture)?;
            require_error(result.map_err(close_setup_failure), "held custody deadline")?;
            assert_eq!(
                final_attempts, attempts,
                "expired clock must not launch fixed kill"
            );
            if !still_live {
                return Err(
                    "expired clock dispatched a signal to its controlled fixture".to_string(),
                );
            }
            Ok(())
        }

        #[test]
        fn strict_monitor_expiry_poison_is_shared_with_abort_and_drop_owner() -> Result<(), String>
        {
            let held = Instant::now() + Duration::from_millis(500);
            let mut guard = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/usr/bin/sleep", &["30"], true),
                held,
            )
            .map_err(close_setup_failure)?;
            assert_eq!(guard.held_deadline, Some(held));
            assert_eq!(guard.child.bounded_drop_until, Some(held));
            let signals = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            let result = guard.wait_supervised(Instant::now(), Duration::from_secs(2), || {
                thread::sleep(
                    held.saturating_duration_since(Instant::now()) + Duration::from_millis(10),
                );
                Ok(())
            });
            let abort = guard.abort();
            let refused = guard.refused;
            let settlement = guard.take_settlement();
            let final_signals = SIGNAL_ATTEMPTS.with(std::cell::Cell::get);
            close_fixture(&mut guard.child)?;
            require_error(result, "held custody deadline")?;
            require_error(abort, "held custody deadline")?;
            assert_eq!(
                final_signals, signals,
                "expired abort must not reset the signal clock"
            );
            if !refused || settlement.is_some() || guard.settled_status().is_some() {
                return Err(
                    "late monitor manufactured settlement or lost sticky refusal".to_string(),
                );
            }
            Ok(())
        }

        #[test]
        fn strict_direct_drop_and_reap_cannot_restart_expired_grace() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let mut child = OwnedProcess::spawn_with_bounded_drop_until(
                test_command("/usr/bin/sleep", &["30"], true),
                held,
            )
            .map_err(close_owned_spawn_failure)?;
            assert_eq!(child.bounded_drop_until, Some(held));
            thread::sleep(
                held.saturating_duration_since(Instant::now()) + Duration::from_millis(10),
            );
            let started = Instant::now();
            let reaped = child.reap_until(held);
            child.drop_until(held);
            let elapsed = started.elapsed();
            let still_live = identity(child.id())?.live();
            close_fixture(&mut child)?;
            if reaped || !still_live || elapsed >= Duration::from_millis(500) {
                return Err(format!(
                    "expired direct fallback reset its grace: reaped={reaped}, live={still_live}, elapsed={elapsed:?}"
                ));
            }
            Ok(())
        }

        #[test]
        fn strict_actual_group_settlement_retains_original_clock_and_status() -> Result<(), String>
        {
            let held = Instant::now() + Duration::from_secs(2);
            let mut guard = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/bin/sh", &["-c", "exit 7"], true),
                held,
            )
            .map_err(close_setup_failure)?;
            let outcome = guard.wait(Instant::now(), Duration::from_secs(2))?;
            let (status, _, timed_out) = outcome.into_parts();
            if timed_out || status.code() != Some(7) {
                return Err("strict actual primary status changed".to_string());
            }
            let receipt = guard
                .take_settlement()
                .ok_or_else(|| "strict timely group lost its real settlement".to_string())?;
            assert_eq!(receipt.status(), status);
            assert_eq!(guard.held_deadline, Some(held));
            if guard.take_settlement().is_some() {
                return Err("strict group settlement was cloned".to_string());
            }
            Ok(())
        }

        #[test]
        fn post_spawn_clock_crossing_retains_actual_direct_child_without_fresh_grace()
        -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let spawned = super::super::super::with_post_spawn_deadline_barrier(|| {
                OwnedProcess::spawn_with_bounded_drop_until(
                    test_command("/usr/bin/sleep", &["30"], true),
                    held,
                )
            });
            let failure = match spawned {
                Err(failure) => failure,
                Ok(mut child) => {
                    close_fixture(&mut child)?;
                    return Err(
                        "post-spawn barrier did not refuse the actual late spawn".to_string()
                    );
                }
            };
            let (error, child) = failure.into_parts();
            let mut child = child.ok_or_else(|| {
                "post-spawn refusal discarded its actual direct-child handle".to_string()
            })?;
            let checks = (|| {
                if !error
                    .to_string()
                    .contains("spawn crossed its held deadline")
                    || Instant::now() < held
                    || child.bounded_drop_until != Some(held)
                {
                    return Err(
                        "post-spawn refusal changed its primary error or original clock"
                            .to_string(),
                    );
                }
                let before = identity(child.id())?;
                if before.pid != child.id()
                    || before.parent != std::process::id()
                    || before.group != child.id()
                    || before.start == 0
                    || !before.live()
                {
                    return Err(
                        "retained spawn has no actual live direct-child identity".to_string()
                    );
                }
                if child.reap_until(held) {
                    return Err("expired original ceiling certified direct reap".to_string());
                }
                let after = identity(child.id())?;
                if !before.same_owner(&after) || !after.live() {
                    return Err(
                        "late spawn custody changed before fixture-only cleanup".to_string()
                    );
                }
                Ok(())
            })();
            close_fixture(&mut child)?;
            checks?;
            require_not_live(child.id())?;
            Ok(())
        }

        #[test]
        fn group_setup_clock_crossing_retains_unqualified_direct_custody() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let spawned = super::super::super::with_post_spawn_deadline_barrier(|| {
                QualifiedGroupOwner::spawn_with_deadline(
                    test_command("/usr/bin/sleep", &["30"], true),
                    held,
                )
            });
            let failure = match spawned {
                Err(failure) => failure,
                Ok(mut owner) => {
                    owner.close_fixture_custody()?;
                    return Err("group setup accepted a post-spawn clock crossing".to_string());
                }
            };
            let (message, child) = failure.into_parts();
            let mut child = child.ok_or_else(|| {
                "failed group setup discarded actual unqualified child custody".to_string()
            })?;
            let checks = (|| {
                let before = identity(child.id())?;
                if !message.contains("spawn crossed its held deadline")
                    || child.bounded_drop_until != Some(held)
                    || !before.live()
                    || before.parent != std::process::id()
                    || before.group != child.id()
                    || before.start == 0
                {
                    return Err(
                        "group setup error lost the real child, clock or primary failure"
                            .to_string(),
                    );
                }
                let after = identity(child.id())?;
                require_owner(&before, &after)?;
                Ok(())
            })();
            close_fixture(&mut child)?;
            checks?;
            require_not_live(child.id())?;
            Ok(())
        }

        #[test]
        fn fixed_signal_clock_crossing_retains_helper_and_primary_custody() -> Result<(), String> {
            let held = Instant::now() + Duration::from_millis(500);
            let mut owner = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/usr/bin/sleep", &["30"], true),
                held,
            )
            .map_err(close_setup_failure)?;
            let primary = owner.id();
            let failed = super::super::super::with_post_spawn_deadline_barrier(|| owner.abort());
            let checks = (|| {
                require_error(failed, "spawn crossed its held deadline")?;
                let helper = owner.unconfirmed_signal.as_ref().ok_or_else(|| {
                    "late fixed signal discarded its actual helper handle".to_string()
                })?;
                let observed = identity(helper.id())?;
                if owner.id() != primary
                    || owner.held_deadline != Some(held)
                    || helper.bounded_drop_until != Some(held)
                    || observed.pid != helper.id()
                    || observed.parent != std::process::id()
                    || observed.start == 0
                    || owner.retained_process_count() != 2
                    || owner.take_settlement().is_some()
                    || owner.settled_status().is_some()
                {
                    return Err(
                        "fixed signal expiry fabricated settlement or lost actual custody"
                            .to_string(),
                    );
                }
                require_error(owner.abort(), "held custody deadline")?;
                Ok(())
            })();
            owner.close_fixture_custody()?;
            checks?;
            require_not_live(primary)?;
            Ok(())
        }

        #[cfg(feature = "lang-rust")]
        #[test]
        fn enclosing_same_cutoff_disposes_actual_primary_and_retained_signal_helper()
        -> Result<(), String> {
            let started = Instant::now();
            let held = started + Duration::from_millis(100);
            let outer = started + Duration::from_secs(5);
            let mut owner = QualifiedGroupOwner::spawn_with_deadline(
                test_command("/usr/bin/sleep", &["30"], true),
                held,
            )
            .map_err(close_setup_failure)?;
            let failed = super::super::super::with_post_spawn_deadline_barrier(|| owner.abort());
            let primary = owner.id();
            let before = owner.leader.clone();
            let helper = owner
                .unconfirmed_signal
                .as_ref()
                .ok_or("real failed signal did not retain its helper")?
                .id();
            let mut progress = EnclosingDispositionProgress::new();
            // This endpoint was admitted before either child could spawn.
            owner.dispose_enclosing_until(outer, &mut progress)?;
            require_error(failed, "spawn crossed its held deadline")?;
            if progress.primary_status.is_none()
                || progress.helper_status.is_none()
                || !progress.no_live_members
                || progress
                    .leader
                    .as_ref()
                    .is_none_or(|actual| !before.same_owner(actual))
                || owner.held_deadline != Some(held)
                || !owner.refused
                || owner.settled
                || owner.settlement.is_some()
                || owner.settled_status.is_some()
                || owner.child.bounded_drop_until != Some(held)
                || Instant::now() >= outer
            {
                return Err(
                    "enclosing helper disposition changed capture or lost actual progress"
                        .to_string(),
                );
            }
            require_not_live(primary)?;
            require_not_live(helper)
        }

        #[test]
        fn reaped_primary_cannot_supply_a_group_settlement() -> Result<(), String> {
            let mut guard =
                QualifiedGroupOwner::spawn(test_command("sh", &["-c", "exit 0"], true))?;
            let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
            loop {
                if guard
                    .child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_some()
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(
                        "premature reap control did not observe actual primary exit".to_string()
                    );
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
pub(crate) use linux::GroupSetupFailure;
#[cfg(target_os = "linux")]
pub use linux::{
    ObservedProcessIdentity, QualifiedGroupOwner, QualifiedGroupSettlement, QualifiedGroupWait,
};

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
use linux::EnclosingDispositionProgress;
