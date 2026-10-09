//! Built-binary stdout/stderr discriminator for CLI analysis progress (#4810).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ripr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
}

/// Exact tracked sample bytes in a fixture-owned workspace. The diff's
/// repository-relative paths stay unchanged, so this is real sample analysis.
struct OwnedProgressSample {
    root: PathBuf,
}

impl OwnedProgressSample {
    fn new(tag: &str) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "ripr-cli-progress-sample-{tag}-{}-{stamp}",
            std::process::id()
        ));
        // Acquire this exact directory; never adopt an existing fixture root.
        std::fs::create_dir(&root)
            .map_err(|error| format!("create owned progress sample: {error}"))?;
        let fixture = Self { root };
        std::fs::write(fixture.root.join("Cargo.toml"), "[workspace]\n")
            .map_err(|error| format!("write sample workspace boundary: {error}"))?;
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample");
        let destination = fixture.root.join("crates/ripr/examples/sample");
        for relative in ["example.diff", "src/lib.rs", "tests/pricing.rs"] {
            let target = destination.join(relative);
            let parent = target.parent().ok_or("sample target has no parent")?;
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create sample parent: {error}"))?;
            std::fs::copy(source.join(relative), &target)
                .map_err(|error| format!("copy exact sample {relative}: {error}"))?;
        }
        Ok(fixture)
    }

    fn root_arg(&self) -> String {
        self.root.display().to_string()
    }

    fn diff_arg(&self) -> String {
        self.root
            .join("crates/ripr/examples/sample/example.diff")
            .display()
            .to_string()
    }

    fn sample_root_arg(&self) -> String {
        self.root
            .join("crates/ripr/examples/sample")
            .display()
            .to_string()
    }
}

impl Drop for OwnedProgressSample {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The format/progress contract needs real, nonempty diff analysis, not this
/// repository's unrelated source/test inventory. Keep one root for each pair
/// so the loud/quiet byte comparison observes the same analysis identity.
struct DiffFixture(PathBuf);

impl DiffFixture {
    fn new() -> Result<Self, String> {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tag = format!(
            "diff-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let fixture = Self(repo_fixture_root(&tag)?);
        // The shared owned factory already writes the genuine [workspace]
        // boundary. A second table would make its Cargo manifest invalid.
        let config = ripr::config::load_for_root(&fixture.0)?;
        let languages = config
            .languages
            .enabled
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>();
        if config.source_path.is_some() || languages != ["rust"] {
            return Err(
                "diff fixture must use built-in Rust defaults at its owned workspace boundary"
                    .to_string(),
            );
        }
        std::fs::create_dir_all(fixture.0.join("tests"))
            .map_err(|error| format!("create diff fixture tests: {error}"))?;
        std::fs::write(
            fixture.0.join("tests/threshold.rs"),
            format!(
                "use ripr_cli_progress_{}::over_threshold;\n\n#[test]\nfn either_side_of_threshold() {{\n    assert!(over_threshold(11, 10));\n    assert!(!over_threshold(9, 10));\n}}\n",
                tag.replace('-', "_")
            ),
        )
        .map_err(|error| format!("write diff fixture tests: {error}"))?;
        std::fs::write(
            fixture.0.join("change.diff"),
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n-    amount > threshold\n+    amount >= threshold\n }\n",
        )
        .map_err(|error| format!("write diff fixture input: {error}"))?;
        Ok(fixture)
    }
}

impl Drop for DiffFixture {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.0);
    }
}

fn run_check(fixture: &DiffFixture, extra: &[&str]) -> Result<Output, String> {
    run_check_at(&fixture.0, &fixture.0.join("change.diff"), extra)
}

fn run_check_at(root: &Path, diff: &Path, extra: &[&str]) -> Result<Output, String> {
    let root_arg = root.display().to_string();
    let diff_arg = diff.display().to_string();
    let mut args = vec![
        "check",
        "--root",
        root_arg.as_str(),
        "--diff",
        diff_arg.as_str(),
    ];
    args.extend_from_slice(extra);
    ripr()
        .current_dir(root)
        .args(&args)
        .output()
        .map_err(|error| format!("run ripr {args:?}: {error}"))
}

