use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Grace period for draining stdout/stderr pipes after a process-group kill.
///
/// After `terminate_timed_process_tree` fires, any descendant that escaped the
/// group-kill may still hold the inherited pipe write-end open, keeping
/// `read_to_end` blocked. This constant caps how long we wait for the pipes to
/// drain before giving up and returning whatever was captured so far, plus a
/// diagnostic note in the output.  Five seconds is generous relative to the
/// typical `~100 ms` group-kill propagation delay while still being a hard
/// upper bound.
const POST_KILL_DRAIN_GRACE: Duration = Duration::from_secs(5);

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(crate) struct CapturedOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) struct TimedOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

pub(crate) struct TimedFileOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
    pub(crate) stdout_bytes: usize,
}

struct WaitOutcome {
    status: ExitStatus,
    duration: Duration,
    timed_out: bool,
}

pub(crate) fn run(program: &str, args: &[&str]) -> Result<ExitStatus, String> {
    run_with_envs(program, args, &[])
}

pub(crate) fn run_with_envs(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<ExitStatus, String> {
    let env_text = envs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ");
    let env_prefix = if env_text.is_empty() {
        String::new()
    } else {
        format!("{env_text} ")
    };
    eprintln!("$ {env_prefix}{} {}", program, args.join(" "));
    let mut command = Command::new(program);
    command.args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    let status = command
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(status)
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

pub(crate) fn command_success_owned(program: &str, args: &[String]) -> Result<bool, String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    Ok(status.success())
}

pub(crate) fn run_owned(program: &str, args: &[String]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

pub(crate) fn run_in_dir(program: &Path, args: &[&str], cwd: &Path) -> Result<ExitStatus, String> {
    run_in_dir_with_envs(program, args, cwd, &[])
}

pub(crate) fn run_in_dir_with_envs(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    envs: &[(&str, &str)],
) -> Result<ExitStatus, String> {
    let env_text = envs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ");
    let env_prefix = if env_text.is_empty() {
        String::new()
    } else {
        format!("{env_text} ")
    };
    eprintln!(
        "$ (cd {} && {}{} {})",
        cwd.display(),
        env_prefix,
        program.display(),
        args.join(" ")
    );
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    for (name, value) in envs {
        command.env(name, value);
    }
    let status = command.status().map_err(|err| {
        format!(
            "failed to run {} in {}: {err}",
            program.display(),
            cwd.display()
        )
    })?;
    if status.success() {
        Ok(status)
    } else {
        Err(format!(
            "{} {} failed with {status} in {}",
            program.display(),
            args.join(" "),
            cwd.display()
        ))
    }
}

pub(crate) fn run_output(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn run_output_owned(program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{program} {} failed with {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn run_output_optional(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Ok(String::new())
    }
}

pub(crate) fn capture_output(
    program: &str,
    args: &[&str],
    error_context: &str,
) -> Result<CapturedOutput, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    Ok(CapturedOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn capture_output_with_timeout(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
    timeout: Duration,
    error_context: &str,
) -> Result<TimedOutput, String> {
    let started = Instant::now();
    let mut command = Command::new(program);
    command.args(args);
    configure_timed_child_command(&mut command);
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr for {error_context}"))?;
    let echo_latency_trace = envs
        .iter()
        .any(|(name, _)| *name == "RIPR_REPO_EXPOSURE_LATENCY_TRACE");

    // Always use channel-based readers so the post-kill drain path can apply a
    // bounded wait via `recv_timeout` (see `drain_stream_reader_bounded`).
    let (stdout_handle, stdout_rx) = spawn_stream_reader_channel(stdout);
    let (stderr_handle, stderr_rx) = if echo_latency_trace {
        spawn_latency_stream_reader_channel(stderr)
    } else {
        spawn_stream_reader_channel(stderr)
    };

    let wait_outcome = wait_for_child_with_timeout(&mut child, started, timeout, error_context)?;

    // Always use the bounded drain.  On a normal process exit the pipe
    // write-ends are already closed, so the reader threads finish promptly and
    // the grace timeout is never reached — behavior is identical to an
    // unbounded join.  On a timed-out kill the grace timeout caps the drain if
    // a descendant escaped the process-group kill and still holds the pipe open,
    // guaranteeing the function returns in bounded time regardless.
    let stdout = drain_stream_reader_bounded(
        stdout_rx,
        stdout_handle,
        POST_KILL_DRAIN_GRACE,
        "stdout",
        error_context,
    )?;
    let stderr = drain_stream_reader_bounded(
        stderr_rx,
        stderr_handle,
        POST_KILL_DRAIN_GRACE,
        "stderr",
        error_context,
    )?;

    Ok(TimedOutput {
        status: Some(wait_outcome.status),
        stdout,
        stderr,
        duration: wait_outcome.duration,
        timed_out: wait_outcome.timed_out,
    })
}

pub(crate) fn capture_stdout_to_file_with_timeout(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
    stdout_path: &Path,
    timeout: Duration,
    error_context: &str,
) -> Result<TimedFileOutput, String> {
    let started = Instant::now();
    let stdout_tmp_path = stdout_capture_temp_path(stdout_path);
    let stdout_file = fs::File::create(&stdout_tmp_path).map_err(|err| {
        format!(
            "failed to create stdout file {} for {error_context}: {err}",
            stdout_tmp_path.display()
        )
    })?;
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped());
    configure_timed_child_command(&mut command);
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = match command.stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) => {
            let _ = fs::remove_file(&stdout_tmp_path);
            return Err(format!("failed to run {error_context}: {err}"));
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr for {error_context}"))?;
    let echo_latency_trace = envs
        .iter()
        .any(|(name, _)| *name == "RIPR_REPO_EXPOSURE_LATENCY_TRACE");
    let (stdout_writer_handle, stdout_writer_rx) =
        spawn_stream_file_writer_channel(stdout, stdout_file);
    let (stderr_handle, stderr_rx) = if echo_latency_trace {
        spawn_latency_stream_reader_channel(stderr)
    } else {
        spawn_stream_reader_channel(stderr)
    };

    let wait_outcome = wait_for_child_with_timeout(&mut child, started, timeout, error_context)?;

    // Use bounded drains for the same reason as in `capture_output_with_timeout`:
    // after a group-kill an escaped descendant may keep the pipe open.
    let stdout_bytes = match drain_file_writer_bounded(
        stdout_writer_rx,
        stdout_writer_handle,
        POST_KILL_DRAIN_GRACE,
        "stdout",
        error_context,
    ) {
        Ok(stdout_bytes) => stdout_bytes,
        Err(err) => {
            let _ = fs::remove_file(&stdout_tmp_path);
            return Err(err);
        }
    };
    publish_stdout_capture(&stdout_tmp_path, stdout_path, error_context)?;
    let stderr = drain_stream_reader_bounded(
        stderr_rx,
        stderr_handle,
        POST_KILL_DRAIN_GRACE,
        "stderr",
        error_context,
    )?;
    Ok(TimedFileOutput {
        status: Some(wait_outcome.status),
        stderr,
        duration: wait_outcome.duration,
        timed_out: wait_outcome.timed_out,
        stdout_bytes,
    })
}

fn stdout_capture_temp_path(stdout_path: &Path) -> std::path::PathBuf {
    let file_name = stdout_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("stdout");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    stdout_path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        unique
    ))
}

fn publish_stdout_capture(
    tmp_path: &Path,
    stdout_path: &Path,
    error_context: &str,
) -> Result<(), String> {
    match fs::remove_file(stdout_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "failed to remove stale stdout file {} for {error_context}: {err}",
                stdout_path.display()
            ));
        }
    }
    fs::rename(tmp_path, stdout_path).map_err(|err| {
        format!(
            "failed to publish stdout file {} for {error_context}: {err}",
            stdout_path.display()
        )
    })
}

