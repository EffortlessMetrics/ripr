use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{
    ExposureClass, OracleKind, OracleStrength, ProbeId, RiprEvidence, SourceLocation, SymbolId,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// A bounded transitive-reach walk found a candidate path (test -> ... ->
    /// owner) that ripr cannot fully resolve (lexical-only, name-match only).
    /// Classification stays `no_static_path`. See RIPR-SPEC-0114.
    TransitiveReachUnresolved,
    /// A Rust reach walk found a candidate test entry path that stops at a
    /// same-repo macro invocation whose definition lexically names the changed
    /// owner. ripr does not expand the macro; classification stays
    /// `no_static_path`. See RIPR-SPEC-0117.
    MacroReachUnresolved,
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
            StopReason::TransitiveReachUnresolved => "transitive_reach_unresolved",
            StopReason::MacroReachUnresolved => "macro_reach_unresolved",
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowSinkKind {
    ReturnValue,
    ErrorVariant,
    StructField,
    EventCall,
    StateWrite,
    Persistence,
    LogMessage,
    ConfigChange,
    CallEffect,
    MatchArm,
    Unknown,
}

impl FlowSinkKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlowSinkKind::ReturnValue => "return_value",
            FlowSinkKind::ErrorVariant => "error_variant",
            FlowSinkKind::StructField => "struct_field",
            FlowSinkKind::EventCall => "event_call",
            FlowSinkKind::StateWrite => "state_write",
            FlowSinkKind::Persistence => "persistence",
            FlowSinkKind::LogMessage => "log_message",
            FlowSinkKind::ConfigChange => "config_change",
            FlowSinkKind::CallEffect => "call_effect",
            FlowSinkKind::MatchArm => "match_arm",
            FlowSinkKind::Unknown => "unknown",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FlowSinkKind::ReturnValue => "returned value",
            FlowSinkKind::ErrorVariant => "error variant",
            FlowSinkKind::StructField => "constructed field",
            FlowSinkKind::EventCall => "event or outbound call",
            FlowSinkKind::StateWrite => "state write",
            FlowSinkKind::Persistence => "persistence write",
            FlowSinkKind::LogMessage => "log message",
            FlowSinkKind::ConfigChange => "configuration change",
            FlowSinkKind::CallEffect => "call effect",
            FlowSinkKind::MatchArm => "match arm result",
            FlowSinkKind::Unknown => "unknown sink",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSinkFact {
    pub kind: FlowSinkKind,
    pub text: String,
    pub line: usize,
    pub owner: Option<SymbolId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueContext {
    FunctionArgument,
    AssertionArgument,
    BuilderMethod,
    TableRow,
    EnumVariant,
    ReturnValue,
    Unknown,
}

impl ValueContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValueContext::FunctionArgument => "function_argument",
            ValueContext::AssertionArgument => "assertion_argument",
            ValueContext::BuilderMethod => "builder_method",
            ValueContext::TableRow => "table_row",
            ValueContext::EnumVariant => "enum_variant",
            ValueContext::ReturnValue => "return_value",
            ValueContext::Unknown => "unknown",
        }
    }
}

/// A single observed value extracted from a test assertion.
///
/// The `text` field holds the full assertion source text; it is used by the
/// human renderer.  The JSON renderer (schema 0.2+) **deduplicates** it into a
/// finding-level `assertion_texts` map keyed by line number, so `text` does
/// **not** appear in per-value objects in the JSON output.  Downstream JSON
/// consumers should recover the assertion source via
/// `finding.assertion_texts[line.to_string()]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueFact {
    pub line: usize,
    pub text: String,
    pub value: String,
    pub context: ValueContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingDiscriminatorFact {
    pub value: String,
    pub reason: String,
    pub flow_sink: Option<FlowSinkFact>,
}

