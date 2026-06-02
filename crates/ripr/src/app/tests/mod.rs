use super::{
    CHECK_OUTPUT_SCHEMA_VERSION, CheckOutput, Mode, OutputFormat, render_check,
    render_check_with_config, selector::selector_matches_location,
};
use crate::domain::{
    ActivationEvidence, Confidence, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily,
    ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
    StopReason, Summary,
};
use std::path::PathBuf;

mod badge_rendering;
mod mode_and_selector;
mod rendering_contracts;

fn sample_finding(file: &str, line: usize) -> Finding {
    Finding {
        id: "probe:src_lib_rs:42:error_path".to_string(),
        canonical_gap: None,
        probe: Probe {
            id: ProbeId("probe:src_lib_rs:42:error_path".to_string()),
            family: ProbeFamily::ErrorPath,
            location: SourceLocation::new(file, line, 1),
            owner: None,
            delta: crate::domain::DeltaKind::Control,
            before: None,
            after: None,
            expression: "sample_expr".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        },
        class: ExposureClass::WeaklyExposed,
        ripr: RiprEvidence {
            reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "reached"),
            infect: StageEvidence::new(StageState::Weak, Confidence::Low, "infected"),
            propagate: StageEvidence::new(StageState::No, Confidence::Medium, "not propagated"),
            reveal: RevealEvidence {
                observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observed"),
                discriminate: StageEvidence::new(
                    StageState::No,
                    Confidence::Medium,
                    "no discriminator",
                ),
            },
        },
        confidence: 0.5,
        evidence: vec!["changed test".to_string()],
        missing: vec!["strong oracle".to_string()],
        flow_sinks: Vec::new(),
        activation: ActivationEvidence::default(),
        stop_reasons: vec![StopReason::NoChangedRustLine],
        related_tests: vec![RelatedTest {
            name: "sample_test".to_string(),
            file: "tests/sample.rs".into(),
            line: 10,
            oracle: None,
            oracle_kind: crate::domain::OracleKind::Unknown,
            oracle_strength: OracleStrength::Weak,
        }],
        recommended_next_step: Some("add stronger assertion".to_string()),
        language: None,
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
    }
}

fn check_output_with(findings: Vec<Finding>) -> CheckOutput {
    CheckOutput {
        schema_version: CHECK_OUTPUT_SCHEMA_VERSION.to_string(),
        tool: "ripr".to_string(),
        mode: Mode::Draft,
        root: PathBuf::from("."),
        base: Some("origin/main".to_string()),
        summary: Summary::default(),
        findings,
    }
}

fn check_output_with_temp_seam_workspace(findings: Vec<Finding>) -> Result<CheckOutput, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-app-repo-badge-{stamp}"));
    std::fs::create_dir_all(root.join("src"))
        .map_err(|err| format!("create temp src dir: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"ripr-app-repo-badge\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .map_err(|err| format!("write temp Cargo.toml: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
    )
    .map_err(|err| format!("write temp src/lib.rs: {err}"))?;

    let mut output = check_output_with(findings);
    output.root = root;
    Ok(output)
}
