use std::{path::PathBuf, process::Command};

fn ripr_bin() -> &'static str {
    env!("CARGO_BIN_EXE_ripr")
}

fn sample_diff() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample/example.diff")
}

#[test]
fn version_runs() {
    let output = Command::new(ripr_bin()).arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr"));
}

#[test]
fn help_runs() {
    let output = Command::new(ripr_bin()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("static RIPR"));
}

#[test]
fn check_with_sample_diff_runs() {
    let output = Command::new(ripr_bin())
        .arg("check")
        .arg("--diff")
        .arg(sample_diff())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Summary:"));
    assert!(stdout.contains("Static exposure:"));
}

#[test]
fn check_json_with_sample_diff_emits_expected_fields() {
    let output = Command::new(ripr_bin())
        .arg("check")
        .arg("--diff")
        .arg(sample_diff())
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"schema_version\""));
    assert!(stdout.contains("\"classification\""));
}
