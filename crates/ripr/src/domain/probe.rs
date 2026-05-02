use std::path::PathBuf;

use super::{ExposureClass, OracleStrength, ProbeId, RiprEvidence, SourceLocation, SymbolId};

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
pub enum StopReason {
    MaxDepthReached,
    ExternalCrateBoundary,
    DynamicDispatchUnresolved,
    ProcMacroOpaque,
    FixtureOpaque,
    FeatureUnknown,
    AsyncBoundaryOpaque,
    NoChangedRustLine,
    InfectionEvidenceUnknown,
    PropagationEvidenceUnknown,
    StaticProbeUnknown,
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
            StopReason::InfectionEvidenceUnknown => "infection_evidence_unknown",
            StopReason::PropagationEvidenceUnknown => "propagation_evidence_unknown",
            StopReason::StaticProbeUnknown => "static_probe_unknown",
        }
    }

    pub fn for_unknown_class(class: &ExposureClass) -> Option<Self> {
        match class {
            ExposureClass::InfectionUnknown => Some(StopReason::InfectionEvidenceUnknown),
            ExposureClass::PropagationUnknown => Some(StopReason::PropagationEvidenceUnknown),
            ExposureClass::StaticUnknown => Some(StopReason::StaticProbeUnknown),
            _ => None,
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

impl Finding {
    pub fn unknown_has_stop_reason(&self) -> bool {
        !self.class.requires_stop_reason() || !self.stop_reasons.is_empty()
    }

    pub fn effective_stop_reasons(&self) -> Vec<StopReason> {
        if self.unknown_has_stop_reason() {
            return self.stop_reasons.clone();
        }
        StopReason::for_unknown_class(&self.class)
            .into_iter()
            .collect()
    }
}
