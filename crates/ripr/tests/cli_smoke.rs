use std::process::Command;

#[test]
fn version_runs() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin).arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr"));
}

#[test]
fn help_runs() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin).arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("static RIPR"));
}

#[test]
fn doctor_runs() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin).arg("doctor").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr doctor"));
    assert!(stdout.contains("Cargo.toml found"));
}

#[test]
fn check_json_on_sample_diff_reports_expected_contract() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap();
    let diff_path = repo_root.join("crates/ripr/examples/sample/example.diff");

    let output = Command::new(bin)
        .current_dir(repo_root)
        .args(["check", "--diff"])
        .arg(&diff_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["schema_version"], "0.1");
    assert_eq!(parsed["tool"], "ripr");
    assert_eq!(parsed["mode"], "draft");

    let findings = parsed["findings"].as_array().unwrap();
    assert!(!findings.is_empty());
    assert!(findings
        .iter()
        .any(|finding| finding["classification"] == "weakly_exposed"));
    assert!(findings
        .iter()
        .any(|finding| finding["classification"] == "infection_unknown"));
}
