use super::model::*;
use super::{array_field, escape, field, number_field};

pub(in crate::output::json) fn report_json(
    out: &mut String,
    report: &FindingAlignmentReport,
    indent: usize,
) {
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
