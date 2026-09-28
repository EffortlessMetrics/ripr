//! Hardened git helper for integration-test fixture setup (#3742, Slice 1).
//!
//! Contract mirror of `crate::testing::fixture_git`: every fixture
//! invocation runs under a generous deadline with piped output drained on
//! reader threads while the child runs, idempotent commands are retried
//! once after a timeout, and a timed-out `commit` is reconciled against
//! HEAD instead of re-run. The subcommand is parsed past global options,
//! only side-effect-free `checkout` forms are retried, and HEAD probe
//! failures propagate instead of masquerading as an unborn repository.
//! Integration-test targets cannot see `pub(crate)` library items, so this
//! harness-local copy carries the same behavior on `std` only; keep the
//! deadline, retry set, reconcile rule, and drain discipline in sync with
//! the canonical in-crate module.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Per-invocation fixture deadline, mirroring the canonical helper.
pub const FIXTURE_GIT_DEADLINE: Duration = Duration::from_secs(30);

/// Cap on stored per-stream output. Draining continues past the cap so a
/// noisy child can never block on a full pipe; only retention stops.
const FIXTURE_GIT_STREAM_CAP: usize = 1024 * 1024;

/// Fixture git subcommands whose re-execution cannot change fixture state.
/// `commit` is deliberately absent (reconciled, never re-run) and
/// `checkout` is deliberately absent as a blanket entry (see
/// `is_retryable_checkout`).
const RETRYABLE_FIXTURE_GIT: &[&str] =
    &["init", "config", "add", "update-ref", "rev-parse", "status"];

/// Poll interval while waiting out a fixture invocation.
const FIXTURE_GIT_POLL: Duration = Duration::from_millis(10);

/// Prefix identifying a deadline expiry (never a git failure).
const FIXTURE_GIT_TIMEOUT_PREFIX: &str = "fixture_git_timeout";

/// The git subcommand in `args`, skipping global options, so `["-c",
/// "init.templateDir=", "init", "-q"]` resolves to `init`. Malformed
/// input yields the leading token or `None`, which both fail closed.
fn fixture_subcommand<'a>(args: &'a [&str]) -> Option<&'a str> {
    let mut rest = args;
    loop {
        let (&head, tail) = rest.split_first()?;
        if head == "-c" || head == "--config" || head == "-C" {
            rest = tail.get(1..)?;
            continue;
        }
        if head.starts_with("--") && head.contains('=') {
            rest = tail;
            continue;
        }
        return Some(head);
    }
}

/// Whether a `checkout` invocation may be retried after a timeout. Only
/// detached checkout of an explicit revision and pathspec restores are
/// retryable; stateful forms (`-b`, `-B`, `--orphan`, `-`, branch
/// switches) must surface the original timeout instead.
fn is_retryable_checkout(args: &[&str]) -> bool {
    let mut index = 0;
    while index < args.len() {
        let head = args[index];
        if head == "-c" || head == "--config" || head == "-C" {
            index += 2;
            continue;
        }
        if head.starts_with("--") && head.contains('=') {
            index += 1;
            continue;
        }
        break;
    }
    let tail = args.get(index..).and_then(|rest| rest.split_first());
    let Some((&"checkout", mut rest)) = tail else {
        return false;
    };
    let mut detached = false;
    while let Some((&head, tail)) = rest.split_first() {
        match head {
            "--detach" => {
                detached = true;
                rest = tail;
            }
            "--" => return true,
            head if head.starts_with('-') => return false,
            _ => {
                if detached {
                    rest = tail;
                } else {
                    return false;
                }
            }
        }
    }
    detached
}

/// Run one fixture `git` invocation under the shared deadline, with one
/// idempotent retry and commit reconcile. Returns `Ok(())` only when git
/// itself reports success (or a timed-out commit provably landed).
pub fn fixture_git_ok(root: &Path, args: &[&str]) -> Result<(), String> {
    fixture_git_ok_with_deadline(root, args, FIXTURE_GIT_DEADLINE)
}