#[test]
fn owned_sample_progress_retains_original_nonempty_input_and_quiet_parity() -> Result<(), String> {
    let fixture = OwnedProgressSample::new("original-sample-parity")?;
    let config = ripr::config::load_for_root(&fixture.root)?;
    let languages = config
        .languages
        .enabled
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>();
    assert!(
        config.source_path.is_none(),
        "owned sample must not inherit checkout config"
    );
    assert_eq!(
        languages,
        ["rust"],
        "sample parity requires built-in Rust defaults"
    );
    let diff = fixture.diff_arg();
    let loud = run_check_at(&fixture.root, Path::new(&diff), &["--format", "json"])?;
    let quiet = run_check_at(
        &fixture.root,
        Path::new(&diff),
        &["--format", "json", "--quiet"],
    )?;
    assert!(loud.status.success(), "{}", stderr_text(&loud));
    assert!(quiet.status.success(), "{}", stderr_text(&quiet));
    assert_eq!(
        loud.stdout, quiet.stdout,
        "original sample machine bytes must retain quiet parity"
    );
    let parsed: serde_json::Value = serde_json::from_slice(&loud.stdout)
        .map_err(|error| format!("owned sample stdout is not JSON: {error}"))?;
    assert_eq!(parsed["schema_version"], "0.2");
    assert!(
        !parsed["findings"]
            .as_array()
            .ok_or("sample findings missing")?
            .is_empty(),
        "original tracked sample bytes must still exercise actual analysis"
    );
    assert!(stderr_text(&loud).contains("ripr progress: loading_input [diff]"));
    assert!(!stderr_text(&quiet).contains("ripr progress:"));
    Ok(())
}

fn assert_fixture_json_findings(
    fixture: &DiffFixture,
    parsed: &serde_json::Value,
) -> Result<(), String> {
    let expected_file = fixture
        .0
        .join("src/lib.rs")
        .to_string_lossy()
        .replace('\\', "/")
        .replace('%', "%25");
    assert_eq!(
        parsed["summary"]["changed_rust_files"], 1,
        "fixture must analyze its one changed Rust file"
    );
    let findings = parsed["findings"]
        .as_array()
        .ok_or_else(|| "diff fixture must produce a findings array".to_string())?;
    assert!(
        findings.iter().any(|finding| {
            finding
                .pointer("/probe/file")
                .and_then(serde_json::Value::as_str)
                == Some(expected_file.as_str())
                && finding
                    .pointer("/probe/expression")
                    .and_then(serde_json::Value::as_str)
                    == Some("amount >= threshold")
        }),
        "real analyzer must report the fixture's changed predicate: {parsed}"
    );
    Ok(())
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout_has_progress_record(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.trim_start().starts_with("ripr progress:"))
}

#[test]
fn check_json_stdout_parses_while_progress_stays_on_stderr() -> Result<(), String> {
    let fixture = DiffFixture::new()?;
    let output = run_check(&fixture, &["--format", "json"])?;
    assert!(
        output.status.success(),
        "check json failed: {}",
        stderr_text(&output)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("stdout is not JSON: {error}"))?;
    assert_eq!(parsed["schema_version"], "0.2");
    assert!(
        !parsed["findings"]
            .as_array()
            .ok_or("sample findings missing")?
            .is_empty(),
        "owned sample diff must exercise actual analysis"
    );
    assert_fixture_json_findings(&fixture, &parsed)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = stderr_text(&output);
    assert!(
        !stdout.contains("ripr progress:"),
        "progress leaked onto stdout"
    );
    assert!(
        stderr.contains("ripr progress: loading_input [diff]"),
        "missing loading_input on stderr: {stderr}"
    );
    assert!(
        stderr.contains("ripr progress: analyzing [diff]"),
        "missing analyzing on stderr: {stderr}"
    );
    assert!(
        stderr.contains("ripr progress: completed [diff]"),
        "missing completed on stderr: {stderr}"
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "non-TTY stderr has ANSI: {stderr}"
    );
    assert!(!stderr.contains('\r'), "non-TTY stderr has CR: {stderr}");
    assert!(
        !stderr.contains('%'),
        "unknown totals leaked a percent: {stderr}"
    );
    assert!(
        !stderr.to_ascii_lowercase().contains("eta"),
        "unknown totals leaked an ETA: {stderr}"
    );
    Ok(())
}

