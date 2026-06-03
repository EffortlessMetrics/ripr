//! Shared static evidence record projection for Lane 1 outputs.
//!
//! This module gives repo exposure and downstream advisory reports a single
//! seam-native evidence shape. It is a projection over existing analyzer facts
//! only: it does not run mutation testing, make policy decisions, mutate
//! baselines, or change seam grip classifications.

use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_AGENT_VERIFY_ARTIFACT, agent_receipt_command,
};
use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::CanonicalGapIdentity;
use crate::analysis::seams::{SeamGripClass, SeamKind};
use crate::analysis::test_grip_evidence::{RelationReason, oracle_semantics_for};
use crate::domain::{OracleKind, OracleStrength, StageEvidence, StageState};
use crate::output::agent_seam_packets::{
    AssertionShape, CandidateValue, RecommendedTest, assertion_shape_for_entry,
    candidate_values_for, missing_discriminator_records_for, nearest_strong_test_to_imitate,
    recommended_test_for,
};
use serde_json::{Value, json};

pub(crate) const EVIDENCE_RECORD_SCHEMA_VERSION: &str = "0.1";
pub(crate) const CROSS_LANGUAGE_ORACLE_VISIBILITY_CATEGORY: &str =
    "cross_language_oracle_visibility_unresolved";
pub(crate) const CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE: &str =
    "analysis/cross-language-oracle-visibility";
pub(crate) const CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY: &str =
    "cross_language_target_unresolved";
pub(crate) const CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE: &str =
    "analysis/cross-language-test-target-inference";

const MAX_RELATED_TESTS_PER_EVIDENCE_RECORD: usize = 8;
const VERIFY_COMMAND: &str = "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecord {
    pub(crate) seam_id: String,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) canonical_gap_group_size: Option<usize>,
    pub(crate) canonical_gap_reason: Option<String>,
    pub(crate) raw_findings: Vec<EvidenceRecordRawFinding>,
    pub(crate) canonical_item: EvidenceRecordCanonicalItem,
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
    pub(crate) presentation_text: Option<EvidenceRecordPresentationText>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordLocation {
    pub(crate) file: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordRawFinding {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) kind: String,
    pub(crate) expression: String,
    pub(crate) probe_kind: String,
    pub(crate) source_id: String,
    pub(crate) evidence_record_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordCanonicalItem {
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) raw_group_size: usize,
    pub(crate) canonical_item_kind: String,
    pub(crate) evidence_class: String,
    pub(crate) gap_state: String,
    pub(crate) actionability: String,
    pub(crate) group_reason: Option<String>,
    pub(crate) primary_anchor: Option<EvidenceRecordPrimaryAnchor>,
    pub(crate) raw_spans: Vec<EvidenceRecordRawSpan>,
    pub(crate) static_limitations: Vec<EvidenceRecordStaticLimitation>,
    pub(crate) why: String,
    pub(crate) recommended_repair: String,
    pub(crate) repair_route: Option<EvidenceRecordCanonicalRepairRoute>,
    pub(crate) related_test: Option<EvidenceRecordAlignmentRelatedTest>,
    pub(crate) verify_command: Option<String>,
    pub(crate) receipt_command: Option<String>,
    pub(crate) confidence: EvidenceRecordAlignmentConfidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordPrimaryAnchor {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) kind: String,
    pub(crate) source_id: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordRawSpan {
    pub(crate) file: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) kind: String,
    pub(crate) source_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordCanonicalRepairRoute {
    pub(crate) repair_kind: String,
    pub(crate) target_test_type: String,
    pub(crate) suggested_assertion: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordAlignmentRelatedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordAlignmentConfidence {
    pub(crate) basis: String,
    pub(crate) notes: Vec<String>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecordPresentationText {
    pub(crate) visibility: String,
    pub(crate) observer: String,
    pub(crate) actionability: String,
    pub(crate) source_kind: String,
    pub(crate) canonical_group_reason: Option<String>,
    pub(crate) recommended_observer: String,
}

pub(crate) fn evidence_record_for(
    entry: &ClassifiedSeam,
    canonical_gap: Option<&CanonicalGapIdentity>,
) -> EvidenceRecord {
    let missing_records = missing_discriminator_records_for(entry);
    let actionability = actionability_for(entry, &missing_records);
    let recommendation = recommendation_for(entry, &missing_records, &actionability);
    let related_tests_total = entry.evidence.related_tests.len();
    let static_limitations = static_limitations_for(entry);
    let raw_findings = raw_findings_for(entry);
    let canonical_item = canonical_item_for(
        entry,
        canonical_gap,
        &recommendation,
        &actionability,
        &static_limitations,
        &raw_findings,
    );

    EvidenceRecord {
        seam_id: entry.seam.id().as_str().to_string(),
        canonical_gap_id: canonical_gap.map(|gap| gap.id.clone()),
        canonical_gap_group_size: canonical_gap.map(|gap| gap.group_size),
        canonical_gap_reason: canonical_gap.map(|gap| gap.reason.to_string()),
        raw_findings,
        canonical_item,
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
        static_limitations,
        presentation_text: None,
    }
}

pub(crate) fn evidence_record_json_value(record: &EvidenceRecord) -> Value {
    json!({
        "schema_version": EVIDENCE_RECORD_SCHEMA_VERSION,
        "seam_id": record.seam_id.as_str(),
        "canonical_gap_id": record.canonical_gap_id.as_deref(),
        "canonical_gap_group_size": record.canonical_gap_group_size,
        "canonical_gap_reason": record.canonical_gap_reason.as_deref(),
        "raw_findings": record
            .raw_findings
            .iter()
            .map(raw_finding_json)
            .collect::<Vec<_>>(),
        "canonical_item": canonical_item_json(&record.canonical_item),
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
        "presentation_text": record
            .presentation_text
            .as_ref()
            .map_or(Value::Null, presentation_text_json),
    })
}

use crate::output::path::display_path;

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

fn raw_findings_for(entry: &ClassifiedSeam) -> Vec<EvidenceRecordRawFinding> {
    vec![EvidenceRecordRawFinding {
        file: display_path(entry.seam.file()),
        line: entry.seam.display_line(),
        kind: entry.class.as_str().to_string(),
        expression: entry.seam.expression().to_string(),
        probe_kind: entry.seam.kind().as_str().to_string(),
        source_id: entry.seam.id().as_str().to_string(),
        evidence_record_ref: entry.seam.id().as_str().to_string(),
    }]
}

fn canonical_item_for(
    entry: &ClassifiedSeam,
    canonical_gap: Option<&CanonicalGapIdentity>,
    recommendation: &EvidenceRecordRecommendation,
    actionability: &EvidenceRecordActionability,
    static_limitations: &[EvidenceRecordStaticLimitation],
    raw_findings: &[EvidenceRecordRawFinding],
) -> EvidenceRecordCanonicalItem {
    let gap_state = gap_state_for(entry, actionability);
    let canonical_item_kind = canonical_item_kind_for(gap_state);
    let alignment_actionability = alignment_actionability_for(entry, actionability);
    let raw_group_size = canonical_gap
        .map(|gap| gap.group_size)
        .unwrap_or(raw_findings.len());
    let group_reason = canonical_gap.map(|gap| gap.reason.to_string());

    EvidenceRecordCanonicalItem {
        canonical_gap_id: canonical_gap.map(|gap| gap.id.clone()),
        raw_group_size,
        canonical_item_kind: canonical_item_kind.to_string(),
        evidence_class: entry.seam.kind().as_str().to_string(),
        gap_state: gap_state.to_string(),
        actionability: alignment_actionability.to_string(),
        group_reason: group_reason.clone(),
        primary_anchor: primary_anchor_for(raw_findings, group_reason.as_deref()),
        raw_spans: raw_spans_for(raw_findings),
        static_limitations: static_limitations.to_vec(),
        why: actionability.reason.clone(),
        recommended_repair: recommended_repair_for(
            gap_state,
            recommendation,
            actionability,
            static_limitations,
        ),
        repair_route: canonical_repair_route_for(entry, recommendation, gap_state),
        related_test: recommendation
            .nearest_test_to_imitate
            .as_ref()
            .map(alignment_related_test_for)
            .or_else(|| {
                recommendation
                    .recommended_test
                    .as_ref()
                    .map(alignment_related_test_for_recommended_target)
            }),
        verify_command: recommendation.verify_command.clone(),
        receipt_command: canonical_receipt_command_for(entry, gap_state),
        confidence: alignment_confidence_for(gap_state, static_limitations),
    }
}

fn alignment_related_test_for_recommended_target(
    test: &EvidenceRecordRecommendedTest,
) -> EvidenceRecordAlignmentRelatedTest {
    EvidenceRecordAlignmentRelatedTest {
        name: test.name.clone(),
        file: test.file.clone(),
        line: 1,
        reason: "recommended_test_target".to_string(),
    }
}

fn primary_anchor_for(
    raw_findings: &[EvidenceRecordRawFinding],
    group_reason: Option<&str>,
) -> Option<EvidenceRecordPrimaryAnchor> {
    raw_findings
        .first()
        .map(|finding| EvidenceRecordPrimaryAnchor {
            file: finding.file.clone(),
            line: finding.line,
            kind: finding.kind.clone(),
            source_id: finding.source_id.clone(),
            reason: primary_anchor_reason(group_reason).to_string(),
        })
}

fn primary_anchor_reason(group_reason: Option<&str>) -> &'static str {
    if group_reason.is_some() {
        "canonical_group_primary_raw_finding"
    } else {
        "record_location"
    }
}

fn raw_spans_for(raw_findings: &[EvidenceRecordRawFinding]) -> Vec<EvidenceRecordRawSpan> {
    raw_findings
        .iter()
        .map(|finding| EvidenceRecordRawSpan {
            file: finding.file.clone(),
            start_line: finding.line,
            end_line: finding.line,
            kind: finding.kind.clone(),
            source_id: finding.source_id.clone(),
        })
        .collect()
}

fn gap_state_for(
    entry: &ClassifiedSeam,
    actionability: &EvidenceRecordActionability,
) -> &'static str {
    if actionability.class == "static_limitation" {
        "static_limitation"
    } else if actionability.has_concrete_guidance {
        "actionable"
    } else if matches!(entry.class, SeamGripClass::StronglyGripped) {
        "already_observed"
    } else if matches!(entry.class, SeamGripClass::Intentional) {
        "internal_only"
    } else {
        "unknown"
    }
}

fn canonical_item_kind_for(gap_state: &str) -> &'static str {
    match gap_state {
        "actionable" => "gap",
        "already_observed" => "observed",
        "internal_only" => "no_action",
        "static_limitation" => "limitation",
        _ => "evidence",
    }
}

