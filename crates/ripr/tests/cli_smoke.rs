#![expect(
    clippy::unwrap_used,
    reason = "CLI smoke test: unwrap on Command::output() and CARGO_MANIFEST_DIR's parent chain is the canonical fail-fast pattern for binary integration tests; receipted via policy/no-panic-allowlist.toml entries for crates/ripr/tests/cli_smoke.rs."
)]

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

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

fn run_command(
    program: &str,
    current_dir: Option<&Path>,
    args: &[&str],
) -> Result<Output, std::io::Error> {
    spawn_command(program, current_dir, args, &[])
}

/// The single process spawn point for this harness. Both `run_command` and
/// `run_command_with_env` route through here so the suite keeps one tracked
/// spawn site rather than one per calling convention.
fn spawn_command(
    program: &str,
    current_dir: Option<&Path>,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Output, std::io::Error> {
    let mut command = Command::new(program);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.args(args).output()
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
    spawn_command(program, Some(current_dir), args, env)
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = run_command("git", Some(root), args)
        .map_err(|err| format!("failed to run git {args:?}: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
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

fn assert_stdout_matches_fixture(
    output: &Output,
    fixture_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_success(output);
    let expected = std::fs::read_to_string(workspace_root().join(fixture_path))?;
    let actual = String::from_utf8(output.stdout.clone())?;
    assert_eq!(
        normalize_newlines(&actual),
        normalize_newlines(&expected),
        "stdout drifted from {fixture_path}"
    );
    Ok(())
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
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
    "producer": {{"tool": "ripr", "version": "0.10.0"}},
    "repository": {{"root": "{root_identity}", "head": "{head}"}},
    "analysis": {{"format": "repo-exposure-json", "mode": "draft", "base_revision": null, "input_identity": "input:fixture", "command": "ripr check --format repo-exposure-json", "profile": "draft", "worktree": "clean"}},
    "snapshot_identity": "snapshot:input:fixture",
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
  "schema_version": "0.1",
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
        "producer": {"tool": "ripr", "version": "0.10.0"},
        "repository": {"root": root_identity, "head": head},
        "analysis": {"format": "repo-exposure-json", "mode": "draft", "base_revision": null, "input_identity": "input:fixture", "command": "ripr check --format repo-exposure-json", "profile": "draft", "worktree": worktree},
        "snapshot_identity": "snapshot:input:fixture",
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
    let mut rendered = serde_json::to_string_pretty(&value)?;
    rendered.push('\n');
    Ok(rendered)
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

#[test]
fn version_runs() {
    let output = run_ripr(&["--version"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr"));
}

#[test]
fn help_runs() {
    let output = run_ripr(&["--help"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("find changed Rust code where nearby tests"));
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
    assert!(stdout.contains("Static exposure: weakly_exposed"));
    assert!(stdout.contains("Evidence:"));
    assert!(stdout.contains("Missing discriminator:"));
    assert!(stdout.contains("Next step:"));
    assert!(stdout.contains("lower-priority finding(s) omitted"));
    assert!(stdout.contains("--format human-full"));
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
  "summary": {"unchanged_tests": true},
  "comments": [],
  "summary_only": [],
  "suppressed": []
}"#;

const SMOKE_COMPLETE_GAP_LEDGER_JSON: &str = r#"{
  "gap_records": [
    {
      "gap_id": "gap:pricing",
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert_eq!(
        json_pointer_str(&report, "/commands/verify")?,
        "ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
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
    assert!(stdout.contains("cost class:      varies with diff and workspace size"));
    assert!(stdout.contains(&format!("writes to:       {reports_arg}/")));
    assert!(stdout.contains("cache location:  target/ripr/cache/"));
    assert!(stdout.contains("git reads:       yes (diff between base and head)"));
    assert!(stdout.contains("network:         none"));
    assert!(stdout.contains("Start here:"));
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
    assert!(stdout.contains("Verify command: `cargo xtask fixtures boundary_gap`"));
    assert!(stdout.contains("Receipt command: `ripr receipt write --gap "));
    assert!(stdout.contains("Receipt path: `target/ripr/receipts/"));
    assert!(stdout.contains("Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval."));

    let json_path = reports.join("start-here.json");
    let md_path = reports.join("start-here.md");
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json_path)?)?;
    assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
    assert_eq!(json_pointer_str(&report, "/kind")?, "first_pr_start_here");
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
    assert!(markdown.contains("- Receipt command: `ripr receipt write --gap "));
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
    assert!(packet_stdout.contains(r#""schema_version": "0.3""#));
    assert!(packet_stdout.contains(r#""packets_total": 1"#));
    assert!(packet_stdout.contains(&format!(r#""seam_id": "{seam_id}""#)));
    assert!(packet_stdout.contains(r#""task": "write_targeted_test""#));
    std::fs::remove_dir_all(root)?;
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
    assert_stdout_matches_fixture(&packet, &format!("{base}/agent-packet.json"))?;

    let brief = run_ripr_in_workspace(&[
        "agent",
        "brief",
        "--root",
        "fixtures/boundary_gap/input",
        "--seam-id",
        seam_id,
        "--json",
    ])?;
    assert_stdout_matches_fixture(&brief, &format!("{base}/agent-brief.json"))?;

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
    assert_stdout_matches_fixture(&verify, &format!("{base}/agent-verify.json"))?;

    let out_dir = unique_temp_workspace("agent-receipt-fixture");
    std::fs::create_dir_all(&out_dir)?;
    let receipt_path = out_dir.join("agent-receipt.json");
    let receipt = run_ripr_in_workspace(&[
        "agent",
        "receipt",
        "--root",
        ".",
        "--verify-json",
        "fixtures/boundary_gap/expected/editor-agent-loop/agent-verify.json",
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
    assert_eq!(
        normalize_agent_receipt_fixture(&actual_receipt)?,
        normalize_agent_receipt_fixture(&expected_receipt)?,
        "agent receipt fixture drifted"
    );
    std::fs::remove_dir_all(out_dir)?;
    std::fs::remove_dir_all(artifact_dir)?;
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
        "unchanged"
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
    assert!(proof_md.contains("After: weakly_gripped"));
    assert!(proof_md.contains("State: unchanged"));
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
        (
            "unchanged-after-attempt",
            "unchanged_after_attempt",
            "unchanged_after_attempt",
            "revise_focused_test",
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

        let report_path = fixture_dir.join(case_dir).join("first-useful-action.json");
        let markdown_path = fixture_dir.join(case_dir).join("first-useful-action.md");
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(report_path)?)?;
        assert_eq!(json_pointer_str(&report, "/schema_version")?, "0.1");
        assert_eq!(json_pointer_str(&report, "/tool")?, "ripr");
        assert_eq!(json_pointer_str(&report, "/kind")?, "first_useful_action");
        assert_eq!(json_pointer_str(&report, "/status")?, status);
        assert_eq!(json_pointer_str(&report, "/action_kind")?, action_kind);
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
    assert!(workflow_json.contains("ripr agent verify --root fixtures/boundary_gap/input"));
    assert!(commands_md.contains("# RIPR Agent Workflow"));
    assert!(commands_md.contains("Does not edit source files."));
    assert!(commands_md.contains("Does not call an LLM API."));
    assert!(agent_brief_json.contains(seam_id));

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
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--phase",
        "before",
    ]);
    assert_success(&before);

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

    let after = run_ripr(&[
        "agent",
        "repair",
        "--root",
        &root_arg,
        "--seam-id",
        "67fc764ba37d77bd",
        "--phase",
        "after",
    ]);
    assert_success(&after);
    let after_snapshot_text = std::fs::read_to_string(&after_snapshot)?;
    assert!(!after_snapshot_text.contains("previous repair run"));

    let verify_json = root.join("target/ripr/workflow/agent-verify.json");
    assert!(verify_json.is_file());
    let verify: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(verify_json)?)?;
    assert_eq!(verify["tool"], "ripr");
    let receipt_path = root.join("target/ripr/reports/agent-receipt.json");
    let receipt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["provenance"]["seam_id"], "67fc764ba37d77bd");
    assert!(String::from_utf8_lossy(&after.stdout).contains("\"status\": \"complete\""));
    assert!(String::from_utf8_lossy(&after.stderr).contains("after phase complete"));

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
    assert!(stdout.contains(r#""schema_version": "0.1""#));
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
    let altered = std::fs::read_to_string(&after)?
        .replace("input:fixture", "input:other")
        .replace("snapshot:input:fixture", "snapshot:input:other");
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
    std::fs::remove_dir_all(root)?;
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("historical_noncurrent"));
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("dirty_worktree"));
    std::fs::remove_dir_all(root)?;
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
    assert!(text.contains(r#""schema_version": "0.4""#));
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
    assert!(text.contains(r#""safe_to_merge": false"#));
    assert!(text.contains(r#""test_changed": "pricing_boundary""#));
    assert!(text.contains(r#""cargo test pricing_boundary""#));
    std::fs::remove_dir_all(root)?;
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
    let altered = std::fs::read_to_string(&after)?.replace(
        "\"input_identity\": \"input:fixture\"",
        "\"input_identity\": \"input:other\"",
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("[incomparable_analysis_inputs]"));
    std::fs::remove_dir_all(root)?;
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
    assert!(stdout.contains("Safe next action: run `ripr first-pr --root"));
    assert!(stdout.contains("Recovery states: missing artifact, stale evidence, wrong root"));
    assert!(stdout.contains("Proof rail: verify command, receipt command, and receipt path"));

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert_eq!(report["schema_version"], "0.2");
    assert_eq!(report["tool"], "ripr");
    std::fs::remove_dir_all(workspace).map_err(|err| format!("remove workspace: {err}"))?;
    Ok(())
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
        stdout.contains("ripr check --base origin/main"),
        "expected the diff-first recommended command in stdout:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert!(
        clean_out.contains("Recommended first command: ripr check --base origin/main"),
        "clean worktree must recommend the diff-first command directly:\n{clean_out}"
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
    assert!(
        dirty_out.contains("Recommended first command: ripr check --base HEAD --worktree"),
        "dirty worktree must recommend the worktree command:\n{dirty_out}"
    );
    assert!(
        dirty_out.contains("staged and unstaged tracked edits"),
        "dirty worktree must disclose the tracked-edit scope:\n{dirty_out}"
    );
    assert!(
        !dirty_out.contains("Recommended first command: ripr check --base origin/main"),
        "dirty worktree must NOT give the unconditional clean recommendation:\n{dirty_out}"
    );

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert!(stdout.contains(&format!("# {}", workspace.join("ripr.toml").display())));
    assert!(stdout.contains(&format!(
        "# {}",
        workspace.join(".github/workflows/ripr.yml").display()
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
    assert!(stdout.contains("ripr agent verify"));
    assert!(stdout.contains("ripr agent receipt"));
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert!(workflow.contains("cargo install ripr --locked"));
    assert!(workflow.contains("ripr pilot"));
    assert!(workflow.contains("--format sarif"));
    assert!(workflow.contains("--format repo-sarif"));
    assert!(workflow.contains("--format repo-badge-json"));
    assert!(workflow.contains("ripr agent start"));
    assert!(workflow.contains("ripr agent packet"));
    assert!(workflow.contains("ripr agent verify"));
    assert!(workflow.contains("ripr agent receipt"));
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
    assert!(workflow.contains("target/ripr/agent/agent-verify.json"));
    assert!(workflow.contains("target/ripr/agent/agent-receipt.json"));
    assert!(workflow.contains("target/ripr/reports/targeted-test-outcome.json"));
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
    assert!(workflow.contains("cargo xtask operator-cockpit"));
    assert!(workflow.contains("continue-on-error: true"));
    assert!(workflow.contains("actions/upload-artifact@v7"));
    assert!(workflow.contains("RIPR_UPLOAD_SARIF"));
    assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
    assert!(!workflow.contains("fail-on-new-warning"));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"acknowledgeable\""));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"baseline-check\""));
    assert!(!workflow.contains("RIPR_GATE_MODE: \"calibrated-gate\""));

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    assert!(stdout.contains("Run after adding the focused test:"));

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

    let _ = std::fs::remove_dir_all(&out_dir);
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
    let _ = std::fs::remove_dir_all(&workspace);
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
    let _ = std::fs::remove_dir_all(&workspace);
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
    let _ = std::fs::remove_dir_all(&workspace);
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
    let _ = std::fs::remove_file(&before_path);
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

    let _ = std::fs::remove_dir_all(&ledger_dir);
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

    let _ = std::fs::remove_dir_all(&out_dir);
    Ok(())
}

#[test]
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
        "verify: pytest tests/test_pricing.py::test_calculate_discount_smoke",
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
        r#""verify_command": "pytest tests/test_pricing.py::test_calculate_discount_smoke""#,
    ] {
        assert!(
            summary_json.contains(needle),
            "missing summary JSON needle: {needle}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out_dir);
    Ok(())
}

#[test]
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

    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&workspace);
    Ok(())
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&out_dir);
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
    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
        assert_eq!(
            fresh_context.stdout, reused_context.stdout,
            "context --from output must be byte-identical to the fresh run"
        );
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
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
    let _ = std::fs::remove_dir_all(&dir);
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
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&dir);
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
    assert_eq!(
        fresh.stdout, reused.stdout,
        "context --from --no-unchanged-tests must be byte-identical to the fresh run"
    );

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
    let _ = std::fs::remove_dir_all(&dir);
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
    let _ = std::fs::remove_dir_all(&dir);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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
    // Empty unified diff: no findings, no changed owners, no related
    // tests. The unrelated TE entry must therefore be filtered out
    // under diff scope.
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
        "badge-plus-json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""scope": "diff""#));
    assert!(
        stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#),
        "diff-scope `ripr+` must filter out unrelated repo-wide TE debt: {stdout}"
    );
    // The headline must reflect the filter: no exposure gaps (empty
    // diff) and no unrelated TE debt = 0.
    assert!(stdout.contains(r#""message": "0""#));

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&workspace);
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

    let _ = std::fs::remove_dir_all(&out_dir);
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

    let _ = std::fs::remove_dir_all(&out_dir);
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

    let _ = std::fs::remove_dir_all(&out_dir);
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

/// Bug 1 regression guard: `ripr check --mode fast` with no --diff/--base must
/// show the no-scope disclosure. --mode is a speed tier on the diff path; it does
/// NOT provide analysis scope. Before the fix, the --mode arm set
/// scope_explicitly_provided = true, suppressing the disclosure.
#[test]
fn check_mode_fast_alone_shows_no_scope_disclosure_smoke() {
    // Run in a temp git repo with one commit and HEAD up-to-date, so
    // resolve_default_base succeeds but the diff against HEAD is empty.
    // With --mode fast and no --diff/--base, the result is empty and the
    // no-scope disclosure must fire on stdout (Bug 1 regression guard).
    // Before the fix, the --mode arm set scope_explicitly_provided = true,
    // suppressing the disclosure.
    let root = unique_temp_workspace("mode-fast-no-scope");
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
        "[package]\nname = \"mode-fast-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    run_git(&root, &["add", "."]).unwrap();
    run_git(&root, &["commit", "-m", "initial"]).unwrap();
    // HEAD is now up-to-date — diff against HEAD is empty.
    // With --mode fast and no --diff/--base, scope_explicitly_provided is false,
    // so the no-scope disclosure must fire.
    let bin = env!("CARGO_BIN_EXE_ripr");
    let root_str = root.to_string_lossy().into_owned();
    let output = std::process::Command::new(bin)
        .args(["check", "--root", &root_str, "--mode", "fast"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The output must contain the no-scope disclosure note, not be silently empty.
    assert!(
        stdout.contains("no analysis scope was provided"),
        "check --mode fast alone must show no-scope disclosure (Bug 1); got stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("--format repo-exposure-md"),
        "no-scope guidance must recommend --format repo-exposure-md (Bug 2); got:\n{stdout}"
    );
    assert!(
        !stdout.contains("--mode fast"),
        "no-scope guidance must NOT recommend --mode fast; got:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
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
        human.contains("uncommitted changes to tracked source were not analyzed"),
        "check --base HEAD with uncommitted edit must show Note in human output; got:\n{human}"
    );
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
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

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

// ── ripr pr-summary (Campaign 31 item 8: binary-first downstream CI) ──

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
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
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
    init_git_fixture_repo(&root)?;
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
    std::fs::write(root.join("after.json"), &snapshot.stdout)?;

    let verify = "ripr agent verify --root . --before before.json --after after.json --json";
    let verify_spec = serde_json::json!({
        "schema_version": "1",
        "command_id": "ripr:agent:verify",
        "role": "verify",
        "execution_mode": "direct",
        "program": "ripr",
        "args": ["agent", "verify", "--root", ".", "--before", "before.json", "--after", "after.json", "--json"],
        "cwd": ".",
        "env_set": [],
        "env_passthrough": [],
        "environment": "clean",
        "stdin": "null",
        "timeout_ms": 120000,
        "cancellation": "allowed",
        "network": "forbidden",
        "expected_result_parser": "declared_json",
        "expected_exit_codes": [0],
        "expected_writes": [],
        "cost_class": "unknown",
        "platforms": ["linux", "macos", "windows"],
        "human_display": verify,
        "authority_boundary": "verification_route_only"
    });
    let receipt =
        "ripr agent receipt --root . --verify-json verify.json --seam-id gap-verify-execute --json";
    let receipt_spec = serde_json::json!({
        "schema_version": "1",
        "command_id": "ripr:agent:receipt",
        "role": "receipt",
        "execution_mode": "direct",
        "program": "ripr",
        "args": ["agent", "receipt", "--root", ".", "--verify-json", "verify.json", "--seam-id", "gap-verify-execute", "--json"],
        "cwd": ".",
        "env_set": [],
        "env_passthrough": [],
        "environment": "clean",
        "stdin": "null",
        "timeout_ms": 120000,
        "cancellation": "allowed",
        "network": "forbidden",
        "expected_result_parser": "declared_json",
        "expected_exit_codes": [0],
        "expected_writes": [],
        "cost_class": "unknown",
        "platforms": ["linux", "macos", "windows"],
        "human_display": receipt,
        "authority_boundary": "receipt_route_only"
    });
    let ledger = serde_json::json!({
        "kind": "gap_decision_ledger",
        "root": ".",
        "records": [{
            "gap_id": "gap-verify-execute",
            "canonical_gap_id": "gap-verify-execute",
            "kind": "MissingBoundaryAssertion",
            "language": "rust",
            "language_status": "actionable",
            "scope": "pr_local",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "projection_eligibility": {"agent_packet": {"eligible": true, "reason": ""}},
            "anchor": {"file": "marker.txt", "line": 1, "owner": "marker"},
            "repair_route": {
                "route_kind": "StrengthenExistingTest",
                "target_file": "tests/marker.rs",
                "related_test": "marker_boundary",
                "missing_discriminator": "x == y",
                "assertion_shape": "assert_eq!(x, y)",
                "changed_behavior": "if x >= y"
            },
            "verification_commands": [verify],
            "receipt_command": receipt,
            "command_specs": {"verify": [verify_spec], "receipt": [receipt_spec]},
            "evidence_ids": ["probe:marker"]
        }]
    });
    let ledger_path = root.join("gap-ledger.json");
    std::fs::write(&ledger_path, serde_json::to_vec_pretty(&ledger)?)?;
    let ledger_arg = ledger_path.to_string_lossy().into_owned();

    let packet = run_ripr(&[
        "agent",
        "packet",
        "--root",
        &root_arg,
        "--gap-ledger",
        &ledger_arg,
        "--gap-id",
        "gap-verify-execute",
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
) -> Result<(serde_json::Value, bool), Box<dyn std::error::Error>> {
    let output = run_command(env!("CARGO_BIN_EXE_ripr"), Some(root), argv)?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "verify-execute must always emit typed JSON on stdout: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })?;
    Ok((parsed, output.status.success()))
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
    let escaped = "ripr agent verify --root . --before ../escape.json --after after.json --json";
    let mut outside = packet.clone();
    outside["packets"][0]["verify_command"] = serde_json::Value::String(escaped.to_string());
    outside["packets"][0]["verification_commands"] = serde_json::json!([escaped]);
    outside["packets"][0]["command_specs"]["verify"][0]["args"][5] =
        serde_json::Value::String("../escape.json".to_string());
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
        let (parsed, succeeded) = verify_execute_disposition(&root, &argv)?;
        assert_eq!(parsed["disposition"], expected, "{label}: {parsed}");
        assert_eq!(parsed["executed"], false, "{label} must not execute");
        assert_eq!(parsed["result_committed"], false, "{label}");
        assert!(!succeeded, "{label} must exit nonzero");
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
    let (parsed, succeeded) = verify_execute_disposition(
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
    assert!(!succeeded, "an uncommitted result must exit nonzero");
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
    std::fs::write(root.join("after.json"), "not-a-repo-exposure-artifact")?;
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

    let (parsed, succeeded) = verify_execute_disposition(
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
    assert!(!succeeded);
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
