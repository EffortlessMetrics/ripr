use crate::app::CheckOutput;

pub fn render(output: &CheckOutput) -> String {
    let mut out = String::new();
    for finding in &output.findings {
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
        let annotation_level = match finding.class.severity() {
            "info" => "notice",
            "note" => "notice",
            _ => "warning",
        };
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
        Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        Summary,
    };

    #[test]
    fn render_reports_notice_for_empty_findings() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: ".".into(),
            base: Some("origin/main".to_string()),
            summary: Summary {
                changed_rust_files: 0,
                probes: 0,
                findings: 0,
                exposed: 0,
                weakly_exposed: 0,
                reachable_unrevealed: 0,
                no_static_path: 0,
                infection_unknown: 0,
                propagation_unknown: 0,
                static_unknown: 0,
            },
            findings: Vec::new(),
        };

        assert_eq!(
            render(&output),
            "::notice title=ripr::No static mutation exposure findings found\n"
        );
    }

    #[test]
    fn render_escapes_command_special_chars_and_stop_reasons() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: ".".into(),
            base: Some("origin/main".to_string()),
            summary: Summary {
                changed_rust_files: 1,
                probes: 1,
                findings: 1,
                exposed: 1,
                weakly_exposed: 0,
                reachable_unrevealed: 0,
                no_static_path: 0,
                infection_unknown: 0,
                propagation_unknown: 0,
                static_unknown: 0,
            },
            findings: vec![sample_finding()],
        };

        assert_eq!(
            render(&output),
            "::notice file=src/lib.rs,line=11,title=ripr exposed::line1%2Cline2%3A100%25 Stop reason%3A max_depth_reached\n"
        );
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:11:error_path".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:11:error_path".to_string()),
                location: SourceLocation::new("src/lib.rs", 11, 4),
                owner: None,
                family: ProbeFamily::ErrorPath,
                delta: DeltaKind::Control,
                before: Some("ok".to_string()),
                after: Some("err".to_string()),
                expression: "do_work()".to_string(),
                expected_sinks: vec!["return".to_string()],
                required_oracles: vec!["exact_error_variant".to_string()],
            },
            class: ExposureClass::Exposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "changed path"),
                infect: StageEvidence::new(StageState::Yes, Confidence::High, "value differs"),
                propagate: StageEvidence::new(StageState::Yes, Confidence::High, "returned"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(
                        StageState::Yes,
                        Confidence::High,
                        "assert observes output",
                    ),
                    discriminate: StageEvidence::new(
                        StageState::Yes,
                        Confidence::High,
                        "assert discriminates",
                    ),
                },
            },
            confidence: 0.95,
            evidence: vec!["error path reached".to_string()],
            missing: Vec::new(),
            stop_reasons: vec![crate::domain::StopReason::MaxDepthReached],
            related_tests: vec![RelatedTest {
                name: "tests::reports_error".to_string(),
                file: "tests/lib_test.rs".into(),
                line: 8,
                oracle: Some("assert_eq!(err, MyError::Boom)".to_string()),
                oracle_strength: OracleStrength::Strong,
            }],
            recommended_next_step: Some("line1,line2:100%".to_string()),
        }
    }
}
