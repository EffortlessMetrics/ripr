use crate::domain::{Finding, OracleKind, OracleStrength, RelatedTest};

use super::{array_field, escape, field, number_field};

const PRESENTATION_TEXT_CLASS: &str = "presentation_text";
const CONFIG_POLICY_CLASS: &str = "config_or_policy_constant";
const GROUP_REASON_DECL_LITERAL: &str = "declaration_and_literal_same_text_constant";
const GROUP_REASON_OWNER: &str = "constant_owner_identity";
const GROUP_REASON_CONFIG_POLICY: &str = "same_config_policy_constant";
const VISIBILITY_UNKNOWN_CATEGORY: &str = "presentation_text_visibility_unknown";
const VISIBILITY_UNKNOWN_REPAIR_ROUTE: &str = "trace_string_constant_to_output_or_snapshot_test";
const CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY: &str = "config_policy_flow_unknown";
const CONFIG_POLICY_FLOW_UNKNOWN_REPAIR_ROUTE: &str =
    "trace_constant_to_output_schema_validation_or_behavior_sink";
const OPAQUE_CONFIG_LOOKUP_CATEGORY: &str = "opaque_config_lookup";
const OPAQUE_CONFIG_LOOKUP_REPAIR_ROUTE: &str =
    "add_fixture_backed_support_for_opaque_config_lookup";
const MACRO_GENERATED_CONFIG_OUTPUT_CATEGORY: &str = "macro_generated_config_output";
const MACRO_GENERATED_CONFIG_OUTPUT_REPAIR_ROUTE: &str =
    "add_fixture_backed_support_for_generated_config_schema_output";
const DYNAMIC_CONFIG_DISPATCH_CATEGORY: &str = "dynamic_config_dispatch";
const DYNAMIC_CONFIG_DISPATCH_REPAIR_ROUTE: &str =
    "add_fixture_backed_support_for_dynamic_config_dispatch";

pub(super) struct FindingAlignmentReport {
    summary: FindingAlignmentSummary,
    items: Vec<FindingAlignmentItem>,
}

struct FindingAlignmentSummary {
    raw_signals: usize,
    canonical_items: usize,
    aligned_raw_findings: usize,
    unaligned_raw_findings: usize,
    duplicate_groups_total: usize,
    actionable_gaps: usize,
    already_observed: usize,
    internal_no_action: usize,
    static_limitations: usize,
    unknown: usize,
    calibrated_supported: usize,
    uncalibrated: usize,
    repair_route_coverage: usize,
    actionable_items_without_repair_route: usize,
    verify_command_coverage: usize,
    actionable_items_without_verify_command: usize,
    presentation_text_total: usize,
    presentation_text_user_visible: usize,
    presentation_text_observed: usize,
    presentation_text_unobserved: usize,
    presentation_text_internal_only: usize,
    presentation_text_visibility_unknown: usize,
    presentation_text_observer_unknown: usize,
    presentation_text_duplicate_groups: usize,
    presentation_text_actionable_snapshot: usize,
    presentation_text_actionable_output_repairs: usize,
    presentation_text_no_action: usize,
    presentation_text_static_limitations: usize,
    config_policy_constant_total: usize,
    config_policy_user_visible: usize,
    config_policy_observed: usize,
    config_policy_unobserved: usize,
    config_policy_internal_only: usize,
    config_policy_flow_unknown: usize,
    config_policy_observer_unknown: usize,
    config_policy_duplicate_groups: usize,
    config_policy_actionable_output_observer: usize,
    config_policy_actionable_behavior_discriminator: usize,
    config_policy_no_action: usize,
    config_policy_static_limitations: usize,
    config_policy_repair_route_coverage: usize,
    config_policy_verify_command_coverage: usize,
}

struct FindingAlignmentItem {
    canonical_gap_id: String,
    canonical_item_kind: String,
    evidence_class: String,
    gap_state: String,
    actionability: String,
    raw_group_size: usize,
    group_reason: String,
    primary_anchor: Option<FindingAlignmentPrimaryAnchor>,
    raw_spans: Vec<FindingAlignmentRawSpan>,
    why: String,
    recommended_repair: String,
    repair_route: Option<FindingAlignmentRepairRoute>,
    related_test: Option<FindingAlignmentRelatedTest>,
    verify_command: String,
    static_limitations: Vec<FindingAlignmentStaticLimitation>,
    confidence: FindingAlignmentConfidence,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    presentation_text: Option<FindingAlignmentPresentationText>,
    config_policy: Option<FindingAlignmentConfigPolicy>,
}

struct FindingAlignmentRawFinding {
    file: String,
    line: usize,
    kind: String,
    expression: String,
    probe_kind: String,
    source_id: String,
    evidence_record_ref: String,
}

struct FindingAlignmentPrimaryAnchor {
    file: String,
    line: usize,
    kind: String,
    source_id: String,
    reason: String,
}

struct FindingAlignmentRawSpan {
    file: String,
    start_line: usize,
    end_line: usize,
    kind: String,
    source_id: String,
}

struct FindingAlignmentStaticLimitation {
    category: String,
    repair_route: String,
    user_actionability: String,
}

struct FindingAlignmentConfidence {
    basis: String,
    notes: Vec<String>,
}

struct FindingAlignmentRelatedTest {
    name: String,
    file: String,
    line: usize,
}

struct FindingAlignmentRepairRoute {
    repair_kind: String,
    target_test_type: String,
    suggested_assertion: String,
}

struct FindingAlignmentPresentationText {
    constant_name: String,
    text_literal: Option<String>,
    visibility: String,
    observer: String,
    actionability: String,
    source_kind: String,
    canonical_group_reason: String,
    recommended_observer: String,
    repair_kind: String,
    target_test_type: String,
    suggested_assertion: String,
}

struct FindingAlignmentConfigPolicy {
    constant: String,
    role: String,
    source_kind: String,
    visibility: String,
    observer: String,
    actionability: String,
    repair_kind: String,
    target_test_type: String,
    suggested_assertion: String,
}

struct PresentationTextClassification {
    canonical_item_kind: String,
    gap_state: String,
    actionability: String,
    why: String,
    recommended_repair: String,
    related_test: Option<FindingAlignmentRelatedTest>,
    static_limitations: Vec<FindingAlignmentStaticLimitation>,
    confidence: FindingAlignmentConfidence,
    visibility: String,
    observer: String,
    presentation_actionability: String,
    recommended_observer: String,
    repair_kind: String,
    target_test_type: String,
    suggested_assertion: String,
}

struct ConfigPolicyClassification {
    canonical_item_kind: String,
    gap_state: String,
    actionability: String,
    why: String,
    recommended_repair: String,
    related_test: Option<FindingAlignmentRelatedTest>,
    static_limitations: Vec<FindingAlignmentStaticLimitation>,
    confidence: FindingAlignmentConfidence,
    role: String,
    visibility: String,
    observer: String,
    config_actionability: String,
    repair_kind: String,
    target_test_type: String,
    suggested_assertion: String,
}

struct PresentationTextSink {
    recommended_observer: &'static str,
    repair_target: &'static str,
    description: &'static str,
    target_test_type: &'static str,
    assertion_subject: &'static str,
}

struct ConfigPolicySink {
    role: &'static str,
    repair_target: &'static str,
    description: &'static str,
    actionability: &'static str,
    repair_kind: &'static str,
    target_test_type: &'static str,
    assertion_subject: &'static str,
}

struct ConfigPolicyLimitation {
    category: &'static str,
    repair_route: &'static str,
    why: &'static str,
    recommended_repair: &'static str,
    user_actionability: &'static str,
    role: &'static str,
    suggested_assertion: &'static str,
}

#[derive(Clone)]
struct PresentationTextDeclaration {
    constant_name: String,
    inline_literal: Option<String>,
}

pub(super) fn report_for_findings(findings: &[Finding]) -> Option<FindingAlignmentReport> {
    let mut used = vec![false; findings.len()];
    let mut items = Vec::new();

    for (index, finding) in findings.iter().enumerate() {
        if used[index] {
            continue;
        }

        if let Some(declaration) = parse_config_policy_declaration(&finding.probe.expression) {
            let mut raw_indices = vec![index];
            if declaration.inline_literal.is_none()
                && finding.probe.expression.trim_end().ends_with('=')
                && let Some(literal_index) = adjacent_literal_index(findings, &used, index)
            {
                raw_indices.push(literal_index);
            }
            raw_indices.extend(config_policy_supporting_indices(
                findings,
                &used,
                index,
                &declaration.constant_name,
            ));

            used[index] = true;
            for raw_index in raw_indices.iter().skip(1) {
                used[*raw_index] = true;
            }

            let source_findings = raw_indices
                .iter()
                .map(|raw_index| &findings[*raw_index])
                .collect::<Vec<_>>();
            let classification =
                classify_config_policy_constant(&declaration.constant_name, &source_findings);
            let raw_findings = raw_indices
                .iter()
                .map(|raw_index| raw_finding_for(&findings[*raw_index]))
                .collect::<Vec<_>>();
            items.push(config_policy_item(
                &declaration.constant_name,
                raw_findings,
                classification,
            ));
            continue;
        }

        let Some(declaration) = parse_presentation_text_declaration(&finding.probe.expression)
        else {
            continue;
        };

        let mut raw_indices = vec![index];
        let literal = declaration.inline_literal.clone().or_else(|| {
            adjacent_literal_index(findings, &used, index).map(|literal_index| {
                raw_indices.push(literal_index);
                parse_string_literal(&findings[literal_index].probe.expression).unwrap_or_default()
            })
        });

        used[index] = true;
        for raw_index in raw_indices.iter().skip(1) {
            used[*raw_index] = true;
        }

        let source_findings = raw_indices
            .iter()
            .map(|raw_index| &findings[*raw_index])
            .collect::<Vec<_>>();
        let classification =
            classify_presentation_text(&declaration.constant_name, &source_findings);
        let raw_findings = raw_indices
            .iter()
            .map(|raw_index| raw_finding_for(&findings[*raw_index]))
            .collect::<Vec<_>>();
        items.push(presentation_text_item(
            &declaration.constant_name,
            literal,
            raw_findings,
            classification,
        ));
    }

    if items.is_empty() {
        return None;
    }

    let summary = summary_for(findings.len(), &items);
    Some(FindingAlignmentReport { summary, items })
}

pub(super) fn report_json(out: &mut String, report: &FindingAlignmentReport, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "scope", "supported_classes", true);
    array_field(
        out,
        indent + 1,
        "supported_evidence_classes",
        &[
            PRESENTATION_TEXT_CLASS.to_string(),
            CONFIG_POLICY_CLASS.to_string(),
        ],
        true,
    );
    out.push_str(&format!("{}\"summary\": ", "  ".repeat(indent + 1)));
    summary_json(out, &report.summary);
    out.push_str(",\n");
    out.push_str(&format!("{}\"items\": [\n", "  ".repeat(indent + 1)));
    for (index, item) in report.items.iter().enumerate() {
        item_json(out, item, indent + 2);
        if index + 1 != report.items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]\n", "  ".repeat(indent + 1)));
    out.push_str(&format!("{sp}}}"));
}

