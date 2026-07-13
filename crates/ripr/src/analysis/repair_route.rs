//! Producer-owned repair-route readiness.
//!
//! This module decides whether the analyzer facts are sufficient to localize
//! one safe test-only repair. Output projections consume this result; they do
//! not infer test identity, effect sinks, or missing observations themselves.

use super::seam_classification::ClassifiedSeam;
use super::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind};
use super::test_grip_evidence::{RelatedTestGrip, TestGripEvidence, TestTargetEvidence};
use crate::analysis::canonical_gap::canonical_gap_identity;
use crate::domain::{OracleKind, OracleStrength, StageState};
use std::path::PathBuf;

pub(crate) const REPAIR_ROUTE_AUTHORITY_BOUNDARY: &str =
    "analysis/producer-owned-repair-route-readiness";
const PRODUCER_DISCRIMINATOR_EVIDENCE: &str = "producer-owned missing discriminator fact";
const SAFE_TEST_TARGET_EVIDENCE: &str = "safe test target";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairRouteState {
    Ready,
    AlreadyGripped,
    PolicyExcluded,
    StaticLimitation,
}

/// The producer-owned choice of where a test-only repair may land.
///
/// `Missing` is deliberate: a related-test summary, a path, or a renderer
/// heuristic is not permission to edit that location. New-test proposals are
/// represented explicitly so a future producer can supply them without
/// overloading an existing-test identity.
#[allow(
    dead_code,
    reason = "reserved typed target proposal variants await a RustIndex proposal producer"
)]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairTargetSelection {
    Existing(TestTargetEvidence),
    Proposed(NewTestTargetProposal),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct NewTestTargetProposal {
    pub(crate) kind: NewTestKind,
    pub(crate) file: PathBuf,
    pub(crate) owner: String,
    pub(crate) provenance: NewTestProposalProvenance,
}

#[allow(
    dead_code,
    reason = "reserved typed new-test kinds await a RustIndex proposal producer"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NewTestKind {
    InlineUnit,
    Integration,
}

#[allow(
    dead_code,
    reason = "reserved typed proposal provenance awaits a RustIndex proposal producer"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NewTestProposalProvenance {
    ProducerOwned,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RepairRouteReadiness {
    pub(crate) state: RepairRouteState,
    pub(crate) seam_id: String,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) present_evidence: Vec<String>,
    pub(crate) missing_evidence: Vec<String>,
    pub(crate) target_selection: RepairTargetSelection,
    pub(crate) test_target: Option<TestTargetEvidence>,
    pub(crate) proposed_oracle: Option<OracleKind>,
    pub(crate) current_oracle: Option<OracleKind>,
    pub(crate) authority_boundary: &'static str,
}

impl RepairRouteReadiness {
    pub(crate) fn is_repair_ready(&self) -> bool {
        self.state == RepairRouteState::Ready
            && !matches!(self.target_selection, RepairTargetSelection::Missing)
    }
}

pub(crate) fn repair_projection_ready(entry: &ClassifiedSeam) -> bool {
    repair_route_readiness(entry).is_repair_ready()
}

pub(crate) fn repair_route_readiness(entry: &ClassifiedSeam) -> RepairRouteReadiness {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let mut readiness = match seam.kind() {
        SeamKind::PredicateBoundary
        | SeamKind::ErrorVariant
        | SeamKind::ReturnValue
        | SeamKind::FieldConstruction
        | SeamKind::MatchArm => value_route_readiness(seam, evidence),
        SeamKind::SideEffect | SeamKind::CallPresence => effect_route_readiness(seam, evidence),
    };
    apply_grip_class_ceiling(entry.class, &mut readiness);
    readiness.seam_id = seam.id().as_str().to_string();
    readiness.canonical_gap_id = canonical_gap_identity(entry).map(|identity| identity.id);
    readiness.authority_boundary = REPAIR_ROUTE_AUTHORITY_BOUNDARY;
    readiness
}

fn apply_grip_class_ceiling(class: SeamGripClass, readiness: &mut RepairRouteReadiness) {
    match class {
        SeamGripClass::StronglyGripped => {
            readiness.state = RepairRouteState::AlreadyGripped;
            readiness.missing_evidence.clear();
        }
        SeamGripClass::Intentional | SeamGripClass::Suppressed => {
            readiness.state = RepairRouteState::PolicyExcluded;
            readiness.missing_evidence.clear();
        }
        SeamGripClass::ActivationUnknown
        | SeamGripClass::PropagationUnknown
        | SeamGripClass::ObservationUnknown
        | SeamGripClass::DiscriminationUnknown
        | SeamGripClass::Opaque => {
            let missing_stage = incomplete_stage_evidence(class).to_string();
            if !readiness
                .required_evidence
                .iter()
                .any(|evidence| evidence == &missing_stage)
            {
                readiness.required_evidence.push(missing_stage.clone());
            }
            if !readiness
                .missing_evidence
                .iter()
                .any(|evidence| evidence == &missing_stage)
            {
                readiness.missing_evidence.push(missing_stage);
            }
            readiness.state = RepairRouteState::StaticLimitation;
        }
        SeamGripClass::WeaklyGripped
        | SeamGripClass::Ungripped
        | SeamGripClass::ReachableUnrevealed => {}
    }
}

fn incomplete_stage_evidence(class: SeamGripClass) -> &'static str {
    match class {
        SeamGripClass::ActivationUnknown => "incomplete evidence stage: activation",
        SeamGripClass::PropagationUnknown => "incomplete evidence stage: propagation",
        SeamGripClass::ObservationUnknown => "incomplete evidence stage: observation",
        SeamGripClass::DiscriminationUnknown => "incomplete evidence stage: discrimination",
        SeamGripClass::Opaque => "incomplete evidence stage: opaque",
        _ => "incomplete evidence stage: unknown",
    }
}

