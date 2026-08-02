//! Shared git invocation helper.
//!
//! All git subprocess spawns in the published crate should delegate to
//! [`run_git`] / [`run_git_output_with_deadline`] so error formatting stays
//! unified and the process-policy allowlist has a single canonical entry
//! point.
//!
//! #2303: every entry point accepts an optional cooperative deadline. When a
//! deadline is set, the child is polled on a short interval; a git invocation
//! that exceeds the deadline is terminated and reaped, and the caller gets a
//! named, matchable error with the [`GIT_INVOCATION_TIMEOUT_PREFIX`] prefix.
//! The poll loop also checks cooperative analysis cancellation each tick, so
//! a hung git invocation honors an LSP refresh supersede instead of pinning
//! the refresh worker. `None` keeps the invocation unbounded (the CLI
//! behavior — byte-identical to the pre-#2303 path).

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Grace period for draining stdout/stderr after a timed process-tree kill.
///
/// A descendant can briefly retain an inherited pipe handle after the parent
/// is terminated. The bounded drain keeps a Windows timeout from blocking the
/// LSP worker indefinitely while still allowing normal output to finish.
const POST_KILL_DRAIN_GRACE: Duration = Duration::from_secs(5);

/// Named, matchable prefix for git invocation timeout errors (#2303). The
/// LSP refresh path matches this prefix to convert a diff-load timeout into
/// a committed limited snapshot instead of a dropped refresh.
pub(crate) const GIT_INVOCATION_TIMEOUT_PREFIX: &str = "git_invocation_timeout";

/// True when `error` is the named git invocation timeout error (#2303).
/// Matchable in the style of `analysis::cancellation::is_cancellation_error`.
pub(crate) fn is_git_invocation_timeout(error: &str) -> bool {
    error.starts_with(GIT_INVOCATION_TIMEOUT_PREFIX)
}

/// Poll interval for the deadline/cancellation wait loop.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run `git -C <root> <args...>` with no deadline and return trimmed stdout
/// on success.
///
/// Returns a unified error on failure:
/// ```text
/// git -C <root> <args...> failed
/// stdout: <first 500 chars>
/// stderr: <trimmed>
/// ```
#[allow(
    dead_code,
    reason = "trimmed-stdout convenience wrapper — production callers (#1921 migration) need the raw-Output variant for their own exit-status/error-text contracts; exercised by this module's tests"
)]
pub(crate) fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = run_git_output_with_deadline(root, args, None)?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_string())
            .map_err(|err| {
                format!(
                    "git -C {} {:?} produced non-UTF-8 output: {err}",
                    root.display(),
                    args
                )
            })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "git -C {} {:?} failed\nstdout: {}\nstderr: {}",
            root.display(),
            args,
            stdout.trim(),
            stderr.trim()
        ))
    }
}

/// Trimmed stdout of a successful invocation under a deadline, for tests
/// that assert output parity against [`run_git`].
#[cfg(test)]
fn trimmed_stdout(output: &std::process::Output) -> Result<String, String> {
    String::from_utf8(output.stdout.clone())
        .map(|value| value.trim().to_string())
        .map_err(|err| format!("non-UTF-8 stdout: {err}"))
}

/// Run `git -C <root> <args...>` under an optional cooperative deadline and
/// return the raw [`Output`] regardless of exit status (#2303).
///
/// `Err` is reserved for invocation-level failures: spawn failure, wait
/// failure, cooperative cancellation, a zero deadline (rejected before
/// spawning), or deadline expiry (named [`GIT_INVOCATION_TIMEOUT_PREFIX`]
/// error, child terminated and reaped). A non-zero exit status is `Ok` so
/// callers that probe (`rev-parse --verify --quiet`, `symbolic-ref --quiet`)
/// keep their own status handling.
pub(crate) fn run_git_output_with_deadline(
    root: &Path,
    args: &[&str],
    timeout: Option<Duration>,
) -> Result<Output, String> {
    let describe = format!("git -C {} {:?}", root.display(), args);
    let mut command = Command::new("git");
    // `current_dir(root)`, not `git -C <root>`: for a missing/unusable root
    // the spawn itself fails, preserving the established
    // `failed to run git …` error family the context/explain invalid-root
    // contract pins (a `-C` flag would let git report the bad root as a
    // non-zero exit instead, changing the error text). For valid roots the
    // two forms are equivalent.
    command.current_dir(root).args(args);
    collect_output_with_deadline(&mut command, timeout, &describe)
}