fn summary_for(raw_signals: usize, items: &[FindingAlignmentItem]) -> FindingAlignmentSummary {
    let presentation_items = items
        .iter()
        .filter(|item| item.evidence_class == PRESENTATION_TEXT_CLASS)
        .collect::<Vec<_>>();
    let config_policy_items = items
        .iter()
        .filter(|item| item.evidence_class == CONFIG_POLICY_CLASS)
        .collect::<Vec<_>>();

    let overall = overall_alignment_counts(items);
    let presentation = presentation_text_counts(&presentation_items);
    let config_policy = config_policy_counts(&config_policy_items);

    FindingAlignmentSummary {
        raw_signals,
        canonical_items: items.len(),
        aligned_raw_findings: overall.aligned_raw_findings,
        unaligned_raw_findings: raw_signals.saturating_sub(overall.aligned_raw_findings),
        duplicate_groups_total: overall.duplicate_groups_total,
        actionable_gaps: overall.actionable_gaps,
        already_observed: overall.already_observed,
        internal_no_action: overall.internal_no_action,
        static_limitations: overall.static_limitations,
        unknown: overall.unknown,
        calibrated_supported: overall.calibrated_supported,
        uncalibrated: overall.uncalibrated,
        repair_route_coverage: overall.repair_route_coverage,
        actionable_items_without_repair_route: overall.actionable_items_without_repair_route,
        verify_command_coverage: overall.verify_command_coverage,
        actionable_items_without_verify_command: overall.actionable_items_without_verify_command,
        presentation_text_total: presentation.total,
        presentation_text_user_visible: presentation.user_visible,
        presentation_text_observed: presentation.observed,
        presentation_text_unobserved: presentation.unobserved,
        presentation_text_internal_only: presentation.internal_only,
        presentation_text_visibility_unknown: presentation.visibility_unknown,
        presentation_text_observer_unknown: presentation.observer_unknown,
        presentation_text_duplicate_groups: presentation.duplicate_groups,
        presentation_text_actionable_snapshot: presentation.actionable_output_repairs,
        presentation_text_actionable_output_repairs: presentation.actionable_output_repairs,
        presentation_text_no_action: presentation.no_action,
        presentation_text_static_limitations: presentation.static_limitations,
        config_policy_constant_total: config_policy.total,
        config_policy_user_visible: config_policy.user_visible,
        config_policy_observed: config_policy.observed,
        config_policy_unobserved: config_policy.unobserved,
        config_policy_internal_only: config_policy.internal_only,
        config_policy_flow_unknown: config_policy.flow_unknown,
        config_policy_observer_unknown: config_policy.observer_unknown,
        config_policy_duplicate_groups: config_policy.duplicate_groups,
        config_policy_actionable_output_observer: config_policy.actionable_output_observer,
        config_policy_actionable_behavior_discriminator: config_policy
            .actionable_behavior_discriminator,
        config_policy_no_action: config_policy.no_action,
        config_policy_static_limitations: config_policy.static_limitations,
        config_policy_repair_route_coverage: config_policy.repair_route_coverage,
        config_policy_verify_command_coverage: config_policy.verify_command_coverage,
    }
}

struct OverallAlignmentCounts {
    aligned_raw_findings: usize,
    duplicate_groups_total: usize,
    actionable_gaps: usize,
    already_observed: usize,
    internal_no_action: usize,
    static_limitations: usize,
    unknown: usize,
    calibrated_supported: usize,
    uncalibrated: usize,
    repair_route_coverage: usize,
    actionable_items_without_repair_route: usize,
    verify_command_coverage: usize,
    actionable_items_without_verify_command: usize,
}

fn overall_alignment_counts(items: &[FindingAlignmentItem]) -> OverallAlignmentCounts {
    let aligned_raw_findings = items
        .iter()
        .map(|item| item.raw_findings.len())
        .sum::<usize>();
    let duplicate_groups_total = items
        .iter()
        .filter(|item| item.raw_findings.len() > 1)
        .count();
    let actionable_gaps = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .count();
    let already_observed = items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let internal_no_action = items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let static_limitations = items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();
    let unknown = items
        .iter()
        .filter(|item| item.gap_state == "unknown")
        .count();
    let calibrated_supported = items
        .iter()
        .filter(|item| item.confidence.basis == "calibrated")
        .count();
    let uncalibrated = items.len().saturating_sub(calibrated_supported);
    let repair_route_coverage = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| item_has_repair_route(item))
        .count();
    let verify_command_coverage = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| item_has_verify_command(item))
        .count();

    OverallAlignmentCounts {
        aligned_raw_findings,
        duplicate_groups_total,
        actionable_gaps,
        already_observed,
        internal_no_action,
        static_limitations,
        unknown,
        calibrated_supported,
        uncalibrated,
        repair_route_coverage,
        actionable_items_without_repair_route: actionable_gaps
            .saturating_sub(repair_route_coverage),
        verify_command_coverage,
        actionable_items_without_verify_command: actionable_gaps
            .saturating_sub(verify_command_coverage),
    }
}

struct PresentationTextCounts {
    total: usize,
    user_visible: usize,
    observed: usize,
    unobserved: usize,
    internal_only: usize,
    visibility_unknown: usize,
    observer_unknown: usize,
    duplicate_groups: usize,
    actionable_output_repairs: usize,
    no_action: usize,
    static_limitations: usize,
}

fn presentation_text_counts(items: &[&FindingAlignmentItem]) -> PresentationTextCounts {
    let visibility_unknown = items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "unknown")
        })
        .count();
    let user_visible = items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "user_visible")
        })
        .count();
    let observed = items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let unobserved = items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "user_visible" && text.observer == "none")
        })
        .count();
    let internal_only = items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let observer_unknown = items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.observer == "unknown")
        })
        .count();
    let duplicate_groups = items
        .iter()
        .filter(|item| {
            item.presentation_text.as_ref().is_some_and(|text| {
                text.canonical_group_reason == GROUP_REASON_DECL_LITERAL
                    && item.raw_findings.len() > 1
            })
        })
        .count();
    let actionable_output_repairs = items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.actionability == "add_output_observer")
        })
        .count();
    let no_action = items
        .iter()
        .filter(|item| {
            item.presentation_text.as_ref().is_some_and(|text| {
                text.actionability == "already_observed"
                    || text.actionability == "no_action_internal"
            })
        })
        .count();
    let static_limitations = items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();

    PresentationTextCounts {
        total: items.len(),
        user_visible,
        observed,
        unobserved,
        internal_only,
        visibility_unknown,
        observer_unknown,
        duplicate_groups,
        actionable_output_repairs,
        no_action,
        static_limitations,
    }
}

struct ConfigPolicyCounts {
    total: usize,
    user_visible: usize,
    observed: usize,
    unobserved: usize,
    internal_only: usize,
    flow_unknown: usize,
    observer_unknown: usize,
    duplicate_groups: usize,
    actionable_output_observer: usize,
    actionable_behavior_discriminator: usize,
    no_action: usize,
    static_limitations: usize,
    repair_route_coverage: usize,
    verify_command_coverage: usize,
}

fn config_policy_counts(items: &[&FindingAlignmentItem]) -> ConfigPolicyCounts {
    let user_visible = items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.visibility == "user_visible")
        })
        .count();
    let observed = items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let unobserved = items
        .iter()
        .filter(|item| {
            item.config_policy.as_ref().is_some_and(|config| {
                config.visibility == "user_visible" && config.observer == "none"
            })
        })
        .count();
    let internal_only = items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let flow_unknown = items
        .iter()
        .filter(|item| {
            item.static_limitations
                .iter()
                .any(|limitation| limitation.category == CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY)
        })
        .count();
    let observer_unknown = items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.observer == "unknown")
        })
        .count();
    let duplicate_groups = items
        .iter()
        .filter(|item| {
            item.group_reason == GROUP_REASON_CONFIG_POLICY && item.raw_findings.len() > 1
        })
        .count();
    let actionable_output_observer = items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.actionability == "add_output_observer")
        })
        .count();
    let actionable_behavior_discriminator = items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.actionability == "add_behavior_discriminator")
        })
        .count();
    let no_action = items
        .iter()
        .filter(|item| {
            item.config_policy.as_ref().is_some_and(|config| {
                config.actionability == "already_observed"
                    || config.actionability == "no_action_internal"
            })
        })
        .count();
    let static_limitations = items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();
    let repair_route_coverage = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| item_has_repair_route(item))
        .count();
    let verify_command_coverage = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| item_has_verify_command(item))
        .count();

    ConfigPolicyCounts {
        total: items.len(),
        user_visible,
        observed,
        unobserved,
        internal_only,
        flow_unknown,
        observer_unknown,
        duplicate_groups,
        actionable_output_observer,
        actionable_behavior_discriminator,
        no_action,
        static_limitations,
        repair_route_coverage,
        verify_command_coverage,
    }
}

fn presentation_text_item(
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
    let primary_anchor = primary_anchor_for(&raw_findings, group_reason);
    let raw_spans = raw_spans_for(&raw_findings);
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
    debug_assert_static_limitation_metadata(
        &classification.gap_state,
        &classification.static_limitations,
    );

    FindingAlignmentItem {
        canonical_gap_id: format!("presentation_text::{constant_name}"),
        canonical_item_kind: classification.canonical_item_kind,
        evidence_class: PRESENTATION_TEXT_CLASS.to_string(),
        gap_state: classification.gap_state,
        actionability: classification.actionability,
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        primary_anchor,
        raw_spans,
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

fn config_policy_item(
    constant_name: &str,
    raw_findings: Vec<FindingAlignmentRawFinding>,
    classification: ConfigPolicyClassification,
) -> FindingAlignmentItem {
    let group_reason = if raw_findings.len() > 1 {
        GROUP_REASON_DECL_LITERAL
    } else {
        GROUP_REASON_OWNER
    };
    let primary_anchor = primary_anchor_for(&raw_findings, group_reason);
    let raw_spans = raw_spans_for(&raw_findings);
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
    debug_assert_static_limitation_metadata(
        &classification.gap_state,
        &classification.static_limitations,
    );

    FindingAlignmentItem {
        canonical_gap_id: format!("config_or_policy_constant::{constant_name}"),
        canonical_item_kind: classification.canonical_item_kind,
        evidence_class: CONFIG_POLICY_CLASS.to_string(),
        gap_state: classification.gap_state,
        actionability: classification.actionability,
        raw_group_size: raw_findings.len(),
        group_reason: group_reason.to_string(),
        primary_anchor,
        raw_spans,
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

fn debug_assert_static_limitation_metadata(
    gap_state: &str,
    limitations: &[FindingAlignmentStaticLimitation],
) {
    if gap_state != "static_limitation" {
        return;
    }

    debug_assert!(
        !limitations.is_empty(),
        "static-limitation finding alignment item must carry named limitation metadata"
    );
    debug_assert!(
        limitations.iter().all(|limitation| {
            finding_alignment_limitation_category_is_named(&limitation.category)
                && finding_alignment_limitation_repair_route_is_named(&limitation.repair_route)
                && finding_alignment_limitation_user_actionability_is_named(
                    &limitation.user_actionability,
                )
        }),
        "static-limitation finding alignment item must carry named category, repair route, and user actionability"
    );
}

fn finding_alignment_limitation_category_is_named(category: &str) -> bool {
    !matches!(category.trim(), "" | "static_unknown" | "unknown")
}

fn finding_alignment_limitation_repair_route_is_named(repair_route: &str) -> bool {
    !matches!(repair_route.trim(), "" | "unknown")
}

fn finding_alignment_limitation_user_actionability_is_named(user_actionability: &str) -> bool {
    !matches!(user_actionability.trim(), "" | "unknown")
}

fn classify_presentation_text(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> PresentationTextClassification {
    let source_file = raw_findings
        .first()
        .map(|finding| finding.probe.location.file.display().to_string())
        .unwrap_or_default();

    if is_internal_only_text(constant_name, &source_file) {
        return internal_only_classification();
    }

    if let Some(sink) = visible_sink_for(constant_name, &source_file) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_classification(sink, observer, related_test);
        }

        return actionable_output_classification(sink, constant_name);
    }

    visibility_unknown_classification()
}

fn visibility_unknown_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "limitation".to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_visibility".to_string(),
        why: "Changed presentation text could not be traced to or away from a user-visible output sink.".to_string(),
        recommended_repair:
            "Trace the string constant to a rendered output path or confirm it is internal-only."
                .to_string(),
        related_test: None,
        static_limitations: vec![FindingAlignmentStaticLimitation {
            category: VISIBILITY_UNKNOWN_CATEGORY.to_string(),
            repair_route: VISIBILITY_UNKNOWN_REPAIR_ROUTE.to_string(),
            user_actionability: "unknown_until_visibility_known".to_string(),
        }],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visibility-unknown presentation text is benchmark-pinned; no user test debt is claimed without an output sink.".to_string(),
            ],
        },
        visibility: "unknown".to_string(),
        observer: "unknown".to_string(),
        presentation_actionability: "static_limitation_visibility_unknown".to_string(),
        recommended_observer: "unknown".to_string(),
        repair_kind: "inspect_visibility".to_string(),
        target_test_type: "unknown".to_string(),
        suggested_assertion:
            "Trace the constant to a supported output sink before adding or updating tests."
                .to_string(),
    }
}