fn alignment_actionability_for(
    entry: &ClassifiedSeam,
    actionability: &EvidenceRecordActionability,
) -> &'static str {
    match actionability.class.as_str() {
        "actionable_focused_test" => "add_focused_test",
        "actionable_assertion_upgrade" => "upgrade_assertion",
        "actionable_related_test_extension" => "extend_related_test",
        "static_limitation" => "static_limitation",
        "not_policy_relevant" if matches!(entry.class, SeamGripClass::StronglyGripped) => {
            "already_observed"
        }
        "not_policy_relevant" if matches!(entry.class, SeamGripClass::Intentional) => "no_action",
        _ => "unknown",
    }
}

fn recommended_repair_for(
    gap_state: &str,
    recommendation: &EvidenceRecordRecommendation,
    actionability: &EvidenceRecordActionability,
    static_limitations: &[EvidenceRecordStaticLimitation],
) -> String {
    if actionability.has_concrete_guidance {
        return actionable_repair_summary(recommendation)
            .unwrap_or_else(|| recommendation.reason.clone());
    }

    match gap_state {
        "already_observed" => {
            "No new RIPR action; current static evidence already observes this seam.".to_string()
        }
        "internal_only" => "No user test action in documented scope.".to_string(),
        "static_limitation" => static_limitations.first().map_or_else(
            || recommendation.reason.clone(),
            |limitation| {
                format!(
                    "Inspect static limitation `{}` via `{}`.",
                    limitation.category, limitation.repair_route
                )
            },
        ),
        _ => recommendation.reason.clone(),
    }
}

fn actionable_repair_summary(recommendation: &EvidenceRecordRecommendation) -> Option<String> {
    let assertion = recommendation.assertion_shape.as_ref()?;
    let mut summary = format!("Add or strengthen `{}`", assertion.example);
    if let Some(candidate) = recommendation.candidate_values.first()
        && !candidate.value.trim().is_empty()
    {
        summary.push_str(&format!(" for `{}`", candidate.value.trim()));
    }
    if let Some(target) = recommendation.recommended_test.as_ref() {
        summary.push_str(&format!(" in `{}` as `{}`", target.file, target.name));
    }
    summary.push('.');
    Some(summary)
}

fn canonical_repair_route_for(
    entry: &ClassifiedSeam,
    recommendation: &EvidenceRecordRecommendation,
    gap_state: &str,
) -> Option<EvidenceRecordCanonicalRepairRoute> {
    if gap_state != "actionable" {
        return None;
    }

    let assertion_shape = recommendation.assertion_shape.as_ref()?;
    Some(EvidenceRecordCanonicalRepairRoute {
        repair_kind: canonical_repair_kind_for(entry.seam.kind()).to_string(),
        target_test_type: canonical_target_test_type_for(entry.seam.kind()).to_string(),
        suggested_assertion: assertion_shape.example.clone(),
    })
}

fn canonical_repair_kind_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "add_boundary_assertion",
        SeamKind::ErrorVariant => "add_exact_error_variant",
        SeamKind::ReturnValue => "add_return_value_assertion",
        SeamKind::FieldConstruction => "add_field_assertion",
        SeamKind::SideEffect => "add_side_effect_observer",
        SeamKind::MatchArm => "add_match_arm_assertion",
        SeamKind::CallPresence => "add_call_observer",
    }
}

