use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    eprintln!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
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
    let stdout_reader = thread::spawn(move || read_stream(stdout));
    let stderr_reader = thread::spawn(move || {
        if echo_latency_trace {
            read_stream_with_latency_progress(stderr)
        } else {
            read_stream(stderr)
        }
    });

    let wait_outcome = wait_for_child_with_timeout(&mut child, started, timeout, error_context)?;
    let stdout = join_stream_reader(stdout_reader, "stdout", error_context)?;
    let stderr = join_stream_reader(stderr_reader, "stderr", error_context)?;
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
    let stdout_file = fs::File::create(stdout_path).map_err(|err| {
        format!(
            "failed to create stdout file {} for {error_context}: {err}",
            stdout_path.display()
        )
    })?;
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped());
    configure_timed_child_command(&mut command);
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = command
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
    let stdout_writer = thread::spawn(move || stream_to_file(stdout, stdout_file));
    let stderr_reader = thread::spawn(move || {
        if echo_latency_trace {
            read_stream_with_latency_progress(stderr)
        } else {
            read_stream(stderr)
        }
    });

    let wait_outcome = wait_for_child_with_timeout(&mut child, started, timeout, error_context)?;
    let stdout_bytes = join_stream_file_writer(stdout_writer, "stdout", error_context)?;
    let stderr = join_stream_reader(stderr_reader, "stderr", error_context)?;
    Ok(TimedFileOutput {
        status: Some(wait_outcome.status),
        stderr,
        duration: wait_outcome.duration,
        timed_out: wait_outcome.timed_out,
        stdout_bytes,
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

fn join_stream_file_writer(
    writer: thread::JoinHandle<Result<usize, String>>,
    stream_name: &str,
    error_context: &str,
) -> Result<usize, String> {
    match writer.join() {
        Ok(result) => result,
        Err(_) => Err(format!(
            "{stream_name} writer thread failed while running {error_context}"
        )),
    }
}

fn join_stream_reader(
    reader: thread::JoinHandle<Result<String, String>>,
    stream_name: &str,
    error_context: &str,
) -> Result<String, String> {
    match reader.join() {
        Ok(result) => result,
        Err(_) => Err(format!(
            "{stream_name} reader thread failed while running {error_context}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CapturedOutput, capture_output, capture_output_with_timeout,
        capture_stdout_to_file_with_timeout, command_success_owned,
        read_stream_with_latency_progress, run, run_in_dir, run_output, run_output_optional,
        run_output_owned, run_owned, terminate_after_timeout, timeout_was_enforced,
    };
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

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
        let (program, args) = long_running_command();
        let output = capture_output_with_timeout(
            program,
            &args,
            &[],
            Duration::from_millis(100),
            "long-running command",
        )?;

        assert!(output.timed_out, "long-running command should time out");
        assert!(
            !output.status.is_some_and(|status| status.success()),
            "timed-out long-running command should not exit successfully"
        );
        Ok(())
    }

    #[cfg(unix)]
    fn long_running_command() -> (&'static str, Vec<String>) {
        ("sh", vec!["-c".to_string(), "sleep 30".to_string()])
    }

    #[cfg(windows)]
    fn long_running_command() -> (&'static str, Vec<String>) {
        (
            "cmd",
            vec!["/C".to_string(), "ping -n 30 127.0.0.1 >NUL".to_string()],
        )
    }

    #[cfg(unix)]
    #[test]
    fn capture_output_with_timeout_terminates_pipe_inheriting_descendants() -> Result<(), String> {
        let args = vec!["-c".to_string(), "sleep 30 & wait".to_string()];
        let output = capture_output_with_timeout(
            "sh",
            &args,
            &[],
            Duration::from_millis(100),
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
        let started = std::time::Instant::now();
        let current_exe =
            std::env::current_exe().map_err(|err| format!("locate current test binary: {err}"))?;
        let current_exe = current_exe.to_string_lossy().into_owned();
        let args = vec![
            "--exact".to_string(),
            "run::tests::pipe_inheriting_descendant_helper".to_string(),
            "--nocapture".to_string(),
        ];
        let output = capture_output_with_timeout(
            &current_exe,
            &args,
            &[("RIPR_XTASK_PIPE_DESCENDANT_HELPER", "1")],
            Duration::from_millis(100),
            "pipe-inheriting descendant",
        )?;

        assert!(
            output.timed_out,
            "pipe-inheriting descendant should time out"
        );
        assert!(
            started.elapsed() < Duration::from_secs(45),
            "pipe-inheriting descendant should not keep captured pipes open"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn pipe_inheriting_descendant_helper() -> Result<(), String> {
        if std::env::var_os("RIPR_XTASK_PIPE_DESCENDANT_HELPER").is_none() {
            return Ok(());
        }

        let mut child = Command::new("cmd")
            .args(["/C", "ping -n 120 127.0.0.1"])
            .spawn()
            .map_err(|err| format!("spawn pipe-inheriting descendant: {err}"))?;
        thread::sleep(Duration::from_mins(2));
        let _ = child.wait();
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
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove streamed stdout file: {err}"))?;
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
}
