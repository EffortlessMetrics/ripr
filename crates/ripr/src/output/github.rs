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
        Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily, ProbeId, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, StopReason, Summary,
    };
    use std::path::PathBuf;

    #[test]
    fn render_emits_notice_when_findings_are_empty() {
        let output = empty_output();

        assert_eq!(
            render(&output),
            "::notice title=ripr::No static mutation exposure findings found\n"
        );
    }

    #[test]
    fn render_escapes_title_and_message_and_joins_stop_reasons() {
        let finding = Finding {
            id: "probe:crate_src_lib.rs:21:error_path".to_string(),
            probe: Probe {
                id: ProbeId("probe:crate_src_lib.rs:21:error_path".to_string()),
                location: SourceLocation::new("crate/src/lib.rs", 21, 1),
                owner: None,
                family: ProbeFamily::ErrorPath,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "check()".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::PropagationUnknown,
            ripr: unknown_ripr(),
            confidence: 0.5,
            evidence: vec![],
            missing: vec![],
            stop_reasons: vec![StopReason::PropagationEvidenceUnknown],
            related_tests: vec![],
            recommended_next_step: Some("check 100%: this,\nnext line".to_string()),
        };

        let mut output = empty_output();
        output.findings.push(finding);

        assert_eq!(
            render(&output),
            "::notice file=crate/src/lib.rs,line=21,title=ripr propagation_unknown::check 100%25%3A this%2C%0Anext line Stop reason%3A propagation_evidence_unknown\n"
        );
    }

    fn empty_output() -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            summary: Summary::default(),
            findings: vec![],
        }
    }

    fn unknown_ripr() -> RiprEvidence {
        let stage = StageEvidence::new(StageState::Unknown, Confidence::Unknown, "unknown");
        RiprEvidence {
            reach: stage.clone(),
            infect: stage.clone(),
            propagate: stage.clone(),
            reveal: RevealEvidence {
                observe: stage.clone(),
                discriminate: stage,
            },
        }
    }
}
