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

    out.push_str("\nChanged\n");
    if let Some(before) = &finding.probe.before {
        out.push_str(&format!("  before: {before}\n"));
    }
    if let Some(after) = &finding.probe.after {
        out.push_str(&format!("  after:  {after}\n"));
    } else {
        out.push_str(&format!("  expr:   {}\n", finding.probe.expression));
    }

    out.push_str("\nProbe\n");
    out.push_str(&format!(
        "  family: {}\n  delta:  {}\n",
        finding.probe.family.as_str(),
        finding.probe.delta.as_str()
    ));
    if let Some(owner) = &finding.probe.owner {
        out.push_str(&format!("  owner:  {owner}\n"));
    }

    out.push_str("\nStatic exposure\n");
    out.push_str(&format!(
        "  {} ({}, confidence {:.2})\n",
        finding.class.as_str(),
        finding.class.severity(),
        finding.confidence
    ));

    out.push_str("\nEvidence\n");
    for line in evidence_path_lines(finding) {
        out.push_str(&format!("  - {line}\n"));
    }

    let weakness = weakness_lines(finding);
    if !weakness.is_empty() {
        out.push_str("\nWeakness\n");
        for line in weakness {
            out.push_str(&format!("  - {line}\n"));
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
        out.push_str("\nNext step\n");
        out.push_str(&format!("  {step}\n"));
    }

    out
}

fn evidence_path_lines(finding: &Finding) -> Vec<String> {
    let mut lines = vec![
        format!(
            "reach {}: {}",
            finding.ripr.reach.state.as_str(),
            finding.ripr.reach.summary
        ),
        format!(
            "infection {}: {}",
            finding.ripr.infect.state.as_str(),
            finding.ripr.infect.summary
        ),
        format!(
            "propagation {}: {}",
            finding.ripr.propagate.state.as_str(),
            finding.ripr.propagate.summary
        ),
        format!(
            "observation {}: {}",
            finding.ripr.reveal.observe.state.as_str(),
            finding.ripr.reveal.observe.summary
        ),
        format!(
            "discriminator {}: {}",
            finding.ripr.reveal.discriminate.state.as_str(),
            finding.ripr.reveal.discriminate.summary
        ),
    ];

    for sink in &finding.flow_sinks {
        lines.push(format!(
            "local flow reaches {}: {} (line {})",
            sink.kind.label(),
            sink.text,
            sink.line
        ));
    }

    for test in finding.related_tests.iter().take(5) {
        let oracle_kind = display_label(test.oracle_kind.as_str());
        let mut line = format!(
            "related test {}:{} {} uses {} {} oracle",
            test.file.display(),
            test.line,
            test.name,
            test.oracle_strength.as_str(),
            oracle_kind
        );
        if let Some(oracle) = &test.oracle {
            line.push_str(&format!(": {oracle}"));
        }
        lines.push(line);
    }

    for value in finding.activation.observed_values.iter().take(8) {
        let context = display_label(value.context.as_str());
        lines.push(format!(
            "observed {} value {} at line {}",
            context, value.value, value.line
        ));
    }

    if lines.len() == 5 && !finding.evidence.is_empty() {
        lines.extend(finding.evidence.iter().cloned());
    }

    lines
}

fn weakness_lines(finding: &Finding) -> Vec<String> {
    let discriminator_values: Vec<&str> = finding
        .activation
        .missing_discriminators
        .iter()
        .map(|fact| fact.value.as_str())
        .collect();
    let mut lines = finding
        .missing
        .iter()
        .filter(|missing| !is_duplicate_discriminator_missing(missing, &discriminator_values))
        .cloned()
        .collect::<Vec<_>>();
    for discriminator in &finding.activation.missing_discriminators {
        lines.push(format!(
            "missing discriminator {}: {}",
            discriminator.value, discriminator.reason
        ));
    }
    lines
}

fn is_duplicate_discriminator_missing(missing: &str, discriminator_values: &[&str]) -> bool {
    let Some(value) = missing.strip_prefix("Missing discriminator value: ") else {
        return false;
    };
    discriminator_values.contains(&value)
}

fn display_label(value: &str) -> String {
    value.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::{render, render_finding};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FlowSinkFact,
        FlowSinkKind, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, Summary, ValueContext, ValueFact,
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
        assert!(rendered.contains("Changed\n"));
        assert!(rendered.contains("before: if enabled"));
        assert!(rendered.contains("after:  if disabled"));
        assert!(rendered.contains("Probe\n"));
        assert!(rendered.contains("family: predicate"));
        assert!(rendered.contains("Static exposure\n"));
        assert!(rendered.contains("weakly_exposed (warning, confidence 0.70)"));
        assert!(rendered.contains("Evidence\n"));
        assert!(rendered.contains("reach yes: reaches test"));
        assert!(rendered.contains("infection weak: weak mutation"));
        assert!(rendered.contains("propagation unknown: propagation unclear"));
        assert!(rendered.contains("observation yes: observed"));
        assert!(rendered.contains("discriminator no: no discriminator"));
        assert!(rendered.contains("local flow reaches returned value: disabled_result (line 8)"));
        assert!(rendered.contains(&format!(
            "{related_path}:22 test_handles_disabled uses strong exact value oracle: assert_eq!(actual, expected)"
        )));
        assert!(rendered.contains("observed function argument value enabled = false at line 22"));
        assert!(rendered.contains("Weakness\n"));
        assert!(rendered.contains("missing strong oracle"));
        assert!(rendered.contains(
            "missing discriminator enabled == false: related tests do not use the changed value"
        ));
        assert!(rendered.contains("Next step\n"));
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
            flow_sinks: vec![FlowSinkFact {
                kind: FlowSinkKind::ReturnValue,
                text: "disabled_result".to_string(),
                line: 8,
                owner: None,
            }],
            activation: ActivationEvidence {
                observed_values: vec![ValueFact {
                    line: 22,
                    text: "sample(false)".to_string(),
                    value: "enabled = false".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "enabled == false".to_string(),
                    reason: "related tests do not use the changed value".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "test_handles_disabled".to_string(),
                file: PathBuf::from("tests/sample.rs"),
                line: 22,
                oracle: Some("assert_eq!(actual, expected)".to_string()),
                oracle_kind: OracleKind::ExactValue,
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
