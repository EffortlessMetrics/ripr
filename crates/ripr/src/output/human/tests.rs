use super::{render, render_finding};
use crate::app::{CheckOutput, Mode};
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
    RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, Summary,
};
use std::path::PathBuf;

#[test]
fn render_includes_summary_counts_and_empty_findings_message() {
    let output = CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: Mode::Draft,
        root: PathBuf::from("repo"),
        base: None,
        summary: Summary {
            probes: 8,
            exposed: 1,
            weakly_exposed: 2,
            reachable_unrevealed: 1,
            no_static_path: 1,
            static_unknown: 1,
            infection_unknown: 1,
            propagation_unknown: 1,
            ..Summary::default()
        },
        findings: vec![],
    };

    let rendered = render(&output);

    assert!(rendered.contains("mode: draft"));
    assert!(rendered.contains(
        "Summary: 8 probe(s), 1 exposed, 2 weak, 1 unrevealed, 1 no path, 3 unknown"
    ));
    assert!(rendered.contains("No diff-derived mutation exposure probes found."));
}

#[test]
fn render_finding_includes_ripr_evidence_related_tests_gap_and_next_step() {
    let finding = sample_finding();
    let location = finding.probe.location.file.display().to_string();
    let related_path = finding.related_tests[0].file.display().to_string();

    let rendered = render_finding(&finding);

    assert!(rendered.contains(&format!("WARNING {location}:7")));
    assert!(rendered.contains("Static exposure: weakly_exposed (predicate, control)"));
    assert!(rendered.contains("Changed behavior:"));
    assert!(rendered.contains("before: if enabled"));
    assert!(rendered.contains("after:  if disabled"));
    assert!(rendered.contains("RIPR:"));
    assert!(rendered.contains("Reach:       yes — reaches test"));
    assert!(rendered.contains("Infect:      weak — weak mutation"));
    assert!(rendered.contains("Propagate:   unknown — propagation unclear"));
    assert!(rendered.contains("Observe:     yes — observed"));
    assert!(rendered.contains("Discriminate: no — no discriminator"));
    assert!(rendered.contains("Related tests / oracles:"));
    assert!(rendered.contains(&format!(
        "{related_path}:22 test_handles_disabled [strong] — assert_eq!(actual, expected)"
    )));
    assert!(rendered.contains("Gap:"));
    assert!(rendered.contains("missing strong oracle"));
    assert!(rendered.contains("Recommended next step:"));
    assert!(rendered.contains("Add assertion for disabled path result."));
}

#[test]
fn human_output_includes_effective_stop_reasons_for_unknowns() {
    let output = render_finding(&unknown_finding());

    assert!(output.contains("Stop reasons:"));
    assert!(output.contains("  - static_probe_unknown"));
}

fn sample_finding() -> Finding {
    Finding {
        id: "probe:sample.rs:7:predicate".to_string(),
        probe: Probe {
            id: ProbeId("probe:sample.rs:7:predicate".to_string()),
            location: SourceLocation::new("src/sample.rs", 7, 3),
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: Some("if enabled".to_string()),
            after: Some("if disabled".to_string()),
            expression: "enabled".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        },
        class: ExposureClass::WeaklyExposed,
        ripr: RiprEvidence {
            reach: stage(StageState::Yes, Confidence::High, "reaches test"),
            infect: stage(StageState::Weak, Confidence::Medium, "weak mutation"),
            propagate: stage(StageState::Unknown, Confidence::Low, "propagation unclear"),
            reveal: RevealEvidence {
                observe: stage(StageState::Yes, Confidence::High, "observed"),
                discriminate: stage(StageState::No, Confidence::Medium, "no discriminator"),
            },
        },
        confidence: 0.7,
        evidence: vec![],
        missing: vec!["missing strong oracle".to_string()],
        stop_reasons: vec![],
        related_tests: vec![RelatedTest {
            name: "test_handles_disabled".to_string(),
            file: PathBuf::from("tests/sample.rs"),
            line: 22,
            oracle: Some("assert_eq!(actual, expected)".to_string()),
            oracle_strength: OracleStrength::Strong,
        }],
        recommended_next_step: Some("Add assertion for disabled path result.".to_string()),
    }
}

fn unknown_finding() -> Finding {
    Finding {
        id: "probe:src_lib_rs:1:static_unknown".to_string(),
        probe: Probe {
            id: ProbeId("probe:src_lib_rs:1:static_unknown".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: None,
            family: ProbeFamily::StaticUnknown,
            delta: DeltaKind::Unknown,
            before: None,
            after: None,
            expression: "unknown syntax".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        },
        class: ExposureClass::StaticUnknown,
        ripr: RiprEvidence {
            reach: stage(StageState::Unknown, Confidence::Low, "reach unknown"),
            infect: stage(StageState::Unknown, Confidence::Low, "infection unknown"),
            propagate: stage(StageState::Unknown, Confidence::Low, "propagation unknown"),
            reveal: RevealEvidence {
                observe: stage(StageState::Unknown, Confidence::Low, "observe unknown"),
                discriminate: stage(StageState::Unknown, Confidence::Low, "discriminate unknown"),
            },
        },
        confidence: 0.3,
        evidence: vec![],
        missing: vec![],
        stop_reasons: vec![],
        related_tests: vec![],
        recommended_next_step: None,
    }
}

fn stage(state: StageState, confidence: Confidence, summary: &str) -> StageEvidence {
    StageEvidence::new(state, confidence, summary)
}
