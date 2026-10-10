//! Complete-only forwarding to the shared qualified byte owner.
//! Real success/error receipts are preserved by the explicit-clock entrypoint.

use super::{ByteCaptureBudget, TimedBytesOutput};
use ripr::process_owner::{
    CompleteByteCapture, CompleteCaptureBudget, CompleteCaptureError, CompleteCapturedBytes,
};
use std::any::Any;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Instant;

/// Forward the exact actual observation and consume-once receipt without rebuilding
/// them from status, paths or saved data. The caller owns invocation authentication.
pub(crate) fn capture_complete_bytes_in_dir_with_deadline<L: Any + Send + Sync>(
    command: (&Path, &[String]),
    source: (&Path, Option<&[u8]>),
    env_remove: &[&str],
    execution: (CompleteCaptureBudget, &str),
    held_deadline: Instant,
    lease: Arc<L>,
    settled: impl FnMut(ExitStatus),
) -> Result<CompleteCapturedBytes, CompleteCaptureError> {
    CompleteByteCapture::capture_with_deadline(
        command,
        source,
        env_remove,
        execution,
        held_deadline,
        lease,
        settled,
    )
}

/// Compatibility DTO only. Its discarded receipt cannot authorize stage cleanup.
pub(crate) fn capture_complete_bytes_in_dir_with_budget(
    program: &Path,
    args: &[String],
    source: (&Path, Option<&[u8]>),
    env_remove: &[&str],
    budget: ByteCaptureBudget,
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    capture_legacy(
        program,
        args,
        source,
        env_remove,
        budget,
        error_context,
        |_| {},
    )
}

