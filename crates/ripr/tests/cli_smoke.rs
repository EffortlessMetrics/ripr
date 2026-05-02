use std::path::PathBuf;
use std::process::Command;

fn sample_diff() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("sample")
        .join("example.diff")
}

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
fn check_runs_with_sample_diff() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin)
        .args(["check", "--diff"])
        .arg(sample_diff())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Summary:"));
    assert!(stdout.contains("Static exposure:"));
}

#[test]
fn check_json_runs_with_sample_diff() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin)
        .args(["check", "--diff"])
        .arg(sample_diff())
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"schema_version\""));
    assert!(stdout.contains("\"summary\""));
    assert!(stdout.contains("\"classification\""));
}