fn wait_for_child_with_timeout(
    child: &mut Child,
    started: Instant,
    timeout: Duration,
    error_context: &str,
) -> Result<WaitOutcome, String> {
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to poll {error_context}: {err}"))?
        {
            return Ok(WaitOutcome {
                status,
                duration: started.elapsed(),
                timed_out: false,
            });
        }

        if started.elapsed() >= timeout {
            let termination_requested = terminate_after_timeout(child, error_context)?;
            let status = child
                .wait()
                .map_err(|err| format!("failed to finish timed-out {error_context}: {err}"))?;
            return Ok(WaitOutcome {
                status,
                duration: started.elapsed(),
                timed_out: timeout_was_enforced(termination_requested, &status),
            });
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn configure_timed_child_command(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

fn timeout_was_enforced(termination_requested: bool, _status: &ExitStatus) -> bool {
    termination_requested
}

fn terminate_after_timeout(child: &mut Child, error_context: &str) -> Result<bool, String> {
    if child
        .try_wait()
        .map_err(|err| format!("failed to poll {error_context}: {err}"))?
        .is_some()
    {
        return Ok(false);
    }
    let tree_terminated = terminate_timed_process_tree(child);
    if tree_terminated {
        return Ok(true);
    }
    match child.kill() {
        Ok(()) => Ok(true),
        Err(kill_err) => {
            if child
                .try_wait()
                .map_err(|err| format!("failed to poll {error_context}: {err}"))?
                .is_some()
            {
                Ok(false)
            } else {
                Err(format!(
                    "failed to terminate timed-out {error_context}: {kill_err}"
                ))
            }
        }
    }
}

fn terminate_timed_process_tree(child: &Child) -> bool {
    #[cfg(unix)]
    {
        let group = format!("-{}", child.id());
        let status = Command::new("kill")
            .args(["-KILL", "--", group.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        status.is_ok_and(|status| status.success())
    }
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let status = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        status.is_ok_and(|status| status.success())
    }
    #[cfg(not(unix))]
    #[cfg(not(windows))]
    {
        let _ = child;
        false
    }
}

fn read_stream<T: Read>(mut stream: T) -> Result<String, String> {
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read process output: {err}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn stream_to_file<T: Read>(mut stream: T, mut file: fs::File) -> Result<usize, String> {
    let mut total = 0usize;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let bytes = stream
            .read(&mut buf)
            .map_err(|err| format!("failed to read process stdout: {err}"))?;
        if bytes == 0 {
            break;
        }
        file.write_all(&buf[..bytes])
            .map_err(|err| format!("failed to write process stdout: {err}"))?;
        total = total.saturating_add(bytes);
    }
    file.flush()
        .map_err(|err| format!("failed to flush process stdout: {err}"))?;
    Ok(total)
}

fn read_stream_with_latency_progress<T: Read>(stream: T) -> Result<String, String> {
    let mut reader = BufReader::new(stream);
    let mut out = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| format!("failed to read process output: {err}"))?;
        if bytes == 0 {
            break;
        }
        if line.starts_with("ripr_repo_exposure_latency ") {
            eprint!("{line}");
        }
        out.push_str(&line);
    }
    Ok(out)
}

/// Spawn a stream reader that delivers its result over a channel rather than
/// only through `JoinHandle::join`.  Returns `(handle, receiver)`.  The handle
/// is kept so the OS can reap the thread; the receiver is used to impose a
/// deadline on the drain via `recv_timeout`.
fn spawn_stream_reader_channel<T: Read + Send + 'static>(
    stream: T,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<String, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = read_stream(stream);
        // Ignore send errors: the receiver may have been abandoned on grace
        // expiry, which is expected.
        let _ = tx.send(result);
    });
    (handle, rx)
}

