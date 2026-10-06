mod hard_enforcement_readiness;
mod trusted_storage;
pub(crate) use trusted_storage::TrustedStorageMonitor;

#[cfg(target_os = "linux")]
mod owned_capture_group;
#[cfg(target_os = "linux")]
pub(crate) use owned_capture_group::FilePolicyScope;
#[cfg(not(target_os = "linux"))]
pub(crate) struct FilePolicyScope;

/// Ordinary calls have no handoff and retain their original isolated captures.
pub(crate) fn file_policy_capture_scope() -> Result<Option<FilePolicyScope>, String> {
    #[cfg(target_os = "linux")]
    {
        FilePolicyScope::requested()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(None)
    }
}

/// Only the file-policy owner can select this literal Cargo route.
pub(crate) fn capture_file_policy_cargo(
    args: &[String],
    timeout: Duration,
    error_context: &str,
    scope: &FilePolicyScope,
    observation: Option<&crate::policy::phase_diagnostics::PhaseToken>,
) -> Result<TimedOutput, String> {
    capture_output_sampled_scoped(
        ("cargo", args, None),
        &[],
        (Some(timeout), false),
        error_context,
        observation,
        Some(scope),
    )
    .map(|(output, _)| output)
}

pub(crate) use hard_enforcement_readiness::hard_enforcement_readiness_report;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use ripr::process_owner::OwnedProcess;
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

/// Budget for confirming that a timed-out Unix process group has no live
/// members (#5382).
///
/// A recorded `timed_out` result is not proof that descendants died: the first
/// `kill -KILL -- -<pgid>` can race a fork (GNU `time` starting `ripr` is the
/// motivating case). After reaping the direct child, the timeout path polls
/// until the group is empty, re-sending SIGKILL each round, and refuses to
/// return `timed_out: true` if the group is still populated when this budget
/// expires. Two seconds is long enough for SIGKILL and a missed-fork retry,
/// and short enough that an unkillable leftover becomes an explicit error
/// instead of a silent timeout.
#[cfg(unix)]
const POST_KILL_GROUP_CONFIRM_GRACE: Duration = Duration::from_secs(2);

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(crate) struct CapturedOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) struct CapturedBytes {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProcessErrorKind {
    Launch,
    Exit,
}

#[derive(Debug)]
pub(crate) struct ProcessError {
    pub(crate) kind: ProcessErrorKind,
    pub(crate) message: String,
}

pub(crate) fn capture_process_output(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
) -> Result<Vec<u8>, ProcessError> {
    capture_process_output_isolated(program, args, &[], &[], envs)
}

pub(crate) fn capture_process_output_isolated(
    program: &str,
    args: &[String],
    inherited_envs: &[(&str, &str)],
    removed_envs: &[&str],
    envs: &[(&str, &str)],
) -> Result<Vec<u8>, ProcessError> {
    capture_process_output_in(program, args, None, inherited_envs, removed_envs, envs)
}

/// `capture_process_output_isolated` with an explicit working directory for the
/// child.
///
/// A caller that needs the child to resolve relative paths from somewhere other
/// than the xtask process's own directory uses this. It sets the directory on
/// the child only: `std::env::set_current_dir` is process-wide and the fixture
/// runs are rayon-parallel, so mutating the xtask process's directory would
/// race every sibling run.
pub(crate) fn capture_process_output_in(
    program: &str,
    args: &[String],
    current_dir: Option<&Path>,
    inherited_envs: &[(&str, &str)],
    removed_envs: &[&str],
    envs: &[(&str, &str)],
) -> Result<Vec<u8>, ProcessError> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    for (name, value) in inherited_envs {
        command.env(name, value);
    }
    for name in removed_envs {
        command.env_remove(name);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.output().map_err(|error| ProcessError {
        kind: ProcessErrorKind::Launch,
        message: format!("failed to launch {program}: {error}"),
    })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(ProcessError {
            kind: ProcessErrorKind::Exit,
            message: format!(
                "{program} {} failed with {}; stdout: {}; stderr: {}",
                args.join(" "),
                output.status,
                stdout,
                stderr
            ),
        })
    }
}

pub(crate) fn run_process_status(program: &str, args: &[String]) -> Result<(), ProcessError> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| ProcessError {
            kind: ProcessErrorKind::Launch,
            message: format!("failed to launch {program}: {error}"),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ProcessError {
            kind: ProcessErrorKind::Exit,
            message: format!("{program} {} failed with {status}", args.join(" ")),
        })
    }
}

pub(crate) struct TimedOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

pub(crate) struct TimedBytesOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
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

/// Bounded stdout/stderr captured from one timed command in an explicit working directory.
pub(crate) struct TimedBoundedOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr_truncated: bool,
}

struct BoundedBytes {
    bytes: Vec<u8>,
    truncated: bool,
}

struct WaitOutcome {
    status: ExitStatus,
    duration: Duration,
    timed_out: bool,
    /// Highest `VmHWM` observed while polling, when sampling was requested
    /// and the platform exposes it (Linux `/proc`).
    peak_rss_bytes: Option<u64>,
}

/// A timed child run with its sampled peak resident memory.
pub(crate) struct MeasuredOutput {
    pub(crate) output: TimedOutput,
    pub(crate) peak_rss_bytes: Option<u64>,
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
    run_process_status(program, args).map_err(|error| error.message)
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

/// `run_output` returning raw stdout bytes (#4006). Path inventories decode
/// through the shared NUL authority at the call site; `String::from_utf8_lossy`
/// here would collapse non-UTF-8 names before identity comparison.
/// Delegates to the shared capture spawn site, so this adds no new process
/// spawn for the process-policy gate; failure messages carry the shared
/// runner's stdout/stderr context instead of `run_output`'s terse form.
pub(crate) fn run_output_bytes(program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    capture_process_output_in(program, &owned, None, &[], &[], &[]).map_err(|error| error.message)
}

/// `run_output_optional` returning raw stdout bytes (#4006): success yields
/// stdout, exit failure yields empty output, launch failure still errors —
/// the exact `run_output_optional` contract, without lossy decoding and
/// without a new spawn site (see `run_output_bytes`).
pub(crate) fn run_output_optional_bytes(program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    match capture_process_output_in(program, &owned, None, &[], &[], &[]) {
        Ok(stdout) => Ok(stdout),
        Err(error) if matches!(error.kind, ProcessErrorKind::Exit) => Ok(Vec::new()),
        Err(error) => Err(error.message),
    }
}

pub(crate) fn run_output_owned(program: &str, args: &[String]) -> Result<String, String> {
    run_output_owned_with_envs(program, args, &[])
}

/// `run_output_owned` with an explicit working directory for the child.
///
/// A fixture render that must resolve relative paths from inside a packet tree
/// runs here rather than mutating the xtask process directory, which the
/// rayon-parallel fixture runs share.
pub(crate) fn run_output_owned_in(
    program: &str,
    args: &[String],
    current_dir: &Path,
) -> Result<String, String> {
    let output = capture_process_output_in(program, args, Some(current_dir), &[], &[], &[])
        .map_err(|error| error.message)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// `run_output_owned` with an explicit child environment overlay.
///
/// Callers that must pin a child's environment — for example a fixture run that
/// pins `RIPR_CACHE_DIR` so the cache it writes is the cache the caller cleared
/// (#3054) — use this instead of mutating the xtask process environment, which
/// the rayon-parallel fixture runs share.
pub(crate) fn run_output_owned_with_envs(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
) -> Result<String, String> {
    let output = capture_process_output(program, args, envs).map_err(|error| error.message)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// Default deadline for building the ripr binary inside PR-evidence commands.
///
/// The required Rust gates build the workspace before PR evidence runs, so
/// this build is incremental in CI; fifteen minutes stays generous for a cold
/// local build while still bounding a wedged toolchain (issue #2230).
pub(crate) const DEFAULT_TOOL_BUILD_TIMEOUT_SECS: u64 = 900;

/// Env override (seconds, positive integer) for the tool-build deadline.
pub(crate) const TOOL_BUILD_TIMEOUT_ENV: &str = "RIPR_TOOL_BUILD_TIMEOUT_SECS";

pub(crate) fn tool_build_timeout() -> Result<Duration, String> {
    env_timeout_secs(TOOL_BUILD_TIMEOUT_ENV, DEFAULT_TOOL_BUILD_TIMEOUT_SECS)
}

pub(crate) fn env_timeout_secs(name: &str, default_secs: u64) -> Result<Duration, String> {
    // A malformed override must fail fast, not silently fall back to the
    // default: NotPresent means "no override", anything else is validated.
    match std::env::var(name) {
        Ok(value) => {
            parse_env_timeout_secs(name, Some(&value), default_secs).map(Duration::from_secs)
        }
        Err(std::env::VarError::NotPresent) => Ok(Duration::from_secs(default_secs)),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!(
            "{name} must be valid UTF-8 text naming a positive integer"
        )),
    }
}

fn parse_env_timeout_secs(
    name: &str,
    value: Option<&str>,
    default_secs: u64,
) -> Result<u64, String> {
    let Some(value) = value else {
        return Ok(default_secs);
    };
    let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|err| format!("{name} must be a positive integer: {err}"))?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err(format!("{name} must be a positive integer"))
    }
}

/// Like `run_output_owned`, but bounds the child with the shared
/// timed-process machinery so a wedged build cannot hang the caller. A
/// timeout is a named result: the child process tree is terminated and the
/// error names the deadline and the calling context.
pub(crate) fn run_output_owned_with_timeout(
    program: &str,
    args: &[String],
    timeout: Duration,
    error_context: &str,
) -> Result<String, String> {
    let output = capture_output_with_timeout(program, args, &[], timeout, error_context)?;
    if output.timed_out {
        let seconds = timeout.as_secs();
        return Err(format!(
            "{error_context} timed out after {seconds} {}; the child process tree was terminated",
            if seconds == 1 { "second" } else { "seconds" }
        ));
    }
    let Some(status) = output.status else {
        return Err(format!("{error_context} did not report a process status"));
    };
    if status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{program} {} failed with {status}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            output.stdout.trim(),
            output.stderr.trim()
        ))
    }
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

