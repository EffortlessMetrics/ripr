//! Shared git invocation helper.
//!
//! All git subprocess spawns in the published crate should delegate to
//! [`run_git`], [`run_git_output_with_deadline`], or
//! [`run_git_output_with_deadline_and_limit`] so error formatting stays
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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::process_owner::OwnedProcess;

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

const GIT_TIMEOUT_REPAIR_GUIDANCE: &str = " Repair route: raise or disable the git deadline (0 disables it) — \
     --git-timeout SECS or RIPR_GIT_TIMEOUT=<seconds> for CLI runs, the \
     gitTimeoutMs initialization option for editor sessions — then re-run.";

/// Cause and repair when the git program itself is missing (#4735).
///
/// Shared by the git spawn authority, `ripr check`, and the doctor `tool_git`
/// line so a gitless environment is not handed the raw argv and then told to
/// run a command that also needs git.
pub(crate) const GIT_NOT_FOUND_ON_PATH_MESSAGE: &str =
    "git was not found on PATH; install git, or pass a saved diff with `--diff PATH` / `--diff -`";

/// True when `error` is the named git invocation timeout error (#2303).
/// Matchable in the style of `analysis::cancellation::is_cancellation_error`:
/// require the exact raw tag and its colon delimiter, without wrapper text.
pub(crate) fn is_git_invocation_timeout(error: &str) -> bool {
    error
        .strip_prefix(GIT_INVOCATION_TIMEOUT_PREFIX)
        .is_some_and(|suffix| suffix.starts_with(':'))
}

/// True when `error` is the named missing-git spawn failure (#4735).
pub(crate) fn is_git_not_found_on_path(error: &str) -> bool {
    error == GIT_NOT_FOUND_ON_PATH_MESSAGE
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
    let command = git_command(root, args);
    // `current_dir(root)`, not `git -C <root>`: for a missing/unusable root
    // the spawn itself fails, preserving the established
    // `failed to run git …` error family the context/explain invalid-root
    // contract pins (a `-C` flag would let git report the bad root as a
    // non-zero exit instead, changing the error text). For valid roots the
    // two forms are equivalent.
    collect_output_with_deadline(command, timeout, &describe)
}

/// Longest working directory, in UTF-16 units and without the trailing
/// separator, that `CreateProcessW` accepts (`MAX_PATH` minus the terminator
/// and the separator `SetCurrentDirectoryW` appends). A `longPathAware`
/// manifest does not lift it: on a host with `LongPathsEnabled=1`, a
/// manifested Rust binary and PowerShell 7 both got error 267 for a
/// 361-unit working directory (#4350 probe, 2026-09-29).
const WINDOWS_MAX_WORKING_DIRECTORY_UNITS: usize = 258;

/// Win32 codes `CreateProcessW` returns for a working directory it cannot
/// use because of its length: `ERROR_DIRECTORY` (267, observed on #4350)
/// and `ERROR_FILENAME_EXCED_RANGE` (206). Any other code, such as a
/// missing or denied program, keeps its own message: moving the checkout
/// would not fix it.
const WINDOWS_PATH_LIMIT_ERRORS: [i32; 2] = [267, 206];

/// What a spawn failure message needs from a [`Command`] that the spawn
/// consumes.
struct SpawnSite {
    program: String,
    working_directory: Option<PathBuf>,
}

impl SpawnSite {
    fn of(command: &Command) -> Self {
        Self {
            program: command.get_program().to_string_lossy().into_owned(),
            // Windows resolves a relative working directory against this
            // process's directory, so the limit applies to the joined path.
            working_directory: command
                .get_current_dir()
                .map(|dir| std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf())),
        }
    }

    /// Spawn-failure text for the shared process authority.
    ///
    /// Keeps the `failed to run …` family every caller and contract matches
    /// on. When Windows refuses a working directory past `MAX_PATH` (#4350)
    /// the raw `The directory name is invalid. (os error 267)` names neither
    /// the cause nor a way out, and no other spawn shape helps: Git for
    /// Windows refuses the same root through `-C` (even with
    /// `core.longpaths`), through `GIT_DIR` (`'$GIT_DIR' too big`), and
    /// through a short junction, because it resolves the junction back to the
    /// long root before its work-tree commands (probe table on #4350,
    /// issuecomment-5881212067). So that one case leads with
    /// the limit and the remedy, ahead of the long invocation text that
    /// bounded LSP status messages would otherwise truncate it behind.
    fn failure_message(&self, describe: &str, err: &std::io::Error) -> String {
        self.failure_message_on(cfg!(windows), describe, err)
    }

    fn failure_message_on(&self, is_windows: bool, describe: &str, err: &std::io::Error) -> String {
        let path_limit_error = err
            .raw_os_error()
            .is_some_and(|code| WINDOWS_PATH_LIMIT_ERRORS.contains(&code));
        if let Some(units) = self
            .working_directory
            .as_deref()
            .filter(|_| path_limit_error)
            .and_then(|dir| windows_overlong_working_directory(is_windows, dir))
        {
            return windows_path_limit_message(&self.program, units, err, describe);
        }
        if git_spawn_failed_because_missing_on_path(
            &self.program,
            self.working_directory.as_deref(),
            err,
        ) {
            return GIT_NOT_FOUND_ON_PATH_MESSAGE.to_string();
        }
        format!("failed to run {describe}: {err}")
    }
}

/// The repair for Git's refusal of a repository another user owns
/// (`safe.directory`, #4530), or `None` when `stderr` is not that refusal.
///
/// Git answers every command in such a repository with `detected dubious
/// ownership`, so a caller that reads only the exit status mistakes it for
/// "not a repository" and sends the user somewhere the repository already is.
/// The path comes from Git's own message when present: it is the top-level
/// directory Git wants trusted, which can differ from the analyzed root.
/// `outcome` follows the ownership clause, for callers that must say what
/// did not happen (for example " (the analysis did not run)").
pub(crate) fn dubious_ownership_message(
    root: &Path,
    stderr: &[u8],
    outcome: &str,
) -> Option<String> {
    let stderr = String::from_utf8_lossy(stderr);
    let line = stderr
        .lines()
        .find(|line| line.contains("detected dubious ownership in repository"))?;
    let repository = line
        .split_once(" at '")
        .and_then(|(_, rest)| rest.strip_suffix('\''))
        .map_or_else(|| root.display().to_string(), str::to_string);
    // Only a path made of shell-inert characters is rendered inside a
    // command to paste: a space splits the `--add` value, and `$()` or a
    // quote would run or break in the user's shell (#4606 review). Any other
    // path names the setting instead of a command.
    let repair = if repository
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '\\' | '.' | '_' | '-' | ':'))
    {
        format!("run `git config --global --add safe.directory {repository}`")
    } else {
        "add that exact path to Git's `safe.directory` setting (`git config --global --add \
         safe.directory <path>`, quoted for your shell)"
            .to_string()
    };
    Some(format!(
        "Git refuses the repository at `{repository}` because another user owns it{outcome}. \
         If you trust it, {repair} and retry."
    ))
}

/// `NotFound` is also what a missing working directory produces, so only a
/// missing git program with a usable cwd becomes the PATH diagnosis. A
/// non-git program (the doctor Perl-exporter probe) keeps its own spawn text.
fn git_spawn_failed_because_missing_on_path(
    program: &str,
    working_directory: Option<&Path>,
    err: &std::io::Error,
) -> bool {
    program_is_git(program)
        && err.kind() == std::io::ErrorKind::NotFound
        && working_directory.is_none_or(Path::exists)
}

fn program_is_git(program: &str) -> bool {
    program
        .rsplit(['/', '\\'])
        .next()
        .is_some_and(|name| name == "git" || name == "git.exe")
}

/// The remedy leads so it survives the LSP's 240-character client bound
/// (`lsp::component_outcome::bounded_message`) behind the caller prefixes;
/// the limit and the original error follow for the CLI, which prints all of
/// it.
pub(crate) fn windows_path_limit_message(
    program: &str,
    units: usize,
    err: &std::io::Error,
    describe: &str,
) -> String {
    format!(
        "failed to run {program}: clone or move the repository to a shorter path; the \
         workspace root is {units} characters, over the {WINDOWS_MAX_WORKING_DIRECTORY_UNITS} \
         Windows allows for a working directory (MAX_PATH) ({err}; {describe})"
    )
}

/// Length of `dir` in UTF-16 units when it exceeds the Windows working
/// directory limit. Std strips a verbatim `\\?\` prefix before calling
/// `CreateProcessW`, so the prefix does not count against the limit.
fn windows_overlong_working_directory(is_windows: bool, dir: &Path) -> Option<usize> {
    if !is_windows {
        return None;
    }
    let text = dir.to_string_lossy();
    let spelled = match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(&text).to_string(),
    };
    let units = spelled.trim_end_matches(['\\', '/']).encode_utf16().count();
    (units > WINDOWS_MAX_WORKING_DIRECTORY_UNITS).then_some(units)
}

/// Config every ripr git invocation carries. A repository's own
/// `core.fsmonitor` names a program git runs on index refresh (`status`,
/// worktree `diff`); a clone cannot ship `.git/config`, but an extracted
/// archive or a planted nested repository can.
pub(crate) const UNTRUSTED_REPOSITORY_CONFIG: [&str; 2] = ["-c", "core.fsmonitor=false"];

fn git_command(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args(UNTRUSTED_REPOSITORY_CONFIG)
        .args(args);
    command
}

/// What Git established about a directory that contains a `.git` entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkTreeRootProbe {
    Root,
    InsideWorkTree,
    /// Git could not certify the marker. It still bounds an ancestor walk,
    /// but must never be promoted to a verified Git top level.
    Unverified,
}

