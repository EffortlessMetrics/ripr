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
        profile::for_class(self).label
    }

    pub fn severity(&self) -> &'static str {
        profile::for_class(self).severity
    }

    pub fn requires_stop_reason(&self) -> bool {
        profile::for_class(self).requires_stop_reason
    }
}

mod profile {
    use super::ExposureClass;

    pub(super) struct ExposureProfile {
        pub(super) label: &'static str,
        pub(super) severity: &'static str,
        pub(super) requires_stop_reason: bool,
    }

    pub(super) fn for_class(class: &ExposureClass) -> ExposureProfile {
        match class {
            ExposureClass::Exposed => ExposureProfile {
                label: "exposed",
                severity: "info",
                requires_stop_reason: false,
            },
            ExposureClass::WeaklyExposed => ExposureProfile {
                label: "weakly_exposed",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::ReachableUnrevealed => ExposureProfile {
                label: "reachable_unrevealed",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::NoStaticPath => ExposureProfile {
                label: "no_static_path",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::InfectionUnknown => ExposureProfile {
                label: "infection_unknown",
                severity: "warning",
                requires_stop_reason: true,
            },
            ExposureClass::PropagationUnknown => ExposureProfile {
                label: "propagation_unknown",
                severity: "note",
                requires_stop_reason: true,
            },
            ExposureClass::StaticUnknown => ExposureProfile {
                label: "static_unknown",
                severity: "note",
                requires_stop_reason: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ExposureClass;
    use std::collections::HashSet;

    fn all_exposure_classes() -> [ExposureClass; 7] {
        [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ]
    }

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

    #[test]
    fn exposure_class_contract_terms_are_unique() {
        let mut seen = HashSet::new();

        for class in all_exposure_classes() {
            assert!(
                seen.insert(class.as_str()),
                "duplicate contract term found for {}",
                class.as_str()
            );
        }

        assert_eq!(seen.len(), 7, "every class should map to a unique term");
    }

    #[test]
    fn exposure_class_severity_values_stay_in_supported_set() {
        let supported = HashSet::from(["info", "warning", "note"]);
        for class in all_exposure_classes() {
            assert!(
                supported.contains(class.severity()),
                "unsupported severity {} for {}",
                class.severity(),
                class.as_str()
            );
        }
    }
}