#[test]
fn check_quiet_keeps_json_stdout_byte_identical_and_drops_progress() -> Result<(), String> {
    let fixture = DiffFixture::new()?;
    let loud = run_check(&fixture, &["--format", "json"])?;
    let quiet = run_check(&fixture, &["--format", "json", "--quiet"])?;
    assert!(loud.status.success(), "{}", stderr_text(&loud));
    assert!(quiet.status.success(), "{}", stderr_text(&quiet));
    assert_eq!(
        loud.stdout, quiet.stdout,
        "quiet must not change machine stdout"
    );
    let parsed: serde_json::Value = serde_json::from_slice(&loud.stdout)
        .map_err(|error| format!("stdout is not JSON: {error}"))?;
    assert_fixture_json_findings(&fixture, &parsed)?;
    let quiet_err = stderr_text(&quiet);
    assert!(
        !quiet_err.contains("ripr progress:"),
        "--quiet must suppress progress: {quiet_err}"
    );
    assert!(
        stderr_text(&loud).contains("ripr progress:"),
        "control run must have emitted progress"
    );
    Ok(())
}

#[test]
fn check_sarif_stdout_is_unchanged_by_progress() -> Result<(), String> {
    let fixture = DiffFixture::new()?;
    let loud = run_check(&fixture, &["--format", "sarif"])?;
    let quiet = run_check(&fixture, &["--format", "sarif", "--quiet"])?;
    assert!(loud.status.success(), "{}", stderr_text(&loud));
    assert!(quiet.status.success(), "{}", stderr_text(&quiet));
    assert_eq!(loud.stdout, quiet.stdout);
    let stdout = String::from_utf8_lossy(&loud.stdout);
    assert!(stdout.contains("\"version\": \"2.1.0\"") || stdout.contains("sarif"));
    assert!(!stdout.contains("ripr progress:"));
    assert!(stderr_text(&loud).contains("ripr progress:"));
    let parsed: serde_json::Value = serde_json::from_slice(&loud.stdout)
        .map_err(|error| format!("stdout is not SARIF JSON: {error}"))?;
    assert_eq!(parsed["version"], "2.1.0");
    let results = parsed
        .pointer("/runs/0/results")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "SARIF fixture must produce a results array".to_string())?;
    assert!(
        results.iter().any(|result| {
            result
                .pointer("/locations/0/physicalLocation/artifactLocation/uri")
                .and_then(serde_json::Value::as_str)
                == Some("src/lib.rs")
        }),
        "SARIF must contain a located finding for the changed fixture: {parsed}"
    );
    Ok(())
}

#[test]
fn check_progress_failure_emits_failed_not_completed() -> Result<(), String> {
    let fixture = OwnedProgressSample::new("check_progress_failure_emits_failed_not_completed")?;
    let root = fixture.root_arg();
    let missing = fixture.root.join("absent-progress.diff");
    assert!(!missing.exists());
    let missing_arg = missing.display().to_string();
    let output = ripr()
        .current_dir(&fixture.root)
        .args([
            "check",
            "--root",
            root.as_str(),
            "--diff",
            missing_arg.as_str(),
            "--format",
            "json",
        ])
        .output()
        .map_err(|error| format!("run failing check: {error}"))?;
    assert!(
        !output.status.success(),
        "missing diff must fail the command"
    );
    let stderr = stderr_text(&output);
    assert!(
        stderr.contains("ripr progress: failed [diff]"),
        "failure must project failed: {stderr}"
    );
    assert!(
        !stderr.contains("ripr progress: completed"),
        "failure must not project completed: {stderr}"
    );
    Ok(())
}

#[test]
fn check_help_does_not_spray_progress() -> Result<(), String> {
    let output = ripr()
        .args(["check", "--help"])
        .output()
        .map_err(|error| format!("run check help: {error}"))?;
    assert!(output.status.success());
    let stderr = stderr_text(&output);
    assert!(
        !stderr.contains("ripr progress:"),
        "help is a short command: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--quiet"));
    assert!(stdout.contains("percentage or ETA"));
    Ok(())
}

#[test]
fn check_github_stdout_is_unchanged_by_progress() -> Result<(), String> {
    machine_format_stdout_is_unchanged("github")
}

fn machine_format_stdout_is_unchanged(format: &str) -> Result<(), String> {
    let fixture = DiffFixture::new()?;
    let loud = run_check(&fixture, &["--format", format])?;
    let quiet = run_check(&fixture, &["--format", format, "--quiet"])?;
    assert!(loud.status.success(), "{}", stderr_text(&loud));
    assert!(quiet.status.success(), "{}", stderr_text(&quiet));
    assert_eq!(loud.stdout, quiet.stdout);
    let stdout = String::from_utf8_lossy(&loud.stdout);
    assert!(!stdout.contains("ripr progress:"));
    assert!(stderr_text(&loud).contains("ripr progress: loading_input [diff]"));
    assert!(
        stdout.lines().any(|line| {
            (line.starts_with("::warning ")
                || line.starts_with("::error ")
                || line.starts_with("::notice "))
                && line.contains("file=src/lib.rs,")
                && line.contains("title=ripr ")
        }),
        "GitHub must emit a finding annotation, not an empty-result notice: {stdout}"
    );
    Ok(())
}

