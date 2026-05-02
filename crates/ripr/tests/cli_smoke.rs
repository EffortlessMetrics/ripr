use std::path::PathBuf;
use std::process::Command;

fn ripr_bin() -> &'static str {
    env!("CARGO_BIN_EXE_ripr")
}

fn sample_diff_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("sample")
        .join("example.diff")
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
fn check_sample_diff_human_output_runs() {
    let output = Command::new(ripr_bin())
        .args(["check", "--diff"])
        .arg(sample_diff_path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr static RIPR exposure analysis"));
    assert!(stdout.contains("Static exposure:"));
}

#[test]
fn check_sample_diff_json_output_is_valid() {
    let output = Command::new(ripr_bin())
        .args(["check", "--diff"])
        .arg(sample_diff_path())
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.get("summary").is_some());
    assert!(parsed.get("findings").is_some());
}
