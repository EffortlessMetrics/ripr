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
    assert!(stdout.contains("Static exposure\n  weakly_exposed"));
    assert!(stdout.contains("Evidence\n"));
    assert!(stdout.contains("observed function argument value"));
    assert!(stdout.contains("missing discriminator"));
    assert!(stdout.contains("Next step\n"));
}

#[test]
fn check_json_output_has_stable_contract_fields() {
    let root = workspace_root().display().to_string();
    let diff = sample_diff().display().to_string();
    let output = run_ripr(&["check", "--root", &root, "--diff", &diff, "--json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.1""#));
    assert!(stdout.contains(r#""classification": "weakly_exposed""#));
    assert!(stdout.contains(r#""evidence_path""#));
    assert!(stdout.contains(r#""flow_sinks""#));
    assert!(stdout.contains(r#""activation""#));
    assert!(stdout.contains(r#""missing_discriminators""#));
    assert!(stdout.contains(r#""oracle_kind""#));
    assert!(stdout.contains(r#""recommended_next_step""#));
    assert!(stdout.contains(r#""suggested_next_action""#));
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
    assert!(stdout.contains("Receipt command: `ripr outcome --before"));
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
    assert!(markdown.contains("- Receipt command: `ripr outcome --before"));
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
    assert!(text.contains(r#""ripr_version": "0.8.0""#));
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
    assert!(stdout.contains(r#""schema_version": "0.5""#));
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
    assert!(json.contains("\"matched_by\": \"seam_id\""));
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
    assert!(stdout.contains(r#""schema_version": "0.5""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(stdout.contains(r#""counts""#));
    assert!(stdout.contains(r#""reason_counts""#));
    assert!(stdout.contains(r#""policy""#));
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
    assert!(stdout.contains(r#""schema_version": "0.5""#));
    assert!(stdout.contains(r#""kind": "ripr""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(
        !stdout.contains(r#""scope": "diff""#),
        "repo scope output must not also carry diff scope: {stdout}"
    );
    assert!(stdout.contains(r#""label": "ripr""#));
    assert!(stdout.contains(r#""counts""#));

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
    assert!(stdout.contains(r#""schema_version": "0.5""#));
    assert!(stdout.contains(r#""basis": "gap_decision_ledger""#));
    assert!(stdout.contains(r#""message": "1""#));
    assert!(stdout.contains(r#""analyzed_gap_records": 2"#));

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
    assert!(stdout.contains(r#""schema_version": "0.5""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""basis": "canonical_actionable_gap""#));
    assert!(stdout.contains(r#""label": "ripr+""#));

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
        "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path",
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
        "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path",
        "--json",
    ]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""id": "probe:crates_ripr_examples_sample_src_lib.rs:21:error_path""#)
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
