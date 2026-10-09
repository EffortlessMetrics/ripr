//! Cooperative Linux ownership for the source-owned file-policy checker and
//! trusted repository build preparation.
//! Diagnostic process records never authorize termination. The actual unreaped
//! OwnedProcess leader leases the numeric group until settlement and reaping.
use super::*;

pub(super) const ENV: &str = "RIPR_XTASK_OWNED_FILE_POLICY_CAPTURE";
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
    let text = std::str::from_utf8(&bytes).map_err(|_| {
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

fn parse_handoff(value: &str) -> Result<(u32, u64, Duration), String> {
    if value.len() > 128 {
        return Err("owned file-policy handoff exceeds its bound".to_string());
    }
    let values: Vec<_> = value.split(':').collect();
    if values.len() != 4 || values[0] != "1" {
        return Err("owned file-policy handoff malformed".to_string());
    }
    let pid = values[1]
        .parse::<u32>()
        .map_err(|err| format!("owned file-policy parent PID: {err}"))?;
    let start = values[2]
        .parse::<u64>()
        .map_err(|err| format!("owned file-policy parent start: {err}"))?;
    let outer = Duration::from_millis(
        values[3]
            .parse::<u64>()
            .map_err(|err| format!("owned file-policy deadline: {err}"))?,
    );
    if pid == 0 || start == 0 || outer.is_zero() {
        return Err("owned file-policy handoff has no live identity or deadline".to_string());
    }
    Ok((pid, start, outer))
}

pub(super) struct ParentHandoff {
    text: String,
}
impl ParentHandoff {
    pub(super) fn new(timeout: Duration) -> Result<Self, String> {
        let parent = identity(std::process::id())?;
        if !parent.live() || timeout.is_zero() || u64::try_from(timeout.as_millis()).is_err() {
            return Err("owned file-policy parent or deadline unavailable".to_string());
        }
        Ok(Self {
            text: format!("1:{}:{}:{}", parent.pid, parent.start, timeout.as_millis()),
        })
    }
    pub(super) fn value(&self) -> &str {
        &self.text
    }
}
#[derive(Clone)]
pub(crate) struct FilePolicyScope {
    checker: Identity,
    parent: Identity,
    outer: Duration,
}
impl FilePolicyScope {
    pub(super) fn requested() -> Result<Option<Self>, String> {
        let Some(value) = std::env::var_os(ENV) else {
            return Ok(None);
        };
        let value = value
            .to_str()
            .ok_or_else(|| "owned file-policy handoff is not UTF-8".to_string())?;
        if std::env::args().nth(1).as_deref() != Some("check-file-policy")
            || std::env::var("RIPR_SOURCE_PROMOTION_VALIDATION").as_deref() != Ok("1")
            || !std::env::var("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA").is_ok_and(|value| {
                value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err("owned file-policy handoff outside the selected checker route".to_string());
        }
        Self::resolve(value).map(Some)
    }
    fn resolve(value: &str) -> Result<Self, String> {
        let (pid, start, outer) = parse_handoff(value)?;
        let checker = identity(std::process::id())?;
        let parent = identity(pid)?;
        let scope = Self {
            checker,
            parent,
            outer,
        };
        if scope.parent.start != start {
            return Err("owned file-policy parent start mismatch".to_string());
        }
        scope.verify(outer)?;
        Ok(scope)
    }
    pub(super) fn verify_spawn(&self, pid: u32) -> Result<(), String> {
        self.verify(self.outer)?;
        let child = identity(pid)?;
        if child.parent != self.checker.pid || child.group != self.checker.group {
            return Err(
                "owned file-policy spawned child has foreign lineage; no group signal permitted"
                    .to_string(),
            );
        }
        Ok(())
    }
    pub(super) fn verify(&self, timeout: Duration) -> Result<(), String> {
        self.verify_observed(
            &identity(std::process::id())?,
            &identity(self.parent.pid)?,
            timeout,
        )
    }
    fn verify_observed(
        &self,
        checker: &Identity,
        parent: &Identity,
        timeout: Duration,
    ) -> Result<(), String> {
        if self.outer.is_zero() || timeout < self.outer {
            return Err("owned file-policy inner deadline is shorter than delegated outer bound; spawn refused".to_string());
        }
        if self.checker.pid != std::process::id()
            || !self.checker.same_owner(checker)
            || checker.pid != checker.group
            || !checker.live()
            || checker.parent != parent.pid
            || !self.parent.same_owner(parent)
            || !parent.live()
        {
            return Err("owned file-policy lineage mismatch; spawn refused".to_string());
        }
        Ok(())
    }
}

pub(super) struct OwnedCaptureGuard {
    child: OwnedProcess,
    leader: Identity,
    settled: bool,
    refused: bool,
}
impl OwnedCaptureGuard {
    pub(super) fn new(child: OwnedProcess) -> Result<Self, String> {
        let leader = identity(child.id())?;
        if leader.pid != leader.group || leader.parent != std::process::id() {
            return Err("owned capture did not create its own direct-child group".to_string());
        }
        Ok(Self {
            child,
            leader,
            settled: false,
            refused: false,
        })
    }
    pub(super) fn child(&mut self) -> &mut OwnedProcess {
        &mut self.child
    }
    fn qualified(&mut self) -> Result<Identity, String> {
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
        result
    }
    fn members(&mut self, deadline: Instant) -> Result<Vec<u32>, String> {
        let result = scan_group(self.leader.group, deadline);
        if result.is_err() {
            self.refused = true;
        }
        result
    }
    fn signal(&mut self, deadline: Instant) -> Result<(), String> {
        if self.refused {
            return Err("owned capture group settlement previously refused".to_string());
        }
        let observed = self.qualified();
        let members = self.members(deadline);
        signal_authority(&self.leader, observed, members)?;
        self.qualified()?;
        bounded_group_signal(self.leader.group, deadline)
    }
    fn settle(&mut self, terminate: bool) -> Result<(ExitStatus, bool), String> {
        let result = self.settle_qualified(terminate);
        if result.is_err() && !self.settled {
            self.refused = true;
        }
        result
    }
    fn settle_qualified(&mut self, terminate: bool) -> Result<(ExitStatus, bool), String> {
        if self.refused {
            return Err("owned capture group settlement refused; cleanup unconfirmed".to_string());
        }
        let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
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
            thread::sleep(Duration::from_millis(10));
        }
        // The numeric group lease remains pinned by this actual unreaped child
        // until all live members are gone. Only now may try_wait reap it.
        let status = loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|err| format!("owned capture bounded reap: {err}"))?
            {
                break status;
            }
            if Instant::now() >= deadline {
                return Err("owned capture primary reap exceeded settlement bound".to_string());
            }
            thread::sleep(Duration::from_millis(10));
        };
        self.settled = true;
        if unexpected_survivor {
            return Err("owned capture primary exited with live group members; cleanup failure despite settlement".to_string());
        }
        Ok((status, signaled))
    }
    pub(super) fn wait(
        &mut self,
        started: Instant,
        timeout: Duration,
    ) -> Result<WaitOutcome, String> {
        self.wait_supervised(started, timeout, || Ok(()))
    }

    pub(super) fn wait_supervised(
        &mut self,
        started: Instant,
        timeout: Duration,
        mut monitor: impl FnMut() -> Result<(), String>,
    ) -> Result<WaitOutcome, String> {
        loop {
            monitor()?;
            let leader = self.qualified()?;
            if !leader.live() || started.elapsed() >= timeout {
                let expired = leader.live() && started.elapsed() >= timeout;
                let (status, signaled) = self.settle(expired)?;
                return Ok(WaitOutcome {
                    status,
                    duration: started.elapsed(),
                    timed_out: expired && signaled,
                    peak_rss_bytes: None,
                });
            }
            thread::sleep(poll_interval(false));
        }
    }
    pub(super) fn abort(&mut self) -> Result<(), String> {
        if self.settled {
            return Ok(());
        }
        self.settle(true).map(|_| ())
    }
}
impl Drop for OwnedCaptureGuard {
    fn drop(&mut self) {
        if !self.settled && !self.refused {
            let _ = self.abort();
        }
        // Unconfirmed scope is never retried as a numeric group kill. The
        // underlying OwnedProcess performs direct-handle bounded fallback only.
    }
}

