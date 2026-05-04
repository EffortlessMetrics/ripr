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

    #[test]
    fn summarize_findings_sets_changed_files_probes_and_findings() {
        // Test with empty findings - verifies basic accounting works.
        let findings: Vec<Finding> = vec![];
        let summary = summarize_findings(5, &findings);

        assert_eq!(summary.changed_rust_files, 5);
        assert_eq!(summary.probes, 0);
        assert_eq!(summary.findings, 0);
    }
}
