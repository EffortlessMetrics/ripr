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
    fn labels_and_severities_match_contract() {
        let cases = [
            (ExposureClass::Exposed, "exposed", "info", false),
            (
                ExposureClass::WeaklyExposed,
                "weakly_exposed",
                "warning",
                false,
            ),
            (
                ExposureClass::ReachableUnrevealed,
                "reachable_unrevealed",
                "warning",
                false,
            ),
            (
                ExposureClass::NoStaticPath,
                "no_static_path",
                "warning",
                false,
            ),
            (
                ExposureClass::InfectionUnknown,
                "infection_unknown",
                "warning",
                true,
            ),
            (
                ExposureClass::PropagationUnknown,
                "propagation_unknown",
                "note",
                true,
            ),
            (ExposureClass::StaticUnknown, "static_unknown", "note", true),
        ];

        for (class, label, severity, requires_stop_reason) in cases {
            assert_eq!(class.as_str(), label);
            assert_eq!(class.severity(), severity);
            assert_eq!(class.requires_stop_reason(), requires_stop_reason);
        }
    }
}
