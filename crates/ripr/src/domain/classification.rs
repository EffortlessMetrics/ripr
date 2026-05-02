#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExposureClass {
    Exposed,
    WeaklyExposed,
    ReachableUnrevealed,
    NoStaticPath,
    InfectionUnknown,
    PropagationUnknown,
    StaticUnknown,
}

impl ExposureClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExposureClass::Exposed => "exposed",
            ExposureClass::WeaklyExposed => "weakly_exposed",
            ExposureClass::ReachableUnrevealed => "reachable_unrevealed",
            ExposureClass::NoStaticPath => "no_static_path",
            ExposureClass::InfectionUnknown => "infection_unknown",
            ExposureClass::PropagationUnknown => "propagation_unknown",
            ExposureClass::StaticUnknown => "static_unknown",
        }
    }

    pub fn severity(&self) -> &'static str {
        match self {
            ExposureClass::Exposed => "info",
            ExposureClass::WeaklyExposed => "warning",
            ExposureClass::ReachableUnrevealed => "warning",
            ExposureClass::NoStaticPath => "warning",
            ExposureClass::InfectionUnknown => "warning",
            ExposureClass::PropagationUnknown => "note",
            ExposureClass::StaticUnknown => "note",
        }
    }

    pub fn requires_stop_reason(&self) -> bool {
        matches!(
            self,
            ExposureClass::InfectionUnknown
                | ExposureClass::PropagationUnknown
                | ExposureClass::StaticUnknown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ExposureClass;

    #[test]
    fn exposure_class_strings_match_contract_terms() {
        let cases = [
            (ExposureClass::Exposed, "exposed"),
            (ExposureClass::WeaklyExposed, "weakly_exposed"),
            (ExposureClass::ReachableUnrevealed, "reachable_unrevealed"),
            (ExposureClass::NoStaticPath, "no_static_path"),
            (ExposureClass::InfectionUnknown, "infection_unknown"),
            (ExposureClass::PropagationUnknown, "propagation_unknown"),
            (ExposureClass::StaticUnknown, "static_unknown"),
        ];

        for (class, expected) in cases {
            assert_eq!(class.as_str(), expected);
        }
    }

    #[test]
    fn exposure_class_severities_match_output_expectations() {
        let cases = [
            (ExposureClass::Exposed, "info"),
            (ExposureClass::WeaklyExposed, "warning"),
            (ExposureClass::ReachableUnrevealed, "warning"),
            (ExposureClass::NoStaticPath, "warning"),
            (ExposureClass::InfectionUnknown, "warning"),
            (ExposureClass::PropagationUnknown, "note"),
            (ExposureClass::StaticUnknown, "note"),
        ];

        for (class, expected) in cases {
            assert_eq!(class.severity(), expected);
        }
    }

    #[test]
    fn stop_reason_requirement_is_only_for_unknown_classes() {
        assert!(!ExposureClass::Exposed.requires_stop_reason());
        assert!(!ExposureClass::WeaklyExposed.requires_stop_reason());
        assert!(!ExposureClass::ReachableUnrevealed.requires_stop_reason());
        assert!(!ExposureClass::NoStaticPath.requires_stop_reason());
        assert!(ExposureClass::InfectionUnknown.requires_stop_reason());
        assert!(ExposureClass::PropagationUnknown.requires_stop_reason());
        assert!(ExposureClass::StaticUnknown.requires_stop_reason());
    }
}
