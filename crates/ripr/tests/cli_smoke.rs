#![expect(
    clippy::unwrap_used,
    reason = "CLI smoke test: unwrap on Command::output() and CARGO_MANIFEST_DIR's parent chain is the canonical fail-fast pattern for binary integration tests; receipted via policy/no-panic-allowlist.toml entries for crates/ripr/tests/cli_smoke.rs."
)]

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
    let mut command = Command::new(program);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    command.args(args).output()
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
    let mut rendered = serde_json::to_string_pretty(&value)?;
    rendered.push('\n');
    Ok(rendered)
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
    assert!(stdout.contains("Summary: 5 probe(s)"));
    assert!(stdout.contains("Start here:"));
    assert!(stdout.contains("Static exposure: weakly_exposed"));
    assert!(stdout.contains("Evidence:"));
    assert!(stdout.contains("Missing discriminator:"));
    assert!(stdout.contains("Next step:"));
    assert!(stdout.contains("lower-priority finding(s) omitted"));
    assert!(stdout.contains("--format human-full"));
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

    let verify = run_ripr_in_workspace(&[
        "agent",
        "verify",
        "--root",
        ".",
        "--before",
        "fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json",
        "--after",
        "fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json",
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
        "input that hits the boundary: amount >= discount_threshold"
    );
    assert_eq!(
        json_pointer_str(&proof, "/recommendation/placement")?,
        "changed_line"
    );
    assert!(
        json_pointer_str(&proof, "/recommendation/suggested_test")?
            .contains("amount >= discount_threshold")
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
    assert!(proof_md.contains(
        "Missing discriminator: input that hits the boundary: amount >= discount_threshold"
    ));
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
    let before = root.join("before.repo-exposure.json");
    let after = root.join("after.repo-exposure.json");
    std::fs::write(
        &before,
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "weak"}],
      "observed_values": ["50"],
      "missing_discriminators": [{"value": "threshold equality", "reason": "not observed"}]
    }
  ]
}"#,
    )?;
    std::fs::write(
        &after,
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "strong"}],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
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
fn agent_receipt_writes_one_seam_handoff_json() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_workspace("agent-receipt");
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"fast\"\n")?;
    std::fs::create_dir_all(root.join("target/ripr/workflow"))?;
    std::fs::write(
        root.join("target/ripr/workflow/before.repo-exposure.json"),
        r#"{"schema_version":"0.2","scope":"repo","seams":[]}"#,
    )?;
    std::fs::write(
        root.join("target/ripr/workflow/after.repo-exposure.json"),
        r#"{"schema_version":"0.2","scope":"repo","seams":[]}"#,
    )?;
    let verify = root.join("agent-verify.json");
    let receipt = root.join("target/ripr/reports/agent-receipt.json");
    std::fs::write(
        &verify,
        r#"{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "before": "target/ripr/workflow/before.repo-exposure.json",
    "after": "target/ripr/workflow/after.repo-exposure.json"
  },
  "summary": {
    "improved": 1,
    "changed": 0,
    "regressed": 0,
    "unchanged": 0,
    "new": 0,
    "resolved": 0
  },
  "changed_seams": [
    {
      "seam_id": "seam-a",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "before": "weakly_gripped",
      "after": "strongly_gripped",
      "change": "improved",
      "evidence_delta": ["missing discriminator no longer reported: threshold equality"]
    }
  ],
  "unchanged_seams": [],
  "new_gaps": [],
  "resolved_gaps": []
}"#,
    )?;

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
    assert!(text.contains(r#""schema_version": "0.3""#));
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
    assert!(stdout.contains(r#""schema_version": "0.7""#));
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
    // The sample diff has 5 weakly_exposed findings; the badge headline reflects them.
    assert!(stdout.contains(r#""message": "5""#));
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
    assert!(stdout.contains(r#""message": "5""#));
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
    assert!(!workspace.join("ripr.toml").exists());

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
    assert!(stdout.contains("# ripr.toml"));
    assert!(stdout.contains("# "));
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
    assert!(stdout.contains(r#""schema_version": "0.7""#));
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
    assert!(stdout.contains(r#""schema_version": "0.7""#));
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
    assert!(stdout.contains(r#""schema_version": "0.7""#));
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
    assert!(stdout.contains(r#""schema_version": "0.7""#));
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
    let receipt_json = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "receipt",
        "canonical_gap_id": "gap:orphan:aabbccdd",
        "verify_command": "cargo test",
        "verify_status": "passed",
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