pub(crate) fn capture_output_bytes(
    program: &str,
    args: &[&str],
    error_context: &str,
) -> Result<CapturedBytes, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    Ok(CapturedBytes {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

pub(crate) fn capture_output_in_dir(
    program: &str,
    args: &[String],
    cwd: &Path,
    error_context: &str,
) -> Result<CapturedOutput, String> {
    capture_output_in_dir_with_envs(program, args, cwd, error_context, &[], &[])
}

pub(crate) fn capture_output_in_dir_with_envs(
    program: &str,
    args: &[String],
    cwd: &Path,
    error_context: &str,
    envs: &[(&str, &str)],
    env_remove: &[&str],
) -> Result<CapturedOutput, String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    for (name, value) in envs {
        command.env(name, value);
    }
    for name in env_remove {
        command.env_remove(name);
    }
    let output = command
        .output()
        .map_err(|err| format!("failed to run {error_context} in {}: {err}", cwd.display()))?;
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
    capture_output_with_deadline(program, args, envs, Some(timeout), error_context)
}

/// `capture_output_with_timeout` in an explicit working directory that also
/// samples the child's peak resident memory (Linux `VmHWM`, polled with the
/// deadline loop, so the value is a lower bound). Other platforms report
/// `None`. Used by the developer-experience scoreboard.
pub(crate) fn capture_output_measured(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    envs: &[(&str, &str)],
    timeout: Duration,
    error_context: &str,
) -> Result<MeasuredOutput, String> {
    let (output, peak_rss_bytes) =
        capture_output_sampled(program, args, cwd, envs, Some(timeout), true, error_context)?;
    Ok(MeasuredOutput {
        output,
        peak_rss_bytes,
    })
}

/// `capture_output_with_timeout` with no per-step wall-clock cap.
///
/// For callers whose child may legitimately run longer than any fixed
/// host-dependent cutoff, such as a cold Cargo/Clippy build in precommit
/// (#4150).  The child is still spawned through the shared owned-subprocess
/// authority and the post-exit pipe drain remains bounded by
/// `POST_KILL_DRAIN_GRACE`, so a direct child that exits while a
/// pipe-inheriting descendant survives cannot block the caller for the
/// descendant's lifetime: the output is truncated with a named diagnostic
/// instead.  A genuinely hung child still waits until an operator cancels
/// the run; the hosted job bounds are the only automated stop.
pub(crate) fn capture_output_without_timeout(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
    error_context: &str,
) -> Result<TimedOutput, String> {
    capture_output_with_deadline(program, args, envs, None, error_context)
}

fn capture_output_with_deadline(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
    deadline: Option<Duration>,
    error_context: &str,
) -> Result<TimedOutput, String> {
    capture_output_sampled(program, args, None, envs, deadline, false, error_context)
        .map(|(output, _)| output)
}

pub(crate) fn capture_output_with_timeout_observed(
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
    timeout: Duration,
    error_context: &str,
    observation: &crate::policy::phase_diagnostics::PhaseToken,
) -> Result<TimedOutput, String> {
    capture_output_sampled_observed(
        (program, args, None),
        envs,
        (Some(timeout), false),
        error_context,
        Some(observation),
    )
    .map(|(output, _)| output)
}

fn capture_output_sampled(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    envs: &[(&str, &str)],
    deadline: Option<Duration>,
    sample_rss: bool,
    error_context: &str,
) -> Result<(TimedOutput, Option<u64>), String> {
    capture_output_sampled_observed(
        (program, args, cwd),
        envs,
        (deadline, sample_rss),
        error_context,
        None,
    )
}
fn capture_output_sampled_observed(
    source: (&str, &[String], Option<&Path>),
    envs: &[(&str, &str)],
    budget: (Option<Duration>, bool),
    error_context: &str,
    observation: Option<&crate::policy::phase_diagnostics::PhaseToken>,
) -> Result<(TimedOutput, Option<u64>), String> {
    capture_output_sampled_scoped(source, envs, budget, error_context, observation, None)
}
fn capture_output_sampled_scoped(
    source: (&str, &[String], Option<&Path>),
    envs: &[(&str, &str)],
    budget: (Option<Duration>, bool),
    error_context: &str,
    observation: Option<&crate::policy::phase_diagnostics::PhaseToken>,
    scope: Option<&FilePolicyScope>,
) -> Result<(TimedOutput, Option<u64>), String> {
    let (program, args, cwd) = source;
    let (deadline, sample_rss) = budget;
    let delegated = scope.is_some();
    #[cfg(target_os = "linux")]
    if let Some(scope) = scope {
        scope.verify(deadline.ok_or_else(|| {
            "owned file-policy inner deadline absent; spawn refused".to_string()
        })?)?;
    }
    #[cfg(not(target_os = "linux"))]
    if delegated {
        return Err("owned file-policy delegated captures are Linux-only".to_string());
    }
    let started = Instant::now();
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if !delegated {
        configure_timed_child_command(&mut command);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    // The timed-child spawn goes through the shared owned-subprocess
    // authority (#3803): on Windows the child is assigned to a
    // kill-on-termination Job Object before its user code runs, so the
    // timeout path below owns the whole tree instead of shelling out to
    // `taskkill /T /F`.
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = if delegated {
        OwnedProcess::spawn_with_bounded_drop(command)
    } else {
        OwnedProcess::spawn(command)
    }
    .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    #[cfg(target_os = "linux")]
    if let Some(scope) = scope {
        scope.verify_spawn(child.id())?;
    }
    let captured_observation = observation.map(|phase| phase.spawn(child.id()));
    let stdout = child
        .stdout_pipe()
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
    let stderr = child
        .stderr_pipe()
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
    } else if let Some(observation) = captured_observation {
        spawn_stream_reader_channel_observed(stderr, observation)
    } else {
        spawn_stream_reader_channel(stderr)
    };

    let wait_outcome = wait_for_child_sampled(
        &mut child,
        started,
        if delegated { None } else { deadline },
        sample_rss,
        error_context,
    )?;

    // Always use the bounded drain.  On a normal process exit the pipe
    // write-ends are already closed, so the reader threads finish promptly and
    // the grace timeout is never reached — behavior is identical to an
    // unbounded join.  On a timed-out kill the grace timeout caps the drain if
    // a descendant escaped the process-group kill and still holds the pipe open,
    // guaranteeing the function returns in bounded time regardless.  The same
    // grace bounds the no-deadline path when the direct child exits but a
    // pipe-inheriting descendant survives it.
    let (stdout, stderr) = if delegated {
        (
            drain_owned_stream_reader(stdout_rx, stdout_handle, "stdout", error_context)?,
            drain_owned_stream_reader(stderr_rx, stderr_handle, "stderr", error_context)?,
        )
    } else {
        (
            drain_stream_reader_bounded(
                stdout_rx,
                stdout_handle,
                POST_KILL_DRAIN_GRACE,
                "stdout",
                error_context,
            )?,
            drain_stream_reader_bounded(
                stderr_rx,
                stderr_handle,
                POST_KILL_DRAIN_GRACE,
                "stderr",
                error_context,
            )?,
        )
    };

    Ok((
        TimedOutput {
            status: Some(wait_outcome.status),
            stdout,
            stderr,
            duration: wait_outcome.duration,
            timed_out: wait_outcome.timed_out,
        },
        wait_outcome.peak_rss_bytes,
    ))
}

pub(crate) fn capture_bytes_in_dir_with_timeout(
    program: &Path,
    args: &[String],
    cwd: &Path,
    envs: &[(&str, &str)],
    env_remove: &[&str],
    timeout: Duration,
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    capture_bytes_with_input(
        program,
        args,
        (cwd, None),
        envs,
        env_remove,
        (timeout, None),
        error_context,
    )
}

/// Bounded binary input/output for Git's batch protocol. The existing process
/// owner retains child/descendant custody while stdin is written concurrently.
#[cfg(all(test, unix))]
pub(crate) fn capture_bytes_in_dir_with_input_timeout(
    program: &Path,
    args: &[String],
    cwd: &Path,
    input: &[u8],
    env_remove: &[&str],
    timeout: Duration,
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    capture_bytes_with_input(
        program,
        args,
        (cwd, Some(input)),
        &[],
        env_remove,
        (timeout, None),
        error_context,
    )
}

/// Explicit resource limits for selected-source Git capture. Other callers
/// retain their existing behavior; the owned process lifecycle is shared.
#[derive(Clone, Copy)]
pub(crate) struct ByteCaptureBudget {
    pub(crate) timeout: Duration,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
}

pub(crate) fn capture_bytes_in_dir_with_budget(
    program: &Path,
    args: &[String],
    source: (&Path, Option<&[u8]>),
    env_remove: &[&str],
    budget: ByteCaptureBudget,
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    capture_bytes_with_input(
        program,
        args,
        source,
        &[],
        env_remove,
        (budget.timeout, Some(budget)),
        error_context,
    )
}

fn capture_bytes_with_input(
    program: &Path,
    args: &[String],
    source: (&Path, Option<&[u8]>),
    envs: &[(&str, &str)],
    env_remove: &[&str],
    deadline: (Duration, Option<ByteCaptureBudget>),
    error_context: &str,
) -> Result<TimedBytesOutput, String> {
    let (cwd, input) = source;
    let (timeout, budget) = deadline;
    let started = Instant::now();
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    configure_timed_child_command(&mut command);
    for name in env_remove {
        command.env_remove(name);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    // Same owned-subprocess spawn as `capture_output_with_timeout` (#3803).
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = OwnedProcess::spawn(command)
        .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    let stdout = child
        .stdout_pipe()
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
    let stderr = child
        .stderr_pipe()
        .take()
        .ok_or_else(|| format!("failed to capture stderr for {error_context}"))?;
    let (stdout_handle, stdout_rx) =
        spawn_byte_reader_channel(stdout, budget.map(|v| ("stdout", v.stdout_bytes)));
    let (stderr_handle, stderr_rx) =
        spawn_byte_reader_channel(stderr, budget.map(|v| ("stderr", v.stderr_bytes)));
    let input_completion = if let Some(bytes) = input {
        let mut stdin = child
            .stdin_pipe()
            .take()
            .ok_or_else(|| format!("failed to capture stdin for {error_context}"))?;
        let bytes = bytes.to_vec();
        let (sender, receiver) = mpsc::channel();
        // Closing stdin after the write supplies batch EOF. No join can hold the
        // caller past the owned process deadline/drain grace.
        let _input_writer = thread::Builder::new()
            .name("bounded-process-stdin".to_string())
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
    let wait_outcome =
        wait_for_child_with_deadline(&mut child, started, Some(timeout), error_context)?;
    let stdout = drain_byte_reader_bounded(
        stdout_rx,
        stdout_handle,
        POST_KILL_DRAIN_GRACE,
        "stdout",
        error_context,
        budget.is_some(),
    )?;
    let stderr = drain_byte_reader_bounded(
        stderr_rx,
        stderr_handle,
        POST_KILL_DRAIN_GRACE,
        "stderr",
        error_context,
        budget.is_some(),
    )?;
    if let Some(receiver) = input_completion {
        let result = receiver
            .recv_timeout(POST_KILL_DRAIN_GRACE)
            .map_err(|error| format!("stdin completion for {error_context}: {error}"))?;
        if let Err(error) = result {
            // A deadline kill closes the reader while a large stdin write may
            // still be blocked. Preserve that observed timeout, rather than
            // replacing it with its expected BrokenPipe consequence. Early
            // exit and every other writer/drain failure still refuse.
            if !wait_outcome.timed_out || error.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(format!("write stdin for {error_context}: {error}"));
            }
        }
    }
    Ok(TimedBytesOutput {
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
    // Same owned-subprocess spawn as `capture_output_with_timeout` (#3803).
    command.stderr(Stdio::piped());
    let mut child = match OwnedProcess::spawn(command) {
        Ok(child) => child,
        Err(err) => {
            remove_stdout_capture_temp(&stdout_tmp_path);
            return Err(format!("failed to run {error_context}: {err}"));
        }
    };
    let stdout = match child.stdout_pipe().take() {
        Some(stdout) => stdout,
        None => {
            remove_stdout_capture_temp(&stdout_tmp_path);
            return Err(format!("failed to capture stdout for {error_context}"));
        }
    };
    let stderr = match child.stderr_pipe().take() {
        Some(stderr) => stderr,
        None => {
            remove_stdout_capture_temp(&stdout_tmp_path);
            return Err(format!("failed to capture stderr for {error_context}"));
        }
    };
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

    let wait_outcome =
        match wait_for_child_with_deadline(&mut child, started, Some(timeout), error_context) {
            Ok(wait_outcome) => wait_outcome,
            Err(err) => {
                remove_stdout_capture_temp(&stdout_tmp_path);
                return Err(err);
            }
        };

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
            remove_stdout_capture_temp(&stdout_tmp_path);
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

/// Run a command in `cwd` with the repository process-tree timeout authority
/// while retaining at most `max_stream_bytes` from each output stream.
pub(crate) fn capture_output_in_dir_with_timeout_bounded(
    program: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    cwd: &Path,
    timeout: Duration,
    max_stream_bytes: usize,
    error_context: &str,
) -> Result<TimedBoundedOutput, String> {
    capture_output_in_dir_with_timeout_bounded_observed(
        (program, args, cwd),
        envs,
        (timeout, max_stream_bytes),
        error_context,
        |_pid| {},
    )
}
pub(crate) fn capture_output_in_dir_with_timeout_bounded_observed(
    source: (&Path, &[String], &Path),
    envs: &[(&str, &str)],
    budget: (Duration, usize),
    error_context: &str,
    spawned: impl FnMut(u32),
) -> Result<TimedBoundedOutput, String> {
    capture_bounded_supervised(source, envs, budget, error_context, spawned, false, || {
        Ok(())
    })
    .and_then(TrustedPreparationCapture::checked)
}

fn capture_bounded_supervised(
    source: (&Path, &[String], &Path),
    envs: &[(&str, &str)],
    budget: (Duration, usize),
    error_context: &str,
    mut spawned: impl FnMut(u32),
    supervised: bool,
    mut monitor: impl FnMut() -> Result<(), String>,
) -> Result<TrustedPreparationCapture, String> {
    let (program, args, cwd) = source;
    let (timeout, max_stream_bytes) = budget;
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_timed_child_command(&mut command);
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = if supervised {
        OwnedProcess::spawn_with_bounded_drop(command)
    } else {
        OwnedProcess::spawn(command)
    }
    .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    spawned(child.id());
    let stdout = child
        .stdout_pipe()
        .take()
        .ok_or_else(|| format!("failed to capture stdout for {error_context}"))?;
    let stderr = child
        .stderr_pipe()
        .take()
        .ok_or_else(|| format!("failed to capture stderr for {error_context}"))?;
    let overflow = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (stdout_handle, stdout_rx) = if supervised {
        spawn_supervised_stream_reader(stdout, max_stream_bytes, overflow.clone())
    } else {
        spawn_bounded_stream_reader_channel(stdout, max_stream_bytes)
    };
    let (stderr_handle, stderr_rx) = if supervised {
        spawn_supervised_stream_reader(stderr, max_stream_bytes, overflow.clone())
    } else {
        spawn_bounded_stream_reader_channel(stderr, max_stream_bytes)
    };

    let wait_outcome = if supervised {
        wait_for_trusted_child(&mut child, started, timeout, || {
            refuse_preparation_overflow(&overflow)?;
            monitor()
        })
    } else {
        wait_for_child_with_deadline(&mut child, started, Some(timeout), error_context)
            .map(|outcome| (outcome, None))
    };
    let stdout = drain_capture_stream_reader(
        supervised,
        stdout_rx,
        stdout_handle,
        POST_KILL_DRAIN_GRACE,
        "stdout",
        error_context,
    );
    let stderr = drain_capture_stream_reader(
        supervised,
        stderr_rx,
        stderr_handle,
        POST_KILL_DRAIN_GRACE,
        "stderr",
        error_context,
    );
    finish_preparation_capture(wait_outcome, stdout, stderr)
}

fn finish_preparation_capture(
    wait_outcome: Result<(WaitOutcome, Option<String>), String>,
    stdout: Result<BoundedBytes, String>,
    stderr: Result<BoundedBytes, String>,
) -> Result<TrustedPreparationCapture, String> {
    // Try both bounded drains before deciding whether capture is complete.
    // Unknown drains keep capture unavailable and cannot erase the original
    // observation/cleanup failure or manufacture a retained output prefix.
    let drain_failures: Vec<_> = [stdout.as_ref().err(), stderr.as_ref().err()]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    let (outcome, failure_reason) = match wait_outcome {
        Ok(outcome) => outcome,
        Err(reason) => {
            let mut reasons = vec![reason];
            reasons.extend(drain_failures);
            return Err(reasons.join("; "));
        }
    };
    if !drain_failures.is_empty() {
        let mut reasons = failure_reason.into_iter().collect::<Vec<_>>();
        reasons.extend(drain_failures);
        return Err(reasons.join("; "));
    }
    let stdout = stdout?;
    let stderr = stderr?;
    Ok(TrustedPreparationCapture {
        output: TimedBoundedOutput {
            status: Some(outcome.status),
            stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
            duration: outcome.duration,
            timed_out: outcome.timed_out,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        },
        failure_reason,
    })
}

/// Captured diagnostics never imply successful preparation or policy acceptance.
pub(crate) struct TrustedPreparationCapture {
    pub(crate) output: TimedBoundedOutput,
    pub(crate) failure_reason: Option<String>,
}
impl TrustedPreparationCapture {
    fn checked(self) -> Result<TimedBoundedOutput, String> {
        match self.failure_reason {
            Some(reason) => Err(reason),
            None => Ok(self.output),
        }
    }
}

/// Trusted repository-pinned Cargo only. This supervises known storage paths
/// and owns cooperative Linux group settlement (Windows uses the Job owner).
/// It does not claim arbitrary-process containment or a hard storage quota.
pub(crate) fn capture_trusted_build_preparation(
    args: &[String],
    root: &Path,
    timeout: Duration,
    max_stream_bytes: usize,
    storage: &mut TrustedStorageMonitor,
) -> Result<TrustedPreparationCapture, String> {
    #[cfg(not(any(target_os = "linux", windows)))]
    return Err("trusted preparation requires a supported Linux group or Windows Job owner".into());
    #[cfg(any(target_os = "linux", windows))]
    {
        storage.check_now()?;
        let source = (Path::new("cargo"), args, root);
        let envs = [("CARGO_BUILD_JOBS", "1"), ("GIT_NO_REPLACE_OBJECTS", "1")];
        #[cfg(target_os = "linux")]
        let capture = capture_owned_group_supervised(
            source,
            &envs,
            (timeout, max_stream_bytes),
            "trusted materialized build preparation",
            |_pid| Ok(()),
            false,
            || storage.check(),
        );
        #[cfg(windows)]
        let capture = capture_bounded_supervised(
            source,
            &envs,
            (timeout, max_stream_bytes),
            "trusted materialized build preparation",
            |_pid| {},
            true,
            || storage.check(),
        );
        let mut capture = capture?;
        let postcondition = storage.check_now().and_then(|()| {
            if capture.output.stdout_truncated || capture.output.stderr_truncated {
                Err(
                    "trusted preparation output overflow or incomplete drain; build reuse refused"
                        .into(),
                )
            } else {
                Ok(())
            }
        });
        if let Err(reason) = postcondition {
            capture.failure_reason = Some(match capture.failure_reason {
                Some(prior) => format!("{prior}; {reason}"),
                None => reason,
            });
        }
        Ok(capture)
    }
}

fn refuse_preparation_overflow(flag: &std::sync::atomic::AtomicBool) -> Result<(), String> {
    if flag.load(std::sync::atomic::Ordering::Relaxed) {
        Err("trusted preparation output overflow; owned capture aborted".into())
    } else {
        Ok(())
    }
}

fn spawn_supervised_stream_reader<T: Read + Send + 'static>(
    mut stream: T,
    max_bytes: usize,
    overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<BoundedBytes, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = (|| {
            let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
            let mut buf = [0; 64 * 1024];
            let mut truncated = false;
            loop {
                let read = stream.read(&mut buf).map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                let keep = read.min(max_bytes.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&buf[..keep]);
                if keep < read {
                    truncated = true;
                    overflow.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Ok(BoundedBytes { bytes, truncated })
        })();
        let _ = tx.send(result);
    });
    (handle, rx)
}

#[cfg(test)]
#[test]
fn materialized_build_preparation_overflow_sets_abort_and_retains_only_bounded_prefix()
-> Result<(), String> {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (handle, rx) =
        spawn_supervised_stream_reader(std::io::Cursor::new(vec![42u8; 17]), 16, flag.clone());
    let output =
        drain_owned_stream_reader(rx, handle, "stdout", "bounded preparation reader control")?;
    assert_eq!(output.bytes, vec![42u8; 16]);
    assert!(output.truncated);
    assert!(refuse_preparation_overflow(&flag).is_err());
    Ok(())
}

#[cfg(test)]
#[test]
fn materialized_build_preparation_capture_preserves_original_cleanup_and_both_drain_errors() {
    let result = finish_preparation_capture(
        Err("original storage observation; owned cleanup unconfirmed".into()),
        Err("stdout drain unconfirmed".into()),
        Err("stderr drain unconfirmed".into()),
    );
    assert_eq!(result.err(), Some(
        "original storage observation; owned cleanup unconfirmed; stdout drain unconfirmed; stderr drain unconfirmed".into()
    ));
}

#[cfg(all(test, any(unix, windows)))]
#[test]
fn materialized_build_preparation_capture_retains_failed_observation_without_success_credit()
-> Result<(), String> {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    let captured = finish_preparation_capture(
        Ok((
            WaitOutcome {
                status: ExitStatus::from_raw(0),
                duration: Duration::from_millis(7),
                timed_out: false,
                peak_rss_bytes: None,
            },
            Some("original observed storage stop".into()),
        )),
        Ok(BoundedBytes {
            bytes: b"actual stdout".to_vec(),
            truncated: false,
        }),
        Ok(BoundedBytes {
            bytes: b"actual stderr".to_vec(),
            truncated: false,
        }),
    )?;
    assert!(
        captured
            .output
            .status
            .is_some_and(|status| status.success())
    );
    assert_eq!(captured.output.stdout, "actual stdout");
    assert_eq!(captured.output.stderr, "actual stderr");
    assert!(!captured.output.timed_out);
    assert_eq!(
        captured.checked().err(),
        Some("original observed storage stop".into())
    );
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
#[test]
fn materialized_build_preparation_linux_monitor_retains_actual_settled_group_failure()
-> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "ripr-preparation-group-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let result = (|| {
        let marker = root.join("owned-members");
        let marker_text = marker.to_string_lossy().into_owned();
        let args = vec!["-c".into(),
            "sleep 30 & member=$!; printf '%s\\n' 'actual-preparation-group-stdout'; printf '%s\\n' 'actual-preparation-group-stderr' >&2; printf '%s %s' \"$$\" \"$member\" > \"$RIPR_PREPARATION_MEMBER_MARKER\"; wait".into()];
        let mut leader = None;
        let captured = capture_owned_group_supervised(
            (Path::new("/bin/sh"), &args, &root),
            &[("RIPR_PREPARATION_MEMBER_MARKER", &marker_text)],
            (Duration::from_secs(5), 8192),
            "actual qualified Linux preparation observation control",
            |pid| {
                leader = Some(pid);
                Ok(())
            },
            false,
            || {
                if fs::read_to_string(&marker).ok().is_some_and(|text| {
                    text.split_whitespace()
                        .map(str::parse::<u32>)
                        .collect::<Result<Vec<_>, _>>()
                        .is_ok_and(|pids| pids.len() == 2 && pids.iter().all(|pid| *pid > 0))
                }) {
                    Err("synthetic observed preparation storage stop".into())
                } else {
                    Ok(())
                }
            },
        )?;
        assert!(captured.output.status.is_some());
        assert!(
            captured
                .output
                .stdout
                .contains("actual-preparation-group-stdout")
        );
        assert!(
            captured
                .output
                .stderr
                .contains("actual-preparation-group-stderr")
        );
        assert!(!captured.output.timed_out);
        assert!(
            captured
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("synthetic observed"))
        );
        let members = fs::read_to_string(&marker).map_err(|error| error.to_string())?;
        let pids = members
            .split_whitespace()
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(pids.len(), 2);
        assert_eq!(Some(pids[0]), leader);
        for pid in pids {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("owned group member unobservable: {error}")),
                Ok(stat) => {
                    let (_, fields) = stat
                        .rsplit_once(')')
                        .ok_or("owned member state unavailable")?;
                    if !matches!(fields.split_whitespace().next(), Some("Z" | "X" | "x")) {
                        return Err("owned preparation group member remains live".into());
                    }
                }
            }
        }
        assert_eq!(
            captured.checked().err(),
            Some("synthetic observed preparation storage stop".into())
        );
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
    result.and(cleanup)
}

#[cfg(all(test, windows))]
#[test]
fn materialized_build_preparation_windows_monitor_stops_the_actual_owned_job() -> Result<(), String>
{
    let root = std::env::temp_dir().join(format!(
        "ripr-preparation-job-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let result = (|| {
        let marker = root.join("owned-members");
        let marker_text = marker.to_string_lossy().into_owned();
        let script = "$child = Start-Process -WindowStyle Hidden -FilePath ($PSHOME + '\\powershell.exe') -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 30') -PassThru; Write-Output 'actual-bounded-preparation-prefix'; [Console]::Error.WriteLine('actual-bounded-preparation-stderr'); [IO.File]::WriteAllText($env:RIPR_PREPARATION_MEMBER_MARKER, [string]$PID + ' ' + [string]$child.Id); Start-Sleep -Seconds 30";
        let args = vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            script.into(),
        ];
        let mut leader = None;
        let captured = capture_bounded_supervised(
            (Path::new("powershell.exe"), &args, &root),
            &[("RIPR_PREPARATION_MEMBER_MARKER", &marker_text)],
            (Duration::from_secs(5), 8192),
            "actual owned Windows preparation monitor control",
            |pid| leader = Some(pid),
            true,
            || {
                let complete = fs::read_to_string(&marker).ok().is_some_and(|text| {
                    let pids = text
                        .split_whitespace()
                        .map(str::parse::<u32>)
                        .collect::<Result<Vec<_>, _>>();
                    pids.is_ok_and(|pids| pids.len() == 2 && pids.iter().all(|pid| *pid > 0))
                });
                if complete {
                    Err("synthetic observed preparation storage stop".into())
                } else {
                    Ok(())
                }
            },
        )?;
        assert!(
            captured
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("synthetic observed"))
        );
        assert!(
            captured
                .output
                .stdout
                .contains("actual-bounded-preparation-prefix")
        );
        assert!(!captured.output.timed_out);
        assert!(captured.output.status.is_some());
        assert!(
            captured
                .output
                .stderr
                .contains("actual-bounded-preparation-stderr")
        );
        let members = fs::read_to_string(&marker).map_err(|error| error.to_string())?;
        let pids = members
            .split_whitespace()
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(pids.len(), 2);
        assert_eq!(Some(pids[0]), leader);
        let verify_args = vec![
            "-NoProfile".into(), "-NonInteractive".into(), "-Command".into(),
            "foreach ($ownedMemberId in $env:RIPR_PREPARATION_MEMBER_IDS.Split(' ')) { try { $observed = Get-Process -Id ([int]$ownedMemberId) -ErrorAction Stop; if (-not $observed.HasExited) { throw 'owned preparation member remains live' } } catch { if ($_.FullyQualifiedErrorId -notlike 'NoProcessFoundForGivenId*') { throw } } }; Write-Output 'owned-preparation-members-settled'".into(),
        ];
        let observed = capture_output_in_dir_with_timeout_bounded(
            Path::new("powershell.exe"),
            &verify_args,
            &[("RIPR_PREPARATION_MEMBER_IDS", members.trim())],
            &root,
            Duration::from_secs(5),
            8192,
            "read-only actual owned Job member observation",
        )?;
        if observed.timed_out
            || observed.stdout_truncated
            || observed.stderr_truncated
            || !observed.status.is_some_and(|status| status.success())
            || !observed
                .stdout
                .contains("owned-preparation-members-settled")
        {
            return Err("owned preparation Job member remains live or unobservable".into());
        }
        assert_eq!(
            captured.checked().err(),
            Some("synthetic observed preparation storage stop".into())
        );
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
    result.and(cleanup)
}

fn wait_for_trusted_child(
    child: &mut OwnedProcess,
    started: Instant,
    timeout: Duration,
    mut monitor: impl FnMut() -> Result<(), String>,
) -> Result<(WaitOutcome, Option<String>), String> {
    loop {
        if let Err(reason) = monitor() {
            return match child.terminate_tree() {
                Ok(()) => retain_settled_observation_failure(child, started, reason, "owned"),
                Err(cleanup) => Err(format!("{reason}; owned cleanup unconfirmed: {cleanup}")),
            };
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                let reason =
                    format!("trusted preparation primary observation unavailable: {error}");
                return match child.terminate_tree() {
                    Ok(()) => Err(reason),
                    Err(cleanup) => Err(format!("{reason}; owned cleanup unconfirmed: {cleanup}")),
                };
            }
        };
        let expired = started.elapsed() >= timeout;
        if status.is_some() || expired {
            child.terminate_tree()?;
            let status = status
                .or(child.try_wait().map_err(|error| error.to_string())?)
                .ok_or("trusted preparation primary reap unavailable")?;
            return Ok((
                WaitOutcome {
                    status,
                    duration: started.elapsed(),
                    timed_out: expired,
                    peak_rss_bytes: None,
                },
                None,
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Called only after the existing owner has explicitly settled its Job/group.
/// A reaped status retains that disposition, while the original observation
/// failure still refuses preparation. Later Drop cannot establish this result.
fn retain_settled_observation_failure(
    child: &mut OwnedProcess,
    started: Instant,
    reason: String,
    owner: &str,
) -> Result<(WaitOutcome, Option<String>), String> {
    child
        .try_wait()
        .map_err(|error| format!("{reason}; {owner} primary reap unavailable: {error}"))
        .and_then(|status| {
            status.ok_or_else(|| format!("{reason}; {owner} primary reap unavailable"))
        })
        .map(|status| {
            (
                WaitOutcome {
                    status,
                    duration: started.elapsed(),
                    timed_out: false,
                    peak_rss_bytes: None,
                },
                Some(reason),
            )
        })
}

/// The selected source-owned file-policy checker owns this cooperative Linux
/// group. Ordinary captures and Windows retain their original implementations.
pub(crate) fn capture_owned_file_policy_group(
    source: (&Path, &[String], &Path),
    envs: &[(&str, &str)],
    budget: (Duration, usize),
    error_context: &str,
    spawned: impl FnMut(u32),
) -> Result<TimedBoundedOutput, String> {
    #[cfg(target_os = "linux")]
    {
        let mut spawned = spawned;
        capture_owned_file_policy_group_checked(source, envs, budget, error_context, |pid| {
            spawned(pid);
            Ok(())
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        capture_output_in_dir_with_timeout_bounded_observed(
            source,
            envs,
            budget,
            error_context,
            spawned,
        )
    }
}

#[cfg(target_os = "linux")]
fn capture_owned_file_policy_group_checked(
    source: (&Path, &[String], &Path),
    envs: &[(&str, &str)],
    budget: (Duration, usize),
    error_context: &str,
    spawned: impl FnMut(u32) -> Result<(), String>,
) -> Result<TimedBoundedOutput, String> {
    capture_owned_group_supervised(
        source,
        envs,
        budget,
        error_context,
        spawned,
        true,
        || Ok(()),
    )
    .and_then(TrustedPreparationCapture::checked)
}

#[cfg(target_os = "linux")]
fn capture_owned_group_supervised(
    source: (&Path, &[String], &Path),
    envs: &[(&str, &str)],
    budget: (Duration, usize),
    error_context: &str,
    mut spawned: impl FnMut(u32) -> Result<(), String>,
    checker_handoff: bool,
    mut monitor: impl FnMut() -> Result<(), String>,
) -> Result<TrustedPreparationCapture, String> {
    let (program, args, cwd) = source;
    let (timeout, max_stream_bytes) = budget;
    let handoff = if checker_handoff {
        Some(owned_capture_group::ParentHandoff::new(timeout)?)
    } else {
        None
    };
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_timed_child_command(&mut command);
    for (name, value) in envs {
        command.env(name, value);
    }
    if let Some(handoff) = &handoff {
        command.env(owned_capture_group::ENV, handoff.value());
    }
    let child = OwnedProcess::spawn_with_bounded_drop(command)
        .map_err(|err| format!("failed to run {error_context}: {err}"))?;
    // Establish the actual unreaped leader lease before callbacks or pipes.
    let mut guard = owned_capture_group::OwnedCaptureGuard::new(child)?;
    let result = (|| {
        spawned(guard.child().id())?;
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
        let overflow = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (stdout_handle, stdout_rx) = if checker_handoff {
            spawn_bounded_stream_reader_channel(stdout, max_stream_bytes)
        } else {
            spawn_supervised_stream_reader(stdout, max_stream_bytes, overflow.clone())
        };
        let (stderr_handle, stderr_rx) = if checker_handoff {
            spawn_bounded_stream_reader_channel(stderr, max_stream_bytes)
        } else {
            spawn_supervised_stream_reader(stderr, max_stream_bytes, overflow.clone())
        };
        let outcome = if checker_handoff {
            guard.wait(started, timeout)
        } else {
            guard.wait_supervised(started, timeout, || {
                refuse_preparation_overflow(&overflow)?;
                monitor()
            })
        };
        let retained_outcome = match outcome {
            Ok(outcome) => Ok((outcome, None)),
            Err(reason) => match guard.abort() {
                Ok(()) => {
                    retain_settled_observation_failure(guard.child(), started, reason, "qualified")
                }
                Err(cleanup) => Err(format!(
                    "{reason}; qualified group cleanup unconfirmed: {cleanup}"
                )),
            },
        };
        let stdout = drain_owned_stream_reader(stdout_rx, stdout_handle, "stdout", error_context);
        let stderr = drain_owned_stream_reader(stderr_rx, stderr_handle, "stderr", error_context);
        finish_preparation_capture(retained_outcome, stdout, stderr)
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

/// A scoped drain can never become successful output or warmup credit.
fn drain_owned_stream_reader<T>(
    rx: mpsc::Receiver<Result<T, String>>,
    handle: thread::JoinHandle<()>,
    stream_name: &str,
    error_context: &str,
) -> Result<T, String> {
    drain_owned_stream_reader_with_grace(
        rx,
        handle,
        POST_KILL_DRAIN_GRACE,
        stream_name,
        error_context,
    )
}

fn drain_owned_stream_reader_with_grace<T>(
    rx: mpsc::Receiver<Result<T, String>>,
    _handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
) -> Result<T, String> {
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "owned {stream_name} drain incomplete for {error_context}; cleanup and warmup credit refused"
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "owned {stream_name} reader disconnected for {error_context}; cleanup and warmup credit refused"
        )),
    }
}

fn spawn_bounded_stream_reader_channel<T: Read + Send + 'static>(
    stream: T,
    max_bytes: usize,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<BoundedBytes, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = read_stream_bounded(stream, max_bytes);
        let _ = tx.send(result);
    });
    (handle, rx)
}

fn read_stream_bounded<T: Read>(mut stream: T, max_bytes: usize) -> Result<BoundedBytes, String> {
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut buf = [0u8; 64 * 1024];
    let mut truncated = false;
    loop {
        let read = stream
            .read(&mut buf)
            .map_err(|err| format!("failed to read bounded process output: {err}"))?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(bytes.len());
        let keep = remaining.min(read);
        bytes.extend_from_slice(&buf[..keep]);
        if keep < read {
            truncated = true;
        }
    }
    Ok(BoundedBytes { bytes, truncated })
}

fn drain_capture_stream_reader(
    supervised: bool,
    rx: mpsc::Receiver<Result<BoundedBytes, String>>,
    handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
) -> Result<BoundedBytes, String> {
    if supervised {
        drain_owned_stream_reader_with_grace(rx, handle, grace, stream_name, error_context)
    } else {
        drain_bounded_stream_reader(rx, handle, grace, stream_name, error_context)
    }
}

#[cfg(test)]
#[test]
fn materialized_build_preparation_supervised_drain_timeout_refuses_unknown_output() {
    let (tx, rx) = mpsc::channel::<Result<BoundedBytes, String>>();
    let handle = thread::spawn(|| {});
    let output = drain_capture_stream_reader(
        true,
        rx,
        handle,
        Duration::ZERO,
        "stdout",
        "trusted drain control",
    );
    assert_eq!(output.err(), Some(
        "owned stdout drain incomplete for trusted drain control; cleanup and warmup credit refused".into()
    ));
    drop(tx);
}

#[cfg(test)]
#[test]
fn materialized_build_preparation_supervised_drain_disconnect_refuses_unknown_output() {
    let (tx, rx) = mpsc::channel::<Result<BoundedBytes, String>>();
    drop(tx);
    let handle = thread::spawn(|| {});
    let output = drain_capture_stream_reader(
        true,
        rx,
        handle,
        Duration::ZERO,
        "stderr",
        "trusted drain control",
    );
    assert_eq!(output.err(), Some(
        "owned stderr reader disconnected for trusted drain control; cleanup and warmup credit refused".into()
    ));
}

fn drain_bounded_stream_reader(
    rx: mpsc::Receiver<Result<BoundedBytes, String>>,
    _handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
) -> Result<BoundedBytes, String> {
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(BoundedBytes {
            bytes: format!(
                "[ripr-xtask: {stream_name} drain exceeded post-kill grace ({}s) for {error_context}; output truncated]",
                grace.as_secs()
            )
            .into_bytes(),
            truncated: true,
        }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "{stream_name} reader thread disconnected while running {error_context}"
        )),
    }
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

fn remove_stdout_capture_temp(path: &Path) {
    let _ = fs::remove_file(path);
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

fn wait_for_child_with_deadline(
    child: &mut OwnedProcess,
    started: Instant,
    deadline: Option<Duration>,
    error_context: &str,
) -> Result<WaitOutcome, String> {
    wait_for_child_sampled(child, started, deadline, false, error_context)
}

fn wait_for_child_sampled(
    child: &mut OwnedProcess,
    started: Instant,
    deadline: Option<Duration>,
    sample_rss: bool,
    error_context: &str,
) -> Result<WaitOutcome, String> {
    let mut peak_rss_bytes = None;
    loop {
        // Sample before polling exit: once the child is reaped its /proc
        // entry is gone. VmHWM is the kernel's own high-water mark, so the
        // last sample before exit already covers earlier peaks.
        if sample_rss && let Some(bytes) = proc_peak_rss_bytes(child.id()) {
            peak_rss_bytes = Some(peak_rss_bytes.map_or(bytes, |seen: u64| seen.max(bytes)));
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to poll {error_context}: {err}"))?
        {
            return Ok(WaitOutcome {
                status,
                duration: started.elapsed(),
                timed_out: false,
                peak_rss_bytes,
            });
        }

        if let Some(timeout) = deadline
            && started.elapsed() >= timeout
        {
            #[cfg(unix)]
            let pgid = child.id();
            let termination_requested = terminate_after_timeout(child, error_context)?;
            let status = child
                .wait()
                .map_err(|err| format!("failed to finish timed-out {error_context}: {err}"))?;
            #[cfg(unix)]
            if termination_requested {
                // A timeout is only recorded after the Unix process group is
                // confirmed empty (or this call fails closed that it could
                // not). Windows Job Object termination already owns the tree.
                confirm_timed_process_group_gone(pgid, error_context)?;
            }
            return Ok(WaitOutcome {
                status,
                duration: started.elapsed(),
                timed_out: timeout_was_enforced(termination_requested, &status),
                peak_rss_bytes,
            });
        }

        thread::sleep(poll_interval(sample_rss));
    }
}

/// Measured runs poll finer so a sub-second command's wall time is not
/// rounded up to the next 100 ms tick; scoreboard timings depend on it.
pub(crate) fn poll_interval(sample_rss: bool) -> Duration {
    Duration::from_millis(if sample_rss { 10 } else { 100 })
}

/// Peak resident set size of a live process from `/proc/<pid>/status`
/// (`VmHWM`, reported in kB). `None` off Linux or once the process is gone.
fn proc_peak_rss_bytes(pid: u32) -> Option<u64> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    parse_vm_hwm_bytes(&status)
}

pub(crate) fn parse_vm_hwm_bytes(status: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kb = line
        .trim_start_matches("VmHWM:")
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    kb.checked_mul(1024)
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

fn terminate_after_timeout(child: &mut OwnedProcess, error_context: &str) -> Result<bool, String> {
    if child
        .try_wait()
        .map_err(|err| format!("failed to poll {error_context}: {err}"))?
        .is_some()
    {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        // Owned Job Object termination (#3803): one typed request kills the
        // whole assigned tree, and the direct child cannot linger past it
        // the way the `taskkill /T /F` report-before-exit race allowed, so
        // no exit-grace confirmation is needed before the caller's reap.
        match child.terminate_tree() {
            Ok(()) => Ok(true),
            Err(err) => {
                if child
                    .try_wait()
                    .map_err(|err| format!("failed to poll {error_context}: {err}"))?
                    .is_some()
                {
                    // The tree died in the race between the termination
                    // request and its failure report; still an enforced
                    // timeout.
                    Ok(true)
                } else {
                    Err(format!(
                        "failed to terminate timed-out {error_context}: {err}"
                    ))
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
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
                    Ok(tree_terminated)
                } else {
                    Err(format!(
                        "failed to terminate timed-out {error_context}: {kill_err}"
                    ))
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn terminate_timed_process_tree(child: &OwnedProcess) -> bool {
    signal_process_group(child.id(), "-KILL").is_ok_and(|status| status.success())
}

#[cfg(not(windows))]
fn signal_process_group(pgid: u32, signal: &str) -> std::io::Result<ExitStatus> {
    run_kill_on_group(pgid, signal).map(|output| output.status)
}

#[cfg(not(windows))]
fn run_kill_on_group(pgid: u32, signal: &str) -> std::io::Result<std::process::Output> {
    let group = format!("-{pgid}");
    Command::new("kill")
        .args([signal, "--", group.as_str()])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

/// After a timeout kill, confirm the child's Unix process group has no live
/// members, or fail closed that confirmation was impossible.
#[cfg(unix)]
fn confirm_timed_process_group_gone(pgid: u32, error_context: &str) -> Result<(), String> {
    confirm_process_group_gone(pgid, POST_KILL_GROUP_CONFIRM_GRACE, error_context)
}

#[cfg(unix)]
fn confirm_process_group_gone(
    pgid: u32,
    budget: Duration,
    error_context: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    loop {
        let members = process_group_live_members(pgid)?;
        if members.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{error_context} timed out but could not confirm process group {pgid} is gone; still running: {members:?}"
            ));
        }
        let _ = signal_process_group(pgid, "-KILL");
        thread::sleep(Duration::from_millis(50));
    }
}

/// Live members of `pgid`, fail-closed: a probe that cannot run is an error,
/// not an empty group. A complete Linux `/proc` scan of every readable
/// numeric pid is the authority for *which* PIDs are live, so SIGKILL
/// zombies are omitted and a live other-uid descendant is not hidden behind
/// a same-uid zombie. An empty complete scan still consults `kill -0`:
/// ESRCH or a successful probe (zombie) may confirm gone, but EPERM/unknown
/// must not. An incomplete scan (unreadable or unparseable `stat`) uses the
/// probe alone. The `kill` probe runs under `LC_ALL=C` so ESRCH text is
/// stable across caller locales.
#[cfg(unix)]
fn process_group_live_members(pgid: u32) -> Result<Vec<u32>, String> {
    #[cfg(target_os = "linux")]
    {
        let scan = scan_proc_pgrp(pgid);
        let probe = match &scan {
            Some(ProcGroupScan::Complete { live }) if !live.is_empty() => {
                Ok(GroupProbe::SignaledAlive)
            }
            _ => probe_process_group_alive(pgid),
        };
        members_from_group_scan(pgid, scan, probe)
    }
    #[cfg(not(target_os = "linux"))]
    {
        members_from_group_scan(pgid, None, probe_process_group_alive(pgid))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(any(unix, test))]
enum ProcGroupScan {
    Complete { live: Vec<u32> },
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(unix, test))]
enum GroupProbe {
    SignaledAlive,
    Gone,
    Inaccessible,
}

#[cfg(any(unix, test))]
fn members_from_group_scan(
    pgid: u32,
    scan: Option<ProcGroupScan>,
    probe: Result<GroupProbe, String>,
) -> Result<Vec<u32>, String> {
    match scan {
        Some(ProcGroupScan::Complete { live }) if !live.is_empty() => Ok(live),
        Some(ProcGroupScan::Complete { .. }) => match probe {
            Ok(GroupProbe::Gone | GroupProbe::SignaledAlive) => Ok(Vec::new()),
            Ok(GroupProbe::Inaccessible) => Ok(vec![pgid]),
            Err(err) => Err(err),
        },
        Some(ProcGroupScan::Incomplete) | None => match probe {
            Ok(GroupProbe::Gone) => Ok(Vec::new()),
            Ok(GroupProbe::SignaledAlive | GroupProbe::Inaccessible) => Ok(vec![pgid]),
            Err(err) => Err(err),
        },
    }
}

#[cfg(unix)]
fn probe_process_group_alive(pgid: u32) -> Result<GroupProbe, String> {
    let output = run_kill_on_group(pgid, "-0")
        .map_err(|err| format!("could not probe process group {pgid}: {err}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(classify_group_probe(output.status.success(), &stderr))
}

#[cfg(any(unix, test))]
fn classify_group_probe(success: bool, stderr: &str) -> GroupProbe {
    if success {
        return GroupProbe::SignaledAlive;
    }
    // Fail closed: only ESRCH is an empty group. EPERM and unknown kill
    // diagnostics mean a member may still exist.
    if stderr.contains("No such process") {
        GroupProbe::Gone
    } else {
        GroupProbe::Inaccessible
    }
}

/// Non-zombie PIDs whose `/proc/<pid>/stat` pgrp matches `pgid`, regardless
/// of uid. `None` if `/proc` cannot be listed. `Incomplete` if a numeric
/// entry exists but its `stat` is unreadable for a reason other than NotFound.
#[cfg(target_os = "linux")]
fn scan_proc_pgrp(pgid: u32) -> Option<ProcGroupScan> {
    let entries = fs::read_dir("/proc").ok()?;
    let mut live = Vec::new();
    let mut complete = true;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                complete = false;
                continue;
            }
        };
        let name = entry.file_name();
        let Some(pid_str) = name.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        match fs::read_to_string(entry.path().join("stat")) {
            Ok(stat) => match parse_stat_pgrp(&stat) {
                Some(ProcStatPgrp::Live { pgrp, .. }) if pgrp == pgid => live.push(pid),
                Some(_) => {}
                None => complete = false,
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => complete = false,
        }
    }
    Some(if complete {
        ProcGroupScan::Complete { live }
    } else {
        ProcGroupScan::Incomplete
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(target_os = "linux", test))]
enum ProcStatPgrp {
    Live { state: char, pgrp: u32 },
    NotLive,
}

/// `/proc/<pid>/stat` state and pgrp. `comm` may contain spaces and
/// parentheses, so this splits only after the last `)`. Zombies and dead
/// tasks are `NotLive`. Unparseable text is `None` so the scan stays
/// incomplete instead of pretending the member was gone.
#[cfg(any(target_os = "linux", test))]
fn parse_stat_pgrp(stat: &str) -> Option<ProcStatPgrp> {
    let close = stat.rfind(')')?;
    let rest = stat.get(close.saturating_add(1)..)?;
    let mut fields = rest.split_whitespace();
    let state = fields.next()?.chars().next()?;
    if matches!(state, 'Z' | 'X') {
        return Some(ProcStatPgrp::NotLive);
    }
    let _ppid = fields.next()?;
    let pgrp = fields.next()?.parse().ok()?;
    Some(ProcStatPgrp::Live { state, pgrp })
}

fn read_stream<T: Read>(mut stream: T) -> Result<String, String> {
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read process output: {err}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_stream_bytes<T: Read>(mut stream: T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read process output: {err}"))?;
    Ok(bytes)
}

fn read_stream_bytes_limited(
    stream: impl Read,
    name: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    stream
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read bounded {name}: {error}"))?;
    if bytes.len() > limit {
        return Err(format!("{name} exceeds its {limit}-byte output budget"));
    }
    Ok(bytes)
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
fn spawn_stream_reader_channel_observed<T: Read + Send + 'static>(
    mut stream: T,
    observation: crate::policy::phase_diagnostics::CaptureObservation,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<String, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        let result = loop {
            match stream.read(&mut chunk) {
                Ok(0) => break Ok(String::from_utf8_lossy(&bytes).into_owned()),
                Ok(count) => {
                    bytes.extend_from_slice(&chunk[..count]);
                    observation.tail(&chunk[..count]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => break Err(format!("failed to read process output: {error}")),
            }
        };
        let _send_result = tx.send(result);
    });
    (handle, rx)
}

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

fn spawn_byte_reader_channel<T: Read + Send + 'static>(
    stream: T,
    limit: Option<(&'static str, usize)>,
) -> (
    thread::JoinHandle<()>,
    mpsc::Receiver<Result<Vec<u8>, String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = match limit {
            Some((name, bytes)) => read_stream_bytes_limited(stream, name, bytes),
            None => read_stream_bytes(stream),
        };
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

fn drain_byte_reader_bounded(
    rx: mpsc::Receiver<Result<Vec<u8>, String>>,
    _handle: thread::JoinHandle<()>,
    grace: Duration,
    stream_name: &str,
    error_context: &str,
    require_complete: bool,
) -> Result<Vec<u8>, String> {
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) if require_complete => Err(format!(
            "{stream_name} drain exceeded post-kill grace ({}s) for {error_context}; byte output is not established",
            grace.as_secs()
        )),
        // Preserve established uncapped callers' timeout/reporting contract.
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(format!(
            "[ripr-xtask: {stream_name} drain exceeded post-kill grace ({}s) for {error_context}; output truncated]",
            grace.as_secs()
        ).into_bytes()),
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
    #[test]
    fn measured_runs_poll_ten_times_finer_than_plain_timed_runs() {
        assert_eq!(super::poll_interval(true), super::Duration::from_millis(10));
        assert_eq!(
            super::poll_interval(false),
            super::Duration::from_millis(100)
        );
    }

    #[cfg(unix)]
    use super::POST_KILL_GROUP_CONFIRM_GRACE;
    use super::{
        CapturedOutput, POST_KILL_DRAIN_GRACE, capture_output, capture_output_with_timeout,
        capture_output_without_timeout, capture_stdout_to_file_with_timeout, command_success_owned,
        drain_stream_reader_bounded, parse_env_timeout_secs, parse_stat_pgrp,
        read_stream_with_latency_progress, remove_stdout_capture_temp, run, run_in_dir, run_output,
        run_output_optional, run_output_owned, run_output_owned_with_envs,
        run_output_owned_with_timeout, run_owned, spawn_stream_reader_channel,
        terminate_after_timeout, timeout_was_enforced,
    };
    use crate::acquire_test_cwd_read_guard;
    use ripr::process_owner::OwnedProcess;
    use std::fs;
    use std::io::{Cursor, Read};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use std::os::unix::process::CommandExt;

    type TestCommand = (String, Vec<String>, Vec<(String, String)>);

    #[test]
    fn run_reports_success_and_failure_status() -> Result<(), String> {
        // Hold the shared cwd lock so concurrent cwd-manipulating tests in
        // main.rs cannot race this subprocess spawn (issue #2044).
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
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

    /// Fixture runs pin `RIPR_CACHE_DIR` through this overlay so the cache the
    /// child writes is the cache the runner cleared (#3054). `git var
    /// GIT_AUTHOR_IDENT` echoes `GIT_AUTHOR_NAME` back, which makes the child's
    /// view of the environment observable without a shell.
    #[test]
    fn run_output_owned_with_envs_overlays_the_child_environment() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        // Echo the variable through the platform shell rather than through
        // `git`. Both earlier attempts used `git var`, and both failed on a CI
        // runner for reasons that had nothing to do with env overlaying:
        // `GIT_AUTHOR_IDENT` needs `user.name`/`user.email` from ambient config
        // (exit 128), and `GIT_EDITOR` exits 1 with empty output when no editor
        // resolves. A shell `echo` always exits 0 and prints exactly what the
        // child environment holds, so this asserts the overlay and nothing else.
        #[cfg(windows)]
        let (program, args) = (
            "cmd",
            vec!["/C".to_string(), "echo %RIPR_ENV_PROBE%".to_string()],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            "sh",
            vec![
                "-c".to_string(),
                "printf %s \"$RIPR_ENV_PROBE\"".to_string(),
            ],
        );

        let overlaid =
            run_output_owned_with_envs(program, &args, &[("RIPR_ENV_PROBE", "ripr-env-overlay")])?;
        if !overlaid.contains("ripr-env-overlay") {
            return Err(format!(
                "the overlaid value should reach the child: {overlaid}"
            ));
        }

        let plain = run_output_owned(program, &args)?;
        if plain.contains("ripr-env-overlay") {
            return Err(format!(
                "an empty overlay must not set the variable: {plain}"
            ));
        }
        Ok(())
    }

    #[test]
    fn run_output_owned_includes_stderr_on_failure() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
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
    fn run_output_owned_with_timeout_returns_stdout_on_success() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        let args = vec!["--version".to_string()];
        let stdout = run_output_owned_with_timeout(
            "rustc",
            &args,
            Duration::from_secs(30),
            "rustc version",
        )?;
        if !stdout.contains("rustc") {
            return Err(format!("rustc version output should name rustc: {stdout}"));
        }
        Ok(())
    }

    #[test]
    fn run_output_owned_with_timeout_reports_named_timeout() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        let (program, args, _envs) = long_running_command()?;
        let err = match run_output_owned_with_timeout(
            &program,
            &args,
            Duration::from_secs(1),
            "bounded build probe",
        ) {
            Ok(stdout) => {
                return Err(format!(
                    "long-running command should time out, got stdout: {stdout}"
                ));
            }
            Err(err) => err,
        };
        for expected in [
            "bounded build probe",
            "timed out after 1 second;",
            "process tree was terminated",
        ] {
            if !err.contains(expected) {
                return Err(format!("timeout message should name {expected:?}: {err}"));
            }
        }
        Ok(())
    }

    #[test]
    fn parse_env_timeout_secs_validates_values() -> Result<(), String> {
        assert_eq!(
            parse_env_timeout_secs("RIPR_TEST_TIMEOUT", None, 900),
            Ok(900)
        );
        assert_eq!(
            parse_env_timeout_secs("RIPR_TEST_TIMEOUT", Some("120"), 900),
            Ok(120)
        );
        assert_eq!(
            parse_env_timeout_secs("RIPR_TEST_TIMEOUT", Some(" 45 "), 900),
            Ok(45)
        );
        assert_eq!(
            parse_env_timeout_secs("RIPR_TEST_TIMEOUT", Some("0"), 900),
            Err("RIPR_TEST_TIMEOUT must be a positive integer".to_string())
        );
        let err = match parse_env_timeout_secs("RIPR_TEST_TIMEOUT", Some("abc"), 900) {
            Ok(value) => return Err(format!("invalid timeout should fail, got {value}")),
            Err(err) => err,
        };
        assert!(err.contains("RIPR_TEST_TIMEOUT"));
        assert!(err.contains("positive integer"));
        Ok(())
    }

    #[test]
    fn capture_output_returns_status_stdout_and_stderr() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
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
    fn covered_by_observed_reader_preserves_original_bytes_and_invalid_utf8() -> Result<(), String>
    {
        let input = b"Cargo progress\n\xff\nripr_covered_by_phase fabricated child text\n".to_vec();
        let observation = crate::policy::phase_diagnostics::Observation::for_test(7);
        let (handle, receiver) = super::spawn_stream_reader_channel_observed(
            std::io::Cursor::new(input.clone()),
            observation,
        );
        let actual = super::drain_stream_reader_bounded(
            receiver,
            handle,
            Duration::from_secs(1),
            "stderr",
            "owned observer reader control",
        )?;
        if actual != String::from_utf8_lossy(&input) {
            return Err("optional observer changed original child stream bytes".into());
        }
        Ok(())
    }

    #[test]
    fn capture_output_with_timeout_reports_completed_process() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
        let marker = unique_pgid_marker("pipe-descendant");
        let args = vec![
            "-c".to_string(),
            r#"printf %s "$$" > "$RIPR_XTASK_PGID_MARKER"; sleep 30 & wait"#.to_string(),
        ];
        let marker_env = marker.to_string_lossy().into_owned();
        let envs = [("RIPR_XTASK_PGID_MARKER", marker_env.as_str())];
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
            &envs,
            Duration::from_secs(5),
            "pipe-inheriting descendant",
        )?;

        assert!(
            output.timed_out,
            "pipe-inheriting descendant should time out"
        );
        assert_recorded_timeout_left_process_group_gone(&marker)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn capture_output_with_timeout_confirms_gnu_time_descendant_group_gone() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        if !Path::new("/usr/bin/time").exists() {
            return Ok(());
        }
        let marker = unique_pgid_marker("gnu-time");
        // `exec` keeps the process-group leader pid while GNU `time` forks
        // `sleep`, which is the scale-cliff wrapper shape that motivated #5382.
        let args = vec![
            "-c".to_string(),
            r#"printf %s "$$" > "$RIPR_XTASK_PGID_MARKER"; exec /usr/bin/time -v sleep 30"#
                .to_string(),
        ];
        let marker_env = marker.to_string_lossy().into_owned();
        let envs = [("RIPR_XTASK_PGID_MARKER", marker_env.as_str())];
        let output = capture_output_with_timeout(
            "sh",
            &args,
            &envs,
            Duration::from_secs(5),
            "gnu-time descendant",
        )?;

        if !output.timed_out {
            return Err("gnu-time wrapped sleep should time out".to_string());
        }
        assert_recorded_timeout_left_process_group_gone(&marker)?;
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn capture_output_with_timeout_terminates_pipe_inheriting_descendants() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        let marker = std::env::temp_dir().join(format!(
            "ripr-xtask-pipe-descendant-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let script = [
            "$p = Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 30') -NoNewWindow -PassThru",
            "Wait-Process -Id $p.Id",
            "Set-Content -LiteralPath $env:RIPR_XTASK_DESCENDANT_MARKER -Value alive",
        ]
        .join("; ");
        let args = vec!["-NoProfile".to_string(), "-Command".to_string(), script];
        let marker_env = marker.to_string_lossy().into_owned();
        let envs = [("RIPR_XTASK_DESCENDANT_MARKER", marker_env.as_str())];
        // The owned Job Object (#3803) contains descendants from process
        // creation, so even a descendant started late in the run dies with
        // the tree: assignment precedes any child user code and there is no
        // snapshot window to escape through (#1022's race is structural now).
        let output = capture_output_with_timeout(
            "powershell",
            &args,
            &envs,
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

    #[cfg(unix)]
    #[test]
    fn capture_output_without_timeout_returns_after_direct_child_exit_with_lingering_pipe_descendant()
    -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        // The direct `sh` child exits 0 immediately; the backgrounded `sleep`
        // inherits the captured stdout/stderr pipes and outlives it.  The
        // no-deadline runner must still return bounded by the post-exit drain
        // grace instead of waiting for the descendant (#4150 review control),
        // and it must not report a step timeout.
        let args = vec!["-c".to_string(), "sleep 60 & exit 0".to_string()];
        let output =
            capture_output_without_timeout("sh", &args, &[], "post-exit descendant drain")?;

        assert!(
            output.status.is_some_and(|status| status.success()),
            "direct child that exited 0 must not be reported as failed: {:?}",
            output.status
        );
        assert!(
            !output.timed_out,
            "the no-deadline path must never report a step timeout"
        );
        assert!(
            output.duration < Duration::from_secs(30),
            "returned after {}s; the runner waited for the 60s pipe-holding descendant instead of honoring the drain grace",
            output.duration.as_secs()
        );
        assert!(
            output.stdout.contains("output truncated"),
            "truncated-capture diagnostic missing from stdout: {:?}",
            output.stdout
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn capture_output_without_timeout_returns_after_direct_child_exit_with_lingering_pipe_descendant()
    -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        // Same control as the Unix variant: the direct `cmd` child exits 0
        // immediately after `start /b`-ing a longer-lived ping descendant that
        // inherits the captured stdout/stderr pipes.  The owned-subprocess Job
        // Object does not fire (no step timeout), so only the bounded
        // post-exit drain keeps the call from waiting out the descendant.
        // `cmd`/`ping` are used instead of PowerShell so the control does not
        // depend on a heavyweight shell under a loaded host.
        let script = "start /b ping -n 60 127.0.0.1 >NUL & exit /b 0";
        let args = vec![
            "/d".to_string(),
            "/s".to_string(),
            "/c".to_string(),
            script.to_string(),
        ];
        let output =
            capture_output_without_timeout("cmd", &args, &[], "post-exit descendant drain")?;

        assert!(
            output.status.is_some_and(|status| status.success()),
            "direct child that exited 0 must not be reported as failed: {:?}",
            output.status
        );
        assert!(
            !output.timed_out,
            "the no-deadline path must never report a step timeout"
        );
        assert!(
            output.duration < Duration::from_secs(30),
            "returned after {}s; the runner waited for the 60s pipe-holding descendant instead of honoring the drain grace",
            output.duration.as_secs()
        );
        assert!(
            output.stderr.contains("output truncated"),
            "truncated-capture diagnostic missing from stderr: {:?}",
            output.stderr
        );
        Ok(())
    }

    #[test]
    fn capture_stdout_to_file_with_timeout_streams_stdout_to_file() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
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
        let _cwd_guard = acquire_test_cwd_read_guard();
        let mut command = Command::new("rustc");
        command.arg("--version");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child =
            OwnedProcess::spawn(command).map_err(|err| format!("spawn rustc version: {err}"))?;

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
        let _cwd_guard = acquire_test_cwd_read_guard();
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

    #[test]
    fn parse_stat_pgrp_reads_the_field_after_comm() -> Result<(), String> {
        if parse_stat_pgrp("42 (sleep) S 1 99 0 -1")
            != Some(super::ProcStatPgrp::Live {
                state: 'S',
                pgrp: 99,
            })
        {
            return Err("pgrp is the third field after the closing comm parenthesis".to_string());
        }
        if parse_stat_pgrp("7 (name with spaces) R 3 88 0")
            != Some(super::ProcStatPgrp::Live {
                state: 'R',
                pgrp: 88,
            })
        {
            return Err("comm may contain spaces; parse after the last ')'".to_string());
        }
        if parse_stat_pgrp("10 (weird)name) S 1 3 0")
            != Some(super::ProcStatPgrp::Live {
                state: 'S',
                pgrp: 3,
            })
        {
            return Err("comm may contain ')'; parse after the last ')'".to_string());
        }
        if parse_stat_pgrp("9 (sleep) Z 1 9 9 0") != Some(super::ProcStatPgrp::NotLive) {
            return Err("zombie group members are not still-running".to_string());
        }
        if parse_stat_pgrp("no-paren-line").is_some() {
            return Err("malformed stat should not invent a pgrp".to_string());
        }
        if parse_stat_pgrp("9 (sleep) S").is_some() {
            return Err("truncated non-zombie stat must stay unparseable".to_string());
        }
        Ok(())
    }

    #[test]
    fn members_from_group_scan_fails_closed_on_incomplete_readable_probe() -> Result<(), String> {
        let live = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: vec![9] }),
            Ok(super::GroupProbe::Gone),
        )?;
        if live != vec![9] {
            return Err(format!("complete live scan should win: {live:?}"));
        }
        let empty_zombie = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: Vec::new() }),
            Ok(super::GroupProbe::SignaledAlive),
        )?;
        if !empty_zombie.is_empty() {
            return Err(
                "complete empty scan must ignore kill -0 zombies and not invent members"
                    .to_string(),
            );
        }
        let empty_gone = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: Vec::new() }),
            Ok(super::GroupProbe::Gone),
        )?;
        if !empty_gone.is_empty() {
            return Err("complete empty scan plus ESRCH must confirm gone".to_string());
        }
        let setuid = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: Vec::new() }),
            Ok(super::GroupProbe::Inaccessible),
        )?;
        if setuid != vec![1234] {
            return Err(format!(
                "complete empty scan plus EPERM must not confirm gone: {setuid:?}"
            ));
        }
        let unread = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Incomplete),
            Ok(super::GroupProbe::SignaledAlive),
        )?;
        if unread != vec![1234] {
            return Err(format!(
                "incomplete scan plus a live probe must not confirm gone: {unread:?}"
            ));
        }
        let unread_gone = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Incomplete),
            Ok(super::GroupProbe::Gone),
        )?;
        if !unread_gone.is_empty() {
            return Err("incomplete scan plus ESRCH may confirm gone".to_string());
        }
        let unread_eperm = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Incomplete),
            Ok(super::GroupProbe::Inaccessible),
        )?;
        if unread_eperm != vec![1234] {
            return Err(format!(
                "incomplete scan plus EPERM must not confirm gone: {unread_eperm:?}"
            ));
        }
        let missing =
            super::members_from_group_scan(7, None, Ok(super::GroupProbe::SignaledAlive))?;
        if missing != vec![7] {
            return Err(format!(
                "unavailable /proc must use the group probe: {missing:?}"
            ));
        }
        let complete_ignores_probe_on_live = super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: vec![9] }),
            Err("probe failed".to_string()),
        )?;
        if complete_ignores_probe_on_live != vec![9] {
            return Err("complete live scan must not surface a skipped probe error".to_string());
        }
        if super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Complete { live: Vec::new() }),
            Err("could not probe process group 1234: e".to_string()),
        )
        .is_ok()
        {
            return Err("empty complete scan plus probe failure must fail closed".to_string());
        }
        if super::members_from_group_scan(
            1234,
            Some(super::ProcGroupScan::Incomplete),
            Err("could not probe process group 1234: e".to_string()),
        )
        .is_ok()
        {
            return Err("incomplete scan plus probe failure must fail closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn classify_group_probe_treats_non_esrch_as_populated() -> Result<(), String> {
        if super::classify_group_probe(true, "") != super::GroupProbe::SignaledAlive {
            return Err("successful kill -0 means the group is populated".to_string());
        }
        if super::classify_group_probe(false, "kill: (-9): No such process\n")
            != super::GroupProbe::Gone
        {
            return Err("ESRCH must mean the group is gone".to_string());
        }
        if super::classify_group_probe(false, "kill: (-9): Operation not permitted\n")
            != super::GroupProbe::Inaccessible
        {
            return Err("EPERM must not confirm the group is gone".to_string());
        }
        if super::classify_group_probe(false, "") != super::GroupProbe::Inaccessible {
            return Err("unknown nonzero kill status must not confirm gone".to_string());
        }
        if super::classify_group_probe(false, "kill: (-9): Aucun processus de ce type\n")
            != super::GroupProbe::Inaccessible
        {
            return Err(
                "translated ESRCH must not confirm gone; the kill probe must use LC_ALL=C"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn probe_process_group_alive_classifies_a_missing_group_as_gone() -> Result<(), String> {
        match super::probe_process_group_alive(i32::MAX as u32) {
            Ok(super::GroupProbe::Gone) => Ok(()),
            other => Err(format!(
                "LC_ALL=C kill -0 of a missing group should be ESRCH, got {other:?}"
            )),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_proc_pgrp_includes_readable_other_uid_members() -> Result<(), String> {
        let Ok(stat) = fs::read_to_string("/proc/1/stat") else {
            return Ok(());
        };
        let Some(super::ProcStatPgrp::Live { pgrp, .. }) = parse_stat_pgrp(&stat) else {
            return Ok(());
        };
        match super::scan_proc_pgrp(pgrp) {
            Some(super::ProcGroupScan::Complete { live }) if live.contains(&1) => Ok(()),
            other => Err(format!(
                "readable pid 1 in group {pgrp} must appear in the scan, got {other:?}"
            )),
        }
    }

    #[test]
    fn remove_stdout_capture_temp_deletes_the_file() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-xtask-stdout-tmp-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::write(&path, "capture").map_err(|err| format!("write temp capture: {err}"))?;
        remove_stdout_capture_temp(&path);
        if path.exists() {
            let _ = fs::remove_file(&path);
            return Err("wait-error cleanup must remove the stdout temp file".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn post_kill_group_confirm_grace_is_two_seconds() -> Result<(), String> {
        if POST_KILL_GROUP_CONFIRM_GRACE != Duration::from_secs(2) {
            return Err(format!(
                "POST_KILL_GROUP_CONFIRM_GRACE should be 2 s; got {:?}",
                POST_KILL_GROUP_CONFIRM_GRACE
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn confirm_process_group_gone_kills_a_live_group_within_budget() -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        let (mut child, pgid) = spawn_sleep_in_own_group()?;
        wait_until_process_group_has_members(pgid)?;
        if let Err(err) =
            super::confirm_process_group_gone(pgid, Duration::from_secs(2), "live-group")
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
        let leftover = super::process_group_live_members(pgid)?;
        let _ = child.wait();
        if !leftover.is_empty() {
            return Err(format!(
                "confirm_process_group_gone should leave group {pgid} empty, still running: {leftover:?}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn confirm_process_group_gone_reports_when_a_live_group_outlives_zero_budget()
    -> Result<(), String> {
        let _cwd_guard = acquire_test_cwd_read_guard();
        let (mut child, pgid) = spawn_sleep_in_own_group()?;
        if let Err(err) = wait_until_process_group_has_members(pgid) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
        let err = match super::confirm_process_group_gone(pgid, Duration::ZERO, "zero-budget") {
            Ok(()) => {
                let _ = super::signal_process_group(pgid, "-KILL");
                let _ = child.wait();
                return Err(
                    "zero-budget confirmation must not claim a live group is gone".to_string(),
                );
            }
            Err(err) => err,
        };
        let still_live = super::process_group_live_members(pgid);
        let _ = super::signal_process_group(pgid, "-KILL");
        let _ = child.wait();
        for expected in ["could not confirm", "zero-budget", &pgid.to_string()] {
            if !err.contains(expected) {
                return Err(format!(
                    "unconfirmed timeout should name {expected:?}; got {err}"
                ));
            }
        }
        match still_live {
            Ok(members) if members.is_empty() => Err(
                "zero-budget path should not kill the live group before reporting unconfirmed"
                    .to_string(),
            ),
            Ok(_) => Ok(()),
            Err(err) => Err(format!(
                "live group should still be probeable after an unconfirmed report: {err}"
            )),
        }
    }

    #[cfg(unix)]
    fn unique_pgid_marker(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ripr-xtask-pgid-{label}-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[cfg(unix)]
    fn assert_recorded_timeout_left_process_group_gone(marker: &Path) -> Result<(), String> {
        let pgid_text = fs::read_to_string(marker).map_err(|err| {
            format!(
                "timed-out child should have recorded its pgid at {}: {err}",
                marker.display()
            )
        })?;
        let _ = fs::remove_file(marker);
        let pgid = pgid_text
            .trim()
            .parse::<u32>()
            .map_err(|err| format!("pgid marker should contain a pid, got {pgid_text:?}: {err}"))?;
        let members = super::process_group_live_members(pgid)?;
        if !members.is_empty() {
            let _ = super::signal_process_group(pgid, "-KILL");
            return Err(format!(
                "recorded timeout coexisted with live process group {pgid} members {members:?}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    fn spawn_sleep_in_own_group() -> Result<(std::process::Child, u32), String> {
        let mut command = Command::new("sleep");
        command.arg("30");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        command.process_group(0);
        let child = command
            .spawn()
            .map_err(|err| format!("spawn sleep in its own process group: {err}"))?;
        let pgid = child.id();
        Ok((child, pgid))
    }

    #[cfg(unix)]
    fn wait_until_process_group_has_members(pgid: u32) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match super::process_group_live_members(pgid) {
                Ok(members) if !members.is_empty() => return Ok(()),
                Ok(_) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "sleep process group {pgid} did not appear in /proc or kill -0"
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// A `Read` implementation that blocks indefinitely on every `read` call.
    ///
    /// This simulates a pipe whose write-end is held open by a descendant that
    /// escaped the process-group kill.  It is used to exercise the
    /// `drain_stream_reader_bounded` grace-timeout path at the unit level
    /// without spawning a real process tree.
    ///
    /// Note on Windows/platform portability: the owned Job Object tree now
    /// terminates descendants deterministically (see #3803), but this
    /// unit-level seam test remains the authoritative check for the
    /// bounded-drain guarantee on all platforms without a real process tree.
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

#[cfg(all(test, unix))]
#[path = "run/input_tests.rs"]
mod input_tests;
