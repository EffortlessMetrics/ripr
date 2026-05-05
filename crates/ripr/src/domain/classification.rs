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
    use proptest::prelude::*;

    fn any_exposure_class() -> impl Strategy<Value = ExposureClass> {
        prop_oneof![
            Just(ExposureClass::Exposed),
            Just(ExposureClass::WeaklyExposed),
            Just(ExposureClass::ReachableUnrevealed),
            Just(ExposureClass::NoStaticPath),
            Just(ExposureClass::InfectionUnknown),
            Just(ExposureClass::PropagationUnknown),
            Just(ExposureClass::StaticUnknown),
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

    proptest! {
        #[test]
        fn exposure_class_labels_use_safe_contract_characters(class in any_exposure_class()) {
            let label = class.as_str();
            prop_assert!(!label.is_empty());
            prop_assert!(label.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
        }

        #[test]
        fn exposure_class_unknown_suffix_matches_stop_reason(class in any_exposure_class()) {
            let has_unknown_suffix = class.as_str().ends_with("_unknown");
            prop_assert_eq!(class.requires_stop_reason(), has_unknown_suffix);
        }

        #[test]
        fn exposure_class_stop_reason_never_uses_info_severity(class in any_exposure_class()) {
            if class.requires_stop_reason() {
                prop_assert_ne!(class.severity(), "info");
            }
        }
    }
}