/// Spawn a latency-progress stream reader that delivers over a channel.
fn spawn_latency_stream_reader_channel<T: Read + Send + 'static>(
    stream: T,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<String, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = read_stream_with_latency_progress(stream);
        let _ = tx.send(result);
    });
    (handle, rx)
}

/// Drain a stream reader within a bounded grace period.
///
/// Used after a process-group kill to guard against escaped descendants that
/// still hold the pipe write-end open.  If `grace` elapses before the reader
/// finishes, the `JoinHandle` is leaked (the thread will unblock when the
/// process eventually exits or the OS reclaims the fd) and the function returns
/// whatever was captured up to the kill, prefixed with a diagnostic message.
/// The caller should check `timed_out` and propagate the diagnostic
/// appropriately; it is not an error to abandon the drain.
fn drain_stream_reader_bounded(
    rx: mpsc::Receiver<Result<String, String>>,
    // The handle is intentionally held until after recv_timeout so the thread
    // stays alive while we wait.  On grace expiry we drop it; the thread
    // continues in the background until the fd closes.
    _handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
) -> Result<String, String> {
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // A descendant escaped the process-group kill and still holds the
            // pipe open.  Return an empty string with a diagnostic; the caller
            // already knows the process timed out.
            Ok(format!(
                "[ripr-xtask: {stream_name} drain exceeded post-kill grace \
                 ({grace_secs}s) for {error_context}; a descendant process \
                 escaped group-kill and kept the pipe open — output truncated]",
                grace_secs = grace.as_secs(),
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "{stream_name} reader thread disconnected while running {error_context}"
        )),
    }
}