fn internal_only_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "no_action".to_string(),
        gap_state: "internal_only".to_string(),
        actionability: "no_action".to_string(),
        why: "Changed label is confined to an internal proof, policy, or config-only path in fixture-backed scope.".to_string(),
        recommended_repair: "No user test action.".to_string(),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Internal-only presentation labels are benchmark-pinned as no-action evidence, not user-visible output debt.".to_string(),
            ],
        },
        visibility: "internal_only".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "no_action_internal".to_string(),
        recommended_observer: "none".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: "none".to_string(),
        suggested_assertion: "No user-facing assertion is recommended for this internal label."
            .to_string(),
    }
}

fn actionable_output_classification(
    sink: PresentationTextSink,
    constant_name: &str,
) -> PresentationTextClassification {
    let recommended_repair = format!(
        "Add or update a {} for {constant_name}.",
        sink.repair_target,
    );
    let suggested_assertion = format!(
        "Assert {} includes the {constant_name} text.",
        sink.assertion_subject,
    );

    PresentationTextClassification {
        canonical_item_kind: "gap".to_string(),
        gap_state: "actionable".to_string(),
        actionability: "add_output_observer".to_string(),
        why: format!(
            "Changed text flows to {} and no supported output observer is found.",
            sink.description
        ),
        recommended_repair,
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visible unobserved presentation text is actionable only for supported sink patterns.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "add_output_observer".to_string(),
        recommended_observer: sink.recommended_observer.to_string(),
        repair_kind: "output_observer".to_string(),
        target_test_type: sink.target_test_type.to_string(),
        suggested_assertion,
    }
}

fn observed_classification(
    sink: PresentationTextSink,
    observer: &'static str,
    related_test: FindingAlignmentRelatedTest,
) -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "observed".to_string(),
        gap_state: "already_observed".to_string(),
        actionability: "already_observed".to_string(),
        why: format!(
            "Changed text flows to {} and a supported {observer} observer covers it.",
            sink.description
        ),
        recommended_repair: "No new RIPR action.".to_string(),
        related_test: Some(related_test),
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Observed presentation text stays visible as evidence without becoming a user repair.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: observer.to_string(),
        presentation_actionability: "already_observed".to_string(),
        recommended_observer: observer.to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: observer.to_string(),
        suggested_assertion: format!(
            "Existing {observer} observer already covers the {}.",
            sink.description
        ),
    }
}

fn classify_config_policy_constant(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> ConfigPolicyClassification {
    let source_file = raw_findings
        .first()
        .map(|finding| finding.probe.location.file.display().to_string())
        .unwrap_or_default();

    if is_internal_only_config_policy(constant_name, &source_file) {
        return internal_config_policy_classification();
    }

    if is_macro_generated_config_policy_output(constant_name, &source_file) {
        return config_policy_limitation_classification(ConfigPolicyLimitation {
            category: MACRO_GENERATED_CONFIG_OUTPUT_CATEGORY,
            repair_route: MACRO_GENERATED_CONFIG_OUTPUT_REPAIR_ROUTE,
            why: "Changed schema label may be emitted through macro-generated output, but RIPR cannot trace the generated output path in supported scope.",
            recommended_repair: "Add fixture-backed support for the generated schema output path before claiming visibility or observer debt.",
            user_actionability: "unknown_until_generated_output_supported",
            role: "schema_field_label",
            suggested_assertion: "Add generated-output fixture support before recommending schema or golden observer work.",
        });
    }

    if is_dynamic_config_policy_dispatch(constant_name, &source_file) {
        return config_policy_limitation_classification(ConfigPolicyLimitation {
            category: DYNAMIC_CONFIG_DISPATCH_CATEGORY,
            repair_route: DYNAMIC_CONFIG_DISPATCH_REPAIR_ROUTE,
            why: "Changed config selector is routed through dynamic dispatch, but RIPR cannot resolve the target or output sink in supported scope.",
            recommended_repair: "Add fixture-backed support for the dispatch shape before claiming visibility or behavior-observer debt.",
            user_actionability: "unknown_until_dispatch_supported",
            role: "behavior_selector",
            suggested_assertion: "Resolve the dispatch target and supported sink before recommending behavior-discriminator tests.",
        });
    }

    if let Some(sink) = opaque_config_policy_lookup_sink_for(constant_name, raw_findings) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_config_policy_classification(sink, observer, related_test);
        }

        return actionable_config_policy_classification(sink, constant_name);
    }

    if is_opaque_config_policy_lookup(constant_name, &source_file) {
        return config_policy_limitation_classification(ConfigPolicyLimitation {
            category: OPAQUE_CONFIG_LOOKUP_CATEGORY,
            repair_route: OPAQUE_CONFIG_LOOKUP_REPAIR_ROUTE,
            why: "Changed config or policy constant is routed through an unsupported lookup helper.",
            recommended_repair: "Add fixture-backed support for this lookup shape before claiming visibility or observer debt.",
            user_actionability: "unknown_until_lookup_supported",
            role: "unknown",
            suggested_assertion: "Inspect the lookup path or add analyzer support before adding output-observer tests.",
        });
    }

    if let Some(sink) = config_policy_visible_sink_for(constant_name, &source_file) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_config_policy_classification(sink, observer, related_test);
        }

        return actionable_config_policy_classification(sink, constant_name);
    }

    config_policy_limitation_classification(ConfigPolicyLimitation {
        category: CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY,
        repair_route: CONFIG_POLICY_FLOW_UNKNOWN_REPAIR_ROUTE,
        why: "Changed config or policy constant could not be traced to or away from a supported output, schema, validation, or behavior sink.",
        recommended_repair: "Trace the constant to a supported output, schema, validation, or behavior sink before claiming user test debt.",
        user_actionability: "unknown_until_config_flow_known",
        role: "unknown",
        suggested_assertion: "Trace the constant to a supported output or behavior sink before adding tests.",
    })
}

fn internal_config_policy_classification() -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "no_action".to_string(),
        gap_state: "internal_only".to_string(),
        actionability: "no_action_internal".to_string(),
        why: "Changed constant is confined to internal allowlist, proof, or policy metadata in fixture-backed scope.".to_string(),
        recommended_repair: "No user test action.".to_string(),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Internal config or policy metadata is benchmark-pinned as no-action evidence, not user test debt.".to_string(),
            ],
        },
        role: "internal_policy_metadata".to_string(),
        visibility: "internal_only".to_string(),
        observer: "none".to_string(),
        config_actionability: "no_action_internal".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: "none".to_string(),
        suggested_assertion:
            "No user-facing assertion is recommended for this internal policy constant."
                .to_string(),
    }
}

fn actionable_config_policy_classification(
    sink: ConfigPolicySink,
    constant_name: &str,
) -> ConfigPolicyClassification {
    let recommended_repair = format!(
        "Add or update a {} for {constant_name}.",
        sink.repair_target
    );
    let suggested_assertion = format!(
        "Assert {} includes the {constant_name} value or selected behavior.",
        sink.assertion_subject
    );

    ConfigPolicyClassification {
        canonical_item_kind: "gap".to_string(),
        gap_state: "actionable".to_string(),
        actionability: sink.actionability.to_string(),
        why: format!(
            "Changed config or policy constant flows to {} and no supported observer or discriminator is found.",
            sink.description
        ),
        recommended_repair,
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visible unobserved config or policy constants are actionable only for supported sink patterns.".to_string(),
            ],
        },
        role: sink.role.to_string(),
        visibility: "user_visible".to_string(),
        observer: "none".to_string(),
        config_actionability: sink.actionability.to_string(),
        repair_kind: sink.repair_kind.to_string(),
        target_test_type: sink.target_test_type.to_string(),
        suggested_assertion,
    }
}

fn observed_config_policy_classification(
    sink: ConfigPolicySink,
    observer: &'static str,
    related_test: FindingAlignmentRelatedTest,
) -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "observed".to_string(),
        gap_state: "already_observed".to_string(),
        actionability: "already_observed".to_string(),
        why: format!(
            "Changed config or policy constant flows to {} and a supported {observer} observer covers it.",
            sink.description
        ),
        recommended_repair: "No new RIPR action.".to_string(),
        related_test: Some(related_test),
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Observed config or policy constants stay visible as evidence without becoming user repair work.".to_string(),
            ],
        },
        role: sink.role.to_string(),
        visibility: "user_visible".to_string(),
        observer: observer.to_string(),
        config_actionability: "already_observed".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: observer.to_string(),
        suggested_assertion: format!(
            "Existing {observer} observer already covers the {}.",
            sink.description
        ),
    }
}

fn config_policy_limitation_classification(
    limitation: ConfigPolicyLimitation,
) -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "limitation".to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_config_flow".to_string(),
        why: limitation.why.to_string(),
        recommended_repair: limitation.recommended_repair.to_string(),
        related_test: None,
        static_limitations: vec![FindingAlignmentStaticLimitation {
            category: limitation.category.to_string(),
            repair_route: limitation.repair_route.to_string(),
            user_actionability: limitation.user_actionability.to_string(),
        }],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Config/policy unknowns are benchmark-pinned as named limitations; no user test debt is claimed without supported sink evidence.".to_string(),
            ],
        },
        role: limitation.role.to_string(),
        visibility: "unknown".to_string(),
        observer: "unknown".to_string(),
        config_actionability: "inspect_config_flow".to_string(),
        repair_kind: "inspect_config_flow".to_string(),
        target_test_type: "unknown".to_string(),
        suggested_assertion: limitation.suggested_assertion.to_string(),
    }
}

fn visible_sink_for(constant_name: &str, file: &str) -> Option<PresentationTextSink> {
    let file = normalize_token_text(file);

    if name_has_token(constant_name, "HELP")
        && (file.contains("help") || file.contains("cli") || file.contains("command"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "cli_help_output",
            repair_target: "help-output snapshot assertion",
            description: "CLI help output",
            target_test_type: "help_output_snapshot",
            assertion_subject: "CLI help output",
        });
    }

    if name_has_token(constant_name, "REPORT") && file.contains("report") {
        return Some(PresentationTextSink {
            recommended_observer: "report_render",
            repair_target: "report-render or golden-output test",
            description: "rendered report output",
            target_test_type: "report_render_or_golden",
            assertion_subject: "the rendered report output",
        });
    }

    if (name_has_token(constant_name, "TABLE") || name_has_token(constant_name, "DISPLAY"))
        && (file.contains("table") || file.contains("display") || file.contains("render"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "table_render",
            repair_target: "table-render or golden-output test",
            description: "rendered table output",
            target_test_type: "table_render_or_golden",
            assertion_subject: "the rendered table output",
        });
    }

    None
}

