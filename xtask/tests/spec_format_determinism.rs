use std::fs::{self, FileTimes};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const VALID_SPEC: &str = r#"# RIPR-SPEC-9999: Time-independent fixture

Status: proposed

## Problem

Prove that structural spec validation does not depend on elapsed time.

## Behavior

The same candidate tree has the same format result whenever it is checked.

## Required Evidence

The format command succeeds for an old committed proposed spec.

## Non-Goals

This fixture does not establish implementation, evidence, or support state.

## Acceptance Examples

An unchanged old spec remains structurally valid.

## Test Mapping

- this integration test

## Implementation Mapping

- `xtask/src/main.rs`

## Metrics

- deterministic result for identical candidate bytes
"#;

fn temp_root(label: &str) -> Result<TempRoot, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-spec-format-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("docs/specs")).map_err(|error| error.to_string())?;
    Ok(TempRoot(root))
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl AsRef<Path> for TempRoot {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_spec(root: &Path, text: &str) -> Result<(), String> {
    fs::write(
        root.join("docs/specs/RIPR-SPEC-9999-time-independent-fixture.md"),
        text,
    )
    .map_err(|error| error.to_string())
}

fn backdate_spec(root: &Path) -> Result<(), String> {
    let path = root.join("docs/specs/RIPR-SPEC-9999-time-independent-fixture.md");
    let old = SystemTime::UNIX_EPOCH;
    // Windows SetFileTime requires a write-access handle; a read-only
    // `File::open` fails with access denied, so open for update instead.
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|error| error.to_string())?
        .set_times(FileTimes::new().set_modified(old))
        .map_err(|error| error.to_string())?;
    let modified = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .modified()
        .map_err(|error| error.to_string())?;
    if modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        >= Duration::from_hours(24)
    {
        return Err("fixture spec mtime was not backdated".to_owned());
    }
    Ok(())
}

fn run_check(root: &Path) -> Result<std::process::Output, String> {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-spec-format")
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())
}

fn output_text(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("git {args:?} failed: {}", output_text(&output)))
    }
}

#[test]
fn old_proposed_spec_remains_structurally_valid() -> Result<(), String> {
    let root = temp_root("old-history")?;
    write_spec(&root, VALID_SPEC)?;
    backdate_spec(&root)?;
    run_git(&root, &["-c", "commit.gpgsign=false", "init", "-q"])?;
    run_git(&root, &["add", "--", "docs/specs"])?;
    let commit = Command::new("git")
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "old proposed spec",
        ])
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .current_dir(&root)
        .output()
        .map_err(|error| error.to_string())?;
    if !commit.status.success() {
        return Err(format!(
            "old fixture commit failed: {}",
            output_text(&commit)
        ));
    }

    let output = run_check(&root)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "old proposed spec should remain structurally valid: {}",
            output_text(&output)
        ))
    }
}

#[test]
fn spec_format_does_not_require_git_history() -> Result<(), String> {
    let root = temp_root("no-git")?;
    write_spec(&root, VALID_SPEC)?;
    let output = run_check(&root)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "structural validation should not require Git history: {}",
            output_text(&output)
        ))
    }
}

#[test]
fn structural_spec_errors_remain_blocking() -> Result<(), String> {
    let root = temp_root("missing-heading")?;
    let invalid = VALID_SPEC.replace(
        "## Metrics\n\n- deterministic result for identical candidate bytes\n",
        "",
    );
    write_spec(&root, &invalid)?;
    let output = run_check(&root)?;
    let text = output_text(&output);
    if output.status.success() {
        return Err(format!(
            "missing required heading passed unexpectedly: {text}"
        ));
    }
    if !text.contains("missing `## Metrics`") {
        return Err(format!(
            "missing-heading diagnostic was not preserved: {text}"
        ));
    }
    Ok(())
}
