use crate::domain::ExposureClass;

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

impl Summary {
    pub fn increment_exposure_class(&mut self, class: &ExposureClass) {
        match class {
            ExposureClass::Exposed => self.exposed += 1,
            ExposureClass::WeaklyExposed => self.weakly_exposed += 1,
            ExposureClass::ReachableUnrevealed => self.reachable_unrevealed += 1,
            ExposureClass::NoStaticPath => self.no_static_path += 1,
            ExposureClass::InfectionUnknown => self.infection_unknown += 1,
            ExposureClass::PropagationUnknown => self.propagation_unknown += 1,
            ExposureClass::StaticUnknown => self.static_unknown += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Summary;
    use crate::domain::ExposureClass;

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

    #[test]
    fn increment_exposure_class_counts_each_class() {
        let mut summary = Summary::default();

        let classes = [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ];

        for class in classes {
            summary.increment_exposure_class(&class);
        }

        assert_eq!(summary.exposed, 1);
        assert_eq!(summary.weakly_exposed, 1);
        assert_eq!(summary.reachable_unrevealed, 1);
        assert_eq!(summary.no_static_path, 1);
        assert_eq!(summary.infection_unknown, 1);
        assert_eq!(summary.propagation_unknown, 1);
        assert_eq!(summary.static_unknown, 1);
    }

    #[test]
    fn increment_exposure_class_accumulates_repeated_classes() {
        let mut summary = Summary::default();

        summary.increment_exposure_class(&ExposureClass::NoStaticPath);
        summary.increment_exposure_class(&ExposureClass::NoStaticPath);
        summary.increment_exposure_class(&ExposureClass::NoStaticPath);

        assert_eq!(summary.no_static_path, 3);
    }
}