fn bounded_group_signal(group: u32, deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        return Err("owned group signal has no remaining settlement budget".to_string());
    }
    let mut command = Command::new("/usr/bin/kill");
    command
        .args(["-KILL", "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedProcess::spawn_with_bounded_drop(command)
        .map_err(|err| format!("owned group signal launch: {err}"))?;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("owned group signal poll: {err}"))?
        {
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
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    const WORKER: &str = "run::owned_capture_group::tests::owned_file_policy_group_worker";
    const CARGO_SHIM: &str = r#"#!/bin/sh
if [ "$#" -ne 2 ] || [ "$1" != "test" ] || [ "$2" != "owned-proof" ]; then
    exit 71
fi
/usr/bin/sleep 10 &
descendant=$!
printf '%s %s' "$$" "$descendant" > "$RIPR_OWNED_GROUP_MEMBER_MARKER"
wait
"#;
    struct Fixture {
        path: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Result<Self, String> {
            let path = std::env::temp_dir().join(format!(
                "ripr-owned-group-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).map_err(|err| format!("claim owned group fixture: {err}"))?;
            Ok(Self { path })
        }
        fn marker(&self) -> std::path::PathBuf {
            self.path.join("members")
        }
        fn cargo_environment(&self) -> Result<String, String> {
            use std::os::unix::fs::PermissionsExt;
            let cargo = self.path.join("cargo");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&cargo)
                .map_err(|err| format!("claim owned cargo shim: {err}"))?;
            file.write_all(CARGO_SHIM.as_bytes())
                .map_err(|err| format!("write owned cargo shim: {err}"))?;
            file.set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(|err| format!("owned cargo shim mode: {err}"))?;
            let mut paths = vec![self.path.clone()];
            if let Some(original) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&original));
            }
            std::env::join_paths(paths)
                .map_err(|err| format!("owned cargo child PATH: {err}"))?
                .into_string()
                .map_err(|err| format!("owned cargo child PATH is not UTF-8: {err:?}"))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
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
            (vec![0xff], 3),
        ] {
            let failure = ReadFailure {
                prefix: std::io::Cursor::new(prefix),
                code,
            };
            require_error(
                read_scanned_identity(failure, 9),
                "owned capture /proc/9/stat",
            )?;
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

    fn members(marker: &Path) -> Result<(u32, u32), String> {
        let text =
            fs::read_to_string(marker).map_err(|err| format!("owned member marker: {err}"))?;
        let fields: Vec<_> = text.split_whitespace().collect();
        if fields.len() != 2 {
            return Err(format!("owned member marker incomplete: {text:?}"));
        }
        Ok((
            fields[0]
                .parse()
                .map_err(|err| format!("owned child PID: {err}"))?,
            fields[1]
                .parse()
                .map_err(|err| format!("owned grandchild PID: {err}"))?,
        ))
    }
    fn wait_for_marker(marker: &Path) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if members(marker).is_ok() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("owned worker did not publish its actual spawned members".to_string());
            }
            thread::sleep(Duration::from_millis(5));
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
    fn worker_args() -> Vec<String> {
        [
            "check-file-policy",
            "--ignored",
            "--exact",
            WORKER,
            "--nocapture",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect()
    }
    struct WorkerInvocation {
        args: Vec<String>,
        envs: Vec<(&'static str, String)>,
    }
    fn worker_invocation(fixture: &Fixture, mode: &str) -> Result<WorkerInvocation, String> {
        let mut args = worker_args();
        if mode == "invalid-argv" {
            args[0] = "check-command-catalog".to_string();
        }
        // This is the reviewed input marker for the route's format gate, not a
        // binary-source attestation or an authority to signal any process.
        let source = if mode == "invalid-source" {
            "invalid"
        } else {
            "eec85c62d2e041ffe0d245f14c354a89f9cc80d0"
        };
        Ok(WorkerInvocation {
            args,
            envs: vec![
                ("RIPR_OWNED_GROUP_WORKER_MODE", mode.to_string()),
                (
                    "RIPR_OWNED_GROUP_MEMBER_MARKER",
                    fixture.marker().to_string_lossy().into_owned(),
                ),
                (
                    "RIPR_SOURCE_PROMOTION_VALIDATION",
                    if mode == "invalid-validation" {
                        "0"
                    } else {
                        "1"
                    }
                    .to_string(),
                ),
                (
                    "RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA",
                    source.to_string(),
                ),
                ("PATH", fixture.cargo_environment()?),
            ],
        })
    }
    fn worker(
        fixture: &Fixture,
        mode: &str,
        spawned: impl FnMut(u32),
    ) -> Result<TimedBoundedOutput, String> {
        let executable =
            std::env::current_exe().map_err(|err| format!("test executable: {err}"))?;
        let invocation = worker_invocation(fixture, mode)?;
        let envs: Vec<_> = invocation
            .envs
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        capture_owned_file_policy_group(
            (&executable, &invocation.args, &fixture.path),
            &envs,
            (Duration::from_secs(3), 8192),
            "synthetic owned file-policy group",
            spawned,
        )
    }
    #[test]
    fn owned_file_policy_scope_refuses_foreign_shorter_and_incomplete_authority()
    -> Result<(), String> {
        let checker = Identity {
            pid: std::process::id(),
            parent: 91,
            group: std::process::id(),
            start: 17,
            state: 'R',
        };
        let parent = Identity {
            pid: 91,
            parent: 72,
            group: 72,
            start: 11,
            state: 'S',
        };
        let scope = FilePolicyScope {
            checker: checker.clone(),
            parent: parent.clone(),
            outer: Duration::from_mins(3),
        };
        scope.verify_observed(&checker, &parent, Duration::from_mins(5))?;
        scope.verify_observed(&checker, &parent, Duration::from_mins(30))?;
        require_error(
            scope.verify_observed(&checker, &parent, Duration::from_secs(179)),
            "shorter",
        )?;
        for changed in [
            Identity {
                start: 18,
                ..checker.clone()
            },
            Identity {
                group: checker.group + 1,
                ..checker.clone()
            },
            Identity {
                parent: 92,
                ..checker.clone()
            },
            Identity {
                state: 'Z',
                ..checker.clone()
            },
        ] {
            require_error(
                scope.verify_observed(&changed, &parent, Duration::from_mins(5)),
                "lineage",
            )?;
        }
        require_error(
            scope.verify_observed(
                &checker,
                &Identity {
                    start: 12,
                    ..parent.clone()
                },
                Duration::from_mins(5),
            ),
            "lineage",
        )?;
        let mut signals = 0;
        for observed in [
            Err("missing leader stat".to_string()),
            Ok(Identity {
                start: 18,
                ..checker.clone()
            }),
            Ok(Identity {
                group: 12,
                ..checker.clone()
            }),
        ] {
            let decision = signal_authority(&checker, observed, Ok(vec![checker.pid]));
            if decision.is_ok() {
                signals += 1;
            }
            match decision {
                Err(_) => {}
                Ok(_) => return Err("unqualified signal gate admitted a dispatcher".to_string()),
            }
        }
        let incomplete = signal_authority(
            &checker,
            Ok(checker.clone()),
            Err("incomplete scan".to_string()),
        );
        if incomplete.is_ok() {
            signals += 1;
        }
        require_error(incomplete, "incomplete")?;
        assert_eq!(
            signals, 0,
            "unqualified identities/scans must never dispatch a group signal"
        );
        require_error(parse_identity("9 (bad) S 1 9", 9), "identity")?;
        require_error(
            parse_identity("9 (owned) Q 1 9 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17", 9),
            "state unsupported",
        )?;
        for bad in [
            "",
            "1:bad:1:300",
            "1:1:bad:300",
            "1:1:1:bad",
            "1:1:1:300:extra",
        ] {
            require_error(parse_handoff(bad), "owned file-policy")?;
        }
        // A complete scan and genuine identity reach the exact same gate.
        assert_eq!(
            signal_authority(&checker, Ok(checker.clone()), Ok(vec![checker.pid]))?,
            vec![checker.pid]
        );
        // The real child argv/env selection gate refuses before literal Cargo.
        for mode in ["invalid-argv", "invalid-validation", "invalid-source"] {
            let fixture = Fixture::new()?;
            let output = worker(&fixture, mode, |_| {})?;
            if !output.status.is_some_and(|status| status.success())
                || output.timed_out
                || output.stdout_truncated
                || output.stderr_truncated
                || !output.stdout.contains(WORKER)
                || !output.stdout.contains("1 passed")
                || fixture.marker().exists()
            {
                return Err(format!(
                    "actual requested() refusal control was not executed: {mode}"
                ));
            }
        }
        Ok(())
    }
    #[test]
    fn owned_file_policy_timeout_settles_inherited_descendants_and_preserves_foreign_group()
    -> Result<(), String> {
        let fixture = Fixture::new()?;
        // This is an actual spawn-owned separate PGID, the old isolation shape.
        // It is deliberately outside the checker lease and must survive its kill.
        let mut separate =
            OwnedProcess::spawn_with_bounded_drop(test_command("sleep", &["30"], true))
                .map_err(|err| format!("separate owned group: {err}"))?;
        let separate_identity = identity(separate.id())?;
        let mut checker_pid = None;
        let output = worker(&fixture, "timeout", |pid| checker_pid = Some(pid))?;
        if !output.timed_out {
            return Err("shared-group worker must enforce its outer deadline".to_string());
        }
        let checker = checker_pid.ok_or_else(|| "actual checker PID unavailable".to_string())?;
        let (child, grandchild) = members(&fixture.marker())?;
        require_not_live(checker)?;
        require_not_live(child)?;
        require_not_live(grandchild)?;
        let still_separate = identity(separate.id())?;
        if !separate_identity.same_owner(&still_separate) || !still_separate.live() {
            return Err(
                "outer cleanup terminated or changed the unrelated separate group".to_string(),
            );
        }
        separate
            .kill()
            .map_err(|err| format!("settle independently owned sentinel: {err}"))?;
        let deadline = Instant::now() + POST_KILL_GROUP_CONFIRM_GRACE;
        loop {
            if separate
                .try_wait()
                .map_err(|err| format!("reap sentinel: {err}"))?
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err("independently owned sentinel reap unconfirmed".to_string());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    #[test]
    fn owned_file_policy_completion_and_capture_error_refuse_cleanup_credit() -> Result<(), String>
    {
        for mode in ["normal", "nonzero"] {
            let fixture = Fixture::new()?;
            require_error(
                worker(&fixture, mode, |_| {}),
                "exited with live group members",
            )?;
            let (child, grandchild) = members(&fixture.marker())?;
            require_not_live(child)?;
            require_not_live(grandchild)?;
        }
        // Completed normal/nonzero captures with no survivors preserve status.
        for (code, expected_success) in [("0", true), ("7", false)] {
            let fixture = Fixture::new()?;
            let args = vec![
                "-c".to_string(),
                format!("printf exact-owned-output; exit {code}"),
            ];
            let output = capture_owned_file_policy_group(
                (Path::new("sh"), &args, &fixture.path),
                &[],
                (Duration::from_secs(3), 8192),
                "owned clean completion",
                |_| {},
            )?;
            if output.timed_out
                || output.status.is_some_and(|status| status.success()) != expected_success
                || output.stdout != "exact-owned-output"
                || output.stdout_truncated
                || output.stderr_truncated
            {
                return Err(
                    "owned completed capture changed original status or byte contract".to_string(),
                );
            }
        }
        // An actual fallible capture callback must settle before releasing its owner.
        let fixture = Fixture::new()?;
        let executable =
            std::env::current_exe().map_err(|err| format!("test executable: {err}"))?;
        let invocation = worker_invocation(&fixture, "timeout")?;
        let envs: Vec<_> = invocation
            .envs
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        let mut checker_pid = None;
        let result = capture_owned_file_policy_group_checked(
            (&executable, &invocation.args, &fixture.path),
            &envs,
            (Duration::from_secs(3), 8192),
            "synthetic capture error",
            |pid| {
                checker_pid = Some(pid);
                wait_for_marker(&fixture.marker())?;
                Err("synthetic owned capture error".to_string())
            },
        );
        require_error(result, "synthetic owned capture error")?;
        require_not_live(
            checker_pid.ok_or_else(|| "capture error had no actual child".to_string())?,
        )?;
        let (child, grandchild) = members(&fixture.marker())?;
        require_not_live(child)?;
        require_not_live(grandchild)?;
        // Exercise the actual scoped drain owner, not a copied result predicate.
        // Timeout cannot produce TimedOutput and therefore cannot earn warmup credit.
        let (sender, receiver) = mpsc::channel::<Result<String, String>>();
        require_error(
            drain_owned_stream_reader(
                receiver,
                thread::spawn(|| {}),
                "stderr",
                "strict timeout control",
            ),
            "drain incomplete",
        )?;
        drop(sender);
        let (sender, receiver) = mpsc::channel::<Result<String, String>>();
        drop(sender);
        require_error(
            drain_owned_stream_reader(
                receiver,
                thread::spawn(|| {}),
                "stderr",
                "strict disconnected control",
            ),
            "reader disconnected",
        )?;
        // The original ordinary helper still returns its diagnostic success note.
        let (sender, receiver) = mpsc::channel::<Result<String, String>>();
        let note = drain_stream_reader_bounded(
            receiver,
            thread::spawn(|| {}),
            Duration::ZERO,
            "stderr",
            "ordinary grace parity",
        )?;
        drop(sender);
        if !note.contains("drain exceeded post-kill grace") {
            return Err("ordinary capture drain semantics changed".to_string());
        }
        // Real requested shorter bound refuses before the missing program can spawn.
        let fixture = Fixture::new()?;
        let output = worker(&fixture, "shorter", |_| {})?;
        if !output.status.is_some_and(|status| status.success())
            || fixture.marker().exists()
            || output.timed_out
        {
            return Err("shorter inner deadline did not refuse before spawn".to_string());
        }
        Ok(())
    }

    #[test]
    fn materialized_build_preparation_supervision_settles_actual_cooperative_descendants()
    -> Result<(), String> {
        for mode in ["storage-stop", "overflow", "timeout"] {
            let fixture = Fixture::new()?;
            let marker = fixture.marker().to_string_lossy().into_owned();
            let args = vec![
                "-c".to_string(),
                "sleep 30 & first=$!; sleep 30 & second=$!; printf '%s %s' \"$first\" \"$second\" > \"$RIPR_PREPARATION_MEMBER_MARKER\"; printf bounded-preparation-output; wait".to_string(),
            ];
            let mut leader = None;
            let capture = capture_owned_group_supervised(
                (Path::new("sh"), &args, &fixture.path),
                &[("RIPR_PREPARATION_MEMBER_MARKER", &marker)],
                (
                    if mode == "timeout" {
                        Duration::from_millis(100)
                    } else {
                        Duration::from_secs(3)
                    },
                    if mode == "overflow" { 8 } else { 8192 },
                ),
                "actual cooperative preparation supervision control",
                |pid| {
                    leader = Some(pid);
                    wait_for_marker(&fixture.marker())
                },
                false,
                || {
                    if mode == "storage-stop" {
                        Err("synthetic observed storage bound".into())
                    } else {
                        Ok(())
                    }
                },
            )?;
            if mode == "timeout" {
                assert!(capture.output.timed_out);
            } else {
                assert!(capture.failure_reason.is_some());
            }
            if mode == "overflow" {
                assert!(capture.output.stdout_truncated);
                assert!(capture.output.stdout.len() <= 8);
            }
            require_not_live(leader.ok_or("preparation control leader unavailable")?)?;
            let (child, grandchild) = members(&fixture.marker())?;
            require_not_live(child)?;
            require_not_live(grandchild)?;
        }
        Ok(())
    }
    #[test]
    fn owned_file_policy_bounded_drop_is_explicit_and_finite() -> Result<(), String> {
        for bounded in [false, true] {
            let command = test_command("sleep", &["30"], true);
            let owner = if bounded {
                OwnedProcess::spawn_with_bounded_drop(command)
            } else {
                OwnedProcess::spawn(command)
            }
            .map_err(|err| format!("drop parity owner: {err}"))?;
            let pid = owner.id();
            let started = Instant::now();
            drop(owner);
            if started.elapsed() > Duration::from_secs(6) {
                return Err(
                    "live direct-child ownership release exceeded existing fallback budget"
                        .to_string(),
                );
            }
            require_not_live(pid)?;
        }
        // An altered lease makes the actual live guard refuse without group kill.
        let child = OwnedProcess::spawn_with_bounded_drop(test_command("sleep", &["30"], true))
            .map_err(|err| format!("fault-injected lease owner: {err}"))?;
        let mut guard = OwnedCaptureGuard::new(child)?;
        let pid = guard.child().id();
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

    #[test]
    #[ignore = "owned child-only helper; never a bootstrap acceptance witness"]
    fn owned_file_policy_group_worker() -> Result<(), String> {
        let mode = std::env::var("RIPR_OWNED_GROUP_WORKER_MODE")
            .map_err(|err| format!("owned worker mode missing: {err}"))?;
        if matches!(
            mode.as_str(),
            "invalid-argv" | "invalid-validation" | "invalid-source"
        ) {
            require_error(
                FilePolicyScope::requested(),
                "outside the selected checker route",
            )?;
            return Ok(());
        }
        let scope = FilePolicyScope::requested()?.ok_or_else(|| {
            "actual file-policy requested() did not select owned scope".to_string()
        })?;
        let marker = std::env::var("RIPR_OWNED_GROUP_MEMBER_MARKER")
            .map_err(|err| format!("owned member marker missing: {err}"))?;
        if mode == "shorter" {
            let args = vec!["test".to_string(), "owned-proof".to_string()];
            require_error(
                capture_file_policy_cargo(
                    &args,
                    Duration::from_millis(1),
                    "shorter literal Cargo scoped refusal",
                    &scope,
                    None,
                ),
                "shorter",
            )?;
            return Ok(());
        }
        let args = ["-c".to_string(),
            r#"sleep 30 & descendant=$!; printf '%s %s' "$$" "$descendant" > "$RIPR_OWNED_GROUP_MEMBER_MARKER"; wait"#.to_string()];
        if mode == "timeout" {
            let args = vec!["test".to_string(), "owned-proof".to_string()];
            let _output = capture_file_policy_cargo(
                &args,
                Duration::from_mins(5),
                "synthetic literal Cargo owned route",
                &scope,
                None,
            )?;
            return Err("owned worker continued beyond its parent's enforced deadline".to_string());
        }
        if mode != "normal" && mode != "nonzero" {
            return Err("unknown owned worker mode".to_string());
        }
        scope.verify(Duration::from_mins(5))?;
        let string_refs: Vec<_> = args.iter().map(String::as_str).collect();
        let mut command = test_command("sh", &string_refs, false);
        // Preserve held pipes so successful root exit is not a drain certificate.
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let _child = OwnedProcess::spawn_with_bounded_drop(command)
            .map_err(|err| format!("owned survivor child: {err}"))?;
        wait_for_marker(Path::new(&marker))?;
        // Deliberate controlled primary exit bypasses the child's destructor;
        // the actual outer group lease must settle both spawned descendants.
        std::process::exit(if mode == "normal" { 0 } else { 7 });
    }
}