/// Drain a file-writer thread within a bounded grace period.  Returns 0 bytes
/// on grace expiry (the partial file content written before the kill remains).
fn drain_file_writer_bounded(
    rx: mpsc::Receiver<Result<usize, String>>,
    _handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
) -> Result<usize, String> {
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(0),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "{stream_name} writer thread disconnected while running {error_context}"
        )),
    }
}

/// Spawn a stream-to-file writer that delivers its byte count over a channel.
fn spawn_stream_file_writer_channel<T: Read + Send + 'static>(
    stream: T,
    file: fs::File,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<usize, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = stream_to_file(stream, file);
        let _ = tx.send(result);
    });
    (handle, rx)
}

#[cfg(test)]
mod tests {
    use super::{
        CapturedOutput, POST_KILL_DRAIN_GRACE, capture_output, capture_output_with_timeout,
        capture_stdout_to_file_with_timeout, command_success_owned, drain_stream_reader_bounded,
        read_stream_with_latency_progress, run, run_in_dir, run_output, run_output_optional,
        run_output_owned, run_owned, spawn_stream_reader_channel, terminate_after_timeout,
        timeout_was_enforced,
    };
    use std::fs;
    use std::io::{Cursor, Read};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    type TestCommand = (String, Vec<String>, Vec<(String, String)>);

    #[test]
    fn run_reports_success_and_failure_status() -> Result<(), String> {
        let status = run("rustc", &["--version"])?;
        if !status.success() {
            return Err("rustc --version should succeed".to_string());
        }

        let Err(err) = run("rustc", &["--ripr-invalid-test-flag"]) else {
            return Err("invalid rustc flag should fail".to_string());
        };
        if !err.contains("failed with") {
            return Err(format!("failure message should include status: {err}"));
        }
        Ok(())
    }

    #[test]
    fn owned_run_helpers_report_success_and_failure_status() -> Result<(), String> {
        let version_args = vec!["--version".to_string()];
        if !command_success_owned("rustc", &version_args)? {
            return Err("rustc --version should report success".to_string());
        }
        run_owned("rustc", &version_args)?;

        let bad_args = vec!["--ripr-invalid-test-flag".to_string()];
        if command_success_owned("rustc", &bad_args)? {
            return Err("invalid rustc flag should report failure".to_string());
        }
        let Err(err) = run_owned("rustc", &bad_args) else {
            return Err("invalid rustc flag should fail".to_string());
        };
        if !err.contains("failed with") {
            return Err(format!("failure message should include status: {err}"));
        }
        Ok(())
    }

    #[test]
    fn run_in_dir_reports_success_and_failure_with_cwd() -> Result<(), String> {
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
        let status = run_in_dir(Path::new("rustc"), &["--version"], cwd)?;
        if !status.success() {
            return Err("rustc --version should succeed".to_string());
        }

        let Err(err) = run_in_dir(Path::new("rustc"), &["--ripr-invalid-test-flag"], cwd) else {
            return Err("invalid rustc flag should fail".to_string());
        };
        if !err.contains("failed with") || !err.contains(&cwd.display().to_string()) {
            return Err(format!(
                "failure message should include status and cwd: {err}"
            ));
        }
        Ok(())
    }

    #[test]
    fn run_output_reports_stdout_and_failure() -> Result<(), String> {
        let stdout = run_output("rustc", &["--version"])?;
        if !stdout.contains("rustc") {
            return Err(format!("rustc version output should name rustc: {stdout}"));
        }

        let Err(err) = run_output("rustc", &["--ripr-invalid-test-flag"]) else {
            return Err("invalid rustc flag should fail".to_string());
        };
        if !err.contains("failed with") {
            return Err(format!("failure message should include status: {err}"));
        }
        Ok(())
    }