#[test]
fn check_worktree_projects_worktree_scope_on_stderr() -> Result<(), String> {
    // Analyze the sample crate rather than this repository. A worktree scan of
    // ripr itself is slow, and its JSON findings can mention `ripr progress:`
    // as source text.
    let fixture = OwnedProgressSample::new("worktree")?;
    let root = fixture.sample_root_arg();
    let output = ripr()
        .current_dir(&fixture.root)
        .args([
            "check",
            "--root",
            root.as_str(),
            "--worktree",
            "--format",
            "json",
        ])
        .output()
        .map_err(|error| format!("run worktree check: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = stderr_text(&output);
    assert!(
        !stdout_has_progress_record(&stdout),
        "progress leaked onto stdout: {stdout}"
    );
    assert!(
        stderr.contains("ripr progress: loading_input [worktree]"),
        "missing worktree scope on stderr: {stderr}"
    );
    assert!(
        !stderr.contains("[diff]"),
        "worktree run must not project the diff scope: {stderr}"
    );
    if output.status.success() {
        assert!(
            stderr.contains("ripr progress: completed [worktree]"),
            "successful worktree check must commit completed: {stderr}"
        );
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("stdout is not JSON: {error}"))?;
        assert_eq!(parsed["schema_version"], "0.2");
    } else {
        assert!(
            stderr.contains("ripr progress: failed [worktree]"),
            "failed worktree check must project failed: {stderr}"
        );
        assert!(
            !stderr.contains("ripr progress: completed"),
            "worktree failure must not project completed: {stderr}"
        );
    }
    Ok(())
}

#[test]
fn check_quiet_failure_keeps_errors_and_drops_progress() -> Result<(), String> {
    let fixture = OwnedProgressSample::new("check_quiet_failure_keeps_errors_and_drops_progress")?;
    let root = fixture.root_arg();
    let missing = fixture.root.join("absent-progress.diff");
    assert!(!missing.exists());
    let missing_arg = missing.display().to_string();
    let output = ripr()
        .current_dir(&fixture.root)
        .args([
            "check",
            "--root",
            root.as_str(),
            "--diff",
            missing_arg.as_str(),
            "--format",
            "json",
            "--quiet",
        ])
        .output()
        .map_err(|error| format!("run quiet failing check: {error}"))?;
    assert!(
        !output.status.success(),
        "missing diff must fail under --quiet"
    );
    let stderr = stderr_text(&output);
    assert!(
        !stderr.contains("ripr progress:"),
        "--quiet must suppress progress on failure: {stderr}"
    );
    assert!(
        !stderr.trim().is_empty(),
        "--quiet must still report the command error"
    );
    Ok(())
}

#[test]
fn check_unwritable_artifact_projects_failed_not_completed() -> Result<(), String> {
    let fixture = DiffFixture::new()?;
    let dir = fixture.0.join("artifact-dir");
    std::fs::create_dir_all(&dir).map_err(|error| format!("create artifact dir: {error}"))?;
    let dir_arg = dir.display().to_string();
    let output = run_check(
        &fixture,
        &["--format", "json", "--write-artifact", dir_arg.as_str()],
    )?;
    assert!(
        !output.status.success(),
        "writing an artifact onto a directory must fail the command: {}",
        stderr_text(&output)
    );
    let stderr = stderr_text(&output);
    assert!(
        stderr.contains("ripr progress: building_output [diff]"),
        "artifact failure must follow real analysis: {stderr}"
    );
    assert!(
        stderr.contains("ripr progress: failed [diff]"),
        "post-analysis artifact failure must project failed: {stderr}"
    );
    assert!(
        !stderr.contains("ripr progress: completed"),
        "post-analysis artifact failure must not project completed: {stderr}"
    );
    Ok(())
}

// --- #4945: repo-scoped audit-path formats disclose progress and cost ---

/// Small single-crate fixture workspace for repo-format runs. Repo formats
/// analyze the live tree and may write the seam-facts cache under the root,
/// so these tests never point the binary at this repository or at a fixture
/// inside the checkout.
fn repo_fixture_root(tag: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-cli-progress-{tag}-{stamp}"));
    std::fs::create_dir_all(root.join("src")).map_err(|error| format!("create src: {error}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname=\"ripr-cli-progress-{tag}\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\n[workspace]\n"
        ),
    )
    .map_err(|error| format!("write Cargo.toml: {error}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|error| format!("write src/lib.rs: {error}"))?;
    Ok(root)
}

fn ignore_remove_dir_all(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn run_repo_format(root: &Path, format: &str, extra: &[&str]) -> Result<Output, String> {
    let root_arg = root.display().to_string();
    let mut args = vec!["check", "--root", root_arg.as_str(), "--format", format];
    args.extend_from_slice(extra);
    ripr()
        .args(&args)
        .output()
        .map_err(|error| format!("run ripr check --format {format}: {error}"))
}

#[test]
fn repo_format_audit_path_progress_reaches_stderr_and_stdout_stays_clean() -> Result<(), String> {
    let root = repo_fixture_root("seams")?;
    let output = run_repo_format(&root, "repo-seams-json", &[])?;
    assert!(
        output.status.success(),
        "repo-seams-json must succeed on the fixture: {}",
        stderr_text(&output)
    );
    let stderr = stderr_text(&output);
    // #4945: invocation-time cost disclosure naming the audit-path class.
    assert!(
        stderr.contains("full-repo audit path"),
        "missing audit-path cost disclosure: {stderr}"
    );
    assert!(
        stderr.contains("repo-seams-json"),
        "disclosure must name the invoked format: {stderr}"
    );
    // The repo walk now projects the same producer stages as the diff path.
    assert!(
        stderr.contains("ripr progress: analyzing [repo]"),
        "missing repo-scope analyzing stage: {stderr}"
    );
    assert!(
        stderr.contains("ripr progress: completed [repo]"),
        "missing repo-scope completed stage: {stderr}"
    );
    assert!(
        !stderr.contains("[diff]"),
        "repo run must not project the diff scope: {stderr}"
    );
    // Stdout cleanliness pin: the artifact is the ONLY stdout content.
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("stdout is not pure JSON: {error}"))?;
    assert_eq!(parsed["scope"], "repo");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("ripr progress:"),
        "progress leaked onto stdout: {stdout}"
    );
    assert!(
        !stdout.contains("audit path"),
        "disclosure leaked onto stdout: {stdout}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn repo_format_quiet_keeps_stdout_byte_identical_and_drops_progress_stages() -> Result<(), String> {
    let root = repo_fixture_root("seams-quiet")?;
    let loud = run_repo_format(&root, "repo-seams-json", &[])?;
    let quiet = run_repo_format(&root, "repo-seams-json", &["--quiet"])?;
    assert!(loud.status.success(), "{}", stderr_text(&loud));
    assert!(quiet.status.success(), "{}", stderr_text(&quiet));
    // Byte-shape pin: the disclosure/progress work must not move stdout.
    assert_eq!(
        loud.stdout, quiet.stdout,
        "--quiet must not change machine stdout"
    );
    let quiet_err = stderr_text(&quiet);
    assert!(
        !quiet_err.contains("ripr progress:"),
        "--quiet must suppress progress stages: {quiet_err}"
    );
    // The cost disclosure is an invocation-time advisory like the repo-scope
    // --base/--diff warning, not part of the progress stream, so --quiet
    // keeps it.
    assert!(
        quiet_err.contains("full-repo audit path"),
        "--quiet must keep the cost disclosure: {quiet_err}"
    );
    assert!(
        stderr_text(&loud).contains("ripr progress: analyzing [repo]"),
        "loud control must project repo stages"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

#[test]
fn repo_format_disclosure_is_absent_outside_the_audit_path_group() -> Result<(), String> {
    let root = repo_fixture_root("controls")?;
    // Diff-scoped machine format: no audit-path claim. The scope is an
    // explicit --diff so the run completes regardless of git availability;
    // either way the disclosure must never fire for this format.
    let diff = root.join("control.diff");
    std::fs::write(
        &diff,
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n+pub fn over_threshold(amount: i32, threshold: i32, margin: i32) -> bool {\n",
    )
    .map_err(|error| format!("write control diff: {error}"))?;
    let diff_arg = diff.display().to_string();
    let diff_json = run_repo_format(&root, "json", &["--diff", diff_arg.as_str()])?;
    assert!(
        !stderr_text(&diff_json).contains("audit path"),
        "diff-scoped json must not claim audit-path cost: {}",
        stderr_text(&diff_json)
    );
    // Repo badge surface: since #5261 its canonical_actionable_gap count
    // derives from the same full classified walk repo-exposure renders, so
    // it joined the audit-path group and must disclose the cost class it
    // now charges. The diff-scoped control above stays outside the group.
    let badge = run_repo_format(&root, "repo-badge-json", &[])?;
    assert!(badge.status.success(), "{}", stderr_text(&badge));
    let badge_err = stderr_text(&badge);
    assert!(
        badge_err.contains("full-repo audit path"),
        "repo-badge-json must disclose audit-path cost now that it walks the          classified inventory: {badge_err}"
    );
    assert!(
        badge_err.contains("warm reruns reuse the seam-facts cache"),
        "cache-backed badge disclosure must claim warm reruns: {badge_err}"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}

// --- #5019: pilot projects the same producer-owned repo-scope progress ---

fn run_pilot(root: &Path, out: &Path, extra: &[&str]) -> Result<Output, String> {
    let root_arg = root.display().to_string();
    let out_arg = out.display().to_string();
    let mut args = vec![
        "pilot",
        "--root",
        root_arg.as_str(),
        "--out",
        out_arg.as_str(),
    ];
    args.extend_from_slice(extra);
    ripr()
        .args(&args)
        .output()
        .map_err(|error| format!("run ripr pilot: {error}"))
}

#[test]
fn pilot_projects_repo_stages_on_stderr_and_keeps_packet_bytes_unchanged() -> Result<(), String> {
    let root = repo_fixture_root("pilot-progress")?;
    let out = root.join("pilot-out");
    let loud = run_pilot(&root, &out, &[])?;
    assert!(
        loud.status.success(),
        "pilot must succeed on the fixture: {}",
        stderr_text(&loud)
    );
    let loud_err = stderr_text(&loud);
    // Pilot analyzes the whole repo inventory, so the progress scope is
    // repo, exactly as on the check repo-format audit path.
    assert!(
        loud_err.contains("ripr progress: loading_input [repo]"),
        "missing repo-scope loading_input on stderr: {loud_err}"
    );
    assert!(
        loud_err.contains("ripr progress: analyzing [repo]"),
        "missing repo-scope analyzing on stderr: {loud_err}"
    );
    assert!(
        loud_err.contains("ripr progress: completed [repo]"),
        "missing repo-scope completed on stderr: {loud_err}"
    );
    assert!(
        !loud_err.contains("[diff]"),
        "pilot must not project the diff scope: {loud_err}"
    );
    assert!(
        !loud_err.contains('\u{1b}'),
        "non-TTY pilot stderr has ANSI: {loud_err}"
    );
    assert!(
        !loud_err.contains('\r'),
        "non-TTY pilot stderr has CR: {loud_err}"
    );
    let loud_stdout = String::from_utf8_lossy(&loud.stdout);
    assert!(
        !loud_stdout.contains("ripr progress:"),
        "progress leaked onto pilot stdout: {loud_stdout}"
    );

    // Machine contract pin: the progress stream must not move the packet.
    let summary_path = out.join("pilot-summary.json");
    let loud_summary = std::fs::read(&summary_path)
        .map_err(|error| format!("read loud pilot-summary.json: {error}"))?;
    let parsed: serde_json::Value = serde_json::from_slice(&loud_summary)
        .map_err(|error| format!("pilot-summary.json is not JSON: {error}"))?;
    assert_eq!(parsed["schema_version"], "0.2");

    // Removal experiment (#2608 closure rule): --quiet drops every progress
    // line while the emitted packet stays byte-identical.
    let quiet = run_pilot(&root, &out, &["--quiet"])?;
    assert!(
        quiet.status.success(),
        "quiet pilot must succeed on the fixture: {}",
        stderr_text(&quiet)
    );
    let quiet_err = stderr_text(&quiet);
    assert!(
        !quiet_err.contains("ripr progress:"),
        "--quiet must suppress pilot progress: {quiet_err}"
    );
    assert_eq!(
        loud.stdout, quiet.stdout,
        "--quiet must not change pilot terminal stdout"
    );
    let quiet_summary = std::fs::read(&summary_path)
        .map_err(|error| format!("read quiet pilot-summary.json: {error}"))?;
    assert_eq!(
        loud_summary, quiet_summary,
        "progress wiring must not change pilot-summary.json bytes"
    );
    ignore_remove_dir_all(&root);
    Ok(())
}
