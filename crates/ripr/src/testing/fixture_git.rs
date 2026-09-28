//! Hardened git helper for unit-test fixture setup (#3742, Slice 1).
//!
//! Fixture repositories are built with short-lived `git` child processes.
//! Host slowness (Defender scans of fresh `.git` directories on Windows,
//! parallel-build CPU starvation) routinely pushes a single `git add` or
//! `commit` past short windows, so every fixture invocation runs under a
//! generous deadline, idempotent commands are retried once after a timeout,
//! and a timed-out `commit` is reconciled against HEAD instead of re-run
//! (a re-run would fail with "nothing to commit" and mask the real outcome).
//! Extracted from the `edit_cage` test helper born in the #3597 review and
//! hardened in review of this slice: the subcommand is parsed past global
//! options, only side-effect-free `checkout` forms are retried, and HEAD
//! probe failures propagate instead of masquerading as an unborn repo.
//! The retry set and reconcile rule below are the contract that the
//! harness-local `tests/common` mirror keeps in sync.

use std::path::Path;
use std::time::Duration;

/// Per-invocation fixture deadline. Host git slowness (Defender scans
/// of fresh `.git` directories on Windows) routinely pushes a single
/// `git add`/`commit` past shorter windows, so the deadline is generous
/// and idempotent commands are retried once on a timeout.
pub(crate) const FIXTURE_GIT_DEADLINE: Duration = Duration::from_secs(30);

/// Per-stream capture cap for one fixture invocation.
const FIXTURE_GIT_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;

/// Fixture git subcommands whose re-execution cannot change fixture state.
/// `commit` is deliberately absent: a timed-out commit may still have
/// landed, so it is reconciled by checking whether HEAD moved instead
/// of blindly re-running (a re-run would fail with "nothing to
/// commit"), (#3597 review). `checkout` is deliberately absent as a
/// blanket entry: only side-effect-free `checkout` forms (see
/// `is_retryable_checkout`) may be retried, because a timed-out stateful
/// checkout may already have moved HEAD or created a branch before the
/// retry runs.
const RETRYABLE_FIXTURE_GIT: &[&str] =
    &["init", "config", "add", "update-ref", "rev-parse", "status"];

/// The git subcommand in `args`, skipping global options, so `["-c",
/// "init.templateDir=", "init", "-q"]` resolves to `init`. Only the
/// value-taking globals reachable from fixture call sites are skipped
/// (`-c`/`--config`/`-C` pairs and inline `--opt=value` forms); anything
/// else, including malformed input, yields the leading token or `None`,
/// which both fail closed (no retry, no reconcile).
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
/// forms that cannot create or move refs are retryable: detached checkout
/// of an explicit revision, and pathspec restores. Stateful forms (`-b`,
/// `-B`, `--orphan`, `-`, branch switches) must surface the original
/// timeout instead of re-running against state the first attempt may
/// already have changed.
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
            // Pathspec restore: re-running restores the same content.
            "--" => return true,
            // Every other flag form (`-b`, `-B`, `--orphan`, `-q`, `-`,
            // `--merge`, `--track`, …) can move HEAD or create state.
            head if head.starts_with('-') => return false,
            // A positional revision is only safe detached.
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
pub(crate) fn fixture_git_ok(root: &Path, args: &[&str]) -> Result<(), String> {
    fixture_git_ok_with_deadline(root, args, FIXTURE_GIT_DEADLINE)
}

