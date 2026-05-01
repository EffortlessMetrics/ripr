use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(file: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProbeId(pub String);

impl fmt::Display for ProbeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub String);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeFamily {
    Predicate,
    ReturnValue,
    ErrorPath,
    CallDeletion,
    FieldConstruction,
    SideEffect,
    MatchArm,
    StaticUnknown,
}

impl ProbeFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProbeFamily::Predicate => "predicate",
            ProbeFamily::ReturnValue => "return_value",
            ProbeFamily::ErrorPath => "error_path",
            ProbeFamily::CallDeletion => "call_deletion",
            ProbeFamily::FieldConstruction => "field_construction",
            ProbeFamily::SideEffect => "side_effect",
            ProbeFamily::MatchArm => "match_arm",
            ProbeFamily::StaticUnknown => "static_unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeltaKind {
    Value,
    Control,
    Effect,
    Unknown,
}

impl DeltaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeltaKind::Value => "value",
            DeltaKind::Control => "control",
            DeltaKind::Effect => "effect",
            DeltaKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OracleStrength {
    Strong,
    Medium,
    Weak,
    Smoke,
    None,
    Unknown,
}

impl OracleStrength {
    pub fn as_str(&self) -> &'static str {
        match self {
            OracleStrength::Strong => "strong",
            OracleStrength::Medium => "medium",
            OracleStrength::Weak => "weak",
            OracleStrength::Smoke => "smoke",
            OracleStrength::None => "none",
            OracleStrength::Unknown => "unknown",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            OracleStrength::Strong => 5,
            OracleStrength::Medium => 4,
            OracleStrength::Weak => 3,
            OracleStrength::Smoke => 2,
            OracleStrength::Unknown => 1,
            OracleStrength::None => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageState {
    Yes,
    Weak,
    No,
    Unknown,
    Opaque,
    NotApplicable,
}

impl StageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageState::Yes => "yes",
            StageState::Weak => "weak",
            StageState::No => "no",
            StageState::Unknown => "unknown",
            StageState::Opaque => "opaque",
            StageState::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
            Confidence::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageEvidence {
    pub state: StageState,
    pub confidence: Confidence,
    pub summary: String,
}

impl StageEvidence {
    pub fn new(state: StageState, confidence: Confidence, summary: impl Into<String>) -> Self {
        Self {
            state,
            confidence,
            summary: summary.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealEvidence {
    pub observe: StageEvidence,
    pub discriminate: StageEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiprEvidence {
    pub reach: StageEvidence,
    pub infect: StageEvidence,
    pub propagate: StageEvidence,
    pub reveal: RevealEvidence,
}

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    MaxDepthReached,
    ExternalCrateBoundary,
    DynamicDispatchUnresolved,
    ProcMacroOpaque,
    FixtureOpaque,
    FeatureUnknown,
    AsyncBoundaryOpaque,
    NoChangedRustLine,
}

impl StopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StopReason::MaxDepthReached => "max_depth_reached",
            StopReason::ExternalCrateBoundary => "external_crate_boundary",
            StopReason::DynamicDispatchUnresolved => "dynamic_dispatch_unresolved",
            StopReason::ProcMacroOpaque => "proc_macro_opaque",
            StopReason::FixtureOpaque => "fixture_opaque",
            StopReason::FeatureUnknown => "feature_unknown",
            StopReason::AsyncBoundaryOpaque => "async_boundary_opaque",
            StopReason::NoChangedRustLine => "no_changed_rust_line",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub id: ProbeId,
    pub location: SourceLocation,
    pub owner: Option<SymbolId>,
    pub family: ProbeFamily,
    pub delta: DeltaKind,
    pub before: Option<String>,
    pub after: Option<String>,
    pub expression: String,
    pub expected_sinks: Vec<String>,
    pub required_oracles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedTest {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub oracle: Option<String>,
    pub oracle_strength: OracleStrength,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub id: String,
    pub probe: Probe,
    pub class: ExposureClass,
    pub ripr: RiprEvidence,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub missing: Vec<String>,
    pub stop_reasons: Vec<StopReason>,
    pub related_tests: Vec<RelatedTest>,
    pub recommended_next_step: Option<String>,
}

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