/// Verify a candidate root using the shared bounded process authority.
///
/// An empty prefix identifies the work-tree root without decoding or trimming
/// its path. A nonempty prefix proves Git found an enclosing repository, as
/// happens when it ignores an inert nested `.git` directory. Refusals and
/// missing Git remain unverified barriers: neither may widen root discovery.
pub(crate) fn probe_work_tree_root(root: &Path) -> Result<WorkTreeRootProbe, String> {
    let args = ["rev-parse", "--is-inside-work-tree", "--show-prefix"];
    let mut command = git_command(root, &args);
    // A hook or wrapper's repository selectors do not certify this directory.
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR");
    let describe = format!("git root probe in {}", root.display());
    let output = match collect_output_with_deadline_and_limit(
        command,
        Duration::from_secs(5),
        8 * 1024,
        &describe,
    ) {
        Ok(output) => output,
        Err(error) if is_git_not_found_on_path(&error) => return Ok(WorkTreeRootProbe::Unverified),
        Err(error) => return Err(work_tree_root_probe_error(error)),
    };
    if !output.status.success() {
        return if output.status.code().is_some() {
            // Includes invalid gitfiles and Git's dubious-ownership refusal.
            // Without a positive answer, do not cross the candidate marker.
            Ok(WorkTreeRootProbe::Unverified)
        } else {
            Err(format!("{describe} terminated without an exit code"))
        };
    }
    Ok(classify_work_tree_root_stdout(&output.stdout))
}

fn work_tree_root_probe_error(error: String) -> String {
    // This probe has a fixed deadline. Its check/cache callers name their own
    // explicit-root escape hatches; configurable diff-load advice cannot help.
    // Match the raw timeout tag and exact suffix so cleanup failures (which
    // may quote a suppressed timeout) and arbitrary path bytes remain intact.
    if is_git_invocation_timeout(&error)
        && let Some(cause) = error.strip_suffix(GIT_TIMEOUT_REPAIR_GUIDANCE)
    {
        return cause.to_string();
    }
    error
}

fn classify_work_tree_root_stdout(stdout: &[u8]) -> WorkTreeRootProbe {
    let prefix = stdout
        .strip_prefix(b"true\n")
        .and_then(|rest| rest.strip_suffix(b"\n"))
        .or_else(|| {
            stdout
                .strip_prefix(b"true\r\n")
                .and_then(|rest| rest.strip_suffix(b"\r\n"))
        });
    match prefix {
        Some([]) => WorkTreeRootProbe::Root,
        Some(_) => WorkTreeRootProbe::InsideWorkTree,
        None => WorkTreeRootProbe::Unverified,
    }
}

/// [`run_git_output_with_deadline`] with extra environment variables set on
/// the child, for probes that must scope repository discovery (for example
/// `GIT_CEILING_DIRECTORIES`).
pub(crate) fn run_git_output_with_deadline_and_env(
    root: &Path,
    args: &[&str],
    envs: &[(&str, &std::ffi::OsStr)],
    timeout: Option<Duration>,
) -> Result<Output, String> {
    let describe = format!("git -C {} {:?}", root.display(), args);
    let mut command = git_command(root, args);
    for (key, value) in envs {
        command.env(key, value);
    }
    collect_output_with_deadline(command, timeout, &describe)
}

/// Run Git through the shared deadline/process-tree authority while retaining
/// at most `max_output_bytes` from each output stream.
///
/// The reader continues draining after the cap so the child cannot deadlock,
/// but excess bytes are discarded and the invocation fails closed after the
/// child exits. This is intended for repository-inventory consumers where an
/// attacker-controlled path set must not cause unbounded allocation.
pub(crate) fn run_git_output_with_deadline_and_limit(
    root: &Path,
    args: &[&str],
    timeout: Duration,
    max_output_bytes: usize,
) -> Result<Output, String> {
    if max_output_bytes == 0 {
        return Err("git output limit must be greater than zero".to_string());
    }
    let describe = format!("git -C {} {:?}", root.display(), args);
    collect_output_with_deadline_and_limit(
        git_command(root, args),
        timeout,
        max_output_bytes,
        &describe,
    )
}

#[cfg(test)]
pub(crate) fn run_git_output_with_deadline_and_limit_isolated(
    root: &Path,
    args: &[&str],
    timeout: Duration,
    max_output_bytes: usize,
) -> Result<Output, String> {
    if max_output_bytes == 0 {
        return Err("git output limit must be greater than zero".to_string());
    }
    let describe = format!("isolated git -C {} {:?}", root.display(), args);
    let mut command = git_command(root, args);
    let null_config = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command
        .env("GIT_CONFIG_GLOBAL", null_config)
        .env("GIT_CONFIG_SYSTEM", null_config)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    collect_output_with_deadline_and_limit(command, timeout, max_output_bytes, &describe)
}

/// Spawn an arbitrary prepared `command` under the shared deadline,
/// cancellation and bounded-capture contract. Git callers reach it through
/// the wrappers above; the doctor's Perl exporter capability probe uses it
/// directly so an unknown PATH binary can neither hang nor flood the doctor.
///
/// The command is consumed by value: the owned subprocess authority
/// (#3803) takes it over for the Job Object-backed spawn on Windows.
pub(crate) fn collect_output_with_deadline_and_limit(
    command: Command,
    timeout: Duration,
    max_output_bytes: usize,
    describe: &str,
) -> Result<Output, String> {
    collect_output_with_optional_deadline_and_limit(
        command,
        Some(timeout),
        max_output_bytes,
        describe,
    )
}

/// [`run_git_output_with_deadline_and_limit`] for a caller whose deadline is
/// optional: `None` (for example `--git-timeout 0`) waits for Git without a
/// deadline while still bounding captured output.
pub(crate) fn run_git_output_with_optional_deadline_and_limit(
    root: &Path,
    args: &[&str],
    timeout: Option<Duration>,
    max_output_bytes: usize,
) -> Result<Output, String> {
    if max_output_bytes == 0 {
        return Err("git output limit must be greater than zero".to_string());
    }
    let describe = format!("git -C {} {:?}", root.display(), args);
    collect_output_with_optional_deadline_and_limit(
        git_command(root, args),
        timeout,
        max_output_bytes,
        &describe,
    )
}

fn collect_output_with_optional_deadline_and_limit(
    mut command: Command,
    timeout: Option<Duration>,
    max_output_bytes: usize,
    describe: &str,
) -> Result<Output, String> {
    if timeout.is_some_and(|timeout| timeout.is_zero()) {
        return Err(format!(
            "{GIT_INVOCATION_TIMEOUT_PREFIX}: {describe} was given a zero deadline (not spawned)"
        ));
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let spawn_site = SpawnSite::of(&command);
    let mut child =
        OwnedProcess::spawn(command).map_err(|err| spawn_site.failure_message(describe, &err))?;
    let stdout_reader = child
        .stdout_pipe()
        .take()
        .map(|pipe| spawn_bounded_pipe_reader(pipe, max_output_bytes));
    let stderr_reader = child
        .stderr_pipe()
        .take()
        .map(|pipe| spawn_bounded_pipe_reader(pipe, max_output_bytes));

    let wait = poll_child(&mut child, timeout, describe);
    let timed_out = !matches!(&wait, ChildWait::Exited(_));
    let drain_deadline = timed_out.then(|| Instant::now() + POST_KILL_DRAIN_GRACE);
    let stdout_result =
        drain_bounded_pipe_reader(stdout_reader, timed_out, drain_deadline, "stdout", describe);
    let stderr_result =
        drain_bounded_pipe_reader(stderr_reader, timed_out, drain_deadline, "stderr", describe);
    let stdout = stdout_result?;
    let stderr = stderr_result?;

    match wait {
        ChildWait::Exited(_) if stdout.exceeded || stderr.exceeded => Err(format!(
            "git_output_limit_exceeded: {describe} exceeded the {max_output_bytes}-byte per-stream capture limit"
        )),
        ChildWait::Exited(status) => Ok(Output {
            status,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
        }),
        ChildWait::TimedOut(message) | ChildWait::Cancelled(message) => Err(message),
        ChildWait::WaitFailed(err) => Err(format!("failed while waiting on {describe}: {err}")),
        ChildWait::CleanupFailed(message) => Err(message),
    }
}

struct BoundedPipeOutput {
    bytes: Vec<u8>,
    exceeded: bool,
    read_error: Option<String>,
}

type BoundedPipeReader = (
    std::thread::JoinHandle<()>,
    mpsc::Receiver<BoundedPipeOutput>,
);

fn spawn_bounded_pipe_reader(
    mut pipe: impl std::io::Read + Send + 'static,
    max_bytes: usize,
) -> BoundedPipeReader {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut retained = Vec::with_capacity(max_bytes.min(64 * 1024));
        let mut exceeded = false;
        let mut chunk = [0_u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    let remaining = max_bytes.saturating_sub(retained.len());
                    let keep = read.min(remaining);
                    retained.extend_from_slice(&chunk[..keep]);
                    exceeded |= keep < read;
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => {
                    let _ = sender.send(BoundedPipeOutput {
                        bytes: retained,
                        exceeded,
                        read_error: Some(err.to_string()),
                    });
                    return;
                }
            }
        }
        let _ = sender.send(BoundedPipeOutput {
            bytes: retained,
            exceeded,
            read_error: None,
        });
    });
    (handle, receiver)
}

fn drain_bounded_pipe_reader(
    reader: Option<BoundedPipeReader>,
    timed_out: bool,
    deadline: Option<Instant>,
    stream_name: &str,
    describe: &str,
) -> Result<BoundedPipeOutput, String> {
    let Some((handle, receiver)) = reader else {
        return Ok(BoundedPipeOutput {
            bytes: Vec::new(),
            exceeded: false,
            read_error: None,
        });
    };
    let received = match deadline {
        Some(deadline) => receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())),
        None => receiver
            .recv()
            .map_err(|_receive_error| mpsc::RecvTimeoutError::Disconnected),
    };
    match received {
        Ok(output) => {
            handle.join().map_err(|_panic_payload| {
                format!("{stream_name} pipe reader panicked while collecting {describe}")
            })?;
            if let Some(error) = &output.read_error {
                return Err(format!(
                    "failed while reading {stream_name} from {describe}: {error}"
                ));
            }
            Ok(output)
        }
        Err(mpsc::RecvTimeoutError::Timeout) if timed_out => Ok(BoundedPipeOutput {
            bytes: Vec::new(),
            exceeded: false,
            read_error: None,
        }),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "{stream_name} pipe did not drain after {describe} completed"
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "{stream_name} pipe reader failed while collecting {describe}"
        )),
    }
}