fn canonical_target_test_type_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "boundary_discriminator",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "return_value_discriminator",
        SeamKind::FieldConstruction => "field_value_discriminator",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_arm_discriminator",
        SeamKind::CallPresence => "call_presence_observer",
    }
}

fn alignment_related_test_for(
    test: &EvidenceRecordRelatedTest,
) -> EvidenceRecordAlignmentRelatedTest {
    EvidenceRecordAlignmentRelatedTest {
        name: test.name.clone(),
        file: test.file.clone(),
        line: test.line,
        reason: test.relation_reason.clone(),
    }
}

fn alignment_confidence_for(
    gap_state: &str,
    static_limitations: &[EvidenceRecordStaticLimitation],
) -> EvidenceRecordAlignmentConfidence {
    let mut notes = vec!["no imported runtime calibration data".to_string()];
    notes.extend(
        static_limitations
            .iter()
            .map(|limitation| format!("static limitation: {}", limitation.category)),
    );

    EvidenceRecordAlignmentConfidence {
        basis: if gap_state == "unknown" {
            "unknown".to_string()
        } else {
            "static_only".to_string()
        },
        notes,
    }
}

fn is_static_limited(entry: &ClassifiedSeam) -> bool {
    cross_language_oracle_visibility_unresolved(entry)
        || cross_language_test_target_unresolved(entry)
        || matches!(entry.class, SeamGripClass::Opaque)
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

pub(crate) fn cross_language_oracle_visibility_unresolved(entry: &ClassifiedSeam) -> bool {
    if !entry.class.is_headline_eligible() && !matches!(entry.class, SeamGripClass::Opaque) {
        return false;
    }

    if has_external_language_related_test(entry) {
        return true;
    }

    cross_language_surface_hint(entry) && !has_rust_related_test(entry)
}

pub(crate) fn cross_language_test_target_unresolved(entry: &ClassifiedSeam) -> bool {
    if !entry.class.is_headline_eligible() && !matches!(entry.class, SeamGripClass::Opaque) {
        return false;
    }

    (has_external_language_related_test(entry) || cross_language_surface_hint(entry))
        && !has_rust_side_target_context(entry)
}

fn has_rust_related_test(entry: &ClassifiedSeam) -> bool {
    entry
        .evidence
        .related_tests
        .iter()
        .any(related_test_is_rust)
}

fn has_rust_side_target_context(entry: &ClassifiedSeam) -> bool {
    entry.evidence.related_tests.iter().any(|test| {
        related_test_is_rust(test)
            && matches!(
                test.relation_reason,
                RelationReason::DirectOwnerCall | RelationReason::HelperOwnerCall
            )
    })
}

fn related_test_is_rust(test: &crate::analysis::test_grip_evidence::RelatedTestGrip) -> bool {
    test.file
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

fn has_external_language_related_test(entry: &ClassifiedSeam) -> bool {
    entry.evidence.related_tests.iter().any(|test| {
        test.file
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "rb" | "java"
                )
            })
    })
}

fn cross_language_surface_hint(entry: &ClassifiedSeam) -> bool {
    let file = display_path(entry.seam.file()).to_ascii_lowercase();
    let owner = entry.seam.owner().to_ascii_lowercase();
    let expression = entry.seam.expression().to_ascii_lowercase();

    path_has_cross_language_segment(&file)
        || text_has_cross_language_marker(&owner)
        || text_has_cross_language_marker(&expression)
}

fn path_has_cross_language_segment(file: &str) -> bool {
    let normalized = file.replace('\\', "/");
    normalized.split('/').any(|segment| {
        matches!(
            segment,
            "ffi"
                | "binding"
                | "bindings"
                | "napi"
                | "neon"
                | "wasm"
                | "wasm_bindgen"
                | "pyo3"
                | "python"
                | "jni"
                | "uniffi"
                | "cxx"
                | "node"
                | "jsc"
                | "javascriptcore"
        )
    })
}