/// `fixture_git_ok` with an explicit deadline. Fixture setup always
/// passes `FIXTURE_GIT_DEADLINE`; tests pass a zero deadline to observe
/// the timeout path without waiting out the production window.
pub fn fixture_git_ok_with_deadline(
    root: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<(), String> {
    let subcommand = fixture_subcommand(args);
    // A `commit` is reconciled against the revision it started from. A
    // failed pre-commit probe is not an unborn repository, so it
    // propagates instead of seeding the comparison with `None`.
    let head_before = if subcommand == Some("commit") {
        fixture_head(root, deadline)?
    } else {
        None
    };

    let first = match run_git_deadline(root, args, deadline) {
        Ok(FixtureGitOutcome::Finished(output)) if output.status.success() => return Ok(()),
        Ok(FixtureGitOutcome::Finished(output)) => {
            return Err(format!(
                "fixture git {args:?} failed in {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        // The timeout is the retryable/reconcilable outcome; a spawn or
        // wait failure propagates untouched.
        Ok(FixtureGitOutcome::TimedOut(error)) => error,
        Err(error) => return Err(error),
    };

    // Reconcile a timed-out commit: HEAD moved past the pre-commit
    // revision, so the commit landed despite the deadline. A failed
    // post-commit probe leaves landing unprovable, so the original
    // timeout is reported.
    if subcommand == Some("commit") {
        match fixture_head(root, deadline) {
            Ok(head_after) => {
                let landed = head_after.is_some() && head_after != head_before;
                if landed {
                    return Ok(());
                }
                return Err(first);
            }
            Err(_) => return Err(first),
        }
    }

    // Only provably idempotent commands are re-executed.
    let retryable = match subcommand {
        Some(command) if RETRYABLE_FIXTURE_GIT.contains(&command) => true,
        Some("checkout") => is_retryable_checkout(args),
        _ => false,
    };
    if !retryable {
        return Err(first);
    }
    match run_git_deadline(root, args, deadline) {
        Ok(FixtureGitOutcome::Finished(output)) if output.status.success() => Ok(()),
        Ok(FixtureGitOutcome::Finished(output)) => Err(format!(
            "fixture git {args:?} failed again in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Ok(FixtureGitOutcome::TimedOut(error)) => Err(format!("retry timed out: {error}")),
        Err(error) => Err(error),
    }
}

/// `fixture_git_ok` for commands whose stdout the test consumes
/// (`rev-parse`, `diff`): same deadline, single idempotent retry, and
/// stream-drain discipline. Stdout is decoded strictly — non-UTF-8 output
/// fails with a named error instead of recording replacement characters.
/// `commit` is rejected: landing reconcile carries no stdout, so a commit
/// through this helper would silently discard the reconcile contract; use
/// `fixture_git_ok` for state-changing invocations.
// Shared test helper consumed by a different integration-test target; dead in
// targets that only consume `fixture_git_ok` (see .ripr/allow-attributes.txt).
#[allow(dead_code, reason = "consumed by the generated-workflow target only")]
pub fn fixture_git_output(root: &Path, args: &[&str]) -> Result<String, String> {
    if fixture_subcommand(args) == Some("commit") {
        return Err("fixture_git_output does not support commit; use fixture_git_ok".to_string());
    }
    let first = match run_git_deadline(root, args, FIXTURE_GIT_DEADLINE) {
        Ok(FixtureGitOutcome::Finished(output)) if output.status.success() => {
            return decode_stdout(root, args, &output.stdout);
        }
        Ok(FixtureGitOutcome::Finished(output)) => {
            return Err(format!(
                "fixture git {args:?} failed in {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(FixtureGitOutcome::TimedOut(error)) => error,
        Err(error) => return Err(error),
    };

    // Only provably idempotent commands are re-executed.
    let retryable = match fixture_subcommand(args) {
        Some(command) if RETRYABLE_FIXTURE_GIT.contains(&command) => true,
        Some("checkout") => is_retryable_checkout(args),
        _ => false,
    };
    if !retryable {
        return Err(first);
    }
    match run_git_deadline(root, args, FIXTURE_GIT_DEADLINE) {
        Ok(FixtureGitOutcome::Finished(output)) if output.status.success() => {
            decode_stdout(root, args, &output.stdout)
        }
        Ok(FixtureGitOutcome::Finished(output)) => Err(format!(
            "fixture git {args:?} failed again in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Ok(FixtureGitOutcome::TimedOut(error)) => Err(format!("retry timed out: {error}")),
        Err(error) => Err(error),
    }
}

/// Strict stdout decode for [`fixture_git_output`]: fixture output feeds
/// identity comparisons, so lossy conversion would collapse distinct paths.
// Companion to `fixture_git_output`; same per-target dead-code note.
#[allow(dead_code, reason = "consumed by the generated-workflow target only")]
fn decode_stdout(root: &Path, args: &[&str], stdout: &[u8]) -> Result<String, String> {
    String::from_utf8(stdout.to_vec()).map_err(|error| {
        format!(
            "fixture git {args:?} produced non-UTF-8 output in {}: {error}",
            root.display()
        )
    })
}

/// Current HEAD revision of the fixture repository, or `None` when the
/// repository has no commits. Runner failures propagate: a failed probe
/// is not evidence of an unborn repository.
fn fixture_head(root: &Path, deadline: Duration) -> Result<Option<String>, String> {
    let outcome = run_git_deadline(root, &["rev-parse", "-q", "--verify", "HEAD"], deadline)
        .map_err(|error| format!("probe fixture HEAD in {}: {error}", root.display()))?;
    let output = match outcome {
        FixtureGitOutcome::Finished(output) => output,
        FixtureGitOutcome::TimedOut(error) => {
            return Err(format!("probe fixture HEAD in {}: {error}", root.display()));
        }
    };
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

enum FixtureGitOutcome {
    Finished(std::process::Output),
    TimedOut(String),
}

/// Drain one piped stream to EOF on a helper thread so the poll loop
/// below never deadlocks against a full OS pipe buffer (a noisy hook or
/// configuration can otherwise wedge fixture setup). Retention stops at
/// the stream cap; draining continues so the child can always proceed.
fn spawn_stream_drain(
    pipe: impl std::io::Read + Send + 'static,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut pipe = pipe;
        let mut stored = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match std::io::Read::read(&mut pipe, &mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    let room = FIXTURE_GIT_STREAM_CAP.saturating_sub(stored.len());
                    stored.extend_from_slice(&chunk[..read.min(room)]);
                }
                Err(_) => break,
            }
        }
        stored
    })
}

fn run_git_deadline(
    root: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<FixtureGitOutcome, String> {
    // Mirror the canonical runner in `crate::git`: a zero deadline is refused
    // before spawning. Spawning would let any git that exits before the first
    // `try_wait` below report as finished, so a zero-deadline caller would
    // see success, a git failure or a timeout depending on scheduling.
    if deadline.is_zero() {
        return Ok(FixtureGitOutcome::TimedOut(format!(
            "{FIXTURE_GIT_TIMEOUT_PREFIX}: fixture git {args:?} was given a zero deadline (not spawned) in {}",
            root.display()
        )));
    }
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn fixture git {args:?} in {}: {error}", root.display()))?;
    let stdout_drain = child.stdout.take().map(spawn_stream_drain);
    let stderr_drain = child.stderr.take().map(spawn_stream_drain);
    let started = Instant::now();
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("wait fixture git {args:?} in {}: {error}", root.display()))?
        {
            Some(status) => {
                // The child exited; joining reaps it and collecting the
                // drained concurrently-read streams cannot block.
                let _ = child.wait();
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(handle) = stdout_drain {
                    stdout = handle.join().map_err(|error| {
                        format!("stdout drain failed for fixture git {args:?}: {error:?}")
                    })?;
                }
                if let Some(handle) = stderr_drain {
                    stderr = handle.join().map_err(|error| {
                        format!("stderr drain failed for fixture git {args:?}: {error:?}")
                    })?;
                }
                return Ok(FixtureGitOutcome::Finished(std::process::Output {
                    status,
                    stdout,
                    stderr,
                }));
            }
            None if started.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                // Reader threads observe EOF once the child is gone; no
                // descendant holds these pipes in fixture setup.
                return Ok(FixtureGitOutcome::TimedOut(format!(
                    "{FIXTURE_GIT_TIMEOUT_PREFIX}: fixture git {args:?} exceeded {deadline:?} in {}",
                    root.display()
                )));
            }
            None => std::thread::sleep(FIXTURE_GIT_POLL),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_GIT_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Fixture root that removes itself on every exit path, so a failing
    /// test never leaks its temp directory.
    struct TempDirGuard(PathBuf);

    impl std::ops::Deref for TempDirGuard {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn unique_root(label: &str) -> Result<TempDirGuard, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-common-fixture-git-{label}-{}-{}",
            std::process::id(),
            FIXTURE_GIT_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
        Ok(TempDirGuard(root))
    }

    #[test]
    fn resolves_the_subcommand_past_global_options() -> Result<(), String> {
        if fixture_subcommand(&["-c", "init.templateDir=", "init", "-q"]) != Some("init") {
            return Err("global -c pair must resolve to the init subcommand".to_string());
        }
        if !is_retryable_checkout(&["checkout", "--detach", "deadbeef"]) {
            return Err("detached checkout must stay retryable".to_string());
        }
        if is_retryable_checkout(&["checkout", "-b", "probe-branch"]) {
            return Err("branch creation must never retry".to_string());
        }
        Ok(())
    }

    #[test]
    fn zero_deadline_reports_timeouts_without_retrying_state() -> Result<(), String> {
        let root = unique_root("timeout")?;
        match fixture_git_ok_with_deadline(
            &root,
            &["-c", "init.templateDir=", "init"],
            Duration::ZERO,
        ) {
            Ok(()) => return Err("zero-deadline init unexpectedly succeeded".to_string()),
            // Both attempts refused before spawning: init is retried, and the
            // retry is refused the same way.
            Err(error) if error.contains("retry timed out") && error.contains("(not spawned)") => {}
            Err(error) => {
                return Err(format!("unexpected init error shape: {error}"));
            }
        }
        // A commit is never re-run. Its pre-commit HEAD probe is refused
        // before spawning, and that refusal propagates, so the commit itself
        // never runs. When the runner still spawned on a zero deadline, this
        // outcome depended on whether each git exited before the first poll
        // (#3742), and hosted CI hit the init retry's lock failure on #4014.
        match fixture_git_ok_with_deadline(&root, &["commit", "-m", "x"], Duration::ZERO) {
            Ok(()) => Err("zero-deadline commit unexpectedly succeeded".to_string()),
            Err(error)
                if error.contains("probe fixture HEAD") && error.contains("(not spawned)") =>
            {
                Ok(())
            }
            Err(error) => Err(format!("unexpected commit error shape: {error}")),
        }
    }
}
