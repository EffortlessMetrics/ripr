use super::model::*;
use super::summary::route_field_is_missing;

pub(super) fn presentation_text_item(
    constant_name: &str,
    text_literal: Option<String>,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    classification: PresentationTextClassification,
) -> FindingAlignmentItem {
    let group_reason = if raw_findings.len() > 1 {
        GROUP_REASON_DECL_LITERAL
    } else {
        GROUP_REASON_OWNER
    };
    let repair_route = repair_route_for(
        &classification.gap_state,
        &classification.repair_kind,
        &classification.target_test_type,
        &classification.suggested_assertion,
    );
    debug_assert!(
        classification.gap_state != "actionable" || repair_route.is_some(),
        "actionable finding alignment item must carry a concrete repair route"
    );

    FindingAlignmentItem {
        canonical_gap_id: format!("presentation_text::{constant_name}"),
        canonical_item_kind: classification.canonical_item_kind,
        evidence_class: PRESENTATION_TEXT_CLASS.to_string(),
        gap_state: classification.gap_state,
        actionability: classification.actionability,
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        why: classification.why,
        recommended_repair: classification.recommended_repair,
        repair_route,
        related_test: classification.related_test,
        verify_command: "cargo xtask evidence-quality-scorecard".to_string(),
        static_limitations: classification.static_limitations,
        confidence: classification.confidence,
        raw_findings,
        presentation_text: Some(FindingAlignmentPresentationText {
            constant_name: constant_name.to_string(),
            text_literal,
            visibility: classification.visibility,
            observer: classification.observer,
            actionability: classification.presentation_actionability,
            source_kind: "const_decl".to_string(),
            canonical_group_reason: group_reason.to_string(),
            recommended_observer: classification.recommended_observer,
            repair_kind: classification.repair_kind,
            target_test_type: classification.target_test_type,
            suggested_assertion: classification.suggested_assertion,
        }),
        config_policy: None,
    }
}

pub(super) fn config_policy_item(
    constant_name: &str,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    classification: ConfigPolicyClassification,
) -> FindingAlignmentItem {
    let group_reason = if raw_findings.len() > 1 {
        GROUP_REASON_CONFIG_POLICY
    } else {
        GROUP_REASON_OWNER
    };
    let repair_route = repair_route_for(
        &classification.gap_state,
        &classification.repair_kind,
        &classification.target_test_type,
        &classification.suggested_assertion,
    );
    debug_assert!(
        classification.gap_state != "actionable" || repair_route.is_some(),
        "actionable finding alignment item must carry a concrete repair route"
    );

    FindingAlignmentItem {
        canonical_gap_id: format!("config_or_policy_constant::{constant_name}"),
        canonical_item_kind: classification.canonical_item_kind,
        evidence_class: CONFIG_POLICY_CLASS.to_string(),
        gap_state: classification.gap_state,
        actionability: classification.actionability,
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        why: classification.why,
        recommended_repair: classification.recommended_repair,
        repair_route,
        related_test: classification.related_test,
        verify_command: "cargo xtask evidence-quality-scorecard".to_string(),
        static_limitations: classification.static_limitations,
        confidence: classification.confidence,
        raw_findings,
        presentation_text: None,
        config_policy: Some(FindingAlignmentConfigPolicy {
            constant: constant_name.to_string(),
            role: classification.role,
            source_kind: "const_decl".to_string(),
            visibility: classification.visibility,
            observer: classification.observer,
            actionability: classification.config_actionability,
            repair_kind: classification.repair_kind,
            target_test_type: classification.target_test_type,
            suggested_assertion: classification.suggested_assertion,
        }),
    }
}
fn repair_route_for(
    gap_state: &str,
    repair_kind: &str,
    target_test_type: &str,
    suggested_assertion: &str,
) -> Option<FindingAlignmentRepairRoute> {
    if gap_state != "actionable" {
        return None;
    }

    if route_field_is_missing(repair_kind)
        || route_field_is_missing(target_test_type)
        || suggested_assertion.trim().is_empty()
    {
        return None;
    }

    Some(FindingAlignmentRepairRoute {
        repair_kind: repair_kind.to_string(),
        target_test_type: target_test_type.to_string(),
        suggested_assertion: suggested_assertion.to_string(),
    })
}