fn text_has_cross_language_marker(text: &str) -> bool {
    [
        "from_js",
        "to_js",
        "jsvalue",
        "js_value",
        "jsc::",
        "javascriptcore",
        "napi",
        "wasm_bindgen",
        "extern \"c\"",
        "extern_c",
        "no_mangle",
        "export_name",
        "ffi",
        "binding",
        "pyo3",
        "python",
        "node_api",
        "jni",
        "uniffi",
        "cxx::bridge",
    ]
    .iter()
    .any(|marker| text.contains(marker))
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
    let assertion_shape =
        actionable.then(|| assertion_shape_record(assertion_shape_for_entry(entry)));
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

fn canonical_receipt_command_for(entry: &ClassifiedSeam, gap_state: &str) -> Option<String> {
    if gap_state != "actionable" {
        return None;
    }

    Some(agent_receipt_command(
        ".",
        WORKFLOW_AGENT_VERIFY_ARTIFACT,
        entry.seam.id().as_str(),
        Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
    ))
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
    if cross_language_oracle_visibility_unresolved(entry) {
        limitations.push(EvidenceRecordStaticLimitation {
            stage: "cross_language_oracle_visibility".to_string(),
            state: "unknown".to_string(),
            reason: "Rust seam is on a binding, FFI, or external-language surface, and RIPR cannot prove the external oracle path from Rust static evidence.".to_string(),
            category: CROSS_LANGUAGE_ORACLE_VISIBILITY_CATEGORY.to_string(),
            repair_route: CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE.to_string(),
        });
    }
    if cross_language_test_target_unresolved(entry) {
        limitations.push(EvidenceRecordStaticLimitation {
            stage: "cross_language_test_target".to_string(),
            state: "unknown".to_string(),
            reason: "Rust seam is on a binding, FFI, or external-language surface without Rust-side test context, so RIPR cannot select a safe repair target."
                .to_string(),
            category: CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY.to_string(),
            repair_route: CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE.to_string(),
        });
    }
    push_stage_limitation(&mut limitations, "reach", &entry.evidence.reach, entry);
    push_stage_limitation(
        &mut limitations,
        "activate",
        &entry.evidence.activate,
        entry,
    );
    push_stage_limitation(
        &mut limitations,
        "propagate",
        &entry.evidence.propagate,
        entry,
    );
    push_stage_limitation(&mut limitations, "observe", &entry.evidence.observe, entry);
    push_stage_limitation(
        &mut limitations,
        "discriminate",
        &entry.evidence.discriminate,
        entry,
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
    entry: &ClassifiedSeam,
) {
    if matches!(evidence.state, StageState::Unknown | StageState::Opaque) {
        let state = evidence.state.as_str();
        let base_category = static_limitation_category(stage, state, &evidence.summary);
        let category = static_limitation_category_for_entry(base_category, entry);
        limitations.push(EvidenceRecordStaticLimitation {
            stage: stage.to_string(),
            state: state.to_string(),
            reason: evidence.summary.clone(),
            category: category.to_string(),
            repair_route: static_limitation_repair_route_for_entry(category, entry).to_string(),
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
    } else if reason.contains("no direct owner call observed for value-insensitive seam") {
        "activation_owner_call_absent"
    } else if reason.contains("owner call") {
        "activation_owner_call_unresolved"
    } else if reason.contains("boundary activation operand")
        && (reason.contains("local")
            || reason.contains("iterator")
            || reason.contains("closure")
            || reason.contains("computed"))
    {
        "activation_boundary_input_unresolved"
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
            _ => "static_limitation_unclassified",
        }
    }
}

fn static_limitation_repair_route(category: &str) -> &'static str {
    match category {
        "activation_owner_call_absent" => "analysis/owner-call-absence-triage",
        "activation_owner_call_absent_call_presence_target_affinity" => {
            "analysis/call-presence-target-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_assertion_target_affinity" => {
            "analysis/assertion-target-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_affinity_only" => {
            "analysis/related-test-affinity-owner-call-tracing"
        }
        "activation_owner_call_absent_same_file_only" => "analysis/same-file-owner-call-tracing",
        "activation_owner_call_unresolved" => "analysis/related-test-ranking-audit-fixes",
        "activation_boundary_input_unresolved" => {
            "analysis/local-computed-boundary-operand-resolution"
        }
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
        "static_limitation_unclassified" => "analysis/static-limitation-taxonomy",
        _ => "analysis/static-limitation-taxonomy",
    }
}

fn static_limitation_category_for_entry(
    category: &'static str,
    entry: &ClassifiedSeam,
) -> &'static str {
    if category == "activation_owner_call_absent" {
        if owner_call_absence_has_call_presence_target_affinity(entry) {
            "activation_owner_call_absent_call_presence_target_affinity"
        } else if owner_call_absence_has_assertion_target_affinity(entry) {
            "activation_owner_call_absent_assertion_target_affinity"
        } else if owner_call_absence_is_same_file_primary(entry) {
            "activation_owner_call_absent_same_file_only"
        } else if owner_call_absence_is_affinity_only(entry) {
            "activation_owner_call_absent_affinity_only"
        } else {
            category
        }
    } else {
        category
    }
}

fn static_limitation_repair_route_for_entry(
    category: &str,
    entry: &ClassifiedSeam,
) -> &'static str {
    if category == "activation_owner_call_absent_same_file_only"
        && owner_call_absence_same_file_call_presence_is_method_chain(entry)
    {
        "analysis/same-file-receiver-method-owner-call-tracing"
    } else if category == "activation_owner_call_absent_assertion_target_affinity"
        && owner_call_absence_assertion_target_is_return_value(entry)
    {
        "analysis/assertion-target-return-value-owner-call-tracing"
    } else if matches!(
        category,
        "activation_owner_call_absent_call_presence_target_affinity"
            | "activation_owner_call_absent_assertion_target_affinity"
            | "activation_owner_call_absent_affinity_only"
            | "activation_owner_call_absent_same_file_only"
    ) {
        static_limitation_repair_route(category)
    } else if category == "activation_owner_call_absent"
        && owner_call_absence_has_call_presence_target_affinity(entry)
    {
        "analysis/call-presence-target-affinity-owner-call-tracing"
    } else if category == "activation_owner_call_absent"
        && owner_call_absence_has_assertion_target_affinity(entry)
    {
        "analysis/assertion-target-affinity-owner-call-tracing"
    } else if category == "activation_owner_call_absent"
        && owner_call_absence_is_affinity_only(entry)
    {
        "analysis/related-test-affinity-owner-call-tracing"
    } else if category == "activation_boundary_input_unresolved"
        && boundary_input_limitation_is_iterator_derived(entry)
    {
        "analysis/iterator-boundary-operand-resolution"
    } else if category == "activation_boundary_input_unresolved"
        && boundary_input_limitation_is_closure_derived(entry)
    {
        "analysis/closure-boundary-operand-resolution"
    } else if category == "activation_boundary_input_unresolved"
        && boundary_input_limitation_has_member_operand(entry)
    {
        "analysis/local-member-boundary-operand-resolution"
    } else {
        static_limitation_repair_route(category)
    }
}

fn owner_call_absence_has_call_presence_target_affinity(entry: &ClassifiedSeam) -> bool {
    entry.seam.kind() == SeamKind::CallPresence
        && owner_call_absence_has_assertion_target_affinity(entry)
}

fn owner_call_absence_has_assertion_target_affinity(entry: &ClassifiedSeam) -> bool {
    entry
        .evidence
        .related_tests
        .iter()
        .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity)
}

fn owner_call_absence_assertion_target_is_return_value(entry: &ClassifiedSeam) -> bool {
    entry.seam.kind() == SeamKind::ReturnValue
        && owner_call_absence_has_assertion_target_affinity(entry)
}

fn owner_call_absence_is_affinity_only(entry: &ClassifiedSeam) -> bool {
    !entry.evidence.related_tests.is_empty()
        && entry.evidence.related_tests.iter().all(|test| {
            !matches!(
                test.relation_reason,
                RelationReason::DirectOwnerCall | RelationReason::HelperOwnerCall
            )
        })
}

fn owner_call_absence_is_same_file_primary(entry: &ClassifiedSeam) -> bool {
    !entry.evidence.related_tests.is_empty()
        && !entry.evidence.related_tests.iter().any(|test| {
            matches!(
                test.relation_reason,
                RelationReason::DirectOwnerCall
                    | RelationReason::HelperOwnerCall
                    | RelationReason::AssertionTargetAffinity
            )
        })
        && entry
            .evidence
            .related_tests
            .first()
            .is_some_and(|test| test.relation_reason == RelationReason::SameTestFile)
}

fn owner_call_absence_same_file_call_presence_is_method_chain(entry: &ClassifiedSeam) -> bool {
    entry.seam.kind() == SeamKind::CallPresence
        && call_presence_expression_is_method_chain(entry.seam.expression())
}

fn call_presence_expression_is_method_chain(expression: &str) -> bool {
    let Some(dot_index) = expression.find('.') else {
        return false;
    };
    let Some(paren_index) = expression.find('(') else {
        return false;
    };
    dot_index < paren_index
}

fn stage_json(stage: &EvidenceRecordStage) -> Value {
    json!({
        "state": stage.state.as_str(),
        "confidence": stage.confidence.as_str(),
        "summary": stage.summary.as_str(),
    })
}

fn boundary_input_limitation_is_iterator_derived(entry: &ClassifiedSeam) -> bool {
    let reason = entry.evidence.activate.summary.to_ascii_lowercase();
    reason.contains("boundary activation operand") && reason.contains("iterator-derived")
}

fn boundary_input_limitation_is_closure_derived(entry: &ClassifiedSeam) -> bool {
    let reason = entry.evidence.activate.summary.to_ascii_lowercase();
    reason.contains("boundary activation operand") && reason.contains("closure-derived")
}

fn boundary_input_limitation_has_member_operand(entry: &ClassifiedSeam) -> bool {
    entry
        .seam
        .expression()
        .split_whitespace()
        .any(token_has_member_access)
}

fn token_has_member_access(token: &str) -> bool {
    let token = token.trim();
    let token = if token.starts_with('(') && token.ends_with(')') {
        let inner = &token[1..token.len().saturating_sub(1)];
        if inner.contains('(') || inner.contains(')') {
            return false;
        }
        inner
    } else if token.contains('(') || token.contains(')') {
        return false;
    } else {
        token
    };
    let token =
        token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '.');
    let mut parts = token.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    if !is_member_operand_part(first) {
        return false;
    }
    let mut member_count = 0usize;
    for part in parts {
        if !is_member_operand_part(part) {
            return false;
        }
        member_count += 1;
    }
    member_count > 0
}