/// Spawn `command` with piped stdout/stderr, collect the full output under
/// an optional deadline, and enforce the #2303 timeout/cancellation
/// contract. `describe` is the human-readable invocation used in error text.
///
/// The poll loop drains both pipes on reader threads so a verbose child
/// cannot fill the OS pipe buffer and deadlock against `try_wait` (the
/// pre-#2303 Perl precedent avoided this with `Stdio::null`; git output is
/// needed, so the pipes are drained instead).
fn collect_output_with_deadline(
    command: &mut Command,
    timeout: Option<Duration>,
    describe: &str,
) -> Result<Output, String> {
    if let Some(deadline) = timeout
        && deadline.is_zero()
    {
        return Err(format!(
            "{GIT_INVOCATION_TIMEOUT_PREFIX}: {describe} was given a zero deadline (not spawned)"
        ));
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run {describe}: {err}"))?;
    let stdout_reader = child.stdout.take().map(spawn_pipe_reader);
    let stderr_reader = child.stderr.take().map(spawn_pipe_reader);

    let wait = poll_child(&mut child, timeout, describe);
    let timed_out = !matches!(&wait, ChildWait::Exited(_));
    let drain_deadline = timed_out.then(|| Instant::now() + POST_KILL_DRAIN_GRACE);
    let stdout_result =
        drain_pipe_reader(stdout_reader, timed_out, drain_deadline, "stdout", describe);
    let stderr_result =
        drain_pipe_reader(stderr_reader, timed_out, drain_deadline, "stderr", describe);
    let stdout = stdout_result?;
    let stderr = stderr_result?;

    match wait {
        ChildWait::Exited(status) => Ok(Output {
            status,
            stdout,
            stderr,
        }),
        ChildWait::TimedOut(message) | ChildWait::Cancelled(message) => Err(message),
        ChildWait::WaitFailed(err) => Err(format!("failed while waiting on {describe}: {err}")),
    }
}

/// Outcome of the shared deadline-aware child wait (#2303). In every
/// non-`Exited` arm the child has already been terminated and reaped, so no
/// orphan process holds a handle. `WaitFailed` carries the raw wait error so
/// each caller wraps it in its own established message text.
pub(crate) enum ChildWait {
    Exited(std::process::ExitStatus),
    TimedOut(String),
    Cancelled(String),
    WaitFailed(String),
}

/// Poll `child` with `try_wait` on a short interval up to the optional
/// deadline, checking cooperative analysis cancellation each tick so a hung
/// child honors an LSP refresh supersede (#2303). Lifted from the Perl
/// facts exporter wait in `app::check` (pre-#2303 `ChildWaitTimeoutExt`)
/// and shared by both call families.
pub(crate) fn poll_child(
    child: &mut std::process::Child,
    timeout: Option<Duration>,
    describe: &str,
) -> ChildWait {
    let deadline = timeout.map(|limit| Instant::now() + limit);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return ChildWait::Exited(status),
            Ok(None) => {
                if let Err(cancelled) = crate::analysis::cancellation::checkpoint() {
                    terminate_child_tree(child);
                    return ChildWait::Cancelled(cancelled);
                }
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    terminate_child_tree(child);
                    let timeout_ms = timeout.map_or(0, |limit| limit.as_millis());
                    return ChildWait::TimedOut(format!(
                        "{GIT_INVOCATION_TIMEOUT_PREFIX}: {describe} exceeded the {timeout_ms}ms deadline (process terminated)"
                    ));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => {
                terminate_child_tree(child);
                return ChildWait::WaitFailed(err.to_string());
            }
        }
    }
}

/// Read one piped child stream to EOF on a helper thread so the poll loop
/// never deadlocks against a full OS pipe buffer.
fn spawn_pipe_reader(
    mut pipe: impl std::io::Read + Send + 'static,
) -> (std::thread::JoinHandle<()>, mpsc::Receiver<Vec<u8>>) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = pipe.read_to_end(&mut buffer);
        let _ = sender.send(buffer);
    });
    (handle, receiver)
}

fn drain_pipe_reader(
    reader: Option<(std::thread::JoinHandle<()>, mpsc::Receiver<Vec<u8>>)>,
    timed_out: bool,
    deadline: Option<Instant>,
    stream_name: &str,
    describe: &str,
) -> Result<Vec<u8>, String> {
    let Some((handle, receiver)) = reader else {
        return Ok(Vec::new());
    };
    let received = match deadline {
        Some(deadline) => receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())),
        None => receiver
            .recv()
            .map_err(|_receive_error| mpsc::RecvTimeoutError::Disconnected),
    };
    match received {
        Ok(buffer) => {
            handle.join().map_err(|_panic_payload| {
                format!("{stream_name} pipe reader panicked while collecting {describe}")
            })?;
            Ok(buffer)
        }
        Err(mpsc::RecvTimeoutError::Timeout) if timed_out => {
            // The process was already terminated and reaped. Dropping the
            // handle detaches a reader whose pipe write-end escaped with a
            // descendant; the timeout path must not wait for that OS handle.
            Ok(Vec::new())
        }
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "{stream_name} pipe did not drain after {describe} completed"
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "{stream_name} pipe reader failed while collecting {describe}"
        )),
    }
}