/// Prefix the classifier puts on the value-shaped entries of `Finding.missing`.
///
/// The field mixes value-shaped entries (`Missing discriminator value: X`,
/// built from a [`MissingDiscriminatorFact`]) with prose entries ("No strong
/// discriminator was detected"), so consumers that already print a
/// "Missing discriminator" label must strip this prefix to avoid restating it.
/// Shared here because the classifier writes it and the renderers read it.
///
/// Crate-private: this is an internal formatting convention, not a contract
/// embedders may bind to. `lib.rs` re-exports `pub mod domain`, so a `pub`
/// constant here would publish the exact wording as library API.
pub(crate) const MISSING_DISCRIMINATOR_VALUE_PREFIX: &str = "Missing discriminator value: ";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingCanonicalGap {
    pub id: String,
    pub language: String,
    pub file: String,
    pub owner: String,
    pub behavior_kind: String,
    pub probe_kind: String,
    pub normalized_discriminator: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationEvidence {
    pub observed_values: Vec<ValueFact>,
    pub missing_discriminators: Vec<MissingDiscriminatorFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedTest {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub oracle: Option<String>,
    pub oracle_kind: OracleKind,
    pub oracle_strength: OracleStrength,
    /// Why this test was related to the probe. `None` when the match origin
    /// is unknown (legacy callers). `Some` for all diff-check findings that
    /// go through `classify/related_tests.rs`.
    pub relation_reason: Option<crate::domain::RelationReason>,
    /// Confidence that this test grips the changed behavior. `None` when
    /// `relation_reason` is `None`. Derived from `relation_reason` when set.
    pub relation_confidence: Option<crate::domain::RelationConfidence>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub canonical_gap: Option<FindingCanonicalGap>,
    pub probe: Probe,
    pub class: ExposureClass,
    pub ripr: RiprEvidence,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub missing: Vec<String>,
    pub flow_sinks: Vec<FlowSinkFact>,
    pub activation: ActivationEvidence,
    pub stop_reasons: Vec<StopReason>,
    pub related_tests: Vec<RelatedTest>,
    pub recommended_next_step: Option<String>,
    /// Source language the adapter that produced this finding identifies as.
    /// Additive optional per RIPR-SPEC-0026; populated by the per-language
    /// adapter (e.g. `RustAdapter` sets `Some(LanguageId::Rust)`).
    pub language: Option<crate::domain::LanguageId>,
    /// Adapter status for the source language. Omitted for `Rust` per the
    /// spec; preview adapters (TypeScript, Python) will set
    /// `Some(LanguageStatus::Preview)` when they land.
    pub language_status: Option<crate::domain::LanguageStatus>,
    /// Syntactic owner kind identified by preview language adapters.
    /// Additive optional per RIPR-SPEC-0026; omitted when the adapter does
    /// not yet have a bounded owner kind for the changed line.
    pub owner_kind: Option<crate::domain::OwnerKind>,
    /// Structured static limitation kind for preview evidence when the
    /// adapter can name one. Omitted when no static limit is known.
    pub static_limit_kind: Option<crate::domain::StaticLimitKind>,
    /// Significant tokens of the changed line — the *changed sink* the change
    /// touches. Additive optional per RIPR-SPEC-0028; populated by the Python
    /// preview adapter so a consumer can see what an oracle would need to
    /// observe. Omitted by adapters that do not compute it.
    pub changed_sink: Option<String>,
    /// Assertion text of the strongest related-test oracle the adapter
    /// inspected when deciding sink alignment. Additive optional per
    /// RIPR-SPEC-0028; omitted when no strong oracle was observed.
    pub observed_sink: Option<String>,
    /// Which token category the strongest oracle matched, surfacing *why* a
    /// strong oracle did or did not credit `exposed`. One of
    /// [`ORACLE_ALIGNMENT_VALUES`]. Additive optional per RIPR-SPEC-0028;
    /// omitted by adapters that do not compute sink alignment.
    pub oracle_alignment: Option<String>,
    /// Stable snake_case reason token explaining the `oracle_alignment` value.
    /// Additive optional per RIPR-SPEC-0028.
    pub alignment_reason: Option<String>,
    /// Producer-owned resolution of this finding's source against the
    /// candidate (head-side) revision (#3212 / #3280). Set by the producer
    /// that observed the diff evidence; `UnresolvedSubject` is the explicit
    /// unknown for surfaces that do not resolve it and the
    /// backward-compatibility default for artifacts written before the
    /// field existed. A `BaseDeleted` finding is base-side evidence; a
    /// `MovedOrRenamed` finding carries unresolved movement evidence.
    /// Neither is a candidate edit target.
    #[serde(default)]
    pub source_currentness: SourceCurrentness,
}

/// Typed source-currentness disposition for a [`Finding`] (#3212 / #3280).
///
/// The vocabulary is deliberately conservative: the producer states which
/// revision owns the actionable source, or states that it could not tell.
/// It never claims a deleted-side record is current.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCurrentness {
    /// The finding's source expression is present in the candidate source
    /// at the recorded location; the location is a candidate edit target.
    CandidateCurrent,
    /// The expression was removed on the candidate side. The retained
    /// evidence is base-side and is not a candidate edit target. In the C1
    /// producer slice, the recorded probe coordinate remains the projected
    /// new-side coordinate; consumer re-coordination is owned by #3281.
    BaseDeleted,
    /// Movement evidence exists (the same expression re-appears elsewhere
    /// in the candidate file), but the producer cannot prove the candidate
    /// identity of the exact source. Not a candidate edit target.
    MovedOrRenamed,
    /// The producing surface does not resolve source currentness; the
    /// disposition is explicitly unknown, never fabricated. Also the
    /// deserialize default for pre-#3280 artifacts.
    #[default]
    UnresolvedSubject,
}

impl SourceCurrentness {
    /// Stable wire label (matches the serde form).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CandidateCurrent => "candidate_current",
            Self::BaseDeleted => "base_deleted",
            Self::MovedOrRenamed => "moved_or_renamed",
            Self::UnresolvedSubject => "unresolved_subject",
        }
    }

    /// Delta-only resolution for producers that seed probes from head-side
    /// lines and never produce removed-only probes (#3281). A probe whose
    /// expression is candidate-side code (`after` present) is
    /// `CandidateCurrent`; a producer with no delta evidence at all (the
    /// Perl fact-packet path) stays the explicit unknown. Removed-only
    /// probes need the diff-level resolver in
    /// `analysis::probes` (movement evidence), not this helper.
    pub fn from_probe_delta(before: Option<&str>, after: Option<&str>) -> Self {
        match (before, after) {
            (_, Some(_)) => Self::CandidateCurrent,
            (None, None) => Self::UnresolvedSubject,
            // A before-only probe from a delta-only producer carries no
            // movement evidence; the diff-level resolver owns that case.
            (Some(_), None) => Self::UnresolvedSubject,
        }
    }

    /// Whether this disposition may drive current candidate-side
    /// obligations (repair routes, gate candidates, diagnostics, agent
    /// actions). Only established candidate currentness qualifies: base-side
    /// evidence and the explicit unknown are never silently upgraded
    /// because classification or severity suggests action (#3281).
    pub fn permits_candidate_action(&self) -> bool {
        matches!(self, Self::CandidateCurrent)
    }
}

