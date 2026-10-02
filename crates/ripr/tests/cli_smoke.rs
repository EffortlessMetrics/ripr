#![expect(
    clippy::unwrap_used,
    reason = "CLI smoke test: unwrap on Command::output() and CARGO_MANIFEST_DIR's parent chain is the canonical fail-fast pattern for binary integration tests; receipted via policy/no-panic-allowlist.toml entries for crates/ripr/tests/cli_smoke.rs."
)]

use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "common/advisory_write_safety.rs"]
mod advisory_write_safety;
#[path = "../src/build_commit_record.rs"]
mod build_commit_record;
#[path = "common/mod.rs"]
mod common;

// All plain fixture-setup git invocations below route through the shared
// hardened helper (deadline + one idempotent retry + commit reconcile,
// #3742 Slice 1). Raw `run_command("git", …)` remains only where a test
// needs the raw `Output` (commit identity probes, failure-shape asserts).
use common::fixture_git::fixture_git_ok as run_git;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn run_ripr(args: &[&str]) -> Output {
    let bin = env!("CARGO_BIN_EXE_ripr");
    Command::new(bin).args(args).output().unwrap()
}

fn run_ripr_in_workspace(args: &[&str]) -> Result<Output, std::io::Error> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root = workspace_root();
    run_command(bin, Some(&root), args)
}

/// Run `ripr` with the given bytes piped to its stdin, the way a shell
/// `git diff origin/main | ripr check --diff -` feeds the process (#4319).
fn run_ripr_with_stdin(args: &[&str], stdin: &[u8]) -> Result<Output, std::io::Error> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    spawn_command(bin, None, args, &[], None, Some(stdin))
}

fn run_command(
    program: &str,
    current_dir: Option<&Path>,
    args: &[&str],
) -> Result<Output, std::io::Error> {
    spawn_command(program, current_dir, args, &[], None, None)
}

/// The single process spawn point for this harness. Both `run_command` and
/// `run_command_with_env` route through here so the suite keeps one tracked
/// spawn site rather than one per calling convention. With `redirect`, the
/// child's stdout and stderr go to those files the way a shell `> out 2> err`
/// sends them (each file is created before the child starts), and the
/// returned `Output` carries empty captured streams. With `stdin`, the bytes
/// are piped to the child the way a shell `printf … | command` does: the
/// write end is closed after writing so the child sees EOF, and the child's
/// stdout/stderr are captured unless `redirect` sends them to files.
fn spawn_command(
    program: &str,
    current_dir: Option<&Path>,
    args: &[&str],
    env: &[(&str, &str)],
    redirect: Option<(&Path, &Path)>,
    stdin: Option<&[u8]>,
) -> Result<Output, std::io::Error> {
    let mut command = probe_command(program);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.args(args);
    match (stdin, redirect) {
        (None, None) => command.output(),
        (None, Some((stdout, stderr))) => {
            command
                .stdout(std::fs::File::create(stdout)?)
                .stderr(std::fs::File::create(stderr)?);
            Ok(Output {
                status: command.status()?,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
        (Some(input), redirect) => {
            command.stdin(Stdio::piped());
            match redirect {
                None => {
                    command.stdout(Stdio::piped());
                    command.stderr(Stdio::piped());
                }
                Some((stdout, stderr)) => {
                    command
                        .stdout(std::fs::File::create(stdout)?)
                        .stderr(std::fs::File::create(stderr)?);
                }
            }
            run_owned_stdin_probe(command, input, std::time::Duration::from_secs(30))
        }
    }
}

fn probe_command(program: &str) -> Command {
    Command::new(program)
}

#[test]
fn stdin_probe_fixture_entry() {
    if std::env::var("RIPR_STDIN_PROBE_CHILD").as_deref() == Ok("hang") {
        std::thread::sleep(std::time::Duration::from_mins(1));
    }
}

fn stdin_probe_fixture(mode: &str) -> Result<Command, std::io::Error> {
    let executable = std::env::current_exe()?;
    let executable = executable
        .to_str()
        .ok_or_else(|| std::io::Error::other("test executable path is not UTF-8"))?;
    let mut command = probe_command(executable);
    command
        .args(["--exact", "stdin_probe_fixture_entry"])
        .env("RIPR_STDIN_PROBE_CHILD", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

#[test]
fn stdin_probe_stalled_child_returns_bounded_timeout() -> Result<(), std::io::Error> {
    let started = std::time::Instant::now();
    let result = run_owned_stdin_probe(
        stdin_probe_fixture("hang")?,
        b"small fixture",
        std::time::Duration::from_millis(100),
    );
    assert_eq!(
        result.err().map(|err| err.kind()),
        Some(std::io::ErrorKind::TimedOut)
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    Ok(())
}

#[test]
fn stdin_probe_early_exit_returns_write_error() -> Result<(), std::io::Error> {
    let result = run_owned_stdin_probe(
        stdin_probe_fixture("exit")?,
        &vec![b'x'; 2 * 1024 * 1024],
        std::time::Duration::from_secs(5),
    );
    assert_eq!(
        result.err().map(|err| err.kind()),
        Some(std::io::ErrorKind::BrokenPipe)
    );
    Ok(())
}

/// Bound writing, process completion and capture to one deadline. The owner
/// terminates/reaps on every returned error; no writer/drain join can stall
/// the harness beyond that deadline.
fn run_owned_stdin_probe(
    command: Command,
    input: &[u8],
    budget: std::time::Duration,
) -> Result<Output, std::io::Error> {
    let deadline = std::time::Instant::now() + budget;
    let mut child = ripr::process_owner::OwnedProcess::spawn(command)?;
    let mut pipe = child
        .stdin_pipe()
        .take()
        .ok_or_else(|| std::io::Error::other("child stdin was not captured"))?;
    let input = input.to_vec();
    let (write_tx, write_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = pipe.write_all(&input);
        drop(pipe); // EOF even on a failed write.
        let _ = write_tx.send(result);
    });
    fn drain<R: std::io::Read + Send + 'static>(
        pipe: Option<R>,
    ) -> std::sync::mpsc::Receiver<Result<Vec<u8>, std::io::Error>> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = match pipe {
                Some(mut pipe) => pipe.read_to_end(&mut bytes).map(|_| bytes),
                None => Ok(bytes),
            };
            let _ = tx.send(result);
        });
        rx
    }
    fn receive<T>(
        rx: &std::sync::mpsc::Receiver<Result<T, std::io::Error>>,
        deadline: std::time::Instant,
    ) -> Result<T, std::io::Error> {
        rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))?
    }
    let stdout = drain(child.stdout_pipe().take());
    let stderr = drain(child.stderr_pipe().take());
    let result = (|| {
        receive(&write_rx, deadline)?;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "stdin probe exceeded its deadline",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        Ok(Output {
            status,
            stdout: receive(&stdout, deadline)?,
            stderr: receive(&stderr, deadline)?,
        })
    })();
    child.terminate_tree().map_err(std::io::Error::other)?;
    result
}

/// Run a command with extra environment variables set, so tests can plant an
/// ambient-secret canary in the parent and assert it does not cross a process
/// boundary.
fn run_command_with_env(
    program: &str,
    current_dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Output, std::io::Error> {
    spawn_command(program, Some(current_dir), args, env, None, None)
}

/// Retry only `ETXTBSY` (`ExecutableFileBusy`). Copying `ripr` and executing
/// the copy can fail while another test still holds a writable descriptor
/// from a `fork` that has not reached `exec` (#4296). Three attempts and a
/// 25ms pause match the doctor shim probe. Any other error, and any process
/// that actually started, is returned unchanged.
fn output_retrying_text_file_busy<T>(
    mut run: impl FnMut() -> Result<T, std::io::Error>,
) -> Result<T, std::io::Error> {
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        match run() {
            Err(error) if retry_text_file_busy(error.kind(), attempt) => {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            other => return other,
        }
    }
}

fn retry_text_file_busy(kind: std::io::ErrorKind, attempt: usize) -> bool {
    kind == std::io::ErrorKind::ExecutableFileBusy && attempt < 3
}

#[test]
fn text_file_busy_retry_stops_at_three_and_ignores_other_errors() {
    assert!(retry_text_file_busy(
        std::io::ErrorKind::ExecutableFileBusy,
        1
    ));
    assert!(retry_text_file_busy(
        std::io::ErrorKind::ExecutableFileBusy,
        2
    ));
    assert!(!retry_text_file_busy(
        std::io::ErrorKind::ExecutableFileBusy,
        3
    ));
    assert!(!retry_text_file_busy(std::io::ErrorKind::NotFound, 1));
    assert!(!retry_text_file_busy(
        std::io::ErrorKind::PermissionDenied,
        1
    ));
    assert!(!retry_text_file_busy(std::io::ErrorKind::WouldBlock, 1));
}

/// The predicate test above cannot see the loop: a wrapper reduced to one
/// `run()` call would still pass it. A scripted closure pins what the wrapper
/// does with each result: how many times it runs, and what it returns.
#[test]
fn text_file_busy_wrapper_reruns_only_busy_and_returns_the_final_result() -> Result<(), String> {
    use std::io::{Error, ErrorKind};
    fn scripted(script: Vec<Result<u8, ErrorKind>>) -> (Result<u8, ErrorKind>, usize) {
        let mut script = script.into_iter();
        let mut calls = 0usize;
        let result = output_retrying_text_file_busy(|| {
            calls += 1;
            match script.next() {
                Some(Ok(value)) => Ok(value),
                Some(Err(kind)) => Err(Error::from(kind)),
                None => Err(Error::other("script exhausted")),
            }
        });
        (result.map_err(|error| error.kind()), calls)
    }
    let busy = ErrorKind::ExecutableFileBusy;
    // (name, scripted results, expected result, expected calls)
    type Case = (
        &'static str,
        Vec<Result<u8, ErrorKind>>,
        Result<u8, ErrorKind>,
        usize,
    );
    let cases: [Case; 5] = [
        ("started first time", vec![Ok(7)], Ok(7), 1),
        (
            "busy twice then started",
            vec![Err(busy), Err(busy), Ok(9)],
            Ok(9),
            3,
        ),
        (
            "busy on every attempt",
            vec![Err(busy), Err(busy), Err(busy), Ok(1)],
            Err(busy),
            3,
        ),
        (
            "other error is not retried",
            vec![Err(ErrorKind::NotFound), Ok(1)],
            Err(ErrorKind::NotFound),
            1,
        ),
        (
            "other error after busy is returned",
            vec![Err(busy), Err(ErrorKind::PermissionDenied), Ok(1)],
            Err(ErrorKind::PermissionDenied),
            2,
        ),
    ];
    let mut failures = Vec::new();
    for (name, script, expected, expected_calls) in cases {
        let (result, calls) = scripted(script);
        if result != expected || calls != expected_calls {
            failures.push(format!(
                "{name}: got {result:?} after {calls} calls, expected {expected:?} after {expected_calls}"
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn run_isolated_binary(
    binary: &Path,
    current_dir: &Path,
    args: &[&str],
    coverage_profile: Option<&Path>,
) -> Result<Output, std::io::Error> {
    output_retrying_text_file_busy(|| {
        let mut command = Command::new(binary);
        command.current_dir(current_dir).env_clear();
        if let Ok(path) = std::env::var("PATH") {
            command.env("PATH", path);
        }
        if let Some(profile) = coverage_profile {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command.args(args).output()
    })
}

fn cleanup_temp_dir(path: Option<&Path>) -> Result<(), std::io::Error> {
    if let Some(path) = path
        && path.exists()
    {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

/// Best-effort temp-dir teardown. The `io::Result` is matched with `if let`
/// so a `#[must_use]` cleanup failure is an explicit ignore.
fn ignore_remove_dir_all(path: &Path) {
    if let Ok(()) = std::fs::remove_dir_all(path) {}
}

fn inherited_coverage_is_enabled() -> bool {
    std::env::var_os("LLVM_PROFILE_FILE").is_some() || std::env::var_os("CARGO_LLVM_COV").is_some()
}

fn snapshot_tree(root: &Path) -> Result<Vec<String>, std::io::Error> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<String>) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child = entry.path();
            let relative = child
                .strip_prefix(root)
                .map_err(std::io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                entries.push(format!("dir:{relative}"));
                visit(root, &child, entries)?;
            } else if metadata.is_file() {
                let content = std::fs::read(&child)?;
                entries.push(format!("file:{relative}:{}", sha256_hex_bytes(&content)));
            } else {
                entries.push(format!("other:{relative}"));
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    entries.sort();
    Ok(entries)
}

fn snapshot_diff(before: &[String], after: &[String]) -> String {
    let removed = before
        .iter()
        .filter(|entry| !after.contains(entry))
        .map(|entry| format!("- {entry}"))
        .collect::<Vec<_>>();
    let added = after
        .iter()
        .filter(|entry| !before.contains(entry))
        .map(|entry| format!("+ {entry}"))
        .collect::<Vec<_>>();
    removed
        .into_iter()
        .chain(added)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_concrete_commit_id(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn bounded_command_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(512).collect()
}

fn strip_optional_git_line_ending(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .unwrap_or(bytes)
}

fn select_fixture_repository_head(
    git_succeeded: bool,
    stdout: &[u8],
    stderr: &[u8],
    actions_fallback: Option<&str>,
) -> Result<String, String> {
    let candidate = String::from_utf8_lossy(strip_optional_git_line_ending(stdout)).to_string();
    if git_succeeded {
        if is_concrete_commit_id(&candidate) {
            return Ok(candidate.to_ascii_lowercase());
        }
        return Err(format!(
            "git rev-parse --verify HEAD^{{commit}} returned non-concrete repository HEAD `{candidate}`"
        ));
    }

    if let Some(fallback) = actions_fallback
        && is_concrete_commit_id(fallback)
    {
        return Ok(fallback.to_ascii_lowercase());
    }

    Err(format!(
        "git rev-parse --verify HEAD^{{commit}} failed; stdout: {}; stderr: {}",
        bounded_command_text(stdout),
        bounded_command_text(stderr)
    ))
}

fn concrete_fixture_repository_head(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = run_command(
        "git",
        Some(root),
        &["rev-parse", "--verify", "HEAD^{commit}"],
    )?;
    let root_is_workspace = root
        .canonicalize()
        .ok()
        .zip(workspace_root().canonicalize().ok())
        .is_some_and(|(root, workspace)| root == workspace);
    let actions_fallback =
        if root_is_workspace && std::env::var("GITHUB_ACTIONS").ok().as_deref() == Some("true") {
            std::env::var("GITHUB_SHA").ok()
        } else {
            None
        };
    select_fixture_repository_head(
        output.status.success(),
        &output.stdout,
        &output.stderr,
        actions_fallback.as_deref(),
    )
    .map_err(Into::into)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn sample_diff() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample/example.diff")
}

fn unique_temp_workspace(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ripr-{label}-{stamp}-{pid}-{counter}"))
}

fn unique_external_workspace(label: &str) -> Result<PathBuf, String> {
    let workspace = workspace_root()
        .canonicalize()
        .map_err(|error| format!("canonicalize workspace root: {error}"))?;
    let candidate = unique_temp_workspace(label);
    let parent = workspace
        .parent()
        .ok_or_else(|| "workspace root has no parent".to_string())?;
    let name = candidate
        .file_name()
        .ok_or_else(|| "temporary fixture has no file name".to_string())?;
    Ok(parent.join(name))
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected command to succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected command to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

/// Issue #3872: CLI-rendered command redirects anchor at the resolved --root
/// — the child process working directory, which is the workspace root for
/// `run_ripr_in_workspace`. Project that machine prefix to `<cwd>/` before
/// comparing so the fixture pins the anchored shape, never a machine
/// directory (same placeholder rule as the lib-test projections).
fn assert_anchored_stdout_matches_fixture(
    output: &Output,
    fixture_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_success(output);
    let expected = std::fs::read_to_string(workspace_root().join(fixture_path))?;
    let actual = String::from_utf8(output.stdout.clone())?;
    let prefix = format!("{}/", workspace_root().to_string_lossy().replace('\\', "/"));
    assert_eq!(
        normalize_newlines(&actual).replace(&prefix, "<cwd>/"),
        normalize_newlines(&expected),
        "stdout drifted from {fixture_path}"
    );
    Ok(())
}

fn write_bound_repo_exposure_fixture(
    root: &Path,
    path: &Path,
    seam_json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let head = concrete_fixture_repository_head(root)?;
    let root_identity = root.canonicalize()?.to_string_lossy().replace('\\', "/");
    let placeholder = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let raw = format!(
        r#"{{
  "schema_version": "0.3",
  "artifact": {{
    "kind": "repo_exposure",
    "schema_version": "1",
    "canonicalization": "raw_json_placeholder_v1",
    "producer": {{"tool": "ripr", "version": "0.11.0"}},
    "repository": {{"root": "{root_identity}", "head": "{head}"}},
    "analysis": {{"format": "repo-exposure-json", "mode": "draft", "base_revision": null, "input_identity": "input:v4:fnv1a64:00000000000000f1", "command": "ripr check --format repo-exposure-json", "profile": "draft", "worktree": "clean"}},
    "snapshot_identity": "snapshot:input:v4:fnv1a64:00000000000000f1;revision:{head}",
    "content_sha256": "{placeholder}"
  }},
  "scope": "repo",
  "run_status": "complete",
  "seams": [{seam_json}]
}}"#
    );
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let digest = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    // Exact-one rule (#2921): the placeholder must appear only as the
    // governed `artifact.content_sha256` value before the global replace is
    // safe. A second placeholder-shaped string would be silently rewritten.
    let placeholder_occurrences = raw.matches(placeholder).count();
    assert_eq!(
        placeholder_occurrences, 1,
        "bound repo-exposure fixture must contain exactly one content_sha256 placeholder"
    );
    std::fs::write(path, raw.replace(placeholder, &digest))?;
    Ok(())
}

fn write_fabricated_agent_verify_json(
    path: &Path,
    before: &Path,
    after: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let before_display = before.display().to_string().replace('\\', "/");
    let after_display = after.display().to_string().replace('\\', "/");
    std::fs::write(
        path,
        format!(
            r#"{{
  "schema_version": "0.3",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {{"before": "{}", "after": "{}"}},
  "summary": {{"improved": 1, "changed": 0, "regressed": 0, "unchanged": 0, "new": 0, "resolved": 0}},
  "changed_seams": [{{"seam_id":"seam-a","seam_kind":"predicate_boundary","file":"src/pricing.rs","line":42,"before":"weakly_gripped","after":"strongly_gripped","change":"improved","evidence_delta":[]}}],
  "unchanged_seams": [], "new_gaps": [], "resolved_gaps": []
}}"#,
            before_display, after_display
        ),
    )?;
    Ok(())
}

fn init_git_fixture_repo(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(root.join("marker.txt"), "fixture\n")?;
    run_git(root, &["init"])?;
    // Ordinary Rust repair fixtures meet the build-output precondition
    // without adding another tracked analysis input. Dedicated ignore tests
    // replace this effective local rule to exercise missing/partial ignores.
    std::fs::write(root.join(".git/info/exclude"), "/target/\n")?;
    run_git(root, &["add", "marker.txt"])?;
    let commit = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    )?;
    assert!(commit.status.success(), "fixture commit failed: {commit:?}");
    Ok(())
}

fn init_producer_fixture_repo(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
    for relative in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
        std::fs::copy(fixture_root.join(relative), root.join(relative))?;
    }
    run_git(root, &["init"])?;
    std::fs::write(root.join(".git/info/exclude"), "/target/\n")?;
    // Keep checkouts byte-identical to what was committed. Windows Git
    // defaults to autocrlf=true, so a `git checkout HEAD -- <file>` restore
    // would rewrite LF bytes as CRLF and the edit cage (which digests raw
    // bytes) would see the restored file as a new edit.
    run_git(root, &["config", "core.autocrlf", "false"])?;
    run_git(
        root,
        &["add", "Cargo.toml", "src/lib.rs", "tests/pricing.rs"],
    )?;
    let commit = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    )?;
    assert!(
        commit.status.success(),
        "producer fixture commit failed: {commit:?}"
    );
    Ok(())
}

/// Move the fixture repository to a new empty commit so a later-bound
/// repo-exposure artifact is lineage-ordered after an earlier one (#2922).
fn advance_fixture_head(root: &Path, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let commit = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    )?;
    assert!(
        commit.status.success(),
        "movement commit failed: {commit:?}"
    );
    Ok(())
}

fn recommit_repo_exposure_json(mut raw: String) -> String {
    let placeholder = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let key = "\"content_sha256\"";
    if let Some(key_start) = raw.find(key) {
        let value_search_start = key_start + key.len();
        if let Some(value_offset) = raw[value_search_start..].find('"') {
            let value_start = value_search_start + value_offset + 1;
            if let Some(end_offset) = raw[value_start..].find('"') {
                raw.replace_range(value_start..value_start + end_offset, placeholder);
            }
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let digest = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    raw = raw.replace(placeholder, &digest);
    raw
}

/// The validated content commitment declared by a bound repo-exposure
/// artifact. Tamper-control cases retain this digest pair (original vs
/// mutated) in their failure messages (#2922 PR B).
fn repo_exposure_artifact_content_digest(
    path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    value
        .pointer("/artifact/content_sha256")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "artifact {} is missing artifact.content_sha256",
                path.display()
            )
            .into()
        })
}

fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn bind_repo_exposure_fixture_with_worktree(
    root: &Path,
    source: &Path,
    destination: &Path,
    worktree: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(source)?)?;
    value["schema_version"] = serde_json::Value::String("0.3".to_string());
    value["run_status"] = serde_json::Value::String("complete".to_string());
    let head = concrete_fixture_repository_head(root)?;
    let root_identity = root.canonicalize()?.to_string_lossy().replace('\\', "/");
    let placeholder = serde_json::Value::String(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    );
    value["artifact"] = serde_json::json!({
        "kind": "repo_exposure",
        "schema_version": "1",
        "canonicalization": "raw_json_placeholder_v1",
        "producer": {"tool": "ripr", "version": "0.11.0"},
        "repository": {"root": root_identity, "head": head},
        "analysis": {"format": "repo-exposure-json", "mode": "draft", "base_revision": null, "input_identity": "input:v4:fnv1a64:00000000000000f1", "command": "ripr check --format repo-exposure-json", "profile": "draft", "worktree": worktree},
        "snapshot_identity": format!("snapshot:input:v4:fnv1a64:00000000000000f1;revision:{head}"),
        "content_sha256": placeholder,
    });
    let raw = serde_json::to_string_pretty(&value)?;
    std::fs::write(destination, recommit_repo_exposure_json(raw))?;
    Ok(())
}

fn normalize_generated_at(text: String) -> String {
    text.lines()
        .map(|line| {
            if line.trim_start().starts_with("\"generated_at\":") {
                "  \"generated_at\": \"2026-05-09T12:00:00Z\",".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_agent_verify_fixture(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut value: serde_json::Value = serde_json::from_str(text)?;
    // The bound artifact content commitments embed the live repository head
    // (#2922 PR B), so the golden cannot pin them; normalize both sides.
    if let Some(inputs) = value
        .get_mut("inputs")
        .and_then(serde_json::Value::as_object_mut)
    {
        for key in ["before_content_sha256", "after_content_sha256"] {
            if inputs.contains_key(key) {
                inputs.insert(
                    key.to_string(),
                    serde_json::Value::String("<content_sha256>".to_string()),
                );
            }
        }
    }
    let mut rendered = serde_json::to_string_pretty(&value)?;
    rendered.push('\n');
    Ok(rendered)
}

fn normalize_agent_receipt_fixture(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut value: serde_json::Value = serde_json::from_str(text)?;
    if let Some(provenance) = value
        .get_mut("provenance")
        .and_then(serde_json::Value::as_object_mut)
    {
        provenance.insert(
            "generated_at".to_string(),
            serde_json::Value::String("<generated_at>".to_string()),
        );
        provenance.insert(
            "ripr_version".to_string(),
            serde_json::Value::String("<ripr_version>".to_string()),
        );
        for artifact in ["before_artifact", "after_artifact", "verify_artifact"] {
            if let Some(artifact) = provenance
                .get_mut(artifact)
                .and_then(serde_json::Value::as_object_mut)
            {
                artifact.insert(
                    "sha256".to_string(),
                    serde_json::Value::String("<sha256>".to_string()),
                );
            }
        }
    }
    let object = value
        .as_object_mut()
        .ok_or("agent receipt fixture should be a JSON object")?;
    if object.contains_key("analysis_outcome_error") {
        object.insert(
            "analysis_outcome_error".to_string(),
            serde_json::Value::String("<analysis_outcome_error>".to_string()),
        );
    }
    // Only `analysis_outcome.analysis_complete` is asserted separately below;
    // the counts and semantic digest are source-sensitive, while this fixture
    // pins the receipt envelope and projection status.
    if object.contains_key("analysis_outcome") {
        object.insert("analysis_outcome".to_string(), serde_json::Value::Null);
    }
    let mut rendered = serde_json::to_string_pretty(&value)?;
    rendered.push('\n');
    Ok(rendered)
}

fn normalize_unchanged_repo_exposure_producer_fixture(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let fixture_root = workspace_root().join("fixtures/boundary_gap/input");
    let canonical_root = fixture_root
        .canonicalize()?
        .to_string_lossy()
        .replace('\\', "/");
    let produced_root = json_pointer_str(&value, "/artifact/repository/root")?;
    assert_ne!(
        produced_root, ".",
        "the production repo-exposure route must emit a canonical absolute root"
    );
    assert_eq!(produced_root, canonical_root);

    let stable_head = "8a00693e680c8ad5172d5191bbb08484cbd5e300";
    value["artifact"]["repository"]["root"] = serde_json::json!(".");
    value["artifact"]["repository"]["head"] = serde_json::json!(stable_head);
    value["artifact"]["analysis"]["worktree"] = serde_json::json!("dirty");
    let input_identity = json_pointer_str(&value, "/artifact/analysis/input_identity")?;
    value["artifact"]["snapshot_identity"] =
        serde_json::json!(format!("snapshot:{input_identity};revision:{stable_head}"));
    let mut raw = serde_json::to_string_pretty(&value)?;
    raw.push('\n');
    serde_json::from_str(&recommit_repo_exposure_json(raw)).map_err(Into::into)
}

fn assert_repo_exposure_rejects_mutation(
    snapshot: &serde_json::Value,
    mutate: impl FnOnce(&mut serde_json::Value),
    expected_error: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = unique_temp_workspace("unchanged-producer-mutation")
        .file_name()
        .ok_or("mutation fixture name should exist")?
        .to_owned();
    let root = workspace_root().join("target/ripr").join(name);
    std::fs::create_dir_all(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    std::fs::write(&before, serde_json::to_string_pretty(snapshot)?)?;
    let mut mutated = snapshot.clone();
    mutate(&mut mutated);
    std::fs::write(&after, serde_json::to_string_pretty(&mutated)?)?;
    let output = run_ripr_in_workspace(&[
        "agent",
        "verify",
        "--root",
        ".",
        "--before",
        before.to_str().ok_or("before path should be utf-8")?,
        "--after",
        after.to_str().ok_or("after path should be utf-8")?,
        "--json",
    ])?;
    assert_failure(&output);
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains(expected_error),
        "unexpected rejection: {stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn normalize_agent_receipt_fixture_rejects_non_object_json() -> Result<(), String> {
    for fixture in ["[]", "null"] {
        if normalize_agent_receipt_fixture(fixture).is_ok() {
            return Err(format!("non-object fixture should be rejected: {fixture}"));
        }
    }
    Ok(())
}

fn json_string_field(text: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\": \"");
    let start = text.find(&pattern)? + pattern.len();
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

fn json_pointer_str<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected string at JSON pointer `{pointer}`").into())
}

fn json_pointer_bool(
    value: &serde_json::Value,
    pointer: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("expected bool at JSON pointer `{pointer}`").into())
}

fn agent_brief_sample_workspace(
    label: &str,
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    // Keep the sample tree representative of a Cargo package.  The source
    // and integration-test files intentionally live in different directories;
    // without a manifest the authority model must treat them as distinct
    // manifest-less packages, so the packet cannot recover the related test.
    std::fs::create_dir_all(&root)?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"agent-brief-sample\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample/src/lib.rs"),
        root.join("src/lib.rs"),
    )?;
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample/tests/pricing.rs"),
        root.join("tests/pricing.rs"),
    )?;
    let diff = root.join("change.diff");
    std::fs::write(
        &diff,
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -8,1 +8,1 @@\n-old\n+new\n",
    )?;
    Ok((root, diff))
}

#[test]
fn concrete_fixture_repository_head_matches_a_committed_fixture()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("concrete-head");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let actual = concrete_fixture_repository_head(&root)?;
    let expected = run_command(
        "git",
        Some(&root),
        &["rev-parse", "--verify", "HEAD^{commit}"],
    )?;
    if !expected.status.success() {
        return Err(format!(
            "fixture rev-parse failed: {}",
            bounded_command_text(&expected.stderr)
        )
        .into());
    }
    let expected = String::from_utf8(expected.stdout)?
        .trim()
        .to_ascii_lowercase();
    std::fs::remove_dir_all(&root)?;
    if actual != expected {
        return Err(format!("fixture HEAD mismatch: actual={actual} expected={expected}").into());
    }
    Ok(())
}

#[test]
fn fixture_repository_head_rejects_unresolved_head_and_malformed_actions_fallback()
-> Result<(), String> {
    let result = select_fixture_repository_head(
        false,
        b"HEAD\n",
        b"fatal: ambiguous argument 'HEAD'",
        Some("not-a-commit"),
    );
    let Err(error) = result else {
        return Err("unresolved HEAD unexpectedly became fixture authority".to_string());
    };
    if !error.contains("stdout: HEAD") || !error.contains("fatal: ambiguous argument") {
        return Err(format!("fixture HEAD error lost command context: {error}"));
    }
    Ok(())
}

#[test]
fn fixture_repository_head_accepts_valid_actions_checkout_fallback_after_git_failure()
-> Result<(), String> {
    let fallback = "0123456789abcdef0123456789abcdef01234567";
    let actual = select_fixture_repository_head(
        false,
        b"HEAD\n",
        b"fatal: ambiguous argument 'HEAD'",
        Some(fallback),
    )?;
    if actual != fallback {
        return Err(format!(
            "valid Actions checkout fallback changed: actual={actual} expected={fallback}"
        ));
    }
    Ok(())
}

#[test]
fn fixture_repository_head_does_not_replace_malformed_success_output_with_fallback()
-> Result<(), String> {
    let fallback = "0123456789abcdef0123456789abcdef01234567";
    let result = select_fixture_repository_head(true, b"HEAD\n", b"", Some(fallback));
    let Err(error) = result else {
        return Err("successful but symbolic git output used the Actions fallback".to_string());
    };
    if !error.contains("non-concrete repository HEAD `HEAD`") {
        return Err(format!("unexpected non-concrete HEAD error: {error}"));
    }
    Ok(())
}

#[test]
fn fixture_repository_head_accepts_only_an_optional_git_line_ending() -> Result<(), String> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    for stdout in [
        commit.to_string(),
        format!("{commit}\n"),
        format!("{commit}\r\n"),
    ] {
        let actual = select_fixture_repository_head(true, stdout.as_bytes(), b"", None)?;
        if actual != commit {
            return Err(format!(
                "optional line ending changed commit identity: {actual}"
            ));
        }
    }
    Ok(())
}

#[test]
fn fixture_repository_head_rejects_whitespace_padded_success_output() -> Result<(), String> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    for stdout in [
        format!(" {commit}\n"),
        format!("{commit} \n"),
        format!("{commit}\n\n"),
    ] {
        let result = select_fixture_repository_head(true, stdout.as_bytes(), b"", None);
        if result.is_ok() {
            return Err(format!(
                "whitespace-padded commit became authority: {stdout:?}"
            ));
        }
    }
    Ok(())
}

/// The `ripr --version` line this build must produce, derived without
/// `build.rs`'s output: the package version plus the commit, with `-dirty`
/// when the inputs that decide the binary differ from it. A packaged crate
/// (this file ships in it) takes the commit from `.cargo_vcs_info.json`; a
/// checkout that tracks this crate takes it from Git; anything else has no
/// commit, and the line is the bare version.
fn expected_version_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Ok(record) = std::fs::read_to_string(crate_dir.join(".cargo_vcs_info.json")) {
        return match build_commit_record::parse_cargo_vcs_info(&record) {
            Some((commit, true)) => format!("ripr {version} ({commit}-dirty)\n"),
            Some((commit, false)) => format!("ripr {version} ({commit})\n"),
            None => format!("ripr {version}\n"),
        };
    }
    let git_stdout = |args: &[&str]| {
        run_command("git", Some(crate_dir), args)
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    if git_stdout(&["ls-files", "--error-unmatch", "--", "Cargo.toml"]).is_none() {
        return format!("ripr {version}\n");
    }
    let Some(head) = git_stdout(&["rev-parse", "--verify", "HEAD"]) else {
        return format!("ripr {version}\n");
    };
    let Some(status) = git_stdout(&[
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--",
        "src",
        "Cargo.toml",
        "build.rs",
        ":(top)Cargo.toml",
        ":(top)Cargo.lock",
        ":(top).cargo",
    ]) else {
        return format!("ripr {version}\n");
    };
    let suffix = if status.is_empty() { "" } else { "-dirty" };
    format!("ripr {version} ({head}{suffix})\n")
}

#[test]
fn version_is_exact_and_precedes_help_or_output_flags() {
    let expected = expected_version_line();
    for args in [
        &["--version"][..],
        &["-V"][..],
        &["--version", "--json"][..],
        &["--json", "--version"][..],
        &["--version", "--help"][..],
        &["--verbose", "--version"][..],
        &["--version", "--verbose"][..],
    ] {
        let output = run_ripr(args);
        assert_success(&output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "version argv {args:?} must emit only the package version"
        );
        assert!(
            output.stderr.is_empty(),
            "version argv {args:?} emitted diagnostics: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let lsp = run_ripr(&["lsp", "--version"]);
    assert_success(&lsp);
    assert_eq!(
        String::from_utf8_lossy(&lsp.stdout),
        format!("ripr-lsp {}\n", env!("CARGO_PKG_VERSION")),
        "command-local LSP version must retain its public output"
    );
    assert!(lsp.stderr.is_empty());
}

#[test]
fn workflow_help_is_bounded_and_repo_byte_identical() -> Result<(), String> {
    let root = unique_temp_workspace("workflow-help");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&root)?;
        // A git repo so the runs resolve a real workspace; the snapshot then
        // covers every byte the workflow-help route could touch (#4824).
        run_git(&root, &["init"])?;
        let before = snapshot_tree(&root)?;
        let root_text = root.to_string_lossy().to_string();
        for args in [
            &["help", "workflow"][..],
            &["help", "workflow", "repair-gap"][..],
            &["help", "workflow", "guided-adoption"][..],
        ] {
            let output = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), args)?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if !output.status.success() {
                return Err(format!(
                    "ripr {args:?} failed in the isolated repo: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
                    output.status.code()
                )
                .into());
            }
            if stdout.len() > 20_000 {
                return Err(format!(
                    "ripr {args:?} render escaped its bound: {} bytes",
                    stdout.len()
                )
                .into());
            }
            if stdout.contains(&root_text) || stderr.contains(&root_text) {
                return Err(format!("ripr {args:?} render leaked the host workspace path").into());
            }
        }
        let after = snapshot_tree(&root)?;
        if after != before {
            return Err(format!(
                "workflow help changed the isolated repo:\n{}",
                snapshot_diff(&before, &after)
            )
            .into());
        }
        Ok(())
    })();
    let cleanup = std::fs::remove_dir_all(&root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error.to_string()),
        (Ok(()), Err(error)) => Err(format!("cleanup failed: {error}")),
        (Err(error), Err(cleanup_error)) => Err(format!(
            "workflow help contract failed: {error}; cleanup failed: {cleanup_error}"
        )),
    }
}

#[test]
fn help_json_is_deterministic_across_roots_env_and_side_effect_free() -> Result<(), String> {
    let root_a = unique_temp_workspace("help-json-a");
    let root_b = unique_temp_workspace("help-json-b");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for root in [&root_a, &root_b] {
            std::fs::create_dir_all(root)?;
            run_git(root, &["init"])?;
        }
        let before_a = snapshot_tree(&root_a)?;
        let before_b = snapshot_tree(&root_b)?;
        let root_a_text = root_a.to_string_lossy().to_string();
        let root_b_text = root_b.to_string_lossy().to_string();
        let bin = env!("CARGO_BIN_EXE_ripr");

        let baseline = run_command(bin, Some(&root_a), &["help", "--json"])?;
        if !baseline.status.success() {
            return Err(format!(
                "ripr help --json failed: status={:?}, stderr={}",
                baseline.status.code(),
                String::from_utf8_lossy(&baseline.stderr)
            )
            .into());
        }
        if !baseline.stderr.is_empty() {
            return Err(format!(
                "ripr help --json wrote to stderr: {:?}",
                String::from_utf8_lossy(&baseline.stderr)
            )
            .into());
        }
        let baseline_stdout = baseline.stdout.clone();

        // Required control 1: two different workspace roots produce the same
        // bytes, and neither root's path leaks into the document.
        let other_root = run_command(bin, Some(&root_b), &["help", "--json"])?;
        if other_root.stdout != baseline_stdout {
            return Err("help --json bytes differ across workspace roots".into());
        }
        let stdout_text = String::from_utf8_lossy(&baseline_stdout).to_string();
        if stdout_text.contains(&root_a_text) || stdout_text.contains(&root_b_text) {
            return Err("help --json leaked a host workspace path".into());
        }

        // Required control 2: TTY/color/width/locale knobs cannot move the
        // document. The strict grammar also rejects every extra flag, so
        // `help --json --quiet` fails closed instead of quietly re-shaping
        // the machine document.
        let env_cases: [&[(&str, &str)]; 6] = [
            &[("NO_COLOR", "1")],
            &[("FORCE_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
            &[("TERM", "dumb")],
            &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
            &[("COLUMNS", "40")],
            &[("COLUMNS", "200"), ("LC_ALL", "C")],
        ];
        for env in env_cases {
            let output = run_command_with_env(bin, &root_a, &["help", "--json"], env)?;
            if !output.status.success() {
                return Err(format!(
                    "ripr help --json failed under env {env:?}: status={:?}, stderr={}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
            if output.stdout != baseline_stdout {
                return Err(format!("help --json bytes moved under env {env:?}").into());
            }
            if !output.stderr.is_empty() {
                return Err(format!(
                    "help --json wrote to stderr under env {env:?}: {:?}",
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
        }
        let rejected = run_command(bin, Some(&root_a), &["help", "--json", "--quiet"])?;
        if rejected.status.success() {
            return Err("help --json --quiet must fail closed, not silently merge flags".into());
        }
        // The global verbosity flag is extracted before dispatch; combined
        // with the machine route it must fail closed with the same usage
        // family instead of emitting a document plus a stderr diagnostic.
        for verbose_args in [
            &["help", "--json", "--verbose"][..],
            &["--verbose", "help", "--json"][..],
            &["help", "--json", "-v"][..],
        ] {
            let rejected = run_command(bin, Some(&root_a), verbose_args)?;
            if rejected.status.success() {
                return Err(format!(
                    "ripr {verbose_args:?} must fail closed, not bypass the machine grammar"
                )
                .into());
            }
            if String::from_utf8_lossy(&rejected.stderr).contains("verbose mode enabled") {
                return Err(format!(
                    "ripr {verbose_args:?} leaked the verbose diagnostic on the machine route"
                )
                .into());
            }
        }

        // Versioned-shape contract: schema_version 1, nonempty sections, and a
        // sha256 hex digest. A partial document must not pass as complete.
        let value: serde_json::Value = serde_json::from_slice(&baseline_stdout)
            .map_err(|error| format!("help --json is not valid JSON: {error}"))?;
        if value["schema_version"] != serde_json::json!(1) {
            return Err(format!(
                "help --json schema_version moved: {}",
                value["schema_version"]
            )
            .into());
        }
        let commands = value["commands"]
            .as_array()
            .ok_or("help --json commands missing")?;
        let workflows = value["workflows"]
            .as_array()
            .ok_or("help --json workflows missing")?;
        if commands.is_empty() || workflows.is_empty() {
            return Err("help --json emitted an empty section".into());
        }
        for row in commands {
            if row.get("relation").is_none() || row.get("discovery").is_none() {
                return Err(format!(
                    "help --json command row {} lost its relation/discovery projection",
                    row["id"]
                )
                .into());
            }
        }
        let digest = value["catalog_digest"]
            .as_str()
            .ok_or("help --json catalog_digest missing")?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("help --json digest is not sha256 hex: {digest:?}").into());
        }
        if value.get("limitations").is_none() || value.get("non_claims").is_none() {
            return Err("help --json lost its limitations/non_claims boundaries".into());
        }

        // Required control 11: the route proves zero workspace side effects.
        let after_a = snapshot_tree(&root_a)?;
        let after_b = snapshot_tree(&root_b)?;
        if after_a != before_a {
            return Err(format!(
                "help --json changed workspace A; diff follows\n{}",
                snapshot_diff(&before_a, &after_a)
            )
            .into());
        }
        if after_b != before_b {
            return Err(format!(
                "help --json changed workspace B; diff follows\n{}",
                snapshot_diff(&before_b, &after_b)
            )
            .into());
        }
        Ok(())
    })();
    let cleanup_a = std::fs::remove_dir_all(&root_a);
    let cleanup_b = std::fs::remove_dir_all(&root_b);
    match (result, cleanup_a, cleanup_b) {
        (Ok(()), Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(()), Ok(())) => Err(error.to_string()),
        (result, cleanup_a, cleanup_b) => Err(format!(
            "help --json contract result={result:?}, cleanup_a={cleanup_a:?}, cleanup_b={cleanup_b:?}"
        )),
    }
}

#[test]
fn isolated_installed_binary_version_contract_is_side_effect_free() -> Result<(), String> {
    let root = unique_temp_workspace("version-installed");
    // Coverage runtimes own only this external directory; the product prefix
    // below remains a strict snapshot and rejects every non-harness mutation.
    let coverage_root =
        inherited_coverage_is_enabled().then(|| unique_temp_workspace("version-installed-profile"));
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if let Some(profile_root) = &coverage_root {
            std::fs::create_dir_all(profile_root)?;
        }
        let bin_dir = root.join("bin");
        std::fs::create_dir_all(&bin_dir)?;
        let source_binary = std::path::Path::new(env!("CARGO_BIN_EXE_ripr"));
        let installed_binary = bin_dir.join(if cfg!(windows) { "ripr.exe" } else { "ripr" });
        std::fs::copy(source_binary, &installed_binary)?;

        let config_path = root.join("ripr.toml");
        std::fs::write(&config_path, "this is not valid TOML\n")?;
        let config_before = std::fs::read(&config_path)?;
        let before = snapshot_tree(&root)?;
        let artifact_path = root.join("target/ripr/reports/version.json");
        let expected_version = expected_version_line();
        let expected_lsp_version = format!("ripr-lsp {}\n", env!("CARGO_PKG_VERSION"));
        let workspace_text = workspace_root().to_string_lossy().to_string();
        let cases = [
            vec!["--version"],
            vec!["-V"],
            vec!["--help", "--version"],
            vec!["-v", "--version"],
            vec!["--version", "-v"],
            vec!["check", "--version"],
            vec!["check", "--diff", "--version"],
            vec!["lsp", "--version"],
        ];

        for args in cases {
            let args = args.as_slice();
            let profile = coverage_root
                .as_ref()
                .map(|root| root.join("ripr-%p-%m.profraw"));
            let output = run_isolated_binary(&installed_binary, &root, args, profile.as_deref())?;
            let stdout = String::from_utf8(output.stdout.clone())?;
            let stderr = String::from_utf8(output.stderr.clone())?;
            if args[0] == "lsp" {
                if !output.status.success() || stdout != expected_lsp_version || !stderr.is_empty()
                {
                    return Err(format!(
                        "isolated lsp version contract failed for {args:?}: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
                        output.status.code()
                    )
                    .into());
                }
            } else if args[0] == "check" {
                let is_diff_file_case = args.len() == 3 && args[1] == "--diff";
                let has_config_error = stderr.contains("invalid ripr.toml");
                if output.status.success()
                    || stdout.contains("ripr —")
                    || stdout.contains(expected_version.trim_end())
                    || stderr.contains(expected_version.trim_end())
                {
                    return Err(format!(
                        "check version-like input escaped its command boundary for {args:?}: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
                        output.status.code()
                    )
                    .into());
                }
                if is_diff_file_case
                    && !has_config_error
                    && (!stderr.contains("resolved workspace root to ")
                        || !stderr.contains("failed to read diff file --version"))
                {
                    return Err(format!(
                        "check --diff --version did not report the expected workspace/diff diagnostic: status={:?}, stderr={stderr:?}",
                        output.status.code()
                    )
                    .into());
                }
            } else if !output.status.success() || stdout != expected_version || !stderr.is_empty() {
                return Err(format!(
                    "isolated top-level version contract failed for {args:?}: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
                    output.status.code()
                )
                .into());
            }

            let after = snapshot_tree(&root)?;
            if after != before {
                return Err(format!(
                    "version-like input changed the isolated cwd for {args:?}:\n{}",
                    snapshot_diff(&before, &after)
                )
                .into());
            }
            if !config_path.exists() || artifact_path.exists() {
                return Err(format!(
                    "version-like input changed config/artifact paths for {args:?}"
                )
                .into());
            }
            let config_after = std::fs::read(&config_path)?;
            if config_after != config_before {
                return Err(format!(
                    "version-like input changed the pre-existing config for {args:?}"
                )
                .into());
            }
            let is_diff_file_case = args.len() == 3 && args[1] == "--diff";
            if !is_diff_file_case
                && (stdout.contains(&workspace_text) || stderr.contains(&workspace_text))
            {
                return Err(format!(
                    "version-like input depended on the workspace path for {args:?}"
                )
                .into());
            }
        }
        Ok(())
    })();
    let cleanup = std::fs::remove_dir_all(&root);
    let profile_cleanup = cleanup_temp_dir(coverage_root.as_deref());
    match (result, cleanup, profile_cleanup) {
        (Ok(()), Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(()), Ok(())) => Err(error.to_string()),
        (Ok(()), Err(error), Ok(())) => Err(format!("cleanup failed: {error}")),
        (Ok(()), Ok(()), Err(error)) => Err(format!("coverage cleanup failed: {error}")),
        (Err(error), Err(cleanup_error), Ok(())) => Err(format!(
            "version contract failed: {error}; cleanup failed: {cleanup_error}"
        )),
        (Err(error), Ok(()), Err(profile_error)) => Err(format!(
            "version contract failed: {error}; coverage cleanup failed: {profile_error}"
        )),
        (Ok(()), Err(cleanup_error), Err(profile_error)) => Err(format!(
            "cleanup failed: {cleanup_error}; coverage cleanup failed: {profile_error}"
        )),
        (Err(error), Err(cleanup_error), Err(profile_error)) => Err(format!(
            "version contract failed: {error}; cleanup failed: {cleanup_error}; coverage cleanup failed: {profile_error}"
        )),
    }
}

#[test]
fn help_runs() {
    let output = run_ripr(&["--help"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("find changed code where nearby tests"));
    assert!(stdout.contains("(Rust; Python and TypeScript in preview)"));
    assert!(stdout.contains("Usage:"));
}

/// The default screen is a first screen, not the catalog. Substring checks alone
/// would still pass if it grew back into the 91-line command dump it used to be,
/// so this pins the four things a first-time reader must get without opting in:
/// a bounded screen, one runnable first action, the advisory boundary, and the
/// route to the rest (#1613).
#[test]
fn help_leads_with_a_bounded_first_screen_that_routes_onward() {
    let output = run_ripr(&["--help"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let lines = stdout.lines().count();
    assert!(
        lines <= 40,
        "the default help screen should stay scannable, got {lines} lines:\n{stdout}"
    );
    assert!(
        stdout.contains("ripr doctor"),
        "the first screen should name a runnable first action, got:\n{stdout}"
    );
    assert!(
        stdout.contains("does not run mutants"),
        "the advisory boundary must not be behind --all, got:\n{stdout}"
    );
    assert!(
        stdout.contains("ripr help --all"),
        "the first screen should route to the full reference, got:\n{stdout}"
    );
}

/// `--all` is the escape hatch the bounded screen promises, so it has to be
/// reachable from the binary and actually carry the commands the short screen
/// drops.
#[test]
fn help_all_prints_the_full_command_reference() {
    let output = run_ripr(&["help", "--all"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let short = run_ripr(&["--help"]);
    assert_success(&short);
    let short_stdout = String::from_utf8_lossy(&short.stdout);
    assert!(
        stdout.lines().count() > short_stdout.lines().count(),
        "help --all should be longer than the default screen, got {} vs {} lines",
        stdout.lines().count(),
        short_stdout.lines().count()
    );

    for command in ["ripr pr-summary", "ripr annotations", "ripr gate evaluate"] {
        assert!(
            stdout.contains(command),
            "help --all should document `{command}`, got:\n{stdout}"
        );
    }
}

#[test]
fn unknown_command_typo_reports_nearest_known_command() {
    let output = run_ripr(&["chekc"]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command \"chekc\". Did you mean `check`? Run `ripr --help`."),
        "stderr should include a typo recovery hint, got:
{stderr}"
    );
}

#[test]
fn check_human_output_reports_sample_findings() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff();
    assert!(diff.exists());

    let diff = diff.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--diff", &diff]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Summary: 4 probe(s)"));
    assert!(stdout.contains("Start here:"));
    assert!(stdout.contains("Static exposure: weak (weakly_exposed, "));
    assert!(stdout.contains("Evidence:"));
    // #4320: the digest missing-discriminator line discloses its one-of-N
    // window, so the pin carries the parenthesized window opening.
    assert!(stdout.contains("Missing discriminator ("));
    assert!(stdout.contains("Next step:"));
    assert!(stdout.contains("lower-priority finding(s) omitted"));
    assert!(stdout.contains("--format human-full"));
}

#[test]
fn check_no_unchanged_tests_restates_the_recall_tradeoff_on_stderr() {
    // #4946(a): the flag's recall/cost tradeoff is restated on stderr where
    // the user waits, and only while the flag is actually active.
    let root = workspace_root().display().to_string();
    let diff = sample_diff();
    assert!(diff.exists());
    let diff = diff.display().to_string();

    let with_flag = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--no-unchanged-tests",
    ]);
    assert_success(&with_flag);
    let stderr = String::from_utf8_lossy(&with_flag.stderr);
    assert!(
        stderr.contains("unchanged tests are excluded from the index"),
        "the active flag must restate its tradeoff on stderr; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Reach evidence cannot name tests the diff does not touch"),
        "the note must name the recall effect, not only the speedup; got:\n{stderr}"
    );

    let without_flag = run_ripr(&["check", "--root", &root, "--diff", &diff]);
    assert_success(&without_flag);
    let stderr = String::from_utf8_lossy(&without_flag.stderr);
    assert!(
        !stderr.contains("unchanged tests are excluded from the index"),
        "the default run must not claim the flag is active; got:\n{stderr}"
    );
}

/// Small single-crate fixture with `include_unchanged_tests = false` set in
/// `ripr.toml` plus a one-file diff, for the config-driven exclusion pins.
/// The tests analyze the live temp tree and may write caches under it, so
/// they never point the binary at this repository.
fn config_unchanged_fixture(tag: &str) -> Result<std::path::PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-cli-smoke-{tag}-{stamp}"));
    std::fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("create src dir: {error}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"ripr-cli-smoke-unchanged\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|error| format!("write Cargo.toml: {error}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|error| format!("write src/lib.rs: {error}"))?;
    std::fs::write(
        root.join("ripr.toml"),
        "[analysis]\ninclude_unchanged_tests = false\n",
    )
    .map_err(|error| format!("write ripr.toml: {error}"))?;
    std::fs::write(
        root.join("control.diff"),
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n+pub fn over_threshold(amount: i32, threshold: i32, margin: i32) -> bool {\n",
    )
    .map_err(|error| format!("write control.diff: {error}"))?;
    Ok(root)
}

#[test]
fn check_config_driven_no_unchanged_tests_names_the_config_knob() -> Result<(), String> {
    // #4946(a) review: when the exclusion comes from ripr.toml rather than
    // the CLI flag, the note must name the config knob and its repair route;
    // "drop the flag" is unreachable advice because there is no positive CLI
    // counterpart, so the flag phrase must stay absent.
    let root = config_unchanged_fixture("config-unchanged")?;
    let root_arg = root.display().to_string();
    let diff_arg = root.join("control.diff").display().to_string();
    let out = run_ripr(&["check", "--root", &root_arg, "--diff", &diff_arg]);
    assert_success(&out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[analysis] include_unchanged_tests = false"),
        "the config-driven exclusion must be disclosed with the owning knob; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Set it to true or remove it from ripr.toml"),
        "the note must name the config repair route; got:\n{stderr}"
    );
    assert!(
        !stderr.contains("Drop the flag"),
        "config-driven exclusion must not advise dropping the flag; got:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn check_repo_formats_do_not_claim_the_unchanged_tests_index_tradeoff() -> Result<(), String> {
    // #4946(a) review: repo-scoped formats run their own corpus walks that
    // never read `include_unchanged_tests`, so the smaller/faster-index note
    // must stay absent there even when the config sets it false.
    let root = config_unchanged_fixture("repo-unchanged")?;
    let root_arg = root.display().to_string();
    let out = run_ripr(&["check", "--root", &root_arg, "--format", "repo-badge-json"]);
    assert_success(&out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("unchanged tests are excluded from the index"),
        "repo-scoped formats must not claim an index narrowing they do not perform; got:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn check_from_a_subcrate_discloses_workspace_root_and_honors_explicit_root() -> Result<(), String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let subcrate = workspace_root().join("crates/ripr");
    let implicit = run_command(
        bin,
        Some(&subcrate),
        &["check", "--base", "HEAD", "--format", "json"],
    )
    .map_err(|error| format!("run implicit-root check: {error}"))?;
    assert_success(&implicit);
    let expected_root = workspace_root()
        .canonicalize()
        .map_err(|error| format!("canonicalize workspace root: {error}"))?;
    let expected_disclosure = format!(
        "ripr: resolved workspace root to {} (Cargo.toml contains [workspace])",
        expected_root.display()
    );
    assert!(
        String::from_utf8_lossy(&implicit.stderr).contains(&expected_disclosure),
        "stderr:\n{}",
        String::from_utf8_lossy(&implicit.stderr)
    );

    let root = workspace_root().display().to_string();
    let explicit = run_command(
        bin,
        Some(&subcrate),
        &[
            "check", "--root", &root, "--base", "HEAD", "--format", "json",
        ],
    )
    .map_err(|error| format!("run explicit-root check: {error}"))?;
    assert_success(&explicit);
    assert!(
        !String::from_utf8_lossy(&explicit.stderr).contains("resolved workspace root to"),
        "explicit --root must skip implicit resolution; stderr:\n{}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    Ok(())
}

#[test]
fn check_from_a_package_source_dir_analyzes_the_crate_root() -> Result<(), String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root = unique_external_workspace("check-package-subdir")?;
    std::fs::create_dir_all(&root).map_err(|error| format!("create fixture: {error}"))?;
    let outcome = check_package_subdir_runs(bin, &root);
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove fixture: {error}"))?;
    let (from_root, from_src, expected_root) = outcome?;
    assert_success(&from_root);
    assert_success(&from_src);

    let findings = |output: &std::process::Output| -> Result<u64, String> {
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("parse check json: {error}"))?;
        json.pointer("/summary/findings")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "summary.findings missing".to_string())
    };
    let root_findings = findings(&from_root)?;
    assert!(
        root_findings > 0,
        "fixture must produce a finding from the root"
    );
    assert_eq!(
        findings(&from_src)?,
        root_findings,
        "a run from src/ must analyze the crate, not scope the diff away (#4610)"
    );
    let stderr = String::from_utf8_lossy(&from_src.stderr);
    let disclosure = format!(
        "ripr: resolved workspace root to {} (nearest Cargo.toml)",
        expected_root.display()
    );
    assert!(stderr.contains(&disclosure), "stderr:\n{stderr}");
    Ok(())
}

/// Commit the boundary_gap crate, change its boundary, and run
/// `check --base HEAD~1` once from the crate root and once from `src/`.
fn check_package_subdir_runs(
    bin: &str,
    root: &Path,
) -> Result<(std::process::Output, std::process::Output, PathBuf), String> {
    init_producer_fixture_repo(root).map_err(|error| format!("init fixture repo: {error}"))?;
    let lib = root.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).map_err(|error| format!("read lib: {error}"))?;
    let changed = source.replacen(">= discount_threshold", "> discount_threshold", 1);
    assert_ne!(changed, source, "fixture boundary must be present");
    std::fs::write(&lib, changed).map_err(|error| format!("write lib: {error}"))?;
    let commit = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-qam",
            "change boundary",
        ],
    )
    .map_err(|error| format!("commit change: {error}"))?;
    assert!(commit.status.success(), "change commit failed: {commit:?}");

    let args = ["check", "--base", "HEAD~1", "--format", "json"];
    let from_root =
        run_command(bin, Some(root), &args).map_err(|error| format!("run root check: {error}"))?;
    let from_src = run_command(bin, Some(&root.join("src")), &args)
        .map_err(|error| format!("run subdir check: {error}"))?;
    let expected_root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize fixture root: {error}"))?;
    Ok((from_root, from_src, expected_root))
}

#[test]
fn check_from_a_directory_without_a_workspace_does_not_disclose_resolution() -> Result<(), String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root = unique_external_workspace("check-no-workspace")?;
    std::fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("create fixture source: {error}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"check-no-workspace\"\nversion = \"0.1.0\"\n",
    )
    .map_err(|error| format!("write fixture manifest: {error}"))?;
    std::fs::write(root.join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n")
        .map_err(|error| format!("write fixture source: {error}"))?;
    let diff = root.join("change.diff");
    std::fs::write(
        &diff,
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn value() -> i32 { 1 }\n+pub fn value() -> i32 { 2 }\n",
    )
    .map_err(|error| format!("write fixture diff: {error}"))?;

    let diff_arg = diff.display().to_string();
    let output = run_command(
        bin,
        Some(&root),
        &["check", "--diff", &diff_arg, "--format", "json"],
    )
    .map_err(|error| format!("run no-workspace check: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();
    let no_disclosure = !stderr.contains("resolved workspace root to");
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove fixture: {error}"))?;
    if !success {
        return Err(format!(
            "no-workspace check failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        ));
    }
    if !no_disclosure {
        return Err(format!(
            "no-workspace check unexpectedly disclosed resolution\nstderr:\n{stderr}"
        ));
    }
    Ok(())
}

#[test]
fn config_validate_discovers_parent_config_from_nested_directory() -> Result<(), String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root = unique_external_workspace("config-validate-parent")?;
    let nested = root.join("crates/member");
    std::fs::create_dir_all(&nested)
        .map_err(|error| format!("create nested config directory: {error}"))?;
    let config_path = root.join("ripr.toml");
    std::fs::write(&config_path, "[analysis]\nmode = \"not-a-mode\"\n")
        .map_err(|error| format!("write invalid parent config: {error}"))?;
    let expected_config_path = config_path
        .canonicalize()
        .map_err(|error| format!("canonicalize parent config: {error}"))?;

    let output = run_command(bin, Some(&nested), &["config", "validate"])
        .map_err(|error| format!("run nested config validate: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let success = output.status.success();
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove config fixture: {error}"))?;
    if success {
        return Err(format!(
            "nested config validate unexpectedly accepted invalid parent config\nstdout:\n{}\nstderr:\n{stderr}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    if !stderr.contains(&expected_config_path.display().to_string()) {
        return Err(format!(
            "nested config validate did not report parent config path {}\nstderr:\n{stderr}",
            expected_config_path.display()
        ));
    }
    Ok(())
}

#[test]
fn check_human_navigation_commands_replay_custom_scope() -> Result<(), String> {
    let root = ".";
    let diff = "crates/ripr/examples/sample/example.diff";
    let output = run_ripr_in_workspace(&["check", "--root", root, "--diff", diff])
        .map_err(|err| err.to_string())?;
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let explain_line = stdout
        .lines()
        .find(|line| line.starts_with("  ripr explain "))
        .ok_or_else(|| format!("check output omitted explain command:\n{stdout}"))?;
    let context_line = stdout
        .lines()
        .find(|line| line.starts_with("  ripr context "))
        .ok_or_else(|| format!("check output omitted context command:\n{stdout}"))?;
    let explain_args = explain_line.split_whitespace().collect::<Vec<_>>();
    let context_args = context_line.split_whitespace().collect::<Vec<_>>();
    if explain_args.first() != Some(&"ripr") || context_args.first() != Some(&"ripr") {
        return Err(format!(
            "unexpected navigation commands:\n{explain_line}\n{context_line}"
        ));
    }

    let explain = run_ripr_in_workspace(&explain_args[1..]).map_err(|err| err.to_string())?;
    assert_success(&explain);
    let selector = explain_args
        .last()
        .copied()
        .ok_or_else(|| "explain command omitted selector".to_string())?;
    let context = run_ripr_in_workspace(&context_args[1..]).map_err(|err| err.to_string())?;
    assert_success(&context);
    if !String::from_utf8_lossy(&explain.stdout).contains(&format!(
        "Next: ripr context --root {root} --diff {diff} --at {selector}"
    )) {
        return Err("explain output omitted its scope-preserving context command".to_string());
    }
    if !String::from_utf8_lossy(&context.stdout).contains("\"version\": \"1.0\"") {
        return Err("context command did not return its JSON packet".to_string());
    }
    Ok(())
}

#[test]
fn check_navigation_replays_explicit_draft_over_configured_ready() -> Result<(), String> {
    let (root, diff) =
        agent_brief_sample_workspace("navigation-explicit-draft").map_err(|err| err.to_string())?;
    std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"ready\"\n")
        .map_err(|err| format!("write ripr.toml: {err}"))?;
    let root_arg = root.display().to_string();
    let diff_arg = diff.display().to_string();
    let output = run_ripr(&[
        "check", "--root", &root_arg, "--diff", &diff_arg, "--mode", "draft",
    ]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let explain_line = stdout
        .lines()
        .find(|line| line.starts_with("  ripr explain "))
        .ok_or_else(|| format!("check output omitted explain command:\n{stdout}"))?;
    let context_line = stdout
        .lines()
        .find(|line| line.starts_with("  ripr context "))
        .ok_or_else(|| format!("check output omitted context command:\n{stdout}"))?;
    if !explain_line.contains("--mode draft") || !context_line.contains("--mode draft") {
        return Err(format!(
            "explicit draft override was omitted:\n{explain_line}\n{context_line}"
        ));
    }
    // #2816: Do NOT split_whitespace the Bash-rendered display line into argv.
    // On Windows, backslash paths are POSIX-single-quoted by the shell_arg
    // encoder; split_whitespace preserves those quotes as literal bytes,
    // producing an invalid path (OS error 123). Instead, construct the argv
    // from known typed values and extract only the finding selector from the
    // display (it is a simple `probe:...` token with no shell quoting).
    let selector = explain_line
        .split_whitespace()
        .find(|token| token.starts_with("probe:"))
        .ok_or_else(|| format!("explain line has no probe selector:\n{explain_line}"))?
        .to_string();
    let explain_args: Vec<&str> = vec![
        "explain", "--root", &root_arg, "--diff", &diff_arg, "--mode", "draft", &selector,
    ];
    let explain = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), &explain_args)
        .map_err(|err| format!("run explicit-draft explain command: {err}"))?;
    assert_success(&explain);
    let context_args: Vec<&str> = vec![
        "context", "--root", &root_arg, "--diff", &diff_arg, "--mode", "draft", "--at", &selector,
    ];
    let context = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), &context_args)
        .map_err(|err| format!("run explicit-draft context command: {err}"))?;
    assert_success(&context);
    Ok(())
}

#[test]
fn check_json_output_has_stable_contract_fields() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--diff", &diff, "--json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.2""#));
    assert!(stdout.contains(r#""classification": "weakly_exposed""#));
    assert!(stdout.contains(r#""evidence_path""#));
    assert!(stdout.contains(r#""flow_sinks""#));
    assert!(stdout.contains(r#""assertion_texts""#));
    assert!(stdout.contains(r#""activation""#));
    assert!(stdout.contains(r#""missing_discriminators""#));
    assert!(stdout.contains(r#""oracle_kind""#));
    assert!(stdout.contains(r#""recommended_next_step""#));
    assert!(stdout.contains(r#""suggested_next_action""#));
}

// ── `check --suppression-policy` (#1441) ──

fn write_suppression_policy(label: &str, text: &str) -> Result<PathBuf, String> {
    let dir = unique_temp_workspace(label);
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let path = dir.join("ripr-suppressions.toml");
    std::fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(path)
}

#[test]
fn check_json_suppression_policy_marks_findings_and_adjusts_summary() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let policy = write_suppression_policy(
        "suppression-json",
        "schema_version = 1\n\n[[suppressions]]\nkind = \"exposure_gap\"\npath = \"crates/ripr/examples/sample/**\"\nreason = \"sample surface accepted for this smoke test\"\nowner = \"repo-owner\"\n",
    )?;
    let policy_arg = policy.display().to_string();

    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--json",
        "--suppression-policy",
        &policy_arg,
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("check JSON should parse: {err}\n{stdout}"))?;

    let findings = value["findings"]
        .as_array()
        .ok_or("findings must be an array")?;
    assert!(!findings.is_empty(), "sample diff must produce findings");
    for finding in findings {
        assert_eq!(
            finding["suppressed"], true,
            "every sample finding lives under the suppressed glob"
        );
        assert_eq!(finding["suppressed_by"], "crates/ripr/examples/sample/**");
    }
    assert_eq!(
        value["summary"]["suppressed_by_policy"].as_u64(),
        Some(findings.len() as u64)
    );
    // Per-class buckets count unsuppressed findings only.
    assert_eq!(value["summary"]["weakly_exposed"].as_u64(), Some(0));
    // `findings` stays the total rendered count.
    assert_eq!(
        value["summary"]["findings"].as_u64(),
        Some(findings.len() as u64)
    );
    assert_eq!(value["suppression_policy"]["path"], policy_arg.as_str());
    assert_eq!(
        value["suppression_policy"]["warnings"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    Ok(())
}

#[test]
fn check_human_suppression_policy_lists_suppressed_findings_compactly() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let policy = write_suppression_policy(
        "suppression-human",
        "schema_version = 1\n\n[[suppressions]]\nkind = \"exposure_gap\"\npath = \"crates/ripr/examples/sample/**\"\nreason = \"sample surface accepted for this smoke test\"\nowner = \"repo-owner\"\n",
    )?;
    let policy_arg = policy.display().to_string();

    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--suppression-policy",
        &policy_arg,
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Suppressed by policy"),
        "human output must disclose policy application: {stdout}"
    );
    assert!(stdout.contains("(selector: crates/ripr/examples/sample/**)"));
    assert!(
        !stdout.contains("Next step\n"),
        "suppressed findings must not render detailed blocks: {stdout}"
    );
    Ok(())
}

#[test]
fn check_github_over_broad_suppression_policy_emits_denominator_notice() -> Result<(), String> {
    // Production-path control for #4393: an over-broad path glob that hides
    // every sample finding must still emit a github denominator notice, not
    // zero bytes, and must not annotate the suppressed findings. Exit status
    // stays success (no gate/exit-code authority change).
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let policy = write_suppression_policy(
        "suppression-github-over-broad",
        "schema_version = 1\n\n[[suppressions]]\nkind = \"exposure_gap\"\npath = \"crates/ripr/examples/sample/**\"\nreason = \"repro: suppress all sample findings\"\nowner = \"qa\"\n",
    )?;
    let policy_arg = policy.display().to_string();

    let unfiltered = run_ripr(&[
        "check", "--root", &root, "--diff", &diff, "--format", "github",
    ]);
    assert_success(&unfiltered);
    let unfiltered_stdout = String::from_utf8_lossy(&unfiltered.stdout);
    let unfiltered_annotations = unfiltered_stdout
        .lines()
        .filter(|line| line.contains("file="))
        .count();
    assert!(
        unfiltered_annotations > 0,
        "unfiltered github output must annotate findings: {unfiltered_stdout}"
    );
    assert!(
        !unfiltered_stdout.contains("suppressed by policy"),
        "unfiltered run must not mention policy: {unfiltered_stdout}"
    );

    let filtered = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "github",
        "--suppression-policy",
        &policy_arg,
    ]);
    assert_success(&filtered);
    let stdout = String::from_utf8_lossy(&filtered.stdout);
    assert!(
        !stdout.is_empty(),
        "all-suppressed github output must not be silent"
    );
    let expected = format!(
        "::notice title=ripr::Annotated 0 of {unfiltered_annotations} static exposure finding(s); {unfiltered_annotations} suppressed by policy"
    );
    assert!(
        stdout.contains(&expected),
        "all-suppressed github output must name the denominator ({expected}): {stdout}"
    );
    assert!(
        !stdout.contains("file="),
        "suppressed findings must stay unannotated: {stdout}"
    );
    assert!(
        !stdout.contains("No static exposure findings found"),
        "all-suppressed is not an empty run: {stdout}"
    );
    Ok(())
}

#[test]
fn check_suppression_policy_missing_file_fails_closed() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();

    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--json",
        "--suppression-policy",
        "does/not/exist.toml",
    ]);
    assert_failure(&output);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read suppression policy"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_suppression_policy_rejects_unsupported_formats() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let policy = write_suppression_policy(
        "suppression-sarif",
        "schema_version = 1\n\n[[suppressions]]\nkind = \"exposure_gap\"\npath = \"crates/**\"\nreason = \"unused\"\nowner = \"repo-owner\"\n",
    )?;
    let policy_arg = policy.display().to_string();

    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "sarif",
        "--suppression-policy",
        &policy_arg,
    ]);
    assert_failure(&output);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--suppression-policy applies to the findings-based check formats"),
        "stderr: {stderr}"
    );
    Ok(())
}

// ── `gate evaluate --exception-policy` (#1442) ──

const SMOKE_PR_GUIDANCE_JSON: &str = r#"{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "summary": {"unchanged_tests": true},
  "comments": [],
  "summary_only": [],
  "suppressed": []
}"#;

const SMOKE_COMPLETE_GAP_LEDGER_JSON: &str = r#"{
  "gap_records": [
    {
      "gap_id": "gap:pricing",
      "source_currentness": "candidate_current",
      "canonical_gap_id": "pricing::discount::threshold",
      "seam_id": "seam-pricing-threshold",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "weakly_exposed",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "target_line": 12,
        "related_test": "tests/pricing.rs::above_threshold_gets_discount",
        "assertion_shape": "assert_eq!(price(threshold), discounted)",
        "missing_discriminator": "amount == discount_threshold",
        "changed_behavior": "amount == discount_threshold",
        "inspection_command": "ripr agent brief --root . --seam-id seam-pricing-threshold --json"
      },
      "anchor": {
        "file": "src/pricing.rs",
        "line": 88,
        "owner": "price",
        "dedupe_fingerprint": "gap:pricing"
      },
      "evidence_ids": ["seam-pricing"],
      "projection_eligibility": {
        "gate_candidate": {
          "eligible": true,
          "reason": "new_repairable_pr_local_gap"
        }
      },
      "verification_commands": ["cargo xtask fixtures boundary_gap"],
      "receipt_command": "ripr receipt write --gap pricing::discount::threshold",
      "safe_gate_predicate": {
        "policy_target_enabled": true,
        "suppressed": false,
        "waived": false,
        "acknowledged_only": false,
        "baseline_known": false,
        "preview_language": false,
        "static_unknown_only": false
      }
    }
  ]
}"#;

fn write_exception_ledger(
    dir: &std::path::Path,
    review_after: &str,
    expires: &str,
) -> Result<PathBuf, String> {
    let path = dir.join("quality-gate-exceptions.toml");
    let ledger = format!(
        "schema_version = 1\npolicy = \"quality-gate-exceptions\"\nstatus = \"active\"\ndue_review = \"fail\"\n\n[[exception]]\nid = \"total-burndown\"\nkind = \"temporary_burndown\"\nscope = \"ripr_plus_total\"\nowner = \"proof-lane\"\nreason = \"Pre-existing gaps predate the gate.\"\nfinal_target = \"unresolved total = 0\"\nevidence = \"target/receipts/quality/ripr-plus.json\"\nremoval_criteria = \"final mode requires zero\"\ncreated = \"2026-01-01\"\nreview_after = \"{review_after}\"\nexpires = \"{expires}\"\n"
    );
    std::fs::write(&path, ledger).map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(path)
}

#[test]
fn gate_evaluate_exception_policy_active_ledger_reports_and_passes() -> Result<(), String> {
    let dir = unique_temp_workspace("gate-exception-active");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let guidance = dir.join("comments.json");
    std::fs::write(&guidance, SMOKE_PR_GUIDANCE_JSON)
        .map_err(|err| format!("write guidance: {err}"))?;
    let ledger = write_exception_ledger(&dir, "9999-01-01", "9999-12-31")?;
    let out = dir.join("gate-decision.json");

    let output = run_ripr(&[
        "gate",
        "evaluate",
        "--pr-guidance",
        &guidance.display().to_string(),
        "--exception-policy",
        &ledger.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_success(&output);

    let decision = std::fs::read_to_string(&out).map_err(|err| format!("read out: {err}"))?;
    let value: serde_json::Value = serde_json::from_str(&decision)
        .map_err(|err| format!("gate decision should parse: {err}\n{decision}"))?;
    assert_eq!(value["exception_policy"]["active_count"], 1);
    assert_eq!(
        value["exception_policy"]["violations"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_ne!(value["status"], "blocked");
    Ok(())
}

#[test]
fn gate_evaluate_exception_policy_expired_ledger_blocks_with_nonzero_exit() -> Result<(), String> {
    let dir = unique_temp_workspace("gate-exception-expired");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let guidance = dir.join("comments.json");
    std::fs::write(&guidance, SMOKE_PR_GUIDANCE_JSON)
        .map_err(|err| format!("write guidance: {err}"))?;
    let ledger = write_exception_ledger(&dir, "2000-01-01", "2000-06-01")?;
    let out = dir.join("gate-decision.json");

    let output = run_ripr(&[
        "gate",
        "evaluate",
        "--pr-guidance",
        &guidance.display().to_string(),
        "--exception-policy",
        &ledger.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("quality_exception_expired"),
        "stderr should include the blocking exception-policy detail: {stderr}"
    );

    let decision = std::fs::read_to_string(&out).map_err(|err| format!("read out: {err}"))?;
    let value: serde_json::Value = serde_json::from_str(&decision)
        .map_err(|err| format!("gate decision should parse: {err}\n{decision}"))?;
    assert_eq!(value["status"], "blocked");
    assert!(
        value["exception_policy"]["violations"]
            .as_array()
            .is_some_and(|violations| violations
                .iter()
                .any(|violation| violation["kind"] == "quality_exception_expired")),
        "decision: {decision}"
    );
    Ok(())
}

#[test]
fn gate_evaluate_exception_policy_missing_ledger_is_config_error() -> Result<(), String> {
    let dir = unique_temp_workspace("gate-exception-missing");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let guidance = dir.join("comments.json");
    std::fs::write(&guidance, SMOKE_PR_GUIDANCE_JSON)
        .map_err(|err| format!("write guidance: {err}"))?;
    let out = dir.join("gate-decision.json");

    let output = run_ripr(&[
        "gate",
        "evaluate",
        "--pr-guidance",
        &guidance.display().to_string(),
        "--exception-policy",
        &dir.join("does-not-exist.toml").display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);
    assert_failure(&output);
    // A config error means the evaluation could not complete: exit 2, the
    // same code as any other usage or operational failure.
    assert_eq!(
        output.status.code(),
        Some(2),
        "a config error must exit with code 2"
    );

    let decision = std::fs::read_to_string(&out).map_err(|err| format!("read out: {err}"))?;
    assert!(
        decision.contains("failed to read exception policy"),
        "decision: {decision}"
    );
    Ok(())
}

#[test]
fn gate_evaluate_complete_gap_ledger_blocks_only_in_explicit_blocking_mode() -> Result<(), String> {
    let dir = unique_temp_workspace("gate-gap-ledger-cli-blocking");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let ledger = dir.join("gap-ledger.json");
    std::fs::write(&ledger, SMOKE_COMPLETE_GAP_LEDGER_JSON)
        .map_err(|err| format!("write gap ledger: {err}"))?;
    let out = dir.join("gate-decision.json");

    let output = run_ripr(&[
        "gate",
        "evaluate",
        "--root",
        &dir.display().to_string(),
        "--gap-ledger",
        &ledger.display().to_string(),
        "--mode",
        "acknowledgeable",
        "--out",
        &out.display().to_string(),
    ]);
    assert_failure(&output);
    // A blocked gate decision is a successful evaluation reaching its blocking
    // decision: the process exits 3, never the could-not-complete code 2.
    assert_eq!(
        output.status.code(),
        Some(3),
        "a blocked gate decision must exit with code 3"
    );

    let decision = std::fs::read_to_string(&out).map_err(|err| format!("read out: {err}"))?;
    let value: serde_json::Value = serde_json::from_str(&decision)
        .map_err(|err| format!("gate decision should parse: {err}\n{decision}"))?;
    assert_eq!(value["status"], "blocked");
    assert_eq!(value["summary"]["blocking"], 1);
    assert_eq!(value["decisions"][0]["source"], "gap_decision_ledger");
    assert_eq!(
        value["decisions"][0]["repair_route"]["seam_id"],
        "seam-pricing-threshold"
    );
    assert_eq!(
        value["decisions"][0]["repair_route"]["inspection_command"],
        "ripr agent brief --root . --seam-id seam-pricing-threshold --json"
    );
    assert!(decision.contains("static_ripr_evidence_only"));
    Ok(())
}

#[test]
fn gate_evaluate_complete_gap_ledger_is_advisory_in_visible_only_mode() -> Result<(), String> {
    let dir = unique_temp_workspace("gate-gap-ledger-cli-visible");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let ledger = dir.join("gap-ledger.json");
    std::fs::write(&ledger, SMOKE_COMPLETE_GAP_LEDGER_JSON)
        .map_err(|err| format!("write gap ledger: {err}"))?;
    let out = dir.join("gate-decision.json");

    let output = run_ripr(&[
        "gate",
        "evaluate",
        "--root",
        &dir.display().to_string(),
        "--gap-ledger",
        &ledger.display().to_string(),
        "--mode",
        "visible-only",
        "--out",
        &out.display().to_string(),
    ]);
    assert_success(&output);

    let decision = std::fs::read_to_string(&out).map_err(|err| format!("read out: {err}"))?;
    let value: serde_json::Value = serde_json::from_str(&decision)
        .map_err(|err| format!("gate decision should parse: {err}\n{decision}"))?;
    assert_eq!(value["status"], "advisory");
    assert_eq!(value["summary"]["blocking"], 0);
    assert_eq!(value["summary"]["advisory"], 1);
    assert_eq!(value["decisions"][0]["decision"], "advisory");
    assert_eq!(value["decisions"][0]["source"], "gap_decision_ledger");
    assert_eq!(
        value["decisions"][0]["repair_route"]["seam_id"],
        "seam-pricing-threshold"
    );
    assert_eq!(
        value["decisions"][0]["repair_route"]["inspection_command"],
        "ripr agent brief --root . --seam-id seam-pricing-threshold --json"
    );
    assert!(decision.contains("static_ripr_evidence_only"));
    Ok(())
}

#[test]
fn check_json_diff_scope_oversized_emits_limited_artifact() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr_with_env(
        &["check", "--root", &root, "--diff", &diff, "--json"],
        &[("RIPR_MAX_DIFF_CHANGED_RUST_LINES", "1")],
    );
    assert_failure(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("limited stdout should parse as JSON: {err}\n{stdout}"))?;
    assert_eq!(value["schema_version"], "0.2");
    assert_eq!(
        value["analysis_scope"]["run_status"],
        "diff_scope_oversized"
    );
    assert_eq!(value["analysis_scope"]["downstream_consumable"], false);
    assert_eq!(
        value["run_limitations"][0]["category"],
        "diff_scope_oversized"
    );
    assert_eq!(value["run_limitations"][0]["downstream_consumable"], false);
    assert_eq!(value["findings"].as_array().map(Vec::len), Some(0));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("diff_scope_oversized"),
        "stderr should still report failed analysis: {stderr}"
    );
    Ok(())
}

#[test]
fn diff_json_reports_changed_surface_before_full_repo_context() -> Result<(), String> {
    let workspace = unique_temp_workspace("diff-first");
    std::fs::create_dir_all(workspace.join("src")).map_err(|e| format!("create src dir: {e}"))?;
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname=\"ripr-diff-first-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|e| format!("write Cargo.toml: {e}"))?;
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|e| format!("write base src/lib.rs: {e}"))?;
    run_git(&workspace, &["init"])?;
    run_git(
        &workspace,
        &["config", "user.email", "ripr@example.invalid"],
    )?;
    run_git(&workspace, &["config", "user.name", "RIPR Test"])?;
    run_git(&workspace, &["add", "."])?;
    run_git(&workspace, &["commit", "-m", "base"])?;
    run_git(
        &workspace,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    )?;
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold\n}\n",
    )
    .map_err(|e| format!("write changed src/lib.rs: {e}"))?;
    run_git(&workspace, &["add", "src/lib.rs"])?;
    run_git(&workspace, &["commit", "-m", "change threshold boundary"])?;

    let root = workspace.display().to_string();
    let output = run_ripr(&[
        "diff",
        "--root",
        &root,
        "--base",
        "refs/remotes/origin/main",
        "--head",
        "HEAD",
        "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("parse diff JSON: {e}\n{stdout}"))?;
    assert_eq!(
        json_pointer_str(&report, "/kind").map_err(|e| e.to_string())?,
        "ripr_diff"
    );
    assert_eq!(
        json_pointer_str(&report, "/run_status").map_err(|e| e.to_string())?,
        "diff_complete_full_repo_limited"
    );
    assert_eq!(
        json_pointer_str(&report, "/runtime_status/diff/state").map_err(|e| e.to_string())?,
        "diff_complete"
    );
    assert_eq!(
        json_pointer_str(&report, "/runtime_status/full_repo_context/state")
            .map_err(|e| e.to_string())?,
        "full_repo_limited"
    );
    assert!(
        !json_pointer_bool(
            &report,
            "/runtime_status/full_repo_context/downstream_consumable",
        )
        .map_err(|e| e.to_string())?
    );
    assert_eq!(
        json_pointer_str(&report, "/changed_files/0/path").map_err(|e| e.to_string())?,
        "src/lib.rs"
    );
    assert_eq!(
        json_pointer_str(&report, "/receipt/outcome_hint").map_err(|e| e.to_string())?,
        "diff_complete/full_repo_limited"
    );
    assert!(
        json_pointer_str(&report, "/receipt/path")
            .map_err(|e| e.to_string())?
            .contains("diff-first")
    );
    let changed_seams = report
        .pointer("/changed_seams")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "expected changed_seams array".to_string())?;
    assert!(
        !changed_seams.is_empty(),
        "diff-first report should preserve changed-seam evidence: {stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn agent_brief_diff_scope_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
    let (root, diff) = agent_brief_sample_workspace("agent-brief-root")?;
    let root_path = root.display().to_string();
    let diff = diff.display().to_string();
    let output = run_ripr(&[
        "agent",
        "brief",
        "--root",
        &root_path,
        "--diff",
        &diff,
        "--json",
        "--max-seams",
        "2",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.1""#));
    assert!(stdout.contains(r#""scope": "working_set""#));
    assert!(stdout.contains(r#""source": "diff""#));
    assert!(stdout.contains(r#""returned": 2"#));
    assert!(stdout.contains(r#""changed_line_intersects_seam""#));
    assert!(stdout.contains(r#""agent-seam-packets-json""#));
    assert!(stdout.contains("repo-exposure-json"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn test_oracle_assistant_proof_cli_writes_canonical_report()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("assistant-loop-proof");
    std::fs::create_dir_all(&workspace)?;
    let out = workspace.join("test-oracle-assistant-proof.json");
    let out_md = workspace.join("test-oracle-assistant-proof.md");
    let out_arg = out.display().to_string();
    let out_md_arg = out_md.display().to_string();
    let output = run_ripr_in_workspace(&[
        "assistant-loop",
        "proof",
        "--pr-guidance",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        "--agent-packet",
        "fixtures/boundary_gap/expected/editor-agent-loop/agent-brief.json",
        "--before",
        "fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json",
        "--after",
        "fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json",
        "--receipt",
        "fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json",
        "--ledger",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json",
        "--out",
        &out_arg,
        "--out-md",
        &out_md_arg,
    ])?;
    assert_success(&output);

    let fixture = workspace_root()
        .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical");
    let expected_json = std::fs::read_to_string(fixture.join("test-oracle-assistant-proof.json"))?;
    let actual_json = std::fs::read_to_string(&out)?;
    assert_eq!(
        normalize_newlines(actual_json.trim_end()),
        normalize_newlines(expected_json.trim_end()),
        "assistant-loop proof JSON fixture drifted"
    );
    let expected_md = std::fs::read_to_string(fixture.join("test-oracle-assistant-proof.md"))?;
    let actual_md = std::fs::read_to_string(&out_md)?;
    assert_eq!(
        normalize_newlines(&actual_md),
        normalize_newlines(&expected_md),
        "assistant-loop proof Markdown fixture drifted"
    );
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn test_oracle_assistant_proof_cli_writes_unchanged_control()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("assistant-loop-proof-unchanged");
    std::fs::create_dir_all(&workspace)?;
    let out = workspace.join("assistant-proof.json");
    let out_md = workspace.join("assistant-proof.md");
    let output = run_ripr_in_workspace(&[
        "assistant-loop",
        "proof",
        "--root",
        ".",
        "--pr-guidance",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        "--agent-packet",
        "fixtures/boundary_gap/expected/editor-agent-loop/agent-brief.json",
        "--before",
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/before.repo-exposure.json",
        "--after",
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/after.repo-exposure.json",
        "--receipt",
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/agent-receipt.json",
        "--out",
        &out.display().to_string(),
        "--out-md",
        &out_md.display().to_string(),
    ])?;
    assert_success(&output);

    let expected = workspace_root().join(
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/assistant-proof.json",
    );
    assert_eq!(
        normalize_newlines(std::fs::read_to_string(out)?.trim_end()),
        normalize_newlines(std::fs::read_to_string(expected)?.trim_end()),
        "unchanged assistant-proof fixture drifted"
    );
    let markdown = std::fs::read_to_string(out_md)?;
    assert!(markdown.contains("After: weakly_gripped"));
    assert!(markdown.contains("State: unchanged"));
    assert!(markdown.contains("PR ledger: not available"));
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn assistant_loop_health_cli_writes_multi_proof_report() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("assistant-loop-health");
    std::fs::create_dir_all(&workspace)?;
    let out = workspace.join("assistant-loop-health.json");
    let out_md = workspace.join("assistant-loop-health.md");
    let out_arg = out.display().to_string();
    let out_md_arg = out_md.display().to_string();
    let output = run_ripr_in_workspace(&[
        "assistant-loop",
        "health",
        "--proof",
        "fixtures/boundary_gap/expected/assistant-loop-health/proofs/complete-improved-proof.json",
        "--proof",
        "fixtures/boundary_gap/expected/assistant-loop-health/proofs/unchanged-proof.json",
        "--proof",
        "fixtures/boundary_gap/expected/assistant-loop-health/proofs/missing-required-proof.json",
        "--out",
        &out_arg,
        "--out-md",
        &out_md_arg,
    ])?;
    assert_success(&output);

    let fixture =
        workspace_root().join("fixtures/boundary_gap/expected/assistant-loop-health/multi-proof");
    let expected_json = std::fs::read_to_string(fixture.join("assistant-loop-health.json"))?;
    let actual_json = std::fs::read_to_string(&out)?;
    assert_eq!(
        normalize_generated_at(normalize_newlines(actual_json.trim_end())),
        normalize_newlines(expected_json.trim_end()),
        "assistant-loop health JSON fixture drifted"
    );
    let expected_md = std::fs::read_to_string(fixture.join("assistant-loop-health.md"))?;
    let actual_md = std::fs::read_to_string(&out_md)?;
    assert_eq!(
        normalize_newlines(&actual_md),
        normalize_newlines(&expected_md),
        "assistant-loop health Markdown fixture drifted"
    );
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn first_action_cli_writes_actionable_report() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("first-action");
    std::fs::create_dir_all(&workspace)?;
    let out = workspace.join("first-useful-action.json");
    let out_md = workspace.join("first-useful-action.md");
    let out_arg = out.display().to_string();
    let out_md_arg = out_md.display().to_string();
    let output = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        "fixtures/boundary_gap/input",
        "--pr-guidance",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        "--assistant-proof",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json",
        "--ledger",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json",
        "--out",
        &out_arg,
        "--out-md",
        &out_md_arg,
    ])?;
    assert_success(&output);

    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out)?)?;
    assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
    assert_eq!(json_pointer_str(&report, "/kind")?, "first_useful_action");
    assert_eq!(json_pointer_str(&report, "/status")?, "actionable");
    assert_eq!(
        json_pointer_str(&report, "/action_kind")?,
        "write_focused_test"
    );
    assert_eq!(
        json_pointer_str(&report, "/selected/seam_id")?,
        "67fc764ba37d77bd"
    );
    // #4304: verify persists to the file the receipt reads, anchored at the
    // resolved --root (the workspace root here), and the analysis outcome the
    // receipt needs lands beside it.
    let prefix = format!("{}/", workspace_root().to_string_lossy().replace('\\', "/"));
    assert_eq!(
        json_pointer_str(&report, "/commands/verify")?.replace(&prefix, "<cwd>/"),
        "ripr agent verify --root <cwd>/fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > <cwd>/fixtures/boundary_gap/input/target/ripr/workflow/agent-verify.json"
    );
    assert_eq!(
        json_pointer_str(&report, "/commands/analysis_outcome")?.replace(&prefix, "<cwd>/"),
        "ripr check --root <cwd>/fixtures/boundary_gap/input --mode draft --format json > <cwd>/fixtures/boundary_gap/input/target/ripr/workflow/analysis-outcome.json"
    );
    assert_eq!(
        json_pointer_str(&report, "/target/suggested_test_name")?,
        "discounted_total_boundary_discriminator"
    );
    assert_eq!(
        json_pointer_str(&report, "/inputs/assistant_proof")?,
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json"
    );

    let markdown = std::fs::read_to_string(&out_md)?;
    assert!(markdown.contains("# RIPR First Useful Action"));
    assert!(markdown.contains("Status: actionable"));
    assert!(markdown.contains("Action: write_focused_test"));
    assert!(markdown.contains("Does not run mutation testing."));
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn first_action_cli_preserves_unchanged_control_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let workspace = unique_temp_workspace("first-action-unchanged");
    std::fs::create_dir_all(&workspace)?;
    let out = workspace.join("first-useful-action.json");
    let out_md = workspace.join("first-useful-action.md");
    let output = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        "fixtures/boundary_gap/input",
        "--pr-guidance",
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        "--assistant-proof",
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/assistant-proof.json",
        "--receipt",
        "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/agent-receipt.json",
        "--out",
        &out.display().to_string(),
        "--out-md",
        &out_md.display().to_string(),
    ])?;
    assert_success(&output);

    let rendered = std::fs::read_to_string(out)?;
    assert!(rendered.contains(r#""status": "missing_required_artifact""#));
    assert!(rendered.contains("receipt movement `unchanged` is not promotable"));
    assert!(std::fs::read_to_string(out_md)?.contains("missing_required_artifact"));
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn first_pr_cli_writes_start_here_packet() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("first-pr");
    let reports = workspace.join("target/ripr/reports");
    std::fs::create_dir_all(&reports)?;
    let reports_arg = reports.display().to_string();
    let output = run_ripr_in_workspace(&[
        "first-pr",
        "--root",
        ".",
        "--base",
        "HEAD",
        "--head",
        "HEAD",
        "--gap-ledger",
        "fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json",
        "--out-dir",
        &reports_arg,
    ])?;
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr first-pr - side effects and cost disclosure"));
    assert!(stdout.contains("cost class:      artifact composition only; runs no analysis"));
    assert!(stdout.contains(&format!("writes to:       {reports_arg}/")));
    assert!(stdout.contains("cache location:  none"));
    assert!(stdout.contains("git reads:       yes (base and head preflight)"));
    assert!(stdout.contains("network:         none"));
    assert!(stdout.contains("Start here:"));
    // Summary and Wrote lines render the resolved locations with stable
    // separators on every host (display_path, not Path::display).
    let wrote_base = reports_arg.replace('\\', "/");
    assert!(stdout.contains(&format!("Start here: {wrote_base}/start-here.md")));
    assert!(stdout.contains(&format!("Wrote {wrote_base}/start-here.json")));
    assert!(stdout.contains(&format!("Wrote {wrote_base}/start-here.md")));
    assert!(stdout.contains("State: top_gap"));
    assert!(stdout.contains("Safe next action: repair one named gap"));
    assert!(stdout.contains("Top actionable gap: missing boundary assertion"));
    assert!(stdout.contains("Changed behavior: `amount >= threshold`"));
    assert!(
        stdout
            .contains("Current evidence strength: Static evidence found related Rust test context")
    );
    assert!(
        stdout.contains(
            "Missing discriminator: Equality-boundary assertion for the changed behavior."
        )
    );
    assert!(
        stdout.contains(
            "Focused proof intent: Add a focused boundary assertion in `tests/pricing.rs`"
        )
    );
    assert!(stdout.contains(
        "Why this matters: A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior."
    ));
    assert!(stdout.contains("Verify after the test edit: `cargo xtask fixtures boundary_gap`"));
    assert!(stdout.contains("Receipt after verify: `ripr receipt write --gap "));
    assert!(stdout.contains("Receipt path: `target/ripr/receipts/"));
    assert!(stdout.contains("Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval."));

    let json_path = reports.join("start-here.json");
    let md_path = reports.join("start-here.md");
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json_path)?)?;
    assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
    assert_eq!(json_pointer_str(&report, "/kind")?, "first_pr_start_here");
    assert_eq!(
        json_pointer_str(&report, "/ripr_version")?,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(json_pointer_str(&report, "/status")?, "actionable");
    assert_eq!(json_pointer_str(&report, "/selected/state")?, "top_gap");
    assert_eq!(
        json_pointer_str(&report, "/selected/kind")?,
        "MissingBoundaryAssertion"
    );
    assert_eq!(
        json_pointer_str(&report, "/selected/repair/route")?,
        "AddBoundaryAssertion"
    );
    assert_eq!(
        json_pointer_str(&report, "/selected/static_evidence_boundary")?,
        "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval."
    );
    assert_eq!(
        json_pointer_str(&report, "/selected/why")?,
        "A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior."
    );
    assert_eq!(json_pointer_str(&report, "/inputs/base")?, "HEAD");
    assert_eq!(json_pointer_str(&report, "/inputs/head")?, "HEAD");
    assert_eq!(json_pointer_str(&report, "/preflight/mode")?, "write");
    assert!(
        report
            .pointer("/preflight/checks")
            .is_some_and(|value| value.is_array())
    );
    assert_eq!(
        json_pointer_str(&report, "/commands/verify")?,
        "cargo xtask fixtures boundary_gap"
    );

    let markdown = std::fs::read_to_string(&md_path)?;
    assert!(markdown.contains("# RIPR First PR Start Here"));
    assert!(markdown.contains("Status: advisory"));
    assert!(markdown.contains("## Preflight"));
    assert!(markdown.contains("- Top actionable gap: missing boundary assertion"));
    assert!(
        markdown.contains(
            "- Current evidence strength: Static evidence found related Rust test context"
        )
    );
    assert!(markdown.contains("- Missing discriminator: Equality-boundary assertion"));
    assert!(markdown.contains("- Receipt after verify: `ripr receipt write --gap "));
    assert!(markdown.contains("- Receipt path: `target/ripr/receipts/"));
    assert!(markdown.contains("Pass/fail authority remains with explicit gate-decision artifacts"));
    let check_output = run_ripr_in_workspace(&[
        "start-here",
        "--root",
        ".",
        "--base",
        "HEAD",
        "--head",
        "HEAD",
        "--gap-ledger",
        "fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json",
        "--out-dir",
        &reports_arg,
        "--check",
    ])?;
    assert_success(&check_output);
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(check_stdout.contains("Start here:"));
    assert!(check_stdout.contains("State: top_gap"));
    assert!(check_stdout.contains("First PR start-here packet ok:"));
    // --check validates without rewriting, so the disclosure must not claim writes.
    assert!(
        check_stdout
            .contains("writes to:       none (--check validates an existing start-here packet)")
    );
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn first_pr_check_missing_packet_suggests_rooted_out_dir() -> Result<(), Box<dyn std::error::Error>>
{
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["first-pr", "--root", &root, "--check"]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("it does not create one"), "{stderr}");
    // The suggested recovery must reproduce the exact directory `--check`
    // validated: rooted at `--root`, never CWD-relative, with stable
    // separators on every host.
    let expected_out_dir = workspace
        .join("target/ripr/reports")
        .display()
        .to_string()
        .replace('\\', "/");
    let plain = format!("--out-dir {expected_out_dir}");
    let quoted = format!("--out-dir '{expected_out_dir}'");
    assert!(stderr.contains(&plain) || stderr.contains(&quoted));
    assert!(!stderr.contains("--out-dir target/ripr/reports"));
    assert!(!stderr.contains("--out-dir 'target/ripr/reports'"));
    // The missing path renders with stable separators even on Windows.
    assert!(stderr.contains(&format!("Missing:\n  {expected_out_dir}/start-here.json")));
    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

/// The write command a `first-pr --check` missing-packet recovery suggests,
/// as arguments after `ripr` (each shell word decoded; the fixture paths have
/// no spaces).
fn suggested_first_pr_write(stderr: &str) -> Result<Vec<String>, String> {
    let line = stderr
        .split("Create and validate it with:\n")
        .nth(1)
        .and_then(|rest| rest.lines().next())
        .ok_or_else(|| format!("no suggested write command:\n{stderr}"))?;
    let mut args = line
        .split_whitespace()
        .map(|arg| {
            decode_shell_token(arg).ok_or_else(|| format!("malformed shell word `{arg}`: {line}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if args.first().map(String::as_str) != Some("ripr") {
        return Err(format!("suggested command is not a ripr command: {line}"));
    }
    args.remove(0);
    Ok(args)
}

/// #3999: the suggested write binds `--root` to the repository the check
/// resolved, so it names an absolute root ending in the scratch repository's
/// directory rather than a caller-relative spelling such as `.`.
fn assert_bound_write_root(write: &[String], repo: &Path, stderr: &str) -> Result<(), String> {
    let repo_name = repo
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("scratch repository has no name: {}", repo.display()))?;
    let bound = write
        .iter()
        .position(|arg| arg == "--root")
        .and_then(|index| write.get(index + 1))
        .ok_or_else(|| format!("the suggested write must name --root:\n{stderr}"))?;
    if Path::new(bound).is_absolute() && bound.trim_end_matches(['/', '\\']).ends_with(&repo_name) {
        Ok(())
    } else {
        Err(format!(
            "the suggested write must bind --root to the absolute repository `{repo_name}`, got `{bound}`:\n{stderr}"
        ))
    }
}

fn run_ripr_owned(args: &[String]) -> Output {
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_ripr(&args)
}

/// #4285: `--check` never diffs, so a repository where no default base
/// resolves (no origin, no main/master: a CI merge-ref checkout) must still
/// answer a missing packet with the recovery, not the base failure. Its
/// suggested write cannot resolve a base either, so it must demand `--base`
/// rather than omit or guess it; filled in, that command writes the packet.
/// (Windows Advisory surfaced it: the suite's temp root is under the checkout,
/// and the lane's shallow PR checkout has no default base.)
#[test]
fn first_pr_check_missing_packet_recovers_without_a_resolvable_base()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = make_temp_workspace(None)?;
    run_git(&workspace, &["init", "-q", "-b", "topic"])?;
    run_git(&workspace, &["add", "Cargo.toml", "src/lib.rs"])?;
    run_git(
        &workspace,
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    )?;
    // The CI merge-ref shape: a detached HEAD and no base-named branch.
    run_git(&workspace, &["checkout", "-q", "--detach"])?;
    run_git(&workspace, &["branch", "-q", "-D", "topic"])?;
    let root = workspace.display().to_string();
    // Run from inside the repository with a relative `--root .` so the
    // suggested write must bind it (#3999) to work from the harness's cwd.
    let output = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&workspace),
        &["first-pr", "--root", ".", "--check"],
    )?;
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("it does not create one"), "{stderr}");
    assert!(
        stderr.contains("Replace <ref> with the branch or commit this PR is based on"),
        "{stderr}"
    );
    assert!(
        stderr.contains("could not resolve a default base"),
        "the recovery must name why --base is required:\n{stderr}"
    );
    assert!(!stderr.contains("origin/main --head"), "{stderr}");

    // The suggested write names the base it needs; with a real ref filled in
    // it writes a packet that `--check` with the same base accepts.
    let mut write = suggested_first_pr_write(&stderr)?;
    assert_bound_write_root(&write, &workspace, &stderr)?;
    let base = write
        .iter()
        .position(|arg| arg == "--base")
        .ok_or_else(|| format!("the suggested write must demand --base:\n{stderr}"))?;
    assert_eq!(
        write.get(base + 1).map(String::as_str),
        Some("<ref>"),
        "{stderr}"
    );
    write[base + 1] = "HEAD".to_string();
    let written = run_ripr_owned(&write);
    assert_success(&written);
    assert!(
        workspace
            .join("target/ripr/reports/start-here.json")
            .is_file()
    );
    let recheck = run_ripr(&["first-pr", "--root", &root, "--base", "HEAD", "--check"]);
    assert_success(&recheck);
    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #4285 positive half: where the default base resolves, the recovery's
/// suggested write omits `--base` and, run exactly as printed, resolves the
/// same default (#4260) and writes the packet; `--check` then resolves the
/// omitted base too, so the packet it validates is current.
#[test]
fn first_pr_check_recovery_write_resolves_the_default_base() -> Result<(), String> {
    let root = no_origin_master_repo("first-pr-check-default-base", "master")?;
    let root_str = root.display().to_string();
    let missing = run_ripr(&["first-pr", "--root", &root_str, "--check"]);
    assert_failure(&missing);
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(stderr.contains("it does not create one"), "{stderr}");
    assert!(!stderr.contains("--base"), "{stderr}");

    let suggested = suggested_first_pr_write(&stderr)?;
    assert_bound_write_root(&suggested, &root, &stderr)?;
    let write = run_ripr_owned(&suggested);
    assert_success(&write);
    let base = json_string_at(
        &root.join("target/ripr/reports/start-here.json"),
        "/inputs/base",
    )?;
    assert_eq!(base, "master");

    let check = run_ripr(&["first-pr", "--root", &root_str, "--check"]);
    assert_success(&check);
    assert!(
        String::from_utf8_lossy(&check.stdout).contains("First PR start-here packet ok"),
        "{check:?}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn report_packet_index_cli_writes_packet_index() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("report-packet-index");
    let reports = workspace.join("target/ripr/reports");
    let review = workspace.join("target/ripr/review");
    std::fs::create_dir_all(&reports)?;
    std::fs::create_dir_all(&review)?;
    std::fs::write(
        reports.join("pr-review-front-panel.md"),
        "Status: blocked\n",
    )?;
    std::fs::write(
        reports.join("pr-review-front-panel.json"),
        r#"{"status":"blocked"}"#,
    )?;
    std::fs::write(reports.join("gate-decision.md"), "Status: blocked\n")?;
    std::fs::write(
        reports.join("gate-decision.json"),
        r#"{"decision":"blocked"}"#,
    )?;
    std::fs::write(reports.join("first-useful-action.md"), "Status: pass\n")?;
    std::fs::write(review.join("comments.md"), "comments\n")?;

    let out = workspace.join("target/ripr/reports/index.json");
    let out_md = workspace.join("target/ripr/reports/index.md");
    let reports_arg = reports.display().to_string();
    let review_arg = review.display().to_string();
    let out_arg = out.display().to_string();
    let out_md_arg = out_md.display().to_string();

    let output = run_ripr(&[
        "reports",
        "index",
        "--reports-dir",
        &reports_arg,
        "--review-dir",
        &review_arg,
        "--out",
        &out_arg,
        "--out-md",
        &out_md_arg,
    ]);
    assert_success(&output);

    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out)?)?;
    assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
    assert_eq!(json_pointer_str(&report, "/kind")?, "report_packet_index");
    assert_eq!(json_pointer_str(&report, "/status")?, "fail");
    assert_eq!(
        json_pointer_str(&report, "/summary/gate_authority")?,
        "target/ripr/reports/gate-decision.md"
    );
    assert!(std::fs::read_to_string(&out_md)?.contains("Gate authority:"));

    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

#[test]
fn agent_brief_diff_scope_omits_configured_off_seams() -> Result<(), Box<dyn std::error::Error>> {
    let (root, diff) = agent_brief_sample_workspace("agent-brief-config-off")?;
    std::fs::write(
        root.join("ripr.toml"),
        "[severity.seams]\nweakly_gripped = \"off\"\n",
    )?;
    let root_path = root.display().to_string();
    let diff = diff.display().to_string();
    let output = run_ripr(&[
        "agent", "brief", "--root", &root_path, "--diff", &diff, "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""returned": 0"#));
    assert!(stdout.contains("configured off for weakly_gripped seams"));
    assert!(!stdout.contains(r#""severity": "off""#));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_packet_expands_one_brief_seam_by_id() -> Result<(), Box<dyn std::error::Error>> {
    let (root, diff) = agent_brief_sample_workspace("agent-packet-root")?;
    let root_path = root.display().to_string();
    let diff = diff.display().to_string();
    let brief = run_ripr(&[
        "agent", "brief", "--root", &root_path, "--diff", &diff, "--json",
    ]);
    assert_success(&brief);
    let brief_stdout = String::from_utf8_lossy(&brief.stdout);
    let seam_id = json_string_field(&brief_stdout, "seam_id")
        .ok_or("expected brief output to include a seam_id")?;

    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_path,
        "--seam-id",
        &seam_id,
        "--json",
    ]);
    assert_success(&packet);

    let packet_stdout = String::from_utf8_lossy(&packet.stdout);
    assert!(packet_stdout.contains(r#""schema_version": "0.5""#));
    assert!(packet_stdout.contains(r#""analysis_outcome_status": "not_applicable""#));
    assert!(packet_stdout.contains(r#""packets_total": 1"#));
    assert!(packet_stdout.contains(&format!(r#""seam_id": "{seam_id}""#)));
    assert!(packet_stdout.contains(r#""task": "write_targeted_test""#));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// A cold agent's most common first repair call passes the `probe:...` finding
/// ID printed by `ripr check`. The refusal must say where seam IDs come from,
/// through both the packet surface and the repair transaction that wraps it.
#[test]
fn agent_packet_unknown_seam_id_names_the_seam_id_source() -> Result<(), Box<dyn std::error::Error>>
{
    let (root, _diff) = agent_brief_sample_workspace("agent-packet-unknown-seam")?;
    let root_path = root.display().to_string();

    let probe_id = "probe:src_lib.rs:predicate:566edf6b";
    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_path,
        "--seam-id",
        probe_id,
        "--json",
    ]);
    assert!(!packet.status.success());
    let stderr = String::from_utf8_lossy(&packet.stderr);
    assert!(
        stderr.contains("is a `ripr check` finding ID, not a seam ID"),
        "{stderr}"
    );
    // The pilot route names the root the failing call used, not `.`.
    assert!(
        stderr.contains("run `ripr pilot --root ")
            && stderr.contains("agent-packet-unknown-seam")
            && stderr.contains("to list current seam IDs"),
        "{stderr}"
    );

    let repair = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_path,
        "--seam-id",
        probe_id,
        "--phase",
        "before",
    ]);
    assert!(!repair.status.success());
    let stderr = String::from_utf8_lossy(&repair.stderr);
    assert!(stderr.contains("not a seam ID"), "{stderr}");
    assert!(
        stderr.contains("ripr pilot --root ") && stderr.contains("agent-packet-unknown-seam"),
        "{stderr}"
    );

    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_path,
        "--seam-id",
        "0000000000000000",
        "--json",
    ]);
    assert!(!packet.status.success());
    let stderr = String::from_utf8_lossy(&packet.stderr);
    assert!(!stderr.contains("not a seam ID"), "{stderr}");
    assert!(
        stderr.contains("Run `ripr pilot --root ") && stderr.contains("to list current seam IDs"),
        "{stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// `ripr agent card` is the default bounded handoff (#4667): one seam projects
/// to the versioned `RepairCardV1`, the complete canonical packet stays
/// behind the card's explicit packet route, and the same typed fields render
/// as the compact human summary without `--json`.
#[test]
fn agent_card_hands_off_one_seam_as_the_default_repair_card()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-card-handoff");
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"boundary_gap_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"boundary_gap_fixture\"\npath = \"src/lib.rs\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\n",
    )?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        "use boundary_gap_fixture::discounted_total;\n\n#[test]\nfn below_threshold_has_no_discount() {\n    assert_eq!(discounted_total(50, 100), 50);\n}\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n",
    )?;
    init_git_fixture_repo(&root)?;
    run_git(&root, &["add", "Cargo.toml", "src", "tests"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture source",
        ],
    )?;
    assert!(
        commit.status.success(),
        "fixture source commit failed: {commit:?}"
    );
    // Dirty the worktree so the current analysis names the seam's gap and its
    // witness; the boundary predicate itself is unchanged.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 20\n    } else {\n        amount\n    }\n}\n",
    )?;

    let root_arg = root.display().to_string();
    let card = run_ripr(&[
        "agent",
        "card",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert_success(&card);
    let card_stdout = String::from_utf8_lossy(&card.stdout);
    let card_json: serde_json::Value = serde_json::from_str(&card_stdout)?;
    assert_eq!(card_json["schema_version"], "repair_card.v1");
    assert_eq!(card_json["subject"]["seam_id"], "67fc764ba37d77bd");
    let head = run_command("git", Some(&root), &["rev-parse", "HEAD"])?;
    assert!(head.status.success());
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    assert_eq!(card_json["snapshot"]["repository_head"], head);
    // #5008: the card binds the committed HEAD while its analysis read the
    // dirty working tree, so both currentness axes must project the accepted
    // dirty draft instead of an unchecked `current` claim.
    assert_eq!(card_json["snapshot"]["currentness"], "accepted_dirty_draft");
    assert_eq!(
        card_json["done_when"]["currentness"],
        "accepted_dirty_draft"
    );
    // The complete packet is routed, never embedded: the wire card names the
    // packet route and carries no packet envelope content.
    assert!(
        !card_stdout.contains("agent-seam-packets-json"),
        "the wire card must not embed the canonical packet:\n{card_stdout}"
    );
    let references = card_json["detail_references"]
        .as_array()
        .ok_or("detail_references must be an array")?;
    let packet_reference = references
        .iter()
        .find(|reference| reference["family"] == "canonical_packet")
        .ok_or("canonical packet detail reference missing")?;
    assert_eq!(packet_reference["state"], "current");
    let route = packet_reference["route"]
        .as_str()
        .ok_or("canonical packet reference must name a route")?;
    assert!(
        route.contains("ripr agent packet --seam-id 67fc764ba37d77bd"),
        "{route}"
    );

    // The default surface is the compact human summary of the same card.
    let human = run_ripr(&[
        "agent",
        "card",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
    ]);
    assert_success(&human);
    let human_stdout = String::from_utf8_lossy(&human.stdout);
    for needle in [
        "Repair card ",
        "  seam: 67fc764ba37d77bd",
        "  next action:",
        "  full packet: ripr agent packet ",
        "--seam-id 67fc764ba37d77bd --json",
        // #5008: the human summary names the dirty-draft state verbatim, not
        // the `{:?}` debug spelling.
        "accepted_dirty_draft",
    ] {
        assert!(
            human_stdout.contains(needle),
            "missing {needle:?}:\n{human_stdout}"
        );
    }

    // Committing the evidence change makes the scope clean: both axes
    // project `current` again.
    run_git(&root, &["add", "src/lib.rs"])?;
    let committed = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture source with discount",
        ],
    )?;
    assert!(
        committed.status.success(),
        "fixture follow-up commit failed: {committed:?}"
    );
    let clean_card = run_ripr(&[
        "agent",
        "card",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert_success(&clean_card);
    let clean_stdout = String::from_utf8_lossy(&clean_card.stdout);
    let clean_json: serde_json::Value = serde_json::from_str(&clean_stdout)?;
    assert_eq!(clean_json["snapshot"]["currentness"], "current");
    assert_eq!(clean_json["done_when"]["currentness"], "current");

    // A cold agent's `probe:...` finding ID is refused with the same seam-ID
    // source hint the packet surface names. #5007: the refusal is typed —
    // exit code 3, not an operational failure — and under `--json` stderr
    // carries one parseable `agent_card_refusal` envelope with the prose
    // rendering after it, while stdout stays empty.
    let unknown = run_ripr(&[
        "agent",
        "card",
        "--root",
        &root_arg,
        "--seam-id",
        "probe:src_lib.rs:predicate:566edf6b",
    ]);
    assert_eq!(
        unknown.status.code(),
        Some(3),
        "a named refusal takes the decision exit code: {unknown:?}"
    );
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(
        stderr.contains("is a `ripr check` finding ID, not a seam ID"),
        "{stderr}"
    );
    let typed = run_ripr(&[
        "agent",
        "card",
        "--root",
        &root_arg,
        "--seam-id",
        "probe:src_lib.rs:predicate:566edf6b",
        "--json",
    ]);
    assert_eq!(
        typed.status.code(),
        Some(3),
        "a named refusal takes the decision exit code: {typed:?}"
    );
    assert!(
        typed.stdout.is_empty(),
        "the card-artifact stdout stays empty on a refusal: {typed:?}"
    );
    let typed_stderr = String::from_utf8_lossy(&typed.stderr);
    let envelope_end = typed_stderr
        .find("\n}\n")
        .ok_or("the refusal envelope did not terminate on stderr")?
        + 3;
    let envelope: serde_json::Value = serde_json::from_str(&typed_stderr[..envelope_end])?;
    assert_eq!(envelope["schema_version"], "0.1", "{envelope}");
    assert_eq!(envelope["kind"], "agent_card_refusal", "{envelope}");
    assert_eq!(envelope["error"]["kind"], "seam_not_found", "{envelope}");
    assert_eq!(
        envelope["error"]["seam_id"], "probe:src_lib.rs:predicate:566edf6b",
        "{envelope}"
    );
    assert!(
        envelope["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("was not found")),
        "{envelope}"
    );
    assert!(
        envelope["error"]["remedy_route"]
            .as_str()
            .is_some_and(|route| route.starts_with("ripr pilot --root ")),
        "{envelope}"
    );
    assert!(
        typed_stderr[envelope_end..].contains("is a `ripr check` finding ID, not a seam ID"),
        "the human prose rendering stays on stderr after the envelope: {typed_stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// #5005: the same seam at the same head mints the same `repair_card_id`
/// and `complete_evidence_digest` from two equivalent checkout roots. The
/// card content-hashes the canonical packet envelope; before the portable
/// render, the packet's `next`-block command strings embedded each
/// checkout's absolute root spelling and root-dependent digests leaked into
/// the card identity.
#[test]
fn agent_card_identity_is_portable_across_equivalent_checkout_roots()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-card-portable-identity-a");
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"boundary_gap_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"boundary_gap_fixture\"\npath = \"src/lib.rs\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 20\n    } else {\n        amount\n    }\n}\n",
    )?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        "use boundary_gap_fixture::discounted_total;\n\n#[test]\nfn below_threshold_has_no_discount() {\n    assert_eq!(discounted_total(50, 100), 50);\n}\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n",
    )?;
    init_git_fixture_repo(&root)?;
    run_git(&root, &["add", "Cargo.toml", "src", "tests"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture source",
        ],
    )?;
    assert!(
        commit.status.success(),
        "fixture source commit failed: {commit:?}"
    );
    // The equivalent checkout: a clone of the same head at a different
    // absolute path, so any absolute-root spelling in the content-bound
    // packet would differ between the two card runs.
    let second = unique_temp_workspace("agent-card-portable-identity-b");
    let clone = run_command(
        "git",
        Some(&root),
        &["clone", &root.to_string_lossy(), &second.to_string_lossy()],
    )?;
    assert!(
        clone.status.success(),
        "equivalent checkout clone failed: {clone:?}"
    );

    let card_for = |root_arg: &str| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let card = run_ripr(&[
            "agent",
            "card",
            "--root",
            root_arg,
            "--seam-id",
            "67fc764ba37d77bd",
            "--json",
        ]);
        assert_success(&card);
        let stdout = String::from_utf8_lossy(&card.stdout);
        Ok(serde_json::from_str(&stdout)?)
    };
    let first_card = card_for(&root.display().to_string())?;
    let second_card = card_for(&second.display().to_string())?;
    assert_eq!(
        first_card["snapshot"]["repository_head"], second_card["snapshot"]["repository_head"],
        "both checkouts must bind the same head for the identity comparison"
    );
    assert_eq!(
        first_card["repair_card_id"], second_card["repair_card_id"],
        "equivalent checkout roots minted different repair card identities"
    );
    assert_eq!(
        first_card["complete_evidence_digest"], second_card["complete_evidence_digest"],
        "equivalent checkout roots minted different complete evidence identities"
    );
    std::fs::remove_dir_all(&root)?;
    std::fs::remove_dir_all(&second)?;
    Ok(())
}

/// A statically admitted inline test is not an executable repair surface.
/// The default card must disclose the same refusal as the Before phase,
/// while a separate test file remains ready and can publish an attempt.
#[test]
fn agent_card_readiness_agrees_with_repair_target_admission()
-> Result<(), Box<dyn std::error::Error>> {
    for inline in [true, false] {
        let root = unbuilt_repair_fixture(if inline {
            "agent-card-inline-admission"
        } else {
            "agent-card-separate-admission"
        })?;
        if inline {
            let tests = std::fs::read_to_string(root.join("tests/pricing.rs"))?.replace(
                "use boundary_gap_fixture::discounted_total;",
                "use super::discounted_total;",
            );
            let mut source = std::fs::read_to_string(root.join("src/lib.rs"))?;
            source.push_str(&format!("\n#[cfg(test)]\nmod tests {{\n{tests}\n}}\n"));
            std::fs::write(root.join("src/lib.rs"), source)?;
            std::fs::remove_file(root.join("tests/pricing.rs"))?;
            run_git(&root, &["add", "src/lib.rs", "tests/pricing.rs"])?;
            commit_repair_fixture(&root, &["-qm", "inline tests"])?;
        }
        let root_arg = root.display().to_string();
        let card = run_ripr(&[
            "agent",
            "card",
            "--root",
            &root_arg,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
        ]);
        assert_success(&card);
        let card: serde_json::Value = serde_json::from_slice(&card.stdout)?;
        assert_eq!(card["subject"]["seam_id"], BOUNDARY_GAP_SEAM_ID);
        assert_eq!(card["selected_target"]["kind"], "existing");
        assert_eq!(
            card["selected_target"]["file"],
            if inline {
                "src/lib.rs"
            } else {
                "tests/pricing.rs"
            }
        );
        assert_eq!(card["readiness"]["repair_ready"], !inline, "{card:#}");
        let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
        if inline {
            assert_failure(&before);
            let blocker = card["exact_blocker"]
                .as_str()
                .ok_or("missing cage blocker")?;
            assert!(blocker.contains("no selected edit target"), "{blocker}");
            assert!(String::from_utf8_lossy(&before.stderr).contains(blocker));
            let human = run_ripr(&[
                "agent",
                "card",
                "--root",
                &root_arg,
                "--seam-id",
                BOUNDARY_GAP_SEAM_ID,
            ]);
            assert_success(&human);
            let human = String::from_utf8_lossy(&human.stdout);
            assert!(
                human.contains("repair_ready=false") && human.contains(blocker),
                "{human}"
            );
            assert!(
                card["readiness"]["missing_evidence"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(blocker)))
            );
            assert_eq!(card["allowed_files"], serde_json::json!([]));
            assert_eq!(card["forbidden_files"], serde_json::json!(["src/lib.rs"]));
            assert!(card["next_action"].is_null());
            assert!(before.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&before.stderr).contains("before phase complete"));
        } else {
            assert_success(&before);
            assert_eq!(
                card["allowed_files"],
                serde_json::json!(["tests/pricing.rs"])
            );
            assert_eq!(card["readiness"]["missing_evidence"], serde_json::json!([]));
            let (attempt_id, manifest) = sole_repair_attempt(&root)?;
            let before: serde_json::Value = serde_json::from_slice(&before.stdout)?;
            assert_eq!(before["repair_attempt"]["attempt_id"], attempt_id);
            assert_eq!(manifest["state"], "awaiting_edit");
        }
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

/// The `unchanged_after_attempt` route is reachable only from a promotable
/// receipt: a live verify pair, its analysis outcome, and a receipt bound to
/// both (#4268). The committed unchanged-after-attempt receipt is
/// portable-normalized and fails closed, so this chain is the route's proof.
#[test]
fn first_action_routes_live_unchanged_receipt_to_revise_focused_test()
-> Result<(), Box<dyn std::error::Error>> {
    let seam_id = "67fc764ba37d77bd";
    // Own directory: the improved-receipt chain above writes its own
    // verify pair and analysis outcome under `test-agent-verify`.
    let artifact_rel = "target/ripr/test-agent-verify-unchanged";
    let artifact_dir = workspace_root().join(artifact_rel);
    std::fs::create_dir_all(&artifact_dir)?;
    // The same pre-attempt snapshot on both sides: the attempt moved nothing.
    let snapshot = workspace_root()
        .join("fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json");
    for side in ["before", "after"] {
        bind_repo_exposure_fixture_with_worktree(
            &workspace_root(),
            &snapshot,
            &artifact_dir.join(format!("{side}.repo-exposure.json")),
            "dirty",
        )?;
    }
    let before_path = format!("{artifact_rel}/before.repo-exposure.json");
    let after_path = format!("{artifact_rel}/after.repo-exposure.json");
    let verify = run_ripr_in_workspace(&[
        "agent",
        "verify",
        "--root",
        ".",
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ])?;
    assert_success(&verify);
    std::fs::write(artifact_dir.join("agent-verify.json"), &verify.stdout)?;
    let analysis_outcome = run_ripr_in_workspace(&[
        "check", "--root", ".", "--mode", "draft", "--base", "HEAD", "--format", "json",
    ])?;
    assert_success(&analysis_outcome);
    std::fs::write(
        artifact_dir.join("analysis-outcome.json"),
        &analysis_outcome.stdout,
    )?;

    let out_dir = unique_temp_workspace("first-action-unchanged-receipt");
    std::fs::create_dir_all(&out_dir)?;
    let receipt_path = out_dir.join("agent-receipt.json");
    let receipt_arg = receipt_path
        .to_str()
        .ok_or("receipt path should be utf-8")?;
    let verify_path = format!("{artifact_rel}/agent-verify.json");
    let receipt = run_ripr_in_workspace(&[
        "agent",
        "receipt",
        "--root",
        ".",
        "--verify-json",
        &verify_path,
        "--seam-id",
        seam_id,
        "--json",
        "--out",
        receipt_arg,
    ])?;
    assert_success(&receipt);
    let receipt_value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&receipt_path)?)?;
    // Assert the stimulus before the route: a complete receipt that records
    // unchanged movement for the seam.
    assert_eq!(receipt_value["analysis_outcome_status"], "complete");
    assert_eq!(receipt_value["provenance"]["movement"], "unchanged");

    let first_action_out = out_dir.join("first-action.json");
    let first_action_md = out_dir.join("first-action.md");
    let first_action = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        ".",
        "--receipt",
        receipt_arg,
        "--out",
        first_action_out
            .to_str()
            .ok_or("first-action output path should be utf-8")?,
        "--out-md",
        first_action_md
            .to_str()
            .ok_or("first-action markdown path should be utf-8")?,
    ])?;
    assert_success(&first_action);
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&first_action_out)?)?;
    assert_eq!(report["status"], "unchanged_after_attempt", "{report:#}");
    assert_eq!(report["action_kind"], "revise_focused_test");
    assert_eq!(report["evidence"]["static_movement"], "unchanged");
    assert_eq!(report["fallback"]["kind"], "unchanged_after_attempt");
    std::fs::remove_dir_all(out_dir)?;
    Ok(())
}

#[test]
fn editor_agent_loop_fixture_outputs_match_expected() -> Result<(), Box<dyn std::error::Error>> {
    let base = "fixtures/boundary_gap/expected/editor-agent-loop";
    let seam_id = "67fc764ba37d77bd";

    let packet = run_ripr_in_workspace(&[
        "agent",
        "packet",
        "--root",
        "fixtures/boundary_gap/input",
        "--seam-id",
        seam_id,
        "--json",
    ])?;
    assert_anchored_stdout_matches_fixture(&packet, &format!("{base}/agent-packet.json"))?;

    let brief = run_ripr_in_workspace(&[
        "agent",
        "brief",
        "--root",
        "fixtures/boundary_gap/input",
        "--seam-id",
        seam_id,
        "--json",
    ])?;
    assert_anchored_stdout_matches_fixture(&brief, &format!("{base}/agent-brief.json"))?;

    let artifact_dir = workspace_root().join("target/ripr/test-agent-verify");
    std::fs::create_dir_all(&artifact_dir)?;
    let before_artifact = artifact_dir.join("before.repo-exposure.json");
    let after_artifact = artifact_dir.join("after.repo-exposure.json");
    bind_repo_exposure_fixture_with_worktree(
        &workspace_root(),
        &workspace_root()
            .join("fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json"),
        &before_artifact,
        "dirty",
    )?;
    bind_repo_exposure_fixture_with_worktree(
        &workspace_root(),
        &workspace_root()
            .join("fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json"),
        &after_artifact,
        "dirty",
    )?;
    let before_artifact_path = "target/ripr/test-agent-verify/before.repo-exposure.json";
    let after_artifact_path = "target/ripr/test-agent-verify/after.repo-exposure.json";
    let verify = run_ripr_in_workspace(&[
        "agent",
        "verify",
        "--root",
        ".",
        "--before",
        before_artifact_path,
        "--after",
        after_artifact_path,
        "--json",
    ])?;
    assert_success(&verify);
    // The verify JSON binds the exact artifact content commitments (#2922
    // PR B), which embed the live repository head, so the static golden can
    // only pin the digest-normalized shape.
    let expected_verify =
        std::fs::read_to_string(workspace_root().join(format!("{base}/agent-verify.json")))?;
    let actual_verify = String::from_utf8(verify.stdout.clone())?;
    assert_eq!(
        normalize_agent_verify_fixture(&actual_verify)?,
        normalize_agent_verify_fixture(&expected_verify)?,
        "agent verify fixture drifted"
    );
    // The receipt path re-binds the exact artifact bytes, so it must consume
    // the live canonical verify output — never the digest-normalized golden.
    let verify_artifact_path = "target/ripr/test-agent-verify/agent-verify.json";
    std::fs::write(artifact_dir.join("agent-verify.json"), &verify.stdout)?;
    let analysis_outcome = run_ripr_in_workspace(&[
        "check", "--root", ".", "--mode", "draft", "--base", "HEAD", "--format", "json",
    ])?;
    assert_success(&analysis_outcome);
    std::fs::write(
        artifact_dir.join("analysis-outcome.json"),
        &analysis_outcome.stdout,
    )?;

    let out_dir = unique_temp_workspace("agent-receipt-fixture");
    std::fs::create_dir_all(&out_dir)?;
    let receipt_path = out_dir.join("agent-receipt.json");
    let receipt = run_ripr_in_workspace(&[
        "agent",
        "receipt",
        "--root",
        ".",
        "--verify-json",
        verify_artifact_path,
        "--seam-id",
        seam_id,
        "--json",
        "--out",
        receipt_path
            .to_str()
            .ok_or("receipt path should be utf-8")?,
    ])?;
    assert_success(&receipt);
    let expected_receipt =
        std::fs::read_to_string(workspace_root().join(base).join("agent-receipt.json"))?;
    let actual_receipt = std::fs::read_to_string(&receipt_path)?;
    let actual_receipt_value: serde_json::Value = serde_json::from_str(&actual_receipt)?;
    assert_eq!(actual_receipt_value["status"], "advisory");
    assert_eq!(actual_receipt_value["analysis_outcome_status"], "complete");
    assert_eq!(
        actual_receipt_value["analysis_outcome"]["analysis_complete"],
        true
    );
    assert_eq!(
        normalize_agent_receipt_fixture(&actual_receipt)?,
        normalize_agent_receipt_fixture(&expected_receipt)?,
        "agent receipt fixture drifted"
    );
    let first_action_out = out_dir.join("first-action.json");
    let first_action_md = out_dir.join("first-action.md");
    let first_action = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        ".",
        "--receipt",
        receipt_path
            .to_str()
            .ok_or("receipt path should be utf-8")?,
        "--out",
        first_action_out
            .to_str()
            .ok_or("first-action output path should be utf-8")?,
        "--out-md",
        first_action_md
            .to_str()
            .ok_or("first-action markdown path should be utf-8")?,
    ])?;
    assert_success(&first_action);
    let first_action_json = std::fs::read_to_string(first_action_out)?;
    assert!(
        first_action_json.contains(r#""status": "already_improved""#),
        "valid receipt was not promoted: {first_action_json}"
    );
    assert!(first_action_json.contains(seam_id));
    // A receipt with a valid verify pair but no producer analysis outcome is
    // not promotable, even though the verify artifact itself is canonical.
    let analysis_outcome_path = artifact_dir.join("analysis-outcome.json");
    let analysis_outcome_bytes = std::fs::read(&analysis_outcome_path)?;
    std::fs::remove_file(&analysis_outcome_path)?;
    let missing_analysis_out = out_dir.join("missing-analysis.json");
    let missing_analysis_md = out_dir.join("missing-analysis.md");
    let missing_analysis = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        ".",
        "--receipt",
        receipt_path
            .to_str()
            .ok_or("receipt path should be utf-8")?,
        "--out",
        missing_analysis_out
            .to_str()
            .ok_or("output path should be utf-8")?,
        "--out-md",
        missing_analysis_md
            .to_str()
            .ok_or("markdown path should be utf-8")?,
    ])?;
    assert_success(&missing_analysis);
    let missing_analysis_json = std::fs::read_to_string(missing_analysis_out)?;
    assert!(missing_analysis_json.contains("missing_required_artifact"));
    assert!(!missing_analysis_json.contains(r#""status": "already_improved""#));
    std::fs::write(&analysis_outcome_path, &analysis_outcome_bytes)?;

    let different_cwd = unique_temp_workspace("first-action-cwd");
    std::fs::create_dir_all(&different_cwd)?;
    let mut absolute_analysis: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&analysis_outcome_path)?)?;
    absolute_analysis["root"] =
        serde_json::Value::String(workspace_root().display().to_string().replace('\\', "/"));
    std::fs::write(
        &analysis_outcome_path,
        serde_json::to_vec_pretty(&absolute_analysis)?,
    )?;
    let cwd_out = different_cwd.join("first-action.json");
    let cwd_md = different_cwd.join("first-action.md");
    let cwd_root = workspace_root().display().to_string();
    let cwd_result = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&different_cwd),
        &[
            "first-action",
            "--root",
            &cwd_root,
            "--receipt",
            receipt_path
                .to_str()
                .ok_or("receipt path should be utf-8")?,
            "--out",
            cwd_out.to_str().ok_or("output path should be utf-8")?,
            "--out-md",
            cwd_md.to_str().ok_or("markdown path should be utf-8")?,
        ],
    )?;
    assert_success(&cwd_result);
    assert!(std::fs::read_to_string(cwd_out)?.contains(r#""status": "already_improved""#));
    std::fs::remove_dir_all(different_cwd)?;
    let original_receipt: serde_json::Value = serde_json::from_str(&actual_receipt)?;
    // The producer records its non-dot relative `--root` argument.  A
    // consumer invoked from that argument's parent must resolve it once, not
    // append it to the already-selected root (`repo/repo`).
    let mut relative_root_receipt = original_receipt.clone();
    let relative_root_name = workspace_root()
        .file_name()
        .ok_or("workspace root should have a name")?
        .to_string_lossy()
        .to_string();
    relative_root_receipt["provenance"]["repo_root"] =
        serde_json::Value::String(relative_root_name.clone());
    let relative_root_receipt_path = out_dir.join("relative-root.json");
    std::fs::write(
        &relative_root_receipt_path,
        serde_json::to_string_pretty(&relative_root_receipt)?,
    )?;
    let mut relative_analysis: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&analysis_outcome_path)?)?;
    relative_analysis["root"] = serde_json::Value::String(relative_root_name.clone());
    std::fs::write(
        &analysis_outcome_path,
        serde_json::to_vec_pretty(&relative_analysis)?,
    )?;
    let workspace_root_for_parent = workspace_root();
    let parent_cwd = workspace_root_for_parent
        .parent()
        .ok_or("workspace should have parent")?;
    let relative_root_out = out_dir.join("relative-root-first-action.json");
    let relative_root_md = out_dir.join("relative-root-first-action.md");
    let relative_root_result = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(parent_cwd),
        &[
            "first-action",
            "--root",
            &relative_root_name,
            "--receipt",
            relative_root_receipt_path
                .to_str()
                .ok_or("relative receipt path should be utf-8")?,
            "--out",
            relative_root_out
                .to_str()
                .ok_or("relative output path should be utf-8")?,
            "--out-md",
            relative_root_md
                .to_str()
                .ok_or("relative markdown path should be utf-8")?,
        ],
    )?;
    assert_success(&relative_root_result);
    assert!(
        std::fs::read_to_string(relative_root_out)?.contains(r#""status": "already_improved""#)
    );
    std::fs::write(&analysis_outcome_path, &analysis_outcome_bytes)?;

    let original_analysis: serde_json::Value = serde_json::from_slice(&analysis_outcome_bytes)?;
    let mut swapped_analysis = original_analysis.clone();
    swapped_analysis["analysis_outcome"]["outcome"]["identity"]["repository_identity"] =
        serde_json::Value::String("swapped-repository-identity".to_string());
    std::fs::write(
        &analysis_outcome_path,
        serde_json::to_vec_pretty(&swapped_analysis)?,
    )?;
    let swapped_sidecar_out = out_dir.join("swapped-analysis-sidecar.json");
    let swapped_sidecar_md = out_dir.join("swapped-analysis-sidecar.md");
    let swapped_sidecar = run_ripr_in_workspace(&[
        "first-action",
        "--root",
        ".",
        "--receipt",
        receipt_path
            .to_str()
            .ok_or("receipt path should be utf-8")?,
        "--out",
        swapped_sidecar_out
            .to_str()
            .ok_or("swapped sidecar output path should be utf-8")?,
        "--out-md",
        swapped_sidecar_md
            .to_str()
            .ok_or("swapped sidecar markdown path should be utf-8")?,
    ])?;
    assert_success(&swapped_sidecar);
    let swapped_sidecar_rendered = std::fs::read_to_string(swapped_sidecar_out)?;
    let swapped_sidecar_value: serde_json::Value = serde_json::from_str(&swapped_sidecar_rendered)?;
    assert_eq!(
        swapped_sidecar_value["status"], "missing_required_artifact",
        "swapped analysis sidecar changed status: {swapped_sidecar_rendered}"
    );
    assert_eq!(
        swapped_sidecar_value["action_kind"], "generate_missing_artifact",
        "swapped analysis sidecar changed action: {swapped_sidecar_rendered}"
    );
    let swapped_warnings = swapped_sidecar_value["warnings"]
        .as_array()
        .ok_or("swapped sidecar warnings should be an array")?;
    assert!(
        swapped_warnings.iter().any(|warning| {
            warning.as_str().is_some_and(|warning| {
                warning.contains("analysis outcome semantic digest commitment")
            })
        }),
        "swapped analysis sidecar warning lost: {swapped_sidecar_rendered}"
    );
    std::fs::write(&analysis_outcome_path, &analysis_outcome_bytes)?;

    let original_verify = std::fs::read_to_string(artifact_dir.join("agent-verify.json"))?;
    let assert_not_promoted = |label: &str,
                               expected_reason: &str,
                               receipt: serde_json::Value|
     -> Result<(), Box<dyn std::error::Error>> {
        let receipt_text = serde_json::to_string_pretty(&receipt)?;
        std::fs::write(&receipt_path, receipt_text)?;
        let out = out_dir.join(format!("{label}.json"));
        let md = out_dir.join(format!("{label}.md"));
        let result = run_ripr_in_workspace(&[
            "first-action",
            "--root",
            ".",
            "--receipt",
            receipt_path
                .to_str()
                .ok_or("receipt path should be utf-8")?,
            "--out",
            out.to_str().ok_or("output path should be utf-8")?,
            "--out-md",
            md.to_str().ok_or("markdown path should be utf-8")?,
        ])?;
        assert_success(&result);
        let rendered = std::fs::read_to_string(out)?;
        let value: serde_json::Value = serde_json::from_str(&rendered)?;
        assert_eq!(
            value["status"], "missing_required_artifact",
            "{label} changed status: {rendered}"
        );
        assert_eq!(
            value["action_kind"], "generate_missing_artifact",
            "{label} changed action: {rendered}"
        );
        assert!(
            value["warnings"].as_array().is_some_and(|warnings| {
                warnings.iter().any(|warning| {
                    warning
                        .as_str()
                        .is_some_and(|warning| warning.contains(expected_reason))
                })
            }),
            "{label} lacked expected rejection `{expected_reason}`: {rendered}"
        );
        Ok(())
    };
    let mut malformed = original_receipt.clone();
    let malformed_path = "target/ripr/test-agent-verify/3408-malformed.json";
    std::fs::write(workspace_root().join(malformed_path), "{not json")?;
    malformed["inputs"]["agent_verify_json"] =
        serde_json::Value::String(malformed_path.to_string());
    malformed["provenance"]["verify_artifact"]["path"] =
        serde_json::Value::String(malformed_path.to_string());
    malformed["provenance"]["verify_artifact"]["sha256"] =
        serde_json::Value::String(sha256_hex_bytes(b"{not json"));
    assert_not_promoted("malformed", "[malformed]", malformed)?;
    std::fs::write(artifact_dir.join("agent-verify.json"), &original_verify)?;
    std::fs::write(&analysis_outcome_path, b"{not json")?;
    assert_not_promoted(
        "malformed-analysis",
        "analysis outcome artifact is unavailable",
        original_receipt.clone(),
    )?;
    std::fs::write(&analysis_outcome_path, &analysis_outcome_bytes)?;
    let mut noncanonical_currentness = original_receipt.clone();
    let noncanonical_currentness_path =
        "target/ripr/test-agent-verify/3408-noncanonical-currentness.json";
    let mut noncanonical_verify: serde_json::Value = serde_json::from_str(&original_verify)?;
    noncanonical_verify["artifact_currentness"] =
        serde_json::Value::String("historical_noncurrent".to_string());
    let noncanonical_text = serde_json::to_string_pretty(&noncanonical_verify)?;
    std::fs::write(
        workspace_root().join(noncanonical_currentness_path),
        &noncanonical_text,
    )?;
    noncanonical_currentness["inputs"]["agent_verify_json"] =
        serde_json::Value::String(noncanonical_currentness_path.to_string());
    noncanonical_currentness["provenance"]["verify_artifact"]["path"] =
        serde_json::Value::String(noncanonical_currentness_path.to_string());
    noncanonical_currentness["provenance"]["verify_artifact"]["sha256"] =
        serde_json::Value::String(sha256_hex_bytes(noncanonical_text.as_bytes()));
    assert_not_promoted(
        "noncanonical-currentness",
        "[not_canonical]",
        noncanonical_currentness,
    )?;
    let mut swapped = original_receipt.clone();
    let after_path = swapped["provenance"]["after_artifact"]["path"]
        .as_str()
        .ok_or("after path")?
        .to_string();
    let after_sha = swapped["provenance"]["after_artifact"]["sha256"].clone();
    swapped["provenance"]["before_artifact"]["path"] = serde_json::Value::String(after_path);
    swapped["provenance"]["before_artifact"]["sha256"] = after_sha;
    assert_not_promoted("swapped", "receipt artifact paths do not match", swapped)?;
    let mut wrong_root = original_receipt.clone();
    wrong_root["provenance"]["repo_root"] = serde_json::Value::String("..".to_string());
    assert_not_promoted("wrong-root", "receipt repo_root does not match", wrong_root)?;
    let mut wrong_seam = original_receipt.clone();
    wrong_seam["provenance"]["seam_id"] = serde_json::Value::String("forged-seam".to_string());
    assert_not_promoted("wrong-seam", "verify artifact has no", wrong_seam)?;
    let mut forged = original_receipt.clone();
    let forged_path = "target/ripr/test-agent-verify/3408-forged.json";
    let mut forged_verify: serde_json::Value = serde_json::from_str(&original_verify)?;
    forged_verify["changed_seams"][0]["seam_id"] =
        serde_json::Value::String("forged-seam".to_string());
    let forged_text = serde_json::to_string_pretty(&forged_verify)?;
    std::fs::write(workspace_root().join(forged_path), &forged_text)?;
    forged["inputs"]["agent_verify_json"] = serde_json::Value::String(forged_path.to_string());
    forged["provenance"]["verify_artifact"]["path"] =
        serde_json::Value::String(forged_path.to_string());
    forged["provenance"]["verify_artifact"]["sha256"] =
        serde_json::Value::String(sha256_hex_bytes(forged_text.as_bytes()));
    assert_not_promoted("forged", "[not_canonical]", forged)?;
    let mut forged_grip = original_receipt.clone();
    forged_grip["seam"]["grip_class"] = serde_json::Value::String("forged_class".to_string());
    assert_not_promoted("forged-grip", "verify artifact has no", forged_grip)?;
    let mut forged_strength = original_receipt.clone();
    forged_strength["current_evidence_strength"] =
        serde_json::Value::String("forged_strength".to_string());
    assert_not_promoted("forged-strength", "verify artifact has no", forged_strength)?;
    std::fs::remove_dir_all(out_dir)?;
    std::fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn resolved_receipt_promotes_through_first_action() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("first-action-resolved");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"resolved-seam","kind":"predicate_boundary","file":"src/lib.rs","line":7,"grip_class":"weakly_gripped"}"#,
    )?;
    let mut before_value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&before)?)?;
    before_value["artifact"]["analysis"]["worktree"] =
        serde_json::Value::String("dirty".to_string());
    std::fs::write(
        &before,
        recommit_repo_exposure_json(serde_json::to_string_pretty(&before_value)?),
    )?;
    write_bound_repo_exposure_fixture(&root, &after, "")?;
    let mut after_value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&after)?)?;
    after_value["artifact"]["analysis"]["worktree"] =
        serde_json::Value::String("dirty".to_string());
    std::fs::write(
        &after,
        recommit_repo_exposure_json(serde_json::to_string_pretty(&after_value)?),
    )?;

    let verify = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify);
    let verify_value: serde_json::Value = serde_json::from_slice(&verify.stdout)?;
    let resolved = verify_value["resolved_gaps"]
        .as_array()
        .and_then(|items| items.first())
        .ok_or("verify should emit a resolved gap")?;
    assert_eq!(resolved["change"], "resolved");
    let seam_id = resolved["seam_id"].as_str().ok_or("resolved seam id")?;
    let verify_path = root.join("agent-verify.json");
    std::fs::write(&verify_path, &verify.stdout)?;
    let analysis = run_ripr(&[
        "check",
        "--root",
        &root.display().to_string(),
        "--mode",
        "draft",
        "--base",
        "HEAD",
        "--format",
        "json",
    ]);
    assert_success(&analysis);
    std::fs::write(root.join("analysis-outcome.json"), &analysis.stdout)?;
    let receipt = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify_path.display().to_string(),
        "--seam-id",
        seam_id,
        "--json",
    ]);
    assert_success(&receipt);
    let receipt_path = root.join("agent-receipt.json");
    std::fs::write(&receipt_path, &receipt.stdout)?;
    let out = root.join("first-action.json");
    let md = root.join("first-action.md");
    let first_action = run_ripr(&[
        "first-action",
        "--root",
        &root.display().to_string(),
        "--receipt",
        &receipt_path.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--out-md",
        &md.display().to_string(),
    ]);
    assert_success(&first_action);
    let rendered = std::fs::read_to_string(&out)?;
    assert!(
        rendered.contains(r#""status": "already_improved""#),
        "{rendered}"
    );
    let mut forged_resolved: serde_json::Value = serde_json::from_slice(&receipt.stdout)?;
    forged_resolved["provenance"]["before_class"] =
        serde_json::Value::String("forged_class".to_string());
    std::fs::write(
        &receipt_path,
        serde_json::to_string_pretty(&forged_resolved)?,
    )?;
    let forged_out = root.join("forged-resolved-first-action.json");
    let forged_md = root.join("forged-resolved-first-action.md");
    let forged_first_action = run_ripr(&[
        "first-action",
        "--root",
        &root.display().to_string(),
        "--receipt",
        &receipt_path.display().to_string(),
        "--out",
        &forged_out.display().to_string(),
        "--out-md",
        &forged_md.display().to_string(),
    ]);
    assert_success(&forged_first_action);
    let forged_rendered = std::fs::read_to_string(&forged_out)?;
    assert!(
        forged_rendered.contains("missing_required_artifact")
            && !forged_rendered.contains(r#""status": "already_improved""#),
        "forged resolved receipt was promoted: {forged_rendered}"
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn test_oracle_assistant_canonical_review_loop_fixture_pins_expected_surfaces()
-> Result<(), Box<dyn std::error::Error>> {
    let base = "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical";
    let fixture_dir = workspace_root().join(base);
    let proof_path = fixture_dir.join("test-oracle-assistant-proof.json");
    let proof_md_path = fixture_dir.join("test-oracle-assistant-proof.md");

    let proof_text = std::fs::read_to_string(&proof_path)?;
    let proof: serde_json::Value = serde_json::from_str(&proof_text)?;
    let seam_id = json_pointer_str(&proof, "/seam/seam_id")?;
    assert_eq!(seam_id, "67fc764ba37d77bd");
    assert_eq!(
        json_pointer_str(&proof, "/kind")?,
        "test_oracle_assistant_loop"
    );
    assert_eq!(json_pointer_str(&proof, "/status")?, "advisory");
    assert_eq!(
        json_pointer_str(&proof, "/seam/grip_class")?,
        "weakly_gripped"
    );
    assert_eq!(
        json_pointer_str(&proof, "/seam/missing_discriminator")?,
        "discount_threshold (equality boundary)"
    );
    assert_eq!(
        json_pointer_str(&proof, "/recommendation/placement")?,
        "changed_line"
    );
    assert!(
        json_pointer_str(&proof, "/recommendation/suggested_test")?
            .contains("amount == discount_threshold")
    );
    assert_eq!(
        json_pointer_str(&proof, "/evidence_movement/state")?,
        "improved"
    );
    assert!(json_pointer_bool(&proof, "/limits/advisory")?);
    for pointer in [
        "/limits/source_edits",
        "/limits/generated_tests",
        "/limits/external_service",
        "/limits/runtime_mutation_execution",
        "/limits/ci_blocking_default",
    ] {
        assert!(!json_pointer_bool(&proof, pointer)?);
    }

    for pointer in [
        "/inputs/pr_guidance",
        "/inputs/agent_packet",
        "/inputs/before",
        "/inputs/after",
        "/inputs/receipt",
        "/inputs/ledger",
    ] {
        let path = json_pointer_str(&proof, pointer)?;
        assert!(
            workspace_root().join(path).exists(),
            "expected `{path}` from `{pointer}` to exist"
        );
    }
    assert!(
        proof
            .pointer("/inputs/coverage_frontier")
            .is_some_and(serde_json::Value::is_null)
    );

    let pr_guidance_path = workspace_root().join(json_pointer_str(&proof, "/inputs/pr_guidance")?);
    let agent_packet_path =
        workspace_root().join(json_pointer_str(&proof, "/inputs/agent_packet")?);
    let receipt_path = workspace_root().join(json_pointer_str(&proof, "/inputs/receipt")?);
    let ledger_path = workspace_root().join(json_pointer_str(&proof, "/inputs/ledger")?);

    let pr_guidance: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(pr_guidance_path)?)?;
    let agent_packet: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(agent_packet_path)?)?;
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(receipt_path)?)?;
    let ledger: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(ledger_path)?)?;

    assert_eq!(
        json_pointer_str(&pr_guidance, "/comments/0/seam_id")?,
        seam_id
    );
    assert_eq!(
        json_pointer_str(&agent_packet, "/top_seams/0/seam_id")?,
        seam_id
    );
    assert_eq!(json_pointer_str(&receipt, "/provenance/seam_id")?, seam_id);
    assert_eq!(
        json_pointer_str(&ledger, "/top_repair_route/seam_id")?,
        seam_id
    );
    assert_eq!(
        json_pointer_str(&ledger, "/repair_receipts/0/seam_id")?,
        seam_id
    );
    assert_eq!(
        json_pointer_str(&proof, "/evidence_movement/state")?,
        json_pointer_str(&receipt, "/provenance/movement")?
    );
    assert_eq!(
        json_pointer_str(&ledger, "/repair_receipts/0/static_movement/state")?,
        json_pointer_str(&proof, "/evidence_movement/state")?
    );
    assert_eq!(
        json_pointer_str(&agent_packet, "/top_seams/0/recommended_test/file")?,
        "tests/pricing.rs"
    );
    assert_eq!(
        json_pointer_str(&agent_packet, "/top_seams/0/recommended_test/name")?,
        "discounted_total_boundary_discriminator"
    );
    assert_eq!(
        json_pointer_str(
            &agent_packet,
            "/top_seams/0/nearest_strong_test_to_imitate/name"
        )?,
        "below_threshold_has_no_discount"
    );

    let proof_md = std::fs::read_to_string(proof_md_path)?;
    assert!(proof_md.contains("Status: advisory"));
    assert!(proof_md.contains("Missing discriminator: discount_threshold (equality boundary)"));
    assert!(proof_md.contains("After: strongly_gripped"));
    assert!(proof_md.contains("State: improved"));
    assert!(proof_md.contains("Gate: not configured"));
    Ok(())
}

#[test]
fn first_useful_action_corpus_pins_routing_cases() -> Result<(), Box<dyn std::error::Error>> {
    let base = "fixtures/boundary_gap/expected/first-useful-action";
    let fixture_dir = workspace_root().join(base);
    let corpus_path = fixture_dir.join("corpus.json");
    let corpus: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(corpus_path)?)?;
    assert_eq!(json_pointer_str(&corpus, "/schema_version")?, "0.1");
    assert_eq!(
        json_pointer_str(&corpus, "/kind")?,
        "first_useful_action_corpus"
    );
    assert_eq!(json_pointer_str(&corpus, "/spec")?, "RIPR-SPEC-0020");

    let cases = corpus
        .pointer("/cases")
        .and_then(serde_json::Value::as_array)
        .ok_or("expected `/cases` array")?;
    let expected = [
        (
            "actionable",
            "actionable_pr_local_boundary",
            "actionable",
            "write_focused_test",
        ),
        (
            "repair-start",
            "repair_start_carried",
            "actionable",
            "write_focused_test",
        ),
        (
            "stale",
            "stale_editor_evidence",
            "stale",
            "refresh_evidence",
        ),
        (
            "missing-required-artifact",
            "missing_assistant_proof",
            "missing_required_artifact",
            "generate_missing_artifact",
        ),
        (
            "baseline-only",
            "baseline_only_debt",
            "baseline_only",
            "acknowledge_baseline",
        ),
        (
            "acknowledged",
            "acknowledged_pr_gap",
            "acknowledged",
            "inspect_proof_report",
        ),
        ("waived", "waived_pr_gap", "waived", "no_action"),
        (
            "suppressed",
            "suppressed_configured_off",
            "suppressed",
            "no_action",
        ),
        (
            "no-actionable-seam",
            "no_actionable_seam_clean",
            "no_actionable_seam",
            "no_action",
        ),
        (
            "already-improved",
            "already_improved_receipt",
            "already_improved",
            "no_action",
        ),
        // The portable-normalized receipt is not promotable, so the router
        // fails closed before it can route `unchanged` movement.
        (
            "unchanged-after-attempt",
            "unchanged_after_attempt",
            "missing_required_artifact",
            "generate_missing_artifact",
        ),
    ];
    assert_eq!(cases.len(), expected.len());

    for (case_dir, case_id, status, action_kind) in expected {
        let Some(case) = cases
            .iter()
            .find(|case| case.get("id").and_then(serde_json::Value::as_str) == Some(case_id))
        else {
            return Err(format!("missing first useful action case `{case_id}`").into());
        };
        assert_eq!(json_pointer_str(case, "/expected/status")?, status);
        assert_eq!(
            json_pointer_str(case, "/expected/action_kind")?,
            action_kind
        );

        let unchanged_control = if case_id == "unchanged_after_attempt" {
            let proof_artifact = json_pointer_str(case, "/inputs/assistant_proof/artifact")?;
            let receipt_artifact = json_pointer_str(case, "/inputs/receipt/artifact")?;
            assert_eq!(
                proof_artifact,
                "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/assistant-proof.json"
            );
            assert_eq!(
                receipt_artifact,
                "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/agent-receipt.json"
            );

            let proof: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                workspace_root().join(proof_artifact),
            )?)?;
            let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                workspace_root().join(receipt_artifact),
            )?)?;
            assert_eq!(
                json_pointer_str(&proof, "/evidence_movement/state")?,
                "unchanged"
            );
            assert_eq!(
                json_pointer_str(&proof, "/evidence_movement/after_class")?,
                "weakly_gripped"
            );
            let before_artifact = json_pointer_str(&proof, "/inputs/before")?;
            let after_artifact = json_pointer_str(&proof, "/inputs/after")?;
            assert_eq!(
                before_artifact,
                "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/before.repo-exposure.json"
            );
            assert_eq!(
                after_artifact,
                "fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/after.repo-exposure.json"
            );
            let before: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                workspace_root().join(before_artifact),
            )?)?;
            let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                workspace_root().join(after_artifact),
            )?)?;
            let before_bytes = std::fs::read(workspace_root().join(before_artifact))?;
            let after_bytes = std::fs::read(workspace_root().join(after_artifact))?;
            let produced = run_ripr_in_workspace(&[
                "check",
                "--root",
                "fixtures/boundary_gap/input",
                "--format",
                "repo-exposure-json",
            ])?;
            assert_success(&produced);
            let produced: serde_json::Value = serde_json::from_slice(&produced.stdout)?;
            let normalized_produced = normalize_unchanged_repo_exposure_producer_fixture(produced)?;
            assert_eq!(
                before, normalized_produced,
                "before snapshot must be the portable normalization of production output"
            );
            assert_eq!(
                after, normalized_produced,
                "after snapshot must be the portable normalization of production output"
            );
            let evidence_record = before
                .pointer("/seams/0/evidence_record")
                .ok_or("portable producer fixture must retain evidence_record")?;
            assert_eq!(
                json_pointer_str(evidence_record, "/canonical_item/evidence_class")?,
                "predicate_boundary"
            );
            assert_eq!(
                json_pointer_str(evidence_record, "/canonical_item/gap_state")?,
                "actionable"
            );
            assert_repo_exposure_rejects_mutation(
                &before,
                |snapshot| {
                    if let Some(seam) = snapshot
                        .pointer_mut("/seams/0")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        seam.remove("evidence_record");
                    }
                },
                "content commitment mismatch",
            )?;
            assert_repo_exposure_rejects_mutation(
                &before,
                |snapshot| {
                    snapshot["seams"][0]["evidence_record"]["canonical_item"]["gap_state"] =
                        serde_json::json!("suppressed");
                },
                "content commitment mismatch",
            )?;
            assert_repo_exposure_rejects_mutation(
                &before,
                |snapshot| {
                    snapshot["artifact"]["content_sha256"] = serde_json::json!(
                        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    );
                },
                "content commitment mismatch",
            )?;
            let seam_class =
                |snapshot: &serde_json::Value| -> Result<String, Box<dyn std::error::Error>> {
                    snapshot
                        .pointer("/seams")
                        .and_then(serde_json::Value::as_array)
                        .and_then(|seams| {
                            seams.iter().find(|seam| {
                                seam.pointer("/seam_id").and_then(serde_json::Value::as_str)
                                    == Some("67fc764ba37d77bd")
                            })
                        })
                        .and_then(|seam| seam.pointer("/grip_class"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| "target seam must have a grip_class".into())
                };
            let before_class = seam_class(&before)?;
            let after_class = seam_class(&after)?;
            assert_eq!(before_class, "weakly_gripped");
            assert_eq!(after_class, "weakly_gripped");
            assert_eq!(
                before_class, after_class,
                "scenario snapshots must derive unchanged movement"
            );
            assert_eq!(
                json_pointer_str(&proof, "/evidence_movement/before_class")?,
                &before_class
            );
            assert_eq!(
                json_pointer_str(&proof, "/evidence_movement/after_class")?,
                &after_class
            );
            assert!(
                proof
                    .pointer("/inputs/ledger")
                    .is_some_and(serde_json::Value::is_null)
            );
            assert!(
                proof
                    .pointer("/ci_projection/ledger")
                    .is_some_and(serde_json::Value::is_null)
            );
            assert_eq!(json_pointer_str(&receipt, "/seam/change")?, "unchanged");
            assert_eq!(json_pointer_str(&receipt, "/seam/after")?, "weakly_gripped");
            assert_eq!(json_pointer_str(&receipt, "/seam/before")?, before_class);
            assert_eq!(json_pointer_str(&receipt, "/seam/after")?, after_class);
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/before_artifact/path")?,
                before_artifact
            );
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/after_artifact/path")?,
                after_artifact
            );
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/before_artifact/sha256")?,
                sha256_hex_bytes(&before_bytes)
            );
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/after_artifact/sha256")?,
                sha256_hex_bytes(&after_bytes)
            );
            let verify_artifact = json_pointer_str(&receipt, "/inputs/agent_verify_json")?;
            let verify_bytes = std::fs::read(workspace_root().join(verify_artifact))?;
            let verify: serde_json::Value = serde_json::from_slice(&verify_bytes)?;
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/verify_artifact/path")?,
                verify_artifact
            );
            assert_eq!(
                json_pointer_str(&receipt, "/provenance/verify_artifact/sha256")?,
                sha256_hex_bytes(&verify_bytes)
            );
            assert_eq!(
                json_pointer_str(&verify, "/inputs/before")?,
                before_artifact
            );
            assert_eq!(json_pointer_str(&verify, "/inputs/after")?, after_artifact);
            assert_eq!(
                json_pointer_str(&verify, "/inputs/before_content_sha256")?,
                json_pointer_str(&before, "/artifact/content_sha256")?
            );
            assert_eq!(
                json_pointer_str(&verify, "/inputs/after_content_sha256")?,
                json_pointer_str(&after, "/artifact/content_sha256")?
            );
            assert_eq!(
                json_pointer_str(&verify, "/unchanged_seams/0/change")?,
                "unchanged"
            );
            assert_eq!(
                json_pointer_str(&verify, "/unchanged_seams/0/gap_movement")?,
                "unchanged"
            );
            assert_eq!(
                verify.pointer("/unchanged_seams/0/observed_values_added"),
                Some(&serde_json::json!([]))
            );
            assert_eq!(
                verify.pointer("/unchanged_seams/0/observed_values_removed"),
                Some(&serde_json::json!([]))
            );
            assert_eq!(
                verify.pointer("/unchanged_seams/0/related_test_delta"),
                Some(&serde_json::json!(0))
            );
            assert_eq!(
                receipt.pointer("/seam/evidence_delta"),
                verify.pointer("/unchanged_seams/0/evidence_delta")
            );
            Some((proof_artifact, receipt_artifact))
        } else {
            None
        };

        let report_path = fixture_dir.join(case_dir).join("first-useful-action.json");
        let markdown_path = fixture_dir.join(case_dir).join("first-useful-action.md");
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(report_path)?)?;
        assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
        assert_eq!(json_pointer_str(&report, "/tool")?, "ripr");
        assert_eq!(json_pointer_str(&report, "/kind")?, "first_useful_action");
        assert_eq!(json_pointer_str(&report, "/status")?, status);
        assert_eq!(json_pointer_str(&report, "/action_kind")?, action_kind);

        if let Some((proof_artifact, receipt_artifact)) = unchanged_control {
            assert_eq!(
                json_pointer_str(&report, "/inputs/assistant_proof")?,
                proof_artifact
            );
            assert_eq!(
                json_pointer_str(&report, "/inputs/receipt")?,
                receipt_artifact
            );
            assert!(
                report
                    .pointer("/inputs/ledger")
                    .is_some_and(serde_json::Value::is_null)
            );
            assert_eq!(
                json_pointer_str(&report, "/evidence/assistant_proof")?,
                proof_artifact
            );
            assert_eq!(
                json_pointer_str(&report, "/evidence/receipt")?,
                receipt_artifact
            );
            assert!(
                report
                    .pointer("/evidence/ledger")
                    .is_some_and(serde_json::Value::is_null)
            );
            assert_eq!(
                json_pointer_str(&report, "/evidence/static_movement")?,
                "unknown"
            );
            assert_eq!(
                json_pointer_str(&report, "/warnings/0")?,
                "receipt movement `unchanged` is not promotable: receipt does not contain a complete analysis outcome"
            );
        }
        assert_eq!(
            json_pointer_str(&report, "/generated_at")?,
            "2026-05-09T12:00:00Z"
        );

        let why_first = report
            .pointer("/why_first")
            .and_then(serde_json::Value::as_array)
            .ok_or("expected why_first array")?;
        assert!(
            !why_first.is_empty(),
            "`{case_id}` should explain why the route came first"
        );

        let limits = report
            .pointer("/limits")
            .and_then(serde_json::Value::as_array)
            .ok_or("expected limits array")?;
        assert!(
            limits
                .iter()
                .any(|limit| limit.as_str() == Some("Static evidence only.")),
            "`{case_id}` should preserve the static-evidence limit"
        );

        if case
            .pointer("/expected/fallback")
            .is_some_and(|v| !v.is_null())
        {
            assert!(
                report.pointer("/fallback").is_some_and(|v| !v.is_null()),
                "`{case_id}` should include a fallback report object"
            );
        }

        if case_id == "repair_start_carried" {
            // #3906: the carried command is the card's own string, byte for byte.
            let guidance_artifact = json_pointer_str(case, "/inputs/pr_guidance/artifact")?;
            let guidance: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                workspace_root().join(guidance_artifact),
            )?)?;
            assert_eq!(
                json_pointer_str(&report, "/commands/repair")?,
                json_pointer_str(&guidance, "/comments/0/llm_guidance/repair_command")?
            );
            assert_eq!(
                json_pointer_str(&report, "/selected/seam_id")?,
                json_pointer_str(&guidance, "/comments/0/seam_id")?
            );
            assert!(
                report
                    .pointer("/inputs/assistant_proof")
                    .is_some_and(serde_json::Value::is_null),
                "`{case_id}` must lead before any assistant proof exists"
            );
        }

        if case_id == "missing_assistant_proof" {
            assert!(
                report
                    .pointer("/inputs/assistant_proof")
                    .is_some_and(serde_json::Value::is_null),
                "`{case_id}` should not claim a missing assistant proof input is present"
            );
        }

        let markdown = std::fs::read_to_string(markdown_path)?;
        assert!(
            markdown.contains(&format!("Status: {status}")),
            "`{case_id}` Markdown should pin status `{status}`"
        );
        assert!(
            markdown.contains(&format!("Action: {action_kind}")),
            "`{case_id}` Markdown should pin action `{action_kind}`"
        );
    }
    Ok(())
}

#[test]
fn agent_start_writes_source_edit_free_workflow_packet() -> Result<(), Box<dyn std::error::Error>> {
    let seam_id = "67fc764ba37d77bd";
    let out_dir = unique_temp_workspace("agent-start");
    let out = out_dir
        .to_str()
        .ok_or("workflow output path should be utf-8")?;

    let output = run_ripr_in_workspace(&[
        "agent",
        "start",
        "--root",
        "fixtures/boundary_gap/input",
        "--seam-id",
        seam_id,
        "--out",
        out,
    ])?;
    assert_success(&output);

    let workflow_json = std::fs::read_to_string(out_dir.join("workflow.json"))?;
    let commands_md = std::fs::read_to_string(out_dir.join("commands.md"))?;
    let agent_brief_json = std::fs::read_to_string(out_dir.join("agent-brief.json"))?;

    assert!(workflow_json.contains(r#""schema_version": "0.1""#));
    assert!(workflow_json.contains(r#""source_edits": false"#));
    assert!(workflow_json.contains(r#""llm_api_calls": false"#));
    assert!(workflow_json.contains(seam_id));
    // #4000: generated commands bind the selected root the invocation resolved.
    // The expected root uses the renderer's shell quoting, so a checkout path
    // with spaces or other shell metacharacters still matches exactly.
    let expected_verify = format!(
        "ripr agent verify --root {} ",
        renderer_shell_arg(&format!(
            "{}/fixtures/boundary_gap/input",
            workspace_root().to_string_lossy().replace('\\', "/")
        ))
    );
    let workflow: serde_json::Value = serde_json::from_str(&workflow_json)?;
    assert!(
        json_strings(&workflow)
            .iter()
            .any(|value| value.contains(&expected_verify)),
        "workflow.json should carry `{expected_verify}`: {workflow_json}"
    );
    assert!(commands_md.contains("# RIPR Agent Workflow"));
    assert!(commands_md.contains("Does not edit source files."));
    assert!(commands_md.contains("Does not call an LLM API."));
    assert!(agent_brief_json.contains(seam_id));

    std::fs::remove_dir_all(out_dir)?;
    Ok(())
}

#[test]
fn agent_start_packet_discloses_that_generated_commands_assume_bash()
-> Result<(), Box<dyn std::error::Error>> {
    let out_dir = unique_temp_workspace("agent-start-shell-disclosure");
    let out = out_dir
        .to_str()
        .ok_or("workflow output path should be utf-8")?;

    let output = run_ripr_in_workspace(&[
        "agent",
        "start",
        "--root",
        "fixtures/boundary_gap/input",
        "--seam-id",
        "67fc764ba37d77bd",
        "--out",
        out,
    ])?;
    assert_success(&output);

    let commands_md = std::fs::read_to_string(out_dir.join("commands.md"))?;
    let workflow_json = std::fs::read_to_string(out_dir.join("workflow.json"))?;

    // The commands are already fenced as ```bash, so a bare `bash` substring is
    // not evidence. Require the prose disclosure ahead of the first fence.
    let disclosure = commands_md
        .find("Each step includes Bash command forms and, where supported, PowerShell forms; unavailable variants are disclosed.")
        .ok_or_else(|| format!("commands.md must disclose the bash assumption:\n{commands_md}"))?;
    let first_fence = commands_md
        .find("```bash")
        .ok_or("commands.md must still fence commands as bash")?;
    assert!(
        disclosure < first_fence,
        "bash disclosure must precede the first copyable command in commands.md"
    );
    assert!(
        commands_md.contains("PowerShell"),
        "commands.md must name the shells that do not accept these commands:\n{commands_md}"
    );
    assert!(
        workflow_json.contains(r#""command_shell": "bash""#),
        "workflow.json must name the shell its command strings assume:\n{workflow_json}"
    );

    std::fs::remove_dir_all(out_dir)?;
    Ok(())
}

#[test]
fn agent_repair_phases_materialize_snapshots_and_verify_json()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-repair-phases");
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"boundary_gap_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"boundary_gap_fixture\"\npath = \"src/lib.rs\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\n",
    )?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        "use boundary_gap_fixture::discounted_total;\n\n#[test]\nfn below_threshold_has_no_discount() {\n    assert_eq!(discounted_total(50, 100), 50);\n}\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n",
    )?;
    init_git_fixture_repo(&root)?;
    run_git(&root, &["add", "Cargo.toml", "src", "tests"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "fixture source",
        ],
    )?;
    assert!(
        commit.status.success(),
        "fixture source commit failed: {commit:?}"
    );

    let root_arg = root.display().to_string();
    let before = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--phase",
        "before",
    ]);
    assert_success(&before);
    // The before phase's stdout is one JSON document: the packet envelope
    // carrying the additive `repair_attempt` continuation (#4329), and its
    // narration names a test-only edit without re-sending the user to take
    // the before snapshot this phase already wrote.
    let before_stdout: serde_json::Value = serde_json::from_slice(&before.stdout)?;
    assert_eq!(before_stdout["packets"][0]["seam_id"], "67fc764ba37d77bd");
    let before_stdout_text = String::from_utf8_lossy(&before.stdout);
    assert!(
        !before_stdout_text.contains("ripr: ") && !before_stdout_text.contains("Next:"),
        "before phase narration belongs on stderr, not in the packet JSON:\n{before_stdout_text}"
    );
    // A driver that captures only stdout can complete before → edit → after
    // without stderr: the document names the attempt, its manifest, the
    // packet path, and the exact resume command (#4329).
    assert_eq!(before_stdout["schema_version"], "0.5");
    let continuation = &before_stdout["repair_attempt"];
    let attempt_id = continuation["attempt_id"]
        .as_str()
        .ok_or("before-phase stdout is missing repair_attempt.attempt_id")?
        .to_string();
    let next_command = continuation["next_command"]
        .as_str()
        .ok_or("before-phase stdout is missing repair_attempt.next_command")?
        .to_string();
    assert!(
        next_command.contains(&format!("--attempt {attempt_id}"))
            && next_command.contains("--phase after"),
        "next command must resume this attempt:\n{next_command}"
    );
    assert!(
        continuation["manifest_path"]
            .as_str()
            .is_some_and(|path| path.contains("attempt.json")),
        "continuation must name the attempt manifest: {continuation}"
    );
    assert!(
        continuation["packet_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("target/ripr/workflow/agent-packet.json")),
        "continuation must name the packet path: {continuation}"
    );
    let before_stderr = String::from_utf8_lossy(&before.stderr);
    assert!(
        before_stderr
            .contains("add or strengthen one focused test (leave production code unchanged)"),
        "before phase must name a test-only edit:\n{before_stderr}"
    );
    assert!(
        !before_stderr.contains("Edit the source code"),
        "before phase must not ask for a source edit:\n{before_stderr}"
    );
    assert!(
        !before_stderr.contains("Next: ripr check"),
        "before phase must not re-issue the snapshot it already took:\n{before_stderr}"
    );

    let before_snapshot = root.join("target/ripr/workflow/before.repo-exposure.json");
    assert!(before_snapshot.is_file());
    let packet_path = root.join("target/ripr/workflow/agent-packet.json");
    let packet: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&packet_path)?)?;
    assert_eq!(packet["packets_total"], 1);
    assert_eq!(packet["packets"][0]["seam_id"], "67fc764ba37d77bd");

    let after_snapshot = root.join("target/ripr/workflow/after.repo-exposure.json");
    let stale_after = serde_json::json!({
        "stale_marker": "previous repair run",
        "source": std::fs::read_to_string(&before_snapshot)?,
    });
    std::fs::write(&after_snapshot, serde_json::to_vec(&stale_after)?)?;

    // Simulate the agent edit step between phases: the after snapshot must
    // observe movement, disclosed through the dirty worktree, because a fully
    // current same-revision pair is rejected as no-movement (#2922).
    std::fs::write(
        root.join("tests/pricing.rs"),
        "use boundary_gap_fixture::discounted_total;\n\n#[test]\nfn below_threshold_has_no_discount() {\n    assert_eq!(discounted_total(50, 100), 50);\n}\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n\n#[test]\nfn at_threshold_discounts() {\n    assert_eq!(discounted_total(100, 100), 90);\n}\n",
    )?;

    // Drive the after phase from the stdout document alone (#4329): run the
    // printed command's arguments, not a hand-written equivalent, so a drift
    // between what the continuation names and what the loop needs is the
    // bug. This driver adds --json because the assertions below read the
    // after phase's result document from stdout; the seam-selected after
    // phase stays covered by the replay and receipt checks below.
    let mut next_args: Vec<&str> = next_command
        .strip_prefix("ripr ")
        .ok_or_else(|| format!("next command is not a ripr command: {next_command}"))?
        .split_whitespace()
        .collect();
    next_args.push("--json");
    let after = run_ripr(&next_args);
    assert_success(&after);
    let after_stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        after_stderr.contains("ripr: result for seam `67fc764ba37d77bd`: weak -> "),
        "after phase must name the seam's movement:\n{after_stderr}"
    );
    assert!(
        !after_stderr.contains("could not read"),
        "after phase must read the receipt it just wrote:\n{after_stderr}"
    );
    assert!(
        after_stderr.contains("ripr: after phase complete. Receipt: "),
        "after phase must name the receipt path:\n{after_stderr}"
    );
    let after_snapshot_text = std::fs::read_to_string(&after_snapshot)?;
    assert!(!after_snapshot_text.contains("previous repair run"));

    let verify_json = root.join("target/ripr/workflow/agent-verify.json");
    assert!(verify_json.is_file());
    let verify: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(verify_json)?)?;
    assert_eq!(verify["tool"], "ripr");
    let receipt_path = root.join("target/ripr/reports/agent-receipt.json");
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["provenance"]["seam_id"], "67fc764ba37d77bd");
    assert_eq!(receipt["repair_attempt"]["seam_id"], "67fc764ba37d77bd");
    assert_eq!(receipt["repair_attempt"]["current"], true);
    assert_eq!(
        receipt["repair_attempt"]["edit_cage_verdict"]["status"],
        "compliant"
    );
    assert_eq!(receipt["test_changed"], "tests/pricing.rs");
    let attempts = std::fs::read_dir(root.join("target/ripr/repair-attempts"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    assert_eq!(attempts.len(), 1);
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        attempts[0].path().join("attempt.json"),
    )?)?;
    assert_eq!(manifest["state"], "ready_to_finish");
    assert_eq!(
        receipt["repair_attempt"]["attempt_id"],
        manifest["repair_attempt_id"]
    );
    assert_eq!(manifest["after"]["current"], true);
    let wrong_seam = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "wrong-seam",
        "--json",
    ]);
    assert!(!wrong_seam.status.success());
    let packet_bytes = std::fs::read(&packet_path)?;
    std::fs::write(&packet_path, b"tampered packet")?;
    let tampered = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert!(!tampered.status.success());
    std::fs::write(&packet_path, packet_bytes)?;
    let replay = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--phase",
        "after",
    ]);
    assert!(!replay.status.success());
    let manifest_path = attempts[0].path().join("attempt.json");
    let manifest_bytes = std::fs::read(&manifest_path)?;
    let mut root_tampered: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    root_tampered["root"] = serde_json::Value::String(root.join("elsewhere").display().to_string());
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&root_tampered)?)?;
    let root_rejected = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert!(!root_rejected.status.success());
    std::fs::write(&manifest_path, &manifest_bytes)?;

    let mut path_tampered: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    path_tampered["artifacts"][0]["path"] = serde_json::Value::String("../outside".to_string());
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&path_tampered)?)?;
    let path_rejected = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert!(!path_rejected.status.success());
    std::fs::write(&manifest_path, &manifest_bytes)?;

    let manifest_value: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    let baseline_rel = manifest_value["artifacts"]
        .as_array()
        .and_then(|artifacts| {
            artifacts
                .iter()
                .find(|artifact| artifact["role"] == "edit_cage_baseline")
        })
        .and_then(|artifact| artifact["path"].as_str())
        .ok_or("baseline artifact missing")?;
    let baseline_path = root.join(baseline_rel);
    let baseline_bytes = std::fs::read(&baseline_path)?;
    std::fs::write(&baseline_path, b"tampered baseline")?;
    let baseline_rejected = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert!(!baseline_rejected.status.success());
    std::fs::write(&baseline_path, baseline_bytes.clone())?;
    let mut coordinated: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    if let Some(artifact) = coordinated["artifacts"]
        .as_array_mut()
        .and_then(|artifacts| {
            artifacts
                .iter_mut()
                .find(|artifact| artifact["role"] == "edit_cage_baseline")
        })
    {
        artifact["sha256"] = serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
        artifact["bytes"] = serde_json::Value::from(16_u64);
    }
    std::fs::write(&baseline_path, b"coordinated rewrite")?;
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&coordinated)?)?;
    let coordinated_rejected = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root_arg,
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "67fc764ba37d77bd",
        "--json",
    ]);
    assert!(!coordinated_rejected.status.success());
    std::fs::write(&baseline_path, baseline_bytes)?;
    std::fs::write(&manifest_path, &manifest_bytes)?;
    // The after phase's stdout is exactly one JSON document — the versioned
    // `repair_after_result` envelope holding the verify 0.3 document under
    // `verify` and the status report under `agent_status` — so one
    // JSON.parse consumes it. The embedded status is `complete` only when the
    // receipt it issued is `advisory` (improved grip at the current HEAD); an
    // `invalid` or `incomplete` receipt keeps it at `warning` (F15-4).
    let expected_status = if receipt["status"] == "advisory" {
        "complete"
    } else {
        "warning"
    };
    let after_document: serde_json::Value = serde_json::from_slice(&after.stdout)?;
    assert_eq!(
        after_document["schema_version"].as_str(),
        Some("0.1"),
        "the after-phase success stdout is the repair_after_result envelope:\n{}",
        String::from_utf8_lossy(&after.stdout)
    );
    assert_eq!(
        after_document["kind"].as_str(),
        Some("repair_after_result"),
        "the after-phase success stdout names its envelope kind:\n{}",
        String::from_utf8_lossy(&after.stdout)
    );
    assert_eq!(
        after_document["verify"]["schema_version"].as_str(),
        Some("0.3"),
        "the envelope carries the verify document intact under `verify`:\n{}",
        String::from_utf8_lossy(&after.stdout)
    );
    assert_eq!(
        after_document["agent_status"]["status"].as_str(),
        Some(expected_status),
        "after phase status must follow the receipt ({}):\n{}",
        receipt["status"],
        String::from_utf8_lossy(&after.stdout)
    );
    assert_eq!(
        after_document["verify"]["status"].as_str(),
        Some("advisory"),
        "the verify outcome keeps its own top-level status under `verify`:\n{}",
        String::from_utf8_lossy(&after.stdout)
    );
    assert!(String::from_utf8_lossy(&after.stderr).contains("after phase complete"));

    std::fs::remove_dir_all(root)?;
    Ok(())
}

const BOUNDARY_GAP_SEAM_ID: &str = "67fc764ba37d77bd";

const CARGO_BUILD_OUTPUT_FILES: [&str; 3] = [
    "target/debug/.fingerprint/boundary_gap_fixture-1/invoked.timestamp",
    "target/debug/deps/pricing-1.d",
    "target/debug/incremental/pricing-1/s-1/0.o",
];

fn write_repo_file(
    root: &Path,
    relative: &str,
    contents: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

/// Stand-in for what `cargo test` writes under Cargo's build directory.
fn write_cargo_build_output(
    root: &Path,
    generation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for relative in CARGO_BUILD_OUTPUT_FILES {
        write_repo_file(root, relative, generation)?;
    }
    Ok(())
}

/// A committed boundary-gap crate that ignores Cargo's build output (and a
/// `notes.log`), with the state a newcomer has after building once: build
/// output under `target/` and an untracked, generated `Cargo.lock`.
fn built_repair_fixture(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    init_producer_fixture_repo(&root)?;
    std::fs::write(root.join(".gitignore"), "/target\nnotes.log\n")?;
    run_git(&root, &["add", ".gitignore"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "ignore build output",
        ],
    )?;
    assert!(commit.status.success(), "ignore commit failed: {commit:?}");
    write_cargo_build_output(&root, "first build")?;
    std::fs::write(
        root.join("Cargo.lock"),
        "# generated by the first build\nversion = 4\n",
    )?;
    Ok(root)
}

fn run_repair_phase(root: &Path, selector: &[&str], phase: &str) -> Result<Output, std::io::Error> {
    let root_arg = root.display().to_string();
    let mut args = vec!["agent", "repair", "--json", "--root", root_arg.as_str()];
    args.extend_from_slice(selector);
    args.extend(["--phase", phase]);
    run_command(env!("CARGO_BIN_EXE_ripr"), None, &args)
}

fn sole_repair_attempt(
    root: &Path,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let mut attempts = std::fs::read_dir(root.join("target/ripr/repair-attempts"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    assert_eq!(
        attempts.len(),
        1,
        "expected one repair attempt: {attempts:?}"
    );
    let path = attempts.pop().ok_or("missing repair attempt")?;
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("repair attempt directory name is not UTF-8")?
        .to_string();
    let manifest = serde_json::from_str(&std::fs::read_to_string(path.join("attempt.json"))?)?;
    Ok((id, manifest))
}

/// The seam's focused boundary test, added between the phases.
fn add_boundary_test(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join("tests/pricing.rs");
    let mut tests = std::fs::read_to_string(&path)?;
    tests.push_str(
        "\n#[test]\nfn equality_boundary_discounts() {\n    assert_eq!(discounted_total(100, 100), 90);\n}\n",
    );
    std::fs::write(path, tests)?;
    Ok(())
}

struct RepairBuildIgnoreFixture(PathBuf);

impl RepairBuildIgnoreFixture {
    fn new(root: PathBuf) -> std::io::Result<Self> {
        // Arm recursive cleanup only after exclusively claiming this tree.
        std::fs::create_dir(&root)?;
        Ok(Self(root))
    }
}

impl Drop for RepairBuildIgnoreFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn disable_ambient_git_excludes(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let global_excludes = root.join(".git/empty-global-excludes");
    std::fs::write(&global_excludes, "")?;
    run_git(
        root,
        &[
            "config",
            "core.excludesFile",
            &global_excludes.display().to_string(),
        ],
    )?;
    Ok(())
}

#[test]
fn agent_repair_before_requires_effective_cargo_build_directory_ignore()
-> Result<(), Box<dyn std::error::Error>> {
    for rule in [
        "",
        "target/debug/\n",
        "/target/*\n!/target/keep.log\n",
        "/target/**\n!/target/keep.log\n",
    ] {
        let fixture =
            RepairBuildIgnoreFixture::new(unique_temp_workspace("agent-repair-build-ignore"))?;
        let root = fixture.0.as_path();
        init_producer_fixture_repo(root)?;
        // Neither ambient global excludes nor a populated build tree may
        // supply the precondition this fresh adopter repository lacks.
        let excludes = root.join(".git/info/exclude");
        std::fs::write(&excludes, rule)?;
        disable_ambient_git_excludes(root)?;
        let before = run_repair_phase(root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
        if before.status.success() {
            return Err(format!(
                "missing whole-directory ignore must refuse BEFORE the edit, rule {rule:?}: {before:?}"
            ).into());
        }
        let stderr = String::from_utf8_lossy(&before.stderr);
        for required in [
            "target/",
            "Git-ignored",
            "/target/",
            "no repair attempt was started",
        ] {
            if !stderr.contains(required) {
                return Err(
                    format!("missing actionable precondition {required:?}: {stderr}").into(),
                );
            }
        }
        if !before.stdout.is_empty() || stderr.contains("before phase complete") {
            return Err(format!(
                "refused before phase must not publish a packet or completion: {before:?}"
            )
            .into());
        }
        for artifact in [
            "target/ripr/workflow/workflow.json",
            "target/ripr/workflow/agent-packet.json",
            "target/ripr/workflow/before.repo-exposure.json",
            "target/ripr/reports/agent-receipt.json",
        ] {
            if root.join(artifact).exists() {
                return Err(format!("precondition refused after publishing {artifact}").into());
            }
        }
        let attempts = root.join("target/ripr/repair-attempts");
        let published = if attempts.exists() {
            std::fs::read_dir(attempts)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .any(|entry| entry.path().join("attempt.json").exists())
        } else {
            false
        };
        if published {
            return Err("precondition refusal published a durable repair attempt".into());
        }
    }
    Ok(())
}

#[test]
fn agent_repair_accepts_empty_effectively_ignored_build_tree_and_actual_cargo_test()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = RepairBuildIgnoreFixture::new(unique_temp_workspace(
        "agent-repair-effective-build-ignore",
    ))?;
    let root = fixture.0.as_path();
    init_producer_fixture_repo(root)?;
    // The harness temp root lives under this repository's target directory;
    // make the dependency-free adopter crate its own Cargo workspace.
    let manifest_path = root.join("Cargo.toml");
    let mut manifest = std::fs::read_to_string(&manifest_path)?;
    manifest.push_str("\n[workspace]\n");
    std::fs::write(manifest_path, manifest)?;
    run_git(root, &["add", "Cargo.toml"])?;
    commit_repair_fixture(root, &["-qm", "standalone adopter workspace"])?;
    if root.join(".gitignore").exists() || root.join("target").exists() {
        return Err(
            "fixture must rely on effective excludes, with no pre-existing Cargo build".into(),
        );
    }
    let before = run_repair_phase(root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    if !before.status.success() {
        return Err(format!("effective directory ignore must admit before: {before:?}").into());
    }
    let (attempt_id, _) = sole_repair_attempt(root)?;
    add_boundary_test(root)?;
    // Explicitly anchor the child build here even when the parent Cargo
    // invocation shares a CARGO_TARGET_DIR with another verification lane.
    let target = root.join("target").display().to_string();
    let test = run_command_with_env(
        "cargo",
        root,
        &[
            "test",
            "--offline",
            "equality_boundary_discounts",
            "--",
            "--exact",
        ],
        &[("CARGO_TARGET_DIR", &target)],
    )?;
    if !test.status.success()
        || !String::from_utf8_lossy(&test.stdout)
            .contains("test equality_boundary_discounts ... ok")
        || !root.join("target/.rustc_info.json").exists()
    {
        return Err(format!(
            "focused Cargo test must run and write actual build output here: {test:?}"
        )
        .into());
    }
    let after = run_repair_phase(root, &["--attempt", &attempt_id], "after")?;
    if !after.status.success() {
        return Err(
            format!("actual Cargo build must not terminally fail repair: {after:?}").into(),
        );
    }
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?;
    if receipt
        .pointer("/repair_attempt/edit_cage_verdict/status")
        .and_then(serde_json::Value::as_str)
        != Some("compliant")
    {
        return Err(
            format!("actual build repair must produce its compliant receipt: {receipt}").into(),
        );
    }
    Ok(())
}

#[test]
fn agent_repair_names_build_ignore_drift_without_redirect_blame()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture =
        RepairBuildIgnoreFixture::new(unique_temp_workspace("agent-repair-build-ignore-drift"))?;
    let root = fixture.0.as_path();
    init_producer_fixture_repo(root)?;
    disable_ambient_git_excludes(root)?;
    let before = run_repair_phase(root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    if !before.status.success() {
        return Err(format!("effective ignore before fixture failed: {before:?}").into());
    }
    let (attempt_id, _) = sole_repair_attempt(root)?;
    // An older prepared attempt, or a changed local exclude rule, can still
    // reach the after-phase refusal. Its cause must not become redirect advice.
    std::fs::write(root.join(".git/info/exclude"), "")?;
    add_boundary_test(root)?;
    write_cargo_build_output(root, "build after local ignore drift")?;
    let after = run_repair_phase(root, &["--attempt", &attempt_id], "after")?;
    let stderr = String::from_utf8_lossy(&after.stderr);
    if after.status.success()
        || !stderr.contains("declared build directory `target/`")
        || !stderr.contains("effective Git ignore rules")
        || stderr.contains("If it is a file you redirected")
        || stderr.contains("If one is a file you redirected")
    {
        return Err(
            format!("build paths must stay refused with truthful recovery: {after:?}").into(),
        );
    }
    let (_, manifest) = sole_repair_attempt(root)?;
    if manifest.get("state").and_then(serde_json::Value::as_str) != Some("failed") {
        return Err(format!("ignore drift must not loosen the cage: {manifest}").into());
    }
    Ok(())
}

/// Without `--json`, each phase prints a short summary whose file paths
/// resolve against `--root`, so a caller running from another directory can
/// open them.
#[test]
fn agent_repair_default_stdout_is_a_summary_naming_root_resolved_files()
-> Result<(), Box<dyn std::error::Error>> {
    let root = built_repair_fixture("agent-repair-default-stdout")?;
    let elsewhere = std::env::temp_dir();
    let root_arg = root.display().to_string();
    let bin = env!("CARGO_BIN_EXE_ripr");

    let before = run_command(
        bin,
        Some(&elsewhere),
        &[
            "agent",
            "repair",
            "--root",
            &root_arg,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--phase",
            "before",
        ],
    )?;
    assert_success(&before);
    let stdout = String::from_utf8_lossy(&before.stdout);
    assert!(!stdout.trim_start().starts_with('{'), "{stdout}");
    let packet = root.join("target/ripr/workflow/agent-packet.json");
    assert!(
        stdout.contains(&packet.display().to_string()),
        "before summary must name the root-resolved packet:\n{stdout}"
    );
    assert!(stdout.contains("Next, after the test edit: "), "{stdout}");
    let (attempt_id, _) = sole_repair_attempt(&root)?;

    add_boundary_test(&root)?;
    let after = run_command(
        bin,
        Some(&elsewhere),
        &[
            "agent",
            "repair",
            "--root",
            &root_arg,
            "--attempt",
            &attempt_id,
            "--phase",
            "after",
        ],
    )?;
    assert_success(&after);
    let stdout = String::from_utf8_lossy(&after.stdout);
    assert!(!stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "Result for seam `{BOUNDARY_GAP_SEAM_ID}`: weak -> exposed (weakly_gripped -> strongly_gripped, improved)"
        )),
        "{stdout}"
    );
    for artifact in [
        "target/ripr/reports/agent-receipt.json",
        "target/ripr/workflow/agent-verify.json",
    ] {
        assert!(
            stdout.contains(&root.join(artifact).display().to_string()),
            "after summary must name the root-resolved {artifact}:\n{stdout}"
        );
    }

    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_repair_admits_cargo_build_output_and_unchanged_untracked_lockfile()
-> Result<(), Box<dyn std::error::Error>> {
    let root = built_repair_fixture("agent-repair-build-output")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, _) = sole_repair_attempt(&root)?;

    // The documented loop: add the focused test, then run the project tests,
    // which rewrites and adds Cargo build output under the ignored `target/`.
    add_boundary_test(&root)?;
    write_cargo_build_output(&root, "rebuilt by cargo test")?;
    write_repo_file(&root, "target/debug/deps/pricing-2", "new test binary")?;

    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_success(&after);
    let after_stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        after_stderr.contains(&format!(
            "result for seam `{BOUNDARY_GAP_SEAM_ID}`: weak -> exposed (weakly_gripped -> strongly_gripped, improved)"
        )),
        "after phase must report the movement:\n{after_stderr}"
    );
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "ready_to_finish");
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?;
    let verdict = &receipt["repair_attempt"]["edit_cage_verdict"];
    assert_eq!(verdict["status"], "compliant", "{verdict}");
    let changed = verdict["changed_paths"]
        .as_array()
        .ok_or("receipt verdict has no changed_paths")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    assert!(changed.contains(&"tests/pricing.rs"), "{changed:?}");
    assert!(
        changed
            .iter()
            .all(|path| *path == "tests/pricing.rs" || path.starts_with("target/ripr/")),
        "only the test edit and command-owned target/ripr writes may be observed: {changed:?}"
    );

    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_repair_still_refuses_writes_outside_build_output_and_test_surface()
-> Result<(), Box<dyn std::error::Error>> {
    let scenarios: [(&str, &str, bool); 3] = [
        // An ignored path that is not Cargo build output.
        ("notes.log", "ignored notes\n", false),
        // A tracked production file.
        ("src/lib.rs", "", true),
        // A pre-existing untracked file whose bytes change during the attempt.
        ("Scratch.md", "rewritten during the attempt\n", false),
    ];
    for (offending, contents, append) in scenarios {
        let root = built_repair_fixture("agent-repair-cage-boundary")?;
        write_repo_file(&root, "Scratch.md", "pre-existing scratch notes\n")?;
        let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
        assert_success(&before);
        let (attempt_id, _) = sole_repair_attempt(&root)?;

        add_boundary_test(&root)?;
        write_cargo_build_output(&root, "rebuilt by cargo test")?;
        if append {
            let path = root.join(offending);
            let mut text = std::fs::read_to_string(&path)?;
            text.push_str("// out-of-cage edit\n");
            std::fs::write(path, text)?;
        } else {
            write_repo_file(&root, offending, contents)?;
        }

        let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
        assert_failure(&after);
        let (_, manifest) = sole_repair_attempt(&root)?;
        assert_eq!(manifest["state"], "failed", "{offending}: {manifest}");
        let violations = manifest["after"]["verdict"]["violations"]
            .as_array()
            .ok_or("failed attempt has no violations")?;
        assert!(
            violations
                .iter()
                .any(|violation| violation["path"] == offending),
            "{offending} must be a violation: {violations:?}"
        );
        assert!(
            violations.iter().all(|violation| !violation["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("target/debug/"))),
            "Cargo build output must not be a violation: {violations:?}"
        );
        let stderr = String::from_utf8_lossy(&after.stderr);
        assert!(
            stderr.contains(&format!("ripr:   {offending} (")),
            "the failure must name the refused path:\n{stderr}"
        );
        assert!(
            stderr.contains(&format!(
                "--seam-id {BOUNDARY_GAP_SEAM_ID} --phase before` while the gap still exists"
            )),
            "the failure must name the recovery route:\n{stderr}"
        );
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

/// Runs one repair phase as a shell `ripr agent repair ... > stdout 2> stderr`
/// does: both files are created (empty) before ripr starts.
fn run_repair_phase_redirected(
    root: &Path,
    selector: &[&str],
    phase: &str,
    stdout: &Path,
    stderr: &Path,
) -> Result<Output, std::io::Error> {
    let root_arg = root.display().to_string();
    let mut args = vec!["agent", "repair", "--json", "--root", root_arg.as_str()];
    args.extend_from_slice(selector);
    args.extend(["--phase", phase]);
    spawn_command(
        env!("CARGO_BIN_EXE_ripr"),
        None,
        &args,
        &[],
        Some((stdout, stderr)),
        None,
    )
}

/// #4216 row 4: the natural `--phase before > packet.json 2> before.err`
/// shape inside the checkout. The shell creates both files before ripr
/// starts, and the before phase writes its stdout document (the packet
/// envelope with the `repair_attempt` continuation, #4329) and keeps writing
/// stderr after the baseline is captured, so the cage cannot tell either
/// file from an authored edit and the attempt stays `Violated`. The before
/// phase must say so up front, and the refusal must name the redirect files
/// with how to avoid them. Redirecting under target/ripr/, as the guidance
/// says, keeps the attempt compliant, and a real out-of-surface source edit
/// made alongside that redirect is still refused without the redirect hint.
#[test]
fn agent_repair_names_output_redirected_into_the_checkout() -> Result<(), Box<dyn std::error::Error>>
{
    const GUIDANCE: &str = "keep this command's output out of the checkout";
    const UNTRACKED_HINT: &str = "not tracked by Git when the before phase ran.";
    const REDIRECT_HINT: &str = "a file you redirected ripr output into";

    // Natural shape: redirect files in the repository root.
    let root = built_repair_fixture("agent-repair-redirect-in-repo")?;
    let packet = root.join("packet.json");
    let before_err = root.join("before.err");
    let before = run_repair_phase_redirected(
        &root,
        &["--seam-id", BOUNDARY_GAP_SEAM_ID],
        "before",
        &packet,
        &before_err,
    )?;
    assert!(before.status.success(), "before phase failed: {before:?}");
    let before_stderr = std::fs::read_to_string(&before_err)?;
    assert!(
        before_stderr.contains(GUIDANCE),
        "the before phase must warn against redirecting into the checkout:\n{before_stderr}"
    );
    assert!(
        std::fs::read_to_string(&packet)?.contains(BOUNDARY_GAP_SEAM_ID),
        "precondition: the packet was redirected into the checkout"
    );
    let (attempt_id, _) = sole_repair_attempt(&root)?;
    add_boundary_test(&root)?;
    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_failure(&after);
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(
        manifest["after"]["verdict"]["status"], "violated",
        "{manifest}"
    );
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(stderr.contains("ripr:   before.err ("), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "`before.err`, `packet.json` were {UNTRACKED_HINT} If one is {REDIRECT_HINT}"
        )),
        "the refusal must name the redirect files and how to avoid it:\n{stderr}"
    );
    std::fs::remove_dir_all(root)?;

    // The guided shape, alone and with a real production edit alongside.
    for edit_source in [false, true] {
        let root = built_repair_fixture("agent-repair-redirect-guided")?;
        std::fs::create_dir_all(root.join("target/ripr"))?;
        let before = run_repair_phase_redirected(
            &root,
            &["--seam-id", BOUNDARY_GAP_SEAM_ID],
            "before",
            &root.join("target/ripr/packet.json"),
            &root.join("target/ripr/before.err"),
        )?;
        assert!(before.status.success(), "before phase failed: {before:?}");
        let (attempt_id, _) = sole_repair_attempt(&root)?;
        add_boundary_test(&root)?;
        if edit_source {
            let path = root.join("src/lib.rs");
            let mut text = std::fs::read_to_string(&path)?;
            text.push_str("// out-of-cage edit\n");
            std::fs::write(path, text)?;
        }
        let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
        let (_, manifest) = sole_repair_attempt(&root)?;
        let stderr = String::from_utf8_lossy(&after.stderr);
        if edit_source {
            assert_failure(&after);
            assert_eq!(
                manifest["after"]["verdict"]["status"], "violated",
                "{manifest}"
            );
            // #4330 packets state `forbidden_files` explicitly (the production
            // anchor), and the cage classifies an edit to that file against
            // the stated rule — ForbiddenPath, not the generic
            // OutsideAllowedSurface this assertion carried before the packet
            // carried the forbidden list.
            assert!(
                stderr.contains("ripr:   src/lib.rs (ForbiddenPath)"),
                "{stderr}"
            );
            assert!(
                !stderr.contains(UNTRACKED_HINT),
                "a tracked source edit is not a redirect file:\n{stderr}"
            );
        } else {
            assert_success(&after);
            assert_eq!(
                manifest["after"]["verdict"]["status"], "compliant",
                "{manifest}"
            );
        }
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

/// Commit in a repair fixture with the suite's fixed identity; `args`
/// follows `git commit` (for example `["-qam", "message"]`).
fn commit_repair_fixture(root: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = vec![
        "-c",
        "user.name=RIPR test",
        "-c",
        "user.email=ripr@example.invalid",
        "commit",
    ];
    command.extend_from_slice(args);
    let commit = run_command("git", Some(root), &command)?;
    assert!(commit.status.success(), "fixture commit failed: {commit:?}");
    Ok(())
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = run_command("git", Some(root), args)?;
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// A committed boundary-gap library crate that ignores Cargo's build output
/// and has never been built: no `Cargo.lock` exists, as in a fresh clone of
/// a library that does not commit one.
fn unbuilt_repair_fixture(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    init_producer_fixture_repo(&root)?;
    std::fs::write(root.join(".gitignore"), "/target\n")?;
    run_git(&root, &["add", ".gitignore"])?;
    commit_repair_fixture(&root, &["-qm", "ignore build output"])?;
    assert!(
        !root.join("Cargo.lock").exists(),
        "precondition: the fixture has no Cargo.lock"
    );
    assert_eq!(git_stdout(&root, &["ls-files", "--", "Cargo.lock"])?, "");
    Ok(root)
}

/// What the first `cargo test` of such a crate writes: build output under
/// the ignored `target/` and a newly generated, untracked `Cargo.lock`.
fn simulate_first_cargo_test(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    write_cargo_build_output(root, "first build by cargo test")?;
    std::fs::write(
        root.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"boundary_gap_fixture\"\nversion = \"0.1.0\"\n",
    )?;
    Ok(())
}

fn repair_receipt(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?)
}

/// `ripr agent receipt` as a developer reruns it from the repository root
/// after the after phase, spelling `--root` as the phases did.
fn rerun_repair_receipt(root: &Path, out: bool) -> Result<Output, std::io::Error> {
    let root_arg = root.display().to_string();
    let mut args = vec![
        "agent",
        "receipt",
        "--root",
        root_arg.as_str(),
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        BOUNDARY_GAP_SEAM_ID,
        "--json",
    ];
    if out {
        args.extend(["--out", "target/ripr/reports/agent-receipt.json"]);
    }
    run_command(env!("CARGO_BIN_EXE_ripr"), Some(root), &args)
}

const INCLUDE_RECEIPT_LINE: &str = "ripr: next: Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review.";

/// F15-1/F15-3: the printed loop on a library crate that does not commit
/// `Cargo.lock`. The first `cargo test` between the phases creates the
/// lockfile; the after phase must still compare the pair, and the receipt it
/// writes must be valid review evidence (`advisory`), with the review line
/// printed only for that valid receipt.
#[test]
fn agent_repair_admits_a_cargo_lock_first_generated_between_the_phases()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unbuilt_repair_fixture("agent-repair-new-lockfile")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, _) = sole_repair_attempt(&root)?;

    add_boundary_test(&root)?;
    simulate_first_cargo_test(&root)?;
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain", "--", "Cargo.lock"])?,
        "?? Cargo.lock",
        "precondition: the lockfile appeared untracked between the phases"
    );

    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_success(&after);
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        stderr.contains(&format!(
            "result for seam `{BOUNDARY_GAP_SEAM_ID}`: weak -> exposed (weakly_gripped -> strongly_gripped, improved)"
        )),
        "{stderr}"
    );
    assert!(stderr.contains(INCLUDE_RECEIPT_LINE), "{stderr}");
    assert!(!stderr.contains("receipt status is"), "{stderr}");

    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest}");
    let receipt = repair_receipt(&root)?;
    assert_eq!(receipt["status"], "advisory", "{receipt}");
    assert_eq!(receipt["analysis_outcome_status"], "complete", "{receipt}");
    assert_eq!(receipt["analysis_outcome_error"], serde_json::Value::Null);
    assert_eq!(receipt["provenance"]["movement"], "improved");
    // #4234: the loop never runs the focused test, and the receipt says so.
    assert_eq!(
        receipt["verification"]["status"], "verification_not_run",
        "{receipt}"
    );
    let verdict = &receipt["repair_attempt"]["edit_cage_verdict"];
    assert_eq!(verdict["status"], "compliant", "{verdict}");
    assert!(
        !verdict["changed_paths"]
            .as_array()
            .ok_or("receipt verdict has no changed_paths")?
            .iter()
            .any(|path| path == "Cargo.lock"),
        "an untracked, build-generated Cargo.lock is not an edit: {verdict}"
    );
    // The producer and the validator agree on the base the scope-less
    // analysis outcome used.
    let outcome: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/workflow/analysis-outcome.json"),
    )?)?;
    assert!(outcome["base"].is_string(), "{outcome}");
    assert_eq!(
        outcome["base"],
        outcome["analysis_outcome"]["outcome"]["identity"]["base_revision"]
    );

    // Rerunning the receipt command after the after phase: the receipt the
    // after phase wrote under target/ripr/reports is command-owned output
    // written after the finish bound its verdict, not an attempt edit.
    let bound_paths = verdict["changed_paths"].clone();
    assert!(
        !bound_paths
            .as_array()
            .ok_or("receipt verdict has no changed_paths")?
            .iter()
            .any(|path| path == "target/ripr/reports/agent-receipt.json"),
        "precondition: the report is written after the finish: {bound_paths}"
    );
    for out in [true, false, true] {
        let rerun = rerun_repair_receipt(&root, out)?;
        assert_success(&rerun);
        let rerun: serde_json::Value = if out {
            repair_receipt(&root)?
        } else {
            serde_json::from_slice(&rerun.stdout)?
        };
        assert_eq!(rerun["status"], "advisory", "{rerun}");
        assert_eq!(
            rerun["repair_attempt"]["edit_cage_verdict"]["changed_paths"], bound_paths,
            "{rerun}"
        );
    }
    // Discriminating negative: any other movement after the finish still
    // breaks the binding, and undoing it restores the receipt.
    std::fs::write(root.join("notes.txt"), "an edit after the after phase\n")?;
    let moved = rerun_repair_receipt(&root, true)?;
    assert_failure(&moved);
    assert!(
        String::from_utf8_lossy(&moved.stderr)
            .contains("repair attempt after verdict binding is tampered or stale"),
        "{}",
        String::from_utf8_lossy(&moved.stderr)
    );
    std::fs::remove_file(root.join("notes.txt"))?;
    assert_success(&rerun_repair_receipt(&root, true)?);

    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// #4445: a repo-scoped format still records `--base` as snapshot
/// provenance and names `--diff` as an input, so an unresolvable base or a
/// missing diff file exits 2 with empty stdout, like the diff-scoped
/// formats. A resolvable base keeps working.
#[test]
fn repo_scope_formats_reject_unresolvable_base_and_missing_diff()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("repo-scope-bad-base");
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scope\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn big(x: u32) -> bool {\n    x >= 10\n}\n",
    )?;
    std::fs::write(root.join(".gitignore"), "/target\n")?;
    run_git(&root, &["init", "-q"])?;
    run_git(&root, &["add", "."])?;
    commit_repair_fixture(&root, &["-qm", "scope"])?;
    let root_arg = root.display().to_string();
    let missing_diff = root.join("no-such.diff").display().to_string();

    for format in [
        "repo-exposure-json",
        "repo-sarif",
        "agent-seam-packets-json",
    ] {
        for bad in [
            ["--base", "no-such-ref-4445"],
            ["--diff", missing_diff.as_str()],
        ] {
            let output = run_ripr(&[
                "check", "--root", &root_arg, bad[0], bad[1], "--format", format,
            ]);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{format} {bad:?}: stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stdout.is_empty(),
                "{format} {bad:?} wrote stdout on a failed invocation"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(bad[1]),
                "{format} {bad:?}: stderr does not name the rejected input: {stderr}"
            );
        }
    }

    let good = run_ripr(&[
        "check",
        "--root",
        &root_arg,
        "--base",
        "HEAD",
        "--format",
        "repo-exposure-json",
    ]);
    assert_success(&good);
    let good: serde_json::Value = serde_json::from_slice(&good.stdout)?;
    assert_eq!(good["artifact"]["analysis"]["base_revision"], "HEAD");
    Ok(())
}

/// F15-12: a seam whose only related test lives inline in another crate has
/// no test file the repair can edit. The before phase refuses before it
/// writes any workflow artifact and never prints a completion line first, so
/// neither the phase nor a later `agent status` reads as a started repair.
#[test]
fn agent_repair_before_refuses_a_seam_without_a_test_file_before_writing_anything()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-repair-no-test-target");
    std::fs::create_dir_all(root.join("crates/rates/src"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/rates\"]\nresolver = \"2\"\n",
    )?;
    std::fs::write(
        root.join("crates/rates/Cargo.toml"),
        "[package]\nname = \"rates\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(
        root.join("crates/rates/src/lib.rs"),
        "/// Tax in basis points for the given region code.\npub fn tax_bps(region: &str) -> u32 {\n    match region {\n        \"EU\" => 2000,\n        _ => 0,\n    }\n}\n\n/// Orders at or above this many items ship free.\npub fn ships_free(items: u32) -> bool {\n    items >= 10\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn eu_tax() {\n        assert_eq!(tax_bps(\"EU\"), 2000);\n    }\n}\n",
    )?;
    std::fs::write(root.join(".gitignore"), "/target\n")?;
    run_git(&root, &["init", "-q"])?;
    run_git(&root, &["add", "."])?;
    commit_repair_fixture(&root, &["-qm", "rates"])?;

    let root_arg = root.display().to_string();
    let exposure = run_ripr(&[
        "check",
        "--root",
        &root_arg,
        "--mode",
        "draft",
        "--format",
        "repo-exposure-json",
    ]);
    assert_success(&exposure);
    let exposure: serde_json::Value = serde_json::from_slice(&exposure.stdout)?;
    let seam = exposure["seams"]
        .as_array()
        .ok_or("repo exposure has no seams array")?
        .iter()
        .find(|seam| seam["owner"] == "crates/rates/src/lib.rs::ships_free")
        .ok_or("precondition: the ships_free boundary seam exists")?;
    let seam_id = seam["seam_id"]
        .as_str()
        .ok_or("seam has no id")?
        .to_string();
    // Fixture construction: the seam's only related test is the inline one.
    assert_eq!(seam["related_tests"][0]["file"], "crates/rates/src/lib.rs");

    let before = run_repair_phase(&root, &["--seam-id", &seam_id], "before")?;
    assert_failure(&before);
    let stderr = String::from_utf8_lossy(&before.stderr);
    assert!(
        stderr.contains(&format!(
            "seam `{seam_id}` has no test file ripr can route a repair to, so no repair attempt was started."
        )),
        "the refusal must say why in plain words:\n{stderr}"
    );
    // #4332: the refusal states the observable packet field, not only the
    // producer-jargon cause, so an agent can check the packet itself.
    assert!(
        stderr.contains("recommended_test.file: \"not_applicable\""),
        "the refusal must name the observable packet field:\n{stderr}"
    );
    assert!(
        !stderr.contains("before phase complete"),
        "a refused before phase must not claim completion:\n{stderr}"
    );
    for artifact in [
        "target/ripr/workflow/before.repo-exposure.json",
        "target/ripr/workflow/agent-packet.json",
        "target/ripr/workflow/workflow.json",
    ] {
        assert!(
            !root.join(artifact).exists(),
            "refused before writing {artifact}"
        );
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// F15-1 boundary: a Git-tracked `Cargo.lock` is an analysis input. Changing
/// it between the phases, or starting to track a generated one, is refused
/// with the named cause and a rerun route, and the attempt stays awaiting
/// the edit so the route works. A manifest-only change names the manifest
/// and no lockfile untrack route.
#[test]
fn agent_repair_refuses_a_tracked_cargo_lock_change_between_the_phases()
-> Result<(), Box<dyn std::error::Error>> {
    for scenario in [
        "tracked lockfile modified",
        "generated lockfile staged",
        "manifest modified",
    ] {
        let root = unbuilt_repair_fixture("agent-repair-tracked-lockfile")?;
        if scenario == "tracked lockfile modified" {
            simulate_first_cargo_test(&root)?;
            run_git(&root, &["add", "Cargo.lock"])?;
            commit_repair_fixture(&root, &["-qm", "commit the lockfile"])?;
            assert_eq!(
                git_stdout(&root, &["ls-files", "--", "Cargo.lock"])?,
                "Cargo.lock",
                "precondition: the lockfile is tracked"
            );
        }
        let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
        assert_success(&before);
        let (attempt_id, _) = sole_repair_attempt(&root)?;

        add_boundary_test(&root)?;
        if scenario == "tracked lockfile modified" {
            let mut lockfile = std::fs::read_to_string(root.join("Cargo.lock"))?;
            lockfile
                .push_str("\n[[package]]\nname = \"updated_dependency\"\nversion = \"1.0.1\"\n");
            std::fs::write(root.join("Cargo.lock"), lockfile)?;
        } else if scenario == "manifest modified" {
            let mut manifest = std::fs::read_to_string(root.join("Cargo.toml"))?;
            manifest.push_str("\n# edited between the repair phases\n");
            std::fs::write(root.join("Cargo.toml"), manifest)?;
        } else {
            simulate_first_cargo_test(&root)?;
            run_git(&root, &["add", "Cargo.lock"])?;
        }
        let changed_input = if scenario == "manifest modified" {
            "Cargo.toml"
        } else {
            "Cargo.lock"
        };

        let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
        assert_failure(&after);
        let stderr = String::from_utf8_lossy(&after.stderr);
        assert!(
            stderr.contains(&format!(
                "ripr: analysis inputs changed after the before phase: {changed_input}."
            )),
            "{scenario}: the refusal must name the changed input:\n{stderr}"
        );
        // The untrack route is offered only when a lockfile moved.
        assert_eq!(
            stderr.contains("git rm --cached Cargo.lock"),
            changed_input == "Cargo.lock",
            "{scenario}: untrack advice must follow the changed input:\n{stderr}"
        );
        assert!(
            stderr.contains(
                "agent verify artifacts are incomparable: analysis input identities differ"
            ),
            "{scenario}:\n{stderr}"
        );
        assert!(
            stderr.contains("then rerun `ripr agent repair --root "),
            "{scenario}: the refusal must give the rerun route:\n{stderr}"
        );
        assert!(
            stderr.contains(&format!("--attempt {attempt_id} --phase after`")),
            "{scenario}:\n{stderr}"
        );
        assert!(
            !stderr.contains(INCLUDE_RECEIPT_LINE),
            "{scenario}:\n{stderr}"
        );
        let (_, manifest) = sole_repair_attempt(&root)?;
        assert_eq!(manifest["state"], "awaiting_edit", "{scenario}: {manifest}");

        // The named route recovers the same attempt.
        if scenario == "tracked lockfile modified" || scenario == "manifest modified" {
            let head = git_stdout(&root, &["rev-parse", "HEAD"])?;
            run_git(&root, &["checkout", &head, "--", changed_input])?;
        } else {
            run_git(&root, &["rm", "--cached", "-q", "Cargo.lock"])?;
        }
        let rerun = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
        assert_success(&rerun);
        assert_eq!(repair_receipt(&root)?["status"], "advisory", "{scenario}");
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

/// F15-6: committing the focused test before the after phase is accepted
/// when HEAD only moved forward and every committed path is inside the edit
/// cage; a committed out-of-cage change is refused by the same cage, and a
/// rewritten history is refused before the attempt finishes, with a recovery
/// that restores the prepared head for the same attempt.
#[test]
fn agent_repair_admits_a_focused_test_committed_between_the_phases()
-> Result<(), Box<dyn std::error::Error>> {
    // Accepted: one commit holding the focused test.
    let root = unbuilt_repair_fixture("agent-repair-committed-test")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, prepared) = sole_repair_attempt(&root)?;
    let before_head = prepared["repository_head"]
        .as_str()
        .ok_or("attempt has no repository head")?
        .to_string();
    add_boundary_test(&root)?;
    simulate_first_cargo_test(&root)?;
    commit_repair_fixture(&root, &["-qam", "test: pin the discount boundary"])?;
    let after_head = git_stdout(&root, &["rev-parse", "HEAD"])?;
    assert_ne!(after_head, before_head, "precondition: HEAD moved");
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain", "--", "tests"])?,
        ""
    );

    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_success(&after);
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(stderr.contains(", improved)"), "{stderr}");
    assert!(stderr.contains(INCLUDE_RECEIPT_LINE), "{stderr}");
    assert!(!stderr.contains("is stale"), "{stderr}");
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest}");
    let receipt = repair_receipt(&root)?;
    assert_eq!(receipt["status"], "advisory", "{receipt}");
    let binding = &receipt["repair_attempt"];
    assert_eq!(binding["current"], true);
    assert_eq!(binding["before_head"], before_head.as_str());
    assert_eq!(binding["after_head"], after_head.as_str());
    assert_eq!(binding["edit_cage_verdict"]["status"], "compliant");
    assert!(
        binding["edit_cage_verdict"]["changed_paths"]
            .as_array()
            .ok_or("no changed_paths")?
            .iter()
            .any(|path| path == "tests/pricing.rs"),
        "the committed test is the attempt's edit: {binding}"
    );
    std::fs::remove_dir_all(root)?;

    // Refused: a production change committed alongside the test.
    let root = unbuilt_repair_fixture("agent-repair-committed-production")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, _) = sole_repair_attempt(&root)?;
    add_boundary_test(&root)?;
    let mut production = std::fs::read_to_string(root.join("src/lib.rs"))?;
    production.push_str("// committed out-of-cage edit\n");
    std::fs::write(root.join("src/lib.rs"), production)?;
    commit_repair_fixture(&root, &["-qam", "test and production"])?;
    // The worktree now matches HEAD: only the commit range carries the
    // production change.
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain", "--", "src"])?,
        ""
    );
    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_failure(&after);
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "failed", "{manifest}");
    let violations = manifest["after"]["verdict"]["violations"]
        .as_array()
        .ok_or("failed attempt has no violations")?;
    assert!(
        violations
            .iter()
            .any(|violation| violation["path"] == "src/lib.rs"),
        "{violations:?}"
    );
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(stderr.contains("ripr:   src/lib.rs ("), "{stderr}");
    assert!(
        stderr.contains("uncommit it first (for example `git reset --soft HEAD~1`"),
        "the recovery must fit a committed edit:\n{stderr}"
    );
    assert!(!stderr.contains(INCLUDE_RECEIPT_LINE), "{stderr}");
    std::fs::remove_dir_all(root)?;

    // Refused before finishing: the prepared commit was amended.
    let root = unbuilt_repair_fixture("agent-repair-amended-head")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, prepared) = sole_repair_attempt(&root)?;
    let before_head = prepared["repository_head"]
        .as_str()
        .ok_or("attempt has no repository head")?
        .to_string();
    add_boundary_test(&root)?;
    commit_repair_fixture(&root, &["-qa", "--amend", "--no-edit"])?;
    let amended = git_stdout(&root, &["rev-parse", "HEAD"])?;
    let ancestry = run_command(
        "git",
        Some(&root),
        &["merge-base", "--is-ancestor", &before_head, &amended],
    )?;
    assert!(
        !ancestry.status.success(),
        "precondition: history was rewritten"
    );
    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_failure(&after);
    assert_eq!(
        after.status.code(),
        Some(3),
        "a diverged HEAD is a typed refusal, not an operational failure"
    );
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(stderr.contains("does not descend from"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "`git reset --soft {before_head}` restores the prepared head"
        )),
        "{stderr}"
    );
    // Exit 3 carries exactly one typed refusal document on stdout.
    let document: serde_json::Value = serde_json::from_slice(&after.stdout)?;
    assert_eq!(document["kind"], "repair_after_refusal", "{document:#}");
    assert_eq!(document["schema_version"], "0.2", "{document:#}");
    assert_eq!(document["attempt_id"], attempt_id.as_str(), "{document:#}");
    assert!(
        document["error"]
            .as_str()
            .is_some_and(|error| error.contains("does not descend from")),
        "{document:#}"
    );
    assert!(
        json_strings(&document["narration"])
            .iter()
            .any(|line| line.contains(&format!(
                "`git reset --soft {before_head}` restores the prepared head"
            ))),
        "the refusal document carries the recovery: {document:#}"
    );
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "awaiting_edit", "{manifest}");
    // The printed recovery finishes the same attempt.
    run_git(&root, &["reset", "-q", "--soft", &before_head])?;
    let rerun = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_success(&rerun);
    assert_eq!(repair_receipt(&root)?["status"], "advisory");
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// An operational error after the after phase selected its attempt (here the
/// retained packet is missing) is an ordinary failure, not a typed refusal:
/// it maps to exit code 2 even though the attempt was already selected.
#[test]
fn agent_repair_operational_error_after_attempt_selection_stays_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unbuilt_repair_fixture("agent-repair-missing-retained-packet")?;
    let before = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before);
    let (attempt_id, manifest) = sole_repair_attempt(&root)?;
    let packet_rel = manifest["artifacts"]
        .as_array()
        .and_then(|artifacts| {
            artifacts
                .iter()
                .find(|artifact| artifact["role"] == "agent_packet")
        })
        .and_then(|artifact| artifact["path"].as_str())
        .ok_or("attempt manifest has no agent_packet artifact")?;
    std::fs::remove_file(root.join(packet_rel))?;

    let after = run_repair_phase(&root, &["--attempt", &attempt_id], "after")?;
    assert_failure(&after);
    assert_eq!(
        after.status.code(),
        Some(2),
        "an unreadable retained packet is operational, not a typed refusal"
    );
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        stderr.contains("canonicalize artifact") && stderr.contains("agent-packet.json"),
        "precondition: the retained packet is unreadable:\n{stderr}"
    );
    let (_, manifest) = sole_repair_attempt(&root)?;
    assert_eq!(manifest["state"], "awaiting_edit", "{manifest}");
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_packet_rejects_configured_off_seam() -> Result<(), Box<dyn std::error::Error>> {
    let (root, diff) = agent_brief_sample_workspace("agent-packet-config-off")?;
    let root_path = root.display().to_string();
    let diff = diff.display().to_string();
    let brief = run_ripr(&[
        "agent", "brief", "--root", &root_path, "--diff", &diff, "--json",
    ]);
    assert_success(&brief);
    let brief_stdout = String::from_utf8_lossy(&brief.stdout);
    let seam_id = json_string_field(&brief_stdout, "seam_id")
        .ok_or("expected brief output to include a seam_id")?;
    std::fs::write(
        root.join("ripr.toml"),
        "[severity.seams]\nweakly_gripped = \"off\"\n",
    )?;

    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_path,
        "--seam-id",
        &seam_id,
        "--json",
    ]);
    assert_failure(&packet);

    let stderr = String::from_utf8_lossy(&packet.stderr);
    let expected = std::fs::read_to_string(
        workspace_root()
            .join("fixtures/boundary_gap/expected/llm-work-loop/configured-off/stderr.txt"),
    )?;
    assert!(stderr.contains(expected.trim()));
    // #4332: the policy-omitted refusal names the listing route instead of
    // dead-ending.
    assert!(
        stderr.contains("Run `ripr pilot"),
        "the policy-omitted refusal must name the listing command:\n{stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// #4332: the seam-selection refusal states the working next action in both
/// directions. Found zero has no id to pass — it names the start command.
/// Found many names the ids, so an agent can pick one with `--attempt`
/// without rerunning to discover them.
#[test]
fn agent_repair_seam_selection_names_start_command_and_found_ids()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unbuilt_repair_fixture("agent-repair-selection-guidance")?;

    // Found zero: the dead-end advice used to be "pass --attempt <id>",
    // which cannot exist yet.
    let after = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "after")?;
    assert_failure(&after);
    let stderr = String::from_utf8_lossy(&after.stderr);
    let expected_start = format!(
        "no awaiting repair attempt exists for seam `{BOUNDARY_GAP_SEAM_ID}`; start one with `ripr agent repair --root {} --seam-id {BOUNDARY_GAP_SEAM_ID} --phase before`",
        // The refusal renders the root through the same display rule as the
        // sibling messages (stable separators).
        root.display().to_string().replace('\\', "/")
    );
    assert!(
        stderr.contains(&expected_start),
        "found-zero must name the start command:\n{stderr}"
    );
    assert!(
        !stderr.contains("pass --attempt"),
        "found-zero must not advise an id that cannot exist:\n{stderr}"
    );

    // Two awaiting attempts for one seam: the refusal names both ids.
    let before_a = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before_a);
    let before_b = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before_b);
    let mut attempts = std::fs::read_dir(root.join("target/ripr/repair-attempts"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| {
            Ok(path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or("attempt directory name is not UTF-8")?)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    attempts.sort();
    assert_eq!(attempts.len(), 2, "precondition: two prepared attempts");

    let after = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "after")?;
    assert_failure(&after);
    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        stderr.contains("expected exactly one awaiting repair attempt for seam")
            && stderr.contains("found 2"),
        "the ambiguity refusal must fire:\n{stderr}"
    );
    for attempt in &attempts {
        assert!(
            stderr.contains(attempt.as_str()),
            "found-many must name the id {attempt}:\n{stderr}"
        );
    }
    assert!(
        stderr.contains("pass --attempt <id> to select the prepared work exactly"),
        "found-many must name the selection flag:\n{stderr}"
    );

    // The named ids are directly actionable: attempt A finishes through its
    // own exact selector.
    let attempt_a = &attempts[0];
    add_boundary_test(&root)?;
    simulate_first_cargo_test(&root)?;
    let finish = run_repair_phase(&root, &["--attempt", attempt_a], "after")?;
    assert_success(&finish);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// The exact scenario from #4332: a failed attempt A and a fresh attempt B
/// for the same seam. The bare receipt command refuses by naming both ids;
/// `agent receipt --attempt <id>` selects exactly one and binds against that
/// attempt's retained packet; a receipt-ready refusal uses the serialized
/// vocabulary the manifest and verdict carry, not Debug spellings.
#[test]
fn agent_receipt_attempt_flag_selects_one_attempt_and_refusals_name_ids()
-> Result<(), Box<dyn std::error::Error>> {
    fn receipt_with_attempt(
        root: &Path,
        attempt: Option<&str>,
        verify_json: &str,
    ) -> Result<Output, std::io::Error> {
        let root_arg = root.display().to_string();
        let mut args = vec![
            "agent",
            "receipt",
            "--root",
            root_arg.as_str(),
            "--verify-json",
            verify_json,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
        ];
        if let Some(attempt) = attempt {
            args.extend(["--attempt", attempt]);
        }
        run_command(env!("CARGO_BIN_EXE_ripr"), Some(root), &args)
    }

    let root = unbuilt_repair_fixture("agent-receipt-attempt-selection")?;

    // Attempt A: prepared, then failed by a committed production-only edit.
    // The gap stays open (no test was added), so a fresh attempt for the same
    // seam can still be prepared — the exact ambiguity scenario.
    let before_a = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before_a);
    let (attempt_a, _) = sole_repair_attempt(&root)?;
    let mut production = std::fs::read_to_string(root.join("src/lib.rs"))?;
    production.push_str("// committed out-of-cage edit\n");
    std::fs::write(root.join("src/lib.rs"), production)?;
    commit_repair_fixture(&root, &["-qam", "production edit outside the cage"])?;
    let after_a = run_repair_phase(&root, &["--attempt", &attempt_a], "after")?;
    assert_failure(&after_a);
    let (_, manifest_a) = sole_repair_attempt(&root)?;
    assert_eq!(manifest_a["state"], "failed", "{manifest_a}");
    // The refused after phase still published its verify document; keep it
    // for the receipt reruns below.
    std::fs::rename(
        root.join("target/ripr/workflow/agent-verify.json"),
        root.join("target/ripr/workflow/agent-verify-a.json"),
    )?;

    // With exactly one attempt for the seam — the failed one — the bare
    // receipt refuses, and the refusal uses the serialized vocabulary the
    // manifest and verdict carry: `failed`/`violated`, not Debug spellings.
    let not_ready = receipt_with_attempt(&root, None, "target/ripr/workflow/agent-verify-a.json")?;
    assert_failure(&not_ready);
    let stderr = String::from_utf8_lossy(&not_ready.stderr);
    assert!(
        stderr.contains(&format!(
            "repair attempt {attempt_a} is not receipt-ready: state `failed`, current true, verdict `violated`"
        )),
        "the receipt-ready refusal must use the serialized vocabulary:\n{stderr}"
    );
    assert!(
        !stderr.contains("state Failed") && !stderr.contains("verdict Violated"),
        "the refusal must not leak Debug spellings:\n{stderr}"
    );

    // Attempt B: fresh, prepared after A's committed edit, finished cleanly.
    let before_b = run_repair_phase(&root, &["--seam-id", BOUNDARY_GAP_SEAM_ID], "before")?;
    assert_success(&before_b);
    let mut attempts = std::fs::read_dir(root.join("target/ripr/repair-attempts"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| {
            Ok(path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or("attempt directory name is not UTF-8")?)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    attempts.sort();
    assert_eq!(attempts.len(), 2, "precondition: two attempts");
    let attempt_b = attempts
        .iter()
        .find(|id| id.as_str() != attempt_a)
        .ok_or("second attempt id not found")?
        .clone();
    let (_, manifest_b) = {
        let path = root
            .join("target/ripr/repair-attempts")
            .join(&attempt_b)
            .join("attempt.json");
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        (attempt_b.clone(), manifest)
    };
    assert_eq!(manifest_b["state"], "awaiting_edit", "{manifest_b}");
    add_boundary_test(&root)?;
    simulate_first_cargo_test(&root)?;
    let after_b = run_repair_phase(&root, &["--attempt", &attempt_b], "after")?;
    assert_success(&after_b);

    // The bare receipt command is permanently ambiguous here; the refusal
    // names both ids instead of leaving the agent guessing.
    let ambiguous = receipt_with_attempt(&root, None, "target/ripr/workflow/agent-verify.json")?;
    assert_failure(&ambiguous);
    let stderr = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(
        stderr.contains(&format!(
            "receipt requires exactly one repair attempt for seam `{BOUNDARY_GAP_SEAM_ID}`"
        )),
        "the ambiguity refusal must fire:\n{stderr}"
    );
    assert!(
        stderr.contains(&attempt_a) && stderr.contains(&attempt_b),
        "the refusal must name both ids:\n{stderr}"
    );
    assert!(
        stderr.contains("pass --attempt <id>"),
        "the refusal must name the selection flag:\n{stderr}"
    );

    // Found zero on the receipt scan: no attempt exists for this seam at
    // all, so the advice names the start command instead of an id. The
    // packet-level cage check runs first, so the compat packet must name the
    // probed seam.
    std::fs::write(
        root.join("target/ripr/workflow/agent-packet.json"),
        r#"{"schema_version":"0.5","packets":[{"seam_id":"no-attempts-seam","recommended_test":{"file":"tests/pricing.rs","name":"some_test"}}]}"#,
    )?;
    let found_zero = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "no-attempts-seam",
        "--json",
    ]);
    assert_failure(&found_zero);
    let stderr = String::from_utf8_lossy(&found_zero.stderr);
    let expected_zero = format!(
        "no repair attempt exists for seam `no-attempts-seam`; start one with `ripr agent repair --root {} --seam-id no-attempts-seam --phase before`",
        // The refusal names the root the caller typed (stable separators).
        root.display().to_string().replace('\\', "/")
    );
    assert!(
        stderr.contains(&expected_zero),
        "receipt found-zero must name the start command:\n{stderr}"
    );

    // A mismatched seam/attempt pair refuses with the ownership diagnostic.
    let mismatched = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "some-other-seam",
        "--attempt",
        &attempt_a,
        "--json",
    ]);
    assert_failure(&mismatched);
    let stderr = String::from_utf8_lossy(&mismatched.stderr);
    assert!(
        stderr.contains(&format!("repair attempt {attempt_a} belongs to seam")),
        "a mismatched pair must refuse with ownership:\n{stderr}"
    );

    // The flag selects the finished attempt and binds to its retained packet.
    let selected = receipt_with_attempt(
        &root,
        Some(&attempt_b),
        "target/ripr/workflow/agent-verify.json",
    )?;
    assert_success(&selected);
    let receipt: serde_json::Value = serde_json::from_slice(&selected.stdout)?;
    assert_eq!(receipt["status"], "advisory", "{receipt}");
    assert_eq!(
        receipt["repair_attempt"]["attempt_id"],
        attempt_b.as_str(),
        "{receipt}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_compares_before_after_repo_exposure_json() -> Result<(), Box<dyn std::error::Error>>
{
    let root = unique_temp_workspace("agent-verify");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "weak"}],
      "observed_values": ["50"],
      "missing_discriminators": [{"value": "threshold equality", "reason": "not observed"}]
    }"#,
    )?;
    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the after artifact is bound to a descended revision.
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "strong"}],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }"#,
    )?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.3""#));
    assert!(stdout.contains(r#""improved": 1"#));
    assert!(stdout.contains(r#""change": "improved""#));
    assert!(stdout.contains(r#""seam_id": "seam-a""#));
    assert!(stdout.contains("missing discriminator no longer reported"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_tampered_committed_artifact() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-tampered");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    let mut tampered = std::fs::read_to_string(&before)?;
    tampered.push(' ');
    std::fs::write(&before, tampered)?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("content commitment mismatch"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_plausible_uncommitted_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-fabricated");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let fabricated =
        r#"{"schema_version":"0.3","scope":"repo","run_status":"complete","seams":[]}"#;
    std::fs::write(&before, fabricated)?;
    std::fs::write(&after, fabricated)?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("canonical repo-exposure artifact"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_incomparable_analysis_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-incomparable-input");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    let old_head = concrete_fixture_repository_head(&root)?;
    std::fs::write(root.join("marker.txt"), "fixture-moved\n")?;
    run_git(&root, &["add", "marker.txt"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "advance fixture",
        ],
    )?;
    assert!(commit.status.success(), "fixture commit failed: {commit:?}");
    let new_head = concrete_fixture_repository_head(&root)?;
    // Tamper the analysis input identity while keeping the after artifact
    // internally consistent (input identity, declared head, and snapshot
    // identity must agree under exact snapshot validation) so the pair
    // comparison — not single-artifact validation — rejects it.
    let altered = std::fs::read_to_string(&after)?
        .replace(
            &format!("\"head\": \"{old_head}\""),
            &format!("\"head\": \"{new_head}\""),
        )
        .replace(
            &format!("snapshot:input:v4:fnv1a64:00000000000000f1;revision:{old_head}"),
            &format!("snapshot:input:v4:fnv1a64:00000000000000f2;revision:{new_head}"),
        )
        .replace(
            "\"input_identity\": \"input:v4:fnv1a64:00000000000000f1\"",
            "\"input_identity\": \"input:v4:fnv1a64:00000000000000f2\"",
        );
    std::fs::write(&after, recommit_repo_exposure_json(altered))?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("analysis input identities differ"));
    assert_eq!(
        output.status.code(),
        Some(3),
        "drifted analysis inputs are a typed refusal on the standalone verify path too"
    );
    assert!(
        output.stdout.is_empty(),
        "a refused verify renders nothing to its stdout (the verify artifact)"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// #4331 review: an operational error that echoes a user path containing a
/// refusal phrase stays an operational failure (exit 2), not a refusal.
#[test]
fn agent_verify_missing_snapshot_named_like_a_refusal_is_operational()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-refusal-named-path");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    for missing in [
        "analysis input identities differ",
        "no repository movement between before and after artifacts",
    ] {
        let output = run_ripr(&[
            "agent",
            "verify",
            "--root",
            &root.display().to_string(),
            "--before",
            missing,
            "--after",
            missing,
            "--json",
        ]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{missing}: {stderr}");
        assert!(
            stderr.contains(missing),
            "the error names the path: {stderr}"
        );
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_comparison_metadata_drift() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "producer version",
            "\"version\": \"0.11.0\"",
            "\"version\": \"0.10.0\"",
            "producer versions differ",
        ),
        (
            "analysis mode",
            "\"mode\": \"draft\", \"base_revision\"",
            "\"mode\": \"release\", \"base_revision\"",
            "analysis modes differ",
        ),
        (
            "analysis profile",
            "\"profile\": \"draft\"",
            "\"profile\": \"release\"",
            "analysis profiles differ",
        ),
        (
            "base revision",
            "\"base_revision\": null",
            "\"base_revision\": \"base:other\"",
            "base revisions differ",
        ),
    ];
    for (label, from, to, expected) in cases {
        let root = unique_temp_workspace(&format!("agent-verify-metadata-{label}"));
        std::fs::create_dir_all(&root)?;
        init_git_fixture_repo(&root)?;
        let before = root.join("before.repo-exposure.json");
        let after = root.join("after.repo-exposure.json");
        let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
        write_bound_repo_exposure_fixture(&root, &before, seam)?;
        write_bound_repo_exposure_fixture(&root, &after, seam)?;
        let mut altered = std::fs::read_to_string(&after)?.replace(from, to);
        if label == "analysis mode" {
            altered = altered.replace("\"profile\": \"draft\"", "\"profile\": \"release\"");
        }
        std::fs::write(&after, recommit_repo_exposure_json(altered))?;
        let output = run_ripr(&[
            "agent",
            "verify",
            "--root",
            &root.display().to_string(),
            "--before",
            &before.display().to_string(),
            "--after",
            &after.display().to_string(),
            "--json",
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = if label == "analysis profile" {
            "invalid or unknown producer identity"
        } else {
            expected
        };
        assert!(stderr.contains(expected), "{label}: {stderr}");
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn agent_verify_accepts_historical_comparable_pair_with_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-historical");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    std::fs::write(root.join("marker.txt"), "fixture-updated\n")?;
    run_git(&root, &["add", "marker.txt"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "advance fixture",
        ],
    )?;
    if !commit.status.success() {
        return Err(format!("historical fixture commit failed: {commit:?}").into());
    }

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_success(&output);
    // Both artifacts stayed bound to the superseded revision, so the pair is
    // (Historical, Historical) (#3027).
    let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&verify, "/artifact_currentness")?,
        "historical_noncurrent"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_discloses_current_head_with_dirty_worktree()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-dirty");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    std::fs::write(root.join("marker.txt"), "unsaved-edit\n")?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_success(&output);
    // Both sides are bound to the live head with a dirty worktree, so the
    // pair token names both dirty sides (#3027); `dirty_worktree` stays
    // reserved for per-artifact evidence.
    let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&verify, "/artifact_currentness")?,
        "dirty_both"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_and_receipt_disclose_historical_before_current_after_pair()
-> Result<(), Box<dyn std::error::Error>> {
    // The expected clean before/after transaction (#3027): the repository
    // moved past the before artifact, so the pair is (Historical, Current) —
    // not a dirty worktree. The receipt path recomputes the same token and
    // its canonical byte comparison must accept the verify output.
    let root = unique_temp_workspace("agent-verify-historical-before");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&output);
    let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&verify, "/artifact_currentness")?,
        "historical_before_current_after"
    );

    let verify_path = root.join("agent-verify.json");
    std::fs::write(&verify_path, &output.stdout)?;
    let receipt = root.join("agent-receipt.json");
    let receipt_output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify_path.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
        "--out",
        &receipt.display().to_string(),
    ]);
    assert_success(&receipt_output);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_accepts_current_before_historical_after_descendant_pair()
-> Result<(), Box<dyn std::error::Error>> {
    // Reachability record for #3229: with the checked-out HEAD at the before
    // artifact's revision and the after artifact bound to a descendant
    // revision, the lineage gate passes (the before head is an ancestor of the
    // after head) and the movement gate passes (distinct revisions), so
    // `current_before_historical_after` is an accepted advisory verify outcome
    // — the descendant shape is reachable, not always lineage-rejected.
    let root = unique_temp_workspace("agent-verify-current-before-historical");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before_head = concrete_fixture_repository_head(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    advance_fixture_head(&root, "descendant movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    // Detach back at the before revision on a clean worktree: the before
    // artifact becomes current again while the after artifact stays bound to
    // the descendant head.
    run_git(&root, &["checkout", "--detach", &before_head])?;

    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&output);
    let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&verify, "/artifact_currentness")?,
        "current_before_historical_after"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_historical_before_token_ignores_declared_dirty_production()
-> Result<(), Box<dyn std::error::Error>> {
    // Non-certification record for #3229: a before artifact that declares a
    // dirty worktree in its envelope is classified by head mismatch alone once
    // the repository moves past it, so the pair token stays
    // `historical_before_current_after` rather than `dirty_before`. The pair
    // token is a movement/currentness shape, not a cleanliness certificate
    // for the historical side.
    let root = unique_temp_workspace("agent-verify-historical-declared-dirty");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    let before_content = std::fs::read_to_string(&before)?;
    let declared_dirty =
        before_content.replace("\"worktree\": \"clean\"", "\"worktree\": \"dirty\"");
    assert_ne!(
        declared_dirty, before_content,
        "fixture must declare a clean worktree before the flip"
    );
    std::fs::write(&before, recommit_repo_exposure_json(declared_dirty))?;
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&output);
    let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&verify, "/artifact_currentness")?,
        "historical_before_current_after"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_discloses_dirty_before_and_dirty_after_pairs()
-> Result<(), Box<dyn std::error::Error>> {
    // A declared dirty worktree on exactly one side names that side (#3027).
    // Both artifacts stay bound to the live head; a single-dirty pair is
    // admissible at the same revision because the movement gate only rejects
    // a fully current pair (#2922).
    for (label, dirty_side, expected) in [
        ("dirty-before", "before", "dirty_before"),
        ("dirty-after", "after", "dirty_after"),
    ] {
        let root = unique_temp_workspace(&format!("agent-verify-{label}"));
        std::fs::create_dir_all(&root)?;
        init_git_fixture_repo(&root)?;
        let before = root.join("before.repo-exposure.json");
        let after = root.join("after.repo-exposure.json");
        let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
        write_bound_repo_exposure_fixture(&root, &before, seam)?;
        write_bound_repo_exposure_fixture(&root, &after, seam)?;
        let dirty_path = if dirty_side == "before" {
            &before
        } else {
            &after
        };
        let declared_dirty = std::fs::read_to_string(dirty_path)?
            .replace("\"worktree\": \"clean\"", "\"worktree\": \"dirty\"");
        assert_ne!(
            declared_dirty,
            std::fs::read_to_string(dirty_path)?,
            "{label}: fixture must declare a clean worktree before the flip"
        );
        std::fs::write(dirty_path, recommit_repo_exposure_json(declared_dirty))?;

        let output = run_ripr(&[
            "agent",
            "verify",
            "--root",
            &root.display().to_string(),
            "--before",
            &before.display().to_string(),
            "--after",
            &after.display().to_string(),
            "--json",
        ]);
        assert_success(&output);
        let verify: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            json_pointer_str(&verify, "/artifact_currentness")?,
            expected,
            "{label}"
        );
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn agent_verify_rejects_unsupported_repo_exposure_schema() -> Result<(), Box<dyn std::error::Error>>
{
    let root = unique_temp_workspace("agent-verify-schema");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    let altered = std::fs::read_to_string(&before)?
        .replace("\"schema_version\": \"0.3\"", "\"schema_version\": \"9.0\"");
    std::fs::write(&before, altered)?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported repo-exposure schema"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_malformed_typed_seam() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-seam-schema");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &before, seam)?;
    write_bound_repo_exposure_fixture(&root, &after, seam)?;
    let altered =
        std::fs::read_to_string(&before)?.replace("\"line\":42", "\"line\":\"not-a-line\"");
    std::fs::write(&before, altered)?;

    let before_path = before.display().to_string();
    let after_path = after.display().to_string();
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before_path,
        "--after",
        &after_path,
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("canonical repo-exposure artifact"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_same_revision_pair_without_movement()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-no-movement");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    // Even an apparent grip improvement is unverifiable when both fully
    // current artifacts are bound to the same clean revision (#2922).
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no repository movement"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "a no-movement verify refusal is a typed refusal on the standalone verify path too"
    );
    assert!(
        output.stdout.is_empty(),
        "a refused verify renders nothing to its stdout (the verify artifact)"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_verify_rejects_reversed_revision_pair() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-verify-reversed");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let earlier = root.join("earlier.repo-exposure.json");
    let later = root.join("later.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &earlier, seam)?;
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(&root, &later, seam)?;

    // Both artifacts are individually valid and mutually comparable; only the
    // lineage check sees that the after input is not descended from the
    // before input (#2922).
    let output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &later.display().to_string(),
        "--after",
        &earlier.display().to_string(),
        "--json",
    ]);
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("revisions are reversed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_lineage_reversed_pair() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-reversed");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let earlier = root.join("earlier.repo-exposure.json");
    let later = root.join("later.repo-exposure.json");
    let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#;
    write_bound_repo_exposure_fixture(&root, &earlier, seam)?;
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(&root, &later, seam)?;
    // Name the later artifact as the receipt's before input: the receipt path
    // re-runs the pair lineage authority and must not become a bypass (#2922).
    let verify = root.join("fabricated-agent-verify.json");
    write_fabricated_agent_verify_json(&verify, &later, &earlier)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[incomparable_lineage]"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_writes_one_seam_handoff_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"fast\"\n")?;
    std::fs::create_dir_all(root.join("target/ripr/workflow"))?;
    let before = root.join("target/ripr/workflow/before.repo-exposure.json");
    let after = root.join("target/ripr/workflow/after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the after artifact is bound to a descended revision.
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify = root.join("agent-verify.json");
    let receipt = root.join("target/ripr/reports/agent-receipt.json");
    let verify_output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify_output);
    std::fs::write(&verify, verify_output.stdout)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--test",
        "pricing_boundary",
        "--command",
        "cargo test pricing_boundary",
        "--json",
        "--out",
        &receipt.display().to_string(),
    ]);
    assert_success(&output);

    let text = std::fs::read_to_string(&receipt)?;
    assert!(text.contains(r#""schema_version": "0.5""#));
    assert!(text.contains(r#""seam_id": "seam-a""#));
    assert!(text.contains(r#""change": "improved""#));
    assert!(text.contains(&format!(
        r#""ripr_version": "{}""#,
        env!("CARGO_PKG_VERSION")
    )));
    assert!(text.contains(r#""repo_root": "#));
    assert!(text.contains(r#""config_fingerprint": "fnv1a64:"#));
    assert!(text.contains(r#""generated_at": "unix_ms:"#));
    assert!(text.contains(r#""command_template_version": "0.1""#));
    assert!(text.contains(r#""before_artifact": {"#));
    assert!(text.contains(r#""after_artifact": {"#));
    assert!(text.contains(r#""verify_artifact": {"#));
    assert!(text.contains(r#""sha256": "#));
    assert!(text.contains(r#""before_class": "weakly_gripped""#));
    assert!(text.contains(r#""after_class": "strongly_gripped""#));
    assert!(text.contains(r#""movement": "improved""#));
    assert!(text.contains(r#""runtime_mutation_execution": false"#));
    assert!(text.contains(r#""next_action": {"#));
    assert!(text.contains(r#""kind": "improved""#));
    assert!(text.contains(r#""test_changed": "pricing_boundary""#));
    assert!(text.contains(r#""cargo test pricing_boundary""#));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// Issue #3967: a relative `agent receipt --out` anchors at the resolved
/// `--root`, so the product-rendered command pasted from a foreign
/// directory writes under the selected root instead of the launch
/// directory. Runs the full command path with the child process CWD
/// pinned to a separate launch dir (no in-process CWD change).
#[test]
fn agent_receipt_relative_out_anchors_at_root_from_foreign_cwd()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-out-anchor");
    let launch = unique_temp_workspace("agent-receipt-out-launch");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&launch)?;
    init_git_fixture_repo(&root)?;
    std::fs::create_dir_all(root.join("target/ripr/workflow"))?;
    let before = root.join("target/ripr/workflow/before.repo-exposure.json");
    let after = root.join("target/ripr/workflow/after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify = root.join("agent-verify.json");
    let verify_output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify_output);
    std::fs::write(&verify, verify_output.stdout)?;

    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = run_command(
        bin,
        Some(&launch),
        &[
            "agent",
            "receipt",
            "--root",
            &root.display().to_string(),
            "--verify-json",
            &verify.display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
            "--out",
            "target/ripr/reports/agent-receipt.json",
        ],
    )?;
    assert_success(&output);

    let anchored = root.join("target/ripr/reports/agent-receipt.json");
    let text = std::fs::read_to_string(&anchored)?;
    assert!(text.contains(r#""seam_id": "seam-a""#));
    assert!(
        !launch
            .join("target/ripr/reports/agent-receipt.json")
            .exists(),
        "relative --out must not write under the launch directory"
    );
    std::fs::remove_dir_all(root)?;
    std::fs::remove_dir_all(launch)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_fabricated_verify_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-fabricated");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the artifact pair stays admissible and the intended
    // canonical-output check is what rejects the fabricated verify JSON.
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify = root.join("fabricated-agent-verify.json");
    write_fabricated_agent_verify_json(&verify, &before, &after)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not canonical output"), "stderr: {stderr}");
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_incomparable_base_revision() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-base-mismatch");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let altered = std::fs::read_to_string(&after)?.replace(
        "\"base_revision\": null",
        "\"base_revision\": \"base:other\"",
    );
    std::fs::write(&after, recommit_repo_exposure_json(altered))?;
    let verify = root.join("fabricated-agent-verify.json");
    write_fabricated_agent_verify_json(&verify, &before, &after)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("[incomparable_base_revision]"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_incomparable_analysis_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-input-mismatch");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let old_head = concrete_fixture_repository_head(&root)?;
    std::fs::write(root.join("marker.txt"), "fixture-moved\n")?;
    run_git(&root, &["add", "marker.txt"])?;
    let commit = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "advance fixture",
        ],
    )?;
    assert!(commit.status.success(), "fixture commit failed: {commit:?}");
    let new_head = concrete_fixture_repository_head(&root)?;
    // Tamper the analysis input identity while keeping the after artifact
    // internally consistent (input identity, declared head, and snapshot
    // identity must agree under exact snapshot validation) so the pair
    // comparison — not single-artifact validation — rejects it.
    let altered = std::fs::read_to_string(&after)?
        .replace(
            &format!("\"head\": \"{old_head}\""),
            &format!("\"head\": \"{new_head}\""),
        )
        .replace(
            &format!("snapshot:input:v4:fnv1a64:00000000000000f1;revision:{old_head}"),
            &format!("snapshot:input:v4:fnv1a64:00000000000000f2;revision:{new_head}"),
        )
        .replace(
            "\"input_identity\": \"input:v4:fnv1a64:00000000000000f1\"",
            "\"input_identity\": \"input:v4:fnv1a64:00000000000000f2\"",
        );
    std::fs::write(&after, recommit_repo_exposure_json(altered))?;
    let verify = root.join("fabricated-agent-verify.json");
    write_fabricated_agent_verify_json(&verify, &before, &after)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[incomparable_analysis_inputs]"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_comparison_metadata_drift() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "producer version",
            "\"version\": \"0.11.0\"",
            "\"version\": \"0.10.0\"",
        ),
        (
            "analysis mode",
            "\"mode\": \"draft\", \"base_revision\"",
            "\"mode\": \"release\", \"base_revision\"",
        ),
        (
            "analysis profile",
            "\"profile\": \"draft\"",
            "\"profile\": \"release\"",
        ),
    ];
    for (label, from, to) in cases {
        let root = unique_temp_workspace(&format!("agent-receipt-metadata-{label}"));
        std::fs::create_dir_all(&root)?;
        init_git_fixture_repo(&root)?;
        let before = root.join("before.repo-exposure.json");
        let after = root.join("after.repo-exposure.json");
        let seam = r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#;
        write_bound_repo_exposure_fixture(&root, &before, seam)?;
        write_bound_repo_exposure_fixture(&root, &after, seam)?;
        let mut altered = std::fs::read_to_string(&after)?.replace(from, to);
        if label == "analysis mode" {
            altered = altered.replace("\"profile\": \"draft\"", "\"profile\": \"release\"");
        }
        std::fs::write(&after, recommit_repo_exposure_json(altered))?;
        let verify = root.join("fabricated-agent-verify.json");
        write_fabricated_agent_verify_json(&verify, &before, &after)?;
        let output = run_ripr(&[
            "agent",
            "receipt",
            "--root",
            &root.display().to_string(),
            "--verify-json",
            &verify.display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = if label == "analysis profile" {
            "invalid or unknown producer identity"
        } else {
            "[incomparable_analysis_inputs]"
        };
        assert!(stderr.contains(expected), "{label}: {stderr}");
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn agent_receipt_rejects_tampered_verify_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-tampered");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the after artifact is bound to a descended revision.
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify = root.join("agent-verify.json");
    let verify_output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify_output);
    let mut verify_value: serde_json::Value = serde_json::from_slice(&verify_output.stdout)?;
    verify_value["changed_seams"][0]["change"] = serde_json::Value::String("resolved".to_string());
    std::fs::write(&verify, serde_json::to_vec_pretty(&verify_value)?)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("not canonical output"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_rerendered_verify_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt-rerendered");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(
        &root,
        &before,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
    )?;
    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the after artifact is bound to a descended revision.
    advance_fixture_head(&root, "after movement")?;
    write_bound_repo_exposure_fixture(
        &root,
        &after,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify = root.join("agent-verify.json");
    let verify_output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify_output);
    // Same semantic values as canonical output, but re-rendered with compact
    // spacing: parses to an equal Value while differing byte-for-byte.
    let verify_value: serde_json::Value = serde_json::from_slice(&verify_output.stdout)?;
    std::fs::write(&verify, serde_json::to_vec(&verify_value)?)?;

    let output = run_ripr(&[
        "agent",
        "receipt",
        "--root",
        &root.display().to_string(),
        "--verify-json",
        &verify.display().to_string(),
        "--seam-id",
        "seam-a",
        "--json",
    ]);
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("not canonical output"));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// Shared setup for the #2922 PR B verify/receipt negative corpus: a
/// comparable moving before/after pair bound to the fixture repository plus
/// the authentic canonical agent-verify output captured from the binary.
/// Returns (before, after, verify JSON path, verify JSON text).
fn write_moving_pair_and_verify(
    root: &Path,
    before_seam_json: &str,
    after_seam_json: &str,
) -> Result<(PathBuf, PathBuf, PathBuf, String), Box<dyn std::error::Error>> {
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    write_bound_repo_exposure_fixture(root, &before, before_seam_json)?;
    advance_fixture_head(root, "after movement")?;
    write_bound_repo_exposure_fixture(root, &after, after_seam_json)?;
    let verify = root.join("agent-verify.json");
    let verify_output = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify_output);
    std::fs::write(&verify, &verify_output.stdout)?;
    let verify_text = String::from_utf8(verify_output.stdout)?;
    Ok((before, after, verify, verify_text))
}

fn run_agent_receipt_command(
    root: &Path,
    verify: &Path,
    seam_id: &str,
    out: Option<&Path>,
) -> Output {
    let mut args = vec![
        "agent".to_string(),
        "receipt".to_string(),
        "--root".to_string(),
        root.display().to_string(),
        "--verify-json".to_string(),
        verify.display().to_string(),
        "--seam-id".to_string(),
        seam_id.to_string(),
        "--json".to_string(),
    ];
    if let Some(out) = out {
        args.push("--out".to_string());
        args.push(out.display().to_string());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ripr(&refs)
}

#[test]
fn agent_verify_binds_artifact_content_commitments() -> Result<(), Box<dyn std::error::Error>> {
    // Positive control for #2922 PR B: canonical verify output is bound to
    // the exact content commitments of the validated before/after artifacts.
    let root = unique_temp_workspace("agent-verify-binding");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (before, after, _verify, verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify: serde_json::Value = serde_json::from_str(&verify_text)?;
    let before_digest = repo_exposure_artifact_content_digest(&before)?;
    let after_digest = repo_exposure_artifact_content_digest(&after)?;
    assert_eq!(json_pointer_str(&verify, "/schema_version")?, "0.3");
    assert_eq!(
        json_pointer_str(&verify, "/inputs/before_content_sha256")?,
        before_digest
    );
    assert_eq!(
        json_pointer_str(&verify, "/inputs/after_content_sha256")?,
        after_digest
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_verify_replayed_against_mutated_pair()
-> Result<(), Box<dyn std::error::Error>> {
    // Verify replay defense (#2922 PR B): a verify JSON produced for pair
    // A/B must not receipt pair A/B' when B' differs from B by any byte —
    // even when the mutation is invisible to the movement render.
    let root = unique_temp_workspace("agent-receipt-replay-mutated");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (before, after, verify, _verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    // Positive control: the authentic chain receipts before any mutation.
    let control_out = root.join("agent-receipt-control.json");
    let control = run_agent_receipt_command(&root, &verify, "seam-a", Some(&control_out));
    assert_success(&control);

    // Mutation: change bytes the movement comparison never reads
    // (`run_status` is not part of the artifact validator's governed fields
    // or the seam payload), then recommit so the artifact stays valid.
    let original_digest = repo_exposure_artifact_content_digest(&after)?;
    let mutation = "set after artifact run_status to `stalled` and recommit its content commitment (invisible to the movement render)";
    let mutated = std::fs::read_to_string(&after)?.replace(
        "\"run_status\": \"complete\"",
        "\"run_status\": \"stalled\"",
    );
    assert_ne!(
        mutated,
        std::fs::read_to_string(&after)?,
        "mutation must change the after artifact bytes"
    );
    std::fs::write(&after, recommit_repo_exposure_json(mutated))?;
    let mutated_digest = repo_exposure_artifact_content_digest(&after)?;
    assert_ne!(
        original_digest, mutated_digest,
        "mutation must change the after artifact content commitment"
    );

    // Control: the mutated pair is still fully valid and verifiable — only
    // the replayed verify JSON is bound to the pre-mutation bytes.
    let fresh_verify = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root.display().to_string(),
        "--before",
        &before.display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&fresh_verify);

    let replay_out = root.join("agent-receipt-replay.json");
    let replay = run_agent_receipt_command(&root, &verify, "seam-a", Some(&replay_out));
    assert_failure(&replay);
    let stderr = String::from_utf8_lossy(&replay.stderr);
    assert!(
        stderr.contains("[not_canonical]"),
        "mutation: {mutation}; original after digest {original_digest}; mutated after digest {mutated_digest}; expected failure kind [not_canonical]; actual stderr: {stderr}"
    );
    assert!(
        !replay_out.exists(),
        "a rejected replay must not leave an authoritative receipt artifact at {}",
        replay_out.display()
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_tampered_pair_binding_fields() -> Result<(), Box<dyn std::error::Error>> {
    // Verify commitment/tamper negatives (#2922 PR B): the bound artifact
    // digest and the verify status are governed output; altering either must
    // fail for one bounded reason.
    let root = unique_temp_workspace("agent-receipt-binding-tamper");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (_before, after, _verify, verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    // Case 1: flip one hex digit of the bound after-artifact digest.
    let bound_digest = repo_exposure_artifact_content_digest(&after)?;
    let first_hex = bound_digest
        .as_bytes()
        .get(7)
        .copied()
        .ok_or("bound digest is too short")? as char;
    let flipped = format!(
        "sha256:{}{}",
        if first_hex == '0' { '1' } else { '0' },
        &bound_digest[8..]
    );
    assert_ne!(bound_digest, flipped, "digest flip must change the digest");
    let tampered = root.join("agent-verify-tampered-digest.json");
    std::fs::write(&tampered, verify_text.replace(&bound_digest, &flipped))?;
    let output = run_agent_receipt_command(&root, &tampered, "seam-a", None);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[not_canonical]"),
        "mutation: flipped one hex digit of the verify JSON after_content_sha256; original digest {bound_digest}; mutated digest {flipped}; expected failure kind [not_canonical]; actual stderr: {stderr}"
    );

    // Case 2: a failed verify cannot be represented as a successful one
    // (and vice versa): the status field is governed canonical output.
    let failed = root.join("agent-verify-failed-status.json");
    std::fs::write(
        &failed,
        verify_text.replace("\"status\": \"advisory\"", "\"status\": \"failed\""),
    )?;
    let output = run_agent_receipt_command(&root, &failed, "seam-a", None);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[not_canonical]"),
        "mutation: rewrote verify JSON status advisory -> failed; expected failure kind [not_canonical]; actual stderr: {stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_unsupported_verify_schema() -> Result<(), Box<dyn std::error::Error>> {
    // Schema-version fail-closed (#2922 PR B, #3027): verify JSON from an
    // older or newer schema than the canonical 0.3 binding shape is rejected
    // for one bounded typed reason before any artifact work. A 0.2 document
    // that is canonical in every other way is NOT recomputed into a valid
    // 0.3 receipt — there is no silent migration.
    let root = unique_temp_workspace("agent-receipt-schema");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (_before, _after, verify, verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    // Positive control: the canonical 0.3 output receipts.
    let control = run_agent_receipt_command(&root, &verify, "seam-a", None);
    assert_success(&control);

    for (label, version) in [("older", "0.1"), ("older", "0.2"), ("newer", "9.9")] {
        let rewritten = root.join(format!("agent-verify-schema-{label}-{version}.json"));
        std::fs::write(
            &rewritten,
            verify_text.replace(
                "\"schema_version\": \"0.3\"",
                &format!("\"schema_version\": \"{version}\""),
            ),
        )?;
        let output = run_agent_receipt_command(&root, &rewritten, "seam-a", None);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("[unsupported_schema]"),
            "mutation: rewrote verify JSON schema_version 0.3 -> {version} ({label} schema); expected failure kind [unsupported_schema]; actual stderr: {stderr}"
        );
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_stale_verify_after_repository_movement()
-> Result<(), Box<dyn std::error::Error>> {
    // Stale verify + receipt replay negatives (#2922 PR B): a verify result
    // produced while the pair was current must not receipt after the
    // repository moves; the replayed receipt re-validation recomputes
    // currentness and the canonical comparison fails closed.
    let root = unique_temp_workspace("agent-receipt-stale-verify");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (_before, _after, verify, _verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;
    let verify_head = concrete_fixture_repository_head(&root)?;
    let verify_digest = sha256_hex_bytes(&std::fs::read(&verify)?);

    // Positive control: the fresh chain receipts.
    let control_out = root.join("agent-receipt-control.json");
    let control = run_agent_receipt_command(&root, &verify, "seam-a", Some(&control_out));
    assert_success(&control);

    // Repository movement after verify: the same verify JSON is now stale.
    advance_fixture_head(&root, "post-verify movement")?;
    let replay_head = concrete_fixture_repository_head(&root)?;
    assert_ne!(
        verify_head, replay_head,
        "fixture must move after the verify result was produced"
    );

    let replay_out = root.join("agent-receipt-replay.json");
    let replay = run_agent_receipt_command(&root, &verify, "seam-a", Some(&replay_out));
    assert_failure(&replay);
    let stderr = String::from_utf8_lossy(&replay.stderr);
    assert!(
        stderr.contains("[not_canonical]"),
        "mutation: repository advanced after verify (head {verify_head} -> {replay_head}); replayed verify digest {verify_digest}; expected failure kind [not_canonical]; actual stderr: {stderr}"
    );
    // The human surface cannot strengthen the typed failure: nothing is
    // rendered to stdout and no receipt artifact is left behind (#2922 PR B).
    assert!(
        replay.stdout.is_empty(),
        "a rejected receipt replay must render nothing to stdout"
    );
    assert!(
        !replay_out.exists(),
        "a rejected receipt replay must not leave an authoritative receipt artifact at {}",
        replay_out.display()
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_rejects_target_absent_from_both_states() -> Result<(), Box<dyn std::error::Error>>
{
    // Movement-outcome cell (#2922): the retained verify target seam does
    // not exist in either state. Verify compares the presented pair fine;
    // the receipt for the absent target fails for one bounded reason.
    let root = unique_temp_workspace("agent-receipt-absent-target");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (_before, _after, verify, _verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-z","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-z","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
    )?;

    let output = run_agent_receipt_command(&root, &verify, "seam-retained-target", None);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "agent receipt seam_id seam-retained-target was not found in agent verify JSON"
        ),
        "mutation: requested a receipt for a target seam absent from both states; expected the seam-selection rejection; actual stderr: {stderr}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn agent_receipt_keeps_unmoved_target_unchanged_when_another_seam_moves()
-> Result<(), Box<dyn std::error::Error>> {
    // Movement-outcome cell (#2922): the after state moves seam-y while the
    // retained verify target seam-x does not move. The typed receipt
    // projection for seam-x must stay `unchanged`; movement on a different
    // item can never strengthen the retained target's receipt.
    let root = unique_temp_workspace("agent-receipt-unmoved-target");
    std::fs::create_dir_all(&root)?;
    init_git_fixture_repo(&root)?;
    let (_before, _after, verify, verify_text) = write_moving_pair_and_verify(
        &root,
        r#"{"seam_id":"seam-x","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}, {"seam_id":"seam-y","kind":"predicate_boundary","file":"src/report.rs","line":21,"grip_class":"weakly_gripped"}"#,
        r#"{"seam_id":"seam-x","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}, {"seam_id":"seam-y","kind":"predicate_boundary","file":"src/report.rs","line":21,"grip_class":"strongly_gripped"}"#,
    )?;
    // Control: the verify result really did record movement on seam-y only.
    let verify_value: serde_json::Value = serde_json::from_str(&verify_text)?;
    let moved: Vec<&str> = verify_value["changed_seams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|seam| seam["seam_id"].as_str())
        .collect();
    assert_eq!(
        moved,
        vec!["seam-y"],
        "control: only seam-y should move in this fixture"
    );

    let output = run_agent_receipt_command(&root, &verify, "seam-x", None);
    assert_success(&output);
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json_pointer_str(&receipt, "/seam/change")?,
        "unchanged",
        "the unmoved retained target must not be reported as moved"
    );
    assert_eq!(
        json_pointer_str(&receipt, "/seam/before")?,
        json_pointer_str(&receipt, "/seam/after")?,
        "the unmoved retained target must keep equal before/after classes"
    );
    assert_eq!(
        json_pointer_str(&receipt, "/summary/receipt_state")?,
        "receipt_movement_unchanged",
        "the typed receipt state must not be upgraded by movement on seam-y"
    );
    assert_eq!(
        json_pointer_str(&receipt, "/summary/next_action/kind")?,
        "unchanged",
        "the next-action projection must not be upgraded by movement on seam-y"
    );
    assert_eq!(
        json_pointer_str(&receipt, "/provenance/movement")?,
        "unchanged"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn check_badge_json_output_has_native_badge_shape() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.8""#));
    // Diff-scoped badge JSON never carries a public projection.
    assert!(!stdout.contains(r#""public_projection""#));
    assert!(stdout.contains(r#""kind": "ripr""#));
    assert!(stdout.contains(r#""scope": "diff""#));
    assert!(stdout.contains(r#""basis": "finding_exposure""#));
    assert!(stdout.contains(r#""label": "ripr""#));
    assert!(stdout.contains(r#""counts""#));
    assert!(stdout.contains(r#""reason_counts""#));
    assert!(stdout.contains(r#""policy""#));
    assert!(stdout.contains(r#""unsuppressed_exposure_gaps""#));
    assert!(stdout.contains(r#""duplicate_activation_and_oracle_shape": 0"#));
    assert!(!stdout.contains(r#""schemaVersion""#));
    // The sample diff has 4 weakly_exposed findings after nested call shapes are
    // excluded; the badge headline reflects the surviving semantic probes.
    assert!(stdout.contains(r#""message": "4""#));
    assert!(stdout.contains(r#""status": "warn""#));
    assert!(stdout.contains(r#""color": "orange""#));
}

#[test]
fn check_badge_shields_output_has_exactly_four_top_level_fields() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-shields",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schemaVersion": 1"#));
    assert!(stdout.contains(r#""label": "ripr""#));
    assert!(stdout.contains(r#""message": "4""#));
    assert!(stdout.contains(r#""color": "orange""#));
    // Native-JSON-only fields must not leak into the Shields shape.
    for forbidden in [
        r#""counts""#,
        r#""reason_counts""#,
        r#""policy""#,
        r#""kind""#,
        r#""status""#,
        r#""scope""#,
        r#""basis""#,
        r#""schema_version""#,
    ] {
        assert!(
            !stdout.contains(forbidden),
            "Shields projection must not contain `{forbidden}`: {stdout}"
        );
    }
    // Message has no denominator and no coverage framing.
    assert!(!stdout.contains('/') || !stdout.contains(r#""message""#));
    assert!(!stdout.to_ascii_lowercase().contains("coverage"));
    assert!(!stdout.to_ascii_lowercase().contains("uncovered"));
}

fn fixture_test_efficiency_report() -> &'static str {
    // Three-test fixture: one bare smoke_only (counts as actionable), one
    // smoke_only with declared_intent (counts as intentional, not headline),
    // one opaque (flows into unknowns_test_efficiency, not headline).
    r#"{
  "schema_version": "0.1",
  "tests": [
    {"class": "smoke_only"},
    {"class": "smoke_only", "declared_intent": {"intent": "smoke", "owner": "x", "reason": "y", "source": ".ripr/test_intent.toml"}},
    {"class": "opaque"}
  ],
  "metrics": {
    "tests_scanned": 3,
    "reason_counts": {
      "smoke_oracle_only": 2,
      "opaque_helper_or_fixture_boundary": 1
    }
  }
}
"#
}

fn make_temp_workspace(report: Option<&str>) -> Result<PathBuf, String> {
    make_temp_workspace_with_suppressions(report, None)
}

#[test]
fn doctor_reports_missing_config_defaults() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config: not found; using built-in defaults"));
    assert!(stdout.contains("Analysis mode default: draft"));
    assert!(stdout.contains("LSP seam diagnostics default: true"));
    assert!(stdout.contains("Suppressions path: .ripr/suppressions.toml"));
    assert!(stdout.contains("Start-here packet: target/ripr/reports/start-here.md"));
    assert!(stdout.contains("`ripr first-pr` composes it once analysis evidence exists"));
    assert!(!stdout.contains("(present; open it first)"));
    // On a workspace with no artifacts, `first-pr` has nothing to compose: it
    // returns `missing_artifacts` and answers with a `ripr check` regeneration
    // command. So the safe next action must route to the analysis command this
    // screen already recommends, not to the compose command.
    assert!(stdout.contains("Safe next action: run the recommended first command below"));
    assert!(
        !stdout.contains("Safe next action: run `ripr first-pr"),
        "a fresh workspace must not be sent to the compose command:\n{stdout}"
    );
    assert!(stdout.contains("Recovery states: missing artifact, stale evidence, wrong root"));
    assert!(stdout.contains("Proof rail: verify command, receipt command, and receipt path"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_present_start_here_packet() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    write_start_here_packet(&workspace, Some(env!("CARGO_PKG_VERSION")))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Start-here packet: target/ripr/reports/start-here.md"));
    assert!(stdout.contains("(present; open it first)"));
    assert!(!stdout.contains("not yet generated"));
    assert!(!stdout.contains("stale_evidence"));
    // With a packet on disk `first-pr` has something to refresh, so naming it
    // here is a real route rather than a dead end.
    assert!(stdout.contains("Safe next action: open that packet"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_version_mismatched_start_here_packet_as_stale() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let recorded = if env!("CARGO_PKG_VERSION") == "0.10.0" {
        "0.9.0"
    } else {
        "0.10.0"
    };
    write_start_here_packet(&workspace, Some(recorded))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stale_evidence"), "{stdout}");
    assert!(stdout.contains(recorded), "{stdout}");
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
    assert!(!stdout.contains("(present; open it first)"), "{stdout}");
    assert!(
        stdout.contains("Safe next action: `ripr first-pr --root ")
            && stdout.contains("--head HEAD` refreshes it")
            && !stdout.contains("--base <ref>"),
        "{stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_start_here_packet_missing_version_as_stale() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    write_start_here_packet(&workspace, None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stale_evidence"), "{stdout}");
    assert!(stdout.contains("missing ripr_version"), "{stdout}");
    assert!(!stdout.contains("(present; open it first)"), "{stdout}");
    assert!(
        stdout.contains("Safe next action: `ripr first-pr --root ")
            && stdout.contains("refreshes it"),
        "{stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

fn write_start_here_packet(workspace: &Path, ripr_version: Option<&str>) -> Result<(), String> {
    let reports = workspace.join("target/ripr/reports");
    std::fs::create_dir_all(&reports).map_err(|e| format!("create reports dir: {e}"))?;
    std::fs::write(reports.join("start-here.md"), "# start here\n")
        .map_err(|e| format!("write start-here.md: {e}"))?;
    let mut packet = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "first_pr_start_here"
    });
    if let Some(version) = ripr_version {
        packet["ripr_version"] = serde_json::Value::String(version.to_string());
    }
    std::fs::write(
        reports.join("start-here.json"),
        serde_json::to_string_pretty(&packet)
            .map_err(|e| format!("serialize start-here.json: {e}"))?,
    )
    .map_err(|e| format!("write start-here.json: {e}"))?;
    Ok(())
}

#[test]
fn doctor_reports_markdown_only_start_here_packet_as_stale() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let reports = workspace.join("target/ripr/reports");
    std::fs::create_dir_all(&reports).map_err(|e| format!("create reports dir: {e}"))?;
    std::fs::write(reports.join("start-here.md"), "# start here\n")
        .map_err(|e| format!("write start-here.md: {e}"))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stale_evidence"), "{stdout}");
    assert!(
        stdout.contains("start-here.json is missing or unreadable"),
        "{stdout}"
    );
    assert!(!stdout.contains("(present; open it first)"), "{stdout}");

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_directory_squatting_packet_path_as_not_generated() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let squat = workspace.join("target/ripr/reports/start-here.md");
    std::fs::create_dir_all(&squat).map_err(|e| format!("create squat dir: {e}"))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("`ripr first-pr` composes it once analysis evidence exists"));
    assert!(!stdout.contains("(present; open it first)"));
    assert!(stdout.contains("Safe next action: run the recommended first command below"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_rejects_missing_and_file_roots_without_leaking_the_git_invocation() -> Result<(), String> {
    // An explicit --root that is not a directory used to reach the diff loader
    // and print git's spawn failure with the whole argv. `check` now validates
    // it through the same authority the other root-taking commands use.
    let workspace = make_temp_workspace(None)?;

    let missing = workspace.join("missing-root");
    let missing_string = missing.display().to_string();
    let missing_output = run_ripr(&["check", "--root", &missing_string, "--base", "HEAD"]);
    assert_failure(&missing_output);
    let missing_stderr = String::from_utf8_lossy(&missing_output.stderr).into_owned();
    assert!(
        missing_stderr.contains("is not a directory"),
        "stderr:\n{missing_stderr}"
    );
    assert!(
        missing_stderr.contains(&missing_string),
        "expected the offending path to be named; stderr:\n{missing_stderr}"
    );
    // Discriminator: the git invocation must not reach the user.
    assert!(
        !missing_stderr.contains("core.quotePath") && !missing_stderr.contains("failed to run git"),
        "git invocation leaked into stderr:\n{missing_stderr}"
    );

    let file_root = workspace.join("root-file");
    std::fs::write(&file_root, "not a directory\n")
        .map_err(|error| format!("write file root: {error}"))?;
    let file_string = file_root.display().to_string();
    let file_output = run_ripr(&["check", "--root", &file_string, "--base", "HEAD"]);
    assert_failure(&file_output);
    assert!(
        String::from_utf8_lossy(&file_output.stderr).contains("is not a directory"),
        "stderr:\n{}",
        String::from_utf8_lossy(&file_output.stderr)
    );

    // Negative control: a real workspace root is still analyzed. The workspace
    // gets its own repository and an explicit base because the temp dir is
    // inside the checkout running the tests (.cargo/config.toml sets TMPDIR to
    // target/). A bare `check` there would diff the enclosing branch against
    // its default base and fail on that branch's size, not on this root.
    run_git(&workspace, &["init"])?;
    run_git(&workspace, &["config", "user.email", "test@test.com"])?;
    run_git(&workspace, &["config", "user.name", "Test"])?;
    run_git(&workspace, &["add", "."])?;
    run_git(&workspace, &["commit", "-m", "initial"])?;
    let workspace_string = workspace.display().to_string();
    let ok_output = run_ripr(&["check", "--root", &workspace_string, "--base", "HEAD"]);
    assert!(
        ok_output.status.success(),
        "expected a valid explicit root to run; stderr:\n{}",
        String::from_utf8_lossy(&ok_output.stderr)
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn config_validate_rejects_missing_and_file_roots() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let missing = workspace.join("missing-root");
    let missing_string = missing.display().to_string();
    let missing_output = run_ripr(&["config", "validate", "--root", &missing_string]);
    assert_failure(&missing_output);
    assert!(
        String::from_utf8_lossy(&missing_output.stderr).contains("is not a directory"),
        "stderr:\n{}",
        String::from_utf8_lossy(&missing_output.stderr)
    );

    let file_root = workspace.join("root-file");
    std::fs::write(&file_root, "not a directory\n")
        .map_err(|error| format!("write file root: {error}"))?;
    let file_string = file_root.display().to_string();
    let file_output = run_ripr(&["config", "validate", "--root", &file_string]);
    assert_failure(&file_output);
    assert!(
        String::from_utf8_lossy(&file_output.stderr).contains("is not a directory"),
        "stderr:\n{}",
        String::from_utf8_lossy(&file_output.stderr)
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// Run `binary doctor --json` on `root` with `path_dirs` ahead of the host
/// PATH (which still supplies git), and parse the report.
fn doctor_json_with_path_prefix(
    binary: &Path,
    root: &Path,
    path_dirs: &[&Path],
) -> Result<(bool, serde_json::Value), String> {
    let host_path = std::env::var_os("PATH").unwrap_or_default();
    let search_path = std::env::join_paths(
        path_dirs
            .iter()
            .map(|dir| dir.to_path_buf())
            .chain(std::env::split_paths(&host_path)),
    )
    .map_err(|error| format!("build PATH: {error}"))?;
    let search_path = search_path.to_string_lossy().into_owned();
    let root_arg = root.display().to_string();
    let output = output_retrying_text_file_busy(|| {
        run_command_with_env(
            &binary.to_string_lossy(),
            root,
            &["doctor", "--root", &root_arg, "--json"],
            &[("PATH", &search_path)],
        )
    })
    .map_err(|error| format!("run doctor: {error}"))?;
    let report = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("doctor JSON did not parse: {error}"))?;
    Ok((output.status.success(), report))
}

/// #3797 control 9: a Cargo build output directory first on PATH replaces the
/// installed `ripr` for everything that runs `ripr`. Doctor must name it,
/// stay quiet when the installed binary is first, and never change its
/// pass/fail status over it.
#[test]
fn doctor_names_workspace_build_first_on_path() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let installed_dir = unique_temp_workspace("doctor-installed-bin");
    let result = (|| -> Result<(), String> {
        std::fs::create_dir_all(&installed_dir)
            .map_err(|error| format!("create installed bin dir: {error}"))?;
        let built = Path::new(env!("CARGO_BIN_EXE_ripr"));
        let built_dir = built.parent().ok_or("built ripr has no parent")?;
        let installed = installed_dir.join(built.file_name().ok_or("built ripr has no name")?);
        std::fs::copy(built, &installed).map_err(|error| format!("install copy: {error}"))?;

        let (installed_ok, report) =
            doctor_json_with_path_prefix(&installed, &workspace, &[&installed_dir])?;
        let binary = &report["binary"];
        if binary["path_ripr_is_running_executable"] != true
            || binary["path_ripr_is_cargo_build_output"] != false
            || binary["executable_is_cargo_build_output"] != false
            || binary["warnings"] != serde_json::json!([])
        {
            return Err(format!(
                "installed ripr first on PATH must be quiet: {binary}"
            ));
        }
        if binary["version"].as_str() != Some(expected_version_line().trim_end()) {
            return Err(format!("doctor must report the --version line: {binary}"));
        }

        let (workspace_ok, report) =
            doctor_json_with_path_prefix(&installed, &workspace, &[built_dir, &installed_dir])?;
        let binary = &report["binary"];
        let warnings = binary["warnings"].to_string();
        if binary["path_ripr_is_cargo_build_output"] != true
            || binary["path_ripr_is_running_executable"] != false
            || !warnings.contains("Cargo build output")
            || !warnings.contains("not the running binary")
        {
            return Err(format!(
                "workspace build first on PATH was not named: {binary}"
            ));
        }
        if workspace_ok != installed_ok {
            return Err(format!(
                "a workspace build on PATH is advisory and must not change doctor status: {report}"
            ));
        }
        Ok(())
    })();
    ignore_remove_dir_all(&installed_dir);
    ignore_remove_dir_all(&workspace);
    result
}

#[test]
fn doctor_json_reports_an_unpinned_generated_workflow_as_advisory() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let workflows = workspace.join(".github/workflows");
    std::fs::create_dir_all(&workflows).map_err(|err| format!("create workflows: {err}"))?;
    // The install step `ripr 0.10.0 init --ci github` wrote.
    std::fs::write(
        workflows.join("ripr.yml"),
        "      - name: Install ripr\n        run: cargo install ripr --locked\n",
    )
    .map_err(|err| format!("write workflow: {err}"))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root, "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("doctor JSON did not parse: {err}"))?;
    std::fs::remove_dir_all(workspace).map_err(|err| format!("remove workspace: {err}"))?;

    let check = report["checks"]
        .as_array()
        .and_then(|checks| {
            checks
                .iter()
                .find(|check| check["name"] == "generated_workflow")
        })
        .ok_or_else(|| format!("doctor JSON must report the stale workflow: {report}"))?;
    assert_eq!(check["status"], "advisory");
    assert!(
        check["evidence"].as_str().is_some_and(|evidence| evidence
            .contains("installs ripr without a version")
            && evidence.contains("ripr init --ci github --force")),
        "{check}"
    );
    Ok(())
}

#[test]
fn doctor_json_reports_current_schema() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root, "--json"]);
    assert_success(&output);

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("doctor JSON did not parse: {err}"))?;
    assert_eq!(report["schema_version"], "0.3");
    assert_eq!(report["tool"], "ripr");
    assert_eq!(report["profile"], "analysis");
    assert_eq!(report["ripr_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["ripr_build_msrv"], env!("CARGO_PKG_RUST_VERSION"));
    assert!(
        report["runtime_probes"].is_array(),
        "doctor JSON must expose typed runtime probe results: {report}"
    );
    std::fs::remove_dir_all(workspace).map_err(|err| format!("remove workspace: {err}"))?;
    Ok(())
}

#[test]
#[cfg(feature = "lang-typescript")]
fn doctor_json_process_fails_for_enabled_missing_node_runtime() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(
        workspace.join("ripr.toml"),
        "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
    )
    .map_err(|error| format!("write ripr.toml: {error}"))?;

    let shim_dir = workspace.join("runtime-shims");
    std::fs::create_dir_all(&shim_dir)
        .map_err(|error| format!("create runtime shim directory: {error}"))?;
    #[cfg(windows)]
    let node_shim = shim_dir.join("node.cmd");
    #[cfg(not(windows))]
    let node_shim = shim_dir.join("node");
    #[cfg(windows)]
    let shim_contents = "@echo off\r\nexit /b 127\r\n";
    #[cfg(not(windows))]
    let shim_contents = "#!/bin/sh\nexit 127\n";
    std::fs::write(&node_shim, shim_contents)
        .map_err(|error| format!("write node runtime shim: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&node_shim)
            .map_err(|error| format!("read node runtime shim permissions: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&node_shim, permissions)
            .map_err(|error| format!("make node runtime shim executable: {error}"))?;
    }

    let search_path = std::env::join_paths([shim_dir])
        .map_err(|error| format!("build isolated runtime PATH: {error}"))?;
    let search_path = search_path.to_string_lossy().into_owned();
    let root = workspace.display().to_string();
    let output = run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        &workspace,
        &["doctor", "--root", &root, "--json"],
        &[("PATH", &search_path)],
    )
    .map_err(|error| format!("run doctor process: {error}"))?;

    if output.status.success() {
        return Err(format!(
            "enabled TypeScript without a working node runtime must fail doctor; stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("doctor failure JSON did not parse: {error}"))?;
    let node_probe = report["runtime_probes"]
        .as_array()
        .and_then(|probes| {
            probes
                .iter()
                .find(|probe| probe["language"] == "typescript" && probe["tool"] == "node")
        })
        .ok_or_else(|| format!("doctor JSON omitted the required node probe: {report}"))?;
    if node_probe["required"] != true || node_probe["status"] != "fail" {
        return Err(format!(
            "required node probe did not fail closed: {node_probe}"
        ));
    }

    std::fs::remove_dir_all(workspace)
        .map_err(|error| format!("remove doctor runtime fixture: {error}"))?;
    Ok(())
}

/// A PATH holding only `git` plus working `node` and `python3` shims — no
/// `cargo` and no `rustc` — so doctor's Rust toolchain scope is observable
/// at the process boundary independent of the host toolchain.
#[cfg(unix)]
fn doctor_path_without_rust_toolchain(workspace: &Path) -> Result<String, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let shims = workspace.join("no-rust-bin");
    std::fs::create_dir_all(&shims).map_err(|error| format!("create shim dir: {error}"))?;
    let host_path = std::env::var_os("PATH").ok_or("PATH is not set")?;
    let git = std::env::split_paths(&host_path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .ok_or("git is not on PATH")?;
    std::os::unix::fs::symlink(&git, shims.join("git"))
        .map_err(|error| format!("link git: {error}"))?;
    for (tool, version) in [("node", "v22.0.0"), ("python3", "Python 3.12.0")] {
        // Stage, close, then publish so the executable path is never open
        // for writing when it is exec'd (#2242).
        let staged = shims.join(format!(".{tool}.staged"));
        let mut file = std::fs::File::create(&staged)
            .map_err(|error| format!("create {tool} shim: {error}"))?;
        file.write_all(format!("#!/bin/sh\necho '{version}'\n").as_bytes())
            .map_err(|error| format!("write {tool} shim: {error}"))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("chmod {tool} shim: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync {tool} shim: {error}"))?;
        drop(file);
        std::fs::rename(&staged, shims.join(tool))
            .map_err(|error| format!("publish {tool} shim: {error}"))?;
    }
    Ok(shims.display().to_string())
}

/// Run `ripr doctor` for `root` (human or `--json`) with `path` as PATH.
#[cfg(unix)]
fn run_doctor_with_path(root: &Path, path: &str, json: bool) -> Result<Output, String> {
    let root_arg = root.display().to_string();
    let mut args = vec!["doctor", "--root", root_arg.as_str()];
    if json {
        args.push("--json");
    }
    run_command_with_env(env!("CARGO_BIN_EXE_ripr"), root, &args, &[("PATH", path)])
        .map_err(|error| format!("run doctor: {error}"))
}

#[cfg(unix)]
fn doctor_check_status(report: &serde_json::Value, name: &str) -> String {
    report["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["name"] == name))
        .and_then(|check| check["status"].as_str())
        .unwrap_or("absent")
        .to_string()
}

/// A Python-only or TypeScript-only root is not a Rust project: doctor must
/// pass without Cargo.toml, cargo, or rustc, and say why those checks were
/// skipped instead of reporting them as broken setup.
#[cfg(unix)]
fn assert_doctor_passes_without_rust_toolchain(
    root: &Path,
    path: &str,
    scope_reason: &str,
) -> Result<(), String> {
    let json_output = run_doctor_with_path(root, path, true)?;
    let report: serde_json::Value = serde_json::from_slice(&json_output.stdout)
        .map_err(|error| format!("doctor JSON did not parse: {error}"))?;
    if !json_output.status.success() || report["status"] != "pass" {
        return Err(format!(
            "doctor --json must pass without a Rust toolchain: {report}"
        ));
    }
    for name in ["cargo_toml", "tool_cargo", "tool_rustc"] {
        if doctor_check_status(&report, name) != "skipped" {
            return Err(format!("{name} must be skipped: {report}"));
        }
    }
    if doctor_check_status(&report, "tool_git") != "pass" {
        return Err(format!("git stays required and must pass: {report}"));
    }

    let human = run_doctor_with_path(root, path, false)?;
    let stdout = String::from_utf8_lossy(&human.stdout);
    if !human.status.success()
        || !stdout.contains(&format!("- Cargo.toml check skipped: {scope_reason}"))
        || !stdout.contains(&format!("- cargo check skipped: {scope_reason}"))
        || !stdout.contains(&format!("- rustc check skipped: {scope_reason}"))
        || stdout.contains("! no Cargo.toml")
        || !stdout.contains("✓ doctor checks passed")
    {
        return Err(format!(
            "human doctor must pass and explain the skipped Rust checks:\n{stdout}"
        ));
    }
    Ok(())
}

#[test]
#[cfg(all(unix, feature = "lang-python"))]
fn doctor_passes_python_only_root_without_rust_toolchain() -> Result<(), String> {
    let workspace = unique_temp_workspace("doctor-python-only");
    std::fs::create_dir_all(workspace.join("src/textfmt"))
        .map_err(|error| format!("create python package: {error}"))?;
    std::fs::write(
        workspace.join("pyproject.toml"),
        "[project]\nname = \"textfmt\"\n",
    )
    .map_err(|error| format!("write pyproject.toml: {error}"))?;
    std::fs::write(
        workspace.join("src/textfmt/__init__.py"),
        "def shout(text):\n    return text.upper()\n",
    )
    .map_err(|error| format!("write python source: {error}"))?;
    let path = doctor_path_without_rust_toolchain(&workspace)?;
    let result = assert_doctor_passes_without_rust_toolchain(
        &workspace,
        &path,
        "Rust not detected at this root (no Cargo.toml or .rs files); in scope: python",
    );
    ignore_remove_dir_all(&workspace);
    result
}

#[test]
#[cfg(all(unix, feature = "lang-typescript"))]
fn doctor_passes_typescript_root_without_rust_toolchain() -> Result<(), String> {
    let workspace = unique_temp_workspace("doctor-typescript-only");
    std::fs::create_dir_all(workspace.join("src"))
        .map_err(|error| format!("create src: {error}"))?;
    std::fs::write(
        workspace.join("ripr.toml"),
        "[languages]\nenabled = [\"typescript\"]\n",
    )
    .map_err(|error| format!("write ripr.toml: {error}"))?;
    std::fs::write(workspace.join("package.json"), "{\"name\": \"pricing\"}\n")
        .map_err(|error| format!("write package.json: {error}"))?;
    std::fs::write(
        workspace.join("src/price.ts"),
        "export const price = (n: number) => n * 2;\n",
    )
    .map_err(|error| format!("write ts source: {error}"))?;
    let path = doctor_path_without_rust_toolchain(&workspace)?;
    let result = assert_doctor_passes_without_rust_toolchain(
        &workspace,
        &path,
        "Rust is not enabled in [languages]",
    );
    ignore_remove_dir_all(&workspace);
    result
}

/// Discriminating negative for the two tests above: under the same PATH, a
/// Rust root still fails on the missing toolchain, and Rust sources without
/// a Cargo.toml still fail the Cargo.toml check.
#[test]
#[cfg(unix)]
fn doctor_discloses_missing_verification_tools_but_still_checks_manifest() -> Result<(), String> {
    let workspace = unique_temp_workspace("doctor-rust-no-toolchain");
    let path = doctor_path_without_rust_toolchain(&workspace)?;

    let with_manifest = workspace.join("with-manifest");
    std::fs::create_dir_all(with_manifest.join("src"))
        .map_err(|error| format!("create rust root: {error}"))?;
    std::fs::write(
        with_manifest.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|error| format!("write Cargo.toml: {error}"))?;
    std::fs::write(with_manifest.join("src/lib.rs"), "pub fn f() {}\n")
        .map_err(|error| format!("write lib.rs: {error}"))?;
    let output = run_doctor_with_path(&with_manifest, &path, true)?;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("doctor JSON did not parse: {error}"))?;
    let manifest_result = if !output.status.success()
        || report["status"] != "pass"
        || doctor_check_status(&report, "cargo_toml") != "pass"
        || doctor_check_status(&report, "tool_cargo") != "advisory"
        || doctor_check_status(&report, "tool_rustc") != "advisory"
    {
        Err(format!(
            "an installed binary must keep analysis available: {report}"
        ))
    } else {
        Ok(())
    };

    let source_build = run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        &with_manifest,
        &[
            "doctor",
            "--root",
            &with_manifest.display().to_string(),
            "--profile",
            "source-build",
            "--json",
        ],
        &[("PATH", path.as_str())],
    )
    .map_err(|error| format!("run source-build doctor: {error}"))?;
    let source_report: serde_json::Value = serde_json::from_slice(&source_build.stdout)
        .map_err(|error| format!("source-build doctor JSON did not parse: {error}"))?;
    let source_result = if source_build.status.success()
        || source_report["status"] != "fail"
        || source_report["profile"] != "source-build"
        || doctor_check_status(&source_report, "tool_cargo") != "fail"
        || doctor_check_status(&source_report, "tool_rustc") != "fail"
    {
        Err(format!(
            "source-build must require cargo and rustc: {source_report}"
        ))
    } else {
        Ok(())
    };

    let sources_only = workspace.join("sources-only");
    std::fs::create_dir_all(sources_only.join("src"))
        .map_err(|error| format!("create sources root: {error}"))?;
    std::fs::write(sources_only.join("src/lib.rs"), "pub fn f() {}\n")
        .map_err(|error| format!("write lib.rs: {error}"))?;
    let output = run_doctor_with_path(&sources_only, &path, false)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let sources_result = if output.status.success() || !stdout.contains("! no Cargo.toml found") {
        Err(format!(
            "Rust sources without Cargo.toml must fail the manifest check:\n{stdout}"
        ))
    } else {
        Ok(())
    };

    ignore_remove_dir_all(&workspace);
    manifest_result.and(source_result).and(sources_result)
}

#[test]
fn doctor_reports_loaded_config_path() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(
        workspace.join("ripr.toml"),
        "[analysis]\nmode = \"deep\"\n\n[lsp]\nseam_diagnostics = true\n",
    )
    .map_err(|e| format!("write ripr.toml: {e}"))?;

    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config: loaded ripr.toml"));
    assert!(stdout.contains("Config path:"));
    assert!(stdout.contains("ripr.toml"));
    assert!(stdout.contains("Analysis mode default: deep"));
    assert!(stdout.contains("LSP seam diagnostics default: true"));
    assert!(!stdout.contains("mode = \"deep\""));
    assert!(!stdout.contains("seam_diagnostics"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_malformed_config_error() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join("ripr.toml"), "[analysis]\nmode = \"slow\"\n")
        .map_err(|e| format!("write ripr.toml: {e}"))?;

    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert!(
        !output.status.success(),
        "malformed config should fail doctor\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config: invalid ripr.toml"));
    assert!(stdout.contains("ripr.toml"));
    assert!(stdout.contains("analysis.mode `slow` is not supported"));
    assert!(!stdout.contains("mode = \"slow\""));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_language_tiers_and_limitations() -> Result<(), String> {
    // A workspace with only Rust markers (Cargo.toml + src/lib.rs).
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Section 1: Detected languages — rust present with (stable) tier.
    assert!(
        stdout.contains("Detected languages:"),
        "expected 'Detected languages:' in stdout:\n{stdout}"
    );
    // The line that contains "Detected languages:" must also contain "rust"
    // and "(stable)".
    let detected_line = stdout
        .lines()
        .find(|l| l.contains("Detected languages:"))
        .unwrap_or("");
    assert!(
        detected_line.contains("rust"),
        "expected 'rust' on the Detected languages line:\n{detected_line}"
    );
    assert!(
        detected_line.contains("(stable)"),
        "expected '(stable)' on the Detected languages line:\n{detected_line}"
    );

    // Anti-overclaim: a Rust-only workspace must NOT list typescript as detected.
    assert!(
        !detected_line.contains("typescript"),
        "must not list typescript when no TS markers are present:\n{detected_line}"
    );

    // Section 3: Known limitations.
    assert!(
        stdout.contains("Known limitations:"),
        "expected 'Known limitations:' in stdout:\n{stdout}"
    );
    // TypeScript preview line.
    assert!(
        stdout.contains("TypeScript/JavaScript/Bun analysis is preview"),
        "expected TypeScript/JavaScript/Bun preview line in stdout:\n{stdout}"
    );
    // Cross-language fail-closed line.
    assert!(
        stdout.contains("cross_language_oracle_visibility_unresolved"),
        "expected cross_language_oracle_visibility_unresolved in stdout:\n{stdout}"
    );

    // Section 4: Recommended first command. The exact wording is
    // worktree-state-aware (see doctor_recommends_worktree_check_on_dirty_worktree);
    // here we only require the diff-first command to be present.
    assert!(
        stdout.contains("Recommended first command: ripr check"),
        "expected the diff-first recommended command in stdout:\n{stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_reports_perl_preview_section_when_perl_markers_present() -> Result<(), String> {
    // A workspace with Perl markers (Makefile.PL + lib/*.pm + t/*.t) must
    // surface the rich "Perl preview:" section (Campaign 31 item 5):
    // project counts, adapter, producer, perllsp, schema, test roots,
    // frameworks, runners, and an exact next command.
    let root = unique_temp_workspace("doctor-perl-preview");
    std::fs::create_dir_all(root.join("lib")).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(root.join("t")).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Makefile.PL"),
        "use ExtUtils::MakeMaker;\nWriteMakefile(NAME => 'Pricing');\n",
    )
    .map_err(|err| err.to_string())?;
    // A minimal Cargo.toml so the doctor's root check passes (the realistic
    // scenario is a mixed Rust+Perl repo; a pure-Perl repo would fail the
    // Cargo.toml check, which is unrelated to the Perl preview).
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mixed-perl\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("lib/Pricing.pm"),
        "package Pricing;\nuse strict;\nsub discount { return 0; }\n1;\n",
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("t/pricing.t"),
        "use Test::More;\nok(1, 'placeholder');\ndone_testing();\n",
    )
    .map_err(|err| err.to_string())?;

    let root_str = root.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root_str]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The Perl preview section heading.
    assert!(
        stdout.contains("- Perl preview:"),
        "expected '- Perl preview:' in stdout:\n{stdout}"
    );
    // Each sub-line of the rich preview.
    assert!(
        stdout.contains("project:"),
        "expected 'project:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("adapter:"),
        "expected 'adapter:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("producer:"),
        "expected 'producer:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("exporter:"),
        "expected 'exporter:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("schema:"),
        "expected 'schema:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("test roots:"),
        "expected 'test roots:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("frameworks:"),
        "expected 'frameworks:' line:\n{stdout}"
    );
    assert!(
        stdout.contains("runners:"),
        "expected 'runners:' line:\n{stdout}"
    );
    assert!(stdout.contains("next:"), "expected 'next:' line:\n{stdout}");

    // Schema reports the expected version (single-sourced from app::PERL_FACT_PACKET_SCHEMA).
    assert!(
        stdout.contains("ripr-perl-facts-v1 expected"),
        "expected 'ripr-perl-facts-v1 expected' on the schema line:\n{stdout}"
    );
    // Test framework detection: Test::More is present in t/pricing.t.
    assert!(
        stdout.contains("Test::More"),
        "expected 'Test::More' detected:\n{stdout}"
    );
    // Test root detection: t/ is present.
    assert!(
        stdout.contains("t/ detected"),
        "expected 't/ detected' on test roots line:\n{stdout}"
    );
    // The recursive count_files now reports the real .pm/.pl/.t counts (> 0).
    let project_line = stdout
        .lines()
        .find(|l| l.contains("project:"))
        .unwrap_or("");
    assert!(
        !project_line.contains("1 .pm, 0 .pl, 0 .t") || project_line.contains("1 .pm, 0 .pl, 1 .t"),
        "project counts must reflect the recursive scan (1 .pm, 1 .t): {project_line}"
    );

    ignore_remove_dir_all(&root);
    Ok(())
}

/// Build a mixed Rust+Perl workspace and a shim directory holding a
/// `perllsp` stub with the given shell body, plus a `perl-ripr-facts` stub
/// that fails `--version` so a host-installed canonical exporter cannot
/// shadow the scenario. Returns (workspace, shim_dir).
#[cfg(unix)]
fn perl_doctor_workspace_with_exporter_stub(
    label: &str,
    perllsp_body: &str,
) -> Result<(PathBuf, PathBuf), String> {
    use std::os::unix::fs::PermissionsExt;
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(root.join("lib")).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mixed-perl\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(root.join("Makefile.PL"), "use ExtUtils::MakeMaker;\n")
        .map_err(|err| err.to_string())?;
    std::fs::write(root.join("lib/Pricing.pm"), "package Pricing;\n1;\n")
        .map_err(|err| err.to_string())?;
    let shim_dir = root.join("exporter-shims");
    std::fs::create_dir_all(&shim_dir).map_err(|err| err.to_string())?;
    for (name, body) in [
        ("perl-ripr-facts", "#!/bin/sh\nexit 127\n"),
        ("perllsp", perllsp_body),
    ] {
        let path = shim_dir.join(name);
        std::fs::write(&path, body).map_err(|err| format!("write {name} stub: {err}"))?;
        let mut permissions = std::fs::metadata(&path)
            .map_err(|err| format!("stat {name} stub: {err}"))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions)
            .map_err(|err| format!("chmod {name} stub: {err}"))?;
    }
    Ok((root, shim_dir))
}

#[cfg(unix)]
fn run_doctor_with_shims_first(root: &Path, shim_dir: &Path) -> Result<Output, String> {
    let mut entries = vec![shim_dir.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&path));
    }
    let search_path =
        std::env::join_paths(entries).map_err(|err| format!("build shim-first PATH: {err}"))?;
    let search_path = search_path.to_string_lossy().into_owned();
    let root_str = root.display().to_string();
    run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        root,
        &["doctor", "--root", &root_str],
        &[("PATH", &search_path)],
    )
    .map_err(|err| format!("run doctor: {err}"))
}

#[test]
#[cfg(unix)]
fn doctor_reports_version_only_perl_exporter_as_incompatible() -> Result<(), String> {
    // The published perllsp 0.17.0 answers `--version` but rejects the
    // managed `ripr-facts` argv with a usage error. Doctor must not call it
    // a found exporter or route the user to `ripr check` through it, and
    // the Perl section must stay advisory (doctor still succeeds).
    let (root, shim_dir) = perl_doctor_workspace_with_exporter_stub(
        "doctor-perl-incompatible-exporter",
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'perllsp 0.17.0'; exit 0; fi\necho 'error: unexpected argument' >&2\nexit 1\n",
    )?;
    let output = run_doctor_with_shims_first(&root, &shim_dir)?;
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exporter_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("exporter:"))
        .unwrap_or("");
    assert!(
        exporter_line.contains("perllsp 0.17.0")
            && exporter_line.contains("does not accept `ripr-facts`")
            && exporter_line.contains("not a compatible exporter"),
        "version-only exporter must be reported incompatible: {exporter_line}\n{stdout}"
    );
    assert!(
        !stdout.contains("exporter: found at") && !stdout.contains("exporter: compatible"),
        "incompatible exporter must not read as found/working:\n{stdout}"
    );
    assert!(
        !stdout.contains("install perllsp"),
        "doctor must not recommend installing perllsp:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
#[cfg(unix)]
fn doctor_reports_ripr_facts_capable_exporter_as_compatible() -> Result<(), String> {
    // Discriminating control for the incompatible case: the same `perllsp`
    // name, but the stub accepts `ripr-facts --help` and documents
    // `--schema`, so the probe must report it compatible.
    let (root, shim_dir) = perl_doctor_workspace_with_exporter_stub(
        "doctor-perl-compatible-exporter",
        "#!/bin/sh\ncase \"$1\" in\n  --version) echo 'perllsp 9.9.9' ;;\n  ripr-facts) echo 'Usage: perllsp ripr-facts --schema <SCHEMA> --root <ROOT> --out <OUT>' ;;\n  *) exit 2 ;;\nesac\n",
    )?;
    let output = run_doctor_with_shims_first(&root, &shim_dir)?;
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exporter_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("exporter:"))
        .unwrap_or("");
    assert!(
        exporter_line.contains("exporter: compatible")
            && exporter_line.contains("perllsp 9.9.9")
            && exporter_line.contains("exporter-shims/perllsp"),
        "ripr-facts-capable exporter must be reported compatible: {exporter_line}\n{stdout}"
    );
    assert!(
        !stdout.contains("not a compatible exporter"),
        "compatible control must not be reported incompatible:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn doctor_omits_perl_preview_when_no_perl_markers() -> Result<(), String> {
    // A Rust-only workspace must NOT emit a Perl preview section.
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("- Perl preview:"),
        "must not emit a Perl preview for a Rust-only workspace:\n{stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_outside_git_or_on_a_missing_root_recommends_a_command_that_can_run() -> Result<(), String>
{
    // #4531: doctor named the non-Git root, then recommended `ripr check`
    // (which cannot run there) after a raw `git status` failure on stderr.
    // A missing root also blamed cargo and rustc for the failed spawn.
    let outside = unique_external_workspace("doctor-outside-git")?;
    std::fs::create_dir_all(outside.join("src")).map_err(|err| err.to_string())?;
    std::fs::write(
        outside.join("Cargo.toml"),
        "[package]\nname = \"outside\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    let root = outside.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The recommendation names the physical root (#5010): the canonicalized
    // root minus its Windows verbatim prefix, so the pasted command analyzes
    // the diagnosed directory without an `//?/` spelling a paste would not
    // resolve. `first_command_at` reproduces the same quoting.
    let physical = root.strip_prefix(r"\\?\").unwrap_or(&root).to_string();
    let result = if !stdout.contains(&format!(
        "- Recommended first command: fix the Git check above, or scan without Git history: `{}`",
        first_command_at(&physical, " --format repo-exposure-md")
    )) || stdout.contains("- Recommended first command: ripr check\n")
        || stderr.contains("working-tree change probe failed")
    {
        Err(format!(
            "a non-Git root must get a runnable first command and no raw probe failure\nstdout:\n{stdout}\nstderr:\n{stderr}"
        ))
    } else {
        Ok(())
    };
    ignore_remove_dir_all(&outside);
    result?;

    let missing = unique_external_workspace("doctor-missing-root")?;
    let root = missing.display().to_string();
    let output = run_ripr(&["doctor", "--root", &root]);
    assert_failure(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // #5010 keeps a runnable recovery command for a missing root, naming its
    // lossless physical spelling; #4531's residual is that cargo and rustc
    // are skipped instead of blamed, and no raw work-tree probe failure is
    // printed on stderr.
    let physical = root.strip_prefix(r"\\?\").unwrap_or(&root).to_string();
    if !stdout.contains("- cargo check skipped: the root directory does not exist")
        || !stdout.contains("- rustc check skipped: the root directory does not exist")
        || stdout.contains("not available")
        || !stdout.contains(&format!(
            "- Recommended first command: {}",
            first_command_at(&physical, "")
        ))
        || !stdout.contains("- The selected root does not exist; rerun with `--root <path>` naming an existing repository directory")
        || stderr.contains("working-tree change probe failed")
    {
        return Err(format!(
            "a missing root must not blame the tools\nstdout:\n{stdout}\nstderr:\n{stderr}"
        ));
    }
    Ok(())
}

#[test]
fn doctor_recommends_worktree_check_on_dirty_worktree() -> Result<(), String> {
    // First-run honesty: doctor must not route a user with uncommitted edits to
    // `ripr check --base origin/main`, which analyzes committed history only and
    // would silently exclude their draft (the RIPR-SPEC-0112 dirty-worktree case).
    let root = unique_temp_workspace("doctor-dirty-wt");
    std::fs::create_dir_all(root.join("src")).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"doctor-dirty-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn f(a: i32) -> i32 { a + 1 }\n",
    )
    .map_err(|err| err.to_string())?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;
    let root_str = root.display().to_string();

    // CLEAN worktree: recommend the diff-first command directly.
    let clean = run_ripr(&["doctor", "--root", &root_str]);
    assert_success(&clean);
    let clean_out = String::from_utf8_lossy(&clean.stdout);
    assert_eq!(
        recommended_first_command_line(&clean_out),
        Some(first_command_at(&root_str, "").as_str()),
        "clean worktree must recommend the diff-first command directly:\n{clean_out}"
    );
    // A relative `--root` is bound to doctor's directory (#4890), so the
    // printed command still names this repository after a `cd`.
    let parent = root
        .parent()
        .ok_or_else(|| format!("{} has no parent", root.display()))?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 name", root.display()))?;
    let relative = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(parent),
        &["doctor", "--root", name],
    )
    .map_err(|err| err.to_string())?;
    assert_success(&relative);
    let relative_out = String::from_utf8_lossy(&relative.stdout);
    assert_eq!(
        recommended_first_command_line(&relative_out),
        Some(first_command_at(&root_str, "").as_str()),
        "a relative --root must print the bound root:\n{relative_out}"
    );
    // The base is resolved, not asserted: `origin/main` does not exist in a
    // repository whose default branch is not `main`, and this fixture has no
    // remote at all.
    assert!(
        !clean_out.contains("Recommended first command: ripr check --base origin/main"),
        "the recommendation must not hardcode a base this repository may not have:\n{clean_out}"
    );

    // DIRTY worktree: route the user to the explicit live-worktree diff.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn f(a: i32) -> i32 { a + 2 }\n",
    )
    .map_err(|err| err.to_string())?;
    let dirty = run_ripr(&["doctor", "--root", &root_str]);
    assert_success(&dirty);
    let dirty_out = String::from_utf8_lossy(&dirty.stdout);
    assert_eq!(
        recommended_first_command_line(&dirty_out),
        Some(first_command_at(&root_str, " --base HEAD --worktree").as_str()),
        "dirty worktree must recommend the worktree command:\n{dirty_out}"
    );
    assert!(
        dirty_out.contains("staged and unstaged tracked edits"),
        "dirty worktree must disclose the tracked-edit scope:\n{dirty_out}"
    );
    assert_ne!(
        recommended_first_command_line(&dirty_out),
        Some(first_command_at(&root_str, "").as_str()),
        "dirty worktree must NOT give the unconditional clean recommendation:\n{dirty_out}"
    );

    ignore_remove_dir_all(&root);
    Ok(())
}

fn run_ripr_without_git(args: &[&str]) -> Output {
    // Empty PATH is the distroless case (#4735). The binary is invoked by
    // absolute path, so the child can start; it cannot find `git`.
    run_ripr_with_env(args, &[("PATH", "")])
}

fn recommended_first_command_line(stdout: &str) -> Option<&str> {
    stdout.lines().find_map(|line| {
        line.trim_end()
            .strip_prefix("- Recommended first command: ")
    })
}

/// Doctor's recommended command after `doctor --root <root>` run from another
/// directory: `ripr check` defaults to `.`, so the root is named, quoted the
/// way `agent::loop_commands::shell_arg` quotes it. `root` is absolute; on
/// Windows command paths render with `/` separators.
fn first_command_at(root: &str, flags: &str) -> String {
    let root = if cfg!(windows) {
        root.replace('\\', "/")
    } else {
        root.to_string()
    };
    let quoted = if !root.is_empty()
        && root
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-' | ':'))
    {
        root.to_string()
    } else {
        format!("'{}'", root.replace('\'', r"'\''"))
    };
    format!("ripr check --root {quoted}{flags}")
}

#[test]
fn check_without_git_names_path_and_diff_routes_without_dumping_argv() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr_without_git(&["check", "--root", &root, "--base", "HEAD"]);
    assert_failure(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing git stays a Failure (exit 2); stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("git was not found on PATH"),
        "stderr must name the missing binary:\n{stderr}"
    );
    assert!(
        stderr.contains("`--diff PATH`") && stderr.contains("`--diff -`"),
        "stderr must name both saved-diff routes:\n{stderr}"
    );
    assert!(
        stderr.contains("install git"),
        "stderr must name the install route:\n{stderr}"
    );
    assert!(
        !stderr.contains("core.quotePath") && !stderr.contains("[\"-c\""),
        "git argv must not leak into stderr:\n{stderr}"
    );

    let diff = workspace.join("change.diff");
    std::fs::write(
        &diff,
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn placeholder() {}\n+pub fn placeholder() { let _ = 1; }\n",
    )
    .map_err(|error| format!("write saved diff: {error}"))?;
    let diff_arg = diff.display().to_string();
    let via_diff = run_ripr_without_git(&["check", "--root", &root, "--diff", &diff_arg]);
    let via_diff_stderr = String::from_utf8_lossy(&via_diff.stderr);
    assert!(
        !via_diff_stderr.contains("git was not found on PATH"),
        "--diff must not demand git; stderr:\n{via_diff_stderr}"
    );
    assert!(
        via_diff.status.success() || via_diff.status.code() == Some(3),
        "--diff without git must complete analysis (0) or a decision (3), not an operational failure; status={:?}\nstdout:\n{}\nstderr:\n{}",
        via_diff.status,
        String::from_utf8_lossy(&via_diff.stdout),
        via_diff_stderr
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_without_git_omitted_base_names_path_not_unresolvable_base() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr_without_git(&["check", "--root", &root]);
    assert_failure(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing git stays a Failure (exit 2); stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("git was not found on PATH"),
        "zero-config check must name the missing binary:\n{stderr}"
    );
    assert!(
        stderr.contains("`--diff PATH`") && stderr.contains("`--diff -`"),
        "zero-config check must name both saved-diff routes:\n{stderr}"
    );
    assert!(
        !stderr.contains("could not resolve a default base"),
        "missing git must not be diagnosed as an unresolved default base:\n{stderr}"
    );
    assert!(
        !stderr.contains("Pass `--base"),
        "missing git must not send the user to `--base`:\n{stderr}"
    );
    assert!(
        !stderr.contains("core.quotePath") && !stderr.contains("[\"-c\""),
        "git argv must not leak into stderr:\n{stderr}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_without_git_names_the_fix_and_recommends_the_diff_route() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr_without_git(&["doctor", "--root", &root]);
    assert_failure(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "doctor without git stays exit 2; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let git_line = stdout
        .lines()
        .find(|line| {
            line.contains("git was not found on PATH") || line.contains("git not available")
        })
        .unwrap_or("");
    assert!(
        git_line.starts_with('!'),
        "missing git must be a failing ! line:\n{stdout}"
    );
    assert!(
        git_line.contains("git was not found on PATH")
            && git_line.contains("install git")
            && git_line.contains("`--diff PATH`")
            && git_line.contains("`--diff -`"),
        "the ! line must name the same fix as check:\n{git_line}"
    );
    assert!(
        !git_line.contains("core.quotePath") && !git_line.contains("[\"-c\""),
        "git argv must not appear on the doctor ! line:\n{git_line}"
    );
    assert_eq!(
        recommended_first_command_line(&stdout),
        Some(first_command_at(&root, " --diff PATH").as_str()),
        "doctor must not recommend a git-backed check:\n{stdout}"
    );
    assert!(
        !stdout.contains("Recommended first command: ripr check --base HEAD --worktree"),
        "a gitless doctor must not recommend --worktree:\n{stdout}"
    );
    assert!(
        stdout.contains("each `!` line above names the check and its fix"),
        "failed-doctor footer must remain:\n{stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn doctor_without_git_does_not_recommend_worktree_on_a_dirty_tree() -> Result<(), String> {
    let root = unique_temp_workspace("doctor-dirty-no-git");
    std::fs::create_dir_all(root.join("src")).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"doctor-dirty-no-git\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn f(a: i32) -> i32 { a + 1 }\n",
    )
    .map_err(|err| err.to_string())?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn f(a: i32) -> i32 { a + 2 }\n",
    )
    .map_err(|err| err.to_string())?;

    let root_str = root.display().to_string();
    let with_git = run_ripr(&["doctor", "--root", &root_str]);
    assert_success(&with_git);
    assert_eq!(
        recommended_first_command_line(&String::from_utf8_lossy(&with_git.stdout)),
        Some(first_command_at(&root_str, " --base HEAD --worktree").as_str()),
        "control: dirty tree with git still recommends --worktree:\n{}",
        String::from_utf8_lossy(&with_git.stdout)
    );

    let without_git = run_ripr_without_git(&["doctor", "--root", &root_str]);
    assert_failure(&without_git);
    let stdout = String::from_utf8_lossy(&without_git.stdout);
    assert_eq!(
        recommended_first_command_line(&stdout),
        Some(first_command_at(&root_str, " --diff PATH").as_str()),
        "dirty tree cannot win over a missing git binary:\n{stdout}"
    );
    assert!(
        !stdout.contains("Recommended first command: ripr check --base HEAD --worktree"),
        "gitless doctor must not recommend --worktree:\n{stdout}"
    );

    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn init_writes_conservative_config_and_doctor_loads_it() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root]);
    assert_success(&output);

    let config_path = workspace.join("ripr.toml");
    let config = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("read generated ripr.toml: {e}"))?;
    assert!(config.contains("mode = \"draft\""));
    assert!(config.contains("include_unchanged_tests = true"));
    assert!(config.contains("weakly_gripped = \"warning\""));
    assert!(config.contains("strongly_gripped = \"off\""));
    assert!(config.contains("intentional = \"off\""));
    assert!(config.contains("suppressed = \"off\""));
    assert!(config.contains("seam_diagnostics = true"));
    assert!(config.contains("max_related_tests = 5"));

    let doctor = run_ripr(&["doctor", "--root", &root]);
    assert_success(&doctor);
    let stdout = String::from_utf8_lossy(&doctor.stdout);
    assert!(stdout.contains("Config: loaded ripr.toml"));
    assert!(stdout.contains("Analysis mode default: draft"));
    assert!(stdout.contains("LSP seam diagnostics default: true"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_dry_run_prints_config_without_writing() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root, "--dry-run"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[analysis]"));
    assert!(stdout.contains("mode = \"draft\""));
    assert!(stdout.contains("seam_diagnostics = true"));
    // #2572: the plan names the target path and the action, and says the run
    // was a preview. Printing the body alone left the reader guessing which
    // path it was for and whether anything had been written.
    assert!(stdout.contains("ripr init plan (dry run — nothing was written)"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("ripr.toml"));
    assert!(stdout.contains("Rerun without --dry-run to apply."));
    assert!(!workspace.join("ripr.toml").exists());

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #4378: on Windows the init preview mixed a forward-slash argv prefix with the
/// joined `\ripr.toml` suffix, and doctor mixed `.\`-relative lines with a
/// verbatim `\\?\` executable. Both surfaces render one slash convention.
#[cfg(windows)]
#[test]
fn windows_init_preview_and_doctor_use_one_separator_convention() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace
        .canonicalize()
        .map_err(|e| format!("canonicalize workspace: {e}"))?
        .display()
        .to_string();
    // Canonical Windows paths carry the verbatim prefix; spell the root the
    // way a user types it, with forward slashes.
    let root = root.trim_start_matches(r"\\?\").replace('\\', "/");
    let dry = run_ripr(&["init", "--root", &root, "--dry-run"]);
    assert_success(&dry);
    let dry_stdout = String::from_utf8_lossy(&dry.stdout);
    let expected = format!("{root}/ripr.toml");
    assert!(
        dry_stdout.contains(&format!("# {expected}")),
        "{dry_stdout}"
    );
    assert!(!dry_stdout.contains('\\'), "{dry_stdout}");

    let doctor = run_ripr(&["doctor", "--root", &root]);
    let doctor_stdout = String::from_utf8_lossy(&doctor.stdout);
    for line in doctor_stdout.lines().filter(|line| {
        line.starts_with("- root:")
            || line.starts_with("- ripr binary:")
            || line.starts_with("- Cache location:")
            || line.contains("Cargo.toml found at")
            || line.contains("root directory exists at")
    }) {
        assert!(!line.contains(r"\\?\"), "verbatim prefix in `{line}`");
        assert!(!line.contains('\\'), "backslash separator in `{line}`");
    }

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #2572: `--dry-run` must predict the run it previews. An existing
/// `ripr.toml` without `--force` makes the real run fail, so the dry run
/// fails the same way instead of printing a config it could not write.
#[test]
fn init_dry_run_fails_like_the_real_run_when_config_exists_without_force() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
        .map_err(|e| format!("write existing ripr.toml: {e}"))?;
    let root = workspace.display().to_string();

    let dry = run_ripr(&["init", "--root", &root, "--dry-run"]);
    let real = run_ripr(&["init", "--root", &root]);

    assert!(
        !dry.status.success(),
        "dry run should fail when the real run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!real.status.success());
    assert_eq!(
        String::from_utf8_lossy(&dry.stderr),
        String::from_utf8_lossy(&real.stderr),
        "dry run and real run must report the same blocker"
    );
    assert!(String::from_utf8_lossy(&dry.stderr).contains("already exists"));
    assert!(String::from_utf8_lossy(&dry.stderr).contains("--force"));

    // The pre-existing config is untouched by either invocation.
    let config = std::fs::read_to_string(workspace.join("ripr.toml"))
        .map_err(|e| format!("read existing ripr.toml: {e}"))?;
    assert!(config.contains("mode = \"deep\""));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #2576 review: a parent that cannot be created is a blocker the plan has to
/// catch. With `<root>/.github` as a regular file, nothing exists at the
/// workflow path, so an existence-only check reads it as `create` — the dry
/// run then reports success while the real run writes `ripr.toml` and only
/// then fails, leaving the repo half-initialized.
#[test]
fn init_dry_run_fails_like_the_real_run_when_workflow_parent_is_a_file() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join(".github"), "not a directory\n")
        .map_err(|e| format!("write .github as a file: {e}"))?;
    let root = workspace.display().to_string();

    let dry = run_ripr(&["init", "--root", &root, "--ci", "github", "--dry-run"]);
    let real = run_ripr(&["init", "--root", &root, "--ci", "github"]);

    assert!(
        !dry.status.success(),
        "dry run should fail when the real run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!real.status.success());
    assert_eq!(
        String::from_utf8_lossy(&dry.stderr),
        String::from_utf8_lossy(&real.stderr),
        "dry run and real run must report the same blocker"
    );
    assert!(
        String::from_utf8_lossy(&dry.stderr).contains("exists and is not a directory"),
        "stderr:\n{}",
        String::from_utf8_lossy(&dry.stderr)
    );

    // The failing run must not have half-initialized the workspace.
    assert!(
        !workspace.join("ripr.toml").exists(),
        "the real run wrote ripr.toml before failing"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #2576 review: `create_new` refuses any occupied path, including a dangling
/// symlink that `Path::exists()` reports as absent. Planning must ask the same
/// question the write asks.
#[cfg(unix)]
#[test]
fn init_dry_run_fails_like_the_real_run_for_a_dangling_symlink_target() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::os::unix::fs::symlink("/nonexistent-ripr-init-target", workspace.join("ripr.toml"))
        .map_err(|e| format!("create dangling symlink: {e}"))?;
    let root = workspace.display().to_string();

    let dry = run_ripr(&["init", "--root", &root, "--dry-run"]);
    let real = run_ripr(&["init", "--root", &root]);

    assert!(
        !dry.status.success(),
        "dry run should fail when the real run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!real.status.success());
    assert_eq!(
        String::from_utf8_lossy(&dry.stderr),
        String::from_utf8_lossy(&real.stderr),
        "dry run and real run must report the same blocker"
    );
    assert!(
        String::from_utf8_lossy(&dry.stderr).contains("already exists"),
        "stderr:\n{}",
        String::from_utf8_lossy(&dry.stderr)
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// #2572: the same agreement property for a root that is not a directory.
#[test]
fn init_dry_run_fails_like_the_real_run_when_root_is_not_a_directory() -> Result<(), String> {
    let missing = "/nonexistent-ripr-init-root/xyz";

    let dry = run_ripr(&["init", "--root", missing, "--dry-run"]);
    let real = run_ripr(&["init", "--root", missing]);

    assert!(
        !dry.status.success(),
        "dry run should fail when the real run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!real.status.success());
    assert_eq!(
        String::from_utf8_lossy(&dry.stderr),
        String::from_utf8_lossy(&real.stderr),
        "dry run and real run must report the same blocker"
    );
    assert!(
        String::from_utf8_lossy(&dry.stderr).contains("is not a directory"),
        "stderr:\n{}",
        String::from_utf8_lossy(&dry.stderr)
    );

    Ok(())
}

/// #2572: when the config exists but `--ci` still has work, the plan reports
/// `leave existing` for the config and `create` for the workflow — matching
/// what the real run then prints.
#[test]
fn init_dry_run_plan_reports_leave_existing_for_untouched_config() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
        .map_err(|e| format!("write existing ripr.toml: {e}"))?;
    let root = workspace.display().to_string();

    let output = run_ripr(&["init", "--root", &root, "--ci", "github", "--dry-run"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("leave existing"), "stdout:\n{stdout}");
    assert!(stdout.contains("create"), "stdout:\n{stdout}");
    assert!(stdout.contains("ripr.yml"), "stdout:\n{stdout}");
    // The untouched config's body is not reprinted — there is nothing to review.
    assert!(
        !stdout.contains("seam_diagnostics = true"),
        "stdout:\n{stdout}"
    );
    assert!(!workspace.join(".github/workflows/ripr.yml").exists());

    // The real run agrees with the plan.
    let real = run_ripr(&["init", "--root", &root, "--ci", "github"]);
    assert_success(&real);
    let real_stdout = String::from_utf8_lossy(&real.stdout);
    assert!(real_stdout.contains("Left existing"));
    assert!(real_stdout.contains("Wrote"));
    assert!(workspace.join(".github/workflows/ripr.yml").exists());

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_ci_github_dry_run_prints_config_and_workflow_without_writing() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root, "--ci", "github", "--dry-run"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // #2572: both body headers now carry the full target path, so the reader
    // can tell which file each block would be written to. The config header
    // previously showed the bare file name while the workflow header showed a
    // full path.
    assert!(stdout.contains("ripr init plan (dry run — nothing was written)"));
    // #4378: on Windows the headers use one separator (forward slashes).
    let shown = |path: PathBuf| {
        let text = path.display().to_string();
        if cfg!(windows) {
            text.replace('\\', "/")
        } else {
            text
        }
    };
    assert!(stdout.contains(&format!("# {}", shown(workspace.join("ripr.toml")))));
    assert!(stdout.contains(&format!(
        "# {}",
        shown(workspace.join(".github/workflows/ripr.yml"))
    )));
    assert!(stdout.contains("Rerun without --dry-run to apply."));
    assert!(stdout.contains(".github"));
    assert!(stdout.contains("RIPR advisory reports"));
    assert!(stdout.contains("continue-on-error: true"));
    assert!(stdout.contains("RIPR_UPLOAD_SARIF"));
    assert!(stdout.contains("actions/upload-artifact@v7"));
    assert!(stdout.contains("target/ripr/agent"));
    assert!(stdout.contains("target/ripr/workflow"));
    assert!(stdout.contains("target/ripr/review"));
    assert!(stdout.contains("RIPR advisory summary"));
    assert!(stdout.contains("target/ripr/review/comments.json"));
    assert!(stdout.contains("ripr agent start"));
    // #3906: CI writes only the before side of the repair loop.
    assert!(!stdout.contains("ripr agent receipt"));
    assert!(stdout.contains("ripr agent status"));
    assert!(stdout.contains("ripr agent review-summary"));
    assert!(stdout.contains("target/ripr/workflow/agent-status.md"));
    assert!(stdout.contains("target/ripr/workflow/agent-review-summary.md"));
    assert!(stdout.contains("#### First-run status"));
    assert!(stdout.contains("Start-here artifact:"));
    assert!(stdout.contains("missing_start_here"));
    assert!(stdout.contains("cat target/ripr/reports/start-here.md"));
    assert!(stdout.contains("### Language preview grouping"));
    assert!(stdout.contains("github/codeql-action/upload-sarif@v4"));
    assert!(!workspace.join("ripr.toml").exists());
    assert!(!workspace.join(".github/workflows/ripr.yml").exists());

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_refuses_existing_config_without_force() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
        .map_err(|e| format!("write existing ripr.toml: {e}"))?;

    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root]);
    assert!(
        !output.status.success(),
        "init should refuse to overwrite without --force\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
    assert!(stderr.contains("--force"));
    let config = std::fs::read_to_string(workspace.join("ripr.toml"))
        .map_err(|e| format!("read existing ripr.toml: {e}"))?;
    assert!(config.contains("mode = \"deep\""));
    assert!(!config.contains("seam_diagnostics = true"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_ci_github_writes_non_blocking_report_workflow() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root, "--ci", "github"]);
    assert_success(&output);

    let workflow_path = workspace.join(".github/workflows/ripr.yml");
    let workflow = std::fs::read_to_string(&workflow_path)
        .map_err(|e| format!("read generated workflow: {e}"))?;
    assert!(workspace.join("ripr.toml").exists());
    assert!(workflow.contains("pull_request:"));
    assert!(workflow.contains("workflow_dispatch:"));
    // The steps use the generating version's CLI, so the install is pinned
    // to it rather than taking the newest crates.io release.
    assert!(workflow.contains(&format!(
        "run: cargo install ripr --version {} --locked\n",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(!workflow.contains("cargo install ripr --locked"));
    // A newer push cancels the older run of the same PR, so two runs never
    // publish the same inline cards (#4448).
    assert!(workflow.contains(
        "\nconcurrency:\n  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}\n  cancel-in-progress: true\n"
    ));
    // Every third-party action is pinned to a commit SHA (#4452).
    assert!(workflow.contains("dtolnay/rust-toolchain@6bed0761d98439e5a578e2877258200ad565ba87"));
    assert!(!workflow.contains("dtolnay/rust-toolchain@stable"));
    assert!(workflow.contains("ripr pilot"));
    assert!(workflow.contains("--format sarif"));
    assert!(workflow.contains("--format repo-sarif"));
    assert!(workflow.contains("--format repo-badge-json"));
    assert!(workflow.contains("ripr agent start"));
    assert!(workflow.contains("ripr agent packet"));
    // #3906 (F60-1): CI has no test edit between snapshots, so it runs no
    // verify, receipt, or outcome; the repair's after phase writes those.
    assert!(!workflow.contains("ripr agent receipt"));
    assert!(!workflow.contains("ripr outcome"));
    assert!(!workflow.contains("> target/ripr/workflow/agent-verify.json"));
    assert!(workflow.contains("ripr review-comments"));
    assert!(workflow.contains("RIPR_COMMENT_MODE"));
    assert!(workflow.contains("pr-comments plan"));
    assert!(workflow.contains("target/ripr/review/comment-publish-plan.json"));
    assert!(workflow.contains("Capture existing RIPR inline comments"));
    assert!(workflow.contains("Plan RIPR inline comments"));
    assert!(workflow.contains("Publish RIPR inline comments"));
    assert!(workflow.contains("ripr agent status"));
    assert!(workflow.contains("ripr agent review-summary"));
    assert!(workflow.contains("target/ripr/workflow/agent-packet.json"));
    assert!(workflow.contains("target/ripr/workflow/agent-brief.json"));
    assert!(workflow.contains("target/ripr/workflow/agent-verify.json"));
    assert!(workflow.contains("target/ripr/reports/agent-receipt.json"));
    assert!(workflow.contains("target/ripr/workflow/agent-status.json"));
    assert!(workflow.contains("target/ripr/workflow/agent-status.md"));
    assert!(workflow.contains("target/ripr/workflow/agent-review-summary.json"));
    assert!(workflow.contains("target/ripr/workflow/agent-review-summary.md"));
    assert!(workflow.contains("target/ripr/agent/agent-packet.json"));
    assert!(workflow.contains("target/ripr/agent/agent-brief.json"));
    assert!(!workflow.contains("target/ripr/agent/agent-verify.json"));
    assert!(!workflow.contains("target/ripr/agent/agent-receipt.json"));
    assert!(!workflow.contains("target/ripr/reports/targeted-test-outcome.json"));
    assert!(workflow.contains("target/ripr/review"));
    assert!(workflow.contains("target/ripr/review/comments.json"));
    assert!(workflow.contains("Run RIPR PR guidance report"));
    assert!(workflow.contains("Emit RIPR PR guidance annotations"));
    assert!(workflow.contains("Add RIPR advisory summary"));
    assert!(workflow.contains("## RIPR advisory summary"));
    assert!(workflow.contains("### Start here"));
    assert!(workflow.contains("#### First-run status"));
    assert!(workflow.contains("Start-here artifact:"));
    assert!(workflow.contains("missing_start_here"));
    assert!(workflow.contains("cat target/ripr/reports/start-here.md"));
    assert!(workflow.contains("### Language preview grouping"));
    assert!(workflow.contains("### SARIF and badge status"));
    assert!(workflow.contains("### PR guidance annotations"));
    assert!(workflow.contains("### Known limits"));
    assert!(!workflow.contains("cargo xtask"));
    assert!(workflow.contains("continue-on-error: true"));
    assert!(workflow.contains("actions/upload-artifact@v7"));
    assert!(workflow.contains("RIPR_UPLOAD_SARIF"));
    assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
    assert!(!workflow.contains("fail-on-new-warning"));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"acknowledgeable\""));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"baseline-check\""));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"calibrated-gate\""));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_ci_github_refuses_existing_workflow_without_force() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let workflow_dir = workspace.join(".github/workflows");
    std::fs::create_dir_all(&workflow_dir).map_err(|e| format!("create workflow dir: {e}"))?;
    std::fs::write(workflow_dir.join("ripr.yml"), "name: Existing\n")
        .map_err(|e| format!("write existing workflow: {e}"))?;

    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root, "--ci", "github"]);
    assert!(
        !output.status.success(),
        "init should refuse to overwrite workflow without --force\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(".github"));
    assert!(stderr.contains("--force"));
    assert!(!workspace.join("ripr.toml").exists());

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn init_force_overwrites_existing_config() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    std::fs::write(workspace.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
        .map_err(|e| format!("write existing ripr.toml: {e}"))?;

    let root = workspace.display().to_string();
    let output = run_ripr(&["init", "--root", &root, "--force"]);
    assert_success(&output);
    let config = std::fs::read_to_string(workspace.join("ripr.toml"))
        .map_err(|e| format!("read overwritten ripr.toml: {e}"))?;
    assert!(config.contains("mode = \"draft\""));
    assert!(config.contains("seam_diagnostics = true"));
    assert!(!config.contains("mode = \"deep\""));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// Runs `ripr init` with every written file capped at 512 bytes. SIGXFSZ is
/// ignored so the cut-short write returns `EFBIG`, as a full disk returns
/// `ENOSPC`, instead of killing the process.
#[cfg(unix)]
fn run_size_limited_init(root: &str, force: bool) -> Result<Output, String> {
    let mut args = vec![
        "-c",
        "trap '' XFSZ; ulimit -f 1 && exec \"$0\" \"$@\"",
        env!("CARGO_BIN_EXE_ripr"),
        "init",
        "--root",
        root,
    ];
    if force {
        args.push("--force");
    }
    run_command("sh", None, &args).map_err(|e| format!("spawn size-limited init: {e}"))
}

#[cfg(unix)]
#[test]
fn init_that_cannot_finish_writing_keeps_the_previous_config() -> Result<(), String> {
    // #4883: --force removed the old config before writing the new one, so a
    // full disk left a truncated fragment where the user's config had been.
    let workspace = make_temp_workspace(None)?;
    let config_path = workspace.join("ripr.toml");
    let previous = "[analysis]\nmode = \"deep\"\n";
    std::fs::write(&config_path, previous).map_err(|e| format!("seed ripr.toml: {e}"))?;
    let root = workspace.display().to_string();

    let forced = run_size_limited_init(&root, true)?;
    assert!(
        !forced.status.success(),
        "the size-limited --force write must fail: {forced:?}"
    );
    let stdout = String::from_utf8_lossy(&forced.stdout);
    assert!(
        !stdout.contains("Overwrote existing"),
        "a failed overwrite must not be reported as done: {stdout}"
    );
    assert_eq!(
        std::fs::read_to_string(&config_path).map_err(|e| format!("read ripr.toml: {e}"))?,
        previous,
        "a failed --force write must leave the previous config in place"
    );
    let leftovers: Vec<String> = std::fs::read_dir(&workspace)
        .map_err(|e| format!("read workspace: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());

    // Without an existing config, a cut-short write must not leave a
    // fragment that a rerun refuses to replace.
    std::fs::remove_file(&config_path).map_err(|e| format!("remove ripr.toml: {e}"))?;
    let fresh = run_size_limited_init(&root, false)?;
    assert!(
        !fresh.status.success(),
        "the size-limited write must fail: {fresh:?}"
    );
    assert!(
        !config_path.exists(),
        "a cut-short init left a truncated ripr.toml"
    );
    let staged: Vec<String> = std::fs::read_dir(&workspace)
        .map_err(|e| format!("read workspace: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert_eq!(
        staged,
        Vec::<String>::new(),
        "a failed init left its staging file"
    );
    assert_success(&run_ripr(&["init", "--root", &root]));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[cfg(unix)]
#[test]
fn init_force_replaces_a_symlinked_config_without_writing_its_target() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    use std::os::unix::fs::PermissionsExt as _;
    let target = workspace.join("elsewhere.toml");
    std::fs::write(&target, "keep\n").map_err(|e| format!("seed target: {e}"))?;
    // A distinctive mode on the link target: the replacement must not take it
    // on, because the rename replaces the link, not the file it points at.
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o604))
        .map_err(|e| format!("chmod target: {e}"))?;
    let fresh_probe = workspace.join("mode-probe");
    std::fs::write(&fresh_probe, "").map_err(|e| format!("write probe: {e}"))?;
    let default_mode = std::fs::metadata(&fresh_probe)
        .map_err(|e| format!("stat probe: {e}"))?
        .permissions()
        .mode()
        & 0o777;
    let config_path = workspace.join("ripr.toml");
    std::os::unix::fs::symlink(&target, &config_path).map_err(|e| format!("symlink: {e}"))?;

    let root = workspace.display().to_string();
    assert_success(&run_ripr(&["init", "--root", &root, "--force"]));
    assert_eq!(
        std::fs::read_to_string(&target).map_err(|e| format!("read target: {e}"))?,
        "keep\n",
        "--force wrote through the symlink"
    );
    let metadata =
        std::fs::symlink_metadata(&config_path).map_err(|e| format!("stat ripr.toml: {e}"))?;
    assert!(
        metadata.file_type().is_file(),
        "ripr.toml must be a regular file"
    );
    assert_eq!(
        metadata.permissions().mode() & 0o777,
        default_mode,
        "the replacement inherited the symlink target's permissions"
    );
    let config = std::fs::read_to_string(&config_path).map_err(|e| format!("read: {e}"))?;
    assert!(config.contains("mode = \"draft\""));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn baseline_create_writes_reviewed_ledger_and_refuses_overwrite() -> Result<(), String> {
    let workspace = unique_temp_workspace("baseline-create");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let source = workspace_root().join(
        "fixtures/boundary_gap/expected/calibrated-gate/visible-only-advisory/gate-decision.json",
    );
    let out = workspace.join(".ripr/gate-baseline.json");
    let source_arg = source.display().to_string();
    let out_arg = out.display().to_string();

    let output = run_ripr(&[
        "baseline",
        "create",
        "--from",
        &source_arg,
        "--out",
        &out_arg,
    ]);
    assert_success(&output);

    let baseline = std::fs::read_to_string(&out).map_err(|e| format!("read baseline: {e}"))?;
    assert!(baseline.contains("\"kind\": \"gate_baseline\""));
    assert!(baseline.contains("\"reviewed\": false"));
    assert!(baseline.contains("\"source_report\""));
    assert!(baseline.contains("\"seam_id\": \"8f7fa8644fd12280\""));
    assert!(baseline.contains("\"entries\": 1"));

    let overwrite = run_ripr(&[
        "baseline",
        "create",
        "--from",
        &source_arg,
        "--out",
        &out_arg,
    ]);
    assert_failure(&overwrite);
    let stderr = String::from_utf8_lossy(&overwrite.stderr);
    assert!(stderr.contains("--force"));

    let dry_run_out = workspace.join(".ripr/dry-run-baseline.json");
    let dry_run_out_arg = dry_run_out.display().to_string();
    let dry_run = run_ripr(&[
        "baseline",
        "create",
        "--from",
        &source_arg,
        "--out",
        &dry_run_out_arg,
        "--dry-run",
    ]);
    assert_success(&dry_run);
    let stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(stdout.contains("\"kind\": \"gate_baseline\""));
    assert!(!dry_run_out.exists());

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn baseline_diff_writes_debt_delta_json_and_markdown() -> Result<(), String> {
    let workspace = unique_temp_workspace("baseline-diff");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let current = workspace_root().join(
        "fixtures/boundary_gap/expected/calibrated-gate/visible-only-advisory/gate-decision.json",
    );
    let baseline = workspace.join(".ripr/gate-baseline.json");
    let out_json = workspace.join("baseline-debt-delta.json");
    let out_md = workspace.join("baseline-debt-delta.md");
    let current_arg = current.display().to_string();
    let baseline_arg = baseline.display().to_string();
    let out_json_arg = out_json.display().to_string();
    let out_md_arg = out_md.display().to_string();

    let create = run_ripr(&[
        "baseline",
        "create",
        "--from",
        &current_arg,
        "--out",
        &baseline_arg,
    ]);
    assert_success(&create);

    let diff = run_ripr(&[
        "baseline",
        "diff",
        "--baseline",
        &baseline_arg,
        "--current",
        &current_arg,
        "--out",
        &out_json_arg,
        "--out-md",
        &out_md_arg,
    ]);
    assert_success(&diff);

    let json = std::fs::read_to_string(&out_json).map_err(|e| format!("read delta json: {e}"))?;
    assert!(json.contains("\"kind\": \"baseline_debt_delta\""));
    assert!(json.contains("\"still_present\": 1"));
    assert!(json.contains("\"matched_by\": \"canonical_gap_id\""));
    let md = std::fs::read_to_string(&out_md).map_err(|e| format!("read delta md: {e}"))?;
    assert!(md.contains("# RIPR Baseline Debt Delta"));
    assert!(md.contains("| Still present | 1 |"));

    let missing_current = workspace.join("missing-current.json");
    let missing_out = workspace.join("missing-current-delta.json");
    let missing_md = workspace.join("missing-current-delta.md");
    let missing = run_ripr(&[
        "baseline",
        "diff",
        "--baseline",
        &baseline_arg,
        "--current",
        &missing_current.display().to_string(),
        "--out",
        &missing_out.display().to_string(),
        "--out-md",
        &missing_md.display().to_string(),
    ]);
    assert_success(&missing);
    let missing_json =
        std::fs::read_to_string(&missing_out).map_err(|e| format!("read missing delta: {e}"))?;
    assert!(missing_json.contains("\"missing_current_input\": 1"));
    assert!(missing_json.contains("required current gate-decision input"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn capped_pr_guidance_baseline_round_trip_preserves_shared_canonical_gap_seams()
-> Result<(), String> {
    let workspace = unique_temp_workspace("capped-baseline-round-trip");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let guidance =
        workspace_root().join("fixtures/boundary_gap/expected/pr-guidance/capped/comments.json");
    let gate = workspace.join("gate-decision.json");
    let baseline = workspace.join("gate-baseline.json");
    let delta = workspace.join("baseline-debt-delta.json");
    let updated = workspace.join("updated-baseline.json");
    let guidance_arg = guidance.display().to_string();
    let gate_arg = gate.display().to_string();
    let baseline_arg = baseline.display().to_string();
    let delta_arg = delta.display().to_string();
    let updated_arg = updated.display().to_string();

    let gate_output = run_ripr(&[
        "gate",
        "evaluate",
        "--root",
        ".",
        "--pr-guidance",
        &guidance_arg,
        "--mode",
        "visible-only",
        "--out",
        &gate_arg,
    ]);
    assert_success(&gate_output);
    let create = run_ripr(&[
        "baseline",
        "create",
        "--from",
        &gate_arg,
        "--out",
        &baseline_arg,
    ]);
    assert_success(&create);
    let baseline_value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&baseline).map_err(|e| format!("read baseline: {e}"))?,
    )
    .map_err(|e| format!("parse baseline: {e}"))?;
    let entries = baseline_value
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "baseline entries missing".to_string())?;
    let entry_count = entries.len();
    assert!(
        entry_count > 1,
        "capped guidance must retain multiple seams"
    );

    let diff = run_ripr(&[
        "baseline",
        "diff",
        "--baseline",
        &baseline_arg,
        "--current",
        &gate_arg,
        "--out",
        &delta_arg,
    ]);
    assert_success(&diff);
    let delta_value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&delta).map_err(|e| format!("read delta: {e}"))?,
    )
    .map_err(|e| format!("parse delta: {e}"))?;
    let summary = delta_value
        .get("delta")
        .ok_or_else(|| "delta counts missing".to_string())?;
    assert_eq!(
        summary
            .get("still_present")
            .and_then(serde_json::Value::as_u64),
        Some(entry_count as u64)
    );
    assert_eq!(
        summary.get("resolved").and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        summary
            .get("new_policy_eligible")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        summary
            .get("stale_baseline_entry")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );

    let update = run_ripr(&[
        "baseline",
        "update",
        "--baseline",
        &baseline_arg,
        "--current",
        &gate_arg,
        "--remove-resolved",
        "--out",
        &updated_arg,
    ]);
    assert_success(&update);
    let updated_value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&updated).map_err(|e| format!("read updated baseline: {e}"))?,
    )
    .map_err(|e| format!("parse updated baseline: {e}"))?;
    assert_eq!(
        updated_value
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(entry_count)
    );
    assert_eq!(
        updated_value
            .pointer("/update/removed_resolved")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert!(
        updated_value
            .pointer("/update/warnings")
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty),
        "unchanged capped evidence must not produce ambiguous or stale warnings: {updated_value}"
    );
    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn baseline_update_removes_resolved_without_adopting_new_debt() -> Result<(), String> {
    let workspace = unique_temp_workspace("baseline-update");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let fixture_dir =
        workspace_root().join("fixtures/boundary_gap/expected/baseline-debt-delta/mixed");
    let baseline = fixture_dir.join("baseline.json");
    let current = fixture_dir.join("current-gate-decision.json");
    let out = workspace.join(".ripr/gate-baseline.json");
    let baseline_arg = baseline.display().to_string();
    let current_arg = current.display().to_string();
    let out_arg = out.display().to_string();

    let update = run_ripr(&[
        "baseline",
        "update",
        "--baseline",
        &baseline_arg,
        "--current",
        &current_arg,
        "--remove-resolved",
        "--out",
        &out_arg,
    ]);
    assert_success(&update);

    let json = std::fs::read_to_string(&out).map_err(|e| format!("read updated baseline: {e}"))?;
    assert!(json.contains("\"kind\": \"gate_baseline\""));
    assert!(json.contains("\"seam_id\": \"same\""));
    assert!(!json.contains("\"seam_id\": \"gone\""));
    assert!(!json.contains("\"seam_id\": \"new\""));
    assert!(json.contains("\"entries\": 2"));
    assert!(json.contains("\"removed_resolved\": 1"));
    assert!(json.contains("\"ignored_new_current\": 3"));
    assert!(json.contains("preserved malformed baseline entry"));

    let no_mode = run_ripr(&[
        "baseline",
        "update",
        "--baseline",
        &baseline_arg,
        "--current",
        &current_arg,
        "--out",
        &out_arg,
    ]);
    assert_failure(&no_mode);
    let stderr = String::from_utf8_lossy(&no_mode.stderr);
    assert!(stderr.contains("--remove-resolved"));
    assert!(stderr.contains("adopting new debt is not supported"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// The first-PR workflow and every evidence record's `verify_command` name
/// pilot's `repo-exposure.json` as the `--before` of `ripr agent verify`
/// (#3906). Walk that route as a user would: pilot, add the missing boundary
/// test, take the after snapshot with the exact command pilot prints, verify.
#[test]
fn pilot_snapshot_is_the_agent_verify_baseline() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("pilot-verify-baseline");
    std::fs::create_dir_all(&root)?;
    init_producer_fixture_repo(&root)?;
    let root_arg = root.display().to_string();

    // From the repository, as a user runs it: pilot's default `--out` is
    // relative to the working directory.
    let pilot = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&root),
        &["pilot", "--root", ".", "--mode", "draft"],
    )?;
    assert_success(&pilot);
    let pilot_dir = root.join("target/ripr/pilot");
    let before: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        pilot_dir.join("repo-exposure.json"),
    )?)?;
    assert!(
        before.pointer("/artifact/content_sha256").is_some(),
        "pilot repo-exposure.json must carry the producer-owned artifact identity"
    );
    let summary: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        pilot_dir.join("pilot-summary.json"),
    )?)?;
    let after_command = summary
        .pointer("/next/after_snapshot_command")
        .and_then(serde_json::Value::as_str)
        .ok_or("pilot summary names no after-snapshot command")?;
    // Run the printed command's arguments, not a hand-written equivalent:
    // a drift between pilot's identity and the command it prints is the bug.
    let (check_part, redirect) = after_command
        .split_once(" > ")
        .ok_or_else(|| format!("after command has no redirect: {after_command}"))?;
    let check_args: Vec<&str> = check_part
        .strip_prefix("ripr ")
        .ok_or_else(|| format!("after command is not a ripr command: {after_command}"))?
        .split_whitespace()
        .collect();
    // Pilot anchors the redirect at the resolved root (#3938), so the printed
    // target may be absolute; either way it must land in pilot's own out dir.
    let redirect_path = std::path::Path::new(redirect.trim().trim_matches('\''));
    let after = if redirect_path.is_absolute() {
        redirect_path.to_path_buf()
    } else {
        root.join(redirect_path)
    };
    assert_eq!(
        after.file_name().and_then(std::ffi::OsStr::to_str),
        Some("after.repo-exposure.json"),
        "{after_command}"
    );
    let pilot_dir = pilot_dir.canonicalize()?;
    assert_eq!(
        after
            .parent()
            .ok_or("after snapshot path has no parent")?
            .canonicalize()?,
        pilot_dir,
        "{after_command}"
    );

    let mut tests = std::fs::read_to_string(root.join("tests/pricing.rs"))?;
    tests.push_str(
        "\n#[test]\nfn threshold_equality_discounts() {\n    assert_eq!(discounted_total(100, 100), 90);\n}\n",
    );
    std::fs::write(root.join("tests/pricing.rs"), tests)?;

    let check = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), &check_args)?;
    assert_success(&check);
    std::fs::write(&after, &check.stdout)?;

    let verify = run_ripr(&[
        "agent",
        "verify",
        "--root",
        &root_arg,
        "--before",
        &pilot_dir.join("repo-exposure.json").display().to_string(),
        "--after",
        &after.display().to_string(),
        "--json",
    ]);
    assert_success(&verify);
    let report: serde_json::Value = serde_json::from_slice(&verify.stdout)?;
    assert_eq!(
        report.pointer("/summary/improved"),
        Some(&serde_json::json!(1)),
        "{report}"
    );
    assert_eq!(
        report.pointer("/summary/gap_movement/closed"),
        Some(&serde_json::json!(1)),
        "{report}"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// When the pilot seam budget truncates the inventory, pilot's snapshot holds
/// fewer seams than the after snapshot `ripr check` takes. It must not carry
/// the comparable identity, or verify would compare two populations.
#[test]
fn pilot_snapshot_truncated_by_the_seam_budget_is_not_a_verify_baseline()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("pilot-verify-truncated");
    std::fs::create_dir_all(&root)?;
    init_producer_fixture_repo(&root)?;
    let mut lib = std::fs::read_to_string(root.join("src/lib.rs"))?;
    lib.push_str(
        "\npub fn shipping_fee(weight: i32, free_limit: i32) -> i32 {\n    if weight > free_limit { 5 } else { 0 }\n}\n",
    );
    std::fs::write(root.join("src/lib.rs"), lib)?;

    let pilot = run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        &root,
        &["pilot", "--root", ".", "--mode", "draft"],
        &[("RIPR_PILOT_SEAM_BUDGET", "1")],
    )?;
    assert_success(&pilot);
    let before: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/pilot/repo-exposure.json"),
    )?)?;
    // Precondition: the budget really truncated a larger inventory.
    let check = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&root),
        &[
            "check",
            "--root",
            ".",
            "--mode",
            "draft",
            "--format",
            "repo-exposure-json",
        ],
    )?;
    assert_success(&check);
    let full: serde_json::Value = serde_json::from_slice(&check.stdout)?;
    let count = |doc: &serde_json::Value| {
        doc.get("seams")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len)
    };
    assert_eq!(count(&before), 1, "{before}");
    assert!(
        count(&full) > 1,
        "fixture must have more seams than the budget: {full}"
    );
    assert!(
        before.get("artifact").is_none(),
        "a budget-truncated pilot snapshot must not carry the comparable identity"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn pilot_writes_default_packet_outputs_for_boundary_gap_fixture() -> Result<(), String> {
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let out_dir = unique_temp_workspace("pilot");
    let output = run_ripr(&[
        "pilot",
        "--root",
        &root.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
    ]);
    assert_success(&output);

    for file in [
        "repo-exposure.json",
        "repo-exposure.md",
        "agent-seam-packets.json",
        "pilot-summary.json",
        "pilot-summary.md",
    ] {
        let path = out_dir.join(file);
        assert!(path.exists(), "pilot output missing {}", path.display());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RIPR pilot complete."));
    assert!(stdout.contains("config: missing, using built-in defaults"));
    assert!(stdout.contains("Top recommendation:"));
    assert!(stdout.contains("focused test:"));
    // #3906: an eligible top seam gets one ordinary route, the repair
    // transaction, not the manual before/after snapshot pair as well.
    assert!(stdout.contains("Next, in order:"), "{stdout}");
    assert!(
        !stdout.contains("Run after adding the focused test:")
            && !stdout.contains("ripr outcome --before"),
        "legacy snapshot choreography must not be offered beside the repair route:\n{stdout}"
    );

    let summary_json = std::fs::read_to_string(out_dir.join("pilot-summary.json"))
        .map_err(|e| format!("read pilot summary json: {e}"))?;
    assert!(summary_json.contains(r#""schema_version": "0.2""#));
    assert!(summary_json.contains(r#""scope": "repo""#));
    assert!(summary_json.contains(r#""status": "complete""#));
    assert!(summary_json.contains(r#""timeout_ms": 30000"#));
    assert!(summary_json.contains(r#""state": "missing""#));
    assert!(summary_json.contains(r#""top_actionable_seams""#));
    assert!(summary_json.contains("ripr outcome --before"));

    let packets = std::fs::read_to_string(out_dir.join("agent-seam-packets.json"))
        .map_err(|e| format!("read agent seam packets: {e}"))?;
    assert!(packets.contains(r#""packets_total""#));
    assert!(packets.contains(r#""task": "write_targeted_test""#));

    // The terminal is the only pilot surface a user sees without opening a
    // file, and the next documented step is `ripr agent repair --seam-id <id>`,
    // which accepts a seam id and nothing else. So the screen must carry the
    // id, and it must be the same id the written packet carries — the packet
    // is produced from the inventory `agent repair` resolves against, so a
    // mismatch would mean the printed command names a seam the repair
    // transaction cannot find.
    let seam_line = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("inspected seam: "))
        .ok_or_else(|| format!("pilot terminal printed no inspected-seam line:\n{stdout}"))?;
    let printed_id = seam_line
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("inspected-seam line carried no id: {seam_line}"))?;
    assert!(
        printed_id.len() == 16 && printed_id.chars().all(|ch| ch.is_ascii_hexdigit()),
        "expected a 16-hex seam id to lead the inspected-seam line, got: {seam_line}"
    );
    assert!(
        packets.contains(&format!(r#""seam_id": "{printed_id}""#)),
        "the printed seam id {printed_id} is absent from agent-seam-packets.json"
    );
    // This fixture's route is actionable, so the paste-ready repair command is
    // present and names the same id. The root is compared through the same
    // normalization the renderer applies (`loop_commands::shell_path` →
    // `display_path`, which rewrites `\` as `/`, then `shell_arg`, which may
    // quote), because `Path::display` keeps native separators and would make
    // this assertion fail on Windows for a command that is in fact correct.
    let repair_line = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("repair this seam: "))
        .ok_or_else(|| format!("pilot did not print a repair command:\n{stdout}"))?;
    let tokens: Vec<&str> = repair_line.split_whitespace().collect();
    let flag_value = |flag: &str| -> Option<&str> {
        tokens
            .iter()
            .position(|token| *token == flag)
            .and_then(|at| tokens.get(at + 1))
            .map(|value| value.trim_matches('\''))
    };
    assert_eq!(
        tokens.first().copied(),
        Some("ripr"),
        "expected a paste-ready ripr command, got: {repair_line}"
    );
    assert_eq!(
        flag_value("--seam-id"),
        Some(printed_id),
        "the repair command must name the seam printed above it: {repair_line}"
    );
    assert_eq!(
        flag_value("--phase"),
        Some("before"),
        "expected the opening phase of the repair transaction: {repair_line}"
    );
    assert_eq!(
        flag_value("--root").map(|value| value.replace('\\', "/")),
        Some(root.display().to_string().replace('\\', "/")),
        "the repair command must name the analyzed root: {repair_line}"
    );

    // The closing block, the JSON `next.repair_command`, and the Markdown
    // Next Commands block all carry that same command (#3906).
    assert!(
        stdout.contains(&format!("  1. {repair_line}\n")),
        "step 1 must be the repair command printed above:\n{stdout}"
    );
    let summary: serde_json::Value =
        serde_json::from_str(&summary_json).map_err(|e| format!("parse pilot summary: {e}"))?;
    assert_eq!(
        summary
            .pointer("/next/repair_command")
            .and_then(serde_json::Value::as_str),
        Some(repair_line),
        "pilot-summary.json next.repair_command"
    );
    let summary_md = std::fs::read_to_string(out_dir.join("pilot-summary.md"))
        .map_err(|e| format!("read pilot summary md: {e}"))?;
    let next_section = summary_md
        .split("## Next Commands")
        .nth(1)
        .ok_or_else(|| format!("pilot-summary.md has no Next Commands:\n{summary_md}"))?;
    assert!(
        next_section.contains(&format!("```bash\n{repair_line}\n```")),
        "Markdown next command must be the repair command:\n{next_section}"
    );
    assert!(
        !next_section.contains("ripr outcome"),
        "Markdown must not offer the snapshot pair beside the repair route:\n{next_section}"
    );

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
fn rerun_changed_test_emits_current_state_only_for_boundary_gap_fixture() -> Result<(), String> {
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.to_string_lossy().into_owned();
    let output = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--json",
    ]);
    assert_success(&output);
    let json = String::from_utf8_lossy(&output.stdout);
    for expected in [
        r#""schema_version": "ripr-targeted-rerun-v1""#,
        r#""state": "current_state_only""#,
        r#""changed_test": "tests/pricing.rs""#,
        r#""canonical_gap_id": "gap:"#,
        r#""repair_route_readiness": {"#,
        r#""state": "ready""#,
        "gap movement is not inferred",
    ] {
        if !json.contains(expected) {
            return Err(format!("rerun report missing {expected:?}: {json}"));
        }
    }
    for forbidden in ["\"improved\"", "\"closed\"", "\"regressed\""] {
        if json.contains(forbidden) {
            return Err(format!(
                "current-state report must not infer {forbidden}: {json}"
            ));
        }
    }
    Ok(())
}

#[test]
fn rerun_changed_test_check_parity_matches_full_pipeline_for_boundary_gap() -> Result<(), String> {
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.to_string_lossy().into_owned();
    let output = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--check-parity",
        "--json",
    ]);
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("parse parity rerun JSON: {err}"))?;
    if report["state"] != "current_state_only"
        || report["parity"]["state"] != "matched"
        || report["parity"]["selected_seam_count"] != report["parity"]["matched_seam_count"]
        || report["parity"]["mismatches"] != serde_json::json!([])
        || report["parity"]["input_mismatches"] != serde_json::json!([])
        || report["parity"]["selected_seam_count"] != serde_json::json!(1)
        || !report["cache"]["input_fingerprint"].is_object()
        || report["cache"]["input_fingerprint"]["workspace_manifests_hash"]
            .as_str()
            .is_none()
        || report["cache"]["input_fingerprint"]["lockfile_hash"]
            .as_str()
            .is_none()
        || report
            .get("limitation")
            .is_some_and(|value| !value.is_null())
        || !report["seams"][0]["related_tests"].is_array()
        || !report["seams"][0]["missing_discriminators"].is_array()
    {
        return Err(format!("unexpected parity rerun receipt: {report}"));
    }
    Ok(())
}

#[test]
fn rerun_before_receipt_names_toolchain_fingerprint_change() -> Result<(), String> {
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.to_string_lossy().into_owned();
    let before = run_ripr_with_env(
        &[
            "rerun",
            "--root",
            &root_arg,
            "--changed-test",
            "tests/pricing.rs",
            "--json",
        ],
        &[("RUSTUP_TOOLCHAIN", "toolchain-before")],
    );
    assert_success(&before);
    let workspace = unique_temp_workspace("rerun-input-fingerprint");
    std::fs::create_dir_all(&workspace)
        .map_err(|err| format!("create fingerprint workspace: {err}"))?;
    let before_path = workspace.join("before.json");
    std::fs::write(&before_path, &before.stdout)
        .map_err(|err| format!("write fingerprint before receipt: {err}"))?;
    let before_arg = before_path.to_string_lossy().into_owned();
    let after = run_ripr_with_env(
        &[
            "rerun",
            "--root",
            &root_arg,
            "--changed-test",
            "tests/pricing.rs",
            "--before",
            &before_arg,
            "--json",
        ],
        &[("RUSTUP_TOOLCHAIN", "toolchain-after")],
    );
    ignore_remove_dir_all(&workspace);
    assert_success(&after);
    let report: serde_json::Value = serde_json::from_slice(&after.stdout)
        .map_err(|err| format!("parse fingerprint rerun JSON: {err}"))?;
    if report["cache"]["invalidation_status"] != "workspace_input_changed"
        || !report["cache"]["recomputation_reasons"]
            .as_array()
            .is_some_and(|reasons| {
                reasons
                    .iter()
                    .any(|reason| reason.as_str() == Some("input_changed:toolchain_hash"))
            })
    {
        return Err(format!("unexpected input fingerprint disclosure: {report}"));
    }
    Ok(())
}

#[test]
fn rerun_gap_before_receipt_names_selector_ledger_change() -> Result<(), String> {
    let root_arg = "fixtures/boundary_gap/input";
    let changed = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--json",
    ])
    .map_err(|err| format!("run changed-test rerun: {err}"))?;
    assert_success(&changed);
    let changed_json: serde_json::Value = serde_json::from_slice(&changed.stdout)
        .map_err(|err| format!("parse changed-test rerun JSON: {err}"))?;
    let seam = changed_json["seams"]
        .as_array()
        .and_then(|seams| seams.first())
        .ok_or_else(|| "changed-test rerun emitted no seam".to_string())?;
    let canonical_gap_id = seam["canonical_gap_id"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks canonical_gap_id".to_string())?;
    let file = seam["file"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks file".to_string())?;
    let owner = seam["owner"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks owner".to_string())?;

    let workspace = unique_temp_workspace("rerun-ledger-fingerprint");
    std::fs::create_dir_all(&workspace)
        .map_err(|err| format!("create ledger fingerprint workspace: {err}"))?;
    let ledger = workspace.join("gap-ledger.json");
    let ledger_json = serde_json::json!({
        "kind": "gap_decision_ledger",
        "root": root_arg,
        "records": [{
            "canonical_gap_id": canonical_gap_id,
            "anchor": { "file": file, "owner": owner },
            "verification_commands": ["cargo test -p pricing boundary"],
            "receipt_command": "ripr receipt write --gap first"
        }]
    });
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&ledger_json)
            .map_err(|err| format!("serialize ledger fingerprint input: {err}"))?,
    )
    .map_err(|err| format!("write ledger fingerprint input: {err}"))?;
    let ledger_arg = ledger.to_string_lossy().into_owned();
    let before = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run ledger fingerprint before rerun: {err}"))?;
    assert_success(&before);
    let before_path = workspace.join("before.json");
    std::fs::write(&before_path, &before.stdout)
        .map_err(|err| format!("write ledger fingerprint before receipt: {err}"))?;

    let mut changed_ledger = ledger_json;
    changed_ledger["records"][0]["receipt_command"] =
        serde_json::Value::String("ripr receipt write --gap second".to_string());
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&changed_ledger)
            .map_err(|err| format!("serialize changed ledger fingerprint input: {err}"))?,
    )
    .map_err(|err| format!("write changed ledger fingerprint input: {err}"))?;
    let before_arg = before_path.to_string_lossy().into_owned();
    let after = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--before",
        &before_arg,
        "--json",
    ])
    .map_err(|err| format!("run ledger fingerprint after rerun: {err}"))?;
    ignore_remove_dir_all(&workspace);
    assert_success(&after);
    let report: serde_json::Value = serde_json::from_slice(&after.stdout)
        .map_err(|err| format!("parse ledger fingerprint after JSON: {err}"))?;
    if report["cache"]["invalidation_status"] != "workspace_input_changed"
        || !report["cache"]["recomputation_reasons"]
            .as_array()
            .is_some_and(|reasons| {
                reasons
                    .iter()
                    .any(|reason| reason.as_str() == Some("input_changed:selector_ledger_hash"))
            })
    {
        return Err(format!(
            "unexpected selector ledger fingerprint disclosure: {report}"
        ));
    }
    Ok(())
}

#[test]
fn rerun_check_parity_names_capped_inventory_and_suppresses_movement() -> Result<(), String> {
    let root = workspace_root().join("fixtures/observation_verified_field_construction/input");
    let root_arg = root.to_string_lossy().into_owned();
    let before = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/item_tests.rs",
        "--json",
    ]);
    assert_success(&before);
    let workspace = unique_temp_workspace("parity-before");
    std::fs::create_dir_all(&workspace).map_err(|err| format!("create before workspace: {err}"))?;
    let before_path = workspace.join("before.json");
    std::fs::write(&before_path, &before.stdout)
        .map_err(|err| format!("write before receipt: {err}"))?;
    let before_arg = before_path.to_string_lossy().into_owned();
    let after = run_ripr_with_env(
        &[
            "rerun",
            "--root",
            &root_arg,
            "--changed-test",
            "tests/item_tests.rs",
            "--before",
            &before_arg,
            "--check-parity",
            "--json",
        ],
        &[("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "1")],
    );
    ignore_remove_dir_all(&workspace);
    assert_success(&after);
    let report: serde_json::Value = serde_json::from_slice(&after.stdout)
        .map_err(|err| format!("parse capped parity rerun JSON: {err}"))?;
    if report["state"] != "limited"
        || report["parity"]["state"] != "limited"
        || report["limitation"]["kind"] != "full_pipeline_parity_incomplete"
        || report.get("movement").is_some_and(|value| !value.is_null())
    {
        return Err(format!("unexpected capped parity rerun receipt: {report}"));
    }
    Ok(())
}

#[test]
fn rerun_changed_test_uses_explicit_before_receipt_for_static_movement() -> Result<(), String> {
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.to_string_lossy().into_owned();
    let before = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--json",
    ]);
    assert_success(&before);
    let before_path = workspace_root().join("target").join(format!(
        "rerun-before-{}-{}.json",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&before_path, &before.stdout).map_err(|err| {
        format!(
            "write explicit before receipt {}: {err}",
            before_path.display()
        )
    })?;
    let before_arg = before_path.to_string_lossy().into_owned();
    let displayed_before = before_arg.replace('\\', "/");
    let after = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--before",
        &before_arg,
        "--json",
    ]);
    if let Ok(()) = std::fs::remove_file(&before_path) {}
    assert_success(&after);
    let json: serde_json::Value = serde_json::from_slice(&after.stdout)
        .map_err(|err| format!("parse targeted rerun movement JSON: {err}"))?;
    let selected_seam_count = json["seams"].as_array().map_or(0, Vec::len);
    if json["state"] != "unchanged"
        || json["movement"]["state"] != "unchanged"
        || json["movement"]["before"] != displayed_before
        || json["movement"]["matched_seam_count"] != serde_json::json!(selected_seam_count)
    {
        return Err(format!("unexpected explicit-before rerun receipt: {json}"));
    }
    Ok(())
}

#[test]
fn rerun_gap_recomputes_fixture_anchor_from_explicit_canonical_ledger() -> Result<(), String> {
    let root_arg = "fixtures/boundary_gap/input";
    let changed = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--json",
    ])
    .map_err(|err| format!("run changed-test rerun: {err}"))?;
    assert_success(&changed);
    let changed_json: serde_json::Value = serde_json::from_slice(&changed.stdout)
        .map_err(|err| format!("parse changed-test rerun JSON: {err}"))?;
    let seam = changed_json["seams"]
        .as_array()
        .and_then(|seams| seams.first())
        .ok_or_else(|| "changed-test rerun emitted no seam".to_string())?;
    let canonical_gap_id = seam["canonical_gap_id"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks canonical_gap_id".to_string())?;
    let file = seam["file"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks file".to_string())?;
    let owner = seam["owner"]
        .as_str()
        .ok_or_else(|| "changed-test rerun seam lacks owner".to_string())?;

    let ledger_dir = workspace_root().join("target").join(format!(
        "rerun-gap-ledger-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&ledger_dir)
        .map_err(|err| format!("create ledger dir {}: {err}", ledger_dir.display()))?;
    let ledger = ledger_dir.join("gap-ledger.json");
    let ledger_json = serde_json::json!({
        "kind": "gap_decision_ledger",
        "root": root_arg,
        "records": [{
            "canonical_gap_id": canonical_gap_id,
            "anchor": { "file": file, "owner": owner },
            "verification_commands": ["cargo test -p pricing boundary"],
            "receipt_command": "ripr outcome --before before.json --after after.json"
        }]
    });
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&ledger_json)
            .map_err(|err| format!("serialize gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write gap ledger {}: {err}", ledger.display()))?;
    let ledger_arg = ledger
        .strip_prefix(workspace_root())
        .map_err(|err| format!("make ledger path relative to workspace: {err}"))?
        .to_string_lossy()
        .into_owned();

    let selected = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run canonical gap rerun: {err}"))?;
    assert_success(&selected);
    let selected_json: serde_json::Value = serde_json::from_slice(&selected.stdout)
        .map_err(|err| format!("parse gap rerun JSON: {err}"))?;
    if selected_json["state"] != "current_state_only"
        || selected_json["selector"]["kind"] != "canonical_gap"
        || selected_json["selector"]["canonical_gap_id"] != canonical_gap_id
        || selected_json["seams"].as_array().is_none_or(Vec::is_empty)
        || selected_json["route"]["verify_commands"][0] != "cargo test -p pricing boundary"
    {
        return Err(format!(
            "unexpected canonical gap rerun report: {selected_json}"
        ));
    }

    let unresolved = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        "gap:missing",
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run unresolved gap rerun: {err}"))?;
    assert_success(&unresolved);
    let unresolved_json: serde_json::Value = serde_json::from_slice(&unresolved.stdout)
        .map_err(|err| format!("parse unresolved gap rerun JSON: {err}"))?;
    if unresolved_json["state"] != "limited"
        || unresolved_json["limitation"]["kind"] != "canonical_gap_unresolved"
        || unresolved_json["seams"] != serde_json::json!([])
    {
        return Err(format!(
            "unexpected unresolved gap rerun report: {unresolved_json}"
        ));
    }

    let mut duplicate_ledger = ledger_json.clone();
    duplicate_ledger["records"]
        .as_array_mut()
        .ok_or_else(|| "constructed gap ledger is missing records array".to_string())?
        .push(ledger_json["records"][0].clone());
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&duplicate_ledger)
            .map_err(|err| format!("serialize duplicate gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write duplicate gap ledger {}: {err}", ledger.display()))?;
    let duplicate = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run duplicate gap rerun: {err}"))?;
    assert_success(&duplicate);
    let duplicate_json: serde_json::Value = serde_json::from_slice(&duplicate.stdout)
        .map_err(|err| format!("parse duplicate gap rerun JSON: {err}"))?;
    if duplicate_json["state"] != "current_state_only"
        || duplicate_json["selector"]["matched_record_count"] != 2
        || duplicate_json["selector"]["recomputed_scope_count"] != 1
        || duplicate_json["seams"].as_array().map_or(0, Vec::len) != 1
    {
        return Err(format!(
            "unexpected duplicate gap rerun report: {duplicate_json}"
        ));
    }

    let mut mixed_ledger = ledger_json.clone();
    let stale_record = serde_json::json!({
        "canonical_gap_id": canonical_gap_id,
        "anchor": { "file": "tests/pricing.rs", "owner": "missing::owner" },
        "verification_commands": ["cargo test -p pricing stale"],
        "receipt_command": "ripr outcome --before before.json --after after.json"
    });
    mixed_ledger["records"]
        .as_array_mut()
        .ok_or_else(|| "constructed mixed gap ledger is missing records array".to_string())?
        .push(stale_record);
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&mixed_ledger)
            .map_err(|err| format!("serialize mixed gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write mixed gap ledger {}: {err}", ledger.display()))?;
    let mixed = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run mixed gap rerun: {err}"))?;
    assert_success(&mixed);
    let mixed_json: serde_json::Value = serde_json::from_slice(&mixed.stdout)
        .map_err(|err| format!("parse mixed gap rerun JSON: {err}"))?;
    if mixed_json["state"] != "current_state_only"
        || mixed_json["seams"].as_array().map_or(0, Vec::len) != 1
        || mixed_json["scope_limitations"]
            .as_array()
            .is_none_or(Vec::is_empty)
        || mixed_json["scope_limitations"][0]["kind"] != "gap_scope_unresolved"
    {
        return Err(format!("unexpected mixed gap rerun report: {mixed_json}"));
    }

    let mut conflict_ledger = ledger_json.clone();
    let mut conflicting_record = ledger_json["records"][0].clone();
    conflicting_record["receipt_command"] = serde_json::json!("ripr receipt write --gap conflict");
    conflict_ledger["records"]
        .as_array_mut()
        .ok_or_else(|| "constructed conflict gap ledger is missing records array".to_string())?
        .push(conflicting_record);
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&conflict_ledger)
            .map_err(|err| format!("serialize conflict gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write conflict gap ledger {}: {err}", ledger.display()))?;
    let conflict = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run conflict gap rerun: {err}"))?;
    assert_success(&conflict);
    let conflict_json: serde_json::Value = serde_json::from_slice(&conflict.stdout)
        .map_err(|err| format!("parse conflict gap rerun JSON: {err}"))?;
    if conflict_json["state"] != "current_state_only"
        || conflict_json["seams"].as_array().map_or(0, Vec::len) != 1
        || conflict_json["route"].get("receipt_command").is_none()
        || conflict_json["route"]["receipt_command"] != serde_json::Value::Null
        || conflict_json["route"]["receipt_command_conflict"]["kind"] != "receipt_command_conflict"
    {
        return Err(format!(
            "unexpected receipt conflict gap rerun report: {conflict_json}"
        ));
    }

    let mut stale_ledger = ledger_json;
    stale_ledger["root"] = serde_json::json!(ledger_dir.join("other-root"));
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&stale_ledger)
            .map_err(|err| format!("serialize stale gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write stale gap ledger {}: {err}", ledger.display()))?;
    let stale = run_ripr_in_workspace(&[
        "rerun",
        "--root",
        root_arg,
        "--gap",
        canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ])
    .map_err(|err| format!("run stale gap rerun: {err}"))?;
    assert_success(&stale);
    let stale_json: serde_json::Value = serde_json::from_slice(&stale.stdout)
        .map_err(|err| format!("parse stale gap rerun JSON: {err}"))?;
    if stale_json["state"] != "limited" || stale_json["limitation"]["kind"] != "stale_gap_ledger" {
        return Err(format!("unexpected stale gap rerun report: {stale_json}"));
    }

    ignore_remove_dir_all(&ledger_dir);
    Ok(())
}

fn multi_seam_gap_workspace() -> Result<PathBuf, String> {
    let root = unique_temp_workspace("rerun-multi-gap");
    std::fs::create_dir_all(root.join("src"))
        .map_err(|err| format!("create multi-gap src directory: {err}"))?;
    std::fs::create_dir_all(root.join("tests"))
        .map_err(|err| format!("create multi-gap test directory: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"rerun_multi_gap_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write multi-gap Cargo.toml: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, threshold: i32) -> i32 {\n    if amount >= threshold {\n        return amount - 10;\n    }\n    if amount >= threshold {\n        return amount - 20;\n    }\n    amount\n}\n",
    )
    .map_err(|err| format!("write multi-gap library: {err}"))?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        "use rerun_multi_gap_fixture::discounted_total;\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n",
    )
    .map_err(|err| format!("write multi-gap test: {err}"))?;
    Ok(root)
}

#[test]
fn rerun_gap_groups_multiple_current_seams() -> Result<(), String> {
    let root = multi_seam_gap_workspace()?;
    let root_arg = root.to_string_lossy().into_owned();
    let changed = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--changed-test",
        "tests/pricing.rs",
        "--json",
    ]);
    assert_success(&changed);
    let changed_json: serde_json::Value = serde_json::from_slice(&changed.stdout)
        .map_err(|err| format!("parse multi-seam changed-test rerun JSON: {err}"))?;
    let seams = changed_json["seams"]
        .as_array()
        .ok_or_else(|| format!("multi-seam changed-test report has no seams: {changed_json}"))?;
    let (canonical_gap_id, matching_seams) = seams
        .iter()
        .filter_map(|candidate| candidate["canonical_gap_id"].as_str())
        .find_map(|candidate_id| {
            let matching = seams
                .iter()
                .filter(|seam| seam["canonical_gap_id"] == candidate_id)
                .collect::<Vec<_>>();
            (matching.len() >= 2).then_some((candidate_id.to_string(), matching))
        })
        .ok_or_else(|| {
            format!(
                "expected two current seams with one canonical gap in multi-seam fixture: {changed_json}"
            )
        })?;
    let records = matching_seams
        .iter()
        .map(|seam| {
            serde_json::json!({
                "canonical_gap_id": canonical_gap_id,
                "anchor": {
                    "file": seam["file"],
                    "owner": seam["owner"],
                },
                "verification_commands": ["cargo test -p rerun_multi_gap_fixture far_above_threshold_discounts"],
                "receipt_command": "ripr receipt write --gap grouped"
            })
        })
        .collect::<Vec<_>>();
    let ledger = root.join("gap-ledger.json");
    std::fs::write(
        &ledger,
        serde_json::to_vec_pretty(&serde_json::json!({
            "kind": "gap_decision_ledger",
            "root": root_arg.clone(),
            "records": records.clone(),
        }))
        .map_err(|err| format!("serialize multi-seam gap ledger: {err}"))?,
    )
    .map_err(|err| format!("write multi-seam gap ledger: {err}"))?;
    let ledger_arg = ledger.to_string_lossy().into_owned();
    let grouped = run_ripr(&[
        "rerun",
        "--root",
        &root_arg,
        "--gap",
        &canonical_gap_id,
        "--gap-ledger",
        &ledger_arg,
        "--json",
    ]);
    assert_success(&grouped);
    let grouped_json: serde_json::Value = serde_json::from_slice(&grouped.stdout)
        .map_err(|err| format!("parse grouped multi-seam rerun JSON: {err}"))?;
    let grouped_seams = grouped_json["seams"]
        .as_array()
        .ok_or_else(|| format!("grouped multi-seam report has no seams: {grouped_json}"))?;
    let unique_seam_ids = grouped_seams
        .iter()
        .filter_map(|seam| seam["seam_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if grouped_json["state"] != "current_state_only"
        || grouped_json["selector"]["matched_record_count"] != serde_json::json!(records.len())
        || grouped_json["selector"]["recomputed_scope_count"] != 1
        || grouped_seams.len() != matching_seams.len()
        || unique_seam_ids.len() != matching_seams.len()
        || grouped_seams
            .iter()
            .any(|seam| seam["canonical_gap_id"] != serde_json::json!(canonical_gap_id))
    {
        return Err(format!(
            "multi-seam canonical grouping was not preserved: {grouped_json}"
        ));
    }
    std::fs::remove_dir_all(&root)
        .map_err(|err| format!("remove multi-seam workspace {}: {err}", root.display()))?;
    Ok(())
}

#[test]
#[cfg(feature = "lang-python")]
fn pilot_accepts_python_project_without_ripr_config() -> Result<(), String> {
    let root = workspace_root().join("fixtures/python/basic");
    let out_dir = unique_temp_workspace("pilot-python-basic");
    let output = run_ripr(&[
        "pilot",
        "--root",
        &root.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RIPR pilot complete."));
    assert!(stdout.contains("Python preview:"));
    assert!(out_dir.join("pilot-summary.json").exists());

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
#[cfg(feature = "lang-python")]
fn pilot_projects_python_repair_card_for_git_diff() -> Result<(), String> {
    let root = unique_temp_workspace("pilot-python-git");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    std::fs::create_dir_all(root.join("tests")).map_err(|err| format!("create tests: {err}"))?;
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"pilot-python-git\"\nversion = \"0.0.0\"\n",
    )
    .map_err(|err| format!("write pyproject: {err}"))?;
    std::fs::write(
        root.join("src/pricing.py"),
        "def calculate_discount(amount, threshold):\n    if amount > threshold:\n        return amount - 10\n    return amount\n",
    )
    .map_err(|err| format!("write baseline pricing: {err}"))?;
    std::fs::write(
        root.join("tests/test_pricing.py"),
        "from src.pricing import calculate_discount\n\n\ndef test_calculate_discount_smoke():\n    result = calculate_discount(125, 100)\n    assert result\n",
    )
    .map_err(|err| format!("write tests: {err}"))?;

    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "ripr@example.invalid"])?;
    run_git(&root, &["config", "user.name", "RIPR Test"])?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "base"])?;
    run_git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    std::fs::write(
        root.join("src/pricing.py"),
        "def calculate_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    )
    .map_err(|err| format!("write changed pricing: {err}"))?;
    run_git(&root, &["add", "src/pricing.py"])?;
    run_git(&root, &["commit", "-m", "change threshold boundary"])?;

    let out_dir = unique_temp_workspace("pilot-python-git-out");
    let output = run_ripr(&[
        "pilot",
        "--root",
        &root.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    for needle in [
        "Top recommendation:",
        "language: python (preview)",
        "repair action: strengthen_existing_test",
        "changed owner: calculate_discount",
        "missing discriminator: amount == threshold",
        "recommended repair: strengthen test_calculate_discount_smoke in tests/test_pricing.py",
        "verify: python -m pytest tests/test_pricing.py::test_calculate_discount_smoke",
        "receipt status: unavailable_until_python_gap_ledger",
    ] {
        assert!(stdout.contains(needle), "missing stdout needle: {needle}");
    }
    assert!(
        !stdout.contains("none ranked by the default pilot policy"),
        "Python repair-card pilot should not render the no-recommendation top line"
    );

    let summary_json = std::fs::read_to_string(out_dir.join("pilot-summary.json"))
        .map_err(|err| format!("read pilot summary json: {err}"))?;
    for needle in [
        r#""python_first_use": {"#,
        r#""status": "ready""#,
        r#""language": "python""#,
        r#""language_status": "preview""#,
        r#""repair_action": "strengthen_existing_test""#,
        r#""changed_owner": "calculate_discount""#,
        r#""missing_discriminator": "amount == threshold""#,
        r#""suggested_test_file": "tests/test_pricing.py""#,
        r#""verify_command": "python -m pytest tests/test_pricing.py::test_calculate_discount_smoke""#,
    ] {
        assert!(
            summary_json.contains(needle),
            "missing summary JSON needle: {needle}"
        );
    }

    ignore_remove_dir_all(&root);
    ignore_remove_dir_all(&out_dir);
    Ok(())
}

/// A committed git repository with one base commit, an `origin/main` ref at
/// that base, and one follow-up commit that applies `change` (#3906 pilot
/// language-route fixtures).
fn pilot_language_fixture_repo(
    label: &str,
    files: &[(&str, &str)],
    change: (&str, &str),
) -> Result<PathBuf, String> {
    let root = unique_temp_workspace(label);
    for (path, text) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))?;
    }
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "ripr@example.invalid"])?;
    run_git(&root, &["config", "user.name", "RIPR Test"])?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "base"])?;
    run_git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    std::fs::write(root.join(change.0), change.1)
        .map_err(|err| format!("write changed {}: {err}", change.0))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "change"])?;
    Ok(root)
}

/// #4216 row 1: the onboarding re-walk's `pyapp` shape. A named module
/// constant threshold leaves the boundary operand unresolved, so the Python
/// preview finding is weakly exposed with no repair card. `check` used to
/// tell the operator to "complete the missing repair-packet fields", which
/// they cannot do, while pilot, first-pr and status each pointed at another
/// command. The safe next action must name why no route exists and the
/// manual step that is left, with no route back into pilot.
#[cfg(feature = "lang-python")]
fn python_check_safe_action(
    label: &str,
    source_base: &str,
    change_from: &str,
    change_to: &str,
    test_source: &str,
) -> Result<String, String> {
    let root = pilot_language_fixture_repo(
        label,
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"py-safe-action\"\nversion = \"0.0.0\"\n",
            ),
            ("pricing/__init__.py", source_base),
            ("tests/test_pricing.py", test_source),
        ],
        (
            "pricing/__init__.py",
            &source_base.replace(change_from, change_to),
        ),
    )?;
    let output = run_ripr(&[
        "check",
        "--root",
        &root.display().to_string(),
        "--base",
        "origin/main",
    ]);
    let _ = std::fs::remove_dir_all(&root);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let line = stdout
        .lines()
        .find(|line| line.starts_with("  Safe next action:"))
        .ok_or_else(|| format!("check printed no safe next action:\n{stdout}"))?;
    assert!(
        stdout.contains("State: preview language, advisory only (preview_limited)"),
        "expected preview_limited triage:\n{stdout}"
    );
    Ok(line.to_string())
}

#[test]
#[cfg(feature = "lang-python")]
fn check_python_finding_without_repair_card_names_the_terminal_manual_step() -> Result<(), String> {
    let line = python_check_safe_action(
        "check-py-no-card",
        // A `global`-rebound constant stays unresolved (#4227), so no
        // concrete boundary value and no repair card exist for it.
        "DISCOUNT_THRESHOLD = 10_000\n\n\ndef configure(threshold):\n    global DISCOUNT_THRESHOLD\n    DISCOUNT_THRESHOLD = threshold\n\n\ndef discounted_total(amount: int) -> int:\n    if amount > DISCOUNT_THRESHOLD:\n        return amount - amount // 10\n    return amount\n",
        "amount > DISCOUNT_THRESHOLD",
        "amount >= DISCOUNT_THRESHOLD",
        "from pricing import discounted_total\n\n\ndef test_no_discount_below_threshold():\n    assert discounted_total(5_000) == 5_000\n\n\ndef test_discount_far_above_threshold():\n    assert discounted_total(20_000) == 18_000\n",
    )?;
    assert_eq!(
        line,
        "  Safe next action: this Python preview finding has no repair card (static evidence names no concrete missing discriminator), so `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; add or strengthen a test by hand, then rerun `ripr check`."
    );
    Ok(())
}

// Discriminating control: a Python finding that DOES carry a repair card is
// routed by that card, so it must not get the no-card terminal line.
#[test]
#[cfg(feature = "lang-python")]
fn check_python_finding_with_repair_card_points_at_the_card() -> Result<(), String> {
    let line = python_check_safe_action(
        "check-py-card",
        "def calculate_discount(amount, threshold):\n    if amount > threshold:\n        return amount - 10\n    return amount\n",
        "amount > threshold",
        "amount >= threshold",
        "from pricing import calculate_discount\n\n\ndef test_calculate_discount_smoke():\n    result = calculate_discount(125, 100)\n    assert result\n",
    )?;
    assert_eq!(
        line,
        "  Safe next action: preview-language evidence is advisory; apply the next step below to the suggested test and run its verify command before relying on it."
    );
    Ok(())
}

/// Run `ripr pilot` and return (stdout, pilot-summary.md, pilot-summary.json,
/// repo-exposure.json).
fn run_pilot_language_fixture(
    root: &Path,
    out_dir: &Path,
) -> Result<(String, String, serde_json::Value, serde_json::Value), String> {
    let output = run_ripr(&[
        "pilot",
        "--root",
        &root.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
    ]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let md = std::fs::read_to_string(out_dir.join("pilot-summary.md"))
        .map_err(|err| format!("read pilot summary md: {err}"))?;
    let summary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("pilot-summary.json"))
            .map_err(|err| format!("read pilot summary json: {err}"))?,
    )
    .map_err(|err| format!("parse pilot summary json: {err}"))?;
    let exposure: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("repo-exposure.json"))
            .map_err(|err| format!("read repo exposure json: {err}"))?,
    )
    .map_err(|err| format!("parse repo exposure json: {err}"))?;
    Ok((stdout, md, summary, exposure))
}

/// Precondition for the no-Rust-seam fixtures: pilot's Rust scan really found
/// nothing to rank, so any route shown is the only signal about the repo.
fn assert_pilot_found_no_rust_seams(summary: &serde_json::Value, exposure: &serde_json::Value) {
    assert_eq!(
        exposure["metrics"]["seams_total"],
        serde_json::json!(0),
        "fixture must have zero Rust seams: {exposure}"
    );
    assert_eq!(summary["actionable_seams_total"], serde_json::json!(0));
    assert_eq!(summary["top_actionable_seams"], serde_json::json!([]));
}

/// The one route pilot reports for `language`, with the `required` state.
fn required_language_route(
    summary: &serde_json::Value,
    language: &str,
) -> Result<serde_json::Value, String> {
    let routes = &summary["language_routes"];
    if routes["state"] != "required" {
        return Err(format!("language routes must be required: {routes}"));
    }
    let matching = routes["routes"]
        .as_array()
        .ok_or_else(|| format!("language_routes.routes is not an array: {routes}"))?
        .iter()
        .filter(|route| route["language"] == language)
        .cloned()
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [route] if route["file_count"].as_u64().unwrap_or(0) >= 1 => Ok(route.clone()),
        _ => Err(format!("expected one detected {language} route: {routes}")),
    }
}

/// The empty Rust ranking must not read as a clean pass, and the Rust
/// repo-exposure snapshot choreography (which can only report "no seams
/// moved") must not be offered as the follow-up.
fn assert_pilot_does_not_read_as_clean(stdout: &str, md: &str) {
    assert!(
        !stdout.contains("none ranked by the default pilot policy"),
        "no-Rust-seam pilot must not print the plain no-recommendation line:\n{stdout}"
    );
    assert!(
        stdout.contains("none: pilot ranks Rust seams and found none here"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Languages outside pilot's Rust seam scan:"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("ripr outcome --before") && !stdout.contains("repo-exposure-json >"),
        "Rust snapshot follow-up must not be offered for a repo with no Rust seams:\n{stdout}"
    );
    assert!(
        md.contains("## Languages Outside The Rust Seam Scan"),
        "{md}"
    );
    assert!(md.contains("This is not a clean result"), "{md}");
    assert!(!md.contains("ripr outcome --before"), "{md}");
}

#[test]
#[cfg(feature = "lang-typescript")]
fn pilot_names_typescript_diff_first_route_when_repo_has_no_rust_seams() -> Result<(), String> {
    let base = "export function discount(amount: number, threshold: number): number {\n  if (amount > threshold) {\n    return amount - 10;\n  }\n  return amount;\n}\n";
    let root = pilot_language_fixture_repo(
        "pilot-lang-ts",
        &[
            (
                "package.json",
                "{\"name\":\"pilot-lang-ts\",\"version\":\"0.0.0\"}\n",
            ),
            ("src/pricing.ts", base),
            (
                "tests/pricing.test.ts",
                "import { discount } from \"../src/pricing\";\ntest(\"discount\", () => {\n  expect(discount(125, 100)).toBeTruthy();\n});\n",
            ),
        ],
        (
            "src/pricing.ts",
            &base.replace("amount > threshold", "amount >= threshold"),
        ),
    )?;
    let out_dir = unique_temp_workspace("pilot-lang-ts-out");
    let (stdout, md, summary, exposure) = run_pilot_language_fixture(&root, &out_dir)?;

    assert_pilot_found_no_rust_seams(&summary, &exposure);
    let route = required_language_route(&summary, "typescript")?;
    let command = format!(
        "ripr check --root {}",
        root.display().to_string().replace('\\', "/")
    );
    assert_eq!(route["language_status"], "preview");
    assert_eq!(route["enabled"], false);
    assert_eq!(route["route"], "check_diff_first");
    assert_eq!(route["command"], serde_json::json!(command));
    // The existing `typescript_diff_first` guidance is surfaced, not reworded.
    assert_eq!(route["guidance_category"], "typescript_diff_first");
    assert!(
        route["guidance"]
            .as_str()
            .is_some_and(|text| text.starts_with("TypeScript is analyzed diff-first;")),
        "{route}"
    );
    assert_pilot_does_not_read_as_clean(&stdout, &md);
    assert!(
        stdout.contains(
            "typescript: 2 files (preview, diff-first; not enabled in ripr.toml [languages])"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "not enabled in ripr.toml [languages])\n    route: {command}\n"
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "Next, analyze the changed code in these languages:\n  {command}\n"
        )),
        "{stdout}"
    );
    assert!(md.contains("`typescript_diff_first`: TypeScript is analyzed diff-first;"));

    // With the preview adapter enabled, the route no longer says "not enabled".
    std::fs::write(
        root.join("ripr.toml"),
        "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
    )
    .map_err(|err| format!("write ripr.toml: {err}"))?;
    let (enabled_stdout, _, enabled_summary, _) = run_pilot_language_fixture(&root, &out_dir)?;
    assert_eq!(
        required_language_route(&enabled_summary, "typescript")?["enabled"],
        true
    );
    assert!(
        enabled_stdout.contains("typescript: 2 files (preview, diff-first)\n"),
        "{enabled_stdout}"
    );

    ignore_remove_dir_all(&root);
    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
#[cfg(feature = "lang-python")]
fn pilot_names_python_check_route_when_repo_has_no_rust_seams() -> Result<(), String> {
    let base = "def calculate_discount(amount, threshold):\n    if amount > threshold:\n        return amount - 10\n    return amount\n";
    let root = pilot_language_fixture_repo(
        "pilot-lang-py",
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"pilot-lang-py\"\nversion = \"0.0.0\"\n",
            ),
            ("src/pricing.py", base),
            (
                "tests/test_pricing.py",
                "from src.pricing import calculate_discount\n\n\ndef test_calculate_discount_smoke():\n    result = calculate_discount(125, 100)\n    assert result\n",
            ),
        ],
        (
            "src/pricing.py",
            &base.replace("amount > threshold", "amount >= threshold"),
        ),
    )?;
    let out_dir = unique_temp_workspace("pilot-lang-py-out");
    let (stdout, md, summary, exposure) = run_pilot_language_fixture(&root, &out_dir)?;

    assert_pilot_found_no_rust_seams(&summary, &exposure);
    // Python was detected and analyzed through the existing diff-first
    // projection; the route adds where to rerun it.
    assert_eq!(summary["python_first_use"]["status"], "ready");
    let route = required_language_route(&summary, "python")?;
    let command = format!(
        "ripr check --root {}",
        root.display().to_string().replace('\\', "/")
    );
    assert_eq!(route["enabled"], true);
    assert_eq!(route["command"], serde_json::json!(command));
    assert_eq!(route["guidance_category"], "python_diff_first");
    assert!(
        route["guidance"].as_str().is_some_and(|text| {
            text.contains("does not render Python findings")
                && text.contains("run 'ripr check' or")
                && !text.contains("origin/main")
        }),
        "{route}"
    );
    let categories = exposure["limitations"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["category"].as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        categories.contains(&"python_diff_first"),
        "python-only repo exposure must name the seam-inventory limit: {exposure}"
    );
    assert_eq!(exposure["metrics"]["seams_total"], 0);
    assert!(
        stdout.contains(&format!(
            "python: 2 files (preview, diff-first)\n    route: {command}\n"
        )),
        "{stdout}"
    );
    // The Python repair card is the top recommendation, so the closing block
    // follows the card's route rather than sending the user to `ripr check`,
    // which only leads back to pilot (rc rehearsal py-pricing).
    assert!(
        stdout.contains("Next, in order:\n  1. ripr first-pr --root ")
            && stdout.contains("\n  3. python -m pytest ")
            && stdout.contains("\n  4. run the receipt command step 1 printed\n"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Next, analyze the changed code in these languages:"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("ripr outcome --before"),
        "Python pilot must not end in the Rust repo-exposure outcome route:\n{stdout}"
    );
    assert!(md.contains(&format!("  - Route: `{command}`")), "{md}");
    assert!(md.contains("`python_diff_first`:"), "{md}");
    assert!(!md.contains("ripr outcome --before"), "{md}");

    // The printed route must analyze the changed Python, not only parse.
    let root_arg = root.display().to_string();
    let routed = run_ripr(&["check", "--root", &root_arg]);
    let routed_stdout = String::from_utf8_lossy(&routed.stdout);
    assert!(
        routed.status.code().is_some_and(|code| code <= 1),
        "routed check failed: {routed:?}"
    );
    assert!(
        routed_stdout.contains("src/pricing.py"),
        "routed check must report the changed Python file:\n{routed_stdout}"
    );

    ignore_remove_dir_all(&root);
    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
fn pilot_says_perl_is_unavailable_when_repo_has_no_rust_seams() -> Result<(), String> {
    let base = "package Pricing;\nsub discount {\n    my ($amount, $threshold) = @_;\n    return $amount > $threshold ? $amount - 10 : $amount;\n}\n1;\n";
    let root = pilot_language_fixture_repo(
        "pilot-lang-pl",
        &[("lib/Pricing.pm", base)],
        (
            "lib/Pricing.pm",
            &base.replace("$amount > $threshold", "$amount >= $threshold"),
        ),
    )?;
    let out_dir = unique_temp_workspace("pilot-lang-pl-out");
    let (stdout, md, summary, exposure) = run_pilot_language_fixture(&root, &out_dir)?;

    assert_pilot_found_no_rust_seams(&summary, &exposure);
    let route = required_language_route(&summary, "perl")?;
    assert_eq!(route["file_count"], 1);
    assert_pilot_does_not_read_as_clean(&stdout, &md);
    if cfg!(feature = "lang-perl") {
        assert_eq!(route["language_status"], "preview");
        assert_eq!(
            route["command"],
            serde_json::json!(format!("ripr check --root {}", root.display()))
        );
    } else {
        let notice = "Perl analysis is not available from this ripr binary. It needs both a ripr build with Cargo feature `lang-perl` (`cargo install ripr --features lang-perl`) and a compatible Perl fact exporter (`perl-ripr-facts`), which is not yet published; no released ripr setup analyzes Perl yet, and adding `perl` to ripr.toml [languages] alone does not enable it.";
        assert_eq!(route["language_status"], "unavailable");
        assert_eq!(route["route"], "unavailable_in_this_binary");
        assert_eq!(route["command"], serde_json::Value::Null);
        assert_eq!(route["guidance"], serde_json::json!(notice));
        assert!(
            stdout.contains(&format!(
                "perl: 1 file (not available in this build)\n    {notice}\n"
            )),
            "{stdout}"
        );
        assert!(
            stdout.contains(
                "No follow-up command applies: this ripr binary cannot analyze the languages listed above."
            ),
            "{stdout}"
        );
        assert!(md.contains(notice), "{md}");
    }

    ignore_remove_dir_all(&root);
    ignore_remove_dir_all(&out_dir);
    Ok(())
}

/// Rust-only output is untouched by language routing, and adding TypeScript
/// beside Rust seams leaves the human output byte-identical: the mixed repo
/// keeps the Rust result and lists the other language in JSON only.
#[test]
fn pilot_keeps_rust_output_byte_identical_when_rust_seams_exist() -> Result<(), String> {
    let root = pilot_language_fixture_repo(
        "pilot-lang-rs",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"pilot-lang-rs\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            ),
            (
                "src/lib.rs",
                "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold\n}\n",
            ),
        ],
        (
            "src/lib.rs",
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        ),
    )?;
    let out_dir = unique_temp_workspace("pilot-lang-rs-out");
    let (rust_stdout, rust_md, rust_summary, rust_exposure) =
        run_pilot_language_fixture(&root, &out_dir)?;

    assert!(
        rust_exposure["metrics"]["seams_total"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "Rust fixture must produce Rust seams: {rust_exposure}"
    );
    assert_eq!(
        rust_summary["language_routes"],
        serde_json::json!({"state": "not_detected", "routes": []})
    );
    assert!(rust_stdout.contains("inspected seam: "), "{rust_stdout}");
    assert!(
        !rust_stdout.contains("Languages outside pilot's Rust seam scan"),
        "{rust_stdout}"
    );

    std::fs::create_dir_all(root.join("web")).map_err(|err| format!("create web: {err}"))?;
    std::fs::write(
        root.join("web/pricing.ts"),
        "export function discount(amount: number): number {\n  return amount - 10;\n}\n",
    )
    .map_err(|err| format!("write web/pricing.ts: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "add typescript"])?;
    let (mixed_stdout, mixed_md, mixed_summary, _) = run_pilot_language_fixture(&root, &out_dir)?;

    assert_eq!(
        mixed_stdout, rust_stdout,
        "mixed repo changed the Rust terminal output"
    );
    assert_eq!(
        mixed_md, rust_md,
        "mixed repo changed the Rust markdown output"
    );
    assert_eq!(mixed_summary["language_routes"]["state"], "supplementary");
    let routes = mixed_summary["language_routes"]["routes"]
        .as_array()
        .ok_or_else(|| format!("routes is not an array: {mixed_summary}"))?;
    assert_eq!(routes.len(), 1, "{mixed_summary}");
    assert_eq!(routes[0]["language"], "typescript");
    assert_eq!(routes[0]["file_count"], 1);
    assert_eq!(
        mixed_summary["top_actionable_seams"], rust_summary["top_actionable_seams"],
        "mixed repo must keep the Rust ranking"
    );

    ignore_remove_dir_all(&root);
    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
#[cfg(feature = "lang-python")]
fn check_detects_python_project_without_ripr_config() {
    let root = workspace_root().join("fixtures/python/basic");
    let diff = root.join("diff.patch");
    let output = run_ripr(&[
        "check",
        "--root",
        &root.display().to_string(),
        "--diff",
        &diff.display().to_string(),
        "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""language": "python""#));
    assert!(stdout.contains(r#""language_status": "preview""#));
    assert!(stdout.contains("python_preview"));
}

#[test]
fn pilot_honors_explicit_mode_over_repo_config() -> Result<(), String> {
    let workspace = make_temp_workspace_with_production_seam()?;
    std::fs::write(
        workspace.join("ripr.toml"),
        "[analysis]\nmode = \"ready\"\n\n[lsp]\nseam_diagnostics = true\n",
    )
    .map_err(|e| format!("write ripr.toml: {e}"))?;
    let out_dir = unique_temp_workspace("pilot-mode");
    let output = run_ripr(&[
        "pilot",
        "--root",
        &workspace.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
        "--mode",
        "draft",
    ]);
    assert_success(&output);

    let summary_json = std::fs::read_to_string(out_dir.join("pilot-summary.json"))
        .map_err(|e| format!("read pilot summary json: {e}"))?;
    assert!(summary_json.contains(r#""mode": "draft""#));
    assert!(summary_json.contains(r#""state": "loaded""#));

    ignore_remove_dir_all(&out_dir);
    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn pilot_uses_repo_config_mode_without_explicit_flag() -> Result<(), String> {
    let workspace = make_temp_workspace_with_production_seam()?;
    std::fs::write(
        workspace.join("ripr.toml"),
        "[analysis]\nmode = \"ready\"\n\n[lsp]\nseam_diagnostics = true\n",
    )
    .map_err(|e| format!("write ripr.toml: {e}"))?;
    let out_dir = unique_temp_workspace("pilot-config-mode");
    let output = run_ripr(&[
        "pilot",
        "--root",
        &workspace.display().to_string(),
        "--out",
        &out_dir.display().to_string(),
    ]);
    assert_success(&output);

    let summary_json = std::fs::read_to_string(out_dir.join("pilot-summary.json"))
        .map_err(|e| format!("read pilot summary json: {e}"))?;
    assert!(summary_json.contains(r#""mode": "ready""#));
    assert!(summary_json.contains(r#""state": "loaded""#));

    ignore_remove_dir_all(&out_dir);
    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// `first-action` and `pr-review front-panel` refuse to run without at least
/// one artifact input, so their usage lines must not render those inputs the
/// way an optional flag with a default is rendered.
///
/// This binds the claim to the refusal in one test: the same command that the
/// help describes is executed with no artifact input and must exit 2. A help
/// line that goes back to bracketing every input, or a command that starts
/// guessing paths, fails here.
#[test]
fn artifact_input_commands_do_not_advertise_defaults_they_refuse_to_use() -> Result<(), String> {
    let workspace = unique_temp_workspace("artifact-input-required");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let root = workspace.display().to_string();

    let overview = run_ripr(&["help", "--all"]);
    assert_success(&overview);
    let overview_stdout = String::from_utf8_lossy(&overview.stdout).to_string();

    let cases: [(&[&str], &str, &str); 2] = [
        (
            &["first-action"],
            "  ripr first-action [--root .] (--pr-guidance",
            "ripr first-action [--root .] [--pr-guidance",
        ),
        (
            &["pr-review", "front-panel"],
            "  ripr pr-review front-panel (--pr-guidance",
            "ripr pr-review front-panel [--pr-guidance",
        ),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (command, required_form, optional_form) in cases {
        let name = command.join(" ");
        if !overview_stdout.contains(required_form) {
            failures.push(format!(
                "`help --all` does not mark {name}'s inputs required"
            ));
        }
        if overview_stdout.contains(optional_form) {
            failures.push(format!(
                "`help --all` still renders {name}'s required inputs as optional defaults"
            ));
        }

        let mut args: Vec<&str> = command.to_vec();
        args.extend_from_slice(&["--root", root.as_str()]);
        let refused = run_ripr(&args);
        let stderr = String::from_utf8_lossy(&refused.stderr).to_string();
        if refused.status.success()
            || !stderr.contains("requires at least one explicit artifact input")
        {
            failures.push(format!(
                "`ripr {name}` no longer refuses a run with no artifact input: {stderr}"
            ));
        }

        let mut help_args: Vec<&str> = command.to_vec();
        help_args.push("--help");
        let help = run_ripr(&help_args);
        assert_success(&help);
        let help_stdout = String::from_utf8_lossy(&help.stdout).to_string();
        if !help_stdout.contains("At least one artifact input is required") {
            failures.push(format!(
                "`ripr {name} --help` does not state the constraint"
            ));
        }
    }

    ignore_remove_dir_all(&workspace);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[test]
fn outcome_prints_markdown_receipt_by_default() -> Result<(), String> {
    let workspace = unique_temp_workspace("outcome-stdout");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create outcome workspace: {e}"))?;
    write_outcome_snapshots(&workspace)?;

    let output = run_ripr(&[
        "outcome",
        "--before",
        &workspace.join("before.json").display().to_string(),
        "--after",
        &workspace.join("after.json").display().to_string(),
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# ripr targeted-test outcome report"));
    assert!(stdout.contains("| moved | 1 |"));
    assert!(stdout.contains("weakly_gripped -> strongly_gripped"));
    assert!(stdout.contains("does not run mutation testing"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// Give a snapshot the artifact identity that `ripr check --format
/// repo-exposure-json` writes, so the process-boundary behaviour can be
/// observed without running an analysis.
fn stamp_outcome_snapshot_head(path: &Path, head: &str) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read snapshot: {e}"))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse snapshot: {e}"))?;
    value["artifact"] = serde_json::json!({
        "kind": "repo_exposure",
        "repository": { "root": "/workspace", "head": head },
    });
    std::fs::write(path, value.to_string()).map_err(|e| format!("write snapshot: {e}"))
}

/// `ripr outcome`'s stderr disclosure must follow the artifacts it was handed,
/// through the real binary rather than the helper alone. Before this, the line
/// asserted unconditionally that the artifacts carry no head SHA, which is
/// false for any snapshot written through the artifact-identity path.
///
/// Both arms run in one test so the identity-free pair is a discriminating
/// control for the identity-carrying pair: a regression that hardcoded either
/// sentence fails here.
#[test]
fn outcome_disclosure_follows_the_artifacts_it_was_given() -> Result<(), String> {
    let workspace = unique_temp_workspace("outcome-head-disclosure");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create outcome workspace: {e}"))?;
    write_outcome_snapshots(&workspace)?;
    let before = workspace.join("before.json").display().to_string();
    let after = workspace.join("after.json").display().to_string();

    // Control: the shape `ripr pilot` writes, with no artifact identity.
    let without_identity = run_ripr(&["outcome", "--before", &before, "--after", &after]);
    assert_success(&without_identity);
    let stderr = String::from_utf8_lossy(&without_identity.stderr).to_string();
    let missing_head_is_disclosed = stderr.contains("does not carry a head SHA");

    let head = "2bd22c0b0718157870e2e78c9a70b9da1c9c1b21";
    stamp_outcome_snapshot_head(&workspace.join("before.json"), head)?;
    stamp_outcome_snapshot_head(&workspace.join("after.json"), head)?;
    let with_identity = run_ripr(&["outcome", "--before", &before, "--after", &after]);
    assert_success(&with_identity);
    let identity_stderr = String::from_utf8_lossy(&with_identity.stderr).to_string();

    ignore_remove_dir_all(&workspace);
    if !missing_head_is_disclosed {
        return Err(format!(
            "identity-free snapshots must still disclose the missing head: {stderr}"
        ));
    }
    if !identity_stderr.contains(head) {
        return Err(format!(
            "a snapshot carrying a head must have it named: {identity_stderr}"
        ));
    }
    if identity_stderr.contains("carry a head SHA") {
        return Err(format!(
            "must not claim the artifacts lack a head they carry: {identity_stderr}"
        ));
    }
    Ok(())
}

#[test]
fn outcome_writes_json_receipt_when_requested() -> Result<(), String> {
    let workspace = unique_temp_workspace("outcome-json");
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create outcome workspace: {e}"))?;
    write_outcome_snapshots(&workspace)?;
    let out_path = workspace.join("target/ripr/outcome/targeted-test-outcome.json");

    let output = run_ripr(&[
        "outcome",
        "--before",
        &workspace.join("before.json").display().to_string(),
        "--after",
        &workspace.join("after.json").display().to_string(),
        "--format",
        "json",
        "--out",
        &out_path.display().to_string(),
    ]);
    assert_success(&output);

    let json = std::fs::read_to_string(&out_path).map_err(|e| format!("read outcome json: {e}"))?;
    assert!(json.contains(r#""schema_version": "0.1""#));
    assert!(json.contains(r#""status": "advisory""#));
    assert!(json.contains(r#""moved": 1"#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn calibrate_cargo_mutants_prints_markdown_by_default() {
    let root = workspace_root();
    let mutants = root
        .join("fixtures/boundary_gap/calibration/runtime-mutants.json")
        .display()
        .to_string();
    let repo = root
        .join("fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json")
        .display()
        .to_string();

    let output = run_ripr(&[
        "calibrate",
        "cargo-mutants",
        "--mutants-json",
        &mutants,
        "--repo-exposure-json",
        &repo,
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# ripr mutation calibration report"));
    assert!(stdout.contains("Status: advisory"));
    assert!(stdout.contains("Static/runtime agreement"));
    assert!(stdout.contains("Runtime Outcome Counts"));
}

#[test]
fn calibrate_cargo_mutants_writes_json_when_requested() -> Result<(), String> {
    let root = workspace_root();
    let out_dir = unique_temp_workspace("calibrate-json");
    let out_path = out_dir.join("mutation-calibration.json");
    let mutants = root
        .join("fixtures/boundary_gap/calibration/runtime-mutants.json")
        .display()
        .to_string();
    let repo = root
        .join("fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json")
        .display()
        .to_string();

    let output = run_ripr(&[
        "calibrate",
        "cargo-mutants",
        "--mutants-json",
        &mutants,
        "--repo-exposure-json",
        &repo,
        "--format",
        "json",
        "--out",
        &out_path.display().to_string(),
    ]);
    assert_success(&output);

    let json =
        std::fs::read_to_string(&out_path).map_err(|e| format!("read calibration json: {e}"))?;
    assert!(json.contains(r#""schema_version": "0.1""#));
    assert!(json.contains(r#""status": "advisory""#));
    assert!(json.contains(r#""agreement""#));
    assert!(json.contains(r#""matches""#));

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

#[test]
fn calibration_runtime_fixture_matches_checked_reports() -> Result<(), String> {
    let root = workspace_root();
    let fixture = root.join("fixtures/boundary_gap/calibration/runtime-fixtures-v1");
    let value = assert_calibration_fixture_matches_checked_reports(&fixture)?;

    assert_eq!(value["agreement"]["static_gap_and_runtime_signal"], 1);
    assert_eq!(value["agreement"]["static_gap_without_runtime_signal"], 3);
    assert_eq!(value["agreement"]["runtime_signal_without_static_gap"], 2);
    assert_eq!(value["agreement"]["static_clean_and_runtime_clean"], 1);
    assert_eq!(value["agreement"]["runtime_inconclusive"], 2);
    assert_eq!(value["metrics"]["ambiguous_file_line_total"], 1);
    assert_eq!(value["metrics"]["unmatched_mutants_total"], 1);
    assert_eq!(value["metrics"]["static_without_runtime_total"], 1);
    assert_eq!(value["metrics"]["join_method_counts"]["file_line"], 1);
    assert_eq!(value["metrics"]["join_method_counts"]["seam_id"], 5);

    Ok(())
}

#[test]
fn calibration_runtime_fixture_v2_matches_checked_reports() -> Result<(), String> {
    let root = workspace_root();
    let fixture = root.join("fixtures/boundary_gap/calibration/runtime-fixtures-v2");
    let value = assert_calibration_fixture_matches_checked_reports(&fixture)?;

    assert_eq!(value["agreement"]["static_gap_and_runtime_signal"], 2);
    assert_eq!(value["agreement"]["static_gap_without_runtime_signal"], 1);
    assert_eq!(value["agreement"]["runtime_signal_without_static_gap"], 1);
    assert_eq!(value["agreement"]["static_clean_and_runtime_clean"], 1);
    assert_eq!(value["agreement"]["runtime_inconclusive"], 1);
    assert_eq!(value["metrics"]["ambiguous_file_line_total"], 1);
    assert_eq!(value["metrics"]["unmatched_mutants_total"], 1);
    assert_eq!(value["metrics"]["static_without_runtime_total"], 0);
    assert_eq!(value["metrics"]["join_method_counts"]["seam_id"], 4);

    assert_eq!(
        calibration_match_confidence(&value, "cal-v2-side-effect-observer")?,
        "supports_static_gap"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v2-mock-expectation")?,
        "supports_static_clean"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v2-weak-snapshot-oracle")?,
        "contradicts_static_gap"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v2-opaque-dispatch")?,
        "supports_static_gap"
    );

    assert_eq!(
        value["ambiguous_file_line_matches"][0]["confidence_label"],
        "ambiguous_runtime_join"
    );
    assert_eq!(
        value["ambiguous_file_line_matches"][0]["candidates"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        value["missed_runtime_signals"][0]["confidence_label"],
        "runtime_only_signal"
    );
    assert!(
        value["missed_runtime_signals"][0]["static"].is_null(),
        "runtime-only signal must not create a static gap"
    );

    Ok(())
}

#[test]
fn calibration_runtime_fixture_v3_matches_checked_reports() -> Result<(), String> {
    let root = workspace_root();
    let fixture = root.join("fixtures/boundary_gap/calibration/runtime-fixtures-v3");
    let value = assert_calibration_fixture_matches_checked_reports(&fixture)?;

    assert_eq!(value["agreement"]["static_gap_and_runtime_signal"], 2);
    assert_eq!(value["agreement"]["static_gap_without_runtime_signal"], 2);
    assert_eq!(value["agreement"]["runtime_signal_without_static_gap"], 2);
    assert_eq!(value["agreement"]["static_clean_and_runtime_clean"], 1);
    assert_eq!(value["agreement"]["runtime_inconclusive"], 1);
    assert_eq!(value["metrics"]["ambiguous_file_line_total"], 1);
    assert_eq!(value["metrics"]["unmatched_mutants_total"], 1);
    assert_eq!(value["metrics"]["static_without_runtime_total"], 1);
    assert_eq!(value["metrics"]["join_method_counts"]["seam_id"], 5);

    assert_eq!(
        calibration_match_confidence(&value, "cal-v3-custom-helper-outcome")?,
        "supports_static_gap"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v3-table-boundary-outcome")?,
        "supports_static_clean"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v3-builder-override-outcome")?,
        "contradicts_static_gap"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v3-snapshot-field-discriminator")?,
        "contradicts_static_clean"
    );
    assert_eq!(
        calibration_match_confidence(&value, "cal-v3-mock-expectation-mismatch")?,
        "supports_static_gap"
    );

    assert!(
        value["static_only_findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding["confidence_label"] == "no_runtime_data"
                    && finding["static"]["seam_id"] == "cal-v3-cross-file-constant-boundary"
            })),
        "cross-file constant sample must remain no_runtime_data until a joined runtime sample exists"
    );
    assert_eq!(
        value["ambiguous_file_line_matches"][0]["confidence_label"],
        "ambiguous_runtime_join"
    );
    assert_eq!(
        value["ambiguous_file_line_matches"][0]["candidates"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert!(
        value["missed_runtime_signals"]
            .as_array()
            .is_some_and(|signals| signals.iter().any(|signal| {
                signal["confidence_label"] == "runtime_only_signal" && signal["static"].is_null()
            })),
        "runtime-only signal must stay calibration context without creating a static gap"
    );

    Ok(())
}

fn assert_calibration_fixture_matches_checked_reports(
    fixture: &Path,
) -> Result<serde_json::Value, String> {
    let mutants = fixture.join("runtime-mutants.json").display().to_string();
    let repo = fixture.join("repo-exposure.json").display().to_string();

    let json_output = run_ripr(&[
        "calibrate",
        "cargo-mutants",
        "--mutants-json",
        &mutants,
        "--repo-exposure-json",
        &repo,
        "--format",
        "json",
    ]);
    assert_success(&json_output);
    let expected_json = std::fs::read_to_string(fixture.join("mutation-calibration.json"))
        .map_err(|e| format!("read checked calibration json: {e}"))?;
    let actual_json = String::from_utf8(json_output.stdout)
        .map_err(|e| format!("decode calibration json stdout: {e}"))?;
    assert_eq!(actual_json, expected_json);

    let value: serde_json::Value = serde_json::from_str(&expected_json)
        .map_err(|e| format!("parse checked calibration json: {e}"))?;

    let md_output = run_ripr(&[
        "calibrate",
        "cargo-mutants",
        "--mutants-json",
        &mutants,
        "--repo-exposure-json",
        &repo,
        "--format",
        "md",
    ]);
    assert_success(&md_output);
    let expected_md = std::fs::read_to_string(fixture.join("mutation-calibration.md"))
        .map_err(|e| format!("read checked calibration markdown: {e}"))?;
    let actual_md = String::from_utf8(md_output.stdout)
        .map_err(|e| format!("decode calibration markdown stdout: {e}"))?;
    assert_eq!(actual_md, expected_md);

    Ok(value)
}

fn calibration_match_confidence<'a>(
    value: &'a serde_json::Value,
    seam_id: &str,
) -> Result<&'a str, String> {
    value["matches"]
        .as_array()
        .and_then(|matches| {
            matches.iter().find_map(|record| {
                (record["static"]["seam_id"] == seam_id)
                    .then(|| record["confidence_label"].as_str())
                    .flatten()
            })
        })
        .ok_or_else(|| format!("missing calibration match for seam `{seam_id}`"))
}

fn write_outcome_snapshots(workspace: &Path) -> Result<(), String> {
    let before = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": ["50"],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#;
    let after = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "strong"}
      ],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
}"#;
    std::fs::write(workspace.join("before.json"), before)
        .map_err(|e| format!("write before snapshot: {e}"))?;
    std::fs::write(workspace.join("after.json"), after)
        .map_err(|e| format!("write after snapshot: {e}"))
}

fn make_temp_workspace_with_production_seam() -> Result<PathBuf, String> {
    make_temp_workspace_with_production_seam_and_report_opt(None)
}

fn make_temp_workspace_with_production_seam_and_report(report: &str) -> Result<PathBuf, String> {
    make_temp_workspace_with_production_seam_and_report_opt(Some(report))
}

fn make_temp_workspace_with_production_seam_and_report_opt(
    report: Option<&str>,
) -> Result<PathBuf, String> {
    let dir = unique_temp_workspace("repo-badge");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname=\"ripr-repo-badge-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|e| format!("write Cargo.toml: {e}"))?;
    std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("create src: {e}"))?;
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|e| format!("write src/lib.rs: {e}"))?;
    if let Some(text) = report {
        let reports = dir.join("target/ripr/reports");
        std::fs::create_dir_all(&reports).map_err(|e| format!("create reports dir: {e}"))?;
        std::fs::write(reports.join("test-efficiency.json"), text)
            .map_err(|e| format!("write report: {e}"))?;
    }
    Ok(dir)
}

fn make_temp_workspace_with_suppressions(
    report: Option<&str>,
    suppressions: Option<&str>,
) -> Result<PathBuf, String> {
    let dir = unique_temp_workspace("badge-plus");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname=\"ripr-badge-plus-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|e| format!("write Cargo.toml: {e}"))?;
    std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("create src: {e}"))?;
    std::fs::write(dir.join("src/lib.rs"), "pub fn placeholder() {}\n")
        .map_err(|e| format!("write src/lib.rs: {e}"))?;
    if let Some(text) = report {
        let reports = dir.join("target/ripr/reports");
        std::fs::create_dir_all(&reports).map_err(|e| format!("create reports dir: {e}"))?;
        std::fs::write(reports.join("test-efficiency.json"), text)
            .map_err(|e| format!("write report: {e}"))?;
    }
    if let Some(text) = suppressions {
        let policy_dir = dir.join(".ripr");
        std::fs::create_dir_all(&policy_dir).map_err(|e| format!("create .ripr dir: {e}"))?;
        std::fs::write(policy_dir.join("suppressions.toml"), text)
            .map_err(|e| format!("write suppressions: {e}"))?;
    }
    Ok(dir)
}

#[test]
fn check_badge_plus_missing_test_efficiency_renders_neutral_badge() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();

    for (format, args) in [
        (
            "badge-plus-json",
            vec![
                "check",
                "--root",
                root.as_str(),
                "--diff",
                diff.as_str(),
                "--format",
                "badge-plus-json",
            ],
        ),
        (
            "badge-plus-shields",
            vec![
                "check",
                "--root",
                root.as_str(),
                "--diff",
                diff.as_str(),
                "--format",
                "badge-plus-shields",
            ],
        ),
        (
            "repo-badge-plus-json",
            vec![
                "check",
                "--root",
                root.as_str(),
                "--format",
                "repo-badge-plus-json",
            ],
        ),
        (
            "repo-badge-plus-shields",
            vec![
                "check",
                "--root",
                root.as_str(),
                "--format",
                "repo-badge-plus-shields",
            ],
        ),
    ] {
        let output = run_ripr(&args);
        assert_success(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(r#""message": "needs test-efficiency""#),
            "stdout must render neutral badge for `{format}`: {stdout}"
        );
        assert!(
            stdout.contains(r#""color": "lightgrey""#),
            "stdout must render neutral color for `{format}`: {stdout}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("test-efficiency.json"),
            "stderr must name the missing report for `{format}`: {stderr}"
        );
        assert!(
            stderr.contains("docs/BADGE_ADOPTION.md"),
            "stderr must point to badge adoption docs for `{format}`: {stderr}"
        );
        assert!(
            !stderr.contains("cargo xtask test-efficiency-report"),
            "stderr must not hardcode repo-private xtask guidance for `{format}`: {stderr}"
        );
    }
    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_plus_json_emits_native_shape_with_fixture_report() -> Result<(), String> {
    // Repo scope parses the repo-wide test-efficiency ledger for validation
    // and reason visibility, but raw test-efficiency debt no longer moves
    // the public headline until it is lifted into the canonical repair model.
    let workspace = make_temp_workspace(Some(fixture_test_efficiency_report()))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-plus-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.8""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(stdout.contains(r#""counts""#));
    assert!(stdout.contains(r#""reason_counts""#));
    assert!(stdout.contains(r#""policy""#));
    // Repo-scoped public badge carries the RIPR-SPEC-0066 projection.
    assert!(stdout.contains(r#""public_projection""#));
    assert!(stdout.contains(r#""run_status": "full""#));
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""intentional_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""unknowns_test_efficiency": 0"#));
    assert!(stdout.contains(r#""analyzed_tests": 3"#));
    // Reason counts include all nine keys, with the fixture values surfacing.
    assert!(stdout.contains(r#""smoke_oracle_only": 2"#));
    assert!(stdout.contains(r#""duplicate_activation_and_oracle_shape": 0"#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_plus_shields_emits_four_field_shape_with_fixture_report() -> Result<(), String> {
    let workspace = make_temp_workspace(Some(fixture_test_efficiency_report()))?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-plus-shields",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schemaVersion": 1"#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(stdout.contains(r#""color":"#));
    // Native-only fields must not leak into Shields shape.
    for forbidden in [
        r#""counts""#,
        r#""reason_counts""#,
        r#""policy""#,
        r#""kind""#,
        r#""status""#,
        r#""scope""#,
        r#""basis""#,
        r#""schema_version""#,
    ] {
        assert!(
            !stdout.contains(forbidden),
            "ripr+ Shields projection must not contain `{forbidden}`: {stdout}"
        );
    }
    // Message has no denominator and no coverage framing.
    assert!(!stdout.to_ascii_lowercase().contains("coverage"));
    assert!(!stdout.to_ascii_lowercase().contains("uncovered"));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_plus_command_exits_zero_by_default_even_with_nonzero_count() -> Result<(), String> {
    // Default policy is fail_on_nonzero=false. The fixture reports 1
    // unsuppressed actionable finding, so the headline is at least 1; the
    // command must still exit zero so CI artifact pipelines work.
    let workspace = make_temp_workspace(Some(fixture_test_efficiency_report()))?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-plus-json",
    ]);
    assert_success(&output);

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_json_emits_repo_scope_metadata() -> Result<(), String> {
    // Repo scope must NOT consume `--diff`; it analyzes the workspace
    // baseline through run_repo_analysis. A no-diff invocation that would
    // produce empty findings under diff scope still produces a real
    // repo-scoped count under repo scope.
    let workspace = make_temp_workspace_with_production_seam()?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.8""#));
    assert!(stdout.contains(r#""kind": "ripr""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(
        !stdout.contains(r#""scope": "diff""#),
        "repo scope output must not also carry diff scope: {stdout}"
    );
    assert!(stdout.contains(r#""label": "ripr""#));
    assert!(stdout.contains(r#""counts""#));
    // Repo-scoped public badge carries the RIPR-SPEC-0066 projection.
    assert!(stdout.contains(r#""public_projection""#));
    assert!(stdout.contains(r#""source_report": "target/ripr/reports/repo-ripr-badge.json""#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_json_can_use_gap_ledger_targets() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let ledger = workspace.join("gap-decision-ledger.json");
    std::fs::write(
        &ledger,
        r#"{
          "gap_records": [
            {
              "gap_id": "gap:repo:pricing:reintroduced-boundary",
              "source_currentness": "candidate_current",
              "kind": "MissingBoundaryAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "repo_scoped",
              "gap_state": "reintroduced",
              "policy_state": "reintroduced",
              "repairability": "repairable",
              "projection_eligibility": {
                "ripr_zero_count": {"eligible": true, "reason": "repo_policy_targeted_unresolved_gap"},
                "ripr_plus_count": {"eligible": true, "reason": "broader_repo_advisory_gap"}
              }
            },
            {
              "gap_id": "gap:repo:waived",
              "source_currentness": "candidate_current",
              "kind": "MissingValueAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "repo_scoped",
              "gap_state": "waived",
              "policy_state": "waived",
              "repairability": "no_action",
              "projection_eligibility": {
                "ripr_zero_count": {"eligible": false, "reason": "waived"},
                "ripr_plus_count": {"eligible": false, "reason": "waived"}
              }
            }
          ]
        }"#,
    )
    .map_err(|e| format!("write gap ledger: {e}"))?;

    let root = workspace.display().to_string();
    let ledger_path = ledger.display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--format",
        "repo-badge-json",
        "--gap-ledger",
        &ledger_path,
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.8""#));
    assert!(stdout.contains(r#""basis": "gap_decision_ledger""#));
    // The gap-ledger repo badge is projected into the closed public vocabulary.
    assert!(stdout.contains(r#""message": "1 actionable""#));
    assert!(stdout.contains(r#""analyzed_gap_records": 2"#));
    assert!(stdout.contains(r#""state": "actionable""#));
    assert!(stdout.contains(r#""actionable_count": 1"#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_shields_keeps_four_fields_without_scope_leak() -> Result<(), String> {
    let workspace = make_temp_workspace_with_production_seam()?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-shields"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schemaVersion": 1"#));
    assert!(stdout.contains(r#""label": "ripr""#));
    assert!(stdout.contains(r#""color""#));
    // Scope is native-only metadata; Shields stays exactly four fields.
    assert!(
        !stdout.contains(r#""scope""#),
        "repo Shields projection must not include scope: {stdout}"
    );
    for forbidden in [
        r#""counts""#,
        r#""reason_counts""#,
        r#""policy""#,
        r#""kind""#,
        r#""status""#,
        r#""basis""#,
        r#""schema_version""#,
    ] {
        assert!(
            !stdout.contains(forbidden),
            "repo Shields projection must not contain `{forbidden}`: {stdout}"
        );
    }

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_plus_json_emits_repo_scope_metadata() -> Result<(), String> {
    let workspace =
        make_temp_workspace_with_production_seam_and_report(fixture_test_efficiency_report())?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-plus-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.8""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(stdout.contains(r#""public_projection""#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_exposure_summary_json_emits_bounded_summary() -> Result<(), String> {
    let workspace = make_temp_workspace_with_production_seam()?;
    let root = workspace.display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--format",
        "repo-exposure-summary-json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.1""#));
    assert!(stdout.contains(r#""format": "repo-exposure-summary-json""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(stdout.contains(r#""raw_seams""#));
    assert!(stdout.contains(r#""unsuppressed_exposure_gaps""#));
    assert!(stdout.contains(r#""reason_breakdown""#));
    assert!(stdout.contains(r#""top_files""#));
    assert!(!stdout.contains(r#""seams": ["#));
    assert!(!stdout.contains(r#""evidence_record""#));
    assert!(!stdout.contains(r#""related_tests""#));
    assert!(!stdout.contains(r#""observed_values""#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_plus_shields_keeps_four_fields() -> Result<(), String> {
    let workspace =
        make_temp_workspace_with_production_seam_and_report(fixture_test_efficiency_report())?;
    let root = workspace.display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--format",
        "repo-badge-plus-shields",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schemaVersion": 1"#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(!stdout.contains(r#""scope""#));
    assert!(!stdout.contains(r#""basis""#));
    let top_level_keys = stdout
        .lines()
        .filter(|line| line.starts_with("  \""))
        .count();
    assert_eq!(
        top_level_keys, 4,
        "expected exactly 4 top-level Shields fields, got: {stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_does_not_consult_diff_arg_when_supplied() -> Result<(), String> {
    // Pin: even if `--diff` is passed, repo formats analyze the repo
    // baseline. The diff arg is silently ignored under repo scope rather
    // than mistakenly mixed into the analysis. This is the regression that
    // unblocks badge/publish-main-endpoint.
    let workspace = make_temp_workspace_with_production_seam()?;
    let root = workspace.display().to_string();
    let empty_diff = workspace.join("empty.patch");
    std::fs::write(
        &empty_diff,
        r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
"#,
    )
    .map_err(|e| format!("write empty.patch: {e}"))?;

    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &empty_diff.display().to_string(),
        "--format",
        "repo-badge-json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""scope": "repo""#));
    // The temp workspace has a probeable predicate; repo badge scope now
    // counts classified seams, so analyzed_seams > 0 even when the diff is
    // empty. Assert the value, not just the key — a key check alone would
    // also pass for `analyzed_seams: 0`, which is exactly the empty-scope
    // behavior this regression pins against.
    assert!(
        stdout.contains(r#""analyzed_seams""#),
        "repo native JSON must include analyzed_seams: {stdout}"
    );
    assert!(
        !stdout.contains(r#""analyzed_seams": 0"#),
        "repo badge must find at least one analyzed seam from the workspace \
         predicate; got analyzed_seams: 0 — this suggests empty scope \
         was used instead: {stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_command_exits_zero_even_with_nonzero_count() {
    // Default policy is fail_on_nonzero=false. The sample diff has gaps but
    // the command must still exit successfully so CI artifact pipelines work.
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-json",
    ]);
    assert_success(&output);
}

#[test]
fn explain_returns_targeted_probe_details() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "explain",
        "--root",
        &root,
        "--diff",
        &diff,
        "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("family: error_path"));
    assert!(stdout.contains("delta:  value"));
    assert!(stdout.contains("Static exposure\n  weakly_exposed"));
    assert!(stdout.contains("No exact error variant discriminator was detected"));
}

#[test]
fn context_json_returns_probe_and_discriminator_guidance() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "context",
        "--root",
        &root,
        "--diff",
        &diff,
        "--at",
        "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250",
        "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            r#""id": "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250""#
        )
    );
    assert!(stdout.contains(r#""discriminate": "weak""#));
    assert!(stdout.contains(r#""missing""#));
}

#[test]
fn explain_unknown_probe_fails_with_clear_error() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "explain",
        "--root",
        &root,
        "--diff",
        &diff,
        "probe:missing:0:not_real",
    ]);
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no finding matched"));
}

// -------- check-artifact reuse smoke (RIPR-SPEC-0140, #2107) --------

/// End-to-end three-step flow: `check --write-artifact` once, then
/// `explain --from` and `context --from` reuse the recorded findings with
/// no scope flags. Finding detail remains identical while navigation names
/// the source identity used by each invocation.
#[test]
fn check_write_artifact_then_explain_and_context_reuse_preserves_detail_and_source_navigation()
-> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let dir = unique_temp_workspace("check-artifact-reuse");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let artifact = dir.join("last-check.json");
    let artifact_arg = artifact.display().to_string();
    let selector = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250";

    let result = (|| {
        let check = run_ripr(&[
            "check",
            "--root",
            &root,
            "--diff",
            &diff,
            "--json",
            "--write-artifact",
            &artifact_arg,
        ]);
        assert_success(&check);
        let artifact_text = std::fs::read_to_string(&artifact)
            .map_err(|err| format!("artifact was not written: {err}"))?;
        assert!(artifact_text.contains("\"ripr-check-artifact-v1\""));
        assert!(artifact_text.contains("\"identity\""));
        assert!(artifact_text.contains("\"diff_bytes_hash\""));

        let fresh_explain = run_ripr(&["explain", "--root", &root, "--diff", &diff, selector]);
        assert_success(&fresh_explain);
        let reused_explain = run_ripr(&[
            "explain",
            "--root",
            &root,
            "--from",
            &artifact_arg,
            selector,
        ]);
        assert_success(&reused_explain);
        let fresh_explain_text = String::from_utf8_lossy(&fresh_explain.stdout);
        let reused_explain_text = String::from_utf8_lossy(&reused_explain.stdout);
        if !fresh_explain_text.contains("Next: ripr context --root")
            || !fresh_explain_text.contains("--diff")
            || !reused_explain_text.contains("Next: ripr context --root")
            || !reused_explain_text.contains("--from")
        {
            return Err(
                "explain navigation did not preserve fresh and artifact sources".to_string(),
            );
        }

        let fresh_context = run_ripr(&[
            "context", "--root", &root, "--diff", &diff, "--at", selector, "--json",
        ]);
        assert_success(&fresh_context);
        let reused_context = run_ripr(&[
            "context",
            "--root",
            &root,
            "--from",
            &artifact_arg,
            "--at",
            selector,
        ]);
        assert_success(&reused_context);
        context_reuse_matches_fresh(&fresh_context.stdout, &reused_context.stdout)?;
        Ok(())
    })();
    ignore_remove_dir_all(&dir);
    result
}

/// RIPR-SPEC-0140: a `context --from` packet equals the fresh `--diff` packet
/// except the witness's explain command, which names the source that
/// produced it (#3952): `--diff` fresh, `--from` reused, as `explain` does.
fn context_reuse_matches_fresh(fresh: &[u8], reused: &[u8]) -> Result<(), String> {
    let parse = |bytes: &[u8], label: &str| -> Result<(serde_json::Value, String), String> {
        let mut packet: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|err| format!("parse {label} context packet: {err}"))?;
        let command = packet
            .pointer_mut("/witness/explain_command")
            .map(serde_json::Value::take)
            .and_then(|command| command.as_str().map(str::to_string))
            .ok_or_else(|| format!("{label} context packet must carry an explain command"))?;
        Ok((packet, command))
    };
    let (fresh_packet, fresh_command) = parse(fresh, "fresh")?;
    let (reused_packet, reused_command) = parse(reused, "reused")?;
    if !fresh_command.contains(" --diff ") || !reused_command.contains(" --from ") {
        return Err(format!(
            "explain commands must name their source: fresh `{fresh_command}`, reused `{reused_command}`"
        ));
    }
    if fresh_packet != reused_packet {
        return Err(format!(
            "context --from packet must match the fresh run\nfresh: {fresh_packet}\nreused: {reused_packet}"
        ));
    }
    Ok(())
}

/// Every identity mismatch class fails closed with a typed error naming the
/// mismatched field — never a silent recompute.
#[test]
fn explain_from_fails_closed_on_tampered_identity() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let dir = unique_temp_workspace("check-artifact-tamper");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let artifact = dir.join("last-check.json");
    let artifact_arg = artifact.display().to_string();
    let selector = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250";

    let result = (|| {
        let check = run_ripr(&[
            "check",
            "--root",
            &root,
            "--diff",
            &diff,
            "--write-artifact",
            &artifact_arg,
        ]);
        assert_success(&check);
        let original = std::fs::read_to_string(&artifact)
            .map_err(|err| format!("artifact was not written: {err}"))?;

        // Mode mismatch (tampered recording).
        let tampered = original.replace("\"mode\": \"draft\"", "\"mode\": \"ready\"");
        std::fs::write(&artifact, &tampered).map_err(|err| format!("write: {err}"))?;
        let output = run_ripr(&[
            "explain",
            "--root",
            &root,
            "--from",
            &artifact_arg,
            selector,
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot be reused")
                && stderr.contains("identity mismatch")
                && stderr.contains("mode"),
            "mode mismatch must be named:\n{stderr}"
        );

        // Unsupported schema version.
        let stale = original.replace("ripr-check-artifact-v1", "ripr-check-artifact-v0");
        std::fs::write(&artifact, &stale).map_err(|err| format!("write: {err}"))?;
        let output = run_ripr(&[
            "explain",
            "--root",
            &root,
            "--from",
            &artifact_arg,
            selector,
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unsupported schema_version"),
            "schema mismatch must be named:\n{stderr}"
        );

        // A scope flag passed alongside --from is an assertion, not an
        // override: a different --diff than the recording fails closed.
        std::fs::write(&artifact, &original).map_err(|err| format!("write: {err}"))?;
        let other_diff = dir.join("other.diff");
        std::fs::copy(sample_diff(), &other_diff).map_err(|err| format!("copy: {err}"))?;
        let other_diff_arg = other_diff.display().to_string();
        let output = run_ripr(&[
            "explain",
            "--root",
            &root,
            "--from",
            &artifact_arg,
            "--diff",
            &other_diff_arg,
            selector,
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("asserted scope does not match") && stderr.contains("diff_source"),
            "asserted --diff mismatch must be named:\n{stderr}"
        );
        Ok(())
    })();
    ignore_remove_dir_all(&dir);
    result
}

/// `--write-artifact` fails closed with a named limitation for run shapes
/// that have no re-resolvable diff-scoped finding set. (`--worktree` runs
/// are supported: their base-to-worktree diff source is re-resolvable —
/// see check_worktree_write_artifact_then_explain_reuse_and_drift_fails_closed.)
#[test]
fn check_write_artifact_rejects_unsupported_run_shapes() {
    let root = workspace_root().display().to_string();
    let dir = unique_temp_workspace("check-artifact-reject");
    let artifact = dir.join("last-check.json");
    let artifact_arg = artifact.display().to_string();

    let repo_scoped = run_ripr(&[
        "check",
        "--root",
        &root,
        "--format",
        "repo-seams-json",
        "--write-artifact",
        &artifact_arg,
    ]);
    assert_failure(&repo_scoped);
    let stderr = String::from_utf8_lossy(&repo_scoped.stderr);
    assert!(
        stderr.contains("repo-scoped"),
        "repo-scope limitation must be named:\n{stderr}"
    );
}

/// `check --worktree --write-artifact` records the base-to-worktree diff
/// source (#2251): `explain --from` reuses it while the worktree matches
/// the recording, a matching `--base` alongside `--from` is accepted as an
/// assertion, and worktree drift between write and reuse fails closed
/// naming diff_bytes_hash.
#[test]
fn check_worktree_write_artifact_then_explain_reuse_and_drift_fails_closed() -> Result<(), String> {
    let root = unique_temp_workspace("worktree-artifact-reuse");
    let result = (|| {
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        run_git(&root, &["init"])?;
        run_git(&root, &["config", "user.email", "test@test.com"])?;
        run_git(&root, &["config", "user.name", "Test"])?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        )
        .map_err(|err| format!("write base lib.rs: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"spec-0140-worktree-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        run_git(&root, &["add", "."])?;
        run_git(&root, &["commit", "-m", "initial"])?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold\n}\n",
        )
        .map_err(|err| format!("write dirty lib.rs: {err}"))?;

        let root_str = root.to_string_lossy().into_owned();
        let artifact = root.join("last-check.json");
        let artifact_arg = artifact.display().to_string();
        let check = run_ripr(&[
            "check",
            "--root",
            &root_str,
            "--base",
            "HEAD",
            "--worktree",
            "--json",
            "--write-artifact",
            &artifact_arg,
        ]);
        assert_success(&check);
        let stdout = String::from_utf8_lossy(&check.stdout);
        let report: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|err| format!("parse check JSON: {err}\n{stdout}"))?;
        let selector = report
            .pointer("/findings/0/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("expected a finding id in JSON:\n{stdout}"))?
            .to_string();
        let artifact_text = std::fs::read_to_string(&artifact)
            .map_err(|err| format!("artifact was not written: {err}"))?;
        if !artifact_text.contains("\"worktree\"") {
            return Err(format!(
                "artifact must record the worktree diff source:\n{artifact_text}"
            ));
        }

        // Matching worktree state: reuse succeeds, with or without the
        // matching --base assertion.
        let reused = run_ripr(&[
            "explain",
            "--root",
            &root_str,
            "--from",
            &artifact_arg,
            &selector,
        ]);
        assert_success(&reused);
        let reused_stdout = String::from_utf8_lossy(&reused.stdout);
        if !reused_stdout.contains("Static exposure") {
            return Err(format!(
                "reused explain lost its exposure section:\n{reused_stdout}"
            ));
        }
        let asserted = run_ripr(&[
            "explain",
            "--root",
            &root_str,
            "--from",
            &artifact_arg,
            "--base",
            "HEAD",
            &selector,
        ]);
        assert_success(&asserted);
        assert_eq!(
            reused.stdout, asserted.stdout,
            "a matching --base assertion must not change reused output"
        );

        // A mismatched --base assertion fails closed naming diff_source.base.
        let wrong_base = run_ripr(&[
            "explain",
            "--root",
            &root_str,
            "--from",
            &artifact_arg,
            "--base",
            "main",
            &selector,
        ]);
        assert_failure(&wrong_base);
        let stderr = String::from_utf8_lossy(&wrong_base.stderr);
        if !(stderr.contains("asserted scope does not match")
            && stderr.contains("diff_source.base"))
        {
            return Err(format!(
                "mismatched --base assertion must be named:\n{stderr}"
            ));
        }

        // Worktree drift between write and reuse fails closed naming
        // diff_bytes_hash — never a silent recompute.
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold && amount > 0\n}\n",
        )
        .map_err(|err| format!("re-dirty lib.rs: {err}"))?;
        let drifted = run_ripr(&[
            "explain",
            "--root",
            &root_str,
            "--from",
            &artifact_arg,
            &selector,
        ]);
        assert_failure(&drifted);
        let stderr = String::from_utf8_lossy(&drifted.stderr);
        if !(stderr.contains("cannot be reused") && stderr.contains("diff_bytes_hash")) {
            return Err(format!("worktree drift must be named:\n{stderr}"));
        }
        Ok(())
    })();
    ignore_remove_dir_all(&root);
    result
}

/// An artifact written with a CLI-only non-default `--mode` is consumable
/// when the same flag is passed on the reuse side, and fails closed naming
/// `mode` when it is not. Finding detail remains equivalent while navigation
/// names the source identity used by each invocation.
#[test]
fn explain_from_consumes_artifact_written_with_non_default_mode() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let dir = unique_temp_workspace("check-artifact-mode");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let artifact = dir.join("ready.json");
    let artifact_arg = artifact.display().to_string();
    let selector = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250";

    let check = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--mode",
        "ready",
        "--write-artifact",
        &artifact_arg,
    ]);
    assert_success(&check);

    let fresh = run_ripr(&[
        "explain", "--root", &root, "--diff", &diff, "--mode", "ready", selector,
    ]);
    assert_success(&fresh);
    let reused = run_ripr(&[
        "explain",
        "--root",
        &root,
        "--from",
        &artifact_arg,
        "--mode",
        "ready",
        selector,
    ]);
    assert_success(&reused);
    let fresh_text = String::from_utf8_lossy(&fresh.stdout);
    let reused_text = String::from_utf8_lossy(&reused.stdout);
    let fresh_detail = fresh_text
        .lines()
        .filter(|line| !line.starts_with("Next: ripr context "))
        .collect::<Vec<_>>();
    let reused_detail = reused_text
        .lines()
        .filter(|line| !line.starts_with("Next: ripr context "))
        .collect::<Vec<_>>();
    assert_eq!(
        fresh_detail, reused_detail,
        "explain --from --mode ready must preserve finding detail"
    );
    assert!(fresh_text.contains("Next: ripr context --root"));
    assert!(fresh_text.contains("--diff"));
    assert!(fresh_text.contains("--mode ready"));
    assert!(reused_text.contains("Next: ripr context --root"));
    assert!(reused_text.contains("--from"));
    assert!(reused_text.contains("--mode ready"));

    let without_flag = run_ripr(&[
        "explain",
        "--root",
        &root,
        "--from",
        &artifact_arg,
        selector,
    ]);
    assert_failure(&without_flag);
    let stderr = String::from_utf8_lossy(&without_flag.stderr);
    assert!(
        stderr.contains("cannot be reused")
            && stderr.contains("identity mismatch")
            && stderr.contains("mode"),
        "mode mismatch must be named:\n{stderr}"
    );
    ignore_remove_dir_all(&dir);
    Ok(())
}

/// An artifact written with `--no-unchanged-tests` is consumable with the
/// same flag (byte-identical) and fails closed naming
/// `analysis_options.include_unchanged_tests` without it.
#[test]
fn context_from_consumes_artifact_written_with_no_unchanged_tests() -> Result<(), String> {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let dir = unique_temp_workspace("check-artifact-unchanged");
    std::fs::create_dir_all(&dir).map_err(|err| format!("mkdir {}: {err}", dir.display()))?;
    let artifact = dir.join("no-unchanged.json");
    let artifact_arg = artifact.display().to_string();
    let selector = "probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250";

    let check = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--no-unchanged-tests",
        "--write-artifact",
        &artifact_arg,
    ]);
    assert_success(&check);

    let fresh = run_ripr(&[
        "context",
        "--root",
        &root,
        "--diff",
        &diff,
        "--no-unchanged-tests",
        "--at",
        selector,
    ]);
    assert_success(&fresh);
    let reused = run_ripr(&[
        "context",
        "--root",
        &root,
        "--from",
        &artifact_arg,
        "--no-unchanged-tests",
        "--at",
        selector,
    ]);
    assert_success(&reused);
    context_reuse_matches_fresh(&fresh.stdout, &reused.stdout)?;

    let without_flag = run_ripr(&[
        "context",
        "--root",
        &root,
        "--from",
        &artifact_arg,
        "--at",
        selector,
    ]);
    assert_failure(&without_flag);
    let stderr = String::from_utf8_lossy(&without_flag.stderr);
    assert!(
        stderr.contains("cannot be reused")
            && stderr.contains("identity mismatch")
            && stderr.contains("analysis_options.include_unchanged_tests"),
        "include_unchanged_tests mismatch must be named:\n{stderr}"
    );
    ignore_remove_dir_all(&dir);
    Ok(())
}

/// Managed `[perl] producer` packet generation cannot join the recorded
/// identity (the packet is generated inside the analysis run), so
/// `--write-artifact` fails closed with a named limitation. No producer
/// process is spawned: the rejection happens before analysis.
#[test]
fn check_write_artifact_rejects_managed_perl_producer() -> Result<(), String> {
    let dir = unique_temp_workspace("check-artifact-producer");
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample");
    let result = (|| {
        std::fs::create_dir_all(dir.join("src")).map_err(|err| format!("mkdir: {err}"))?;
        std::fs::create_dir_all(dir.join("tests")).map_err(|err| format!("mkdir: {err}"))?;
        std::fs::copy(sample.join("example.diff"), dir.join("example.diff"))
            .map_err(|err| format!("copy: {err}"))?;
        std::fs::copy(sample.join("src/lib.rs"), dir.join("src/lib.rs"))
            .map_err(|err| format!("copy: {err}"))?;
        std::fs::copy(
            sample.join("tests/pricing.rs"),
            dir.join("tests/pricing.rs"),
        )
        .map_err(|err| format!("copy: {err}"))?;
        std::fs::write(
            dir.join("ripr.toml"),
            "[perl]\nproducer = \"perl-ripr-facts\"\n",
        )
        .map_err(|err| format!("write config: {err}"))?;

        let root = dir.display().to_string();
        let diff = dir.join("example.diff").display().to_string();
        let artifact = dir.join("a.json");
        let artifact_arg = artifact.display().to_string();
        let output = run_ripr(&[
            "check",
            "--root",
            &root,
            "--diff",
            &diff,
            "--write-artifact",
            &artifact_arg,
        ]);
        assert_failure(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("producer packet generation") && stderr.contains("--perl-facts"),
            "managed producer limitation must be named:\n{stderr}"
        );
        assert!(
            !artifact.exists(),
            "no artifact may be written for the rejected run shape"
        );
        Ok(())
    })();
    ignore_remove_dir_all(&dir);
    result
}

// -------- suppressions/v1 smoke --------

fn fixture_test_efficiency_with_actionable_test() -> &'static str {
    // One bare smoke_only entry the suppressions test can target by name.
    r#"{
  "schema_version": "0.1",
  "tests": [
    {"name": "cli_prints_help", "path": "tests/cli.rs", "class": "smoke_only"}
  ],
  "metrics": {
    "tests_scanned": 1,
    "reason_counts": {"smoke_oracle_only": 1}
  }
}
"#
}

fn fixture_test_efficiency_with_unrelated_actionable_test() -> &'static str {
    // One actionable entry that reaches an owner the placeholder
    // workspace does not have (and whose name does not appear in any
    // diff finding's related_tests). Diff-scope `ripr+` must filter it
    // out; repo-scope public `ripr+` also keeps it out until TE debt is
    // lifted into the canonical repair / verify / receipt model.
    r#"{
  "schema_version": "0.1",
  "tests": [
    {
      "name": "totally_unrelated_test",
      "path": "tests/elsewhere.rs",
      "class": "smoke_only",
      "reached_owners": ["unrelated::module"]
    }
  ],
  "metrics": {
    "tests_scanned": 1,
    "reason_counts": {"smoke_oracle_only": 1}
  }
}
"#
}

#[test]
fn check_repo_badge_plus_ignores_raw_test_efficiency_suppressions() -> Result<(), String> {
    let suppressions = r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
path = "tests/cli.rs"
reason = "Intentionally broad CLI smoke test."
owner = "devtools"
expires = "2099-09-01"
"#;
    let workspace = make_temp_workspace_with_suppressions(
        Some(fixture_test_efficiency_with_actionable_test()),
        Some(suppressions),
    )?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-plus-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Raw test-efficiency debt is not part of the public canonical repair
    // badge basis, so its suppressions do not move public counts.
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""suppressed_test_efficiency_findings": 0"#));
    // intentional remains 0 — declared_intent and suppressions are distinct.
    assert!(stdout.contains(r#""intentional_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""warnings": []"#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_plus_does_not_promote_expired_raw_te_suppression() -> Result<(), String> {
    let suppressions = r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
path = "tests/cli.rs"
reason = "Was intentionally broad."
owner = "devtools"
expires = "2025-01-01"
"#;
    let workspace = make_temp_workspace_with_suppressions(
        Some(fixture_test_efficiency_with_actionable_test()),
        Some(suppressions),
    )?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-plus-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Raw test-efficiency debt is not counted in the public repo headline,
    // even when a stale TE suppression exists.
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""suppressed_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""warnings": []"#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_plus_fails_when_suppressions_manifest_is_malformed() -> Result<(), String> {
    let suppressions = r#"schema_version = 1

[[suppressions]]
kind = "wishful"
finding_id = "probe:x"
owner = "z"
reason = "y"
"#;
    let workspace = make_temp_workspace_with_suppressions(
        Some(fixture_test_efficiency_with_actionable_test()),
        Some(suppressions),
    )?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-plus-json",
    ]);
    assert!(
        !output.status.success(),
        "malformed suppressions manifest must fail the badge command"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(".ripr/suppressions.toml validation failed"),
        "stderr must name the file: {stderr}"
    );
    assert!(
        stderr.contains("unsupported kind `wishful`"),
        "stderr must name the offending value: {stderr}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_shields_remains_four_fields_with_suppressions_warnings() -> Result<(), String> {
    // An unmatched suppression generates a warning. The Shields shape must
    // stay exactly four fields and never leak warnings text.
    let suppressions = r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:does_not_match_any_finding"
owner = "z"
reason = "ghost selector"
"#;
    let workspace = make_temp_workspace_with_suppressions(None, Some(suppressions))?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &diff,
        "--format",
        "badge-shields",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    for forbidden in [r#""warnings""#, "ghost", "did not match"] {
        assert!(
            !stdout.contains(forbidden),
            "Shields projection must not leak `{forbidden}`: {stdout}"
        );
    }
    let top_level = stdout.lines().filter(|l| l.starts_with("  \"")).count();
    assert_eq!(top_level, 4);

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_diff_badge_plus_excludes_unrelated_repo_wide_test_efficiency_debt() -> Result<(), String> {
    // Pin the load-bearing semantic fix: diff-scoped `ripr+` must NOT
    // sum unrelated whole-repo test-efficiency debt into the headline.
    // The fixture has one actionable entry whose reached_owners do not
    // intersect anything the diff touches, so the diff-filtered
    // unsuppressed count stays at 0.
    let workspace = make_temp_workspace(Some(
        fixture_test_efficiency_with_unrelated_actionable_test(),
    ))?;
    let root = workspace.display().to_string();
    // A complete diff with zero behavioral candidates (#4375): a valid
    // unified diff whose only hunk touches a docs file, so no source
    // owners and no related tests change. A header-only diff cannot
    // serve here anymore: a stream that parses file headers but zero
    // hunk bodies is typed as truncated, not as a complete empty
    // analysis, so its badge headline is an analysis-incomplete message
    // rather than the filtered count. The unrelated TE entry must be
    // filtered out under diff scope regardless.
    let docs_only_diff = workspace.join("docs_only.patch");
    std::fs::write(
        &docs_only_diff,
        r#"diff --git a/README.md b/README.md
index 0000000..1111111 100644
--- a/README.md
+++ b/README.md
@@ -1,1 +1,2 @@
 # fixture
+a line
"#,
    )
    .map_err(|e| format!("write docs_only.patch: {e}"))?;
    let output = run_ripr(&[
        "check",
        "--root",
        &root,
        "--diff",
        &docs_only_diff.display().to_string(),
        "--format",
        "badge-plus-json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""scope": "diff""#));
    assert!(
        stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#),
        "diff-scope `ripr+` must filter out unrelated repo-wide TE debt: {stdout}"
    );
    // The headline must reflect the filter: no behavioral candidates
    // (docs-only diff) and no unrelated TE debt = 0.
    assert!(stdout.contains(r#""message": "0""#));

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_badge_plus_does_not_count_unlifted_repo_wide_test_efficiency() -> Result<(), String> {
    // Companion to the diff-scope filter test: repo-scope public `ripr+`
    // also keeps raw TE debt out until it is projected into the same
    // canonical repair model as gap records.
    let workspace = make_temp_workspace(Some(
        fixture_test_efficiency_with_unrelated_actionable_test(),
    ))?;
    let root = workspace.display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--format", "repo-badge-plus-json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(
        stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#),
        "repo-scope public `ripr+` must not count unlifted TE findings: {stdout}"
    );

    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// Helper: run the ripr binary with extra env vars set.
fn run_ripr_with_env(args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let mut cmd = std::process::Command::new(bin);
    cmd.args(args);
    for (key, val) in env {
        cmd.env(key, val);
    }
    cmd.output().unwrap()
}

/// Create a temp workspace with exactly two production functions so that
/// `RIPR_REPO_EXPOSURE_SEAM_LIMIT=1` produces real truncation (analyzed < total).
fn make_two_seam_workspace() -> Result<PathBuf, String> {
    let dir = unique_temp_workspace("seam-limit-smoke");
    std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("create src: {e}"))?;
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname=\"ripr-seam-limit-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|e| format!("write Cargo.toml: {e}"))?;
    // Two predicate-boundary functions -> at least 2 seams.
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn above_min(value: i32, min: i32) -> bool {\n    value >= min\n}\n\npub fn below_max(value: i32, max: i32) -> bool {\n    value <= max\n}\n",
    )
    .map_err(|e| format!("write src/lib.rs: {e}"))?;
    Ok(dir)
}

#[test]
fn check_repo_exposure_json_run_status_seam_limit_applied_and_complete() -> Result<(), String> {
    let workspace = make_two_seam_workspace()?;
    let root = workspace
        .to_str()
        .ok_or("workspace path is not valid UTF-8")?;

    // --- Limited run (RIPR_REPO_EXPOSURE_SEAM_LIMIT=1) ---
    let limited = run_ripr_with_env(
        &["check", "--root", root, "--format", "repo-exposure-json"],
        &[("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "1")],
    );
    if !limited.status.success() {
        return Err(format!(
            "limited run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&limited.stdout),
            String::from_utf8_lossy(&limited.stderr)
        ));
    }
    let limited_stdout = String::from_utf8_lossy(&limited.stdout);
    let limited_json: serde_json::Value = serde_json::from_str(&limited_stdout)
        .map_err(|e| format!("parse limited run JSON: {e}\n{limited_stdout}"))?;

    if limited_json
        .pointer("/run_status")
        .and_then(serde_json::Value::as_str)
        != Some("seam_limit_applied")
    {
        return Err(format!(
            "expected run_status=seam_limit_applied, got: {:?}\n{limited_stdout}",
            limited_json.pointer("/run_status")
        ));
    }
    let category = limited_json
        .pointer("/limitations/0/category")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected limitations[0].category in:\n{limited_stdout}"))?;
    if category != "repo_seam_limit_applied" {
        return Err(format!(
            "expected category=repo_seam_limit_applied, got: {category}"
        ));
    }
    let seams_analyzed = limited_json
        .pointer("/limitations/0/seams_analyzed")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("expected limitations[0].seams_analyzed in:\n{limited_stdout}"))?;
    if seams_analyzed != 1 {
        return Err(format!("expected seams_analyzed=1, got: {seams_analyzed}"));
    }
    let seams_total = limited_json
        .pointer("/limitations/0/seams_total")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("expected limitations[0].seams_total in:\n{limited_stdout}"))?;
    if seams_total < 2 {
        return Err(format!(
            "expected seams_total>=2 for truncation to be real, got: {seams_total}"
        ));
    }
    let control = limited_json
        .pointer("/limitations/0/control")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected limitations[0].control in:\n{limited_stdout}"))?;
    if control != "RIPR_REPO_EXPOSURE_SEAM_LIMIT" {
        return Err(format!(
            "expected control=RIPR_REPO_EXPOSURE_SEAM_LIMIT, got: {control}"
        ));
    }

    // --- Complete run (env var absent) ---
    let complete = run_ripr_with_env(
        &["check", "--root", root, "--format", "repo-exposure-json"],
        &[],
    );
    if !complete.status.success() {
        return Err(format!(
            "complete run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&complete.stdout),
            String::from_utf8_lossy(&complete.stderr)
        ));
    }
    let complete_stdout = String::from_utf8_lossy(&complete.stdout);
    let complete_json: serde_json::Value = serde_json::from_str(&complete_stdout)
        .map_err(|e| format!("parse complete run JSON: {e}\n{complete_stdout}"))?;

    if complete_json
        .pointer("/run_status")
        .and_then(serde_json::Value::as_str)
        != Some("complete")
    {
        return Err(format!(
            "expected run_status=complete, got: {:?}\n{complete_stdout}",
            complete_json.pointer("/run_status")
        ));
    }
    if complete_json.pointer("/limitations").is_some() {
        return Err(format!(
            "expected no limitations key on complete run:\n{complete_stdout}"
        ));
    }

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_exposure_json_limit_source_configured_when_env_set() -> Result<(), String> {
    let workspace = make_two_seam_workspace()?;
    let root = workspace
        .to_str()
        .ok_or("workspace path is not valid UTF-8")?;

    let limited = run_ripr_with_env(
        &["check", "--root", root, "--format", "repo-exposure-json"],
        &[("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "1")],
    );
    if !limited.status.success() {
        return Err(format!(
            "limited run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&limited.stdout),
            String::from_utf8_lossy(&limited.stderr)
        ));
    }
    let limited_stdout = String::from_utf8_lossy(&limited.stdout);
    let limited_json: serde_json::Value = serde_json::from_str(&limited_stdout)
        .map_err(|e| format!("parse limited run JSON: {e}\n{limited_stdout}"))?;

    let limit_source = limited_json
        .pointer("/limitations/0/limit_source")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected limitations[0].limit_source in:\n{limited_stdout}"))?;
    if limit_source != "configured" {
        return Err(format!(
            "expected limit_source=configured when env is set, got: {limit_source}"
        ));
    }

    ignore_remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_repo_exposure_json_cache_roundtrip_preserves_seam_limit_applied() -> Result<(), String> {
    let workspace = make_two_seam_workspace()?;
    let root = workspace
        .to_str()
        .ok_or("workspace path is not valid UTF-8")?;

    // First run (cold) — warms the cache.
    let first = run_ripr_with_env(
        &["check", "--root", root, "--format", "repo-exposure-json"],
        &[("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "1")],
    );
    if !first.status.success() {
        return Err(format!(
            "first (cold) run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&first.stdout),
            String::from_utf8_lossy(&first.stderr)
        ));
    }
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let first_json: serde_json::Value = serde_json::from_str(&first_stdout)
        .map_err(|e| format!("parse first run JSON: {e}\n{first_stdout}"))?;

    if first_json
        .pointer("/run_status")
        .and_then(serde_json::Value::as_str)
        != Some("seam_limit_applied")
    {
        return Err(format!(
            "first (cold) run must report seam_limit_applied, got: {:?}\n{first_stdout}",
            first_json.pointer("/run_status")
        ));
    }

    // Second run (warm — cache hit) — MUST also report seam_limit_applied.
    let second = run_ripr_with_env(
        &["check", "--root", root, "--format", "repo-exposure-json"],
        &[("RIPR_REPO_EXPOSURE_SEAM_LIMIT", "1")],
    );
    if !second.status.success() {
        return Err(format!(
            "second (warm) run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&second.stdout),
            String::from_utf8_lossy(&second.stderr)
        ));
    }
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    let second_json: serde_json::Value = serde_json::from_str(&second_stdout)
        .map_err(|e| format!("parse second run JSON: {e}\n{second_stdout}"))?;

    if second_json
        .pointer("/run_status")
        .and_then(serde_json::Value::as_str)
        != Some("seam_limit_applied")
    {
        return Err(format!(
            "second (warm/cache-hit) run must also report seam_limit_applied (NOT complete); \
             a cache hit must NOT erase the seam-limit status.\n\
             Got: {:?}\n{second_stdout}",
            second_json.pointer("/run_status")
        ));
    }

    ignore_remove_dir_all(&workspace);
    Ok(())
}

// ── ripr receipt write / check smoke tests (RIPR-SPEC-0079) ──────────────────

/// Smoke: `ripr receipt write` with valid args writes a receipt JSON and
/// `ripr receipt check <path>` validates it, both exit 0.
#[test]
fn receipt_write_then_check_exits_zero() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = unique_temp_workspace("receipt-write-check");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("r.json");
    let out_path_str = out_path.to_str().ok_or("out_path is not valid UTF-8")?;

    // Write the receipt.
    let write_output = run_ripr(&[
        "receipt",
        "write",
        "--gap",
        "demo:gap:1",
        "--verify-command",
        "cargo test",
        "--status",
        "passed",
        "--out",
        out_path_str,
        "--json",
    ]);
    assert_success(&write_output);
    let write_stdout = String::from_utf8_lossy(&write_output.stdout);
    assert!(
        write_stdout.contains("\"schema_version\""),
        "receipt write should print JSON, got: {write_stdout}"
    );

    // Parse and validate required fields.
    let value: serde_json::Value = serde_json::from_str(&write_stdout)
        .map_err(|e| format!("receipt JSON did not parse: {e}\nstdout: {write_stdout}"))?;
    assert_eq!(value["schema_version"], "0.1", "schema_version mismatch");
    assert_eq!(value["kind"], "receipt", "kind mismatch");
    assert_eq!(
        value["canonical_gap_id"], "demo:gap:1",
        "canonical_gap_id mismatch"
    );
    assert_eq!(value["verify_status"], "passed", "verify_status mismatch");
    assert_eq!(
        value["verify_command"], "cargo test",
        "verify_command mismatch"
    );
    assert_eq!(
        value["packet_id"],
        serde_json::Value::Null,
        "packet_id should be null"
    );
    assert_eq!(
        value["packet_id_available"], false,
        "packet_id_available should be false"
    );
    assert_eq!(
        value["current_head"].as_str().unwrap_or("").len(),
        40,
        "current_head should be the observed Git SHA"
    );
    assert!(
        value["written_at"].as_str().unwrap_or("").contains('T'),
        "written_at should be RFC3339"
    );

    // Check the receipt using positional path argument.
    let check_output = run_ripr(&["receipt", "check", out_path_str]);
    assert_success(&check_output);
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        check_stdout.contains("structurally valid"),
        "receipt check should report valid, got: {check_stdout}"
    );

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

/// Smoke: a long canonical gap ID stays within the bounded default path and
/// round-trips through `receipt check --gap` from an isolated Git repository.
#[test]
fn receipt_default_long_gap_path_round_trips_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("receipt-default-long-gap");
    std::fs::create_dir_all(&workspace)?;
    run_git(&workspace, &["init"])?;
    run_git(&workspace, &["config", "user.email", "test@test.com"])?;
    run_git(&workspace, &["config", "user.name", "Test"])?;
    std::fs::write(workspace.join("README.md"), "receipt path fixture\n")?;
    run_git(&workspace, &["add", "."])?;
    run_git(&workspace, &["commit", "-m", "initial"])?;

    let gap_id = format!(
        "gap:windows:long:{}",
        "segment-with-punctuation/".repeat(32)
    );
    let bin = env!("CARGO_BIN_EXE_ripr");
    let write = run_command(
        bin,
        Some(&workspace),
        &[
            "receipt",
            "write",
            "--gap",
            gap_id.as_str(),
            "--verify-command",
            "cargo test",
            "--status",
            "passed",
            "--json",
        ],
    )?;
    assert_success(&write);
    let written: serde_json::Value = serde_json::from_slice(&write.stdout)?;
    assert_eq!(written["canonical_gap_id"], gap_id);

    let check = run_command(
        bin,
        Some(&workspace),
        &["receipt", "check", "--gap", gap_id.as_str()],
    )?;
    assert_success(&check);
    assert!(String::from_utf8_lossy(&check.stdout).contains("structurally valid"));

    std::fs::remove_dir_all(workspace)?;
    Ok(())
}

/// Smoke: `ripr receipt write` with `--packet` records packet_id correctly.
#[test]
fn receipt_write_with_packet_id_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = unique_temp_workspace("receipt-with-packet");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("r.json");
    let out_path_str = out_path.to_str().ok_or("out_path is not valid UTF-8")?;

    let write_output = run_ripr(&[
        "receipt",
        "write",
        "--gap",
        "demo:gap:1",
        "--packet",
        "packet-abc123",
        "--verify-command",
        "cargo test",
        "--status",
        "not_run",
        "--out",
        out_path_str,
    ]);
    assert_success(&write_output);
    let stdout = String::from_utf8_lossy(&write_output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("receipt JSON did not parse: {e}\nstdout: {stdout}"))?;
    assert_eq!(
        value["packet_id"], "packet-abc123",
        "packet_id should be set"
    );
    assert_eq!(
        value["packet_id_available"], true,
        "packet_id_available should be true"
    );
    assert_eq!(value["verify_status"], "not_run");

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

/// Smoke: `ripr receipt write` with missing `--gap` exits non-zero with a
/// clear error message.
#[test]
fn receipt_write_missing_gap_exits_nonzero_smoke() {
    let output = run_ripr(&[
        "receipt",
        "write",
        "--verify-command",
        "cargo test",
        "--status",
        "passed",
    ]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("canonical_gap_id"),
        "error should mention canonical_gap_id; got stderr={stderr} stdout={stdout}"
    );
}

/// Smoke: `ripr receipt write` with invalid `--status` exits non-zero with
/// a clear message listing valid values.
#[test]
fn receipt_write_invalid_status_exits_nonzero_smoke() {
    let output = run_ripr(&[
        "receipt",
        "write",
        "--gap",
        "demo:gap:1",
        "--verify-command",
        "cargo test",
        "--status",
        "bogus",
    ]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("bogus"),
        "error should echo bad status; got stderr={stderr} stdout={stdout}"
    );
    assert!(
        combined.contains("passed"),
        "error should list valid values; got stderr={stderr} stdout={stdout}"
    );
}

/// Smoke: `ripr receipt check` on a missing file exits non-zero.
#[test]
fn receipt_check_missing_file_exits_nonzero_smoke() {
    let output = run_ripr(&[
        "receipt",
        "check",
        "--path",
        "target/ripr/receipts/completely-nonexistent-12345.json",
    ]);
    assert_failure(&output);
}

/// Smoke: `ripr receipt --help` exits 0 and mentions expected content.
#[test]
fn receipt_help_exits_zero_smoke() {
    let output = run_ripr(&["receipt", "--help"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ripr receipt write"),
        "help should mention write"
    );
    assert!(
        stdout.contains("ripr receipt check"),
        "help should mention check"
    );
    assert!(
        stdout.contains("RIPR-SPEC-0079"),
        "help should reference spec"
    );
}

/// Smoke (RIPR-SPEC-0110 control 4): `ripr receipt check --ledger` where the
/// receipt's canonical_gap_id is NOT in the ledger exits non-zero with
/// `orphan_receipt`.
#[test]
fn receipt_check_orphan_exits_nonzero() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = unique_temp_workspace("receipt-check-orphan");
    std::fs::create_dir_all(&out_dir)?;

    // Write a receipt for a gap that will NOT appear in the ledger.
    let receipt_path = out_dir.join("receipt.json");
    let current_head = run_command("git", Some(&workspace_root()), &["rev-parse", "HEAD"])?;
    let current_head = String::from_utf8(current_head.stdout)?.trim().to_string();
    let receipt_json = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "receipt",
        "canonical_gap_id": "gap:orphan:aabbccdd",
        "verify_command": "cargo test",
        "verify_status": "passed",
        "current_head": current_head,
        "written_at": "2026-06-14T00:00:00Z"
    });
    std::fs::write(&receipt_path, receipt_json.to_string())?;

    // Write a ledger with a DIFFERENT gap — the receipt's gap is absent.
    let ledger_path = out_dir.join("ledger.json");
    let ledger_json = serde_json::json!([{
        "gap_id": "gap:other:12345678",
        "source_currentness": "candidate_current",
        "canonical_gap_id": "gap:other:12345678",
        "kind": "MissingValueAssertion",
        "language": "rust",
        "language_status": "stable",
        "scope": "repo_scoped",
        "evidence_class": "return_value",
        "gap_state": "actionable",
        "policy_state": "new",
        "repairability": "repairable",
        "authority_boundary": "gate_decision_artifact_only"
    }]);
    std::fs::write(&ledger_path, ledger_json.to_string())?;

    let receipt_path_str = receipt_path.to_str().ok_or("receipt path not UTF-8")?;
    let ledger_path_str = ledger_path.to_str().ok_or("ledger path not UTF-8")?;

    let output = run_ripr(&[
        "receipt",
        "check",
        "--path",
        receipt_path_str,
        "--ledger",
        ledger_path_str,
    ]);
    // Must exit non-zero (orphan_receipt is a real error).
    assert_failure(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("orphan_receipt"),
        "output should mention orphan_receipt; got: {combined}"
    );

    ignore_remove_dir_all(&out_dir);
    Ok(())
}

/// Smoke: `ripr agent receipt` (legacy alias) still exits non-zero with a
/// parse error rather than panicking or silently succeeding with no args —
/// which confirms the alias is still wired.
#[test]
fn agent_receipt_legacy_alias_still_dispatches_smoke() {
    // ripr agent receipt without any args should return a parse error, not panic.
    let output = run_ripr(&["agent", "receipt"]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    // Should say "requires --json" or "requires --verify-json" or similar — any
    // parse error from the agent receipt handler means the alias is dispatching.
    assert!(
        combined.contains("requires") || combined.contains("missing"),
        "agent receipt should still dispatch and return a parse error; got: {combined}"
    );
}

// RIPR-SPEC-0083 regression guards: --mode is a speed tier, not a scope provider.

/// Bug 1 regression guard: `ripr check --mode fast` with no --diff/--base on an
/// empty range must name the compared base instead of claiming no scope was
/// provided (#4012). --mode is a speed tier on the diff path; it does NOT
/// provide analysis scope. The range is established (default base resolved)
/// but empty, so the honest disclosure is "nothing changed", not "no scope".
#[test]
fn check_mode_fast_alone_on_empty_range_names_compared_base_smoke() {
    // Run in a temp git repo with one commit and HEAD up-to-date, so
    // resolve_default_base succeeds but the diff against HEAD is empty.
    let root = unique_temp_workspace("mode-fast-no-scope");
    std::fs::create_dir_all(root.join("src")).unwrap();
    // Pin the initial branch: `resolve_default_base` only recognizes
    // origin/HEAD, origin/main, origin/master, main, and master, so a bare
    // `init` under an exotic `init.defaultBranch` would fail base resolution.
    run_git(&root, &["init", "-b", "main"]).unwrap();
    run_git(&root, &["config", "user.email", "test@test.com"]).unwrap();
    run_git(&root, &["config", "user.name", "Test"]).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mode-fast-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "initial"]).unwrap();
    // HEAD is now up-to-date — diff against HEAD is empty.
    // Route through the shared spawn helper (process-policy budget).
    let root_str = root.to_string_lossy().into_owned();
    let output = run_ripr(&["check", "--root", &root_str, "--mode", "fast"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The empty range must be disclosed as nothing-changed, not as no-scope.
    assert!(
        stdout.contains("nothing to analyze") || stdout.contains("no changed files"),
        "empty range must disclose nothing-changed; got stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.contains("no analysis scope was provided"),
        "empty range must NOT claim no scope was provided; got:\n{stdout}"
    );
    assert!(
        stdout.contains("The compared base was `"),
        "empty range must name the compared base; got:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// #4012: a bare `ripr check` that resolves a default base and analyzes
/// changed files with no probes is an ordinary analyzed-empty result — it
/// must NOT claim no scope was provided, report missing_scope, or offer a
/// scope repair.
#[test]
fn check_bare_run_on_changed_files_shows_no_scope_disclosure_smoke() {
    // Temp repo with an edit commit on a branch off the base branch, so the
    // resolved default range (`<base>...HEAD`) analyzes exactly one changed
    // file with zero probes regardless of the local `git init` default branch
    // name. (Committing the edit on the init branch itself leaves HEAD == the
    // compared base, i.e. an empty range, which is the other test's subject.)
    let root = unique_temp_workspace("bare-check-analyzed-empty");
    std::fs::create_dir_all(root.join("src")).unwrap();
    // Pin the initial branch (see mode-fast-no-scope fixture above): the base
    // commit must land on a branch `resolve_default_base` recognizes.
    run_git(&root, &["init", "-b", "main"]).unwrap();
    run_git(&root, &["config", "user.email", "test@test.com"]).unwrap();
    run_git(&root, &["config", "user.name", "Test"]).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub const VALUE: u32 = 1;\n").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"analyzed-empty-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "base"]).unwrap();
    run_git(&root, &["checkout", "-b", "work"]).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub const VALUE: u32 = 2;\n").unwrap();
    run_git(&root, &["add", "src/lib.rs"]).unwrap();
    run_git(&root, &["commit", "-m", "edit"]).unwrap();
    // Route through the shared spawn helper (process-policy budget).
    let root_str = root.to_string_lossy().into_owned();
    let output = run_ripr(&["check", "--root", &root_str]);
    assert!(
        output.status.success(),
        "bare check on changed files must succeed; got:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("no analysis scope was provided"),
        "analyzed run must NOT claim no scope was provided; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("missing_scope"),
        "analyzed run must NOT report missing_scope; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("provide an analysis scope"),
        "analyzed run must NOT offer a scope repair; got:\n{stdout}"
    );
    ignore_remove_dir_all(&root);
}

// #2644 regression guards: the fast-mode no-op notice is a property of the
// EFFECTIVE (config-merged) mode, not of argv position. #2851 added the notice
// inside the argv parse loop with no tests; these guards pin the resolved-mode
// behavior and the stderr-only contract.

/// Wording-independent fragment of the #2644 notice. Matching the claim rather
/// than the full sentence keeps these guards about the emission site, and makes
/// the negative guards fail on any fast-mode notice, however it is phrased.
const FAST_MODE_NOOP_NOTICE: &str = "fast is currently identical to";

/// A committed single-package git repo for `check --root <repo>` runs.
fn fast_mode_notice_workspace(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fast-mode-notice-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    init_git_fixture_repo(&root)?;
    Ok(root)
}

/// A repo `ripr.toml` with `mode = "fast"` resolves to the fast tier just as
/// `--mode fast` does, so it must get the same no-op notice. Before the fix the
/// notice was tied to the `--mode` argv arm and config-derived fast was silent.
#[test]
fn check_fast_mode_notice_fires_for_config_derived_mode_smoke()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fast_mode_notice_workspace("fast-notice-config")?;
    let root_str = root.display().to_string();

    let without_config = run_ripr(&["check", "--root", &root_str]);
    let without_config_stderr = String::from_utf8_lossy(&without_config.stderr).into_owned();

    std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"fast\"\n")?;
    let with_config = run_ripr(&["check", "--root", &root_str]);
    let with_config_stderr = String::from_utf8_lossy(&with_config.stderr).into_owned();

    ignore_remove_dir_all(&root);
    assert!(
        !without_config_stderr.contains(FAST_MODE_NOOP_NOTICE),
        "default mode must not emit the fast no-op notice; got stderr:\n{without_config_stderr}"
    );
    assert!(
        with_config_stderr.contains(FAST_MODE_NOOP_NOTICE),
        "ripr.toml [analysis] mode = \"fast\" must emit the fast no-op notice; got stderr:\n{with_config_stderr}"
    );
    Ok(())
}

/// `--mode fast --mode deep` resolves to deep, so the notice must not fire —
/// argv-position emission warned about a mode the run never used. stdout must
/// stay byte-identical to the same run without the overridden `--mode fast`.
#[test]
fn check_fast_mode_notice_is_silent_when_a_later_mode_wins_smoke()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fast_mode_notice_workspace("fast-notice-overridden")?;
    let root_str = root.display().to_string();

    let overridden = run_ripr(&[
        "check", "--root", &root_str, "--mode", "fast", "--mode", "deep", "--json",
    ]);
    let deep_only = run_ripr(&["check", "--root", &root_str, "--mode", "deep", "--json"]);
    let overridden_stderr = String::from_utf8_lossy(&overridden.stderr).into_owned();

    ignore_remove_dir_all(&root);
    assert!(
        !overridden_stderr.contains(FAST_MODE_NOOP_NOTICE),
        "--mode fast --mode deep resolves to deep and must not emit the fast no-op notice; got stderr:\n{overridden_stderr}"
    );
    assert_eq!(
        overridden.stdout, deep_only.stdout,
        "the fast no-op notice is stderr-only; stdout must not change"
    );
    assert_eq!(
        overridden.status.code(),
        deep_only.status.code(),
        "the fast no-op notice must not change the exit code"
    );
    Ok(())
}

/// A repeated `--mode fast` resolves to one effective mode, so the notice fires
/// once. Argv-position emission printed it once per occurrence.
#[test]
fn check_fast_mode_notice_is_emitted_once_for_repeated_mode_flags_smoke()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fast_mode_notice_workspace("fast-notice-repeated")?;
    let root_str = root.display().to_string();

    let repeated = run_ripr(&[
        "check", "--root", &root_str, "--mode", "fast", "--mode", "fast", "--json",
    ]);
    let single = run_ripr(&["check", "--root", &root_str, "--mode", "fast", "--json"]);
    let repeated_stderr = String::from_utf8_lossy(&repeated.stderr).into_owned();
    let single_stderr = String::from_utf8_lossy(&single.stderr).into_owned();

    ignore_remove_dir_all(&root);
    assert_eq!(
        repeated_stderr.matches(FAST_MODE_NOOP_NOTICE).count(),
        1,
        "--mode fast --mode fast must emit the fast no-op notice once; got stderr:\n{repeated_stderr}"
    );
    assert_eq!(
        single_stderr.matches(FAST_MODE_NOOP_NOTICE).count(),
        1,
        "--mode fast must emit the fast no-op notice once; got stderr:\n{single_stderr}"
    );
    assert_eq!(
        repeated.stdout, single.stdout,
        "the fast no-op notice is stderr-only; stdout must not change"
    );
    Ok(())
}

/// The notice is emitted before the gap-ledger and repo-exposure-json early
/// returns, so repo-scoped formats keep it. A later emission site would drop
/// the notice on exactly the paths that return before rendering findings.
#[test]
fn check_fast_mode_notice_precedes_repo_scoped_early_returns_smoke()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fast_mode_notice_workspace("fast-notice-repo-scope")?;
    let root_str = root.display().to_string();
    let ledger = root.join("gap-ledger.json");
    std::fs::write(&ledger, "{\"records\": []}\n")?;
    let ledger_str = ledger.display().to_string();

    let repo_exposure = run_ripr(&[
        "check",
        "--root",
        &root_str,
        "--mode",
        "fast",
        "--format",
        "repo-exposure-json",
    ]);
    let gap_ledger = run_ripr(&[
        "check",
        "--root",
        &root_str,
        "--mode",
        "fast",
        "--format",
        "repo-badge-json",
        "--gap-ledger",
        &ledger_str,
    ]);
    let repo_exposure_stderr = String::from_utf8_lossy(&repo_exposure.stderr).into_owned();
    let gap_ledger_stderr = String::from_utf8_lossy(&gap_ledger.stderr).into_owned();
    let repo_exposure_stdout = String::from_utf8_lossy(&repo_exposure.stdout).into_owned();
    let gap_ledger_stdout = String::from_utf8_lossy(&gap_ledger.stdout).into_owned();

    ignore_remove_dir_all(&root);
    assert!(
        repo_exposure_stderr.contains(FAST_MODE_NOOP_NOTICE),
        "--format repo-exposure-json must still get the fast no-op notice; got stderr:\n{repo_exposure_stderr}"
    );
    assert!(
        gap_ledger_stderr.contains(FAST_MODE_NOOP_NOTICE),
        "--gap-ledger must still get the fast no-op notice; got stderr:\n{gap_ledger_stderr}"
    );
    assert!(
        repo_exposure_stdout.starts_with('{'),
        "repo-exposure-json stdout must stay machine-readable JSON; got:\n{repo_exposure_stdout}"
    );
    assert!(
        gap_ledger_stdout.starts_with('{'),
        "repo-badge-json stdout must stay machine-readable JSON; got:\n{gap_ledger_stdout}"
    );
    Ok(())
}

/// Regression guard: `ripr check --base origin/main` (real diff scope) must NOT
/// show the no-scope disclosure when the result is empty.
#[test]
fn check_with_base_scope_does_not_show_no_scope_disclosure_smoke() {
    // Use the in-repo sample diff workspace which has an origin/main analog.
    // We just need to confirm the disclosure is absent when scope is provided.
    // The workspace check may fail (no commits, no origin), but IF it succeeds
    // with 0 findings, the disclosure must be absent.
    let root = unique_temp_workspace("base-scope-no-disclosure");
    std::fs::create_dir_all(root.join("src")).unwrap();
    run_git(&root, &["init"]).unwrap();
    run_git(&root, &["config", "user.email", "test@test.com"]).unwrap();
    run_git(&root, &["config", "user.name", "Test"]).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"no-scope-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "initial"]).unwrap();
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root_str = root.to_string_lossy().into_owned();
    let output = std::process::Command::new(bin)
        .args(["check", "--root", &root_str, "--base", "HEAD"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // With explicit --base, the disclosure must NOT fire even if there are 0 findings.
    assert!(
        !stdout.contains("no analysis scope was provided"),
        "check --base HEAD must NOT show no-scope disclosure; got stdout:\n{stdout}"
    );
    ignore_remove_dir_all(&root);
}

// RIPR-SPEC-0112 regression guards: --base must disclose uncommitted working-tree changes.

/// RIPR-SPEC-0112: `ripr check --base HEAD --json` with an uncommitted change to a
/// tracked .rs file must emit `unanalyzed_working_tree: true` in JSON and the human
/// Note in stdout. The committed diff vs HEAD is empty (no new commits), so findings
/// are zero — this is the false-clean case the disclosure must prevent.
#[test]
fn check_base_head_with_uncommitted_edit_shows_unanalyzed_working_tree_disclosure() {
    let root = unique_temp_workspace("unanalyzed-wt-fires");
    std::fs::create_dir_all(root.join("src")).unwrap();
    run_git(&root, &["init"]).unwrap();
    run_git(&root, &["config", "user.email", "test@test.com"]).unwrap();
    run_git(&root, &["config", "user.name", "Test"]).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0112-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "initial"]).unwrap();
    // Make an UNCOMMITTED edit to a tracked .rs file — this is the uncommitted change
    // that --base HEAD will not analyze.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b + 1 }\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root_str = root.to_string_lossy().into_owned();
    // JSON mode: assert unanalyzed_working_tree == true
    let output = std::process::Command::new(bin)
        .args(["check", "--root", &root_str, "--base", "HEAD", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"unanalyzed_working_tree\": true"),
        "check --base HEAD with uncommitted edit must emit unanalyzed_working_tree: true in JSON; got:\n{stdout}"
    );
    // Human mode: assert the Note is present
    let output_human = std::process::Command::new(bin)
        .args(["check", "--root", &root_str, "--base", "HEAD"])
        .output()
        .unwrap();
    let human = String::from_utf8_lossy(&output_human.stdout);
    assert!(
        human.contains("uncommitted source and test changes were not analyzed"),
        "check --base HEAD with uncommitted edit must show Note in human output; got:\n{human}"
    );
    // The note must name the remedy that works. Staging does not change a
    // committed-history diff, so "commit or stage" was a false remedy.
    assert!(
        human.contains("add `--worktree`"),
        "the disclosure must name --worktree as the remedy; got:\n{human}"
    );
    assert!(
        !human.contains("commit or stage"),
        "the disclosure must not suggest staging, which leaves a --base diff unchanged; got:\n{human}"
    );
    ignore_remove_dir_all(&root);
}

/// RIPR-SPEC-0112: a committed-history check reads every source and test file
/// as committed at HEAD. An uncommitted test edit, or a new untracked test,
/// must not move the counts while the note says the changes were not analyzed
/// (0.11 RC walk: the summary moved 0 -> 1 while the note fired). `--worktree`
/// must see the same edit, so the fixture discriminates. A README-only edit
/// changes nothing an adapter reads and raises no note.
#[test]
fn check_base_reads_tests_as_committed_and_notes_only_source_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::env::temp_dir().join(format!("ripr-0112-committed-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    run_git(&root, &["init", "-q"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"committed-tests\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn ok(n: u32) -> bool { n > 10 }\n",
    )?;
    std::fs::write(
        root.join("tests/ok.rs"),
        "#[test]\nfn t() { let _ = committed_tests::ok(20); }\n",
    )?;
    std::fs::write(root.join("README.md"), "committed tests\n")?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-q", "-m", "base"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn ok(n: u32) -> bool { n >= 10 }\n",
    )?;
    run_git(&root, &["commit", "-q", "-a", "-m", "change"])?;
    let root_str = root.to_string_lossy().into_owned();
    let check = |extra: &[&str]| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut args = vec!["check", "--root", root_str.as_str(), "--base", "HEAD~1"];
        args.extend_from_slice(extra);
        args.push("--json");
        let output = run_ripr(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(serde_json::from_slice(&output.stdout)?)
    };
    let noted = |json: &serde_json::Value| json["unanalyzed_working_tree"] == true;
    let committed = check(&[])?;
    assert!(!noted(&committed));
    let discriminating_test =
        "#[test]\nfn t() { assert!(committed_tests::ok(10)); assert!(!committed_tests::ok(9)); }\n";

    // Uncommitted test edit: plain check keeps the committed result and notes
    // the change; --worktree reads the edit and moves.
    std::fs::write(root.join("tests/ok.rs"), discriminating_test)?;
    let plain = check(&[])?;
    assert_eq!(plain["summary"], committed["summary"]);
    assert_eq!(plain["findings"], committed["findings"]);
    assert!(noted(&plain));
    let worktree = check(&["--worktree"])?;
    assert_ne!(
        worktree["summary"], committed["summary"],
        "fixture must discriminate: the edited test changes a --worktree result"
    );
    run_git(&root, &["checkout", "-q", "--", "tests/ok.rs"])?;

    // A new untracked test is not committed content either.
    std::fs::write(root.join("tests/new.rs"), discriminating_test)?;
    let untracked = check(&[])?;
    assert_eq!(untracked["summary"], committed["summary"]);
    assert!(noted(&untracked));
    std::fs::remove_file(root.join("tests/new.rs"))?;

    // A README edit changes nothing an adapter reads: no note.
    std::fs::write(root.join("README.md"), "edited\n")?;
    let readme = check(&[])?;
    assert_eq!(readme["summary"], committed["summary"]);
    assert!(!noted(&readme));
    ignore_remove_dir_all(&root);
    Ok(())
}

/// RIPR-SPEC-0112 (default base): bare `ripr check` resolves the default base
/// and diffs committed history exactly like an explicit `--base`, so an
/// uncommitted tracked edit is excluded there too and must be disclosed. This
/// is the first-run path: edit a file, run `ripr check`, see nothing.
#[test]
fn check_default_base_with_uncommitted_edit_shows_unanalyzed_working_tree_disclosure()
-> Result<(), String> {
    let root = unique_temp_workspace("unanalyzed-wt-default-base");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init", "-b", "main"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|err| format!("write base lib.rs: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0112-default-base-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold\n}\n",
    )
    .map_err(|err| format!("write dirty lib.rs: {err}"))?;
    let root_str = root.to_string_lossy().into_owned();

    // Fixture precondition: the default base resolves to `main` == HEAD, so
    // the committed diff is empty while the edit exists only in the worktree.
    let worktree = run_ripr(&["check", "--root", &root_str, "--worktree", "--json"]);
    assert_success(&worktree);
    let worktree_stdout = String::from_utf8_lossy(&worktree.stdout);
    let worktree_report: serde_json::Value = serde_json::from_str(&worktree_stdout)
        .map_err(|err| format!("parse --worktree JSON: {err}\n{worktree_stdout}"))?;
    let worktree_findings = worktree_report
        .pointer("/findings")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if worktree_findings == 0 {
        return Err(format!(
            "fixture must carry an analyzable uncommitted edit; --worktree found none:\n{worktree_stdout}"
        ));
    }

    let json = run_ripr(&["check", "--root", &root_str, "--json"]);
    assert_success(&json);
    let json_stdout = String::from_utf8_lossy(&json.stdout);
    if !json_stdout.contains("\"unanalyzed_working_tree\": true") {
        return Err(format!(
            "bare check with an uncommitted edit must emit unanalyzed_working_tree: true; got:\n{json_stdout}"
        ));
    }

    let human = run_ripr(&["check", "--root", &root_str]);
    assert_success(&human);
    let human_stdout = String::from_utf8_lossy(&human.stdout);
    if !human_stdout.contains("uncommitted source and test changes were not analyzed")
        || !human_stdout.contains("add `--worktree`")
    {
        return Err(format!(
            "bare check must disclose the excluded edit and name --worktree; got:\n{human_stdout}"
        ));
    }
    // The generic no-scope note recommends `--base origin/main`, which would
    // exclude the same edit; the specific worktree note replaces it.
    if human_stdout.contains("no analysis scope was provided") {
        return Err(format!(
            "bare check must not pair the worktree note with the no-scope remedy; got:\n{human_stdout}"
        ));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// RIPR-SPEC-0112 (default base, clean): a bare `ripr check` on a clean
/// worktree must not claim uncommitted edits were excluded, and keeps the
/// no-scope disclosure for its empty committed diff.
/// #4012: the disclosure on an established-but-empty default range names the
/// compared base instead of the legacy "no analysis scope was provided" note —
/// a scope WAS established (the default base was resolved and compared), so
/// the legacy wording would be the dishonest claim #4012 repairs. The
/// disclosure-only shape (no working-tree note) is what this test pins.
#[test]
fn check_default_base_with_clean_worktree_keeps_no_scope_note_only() -> Result<(), String> {
    let root = unique_temp_workspace("unanalyzed-wt-default-base-clean");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init", "-b", "main"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .map_err(|err| format!("write lib.rs: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0112-default-base-clean\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;
    // An untracked file is outside every diff source and must not trigger the
    // tracked-edit disclosure.
    std::fs::write(root.join("notes.txt"), "scratch\n")
        .map_err(|err| format!("write untracked file: {err}"))?;
    let root_str = root.to_string_lossy().into_owned();

    let human = run_ripr(&["check", "--root", &root_str]);
    assert_success(&human);
    let stdout = String::from_utf8_lossy(&human.stdout);
    if stdout.contains("uncommitted changes") {
        return Err(format!(
            "clean tracked worktree must not disclose uncommitted edits; got:\n{stdout}"
        ));
    }
    if !stdout.contains("contains no changed files") {
        return Err(format!(
            "empty default-base run must keep the no-scope disclosure (base-naming form per #4012); got:\n{stdout}"
        ));
    }
    // Clean-install walk (0.11): Start-here must name the flag that analyzes
    // uncommitted edits, not only "make a change".
    if !stdout.contains("add `--worktree` to include uncommitted edits") {
        return Err(format!(
            "empty-range Start-here must name `--worktree`; got:\n{stdout}"
        ));
    }
    if !stdout.contains("compared base was `main`") || stdout.contains("--base origin/main") {
        return Err(format!(
            "no-origin repo must name its resolved local main rather than suggest a nonexistent remote ref; got:\n{stdout}"
        ));
    }
    let json = run_ripr(&["check", "--root", &root_str, "--json"]);
    assert_success(&json);
    let value: serde_json::Value = serde_json::from_slice(&json.stdout)
        .map_err(|err| format!("parse no-origin check JSON: {err}"))?;
    if value["base"] != "main"
        || !value["scope_disclosures"][0]["why"]
            .as_str()
            .is_some_and(|why| why.contains("main...HEAD") && !why.contains("origin/main"))
    {
        return Err(format!(
            "no-origin JSON must bind guidance to its resolved local base: {value}"
        ));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// RIPR-SPEC-0112: `ripr check --base HEAD` with a CLEAN worktree (no uncommitted changes)
/// must NOT show the unanalyzed-working-tree disclosure. A genuinely clean worktree
/// means no uncommitted edits — the disclosure must not fire.
#[test]
fn check_base_head_with_clean_worktree_does_not_show_unanalyzed_working_tree_disclosure() {
    let root = unique_temp_workspace("unanalyzed-wt-clean");
    std::fs::create_dir_all(root.join("src")).unwrap();
    run_git(&root, &["init"]).unwrap();
    run_git(&root, &["config", "user.email", "test@test.com"]).unwrap();
    run_git(&root, &["config", "user.name", "Test"]).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0112-clean-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "initial"]).unwrap();
    // No uncommitted changes — worktree is clean.
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root_str = root.to_string_lossy().into_owned();
    let output = std::process::Command::new(bin)
        .args(["check", "--root", &root_str, "--base", "HEAD", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("unanalyzed_working_tree"),
        "check --base HEAD with clean worktree must NOT emit unanalyzed_working_tree; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("uncommitted changes"),
        "check --base HEAD with clean worktree must NOT mention uncommitted changes; got:\n{stdout}"
    );
    ignore_remove_dir_all(&root);
}

/// RIPR-SPEC-0116: `ripr check --base HEAD --worktree --json` analyzes the
/// user's uncommitted tracked edit instead of reporting the committed `HEAD`
/// diff as empty.
#[test]
fn check_worktree_base_head_analyzes_uncommitted_tracked_edit() -> Result<(), String> {
    let root = unique_temp_workspace("worktree-mode-dirty");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|err| format!("write base lib.rs: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0116-worktree-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;

    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount > threshold\n}\n",
    )
    .map_err(|err| format!("write dirty lib.rs: {err}"))?;

    let root_str = root.to_string_lossy().into_owned();
    let output = run_ripr(&[
        "check",
        "--root",
        &root_str,
        "--base",
        "HEAD",
        "--worktree",
        "--json",
    ]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("parse check JSON: {err}\n{stdout}"))?;
    let findings = report
        .pointer("/findings")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("expected findings array in JSON:\n{stdout}"))?;
    if findings.is_empty() {
        return Err(format!(
            "--worktree must analyze the uncommitted tracked edit; got no findings:\n{stdout}"
        ));
    }
    if stdout.contains("unanalyzed_working_tree") {
        return Err(format!(
            "--worktree analysis must not claim the tracked edit was excluded:\n{stdout}"
        ));
    }
    if stdout.contains("no_scope_provided") {
        return Err(format!(
            "--worktree must count as an explicit analysis scope:\n{stdout}"
        ));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// RIPR-SPEC-0112: `--diff` analyzes the supplied patch, not committed
/// history, so an uncommitted tracked edit in the same checkout was not
/// "excluded" from it and must not be disclosed as unanalyzed. The fixture is
/// first shown to trigger the disclosure on a `--base` run, so the negative
/// result below is about `--diff`, not about a clean checkout.
#[test]
fn check_with_a_diff_file_does_not_show_unanalyzed_working_tree_disclosure() -> Result<(), String> {
    let root = unique_temp_workspace("unanalyzed-wt-diff-file");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .map_err(|err| format!("write base lib.rs: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0112-diff-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b + 1 }\n",
    )
    .map_err(|err| format!("write dirty lib.rs: {err}"))?;
    let patch = root.join("change.patch");
    std::fs::write(
        &patch,
        "diff --git a/src/lib.rs b/src/lib.rs\n\
--- a/src/lib.rs\n\
+++ b/src/lib.rs\n\
@@ -1 +1 @@\n\
-pub fn add(a: i32, b: i32) -> i32 { a + b }\n\
+pub fn add(a: i32, b: i32) -> i32 { a + b + 1 }\n",
    )
    .map_err(|err| format!("write patch: {err}"))?;
    let root_str = root.to_string_lossy().into_owned();
    let patch_str = patch.to_string_lossy().into_owned();

    // Fixture construction: this checkout does trigger the disclosure on a
    // committed-history run.
    let base_run = run_ripr(&["check", "--root", &root_str, "--base", "HEAD", "--json"]);
    assert_success(&base_run);
    let base_json = String::from_utf8_lossy(&base_run.stdout).into_owned();
    if !base_json.contains("\"unanalyzed_working_tree\": true") {
        return Err(format!(
            "fixture precondition: a --base run on this dirty checkout must disclose it:\n{base_json}"
        ));
    }
    // Same for the human note, so the negative below cannot pass because the
    // note's wording changed.
    let base_human_run = run_ripr(&["check", "--root", &root_str, "--base", "HEAD"]);
    assert_success(&base_human_run);
    let base_human = String::from_utf8_lossy(&base_human_run.stdout).into_owned();
    if !base_human.contains("uncommitted source and test changes were not analyzed") {
        return Err(format!(
            "fixture precondition: a --base run on this dirty checkout must print the note:\n{base_human}"
        ));
    }

    let json_run = run_ripr(&["check", "--root", &root_str, "--diff", &patch_str, "--json"]);
    assert_success(&json_run);
    let json = String::from_utf8_lossy(&json_run.stdout).into_owned();
    if json.contains("unanalyzed_working_tree") {
        return Err(format!(
            "a --diff run must not emit unanalyzed_working_tree:\n{json}"
        ));
    }
    let human_run = run_ripr(&["check", "--root", &root_str, "--diff", &patch_str]);
    assert_success(&human_run);
    let human = String::from_utf8_lossy(&human_run.stdout).into_owned();
    if human.contains("uncommitted source and test changes were not analyzed") {
        return Err(format!(
            "a --diff run must not print the unanalyzed working tree note:\n{human}"
        ));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// RIPR-SPEC-0116: an empty `--worktree` result is honest when the working tree
/// has no tracked changes against the requested base.
#[test]
fn check_worktree_base_head_clean_worktree_has_no_scope_or_unanalyzed_disclosure()
-> Result<(), String> {
    let root = unique_temp_workspace("worktree-mode-clean");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|err| format!("write base lib.rs: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"spec-0116-worktree-clean-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "initial"])?;

    let root_str = root.to_string_lossy().into_owned();
    let output = run_ripr(&[
        "check",
        "--root",
        &root_str,
        "--base",
        "HEAD",
        "--worktree",
        "--json",
    ]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("unanalyzed_working_tree") {
        return Err(format!(
            "clean --worktree result must not emit unanalyzed_working_tree:\n{stdout}"
        ));
    }
    if stdout.contains("no_scope_provided") {
        return Err(format!(
            "clean --worktree result must still count as scoped analysis:\n{stdout}"
        ));
    }
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("parse check JSON: {err}\n{stdout}"))?;
    let findings = report
        .pointer("/findings")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("expected findings array in JSON:\n{stdout}"))?;
    if !findings.is_empty() {
        return Err(format!(
            "clean HEAD-vs-worktree diff should not produce findings:\n{stdout}"
        ));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// #3893: `agent status` builds its printed next commands from the `--root`
/// the user gave. It previously rendered `--root <root>/target/ripr/workflow`,
/// so the command it told a first-time user to run could not succeed.
#[test]
fn agent_status_next_command_uses_the_workspace_root() -> Result<(), String> {
    let root = unique_temp_workspace("agent-status-root");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"agent-status-root\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    // A known seam in an existing workflow directory keeps the legacy
    // before-snapshot route selected; a fresh workspace routes to `ripr pilot`
    // instead (#3906, `agent_status_routes_a_fresh_workspace_to_pilot`).
    std::fs::create_dir_all(root.join("target/ripr/workflow"))
        .map_err(|err| format!("create workflow dir: {err}"))?;
    std::fs::write(
        root.join("target/ripr/workflow/agent-packet.json"),
        r#"{"packets":[{"seam_id":"seam-a"}]}"#,
    )
    .map_err(|err| format!("write packet: {err}"))?;
    let root_str = root.to_string_lossy().into_owned();

    let json = run_ripr(&["agent", "status", "--root", &root_str, "--json"]);
    assert_success(&json);
    let stdout = String::from_utf8_lossy(&json.stdout);
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("parse agent status JSON: {err}\n{stdout}"))?;
    // Fixture precondition: only the packet exists, so the first next command
    // is the before snapshot.
    let next = report
        .pointer("/next_command/command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected next_command.command:\n{stdout}"))?;
    if !next.starts_with("ripr check --root ") || !next.contains("before.repo-exposure.json") {
        return Err(format!(
            "expected the before-snapshot command first; got `{next}`"
        ));
    }
    if next.contains("target/ripr/workflow --mode") || next.contains("workflow' --mode") {
        return Err(format!(
            "next command must not use the workflow directory as --root; got `{next}`"
        ));
    }
    // Positive grip: the command and the report carry exactly the root the
    // user passed (display paths use `/` on every platform).
    let expected_root = root_str.replace('\\', "/");
    let expected_prefix = format!("ripr check --root {expected_root} --mode");
    let quoted_prefix = format!("ripr check --root '{expected_root}' --mode");
    if !next.starts_with(&expected_prefix) && !next.starts_with(&quoted_prefix) {
        return Err(format!(
            "next command must target the given root `{expected_root}`; got `{next}`"
        ));
    }
    let reported_root = report
        .pointer("/root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("expected root string:\n{stdout}"))?;
    if reported_root != expected_root {
        return Err(format!(
            "reported root must be the given workspace `{expected_root}`; got `{reported_root}`"
        ));
    }

    let markdown = run_ripr(&["agent", "status", "--root", &root_str]);
    assert_success(&markdown);
    let rendered = String::from_utf8_lossy(&markdown.stdout);
    if rendered.contains("Root: ") && rendered.contains("target/ripr/workflow\n") {
        return Err(format!("Markdown root must be the workspace:\n{rendered}"));
    }

    ignore_remove_dir_all(&root);
    Ok(())
}

/// #3893: `agent status` always inspects `target/ripr/workflow`, so a
/// non-default `--out` fails closed instead of being accepted and ignored.
#[test]
fn agent_status_refuses_non_default_out_dir() -> Result<(), String> {
    let root = unique_temp_workspace("agent-status-out");
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"agent-status-out\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    let root_str = root.to_string_lossy().into_owned();

    let custom = run_ripr(&[
        "agent",
        "status",
        "--root",
        &root_str,
        "--out",
        "elsewhere",
        "--json",
    ]);
    if custom.status.success() {
        return Err("agent status --out elsewhere must fail closed".to_string());
    }
    let stderr = String::from_utf8_lossy(&custom.stderr);
    if !stderr.contains("agent status --out elsewhere is not supported") {
        return Err(format!("expected a named --out refusal; got:\n{stderr}"));
    }

    let default = run_ripr(&[
        "agent",
        "status",
        "--root",
        &root_str,
        "--out",
        "target/ripr/workflow",
        "--json",
    ]);
    assert_success(&default);

    ignore_remove_dir_all(&root);
    Ok(())
}

// ── #3906: agent status routes into the repair transaction ──

/// A committed boundary-gap workspace ready for `agent repair`. The label is
/// part of the directory name, so callers can put spaces and non-ASCII
/// characters in the path.
fn repair_route_workspace(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"boundary_gap_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"boundary_gap_fixture\"\npath = \"src/lib.rs\"\n",
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\n",
    )?;
    std::fs::write(root.join("tests/pricing.rs"), REPAIR_ROUTE_WEAK_TEST)?;
    init_git_fixture_repo(&root)?;
    run_git(&root, &["add", "Cargo.toml", "src", "tests"])?;
    repair_route_commit(&root, "fixture source")?;
    Ok(root)
}

const REPAIR_ROUTE_SEAM: &str = "67fc764ba37d77bd";
const REPAIR_ROUTE_WEAK_TEST: &str = "use boundary_gap_fixture::discounted_total;\n\n#[test]\nfn below_threshold_has_no_discount() {\n    assert_eq!(discounted_total(50, 100), 50);\n}\n\n#[test]\nfn far_above_threshold_discounts() {\n    assert_eq!(discounted_total(10_000, 100), 9_990);\n}\n";

fn repair_route_commit(root: &Path, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let commit = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    )?;
    if !commit.status.success() {
        return Err(format!("commit `{message}` failed: {commit:?}").into());
    }
    Ok(())
}

fn repair_route_status(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let root_arg = root.to_string_lossy().into_owned();
    let status = run_ripr(&["agent", "status", "--root", &root_arg, "--json"]);
    assert_success(&status);
    Ok(serde_json::from_slice(&status.stdout)?)
}

/// Runs the before phase and returns the `--attempt ... --phase after`
/// command it printed.
fn repair_route_before(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let root_arg = root.to_string_lossy().into_owned();
    let before = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--seam-id",
        REPAIR_ROUTE_SEAM,
        "--phase",
        "before",
    ]);
    assert_success(&before);
    let stderr = String::from_utf8_lossy(&before.stderr);
    stderr
        .lines()
        .find_map(|line| line.strip_prefix("ripr: attempt next command: "))
        .map(str::to_string)
        .ok_or_else(|| format!("before phase printed no attempt command:\n{stderr}").into())
}

fn repair_route_next(report: &serde_json::Value) -> (String, String) {
    let text = |pointer: &str| {
        report
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (text("/next_command/step"), text("/next_command/command"))
}

fn repair_route_warning_kinds(report: &serde_json::Value) -> Vec<String> {
    report["warnings"]
        .as_array()
        .map(|warnings| {
            warnings
                .iter()
                .filter_map(|warning| warning["kind"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn repair_route_attempt_ids(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ids = std::fs::read_dir(root.join("target/ripr/repair-attempts"))?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort();
    Ok(ids)
}

/// The next command status prints must run from an installed `ripr`: never a
/// `cargo run` or a path into a source checkout, never a placeholder seam.
fn assert_installed_command(command: &str) -> Result<(), String> {
    if !command.starts_with("ripr ")
        || command.contains("cargo ")
        || command.contains("target/debug")
        || command.contains("<seam-id>")
    {
        return Err(format!(
            "next command must be an installed `ripr` command with no placeholder; got `{command}`"
        ));
    }
    Ok(())
}

/// Fresh workspace, path with a space and non-ASCII characters: no seam is
/// known and `target/ripr/workflow/` does not exist, so status routes to the
/// product command that selects a seam instead of a Bash redirect into a
/// missing directory. The printed command is then run as printed.
#[test]
fn agent_status_routes_a_fresh_workspace_to_pilot() -> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status route é")?;
    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "select_seam", "{report:#}");
    assert_installed_command(&command)?;
    let root_display = root.to_string_lossy().replace('\\', "/");
    // #4000: pilot resolves a relative `--out` against the working
    // directory, so the route names the pilot directory under the root.
    assert_eq!(
        command,
        format!("ripr pilot --root '{root_display}' --out '{root_display}/target/ripr/pilot'")
    );
    assert!(!command.contains('>'), "no redirect expected: {command}");
    assert!(!root.join("target/ripr/workflow").exists());
    assert_eq!(report["status"], "incomplete");

    // The route runs: pilot accepts the root and writes its summary under
    // the named `--out`.
    let root_arg = root.to_string_lossy().into_owned();
    let out_arg = format!("{root_display}/target/ripr/pilot");
    let pilot = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&root),
        &["pilot", "--root", &root_arg, "--out", &out_arg],
    )?;
    assert_success(&pilot);
    assert!(root.join("target/ripr/pilot/pilot-summary.json").is_file());

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// One awaiting attempt at the current HEAD: status names exactly the command
/// the before phase printed, even though the legacy artifact loop would have
/// asked for an after snapshot. A path with a space and non-ASCII characters
/// keeps the recorded quoting.
#[test]
fn agent_status_resumes_the_current_awaiting_repair_attempt()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status resume ü")?;
    let printed = repair_route_before(&root)?;
    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, printed);
    assert_installed_command(&command)?;
    assert_eq!(report["repair_attempts"][0]["disposition"], "resumable");
    assert_eq!(report["repair_attempts"][0]["state"], "awaiting_edit");
    assert_eq!(report["repair_attempts"][0]["head_current"], true);

    // The workflow directory is only a compatibility projection: removing it
    // does not change which transaction status resumes.
    std::fs::remove_dir_all(root.join("target/ripr/workflow"))?;
    let report = repair_route_status(&root)?;
    assert_eq!(repair_route_next(&report).1, printed, "{report:#}");

    // The same command in the Markdown view.
    let root_arg = root.to_string_lossy().into_owned();
    let markdown = run_ripr(&["agent", "status", "--root", &root_arg]);
    assert_success(&markdown);
    let rendered = String::from_utf8_lossy(&markdown.stdout);
    assert!(
        rendered.contains(&format!("```bash\n{printed}\n```")),
        "{rendered}"
    );
    assert!(rendered.contains("## Repair Attempts"), "{rendered}");

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// Two attempts, one current: the attempt prepared before history was
/// rewritten (HEAD no longer descends from its prepared head) cannot be
/// resumed, so status resumes the one prepared at the current HEAD. Two
/// current attempts are ambiguous, and status then selects nothing and lists
/// both commands rather than guessing the newest.
#[test]
fn agent_status_resumes_only_the_current_attempt_and_refuses_to_guess()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status several")?;
    let earlier = repair_route_before(&root)?;
    // Rewrite the commit the earlier attempt was prepared at. A commit on top
    // of it would not do: the after phase admits descendant commits, and so
    // does status.
    let amend = run_command(
        "git",
        Some(&root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "--amend",
            "--allow-empty",
            "-m",
            "fixture source, rewritten",
        ],
    )?;
    assert!(amend.status.success(), "{amend:?}");
    let current = repair_route_before(&root)?;

    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, current);
    assert_ne!(command, earlier);
    let dispositions = report["repair_attempts"]
        .as_array()
        .map(|attempts| {
            attempts
                .iter()
                .filter_map(|attempt| attempt["disposition"].as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        dispositions.contains(&"prepared_at_other_head"),
        "{report:#}"
    );
    assert!(dispositions.contains(&"resumable"), "{report:#}");

    let second_current = repair_route_before(&root)?;
    let report = repair_route_status(&root)?;
    assert_eq!(
        report["next_command"],
        serde_json::Value::Null,
        "{report:#}"
    );
    assert!(repair_route_warning_kinds(&report).contains(&"ambiguous_repair_attempts".to_string()));
    let message = report["warnings"]
        .as_array()
        .and_then(|warnings| {
            warnings
                .iter()
                .find(|warning| warning["kind"] == "ambiguous_repair_attempts")
        })
        .and_then(|warning| warning["message"].as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        message.contains(&current) && message.contains(&second_current),
        "{message}"
    );
    assert!(
        !message.contains(&earlier),
        "a stale attempt is not a choice: {message}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// A failed attempt must not be routed into the legacy `agent receipt` step
/// (which is what the artifact loop alone selected, because the failed after
/// phase leaves verify output but no receipt); status starts a new attempt.
/// Once an attempt for the seam finishes, the loop is complete.
#[test]
fn agent_status_restarts_a_failed_attempt_and_completes_a_finished_one()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status failed")?;
    let root_arg = root.to_string_lossy().into_owned();
    let focused_test = format!(
        "{REPAIR_ROUTE_WEAK_TEST}\n#[test]\nfn at_threshold_discounts() {{\n    assert_eq!(discounted_total(100, 100), 90);\n}}\n"
    );

    let failed_command = repair_route_before(&root)?;
    std::fs::write(root.join("tests/pricing.rs"), &focused_test)?;
    std::fs::write(root.join("stray.txt"), "outside the edit cage\n")?;
    let failed_attempt = failed_command
        .split_whitespace()
        .skip_while(|word| *word != "--attempt")
        .nth(1)
        .unwrap_or_default()
        .trim_matches('\'')
        .to_string();
    let after = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--attempt",
        &failed_attempt,
        "--phase",
        "after",
    ]);
    assert!(
        !after.status.success(),
        "stray file must violate the edit cage"
    );
    // Fixture precondition: the attempt recorded a terminal failure.
    let report = repair_route_status(&root)?;
    assert_eq!(
        report["repair_attempts"][0]["state"], "failed",
        "{report:#}"
    );
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_before", "{report:#}");
    assert_installed_command(&command)?;
    assert!(!command.contains("agent receipt"), "{command}");
    assert_eq!(
        command,
        format!(
            "ripr agent repair --root '{}' --seam-id {REPAIR_ROUTE_SEAM} --phase before",
            root_arg.replace('\\', "/")
        )
    );

    std::fs::remove_file(root.join("stray.txt"))?;
    std::fs::write(root.join("tests/pricing.rs"), REPAIR_ROUTE_WEAK_TEST)?;
    let finishing = repair_route_before(&root)?;
    std::fs::write(root.join("tests/pricing.rs"), &focused_test)?;
    let finishing_attempt = repair_route_attempt_ids(&root)?
        .into_iter()
        .find(|id| id != &failed_attempt && finishing.contains(id.as_str()))
        .ok_or("expected the second attempt id")?;
    let after = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--attempt",
        &finishing_attempt,
        "--phase",
        "after",
    ]);
    assert_success(&after);
    let report = repair_route_status(&root)?;
    assert_eq!(
        report["next_command"],
        serde_json::Value::Null,
        "{report:#}"
    );
    // The finished attempt's grip improved, so its seam is not restarted.
    // Whether the loop is `complete` is the receipt's call: only an
    // `advisory` receipt shows the gap closed; any other receipt status keeps
    // status at `warning` and says why.
    let finished = repair_route_attempt(&report, &finishing_attempt)?;
    assert_eq!(
        finished["receipt"]["issued_for_attempt"], true,
        "{report:#}"
    );
    assert_eq!(finished["receipt"]["movement"], "improved", "{report:#}");
    if finished["receipt"]["status"] == "advisory" {
        assert_eq!(report["status"], "complete", "{report:#}");
        assert_eq!(finished["disposition"], "finished", "{report:#}");
    } else {
        assert_eq!(report["status"], "warning", "{report:#}");
        assert_eq!(finished["disposition"], "unconfirmed", "{report:#}");
        assert!(
            repair_route_warning_kinds(&report).contains(&"repair_receipt_unconfirmed".to_string()),
            "{report:#}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// The attempt ID inside a printed `--attempt <id> --phase after` command.
fn repair_route_attempt_id(command: &str) -> Result<String, String> {
    command
        .split_whitespace()
        .skip_while(|word| *word != "--attempt")
        .nth(1)
        .map(|id| id.trim_matches('\'').to_string())
        .ok_or_else(|| format!("no attempt ID in `{command}`"))
}

fn repair_route_after(root: &Path, attempt_id: &str) -> std::process::Output {
    let root_arg = root.to_string_lossy().into_owned();
    run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--attempt",
        attempt_id,
        "--phase",
        "after",
    ])
}

fn repair_route_manifest(
    root: &Path,
    attempt_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/repair-attempts")
            .join(attempt_id)
            .join("attempt.json"),
    )?)?)
}

/// Writes one receipt to both the compatibility projection and the digest-bound
/// attempt-local artifact, then updates `terminal_artifacts` so status still
/// reads this attempt's result.
fn repair_route_write_terminal_receipt(
    root: &Path,
    attempt_id: &str,
    receipt: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(receipt)?;
    std::fs::write(root.join("target/ripr/reports/agent-receipt.json"), &bytes)?;
    let manifest_path = root
        .join("target/ripr/repair-attempts")
        .join(attempt_id)
        .join("attempt.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
    let digest = sha256_hex_bytes(&bytes);
    let size = u64::try_from(bytes.len())?;
    let Some(artifacts) = manifest["terminal_artifacts"].as_array_mut() else {
        return Err("attempt retained no terminal_artifacts".into());
    };
    let Some(receipt_artifact) = artifacts
        .iter_mut()
        .find(|artifact| artifact["role"] == "agent_receipt")
    else {
        return Err("attempt retained no agent_receipt".into());
    };
    let relative = receipt_artifact["path"]
        .as_str()
        .ok_or("agent_receipt path missing")?
        .to_string();
    std::fs::write(root.join(&relative), &bytes)?;
    receipt_artifact["sha256"] = serde_json::Value::String(digest);
    receipt_artifact["bytes"] = serde_json::Value::from(size);
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn repair_route_attempt<'a>(
    report: &'a serde_json::Value,
    attempt_id: &str,
) -> Result<&'a serde_json::Value, String> {
    report["repair_attempts"]
        .as_array()
        .and_then(|attempts| {
            attempts
                .iter()
                .find(|attempt| attempt["attempt_id"] == attempt_id)
        })
        .ok_or_else(|| format!("status does not list attempt `{attempt_id}`: {report:#}"))
}

fn repair_route_markdown(root: &Path) -> Result<String, String> {
    let root_arg = root.to_string_lossy().into_owned();
    let markdown = run_ripr(&["agent", "status", "--root", &root_arg]);
    assert_success(&markdown);
    Ok(String::from_utf8_lossy(&markdown.stdout).into_owned())
}

const REPAIR_ROUTE_BOUNDARY_TEST: &str = "\n#[test]\nfn at_threshold_discounts() {\n    assert_eq!(discounted_total(100, 100), 90);\n}\n";
/// A new test that reaches the seam but, like the existing ones, stays far
/// from the boundary: the edit cage admits it and the grip does not move.
const REPAIR_ROUTE_NON_DISCRIMINATING_TEST: &str = "\n#[test]\nfn well_above_threshold_discounts() {\n    assert_eq!(discounted_total(20_000, 100), 19_990);\n}\n";

/// F15-2: an after phase that refuses leaves its attempt awaiting the edit.
/// Status still resumes that attempt (it is the only way forward), but it
/// names the refusal the attempt recorded instead of printing the same
/// command as if it had never run. A successful retry clears the refusal.
#[test]
fn agent_status_names_a_refused_after_phase_before_repeating_it()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status refused after")?;
    let printed = repair_route_before(&root)?;
    let attempt_id = repair_route_attempt_id(&printed)?;

    // No test edit yet: agent verify refuses a pair without movement.
    let refused = repair_route_after(&root, &attempt_id);
    assert!(!refused.status.success(), "{refused:?}");
    assert_eq!(
        refused.status.code(),
        Some(3),
        "a no-movement verify refusal is a typed refusal, not an operational failure"
    );
    let refused_stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        refused_stderr.contains("no repository movement"),
        "{refused_stderr}"
    );
    let refused_document: serde_json::Value = serde_json::from_slice(&refused.stdout)?;
    assert_eq!(
        refused_document["kind"], "repair_after_refusal",
        "{refused_document:#}"
    );
    assert!(
        refused_document["error"]
            .as_str()
            .is_some_and(|error| error.contains("no repository movement")),
        "{refused_document:#}"
    );
    // Precondition: the attempt record holds the refusal and is still
    // awaiting the edit at the current HEAD.
    let manifest = repair_route_manifest(&root, &attempt_id)?;
    assert_eq!(manifest["state"], "awaiting_edit", "{manifest:#}");
    let recorded = manifest["last_after_refusal"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(recorded.contains("no repository movement"), "{manifest:#}");

    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, printed);
    let reason = report["next_command"]["reason"]
        .as_str()
        .unwrap_or_default();
    assert!(
        reason.contains("was refused") && reason.contains("no repository movement"),
        "the repeated command must say its last run was refused and why: {reason}"
    );
    assert!(
        !reason.contains("once the test is in place"),
        "the refused attempt must not read as untouched: {reason}"
    );
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["disposition"], "resumable", "{report:#}");
    assert_eq!(
        attempt["last_after_refusal"]["reason"],
        serde_json::Value::String(recorded),
        "{report:#}"
    );
    assert_eq!(attempt["receipt"], serde_json::Value::Null, "{report:#}");
    let markdown = repair_route_markdown(&root)?;
    assert!(
        markdown.contains("was refused: agent verify no repository movement"),
        "{markdown}"
    );
    assert!(markdown.contains("last after phase refused"), "{markdown}");

    // The focused edit resolves the cause; the retry finishes and the
    // refusal no longer describes the attempt.
    let test_path = root.join("tests/pricing.rs");
    std::fs::write(
        &test_path,
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    assert_success(&repair_route_after(&root, &attempt_id));
    let manifest = repair_route_manifest(&root, &attempt_id)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest:#}");
    assert!(manifest.get("last_after_refusal").is_none(), "{manifest:#}");
    let report = repair_route_status(&root)?;
    assert_eq!(
        repair_route_attempt(&report, &attempt_id)?["last_after_refusal"],
        serde_json::Value::Null,
        "{report:#}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// The after phase's stdout is exactly one JSON document end-to-end: an
/// orchestrator can run a single JSON.parse on it. The document is the
/// versioned `repair_after_result` envelope (`schema_version` `0.1`): the
/// agent verify 0.3 document rides unchanged under `verify` — every verify
/// field keeps its name and value, including the verify outcome's own
/// `status` — and what `ripr agent status --json` prints rides under
/// `agent_status`. The old shape concatenated the verify JSON and the status
/// JSON with no seam marker, and the intermediate shape spliced
/// `agent_status` into the 0.3 verify document, so two documents sharing one
/// `schema_version` had different shapes depending on the invocation path.
#[test]
fn agent_repair_after_phase_stdout_is_one_json_document() -> Result<(), Box<dyn std::error::Error>>
{
    let root = repair_route_workspace("after stdout single document")?;
    let printed = repair_route_before(&root)?;
    let attempt_id = repair_route_attempt_id(&printed)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    let after = repair_route_after(&root, &attempt_id);
    assert_success(&after);

    let stdout = String::from_utf8_lossy(&after.stdout);
    // A second concatenated document would surface as trailing characters.
    let document: serde_json::Value = serde_json::from_str(&stdout).map_err(|error| {
        format!("after-phase stdout must parse as one JSON document: {error}\n{stdout}")
    })?;
    assert!(
        !stdout.contains("ripr: "),
        "narration belongs on stderr, not in the JSON document:\n{stdout}"
    );
    // The envelope names its own contract: its schema_version and kind are
    // the shape identity a strict-schema consumer dispatches on.
    assert_eq!(document["schema_version"].as_str(), Some("0.1"));
    assert_eq!(document["kind"].as_str(), Some("repair_after_result"));
    // Every existing verify field keeps its name and value under `verify`,
    // and the child stays the agent verify 0.3 document.
    assert_eq!(document["verify"]["schema_version"].as_str(), Some("0.3"));
    assert_eq!(document["verify"]["tool"], "ripr");
    assert_eq!(document["verify"]["status"], "advisory");
    assert!(document["verify"]["inputs"]["before_content_sha256"].is_string());
    assert!(document["verify"]["inputs"]["after_content_sha256"].is_string());
    assert!(document["verify"]["changed_seams"].is_array());
    // The status report rides under `agent_status` (the top-level `status`
    // name is already the verify outcome's, one level down).
    let agent_status = &document["agent_status"];
    assert!(
        matches!(
            agent_status["status"].as_str(),
            Some("complete" | "warning" | "incomplete")
        ),
        "{agent_status}"
    );
    let attempt = agent_status["repair_attempts"]
        .as_array()
        .and_then(|attempts| attempts.first())
        .ok_or("agent_status lists no repair attempt")?;
    assert_eq!(attempt["attempt_id"], attempt_id.as_str());

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// RC rehearsal: a focused test with a wrong expected value still moves the
/// static grip to `improved`, because ripr never runs it. Every surface a cold
/// agent reads (after-phase stderr, the stdout envelope's status report, and
/// `agent status` in both forms) must say the test was not run.
#[test]
fn agent_repair_after_a_failing_test_says_the_test_was_not_run()
-> Result<(), Box<dyn std::error::Error>> {
    const FAILING_BOUNDARY_TEST: &str = "\n#[test]\nfn at_threshold_discounts() {\n    assert_eq!(discounted_total(100, 100), 1);\n}\n";
    let root = repair_route_workspace("after failing test not run")?;
    let attempt_id = repair_route_attempt_id(&repair_route_before(&root)?)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{FAILING_BOUNDARY_TEST}"),
    )?;
    let after = repair_route_after(&root, &attempt_id);
    assert_success(&after);

    let stderr = String::from_utf8_lossy(&after.stderr);
    assert!(
        stderr.contains(", improved)"),
        "precondition: static movement improved:\n{stderr}"
    );
    assert!(
        stderr
            .contains("ripr: test run: none recorded. This receipt compares static evidence only"),
        "{stderr}"
    );
    let document: serde_json::Value = serde_json::from_slice(&after.stdout)?;
    let test_run = &document["agent_status"]["test_run"];
    assert_eq!(test_run["status"], "not_recorded", "{document:#}");
    assert_eq!(test_run["test_changed"], "tests/pricing.rs", "{document:#}");
    assert!(
        test_run["next_step"]
            .as_str()
            .is_some_and(|step| step.contains("keep it only if it passes")),
        "{test_run}"
    );
    let attempt = &document["agent_status"]["repair_attempts"][0];
    assert_eq!(
        attempt["receipt"]["verification_status"],
        "verification_not_run"
    );

    let root_arg = root.to_string_lossy().into_owned();
    let status = run_ripr(&["agent", "status", "--root", &root_arg]);
    assert_success(&status);
    let human = String::from_utf8_lossy(&status.stdout);
    assert!(
        human.contains("Test run: none recorded. The repair receipt compares static evidence only and records no run of the focused test (`tests/pricing.rs`)"),
        "{human}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// F15-4: an after phase whose receipt shows the grip did not move finishes
/// the edit cage but not the gap. Status is not `complete`: it names the
/// receipt's status and movement and restarts the seam with the receipt's own
/// next action. After the test is committed, it also says the evidence is
/// stale against HEAD, and the printed restart still runs.
#[test]
fn agent_status_keeps_the_gap_open_when_the_receipt_did_not_improve()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status unchanged receipt")?;
    let attempt_id = repair_route_attempt_id(&repair_route_before(&root)?)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_NON_DISCRIMINATING_TEST}"),
    )?;
    assert_success(&repair_route_after(&root, &attempt_id));

    // Preconditions: the edit cage admitted the edit and the receipt issued
    // for this attempt records unchanged movement.
    let manifest = repair_route_manifest(&root, &attempt_id)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest:#}");
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?;
    assert_eq!(receipt["repair_attempt"]["attempt_id"], attempt_id.as_str());
    assert_eq!(
        receipt["provenance"]["movement"], "unchanged",
        "{receipt:#}"
    );
    let receipt_status = receipt["status"].as_str().unwrap_or_default().to_string();

    let report = repair_route_status(&root)?;
    assert_ne!(report["status"], "complete", "{report:#}");
    assert_eq!(report["status"], "incomplete", "{report:#}");
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["state"], "ready_to_finish", "{report:#}");
    assert_eq!(attempt["disposition"], "gap_open", "{report:#}");
    assert_eq!(attempt["receipt"]["issued_for_attempt"], true, "{report:#}");
    assert_eq!(attempt["receipt"]["movement"], "unchanged", "{report:#}");
    assert_eq!(
        attempt["receipt"]["receipt_state"], "receipt_movement_unchanged",
        "{report:#}"
    );
    assert_eq!(attempt["receipt"]["shows_gap_closed"], false, "{report:#}");
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_before", "{report:#}");
    assert_installed_command(&command)?;
    let reason = report["next_command"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    for needle in [
        format!("status `{receipt_status}`"),
        "movement `unchanged`".to_string(),
        "does not show the gap closed".to_string(),
        "the gap is still open".to_string(),
        "Add the missing discriminator or stronger assertion named by the packet".to_string(),
    ] {
        assert!(
            reason.contains(&needle),
            "reason lacks `{needle}`: {reason}"
        );
    }
    assert!(!reason.contains("stale against HEAD"), "{reason}");
    let markdown = repair_route_markdown(&root)?;
    assert!(!markdown.contains("Status: complete"), "{markdown}");
    assert!(
        !markdown.contains("No missing agent-loop artifacts were detected."),
        "{markdown}"
    );
    assert!(
        markdown.contains("| ready_to_finish | gap_open |"),
        "{markdown}"
    );

    // Committing the test moves HEAD past the receipt's evidence.
    run_git(&root, &["add", "tests"])?;
    repair_route_commit(&root, "commit the non-discriminating test")?;
    let report = repair_route_status(&root)?;
    assert_eq!(report["status"], "incomplete", "{report:#}");
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["head_current"], false, "{report:#}");
    assert_eq!(attempt["disposition"], "gap_open", "{report:#}");
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_before", "{report:#}");
    let reason = report["next_command"]["reason"]
        .as_str()
        .unwrap_or_default();
    assert!(
        reason.contains("stale against HEAD") && reason.contains("movement `unchanged`"),
        "{reason}"
    );
    assert!(
        repair_route_markdown(&root)?.contains("evidence stale against HEAD"),
        "stale evidence must show in the attempt table"
    );

    // The printed restart runs at the new HEAD: the gap is still there.
    let root_arg = root.to_string_lossy().into_owned();
    assert_eq!(
        command,
        format!(
            "ripr agent repair --root '{}' --seam-id {REPAIR_ROUTE_SEAM} --phase before",
            root_arg.replace('\\', "/")
        )
    );
    repair_route_before(&root)?;

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// Regression for the positive case: an attempt whose own receipt is
/// `advisory` with improved grip at the current HEAD is `complete`. The
/// ordinary after phase currently issues an `invalid` receipt (onboarding
/// finding F15-3: the analysis-outcome base check), so the fixture sets the
/// receipt's status to `advisory` the way a valid issuance records it; the
/// binding to the attempt is left exactly as issued. Status is then stale
/// once HEAD moves, and a receipt bound to another after verdict is not read
/// as this attempt's.
#[test]
fn agent_status_is_complete_only_for_an_advisory_improved_receipt_at_head()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status advisory receipt")?;
    let attempt_id = repair_route_attempt_id(&repair_route_before(&root)?)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    assert_success(&repair_route_after(&root, &attempt_id));

    let receipt_path = root.join("target/ripr/reports/agent-receipt.json");
    let mut receipt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&receipt_path)?)?;
    assert_eq!(receipt["provenance"]["movement"], "improved", "{receipt:#}");
    assert_eq!(receipt["repair_attempt"]["attempt_id"], attempt_id.as_str());
    if receipt["status"] != "advisory" {
        receipt["status"] = serde_json::Value::String("advisory".to_string());
        receipt["analysis_outcome_status"] = serde_json::Value::String("complete".to_string());
        receipt["analysis_outcome_error"] = serde_json::Value::Null;
        repair_route_write_terminal_receipt(&root, &attempt_id, &receipt)?;
    }
    let manifest = repair_route_manifest(&root, &attempt_id)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest:#}");
    assert_eq!(
        receipt["repair_attempt"]["delta_sha256"], manifest["after"]["delta_sha256"],
        "fixture precondition: the receipt stays bound to this after verdict"
    );

    let report = repair_route_status(&root)?;
    assert_eq!(report["status"], "complete", "{report:#}");
    assert_eq!(
        report["next_command"],
        serde_json::Value::Null,
        "{report:#}"
    );
    assert!(repair_route_warning_kinds(&report).is_empty(), "{report:#}");
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["disposition"], "finished", "{report:#}");
    assert_eq!(attempt["receipt"]["shows_gap_closed"], true, "{report:#}");
    assert_eq!(attempt["receipt"]["status"], "advisory", "{report:#}");
    let markdown = repair_route_markdown(&root)?;
    assert!(markdown.contains("Status: complete"), "{markdown}");
    assert!(
        markdown.contains("No missing agent-loop artifacts were detected.\n"),
        "{markdown}"
    );

    // HEAD moves after the receipt: the evidence is stale, so not complete.
    run_git(&root, &["add", "tests"])?;
    repair_route_commit(&root, "commit the focused test")?;
    let report = repair_route_status(&root)?;
    assert_eq!(report["status"], "warning", "{report:#}");
    assert_eq!(
        report["next_command"],
        serde_json::Value::Null,
        "{report:#}"
    );
    let stale = report["warnings"]
        .as_array()
        .and_then(|warnings| {
            warnings
                .iter()
                .find(|warning| warning["kind"] == "repair_receipt_stale")
        })
        .and_then(|warning| warning["message"].as_str())
        .unwrap_or_default()
        .to_string();
    let recorded_head = manifest["after"]["repository_head"]
        .as_str()
        .unwrap_or_default();
    assert!(
        stale.contains("stale against HEAD") && stale.contains(recorded_head),
        "{report:#}"
    );
    assert_eq!(
        repair_route_attempt(&report, &attempt_id)?["head_current"],
        false,
        "{report:#}"
    );
    let markdown = repair_route_markdown(&root)?;
    assert!(markdown.contains("Status: warning"), "{markdown}");
    assert!(markdown.contains("repair_receipt_stale"), "{markdown}");
    assert!(
        markdown.contains("but status does not report the loop complete"),
        "{markdown}"
    );

    // A receipt that names a different after verdict is not this attempt's.
    receipt["repair_attempt"]["delta_sha256"] =
        serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
    repair_route_write_terminal_receipt(&root, &attempt_id, &receipt)?;
    let report = repair_route_status(&root)?;
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["disposition"], "unconfirmed", "{report:#}");
    assert_eq!(
        attempt["receipt"]["issued_for_attempt"], false,
        "{report:#}"
    );
    assert_ne!(report["status"], "complete", "{report:#}");
    assert!(
        repair_route_warning_kinds(&report).contains(&"repair_receipt_unconfirmed".to_string()),
        "{report:#}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn repair_route_amend(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let amend = run_command(
        "git",
        Some(root),
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-a",
            "--amend",
            "--allow-empty",
            "--no-edit",
        ],
    )?;
    if !amend.status.success() {
        return Err(format!("amend failed: {amend:?}").into());
    }
    Ok(())
}

fn repair_route_head(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    git_stdout(root, &["rev-parse", "HEAD"])
}

/// N1 (#3982 x #3986): the after phase admits a focused test committed on
/// top of the prepared head, so status must too. Status resumes the attempt
/// with its recorded after command (not a new attempt whose before phase
/// cannot find the seam once the test closes the gap), and after the finish
/// the attempt is current at the HEAD its after phase recorded, even though
/// that is not the prepared head. Rerunning the finished after phase says it
/// already finished and what to run next (N4). An advisory improved receipt
/// reads "static grip improved (receipt advisory)", never "gap closed" (N9).
#[test]
fn agent_status_follows_a_focused_test_committed_between_the_phases()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status committed test")?;
    let printed = repair_route_before(&root)?;
    let attempt_id = repair_route_attempt_id(&printed)?;
    let prepared_head = repair_route_head(&root)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    run_git(&root, &["add", "tests"])?;
    repair_route_commit(&root, "test: pin the discount boundary")?;
    let committed_head = repair_route_head(&root)?;
    assert_ne!(committed_head, prepared_head, "precondition: HEAD moved");

    // Before the after phase: the attempt is resumable at the descendant HEAD.
    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, printed, "{report:#}");
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["disposition"], "resumable", "{report:#}");
    assert_eq!(attempt["head_current"], true, "{report:#}");
    assert!(
        !report["next_command"]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("start a new attempt"),
        "{report:#}"
    );

    // The recorded after command succeeds at that HEAD.
    assert_success(&repair_route_after(&root, &attempt_id));
    let manifest = repair_route_manifest(&root, &attempt_id)?;
    assert_eq!(manifest["state"], "ready_to_finish", "{manifest:#}");
    assert_eq!(
        manifest["after"]["repository_head"],
        committed_head.as_str(),
        "precondition: the after verdict was recorded at the committed HEAD"
    );
    assert_eq!(manifest["repository_head"], prepared_head.as_str());
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?;
    assert_eq!(receipt["status"], "advisory", "{receipt:#}");
    assert_eq!(receipt["provenance"]["movement"], "improved", "{receipt:#}");

    // After the finish: current at its after head, so complete, not stale.
    let report = repair_route_status(&root)?;
    assert_eq!(report["status"], "complete", "{report:#}");
    assert!(repair_route_warning_kinds(&report).is_empty(), "{report:#}");
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(attempt["disposition"], "finished", "{report:#}");
    assert_eq!(attempt["head_current"], true, "{report:#}");
    let markdown = repair_route_markdown(&root)?;
    assert!(
        markdown.contains("static grip improved (receipt advisory)"),
        "{markdown}"
    );
    assert!(!markdown.contains("gap closed at"), "{markdown}");

    // N4: the finished after phase is not rerun, and the refusal says so.
    let rerun = repair_route_after(&root, &attempt_id);
    assert_failure(&rerun);
    let stderr = String::from_utf8_lossy(&rerun.stderr);
    let root_arg = shell_single_quoted(&root.to_string_lossy().replace('\\', "/"));
    for needle in [
        "already finished".to_string(),
        "state `ready_to_finish`".to_string(),
        "target/ripr/reports/agent-receipt.json".to_string(),
        format!("`ripr agent status --root {root_arg}`"),
        format!(
            "`ripr agent repair --root {root_arg} --seam-id {REPAIR_ROUTE_SEAM} --phase before`"
        ),
    ] {
        assert!(stderr.contains(&needle), "lacks `{needle}`:\n{stderr}");
    }
    assert!(!stderr.contains("ReadyToFinish"), "{stderr}");
    assert!(!stderr.contains("requires awaiting_edit"), "{stderr}");
    assert_eq!(
        repair_route_manifest(&root, &attempt_id)?["state"],
        "ready_to_finish",
        "the refused rerun must not move the attempt"
    );

    // A later commit makes the evidence stale; the warning names two heads.
    repair_route_commit(&root, "move past the receipt")?;
    let moved_head = repair_route_head(&root)?;
    let report = repair_route_status(&root)?;
    assert_eq!(report["status"], "warning", "{report:#}");
    let stale = report["warnings"]
        .as_array()
        .and_then(|warnings| {
            warnings
                .iter()
                .find(|warning| warning["kind"] == "repair_receipt_stale")
        })
        .and_then(|warning| warning["message"].as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        stale.contains(&format!("at HEAD `{committed_head}`"))
            && stale.contains(&format!("HEAD is now `{moved_head}`"))
            && stale.contains("reports static grip improved (receipt advisory)"),
        "{report:#}"
    );
    assert!(!stale.contains("gap closed"), "{stale}");

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// Quote one argument exactly as the product's generated-command renderer
/// (`agent::loop_commands::shell_arg`) does: a plain token stays bare, any
/// other value is POSIX single-quoted.
fn renderer_shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    shell_single_quoted(value)
}

/// Decode one rendered POSIX shell word (bare, single-quoted, or `'\''`
/// concatenations) back to its argument value. Returns `None` for an
/// unterminated quote so a malformed rendering fails the caller's oracle.
fn decode_shell_token(token: &str) -> Option<String> {
    let mut decoded = String::new();
    let mut chars = token.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => loop {
                match chars.next()? {
                    '\'' => break,
                    inner => decoded.push(inner),
                }
            },
            '\\' => decoded.push(chars.next()?),
            other => decoded.push(other),
        }
    }
    Some(decoded)
}

#[test]
fn decode_shell_token_inverts_the_renderer_quoting() {
    // A backslash inside single quotes is literal (the renderer preserves it);
    // only an unquoted backslash, as in the renderer's `'\''`, is an escape.
    for value in [
        "/tmp/plain",
        "/tmp/with space/repo",
        "/tmp/it's here",
        "",
        r"/tmp/repo\sub\dir",
        r"/tmp/back\slash it's",
    ] {
        assert_eq!(
            decode_shell_token(&renderer_shell_arg(value)).as_deref(),
            Some(value),
            "round trip for {value:?}"
        );
    }
    assert_eq!(
        decode_shell_token(r"'/tmp/a\b'"),
        Some(r"/tmp/a\b".to_string())
    );
    assert_eq!(decode_shell_token("'unterminated"), None);
}

/// Every string value in a JSON document, depth first.
fn json_strings(value: &serde_json::Value) -> Vec<&str> {
    match value {
        serde_json::Value::String(text) => vec![text.as_str()],
        serde_json::Value::Array(items) => items.iter().flat_map(json_strings).collect(),
        serde_json::Value::Object(map) => map.values().flat_map(json_strings).collect(),
        _ => Vec::new(),
    }
}

/// POSIX single quotes around a path, as the status command templates print.
fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// N1, rewritten history: when HEAD no longer descends from the prepared
/// head, the after phase refuses before finishing and names two recoveries.
/// Status reports that same refusal and the reset recovery instead of only
/// "prepared at a different HEAD", and following the reset makes the same
/// attempt resumable again.
#[test]
fn agent_status_reports_the_after_phase_recovery_for_rewritten_history()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status rewritten history")?;
    let printed = repair_route_before(&root)?;
    let attempt_id = repair_route_attempt_id(&printed)?;
    let prepared_head = repair_route_head(&root)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    repair_route_amend(&root)?;
    let amended = repair_route_head(&root)?;
    let ancestry = run_command(
        "git",
        Some(&root),
        &["merge-base", "--is-ancestor", &prepared_head, &amended],
    )?;
    assert!(
        !ancestry.status.success(),
        "precondition: history rewritten"
    );

    let reset_sentence = format!(
        "`git reset --soft {prepared_head}` restores the prepared head and keeps your edit staged"
    );
    let report = repair_route_status(&root)?;
    let attempt = repair_route_attempt(&report, &attempt_id)?;
    assert_eq!(
        attempt["disposition"], "prepared_at_other_head",
        "{report:#}"
    );
    assert_eq!(attempt["head_current"], false, "{report:#}");
    let (step, _) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_before", "{report:#}");
    let reason = report["next_command"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    for needle in [
        "does not descend from",
        reset_sentence.as_str(),
        printed.trim_start_matches("ripr "),
    ] {
        assert!(reason.contains(needle), "lacks `{needle}`: {reason}");
    }

    // The after phase gives the same recovery.
    let refused = repair_route_after(&root, &attempt_id);
    assert_failure(&refused);
    assert_eq!(
        refused.status.code(),
        Some(3),
        "a diverged HEAD is a typed refusal, not an operational failure"
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains(&reset_sentence), "{stderr}");
    let document: serde_json::Value = serde_json::from_slice(&refused.stdout)?;
    assert_eq!(document["kind"], "repair_after_refusal", "{document:#}");
    assert!(
        json_strings(&document["narration"])
            .iter()
            .any(|line| line.contains(&reset_sentence)),
        "{document:#}"
    );
    assert_eq!(
        repair_route_manifest(&root, &attempt_id)?["state"],
        "awaiting_edit"
    );

    // Following it resumes the same attempt.
    run_git(&root, &["reset", "-q", "--soft", &prepared_head])?;
    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, printed, "{report:#}");
    assert_success(&repair_route_after(&root, &attempt_id));

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// N6: an after phase refused for changed analysis inputs prints the changed
/// path and the recovery before its terse final error. The attempt records
/// all of it, so status repeats the named cause and recovery, not only the
/// final error line.
#[test]
fn agent_status_repeats_the_named_cause_of_a_refused_after_phase()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status named refusal")?;
    let printed = repair_route_before(&root)?;
    let attempt_id = repair_route_attempt_id(&printed)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_BOUNDARY_TEST}"),
    )?;
    let manifest_path = root.join("Cargo.toml");
    let original_manifest = std::fs::read_to_string(&manifest_path)?;
    std::fs::write(
        &manifest_path,
        format!("{original_manifest}\n[dependencies]\n"),
    )?;

    let refused = repair_route_after(&root, &attempt_id);
    assert_failure(&refused);
    assert_eq!(
        refused.status.code(),
        Some(3),
        "drifted analysis inputs are a typed refusal, not an operational failure"
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("ripr: analysis inputs changed after the before phase: Cargo.toml."),
        "precondition: the after phase names the changed input:\n{stderr}"
    );
    let document: serde_json::Value = serde_json::from_slice(&refused.stdout)?;
    assert_eq!(document["kind"], "repair_after_refusal", "{document:#}");
    assert!(
        json_strings(&document["narration"])
            .iter()
            .any(|line| line.contains("analysis inputs changed after the before phase: Cargo.toml")),
        "{document:#}"
    );

    let manifest = repair_route_manifest(&root, &attempt_id)?;
    let recorded = manifest["last_after_refusal"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    for needle in [
        "analysis input identities differ",
        "Analysis inputs changed after the before phase: Cargo.toml",
        "To recover: restore those files to their before-phase state",
    ] {
        assert!(recorded.contains(needle), "lacks `{needle}`: {recorded}");
    }
    let report = repair_route_status(&root)?;
    let (step, command) = repair_route_next(&report);
    assert_eq!(step, "repair_attempt_after", "{report:#}");
    assert_eq!(command, printed);
    let reason = report["next_command"]["reason"]
        .as_str()
        .unwrap_or_default();
    assert!(
        reason.contains("was refused")
            && reason.contains("Cargo.toml")
            && reason.contains("git checkout"),
        "{reason}"
    );
    assert!(
        repair_route_markdown(&root)?
            .contains("Analysis inputs changed after the before phase: Cargo.toml"),
        "the Markdown view must carry the named cause"
    );

    std::fs::write(&manifest_path, original_manifest)?;
    assert_success(&repair_route_after(&root, &attempt_id));

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

const REPAIR_ROUTE_SHIPPING_SEAM: &str = "59fe980b7be4bc3c";

/// A second weakly gripped seam next to the boundary-gap one.
fn repair_route_two_seam_workspace(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = repair_route_workspace(label)?;
    let mut source = std::fs::read_to_string(root.join("src/lib.rs"))?;
    source.push_str("\npub fn shipping_fee(weight: i32, heavy_limit: i32) -> i32 {\n    if weight >= heavy_limit {\n        weight + 15\n    } else {\n        weight\n    }\n}\n");
    std::fs::write(root.join("src/lib.rs"), source)?;
    std::fs::write(
        root.join("tests/shipping.rs"),
        "use boundary_gap_fixture::shipping_fee;\n\n#[test]\nfn light_parcels_cost_the_base() {\n    assert_eq!(shipping_fee(1, 10), 1);\n}\n\n#[test]\nfn heavy_parcels_cost_more() {\n    assert_eq!(shipping_fee(100, 10), 115);\n}\n",
    )?;
    run_git(&root, &["add", "src", "tests"])?;
    repair_route_commit(&root, "second seam")?;
    Ok(root)
}

/// #4572 first falsifier: finishing B must not destroy A's retained terminal
/// result. The workflow receipt remains a one-slot compatibility projection
/// (B replaces it), but status after a process restart still reads A's own
/// outcome from attempt-local evidence — not B's result and not an artificial
/// loss-of-receipt / superseded state.
#[test]
fn agent_status_retains_an_earlier_attempt_outcome_after_a_later_finish()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_two_seam_workspace("status retain earlier receipt")?;
    let first = repair_route_attempt_id(&repair_route_before(&root)?)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_NON_DISCRIMINATING_TEST}"),
    )?;
    assert_success(&repair_route_after(&root, &first));
    let first_report = repair_route_status(&root)?;
    let first_attempt = repair_route_attempt(&first_report, &first)?;
    assert_eq!(
        first_attempt["disposition"], "gap_open",
        "precondition: the first attempt's own receipt leaves its gap open"
    );
    let first_status = first_attempt["receipt"]["status"].clone();
    let first_movement = first_attempt["receipt"]["movement"].clone();
    assert_eq!(
        first_attempt["receipt"]["issued_for_attempt"], true,
        "{first_report:#}"
    );
    run_git(&root, &["add", "tests"])?;
    repair_route_commit(&root, "first attempt's test")?;

    let root_arg = root.to_string_lossy().into_owned();
    let second_before = run_ripr(&[
        "agent",
        "repair",
        "--json",
        "--root",
        &root_arg,
        "--seam-id",
        REPAIR_ROUTE_SHIPPING_SEAM,
        "--phase",
        "before",
    ]);
    assert_success(&second_before);
    let second = repair_route_attempt_id(
        String::from_utf8_lossy(&second_before.stderr)
            .lines()
            .find_map(|line| line.strip_prefix("ripr: attempt next command: "))
            .ok_or("second before phase printed no attempt command")?,
    )?;
    let mut shipping = std::fs::read_to_string(root.join("tests/shipping.rs"))?;
    shipping.push_str(
        "\n#[test]\nfn at_limit_is_heavy() {\n    assert_eq!(shipping_fee(10, 10), 25);\n}\n",
    );
    std::fs::write(root.join("tests/shipping.rs"), shipping)?;
    assert_success(&repair_route_after(&root, &second));
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("target/ripr/reports/agent-receipt.json"),
    )?)?;
    assert_eq!(
        receipt["repair_attempt"]["attempt_id"],
        second.as_str(),
        "precondition: the compatibility projection now holds B's receipt"
    );

    // Process restart: a fresh `ripr agent status` re-reads the files.
    let report = repair_route_status(&root)?;
    let earlier = repair_route_attempt(&report, &first)?;
    assert_eq!(
        earlier["disposition"], "gap_open",
        "A must still report A's retained outcome after B finishes: {report:#}"
    );
    assert_eq!(earlier["receipt"]["issued_for_attempt"], true, "{report:#}");
    assert_eq!(earlier["receipt"]["status"], first_status, "{report:#}");
    assert_eq!(earlier["receipt"]["movement"], first_movement, "{report:#}");
    assert_eq!(
        earlier["receipt"]["superseded_by"],
        serde_json::Value::Null,
        "a readable attempt-local result is not a superseded loss: {report:#}"
    );
    let earlier_path = earlier["receipt"]["path"]
        .as_str()
        .ok_or("earlier receipt path missing")?;
    assert!(
        earlier_path.contains(&format!("repair-attempts/{first}/")),
        "A's status must name the attempt-local receipt, not the one-slot projection: {earlier:#}"
    );
    let later = repair_route_attempt(&report, &second)?;
    assert_eq!(later["receipt"]["issued_for_attempt"], true, "{report:#}");
    assert_eq!(
        later["receipt"]["superseded_by"],
        serde_json::Value::Null,
        "{report:#}"
    );
    assert_ne!(
        later["receipt"]["movement"], first_movement,
        "B's outcome must stay distinct from A's so this is not a same-result coincidence: {report:#}"
    );

    // Compatibility projection rewritten, removed, or bound to B must not
    // change A's retained reading, and must not fall back to B.
    std::fs::write(
        root.join("target/ripr/reports/agent-receipt.json"),
        "{not json",
    )?;
    let after_malformed = repair_route_status(&root)?;
    let earlier = repair_route_attempt(&after_malformed, &first)?;
    assert_eq!(earlier["disposition"], "gap_open", "{after_malformed:#}");
    assert_eq!(
        earlier["receipt"]["issued_for_attempt"], true,
        "{after_malformed:#}"
    );
    assert_eq!(
        earlier["receipt"]["movement"], first_movement,
        "{after_malformed:#}"
    );

    std::fs::remove_file(root.join("target/ripr/reports/agent-receipt.json"))?;
    let after_removed = repair_route_status(&root)?;
    let earlier = repair_route_attempt(&after_removed, &first)?;
    assert_eq!(earlier["disposition"], "gap_open", "{after_removed:#}");
    assert_eq!(
        earlier["receipt"]["issued_for_attempt"], true,
        "{after_removed:#}"
    );

    let markdown = repair_route_markdown(&root)?;
    assert!(
        !markdown.contains(&format!("receipt superseded by attempt `{second}`")),
        "{markdown}"
    );
    assert!(markdown.contains("gap still open"), "{markdown}");
    assert!(
        !markdown.contains("no receipt issued for this attempt"),
        "{markdown}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// Attempt-local terminal evidence is content-bound. Removing it, changing its
/// payload, or pointing the manifest at a path outside the attempt must not
/// reconstruct A's outcome from B's compatibility receipt.
#[test]
fn agent_status_does_not_fall_back_to_another_attempt_when_local_receipt_is_unusable()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_two_seam_workspace("status local receipt unusable")?;
    let first = repair_route_attempt_id(&repair_route_before(&root)?)?;
    std::fs::write(
        root.join("tests/pricing.rs"),
        format!("{REPAIR_ROUTE_WEAK_TEST}{REPAIR_ROUTE_NON_DISCRIMINATING_TEST}"),
    )?;
    assert_success(&repair_route_after(&root, &first));
    run_git(&root, &["add", "tests"])?;
    repair_route_commit(&root, "first attempt's test")?;

    let root_arg = root.to_string_lossy().into_owned();
    let second_before = run_ripr(&[
        "agent",
        "repair",
        "--root",
        &root_arg,
        "--seam-id",
        REPAIR_ROUTE_SHIPPING_SEAM,
        "--phase",
        "before",
    ]);
    assert_success(&second_before);
    let second = repair_route_attempt_id(
        String::from_utf8_lossy(&second_before.stderr)
            .lines()
            .find_map(|line| line.strip_prefix("ripr: attempt next command: "))
            .ok_or("second before phase printed no attempt command")?,
    )?;
    let mut shipping = std::fs::read_to_string(root.join("tests/shipping.rs"))?;
    shipping.push_str(
        "\n#[test]\nfn at_limit_is_heavy() {\n    assert_eq!(shipping_fee(10, 10), 25);\n}\n",
    );
    std::fs::write(root.join("tests/shipping.rs"), shipping)?;
    assert_success(&repair_route_after(&root, &second));

    let first_manifest_path = root
        .join("target/ripr/repair-attempts")
        .join(&first)
        .join("attempt.json");
    let first_manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&first_manifest_path)?)?;
    let terminal = first_manifest["terminal_artifacts"]
        .as_array()
        .ok_or("first attempt retained no terminal_artifacts")?;
    let receipt_artifact = terminal
        .iter()
        .find(|artifact| artifact["role"] == "agent_receipt")
        .ok_or("first attempt retained no agent_receipt")?;
    let local_receipt = root.join(
        receipt_artifact["path"]
            .as_str()
            .ok_or("agent_receipt path missing")?,
    );
    assert!(
        local_receipt.starts_with(root.join("target/ripr/repair-attempts").join(&first)),
        "retained receipt must stay under A's attempt directory: {}",
        local_receipt.display()
    );

    std::fs::remove_file(&local_receipt)?;
    let missing = repair_route_status(&root)?;
    let earlier = repair_route_attempt(&missing, &first)?;
    assert_ne!(
        earlier["disposition"], "finished",
        "a missing local result is not successful closure: {missing:#}"
    );
    assert_eq!(
        earlier["receipt"]["issued_for_attempt"], false,
        "{missing:#}"
    );
    assert_eq!(earlier["receipt"]["unavailable"], true, "{missing:#}");
    assert_eq!(
        earlier["receipt"]["superseded_by"],
        serde_json::Value::Null,
        "a declared local miss is unavailable, not a superseded projection of B: {missing:#}"
    );
    assert_ne!(
        earlier["receipt"]["movement"],
        repair_route_attempt(&missing, &second)?["receipt"]["movement"],
        "must not project B's movement onto A: {missing:#}"
    );

    std::fs::write(&local_receipt, b"tampered-not-the-retained-receipt")?;
    let tampered = repair_route_status(&root)?;
    let earlier = repair_route_attempt(&tampered, &first)?;
    assert_eq!(
        earlier["receipt"]["issued_for_attempt"], false,
        "{tampered:#}"
    );
    assert_eq!(earlier["receipt"]["unavailable"], true, "{tampered:#}");

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// An attempt manifest status cannot validate is not skipped: status selects
/// no command and says which attempt it refused.
#[test]
fn agent_status_selects_nothing_past_an_unreadable_attempt()
-> Result<(), Box<dyn std::error::Error>> {
    let root = repair_route_workspace("status unreadable")?;
    repair_route_before(&root)?;
    let broken = root.join("target/ripr/repair-attempts/repair-attempt-000000000000000000000000");
    std::fs::create_dir_all(&broken)?;
    std::fs::write(broken.join("attempt.json"), "{ not json")?;

    let report = repair_route_status(&root)?;
    assert_eq!(
        report["next_command"],
        serde_json::Value::Null,
        "{report:#}"
    );
    assert!(repair_route_warning_kinds(&report).contains(&"repair_attempt_unreadable".to_string()));

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

// ── ripr pr-summary (Campaign 31 item 8: binary-first downstream CI) ──

struct PrSummaryScratch(PathBuf);

impl Drop for PrSummaryScratch {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.0);
    }
}

#[test]
fn pr_summary_root_from_foreign_cwd_anchors_artifacts_and_baseline() -> Result<(), String> {
    let scratch = unique_temp_workspace("pr-summary-root");
    std::fs::create_dir(&scratch).map_err(|error| error.to_string())?;
    let _cleanup = PrSummaryScratch(scratch.clone());
    let selected = scratch.join("selected répo");
    let foreign = scratch.join("foreign");
    for root in [&selected, &foreign] {
        std::fs::create_dir_all(root.join("target/ripr/reports")).map_err(|err| err.to_string())?;
    }
    for (root, actionable, before) in [(&selected, 3, 7), (&foreign, 11, 1)] {
        std::fs::write(
            root.join("target/ripr/reports/gap-decision-ledger.json"),
            serde_json::json!({"summary": {"repairable_total": actionable}}).to_string(),
        )
        .map_err(|err| err.to_string())?;
        std::fs::write(
            root.join("before.json"),
            serde_json::json!({"gaps": {"total_actionable": before}}).to_string(),
        )
        .map_err(|err| err.to_string())?;
    }
    let selected_arg = selected
        .to_str()
        .ok_or_else(|| "selected root is not UTF-8".to_string())?;
    let output = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&foreign),
        &[
            "pr-summary",
            "--root",
            selected_arg,
            "--baseline",
            "before.json",
        ],
    )
    .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "foreign-CWD pr-summary failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    for relative in [
        "target/ripr/pr/summary.md",
        "target/ripr/reports/pr-evidence-summary.json",
        "target/ripr/reports/pr-evidence-summary.md",
    ] {
        if !selected.join(relative).is_file() || foreign.join(relative).exists() {
            return Err(format!("summary output escaped selected root: {relative}"));
        }
    }
    let summary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(selected.join("target/ripr/reports/pr-evidence-summary.json"))
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if summary
        .pointer("/gaps/total_actionable")
        .and_then(serde_json::Value::as_u64)
        != Some(3)
        || summary
            .pointer("/gaps/resolved")
            .and_then(serde_json::Value::as_u64)
            != Some(4)
    {
        return Err(format!(
            "summary read foreign artifacts or baseline: {summary}"
        ));
    }
    let relative_root = Path::new("..").join("selected répo");
    let relative_root_arg = relative_root
        .to_str()
        .ok_or_else(|| "relative root is not UTF-8".to_string())?;
    let check = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&foreign),
        &[
            "pr-summary",
            "--root",
            relative_root_arg,
            "--check",
            "--baseline",
            "before.json",
        ],
    )
    .map_err(|err| err.to_string())?;
    if !check.status.success() {
        return Err(format!(
            "rooted summary check failed: {}",
            String::from_utf8_lossy(&check.stderr)
        ));
    }
    Ok(())
}

#[test]
fn pr_summary_rejects_malformed_path_flags_without_outputs() -> Result<(), String> {
    let scratch = unique_temp_workspace("pr-summary-malformed-root");
    std::fs::create_dir(&scratch).map_err(|error| error.to_string())?;
    let _cleanup = PrSummaryScratch(scratch.clone());
    for flag in ["--root", "--baseline"] {
        for value in [None, Some(""), Some("   "), Some("--check")] {
            let mut args = vec!["pr-summary", flag];
            if let Some(value) = value {
                args.push(value);
            }
            let output = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&scratch), &args)
                .map_err(|error| error.to_string())?;
            if output.status.code() != Some(2)
                || !String::from_utf8_lossy(&output.stderr).contains(flag)
            {
                return Err(format!(
                    "malformed {args:?} lacked scoped usage failure: {output:?}"
                ));
            }
        }
    }
    if scratch.join("target").exists() || scratch.join("--check").exists() {
        return Err("malformed summary invocation wrote outputs".to_string());
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn pr_summary_rejects_windows_partially_qualified_paths_before_io() -> Result<(), String> {
    let scratch = unique_temp_workspace("pr-summary-partially-qualified");
    std::fs::create_dir(&scratch).map_err(|error| error.to_string())?;
    let _cleanup = PrSummaryScratch(scratch.clone());
    let selected = scratch.join("selected");
    std::fs::create_dir_all(&selected).map_err(|error| error.to_string())?;
    let selected_text = selected.to_string_lossy();
    for (flag, sentinel) in [("--root", "--baseline"), ("--baseline", "--root")] {
        for value in ["C:repo", "C:", r"\repo", "/repo"] {
            // A missing value on the sibling flag makes a wrong parser fail
            // before IO too, without touching an unowned per-drive directory.
            let args = [
                "pr-summary",
                "--root",
                &selected_text,
                flag,
                value,
                sentinel,
            ];
            let output = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&scratch), &args)
                .map_err(|error| error.to_string())?;
            let error = String::from_utf8_lossy(&output.stderr);
            if output.status.code() != Some(2)
                || !error.contains(flag)
                || !error.contains("partially qualified")
            {
                return Err(format!(
                    "partially qualified {args:?} lacked early scoped failure: {output:?}"
                ));
            }
        }
    }
    if scratch.join("target").exists() || selected.join("target").exists() {
        return Err("partially qualified summary invocation wrote outputs".to_string());
    }
    Ok(())
}

#[test]
fn pr_summary_help_exits_cleanly() {
    let output = run_ripr(&["pr-summary", "--help"]);
    assert!(
        output.status.success(),
        "pr-summary --help must succeed\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--check"),
        "help must mention --check:\n{stdout}"
    );
    assert!(
        stdout.contains("--baseline"),
        "help must mention --baseline:\n{stdout}"
    );
}

#[test]
fn pr_summary_rendered_help_describes_selected_root() -> Result<(), String> {
    let output = run_ripr(&["pr-summary", "--help"]);
    let help = String::from_utf8_lossy(&output.stdout);
    if !output.status.success()
        || !help.contains("--root <path>")
        || !help.contains("default: current directory")
        || !help.contains("relative to the selected root")
        || !help.contains("drive-relative and root-relative paths")
    {
        return Err(format!("summary root help contract missing: {help}"));
    }
    Ok(())
}

#[test]
fn pr_summary_with_missing_artifacts_writes_outputs() -> Result<(), String> {
    let root = unique_temp_workspace("pr-summary-missing");
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"pr-summary-missing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    let bin = std::fs::canonicalize(env!("CARGO_BIN_EXE_ripr"))
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_BIN_EXE_ripr")));
    let output = std::process::Command::new(&bin)
        .current_dir(&root)
        .arg("pr-summary")
        .output()
        .map_err(|err| err.to_string())?;
    assert!(
        output.status.success(),
        "pr-summary must succeed even with missing artifacts:\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.join("target/ripr/pr/summary.md").is_file(),
        "must write target/ripr/pr/summary.md"
    );
    assert!(
        root.join("target/ripr/reports/pr-evidence-summary.json")
            .is_file(),
        "must write pr-evidence-summary.json"
    );
    assert!(
        root.join("target/ripr/reports/pr-evidence-summary.md")
            .is_file(),
        "must write pr-evidence-summary.md"
    );
    let json = std::fs::read_to_string(root.join("target/ripr/reports/pr-evidence-summary.json"))
        .unwrap_or_default();
    assert!(
        json.contains("not_available"),
        "missing artifacts must surface not_available, not zero:\n{json}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn pr_summary_unknown_arg_fails_clearly() {
    let output = run_ripr(&["pr-summary", "--bogus"]);
    assert!(!output.status.success(), "unknown arg must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown pr-summary argument") || stderr.contains("--bogus"),
        "error must name the unknown arg:\n{stderr}"
    );
}

#[test]
fn pr_summary_does_not_invoke_cargo() -> Result<(), String> {
    let root = unique_temp_workspace("pr-summary-no-compile");
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"pr-summary-no-compile\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    let bin = std::fs::canonicalize(env!("CARGO_BIN_EXE_ripr"))
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_BIN_EXE_ripr")));
    let start = std::time::Instant::now();
    let output = std::process::Command::new(&bin)
        .current_dir(&root)
        .arg("pr-summary")
        .output()
        .map_err(|err| err.to_string())?;
    let elapsed = start.elapsed();
    assert!(output.status.success(), "pr-summary must succeed");
    assert!(
        elapsed.as_secs() < 10,
        "pr-summary must complete in <10s (no compile); took {elapsed:?}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

// ── ripr annotations (Campaign 31 item 8b: binary-first annotations) ──

#[test]
fn annotations_help_exits_cleanly() {
    let output = run_ripr(&["annotations", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--comments"),
        "help must mention --comments:\n{stdout}"
    );
    assert!(
        stdout.contains("--out"),
        "help must mention --out:\n{stdout}"
    );
    assert!(
        stdout.contains("--check"),
        "help must mention --check:\n{stdout}"
    );
}

#[test]
fn annotations_with_missing_comments_writes_empty() -> Result<(), String> {
    let root = unique_temp_workspace("annotations-missing");
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"annotations-missing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    let bin = std::fs::canonicalize(env!("CARGO_BIN_EXE_ripr"))
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_BIN_EXE_ripr")));
    let output = std::process::Command::new(&bin)
        .current_dir(&root)
        .arg("annotations")
        .output()
        .map_err(|err| err.to_string())?;
    assert!(
        output.status.success(),
        "annotations must succeed with missing comments.json:\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.join("target/ripr/review/annotations.txt").is_file(),
        "must write annotations.txt even when comments.json is missing"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn annotations_unknown_arg_fails_clearly() {
    let output = run_ripr(&["annotations", "--bogus"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown annotations argument") || stderr.contains("--bogus"),
        "error must name the unknown arg:\n{stderr}"
    );
}

// ── ripr pr-evidence (Campaign 31 item 8c: binary-first PR evidence packet) ──

#[test]
fn pr_evidence_help_exits_cleanly() {
    let output = run_ripr(&["pr-evidence", "--help"]);
    assert!(
        output.status.success(),
        "pr-evidence --help must succeed\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--base"),
        "help must mention --base:\n{stdout}"
    );
    assert!(
        stdout.contains("--head"),
        "help must mention --head:\n{stdout}"
    );
    assert!(
        stdout.contains("--check"),
        "help must mention --check:\n{stdout}"
    );
}

#[test]
fn pr_evidence_with_missing_artifacts_writes_error_packet() -> Result<(), String> {
    // pr-evidence runs a live check; in a bare workspace the result is either
    // an error packet (if git revisions resolve in the parent repo) or a
    // revision error. Both are honest — the key is that it does not silently
    // produce a misleading clean packet. Assert that the output mentions the
    // evidence artifact or an error, not that it succeeds or fails.
    let root = unique_temp_workspace("pr-evidence-missing");
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"pr-evidence-missing\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| err.to_string())?;
    let bin = std::fs::canonicalize(env!("CARGO_BIN_EXE_ripr"))
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_BIN_EXE_ripr")));
    let output = std::process::Command::new(&bin)
        .current_dir(&root)
        .arg("pr-evidence")
        .output()
        .map_err(|err| err.to_string())?;
    // The command either writes evidence (Ok) or surfaces an error (Err).
    // Both are acceptable. What's NOT acceptable: a silent hang or crash.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("repo-exposure")
            || combined.contains("bad base/head")
            || combined.contains("error"),
        "pr-evidence must either write evidence or surface a named error:\n{combined}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn pr_evidence_unknown_arg_fails_clearly() {
    let output = run_ripr(&["pr-evidence", "--bogus"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown pr-evidence argument") || stderr.contains("--bogus"),
        "error must name the unknown arg:\n{stderr}"
    );
}

// ── ripr impacted-evidence (item 8e: binary-first impacted evidence) ──

#[test]
fn impacted_evidence_help_exits_cleanly() {
    let output = run_ripr(&["impacted-evidence", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--label"),
        "help must mention --label:\n{stdout}"
    );
    assert!(
        stdout.contains("--labels"),
        "help must mention --labels:\n{stdout}"
    );
    assert!(
        stdout.contains("--pr-evidence"),
        "help must mention --pr-evidence:\n{stdout}"
    );
}

#[test]
fn impacted_evidence_unknown_arg_fails_clearly() {
    let output = run_ripr(&["impacted-evidence", "--bogus"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown impacted-evidence argument") || stderr.contains("--bogus"),
        "error must name the unknown arg:\n{stderr}"
    );
}

// ── ripr plus (binary-first RIPR+ repo receipt, composition-only) ──

#[test]
fn plus_help_exits_cleanly() {
    let output = run_ripr(&["plus", "--help"]);
    assert!(
        output.status.success(),
        "plus --help must succeed\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--repo-exposure-summary"),
        "help must mention --repo-exposure-summary:\n{stdout}"
    );
    assert!(
        stdout.contains("--gap-ledger"),
        "help must mention --gap-ledger:\n{stdout}"
    );
}

/// An exposure-only counter is not a complete, current RIPR+ quality result.
/// Exercise the installed command and written receipt, not a fabricated gate.
#[test]
fn plus_partial_zero_cannot_be_used_as_a_current_quality_gate() -> Result<(), String> {
    let root = unique_temp_workspace("plus-incomplete-zero");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    run_git(&root, &["init"])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(root.join("README.md"), "quality receipt fixture\n")
        .map_err(|error| error.to_string())?;
    run_git(&root, &["add", "README.md"])?;
    run_git(&root, &["commit", "-m", "candidate"])?;
    for known in [0, 2] {
        let summary = serde_json::json!({
            "schema_version": "0.1",
            "format": "repo-exposure-summary-json",
            "basis": "canonical_actionable_gap",
            "metadata": {"head": "unrelated-source-head", "root": "other-repository"},
            "metrics": {
                "unsuppressed_exposure_gaps": known,
                "suppressed_exposure_gaps": 0,
                "raw_seams": 8,
                "grip_class": {"weakly_gripped": 5}
            },
            "reason_breakdown": {"gap_state": {"static_limitation": 6}},
            "top_files": [],
            // Unrecognized claims cannot promote this legacy input contract.
            "qualification": {"complete": true, "zero_unresolved": true}
        });
        std::fs::write(root.join("summary.json"), summary.to_string())
            .map_err(|error| error.to_string())?;
        let composed = run_command(
            env!("CARGO_BIN_EXE_ripr"),
            Some(&root),
            &["plus", "--repo-exposure-summary", "summary.json"],
        )
        .map_err(|error| error.to_string())?;
        assert!(
            composed.status.success(),
            "{}",
            String::from_utf8_lossy(&composed.stderr)
        );
        let receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("target/ripr/reports/ripr-plus.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(receipt["status"], "indeterminate", "{receipt}");
        assert!(receipt["unresolved"].is_null(), "{receipt}");
        assert_eq!(receipt["known_actionable_unresolved"], known, "{receipt}");
        assert_eq!(receipt["zero_unresolved_established"], false, "{receipt}");
        assert!(
            receipt["head"].is_null(),
            "receipt must not bind unrelated evidence to HEAD: {receipt}"
        );
        assert_eq!(receipt["candidate_binding"], "not_established", "{receipt}");
        let checked = run_command(
            env!("CARGO_BIN_EXE_ripr"),
            Some(&root),
            &["plus", "--repo-exposure-summary", "summary.json", "--check"],
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(checked.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&checked.stderr).contains("zero is not established"));
    }
    let qualified = ripr::app::qualify_legacy_ripr_plus_receipt(serde_json::json!({
        "status": "pass", "unresolved": 0, "head": "old-head", "warnings": []
    }))?;
    assert_eq!(
        ripr::app::qualify_legacy_ripr_plus_receipt(qualified.clone())?,
        qualified
    );
    assert_eq!(
        ripr::app::qualify_legacy_ripr_plus_receipt(serde_json::json!({
            "status": "pass", "unresolved": 0, "warnings": "malformed"
        }))
        .err()
        .as_deref(),
        Some("RIPR+ receipt warnings must be an array")
    );
    let ledger = include_str!(
        "../../../fixtures/first_successful_pr/empty-diff/inputs/reports/gap-decision-ledger.json"
    );
    std::fs::write(root.join("ledger.json"), ledger).map_err(|error| error.to_string())?;
    for (flag, file) in [
        ("--gap-ledger", "ledger.json"),
        ("--repo-exposure-summary", "broken.json"),
    ] {
        std::fs::write(root.join("broken.json"), "{broken").map_err(|error| error.to_string())?;
        // A stale successful artifact must be replaced even on invalid input.
        std::fs::write(
            root.join("target/ripr/reports/ripr-plus.json"),
            r#"{"status":"pass","unresolved":0,"head":"stale"}"#,
        )
        .map_err(|error| error.to_string())?;
        let output = run_command(
            env!("CARGO_BIN_EXE_ripr"),
            Some(&root),
            &["plus", flag, file, "--check"],
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(output.status.code(), Some(2));
        let receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("target/ripr/reports/ripr-plus.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(receipt["status"], "indeterminate");
        assert!(receipt["head"].is_null() && receipt["unresolved"].is_null());
        assert_eq!(receipt["zero_unresolved_established"], false);
        let markdown = std::fs::read_to_string(root.join("target/ripr/reports/ripr-plus.md"))
            .map_err(|error| error.to_string())?;
        assert!(markdown.contains("| Unresolved | N/A"));
        assert!(!markdown.contains("| Unresolved | 0 |"));
    }
    std::fs::remove_dir_all(root).map_err(|error| error.to_string())?;
    Ok(())
}

/// Build a real producer packet for a verify route and return (root, packet).
///
/// The packet comes from `ripr agent packet --gap-ledger`, the canonical
/// producer, so this exercises the shape RIPR actually emits rather than a
/// hand-built approximation of it.
fn producer_verify_packet(
    label: &str,
) -> Result<(PathBuf, serde_json::Value), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(&root)?;
    init_producer_fixture_repo(&root)?;
    let root_arg = root.to_string_lossy().into_owned();

    let snapshot = run_ripr(&[
        "check",
        "--root",
        &root_arg,
        "--format",
        "repo-exposure-json",
    ]);
    assert_success(&snapshot);
    std::fs::write(root.join("before.json"), &snapshot.stdout)?;
    std::fs::create_dir_all(root.join("target/ripr/pilot"))?;
    std::fs::write(
        root.join("target/ripr/pilot/repo-exposure.json"),
        &snapshot.stdout,
    )?;

    // Movement is required for a fully current pair (#2922): advance the
    // fixture so the executed verify route compares ordered revisions.
    advance_fixture_head(&root, "after movement")?;
    let snapshot = run_ripr(&[
        "check",
        "--root",
        &root_arg,
        "--format",
        "repo-exposure-json",
    ]);
    assert_success(&snapshot);
    std::fs::write(root.join("after.json"), &snapshot.stdout)?;
    std::fs::write(
        root.join("target/ripr/pilot/after.repo-exposure.json"),
        &snapshot.stdout,
    )?;

    let ledger_path = root.join("gap-ledger.json");
    let source_path = root.join("after.repo-exposure.json");
    std::fs::write(&source_path, &snapshot.stdout)?;
    let source_arg = source_path.to_string_lossy().into_owned();
    let ledger_arg = ledger_path.to_string_lossy().into_owned();
    let ledger = run_ripr(&[
        "reports",
        "gap-ledger",
        "--repo-exposure",
        &source_arg,
        "--root",
        &root_arg,
        "--out",
        &ledger_arg,
        "--out-md",
        &root.join("gap-ledger.md").to_string_lossy(),
    ]);
    assert_success(&ledger);

    let ledger_json = std::fs::read_to_string(&ledger_path)?;
    let ledger_value: serde_json::Value = serde_json::from_str(&ledger_json)?;
    let gap_id = ledger_value["records"][0]["gap_id"]
        .as_str()
        .ok_or("producer fixture must emit one gap record")?;

    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_arg,
        "--gap-ledger",
        &ledger_arg,
        "--gap-id",
        gap_id,
        "--json",
    ]);
    assert_success(&packet);
    let parsed: serde_json::Value = serde_json::from_slice(&packet.stdout)?;
    // Guard the producer contract this command depends on: the typed verify
    // specs are an array, not a single object.
    assert!(
        parsed["packets"][0]["command_specs"]["verify"].is_array(),
        "producer must emit command_specs.verify as an array:\n{parsed}"
    );
    std::fs::write(root.join("packet.json"), &packet.stdout)?;
    Ok((root, parsed))
}

fn verify_execute_disposition(
    root: &Path,
    argv: &[&str],
) -> Result<(serde_json::Value, Option<i32>), Box<dyn std::error::Error>> {
    let output = run_command(env!("CARGO_BIN_EXE_ripr"), Some(root), argv)?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "verify-execute must always emit typed JSON on stdout: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })?;
    Ok((parsed, output.status.code()))
}

/// The end-to-end contract: a producer-generated packet executes through the
/// public CLI, commits a schema-shaped result, and never carries an ambient
/// secret into its output.
#[test]
fn agent_verify_execute_runs_a_producer_owned_route_and_commits_a_result()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _) = producer_verify_packet("verify-execute-pass")?;
    let canary = "ripr-canary-c0ffee-must-not-appear";
    let output = run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        &root,
        &[
            "agent",
            "verify-execute",
            "--root",
            ".",
            "--packet",
            "packet.json",
            "--result-json",
            "result.json",
            "--authorize",
            "--json",
        ],
        &[("RIPR_TEST_SECRET_CANARY", canary)],
    )?;
    assert_success(&output);
    let stdout = String::from_utf8(output.stdout.clone())?;
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        parsed["disposition"], "verification_executed_pass",
        "expected a pass from the real route:\n{stdout}"
    );
    assert_eq!(parsed["executed"], true);
    assert_eq!(parsed["result_committed"], true);
    assert_eq!(parsed["result"]["process_disposition"], "completed");
    assert_eq!(parsed["result"]["exit_status"], 0);
    assert_eq!(parsed["result"]["currentness"], "current");
    assert_eq!(
        parsed["result"]["head_before"], parsed["result"]["head_after"],
        "an unchanged repository must report identical heads"
    );
    for digest in ["command_spec_sha256", "stdout_sha256", "stderr_sha256"] {
        let value = parsed["result"][digest]
            .as_str()
            .ok_or_else(|| format!("{digest} must be a string"))?;
        assert!(value.starts_with("sha256:"), "{digest} = {value}");
    }
    // Input identities are committed, not merely path-checked.
    assert_eq!(parsed["inputs"].as_array().map(Vec::len), Some(2));

    // The committed artifact must exist and match what stdout reported.
    let committed = std::fs::read_to_string(root.join("result.json"))?;
    let committed: serde_json::Value = serde_json::from_str(&committed)?;
    assert_eq!(committed["result"], parsed["result"]);

    // No ambient secret reaches stdout, stderr, or the committed artifact.
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    for surface in [&stdout, &stderr, &committed.to_string()] {
        assert!(
            !surface.contains(canary),
            "ambient secret leaked into an emitted surface"
        );
    }
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn agent_packet_blocks_untracked_consumed_config_change() -> Result<(), Box<dyn std::error::Error>>
{
    let (root, packet) = producer_verify_packet("untracked-config-currentness")?;
    let gap_id = packet["packets"][0]["gap_id"]
        .as_str()
        .ok_or("producer packet must contain a gap id")?;
    std::fs::write(
        root.join("ripr.toml"),
        "[oracles]\nsnapshot_strength = \"strong\"\n",
    )?;
    let root_arg = root.to_string_lossy().into_owned();
    let ledger_arg = root.join("gap-ledger.json").to_string_lossy().into_owned();
    let output = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_arg,
        "--gap-ledger",
        &ledger_arg,
        "--gap-id",
        gap_id,
        "--json",
    ]);
    assert_success(&output);
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed["packets"].as_array().map(Vec::len),
        Some(0),
        "stale currentness must not expose assignable packets: {parsed}"
    );
    assert_eq!(
        parsed["source_currentness"]["status"], "stale",
        "a newly added consumed config must invalidate assignment: {parsed}"
    );
    assert_eq!(
        parsed["blocked_candidate"]["queue_state"], "blocked_stale",
        "stale currentness must expose the blocked candidate disposition: {parsed}"
    );
    assert!(
        parsed["blocked_candidate"]
            .get("verification_commands")
            .is_none(),
        "stale candidate must not expose verification command authority: {parsed}"
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// The public swarm queue must expose a producer-validated current source as
/// assignable, then withdraw that authority when the selected repository HEAD
/// moves without refreshing the source and ledger.
#[test]
fn swarm_queue_assigns_current_source_and_blocks_changed_head()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _) = producer_verify_packet("swarm-queue-currentness")?;
    let queue_args = [
        "swarm",
        "queue",
        "--root",
        ".",
        "--gap-ledger",
        "gap-ledger.json",
        "--language",
        "rust",
        "--top",
        "1",
        "--format",
        "json",
    ];

    let current = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), &queue_args)?;
    assert_success(&current);
    let current: serde_json::Value = serde_json::from_slice(&current.stdout)?;
    assert_eq!(current["source_currentness"]["status"], "current");
    assert_eq!(current["source_currentness"]["queue_state"], "queued");
    assert_eq!(current["summary"]["assignable_total"], 1);
    assert_eq!(current["summary"]["unreturned_assignable_total"], 0);
    assert_eq!(current["packets"].as_array().map(Vec::len), Some(1));
    assert_eq!(current["packets"][0]["queue_state"], "queued");
    assert_eq!(current["packets"][0]["staleness_status"], "current");
    assert_eq!(current["packets"][0]["assignment"]["eligible"], true);
    assert!(current["packets"][0]["command_specs"]["verify"].is_array());

    advance_fixture_head(&root, "queue currentness movement")?;
    let stale = run_command(env!("CARGO_BIN_EXE_ripr"), Some(&root), &queue_args)?;
    assert_success(&stale);
    let stale: serde_json::Value = serde_json::from_slice(&stale.stdout)?;
    assert_eq!(stale["status"], "blocked");
    assert_eq!(stale["source_currentness"]["status"], "stale");
    assert_eq!(stale["source_currentness"]["queue_state"], "blocked_stale");
    assert_eq!(
        stale["packets"].as_array().map(Vec::len),
        Some(0),
        "a stale source must not expose assignable packets: {stale}"
    );
    let review = stale["blocked_review"]
        .as_array()
        .and_then(|entries| entries.first())
        .ok_or("stale queue must keep the candidate visible as review-only")?;
    assert_eq!(review["queue_state"], "blocked_stale");
    assert_eq!(review["staleness_status"], "stale");
    assert_eq!(review["assignment"]["eligible"], false);
    assert!(review.get("command_specs").is_none());
    assert!(review.get("packet_command_args").is_none());
    assert!(review.get("verify_command").is_none());
    assert!(review.get("allowed_files").is_none());
    assert!(
        review["refresh_commands"]
            .as_array()
            .is_some_and(|commands| !commands.is_empty()),
        "review-only projections must carry their recovery route: {stale}"
    );
    assert_eq!(stale["summary"]["assignable_total"], 0);
    assert_eq!(
        stale["summary"]["blocked_review_total"], 1,
        "the blocked denominator must stay visible"
    );

    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// Every refusal is a typed disposition on stdout, not a bare stderr string,
/// and no refusal commits a result.
#[test]
fn agent_verify_execute_refusals_are_typed_dispositions() -> Result<(), Box<dyn std::error::Error>>
{
    let (root, packet) = producer_verify_packet("verify-execute-refusals")?;

    // A caller-edited typed spec cannot borrow producer authority.
    let mut forged = packet.clone();
    forged["packets"][0]["command_specs"]["verify"][0]["args"][1] =
        serde_json::Value::String("receipt".to_string());
    std::fs::write(
        root.join("forged.json"),
        serde_json::to_vec_pretty(&forged)?,
    )?;

    // An input outside the root is refused even when the display text agrees.
    let escaped = "ripr agent verify --root . --before ../escape.json --after target/ripr/pilot/after.repo-exposure.json --json";
    let mut outside = packet.clone();
    outside["packets"][0]["verify_command"] = serde_json::Value::String(escaped.to_string());
    outside["packets"][0]["verification_commands"] = serde_json::json!([escaped]);
    outside["packets"][0]["command_specs"]["verify"][0]["args"] = serde_json::json!([
        "agent",
        "verify",
        "--root",
        ".",
        "--before",
        "../escape.json",
        "--after",
        "target/ripr/pilot/after.repo-exposure.json",
        "--json"
    ]);
    outside["packets"][0]["command_specs"]["verify"][0]["human_display"] =
        serde_json::Value::String(escaped.to_string());
    std::fs::write(
        root.join("outside.json"),
        serde_json::to_vec_pretty(&outside)?,
    )?;

    let cases: Vec<(&str, Vec<&str>, &str)> = vec![
        (
            "missing authorization",
            vec![
                "agent",
                "verify-execute",
                "--root",
                ".",
                "--packet",
                "packet.json",
                "--result-json",
                "unauthorized.json",
                "--json",
            ],
            "verification_rejected_policy",
        ),
        (
            "caller-authored typed spec",
            vec![
                "agent",
                "verify-execute",
                "--root",
                ".",
                "--packet",
                "forged.json",
                "--result-json",
                "forged-result.json",
                "--authorize",
                "--json",
            ],
            "verification_rejected_policy",
        ),
        (
            "input outside the root",
            vec![
                "agent",
                "verify-execute",
                "--root",
                ".",
                "--packet",
                "outside.json",
                "--result-json",
                "outside-result.json",
                "--authorize",
                "--json",
            ],
            "verification_wrong_root",
        ),
        (
            "result destination outside the root",
            vec![
                "agent",
                "verify-execute",
                "--root",
                ".",
                "--packet",
                "packet.json",
                "--result-json",
                "../outside-result.json",
                "--authorize",
                "--json",
            ],
            "verification_wrong_root",
        ),
    ];
    for (label, argv, expected) in cases {
        let (parsed, exit_code) = verify_execute_disposition(&root, &argv)?;
        assert_eq!(parsed["disposition"], expected, "{label}: {parsed}");
        assert_eq!(parsed["executed"], false, "{label} must not execute");
        assert_eq!(parsed["result_committed"], false, "{label}");
        // A typed refusal ran successfully to a blocking answer: exit 3, not 2.
        assert_eq!(exit_code, Some(3), "{label} must exit with code 3");
    }
    // No refusal left a result behind.
    for name in [
        "unauthorized.json",
        "forged-result.json",
        "outside-result.json",
    ] {
        assert!(
            !root.join(name).exists(),
            "{name} must not exist after a refusal"
        );
    }
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// A destination that cannot be written still reports a typed disposition and
/// leaves the existing artifact untouched.
#[test]
fn agent_verify_execute_reports_result_write_failure() -> Result<(), Box<dyn std::error::Error>> {
    let (root, _) = producer_verify_packet("verify-execute-write-failed")?;
    std::fs::write(root.join("result.json"), "existing-artifact")?;
    let (parsed, exit_code) = verify_execute_disposition(
        &root,
        &[
            "agent",
            "verify-execute",
            "--root",
            ".",
            "--packet",
            "packet.json",
            "--result-json",
            "result.json",
            "--authorize",
            "--json",
        ],
    )?;
    assert_eq!(parsed["disposition"], "verification_result_write_failed");
    // The observation happened; only the commit failed. Both facts are reported.
    assert_eq!(parsed["executed"], true);
    assert_eq!(parsed["result_committed"], false);
    // An uncommitted observation could not complete: it stays on exit code 2.
    assert_eq!(
        exit_code,
        Some(2),
        "an uncommitted result must exit with code 2"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("result.json"))?,
        "existing-artifact",
        "the pre-existing artifact must be preserved"
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// A tampered input is refused by provenance *before* any process starts.
///
/// This is the end-to-end half of the producer-binding contract: the route in
/// the packet is unchanged and internally consistent, but one of the artifacts
/// it names is no longer a valid `ripr` repo-exposure artifact, so no route can
/// be recomputed and nothing is executed.
///
/// It also records a real limitation. The only executable route is
/// `ripr agent verify` over two provenance-valid artifacts, and that command
/// always exits 0 — so `verification_executed_fail` is mapped and unit-tested
/// but not reachable end-to-end today. Reaching it needs a fallible typed route
/// (for example a `cargo test` route) in the command catalog, which this change
/// deliberately does not add.
#[test]
fn agent_verify_execute_refuses_a_tampered_input_before_executing()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _) = producer_verify_packet("verify-execute-tampered")?;
    std::fs::write(
        root.join("target/ripr/pilot/after.repo-exposure.json"),
        "not-a-repo-exposure-artifact",
    )?;
    let output = run_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(&root),
        &[
            "agent",
            "verify-execute",
            "--root",
            ".",
            "--packet",
            "packet.json",
            "--result-json",
            "result.json",
            "--authorize",
            "--json",
        ],
    )?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed["disposition"], "verification_rejected_policy",
        "a tampered artifact must be refused, not executed: {parsed}"
    );
    let reason = parsed["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("provenance validation"),
        "refusal must name provenance: {reason}"
    );
    assert_eq!(
        parsed["executed"], false,
        "no process may start once provenance fails"
    );
    assert_eq!(parsed["result_committed"], false);
    assert!(!output.status.success());
    assert!(
        !root.join("result.json").exists(),
        "a refusal must not leave a result behind"
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// A coherently rewritten packet cannot buy execution.
///
/// Every route representation is rewritten consistently to name files that are
/// not producer artifacts. Consistency checks pass; provenance refuses. This is
/// the end-to-end counterpart of the unit-level forgery test.
#[test]
fn agent_verify_execute_refuses_a_coherent_whole_packet_forgery()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, packet) = producer_verify_packet("verify-execute-forgery")?;
    std::fs::write(root.join("alt-before.json"), "{}")?;
    std::fs::write(root.join("alt-after.json"), "{}")?;
    let forged_route =
        "ripr agent verify --root . --before alt-before.json --after alt-after.json --json";
    let mut forged = packet.clone();
    forged["packets"][0]["verify_command"] = serde_json::Value::String(forged_route.to_string());
    forged["packets"][0]["verification_commands"] = serde_json::json!([forged_route]);
    let spec = &mut forged["packets"][0]["command_specs"]["verify"][0];
    spec["args"][5] = serde_json::Value::String("alt-before.json".to_string());
    spec["args"][7] = serde_json::Value::String("alt-after.json".to_string());
    spec["human_display"] = serde_json::Value::String(forged_route.to_string());
    std::fs::write(
        root.join("forged.json"),
        serde_json::to_vec_pretty(&forged)?,
    )?;

    let (parsed, exit_code) = verify_execute_disposition(
        &root,
        &[
            "agent",
            "verify-execute",
            "--root",
            ".",
            "--packet",
            "forged.json",
            "--result-json",
            "forged-result.json",
            "--authorize",
            "--json",
        ],
    )?;
    assert_eq!(
        parsed["disposition"], "verification_rejected_policy",
        "a coherent forgery must be refused: {parsed}"
    );
    assert_eq!(parsed["executed"], false);
    // A typed refusal maps to the decision exit code 3.
    assert_eq!(exit_code, Some(3));
    assert!(!root.join("forged-result.json").exists());
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// Cancellation yields exactly one terminal disposition and no exit status.
#[test]
fn agent_verify_execute_cancellation_is_a_single_terminal_result()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, _) = producer_verify_packet("verify-execute-cancel")?;
    let (parsed, _) = verify_execute_disposition(
        &root,
        &[
            "agent",
            "verify-execute",
            "--root",
            ".",
            "--packet",
            "packet.json",
            "--result-json",
            "result.json",
            "--authorize",
            "--cancel-after-ms",
            "1",
            "--json",
        ],
    )?;
    // The child may win the race and complete; either way exactly one terminal
    // disposition is reported and the invariants for it hold.
    let disposition = parsed["disposition"]
        .as_str()
        .ok_or("disposition must be a string")?;
    match disposition {
        "verification_cancelled" => {
            assert_eq!(parsed["result"]["process_disposition"], "cancelled");
            assert_eq!(parsed["result"]["cancellation_requested"], true);
            assert!(
                parsed["result"]["exit_status"].is_null(),
                "a cancelled run must not carry an exit status"
            );
        }
        "verification_executed_pass" | "verification_executed_fail" => {
            assert_eq!(parsed["result"]["process_disposition"], "completed");
            assert!(parsed["result"]["exit_status"].is_i64());
        }
        other => return Err(format!("unexpected cancellation disposition {other}").into()),
    }
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn plus_unknown_arg_fails_clearly() {
    let output = run_ripr(&["plus", "--bogus"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown plus argument") || stderr.contains("--bogus"),
        "error must name the unknown arg:\n{stderr}"
    );
}

/// A minimal unified diff piped through stdin, the documented
/// `git diff origin/main | ripr check --diff -` shape (#4319).
const UNIFIED_DIFF_FOR_STDIN_PROBE: &str = "diff --git a/src/lib.rs b/src/lib.rs\n\
index 1111111..2222222 100644\n\
--- a/src/lib.rs\n\
+++ b/src/lib.rs\n\
@@ -1,1 +1,2 @@\n\
+fn changed() {}\n\
 fn existing() {}\n";

struct DeclaredHunkScratch(PathBuf);

impl Drop for DeclaredHunkScratch {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.0);
    }
}

#[test]
fn truncated_declared_hunk_is_incomplete_through_file_and_stdin()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = unique_temp_workspace("declared-hunk-completeness");
    std::fs::create_dir(&scratch)?;
    let _cleanup = DeclaredHunkScratch(scratch.clone());
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.display().to_string();
    let prefix = concat!(
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,5 +1,5 @@\n",
        " pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n",
        "-    if amount > discount_threshold {\n+    if amount >= discount_threshold {\n",
    );
    let complete = format!("{prefix}         amount - 10\n     }} else {{\n         amount\n");
    for (label, input, expected_complete) in [
        ("partial", prefix, false),
        ("complete", complete.as_str(), true),
    ] {
        let path = scratch.join(format!("{label}.patch"));
        std::fs::write(&path, input)?;
        let path_arg = path.display().to_string();
        let file_output = run_command(
            env!("CARGO_BIN_EXE_ripr"),
            None,
            &["check", "--root", &root_arg, "--diff", &path_arg, "--json"],
        )?;
        let stdin_output = run_ripr_with_stdin(
            &["check", "--root", &root_arg, "--diff", "-", "--json"],
            input.as_bytes(),
        )?;
        for (channel, output) in [("file", file_output), ("stdin", stdin_output)] {
            if !output.status.success() {
                return Err(format!(
                    "{label}/{channel}: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            if report
                .pointer("/analysis_outcome/analysis_complete")
                .and_then(serde_json::Value::as_bool)
                != Some(expected_complete)
            {
                return Err(format!("{label}/{channel}: wrong completeness: {report}").into());
            }
            let outcome = report
                .pointer("/analysis_outcome/outcome")
                .ok_or("missing typed outcome")?;
            if outcome
                .pointer("/counts/changed_file_count")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
                || outcome
                    .pointer("/counts/probe_count")
                    .and_then(serde_json::Value::as_u64)
                    .is_none_or(|count| count == 0)
            {
                return Err(
                    "declared-hunk journey must analyze one file and nonzero probes".into(),
                );
            }
            let limitations = outcome
                .get("limitations")
                .and_then(serde_json::Value::as_array)
                .ok_or("missing typed limitations")?;
            let malformed = limitations.iter().any(|item| {
                item.get("kind").and_then(serde_json::Value::as_str) == Some("malformed_diff")
                    && item
                        .get("producer_stage")
                        .and_then(serde_json::Value::as_str)
                        == Some("diff_parse")
                    && item
                        .pointer("/recovery/kind")
                        .and_then(serde_json::Value::as_str)
                        == Some("retry")
            });
            if malformed == expected_complete {
                return Err(format!(
                    "{label}/{channel}: wrong declared-span limitation: {outcome}"
                )
                .into());
            }
        }
    }
    Ok(())
}

#[test]
fn truncated_unsupported_file_hunk_is_incomplete_through_file_and_stdin()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = unique_temp_workspace("declared-hunk-unsupported-language");
    std::fs::create_dir(&scratch)?;
    let _cleanup = DeclaredHunkScratch(scratch.clone());
    let root = workspace_root().join("fixtures/boundary_gap/input");
    let root_arg = root.display().to_string();
    for (label, input, complete) in [
        (
            "partial",
            "--- a/README.md\n+++ b/README.md\n@@ -0,0 +1,2 @@\n+retained\n",
            false,
        ),
        (
            "complete",
            "--- a/README.md\n+++ b/README.md\n@@ -0,0 +1,1 @@\n+retained\n",
            true,
        ),
    ] {
        let path = scratch.join(format!("{label}.patch"));
        std::fs::write(&path, input)?;
        let path_arg = path.display().to_string();
        let file = run_command(
            env!("CARGO_BIN_EXE_ripr"),
            None,
            &["check", "--root", &root_arg, "--diff", &path_arg, "--json"],
        )?;
        let stdin = run_ripr_with_stdin(
            &["check", "--root", &root_arg, "--diff", "-", "--json"],
            input.as_bytes(),
        )?;
        for (channel, output) in [("file", file), ("stdin", stdin)] {
            if !output.status.success() {
                return Err(format!(
                    "{label}/{channel}: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            let outcome = report
                .pointer("/analysis_outcome/outcome")
                .ok_or("missing typed outcome")?;
            if outcome
                .pointer("/counts/changed_file_count")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
                || outcome
                    .pointer("/counts/probe_count")
                    .and_then(serde_json::Value::as_u64)
                    != Some(0)
            {
                return Err(
                    "unsupported-language control must retain one file and zero probes".into(),
                );
            }
            let malformed = outcome
                .get("limitations")
                .and_then(serde_json::Value::as_array)
                .ok_or("missing typed limitations")?
                .iter()
                .any(|item| {
                    item.get("kind").and_then(serde_json::Value::as_str) == Some("malformed_diff")
                        && item
                            .get("producer_stage")
                            .and_then(serde_json::Value::as_str)
                            == Some("diff_parse")
                        && item
                            .pointer("/recovery/kind")
                            .and_then(serde_json::Value::as_str)
                            == Some("retry")
                });
            if report
                .pointer("/analysis_outcome/analysis_complete")
                .and_then(serde_json::Value::as_bool)
                != Some(complete)
                || malformed == complete
            {
                return Err(format!(
                    "{label}/{channel}: unsupported language hid malformed input: {report}"
                )
                .into());
            }
        }
    }
    Ok(())
}

/// #4319 end-to-end silence guarantee for the attached-terminal stdin
/// disclosure at the cli boundary (`cli::parse::disclose_attached_terminal_stdin_read`):
/// the note may be emitted only when the child's stdin is an attached
/// terminal, so a piped run must keep the note text off stderr entirely.
/// This challenges accidental emission on non-terminal stdin. The shared
/// emitter's positive callback is challenged separately in cli/parse.rs;
/// deleting the disclosure cannot be detected by a silence assertion.
///
/// The terminal-positive arm cannot be produced under cargo's captured
/// stdio (no PTY/winpty in this environment), so the on-terminal emission
/// itself remains a manual `check --diff -` spot-check; the pure
/// terminal/non-terminal decision is pinned in `cli/parse.rs`'s unit test.
#[test]
fn check_diff_stdin_from_a_pipe_stays_silent_about_terminal_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    let note = "ripr: reading the diff from the attached terminal; paste the diff and press Ctrl+Z then Enter on Windows, or Ctrl+D on Unix, to end input";
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
    let output = run_ripr_with_stdin(
        &[
            "check",
            "--root",
            &root.display().to_string(),
            "--diff",
            "-",
            "--json",
        ],
        UNIFIED_DIFF_FOR_STDIN_PROBE.as_bytes(),
    )?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "piped diff analysis failed: {stderr}"
    );
    let _: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        stderr.matches(note).count(),
        0,
        "piped `check --diff -` must stay silent about the attached-terminal stdin disclosure:\n{stderr}"
    );
    Ok(())
}

/// A repository with no `origin` remote whose default branch is `master`, on a
/// `work` branch one commit ahead. `master` is deliberately not `main`, so a
/// command that still assumes `origin/main` cannot pass by coincidence.
fn no_origin_master_repo(label: &str, initial_branch: &str) -> Result<PathBuf, String> {
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    run_git(&root, &["init", "-b", initial_branch])?;
    run_git(&root, &["config", "user.email", "test@test.com"])?;
    run_git(&root, &["config", "user.name", "Test"])?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"no-origin-base\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over(amount: i32) -> bool {\n    amount >= 10\n}\n",
    )
    .map_err(|err| format!("write base lib.rs: {err}"))?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "base"])?;
    run_git(&root, &["checkout", "-b", "work"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over(amount: i32) -> bool {\n    amount > 10\n}\n",
    )
    .map_err(|err| format!("write work lib.rs: {err}"))?;
    run_git(&root, &["commit", "-am", "work"])?;
    Ok(root)
}

fn json_string_at(path: &Path, pointer: &str) -> Result<serde_json::Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))?;
    Ok(value.pointer(pointer).cloned().unwrap_or_default())
}

/// #3952 / RIPR-SPEC-0084: `diff`, `first-pr` and `pr-evidence` resolve an
/// omitted `--base` through the loader's authority, as `check` does. Before,
/// each defaulted to `origin/main`: `diff` failed on git's ambiguous-argument
/// error, `pr-evidence` refused the revision, and `first-pr` exited 0 with
/// `origin/main` recorded in `start-here.json` for a repository that has no
/// `origin`.
#[test]
fn history_commands_resolve_the_default_base_without_origin() -> Result<(), String> {
    let root = no_origin_master_repo("no-origin-default-base", "master")?;
    let root_str = root.to_string_lossy().into_owned();
    let bin = env!("CARGO_BIN_EXE_ripr");
    let mut failures = Vec::new();

    let diff = run_command(bin, None, &["diff", "--root", &root_str, "--json"])
        .map_err(|err| format!("spawn ripr diff: {err}"))?;
    let diff_stdout = String::from_utf8_lossy(&diff.stdout);
    let diff_base = serde_json::from_str::<serde_json::Value>(&diff_stdout)
        .map(|value| value["base"].clone())
        .ok();
    if !diff.status.success() || diff_base != Some(serde_json::json!("master")) {
        failures.push(format!(
            "ripr diff must analyze master...HEAD; status {:?}, stdout:\n{diff_stdout}\nstderr:\n{}",
            diff.status.code(),
            String::from_utf8_lossy(&diff.stderr)
        ));
    }

    // Run from the ripr checkout, which has its own `origin/main`, so a
    // resolver that consulted the working directory instead of `--root`
    // would record the wrong base.
    let first_pr = run_command(bin, None, &["first-pr", "--root", &root_str])
        .map_err(|err| format!("spawn ripr first-pr: {err}"))?;
    match json_string_at(
        &root.join("target/ripr/reports/start-here.json"),
        "/inputs/base",
    ) {
        Ok(base) if base == "master" => {}
        other => failures.push(format!(
            "first-pr must record the resolved base `master`; got {other:?}, status {:?}, stderr:\n{}",
            first_pr.status.code(),
            String::from_utf8_lossy(&first_pr.stderr)
        )),
    }

    let evidence = run_command(bin, Some(&root), &["pr-evidence"])
        .map_err(|err| format!("spawn ripr pr-evidence: {err}"))?;
    match json_string_at(&root.join("target/ripr/pr/repo-exposure.json"), "/base") {
        Ok(base) if base == "master" && evidence.status.success() => {}
        other => failures.push(format!(
            "pr-evidence must record the resolved base `master`; got {other:?}, status {:?}, stderr:\n{}",
            evidence.status.code(),
            String::from_utf8_lossy(&evidence.stderr)
        )),
    }

    ignore_remove_dir_all(&root);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n\n"))
    }
}

/// #3952 fail-closed half: when no default base resolves, `first-pr` and
/// `pr-evidence` name the resolution failure and write no packet, rather
/// than recording a base the run never analyzed.
#[test]
fn history_commands_without_a_resolvable_default_base_fail_named() -> Result<(), String> {
    let root = no_origin_master_repo("no-origin-unresolvable-base", "trunk")?;
    let bin = env!("CARGO_BIN_EXE_ripr");
    let mut failures = Vec::new();
    for (args, packet) in [
        (
            ["first-pr", "--root", "."].as_slice(),
            "target/ripr/reports/start-here.json",
        ),
        (
            ["pr-evidence"].as_slice(),
            "target/ripr/pr/repo-exposure.json",
        ),
    ] {
        let output = run_command(bin, Some(&root), args)
            .map_err(|err| format!("spawn ripr {}: {err}", args[0]))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let packet_written = root.join(packet).exists();
        if output.status.success()
            || packet_written
            || !stderr.contains("could not resolve a default base")
        {
            failures.push(format!(
                "{} with no resolvable base must fail named and write nothing; status {:?}, packet written {packet_written}, stderr:\n{stderr}",
                args[0],
                output.status.code()
            ));
        }
    }
    ignore_remove_dir_all(&root);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n\n"))
    }
}

/// A cloned repository's `ripr.toml` must not choose a program for ripr to
/// run. `[perl].executable = "sh"` plus a `ripr-facts` script in the checkout made
/// `ripr check` (and the LSP sidecar on open/save, and `ripr doctor`) run
/// repository code, because the managed argv starts with `ripr-facts`. The
/// refused runs must leave no marker; the opted-in run must still reach the
/// configured executable, so the gate is the only thing that changed.
#[cfg(unix)]
#[test]
fn repo_config_perl_executable_runs_only_with_user_opt_in() -> Result<(), Box<dyn std::error::Error>>
{
    // Outside the ripr workspace: cargo points TMPDIR at target/, where root
    // discovery would climb to this repository's Cargo.toml.
    let root = unique_external_workspace("perl-executable-opt-in")?;
    init_producer_fixture_repo(&root)?;
    std::fs::write(
        root.join("ripr.toml"),
        "[perl]\nproducer = \"perl-ripr-facts\"\nexecutable = \"sh\"\n",
    )?;
    std::fs::write(root.join("lib.pm"), "package Lib;\n1;\n")?;
    let marker = root.join("repo-code-ran");
    std::fs::write(
        root.join("ripr-facts"),
        format!("touch '{}'\n", marker.display()),
    )?;
    let bin = env!("CARGO_BIN_EXE_ripr");
    let opt_in = "RIPR_ALLOW_REPO_PERL_EXECUTABLE";

    let check = run_command_with_env(bin, &root, &["check", "--base", "HEAD"], &[(opt_in, "0")])?;
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !marker.exists(),
        "ripr check ran repository code without opt-in\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("ignoring [perl].executable `sh` from ripr.toml")
            && stderr.contains(opt_in),
        "refusal must name the ignored executable and the opt-in: {stderr}"
    );

    let doctor = run_command_with_env(bin, &root, &["doctor"], &[(opt_in, "0")])?;
    let stdout = String::from_utf8_lossy(&doctor.stdout);
    assert!(
        !marker.exists(),
        "ripr doctor ran repository code without opt-in\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("ignoring [perl].executable `sh` from ripr.toml (not run)"),
        "doctor must say the executable was not run: {stdout}"
    );

    let trusted = run_command_with_env(bin, &root, &["check", "--base", "HEAD"], &[(opt_in, "1")])?;
    assert!(
        marker.exists(),
        "opted-in check must run the configured executable\nstderr:\n{}",
        String::from_utf8_lossy(&trusted.stderr)
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// #4374: an invalid `RIPR_GIT_TIMEOUT` stops `ripr check` with exit 2 and
/// names the variable, instead of silently keeping the default deadline. An
/// explicit `--git-timeout` still wins, so the bad env value is not read.
#[test]
fn check_rejects_invalid_ripr_git_timeout_env() -> Result<(), String> {
    // Outside the repository, so the precedence run below cannot analyze it.
    let workspace = unique_external_workspace("git-timeout-env")?;
    std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    let root = workspace
        .to_str()
        .ok_or("workspace path is not valid UTF-8")?;

    let invalid = run_ripr_with_env(&["check", "--root", root], &[("RIPR_GIT_TIMEOUT", "abc")]);
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    if invalid.status.code() != Some(2) {
        return Err(format!(
            "expected exit 2 for RIPR_GIT_TIMEOUT=abc, got {:?}; stderr: {stderr}",
            invalid.status.code()
        ));
    }
    if !stderr.contains("RIPR_GIT_TIMEOUT requires a non-negative integer (seconds); got \"abc\"") {
        return Err(format!(
            "stderr does not name the variable and value: {stderr}"
        ));
    }

    let explicit = run_ripr_with_env(
        &["check", "--root", root, "--git-timeout", "5"],
        &[("RIPR_GIT_TIMEOUT", "abc")],
    );
    let stderr = String::from_utf8_lossy(&explicit.stderr);
    if stderr.contains("RIPR_GIT_TIMEOUT") {
        return Err(format!(
            "--git-timeout should take precedence over the env var: {stderr}"
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let non_utf8 = probe_command(env!("CARGO_BIN_EXE_ripr"))
            .args(["check", "--root", root])
            .env(
                "RIPR_GIT_TIMEOUT",
                std::ffi::OsString::from_vec(vec![0xff, b'5']),
            )
            .output()
            .map_err(|e| format!("spawn ripr: {e}"))?;
        let stderr = String::from_utf8_lossy(&non_utf8.stderr);
        if non_utf8.status.code() != Some(2)
            || !stderr.contains("RIPR_GIT_TIMEOUT must be valid UTF-8")
        {
            return Err(format!(
                "non-UTF-8 RIPR_GIT_TIMEOUT should fail closed, got {:?}; stderr: {stderr}",
                non_utf8.status.code()
            ));
        }
    }
    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// A report write cut short by the process dying (here the file-size limit
/// terminates it, the same outcome as Ctrl-C or a cancelled CI step mid-write) must
/// leave the previous complete report in place, not a truncated one (#4542).
#[cfg(unix)]
#[test]
fn interrupted_report_write_keeps_the_previous_complete_report()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("interrupted-report-write");
    std::fs::create_dir_all(&workspace)?;
    let fixture = workspace_root().join("fixtures/boundary_gap");
    let fixture_input = fixture.join("input").to_string_lossy().into_owned();
    let check = run_ripr(&[
        "check",
        "--root",
        &fixture_input,
        "--diff",
        &fixture.join("diff.patch").to_string_lossy(),
        "--format",
        "json",
    ]);
    assert_success(&check);
    let check_path = workspace.join("check.json");
    std::fs::write(&check_path, &check.stdout)?;
    let ledger_path = workspace.join("gap-ledger.json");
    let ledger_args = [
        "reports".to_string(),
        "gap-ledger".to_string(),
        "--check-output".to_string(),
        check_path.to_string_lossy().into_owned(),
        "--root".to_string(),
        fixture_input.clone(),
        "--out".to_string(),
        ledger_path.to_string_lossy().into_owned(),
        "--out-md".to_string(),
        workspace
            .join("gap-ledger.md")
            .to_string_lossy()
            .into_owned(),
    ];
    let ledger_arg_refs: Vec<&str> = ledger_args.iter().map(String::as_str).collect();
    assert_success(&run_ripr(&ledger_arg_refs));
    let complete = std::fs::read(&ledger_path)?;
    assert!(
        complete.len() > 1024,
        "the fixture ledger must exceed the 1 KiB limit below to exercise a cut-short write, got {} bytes",
        complete.len()
    );
    serde_json::from_slice::<serde_json::Value>(&complete)?;

    // `ulimit -f 1` caps any file this process writes at 1 KiB.
    let mut limited = vec![
        "-c",
        "ulimit -f 1 && exec \"$0\" \"$@\"",
        env!("CARGO_BIN_EXE_ripr"),
    ];
    limited.extend(ledger_arg_refs.iter().copied());
    let interrupted = run_command("sh", None, &limited)?;
    assert!(
        !interrupted.status.success(),
        "the size-limited write must not succeed: {interrupted:?}"
    );
    let after = std::fs::read(&ledger_path)?;
    assert_eq!(
        after.len(),
        complete.len(),
        "an interrupted write replaced the previous report with {} bytes",
        after.len()
    );
    assert_eq!(
        after, complete,
        "an interrupted write changed the previous report"
    );
    let _ = std::fs::remove_dir_all(&workspace);
    Ok(())
}

/// Run `ripr` with `stdin` wired to an arbitrary source (for example
/// `/dev/zero`) under a deadline. Before #4480 an unbounded input read never
/// returned, so the deadline turns that hang into a failed assertion instead
/// of a stuck test run.
#[cfg(unix)]
fn run_ripr_with_deadline(
    args: &[&str],
    stdin: Stdio,
    budget: std::time::Duration,
) -> Result<Output, std::io::Error> {
    use std::io::Read as _;
    let mut command = probe_command(env!("CARGO_BIN_EXE_ripr"));
    command
        .args(args)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let deadline = std::time::Instant::now() + budget;
    let mut child = ripr::process_owner::OwnedProcess::spawn(command)?;
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = match pipe {
                Some(mut pipe) => pipe.read_to_end(&mut bytes).map(|_| bytes),
                None => Ok(bytes),
            };
            let _ = tx.send(result);
        });
        rx
    };
    let stdout = drain(
        child
            .stdout_pipe()
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn std::io::Read + Send>),
    );
    let stderr = drain(
        child
            .stderr_pipe()
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn std::io::Read + Send>),
    );
    let receive = |rx: &std::sync::mpsc::Receiver<Result<Vec<u8>, std::io::Error>>| {
        rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))?
    };
    let result = (|| {
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "ripr did not finish before the deadline; an input read is unbounded",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        Ok(Output {
            status,
            stdout: receive(&stdout)?,
            stderr: receive(&stderr)?,
        })
    })();
    child.terminate_tree().map_err(std::io::Error::other)?;
    result
}

/// Budget for a bounded read of an endless input: reading the 256 MiB cap
/// from `/dev/zero` takes well under a second, so a generous deadline still
/// separates "refused at the cap" from "reads forever".
#[cfg(unix)]
const ENDLESS_INPUT_BUDGET: std::time::Duration = std::time::Duration::from_secs(90);

#[cfg(unix)]
fn assert_input_limit_refusal(output: &Output, subject: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{subject} must fail as a usage/input error (exit 2):\n{stderr}"
    );
    assert!(
        stderr.contains("268435456 byte input limit (256 MiB)"),
        "{subject} must name the input limit:\n{stderr}"
    );
}

/// #4480: `ripr check --diff /dev/zero` read forever. The real CLI `check`
/// path must refuse it at the shared input cap with exit 2.
#[cfg(unix)]
#[test]
fn check_diff_from_endless_device_is_refused_at_input_limit() -> Result<(), std::io::Error> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
    let output = run_ripr_with_deadline(
        &[
            "check",
            "--root",
            &root.display().to_string(),
            "--diff",
            "/dev/zero",
            "--json",
        ],
        Stdio::null(),
        ENDLESS_INPUT_BUDGET,
    )?;
    assert_input_limit_refusal(&output, "check --diff /dev/zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read diff file /dev/zero"),
        "the refusal must name the diff path:\n{stderr}"
    );
    Ok(())
}

/// #4480: `--diff -` reads stdin; an endless producer must hit the same cap.
#[cfg(unix)]
#[test]
fn check_diff_stdin_from_endless_stream_is_refused_at_input_limit() -> Result<(), std::io::Error> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
    let output = run_ripr_with_deadline(
        &[
            "check",
            "--root",
            &root.display().to_string(),
            "--diff",
            "-",
            "--json",
        ],
        Stdio::from(std::fs::File::open("/dev/zero")?),
        ENDLESS_INPUT_BUDGET,
    )?;
    assert_input_limit_refusal(&output, "check --diff - < /dev/zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read diff from stdin"),
        "the refusal must name stdin:\n{stderr}"
    );
    Ok(())
}

/// #4480: `ripr outcome --before /dev/zero --after /dev/zero` read forever;
/// the JSON artifact flags share the same bounded reader.
#[cfg(unix)]
#[test]
fn outcome_artifacts_from_endless_device_are_refused_at_input_limit() -> Result<(), std::io::Error>
{
    let output = run_ripr_with_deadline(
        &["outcome", "--before", "/dev/zero", "--after", "/dev/zero"],
        Stdio::null(),
        ENDLESS_INPUT_BUDGET,
    )?;
    assert_input_limit_refusal(&output, "outcome --before /dev/zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("read /dev/zero failed"),
        "the refusal must name the artifact path:\n{stderr}"
    );
    Ok(())
}

/// A cloned repository's `rust-toolchain.toml` can name a toolchain `path`,
/// and `/proc/self/cwd` makes that path point into the checkout, so rustup's
/// `cargo` proxy would run the repository's own program. `ripr doctor` must
/// not spawn the toolchain there.
#[cfg(target_os = "linux")]
#[test]
fn doctor_does_not_run_a_repository_selected_toolchain_path() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let root = unique_temp_workspace("toolchain-path-pin");
    let marker = root.join("repository-toolchain-ran");
    let setup = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(root.join("tc/bin"))?;
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"pinned\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        std::fs::write(root.join("src/lib.rs"), "pub fn f() {}\n")?;
        std::fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\npath = \"/proc/self/cwd/tc\"\n",
        )?;
        for tool in ["cargo", "rustc"] {
            let script = root.join("tc/bin").join(tool);
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\ntouch '{}'\necho '{tool} 9.9.9'\n",
                    marker.display()
                ),
            )?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    })();
    let result = setup
        .map_err(|err| format!("fixture: {err}"))
        .and_then(|()| {
            // The fixture only discriminates where `cargo` is a rustup proxy.
            let proxy = probe_command("cargo")
                .arg("--version")
                .current_dir(&root)
                .env_remove("RUSTUP_TOOLCHAIN")
                .output();
            if !marker.exists() {
                println!("skipped: `cargo` here is not a rustup proxy ({proxy:?})");
                return Ok(());
            }
            std::fs::remove_file(&marker).map_err(|err| format!("reset marker: {err}"))?;
            let output = probe_command(env!("CARGO_BIN_EXE_ripr"))
                .arg("doctor")
                .current_dir(&root)
                .env_remove("RUSTUP_TOOLCHAIN")
                .output()
                .map_err(|err| format!("spawn ripr doctor: {err}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if marker.exists() {
                return Err(format!(
                    "ripr doctor ran the repository's toolchain program: {stdout}"
                ));
            }
            if !stdout.contains("selects a toolchain by `path`") {
                return Err(format!(
                    "doctor must name the refused toolchain file: {stdout}"
                ));
            }
            Ok(())
        });
    ignore_remove_dir_all(&root);
    result
}

/// pnpm fetches and runs the release a project's `packageManager` names, and
/// version managers read project files, so doctor probes language runtimes
/// outside the checkout.
#[cfg(unix)]
#[test]
fn doctor_probes_language_runtimes_outside_the_checkout() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let workspace = unique_temp_workspace("runtime-probe-dir");
    let root = workspace.join("checkout");
    let bin = workspace.join("bin");
    let setup = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::create_dir_all(&bin)?;
        std::fs::write(
            root.join("package.json"),
            "{\"name\":\"x\",\"packageManager\":\"pnpm@8.15.0\"}\n",
        )?;
        std::fs::write(root.join("pnpm-lock.yaml"), "")?;
        std::fs::write(
            root.join("src/a.ts"),
            "export const f = (a: number) => a > 1;\n",
        )?;
        std::fs::write(
            root.join("ripr.toml"),
            "[languages]\nenabled = [\"typescript\"]\n",
        )?;
        let shim = bin.join("pnpm");
        std::fs::write(
            &shim,
            "#!/bin/sh\ncase \"$PWD\" in *checkout*) echo 9.9.9-in-checkout ;; *) echo 9.9.9-isolated ;; esac\n",
        )?;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))?;
        Ok(())
    })();
    let result = setup
        .map_err(|err| format!("fixture: {err}"))
        .and_then(|()| {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut dirs = vec![bin.clone()];
            dirs.extend(std::env::split_paths(&path));
            let path = std::env::join_paths(dirs).map_err(|err| format!("PATH: {err}"))?;
            let output = probe_command(env!("CARGO_BIN_EXE_ripr"))
                .arg("doctor")
                .current_dir(&root)
                .env("PATH", path)
                .output()
                .map_err(|err| format!("spawn ripr doctor: {err}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("9.9.9-isolated") || stdout.contains("in-checkout") {
                return Err(format!(
                    "pnpm must be probed outside the checkout: {stdout}"
                ));
            }
            Ok(())
        });
    ignore_remove_dir_all(&workspace);
    result
}

/// An unusable cache directory fails every file-fact lookup the same way.
/// The build reports that once, not once per source file (#4888).
#[test]
fn unusable_cache_directory_is_reported_once_per_build() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = unique_temp_workspace("unusable-cache-dir");
    std::fs::create_dir_all(&workspace)?;
    let not_a_directory = workspace.join("cache-is-a-file");
    std::fs::write(&not_a_directory, "not a directory\n")?;
    let fixture = workspace_root().join("fixtures/boundary_gap");
    let fixture_input = fixture.join("input").to_string_lossy().into_owned();
    let diff = fixture.join("diff.patch").to_string_lossy().into_owned();
    let output = run_command_with_env(
        env!("CARGO_BIN_EXE_ripr"),
        &workspace,
        &[
            "check",
            "--root",
            &fixture_input,
            "--diff",
            &diff,
            "--format",
            "json",
        ],
        &[("RIPR_CACHE_DIR", &not_a_directory.to_string_lossy())],
    )?;
    assert_success(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let cache_lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("repo file fact cache entr"))
        .collect();
    assert_eq!(
        cache_lines.len(),
        1,
        "expected one cache warning line, got {cache_lines:#?}"
    );
    // The fixture has more than one source file, so a per-file warning
    // would have printed several lines; the one line names the count.
    assert!(
        cache_lines[0].contains("entries ignored; first: ("),
        "the warning must name the count and the first reason: {}",
        cache_lines[0]
    );
    ignore_remove_dir_all(&workspace);
    Ok(())
}

/// docs/EXIT_CODES.md: a reader that closes stdout early gets a quiet exit 2
/// (#4728). The reader closes before ripr writes anything, so the JSON write
/// always meets the closed pipe.
#[test]
fn check_json_into_closed_stdout_exits_two_quietly() -> Result<(), std::io::Error> {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample");
    let mut child = probe_command(env!("CARGO_BIN_EXE_ripr"))
        .args(["check", "--root"])
        .arg(&sample)
        .arg("--diff")
        .arg(sample.join("example.diff"))
        .args(["--format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    drop(child.stdout.take());
    let output = child.wait_with_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        !stderr.contains("stdout failed") && !stderr.contains("Broken pipe"),
        "a closed reader must not produce an error report: {stderr}"
    );
    Ok(())
}

#[test]
fn review_guidance_windows_preserve_output_and_bound_retained_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("review-windows");
    std::fs::create_dir_all(root.join("src"))?;
    init_git_fixture_repo(&root)?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"review_windows\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
    )?;
    let source = (0..40)
        .map(|i| format!("pub fn boundary_{i}(n:i32)->bool {{ n >= 10 }}\n"))
        .collect::<String>();
    std::fs::write(root.join("src/lib.rs"), &source)?;
    run_git(&root, &["add", "."])?;
    run_git(
        &root,
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "baseline",
        ],
    )?;
    std::fs::write(root.join("src/lib.rs"), source.replace(">=", ">"))?;
    run_git(&root, &["add", "."])?;
    run_git(
        &root,
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-m",
            "change boundaries",
        ],
    )?;
    let mut expected = None;
    for width in ["1", "7", "32", "33"] {
        let out = root.join("target").join("window/comments.json");
        let output = run_command_with_env(
            env!("CARGO_BIN_EXE_ripr"),
            &root,
            &[
                "review-comments",
                "--root",
                ".",
                "--base",
                "HEAD^",
                "--head",
                "HEAD",
                "--out",
                &out.to_string_lossy(),
            ],
            &[
                ("RIPR_REVIEW_EVIDENCE_WINDOW_SIZE", width),
                ("RIPR_REPO_EXPOSURE_LATENCY_TRACE", "1"),
            ],
        )?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = (
            std::fs::read(&out)?,
            std::fs::read(out.with_extension("md"))?,
        );
        if let Some(ref expected) = expected {
            assert_eq!(&bytes, expected);
        } else {
            expected = Some(bytes);
        }
        let trace = String::from_utf8(output.stderr)?;
        let mut windows = 0;
        for line in trace
            .lines()
            .filter(|line| line.contains("phase=review_evidence_window"))
        {
            windows += 1;
            let counts = line
                .split("_window_")
                .nth(1)
                .ok_or("missing window trace")?;
            let (actual, rest) = counts
                .split_once("_retained_")
                .ok_or("missing retained trace")?;
            assert!(actual.parse::<usize>()? <= width.parse::<usize>()?);
            let retained = rest
                .split_whitespace()
                .next()
                .ok_or("missing retained count")?
                .parse::<usize>()?;
            assert!(retained <= 10);
        }
        assert!(windows > 1, "fixture must span windows: {trace}");
        let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(
            out.parent()
                .ok_or("output parent")?
                .join("run-receipt.json"),
        )?)?;
        assert_eq!(receipt["status"], "complete");
    }
    // Invalid configuration cannot reuse a preceding complete run as authority.
    let out = root.join("target/window/comments.json");
    for base in ["HEAD^", "HEAD"] {
        let refused = run_command_with_env(
            env!("CARGO_BIN_EXE_ripr"),
            &root,
            &[
                "review-comments",
                "--root",
                ".",
                "--base",
                base,
                "--head",
                "HEAD",
                "--out",
                &out.to_string_lossy(),
            ],
            &[("RIPR_REVIEW_EVIDENCE_WINDOW_SIZE", "0")],
        )?;
        assert!(!refused.status.success());
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("target/window/run-receipt.json"))?)?;
        assert_eq!(receipt["status"], "failed");
        assert!(
            receipt["completed_artifacts"]
                .as_array()
                .ok_or("completed array")?
                .is_empty()
        );
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn review_comments_help_survives_invalid_admission_environment()
-> Result<(), Box<dyn std::error::Error>> {
    for variable in [
        "RIPR_REVIEW_GUIDANCE_MAX_INDEX_FILES",
        "RIPR_REVIEW_GUIDANCE_MAX_PAYLOAD_BYTES",
    ] {
        for help in ["--help", "-h"] {
            let output = run_command_with_env(
                env!("CARGO_BIN_EXE_ripr"),
                &workspace_root(),
                &["review-comments", help],
                &[(variable, "invalid")],
            )?;
            assert!(
                output.status.success(),
                "help must bypass {variable}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout)?;
            assert!(stdout.contains("Usage: ripr review-comments"));
            assert!(stdout.contains(variable));
            assert!(stdout.contains("Default: 1200"));
        }
        let output = run_command_with_env(
            env!("CARGO_BIN_EXE_ripr"),
            &workspace_root(),
            &["review-comments"],
            &[(variable, "invalid")],
        )?;
        assert!(
            !output.status.success(),
            "real dispatch must reject malformed {variable}"
        );
        let stderr = String::from_utf8(output.stderr)?;
        assert!(
            stderr.contains(variable) && stderr.contains("must be a positive integer"),
            "{stderr}"
        );
    }
    Ok(())
}
