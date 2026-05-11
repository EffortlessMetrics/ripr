use crate::domain::Finding;

pub(crate) fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        a.probe
            .location
            .file
            .cmp(&b.probe.location.file)
            .then(a.probe.location.line.cmp(&b.probe.location.line))
            .then(a.probe.family.as_str().cmp(b.probe.family.as_str()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Probe, ProbeFamily, ProbeId,
        RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
    };

    fn stage() -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Unknown, "sort test")
    }

    fn finding(id: &str, file: &str, line: usize, family: ProbeFamily) -> Finding {
        Finding {
            id: id.to_string(),
            probe: Probe {
                id: ProbeId(format!("probe-{id}")),
                location: SourceLocation::new(file, line, 1),
                owner: None,
                family,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "value > threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(),
                infect: stage(),
                propagate: stage(),
                reveal: RevealEvidence {
                    observe: stage(),
                    discriminate: stage(),
                },
            },
            confidence: 0.0,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: None,
            language_status: None,
        }
    }

    #[test]
    fn sort_findings_orders_by_file_line_then_probe_family() {
        let mut findings = vec![
            finding(
                "same-line-predicate",
                "src/b.rs",
                10,
                ProbeFamily::Predicate,
            ),
            finding("later-line", "src/a.rs", 20, ProbeFamily::Predicate),
            finding("same-line-error", "src/b.rs", 10, ProbeFamily::ErrorPath),
            finding("earlier-file", "src/a.rs", 5, ProbeFamily::SideEffect),
        ];

        sort_findings(&mut findings);

        let ids: Vec<&str> = findings.iter().map(|finding| finding.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "earlier-file",
                "later-line",
                "same-line-error",
                "same-line-predicate",
            ]
        );
    }
}
