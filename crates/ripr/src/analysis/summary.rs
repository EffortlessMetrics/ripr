use crate::domain::{ExposureClass, Finding, Summary};

pub(crate) fn summarize_findings(changed_rust_files: usize, findings: &[Finding]) -> Summary {
    let mut summary = Summary {
        changed_rust_files,
        probes: findings.len(),
        findings: findings.len(),
        ..Summary::default()
    };

    for finding in findings {
        match finding.class {
            ExposureClass::Exposed => summary.exposed += 1,
            ExposureClass::WeaklyExposed => summary.weakly_exposed += 1,
            ExposureClass::ReachableUnrevealed => summary.reachable_unrevealed += 1,
            ExposureClass::NoStaticPath => summary.no_static_path += 1,
            ExposureClass::InfectionUnknown => summary.infection_unknown += 1,
            ExposureClass::PropagationUnknown => summary.propagation_unknown += 1,
            ExposureClass::StaticUnknown => summary.static_unknown += 1,
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, Probe, ProbeFamily, ProbeId, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState,
    };

    fn stage() -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Unknown, "summary test")
    }

    fn finding(class: ExposureClass) -> Finding {
        Finding {
            id: format!("finding-{}", class.as_str()),
            probe: Probe {
                id: ProbeId(format!("probe-{}", class.as_str())),
                location: SourceLocation::new("src/lib.rs", 1, 1),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "value > threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class,
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
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    #[test]
    fn summarize_findings_sets_changed_files_probes_and_findings() {
        // Test with empty findings - verifies basic accounting works.
        let findings: Vec<Finding> = vec![];
        let summary = summarize_findings(5, &findings);

        assert_eq!(summary.changed_rust_files, 5);
        assert_eq!(summary.probes, 0);
        assert_eq!(summary.findings, 0);
    }

    #[test]
    fn summarize_findings_counts_every_exposure_class() {
        let findings = [
            finding(ExposureClass::Exposed),
            finding(ExposureClass::WeaklyExposed),
            finding(ExposureClass::ReachableUnrevealed),
            finding(ExposureClass::NoStaticPath),
            finding(ExposureClass::InfectionUnknown),
            finding(ExposureClass::PropagationUnknown),
            finding(ExposureClass::StaticUnknown),
        ];

        let summary = summarize_findings(3, &findings);

        assert_eq!(summary.changed_rust_files, 3);
        assert_eq!(summary.probes, 7);
        assert_eq!(summary.findings, 7);
        assert_eq!(summary.exposed, 1);
        assert_eq!(summary.weakly_exposed, 1);
        assert_eq!(summary.reachable_unrevealed, 1);
        assert_eq!(summary.no_static_path, 1);
        assert_eq!(summary.infection_unknown, 1);
        assert_eq!(summary.propagation_unknown, 1);
        assert_eq!(summary.static_unknown, 1);
    }
    #[test]
    fn summarize_findings_accumulates_repeated_classes() {
        let findings = [
            finding(ExposureClass::Exposed),
            finding(ExposureClass::Exposed),
            finding(ExposureClass::NoStaticPath),
            finding(ExposureClass::NoStaticPath),
            finding(ExposureClass::NoStaticPath),
        ];

        let summary = summarize_findings(1, &findings);

        assert_eq!(summary.changed_rust_files, 1);
        assert_eq!(summary.probes, 5);
        assert_eq!(summary.findings, 5);
        assert_eq!(summary.exposed, 2);
        assert_eq!(summary.no_static_path, 3);
        assert_eq!(summary.weakly_exposed, 0);
        assert_eq!(summary.reachable_unrevealed, 0);
        assert_eq!(summary.infection_unknown, 0);
        assert_eq!(summary.propagation_unknown, 0);
        assert_eq!(summary.static_unknown, 0);
    }
}
