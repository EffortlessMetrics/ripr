//! Shared git invocation helper.
//!
//! All git subprocess spawns in the published crate should delegate to
//! [`run_git`] so error formatting stays unified and the process-policy
//! allowlist has a single canonical entry point.

use std::path::Path;
use std::process::Command;

/// Run `git -C <root> <args...>` and return trimmed stdout on success.
///
/// Returns a unified error on failure:
/// ```text
/// git -C <root> <args...> failed
/// stdout: <first 500 chars>
/// stderr: <trimmed>
/// ```
#[allow(
    dead_code,
    reason = "new module — callers migrate incrementally in follow-up slices"
)]
pub(crate) fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git -C {} {:?}: {err}", root.display(), args))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_git_returns_trimmed_stdout_on_success() -> Result<(), String> {
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
}