/// `fixture_git_ok` with an explicit deadline. Production fixture setup
/// always passes `FIXTURE_GIT_DEADLINE`; tests pass a zero deadline to
/// observe the timeout path without waiting out the production window.
pub(crate) fn fixture_git_ok_with_deadline(
    root: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<(), String> {
    let subcommand = fixture_subcommand(args);
    // A `commit` is reconciled against the revision it started from: a
    // timed-out commit that moved HEAD landed; one that left HEAD
    // unchanged did not and must not be re-run blindly ("nothing to
    // commit" would mask the real failure). A failed pre-commit probe is
    // not an unborn repository, so it propagates instead of seeding the
    // comparison with `None`.
    let head_before = if subcommand == Some("commit") {
        fixture_head(root, deadline)?
    } else {
        None
    };

    let first = match crate::git::run_git_output_with_deadline_and_limit_isolated(
        root,
        args,
        deadline,
        FIXTURE_GIT_OUTPUT_LIMIT,
    ) {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => {
            return Err(format!(
                "isolated fixture git {args:?} failed in {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        // The timeout is the retryable/reconcilable outcome; every other
        // runner error propagates untouched.
        Err(error) if crate::git::is_git_invocation_timeout(&error) => error,
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
    match crate::git::run_git_output_with_deadline_and_limit_isolated(
        root,
        args,
        deadline,
        FIXTURE_GIT_OUTPUT_LIMIT,
    ) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "isolated fixture git {args:?} failed again in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) if crate::git::is_git_invocation_timeout(&error) => {
            Err(format!("retry timed out: {error}"))
        }
        Err(error) => Err(error),
    }
}

/// Current HEAD revision of the fixture repository, or `None` when the
/// repository has no commits (fresh `git init`). Runner failures
/// (timeouts, spawn errors) propagate: a failed probe is not evidence of
/// an unborn repository, and converting it to `None` would let a later
/// successful probe masquerade as a moved HEAD during reconciliation.
fn fixture_head(root: &Path, deadline: Duration) -> Result<Option<String>, String> {
    let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
        root,
        &["rev-parse", "-q", "--verify", "HEAD"],
        deadline,
        FIXTURE_GIT_OUTPUT_LIMIT,
    )
    .map_err(|error| format!("probe fixture HEAD in {}: {error}", root.display()))?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

/// Remove a real-git fixture tree, clearing the Windows readonly attribute
/// git sets on object files first (shared by real-git regression tests so
/// the cleanup helper is not hand-copied per module). A missing tree is
/// not an error; any other failure names the path.
pub(crate) fn remove_fixture_tree(root: &Path) -> Result<(), String> {
    #[cfg(windows)]
    clear_fixture_readonly(root)?;
    match std::fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove fixture tree {} failed: {error}",
            root.display()
        )),
    }
}