fn is_member_operand_part(part: &str) -> bool {
    let mut chars = part.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
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

fn raw_finding_json(finding: &EvidenceRecordRawFinding) -> Value {
    json!({
        "file": finding.file.as_str(),
        "line": finding.line,
        "kind": finding.kind.as_str(),
        "expression": finding.expression.as_str(),
        "probe_kind": finding.probe_kind.as_str(),
        "source_id": finding.source_id.as_str(),
        "evidence_record_ref": finding.evidence_record_ref.as_str(),
    })
}

fn canonical_item_json(item: &EvidenceRecordCanonicalItem) -> Value {
    json!({
        "canonical_gap_id": item.canonical_gap_id.as_deref(),
        "raw_group_size": item.raw_group_size,
        "canonical_item_kind": item.canonical_item_kind.as_str(),
        "evidence_class": item.evidence_class.as_str(),
        "gap_state": item.gap_state.as_str(),
        "actionability": item.actionability.as_str(),
        "group_reason": item.group_reason.as_deref(),
        "primary_anchor": item
            .primary_anchor
            .as_ref()
            .map_or(Value::Null, primary_anchor_json),
        "raw_spans": item
            .raw_spans
            .iter()
            .map(raw_span_json)
            .collect::<Vec<_>>(),
        "static_limitations": item
            .static_limitations
            .iter()
            .map(static_limitation_json)
            .collect::<Vec<_>>(),
        "why": item.why.as_str(),
        "recommended_repair": item.recommended_repair.as_str(),
        "repair_route": item
            .repair_route
            .as_ref()
            .map_or(Value::Null, canonical_repair_route_json),
        "related_test": item
            .related_test
            .as_ref()
            .map_or(Value::Null, alignment_related_test_json),
        "verify_command": item.verify_command.as_deref(),
        "receipt_command": item.receipt_command.as_deref(),
        "confidence": {
            "basis": item.confidence.basis.as_str(),
            "notes": item
                .confidence
                .notes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        },
    })
}

fn primary_anchor_json(anchor: &EvidenceRecordPrimaryAnchor) -> Value {
    json!({
        "file": anchor.file.as_str(),
        "line": anchor.line,
        "kind": anchor.kind.as_str(),
        "source_id": anchor.source_id.as_str(),
        "reason": anchor.reason.as_str(),
    })
}

fn raw_span_json(span: &EvidenceRecordRawSpan) -> Value {
    json!({
        "file": span.file.as_str(),
        "start_line": span.start_line,
        "end_line": span.end_line,
        "kind": span.kind.as_str(),
        "source_id": span.source_id.as_str(),
    })
}

fn canonical_repair_route_json(route: &EvidenceRecordCanonicalRepairRoute) -> Value {
    json!({
        "repair_kind": route.repair_kind.as_str(),
        "target_test_type": route.target_test_type.as_str(),
        "suggested_assertion": route.suggested_assertion.as_str(),
    })
}

fn alignment_related_test_json(test: &EvidenceRecordAlignmentRelatedTest) -> Value {
    json!({
        "name": test.name.as_str(),
        "file": test.file.as_str(),
        "line": test.line,
        "reason": test.reason.as_str(),
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

fn presentation_text_json(presentation_text: &EvidenceRecordPresentationText) -> Value {
    json!({
        "visibility": presentation_text.visibility.as_str(),
        "observer": presentation_text.observer.as_str(),
        "actionability": presentation_text.actionability.as_str(),
        "source_kind": presentation_text.source_kind.as_str(),
        "canonical_group_reason": presentation_text.canonical_group_reason.as_deref(),
        "recommended_observer": presentation_text.recommended_observer.as_str(),
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

    fn sample_cross_language_classified(
        class: SeamGripClass,
        related_tests: Vec<RelatedTestGrip>,
    ) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/jsc/Blob.rs",
            "Blob::from_js_without_defer_gc",
            SeamKind::PredicateBoundary,
            42,
            88,
            "array_buffer.shared || array_buffer.resizable",
            RequiredDiscriminator::BoundaryValue {
                description: "array_buffer.shared || array_buffer.resizable".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        ClassifiedSeam {
            evidence: TestGripEvidence {
                seam_id: seam.id().clone(),
                related_tests,
                reach: stage(
                    StageState::Yes,
                    "binding seam is visible through external tests",
                ),
                activate: stage(
                    StageState::Yes,
                    "configured binding route reaches the Rust seam",
                ),
                propagate: stage(StageState::Yes, "external call reaches the Rust boundary"),
                observe: stage(
                    StageState::Weak,
                    "external oracle path remains unresolved in Rust static evidence",
                ),
                discriminate: stage(StageState::Weak, "external discriminator is unresolved"),
                observed_values: Vec::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "resizable_array_buffer".to_string(),
                    reason: "external TypeScript oracle visibility is unresolved".to_string(),
                    flow_sink: None,
                }],
            },
            seam,
            class,
        }
    }

    fn external_typescript_related_test() -> RelatedTestGrip {
        RelatedTestGrip {
            test_name: "blob copies shared buffers".to_string(),
            file: PathBuf::from("test/js/web/fetch/blob.test.ts"),
            line: 41,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "TypeScript exact value oracle".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        }
    }

    fn sample_call_presence_classified() -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/agent_paths.rs",
            "agent_paths::zq_call_presence_owner",
            SeamKind::CallPresence,
            1,
            1,
            "zq_render_target_token(path)",
            RequiredDiscriminator::CallSite {
                target: "zq_render_target_token".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        ClassifiedSeam {
            evidence: TestGripEvidence {
                seam_id: seam.id().clone(),
                related_tests: vec![RelatedTestGrip {
                    test_name: "target_token_affinity_is_not_owner_call".to_string(),
                    file: PathBuf::from("tests/target_affinity.rs"),
                    line: 9,
                    oracle_kind: OracleKind::BroadError,
                    oracle_strength: OracleStrength::Weak,
                    evidence_summary: "assertion mentions call target token".to_string(),
                    relation_reason: RelationReason::AssertionTargetAffinity,
                    relation_confidence: RelationConfidence::Medium,
                }],
                reach: stage(StageState::Yes, "owner is reached"),
                activate: stage(
                    StageState::Unknown,
                    "No direct owner call observed for value-insensitive seam `zq_render_target_token(path)`",
                ),
                propagate: stage(StageState::Yes, "call presence can propagate"),
                observe: stage(StageState::Yes, "assertion observes output"),
                discriminate: stage(StageState::Weak, "broad assertion mentions target"),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
            seam,
            class: SeamGripClass::ActivationUnknown,
        }
    }

    fn sample_actionability(
        class: &str,
        reason: &str,
        has_concrete_guidance: bool,
    ) -> EvidenceRecordActionability {
        EvidenceRecordActionability {
            class: class.to_string(),
            reason: reason.to_string(),
            has_concrete_guidance,
            signals: EvidenceRecordActionabilitySignals {
                missing_discriminator: has_concrete_guidance,
                candidate_value: has_concrete_guidance,
                assertion_shape: has_concrete_guidance,
                related_test: has_concrete_guidance,
                recommended_test_target: has_concrete_guidance,
                verification_command: has_concrete_guidance,
            },
        }
    }

    fn sample_recommendation(reason: &str) -> EvidenceRecordRecommendation {
        EvidenceRecordRecommendation {
            action: "no_action".to_string(),
            reason: reason.to_string(),
            recommended_test: None,
            nearest_test_to_imitate: None,
            candidate_values: Vec::new(),
            assertion_shape: None,
            verify_command: None,
        }
    }

    fn sample_static_limitation(
        category: &str,
        repair_route: &str,
    ) -> EvidenceRecordStaticLimitation {
        EvidenceRecordStaticLimitation {
            stage: "observe".to_string(),
            state: "unknown".to_string(),
            reason: "helper hides assertion".to_string(),
            category: category.to_string(),
            repair_route: repair_route.to_string(),
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
        assert_eq!(json["raw_findings"][0]["file"], "src/pricing.rs");
        assert_eq!(json["raw_findings"][0]["line"], 88);
        assert_eq!(json["raw_findings"][0]["kind"], "weakly_gripped");
        assert_eq!(
            json["raw_findings"][0]["expression"],
            "amount >= discount_threshold"
        );
        assert_eq!(json["raw_findings"][0]["probe_kind"], "predicate_boundary");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "gap");
        assert_eq!(
            json["canonical_item"]["evidence_class"],
            "predicate_boundary"
        );
        assert_eq!(json["canonical_item"]["gap_state"], "actionable");
        assert_eq!(json["canonical_item"]["actionability"], "upgrade_assertion");
        assert_eq!(
            json["canonical_item"]["primary_anchor"]["file"],
            "src/pricing.rs"
        );
        assert_eq!(json["canonical_item"]["primary_anchor"]["line"], 88);
        assert_eq!(
            json["canonical_item"]["primary_anchor"]["reason"],
            "record_location"
        );
        assert_eq!(
            json["canonical_item"]["raw_spans"][0]["start_line"],
            json["raw_findings"][0]["line"]
        );
        assert_eq!(
            json["canonical_item"]["raw_spans"][0]["end_line"],
            json["raw_findings"][0]["line"]
        );
        assert_eq!(
            json["canonical_item"]["repair_route"]["repair_kind"],
            "add_boundary_assertion"
        );
        assert_eq!(
            json["canonical_item"]["repair_route"]["target_test_type"],
            "boundary_discriminator"
        );
        assert_eq!(
            json["canonical_item"]["repair_route"]["suggested_assertion"],
            "assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)"
        );
        assert_eq!(
            json["canonical_item"]["recommended_repair"],
            "Add or strengthen `assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)` for `input that hits the boundary: amount >= discount_threshold` in `tests/pricing_tests.rs` as `discounted_total_boundary_discriminator`."
        );
        assert_eq!(
            json["canonical_item"]["related_test"]["name"],
            "below_threshold_has_no_discount"
        );
        assert_eq!(json["canonical_item"]["confidence"]["basis"], "static_only");
        assert_eq!(json["canonical_item"]["verify_command"], VERIFY_COMMAND);
        let expected_receipt_command = agent_receipt_command(
            ".",
            WORKFLOW_AGENT_VERIFY_ARTIFACT,
            &seam_id,
            Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
        );
        assert_eq!(
            json["canonical_item"]["receipt_command"],
            expected_receipt_command
        );
        assert!(json["presentation_text"].is_null());
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
    fn evidence_record_routes_ts_tested_rust_binding_seam_to_cross_language_limitation() {
        let entry = sample_cross_language_classified(
            SeamGripClass::WeaklyGripped,
            vec![external_typescript_related_test()],
        );
        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["actionability"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["verify_command"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["raw_findings"][0]["file"], "src/jsc/Blob.rs");
        assert_eq!(json["raw_findings"][0]["line"], 88);
        assert_eq!(
            json["static_limitations"][0]["category"],
            CROSS_LANGUAGE_ORACLE_VISIBILITY_CATEGORY
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE
        );
        assert_eq!(
            json["static_limitations"][1]["category"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY
        );
        assert_eq!(
            json["static_limitations"][1]["repair_route"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            CROSS_LANGUAGE_ORACLE_VISIBILITY_CATEGORY
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][1]["category"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY
        );
        assert!(
            json["canonical_item"]["recommended_repair"]
                .as_str()
                .is_some_and(|text| text.contains(CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE))
        );
        assert_eq!(
            json["related_tests"][0]["file"],
            "test/js/web/fetch/blob.test.ts"
        );
    }

    #[test]
    fn evidence_record_routes_binding_seam_without_rust_test_context_to_target_limitation() {
        let entry = sample_cross_language_classified(SeamGripClass::Ungripped, Vec::new());
        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(
            json["static_limitations"][1]["category"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY
        );
        assert_eq!(
            json["static_limitations"][1]["repair_route"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][1]["category"],
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY
        );
    }

    #[test]
    fn evidence_record_names_static_limitations_from_unknown_stages() {
        let record = evidence_record_for(
            &sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown),
            None,
        );
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["actionability"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            "activation_static_unknown"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/static-limitation-taxonomy"
        );
        assert_eq!(
            json["canonical_item"]["recommended_repair"],
            "Inspect static limitation `activation_static_unknown` via `analysis/static-limitation-taxonomy`."
        );
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
    fn evidence_record_carries_recommended_test_as_canonical_related_target() {
        let mut entry = sample_classified(StageState::Yes, SeamGripClass::WeaklyGripped);
        entry.evidence.related_tests.clear();

        let record = evidence_record_for(&entry, None);

        assert_eq!(record.actionability.class, "actionable_focused_test");
        assert!(record.recommendation.nearest_test_to_imitate.is_none());

        let recommended_test = record.recommendation.recommended_test.as_ref();
        let related_test = record.canonical_item.related_test.as_ref();

        assert_eq!(
            related_test.map(|test| test.name.as_str()),
            recommended_test.map(|test| test.name.as_str())
        );
        assert_eq!(
            related_test.map(|test| test.file.as_str()),
            recommended_test.map(|test| test.file.as_str())
        );
        assert_eq!(related_test.map(|test| test.line), Some(1));
        assert_eq!(
            related_test.map(|test| test.reason.as_str()),
            Some("recommended_test_target")
        );
    }

    #[test]
    fn evidence_record_keeps_unresolved_boundary_operands_as_named_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "Boundary activation operand is iterator-derived for seam `idx >= offset`; add analyzer support for iterator boundary operand resolution before emitting an actionable repair packet",
        );
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["observed_values"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            json["missing_discriminators"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            "activation_boundary_input_unresolved"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/iterator-boundary-operand-resolution"
        );
        assert!(
            !json["canonical_item"]["recommended_repair"]
                .as_str()
                .unwrap_or_default()
                .contains("Add or strengthen")
        );
    }

    #[test]
    fn evidence_record_routes_computed_boundary_operands_to_local_computed_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "Boundary activation operands are local or computed for seam `amount >= threshold`; add analyzer support for local/computed boundary operand resolution before emitting an actionable repair packet",
        );
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["observed_values"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            json["missing_discriminators"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            "activation_boundary_input_unresolved"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/local-computed-boundary-operand-resolution"
        );
        assert!(
            !json["canonical_item"]["recommended_repair"]
                .as_str()
                .unwrap_or_default()
                .contains("Add or strengthen")
        );
    }

    #[test]
    fn evidence_record_routes_member_boundary_operands_to_local_member_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.seam = RepoSeam::new(
            "src/activation.rs",
            "activation::owner_call_parameter_values",
            SeamKind::PredicateBoundary,
            58,
            58,
            "call.name != owner_name",
            RequiredDiscriminator::BoundaryValue {
                description: "call.name != owner_name".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        entry.evidence.activate = stage(
            StageState::Unknown,
            "Boundary activation operands are local, member-access, or computed for seam `call.name != owner_name`; add analyzer support for member boundary operand resolution before emitting an actionable repair packet",
        );
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            "activation_boundary_input_unresolved"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/local-member-boundary-operand-resolution"
        );
    }

    #[test]
    fn evidence_record_routes_closure_boundary_operands_to_closure_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "Boundary activation operand is closure-derived for seam `function.id == owner`; add analyzer support for closure boundary operand resolution before emitting an actionable repair packet",
        );
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["observed_values"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            json["missing_discriminators"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["category"],
            "activation_boundary_input_unresolved"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/closure-boundary-operand-resolution"
        );
        assert!(
            !json["canonical_item"]["recommended_repair"]
                .as_str()
                .unwrap_or_default()
                .contains("Add or strengthen")
        );
    }

    #[test]
    fn member_boundary_operand_route_requires_identifier_member_tokens() {
        for token in [
            "call.name",
            "left_value.value",
            "!owner.name",
            "(row.field)",
        ] {
            assert!(
                token_has_member_access(token),
                "member operand token should match: {token}"
            );
        }

        for token in ["1.0", "call.name()", "owner_name", ".field", "owner."] {
            assert!(
                !token_has_member_access(token),
                "non-member operand token should not match: {token}"
            );
        }
    }

    #[test]
    fn evidence_record_splits_same_file_only_owner_call_absence_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return false`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::SameTestFile;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["recommendation"]["verify_command"], Value::Null);
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_same_file_only"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_routes_same_file_receiver_method_owner_call_absence_limitation() {
        let mut entry = sample_call_presence_classified();
        entry.seam = RepoSeam::new(
            "src/canonical_gap.rs",
            "canonical_gap::required_discriminator_text",
            SeamKind::CallPresence,
            101,
            101,
            "description.clone()",
            RequiredDiscriminator::CallSite {
                target: "description.clone()".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `description.clone()`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::SameTestFile;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_same_file_only"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/same-file-receiver-method-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/same-file-receiver-method-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_keeps_same_file_dotted_non_call_owner_absence_on_generic_route() {
        let mut entry = sample_call_presence_classified();
        entry.seam = RepoSeam::new(
            "src/canonical_gap.rs",
            "canonical_gap::required_discriminator_text",
            SeamKind::CallPresence,
            101,
            101,
            "description.value",
            RequiredDiscriminator::CallSite {
                target: "description.value".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `description.value`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::SameTestFile;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_same_file_only"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_routes_same_file_primary_owner_call_absence_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return false`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::SameTestFile;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.related_tests.push(RelatedTestGrip {
            test_name: "pricing_module_smoke".to_string(),
            file: PathBuf::from("tests/pricing/integration.rs"),
            line: 44,
            oracle_kind: OracleKind::BroadError,
            oracle_strength: OracleStrength::Weak,
            evidence_summary: "module proximity".to_string(),
            relation_reason: RelationReason::SameModule,
            relation_confidence: RelationConfidence::Medium,
        });
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_same_file_only"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/same-file-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_routes_assertion_target_return_value_owner_call_absence_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.seam = RepoSeam::new(
            "src/activation.rs",
            "activation::body_contains_wrapped_local_alias",
            SeamKind::ReturnValue,
            431,
            431,
            "return Some(parameter.clone())",
            RequiredDiscriminator::ReturnValue {
                description: "return Some(parameter.clone())".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return Some(parameter.clone())`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::AssertionTargetAffinity;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_assertion_target_affinity"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/assertion-target-return-value-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/assertion-target-return-value-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_splits_assertion_target_owner_call_absence_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return false`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::AssertionTargetAffinity;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Medium;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_assertion_target_affinity"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/assertion-target-affinity-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/assertion-target-affinity-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_splits_call_presence_target_affinity_owner_call_absence_limitation() {
        let record = evidence_record_for(&sample_call_presence_classified(), None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(json["canonical_item"]["canonical_item_kind"], "limitation");
        assert_eq!(json["canonical_item"]["gap_state"], "static_limitation");
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
        assert_eq!(json["canonical_item"]["receipt_command"], Value::Null);
        assert_eq!(json["recommendation"]["verify_command"], Value::Null);
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_call_presence_target_affinity"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/call-presence-target-affinity-owner-call-tracing"
        );
        assert_eq!(
            json["canonical_item"]["static_limitations"][0]["repair_route"],
            "analysis/call-presence-target-affinity-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_keeps_generic_affinity_owner_call_absence_limitation() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return false`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::FixtureOwnerAffinity;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::Low;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent_affinity_only"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/related-test-affinity-owner-call-tracing"
        );
    }

    #[test]
    fn evidence_record_keeps_owner_call_absence_triage_route_when_owner_call_relation_exists() {
        let mut entry = sample_classified(StageState::Unknown, SeamGripClass::ActivationUnknown);
        entry.evidence.activate = stage(
            StageState::Unknown,
            "No direct owner call observed for value-insensitive seam `return false`",
        );
        entry.evidence.related_tests[0].relation_reason = RelationReason::DirectOwnerCall;
        entry.evidence.related_tests[0].relation_confidence = RelationConfidence::High;
        entry.evidence.observed_values.clear();
        entry.evidence.missing_discriminators.clear();

        let record = evidence_record_for(&entry, None);
        let json = evidence_record_json_value(&record);

        assert_eq!(json["actionability"]["class"], "static_limitation");
        assert_eq!(
            json["static_limitations"][0]["category"],
            "activation_owner_call_absent"
        );
        assert_eq!(
            json["static_limitations"][0]["repair_route"],
            "analysis/owner-call-absence-triage"
        );
        assert_eq!(json["canonical_item"]["repair_route"], Value::Null);
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
                "No direct owner call observed for value-insensitive seam `Vec::new()`",
                "activation_owner_call_absent",
            ),
            (
                "activate",
                "unknown",
                "owner call target is unresolved for value-insensitive seam `Vec::new()`",
                "activation_owner_call_unresolved",
            ),
            (
                "activate",
                "unknown",
                "No concrete activation values observed for seam `threshold`",
                "activation_value_unresolved",
            ),
            (
                "activate",
                "unknown",
                "Boundary activation operand is iterator-derived for seam `idx >= offset`; add analyzer support for iterator boundary operand resolution before emitting an actionable repair packet",
                "activation_boundary_input_unresolved",
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
            (
                "unknown",
                "unknown",
                "missing stage",
                "static_limitation_unclassified",
            ),
        ] {
            assert_eq!(
                static_limitation_category(stage, state, reason),
                expected,
                "unexpected category for {stage}/{state}: {reason}"
            );
        }

        for (category, expected) in [
            (
                "activation_owner_call_absent",
                "analysis/owner-call-absence-triage",
            ),
            (
                "activation_owner_call_absent_call_presence_target_affinity",
                "analysis/call-presence-target-affinity-owner-call-tracing",
            ),
            (
                "activation_owner_call_absent_assertion_target_affinity",
                "analysis/assertion-target-affinity-owner-call-tracing",
            ),
            (
                "activation_owner_call_absent_affinity_only",
                "analysis/related-test-affinity-owner-call-tracing",
            ),
            (
                "activation_owner_call_absent_same_file_only",
                "analysis/same-file-owner-call-tracing",
            ),
            (
                "activation_owner_call_unresolved",
                "analysis/related-test-ranking-audit-fixes",
            ),
            (
                "activation_boundary_input_unresolved",
                "analysis/local-computed-boundary-operand-resolution",
            ),
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
            (
                "static_limitation_unclassified",
                "analysis/static-limitation-taxonomy",
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
    fn evidence_record_alignment_helpers_cover_non_actionable_states() {
        let not_policy_relevant =
            sample_actionability("not_policy_relevant", "no user action", false);
        let strong = sample_classified(StageState::Yes, SeamGripClass::StronglyGripped);
        let intentional = sample_classified(StageState::Yes, SeamGripClass::Intentional);
        let unknown = sample_classified(StageState::Yes, SeamGripClass::Suppressed);

        assert_eq!(
            gap_state_for(&strong, &not_policy_relevant),
            "already_observed"
        );
        assert_eq!(
            gap_state_for(&intentional, &not_policy_relevant),
            "internal_only"
        );
        assert_eq!(gap_state_for(&unknown, &not_policy_relevant), "unknown");
        assert_eq!(canonical_item_kind_for("already_observed"), "observed");
        assert_eq!(canonical_item_kind_for("internal_only"), "no_action");
        assert_eq!(canonical_item_kind_for("unknown"), "evidence");
        assert_eq!(
            alignment_actionability_for(&strong, &not_policy_relevant),
            "already_observed"
        );
        assert_eq!(
            alignment_actionability_for(&intentional, &not_policy_relevant),
            "no_action"
        );
        assert_eq!(
            alignment_actionability_for(
                &unknown,
                &sample_actionability("actionable_focused_test", "add focused test", true)
            ),
            "add_focused_test"
        );
        assert_eq!(
            alignment_actionability_for(&unknown, &not_policy_relevant),
            "unknown"
        );

        let recommendation = sample_recommendation("fallback repair");
        assert_eq!(
            recommended_repair_for(
                "already_observed",
                &recommendation,
                &not_policy_relevant,
                &[]
            ),
            "No new RIPR action; current static evidence already observes this seam."
        );
        assert_eq!(
            recommended_repair_for("internal_only", &recommendation, &not_policy_relevant, &[]),
            "No user test action in documented scope."
        );
        assert_eq!(
            recommended_repair_for("unknown", &recommendation, &not_policy_relevant, &[]),
            "fallback repair"
        );
        assert_eq!(
            recommended_repair_for(
                "static_limitation",
                &recommendation,
                &not_policy_relevant,
                &[sample_static_limitation(
                    "opaque_helper_call",
                    "analysis/oracle-semantics-audit-fixes"
                )],
            ),
            "Inspect static limitation `opaque_helper_call` via `analysis/oracle-semantics-audit-fixes`."
        );

        let unknown_confidence = alignment_confidence_for("unknown", &[]);
        assert_eq!(unknown_confidence.basis, "unknown");
        assert_eq!(
            unknown_confidence.notes,
            vec!["no imported runtime calibration data"]
        );
        let limited_confidence = alignment_confidence_for(
            "static_limitation",
            &[sample_static_limitation(
                "opaque_helper_call",
                "analysis/oracle-semantics-audit-fixes",
            )],
        );
        assert_eq!(limited_confidence.basis, "static_only");
        assert_eq!(
            limited_confidence.notes[1],
            "static limitation: opaque_helper_call"
        );
    }

    #[test]
    fn presentation_text_json_serializes_nullable_alignment_fields() {
        let presentation_text = EvidenceRecordPresentationText {
            visibility: "unknown".to_string(),
            observer: "unknown".to_string(),
            actionability: "inspect_visibility".to_string(),
            source_kind: "const_decl".to_string(),
            canonical_group_reason: Some("declaration_and_literal_same_text_constant".to_string()),
            recommended_observer: "cli_help_output".to_string(),
        };

        let json = presentation_text_json(&presentation_text);

        assert_eq!(json["visibility"], "unknown");
        assert_eq!(json["observer"], "unknown");
        assert_eq!(json["actionability"], "inspect_visibility");
        assert_eq!(json["source_kind"], "const_decl");
        assert_eq!(
            json["canonical_group_reason"],
            "declaration_and_literal_same_text_constant"
        );
        assert_eq!(json["recommended_observer"], "cli_help_output");
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
        assert_eq!(json["canonical_item"]["canonical_gap_id"], "gap:abc123");
        assert_eq!(json["canonical_item"]["raw_group_size"], 3);
        assert_eq!(
            json["canonical_item"]["group_reason"],
            crate::analysis::canonical_gap::CANONICAL_GAP_REASON
        );
        assert_eq!(
            json["canonical_item"]["primary_anchor"]["reason"],
            "canonical_group_primary_raw_finding"
        );
        assert_eq!(
            json["canonical_item"]["raw_spans"][0]["file"],
            "src/pricing.rs"
        );
    }
}