fn value_route_readiness(seam: &RepoSeam, evidence: &TestGripEvidence) -> RepairRouteReadiness {
    let discriminator_evidence = format!(
        "{PRODUCER_DISCRIMINATOR_EVIDENCE}: {}",
        seam.required_discriminator().as_str()
    );
    let required = vec![
        discriminator_evidence.clone(),
        SAFE_TEST_TARGET_EVIDENCE.to_string(),
    ];
    let has_discriminator = has_exact_discriminator(seam, evidence);
    let selected_test_target = existing_test_target(evidence, false);
    // No related test is not evidence of a safe new-test location. A future
    // producer may populate `Proposed`; until then the target is Missing.
    let has_safe_target = selected_test_target.is_some();
    let state = if has_discriminator && has_safe_target {
        RepairRouteState::Ready
    } else {
        RepairRouteState::StaticLimitation
    };
    let mut present_evidence = Vec::new();
    if has_discriminator {
        present_evidence.push(discriminator_evidence.clone());
    }
    if has_safe_target {
        present_evidence.push(SAFE_TEST_TARGET_EVIDENCE.to_string());
    }
    let mut missing_evidence = Vec::new();
    if !has_discriminator {
        missing_evidence.push(discriminator_evidence);
    }
    if !has_safe_target {
        missing_evidence.push(SAFE_TEST_TARGET_EVIDENCE.to_string());
    }
    RepairRouteReadiness {
        state,
        seam_id: String::new(),
        canonical_gap_id: None,
        required_evidence: required,
        present_evidence,
        missing_evidence,
        target_selection: selected_test_target
            .clone()
            .map(RepairTargetSelection::Existing)
            .unwrap_or(RepairTargetSelection::Missing),
        test_target: selected_test_target,
        proposed_oracle: Some(oracle_for_seam(seam.kind())),
        current_oracle: current_oracle(evidence, false, true),
        authority_boundary: REPAIR_ROUTE_AUTHORITY_BOUNDARY,
    }
}

