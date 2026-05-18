use crate::domain::Finding;

use super::{array_field, escape, field, number_field};

mod classify;
mod items;
mod model;
mod parse;
mod render;
mod summary;

pub(super) use model::FindingAlignmentReport;
pub(super) use render::report_json;

use classify::{classify_config_policy_constant, classify_presentation_text};
use items::{config_policy_item, presentation_text_item};
use parse::{
    adjacent_literal_index, parse_config_policy_declaration, parse_presentation_text_declaration,
    parse_string_literal, raw_finding_for,
};
use summary::summary_for;

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

#[cfg(test)]
mod tests {
    use super::model::{
        FindingAlignmentConfigPolicy, FindingAlignmentItem, FindingAlignmentPresentationText,
        FindingAlignmentRepairRoute, GROUP_REASON_CONFIG_POLICY, GROUP_REASON_DECL_LITERAL,
        GROUP_REASON_OWNER,
    };
    use super::parse::{parse_presentation_text_declaration, parse_string_literal};
    use super::report_for_findings;
    use super::summary::verify_command_is_missing;
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
        assert_eq!(
            item.canonical_gap_id,
            "config_or_policy_constant::INTERNAL_POLICY_LABEL"
        );
        assert_eq!(item.group_reason, GROUP_REASON_CONFIG_POLICY);
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
        assert!(!item.recommended_repair.contains("mutation"));
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
        item: &FindingAlignmentItem,
    ) -> Result<&FindingAlignmentPresentationText, String> {
        item.presentation_text
            .as_ref()
            .ok_or_else(|| "item should include presentation_text".to_string())
    }

    fn config_policy_for(
        item: &FindingAlignmentItem,
    ) -> Result<&FindingAlignmentConfigPolicy, String> {
        item.config_policy
            .as_ref()
            .ok_or_else(|| "item should include config_policy".to_string())
    }

    fn repair_route_for_item(
        item: &FindingAlignmentItem,
    ) -> Result<&FindingAlignmentRepairRoute, String> {
        item.repair_route
            .as_ref()
            .ok_or_else(|| "actionable item should include repair_route".to_string())
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }
}
