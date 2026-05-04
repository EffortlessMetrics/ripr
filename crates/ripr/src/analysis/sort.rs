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

    #[test]
    fn sort_findings_is_callable() {
        // Seam test: verify the function signature and basic functionality.
        // Integration tests in analysis::tests (analyzes_simple_predicate_gap,
        // repo_analysis_finds_predicate_in_production_file) verify actual sort order.
        let mut findings: Vec<Finding> = vec![];
        // Should not panic on empty input.
        sort_findings(&mut findings);
        assert!(findings.is_empty());
    }
}
