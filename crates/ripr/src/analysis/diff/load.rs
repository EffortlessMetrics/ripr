use std::path::{Path, PathBuf};
use std::process::Command;

pub fn load_diff(
    root: &Path,
    base: Option<&str>,
    diff_file: Option<&PathBuf>,
) -> Result<String, String> {
    if let Some(diff_file) = diff_file {
        return std::fs::read_to_string(diff_file)
            .map_err(|err| format!("failed to read diff file {}: {err}", diff_file.display()));
    }

    // RIPR-SPEC-0084: when the caller passes an explicit base, use it as-is
    // (if it does not exist, git diff will surface a clear error that names
    // the ref the user chose). When the caller passes None (bare `ripr check`,
    // no --base flag), resolve the repo's real default branch rather than
    // hardcoding origin/main.
    let owned;
    let base: &str = if let Some(explicit) = base {
        explicit
    } else {
        owned = resolve_default_base(root)?;
        &owned
    };

    run_git_diff(root, &format!("{base}...HEAD"), &[])
}

/// Resolve the best available base ref for `ripr check` when none was
/// explicitly given by the caller.
///
/// Tries in order, verifying each candidate with `git rev-parse --verify`:
/// 1. `git symbolic-ref --quiet refs/remotes/origin/HEAD` — the remote's own
///    default branch pointer (works for `master`-default, renamed, etc.).
/// 2. `origin/main` — common explicit remote-tracking fallback.
/// 3. `origin/master` — common explicit remote-tracking fallback.
/// 4. `main` — local branch fallback.
/// 5. `master` — local branch fallback.
///
/// Returns `Err` with a named, actionable message when none of the above
/// resolves (e.g. bare repo, no commits, no remote, detached with no
/// branches). The message does NOT claim an empty analysis result — it
/// explicitly says "could not resolve a base (the analysis did not run)".
fn resolve_default_base(root: &Path) -> Result<String, String> {
    // Step 1: ask the remote itself what its default branch is.
    if let Some(remote_head) = git_symbolic_ref_quiet(root, "refs/remotes/origin/HEAD") {
        // symbolic-ref returns e.g. "refs/remotes/origin/master"; convert to
        // the tracking ref form "origin/master".
        if let Some(stripped) = remote_head.strip_prefix("refs/remotes/") {
            let candidate = stripped.to_string();
            if git_ref_exists(root, &candidate) {
                return Ok(candidate);
            }
        }
    }

    // Steps 2-5: explicit fallbacks verified with rev-parse.
    for candidate in &["origin/main", "origin/master", "main", "master"] {
        if git_ref_exists(root, candidate) {
            return Ok((*candidate).to_string());
        }
    }

    // Fail closed: nothing resolves — emit a named, actionable message.
    // This is distinct from "analyzed and found nothing": the analysis did
    // not run because there was no base to diff against.
    Err(
        "could not resolve a default base (no origin/main, origin/master, or local main/master \
         found). Pass `--base <ref>` to diff against a specific ref, or \
         `--root . --format repo-exposure-md` for a full-repo scan."
            .to_string(),
    )
}

