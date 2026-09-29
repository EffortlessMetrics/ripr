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

use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};
use std::time::{Duration, Instant};

#[cfg(windows)]
use process_wrap::std::{ChildWrapper, CommandWrap, JobObject};

/// Poll interval for the owner's bounded reap loop. Matches the shared
/// deadline poll in [`crate::git`] so a terminating tree is reaped on the
/// same cadence the callers already budget for.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

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
            Ok(Self { child })
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
        self.child.try_wait()
    }

    /// Reap the direct child to completion.
    ///
    /// This polls the child instead of blocking on a job completion port so
    /// the wait stays bounded by the caller's termination decisions and can
    /// never hang on queued job notifications.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(WAIT_POLL_INTERVAL);
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
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    std::thread::sleep(WAIT_POLL_INTERVAL);
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
