use super::model::*;

pub(super) fn summary_for(
    raw_signals: usize,
    items: &[FindingAlignmentItem],
) -> FindingAlignmentSummary {
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
    let actionable_items_without_repair_route =
        actionable_gaps.saturating_sub(repair_route_coverage);
    let verify_command_coverage = items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| item_has_verify_command(item))
        .count();
    let actionable_items_without_verify_command =
        actionable_gaps.saturating_sub(verify_command_coverage);
    let presentation_items = items
        .iter()
        .filter(|item| item.evidence_class == PRESENTATION_TEXT_CLASS)
        .collect::<Vec<_>>();
    let config_policy_items = items
        .iter()
        .filter(|item| item.evidence_class == CONFIG_POLICY_CLASS)
        .collect::<Vec<_>>();
    let presentation_text_visibility_unknown = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "unknown")
        })
        .count();
    let presentation_text_user_visible = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "user_visible")
        })
        .count();
    let presentation_text_observed = presentation_items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let presentation_text_unobserved = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.visibility == "user_visible" && text.observer == "none")
        })
        .count();
    let presentation_text_internal_only = presentation_items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let presentation_text_observer_unknown = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.observer == "unknown")
        })
        .count();
    let presentation_text_duplicate_groups = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text.as_ref().is_some_and(|text| {
                text.canonical_group_reason == GROUP_REASON_DECL_LITERAL
                    && item.raw_findings.len() > 1
            })
        })
        .count();
    let presentation_text_actionable_output_repairs = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text
                .as_ref()
                .is_some_and(|text| text.actionability == "add_output_observer")
        })
        .count();
    let presentation_text_no_action = presentation_items
        .iter()
        .filter(|item| {
            item.presentation_text.as_ref().is_some_and(|text| {
                text.actionability == "already_observed"
                    || text.actionability == "no_action_internal"
            })
        })
        .count();
    let presentation_text_static_limitations = presentation_items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();
    let config_policy_user_visible = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.visibility == "user_visible")
        })
        .count();
    let config_policy_observed = config_policy_items
        .iter()
        .filter(|item| item.gap_state == "already_observed")
        .count();
    let config_policy_unobserved = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy.as_ref().is_some_and(|config| {
                config.visibility == "user_visible" && config.observer == "none"
            })
        })
        .count();
    let config_policy_internal_only = config_policy_items
        .iter()
        .filter(|item| item.gap_state == "internal_only")
        .count();
    let config_policy_flow_unknown = config_policy_items
        .iter()
        .filter(|item| {
            item.static_limitations
                .iter()
                .any(|limitation| limitation.category == CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY)
        })
        .count();
    let config_policy_observer_unknown = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.observer == "unknown")
        })
        .count();
    let config_policy_duplicate_groups = config_policy_items
        .iter()
        .filter(|item| {
            item.group_reason == GROUP_REASON_CONFIG_POLICY && item.raw_findings.len() > 1
        })
        .count();
    let config_policy_actionable_output_observer = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.actionability == "add_output_observer")
        })
        .count();
    let config_policy_actionable_behavior_discriminator = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy
                .as_ref()
                .is_some_and(|config| config.actionability == "add_behavior_discriminator")
        })
        .count();
    let config_policy_no_action = config_policy_items
        .iter()
        .filter(|item| {
            item.config_policy.as_ref().is_some_and(|config| {
                config.actionability == "already_observed"
                    || config.actionability == "no_action_internal"
            })
        })
        .count();
    let config_policy_static_limitations = config_policy_items
        .iter()
        .filter(|item| item.gap_state == "static_limitation")
        .count();
    let config_policy_repair_route_coverage = config_policy_items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| {
            !item.recommended_repair.is_empty()
                && item
                    .config_policy
                    .as_ref()
                    .is_some_and(|config| config.repair_kind != "unknown")
        })
        .count();
    let config_policy_verify_command_coverage = config_policy_items
        .iter()
        .filter(|item| item.gap_state == "actionable")
        .filter(|item| !item.verify_command.is_empty() && item.verify_command != "unknown")
        .count();

    FindingAlignmentSummary {
        raw_signals,
        canonical_items: items.len(),
        aligned_raw_findings,
        unaligned_raw_findings: raw_signals.saturating_sub(aligned_raw_findings),
        duplicate_groups_total,
        actionable_gaps,
        already_observed,
        internal_no_action,
        static_limitations,
        unknown,
        calibrated_supported,
        uncalibrated,
        repair_route_coverage,
        actionable_items_without_repair_route,
        verify_command_coverage,
        actionable_items_without_verify_command,
        presentation_text_total: presentation_items.len(),
        presentation_text_user_visible,
        presentation_text_observed,
        presentation_text_unobserved,
        presentation_text_internal_only,
        presentation_text_visibility_unknown,
        presentation_text_observer_unknown,
        presentation_text_duplicate_groups,
        presentation_text_actionable_snapshot: presentation_text_actionable_output_repairs,
        presentation_text_actionable_output_repairs,
        presentation_text_no_action,
        presentation_text_static_limitations,
        config_policy_constant_total: config_policy_items.len(),
        config_policy_user_visible,
        config_policy_observed,
        config_policy_unobserved,
        config_policy_internal_only,
        config_policy_flow_unknown,
        config_policy_observer_unknown,
        config_policy_duplicate_groups,
        config_policy_actionable_output_observer,
        config_policy_actionable_behavior_discriminator,
        config_policy_no_action,
        config_policy_static_limitations,
        config_policy_repair_route_coverage,
        config_policy_verify_command_coverage,
    }
}
pub(super) fn item_has_repair_route(item: &FindingAlignmentItem) -> bool {
    item.repair_route.as_ref().is_some_and(|route| {
        !route_field_is_missing(&route.repair_kind)
            && !route_field_is_missing(&route.target_test_type)
            && !route.suggested_assertion.trim().is_empty()
    })
}

pub(super) fn item_has_verify_command(item: &FindingAlignmentItem) -> bool {
    !verify_command_is_missing(&item.verify_command)
}

pub(super) fn verify_command_is_missing(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == "unknown" || value == "verify_command_unknown"
}

pub(super) fn route_field_is_missing(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == "unknown" || value == "none" || value == "no_action"
}