fn effect_route_readiness(seam: &RepoSeam, evidence: &TestGripEvidence) -> RepairRouteReadiness {
    let mut required_evidence = vec![
        "known production owner/caller".to_string(),
        "exact effect or call target".to_string(),
        "producer-owned test symbol".to_string(),
        "direct owner relationship".to_string(),
        "observable sink localization".to_string(),
    ];
    let mut present_evidence = Vec::new();
    let mut missing_evidence = Vec::new();

    if seam.owner().trim().is_empty() {
        missing_evidence.push("known production owner/caller".to_string());
    } else {
        present_evidence.push("known production owner/caller".to_string());
    }

    let target_is_exact = match (
        seam.kind(),
        seam.required_discriminator(),
        seam.expected_sink(),
    ) {
        (
            SeamKind::CallPresence,
            RequiredDiscriminator::CallSite { target },
            ExpectedSink::SideEffect,
        ) => !target.trim().is_empty(),
        (
            SeamKind::SideEffect,
            RequiredDiscriminator::Effect { sink },
            ExpectedSink::SideEffect,
        ) => !sink.trim().is_empty(),
        _ => false,
    };
    if target_is_exact {
        present_evidence.push("exact effect or call target".to_string());
    } else {
        missing_evidence.push("exact effect or call target".to_string());
    }

    let related = direct_owner_related_test(evidence);
    let target_related = direct_owner_related_test_with_target(evidence);
    let test_target = target_related.and_then(|test| test.test_target.clone());
    if test_target.is_some() {
        present_evidence.push("producer-owned test symbol".to_string());
    } else {
        missing_evidence.push("producer-owned test symbol".to_string());
    }
    if related.is_some() {
        present_evidence.push("direct owner relationship".to_string());
    } else {
        missing_evidence.push("direct owner relationship".to_string());
    }

    let current = target_related.map(|test| test.oracle_kind.clone());
    let current_strength = target_related.map(|test| test.oracle_strength.clone());
    let stages_localize_sink = matches!(
        (
            &evidence.reach.state,
            &evidence.activate.state,
            &evidence.propagate.state
        ),
        (StageState::Yes, StageState::Yes, StageState::Yes)
    );
    if stages_localize_sink {
        present_evidence.push("observable sink localization".to_string());
    } else {
        missing_evidence.push("observable sink localization".to_string());
    }

    let current_mock_is_strong = current.as_ref() == Some(&OracleKind::MockExpectation)
        && current_strength.as_ref() == Some(&OracleStrength::Strong)
        && evidence.observe.state == StageState::Yes
        && evidence.discriminate.state == StageState::Yes;
    let route_is_localized = missing_evidence.is_empty();
    let state = if current_mock_is_strong && route_is_localized {
        RepairRouteState::AlreadyGripped
    } else if route_is_localized {
        RepairRouteState::Ready
    } else {
        RepairRouteState::StaticLimitation
    };

    required_evidence.push("current observation may be absent or weak".to_string());
    if current_mock_is_strong {
        present_evidence.push("strong current MockExpectation".to_string());
    } else {
        present_evidence.push("current observation is absent or weak".to_string());
    }

    RepairRouteReadiness {
        state,
        seam_id: String::new(),
        canonical_gap_id: None,
        required_evidence,
        present_evidence,
        missing_evidence,
        target_selection: test_target
            .clone()
            .map(RepairTargetSelection::Existing)
            .unwrap_or(RepairTargetSelection::Missing),
        test_target,
        proposed_oracle: Some(OracleKind::MockExpectation),
        current_oracle: current,
        authority_boundary: REPAIR_ROUTE_AUTHORITY_BOUNDARY,
    }
}