fn terminate_child_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cancellation::{
        AnalysisAbortKind, AnalysisCancellationToken, is_cancellation_error, with_token,
    };
    use serial_test::serial;

    /// Env flag that makes the re-executed test binary hang instead of
    /// running tests, so timeout/cancellation tests get a deterministic
    /// child that never exits on its own.
    const HANG_ENV: &str = "RIPR_GIT_TIMEOUT_TEST_HANG";
    /// Env flag that makes the re-executed test binary write more than one
    /// OS pipe buffer of stdout before exiting, exercising the drain path.
    const FLOOD_ENV: &str = "RIPR_GIT_TIMEOUT_TEST_FLOOD";

    fn reexec_harness() -> bool {
        if std::env::var_os(HANG_ENV).is_some() {
            std::thread::sleep(Duration::from_mins(2));
            std::process::exit(0);
        }
        if std::env::var_os(FLOOD_ENV).is_some() {
            // Write to fd 1 directly: `println!` inside a test binary is
            // captured by libtest and would never reach the piped stdout.
            use std::io::Write as _;
            let chunk = "0123456789abcdef".repeat(4096); // 64 KiB
            let mut out = std::io::stdout();
            for _ in 0..8 {
                let _ = out.write_all(chunk.as_bytes());
            }
            let _ = out.flush();
            std::process::exit(0);
        }
        false
    }

    fn self_reexec_command(env_key: &str) -> Result<Command, String> {
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let mut command = Command::new(exe);
        command.env(env_key, "1");
        Ok(command)
    }

    fn hang_command() -> Result<Command, String> {
        self_reexec_command(HANG_ENV)
    }

    #[test]
    fn run_git_returns_trimmed_stdout_on_success() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let result = run_git(&root, &["--version"])?;
        if !result.starts_with("git version") {
            return Err(format!("expected 'git version ...', got: {result}"));
        }
        // Verify trimming: --version output ends with a newline that should be stripped.
        if result.ends_with('\n') {
            return Err("output should be trimmed of trailing newline".to_string());
        }
        Ok(())
    }

    #[test]
    fn run_git_returns_error_on_failure() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let result = run_git(&root, &["rev-parse", "--verify", "nonexistent-ref-xyz"]);
        if result.is_ok() {
            return Err("expected error for nonexistent git ref".to_string());
        }
        let err = match result {
            Err(msg) => msg,
            Ok(_) => return Err("expected error for nonexistent git ref".to_string()),
        };
        if !err.contains("failed") {
            return Err(format!("error should contain 'failed': {err}"));
        }
        Ok(())
    }

    #[test]
    fn deadline_kills_and_reaps_a_hung_invocation() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let mut command = hang_command()?;
        let started = Instant::now();
        let result = collect_output_with_deadline(
            &mut command,
            Some(Duration::from_millis(50)),
            "hang-test",
        );
        let elapsed = started.elapsed();
        let err = match result {
            Err(err) => err,
            Ok(_) => return Err("a hung invocation must fail, not collect output".to_string()),
        };
        if !is_git_invocation_timeout(&err) {
            return Err(format!("expected the named timeout error, got: {err}"));
        }
        if !err.contains("exceeded the 50ms deadline") {
            return Err(format!(
                "timeout error should name the deadline, got: {err}"
            ));
        }
        // The child sleeps 120s; a prompt return proves kill+reap happened
        // (a leaked child would block the reader join for the full sleep).
        if elapsed >= Duration::from_secs(30) {
            return Err(format!(
                "timeout path took {elapsed:?}; the hung child was not terminated and reaped"
            ));
        }
        Ok(())
    }

    #[test]
    fn drain_pipe_reader_reports_bounded_edge_states() -> Result<(), String> {
        let empty = drain_pipe_reader(None, false, None, "stdout", "empty")?;
        if !empty.is_empty() {
            return Err("missing pipe reader should produce empty output".to_string());
        }

        let (sender, receiver) = mpsc::channel();
        drop(sender);
        let handle = std::thread::spawn(|| {});
        let disconnected = drain_pipe_reader(
            Some((handle, receiver)),
            false,
            Some(Instant::now()),
            "stderr",
            "disconnected",
        );
        match disconnected {
            Err(message) if message.contains("reader failed") => {}
            Ok(output) => {
                return Err(format!(
                    "disconnected reader should return an error, got output: {output:?}"
                ));
            }
            Err(message) => {
                return Err(format!(
                    "disconnected reader returned the wrong error: {message}"
                ));
            }
        }

        let (release_sender, release_receiver) = mpsc::channel();
        let (output_sender, output_receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = release_receiver.recv();
        });
        let timed_out = drain_pipe_reader(
            Some((handle, output_receiver)),
            true,
            Some(Instant::now()),
            "stdout",
            "timed-out",
        )?;
        if !timed_out.is_empty() {
            return Err("timed-out reader should return empty output".to_string());
        }
        let _ = release_sender.send(());
        drop(output_sender);

        let (release_sender, release_receiver) = mpsc::channel();
        let (output_sender, output_receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = release_receiver.recv();
        });
        let completed = drain_pipe_reader(
            Some((handle, output_receiver)),
            false,
            Some(Instant::now()),
            "stderr",
            "completed-timeout",
        );
        match completed {
            Err(message) if message.contains("did not drain") => {}
            Ok(output) => {
                return Err(format!(
                    "completed reader timeout should be an error, got output: {output:?}"
                ));
            }
            Err(message) => {
                return Err(format!(
                    "completed reader returned the wrong error: {message}"
                ));
            }
        }
        let _ = release_sender.send(());
        drop(output_sender);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    #[serial]
    fn deadline_kills_pipe_inheriting_descendants_without_blocking_the_reader() -> Result<(), String>
    {
        if reexec_harness() {
            return Ok(());
        }
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            "$p = Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 30') -NoNewWindow -PassThru; Wait-Process -Id $p.Id",
        ]);
        let started = Instant::now();
        let result = collect_output_with_deadline(
            &mut command,
            Some(Duration::from_secs(5)),
            "pipe-inheriting-descendant",
        );
        let elapsed = started.elapsed();
        let err = match result {
            Err(err) => err,
            Ok(_) => {
                return Err("a descendant-holding invocation must fail with a timeout".to_string());
            }
        };
        if !is_git_invocation_timeout(&err) {
            return Err(format!("expected the named timeout error, got: {err}"));
        }
        if elapsed >= Duration::from_secs(20) {
            return Err(format!(
                "pipe-inheriting timeout took {elapsed:?}; reader drain was not bounded"
            ));
        }
        Ok(())
    }

    #[test]
    fn zero_deadline_errors_before_spawning() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let result = run_git_output_with_deadline(&root, &["--version"], Some(Duration::ZERO));
        let err = match result {
            Err(err) => err,
            Ok(_) => return Err("a zero deadline must fail before spawning".to_string()),
        };
        if !is_git_invocation_timeout(&err) {
            return Err(format!("expected the named timeout error, got: {err}"));
        }
        if !err.contains("zero deadline (not spawned)") {
            return Err(format!(
                "zero-deadline error should say pre-spawn, got: {err}"
            ));
        }
        Ok(())
    }

    #[test]
    fn cancellation_wins_over_a_long_deadline() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let token = AnalysisCancellationToken::new();
        if !token.cancel(AnalysisAbortKind::Superseded) {
            return Err("fresh token should accept cancellation".to_string());
        }
        let mut command = hang_command()?;
        let started = Instant::now();
        let result = with_token(&token, || {
            collect_output_with_deadline(&mut command, Some(Duration::from_mins(2)), "hang-test")
        });
        let elapsed = started.elapsed();
        let err = match result {
            Err(err) => err,
            Ok(_) => return Err("a cancelled invocation must fail".to_string()),
        };
        if !is_cancellation_error(&err) {
            return Err(format!("expected the cancellation error, got: {err}"));
        }
        if is_git_invocation_timeout(&err) {
            return Err(format!(
                "cancellation must win over the deadline, got: {err}"
            ));
        }
        if elapsed >= Duration::from_secs(30) {
            return Err(format!(
                "cancellation path took {elapsed:?}; the hung child was not terminated and reaped"
            ));
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn output_larger_than_the_pipe_buffer_does_not_deadlock() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let mut command = self_reexec_command(FLOOD_ENV)?;
        let started = Instant::now();
        let output = collect_output_with_deadline(
            &mut command,
            Some(Duration::from_secs(30)),
            "flood-test",
        )?;
        if !output.status.success() {
            return Err(format!("flood child failed: {}", output.status));
        }
        // 8 chunks of 64 KiB must all be collected; a drained pipe is the
        // only way the child could exit without a deadlock.
        if output.stdout.len() < 8 * 64 * 1024 {
            return Err(format!(
                "expected drained output of at least 512 KiB, got {} bytes",
                output.stdout.len()
            ));
        }
        if started.elapsed() >= Duration::from_secs(30) {
            return Err("flood test consumed its whole deadline".to_string());
        }
        Ok(())
    }

    #[test]
    fn successful_invocation_with_deadline_matches_unbounded_output() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let bounded = trimmed_stdout(&run_git_output_with_deadline(
            &root,
            &["--version"],
            Some(Duration::from_secs(30)),
        )?)?;
        let unbounded = run_git(&root, &["--version"])?;
        if bounded != unbounded {
            return Err(format!("bounded {bounded:?} != unbounded {unbounded:?}"));
        }
        Ok(())
    }
}
