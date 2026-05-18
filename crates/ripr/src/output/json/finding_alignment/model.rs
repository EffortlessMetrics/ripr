pub(super) const PRESENTATION_TEXT_CLASS: &str = "presentation_text";
pub(super) const CONFIG_POLICY_CLASS: &str = "config_or_policy_constant";
pub(super) const GROUP_REASON_DECL_LITERAL: &str = "declaration_and_literal_same_text_constant";
pub(super) const GROUP_REASON_OWNER: &str = "constant_owner_identity";
pub(super) const GROUP_REASON_CONFIG_POLICY: &str = "same_config_policy_constant";
pub(super) const VISIBILITY_UNKNOWN_CATEGORY: &str = "presentation_text_visibility_unknown";
pub(super) const VISIBILITY_UNKNOWN_REPAIR_ROUTE: &str =
    "trace_string_constant_to_output_or_snapshot_test";
pub(super) const CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY: &str = "config_policy_flow_unknown";
pub(super) const CONFIG_POLICY_FLOW_UNKNOWN_REPAIR_ROUTE: &str =
    "trace_constant_to_output_schema_validation_or_behavior_sink";
pub(super) const OPAQUE_CONFIG_LOOKUP_CATEGORY: &str = "opaque_config_lookup";
pub(super) const OPAQUE_CONFIG_LOOKUP_REPAIR_ROUTE: &str =
    "add_fixture_backed_support_for_opaque_config_lookup";

pub(in crate::output::json) struct FindingAlignmentReport {
    pub(super) summary: FindingAlignmentSummary,
    pub(super) items: Vec<FindingAlignmentItem>,
}

pub(super) struct FindingAlignmentSummary {
    pub(super) raw_signals: usize,
    pub(super) canonical_items: usize,
    pub(super) aligned_raw_findings: usize,
    pub(super) unaligned_raw_findings: usize,
    pub(super) duplicate_groups_total: usize,
    pub(super) actionable_gaps: usize,
    pub(super) already_observed: usize,
    pub(super) internal_no_action: usize,
    pub(super) static_limitations: usize,
    pub(super) unknown: usize,
    pub(super) calibrated_supported: usize,
    pub(super) uncalibrated: usize,
    pub(super) repair_route_coverage: usize,
    pub(super) actionable_items_without_repair_route: usize,
    pub(super) verify_command_coverage: usize,
    pub(super) actionable_items_without_verify_command: usize,
    pub(super) presentation_text_total: usize,
    pub(super) presentation_text_user_visible: usize,
    pub(super) presentation_text_observed: usize,
    pub(super) presentation_text_unobserved: usize,
    pub(super) presentation_text_internal_only: usize,
    pub(super) presentation_text_visibility_unknown: usize,
    pub(super) presentation_text_observer_unknown: usize,
    pub(super) presentation_text_duplicate_groups: usize,
    pub(super) presentation_text_actionable_snapshot: usize,
    pub(super) presentation_text_actionable_output_repairs: usize,
    pub(super) presentation_text_no_action: usize,
    pub(super) presentation_text_static_limitations: usize,
    pub(super) config_policy_constant_total: usize,
    pub(super) config_policy_user_visible: usize,
    pub(super) config_policy_observed: usize,
    pub(super) config_policy_unobserved: usize,
    pub(super) config_policy_internal_only: usize,
    pub(super) config_policy_flow_unknown: usize,
    pub(super) config_policy_observer_unknown: usize,
    pub(super) config_policy_duplicate_groups: usize,
    pub(super) config_policy_actionable_output_observer: usize,
    pub(super) config_policy_actionable_behavior_discriminator: usize,
    pub(super) config_policy_no_action: usize,
    pub(super) config_policy_static_limitations: usize,
    pub(super) config_policy_repair_route_coverage: usize,
    pub(super) config_policy_verify_command_coverage: usize,
}

pub(super) struct FindingAlignmentItem {
    pub(super) canonical_gap_id: String,
    pub(super) canonical_item_kind: String,
    pub(super) evidence_class: String,
    pub(super) gap_state: String,
    pub(super) actionability: String,
    pub(super) raw_group_size: usize,
    pub(super) group_reason: String,
    pub(super) why: String,
    pub(super) recommended_repair: String,
    pub(super) repair_route: Option<FindingAlignmentRepairRoute>,
    pub(super) related_test: Option<FindingAlignmentRelatedTest>,
    pub(super) verify_command: String,
    pub(super) static_limitations: Vec<FindingAlignmentStaticLimitation>,
    pub(super) confidence: FindingAlignmentConfidence,
    pub(super) raw_findings: Vec<FindingAlignmentRawFinding>,
    pub(super) presentation_text: Option<FindingAlignmentPresentationText>,
    pub(super) config_policy: Option<FindingAlignmentConfigPolicy>,
}

