use crate::app::CheckOutput;
use crate::config::RiprConfig;

/// Render findings as GitHub Actions workflow command annotations.
///
/// Each finding is emitted as one escaped annotation line so newlines and
/// punctuation survive GitHub's workflow-command parser.
pub fn render(output: &CheckOutput) -> String {
    render_with_config(output, &RiprConfig::default())
}

pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let mut out = String::new();
    for finding in &output.findings {
        let Some(annotation_level) = config
            .severity()
            .for_exposure(&finding.class)
            .github_annotation_level()
        else {
            continue;
        };
        let title = format!("ripr {}", finding.class.as_str());
        let mut message = finding
            .recommended_next_step
            .as_deref()
            .unwrap_or("Static RIPR exposure finding")
            .to_string();
        let stop_reasons = finding.effective_stop_reasons();
        if !stop_reasons.is_empty() {
            let reasons = stop_reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(" Stop reason: ");
            message.push_str(&reasons);
        }
        out.push_str(&format!(
            "::{annotation_level} file={},line={},title={}::{}\n",
            finding.probe.location.file.display(),
            finding.probe.location.line,
            escape_cmd(&title),
            escape_cmd(&message)
        ));
    }
    if output.findings.is_empty() {
        out.push_str("::notice title=ripr::No static mutation exposure findings found\n");
    }
    out
}

fn escape_cmd(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(',', "%2C")
        .replace(':', "%3A")
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily, ProbeId, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, Summary,
    };
    use std::path::PathBuf;

    #[test]
    fn render_reports_empty_findings_as_notice() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
        };

        let rendered = render(&output);

        assert_eq!(
            rendered,
            "::notice title=ripr::No static mutation exposure findings found\n"
        );
    }

    #[test]
    fn render_escapes_annotations_and_includes_effective_stop_reason_for_unknowns() {
        let rendered = render(&output_with_unknown_finding());

        assert!(rendered.contains("::notice file=src/lib.rs,line=13,title=ripr static_unknown::"));
        assert!(rendered.contains("Add%3A case%2C with 100%25 coverage%0Athen verify%0Doutcome"));
        assert!(rendered.contains("Stop reason%3A static_probe_unknown"));
    }

    #[test]
    fn render_uses_warning_for_exposed_and_default_message_without_stop_reason() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:21:error_path".to_string(),
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:21:error_path".to_string()),
                    location: SourceLocation::new("src/lib.rs", 21, 1),
                    owner: None,
                    family: ProbeFamily::ErrorPath,
                    delta: DeltaKind::Control,
                    before: Some("ok".to_string()),
                    after: Some("err".to_string()),
                    expression: "result".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::Exposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::Yes, "discriminated"),
                    },
                },
                confidence: 1.0,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: None,
                language: None,
                language_status: None,
                static_limit_kind: None,
            }],
        };

        let rendered = render(&output);

        assert!(rendered.contains("::notice file=src/lib.rs,line=21,title=ripr exposed::"));
        assert!(rendered.contains("Static RIPR exposure finding"));
        assert!(!rendered.contains("Stop reason"));
    }

    #[test]
    fn render_uses_warning_annotation_for_warning_severity_findings() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:34:weak_signal".to_string(),
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:34:weak_signal".to_string()),
                    location: SourceLocation::new("src/lib.rs", 34, 1),
                    owner: None,
                    family: ProbeFamily::Predicate,
                    delta: DeltaKind::Control,
                    before: Some("x > 0".to_string()),
                    after: Some("x >= 0".to_string()),
                    expression: "x > 0".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::WeaklyExposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::No, "not discriminated"),
                    },
                },
                confidence: 0.7,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: Some("Add discriminator assertion".to_string()),
                language: None,
                language_status: None,
                static_limit_kind: None,
            }],
        };

        let rendered = render(&output);

        assert!(rendered.contains("::warning file=src/lib.rs,line=34,title=ripr weakly_exposed::"));
        assert!(rendered.contains("Add discriminator assertion"));
    }

    fn output_with_unknown_finding() -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:13:static_unknown".to_string(),
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:13:static_unknown".to_string()),
                    location: SourceLocation::new("src/lib.rs", 13, 1),
                    owner: None,
                    family: ProbeFamily::StaticUnknown,
                    delta: DeltaKind::Unknown,
                    before: None,
                    after: None,
                    expression: "opaque".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::StaticUnknown,
                ripr: RiprEvidence {
                    reach: stage(StageState::Unknown, "reach unknown"),
                    infect: stage(StageState::Unknown, "infection unknown"),
                    propagate: stage(StageState::Unknown, "propagation unknown"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Unknown, "observe unknown"),
                        discriminate: stage(StageState::Unknown, "discriminate unknown"),
                    },
                },
                confidence: 0.2,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: Some(
                    "Add: case, with 100% coverage\nthen verify\routcome".to_string(),
                ),
                language: None,
                language_status: None,
                static_limit_kind: None,
            }],
        }
    }

    fn stage(state: StageState, reason: &str) -> StageEvidence {
        StageEvidence {
            state,
            confidence: Confidence::Low,
            summary: reason.to_string(),
        }
    }
}
