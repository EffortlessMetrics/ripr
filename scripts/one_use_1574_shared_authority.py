#!/usr/bin/env python3
"""One-use, self-deleting transformation for ripr#1574 shared authorities."""

from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label} marker drifted: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def main() -> None:
    verify = Path("xtask/src/reports/source_promotion_verify.rs")
    text = verify.read_text(encoding="utf-8")
    text = replace_once(
        text,
        "fn validate_preflight(preflight: &Value, source_main: &str) -> Result<(), String> {",
        "pub(crate) fn validate_preflight(preflight: &Value, source_main: &str) -> Result<(), String> {",
        "source-promotion preflight validator",
    )
    text = replace_once(
        text,
        "fn validate_manifest(manifest: &Value, preflight: &Value, digest: &str) -> Result<(), String> {",
        "pub(crate) fn validate_manifest(manifest: &Value, preflight: &Value, digest: &str) -> Result<(), String> {",
        "source-promotion manifest validator",
    )
    verify.write_text(text, encoding="utf-8")

    run = Path("xtask/src/run.rs")
    text = run.read_text(encoding="utf-8")
    struct_marker = """pub(crate) struct TimedFileOutput {
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stderr: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
    pub(crate) stdout_bytes: usize,
}
"""
    struct_addition = struct_marker + """
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
"""
    text = replace_once(text, struct_marker, struct_addition, "TimedFileOutput")

    function_marker = "fn stdout_capture_temp_path(stdout_path: &Path) -> std::path::PathBuf {"
    helper = r'''
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
    let mut child = command
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
    let (stdout_handle, stdout_rx) = spawn_bounded_stream_reader_channel(stdout, max_stream_bytes);
    let (stderr_handle, stderr_rx) = spawn_bounded_stream_reader_channel(stderr, max_stream_bytes);

    let wait_outcome = wait_for_child_with_timeout(&mut child, started, timeout, error_context)?;
    let stdout = drain_bounded_stream_reader(
        stdout_rx,
        stdout_handle,
        POST_KILL_DRAIN_GRACE,
        "stdout",
        error_context,
    )?;
    let stderr = drain_bounded_stream_reader(
        stderr_rx,
        stderr_handle,
        POST_KILL_DRAIN_GRACE,
        "stderr",
        error_context,
    )?;

    Ok(TimedBoundedOutput {
        status: Some(wait_outcome.status),
        stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
        duration: wait_outcome.duration,
        timed_out: wait_outcome.timed_out,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
    })
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

'''
    text = replace_once(text, function_marker, helper + function_marker, "bounded-run insertion")
    run.write_text(text, encoding="utf-8")

    command = Path("xtask/src/command.rs")
    text = command.read_text(encoding="utf-8")
    known = '        "source-promotion verify --preflight <receipt.json> --resolution-manifest <manifest.json> --join-head <sha> --source-main <sha> [--main-head <sha>] [--out <dir>]",\n'
    new_known = known + '        "source-promotion validate-resolved-tree --source-parent <sha> --swarm-parent <sha> --reviewed-tree <tree> --preflight <receipt.json> --preflight-sha256 <digest> --resolution-manifest <manifest.json> --resolution-sha256 <digest> [--out <dir>]",\n'
    if new_known not in text:
        text = replace_once(text, known, new_known, "known command")

    catalog = '''        command_entry(
            "source-promotion verify --preflight <receipt.json> --resolution-manifest <manifest.json> --join-head <sha> --source-main <sha> [--main-head <sha>] [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/source-promotion-verification.{json,md} or explicit --out <dir>",
            false,
            "Verifies an exact history-preserving join, reviewed resolution manifest, ancestry digests, and metadata identity without constructing or mutating Git refs.",
        ),
'''
    catalog_addition = catalog + '''        command_entry(
            "source-promotion validate-resolved-tree --source-parent <sha> --swarm-parent <sha> --reviewed-tree <tree> --preflight <receipt.json> --preflight-sha256 <digest> --resolution-manifest <manifest.json> --resolution-sha256 <digest> [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/resolved-tree/{resolved-tree-validation.json,resolved-tree-validation.md,commands/**} or explicit --out <dir>",
            false,
            "Validates one exact reviewed tree with the source-parent governance catalog and retained bounded evidence before direct-J construction.",
        ),
'''
    if "source-promotion validate-resolved-tree --source-parent <sha>" not in text[text.find("fn command_catalog"):]:
        text = replace_once(text, catalog, catalog_addition, "command catalog")
    command.write_text(text, encoding="utf-8")

    policy = Path("policy/process_allowlist.txt")
    text = policy.read_text(encoding="utf-8")
    old = "xtask/src/run.rs|Command::new|14|repo-policy|"
    new = "xtask/src/run.rs|Command::new|15|repo-policy|"
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise SystemExit("run.rs process count marker drifted")
    policy.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