fn has_exact_discriminator(seam: &RepoSeam, evidence: &TestGripEvidence) -> bool {
    evidence
        .missing_discriminators
        .iter()
        .any(|fact| discriminator_fact_matches(seam.required_discriminator(), &fact.value))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DiscriminatorCompatibilityKey {
    Comparison {
        left: String,
        operator: String,
        right: String,
    },
    EqualityBoundary {
        right: String,
    },
    Exact {
        kind: &'static str,
        value: String,
    },
}

fn discriminator_fact_matches(required: &RequiredDiscriminator, fact: &str) -> bool {
    let Some(required_key) = required_discriminator_key(required) else {
        return false;
    };
    let Some(fact_key) = fact_discriminator_key(required, fact) else {
        return false;
    };
    match (required_key, fact_key) {
        (
            DiscriminatorCompatibilityKey::Comparison {
                left: required_left,
                operator: required_operator,
                right: required_right,
            },
            DiscriminatorCompatibilityKey::Comparison {
                left: fact_left,
                operator: fact_operator,
                right: fact_right,
            },
        ) if required_operator == fact_operator => {
            required_left == fact_left && required_right == fact_right
        }
        (
            DiscriminatorCompatibilityKey::Comparison {
                left: required_left,
                right: required_right,
                ..
            },
            DiscriminatorCompatibilityKey::Comparison {
                left: fact_left,
                operator: fact_operator,
                right: fact_right,
            },
        ) => fact_operator == "==" && required_left == fact_left && required_right == fact_right,
        (
            DiscriminatorCompatibilityKey::Comparison { right, .. },
            DiscriminatorCompatibilityKey::EqualityBoundary { right: fact_right },
        ) => right == fact_right,
        (
            DiscriminatorCompatibilityKey::Exact {
                kind: required_kind,
                value: required_value,
            },
            DiscriminatorCompatibilityKey::Exact {
                kind: fact_kind,
                value: fact_value,
            },
        ) => required_kind == fact_kind && required_value == fact_value,
        _ => false,
    }
}

fn required_discriminator_key(
    required: &RequiredDiscriminator,
) -> Option<DiscriminatorCompatibilityKey> {
    match required {
        RequiredDiscriminator::BoundaryValue { description } => {
            let (left, operator, right) = comparison_parts(description)?;
            Some(DiscriminatorCompatibilityKey::Comparison {
                left,
                operator,
                right,
            })
        }
        RequiredDiscriminator::ReturnValue { description } => {
            exact_key("return_value", description)
        }
        RequiredDiscriminator::ErrorVariant { variant } => exact_key("error_variant", variant),
        RequiredDiscriminator::MatchArmTaken { arm } => exact_key("match_arm", arm),
        RequiredDiscriminator::FieldValue { field } => exact_key("field_value", field),
        RequiredDiscriminator::Effect { sink } => exact_key("effect", sink),
        RequiredDiscriminator::CallSite { target } => exact_key("call_site", target),
    }
}

fn fact_discriminator_key(
    required: &RequiredDiscriminator,
    fact: &str,
) -> Option<DiscriminatorCompatibilityKey> {
    match required {
        RequiredDiscriminator::BoundaryValue { .. } => {
            if let Some((left, operator, right)) = comparison_parts(fact) {
                return Some(DiscriminatorCompatibilityKey::Comparison {
                    left,
                    operator,
                    right,
                });
            }
            let right = fact
                .trim()
                .strip_suffix(" (equality boundary)")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(normalize_identifier)?;
            Some(DiscriminatorCompatibilityKey::EqualityBoundary { right })
        }
        RequiredDiscriminator::ReturnValue { .. } => exact_key("return_value", fact),
        RequiredDiscriminator::ErrorVariant { .. } => exact_key("error_variant", fact),
        RequiredDiscriminator::MatchArmTaken { .. } => exact_key("match_arm", fact),
        RequiredDiscriminator::FieldValue { .. } => exact_key("field_value", fact),
        RequiredDiscriminator::Effect { .. } => exact_key("effect", fact),
        RequiredDiscriminator::CallSite { .. } => exact_key("call_site", fact),
    }
}

fn exact_key(kind: &'static str, value: &str) -> Option<DiscriminatorCompatibilityKey> {
    let value = normalize_discriminator_text(value);
    (!value.is_empty()).then_some(DiscriminatorCompatibilityKey::Exact { kind, value })
}

fn comparison_parts(value: &str) -> Option<(String, String, String)> {
    for operator in [" >= ", " <= ", " == ", " != ", " > ", " < "] {
        if let Some((left, right)) = value.split_once(operator) {
            let left = normalize_identifier(left);
            let right = normalize_identifier(right);
            if !left.is_empty() && !right.is_empty() {
                return Some((left, operator.trim().to_string(), right));
            }
        }
    }
    None
}

fn normalize_discriminator_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_identifier(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn direct_owner_related_test(evidence: &TestGripEvidence) -> Option<&RelatedTestGrip> {
    evidence
        .related_tests
        .iter()
        .find(|test| test.relation_reason == crate::domain::RelationReason::DirectOwnerCall)
}

fn direct_owner_related_test_with_target(evidence: &TestGripEvidence) -> Option<&RelatedTestGrip> {
    evidence.related_tests.iter().find(|test| {
        test.relation_reason == crate::domain::RelationReason::DirectOwnerCall
            && test.test_target.is_some()
    })
}

fn existing_test_target(
    evidence: &TestGripEvidence,
    require_direct_owner: bool,
) -> Option<TestTargetEvidence> {
    evidence
        .related_tests
        .iter()
        .find(|test| {
            (!require_direct_owner
                || test.relation_reason == crate::domain::RelationReason::DirectOwnerCall)
                && test.test_target.is_some()
        })
        .and_then(|test| test.test_target.clone())
}

fn current_oracle(
    evidence: &TestGripEvidence,
    require_direct_owner: bool,
    require_test_target: bool,
) -> Option<OracleKind> {
    evidence
        .related_tests
        .iter()
        .find(|test| {
            (!require_direct_owner
                || test.relation_reason == crate::domain::RelationReason::DirectOwnerCall)
                && (!require_test_target || test.test_target.is_some())
        })
        .map(|test| test.oracle_kind.clone())
}

fn oracle_for_seam(kind: SeamKind) -> OracleKind {
    match kind {
        SeamKind::ErrorVariant => OracleKind::ExactErrorVariant,
        SeamKind::SideEffect | SeamKind::CallPresence => OracleKind::MockExpectation,
        _ => OracleKind::ExactValue,
    }
}

#[cfg(test)]
mod tests {
    use super::discriminator_fact_matches;
    use crate::analysis::seams::RequiredDiscriminator;

    #[test]
    fn boundary_matching_requires_the_same_operator_and_operands() {
        let required = RequiredDiscriminator::BoundaryValue {
            description: "amount >= discount_threshold".to_string(),
        };

        assert!(discriminator_fact_matches(
            &required,
            "discount_threshold (equality boundary)"
        ));
        assert!(!discriminator_fact_matches(
            &required,
            "amount != discount_threshold"
        ));
        assert!(!discriminator_fact_matches(
            &required,
            "discount_threshold_cache_key (equality boundary)"
        ));
    }

    #[test]
    fn boundary_matching_accepts_same_operands_from_equality_fact() {
        let required = RequiredDiscriminator::BoundaryValue {
            description: "amount >= discount_threshold".to_string(),
        };

        assert!(discriminator_fact_matches(
            &required,
            "amount == discount_threshold"
        ));
        assert!(!discriminator_fact_matches(
            &required,
            "amount == discount_threshold_cache_key"
        ));
        assert!(!discriminator_fact_matches(
            &required,
            "other_amount == discount_threshold"
        ));
    }

    #[test]
    fn boundary_matching_accepts_literal_equality_fact_for_same_boundary() {
        let required = RequiredDiscriminator::BoundaryValue {
            description: "amount > 10".to_string(),
        };

        assert!(discriminator_fact_matches(&required, "amount == 10"));
        assert!(!discriminator_fact_matches(&required, "amount == 100"));
    }

    #[test]
    fn non_boundary_matching_requires_exact_producer_text() {
        let required = RequiredDiscriminator::FieldValue {
            field: "discount_threshold".to_string(),
        };

        assert!(discriminator_fact_matches(&required, "discount_threshold"));
        assert!(!discriminator_fact_matches(
            &required,
            "discount_threshold_cache_key"
        ));
    }
}