    #[test]
    fn run_output_owned_includes_stderr_on_failure() -> Result<(), String> {
        let args = vec!["--version".to_string()];
        let stdout = run_output_owned("rustc", &args)?;
        if !stdout.contains("rustc") {
            return Err(format!("rustc version output should name rustc: {stdout}"));
        }

        let bad_args = vec!["--ripr-invalid-test-flag".to_string()];
        let Err(err) = run_output_owned("rustc", &bad_args) else {
            return Err("invalid rustc flag should fail".to_string());
        };
        for expected in ["stdout:", "stderr:", "failed with"] {
            if !err.contains(expected) {
                return Err(format!("failure message should include {expected}: {err}"));
            }
        }
        Ok(())
    }

    #[test]
    fn run_output_optional_returns_empty_for_failure() -> Result<(), String> {
        let stdout = run_output_optional("rustc", &["--version"])?;
        if !stdout.contains("rustc") {
            return Err(format!("rustc version output should name rustc: {stdout}"));
        }

        let empty = run_output_optional("rustc", &["--ripr-invalid-test-flag"])?;
        if !empty.is_empty() {
            return Err(format!("failed optional output should be empty: {empty}"));
        }
        Ok(())
    }

    #[test]
    fn capture_output_returns_status_stdout_and_stderr() -> Result<(), String> {
        let CapturedOutput {
            status,
            stdout,
            stderr,
        } = capture_output("rustc", &["--version"], "rustc version")?;

        if !status.success() {
            return Err("rustc --version should succeed".to_string());
        }
        if !stdout.contains("rustc") {
            return Err(format!("captured stdout should name rustc: {stdout}"));
        }
        if !stderr.is_empty() {
            return Err(format!("captured stderr should be empty: {stderr}"));
        }
        Ok(())
    }

    #[test]
    fn capture_output_with_timeout_reports_completed_process() -> Result<(), String> {
        let args = vec!["--version".to_string()];
        let output = capture_output_with_timeout(
            "rustc",
            &args,
            &[],
            Duration::from_secs(30),
            "rustc version",
        )?;

        if output.timed_out {
            return Err("rustc --version should not time out".to_string());
        }
        if !output.status.is_some_and(|status| status.success()) {
            return Err("rustc --version should succeed".to_string());
        }
        if !output.stdout.contains("rustc") {
            return Err(format!(
                "captured stdout should name rustc: {}",
                output.stdout
            ));
        }
        Ok(())
    }

    #[test]
    fn capture_output_with_timeout_reports_timed_out_process() -> Result<(), String> {
        let (program, args, envs) = long_running_command()?;
        let env_refs = envs
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let output = capture_output_with_timeout(
            &program,
            &args,
            &env_refs,
            Duration::from_millis(100),
            "long-running command",
        )?;

        assert!(output.timed_out, "long-running command should time out");
        #[cfg(unix)]
        assert!(
            output.status.is_some(),
            "timed-out long-running command should report a process status"
        );
        Ok(())
    }

    #[cfg(unix)]
    fn long_running_command() -> Result<TestCommand, String> {
        Ok((
            "sh".to_string(),
            vec!["-c".to_string(), "sleep 30".to_string()],
            Vec::new(),
        ))
    }

    #[cfg(windows)]
    fn long_running_command() -> Result<TestCommand, String> {
        Ok((
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
            Vec::new(),
        ))
    }