/// Controlled enum values for [`Finding::source_currentness`]. Registered in
/// `policy/output_contracts.txt` and documented in `docs/OUTPUT_SCHEMA.md`.
pub const SOURCE_CURRENTNESS_VALUES: [&str; 4] = [
    "candidate_current",
    "base_deleted",
    "moved_or_renamed",
    "unresolved_subject",
];

/// Controlled enum values for [`Finding::oracle_alignment`]. Registered in
/// `policy/output_contracts.txt` and documented in `docs/OUTPUT_SCHEMA.md`.
pub const ORACLE_ALIGNMENT_VALUES: [&str; 5] = [
    "direct",
    "alias",
    "changed_sink_token",
    "orthogonal",
    "unknown",
];

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

    /// The single candidate-actionable eligibility authority (#3281, C2 of
    /// #3212): every count, gate candidate, repair route, diagnostic, and
    /// agent action derived from a finding must flow through this
    /// predicate. A finding qualifies only when its producer proved the
    /// source is candidate-current; `base_deleted` and `moved_or_renamed`
    /// are base-side evidence, and `unresolved_subject` (the Perl
    /// fact-packet path today, and pre-#3280 artifacts) is not established
    /// current. Classifications, severity, and repair readiness never
    /// upgrade a non-current finding.
    pub fn is_candidate_actionable(&self) -> bool {
        self.source_currentness.permits_candidate_action()
    }
}

#[cfg(test)]
mod tests {
    use super::{FlowSinkKind, StopReason, ValueContext};
    use crate::domain::ExposureClass;