/// #5015: one streaming `git cat-file --batch` session for candidate-tree
/// materialization, replacing the previous per-blob
/// `git cat-file blob <oid>` spawn loop (one sequential git process per file,
/// each carrying the full per-invocation deadline).
///
/// The session is a lockstep request/response protocol: the caller queues one
/// object ID on stdin (`request_blob`), reads the response header, then
/// streams the announced byte count to its own destination through
/// [`CatFileBatch::read_blob_bytes`] in bounded chunks. Because each response
/// is fully consumed before the next request is queued, a large blob can
/// never deadlock against an unread pipe: git blocks writing the response
/// exactly while the caller reads it.
///
/// The whole session — every queued blob together — is bounded by ONE
/// overall deadline (`budget`), enforced incrementally on every blocking
/// read through the same named `git_invocation_timeout` classification the
/// polling collector uses. A hung git therefore costs at most one deadline,
/// never one deadline per blob. Blob bytes are never buffered whole: the
/// stdout reader feeds a bounded queue (backpressure), so a slow destination
/// stalls git through the pipe instead of accumulating chunks in memory.
///
/// Fail-closed contract, matching the per-blob path it replaces: a missing
/// object reports `Ok(None)`; a malformed header, a truncated stream, a
/// failed write, deadline expiry, or cooperative cancellation all terminate
/// the owned process tree and return a named error.
pub(crate) struct CatFileBatch {
    child: OwnedProcess,
    stdin: std::process::ChildStdin,
    stdout: CatFileBatchStream,
    stderr: Option<BoundedPipeReader>,
    started: Instant,
    budget: Duration,
    describe: String,
}

/// Reader-thread chunk size for the batch stdout stream. Bounds resident
/// memory per in-flight read regardless of blob size.
const CAT_FILE_BATCH_CHUNK_BYTES: usize = 64 * 1024;

/// Hard cap on one `--batch` response header line (sha256 object ID + type +
/// size comfortably fit); a longer line means a corrupt or adversarial
/// stream, not a real header.
const CAT_FILE_BATCH_HEADER_LINE_BYTES: usize = 4096;

/// Captured stderr bound for the batch session. Error text only; a verbose
/// child cannot deadlock the session through an unread pipe because the
/// reader keeps draining after the cap and discards the excess.
const CAT_FILE_BATCH_STDERR_BYTES: usize = 8 * 1024;

impl CatFileBatch {
    /// Spawn `git cat-file --batch` in `root`. `budget` is the single
    /// overall deadline for the whole session, enforced incrementally.
    pub(crate) fn spawn(root: &Path, budget: Duration) -> Result<Self, String> {
        let describe = format!("git -C {} cat-file --batch", root.display());
        let mut command = git_command(root, &["cat-file", "--batch"]);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let spawn_site = SpawnSite::of(&command);
        let mut child = OwnedProcess::spawn(command)
            .map_err(|err| spawn_site.failure_message(&describe, &err))?;
        let stdin = child
            .stdin_pipe()
            .take()
            .ok_or_else(|| format!("failed to open the stdin pipe for {describe}"))?;
        let stdout_pipe = child
            .stdout_pipe()
            .take()
            .ok_or_else(|| format!("failed to open the stdout pipe for {describe}"))?;
        let stderr = child
            .stderr_pipe()
            .take()
            .map(|pipe| spawn_bounded_pipe_reader(pipe, CAT_FILE_BATCH_STDERR_BYTES));
        Ok(Self {
            child,
            stdin,
            stdout: CatFileBatchStream::new(spawn_cat_file_batch_chunk_reader(stdout_pipe)),
            stderr,
            started: Instant::now(),
            budget,
            describe,
        })
    }

