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
fn check_json_uses_draft_mode_and_static_classifications() {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let output = Command::new(bin)
        .args([
            "check",
            "--diff",
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/sample/example.diff")
                .display()
                .to_string(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["tool"], "ripr");
    assert_eq!(json["mode"], "draft");

    let findings = json["findings"].as_array().unwrap();
    assert!(!findings.is_empty());

    for finding in findings {
        let classification = finding["classification"].as_str().unwrap();
        assert!(
            matches!(
                classification,
                "exposed"
                    | "weakly_exposed"
                    | "reachable_unrevealed"
                    | "no_static_path"
                    | "infection_unknown"
                    | "propagation_unknown"
                    | "static_unknown"
            ),
            "unexpected classification: {classification}"
        );
    }
}