/// Run `git symbolic-ref --quiet <refname>` and return the target on success.
/// Returns `None` when the ref does not exist or is not symbolic (exit ≠ 0).
fn git_symbolic_ref_quiet(root: &Path, refname: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--quiet", refname])
        .current_dir(root)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Return `true` when `git rev-parse --verify --quiet <refname>` succeeds,
/// meaning the ref genuinely exists in the repository.
fn git_ref_exists(root: &Path, refname: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", refname])
        .current_dir(root)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn load_diff_range(root: &Path, base: &str, head: &str) -> Result<String, String> {
    run_git_diff(
        root,
        &format!("{base}...{head}"),
        &["--unified=0", "--no-ext-diff"],
    )
}

fn run_git_diff(root: &Path, range: &str, extra_args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("diff")
        .args(extra_args)
        .arg(range)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Test asserts an expected error variant via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn load_diff_from_file_returns_content() -> std::io::Result<()> {
        let dir = std::env::temp_dir().join("ripr-load-diff-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        let diff_file = dir.join("test.diff");
        fs::write(&diff_file, "test content")?;

        let result = load_diff(&dir, None, Some(&diff_file));
        assert_eq!(result.as_deref(), Ok("test content"));

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn load_diff_with_missing_file_returns_error() -> std::io::Result<()> {
        let result = load_diff(
            &std::env::current_dir()?,
            None,
            Some(&PathBuf::from("/nonexistent/path/to/file")),
        );
        result.expect_err("expected diff load to fail for missing file");
        Ok(())
    }

    // RIPR-SPEC-0084: resolution tests using real temp git repos.

    /// Helper: initialise a git repo, create an initial commit, and return the
    /// repo root. Uses `--initial-branch` if available; falls back to renaming
    /// the default branch via `git symbolic-ref`.
    fn init_git_repo(dir: &Path, branch: &str) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        // Try --initial-branch first (git >= 2.28).
        let status = Command::new("git")
            .args(["init", "--initial-branch", branch])
            .current_dir(dir)
            .output()?
            .status;
        if !status.success() {
            Command::new("git").arg("init").current_dir(dir).output()?;
            Command::new("git")
                .args(["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")])
                .current_dir(dir)
                .output()?;
        }
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()?;
        // Create an initial commit so rev-parse works.
        fs::write(dir.join("README"), "init")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .output()?;
        Ok(())
    }

    #[test]
    fn resolve_default_base_uses_origin_master_when_symbolic_ref_points_there()
    -> std::io::Result<()> {
        // Simulates a repo whose remote default branch is "master" (not "main").
        // We create a local repo, then set refs/remotes/origin/HEAD to point at
        // refs/remotes/origin/master, and create that ref.
        let dir = std::env::temp_dir().join("ripr-resolve-base-origin-master");
        let _ = fs::remove_dir_all(&dir);
        init_git_repo(&dir, "master")?;
        // Create the remote-tracking ref manually (simulates a fetched remote).
        Command::new("git")
            .args(["update-ref", "refs/remotes/origin/master", "HEAD"])
            .current_dir(&dir)
            .output()?;
        Command::new("git")
            .args([
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/master",
            ])
            .current_dir(&dir)
            .output()?;

        let result = resolve_default_base(&dir);
        assert_eq!(
            result.as_deref(),
            Ok("origin/master"),
            "expected origin/master resolution via symbolic-ref"
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolve_default_base_uses_local_main_when_no_remote() -> std::io::Result<()> {
        // Simulates a fresh git init with no remote; local branch is "main".
        let dir = std::env::temp_dir().join("ripr-resolve-base-local-main");
        let _ = fs::remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;
        // Confirm no remote refs exist.
        let refs_remote = dir.join(".git").join("refs").join("remotes");
        let _ = fs::remove_dir_all(&refs_remote);

        let result = resolve_default_base(&dir);
        assert_eq!(
            result.as_deref(),
            Ok("main"),
            "expected local main fallback when no remote"
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn resolve_default_base_returns_named_error_when_nothing_resolves() -> std::io::Result<()> {
        // Simulates a bare repo with no commits and no remote refs. We create
        // a temp dir, run git init, but do NOT create any commits or refs.
        let dir = std::env::temp_dir().join("ripr-resolve-base-no-base");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Command::new("git").arg("init").current_dir(&dir).output()?;
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&dir)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&dir)
            .output()?;
        // No commit, no remote refs, no branches — nothing resolves.

        let result = resolve_default_base(&dir);
        let err = result.expect_err("expected a named error when no base resolves");
        assert!(
            err.contains("could not resolve a default base"),
            "expected named actionable message, got: {err}"
        );
        assert!(
            err.contains("--base <ref>"),
            "expected --base guidance in message, got: {err}"
        );
        assert!(
            err.contains("--format repo-exposure-md"),
            "expected --format repo-exposure-md guidance in message, got: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn explicit_base_is_used_as_is_without_resolution() -> std::io::Result<()> {
        // When an explicit base is given, load_diff does not attempt resolution.
        // A nonexistent explicit base should produce a git-diff error (not the
        // named "could not resolve" message), confirming the explicit path is kept.
        let dir = std::env::temp_dir().join("ripr-explicit-base-no-subst");
        let _ = fs::remove_dir_all(&dir);
        init_git_repo(&dir, "main")?;

        let result = load_diff(&dir, Some("nonexistent-branch-xyz"), None);
        let err = result.expect_err("expected error for nonexistent explicit base");
        // Must NOT contain the auto-resolve message (that would mean we silently
        // substituted the explicit ref).
        assert!(
            !err.contains("could not resolve a default base"),
            "explicit base must not trigger auto-resolve fallback; got: {err}"
        );
        // Must surface a git error referencing the chosen ref.
        assert!(
            err.contains("nonexistent-branch-xyz") || err.contains("git diff failed"),
            "expected git-diff error for explicit bad base, got: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
