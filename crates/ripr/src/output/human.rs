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
        "  Discriminate: {} — {}\n",
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

    if !finding.activation.observed_values.is_empty()
        || !finding.activation.missing_discriminators.is_empty()
    {
        out.push_str("\nActivation evidence:\n");
        for value in finding.activation.observed_values.iter().take(8) {
            out.push_str(&format!(
                "  - observed {} at line {} ({})\n",
                value.value,
                value.line,
                value.context.as_str()
            ));
        }
        for discriminator in &finding.activation.missing_discriminators {
            out.push_str(&format!(
                "  - missing {} — {}\n",
                discriminator.value, discriminator.reason
            ));
        }
    }

    if !finding.missing.is_empty() {
        out.push_str("\nGap:\n");
        for missing in &finding.missing {
            out.push_str(&format!("  - {missing}\n"));
        }
    }

    let stop_reasons = finding.effective_stop_reasons();
    if !stop_reasons.is_empty() {
        out.push_str("\nStop reasons:\n");
        for reason in &stop_reasons {
            out.push_str(&format!("  - {}\n", reason.as_str()));
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
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, Summary,
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
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
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
                reach: unknown_stage("No stable syntax owner"),
                infect: unknown_stage("Changed syntax is not mapped to a probe"),
                propagate: unknown_stage("No propagation model is available"),
                reveal: RevealEvidence {
                    observe: unknown_stage("No observation model is available"),
                    discriminate: unknown_stage("No discriminator model is available"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests: vec![],
            recommended_next_step: Some("Escalate to real mutation testing.".to_string()),
        }
    }

    fn stage(state: StageState, confidence: Confidence, summary: &str) -> StageEvidence {
        StageEvidence::new(state, confidence, summary)
    }

    fn unknown_stage(summary: &str) -> StageEvidence {
        stage(StageState::Unknown, Confidence::Low, summary)
    }
}
