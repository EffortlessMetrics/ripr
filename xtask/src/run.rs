use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    let stdout_reader = thread::spawn(move || read_stream(stdout));
    let stderr_reader = thread::spawn(move || read_stream(stderr));

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to poll {error_context}: {err}"))?
        {
            let stdout = join_stream_reader(stdout_reader, "stdout", error_context)?;
            let stderr = join_stream_reader(stderr_reader, "stderr", error_context)?;
            return Ok(TimedOutput {
                status: Some(status),
                stdout,
                stderr,
                duration: started.elapsed(),
                timed_out: false,
            });
        }

        if started.elapsed() >= timeout {
            let termination_requested = terminate_after_timeout(&mut child, error_context)?;
            let status = child
                .wait()
                .map_err(|err| format!("failed to finish timed-out {error_context}: {err}"))?;
            let timed_out = timeout_was_enforced(termination_requested, &status);
            let stdout = join_stream_reader(stdout_reader, "stdout", error_context)?;
            let stderr = join_stream_reader(stderr_reader, "stderr", error_context)?;
            return Ok(TimedOutput {
                status: Some(status),
                stdout,
                stderr,
                duration: started.elapsed(),
                timed_out,
            });
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn timeout_was_enforced(termination_requested: bool, status: &ExitStatus) -> bool {
    termination_requested && !status.success()
}

fn terminate_after_timeout(child: &mut Child, error_context: &str) -> Result<bool, String> {
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

fn read_stream<T: Read>(mut stream: T) -> Result<String, String> {
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read process output: {err}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
        CapturedOutput, capture_output, capture_output_with_timeout, command_success_owned, run,
        run_in_dir, run_output, run_output_optional, run_output_owned, run_owned,
        terminate_after_timeout, timeout_was_enforced,
    };
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
        let cwd =
            std::env::current_dir().map_err(|err| format!("failed to read current dir: {err}"))?;
        let status = run_in_dir(Path::new("rustc"), &["--version"], &cwd)?;
        if !status.success() {
            return Err("rustc --version should succeed".to_string());
        }

        let Err(err) = run_in_dir(Path::new("rustc"), &["--ripr-invalid-test-flag"], &cwd) else {
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
            Duration::from_secs(5),
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
        let args = vec![
            "metadata".to_string(),
            "--no-deps".to_string(),
            "--format-version".to_string(),
            "1".to_string(),
        ];
        let output =
            capture_output_with_timeout("cargo", &args, &[], Duration::ZERO, "cargo metadata")?;

        assert!(output.timed_out, "cargo metadata should time out");
        assert!(
            !output.status.is_some_and(|status| status.success()),
            "timed-out cargo metadata should not exit successfully"
        );
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
    fn timeout_was_enforced_requires_termination_and_unsuccessful_status() -> Result<(), String> {
        let success = capture_output("rustc", &["--version"], "rustc version")?.status;
        let failure =
            capture_output("rustc", &["--ripr-invalid-test-flag"], "rustc invalid flag")?.status;

        if timeout_was_enforced(true, &success) {
            return Err("successful status should not be treated as enforced timeout".to_string());
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