fn capture_legacy(
    program: &Path,
    args: &[String],
    source: (&Path, Option<&[u8]>),
    env_remove: &[&str],
    budget: ByteCaptureBudget,
    error_context: &str,
    settled: impl FnMut(ExitStatus),
) -> Result<TimedBytesOutput, String> {
    let limits = CompleteCaptureBudget::new(
        budget.timeout,
        source.1.map_or(0, |bytes| bytes.len()),
        budget.stdout_bytes,
        budget.stderr_bytes,
    );
    // Preserve the old execution/settlement/drain semantics. The new explicit
    // clock route is separate and requires the invocation owner's held bound.
    match CompleteByteCapture::capture(
        (program, args),
        source,
        env_remove,
        (limits, error_context),
        Arc::new(()),
        settled,
    ) {
        Ok(output) => {
            let (status, stdout, stderr, duration, timed_out, _receipt) = output.into_parts();
            Ok(TimedBytesOutput {
                status: Some(status),
                stdout,
                stderr,
                duration,
                timed_out,
            })
        }
        Err(mut error) if error.is_timeout_only() => {
            // Only the shared owner's real timeout-only classification permits
            // legacy timeout DATA. Other failures never become successful output.
            let (status, stdout, stderr, duration, timed_out) = error
                .take_failed_observation()
                .ok_or_else(|| format!("{}; timeout observation unavailable", error.message()))?;
            Ok(TimedBytesOutput {
                status: Some(status),
                stdout,
                stderr,
                duration,
                timed_out,
            })
        }
        Err(error) => Err(error.message().to_string()),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux {
    use super::super::configure_timed_child_command;
    use super::*;
    use ripr::process_owner::OwnedProcess;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    fn capture(
        program: &Path,
        args: &[String],
        source: (&Path, Option<&[u8]>),
        env_remove: &[&str],
        budget: ByteCaptureBudget,
        error_context: &str,
        settled: impl FnMut(ExitStatus),
    ) -> Result<TimedBytesOutput, String> {
        capture_legacy(
            program,
            args,
            source,
            env_remove,
            budget,
            error_context,
            settled,
        )
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
        // Preserve failed controls for owner verification; no unsettled Drop cleanup.
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
        #[test]
        fn forwarder_preserves_real_worker_lease_and_consume_once_error_receipt()
        -> Result<(), String> {
            let fixture = Fixture::new()?;
            let lease = Arc::new(());
            let foreign = Arc::new(());
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(3))
                .ok_or("deadline overflow")?;
            let output = capture_complete_bytes_in_dir_with_deadline(
                (
                    Path::new("/bin/sh"),
                    &shell("cat /proc/$$/stat > primary; printf actual-bytes"),
                ),
                (&fixture.0, None),
                &[],
                (
                    CompleteCaptureBudget::new(Duration::from_secs(3), 0, 64, 64),
                    "real receipt forwarder",
                ),
                deadline,
                lease.clone(),
                |_| {},
            )
            .map_err(|error| error.to_string())?;
            let (status, stdout, stderr, _, timed_out, receipt) = output.into_parts();
            assert_eq!(status.code(), Some(0));
            assert_eq!(stdout, b"actual-bytes");
            assert_eq!(stderr, b"");
            assert!(!timed_out);
            if !receipt.matches_lease(&lease) || receipt.matches_lease(&foreign) {
                return Err("forwarder lost actual unique lease identity".to_string());
            }
            let actual = record(&fixture.0.join("primary"))?;
            let worker = receipt.observed_worker();
            assert_eq!(
                (worker.pid(), worker.group(), worker.start()),
                (actual.0, actual.1, actual.2)
            );
            assert_eq!(receipt.observed_parent().pid(), std::process::id());
            assert_eq!(receipt.settled_status(), Some(status));
            no_live_member(actual)?;
            drop(receipt);
            assert_eq!(Arc::strong_count(&lease), 1);
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(3))
                .ok_or("deadline overflow")?;
            let mut error = match capture_complete_bytes_in_dir_with_deadline(
                (Path::new("/bin/sh"), &shell("printf four")),
                (&fixture.0, None),
                &[],
                (
                    CompleteCaptureBudget::new(Duration::from_secs(3), 0, 3, 64),
                    "real overflow forwarder",
                ),
                deadline,
                lease.clone(),
                |_| {},
            ) {
                Err(error) => error,
                Ok(_) => {
                    return Err(
                        "forwarder converted actual overflow to successful bytes".to_string()
                    );
                }
            };
            if error.is_timeout_only()
                || !error
                    .message()
                    .contains("stdout exceeds its 3-byte output budget")
            {
                return Err(format!("wrong real overflow failure: {error}"));
            }
            let cleanup = error
                .take_cleanup_receipt()
                .ok_or("actual combined cleanup receipt was lost")?;
            if !cleanup.matches_lease(&lease)
                || cleanup.matches_lease(&foreign)
                || error.take_cleanup_receipt().is_some()
            {
                return Err("failed receipt mismatched lease or was transferable twice".to_string());
            }
            assert_eq!(
                cleanup.settled_status().and_then(|status| status.code()),
                Some(0)
            );
            drop(cleanup);
            drop(error);
            assert_eq!(Arc::strong_count(&lease), 1);
            Ok(())
        }

        #[test]
        fn expired_forwarder_refuses_before_executable_lookup_without_receipt() -> Result<(), String>
        {
            let fixture = Fixture::new()?;
            let lease = Arc::new(());
            let mut observed = false;
            let mut error = match capture_complete_bytes_in_dir_with_deadline(
                (Path::new("missing-expired-capture-executable"), &[]),
                (&fixture.0, None),
                &[],
                (
                    CompleteCaptureBudget::new(Duration::from_secs(3), 0, 64, 64),
                    "expired forwarder",
                ),
                Instant::now(),
                lease.clone(),
                |_| observed = true,
            ) {
                Err(error) => error,
                Ok(_) => return Err("expired capture admitted output".to_string()),
            };
            if observed
                || !error.message().contains("deadline")
                || !error.matches_lease(&lease)
                || error.observed_outcome().is_some()
                || error.take_cleanup_receipt().is_some()
            {
                return Err(format!(
                    "expired capture reached observation/cleanup or wrong refusal: {error}"
                ));
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
