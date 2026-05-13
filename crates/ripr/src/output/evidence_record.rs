//! Shared static evidence record projection for Lane 1 outputs.
//!
//! This module gives repo exposure and downstream advisory reports a single
//! seam-native evidence shape. It is a projection over existing analyzer facts
//! only: it does not run mutation testing, make policy decisions, mutate
//! baselines, or change seam grip classifications.

use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::CanonicalGapIdentity;
use crate::analysis::seams::{SeamGripClass, SeamKind};
use crate::analysis::test_grip_evidence::oracle_semantics_for;
use crate::domain::{OracleKind, OracleStrength, StageEvidence, StageState};
use crate::output::agent_seam_packets::{
    AssertionShape, CandidateValue, RecommendedTest, assertion_shape_for, candidate_values_for,
    missing_discriminator_records_for, nearest_strong_test_to_imitate, recommended_test_for,
};
use serde_json::{Value, json};

pub(crate) const EVIDENCE_RECORD_SCHEMA_VERSION: &str = "0.1";

const MAX_RELATED_TESTS_PER_EVIDENCE_RECORD: usize = 8;
const VERIFY_COMMAND: &str = "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecord {
    pub(crate) seam_id: String,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) canonical_gap_group_size: Option<usize>,
    pub(crate) canonical_gap_reason: Option<String>,
    pub(crate) owner: String,
    pub(crate) location: EvidenceRecordLocation,
    pub(crate) seam_kind: String,
    pub(crate) grip_class: String,
    pub(crate) headline_eligible: bool,
    pub(crate) evidence_path: EvidenceRecordPath,
    pub(crate) observed_values: Vec<EvidenceRecordObservedValue>,
    pub(crate) missing_discriminators: Vec<EvidenceRecordMissingDiscriminator>,
    pub(crate) related_tests_total: usize,
    pub(crate) related_tests: Vec<EvidenceRecordRelatedTest>,
    pub(crate) recommendation: EvidenceRecordRecommendation,
    pub(crate) actionability: EvidenceRecordActionability,
    pub(crate) calibration: EvidenceRecordCalibration,
    pub(crate) static_limitations: Vec<EvidenceRecordStaticLimitation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordLocation {
    pub(crate) file: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordPath {
    pub(crate) reach: EvidenceRecordStage,
    pub(crate) activate: EvidenceRecordStage,
    pub(crate) propagate: EvidenceRecordStage,
    pub(crate) observe: EvidenceRecordStage,
    pub(crate) discriminate: EvidenceRecordStage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordStage {
    pub(crate) state: String,
    pub(crate) confidence: String,
    pub(crate) summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordObservedValue {
    pub(crate) value: String,
    pub(crate) line: usize,
    pub(crate) text: String,
    pub(crate) context: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordMissingDiscriminator {
    pub(crate) value: String,
    pub(crate) reason: String,
    pub(crate) flow_sink: Option<EvidenceRecordFlowSink>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordFlowSink {
    pub(crate) kind: String,
    pub(crate) text: String,
    pub(crate) line: usize,
    pub(crate) owner: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordRelatedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) evidence_summary: String,
    pub(crate) oracle_semantics: EvidenceRecordOracleSemantics,
    pub(crate) relation_reason: String,
    pub(crate) relation_confidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordOracleSemantics {
    pub(crate) observes: String,
    pub(crate) missing: String,
    pub(crate) upgrade_suggestion: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordRecommendation {
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) recommended_test: Option<EvidenceRecordRecommendedTest>,
    pub(crate) nearest_test_to_imitate: Option<EvidenceRecordRelatedTest>,
    pub(crate) candidate_values: Vec<EvidenceRecordCandidateValue>,
    pub(crate) assertion_shape: Option<EvidenceRecordAssertionShape>,
    pub(crate) verify_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordRecommendedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordCandidateValue {
    pub(crate) value: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordAssertionShape {
    pub(crate) kind: String,
    pub(crate) example: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordActionability {
    pub(crate) class: String,
    pub(crate) reason: String,
    pub(crate) has_concrete_guidance: bool,
    pub(crate) signals: EvidenceRecordActionabilitySignals,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordActionabilitySignals {
    pub(crate) missing_discriminator: bool,
    pub(crate) candidate_value: bool,
    pub(crate) assertion_shape: bool,
    pub(crate) related_test: bool,
    pub(crate) recommended_test_target: bool,
    pub(crate) verification_command: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordCalibration {
    pub(crate) availability: String,
    pub(crate) confidence: String,
    pub(crate) agreement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordStaticLimitation {
    pub(crate) stage: String,
    pub(crate) state: String,
    pub(crate) reason: String,
    pub(crate) category: String,
    pub(crate) repair_route: String,
}

pub(crate) fn evidence_record_for(
    entry: &ClassifiedSeam,
    canonical_gap: Option<&CanonicalGapIdentity>,
) -> EvidenceRecord {
    let missing_records = missing_discriminator_records_for(entry);
    let actionability = actionability_for(entry, &missing_records);
    let recommendation = recommendation_for(entry, &missing_records, &actionability);
    let related_tests_total = entry.evidence.related_tests.len();

    EvidenceRecord {
        seam_id: entry.seam.id().as_str().to_string(),
        canonical_gap_id: canonical_gap.map(|gap| gap.id.clone()),
        canonical_gap_group_size: canonical_gap.map(|gap| gap.group_size),
        canonical_gap_reason: canonical_gap.map(|gap| gap.reason.to_string()),
        owner: entry.seam.owner().to_string(),
        location: EvidenceRecordLocation {
            file: display_path(entry.seam.file()),
            line: entry.seam.display_line(),
        },
        seam_kind: entry.seam.kind().as_str().to_string(),
        grip_class: entry.class.as_str().to_string(),
        headline_eligible: entry.class.is_headline_eligible(),
        evidence_path: EvidenceRecordPath {
            reach: stage_record(&entry.evidence.reach),
            activate: stage_record(&entry.evidence.activate),
            propagate: stage_record(&entry.evidence.propagate),
            observe: stage_record(&entry.evidence.observe),
            discriminate: stage_record(&entry.evidence.discriminate),
        },
        observed_values: entry
            .evidence
            .observed_values
            .iter()
            .map(|value| EvidenceRecordObservedValue {
                value: value.value.clone(),
                line: value.line,
                text: value.text.clone(),
                context: value.context.as_str().to_string(),
            })
            .collect(),
        missing_discriminators: entry
            .evidence
            .missing_discriminators
            .iter()
            .map(|missing| EvidenceRecordMissingDiscriminator {
                value: missing.value.clone(),
                reason: missing.reason.clone(),
                flow_sink: missing
                    .flow_sink
                    .as_ref()
                    .map(|sink| EvidenceRecordFlowSink {
                        kind: sink.kind.as_str().to_string(),
                        text: sink.text.clone(),
                        line: sink.line,
                        owner: sink.owner.as_ref().map(ToString::to_string),
                    }),
            })
            .collect(),
        related_tests_total,
        related_tests: entry
            .evidence
            .related_tests
            .iter()
            .take(MAX_RELATED_TESTS_PER_EVIDENCE_RECORD)
            .map(|test| related_test_record(test, entry.seam.kind()))
            .collect(),
        recommendation,
        actionability,
        calibration: EvidenceRecordCalibration {
            availability: "not_imported".to_string(),
            confidence: "unknown".to_string(),
            agreement: "no_runtime_data".to_string(),
        },
        static_limitations: static_limitations_for(entry),
    }
}

pub(crate) fn evidence_record_json_value(record: &EvidenceRecord) -> Value {
    json!({
        "schema_version": EVIDENCE_RECORD_SCHEMA_VERSION,
        "seam_id": record.seam_id.as_str(),
        "canonical_gap_id": record.canonical_gap_id.as_deref(),
        "canonical_gap_group_size": record.canonical_gap_group_size,
        "canonical_gap_reason": record.canonical_gap_reason.as_deref(),
        "owner": record.owner.as_str(),
        "location": {
            "file": record.location.file.as_str(),
            "line": record.location.line,
        },
        "seam_kind": record.seam_kind.as_str(),
        "grip_class": record.grip_class.as_str(),
        "headline_eligible": record.headline_eligible,
        "evidence_path": {
            "reach": stage_json(&record.evidence_path.reach),
            "activate": stage_json(&record.evidence_path.activate),
            "propagate": stage_json(&record.evidence_path.propagate),
            "observe": stage_json(&record.evidence_path.observe),
            "discriminate": stage_json(&record.evidence_path.discriminate),
        },
        "observed_values": record
            .observed_values
            .iter()
            .map(observed_value_json)
            .collect::<Vec<_>>(),
        "missing_discriminators": record
            .missing_discriminators
            .iter()
            .map(missing_discriminator_json)
            .collect::<Vec<_>>(),
        "related_tests_total": record.related_tests_total,
        "related_tests": record
            .related_tests
            .iter()
            .map(related_test_json)
            .collect::<Vec<_>>(),
        "recommendation": recommendation_json(&record.recommendation),
        "actionability": actionability_json(&record.actionability),
        "calibration": {
            "availability": record.calibration.availability.as_str(),
            "confidence": record.calibration.confidence.as_str(),
            "agreement": record.calibration.agreement.as_str(),
        },
        "static_limitations": record
            .static_limitations
            .iter()
            .map(static_limitation_json)
            .collect::<Vec<_>>(),
    })
}

fn display_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn stage_record(stage: &StageEvidence) -> EvidenceRecordStage {
    EvidenceRecordStage {
        state: stage.state.as_str().to_string(),
        confidence: stage.confidence.as_str().to_string(),
        summary: stage.summary.clone(),
    }
}

fn actionability_for(
    entry: &ClassifiedSeam,
    missing_records: &[crate::output::agent_seam_packets::MissingRecord],
) -> EvidenceRecordActionability {
    let static_limited = is_static_limited(entry);
    let candidate_values = !candidate_values_for(entry, missing_records).is_empty();
    let assertion_shape = !static_limited && entry.class.is_headline_eligible();
    let related_test = !entry.evidence.related_tests.is_empty();
    let recommended_test_target = assertion_shape;
    let verification_command = assertion_shape;
    let missing_discriminator = !missing_records.is_empty();

    let (class, reason) = if static_limited {
        (
            "static_limitation",
            "static evidence is opaque or unknown for this seam",
        )
    } else if !entry.class.is_headline_eligible() {
        (
            "not_policy_relevant",
            "seam is already gripped or intentionally non-actionable under current policy",
        )
    } else if related_test && weak_related_oracle(entry) {
        (
            "actionable_assertion_upgrade",
            "related tests reach the seam but still miss a concrete discriminator",
        )
    } else if related_test {
        (
            "actionable_related_test_extension",
            "extend the nearest related test with the missing discriminator",
        )
    } else if missing_discriminator || candidate_values {
        (
            "actionable_focused_test",
            "write a focused test for the missing discriminator",
        )
    } else {
        (
            "needs_human_design",
            "RIPR does not yet have enough concrete repair context for this seam",
        )
    };

    EvidenceRecordActionability {
        class: class.to_string(),
        reason: reason.to_string(),
        has_concrete_guidance: matches!(
            class,
            "actionable_assertion_upgrade"
                | "actionable_related_test_extension"
                | "actionable_focused_test"
        ),
        signals: EvidenceRecordActionabilitySignals {
            missing_discriminator,
            candidate_value: candidate_values,
            assertion_shape,
            related_test,
            recommended_test_target,
            verification_command,
        },
    }
}

fn is_static_limited(entry: &ClassifiedSeam) -> bool {
    matches!(entry.class, SeamGripClass::Opaque)
        || [
            &entry.evidence.reach,
            &entry.evidence.activate,
            &entry.evidence.propagate,
            &entry.evidence.observe,
            &entry.evidence.discriminate,
        ]
        .iter()
        .any(|stage| matches!(stage.state, StageState::Opaque | StageState::Unknown))
}

fn weak_related_oracle(entry: &ClassifiedSeam) -> bool {
    entry.evidence.related_tests.iter().any(|test| {
        matches!(
            test.oracle_strength,
            OracleStrength::Weak
                | OracleStrength::Smoke
                | OracleStrength::None
                | OracleStrength::Unknown
        )
    })
}

fn recommendation_for(
    entry: &ClassifiedSeam,
    missing_records: &[crate::output::agent_seam_packets::MissingRecord],
    actionability: &EvidenceRecordActionability,
) -> EvidenceRecordRecommendation {
    let actionable = actionability.has_concrete_guidance;
    let static_limited = actionability.class == "static_limitation";
    let action = if actionable {
        "write_targeted_test"
    } else if static_limited {
        "inspect_static_limitation"
    } else {
        "no_action"
    };

    let recommended_test = actionable.then(|| recommended_test_record(recommended_test_for(entry)));
    let assertion_shape = actionable.then(|| {
        assertion_shape_record(assertion_shape_for(
            entry.seam.kind(),
            entry.seam.owner(),
            &entry.evidence,
        ))
    });
    let verify_command = actionable.then(|| VERIFY_COMMAND.to_string());
    let nearest_test_to_imitate = nearest_strong_test_to_imitate(&entry.evidence)
        .or_else(|| entry.evidence.related_tests.first())
        .map(|test| related_test_record(test, entry.seam.kind()));

    EvidenceRecordRecommendation {
        action: action.to_string(),
        reason: actionability.reason.clone(),
        recommended_test,
        nearest_test_to_imitate,
        candidate_values: candidate_values_for(entry, missing_records)
            .into_iter()
            .map(candidate_value_record)
            .collect(),
        assertion_shape,
        verify_command,
    }
}

fn recommended_test_record(test: RecommendedTest) -> EvidenceRecordRecommendedTest {
    EvidenceRecordRecommendedTest {
        name: test.name,
        file: test.file,
        reason: test.reason,
    }
}

fn candidate_value_record(value: CandidateValue) -> EvidenceRecordCandidateValue {
    EvidenceRecordCandidateValue {
        value: value.value,
        reason: value.reason,
    }
}

fn assertion_shape_record(shape: AssertionShape) -> EvidenceRecordAssertionShape {
    EvidenceRecordAssertionShape {
        kind: shape.kind.to_string(),
        example: shape.example,
    }
}

fn related_test_record(
    test: &crate::analysis::test_grip_evidence::RelatedTestGrip,
    seam_kind: SeamKind,
) -> EvidenceRecordRelatedTest {
    let semantics = oracle_semantics_record(&test.oracle_kind, &test.oracle_strength, seam_kind);
    EvidenceRecordRelatedTest {
        name: test.test_name.clone(),
        file: display_path(&test.file),
        line: test.line,
        oracle_kind: test.oracle_kind.as_str().to_string(),
        oracle_strength: test.oracle_strength.as_str().to_string(),
        evidence_summary: test.evidence_summary.clone(),
        oracle_semantics: semantics,
        relation_reason: test.relation_reason.as_str().to_string(),
        relation_confidence: test.relation_confidence.as_str().to_string(),
    }
}

fn oracle_semantics_record(
    kind: &OracleKind,
    strength: &OracleStrength,
    seam_kind: SeamKind,
) -> EvidenceRecordOracleSemantics {
    let semantics = oracle_semantics_for(kind, strength, seam_kind);
    EvidenceRecordOracleSemantics {
        observes: semantics.observes,
        missing: semantics.missing,
        upgrade_suggestion: semantics.upgrade_suggestion,
    }
}

fn static_limitations_for(entry: &ClassifiedSeam) -> Vec<EvidenceRecordStaticLimitation> {
    let mut limitations = Vec::new();
    push_stage_limitation(&mut limitations, "reach", &entry.evidence.reach);
    push_stage_limitation(&mut limitations, "activate", &entry.evidence.activate);
    push_stage_limitation(&mut limitations, "propagate", &entry.evidence.propagate);
    push_stage_limitation(&mut limitations, "observe", &entry.evidence.observe);
    push_stage_limitation(
        &mut limitations,
        "discriminate",
        &entry.evidence.discriminate,
    );

    if matches!(entry.class, SeamGripClass::Opaque) {
        let reason =
            "seam is classified opaque; inspect static evidence before writing a focused test";
        let category = static_limitation_category("classification", "opaque", reason);
        limitations.push(EvidenceRecordStaticLimitation {
            stage: "classification".to_string(),
            state: "opaque".to_string(),
            reason: reason.to_string(),
            category: category.to_string(),
            repair_route: static_limitation_repair_route(category).to_string(),
        });
    }

    limitations
}

fn push_stage_limitation(
    limitations: &mut Vec<EvidenceRecordStaticLimitation>,
    stage: &str,
    evidence: &StageEvidence,
) {
    if matches!(evidence.state, StageState::Unknown | StageState::Opaque) {
        let state = evidence.state.as_str();
        let category = static_limitation_category(stage, state, &evidence.summary);
        limitations.push(EvidenceRecordStaticLimitation {
            stage: stage.to_string(),
            state: state.to_string(),
            reason: evidence.summary.clone(),
            category: category.to_string(),
            repair_route: static_limitation_repair_route(category).to_string(),
        });
    }
}

fn static_limitation_category(stage: &str, state: &str, reason: &str) -> &'static str {
    let reason = reason.to_ascii_lowercase();
    if reason.contains("cross-file")
        || reason.contains("cross file")
        || reason.contains("unresolved constant")
        || reason.contains("constant boundary")
    {
        "cross_file_constant_unresolved"
    } else if reason.contains("macro") || reason.contains("generated") {
        "macro_generated_value"
    } else if reason.contains("opaque helper") || reason.contains("opaque fixture") {
        "opaque_helper_call"
    } else if reason.contains("dynamic dispatch") || reason.contains("opaque dispatch") {
        "dynamic_dispatch"
    } else if reason.contains("mock") {
        "unsupported_mock_shape"
    } else if reason.contains("snapshot") {
        "snapshot_field_unknown"
    } else if reason.contains("side effect")
        || reason.contains("side-effect")
        || reason.contains("effect sink")
    {
        "side_effect_sink_unknown"
    } else if reason.contains("no concrete activation values observed")
        || reason.contains("no literal activation values")
    {
        "activation_value_unresolved"
    } else if stage == "classification" || state == "opaque" {
        "opaque_static_evidence"
    } else {
        match stage {
            "reach" => "reachability_static_unknown",
            "activate" => "activation_static_unknown",
            "propagate" => "propagation_static_unknown",
            "observe" => "observation_static_unknown",
            "discriminate" => "discrimination_static_unknown",
            _ => "static_unknown",
        }
    }
}

fn static_limitation_repair_route(category: &str) -> &'static str {
    match category {
        "activation_value_unresolved" => "analysis/value-resolution-audit-fixes",
        "cross_file_constant_unresolved" => "analysis/cross-file-constant-resolution",
        "macro_generated_value" => "analysis/macro-generated-value-fixtures",
        "opaque_helper_call" => "analysis/oracle-semantics-audit-fixes",
        "dynamic_dispatch" => "calibration/runtime-fixtures-v3",
        "unsupported_mock_shape" => "analysis/oracle-semantics-audit-fixes",
        "snapshot_field_unknown" => "analysis/oracle-semantics-audit-fixes",
        "side_effect_sink_unknown" => "analysis/oracle-semantics-audit-fixes",
        "opaque_static_evidence" => "analysis/static-limitation-taxonomy",
        "reachability_static_unknown" => "analysis/related-test-ranking-audit-fixes",
        "activation_static_unknown" => "analysis/static-limitation-taxonomy",
        "propagation_static_unknown" => "analysis/static-limitation-taxonomy",
        "observation_static_unknown" => "analysis/oracle-semantics-audit-fixes",
        "discrimination_static_unknown" => "analysis/oracle-semantics-audit-fixes",
        _ => "analysis/static-limitation-taxonomy",
    }
}

fn stage_json(stage: &EvidenceRecordStage) -> Value {
    json!({
        "state": stage.state.as_str(),
        "confidence": stage.confidence.as_str(),
        "summary": stage.summary.as_str(),
    })
}

fn observed_value_json(value: &EvidenceRecordObservedValue) -> Value {
    json!({
        "value": value.value.as_str(),
        "line": value.line,
        "text": value.text.as_str(),
        "context": value.context.as_str(),
    })
}

fn missing_discriminator_json(missing: &EvidenceRecordMissingDiscriminator) -> Value {
    json!({
        "value": missing.value.as_str(),
        "reason": missing.reason.as_str(),
        "flow_sink": missing.flow_sink.as_ref().map(flow_sink_json),
    })
}

fn flow_sink_json(sink: &EvidenceRecordFlowSink) -> Value {
    json!({
        "kind": sink.kind.as_str(),
        "text": sink.text.as_str(),
        "line": sink.line,
        "owner": sink.owner.as_deref(),
    })
}

fn related_test_json(test: &EvidenceRecordRelatedTest) -> Value {
    json!({
        "name": test.name.as_str(),
        "file": test.file.as_str(),
        "line": test.line,
        "oracle_kind": test.oracle_kind.as_str(),
        "oracle_strength": test.oracle_strength.as_str(),
        "evidence_summary": test.evidence_summary.as_str(),
        "oracle_semantics": oracle_semantics_json(&test.oracle_semantics),
        "relation_reason": test.relation_reason.as_str(),
        "relation_confidence": test.relation_confidence.as_str(),
    })
}

fn oracle_semantics_json(semantics: &EvidenceRecordOracleSemantics) -> Value {
    json!({
        "observes": semantics.observes.as_str(),
        "missing": semantics.missing.as_str(),
        "upgrade_suggestion": semantics.upgrade_suggestion.as_deref(),
    })
}

fn recommendation_json(recommendation: &EvidenceRecordRecommendation) -> Value {
    json!({
        "action": recommendation.action.as_str(),
        "reason": recommendation.reason.as_str(),
        "recommended_test": recommendation
            .recommended_test
            .as_ref()
            .map(recommended_test_json),
        "nearest_test_to_imitate": recommendation
            .nearest_test_to_imitate
            .as_ref()
            .map(related_test_json),
        "candidate_values": recommendation
            .candidate_values
            .iter()
            .map(candidate_value_json)
            .collect::<Vec<_>>(),
        "assertion_shape": recommendation
            .assertion_shape
            .as_ref()
            .map(assertion_shape_json),
        "verify_command": recommendation.verify_command.as_deref(),
    })
}

fn recommended_test_json(test: &EvidenceRecordRecommendedTest) -> Value {
    json!({
        "name": test.name.as_str(),
        "file": test.file.as_str(),
        "reason": test.reason.as_str(),
    })
}

fn candidate_value_json(value: &EvidenceRecordCandidateValue) -> Value {
    json!({
        "value": value.value.as_str(),
        "reason": value.reason.as_str(),
    })
}

fn assertion_shape_json(shape: &EvidenceRecordAssertionShape) -> Value {
    json!({
        "kind": shape.kind.as_str(),
        "example": shape.example.as_str(),
    })
}

fn actionability_json(actionability: &EvidenceRecordActionability) -> Value {
    json!({
        "class": actionability.class.as_str(),
        "reason": actionability.reason.as_str(),
        "has_concrete_guidance": actionability.has_concrete_guidance,
        "signals": {
            "missing_discriminator": actionability.signals.missing_discriminator,
            "candidate_value": actionability.signals.candidate_value,
            "assertion_shape": actionability.signals.assertion_shape,
            "related_test": actionability.signals.related_test,
            "recommended_test_target": actionability.signals.recommended_test_target,
            "verification_command": actionability.signals.verification_command,
        },
    })
}

fn static_limitation_json(limitation: &EvidenceRecordStaticLimitation) -> Value {
    json!({
        "stage": limitation.stage.as_str(),
        "state": limitation.state.as_str(),
        "reason": limitation.reason.as_str(),
        "category": limitation.category.as_str(),
        "repair_route": limitation.repair_route.as_str(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::domain::{
        Confidence, FlowSinkFact, FlowSinkKind, MissingDiscriminatorFact, OracleKind, ValueContext,
        ValueFact,
    };
    use std::path::PathBuf;

    fn stage(state: StageState, summary: &str) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, summary)
    }

    fn sample_classified(activate_state: StageState, class: SeamGripClass) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        ClassifiedSeam {
            evidence: TestGripEvidence {
                seam_id: seam.id().clone(),
                related_tests: vec![RelatedTestGrip {
                    test_name: "below_threshold_has_no_discount".to_string(),
                    file: PathBuf::from("tests/pricing_tests.rs"),
                    line: 12,
                    oracle_kind: OracleKind::BroadError,
                    oracle_strength: OracleStrength::Weak,
                    evidence_summary: "broad assertion".to_string(),
                    relation_reason: RelationReason::DirectOwnerCall,
                    relation_confidence: RelationConfidence::High,
                }],
                reach: stage(StageState::Yes, "owner is reached"),
                activate: stage(activate_state, "activation evidence unavailable"),
                propagate: stage(StageState::Yes, "return value flow"),
                observe: stage(StageState::Yes, "assertion observes output"),
                discriminate: stage(StageState::Weak, "broad assertion misses boundary"),
                observed_values: vec![ValueFact {
                    line: 12,
                    text: "discounted_total(50, 100)".to_string(),
                    value: "50".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "discount_threshold (equality boundary)".to_string(),
                    reason: "observed values do not include the equality-boundary case".to_string(),
                    flow_sink: Some(FlowSinkFact {
                        kind: FlowSinkKind::ReturnValue,
                        text: "return discounted_total".to_string(),
                        line: 88,
                        owner: None,
                    }),
                }],
            },
            seam,
            class,
        }
    }

    #[test]
    fn evidence_record_carries_identity_path_guidance_and_calibration_placeholder() {
        let entry = sample_classified(StageState::Yes, SeamGripClass::WeaklyGripped);
        let seam_id = entry.seam.id().as_str().to_string();
        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["schema_version"], "0.1");
        assert_eq!(json["seam_id"], seam_id);
        assert!(json["canonical_gap_id"].is_null());
        assert!(json["canonical_gap_group_size"].is_null());
        assert!(json["canonical_gap_reason"].is_null());
        assert_eq!(json["owner"], "pricing::discounted_total");
        assert_eq!(json["location"]["file"], "src/pricing.rs");
        assert_eq!(json["location"]["line"], 88);
        assert_eq!(json["seam_kind"], "predicate_boundary");
        assert_eq!(json["grip_class"], "weakly_gripped");
        assert_eq!(json["headline_eligible"], true);
        assert_eq!(json["evidence_path"]["activate"]["state"], "yes");
        assert_eq!(json["observed_values"][0]["context"], "function_argument");
        assert_eq!(
            json["missing_discriminators"][0]["flow_sink"]["kind"],
            "return_value"
        );
        assert_eq!(
            json["related_tests"][0]["name"],
            "below_threshold_has_no_discount"
        );
        assert_eq!(
            json["related_tests"][0]["oracle_semantics"]["observes"],
            "some error occurred"
        );
        assert_eq!(
            json["related_tests"][0]["oracle_semantics"]["missing"],
            "the exact error variant or payload that would discriminate the changed behavior"
        );
        assert_eq!(
            json["related_tests"][0]["oracle_semantics"]["upgrade_suggestion"],
            "add an exact returned-value assertion at the missing boundary value"
        );
        assert_eq!(
            json["recommendation"]["assertion_shape"]["kind"],
            "exact_return_value"
        );
        assert_eq!(
            json["actionability"]["class"],
            "actionable_assertion_upgrade"
        );
        assert_eq!(json["calibration"]["agreement"], "no_runtime_data");
        assert_eq!(json["static_limitations"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn evidence_record_names_static_limitations_from_unknown_stages() {
        let record = evidence_record_for(
            &sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown),
            None,
        );
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(
            json["recommendation"]["action"],
            "inspect_static_limitation"
        );
        assert_eq!(json["recommendation"]["verify_command"], Value::Null);
        assert_eq!(json["static_limitations"][0]["stage"], "activate");
        assert_eq!(json["static_limitations"][0]["state"], "unknown");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_static_unknown"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/static-limitation-taxonomy"
        );
    }

    #[test]
    fn evidence_record_marks_opaque_seams_as_static_limitation_work() {
        let record = evidence_record_for(
            &sample_classified(StageState::Opaque, SeamGripClass::Opaque),
            None,
        );
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["static_limitations"][0]["stage"], "activate");
        assert_eq!(json["static_limitations"][1]["stage"], "classification");
        assert_eq!(
            json["static_limitations"][1]["category"],
            "opaque_static_evidence"
        );
        assert_eq!(
            json["static_limitations"][1]["repair_route"],
            "analysis/static-limitation-taxonomy"
        );
    }

    #[test]
    fn evidence_record_normalizes_static_limitation_categories() {
        for (stage, state, reason, expected) in [
            (
                "activate",
                "unknown",
                "No concrete activation values observed for seam `threshold`",
                "activation_value_unresolved",
            ),
            (
                "activate",
                "unknown",
                "cross-file constant boundary is unresolved",
                "cross_file_constant_unresolved",
            ),
            (
                "activate",
                "unknown",
                "macro generated value hides literal",
                "macro_generated_value",
            ),
            (
                "observe",
                "unknown",
                "opaque helper hides field",
                "opaque_helper_call",
            ),
            (
                "propagate",
                "unknown",
                "dynamic dispatch target is opaque",
                "dynamic_dispatch",
            ),
            (
                "observe",
                "unknown",
                "mock expectation shape is unsupported",
                "unsupported_mock_shape",
            ),
            (
                "observe",
                "unknown",
                "snapshot field is unknown",
                "snapshot_field_unknown",
            ),
            (
                "propagate",
                "unknown",
                "side-effect sink is unknown",
                "side_effect_sink_unknown",
            ),
            (
                "classification",
                "opaque",
                "seam is classified opaque",
                "opaque_static_evidence",
            ),
            (
                "reach",
                "unknown",
                "no related tests",
                "reachability_static_unknown",
            ),
            (
                "activate",
                "unknown",
                "missing fact",
                "activation_static_unknown",
            ),
            (
                "propagate",
                "unknown",
                "missing sink",
                "propagation_static_unknown",
            ),
            (
                "observe",
                "unknown",
                "missing oracle",
                "observation_static_unknown",
            ),
            (
                "discriminate",
                "unknown",
                "missing exact assertion",
                "discrimination_static_unknown",
            ),
            ("unknown", "unknown", "missing stage", "static_unknown"),
        ] {
            assert_eq!(
                static_limitation_category(stage, state, reason),
                expected,
                "unexpected category for {stage}/{state}: {reason}"
            );
        }

        for (category, expected) in [
            (
                "activation_value_unresolved",
                "analysis/value-resolution-audit-fixes",
            ),
            (
                "cross_file_constant_unresolved",
                "analysis/cross-file-constant-resolution",
            ),
            (
                "macro_generated_value",
                "analysis/macro-generated-value-fixtures",
            ),
            (
                "opaque_helper_call",
                "analysis/oracle-semantics-audit-fixes",
            ),
            ("dynamic_dispatch", "calibration/runtime-fixtures-v3"),
            (
                "unsupported_mock_shape",
                "analysis/oracle-semantics-audit-fixes",
            ),
            (
                "snapshot_field_unknown",
                "analysis/oracle-semantics-audit-fixes",
            ),
            (
                "side_effect_sink_unknown",
                "analysis/oracle-semantics-audit-fixes",
            ),
            (
                "opaque_static_evidence",
                "analysis/static-limitation-taxonomy",
            ),
            (
                "reachability_static_unknown",
                "analysis/related-test-ranking-audit-fixes",
            ),
            (
                "activation_static_unknown",
                "analysis/static-limitation-taxonomy",
            ),
            (
                "propagation_static_unknown",
                "analysis/static-limitation-taxonomy",
            ),
            (
                "observation_static_unknown",
                "analysis/oracle-semantics-audit-fixes",
            ),
            (
                "discrimination_static_unknown",
                "analysis/oracle-semantics-audit-fixes",
            ),
            ("unknown", "analysis/static-limitation-taxonomy"),
        ] {
            assert_eq!(
                static_limitation_repair_route(category),
                expected,
                "unexpected repair route for {category}"
            );
        }
    }

    #[test]
    fn evidence_record_carries_supplied_canonical_gap_identity() {
        let entry = sample_classified(StageState::Yes, SeamGripClass::WeaklyGripped);
        let canonical_gap = CanonicalGapIdentity {
            id: "gap:abc123".to_string(),
            group_size: 3,
            reason: crate::analysis::canonical_gap::CANONICAL_GAP_REASON,
            owner: "pricing::discounted_total".to_string(),
            seam_kind: "predicate_boundary".to_string(),
            flow_sink: "return_value".to_string(),
            missing_discriminator: "amount == threshold".to_string(),
            assertion_shape: "exact_return_value".to_string(),
        };

        let record = evidence_record_for(&entry, Some(&canonical_gap));
        let json = evidence_record_json_value(&record);

        assert_eq!(json["canonical_gap_id"], "gap:abc123");
        assert_eq!(json["canonical_gap_group_size"], 3);
        assert_eq!(
            json["canonical_gap_reason"],
            crate::analysis::canonical_gap::CANONICAL_GAP_REASON
        );
    }
}