pub(super) struct FindingAlignmentRawFinding {
    pub(super) file: String,
    pub(super) line: usize,
    pub(super) kind: String,
    pub(super) expression: String,
    pub(super) probe_kind: String,
    pub(super) source_id: String,
    pub(super) evidence_record_ref: String,
}

pub(super) struct FindingAlignmentStaticLimitation {
    pub(super) category: String,
    pub(super) repair_route: String,
    pub(super) user_actionability: String,
}

pub(super) struct FindingAlignmentConfidence {
    pub(super) basis: String,
    pub(super) notes: Vec<String>,
}

pub(super) struct FindingAlignmentRelatedTest {
    pub(super) name: String,
    pub(super) file: String,
    pub(super) line: usize,
}

pub(super) struct FindingAlignmentRepairRoute {
    pub(super) repair_kind: String,
    pub(super) target_test_type: String,
    pub(super) suggested_assertion: String,
}

pub(super) struct FindingAlignmentPresentationText {
    pub(super) constant_name: String,
    pub(super) text_literal: Option<String>,
    pub(super) visibility: String,
    pub(super) observer: String,
    pub(super) actionability: String,
    pub(super) source_kind: String,
    pub(super) canonical_group_reason: String,
    pub(super) recommended_observer: String,
    pub(super) repair_kind: String,
    pub(super) target_test_type: String,
    pub(super) suggested_assertion: String,
}

pub(super) struct FindingAlignmentConfigPolicy {
    pub(super) constant: String,
    pub(super) role: String,
    pub(super) source_kind: String,
    pub(super) visibility: String,
    pub(super) observer: String,
    pub(super) actionability: String,
    pub(super) repair_kind: String,
    pub(super) target_test_type: String,
    pub(super) suggested_assertion: String,
}

pub(super) struct PresentationTextClassification {
    pub(super) canonical_item_kind: String,
    pub(super) gap_state: String,
    pub(super) actionability: String,
    pub(super) why: String,
    pub(super) recommended_repair: String,
    pub(super) related_test: Option<FindingAlignmentRelatedTest>,
    pub(super) static_limitations: Vec<FindingAlignmentStaticLimitation>,
    pub(super) confidence: FindingAlignmentConfidence,
    pub(super) visibility: String,
    pub(super) observer: String,
    pub(super) presentation_actionability: String,
    pub(super) recommended_observer: String,
    pub(super) repair_kind: String,
    pub(super) target_test_type: String,
    pub(super) suggested_assertion: String,
}

pub(super) struct ConfigPolicyClassification {
    pub(super) canonical_item_kind: String,
    pub(super) gap_state: String,
    pub(super) actionability: String,
    pub(super) why: String,
    pub(super) recommended_repair: String,
    pub(super) related_test: Option<FindingAlignmentRelatedTest>,
    pub(super) static_limitations: Vec<FindingAlignmentStaticLimitation>,
    pub(super) confidence: FindingAlignmentConfidence,
    pub(super) role: String,
    pub(super) visibility: String,
    pub(super) observer: String,
    pub(super) config_actionability: String,
    pub(super) repair_kind: String,
    pub(super) target_test_type: String,
    pub(super) suggested_assertion: String,
}

pub(super) struct PresentationTextSink {
    pub(super) recommended_observer: &'static str,
    pub(super) repair_target: &'static str,
    pub(super) description: &'static str,
    pub(super) target_test_type: &'static str,
    pub(super) assertion_subject: &'static str,
}

pub(super) struct ConfigPolicySink {
    pub(super) role: &'static str,
    pub(super) repair_target: &'static str,
    pub(super) description: &'static str,
    pub(super) actionability: &'static str,
    pub(super) repair_kind: &'static str,
    pub(super) target_test_type: &'static str,
    pub(super) assertion_subject: &'static str,
}

#[derive(Clone)]
pub(super) struct PresentationTextDeclaration {
    pub(super) constant_name: String,
    pub(super) inline_literal: Option<String>,
}
