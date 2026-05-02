use std::process::Command;

fn sample_diff_path() -> String {
    format!(
        "{}/examples/sample/example.diff",
        env!("CARGO_MANIFEST_DIR")
    )
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
fn doctor_runs() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin).arg("doctor").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ripr doctor"));
    assert!(stdout.contains("Cargo.toml"));
}

#[test]
fn check_json_sample_diff_runs() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin)
        .args(["check", "--diff", &sample_diff_path(), "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["tool"], "ripr");
    assert_eq!(json["schema_version"], "0.1");
    let findings = json["findings"].as_array().unwrap();
    assert!(
        !findings.is_empty(),
        "expected at least one finding for sample diff"
    );
}