#[cfg(windows)]
fn clear_fixture_readonly(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("read {} failed: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read fixture entry failed: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("stat {} failed: {error}", path.display()))?;
        if file_type.is_dir() {
            clear_fixture_readonly(&path)?;
            continue;
        }
        let mut permissions = entry
            .metadata()
            .map_err(|error| format!("stat {} failed: {error}", path.display()))?
            .permissions();
        if !permissions.readonly() {
            continue;
        }
        // Clearing FILE_ATTRIBUTE_READONLY is the only way to remove
        // the fixture tree on Windows; the Unix-mode lint does not
        // apply to this cfg-gated helper.
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "clearing the Windows readonly attribute before fixture cleanup"
        )]
        {
            permissions.set_readonly(false);
            std::fs::set_permissions(&path, permissions)
                .map_err(|error| format!("clear readonly on {} failed: {error}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-fixture-git-{label}-{}-{}-{stamp}",
            std::process::id(),
            FIXTURE_GIT_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("create {} failed: {err}", root.display()))?;
        Ok(TempDirGuard(root))
    }

    #[test]
    fn builds_a_committed_fixture_repo() -> Result<(), String> {
        let root = unique_root("committed")?;
        fixture_git_ok(&root, &["-c", "init.templateDir=", "init", "-q"])?;
        fixture_git_ok(&root, &["config", "user.email", "ripr@example.invalid"])?;
        fixture_git_ok(&root, &["config", "user.name", "RIPR Test"])?;
        fixture_git_ok(&root, &["config", "commit.gpgSign", "false"])?;
        std::fs::write(root.join("marker.txt"), "fixture\n")
            .map_err(|err| format!("write marker failed: {err}"))?;
        fixture_git_ok(&root, &["add", "."])?;
        fixture_git_ok(&root, &["commit", "-qm", "baseline"])?;
        fixture_git_ok(&root, &["rev-parse", "--verify", "HEAD"])?;
        Ok(())
    }

    #[test]
    fn reports_a_failing_git_command() -> Result<(), String> {
        let root = unique_root("failing")?;
        match fixture_git_ok(&root, &["no-such-command-xyz"]) {
            Ok(()) => Err("unknown git command unexpectedly succeeded".to_string()),
            Err(error) if error.contains("failed") => Ok(()),
            Err(error) => Err(format!("unexpected error shape: {error}")),
        }
    }

    #[test]
    fn resolves_the_subcommand_past_global_options() -> Result<(), String> {
        if fixture_subcommand(&["-c", "init.templateDir=", "init", "-q"]) != Some("init") {
            return Err("global -c pair must resolve to the init subcommand".to_string());
        }
        if fixture_subcommand(&["commit", "-qm", "baseline"]) != Some("commit") {
            return Err("plain commit must resolve to itself".to_string());
        }
        if fixture_subcommand(&["-c"]).is_some() {
            return Err("dangling global option must resolve to no subcommand".to_string());
        }
        Ok(())
    }

    #[test]
    fn retries_init_past_global_options_on_timeout() -> Result<(), String> {
        let root = unique_root("timeout-globals")?;
        match fixture_git_ok_with_deadline(
            &root,
            &["-c", "init.templateDir=", "init"],
            Duration::ZERO,
        ) {
            Ok(()) => Err("zero-deadline init unexpectedly succeeded".to_string()),
            // A retry ran (and timed out too): the `-c`-prefixed init was
            // recognized as the retryable subcommand.
            Err(error) if error.contains("retry timed out") => Ok(()),
            Err(error) => Err(format!("unexpected error shape: {error}")),
        }
    }

    #[test]
    fn retries_detached_checkout_but_not_branch_creation() -> Result<(), String> {
        let root = unique_root("timeout-checkout")?;
        match fixture_git_ok_with_deadline(
            &root,
            &["checkout", "--detach", "deadbeef"],
            Duration::ZERO,
        ) {
            Ok(()) => return Err("zero-deadline checkout unexpectedly succeeded".to_string()),
            Err(error) if error.contains("retry timed out") => {}
            Err(error) => {
                return Err(format!("unexpected detached-checkout error shape: {error}"));
            }
        }
        match fixture_git_ok_with_deadline(
            &root,
            &["checkout", "-b", "probe-branch"],
            Duration::ZERO,
        ) {
            Ok(()) => Err("zero-deadline branch creation unexpectedly succeeded".to_string()),
            Err(error) if !error.contains("retry timed out") => Ok(()),
            Err(error) => Err(format!("stateful checkout must not retry, got: {error}")),
        }
    }

    #[test]
    fn zero_deadline_retries_then_reports_a_timeout() -> Result<(), String> {
        let root = unique_root("timeout-retry")?;
        match fixture_git_ok_with_deadline(&root, &["init"], Duration::ZERO) {
            Ok(()) => Err("zero-deadline init unexpectedly succeeded".to_string()),
            Err(error) if error.contains("retry timed out") => Ok(()),
            Err(error) => Err(format!("unexpected error shape: {error}")),
        }
    }

    #[test]
    fn zero_deadline_never_retries_a_non_idempotent_command() -> Result<(), String> {
        let root = unique_root("timeout-no-retry")?;
        match fixture_git_ok_with_deadline(&root, &["reset"], Duration::ZERO) {
            Ok(()) => Err("zero-deadline reset unexpectedly succeeded".to_string()),
            Err(error)
                if error.contains(crate::git::GIT_INVOCATION_TIMEOUT_PREFIX)
                    && !error.contains("retry timed out") =>
            {
                Ok(())
            }
            Err(error) => Err(format!("unexpected error shape: {error}")),
        }
    }

    #[test]
    fn zero_deadline_commit_reports_the_probe_failure() -> Result<(), String> {
        let root = unique_root("timeout-commit")?;
        match fixture_git_ok_with_deadline(&root, &["commit", "-m", "x"], Duration::ZERO) {
            Ok(()) => Err("zero-deadline commit unexpectedly succeeded".to_string()),
            // The pre-commit HEAD probe already fails on the zero
            // deadline; that probe error (not a fabricated `None`)
            // surfaces instead of a reconcile decision.
            Err(error)
                if error.contains("probe fixture HEAD")
                    && error.contains(crate::git::GIT_INVOCATION_TIMEOUT_PREFIX) =>
            {
                Ok(())
            }
            Err(error) => Err(format!("unexpected error shape: {error}")),
        }
    }
}