    #[cfg(unix)]
    #[test]
    fn capture_output_with_timeout_terminates_pipe_inheriting_descendants() -> Result<(), String> {
        let args = vec!["-c".to_string(), "sleep 30 & wait".to_string()];
        // The timeout must comfortably exceed the time for `sh` to fork
        // `sleep 30` INTO its process group, otherwise the group-kill on
        // timeout can race a not-yet-grouped descendant: the descendant
        // survives, keeps the inherited stdout pipe open, and `read_to_end`
        // hangs. Under parallel test load process startup can take hundreds of
        // ms, so a tight (100 ms) timeout flaked (#1022). Five seconds is a
        // generous, deterministic margin — do not tighten without re-checking
        // this race.
        let output = capture_output_with_timeout(
            "sh",
            &args,
            &[],
            Duration::from_secs(5),
            "pipe-inheriting descendant",
        )?;

        assert!(
            output.timed_out,
            "pipe-inheriting descendant should time out"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn capture_output_with_timeout_terminates_pipe_inheriting_descendants() -> Result<(), String> {
        let marker = std::env::temp_dir().join(format!(
            "ripr-xtask-pipe-descendant-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let args = vec![
            "/C".to_string(),
            format!(
                "ping -n 8 127.0.0.1 & echo alive > \"{}\"",
                marker.display()
            ),
        ];
        // Same race as the unix variant: give `cmd` ample time to spawn the
        // `ping` descendant before the timeout's taskkill /T fires, so the
        // tree-kill reliably catches it under parallel load (#1022).
        let output = capture_output_with_timeout(
            "cmd",
            &args,
            &[],
            Duration::from_secs(5),
            "pipe-inheriting descendant",
        )?;

        assert!(
            output.timed_out,
            "pipe-inheriting descendant should time out"
        );
        if marker.exists() {
            let _ = fs::remove_file(&marker);
            return Err("timed-out process tree should not run its continuation".to_string());
        }
        Ok(())
    }

    #[test]
    fn capture_stdout_to_file_with_timeout_streams_stdout_to_file() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-xtask-stdout-file-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::write(&path, "stale output")
            .map_err(|err| format!("failed to write stale stdout file: {err}"))?;
        let args = vec!["--version".to_string()];
        let output = capture_stdout_to_file_with_timeout(
            "rustc",
            &args,
            &[],
            &path,
            Duration::from_secs(30),
            "rustc version",
        )?;

        if output.timed_out {
            return Err("rustc --version should not time out".to_string());
        }
        if !output.status.is_some_and(|status| status.success()) {
            return Err("rustc --version should succeed".to_string());
        }
        if output.stdout_bytes == 0 {
            return Err("streamed stdout should report bytes".to_string());
        }
        let captured = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read streamed stdout file: {err}"))?;
        let parent = path
            .parent()
            .ok_or_else(|| "streamed stdout path should have a parent".to_string())?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "streamed stdout path should have a UTF-8 file name".to_string())?;
        let temp_prefix = format!(".{file_name}.");
        let leaked_temp = fs::read_dir(parent)
            .map_err(|err| format!("failed to inspect streamed stdout parent: {err}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| name.starts_with(&temp_prefix) && name.ends_with(".tmp"));
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove streamed stdout file: {err}"))?;
        if let Some(leaked_temp) = leaked_temp {
            return Err(format!(
                "streamed stdout should publish through temp file without leaving {leaked_temp}"
            ));
        }
        if captured.contains("stale output") {
            return Err(format!(
                "captured stdout should overwrite stale file contents: {captured}"
            ));
        }
        if !captured.contains("rustc") {
            return Err(format!("captured stdout should name rustc: {captured}"));
        }
        Ok(())
    }

    #[test]
    fn latency_progress_reader_preserves_captured_stderr() -> Result<(), String> {
        let stderr = "first\nripr_repo_exposure_latency phase=evidence_for_seams status=start duration_ms=0\nlast\n";
        let captured = read_stream_with_latency_progress(Cursor::new(stderr.as_bytes()))?;
        assert_eq!(captured, stderr);
        Ok(())
    }

    #[test]
    fn terminate_after_timeout_returns_false_for_already_finished_child() -> Result<(), String> {
        let mut child = Command::new("rustc")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("spawn rustc version: {err}"))?;

        loop {
            if child
                .try_wait()
                .map_err(|err| format!("poll rustc version: {err}"))?
                .is_some()
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let termination_requested = terminate_after_timeout(&mut child, "rustc version")?;
        let status = child
            .wait()
            .map_err(|err| format!("wait for rustc version: {err}"))?;
        let timed_out = timeout_was_enforced(termination_requested, &status);
        if timed_out {
            return Err("finished process should not be reported as timed out".to_string());
        }
        Ok(())
    }

    #[test]
    fn timeout_was_enforced_reports_requested_termination() -> Result<(), String> {
        let success = capture_output("rustc", &["--version"], "rustc version")?.status;
        let failure =
            capture_output("rustc", &["--ripr-invalid-test-flag"], "rustc invalid flag")?.status;

        if !timeout_was_enforced(true, &success) {
            return Err("requested termination should be reported as timeout".to_string());
        }
        if timeout_was_enforced(false, &failure) {
            return Err("failure without termination should not be a timeout".to_string());
        }
        if !timeout_was_enforced(true, &failure) {
            return Err("terminated failure should be treated as timeout".to_string());
        }
        Ok(())
    }

    /// A `Read` implementation that blocks indefinitely on every `read` call.
    ///
    /// This simulates a pipe whose write-end is held open by a descendant that
    /// escaped the process-group kill.  It is used to exercise the
    /// `drain_stream_reader_bounded` grace-timeout path at the unit level
    /// without spawning a real process tree.
    ///
    /// Note on Windows/platform portability: spawning a real grandchild that
    /// keeps a pipe open across a `taskkill /T /F` is inherently racy and
    /// unreliable in CI, so this unit-level seam test is the authoritative check
    /// for the bounded-drain guarantee on all platforms.
    struct BlockingRead {
        /// Receiving on this channel blocks until the sender half is dropped,
        /// i.e. forever from the `Read` side.
        _park: mpsc::Receiver<()>,
    }

    impl BlockingRead {
        fn new() -> (Self, mpsc::SyncSender<()>) {
            let (tx, rx) = mpsc::sync_channel(0);
            (BlockingRead { _park: rx }, tx)
        }
    }

    impl Read for BlockingRead {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            // Block until the sender half (held by the test to keep the "pipe"
            // open) is dropped.  A real escaped descendant behaves identically:
            // it holds the write-end of the inherited pipe, and `read_to_end`
            // on the read-end never returns until that fd is closed.
            match self._park.recv() {
                Ok(()) | Err(mpsc::RecvError) => Ok(0),
            }
        }
    }

    /// The bounded-drain path must return within `grace + slack` even when the
    /// reader thread is permanently blocked (simulating an escaped descendant
    /// holding the pipe write-end open after a process-group kill).
    ///
    /// Uses a short synthetic grace to keep the test fast.  The production
    /// `POST_KILL_DRAIN_GRACE` constant is also verified to equal 5 s so the
    /// actual timeout is deterministic in CI.
    #[test]
    fn drain_stream_reader_bounded_returns_within_grace_when_pipe_stays_open() -> Result<(), String>
    {
        // Sanity-check the production constant so reviewers know what the real
        // bound is.
        if POST_KILL_DRAIN_GRACE != Duration::from_secs(5) {
            return Err(format!(
                "POST_KILL_DRAIN_GRACE should be 5 s; got {:?}",
                POST_KILL_DRAIN_GRACE
            ));
        }

        // Use a short synthetic grace (200 ms) to keep the test fast while
        // still proving the timeout fires.
        let test_grace = Duration::from_millis(200);
        // Allow up to 2x the grace as wall-clock slack for slow CI machines.
        let wall_limit = test_grace * 2;

        let (blocking, _keeper) = BlockingRead::new();
        // _keeper is kept alive so the BlockingRead::read never returns EOF —
        // it stays blocked for the entire test, simulating the escaped
        // descendant scenario.

        let (handle, rx) = spawn_stream_reader_channel(blocking);

        let started = Instant::now();
        let result = drain_stream_reader_bounded(rx, handle, test_grace, "stdout", "test-context")?;
        let elapsed = started.elapsed();

        // The function must have returned (not hung).
        if elapsed > wall_limit {
            return Err(format!("drain took {elapsed:?}, expected < {wall_limit:?}"));
        }

        // The result must contain the diagnostic message indicating output was
        // truncated due to the escaped-descendant scenario.
        if !result.contains("drain exceeded post-kill grace") {
            return Err(format!(
                "truncated drain should include diagnostic message; got: {result:?}"
            ));
        }
        if !result.contains("test-context") {
            return Err(format!(
                "diagnostic message should name the error context; got: {result:?}"
            ));
        }

        Ok(())
    }
}
