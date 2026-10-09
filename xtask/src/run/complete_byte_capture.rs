//! Complete-mode byte capture; ordinary captures retain their existing owner.
//! Output is admissible only after qualified group settlement and complete drains.

use super::{ByteCaptureBudget, TimedBytesOutput};
use std::path::Path;

pub(crate) fn capture_complete_bytes_in_dir_with_budget(
    program: &Path,
    args: &[String],
    source: (&Path, Option<&[u8]>),
    env_remove: &[&str],
    budget: ByteCaptureBudget,
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    #[cfg(target_os = "linux")]
    {
        linux::capture(
            program,
            args,
            source,
            env_remove,
            budget,
            error_context,
            |_| {},
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (program, args, source, env_remove, budget);
        Err(format!(
            "complete byte capture for {error_context} requires qualified Linux group ownership; spawn refused"
        ))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::super::{
        POST_KILL_DRAIN_GRACE, configure_timed_child_command, drain_byte_reader_bounded,
        owned_capture_group::OwnedCaptureGuard, spawn_byte_reader_channel,
    };
    use super::*;
    use ripr::process_owner::OwnedProcess;
    use std::io::Write;
    use std::process::{Command, ExitStatus, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    // The private observer receives only a status already reaped by the qualified
    // guard. Native controls use it to establish actual leader exit0 on refusal.
    pub(super) fn capture(
        program: &Path,
        args: &[String],
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        budget: ByteCaptureBudget,
        error_context: &str,
        mut settled: impl FnMut(ExitStatus),
    ) -> Result<TimedBytesOutput, String> {
        let (cwd, input) = source;
        // The caller owns input admission. Make the asynchronous copy fallible
        // before spawning; the existing interface supplies no separate input cap.
        let input = input
            .map(|bytes| {
                let mut copy = Vec::new();
                copy.try_reserve_exact(bytes.len())
                    .map_err(|error| format!("reserve stdin for {error_context}: {error}"))?;
                copy.extend_from_slice(bytes);
                Ok::<_, String>(copy)
            })
            .transpose()?;
        if budget.timeout.is_zero() {
            return Err(format!(
                "complete byte capture for {error_context} has no deadline"
            ));
        }
        let started = Instant::now();
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd);
        configure_timed_child_command(&mut command);
        for name in env_remove {
            command.env_remove(name);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let child = OwnedProcess::spawn_with_bounded_drop(command)
            .map_err(|error| format!("failed to run {error_context}: {error}"))?;
        // No try_wait, callback or pipe acquisition may precede this leader lease.
        let mut guard = OwnedCaptureGuard::new(child)?;
        let result = (|| {
            let stdout = guard
                .child()
                .stdout_pipe()
                .take()
                .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
            let stderr = guard
                .child()
                .stderr_pipe()
                .take()
                .ok_or_else(|| format!("failed to capture stderr for {error_context}"))?;
            let (stdout_handle, stdout_rx) =
                spawn_byte_reader_channel(stdout, Some(("stdout", budget.stdout_bytes)));
            let (stderr_handle, stderr_rx) =
                spawn_byte_reader_channel(stderr, Some(("stderr", budget.stderr_bytes)));
            let input_completion = if let Some(bytes) = input {
                let mut stdin = guard
                    .child()
                    .stdin_pipe()
                    .take()
                    .ok_or_else(|| format!("failed to capture stdin for {error_context}"))?;
                let (sender, receiver) = mpsc::channel();
                let _writer = thread::Builder::new()
                    .name("complete-process-stdin".to_string())
                    .spawn(move || {
                        let result = stdin.write_all(&bytes);
                        drop(stdin);
                        let _ = sender.send(result);
                    })
                    .map_err(|error| format!("start stdin writer for {error_context}: {error}"))?;
                Some(receiver)
            } else {
                None
            };
            let outcome = match guard.wait(started, budget.timeout) {
                Ok(outcome) => {
                    settled(outcome.status);
                    Ok(outcome)
                }
                Err(reason) => match guard.abort() {
                    Ok(()) => {
                        // abort() confirms settlement before this cached-status
                        // observation; a refused group must never be reaped here.
                        if let Some(status) = guard.child().try_wait().map_err(|error| {
                            format!("settled leader status for {error_context}: {error}")
                        })? {
                            settled(status);
                        }
                        Err(reason)
                    }
                    Err(cleanup) => Err(format!(
                        "{reason}; qualified group cleanup unconfirmed: {cleanup}"
                    )),
                },
            };
            // One shared grace covers both output streams and the stdin writer.
            // Even an original wait failure is retained while drains are attempted.
            let drain_deadline = Instant::now() + POST_KILL_DRAIN_GRACE;
            let remaining = || drain_deadline.saturating_duration_since(Instant::now());
            let stdout = drain_byte_reader_bounded(
                stdout_rx,
                stdout_handle,
                remaining(),
                "stdout",
                error_context,
                true,
            );
            let stderr = drain_byte_reader_bounded(
                stderr_rx,
                stderr_handle,
                remaining(),
                "stderr",
                error_context,
                true,
            );
            let input_result = if let Some(receiver) = input_completion {
                match receiver.recv_timeout(remaining()) {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(error))
                        if outcome.as_ref().is_ok_and(|wait| wait.timed_out)
                            && error.kind() == std::io::ErrorKind::BrokenPipe =>
                    {
                        Ok(())
                    }
                    Ok(Err(error)) => Err(format!("write stdin for {error_context}: {error}")),
                    Err(error) => Err(format!("stdin completion for {error_context}: {error}")),
                }
            } else {
                Ok(())
            };
            let mut errors = Vec::new();
            for error in [
                outcome.as_ref().err(),
                stdout.as_ref().err(),
                stderr.as_ref().err(),
                input_result.as_ref().err(),
            ]
            .into_iter()
            .flatten()
            {
                errors.push(error.as_str());
            }
            if !errors.is_empty() {
                return Err(format!("{error_context}: {}", errors.join("; ")));
            }
            let outcome = outcome?;
            Ok(TimedBytesOutput {
                status: Some(outcome.status),
                stdout: stdout?,
                stderr: stderr?,
                duration: started.elapsed(),
                timed_out: outcome.timed_out,
            })
        })();
        match result {
            Ok(output) => Ok(output),
            Err(error) => match guard.abort() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!(
                    "{error}; qualified group cleanup unconfirmed: {cleanup}"
                )),
            },
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);

        struct Fixture(PathBuf);
        impl Fixture {
            fn new() -> Result<Self, String> {
                let root = std::env::temp_dir();
                fs::create_dir_all(&root).map_err(|error| format!("fixture root: {error}"))?;
                let path = root.join(format!(
                    "ripr-complete-capture-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).map_err(|error| format!("capture fixture: {error}"))?;
                Ok(Self(path))
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn budget(timeout: Duration) -> ByteCaptureBudget {
            ByteCaptureBudget {
                timeout,
                stdout_bytes: 4096,
                stderr_bytes: 4096,
            }
        }
        fn shell(script: &str) -> Vec<String> {
            vec!["-c".to_string(), script.to_string()]
        }
        // An independent bounded /proc oracle; it never authorizes signaling.
        fn stat(text: &str) -> Result<(u32, u32, u64, bool), String> {
            let (pid, _) = text.split_once(' ').ok_or("fixture stat PID absent")?;
            let close = text.rfind(')').ok_or("fixture stat comm absent")?;
            let fields: Vec<_> = text[close + 1..].split_whitespace().collect();
            let group = fields
                .get(2)
                .ok_or("fixture stat group absent")?
                .parse::<u32>()
                .map_err(|error| error.to_string())?;
            let start = fields
                .get(19)
                .ok_or("fixture stat start absent")?
                .parse::<u64>()
                .map_err(|error| error.to_string())?;
            let live = !matches!(fields.first().copied(), Some("Z" | "X" | "x"));
            Ok((
                pid.parse::<u32>().map_err(|error| error.to_string())?,
                group,
                start,
                live,
            ))
        }
        fn record(path: &Path) -> Result<(u32, u32, u64, bool), String> {
            use std::io::Read;
            let mut text = String::new();
            fs::File::open(path)
                .map_err(|error| format!("stat open: {error}"))?
                .take(4097)
                .read_to_string(&mut text)
                .map_err(|error| format!("stat read: {error}"))?;
            if text.len() > 4096 {
                return Err("fixture stat exceeded bound".to_string());
            }
            stat(&text)
        }
        fn no_live_member(before: (u32, u32, u64, bool)) -> Result<(), String> {
            let path = PathBuf::from(format!("/proc/{}/stat", before.0));
            match record(&path) {
                Ok(after) if after.2 != before.2 || !after.3 => Ok(()),
                Err(_) if !path.exists() => Ok(()),
                observed => Err(format!(
                    "capture returned with unconfirmed member {}: {observed:?}",
                    before.0
                )),
            }
        }
        fn survivor_control(close_pipes: bool) -> Result<(), String> {
            let fixture = Fixture::new()?;
            let mut command = Command::new("/usr/bin/sleep");
            command
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            configure_timed_child_command(&mut command);
            let mut sentinel = OwnedProcess::spawn_with_bounded_drop(command)
                .map_err(|error| format!("sentinel: {error}"))?;
            let sentinel_before = record(Path::new(&format!("/proc/{}/stat", sentinel.id())))?;
            if sentinel_before.0 != sentinel_before.1 || !sentinel_before.3 {
                return Err("sentinel did not establish its own live group".to_string());
            }
            let redirect = if close_pipes { ">/dev/null 2>&1" } else { "" };
            let script = format!(
                "/usr/bin/sleep 30 {redirect} &\ndescendant=$!\n/usr/bin/cat /proc/$$/stat > leader\n/usr/bin/cat /proc/$descendant/stat > descendant\nkill -0 \"$descendant\" || exit 72\nprintf admitted-output\nexit 0"
            );
            let mut leader_status = None;
            let started = Instant::now();
            let result = capture(
                Path::new("/bin/sh"),
                &shell(&script),
                (&fixture.0, None),
                &[],
                budget(Duration::from_secs(3)),
                "complete survivor control",
                |status| leader_status = Some(status),
            );
            // This is the adapter's admission result: no receipt may consume output.
            match result {
                Err(error) if error.contains("exited with live group members") => {}
                Err(error) => return Err(format!("wrong survivor refusal: {error}")),
                Ok(_) => return Err("surviving descendant admitted successful capture".to_string()),
            }
            if leader_status.and_then(|status| status.code()) != Some(0) {
                return Err("actual settled leader did not exit0".to_string());
            }
            let leader = record(&fixture.0.join("leader"))?;
            let descendant = record(&fixture.0.join("descendant"))?;
            if leader.0 != leader.1 || descendant.1 != leader.1 || !descendant.3 {
                return Err(
                    "fixture did not establish a live descendant in the leased group".to_string(),
                );
            }
            no_live_member(leader)?;
            no_live_member(descendant)?;
            let sentinel_after = record(Path::new(&format!("/proc/{}/stat", sentinel.id())))?;
            if sentinel_before != sentinel_after || !sentinel_after.3 {
                return Err("complete capture altered the unrelated sentinel group".to_string());
            }
            if started.elapsed() > Duration::from_secs(11) {
                return Err(
                    "complete survivor capture exceeded deadline plus settlement/drain grace"
                        .to_string(),
                );
            }
            sentinel
                .kill()
                .map_err(|error| format!("sentinel direct kill: {error}"))?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if sentinel
                    .try_wait()
                    .map_err(|error| format!("sentinel reap: {error}"))?
                    .is_some()
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("sentinel reap exceeded bound".to_string());
                }
                thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        }
        #[test]
        fn leader_exit0_with_pipe_closing_descendant_refuses_admission() -> Result<(), String> {
            survivor_control(true)
        }
        #[test]
        fn leader_exit0_with_pipe_holding_descendant_refuses_admission() -> Result<(), String> {
            survivor_control(false)
        }
        #[test]
        fn complete_bytes_preserve_binary_input_status_and_clean_recovery() -> Result<(), String> {
            let fixture = Fixture::new()?;
            for code in [0, 7] {
                let input = b"\x00\xff\nexact-input";
                let output = capture_complete_bytes_in_dir_with_budget(
                    Path::new("/bin/sh"),
                    &shell(&format!("cat; printf exact-stderr >&2; exit {code}")),
                    (&fixture.0, Some(input)),
                    &[],
                    budget(Duration::from_secs(3)),
                    "complete clean control",
                )?;
                if output.status.and_then(|status| status.code()) != Some(code)
                    || output.timed_out
                    || output.stdout != input
                    || output.stderr != b"exact-stderr"
                {
                    return Err(
                        "clean byte capture changed binary input/output or exit status".to_string(),
                    );
                }
            }
            Ok(())
        }
        #[test]
        fn complete_bytes_refuse_overflow_and_settle_timeout() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let mut limits = budget(Duration::from_secs(3));
            limits.stdout_bytes = 3;
            match capture_complete_bytes_in_dir_with_budget(
                Path::new("/bin/sh"),
                &shell("printf four"),
                (&fixture.0, None),
                &[],
                limits,
                "complete overflow control",
            ) {
                Err(error) if error.contains("stdout exceeds its 3-byte output budget") => {}
                Err(error) => return Err(format!("wrong overflow error: {error}")),
                Ok(_) => return Err("byte overflow admitted successful capture".to_string()),
            }
            let started = Instant::now();
            let output = capture_complete_bytes_in_dir_with_budget(
                Path::new("/bin/sh"),
                &shell(
                    "cat /proc/$$/stat > leader; sleep 30 & descendant=$!; cat /proc/$descendant/stat > descendant; wait",
                ),
                (&fixture.0, None),
                &[],
                budget(Duration::from_millis(300)),
                "complete timeout control",
            )?;
            if !output.timed_out || output.status.is_some_and(|status| status.success()) {
                return Err("complete timeout was admitted as normal success".to_string());
            }
            no_live_member(record(&fixture.0.join("leader"))?)?;
            no_live_member(record(&fixture.0.join("descendant"))?)?;
            if started.elapsed() > Duration::from_secs(9) {
                return Err("complete timeout exceeded settlement/drain bound".to_string());
            }
            Ok(())
        }
    }
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn unsupported_complete_capture_refuses_before_spawn() -> Result<(), String> {
        match capture_complete_bytes_in_dir_with_budget(
            Path::new("missing-complete-capture-program"),
            &[],
            (Path::new("."), None),
            &[],
            ByteCaptureBudget {
                timeout: Duration::from_secs(1),
                stdout_bytes: 1,
                stderr_bytes: 1,
            },
            "unsupported control",
        ) {
            Err(error)
                if error.contains("requires qualified Linux group ownership; spawn refused") =>
            {
                Ok(())
            }
            Err(error) => Err(format!("unsupported capture reached wrong route: {error}")),
            Ok(_) => Err("unsupported capture was admitted".to_string()),
        }
    }
}