fn config_policy_visible_sink_for(constant_name: &str, file: &str) -> Option<ConfigPolicySink> {
    let file = normalize_token_text(file);

    if (name_has_token(constant_name, "SCHEMA") || name_has_token(constant_name, "FIELD"))
        && file.contains("schema")
    {
        return Some(ConfigPolicySink {
            role: "schema_field_label",
            repair_target: "schema-render or golden-output test",
            description: "rendered schema output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "schema_render_or_golden",
            assertion_subject: "the rendered schema output",
        });
    }

    if (name_has_token(constant_name, "REPORT")
        || (name_has_token(constant_name, "POLICY") && name_has_token(constant_name, "LABEL")))
        && file.contains("report")
    {
        return Some(ConfigPolicySink {
            role: "rendered_policy_label",
            repair_target: "report-render or golden-output test",
            description: "rendered report output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "report_render_or_golden",
            assertion_subject: "the rendered report output",
        });
    }

    if (name_has_token(constant_name, "CONFIG") || name_has_token(constant_name, "SETTING"))
        && (file.contains("settings") || file.contains("output") || file.contains("render"))
    {
        return Some(ConfigPolicySink {
            role: "rendered_config_label",
            repair_target: "config-output, snapshot, or golden-output test",
            description: "rendered config output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "config_output_or_golden",
            assertion_subject: "the rendered config output",
        });
    }

    if (name_has_token(constant_name, "THRESHOLD")
        || name_has_token(constant_name, "SELECTOR")
        || name_has_token(constant_name, "VALIDATION"))
        && (file.contains("validation") || file.contains("routing") || file.contains("selector"))
    {
        return Some(ConfigPolicySink {
            role: "behavior_selector",
            repair_target: "behavior discriminator test",
            description: "observable validation or routing behavior",
            actionability: "add_behavior_discriminator",
            repair_kind: "behavior_discriminator",
            target_test_type: "validation_behavior",
            assertion_subject: "the selected behavior",
        });
    }

    None
}

fn opaque_config_policy_lookup_sink_for(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> Option<ConfigPolicySink> {
    if name_has_token(constant_name, "OPAQUE")
        && name_has_token(constant_name, "REPORT")
        && raw_findings.iter().any(|finding| {
            is_supported_opaque_report_lookup_evidence(constant_name, &finding.probe.expression)
        })
    {
        return Some(ConfigPolicySink {
            role: "rendered_policy_label",
            repair_target: "report-render or golden-output test",
            description: "fixture-backed opaque lookup report output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "report_render_or_golden",
            assertion_subject: "the rendered report output reached through the lookup",
        });
    }

    None
}

fn is_supported_opaque_report_lookup_evidence(constant_name: &str, expression: &str) -> bool {
    let expression = normalize_token_text(expression);
    let constant = normalize_token_text(constant_name);

    expression.contains(&constant)
        && has_supported_lookup_owner(&expression)
        && has_supported_report_output_sink(&expression)
}

fn has_supported_lookup_owner(expression: &str) -> bool {
    expression.contains("lookup_report_label") || expression.contains("lookup_policy_label")
}

fn has_supported_report_output_sink(expression: &str) -> bool {
    expression.contains("render_report")
        || expression.contains("report_render")
        || expression.contains("report_output")
}

fn is_internal_only_text(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "INTERNAL")
        || name_has_token(constant_name, "PROOF")
        || name_has_token(constant_name, "POLICY")
        || name_has_token(constant_name, "CONFIG")
        || file.contains("proof")
        || file.contains("policy")
        || file.contains("config")
        || file.contains("internal")
}

fn is_internal_only_config_policy(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "INTERNAL")
        || name_has_token(constant_name, "ALLOWLIST")
        || name_has_token(constant_name, "DENYLIST")
        || name_has_token(constant_name, "PROOF")
        || file.contains("internal")
        || file.contains("allowlist")
        || file.contains("denylist")
        || file.contains("proof")
}

fn is_macro_generated_config_policy_output(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    (name_has_token(constant_name, "GENERATED") || file.contains("generated"))
        && (name_has_token(constant_name, "SCHEMA")
            || name_has_token(constant_name, "CONFIG")
            || file.contains("schema"))
}

fn is_dynamic_config_policy_dispatch(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "DYNAMIC") || file.contains("dispatch")
}

fn is_opaque_config_policy_lookup(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "OPAQUE") || file.contains("registry") || file.contains("lookup")
}

fn observer_for_findings(
    raw_findings: &[&Finding],
) -> Option<(&'static str, FindingAlignmentRelatedTest)> {
    raw_findings
        .iter()
        .flat_map(|finding| finding.related_tests.iter())
        .filter_map(|test| {
            observer_for_related_test(test).map(|(rank, observer)| (rank, observer, test))
        })
        .min_by_key(|(rank, _, _)| *rank)
        .map(|(_, observer, test)| (observer, related_test_for(test)))
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

fn item_has_repair_route(item: &FindingAlignmentItem) -> bool {
    item.repair_route.as_ref().is_some_and(|route| {
        !route_field_is_missing(&route.repair_kind)
            && !route_field_is_missing(&route.target_test_type)
            && !route.suggested_assertion.trim().is_empty()
    })
}

fn item_has_verify_command(item: &FindingAlignmentItem) -> bool {
    !verify_command_is_missing(&item.verify_command)
}

fn verify_command_is_missing(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == "unknown" || value == "verify_command_unknown"
}

fn route_field_is_missing(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == "unknown" || value == "none" || value == "no_action"
}

fn observer_for_related_test(test: &RelatedTest) -> Option<(u8, &'static str)> {
    let text = normalize_token_text(&format!("{} {}", test.name, test.file.display()));
    let strong_oracle = matches!(
        test.oracle_strength,
        OracleStrength::Strong | OracleStrength::Medium
    );

    if strong_oracle && text.contains("golden") {
        return Some((0, "golden"));
    }

    if test.oracle_kind == OracleKind::Snapshot || (strong_oracle && text.contains("snapshot")) {
        return Some((1, "snapshot"));
    }

    if strong_oracle && (text.contains("help_output") || text.contains("help")) {
        return Some((2, "cli_help_output"));
    }

    if strong_oracle && (text.contains("report") || text.contains("markdown")) {
        return Some((3, "report_render"));
    }

    if strong_oracle && text.contains("schema") {
        return Some((4, "schema_render"));
    }

    if strong_oracle && (text.contains("config") || text.contains("settings")) {
        return Some((5, "config_output"));
    }

    if strong_oracle && (text.contains("validation") || text.contains("routing")) {
        return Some((6, "validation_behavior"));
    }

    if strong_oracle && (text.contains("table") || text.contains("display")) {
        return Some((7, "table_render"));
    }

    None
}

fn related_test_for(test: &RelatedTest) -> FindingAlignmentRelatedTest {
    FindingAlignmentRelatedTest {
        name: test.name.clone(),
        file: test.file.display().to_string(),
        line: test.line,
    }
}

fn name_has_token(name: &str, token: &str) -> bool {
    name.to_ascii_uppercase()
        .split('_')
        .any(|part| part == token)
}

fn normalize_token_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn raw_finding_for(finding: &Finding) -> FindingAlignmentRawFinding {
    FindingAlignmentRawFinding {
        file: finding.probe.location.file.display().to_string(),
        line: finding.probe.location.line,
        kind: finding.class.as_str().to_string(),
        expression: finding.probe.expression.clone(),
        probe_kind: finding.probe.family.as_str().to_string(),
        source_id: finding.probe.id.0.clone(),
        evidence_record_ref: finding.id.clone(),
    }
}

fn primary_anchor_for(
    raw_findings: &[FindingAlignmentRawFinding],
    group_reason: &str,
) -> Option<FindingAlignmentPrimaryAnchor> {
    raw_findings
        .first()
        .map(|finding| FindingAlignmentPrimaryAnchor {
            file: finding.file.clone(),
            line: finding.line,
            kind: finding.kind.clone(),
            source_id: finding.source_id.clone(),
            reason: primary_anchor_reason(group_reason).to_string(),
        })
}

fn primary_anchor_reason(group_reason: &str) -> &'static str {
    match group_reason {
        GROUP_REASON_DECL_LITERAL => "declaration_line_for_grouped_constant",
        GROUP_REASON_OWNER => "constant_owner_line",
        GROUP_REASON_CONFIG_POLICY => "config_policy_owner_line",
        _ => "first_raw_finding_in_canonical_item",
    }
}

fn raw_spans_for(raw_findings: &[FindingAlignmentRawFinding]) -> Vec<FindingAlignmentRawSpan> {
    raw_findings
        .iter()
        .map(|finding| FindingAlignmentRawSpan {
            file: finding.file.clone(),
            start_line: finding.line,
            end_line: finding.line,
            kind: finding.kind.clone(),
            source_id: finding.source_id.clone(),
        })
        .collect()
}

fn adjacent_literal_index(
    findings: &[Finding],
    used: &[bool],
    declaration_index: usize,
) -> Option<usize> {
    let declaration = &findings[declaration_index];
    findings
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != declaration_index && !used[*index])
        .filter(|(_, candidate)| candidate.probe.location.file == declaration.probe.location.file)
        .filter(|(_, candidate)| {
            candidate.probe.location.line == declaration.probe.location.line
                || candidate.probe.location.line == declaration.probe.location.line + 1
        })
        .find_map(|(index, candidate)| {
            parse_string_literal(&candidate.probe.expression).map(|_| index)
        })
}

fn config_policy_supporting_indices(
    findings: &[Finding],
    used: &[bool],
    declaration_index: usize,
    constant_name: &str,
) -> Vec<usize> {
    let declaration = &findings[declaration_index];
    findings
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != declaration_index && !used[*index])
        .filter(|(_, candidate)| candidate.probe.location.file == declaration.probe.location.file)
        .filter(|(_, candidate)| {
            is_supported_opaque_report_lookup_evidence(constant_name, &candidate.probe.expression)
        })
        .map(|(index, _)| index)
        .collect()
}

fn parse_config_policy_declaration(expression: &str) -> Option<PresentationTextDeclaration> {
    let trimmed = expression.trim();
    let const_pos = trimmed.find("const ")?;
    if const_pos > 0
        && trimmed[..const_pos]
            .chars()
            .last()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }

    let after_const = &trimmed[const_pos + "const ".len()..];
    let name_end = after_const
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(after_const.len());
    let constant_name = &after_const[..name_end];
    if constant_name.is_empty() || !is_config_policy_constant_name(constant_name) {
        return None;
    }

    let after_name = after_const[name_end..].trim_start();
    let after_colon = after_name.strip_prefix(':')?.trim_start();
    let equals_pos = after_colon.find('=')?;
    let after_equals = after_colon[equals_pos + 1..].trim_start();
    Some(PresentationTextDeclaration {
        constant_name: constant_name.to_string(),
        inline_literal: parse_string_literal(after_equals),
    })
}

fn parse_presentation_text_declaration(expression: &str) -> Option<PresentationTextDeclaration> {
    let trimmed = expression.trim();
    let const_pos = trimmed.find("const ")?;
    if const_pos > 0
        && trimmed[..const_pos]
            .chars()
            .last()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }

    let after_const = &trimmed[const_pos + "const ".len()..];
    let name_end = after_const
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(after_const.len());
    let constant_name = &after_const[..name_end];
    if constant_name.is_empty() || !is_presentation_text_constant_name(constant_name) {
        return None;
    }

    let after_name = after_const[name_end..].trim_start();
    let after_colon = after_name.strip_prefix(':')?.trim_start();
    let equals_pos = after_colon.find('=')?;
    let ty = after_colon[..equals_pos].trim();
    if !matches!(ty, "&str" | "&'static str") {
        return None;
    }

    let after_equals = after_colon[equals_pos + 1..].trim_start();
    Some(PresentationTextDeclaration {
        constant_name: constant_name.to_string(),
        inline_literal: parse_string_literal(after_equals),
    })
}

