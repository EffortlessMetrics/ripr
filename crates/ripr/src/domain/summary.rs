#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub changed_rust_files: usize,
    pub probes: usize,
    pub findings: usize,
    pub exposed: usize,
    pub weakly_exposed: usize,
    pub reachable_unrevealed: usize,
    pub no_static_path: usize,
    pub infection_unknown: usize,
    pub propagation_unknown: usize,
    pub static_unknown: usize,
}

#[cfg(test)]
mod tests {
    use super::Summary;

    #[test]
    fn default_summary_starts_with_zero_counts() {
        let summary = Summary::default();

        assert_eq!(summary.changed_rust_files, 0);
        assert_eq!(summary.probes, 0);
        assert_eq!(summary.findings, 0);
        assert_eq!(summary.exposed, 0);
        assert_eq!(summary.weakly_exposed, 0);
        assert_eq!(summary.reachable_unrevealed, 0);
        assert_eq!(summary.no_static_path, 0);
        assert_eq!(summary.infection_unknown, 0);
        assert_eq!(summary.propagation_unknown, 0);
        assert_eq!(summary.static_unknown, 0);
    }
}