    /// Budget left for the whole session; zero means the next blocking wait
    /// classifies as a timeout.
    fn remaining(&self) -> Duration {
        self.budget
            .checked_sub(self.started.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// The absolute instant the overall budget runs out. Every blocking
    /// stream wait recomputes its allowance from this, so dribbling
    /// fragments cannot outlive the budget by resetting per-wait timeouts.
    fn deadline(&self) -> Instant {
        self.started + self.budget
    }

    /// Terminate the owned tree for an abortive outcome and keep the
    /// terminate-then-classify contract (`ChildWait` docs): a failed
    /// termination is reported as an incomplete cleanup naming the
    /// suppressed outcome, never as the outcome itself.
    fn abort(&mut self, trigger: &str, pending: String) -> String {
        match self.child.terminate_tree() {
            Ok(()) => pending,
            Err(cleanup) => format!(
                "{trigger} of {} did not complete tree cleanup; the child may still be \
                 running: {cleanup} (suppressed wait outcome: {pending})",
                self.describe
            ),
        }
    }

    /// Map a stream-read failure onto the session error text: a read that
    /// outlived the remaining budget becomes the named
    /// `git_invocation_timeout` classification (raw prefix preserved so
    /// `is_git_invocation_timeout` keeps matching), anything else is the
    /// stream's own message.
    fn classify_read_error(&mut self, error: CatFileBatchReadError) -> String {
        match error {
            CatFileBatchReadError::TimedOut => {
                let message =
                    git_invocation_timeout_message(&self.describe, self.budget.as_millis());
                self.abort("timeout", message)
            }
            CatFileBatchReadError::Failed(message) => self.abort("stream failure", message),
        }
    }

    /// Queue one blob request and read its response header. Returns the
    /// announced blob size in bytes, or `None` when git reports the object
    /// missing (the caller fails closed naming the identity).
    pub(crate) fn request_blob(&mut self, object: &str) -> Result<Option<u64>, String> {
        if let Err(cancelled) = crate::analysis::cancellation::checkpoint() {
            return Err(self.abort("cancellation", cancelled));
        }
        let write_result = self
            .stdin
            .write_all(object.as_bytes())
            .and_then(|()| self.stdin.write_all(b"\n"))
            .and_then(|()| self.stdin.flush());
        if let Err(err) = write_result {
            let message = format!("failed writing to {}: {err}", self.describe);
            return Err(self.abort("write failure", message));
        }
        let deadline = self.deadline();
        let header = self.stdout.read_line(deadline);
        let header = header.map_err(|error| self.classify_read_error(error))?;
        let header = String::from_utf8_lossy(&header);
        let mut fields = header.split(' ');
        let _echoed_object = fields.next().unwrap_or_default();
        match fields.next().unwrap_or_default() {
            "missing" => Ok(None),
            "blob" => {
                let size = fields
                    .next()
                    .and_then(|size| size.parse::<u64>().ok())
                    .ok_or_else(|| {
                        let message = format!(
                            "malformed git cat-file --batch response header for {object}: \
                             `{header}`"
                        );
                        self.abort("malformed stream", message)
                    })?;
                Ok(Some(size))
            }
            other => {
                let message = format!(
                    "unexpected git cat-file --batch response kind `{other}` for {object}: \
                     expected a blob"
                );
                Err(self.abort("malformed stream", message))
            }
        }
    }

    /// Read exactly `buf.len()` bytes of the current blob's content. Chunks
    /// of `buf` sized by the caller bound resident memory; the overall
    /// session deadline is enforced between chunk reads.
    pub(crate) fn read_blob_bytes(&mut self, buf: &mut [u8]) -> Result<(), String> {
        if let Err(cancelled) = crate::analysis::cancellation::checkpoint() {
            return Err(self.abort("cancellation", cancelled));
        }
        let deadline = self.deadline();
        let read = self.stdout.read_exact(buf, deadline);
        read.map_err(|error| self.classify_read_error(error))
    }

    /// Consume one blob's trailing newline and fail closed on a corrupt
    /// stream framing.
    pub(crate) fn end_blob(&mut self) -> Result<(), String> {
        let mut newline = [0_u8; 1];
        let deadline = self.deadline();
        let read = self.stdout.read_exact(&mut newline, deadline);
        read.map_err(|error| self.classify_read_error(error))?;
        if newline[0] != b'\n' {
            let message = "malformed git cat-file --batch stream: blob not terminated by a newline"
                .to_string();
            return Err(self.abort("malformed stream", message));
        }
        Ok(())
    }

    /// Close stdin (EOF tells git no more requests are coming) and wait for
    /// the child to exit under whatever budget remains. Surfaces the bounded
    /// stderr capture when the child exits non-zero.
    pub(crate) fn finish(mut self) -> Result<(), String> {
        // Budget first: dropping stdin partially moves `self`, after which
        // no whole-`self` method may run — both values are computed here and
        // only field accesses remain below.
        let remaining = self.remaining();
        let deadline = self.deadline();
        drop(self.stdin);
        let wait = poll_child(&mut self.child, Some(remaining), &self.describe);
        let timed_out = !matches!(&wait, ChildWait::Exited(_));
        let drain_deadline = timed_out.then(|| Instant::now() + POST_KILL_DRAIN_GRACE);
        let stderr = drain_bounded_pipe_reader(
            self.stderr.take(),
            timed_out,
            drain_deadline,
            "stderr",
            &self.describe,
        );
        match wait {
            ChildWait::Exited(status) if status.success() => {
                // A child that exited before the deadline but was reaped
                // after it has still overrun the budget: accept no success
                // past the overall deadline. Field accesses only — stdin is
                // already dropped, so no whole-`self` method may run.
                if Instant::now() >= deadline {
                    let message =
                        git_invocation_timeout_message(&self.describe, self.budget.as_millis());
                    return Err(match self.child.terminate_tree() {
                        Ok(()) => message,
                        Err(cleanup) => format!(
                            "timeout of {} did not complete tree cleanup; the child may still \
                             be running: {cleanup} (suppressed wait outcome: {message})",
                            self.describe
                        ),
                    });
                }
                Ok(())
            }
            ChildWait::Exited(status) => {
                let detail = stderr
                    .map(|output| String::from_utf8_lossy(&output.bytes).trim().to_string())
                    .unwrap_or_default();
                if detail.is_empty() {
                    Err(format!("git cat-file --batch exited with {status}"))
                } else {
                    Err(format!(
                        "git cat-file --batch exited with {status}: {detail}"
                    ))
                }
            }
            ChildWait::TimedOut(message) | ChildWait::Cancelled(message) => Err(message),
            ChildWait::WaitFailed(err) => {
                Err(format!("failed while waiting on {}: {err}", self.describe))
            }
            ChildWait::CleanupFailed(message) => Err(message),
        }
    }
}

/// Read failure classification for the batch stream, kept distinct so the
/// session can map a deadline expiry onto the named timeout while passing
/// stream errors through unchanged.
#[derive(Debug)]
enum CatFileBatchReadError {
    TimedOut,
    Failed(String),
}

/// Incremental reader over the chunk channel fed by the stdout reader
/// thread. At most one chunk is resident; clean EOF (the reader thread
/// finished) is `Ok(false)` from `next_chunk`, a read error or a deadline
/// expiry is `Err`.
struct CatFileBatchStream {
    receiver: mpsc::Receiver<Result<Vec<u8>, String>>,
    current: Vec<u8>,
    offset: usize,
}

impl CatFileBatchStream {
    fn new(receiver: mpsc::Receiver<Result<Vec<u8>, String>>) -> Self {
        Self {
            receiver,
            current: Vec::new(),
            offset: 0,
        }
    }

    /// Pull the next chunk when the current one is exhausted.
    /// `Ok(false)` is end of stream. The wait allowance is recomputed from
    /// the absolute `deadline` on every chunk so a stream dribbling
    /// fragments just under each wait can never outlive the overall budget.
    /// Bytes buffered or queued before the deadline are still rejected once
    /// it has passed: consumption after the budget is a timeout, not
    /// success.
    fn next_chunk(&mut self, deadline: Instant) -> Result<bool, CatFileBatchReadError> {
        if Instant::now() >= deadline {
            return Err(CatFileBatchReadError::TimedOut);
        }
        if self.offset < self.current.len() {
            return Ok(true);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match self.receiver.recv_timeout(remaining) {
            Ok(Ok(chunk)) => {
                self.current = chunk;
                self.offset = 0;
                Ok(true)
            }
            Ok(Err(read_error)) => Err(CatFileBatchReadError::Failed(read_error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(CatFileBatchReadError::TimedOut),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    /// Read one `\n`-terminated line, returned without the terminator.
    fn read_line(&mut self, deadline: Instant) -> Result<Vec<u8>, CatFileBatchReadError> {
        let mut line = Vec::new();
        loop {
            if Instant::now() >= deadline {
                return Err(CatFileBatchReadError::TimedOut);
            }
            if let Some(position) = self.current[self.offset..]
                .iter()
                .position(|byte| *byte == b'\n')
            {
                line.extend_from_slice(&self.current[self.offset..self.offset + position]);
                self.offset += position + 1;
                return Ok(line);
            }
            line.extend_from_slice(&self.current[self.offset..]);
            self.offset = self.current.len();
            if line.len() > CAT_FILE_BATCH_HEADER_LINE_BYTES {
                return Err(CatFileBatchReadError::Failed(format!(
                    "git cat-file --batch response header exceeded the \
                     {CAT_FILE_BATCH_HEADER_LINE_BYTES}-byte line limit"
                )));
            }
            if !self.next_chunk(deadline)? {
                return Err(CatFileBatchReadError::Failed(
                    "git cat-file --batch stream ended in the middle of a response header"
                        .to_string(),
                ));
            }
        }
    }

    /// Read exactly `buf.len()` bytes.
    fn read_exact(
        &mut self,
        buf: &mut [u8],
        deadline: Instant,
    ) -> Result<(), CatFileBatchReadError> {
        let mut filled = 0;
        while filled < buf.len() {
            if Instant::now() >= deadline {
                return Err(CatFileBatchReadError::TimedOut);
            }
            if self.offset >= self.current.len() && !self.next_chunk(deadline)? {
                return Err(CatFileBatchReadError::Failed(
                    "git cat-file --batch stream ended in the middle of a blob".to_string(),
                ));
            }
            let available = self.current.len() - self.offset;
            let take = available.min(buf.len() - filled);
            buf[filled..filled + take]
                .copy_from_slice(&self.current[self.offset..self.offset + take]);
            self.offset += take;
            filled += take;
        }
        Ok(())
    }
}

/// Backpressure: at most this many stdout chunks may be queued between the
/// reader thread and the consumer. A slow destination therefore stalls the
/// reader, which stalls git's pipe, which stalls git — memory stays bounded
/// (chunks × 64 KiB) no matter how large the tree or how slow the disk.
const CAT_FILE_BATCH_QUEUED_CHUNKS: usize = 8;

/// Drain the batch child's stdout in `CAT_FILE_BATCH_CHUNK_BYTES` chunks on a
/// helper thread, exactly like [`spawn_bounded_pipe_reader`] but retaining
/// every byte for the streaming parser (the caller, not this reader, owns the
/// overall byte budget). The bounded channel provides backpressure: the send
/// blocks while the consumer is busy, and a failed send means the consumer is
/// gone, so the reader ends either way; dropping its handle detaches it.
fn spawn_cat_file_batch_chunk_reader(
    mut pipe: impl std::io::Read + Send + 'static,
) -> mpsc::Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = mpsc::sync_channel(CAT_FILE_BATCH_QUEUED_CHUNKS);
    std::thread::spawn(move || {
        let mut chunk = [0_u8; CAT_FILE_BATCH_CHUNK_BYTES];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    if sender.send(Ok(chunk[..read].to_vec())).is_err() {
                        break;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => {
                    let _ = sender.send(Err(format!(
                        "failed while reading git cat-file --batch stdout: {err}"
                    )));
                    break;
                }
            }
        }
    });
    receiver
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
    mut command: Command,
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
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let spawn_site = SpawnSite::of(&command);
    let mut child =
        OwnedProcess::spawn(command).map_err(|err| spawn_site.failure_message(describe, &err))?;
    let stdout_reader = child.stdout_pipe().take().map(spawn_pipe_reader);
    let stderr_reader = child.stderr_pipe().take().map(spawn_pipe_reader);

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
        ChildWait::CleanupFailed(message) => Err(message),
    }
}

/// Outcome of the shared deadline-aware child wait (#2303). In every
/// non-`Exited` arm other than `CleanupFailed` the child has already been
/// terminated and reaped, so no orphan process holds a handle. `WaitFailed`
/// carries the raw wait error so each caller wraps it in its own
/// established message text. `CleanupFailed` is the contract-keeping
/// exception: the wait ended abnormally AND the terminate-and-reap could
/// not be completed or confirmed, so the child or its tree may still be
/// alive; the payload names the incomplete cleanup and the suppressed wait
/// outcome instead of implying that termination completed.
pub(crate) enum ChildWait {
    Exited(std::process::ExitStatus),
    TimedOut(String),
    Cancelled(String),
    WaitFailed(String),
    CleanupFailed(String),
}

impl ChildWait {
    /// Short summary of this outcome for cleanup-failure context: a
    /// suppressed arm is reported inside `CleanupFailed` so a caller can
    /// still see which wait outcome the failed cleanup replaced.
    fn summary(&self) -> String {
        match self {
            Self::Exited(status) => format!("child exited with {status}"),
            Self::TimedOut(message)
            | Self::Cancelled(message)
            | Self::WaitFailed(message)
            | Self::CleanupFailed(message) => message.clone(),
        }
    }
}

/// The named, matchable timeout classification shared by the polling
/// collector and the streaming batch session (#2303, #5015). Kept as one
/// constructor so both paths emit byte-identical repair guidance.
fn git_invocation_timeout_message(describe: &str, timeout_ms: u128) -> String {
    format!(
        "{GIT_INVOCATION_TIMEOUT_PREFIX}: {describe} exceeded the {timeout_ms}ms deadline \
         (process terminated).{GIT_TIMEOUT_REPAIR_GUIDANCE}"
    )
}

/// Poll `child` with `try_wait` on a short interval up to the optional
/// deadline, checking cooperative analysis cancellation each tick so a hung
/// child honors an LSP refresh supersede (#2303). Lifted from the Perl
/// facts exporter wait in `app::check` (pre-#2303 `ChildWaitTimeoutExt`)
/// and shared by both call families.
///
/// `child` is the shared owned-subprocess authority (#3803): on Windows a
/// non-`Exited` arm terminates the whole Job Object tree and reaps the
/// direct child before returning; on other platforms the direct-child
/// kill/reap behavior is unchanged. A failed termination is never folded
/// into a `Cancelled`/`TimedOut`/`WaitFailed` arm — the contract that every
/// such arm already terminated and reaped stays true because the caller
/// instead receives [`ChildWait::CleanupFailed`].
pub(crate) fn poll_child(
    child: &mut OwnedProcess,
    timeout: Option<Duration>,
    describe: &str,
) -> ChildWait {
    let deadline = timeout.map(|limit| Instant::now() + limit);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return ChildWait::Exited(status),
            Ok(None) => {
                if let Err(cancelled) = crate::analysis::cancellation::checkpoint() {
                    return terminate_then_classify(
                        describe,
                        "cancellation",
                        ChildWait::Cancelled(cancelled),
                        || child.terminate_tree(),
                    );
                }
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    let timeout_ms = timeout.map_or(0, |limit| limit.as_millis());
                    return terminate_then_classify(
                        describe,
                        "timeout",
                        ChildWait::TimedOut(git_invocation_timeout_message(describe, timeout_ms)),
                        || child.terminate_tree(),
                    );
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => {
                return terminate_then_classify(
                    describe,
                    "wait failure",
                    ChildWait::WaitFailed(err.to_string()),
                    || child.terminate_tree(),
                );
            }
        }
    }
}

/// Terminate the owned tree and return the pending non-`Exited` arm only
/// when the terminate-and-reap completed. A failed termination keeps the
/// [`ChildWait`] contract true by surfacing
/// [`ChildWait::CleanupFailed`] — which records the suppressed outcome —
/// instead of an arm that would imply cleanup succeeded. `terminate` is
/// injected so the classification is provable without a live process.
fn terminate_then_classify(
    describe: &str,
    trigger: &str,
    pending: ChildWait,
    terminate: impl FnOnce() -> Result<(), String>,
) -> ChildWait {
    match terminate() {
        Ok(()) => pending,
        Err(cleanup) => ChildWait::CleanupFailed(format!(
            "{trigger} of {describe} did not complete tree cleanup; the child may still be \
             running: {cleanup} (suppressed wait outcome: {})",
            pending.summary()
        )),
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::analysis::cancellation::{
        AnalysisAbortKind, AnalysisCancellationToken, is_cancellation_error, with_token,
    };
    use serial_test::serial;

    #[test]
    fn work_tree_root_timeout_omits_inapplicable_deadline_guidance() {
        let shared = git_invocation_timeout_message("git root probe in /fixture", 5_000);
        let projected = work_tree_root_probe_error(shared.clone());
        assert!(is_git_invocation_timeout(&projected));
        assert!(projected.contains("exceeded the 5000ms deadline (process terminated)"));
        for ineffective in ["--git-timeout", "RIPR_GIT_TIMEOUT", "gitTimeoutMs"] {
            assert!(shared.contains(ineffective));
            assert!(!projected.contains(ineffective), "{projected}");
        }
        for preserved in [
            "analysis_cancelled: superseded root probe".to_string(),
            "git output exceeded the 8192 byte limit".to_string(),
            "failed to run git: Permission denied".to_string(),
            format!("timeout did not complete tree cleanup (suppressed wait outcome: {shared})"),
        ] {
            assert_eq!(work_tree_root_probe_error(preserved.clone()), preserved);
        }
    }

    #[test]
    fn work_tree_root_probe_preserves_exact_prefix_bytes() {
        for (stdout, expected) in [
            (b"true\n\n".as_slice(), WorkTreeRootProbe::Root),
            (b"true\r\n\r\n".as_slice(), WorkTreeRootProbe::Root),
            (
                b"true\nsrc/\n".as_slice(),
                WorkTreeRootProbe::InsideWorkTree,
            ),
            (
                b"true\n \n/\n".as_slice(),
                WorkTreeRootProbe::InsideWorkTree,
            ),
            (
                b"true\n\xff/\n".as_slice(),
                WorkTreeRootProbe::InsideWorkTree,
            ),
            (
                b"true\r\nsrc/\r\n".as_slice(),
                WorkTreeRootProbe::InsideWorkTree,
            ),
            (b"true\n".as_slice(), WorkTreeRootProbe::Unverified),
            (b"false\n\n".as_slice(), WorkTreeRootProbe::Unverified),
            (b"unexpected\n\n".as_slice(), WorkTreeRootProbe::Unverified),
            (b"".as_slice(), WorkTreeRootProbe::Unverified),
        ] {
            assert_eq!(
                classify_work_tree_root_stdout(stdout),
                expected,
                "{stdout:?}"
            );
        }
    }

    #[test]
    fn dubious_ownership_names_the_safe_directory_repair() -> Result<(), String> {
        // #4530: Git's own refusal text, as 2.43 prints it.
        let stderr = b"fatal: detected dubious ownership in repository at '/srv/repo'\n\
To add an exception for this directory, call:\n\n\tgit config --global --add safe.directory /srv/repo\n";
        let message =
            dubious_ownership_message(Path::new("sub"), stderr, " (the analysis did not run)")
                .ok_or("the refusal must be recognized")?;
        if !message.starts_with(
            "Git refuses the repository at `/srv/repo` because another user owns it (the analysis did not run).",
        ) || !message.contains("git config --global --add safe.directory /srv/repo")
        {
            return Err(format!("expected Git's path and the repair, got {message}"));
        }
        // A path with shell syntax is never rendered inside a pasteable
        // command (#4606 review): a space splits the value and `$()` runs.
        for path in ["/tmp/ripr repo", "/tmp/$(touch x)", "/tmp/it's"] {
            let stderr = format!("fatal: detected dubious ownership in repository at '{path}'\n");
            let message = dubious_ownership_message(Path::new("sub"), stderr.as_bytes(), "")
                .ok_or("the refusal must be recognized")?;
            if message.contains(&format!("safe.directory {path}"))
                || !message.contains(&format!("repository at `{path}`"))
                || !message.contains("quoted for your shell")
            {
                return Err(format!("unsafe path must not be pasteable: {message}"));
            }
        }
        if dubious_ownership_message(
            Path::new("sub"),
            b"fatal: not a git repository (or any of the parent directories): .git\n",
            "",
        )
        .is_some()
        {
            return Err("an ordinary non-repository must not read as an ownership refusal".into());
        }
        Ok(())
    }

    #[test]
    fn git_timeout_tag_requires_exact_raw_error_prefix() {
        for error in [
            "git_invocation_timeout: git command exceeded its deadline",
            "git_invocation_timeout:	git command exceeded its deadline",
            "git_invocation_timeout:\ngit command exceeded its deadline",
        ] {
            assert!(is_git_invocation_timeout(error), "missed {error:?}");
        }
        for error in [
            "git_invocation_timeout",
            "git_invocation_timeoutness: unrelated failure",
            "git_invocation_timeout_metadata: unrelated failure",
            "git_invocation_timeout : invalid delimiter",
            "git_invocation_timeout\n: invalid delimiter",
            "git_invocation_timeout\r\n: invalid delimiter",
            " git_invocation_timeout: not raw",
            "\ngit_invocation_timeout: not raw",
            "ripr: git_invocation_timeout: wrapped",
            "outer: git_invocation_timeout: wrapped",
            "diff_scope_oversized: a different guard",
            "repo_scope_oversized: a different guard",
            "review_guidance_oversized: a different guard",
            "analysis cancelled: DeadlineExceeded",
        ] {
            assert!(!is_git_invocation_timeout(error), "misclassified {error:?}");
        }
    }

    /// Drive letter kept apart from its separator so the local-context gate
    /// does not read these synthetic roots as a committed machine path.
    const DRIVE: &str = "D:";

    fn windows_dir_of_units(units: usize) -> String {
        let prefix = format!(r"{DRIVE}\a\");
        format!("{prefix}{}", "x".repeat(units - prefix.len()))
    }

    /// A repository's `core.fsmonitor` names a program git runs on index
    /// refresh. ripr's git calls must not run it.
    #[cfg(unix)]
    #[test]
    fn repository_fsmonitor_program_does_not_run() -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let root =
            std::env::temp_dir().join(format!("ripr-git-fsmonitor-{}-{stamp}", std::process::id()));
        let marker = root.join("fsmonitor-ran");
        let result = (|| {
            std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
            let hook = format!("touch '{}'", marker.display());
            for args in [
                vec!["init", "-q"],
                vec!["config", "core.fsmonitor", hook.as_str()],
            ] {
                let output = Command::new("git")
                    .args(&args)
                    .current_dir(&root)
                    .env_remove("GIT_DIR")
                    .env_remove("GIT_WORK_TREE")
                    .output()
                    .map_err(|err| format!("git {args:?}: {err}"))?;
                if !output.status.success() {
                    return Err(format!("git {args:?} failed: {output:?}"));
                }
            }
            std::fs::write(root.join("lib.rs"), "fn a() {}\n")
                .map_err(|err| format!("write: {err}"))?;
            run_git(&root, &["add", "lib.rs"])?;
            run_git(&root, &["status", "--porcelain"])?;
            Ok(marker.exists())
        })();
        let _ = std::fs::remove_dir_all(&root);
        if result? {
            return Err("git ran the repository's core.fsmonitor program".to_string());
        }
        Ok(())
    }

    #[test]
    fn overlong_working_directory_is_measured_only_on_windows_past_max_path() {
        let limit = WINDOWS_MAX_WORKING_DIRECTORY_UNITS;
        let at_limit = windows_dir_of_units(limit);
        let past_limit = windows_dir_of_units(limit + 1);
        assert_eq!(
            windows_overlong_working_directory(true, Path::new(&at_limit)),
            None
        );
        assert_eq!(
            windows_overlong_working_directory(true, Path::new(&past_limit)),
            Some(limit + 1)
        );
        assert_eq!(
            windows_overlong_working_directory(false, Path::new(&past_limit)),
            None,
            "no other platform has the MAX_PATH working-directory limit"
        );
    }

    #[test]
    fn overlong_working_directory_ignores_verbatim_prefix_and_trailing_separator() {
        let at_limit = windows_dir_of_units(WINDOWS_MAX_WORKING_DIRECTORY_UNITS);
        for spelling in [
            format!(r"\\?\{at_limit}"),
            format!(r"{at_limit}\"),
            format!(r"\\?\{at_limit}\"),
        ] {
            assert_eq!(
                windows_overlong_working_directory(true, Path::new(&spelling)),
                None,
                "{spelling} is what CreateProcessW receives as {at_limit}"
            );
        }
        let share = format!(r"\\?\UNC\server\{}", "s".repeat(250));
        assert_eq!(
            windows_overlong_working_directory(true, Path::new(&share)),
            Some(r"\\server\".len() + 250)
        );
    }

    #[test]
    fn overlong_working_directory_counts_utf16_units_not_bytes() {
        // U+1D11E is four UTF-8 bytes and two UTF-16 units; Windows limits
        // the latter.
        let base = windows_dir_of_units(WINDOWS_MAX_WORKING_DIRECTORY_UNITS - 2);
        let with_clef = format!("{base}\u{1D11E}");
        assert_eq!(
            windows_overlong_working_directory(true, Path::new(&with_clef)),
            None
        );
        let past = format!("{with_clef}x");
        assert_eq!(
            windows_overlong_working_directory(true, Path::new(&past)),
            Some(WINDOWS_MAX_WORKING_DIRECTORY_UNITS + 1)
        );
    }

    /// Native control for #4350: the shared git authority, spawning under a
    /// real directory past `MAX_PATH`, reports the limit and the remedy
    /// instead of `The directory name is invalid. (os error 267)`.
    /// `CreateProcessW` refuses the working directory whatever the host's
    /// `LongPathsEnabled` policy or the binary's manifest says; if Windows or
    /// std ever lifts that, this fails and names the change.
    #[cfg(windows)]
    #[test]
    fn native_git_spawn_under_an_overlong_root_names_the_path_limit() -> Result<(), String> {
        // One test per process uses this name, so the pid alone is unique.
        let short = std::env::temp_dir().join(format!("ripr-4350-{}", std::process::id()));
        let mut long = short.clone();
        while long.as_os_str().len() <= WINDOWS_MAX_WORKING_DIRECTORY_UNITS + 20 {
            long.push("long-path-segment-0123456789abcdef");
        }
        // Std prefixes `\\?\` for filesystem calls, so creating the tree works
        // even though spawning into it does not.
        std::fs::create_dir_all(&long).map_err(|err| format!("create {long:?}: {err}"))?;
        let result = run_git_output_with_deadline(&long, &["--version"], None);
        let _ = std::fs::remove_dir_all(&short);
        match result {
            Err(message) => {
                assert!(
                    message.starts_with(
                        "failed to run git: clone or move the repository to a shorter path; the \
                         workspace root is "
                    ) && message.contains("(MAX_PATH)"),
                    "{message}"
                );
                Ok(())
            }
            Ok(output) => Err(format!(
                "git spawned under a {}-unit root ({:?}); the MAX_PATH premise of #4350 no \
                 longer holds on this host",
                long.as_os_str().len(),
                output.status
            )),
        }
    }

    #[test]
    fn spawn_site_measures_a_relative_root_joined_to_the_process_directory() -> Result<(), String> {
        let site = SpawnSite::of(&git_command(Path::new("relative-root"), &[]));
        let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
        assert_eq!(
            site.working_directory,
            Some(cwd.join("relative-root")),
            "a short relative spelling can still name a directory past MAX_PATH"
        );
        Ok(())
    }

    #[test]
    fn spawn_failure_names_the_windows_path_limit_and_remedy_for_overlong_roots() {
        let root = windows_dir_of_units(387);
        let site = SpawnSite {
            program: "git".to_string(),
            working_directory: Some(PathBuf::from(&root)),
        };
        let err = std::io::Error::from_raw_os_error(267);
        let describe = format!("git -C {root} [\"diff\"]");

        let windows = site.failure_message_on(true, &describe, &err);
        assert!(
            windows.starts_with(
                "failed to run git: clone or move the repository to a shorter path; the \
                 workspace root is 387 characters, over the 258 Windows allows for a working \
                 directory (MAX_PATH) ("
            ),
            "{windows}"
        );
        assert!(
            windows.ends_with(&format!("{err}; {describe})")),
            "{windows}"
        );

        assert_eq!(
            site.failure_message_on(false, &describe, &err),
            format!("failed to run {describe}: {err}"),
            "other platforms keep the established spawn-failure text"
        );
        let too_long = std::io::Error::from_raw_os_error(206);
        assert!(
            site.failure_message_on(true, &describe, &too_long)
                .contains("the workspace root is 387 characters"),
            "ERROR_FILENAME_EXCED_RANGE is the other path-limit code"
        );
        for other in [
            std::io::Error::from_raw_os_error(2),
            std::io::Error::from_raw_os_error(5),
            std::io::Error::from(std::io::ErrorKind::NotFound),
        ] {
            assert_eq!(
                site.failure_message_on(true, &describe, &other),
                format!("failed to run {describe}: {other}"),
                "a missing or denied git is not a path-limit failure, however long the root"
            );
        }
        let short = SpawnSite {
            program: "git".to_string(),
            working_directory: Some(PathBuf::from(format!(r"{DRIVE}\repo"))),
        };
        assert_eq!(
            short.failure_message_on(true, &describe, &err),
            format!("failed to run {describe}: {err}"),
            "a short root keeps the established text on Windows too"
        );
        let unset = SpawnSite {
            program: "git".to_string(),
            working_directory: None,
        };
        assert_eq!(
            unset.failure_message_on(true, &describe, &err),
            format!("failed to run {describe}: {err}")
        );
    }

    #[test]
    fn spawn_failure_names_missing_git_on_path_without_dumping_argv() {
        let root = std::env::temp_dir();
        assert!(
            root.exists(),
            "the process temp dir must exist so NotFound is the program, not the cwd"
        );
        let site = SpawnSite {
            program: "git".to_string(),
            working_directory: Some(root.clone()),
        };
        let err = std::io::Error::from(std::io::ErrorKind::NotFound);
        let describe = format!(
            "git -C {} [\"-c\", \"core.quotePath=true\", \"diff\", \"main...HEAD\"]",
            root.display()
        );
        let message = site.failure_message_on(false, &describe, &err);
        assert_eq!(message, GIT_NOT_FOUND_ON_PATH_MESSAGE);
        assert!(is_git_not_found_on_path(&message));
        assert!(
            !message.contains("core.quotePath") && !message.contains('['),
            "the git argv must not reach the user: {message}"
        );

        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(
            site.failure_message_on(false, &describe, &denied),
            format!("failed to run {describe}: {denied}"),
            "a denied git is not a missing-PATH diagnosis"
        );

        let missing_root = SpawnSite {
            program: "git".to_string(),
            working_directory: Some(root.join(format!(
                "ripr-missing-git-cwd-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_nanos())
                    .unwrap_or(0)
            ))),
        };
        assert_eq!(
            missing_root.failure_message_on(false, &describe, &err),
            format!("failed to run {describe}: {err}"),
            "a missing working directory is not a missing-PATH diagnosis"
        );

        let perl = SpawnSite {
            program: "perl-ripr-facts".to_string(),
            working_directory: Some(std::env::temp_dir()),
        };
        assert_eq!(
            perl.failure_message_on(false, "perl-ripr-facts --version", &err),
            format!("failed to run perl-ripr-facts --version: {err}"),
            "a missing non-git program must not steal the git PATH diagnosis"
        );
        assert!(program_is_git("git"));
        assert!(program_is_git("git.exe"));
        assert!(program_is_git(&format!(r"{DRIVE}\tools\git.exe")));
        assert!(!program_is_git("git-lfs") && !program_is_git("perl-ripr-facts"));
    }

    /// Native control for #4735: spawning `git` with an empty PATH must name
    /// the missing binary and the `--diff` route, not dump `core.quotePath`.
    #[test]
    fn native_git_spawn_with_empty_path_names_the_missing_binary() -> Result<(), String> {
        let empty_path = std::env::temp_dir().join(format!(
            "ripr-4735-empty-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&empty_path).map_err(|err| format!("create empty PATH: {err}"))?;
        let mut command = git_command(&empty_path, &["--version"]);
        command.env("PATH", &empty_path);
        let result = collect_output_with_deadline(command, None, "git --version");
        let _ = std::fs::remove_dir_all(&empty_path);
        match result {
            Err(message) => {
                if message != GIT_NOT_FOUND_ON_PATH_MESSAGE {
                    return Err(format!(
                        "empty PATH must name the missing git binary, got: {message}"
                    ));
                }
                if message.contains("core.quotePath") || message.contains('[') {
                    return Err(format!("git argv leaked: {message}"));
                }
                Ok(())
            }
            Ok(output) => Err(format!(
                "git spawned with PATH restricted to {empty_path:?} ({:?}); the \
                 missing-binary premise of #4735 no longer holds on this host",
                output.status
            )),
        }
    }

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
        // Run only the harness test that consumes this mode. Without the
        // exact filter the child starts the entire 3,990-test binary before
        // reaching `reexec_harness`, so the parent can falsely report a pipe
        // deadlock under normal parallel workspace load.
        let test_name = match env_key {
            HANG_ENV => "git::tests::deadline_kills_and_reaps_a_hung_invocation",
            FLOOD_ENV => "git::tests::output_larger_than_the_pipe_buffer_does_not_deadlock",
            _ => return Err(format!("unknown git test harness mode: {env_key}")),
        };
        command.args([test_name, "--exact"]);
        command.env(env_key, "1");
        Ok(command)
    }

    fn hang_command() -> Result<Command, String> {
        self_reexec_command(HANG_ENV)
    }

    /// Spawn the deterministic hung fixture child (the re-executed test
    /// binary sleeps 2 minutes and never exits on its own).
    fn spawn_hung_child() -> Result<OwnedProcess, String> {
        let mut command = hang_command()?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        OwnedProcess::spawn(command).map_err(|err| format!("spawn hung fixture child: {err}"))
    }

    /// Guard that terminates and reaps the hung fixture child on every
    /// exit path, so a broken kill path reports a test failure instead of
    /// orphaning a 2-minute sleeper. The owned subprocess authority
    /// (#3803) performs the terminate-and-reap on drop, so an armed guard
    /// can never leak the sleeper; `disarm` after the reap proof is a
    /// no-op termination of the already-exited child.
    struct HungChildGuard(Option<OwnedProcess>);

    impl HungChildGuard {
        fn disarm(&mut self) {
            if let Some(child) = self.0.take() {
                drop(child);
            }
        }
    }

    /// Deterministic kill+reap proof for a hung fixture child (#3742
    /// Slice 2): drives `poll_child` with the 50ms discriminating
    /// deadline (or a pre-cancelled token when `cancelled`), then asserts
    /// termination via `try_wait` instead of a wall-clock bound.
    /// `poll_child` terminates and reaps before returning a non-`Exited`
    /// arm (see `ChildWait`), so `Ok(Some(_))` is the proof; any other
    /// outcome fails after the guard reaps the child.
    fn assert_hung_child_reaped(cancelled: bool) -> Result<(), String> {
        let token = AnalysisCancellationToken::new();
        if cancelled && !token.cancel(AnalysisAbortKind::Superseded) {
            return Err("fresh token should accept cancellation".to_string());
        }
        let mut guard = HungChildGuard(Some(spawn_hung_child()?));
        let child: &mut OwnedProcess = guard.0.as_mut().ok_or("hung child guard is empty")?;
        let wait = if cancelled {
            with_token(&token, || {
                poll_child(child, Some(Duration::from_mins(2)), "hang-reap-proof")
            })
        } else {
            poll_child(child, Some(Duration::from_millis(50)), "hang-reap-proof")
        };
        let arm_ok = match (&wait, cancelled) {
            (ChildWait::TimedOut(message), false) => message.contains("exceeded the 50ms deadline"),
            (ChildWait::Cancelled(message), true) => {
                is_cancellation_error(message) && !is_git_invocation_timeout(message)
            }
            _ => false,
        };
        let exited = matches!(
            guard
                .0
                .as_mut()
                .ok_or("hung child guard is empty")?
                .try_wait(),
            Ok(Some(_))
        );
        // The guard stays armed through both checks: on either failure
        // path Drop still terminates and reaps the child, so a failed
        // proof can never orphan the 2-minute sleeper. Disarm only after
        // the reap proof succeeds.
        if !arm_ok {
            return Err(format!(
                "hung child wait took the wrong arm for cancelled={cancelled}: {}",
                match wait {
                    ChildWait::Exited(status) => format!("exited: {status}"),
                    ChildWait::TimedOut(message) | ChildWait::Cancelled(message) => message,
                    ChildWait::WaitFailed(error) | ChildWait::CleanupFailed(error) => error,
                }
            ));
        }
        if !exited {
            return Err(
                "hung child remained alive after the kill path; termination was not established"
                    .to_string(),
            );
        }
        guard.disarm();
        Ok(())
    }

    /// A failed terminate-and-reap must replace the pending wait arm with
    /// `CleanupFailed`: returning `TimedOut`/`Cancelled`/`WaitFailed` there
    /// would break the contract that every such arm already terminated and
    /// reaped the child (stubbed termination — no live process needed).
    #[test]
    fn cleanup_failure_replaces_the_pending_wait_arm() -> Result<(), String> {
        let wait = terminate_then_classify(
            "stub child",
            "timeout",
            ChildWait::TimedOut("stub: exceeded the deadline".to_string()),
            || Err("incomplete tree cleanup: refused termination request".to_string()),
        );
        let ChildWait::CleanupFailed(message) = &wait else {
            return Err(format!(
                "cleanup failure should surface as CleanupFailed, got: {}",
                wait.summary()
            ));
        };
        if !message.contains("incomplete tree cleanup") {
            return Err(format!(
                "CleanupFailed should carry the incomplete-cleanup evidence: {message}"
            ));
        }
        if !message.contains("stub: exceeded the deadline") {
            return Err(format!(
                "CleanupFailed should record the suppressed wait outcome: {message}"
            ));
        }
        Ok(())
    }

    /// A completed terminate-and-reap returns the pending arm unchanged —
    /// the classification only intervenes when cleanup fails.
    #[test]
    fn completed_termination_returns_the_pending_wait_arm() {
        let wait = terminate_then_classify(
            "stub child",
            "cancellation",
            ChildWait::Cancelled("stub cancelled".to_string()),
            || Ok(()),
        );
        assert!(matches!(wait, ChildWait::Cancelled(ref message) if message == "stub cancelled"));
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
    #[serial]
    fn deadline_kills_and_reaps_a_hung_invocation() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let command = hang_command()?;
        let result =
            collect_output_with_deadline(command, Some(Duration::from_millis(50)), "hang-test");
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
        // #4946(b): the timeout message names both deadline knobs and the
        // 0-disables escape, matching diff_scope_oversized's in-message
        // repair-route pattern — the error alone must be repairable. The
        // same shared wait also serves the editor sidecar, whose deadline is
        // configured by `gitTimeoutMs`, so the route names that knob too
        // (#4946 review).
        if !err.contains("--git-timeout") || !err.contains("RIPR_GIT_TIMEOUT") {
            return Err(format!(
                "timeout error should name the deadline knobs, got: {err}"
            ));
        }
        if !err.contains("0 disables it") {
            return Err(format!(
                "timeout error should name the 0-disables escape, got: {err}"
            ));
        }
        if !err.contains("gitTimeoutMs") {
            return Err(format!(
                "timeout error should name the editor-session deadline knob, got: {err}"
            ));
        }
        // Kill+reap proof without a wall-clock bound: drive the same
        // deadline machinery on a fresh hung child and assert the exit is
        // observed via try_wait.
        assert_hung_child_reaped(false)?;
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
        let marker_path =
            std::env::temp_dir().join(format!("ripr-pipe-descendant-{}.pid", std::process::id()));
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
        // The deadline must outlast two cold PowerShell starts, or the kill
        // lands before the descendant exists and the marker is never written
        // (seen on a loaded runner at 5s, #3922). The descendant still sleeps
        // 60s, so the timeout remains the discriminating input.
        let result = collect_output_with_deadline(
            command,
            Some(Duration::from_secs(20)),
            "pipe-inheriting-descendant",
        );
        let err = match result {
            Err(err) => err,
            Ok(_) => {
                return Err("a descendant-holding invocation must fail with a timeout".to_string());
            }
        };
        if !is_git_invocation_timeout(&err) {
            return Err(format!("expected the named timeout error, got: {err}"));
        }
        if !err.contains("exceeded the 20000ms deadline") {
            return Err(format!(
                "timeout error should name the deadline, got: {err}"
            ));
        }
        let descendant_pid = std::fs::read_to_string(&marker_path)
            .map_err(|read_error| format!("descendant PID marker was not written: {read_error}"))?
            .trim()
            .parse::<u32>()
            .map_err(|parse_error| format!("descendant PID marker was invalid: {parse_error}"))?;
        let process_check = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "if (Get-Process -Id {descendant_pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
                ),
            ])
            .status()
            .map_err(|check_error| format!("failed to inspect descendant process: {check_error}"))?;
        let _ = std::fs::remove_file(&marker_path);
        if process_check.success() {
            let _ = Command::new("taskkill")
                .args(["/PID", &descendant_pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            return Err(format!(
                "pipe-inheriting descendant {descendant_pid} remained alive after timeout; tree termination was not established"
            ));
        }
        // The descendant lives for 60 seconds; the PID check above is the
        // discriminator (bounded pipe draining alone can return without
        // proving that the inherited writer was terminated). No wall-clock
        // bound: drain time past the kill is capped by
        // POST_KILL_DRAIN_GRACE inside the drain path, and the
        // "exceeded the 20000ms deadline" assert above pins the 20s
        // discriminating input. An elapsed assert would only add flake
        // surface under parallel load.
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
    #[serial]
    fn cancellation_wins_over_a_long_deadline() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let token = AnalysisCancellationToken::new();
        if !token.cancel(AnalysisAbortKind::Superseded) {
            return Err("fresh token should accept cancellation".to_string());
        }
        let command = hang_command()?;
        let result = with_token(&token, || {
            collect_output_with_deadline(command, Some(Duration::from_mins(2)), "hang-test")
        });
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
        // Kill+reap proof without a wall-clock bound: drive the same
        // cancellation machinery on a fresh hung child and assert the exit
        // is observed via try_wait.
        assert_hung_child_reaped(true)?;
        Ok(())
    }

    #[test]
    #[serial]
    fn output_larger_than_the_pipe_buffer_does_not_deadlock() -> Result<(), String> {
        if reexec_harness() {
            return Ok(());
        }
        let command = self_reexec_command(FLOOD_ENV)?;
        let output =
            collect_output_with_deadline(command, Some(Duration::from_secs(30)), "flood-test")?;
        if !output.status.success() {
            return Err(format!("flood child failed: {}", output.status));
        }
        // 8 chunks of 64 KiB must all be collected; a drained pipe is the
        // only way the child could exit without a deadlock. No wall-clock
        // bound: `Ok` already proves the child exited before the 30s
        // deadline (a timeout returns `Err`), so an elapsed assert would
        // only add flake surface under parallel load.
        if output.stdout.len() < 8 * 64 * 1024 {
            return Err(format!(
                "expected drained output of at least 512 KiB, got {} bytes",
                output.stdout.len()
            ));
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

    #[test]
    fn bounded_pipe_reader_drains_but_retains_only_the_limit() -> Result<(), String> {
        let reader = spawn_bounded_pipe_reader(std::io::Cursor::new(vec![b'x'; 65_537]), 1_024);
        let output =
            drain_bounded_pipe_reader(Some(reader), false, None, "stdout", "bounded-test")?;
        if output.bytes.len() != 1_024 {
            return Err(format!(
                "bounded reader retained {} bytes instead of 1024",
                output.bytes.len()
            ));
        }
        if !output.exceeded {
            return Err("bounded reader did not report discarded output".to_string());
        }
        Ok(())
    }

    #[test]
    fn bounded_pipe_reader_propagates_error_after_valid_prefix() -> Result<(), String> {
        struct PrefixThenError {
            emitted: bool,
        }

        impl std::io::Read for PrefixThenError {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.emitted {
                    return Err(std::io::Error::other("injected pipe failure"));
                }
                self.emitted = true;
                let prefix = b"complete\0";
                buffer[..prefix.len()].copy_from_slice(prefix);
                Ok(prefix.len())
            }
        }

        let reader = spawn_bounded_pipe_reader(PrefixThenError { emitted: false }, 1_024);
        let error = drain_bounded_pipe_reader(Some(reader), false, None, "stdout", "error-test")
            .err()
            .ok_or_else(|| "pipe read error was accepted as EOF".to_string())?;
        if !error.contains("injected pipe failure") {
            return Err(format!("unexpected pipe read error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn bounded_pipe_reader_retries_interrupted_reads() -> Result<(), String> {
        struct InterruptedThenBytes {
            state: u8,
        }

        impl std::io::Read for InterruptedThenBytes {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                match self.state {
                    0 => {
                        self.state = 1;
                        Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
                    }
                    1 => {
                        self.state = 2;
                        buffer[..2].copy_from_slice(b"ok");
                        Ok(2)
                    }
                    _ => Ok(0),
                }
            }
        }

        let reader = spawn_bounded_pipe_reader(InterruptedThenBytes { state: 0 }, 16);
        let output =
            drain_bounded_pipe_reader(Some(reader), false, None, "stdout", "interrupt-test")?;
        if output.bytes != b"ok" {
            return Err(format!("interrupted read lost bytes: {:?}", output.bytes));
        }
        Ok(())
    }

    #[test]
    fn bounded_git_output_rejects_zero_limit_before_spawn() -> Result<(), String> {
        let missing = Path::new("definitely-missing-git-root");
        let error =
            run_git_output_with_deadline_and_limit(missing, &["status"], Duration::from_secs(1), 0)
                .err()
                .ok_or_else(|| "zero output limit unexpectedly spawned Git".to_string())?;
        if error != "git output limit must be greater than zero" {
            return Err(format!("unexpected zero-limit error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn bounded_git_output_fails_closed_when_stdout_exceeds_limit() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let error = run_git_output_with_deadline_and_limit(
            &root,
            &["--version"],
            Duration::from_secs(30),
            1,
        )
        .err()
        .ok_or_else(|| "one-byte Git output limit unexpectedly succeeded".to_string())?;
        if !error.starts_with("git_output_limit_exceeded:") {
            return Err(format!("unexpected output-limit error: {error}"));
        }
        Ok(())
    }

    /// A stream fed chunk-by-chunk (as the reader thread delivers it)
    /// parses exactly like one delivered whole: header lines and blob
    /// bytes split across arbitrary chunk boundaries reassemble.
    #[test]
    fn cat_file_batch_stream_reads_across_chunk_boundaries() -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        // A small blob whose framing exercises: a header, content with an
        // interior newline split across chunk boundaries, the framing
        // newline, and a second header starting immediately after.
        let stream_bytes: Vec<u8> = {
            let content = b"hello\nworld"; // 11 bytes, contains an interior newline
            let mut bytes = b"<oid-a> blob 11\n".to_vec();
            bytes.extend_from_slice(content);
            bytes.push(b'\n');
            bytes.extend_from_slice(b"<oid-b> missing\n");
            bytes
        };
        // Feed two-byte chunks: every boundary inside the framing is hit.
        std::thread::spawn(move || {
            for pair in stream_bytes.chunks(2) {
                if sender.send(Ok(pair.to_vec())).is_err() {
                    return;
                }
            }
        });
        let mut stream = CatFileBatchStream::new(receiver);
        let deadline = Instant::now() + Duration::from_secs(30);
        let header = stream
            .read_line(deadline)
            .map_err(|error| format!("header read failed: {error:?}"))?;
        if header != b"<oid-a> blob 11" {
            return Err(format!("unexpected header: {header:?}"));
        }
        let mut content = [0_u8; 11];
        stream
            .read_exact(&mut content, deadline)
            .map_err(|error| format!("content read failed: {error:?}"))?;
        if &content != b"hello\nworld" {
            return Err(format!("content corrupted across chunks: {content:?}"));
        }
        let mut newline = [0_u8; 1];
        stream
            .read_exact(&mut newline, deadline)
            .map_err(|error| format!("framing newline read failed: {error:?}"))?;
        if newline != [b'\n'] {
            return Err(format!("framing newline missing: {newline:?}"));
        }
        let second = stream
            .read_line(deadline)
            .map_err(|error| format!("second header read failed: {error:?}"))?;
        if second != b"<oid-b> missing" {
            return Err(format!("unexpected second header: {second:?}"));
        }
        Ok(())
    }

    /// A stream that ends after a parsed blob header but before the
    /// announced content fails closed instead of returning short content.
    #[test]
    fn cat_file_batch_stream_fails_closed_on_truncated_blob() -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        // The header announces 5 content bytes; the stream delivers 3 and
        // ends. Truncation must be caught after header parsing, not by
        // mistaking header bytes for content.
        let stream_bytes = b"<oid> blob 5\nhel".to_vec();
        std::thread::spawn(move || {
            let _ = sender.send(Ok(stream_bytes));
        });
        let mut stream = CatFileBatchStream::new(receiver);
        let deadline = Instant::now() + Duration::from_secs(30);
        let header = stream
            .read_line(deadline)
            .map_err(|error| format!("header read failed: {error:?}"))?;
        if header != b"<oid> blob 5" {
            return Err(format!("unexpected header: {header:?}"));
        }
        let mut content = [0_u8; 5];
        let outcome = stream.read_exact(&mut content, deadline);
        match outcome {
            Err(CatFileBatchReadError::Failed(message)) if message.contains("ended") => Ok(()),
            other => Err(format!("truncated blob must fail closed, got {other:?}")),
        }
    }

    /// A producer that dribbles fragments just under each individual wait
    /// must still hit the absolute deadline: the per-chunk allowance is
    /// recomputed, not restarted.
    #[test]
    fn cat_file_batch_stream_enforces_the_absolute_deadline() -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            // One byte every 20 ms: every fragment arrives well inside any
            // single wait, forever. Only the absolute deadline stops it.
            for byte in 0u8..=255 {
                if sender.send(Ok(vec![byte])).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let mut stream = CatFileBatchStream::new(receiver);
        let deadline = Instant::now() + Duration::from_millis(50);
        let mut buf = [0_u8; 64];
        let outcome = stream.read_exact(&mut buf, deadline);
        match outcome {
            Err(CatFileBatchReadError::TimedOut) => Ok(()),
            other => Err(format!(
                "a dribbling stream must hit the absolute deadline, got {other:?}"
            )),
        }
    }

    /// A read with no remaining budget classifies as a timeout without
    /// waiting on a silent producer.
    #[test]
    fn cat_file_batch_stream_times_out_when_budget_is_spent() -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            let _ = sender.send(Ok(b"late".to_vec()));
        });
        let mut stream = CatFileBatchStream::new(receiver);
        let mut byte = [0_u8; 1];
        let outcome = stream.read_exact(&mut byte, Instant::now());
        match outcome {
            Err(CatFileBatchReadError::TimedOut) => Ok(()),
            other => Err(format!(
                "spent budget must classify as timed out, got {other:?}"
            )),
        }
    }

    /// Bytes that were buffered or queued before the deadline are still
    /// rejected once it has passed: consumption after the budget is a
    /// timeout, not a late success.
    #[test]
    fn cat_file_batch_stream_rejects_ready_bytes_past_the_deadline() -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        // Every byte is already available before the first read.
        let _ = sender.send(Ok(b"ready".to_vec()));
        let mut stream = CatFileBatchStream::new(receiver);
        let mut buf = [0_u8; 5];
        let outcome = stream.read_exact(&mut buf, Instant::now());
        match outcome {
            Err(CatFileBatchReadError::TimedOut) => Ok(()),
            other => Err(format!(
                "ready bytes past the deadline must still time out, got {other:?}"
            )),
        }
    }

    /// Real-repository session round trip (#5015): one process answers
    /// many blob requests, byte-identical to `git show`, and reports a
    /// missing object as `None` instead of failing mid-stream.
    #[test]
    fn cat_file_batch_session_round_trips_blobs_and_reports_missing() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-cat-file-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        let result = (|| {
            std::fs::write(root.join("one.txt"), "first\n").map_err(|err| err.to_string())?;
            std::fs::write(root.join("two.txt"), "second with more bytes\n")
                .map_err(|err| err.to_string())?;
            run_git(&root, &["init", "--initial-branch=main"])?;
            run_git(&root, &["config", "user.email", "ripr@example.invalid"])?;
            run_git(&root, &["config", "user.name", "ripr test"])?;
            run_git(&root, &["add", "."])?;
            run_git(&root, &["commit", "-m", "seed"])?;
            let listing = run_git(&root, &["ls-tree", "-r", "HEAD"])?;
            let mut requested: Vec<(String, String)> = Vec::new();
            for line in listing.lines() {
                let Some((meta, path)) = line.split_once('\t') else {
                    return Err(format!("unexpected ls-tree line: {line}"));
                };
                let object = meta
                    .split_whitespace()
                    .nth(2)
                    .ok_or_else(|| format!("ls-tree line without an object id: {line}"))?;
                requested.push((object.to_string(), path.to_string()));
            }
            let mut session = CatFileBatch::spawn(&root, Duration::from_mins(1))
                .map_err(|err| err.to_string())?;
            for (object, path) in &requested {
                let size = session
                    .request_blob(object)
                    .map_err(|err| err.to_string())?
                    .ok_or_else(|| format!("blob for {path} unexpectedly missing"))?;
                let mut content = vec![0_u8; size as usize];
                session
                    .read_blob_bytes(&mut content)
                    .map_err(|err| err.to_string())?;
                session.end_blob().map_err(|err| err.to_string())?;
                // Raw oracle bytes: `run_git` trims, which would hide a
                // framing error that dropped a trailing newline.
                let oracle = run_git_output_with_deadline(
                    &root,
                    &["show", &format!("HEAD:{path}")],
                    Some(Duration::from_secs(30)),
                )
                .map_err(|err| err.to_string())?;
                if !oracle.status.success() {
                    return Err(format!("git show oracle failed for {path}"));
                }
                if content != oracle.stdout {
                    return Err(format!(
                        "batch bytes for {path} differ from the git show oracle"
                    ));
                }
            }
            let missing = session
                .request_blob("0123456789012345678901234567890123456789")
                .map_err(|err| err.to_string())?;
            if missing.is_some() {
                return Err("a nonexistent object must report as missing".to_string());
            }
            session.finish().map_err(|err| err.to_string())?;
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    /// A zero overall budget makes a blocking wait classify as the named
    /// `git_invocation_timeout`, raw prefix intact, and the process tree is
    /// terminated. Exercised through `finish`: git is deterministically
    /// blocked reading the still-open stdin, so the deadline — not a race
    /// with a fast answer — is what fires.
    #[test]
    fn cat_file_batch_session_enforces_the_overall_budget() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|err| err.to_string())?;
        let session = CatFileBatch::spawn(&root, Duration::ZERO).map_err(|err| err.to_string())?;
        let error = match session.finish() {
            Err(error) => error,
            Ok(()) => {
                return Err("a zero-budget session unexpectedly waited successfully".to_string());
            }
        };
        if !is_git_invocation_timeout(&error) || !error.contains("0ms") {
            return Err(format!(
                "zero budget must classify as the named timeout: {error}"
            ));
        }
        Ok(())
    }
}