fn is_config_policy_constant_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "CONFIG",
        "SETTING",
        "SETTINGS",
        "POLICY",
        "ALLOWLIST",
        "DENYLIST",
        "SCHEMA",
        "FIELD",
        "THRESHOLD",
        "SELECTOR",
        "VALIDATION",
        "ROUTING",
        "ROUTE",
        "OPAQUE",
    ]
    .iter()
    .any(|marker| upper.split('_').any(|part| part == *marker))
}

fn is_presentation_text_constant_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TEXT",
        "LABEL",
        "LABELS",
        "HELP",
        "TITLE",
        "MESSAGE",
        "DESCRIPTION",
        "REPORT",
        "DISPLAY",
        "HEADER",
        "FOOTER",
    ]
    .iter()
    .any(|marker| upper.split('_').any(|part| part == *marker))
}

fn parse_string_literal(expression: &str) -> Option<String> {
    let start = expression.find('"')?;
    let mut value = String::new();
    let mut escaped = false;

    for ch in expression[start + 1..].chars() {
        if escaped {
            match ch {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn summary_json(out: &mut String, summary: &FindingAlignmentSummary) {
    let ratio = if summary.canonical_items == 0 {
        0.0
    } else {
        summary.raw_signals as f64 / summary.canonical_items as f64
    };
    out.push_str(&format!(
        "{{\"raw_signals\":{},\"canonical_items\":{},\"aligned_raw_findings\":{},\"unaligned_raw_findings\":{},\"raw_to_canonical_ratio\":{ratio:.2},\"duplicate_groups_total\":{},\"actionable_gaps\":{},\"already_observed\":{},\"internal_no_action\":{},\"static_limitations\":{},\"unknown\":{},\"calibrated_supported\":{},\"uncalibrated\":{},\"repair_route_coverage\":{},\"actionable_items_without_repair_route\":{},\"verify_command_coverage\":{},\"actionable_items_without_verify_command\":{},\"presentation_text_total\":{},\"presentation_text_user_visible\":{},\"presentation_text_observed\":{},\"presentation_text_unobserved\":{},\"presentation_text_internal_only\":{},\"presentation_text_visibility_unknown\":{},\"presentation_text_observer_unknown\":{},\"presentation_text_duplicate_groups\":{},\"presentation_text_actionable_snapshot\":{},\"presentation_text_actionable_output_repairs\":{},\"presentation_text_no_action\":{},\"presentation_text_static_limitations\":{},\"config_policy_constant_total\":{},\"config_policy_user_visible\":{},\"config_policy_observed\":{},\"config_policy_unobserved\":{},\"config_policy_internal_only\":{},\"config_policy_flow_unknown\":{},\"config_policy_observer_unknown\":{},\"config_policy_duplicate_groups\":{},\"config_policy_actionable_output_observer\":{},\"config_policy_actionable_behavior_discriminator\":{},\"config_policy_no_action\":{},\"config_policy_static_limitations\":{},\"config_policy_repair_route_coverage\":{},\"config_policy_verify_command_coverage\":{}}}",
        summary.raw_signals,
        summary.canonical_items,
        summary.aligned_raw_findings,
        summary.unaligned_raw_findings,
        summary.duplicate_groups_total,
        summary.actionable_gaps,
        summary.already_observed,
        summary.internal_no_action,
        summary.static_limitations,
        summary.unknown,
        summary.calibrated_supported,
        summary.uncalibrated,
        summary.repair_route_coverage,
        summary.actionable_items_without_repair_route,
        summary.verify_command_coverage,
        summary.actionable_items_without_verify_command,
        summary.presentation_text_total,
        summary.presentation_text_user_visible,
        summary.presentation_text_observed,
        summary.presentation_text_unobserved,
        summary.presentation_text_internal_only,
        summary.presentation_text_visibility_unknown,
        summary.presentation_text_observer_unknown,
        summary.presentation_text_duplicate_groups,
        summary.presentation_text_actionable_snapshot,
        summary.presentation_text_actionable_output_repairs,
        summary.presentation_text_no_action,
        summary.presentation_text_static_limitations,
        summary.config_policy_constant_total,
        summary.config_policy_user_visible,
        summary.config_policy_observed,
        summary.config_policy_unobserved,
        summary.config_policy_internal_only,
        summary.config_policy_flow_unknown,
        summary.config_policy_observer_unknown,
        summary.config_policy_duplicate_groups,
        summary.config_policy_actionable_output_observer,
        summary.config_policy_actionable_behavior_discriminator,
        summary.config_policy_no_action,
        summary.config_policy_static_limitations,
        summary.config_policy_repair_route_coverage,
        summary.config_policy_verify_command_coverage
    ));
}

fn item_json(out: &mut String, item: &FindingAlignmentItem, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(
        out,
        indent + 1,
        "canonical_gap_id",
        &item.canonical_gap_id,
        true,
    );
    field(
        out,
        indent + 1,
        "canonical_item_kind",
        &item.canonical_item_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "evidence_class",
        &item.evidence_class,
        true,
    );
    field(out, indent + 1, "gap_state", &item.gap_state, true);
    field(out, indent + 1, "actionability", &item.actionability, true);
    number_field(out, indent + 1, "raw_group_size", item.raw_group_size, true);
    field(out, indent + 1, "group_reason", &item.group_reason, true);
    primary_anchor_json(out, item.primary_anchor.as_ref(), indent + 1);
    out.push_str(",\n");
    raw_spans_json(out, &item.raw_spans, indent + 1);
    out.push_str(",\n");
    field(out, indent + 1, "why", &item.why, true);
    field(
        out,
        indent + 1,
        "recommended_repair",
        &item.recommended_repair,
        true,
    );
    repair_route_json(out, item.repair_route.as_ref(), indent + 1);
    out.push_str(",\n");
    related_test_json(out, item.related_test.as_ref(), indent + 1);
    out.push_str(",\n");
    field(
        out,
        indent + 1,
        "verify_command",
        &item.verify_command,
        true,
    );
    static_limitations_json(out, &item.static_limitations, indent + 1);
    out.push_str(",\n");
    confidence_json(out, &item.confidence, indent + 1);
    out.push_str(",\n");
    raw_findings_json(out, &item.raw_findings, indent + 1);
    out.push_str(",\n");
    presentation_text_json(out, item.presentation_text.as_ref(), indent + 1);
    out.push_str(",\n");
    config_policy_json(out, item.config_policy.as_ref(), indent + 1);
    out.push('\n');
    out.push_str(&format!("{sp}}}"));
}

fn repair_route_json(
    out: &mut String,
    repair_route: Option<&FindingAlignmentRepairRoute>,
    indent: usize,
) {
    out.push_str(&format!("{}\"repair_route\": ", "  ".repeat(indent)));
    let Some(repair_route) = repair_route else {
        out.push_str("null");
        return;
    };
    out.push_str("{\n");
    field(
        out,
        indent + 1,
        "repair_kind",
        &repair_route.repair_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "target_test_type",
        &repair_route.target_test_type,
        true,
    );
    field(
        out,
        indent + 1,
        "suggested_assertion",
        &repair_route.suggested_assertion,
        false,
    );
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

fn related_test_json(
    out: &mut String,
    related_test: Option<&FindingAlignmentRelatedTest>,
    indent: usize,
) {
    out.push_str(&format!("{}\"related_test\": ", "  ".repeat(indent)));
    if let Some(test) = related_test {
        out.push_str("{\n");
        field(out, indent + 1, "name", &test.name, true);
        field(out, indent + 1, "file", &test.file, true);
        number_field(out, indent + 1, "line", test.line, false);
        out.push_str(&format!("{}}}", "  ".repeat(indent)));
    } else {
        out.push_str("null");
    }
}

fn static_limitations_json(
    out: &mut String,
    limitations: &[FindingAlignmentStaticLimitation],
    indent: usize,
) {
    out.push_str(&format!(
        "{}\"static_limitations\": [\n",
        "  ".repeat(indent)
    ));
    for (index, limitation) in limitations.iter().enumerate() {
        let sp = "  ".repeat(indent + 1);
        out.push_str(&format!("{sp}{{\n"));
        field(out, indent + 2, "category", &limitation.category, true);
        field(
            out,
            indent + 2,
            "repair_route",
            &limitation.repair_route,
            true,
        );
        field(
            out,
            indent + 2,
            "user_actionability",
            &limitation.user_actionability,
            false,
        );
        out.push_str(&format!("{sp}}}"));
        if index + 1 != limitations.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn confidence_json(out: &mut String, confidence: &FindingAlignmentConfidence, indent: usize) {
    out.push_str(&format!("{}\"confidence\": {{\n", "  ".repeat(indent)));
    field(out, indent + 1, "basis", &confidence.basis, true);
    array_field(out, indent + 1, "notes", &confidence.notes, false);
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

fn primary_anchor_json(
    out: &mut String,
    primary_anchor: Option<&FindingAlignmentPrimaryAnchor>,
    indent: usize,
) {
    out.push_str(&format!("{}\"primary_anchor\": ", "  ".repeat(indent)));
    let Some(anchor) = primary_anchor else {
        out.push_str("null");
        return;
    };
    out.push_str("{\n");
    field(out, indent + 1, "file", &anchor.file, true);
    number_field(out, indent + 1, "line", anchor.line, true);
    field(out, indent + 1, "kind", &anchor.kind, true);
    field(out, indent + 1, "source_id", &anchor.source_id, true);
    field(out, indent + 1, "reason", &anchor.reason, false);
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

fn raw_spans_json(out: &mut String, raw_spans: &[FindingAlignmentRawSpan], indent: usize) {
    out.push_str(&format!("{}\"raw_spans\": [\n", "  ".repeat(indent)));
    for (index, span) in raw_spans.iter().enumerate() {
        let sp = "  ".repeat(indent + 1);
        out.push_str(&format!("{sp}{{\n"));
        field(out, indent + 2, "file", &span.file, true);
        number_field(out, indent + 2, "start_line", span.start_line, true);
        number_field(out, indent + 2, "end_line", span.end_line, true);
        field(out, indent + 2, "kind", &span.kind, true);
        field(out, indent + 2, "source_id", &span.source_id, false);
        out.push_str(&format!("{sp}}}"));
        if index + 1 != raw_spans.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn raw_findings_json(out: &mut String, raw_findings: &[FindingAlignmentRawFinding], indent: usize) {
    out.push_str(&format!("{}\"raw_findings\": [\n", "  ".repeat(indent)));
    for (index, finding) in raw_findings.iter().enumerate() {
        let sp = "  ".repeat(indent + 1);
        out.push_str(&format!("{sp}{{\n"));
        field(out, indent + 2, "file", &finding.file, true);
        number_field(out, indent + 2, "line", finding.line, true);
        field(out, indent + 2, "kind", &finding.kind, true);
        field(out, indent + 2, "expression", &finding.expression, true);
        field(out, indent + 2, "probe_kind", &finding.probe_kind, true);
        field(out, indent + 2, "source_id", &finding.source_id, true);
        field(
            out,
            indent + 2,
            "evidence_record_ref",
            &finding.evidence_record_ref,
            false,
        );
        out.push_str(&format!("{sp}}}"));
        if index + 1 != raw_findings.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn presentation_text_json(
    out: &mut String,
    presentation_text: Option<&FindingAlignmentPresentationText>,
    indent: usize,
) {
    out.push_str(&format!("{}\"presentation_text\": ", "  ".repeat(indent)));
    let Some(presentation_text) = presentation_text else {
        out.push_str("null");
        return;
    };
    out.push_str("{\n");
    field(
        out,
        indent + 1,
        "constant_name",
        &presentation_text.constant_name,
        true,
    );
    out.push_str(&format!("{}\"text_literal\": ", "  ".repeat(indent + 1)));
    if let Some(text_literal) = &presentation_text.text_literal {
        out.push_str(&format!("\"{}\",\n", escape(text_literal)));
    } else {
        out.push_str("null,\n");
    }
    field(
        out,
        indent + 1,
        "visibility",
        &presentation_text.visibility,
        true,
    );
    field(
        out,
        indent + 1,
        "observer",
        &presentation_text.observer,
        true,
    );
    field(
        out,
        indent + 1,
        "actionability",
        &presentation_text.actionability,
        true,
    );
    field(
        out,
        indent + 1,
        "source_kind",
        &presentation_text.source_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "canonical_group_reason",
        &presentation_text.canonical_group_reason,
        true,
    );
    field(
        out,
        indent + 1,
        "recommended_observer",
        &presentation_text.recommended_observer,
        true,
    );
    field(
        out,
        indent + 1,
        "repair_kind",
        &presentation_text.repair_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "target_test_type",
        &presentation_text.target_test_type,
        true,
    );
    field(
        out,
        indent + 1,
        "suggested_assertion",
        &presentation_text.suggested_assertion,
        false,
    );
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

fn config_policy_json(
    out: &mut String,
    config_policy: Option<&FindingAlignmentConfigPolicy>,
    indent: usize,
) {
    out.push_str(&format!("{}\"config_policy\": ", "  ".repeat(indent)));
    let Some(config_policy) = config_policy else {
        out.push_str("null");
        return;
    };
    out.push_str("{\n");
    field(out, indent + 1, "constant", &config_policy.constant, true);
    field(out, indent + 1, "role", &config_policy.role, true);
    field(
        out,
        indent + 1,
        "source_kind",
        &config_policy.source_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "visibility",
        &config_policy.visibility,
        true,
    );
    field(out, indent + 1, "observer", &config_policy.observer, true);
    field(
        out,
        indent + 1,
        "actionability",
        &config_policy.actionability,
        true,
    );
    field(
        out,
        indent + 1,
        "repair_kind",
        &config_policy.repair_kind,
        true,
    );
    field(
        out,
        indent + 1,
        "target_test_type",
        &config_policy.target_test_type,
        true,
    );
    field(
        out,
        indent + 1,
        "suggested_assertion",
        &config_policy.suggested_assertion,
        false,
    );
    out.push_str(&format!("{}}}", "  ".repeat(indent)));
}

#[cfg(test)]
mod tests {
    use super::{
        GROUP_REASON_DECL_LITERAL, GROUP_REASON_OWNER, parse_presentation_text_declaration,
        parse_string_literal, report_for_findings, verify_command_is_missing,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState,
    };

    #[test]
    fn groups_const_declaration_and_adjacent_literal() -> Result<(), String> {
        let findings = vec![
            finding_at(
                "decl",
                46,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =",
            ),
            finding_at(
                "literal",
                47,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;
        assert_eq!(report.summary.raw_signals, 2);
        assert_eq!(report.summary.canonical_items, 1);
        assert_eq!(report.summary.aligned_raw_findings, 2);
        assert_eq!(report.summary.static_limitations, 1);
        let item = &report.items[0];
        assert_eq!(
            item.canonical_gap_id,
            "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT"
        );
        assert_eq!(item.raw_group_size, 2);
        assert_eq!(item.group_reason, GROUP_REASON_DECL_LITERAL);
        let primary_anchor = item
            .primary_anchor
            .as_ref()
            .ok_or_else(|| "grouped item should expose a primary anchor".to_string())?;
        assert_eq!(primary_anchor.file, "src/device_labels.rs");
        assert_eq!(primary_anchor.line, 46);
        assert_eq!(primary_anchor.kind, "exposed");
        assert_eq!(
            primary_anchor.source_id,
            "probe:src_device_labels_rs:46:decl"
        );
        assert_eq!(
            primary_anchor.reason,
            "declaration_line_for_grouped_constant"
        );
        assert_eq!(item.raw_spans.len(), 2);
        assert_eq!(item.raw_spans[0].start_line, 46);
        assert_eq!(item.raw_spans[0].end_line, 46);
        assert_eq!(item.raw_spans[1].start_line, 47);
        assert_eq!(item.raw_spans[1].end_line, 47);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(item.actionability, "inspect_visibility");
        let presentation_text = presentation_text_for(item)?;
        assert_eq!(
            presentation_text.text_literal.as_deref(),
            Some("apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane")
        );
        Ok(())
    }

    #[test]
    fn canonical_id_is_stable_across_line_movement() -> Result<(), String> {
        let before = vec![
            finding_at(
                "before-decl",
                42,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_MOVED_LABEL: &str =",
            ),
            finding_at(
                "before-literal",
                43,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Help label\";",
            ),
        ];
        let after = vec![
            finding_at(
                "after-decl",
                57,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_MOVED_LABEL: &str =",
            ),
            finding_at(
                "after-literal",
                58,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Help label\";",
            ),
        ];

        let before_report =
            report_for_findings(&before).ok_or_else(|| "before should align".to_string())?;
        let after_report =
            report_for_findings(&after).ok_or_else(|| "after should align".to_string())?;

        assert_eq!(
            before_report.items[0].canonical_gap_id,
            after_report.items[0].canonical_gap_id
        );
        assert_eq!(
            after_report.items[0].canonical_gap_id,
            "presentation_text::HELP_MOVED_LABEL"
        );
        Ok(())
    }

    #[test]
    fn similar_text_in_different_constants_does_not_collide() -> Result<(), String> {
        let findings = vec![
            finding_at(
                "first-decl",
                31,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_DEVICE_LABEL: &str =",
            ),
            finding_at(
                "first-literal",
                32,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Apple CPU/NEON lane\";",
            ),
            finding_at(
                "second-decl",
                35,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const APPLE_REPORT_LABEL: &str =",
            ),
            finding_at(
                "second-literal",
                36,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Apple CPU/NEON lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;

        assert_eq!(report.summary.canonical_items, 2);
        assert_eq!(
            report.items[0].canonical_gap_id,
            "presentation_text::APPLE_DEVICE_LABEL"
        );
        assert_eq!(
            report.items[1].canonical_gap_id,
            "presentation_text::APPLE_REPORT_LABEL"
        );
        Ok(())
    }

    #[test]
    fn string_literal_without_text_constant_does_not_create_item() {
        let findings = vec![finding_at(
            "literal",
            47,
            ExposureClass::StaticUnknown,
            ProbeFamily::StaticUnknown,
            "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
        )];

        assert!(report_for_findings(&findings).is_none());
    }

    #[test]
    fn non_presentation_string_constant_does_not_create_item() {
        let findings = vec![finding_at(
            "decl",
            12,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const CACHE_KEY: &str = \"apple-m3-air-cpu-neon\";",
        )];

        assert!(report_for_findings(&findings).is_none());
    }

    #[test]
    fn declaration_without_literal_uses_owner_identity_group() -> Result<(), String> {
        let findings = vec![finding_at(
            "decl",
            31,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const APPLE_DEVICE_LABEL: &str =",
        )];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "presentation text should align".to_string())?;

        assert_eq!(report.items[0].raw_group_size, 1);
        assert_eq!(report.items[0].group_reason, GROUP_REASON_OWNER);
        assert_eq!(
            report.items[0]
                .primary_anchor
                .as_ref()
                .map(|anchor| anchor.reason.as_str()),
            Some("constant_owner_line")
        );
        assert_eq!(report.items[0].raw_spans.len(), 1);
        assert!(
            presentation_text_for(&report.items[0])?
                .text_literal
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn visible_help_text_without_supported_observer_is_actionable() -> Result<(), String> {
        let lexical_only = related_test(
            "mentions_help_label_without_observer",
            "tests/help_labels.rs",
            17,
            OracleKind::Unknown,
            OracleStrength::None,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/help.rs",
                "decl",
                18,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_DEVICE_LABEL: &str =",
                vec![lexical_only],
            ),
            finding_in_file(
                "src/help.rs",
                "literal",
                19,
                ExposureClass::WeaklyExposed,
                ProbeFamily::StaticUnknown,
                "\"Device label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "visible help text should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.actionable_gaps, 1);
        assert_eq!(report.summary.repair_route_coverage, 1);
        assert_eq!(report.summary.actionable_items_without_repair_route, 0);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(item.canonical_item_kind, "gap");
        assert_eq!(item.gap_state, "actionable");
        assert_eq!(item.actionability, "add_output_observer");
        let repair_route = repair_route_for_item(item)?;
        assert_eq!(repair_route.repair_kind, "output_observer");
        assert_eq!(repair_route.target_test_type, "help_output_snapshot");
        assert_eq!(
            repair_route.suggested_assertion,
            "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        );
        let presentation_text = presentation_text_for(item)?;
        assert_eq!(presentation_text.visibility, "user_visible");
        assert_eq!(presentation_text.observer, "none");
        assert_eq!(presentation_text.recommended_observer, "cli_help_output");
        assert_eq!(
            item.recommended_repair,
            "Add or update a help-output snapshot assertion for HELP_DEVICE_LABEL."
        );
        assert_eq!(presentation_text.repair_kind, "output_observer");
        assert_eq!(presentation_text.target_test_type, "help_output_snapshot");
        assert_eq!(
            presentation_text.suggested_assertion,
            "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        );
        assert!(item.related_test.is_none());
        assert!(!item.recommended_repair.contains("mutation"));
        Ok(())
    }

    #[test]
    fn config_policy_route_coverage_requires_top_level_structured_route() {
        let finding = finding_in_file(
            "src/policy_report.rs",
            "decl",
            22,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const REPORT_POLICY_LABEL: &str =",
        );
        let classification = super::actionable_config_policy_classification(
            super::ConfigPolicySink {
                role: "rendered_policy_label",
                repair_target: "report-render or golden-output test",
                description: "rendered report output",
                actionability: "add_output_observer",
                repair_kind: "output_observer",
                target_test_type: "report_render_or_golden",
                assertion_subject: "the rendered report output",
            },
            "REPORT_POLICY_LABEL",
        );
        let mut item = super::config_policy_item(
            "REPORT_POLICY_LABEL",
            vec![super::raw_finding_for(&finding)],
            classification,
        );
        item.repair_route = None;

        let counts = super::config_policy_counts(&[&item]);

        assert_eq!(counts.actionable_output_observer, 1);
        assert_eq!(counts.repair_route_coverage, 0);
        assert_eq!(counts.verify_command_coverage, 1);
        assert!(!super::item_has_repair_route(&item));
    }

    #[test]
    fn config_policy_verify_coverage_rejects_unknown_sentinel() {
        let finding = finding_in_file(
            "src/policy_report.rs",
            "decl",
            22,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const REPORT_POLICY_LABEL: &str =",
        );
        let classification = super::actionable_config_policy_classification(
            super::ConfigPolicySink {
                role: "rendered_policy_label",
                repair_target: "report-render or golden-output test",
                description: "rendered report output",
                actionability: "add_output_observer",
                repair_kind: "output_observer",
                target_test_type: "report_render_or_golden",
                assertion_subject: "the rendered report output",
            },
            "REPORT_POLICY_LABEL",
        );
        let mut item = super::config_policy_item(
            "REPORT_POLICY_LABEL",
            vec![super::raw_finding_for(&finding)],
            classification,
        );
        item.verify_command = "verify_command_unknown".to_string();

        let counts = super::config_policy_counts(&[&item]);

        assert_eq!(counts.actionable_output_observer, 1);
        assert_eq!(counts.repair_route_coverage, 1);
        assert_eq!(counts.verify_command_coverage, 0);
        assert!(!super::item_has_verify_command(&item));
    }

    #[test]
    fn visible_report_text_with_golden_observer_is_already_observed() -> Result<(), String> {
        let golden = related_test(
            "report_golden_observes_label",
            "tests/golden/report_output.rs",
            22,
            OracleKind::Snapshot,
            OracleStrength::Strong,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/report.rs",
                "decl",
                27,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const REPORT_DEVICE_LABEL: &str =",
                vec![golden],
            ),
            finding_in_file(
                "src/report.rs",
                "literal",
                28,
                ExposureClass::Exposed,
                ProbeFamily::StaticUnknown,
                "\"Report label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "visible report text should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.already_observed, 1);
        assert_eq!(report.summary.actionable_gaps, 0);
        assert_eq!(item.canonical_item_kind, "observed");
        assert_eq!(item.gap_state, "already_observed");
        assert_eq!(item.actionability, "already_observed");
        let presentation_text = presentation_text_for(item)?;
        assert_eq!(presentation_text.visibility, "user_visible");
        assert_eq!(presentation_text.observer, "golden");
        assert_eq!(presentation_text.actionability, "already_observed");
        assert_eq!(presentation_text.repair_kind, "no_action");
        assert_eq!(presentation_text.target_test_type, "golden");
        assert_eq!(
            presentation_text.suggested_assertion,
            "Existing golden observer already covers the rendered report output."
        );
        assert_eq!(
            item.related_test.as_ref().map(|test| test.name.as_str()),
            Some("report_golden_observes_label")
        );
        assert_eq!(item.recommended_repair, "No new RIPR action.");
        Ok(())
    }

    #[test]
    fn internal_only_label_is_no_action() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/proof_lanes.rs",
                "decl",
                12,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const INTERNAL_PROOF_LANE_LABEL: &str =",
            ),
            finding_in_file(
                "src/proof_lanes.rs",
                "literal",
                13,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"internal proof lane\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "internal label should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.internal_no_action, 1);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(item.canonical_item_kind, "no_action");
        assert_eq!(item.gap_state, "internal_only");
        assert_eq!(item.actionability, "no_action");
        let presentation_text = presentation_text_for(item)?;
        assert_eq!(presentation_text.visibility, "internal_only");
        assert_eq!(presentation_text.observer, "none");
        assert_eq!(presentation_text.actionability, "no_action_internal");
        assert_eq!(presentation_text.repair_kind, "no_action");
        assert_eq!(presentation_text.target_test_type, "none");
        assert_eq!(
            presentation_text.suggested_assertion,
            "No user-facing assertion is recommended for this internal label."
        );
        assert!(item.static_limitations.is_empty());
        Ok(())
    }

    #[test]
    fn help_named_text_without_supported_sink_stays_visibility_unknown() -> Result<(), String> {
        let findings = vec![finding_in_file(
            "src/opaque.rs",
            "decl",
            33,
            ExposureClass::Exposed,
            ProbeFamily::FieldConstruction,
            "pub const HELP_DEVICE_LABEL: &str = \"Device label\";",
        )];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "opaque help label should align".to_string())?;
        let item = &report.items[0];

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(item.actionability, "inspect_visibility");
        let presentation_text = presentation_text_for(item)?;
        assert_eq!(presentation_text.visibility, "unknown");
        assert_eq!(presentation_text.observer, "unknown");
        assert_eq!(presentation_text.repair_kind, "inspect_visibility");
        assert_eq!(presentation_text.target_test_type, "unknown");
        assert_eq!(
            presentation_text.suggested_assertion,
            "Trace the constant to a supported output sink before adding or updating tests."
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("presentation_text_visibility_unknown")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.repair_route.as_str()),
            Some("trace_string_constant_to_output_or_snapshot_test")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.user_actionability.as_str()),
            Some("unknown_until_visibility_known")
        );
        Ok(())
    }

    #[test]
    fn internal_policy_metadata_is_no_action() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/policy.rs",
                "decl",
                14,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const INTERNAL_POLICY_LABEL: &str =",
            ),
            finding_in_file(
                "src/policy.rs",
                "literal",
                15,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"internal policy label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "internal policy constant should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.canonical_items, 1);
        assert_eq!(report.summary.internal_no_action, 1);
        assert_eq!(report.summary.config_policy_constant_total, 1);
        assert_eq!(report.summary.config_policy_internal_only, 1);
        assert_eq!(report.summary.config_policy_duplicate_groups, 0);
        assert_eq!(
            item.canonical_gap_id,
            "config_or_policy_constant::INTERNAL_POLICY_LABEL"
        );
        assert_eq!(
            item.primary_anchor
                .as_ref()
                .map(|anchor| (anchor.line, anchor.reason.as_str())),
            Some((14, "declaration_line_for_grouped_constant"))
        );
        assert_eq!(item.raw_spans.len(), 2);
        assert_eq!(item.group_reason, GROUP_REASON_DECL_LITERAL);
        assert_eq!(item.raw_group_size, 2);
        assert_eq!(item.gap_state, "internal_only");
        assert_eq!(item.actionability, "no_action_internal");
        assert_eq!(config_policy.role, "internal_policy_metadata");
        assert_eq!(config_policy.visibility, "internal_only");
        assert_eq!(config_policy.repair_kind, "no_action");
        assert!(item.presentation_text.is_none());
        Ok(())
    }

    #[test]
    fn rendered_policy_label_without_observer_is_actionable() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/report_config.rs",
                "decl",
                22,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const REPORT_POLICY_LABEL: &str =",
            ),
            finding_in_file(
                "src/report_config.rs",
                "literal",
                23,
                ExposureClass::WeaklyExposed,
                ProbeFamily::StaticUnknown,
                "\"Policy label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "rendered policy label should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.actionable_gaps, 1);
        assert_eq!(report.summary.repair_route_coverage, 1);
        assert_eq!(report.summary.actionable_items_without_repair_route, 0);
        assert_eq!(report.summary.verify_command_coverage, 1);
        assert_eq!(report.summary.actionable_items_without_verify_command, 0);
        assert_eq!(report.summary.config_policy_user_visible, 1);
        assert_eq!(report.summary.config_policy_unobserved, 1);
        assert_eq!(report.summary.config_policy_actionable_output_observer, 1);
        assert_eq!(report.summary.config_policy_repair_route_coverage, 1);
        assert_eq!(report.summary.config_policy_verify_command_coverage, 1);
        assert_eq!(item.canonical_item_kind, "gap");
        assert_eq!(item.gap_state, "actionable");
        assert_eq!(item.actionability, "add_output_observer");
        let repair_route = repair_route_for_item(item)?;
        assert_eq!(repair_route.repair_kind, "output_observer");
        assert_eq!(repair_route.target_test_type, "report_render_or_golden");
        assert_eq!(
            repair_route.suggested_assertion,
            "Assert the rendered report output includes the REPORT_POLICY_LABEL value or selected behavior."
        );
        assert_eq!(config_policy.role, "rendered_policy_label");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "none");
        assert_eq!(config_policy.repair_kind, "output_observer");
        assert_eq!(config_policy.target_test_type, "report_render_or_golden");
        assert!(!item.recommended_repair.contains("mutation"));
        Ok(())
    }

    #[test]
    fn behavior_selector_without_discriminator_is_actionable() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/validation_selector.rs",
                "decl",
                40,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const VALIDATION_SELECTOR: &str =",
            ),
            finding_in_file(
                "src/validation_selector.rs",
                "literal",
                41,
                ExposureClass::WeaklyExposed,
                ProbeFamily::StaticUnknown,
                "\"strict\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "behavior selector should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.raw_signals, 2);
        assert_eq!(report.summary.canonical_items, 1);
        assert_eq!(report.summary.actionable_gaps, 1);
        assert_eq!(report.summary.config_policy_user_visible, 1);
        assert_eq!(report.summary.config_policy_unobserved, 1);
        assert_eq!(
            report
                .summary
                .config_policy_actionable_behavior_discriminator,
            1
        );
        assert_eq!(report.summary.config_policy_repair_route_coverage, 1);
        assert_eq!(report.summary.config_policy_verify_command_coverage, 1);
        assert_eq!(
            item.canonical_gap_id,
            "config_or_policy_constant::VALIDATION_SELECTOR"
        );
        assert_eq!(item.group_reason, GROUP_REASON_DECL_LITERAL);
        assert_eq!(item.raw_group_size, 2);
        assert_eq!(item.canonical_item_kind, "gap");
        assert_eq!(item.gap_state, "actionable");
        assert_eq!(item.actionability, "add_behavior_discriminator");
        let repair_route = repair_route_for_item(item)?;
        assert_eq!(repair_route.repair_kind, "behavior_discriminator");
        assert_eq!(repair_route.target_test_type, "validation_behavior");
        assert_eq!(
            repair_route.suggested_assertion,
            "Assert the selected behavior includes the VALIDATION_SELECTOR value or selected behavior."
        );
        assert_eq!(config_policy.role, "behavior_selector");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "none");
        assert_eq!(config_policy.actionability, "add_behavior_discriminator");
        assert_eq!(config_policy.repair_kind, "behavior_discriminator");
        assert_eq!(config_policy.target_test_type, "validation_behavior");
        assert!(!item.recommended_repair.contains("mutation"));
        Ok(())
    }

    #[test]
    fn behavior_selector_with_discriminator_is_already_observed() -> Result<(), String> {
        let validation_observer = related_test(
            "validation_behavior_selects_strict_mode",
            "tests/validation_behavior.rs",
            28,
            OracleKind::RelationalCheck,
            OracleStrength::Strong,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/validation_selector.rs",
                "decl",
                52,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const ROUTING_SELECTOR: &str =",
                vec![validation_observer],
            ),
            finding_in_file(
                "src/validation_selector.rs",
                "literal",
                53,
                ExposureClass::Exposed,
                ProbeFamily::StaticUnknown,
                "\"strict\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "observed behavior selector should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.raw_signals, 2);
        assert_eq!(report.summary.canonical_items, 1);
        assert_eq!(report.summary.actionable_gaps, 0);
        assert_eq!(report.summary.already_observed, 1);
        assert_eq!(report.summary.config_policy_user_visible, 1);
        assert_eq!(report.summary.config_policy_observed, 1);
        assert_eq!(report.summary.config_policy_no_action, 1);
        assert_eq!(item.canonical_item_kind, "observed");
        assert_eq!(item.gap_state, "already_observed");
        assert_eq!(item.actionability, "already_observed");
        assert!(item.repair_route.is_none());
        assert_eq!(config_policy.role, "behavior_selector");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "validation_behavior");
        assert_eq!(config_policy.actionability, "already_observed");
        assert_eq!(config_policy.repair_kind, "no_action");
        assert_eq!(config_policy.target_test_type, "validation_behavior");
        assert_eq!(
            item.related_test.as_ref().map(|test| test.name.as_str()),
            Some("validation_behavior_selects_strict_mode")
        );
        Ok(())
    }

    #[test]
    fn actionable_canonical_items_require_repair_routes() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/help.rs",
                "help-decl",
                18,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const HELP_DEVICE_LABEL: &str =",
            ),
            finding_in_file(
                "src/help.rs",
                "help-literal",
                19,
                ExposureClass::WeaklyExposed,
                ProbeFamily::StaticUnknown,
                "\"Device label\";",
            ),
            finding_in_file(
                "src/validation.rs",
                "validation-decl",
                40,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const VALIDATION_THRESHOLD: i32 = 7;",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "actionable items should align".to_string())?;

        assert_eq!(report.summary.actionable_gaps, 2);
        assert_eq!(report.summary.repair_route_coverage, 2);
        assert_eq!(report.summary.actionable_items_without_repair_route, 0);
        assert_eq!(report.summary.verify_command_coverage, 2);
        assert_eq!(report.summary.actionable_items_without_verify_command, 0);
        for item in report
            .items
            .iter()
            .filter(|item| item.gap_state == "actionable")
        {
            let repair_route = repair_route_for_item(item)?;
            assert_ne!(repair_route.repair_kind, "unknown");
            assert_ne!(repair_route.target_test_type, "unknown");
            assert!(!repair_route.suggested_assertion.trim().is_empty());
            assert!(!verify_command_is_missing(&item.verify_command));
            assert!(!item.recommended_repair.contains("mutation"));
        }

        Ok(())
    }

    #[test]
    fn schema_label_with_golden_observer_is_already_observed() -> Result<(), String> {
        let golden = related_test(
            "schema_render_golden_observes_field",
            "tests/golden/schema_output.rs",
            31,
            OracleKind::Snapshot,
            OracleStrength::Strong,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/schema.rs",
                "decl",
                31,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const SCHEMA_POLICY_FIELD: &str =",
                vec![golden],
            ),
            finding_in_file(
                "src/schema.rs",
                "literal",
                32,
                ExposureClass::Exposed,
                ProbeFamily::StaticUnknown,
                "\"policy\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "schema policy field should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.already_observed, 1);
        assert_eq!(report.summary.config_policy_observed, 1);
        assert_eq!(report.summary.config_policy_no_action, 1);
        assert_eq!(item.canonical_item_kind, "observed");
        assert_eq!(item.gap_state, "already_observed");
        assert_eq!(config_policy.role, "schema_field_label");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "golden");
        assert_eq!(config_policy.repair_kind, "no_action");
        assert_eq!(
            item.related_test.as_ref().map(|test| test.name.as_str()),
            Some("schema_render_golden_observes_field")
        );
        Ok(())
    }

    #[test]
    fn cross_file_config_flow_stays_named_limitation() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/config_labels.rs",
                "decl",
                44,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const CONFIG_TABLE_LABEL: &str =",
            ),
            finding_in_file(
                "src/config_labels.rs",
                "literal",
                45,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Config table label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "config flow unknown should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(report.summary.config_policy_flow_unknown, 1);
        assert_eq!(report.summary.config_policy_observer_unknown, 1);
        assert_eq!(report.summary.config_policy_static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(item.actionability, "inspect_config_flow");
        assert_eq!(config_policy.visibility, "unknown");
        assert_eq!(config_policy.observer, "unknown");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("config_policy_flow_unknown")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.repair_route.as_str()),
            Some("trace_constant_to_output_schema_validation_or_behavior_sink")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.user_actionability.as_str()),
            Some("unknown_until_config_flow_known")
        );
        Ok(())
    }

    #[test]
    fn opaque_config_lookup_stays_named_limitation() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/config_registry.rs",
                "decl",
                58,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const OPAQUE_CONFIG_LABEL: &str =",
            ),
            finding_in_file(
                "src/config_registry.rs",
                "literal",
                59,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Opaque label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "opaque config lookup should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(config_policy.visibility, "unknown");
        assert_eq!(config_policy.repair_kind, "inspect_config_flow");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("opaque_config_lookup")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.repair_route.as_str()),
            Some("add_fixture_backed_support_for_opaque_config_lookup")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.user_actionability.as_str()),
            Some("unknown_until_lookup_supported")
        );
        assert!(!item.recommended_repair.contains("mutation"));
        Ok(())
    }

    #[test]
    fn opaque_report_helper_names_without_supported_lookup_signal_stay_limited()
    -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/report_lookup.rs",
                "decl",
                62,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const OPAQUE_REPORT_LABEL: &str =",
            ),
            finding_in_file(
                "src/report_lookup.rs",
                "literal",
                63,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Opaque report label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "opaque helper name guard should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.actionable_gaps, 0);
        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(report.summary.config_policy_actionable_output_observer, 0);
        assert_eq!(report.summary.config_policy_static_limitations, 1);
        assert_eq!(item.canonical_item_kind, "limitation");
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(config_policy.visibility, "unknown");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("opaque_config_lookup")
        );
        Ok(())
    }

    #[test]
    fn fixture_backed_opaque_report_lookup_with_supported_signal_becomes_actionable()
    -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/report_lookup.rs",
                "decl",
                62,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const OPAQUE_REPORT_LABEL: &str =",
            ),
            finding_in_file(
                "src/report_lookup.rs",
                "literal",
                63,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Opaque report label\";",
            ),
            finding_in_file(
                "src/report_lookup.rs",
                "sink",
                64,
                ExposureClass::Exposed,
                ProbeFamily::SideEffect,
                "render_report(lookup_report_label(OPAQUE_REPORT_LABEL));",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "fixture-backed opaque lookup should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;
        let repair_route = repair_route_for_item(item)?;

        assert_eq!(report.summary.actionable_gaps, 1);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(report.summary.config_policy_actionable_output_observer, 1);
        assert_eq!(report.summary.config_policy_static_limitations, 0);
        assert_eq!(item.canonical_item_kind, "gap");
        assert_eq!(item.gap_state, "actionable");
        assert_eq!(item.actionability, "add_output_observer");
        assert_eq!(item.raw_group_size, 3);
        assert!(item.static_limitations.is_empty());
        assert_eq!(config_policy.role, "rendered_policy_label");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "none");
        assert_eq!(config_policy.repair_kind, "output_observer");
        assert_eq!(repair_route.repair_kind, "output_observer");
        assert_eq!(repair_route.target_test_type, "report_render_or_golden");
        Ok(())
    }

    #[test]
    fn fixture_backed_opaque_report_lookup_can_be_observed() -> Result<(), String> {
        let golden = related_test(
            "opaque_lookup_report_golden_observes_label",
            "tests/report_lookup_golden.rs",
            21,
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let findings = vec![
            finding_in_file_with_related(
                "src/report_lookup.rs",
                "decl",
                62,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const OPAQUE_REPORT_LABEL: &str =",
                vec![golden],
            ),
            finding_in_file(
                "src/report_lookup.rs",
                "literal",
                63,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Opaque report label\";",
            ),
            finding_in_file(
                "src/report_lookup.rs",
                "sink",
                64,
                ExposureClass::Exposed,
                ProbeFamily::SideEffect,
                "render_report(lookup_report_label(OPAQUE_REPORT_LABEL));",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "observed opaque lookup should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.already_observed, 1);
        assert_eq!(report.summary.static_limitations, 0);
        assert_eq!(report.summary.config_policy_observed, 1);
        assert_eq!(item.canonical_item_kind, "observed");
        assert_eq!(item.gap_state, "already_observed");
        assert_eq!(config_policy.visibility, "user_visible");
        assert_eq!(config_policy.observer, "golden");
        assert_eq!(
            item.related_test.as_ref().map(|test| test.name.as_str()),
            Some("opaque_lookup_report_golden_observes_label")
        );
        Ok(())
    }

    #[test]
    fn macro_generated_config_output_stays_named_limitation() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/generated_schema.rs",
                "decl",
                72,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const GENERATED_SCHEMA_LABEL: &str =",
            ),
            finding_in_file(
                "src/generated_schema.rs",
                "literal",
                73,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"Generated schema label\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "macro-generated config output should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(config_policy.role, "schema_field_label");
        assert_eq!(config_policy.visibility, "unknown");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("macro_generated_config_output")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.repair_route.as_str()),
            Some("add_fixture_backed_support_for_generated_config_schema_output")
        );
        Ok(())
    }

    #[test]
    fn dynamic_config_dispatch_stays_named_limitation() -> Result<(), String> {
        let findings = vec![
            finding_in_file(
                "src/config_dispatch.rs",
                "decl",
                86,
                ExposureClass::Exposed,
                ProbeFamily::FieldConstruction,
                "pub const DYNAMIC_CONFIG_SELECTOR: &str =",
            ),
            finding_in_file(
                "src/config_dispatch.rs",
                "literal",
                87,
                ExposureClass::StaticUnknown,
                ProbeFamily::StaticUnknown,
                "\"dynamic\";",
            ),
        ];

        let report = report_for_findings(&findings)
            .ok_or_else(|| "dynamic config dispatch should align".to_string())?;
        let item = &report.items[0];
        let config_policy = config_policy_for(item)?;

        assert_eq!(report.summary.static_limitations, 1);
        assert_eq!(item.gap_state, "static_limitation");
        assert_eq!(config_policy.role, "behavior_selector");
        assert_eq!(config_policy.visibility, "unknown");
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.category.as_str()),
            Some("dynamic_config_dispatch")
        );
        assert_eq!(
            item.static_limitations
                .first()
                .map(|limitation| limitation.repair_route.as_str()),
            Some("add_fixture_backed_support_for_dynamic_config_dispatch")
        );
        Ok(())
    }

    #[test]
    fn parses_presentation_text_const_declaration() -> Result<(), String> {
        let declaration = parse_presentation_text_declaration(
            "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str = \"value\";",
        )
        .ok_or_else(|| "declaration should parse".to_string())?;

        assert_eq!(declaration.constant_name, "APPLE_M3_AIR_DEVICE_LABELS_TEXT");
        assert_eq!(declaration.inline_literal.as_deref(), Some("value"));
        Ok(())
    }

    #[test]
    fn parses_escaped_string_literal() {
        assert_eq!(
            parse_string_literal("\"Help \\\"quoted\\\" label\";").as_deref(),
            Some("Help \"quoted\" label")
        );
    }

    fn finding_at(
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
    ) -> Finding {
        finding_in_file(
            "src/device_labels.rs",
            id_suffix,
            line,
            class,
            family,
            expression,
        )
    }

    fn finding_in_file(
        file: &str,
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
    ) -> Finding {
        finding_in_file_with_related(file, id_suffix, line, class, family, expression, vec![])
    }

    fn finding_in_file_with_related(
        file: &str,
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
        related_tests: Vec<RelatedTest>,
    ) -> Finding {
        let probe_id = format!("probe:src_device_labels_rs:{line}:{id_suffix}");
        Finding {
            id: probe_id.clone(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId(probe_id),
                location: SourceLocation::new(file, line, 1),
                owner: None,
                family,
                delta: DeltaKind::Value,
                before: None,
                after: None,
                expression: expression.to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class,
            ripr: RiprEvidence {
                reach: stage("presentation text raw signal"),
                infect: stage("presentation text raw signal"),
                propagate: stage("presentation text raw signal"),
                reveal: RevealEvidence {
                    observe: stage("presentation text raw signal"),
                    discriminate: stage("presentation text raw signal"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests,
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn related_test(
        name: &str,
        file: &str,
        line: usize,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
    ) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: file.into(),
            line,
            oracle: None,
            oracle_kind,
            oracle_strength,
        }
    }

    fn presentation_text_for(
        item: &super::FindingAlignmentItem,
    ) -> Result<&super::FindingAlignmentPresentationText, String> {
        item.presentation_text
            .as_ref()
            .ok_or_else(|| "item should include presentation_text".to_string())
    }

    fn config_policy_for(
        item: &super::FindingAlignmentItem,
    ) -> Result<&super::FindingAlignmentConfigPolicy, String> {
        item.config_policy
            .as_ref()
            .ok_or_else(|| "item should include config_policy".to_string())
    }

    fn repair_route_for_item(
        item: &super::FindingAlignmentItem,
    ) -> Result<&super::FindingAlignmentRepairRoute, String> {
        item.repair_route
            .as_ref()
            .ok_or_else(|| "actionable item should include repair_route".to_string())
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }
}
