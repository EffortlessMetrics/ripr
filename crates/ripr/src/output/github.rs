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
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, StopReason, Summary,
    };
    use std::path::PathBuf;

    #[test]
    fn render_encodes_title_and_message_for_annotations() {
        let finding = Finding {
            id: "probe:sample:1".to_string(),
            probe: Probe {
                id: ProbeId("probe:sample:1".to_string()),
                location: SourceLocation::new("src/lib.rs", 42, 1),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "flag".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::Exposed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes, "reach"),
                infect: stage(StageState::Yes, "infect"),
                propagate: stage(StageState::Yes, "propagate"),
                reveal: RevealEvidence {
                    observe: stage(StageState::Yes, "observe"),
                    discriminate: stage(StageState::Yes, "discriminate"),
                },
            },
            confidence: 0.8,
            evidence: vec![],
            missing: vec![],
            stop_reasons: vec![StopReason::InfectionEvidenceUnknown],
            related_tests: vec![RelatedTest {
                name: "test_x".to_string(),
                file: PathBuf::from("tests/x.rs"),
                line: 1,
                oracle: None,
                oracle_strength: OracleStrength::Strong,
            }],
            recommended_next_step: Some("line one,\nline two: 100%".to_string()),
        };
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            summary: Summary::default(),
            findings: vec![finding],
        };

        let rendered = render(&output);

        assert_eq!(
            rendered,
            "::notice file=src/lib.rs,line=42,title=ripr exposed::line one%2C%0Aline two%3A 100%25 Stop reason%3A infection_evidence_unknown\n"
        );
    }

    #[test]
    fn render_returns_default_notice_when_no_findings() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
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

    fn stage(state: StageState, summary: &str) -> StageEvidence {
        StageEvidence::new(state, Confidence::High, summary)
    }
}