    #[test]
    fn flow_sink_kind_labels_are_stable_contract_terms() {
        let cases = [
            (FlowSinkKind::ReturnValue, "return_value", "returned value"),
            (FlowSinkKind::ErrorVariant, "error_variant", "error variant"),
            (
                FlowSinkKind::StructField,
                "struct_field",
                "constructed field",
            ),
            (
                FlowSinkKind::EventCall,
                "event_call",
                "event or outbound call",
            ),
            (FlowSinkKind::StateWrite, "state_write", "state write"),
            (
                FlowSinkKind::Persistence,
                "persistence",
                "persistence write",
            ),
            (FlowSinkKind::LogMessage, "log_message", "log message"),
            (
                FlowSinkKind::ConfigChange,
                "config_change",
                "configuration change",
            ),
            (FlowSinkKind::CallEffect, "call_effect", "call effect"),
            (FlowSinkKind::MatchArm, "match_arm", "match arm result"),
            (FlowSinkKind::Unknown, "unknown", "unknown sink"),
        ];

        for (kind, value, label) in cases {
            assert_eq!(kind.as_str(), value);
            assert_eq!(kind.label(), label);
        }
    }

    #[test]
    fn stop_reason_for_unknown_class_matches_contract() {
        assert_eq!(
            StopReason::for_unknown_class(&ExposureClass::PropagationUnknown)
                .map(|reason| reason.as_str()),
            Some("propagation_evidence_unknown")
        );
        assert_eq!(StopReason::for_unknown_class(&ExposureClass::Exposed), None);
    }

    #[test]
    fn value_context_labels_are_stable_contract_terms() {
        let cases = [
            (ValueContext::FunctionArgument, "function_argument"),
            (ValueContext::AssertionArgument, "assertion_argument"),
            (ValueContext::BuilderMethod, "builder_method"),
            (ValueContext::TableRow, "table_row"),
            (ValueContext::EnumVariant, "enum_variant"),
            (ValueContext::ReturnValue, "return_value"),
            (ValueContext::Unknown, "unknown"),
        ];

        for (context, value) in cases {
            assert_eq!(context.as_str(), value);
        }
    }
}

#[cfg(test)]
mod source_currentness_tests {
    use super::{SOURCE_CURRENTNESS_VALUES, SourceCurrentness};

    #[test]
    fn source_currentness_labels_are_the_controlled_vocabulary() {
        // The wire form is pinned where JSON is allowed to be known
        // (`app::check_artifact` tests); here the stable labels themselves
        // are the contract.
        for (value, label) in [
            (SourceCurrentness::CandidateCurrent, "candidate_current"),
            (SourceCurrentness::BaseDeleted, "base_deleted"),
            (SourceCurrentness::MovedOrRenamed, "moved_or_renamed"),
            (SourceCurrentness::UnresolvedSubject, "unresolved_subject"),
        ] {
            assert_eq!(value.as_str(), label);
            assert!(SOURCE_CURRENTNESS_VALUES.contains(&label));
        }
    }
}

#[cfg(test)]
mod candidate_actionable_predicate_tests {
    use super::SourceCurrentness;

    #[test]
    fn only_proven_candidate_currentness_permits_action() {
        // RIPR-SPEC-0152: the single eligibility authority. Class and
        // severity never upgrade a non-current finding.
        for (value, eligible) in [
            (SourceCurrentness::CandidateCurrent, true),
            (SourceCurrentness::BaseDeleted, false),
            (SourceCurrentness::MovedOrRenamed, false),
            (SourceCurrentness::UnresolvedSubject, false),
        ] {
            assert_eq!(value.permits_candidate_action(), eligible, "{value:?}");
        }
    }

    #[test]
    fn delta_rule_resolves_head_side_probes_only() {
        use SourceCurrentness::{BaseDeleted, CandidateCurrent, UnresolvedSubject};
        assert_eq!(
            SourceCurrentness::from_probe_delta(None, Some("head text")),
            CandidateCurrent
        );
        assert_eq!(
            SourceCurrentness::from_probe_delta(Some("base"), Some("head")),
            CandidateCurrent
        );
        assert_eq!(
            SourceCurrentness::from_probe_delta(None, None),
            UnresolvedSubject
        );
        // The delta-only helper never claims base_deleted: removed-only
        // probes need the diff-level resolver with movement evidence.
        assert_eq!(
            SourceCurrentness::from_probe_delta(Some("base"), None),
            UnresolvedSubject
        );
        assert_ne!(
            SourceCurrentness::from_probe_delta(Some("base"), None),
            BaseDeleted
        );
    }
}
