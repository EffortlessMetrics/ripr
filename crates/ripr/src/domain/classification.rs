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
