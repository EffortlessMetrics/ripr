use crate::app::CheckOutput;
use crate::domain::Finding;

pub fn render(output: &CheckOutput) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "ripr static RIPR exposure analysis\nmode: {}\nroot: {}\n\n",
        output.mode.as_str(),
        output.root.display()
    ));
    out.push_str(&format!(
        "Summary: {} probe(s), {} exposed, {} weak, {} unrevealed, {} no path, {} unknown\n\n",
        output.summary.probes,
        output.summary.exposed,
        output.summary.weakly_exposed,
        output.summary.reachable_unrevealed,
        output.summary.no_static_path,
        output.summary.static_unknown
            + output.summary.infection_unknown
            + output.summary.propagation_unknown
    ));

    if output.findings.is_empty() {
        out.push_str("No diff-derived mutation exposure probes found.\n");
        return out;
    }

    for finding in &output.findings {
        out.push_str(&render_finding(finding));
        out.push('\n');
    }
    out
}

pub fn render_finding(finding: &Finding) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} {}:{}\n",
        finding.class.severity().to_ascii_uppercase(),
        finding.probe.location.file.display(),
        finding.probe.location.line
    ));
    out.push_str(&format!(
        "\nStatic exposure: {} ({}, {})\n",
        finding.class.as_str(),
        finding.probe.family.as_str(),
        finding.probe.delta.as_str()
    ));
    out.push_str("\nChanged behavior:\n");
    if let Some(before) = &finding.probe.before {
        out.push_str(&format!("  before: {before}\n"));
    }
    if let Some(after) = &finding.probe.after {
        out.push_str(&format!("  after:  {after}\n"));
    } else {
        out.push_str(&format!("  expr:   {}\n", finding.probe.expression));
    }

    out.push_str("\nRIPR:\n");
    out.push_str(&format!(
        "  Reach:       {} — {}\n",
        finding.ripr.reach.state.as_str(),
        finding.ripr.reach.summary
    ));
    out.push_str(&format!(
        "  Infect:      {} — {}\n",
        finding.ripr.infect.state.as_str(),
        finding.ripr.infect.summary
    ));
    out.push_str(&format!(
        "  Propagate:   {} — {}\n",
        finding.ripr.propagate.state.as_str(),
        finding.ripr.propagate.summary
    ));
    out.push_str(&format!(
        "  Observe:     {} — {}\n",
        finding.ripr.reveal.observe.state.as_str(),
        finding.ripr.reveal.observe.summary
    ));
    out.push_str(&format!(
        "  Discriminate:{} — {}\n",
        finding.ripr.reveal.discriminate.state.as_str(),
        finding.ripr.reveal.discriminate.summary
    ));

    if !finding.related_tests.is_empty() {
        out.push_str("\nRelated tests / oracles:\n");
        for test in finding.related_tests.iter().take(5) {
            out.push_str(&format!(
                "  - {}:{} {} [{}]",
                test.file.display(),
                test.line,
                test.name,
                test.oracle_strength.as_str()
            ));
            if let Some(oracle) = &test.oracle {
                out.push_str(&format!(" — {oracle}"));
            }
            out.push('\n');
        }
    }

    if !finding.missing.is_empty() {
        out.push_str("\nGap:\n");
        for missing in &finding.missing {
            out.push_str(&format!("  - {missing}\n"));
        }
    }

    if let Some(step) = &finding.recommended_next_step {
        out.push_str("\nRecommended next step:\n");
        out.push_str(&format!("  {step}\n"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{render, render_finding};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        Summary,
    };
    use std::path::PathBuf;

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:10:predicate".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:10:predicate".to_string()),
                location: SourceLocation::new("src/lib.rs", 10, 5),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("x > 0".to_string()),
                after: Some("x >= 0".to_string()),
                expression: "x >= 0".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "changed path is reachable"),
                infect: StageEvidence::new(StageState::Weak, Confidence::Medium, "infection may be weak"),
                propagate: StageEvidence::new(StageState::Yes, Confidence::High, "propagates to return value"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Yes, Confidence::High, "test observes output"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "oracle is weak around boundary value",
                    ),
                },
            },
            confidence: 0.7,
            evidence: vec![],
            missing: vec!["No boundary assertion around x == 0".to_string()],
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "boundary_case".to_string(),
                file: PathBuf::from("tests/lib_tests.rs"),
                line: 42,
                oracle: Some("assert!(is_positive(1))".to_string()),
                oracle_strength: OracleStrength::Weak,
            }],
            recommended_next_step: Some(
                "Add assertion for x == 0 and expected predicate behavior.".to_string(),
            ),
        }
    }

    #[test]
    fn render_reports_empty_findings_message() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            summary: Summary {
                probes: 0,
                ..Summary::default()
            },
            findings: vec![],
        };

        let text = render(&output);
        assert!(text.contains("No diff-derived mutation exposure probes found."));
    }

    #[test]
    fn render_finding_includes_gap_and_recommendation_sections() {
        let finding = sample_finding();

        let text = render_finding(&finding);
        assert!(text.contains("Static exposure: weakly_exposed (predicate, control)"));
        assert!(text.contains("Related tests / oracles:"));
        assert!(text.contains("Gap:"));
        assert!(text.contains("Recommended next step:"));
    }
}
