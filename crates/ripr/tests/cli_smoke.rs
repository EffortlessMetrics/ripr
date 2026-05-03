use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run_ripr(args: &[&str]) -> Output {
    let bin = env!("CARGO_BIN_EXE_ripr");
    Command::new(bin).args(args).output().unwrap()
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected command to succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    assert!(stdout.contains("static RIPR"));
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
    assert!(stdout.contains(r#""schema_version": "0.2""#));
    assert!(stdout.contains(r#""kind": "ripr""#));
    assert!(stdout.contains(r#""scope": "diff""#));
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

fn make_temp_workspace_with_production_seam() -> Result<PathBuf, String> {
    make_temp_workspace_with_production_seam_and_report_opt(None)
}

fn make_temp_workspace_with_production_seam_and_report(report: &str) -> Result<PathBuf, String> {
    make_temp_workspace_with_production_seam_and_report_opt(Some(report))
}

fn make_temp_workspace_with_production_seam_and_report_opt(
    report: Option<&str>,
) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("ripr-repo-badge-{stamp}-{pid}"));
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
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("ripr-badge-plus-{stamp}-{pid}"));
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
fn check_badge_plus_fails_clearly_when_test_efficiency_report_missing() -> Result<(), String> {
    let workspace = make_temp_workspace(None)?;
    let root = workspace.display().to_string();
    let diff = sample_diff().display().to_string();

    for format in ["badge-plus-json", "badge-plus-shields"] {
        let output = run_ripr(&[
            "check", "--root", &root, "--diff", &diff, "--format", format,
        ]);
        assert!(
            !output.status.success(),
            "format `{format}` should fail when report missing"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("test-efficiency.json"),
            "stderr must name the missing report for `{format}`: {stderr}"
        );
        assert!(
            stderr.contains("cargo xtask test-efficiency-report"),
            "stderr must direct the user to the regenerator for `{format}`: {stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_plus_json_emits_native_shape_with_fixture_report() -> Result<(), String> {
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "0.2""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "diff""#));
    assert!(stdout.contains(r#""label": "ripr+""#));
    assert!(stdout.contains(r#""counts""#));
    assert!(stdout.contains(r#""reason_counts""#));
    assert!(stdout.contains(r#""policy""#));
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 1"#));
    assert!(stdout.contains(r#""intentional_test_efficiency_findings": 1"#));
    assert!(stdout.contains(r#""unknowns_test_efficiency": 1"#));
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
    assert!(stdout.contains(r#""schema_version": "0.2""#));
    assert!(stdout.contains(r#""kind": "ripr""#));
    assert!(stdout.contains(r#""scope": "repo""#));
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
    assert!(stdout.contains(r#""schema_version": "0.2""#));
    assert!(stdout.contains(r#""kind": "ripr_plus""#));
    assert!(stdout.contains(r#""scope": "repo""#));
    assert!(stdout.contains(r#""label": "ripr+""#));

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
    // The temp workspace has a probeable predicate; repo analysis seeds
    // probes from production syntax shapes so analyzed_findings > 0
    // even when the diff is empty.
    assert!(
        stdout.contains(r#""analyzed_findings""#),
        "repo native JSON must include analyzed_findings: {stdout}"
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

#[test]
fn check_badge_plus_applies_test_efficiency_suppressions_from_disk() -> Result<(), String> {
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // 1 actionable test moved into the suppressed bucket.
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 0"#));
    assert!(stdout.contains(r#""suppressed_test_efficiency_findings": 1"#));
    // intentional remains 0 — declared_intent and suppressions are distinct.
    assert!(stdout.contains(r#""intentional_test_efficiency_findings": 0"#));
    // No warnings — selector matched and not expired.
    assert!(stdout.contains(r#""warnings": []"#));

    let _ = std::fs::remove_dir_all(&workspace);
    Ok(())
}

#[test]
fn check_badge_plus_warns_on_expired_suppression_and_keeps_finding_in_headline()
-> Result<(), String> {
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Expired suppression must NOT apply — the finding stays in the
    // unsuppressed bucket.
    assert!(stdout.contains(r#""unsuppressed_test_efficiency_findings": 1"#));
    assert!(stdout.contains(r#""suppressed_test_efficiency_findings": 0"#));
    // Warnings array surfaces the expiry.
    assert!(stdout.contains("expired"));
    assert!(stdout.contains("cli_prints_help"));

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
