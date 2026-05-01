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
