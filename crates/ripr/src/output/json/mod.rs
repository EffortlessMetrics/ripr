mod context_packet;
mod finding_alignment;
mod formatter;
mod report;

pub use context_packet::render_context_packet;
pub(crate) use context_packet::render_context_packet_dto;
pub use report::render;
pub(crate) use report::render_with_config;

pub(crate) use formatter::{array_field, escape, field, float_field, number_field};

/// Renders a serializable JSON value with the repository's pretty-printing
/// convention and a consistent contextual error message.
pub(crate) fn render_pretty<T>(value: &T, context: &str) -> Result<String, String>
where
    T: serde::Serialize + ?Sized,
{
    serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to render {context} JSON: {err}"))
}

/// Renders pretty JSON and appends the trailing newline expected by artifact
/// writers that are consumed as line-oriented files.
pub(crate) fn render_pretty_with_newline<T>(value: &T, context: &str) -> Result<String, String>
where
    T: serde::Serialize + ?Sized,
{
    render_pretty(value, context).map(|mut rendered| {
        rendered.push('\n');
        rendered
    })
}

#[cfg(test)]
mod tests {
    use super::{context_packet::render_context_packet, render, report::finding_json};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        FlowSinkFact, FlowSinkKind, LanguageId, LanguageStatus, MissingDiscriminatorFact,
        OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest,
        RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind,
        Summary, SymbolId, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn finding_json_includes_effective_stop_reasons_for_unknowns() {
        let finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"stop_reasons\": [\"static_probe_unknown\"]"));
    }

    #[test]
    fn finding_json_promotes_evidence_first_fields() {
        let mut finding = unknown_finding();
        finding.flow_sinks.push(FlowSinkFact {
            kind: FlowSinkKind::ReturnValue,
            text: "total".to_string(),
            line: 7,
            owner: None,
        });
        finding.activation.observed_values.push(ValueFact {
            line: 12,
            text: "discounted_total(50, 100)".to_string(),
            value: "amount = 50".to_string(),
            context: ValueContext::FunctionArgument,
        });
        finding
            .activation
            .missing_discriminators
            .push(MissingDiscriminatorFact {
                value: "amount == discount_threshold".to_string(),
                reason: "No related test calls the boundary value".to_string(),
                flow_sink: None,
            });
        finding.related_tests.push(RelatedTest {
            name: "below_threshold_has_no_discount".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 12,
            oracle: Some("assert_eq!(discounted_total(50, 100), 50);".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
        });

        let mut out = String::new();
        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"evidence_path\""));
        assert!(out.contains("\"flow_sinks\""));
        assert!(out.contains("\"observed_values\""));
        assert!(out.contains("\"missing_discriminators\""));
        assert!(out.contains("\"oracle_kind\": \"exact_value\""));
        assert!(out.contains("\"oracle_strength\": \"strong\""));
        assert!(out.contains("\"suggested_next_action\""));
    }

    #[test]
    fn context_packet_includes_effective_stop_reasons_for_unknowns() {
        let finding = unknown_finding();
        let packet = render_context_packet(&finding, 5);

        assert!(packet.contains("\"stop_reasons\": [\"static_probe_unknown\"]"));
    }

    #[test]
    fn render_omits_base_when_not_set() {
        let output = sample_output(None);
        let rendered = render(&output);

        assert!(!rendered.contains("\"base\""));
    }

    #[test]
    fn render_includes_base_when_set() {
        let output = sample_output(Some("origin/main".to_string()));
        let rendered = render(&output);

        assert!(rendered.contains("\"base\": \"origin/main\""));
    }

    #[test]
    fn render_adds_presentation_text_finding_alignment_when_supported() -> Result<(), String> {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            summary: Summary::default(),
            findings: vec![
                finding_with_expression(
                    "decl",
                    46,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =",
                ),
                finding_with_expression(
                    "literal",
                    47,
                    ExposureClass::StaticUnknown,
                    ProbeFamily::StaticUnknown,
                    "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
                ),
            ],
        };

        let rendered = render(&output);
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("check JSON should parse: {err}"))?;
        let alignment = &value["finding_alignment"];

        assert_eq!(alignment["scope"], "supported_classes");
        assert_eq!(alignment["summary"]["raw_signals"], 2);
        assert_eq!(alignment["summary"]["canonical_items"], 1);
        assert_eq!(alignment["summary"]["aligned_raw_findings"], 2);
        assert_eq!(alignment["summary"]["static_limitations"], 1);
        assert_eq!(alignment["summary"]["repair_route_coverage"], 0);
        assert_eq!(
            alignment["summary"]["actionable_items_without_repair_route"],
            0
        );
        assert_eq!(alignment["summary"]["verify_command_coverage"], 0);
        assert_eq!(
            alignment["summary"]["actionable_items_without_verify_command"],
            0
        );
        assert_eq!(
            alignment["items"][0]["canonical_gap_id"],
            "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT"
        );
        assert_eq!(
            alignment["items"][0]["group_reason"],
            "declaration_and_literal_same_text_constant"
        );
        assert_eq!(alignment["items"][0]["raw_group_size"], 2);
        assert_eq!(alignment["items"][0]["gap_state"], "static_limitation");
        assert_eq!(alignment["items"][0]["actionability"], "inspect_visibility");
        assert_eq!(alignment["items"][0]["primary_anchor"]["line"], 46);
        assert_eq!(
            alignment["items"][0]["primary_anchor"]["reason"],
            "declaration_line_for_grouped_constant"
        );
        assert_eq!(alignment["items"][0]["raw_spans"][0]["start_line"], 46);
        assert_eq!(alignment["items"][0]["raw_spans"][0]["end_line"], 46);
        assert_eq!(alignment["items"][0]["raw_spans"][1]["start_line"], 47);
        assert_eq!(alignment["items"][0]["raw_spans"][1]["end_line"], 47);
        assert!(alignment["items"][0]["repair_route"].is_null());
        assert_eq!(
            alignment["items"][0]["static_limitations"][0]["category"],
            "presentation_text_visibility_unknown"
        );
        assert_eq!(
            alignment["items"][0]["presentation_text"]["text_literal"],
            "apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane"
        );
        assert!(
            !alignment["items"][0]["recommended_repair"]
                .as_str()
                .unwrap_or_default()
                .contains("mutation")
        );
        Ok(())
    }

    #[test]
    fn render_projects_presentation_text_visibility_and_observer_states() -> Result<(), String> {
        let golden = RelatedTest {
            name: "report_golden_observes_label".to_string(),
            file: PathBuf::from("tests/golden/report_output.rs"),
            line: 22,
            oracle: None,
            oracle_kind: OracleKind::Snapshot,
            oracle_strength: OracleStrength::Strong,
        };
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            summary: Summary::default(),
            findings: vec![
                finding_with_expression_in_file(
                    "src/help.rs",
                    "help-decl",
                    18,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const HELP_DEVICE_LABEL: &str =",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/help.rs",
                    "help-literal",
                    19,
                    ExposureClass::WeaklyExposed,
                    ProbeFamily::StaticUnknown,
                    "\"Device label\";",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/report.rs",
                    "report-decl",
                    27,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const REPORT_DEVICE_LABEL: &str =",
                    vec![golden],
                ),
                finding_with_expression_in_file(
                    "src/report.rs",
                    "report-literal",
                    28,
                    ExposureClass::Exposed,
                    ProbeFamily::StaticUnknown,
                    "\"Report label\";",
                    vec![],
                ),
            ],
        };

        let rendered = render(&output);
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("check JSON should parse: {err}"))?;
        let alignment = &value["finding_alignment"];

        assert_eq!(alignment["summary"]["canonical_items"], 2);
        assert_eq!(alignment["summary"]["actionable_gaps"], 1);
        assert_eq!(alignment["summary"]["repair_route_coverage"], 1);
        assert_eq!(
            alignment["summary"]["actionable_items_without_repair_route"],
            0
        );
        assert_eq!(alignment["summary"]["verify_command_coverage"], 1);
        assert_eq!(
            alignment["summary"]["actionable_items_without_verify_command"],
            0
        );
        assert_eq!(alignment["summary"]["already_observed"], 1);
        assert_eq!(
            alignment["summary"]["presentation_text_actionable_output_repairs"],
            1
        );
        assert_eq!(alignment["items"][0]["gap_state"], "actionable");
        assert_eq!(
            alignment["items"][0]["presentation_text"]["recommended_observer"],
            "cli_help_output"
        );
        assert_eq!(
            alignment["items"][0]["presentation_text"]["repair_kind"],
            "output_observer"
        );
        assert_eq!(
            alignment["items"][0]["repair_route"]["repair_kind"],
            "output_observer"
        );
        assert_eq!(
            alignment["items"][0]["presentation_text"]["target_test_type"],
            "help_output_snapshot"
        );
        assert_eq!(
            alignment["items"][0]["repair_route"]["target_test_type"],
            "help_output_snapshot"
        );
        assert_eq!(
            alignment["items"][0]["presentation_text"]["suggested_assertion"],
            "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        );
        assert_eq!(
            alignment["items"][0]["repair_route"]["suggested_assertion"],
            "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        );
        assert_eq!(alignment["items"][1]["gap_state"], "already_observed");
        assert!(alignment["items"][1]["repair_route"].is_null());
        assert_eq!(
            alignment["items"][1]["presentation_text"]["observer"],
            "golden"
        );
        assert_eq!(
            alignment["items"][1]["presentation_text"]["repair_kind"],
            "no_action"
        );
        assert_eq!(
            alignment["items"][1]["related_test"]["name"],
            "report_golden_observes_label"
        );
        Ok(())
    }

    #[test]
    fn render_projects_config_policy_alignment_states() -> Result<(), String> {
        let golden = RelatedTest {
            name: "schema_render_golden_observes_field".to_string(),
            file: PathBuf::from("tests/golden/schema_output.rs"),
            line: 31,
            oracle: None,
            oracle_kind: OracleKind::Snapshot,
            oracle_strength: OracleStrength::Strong,
        };
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            summary: Summary::default(),
            findings: vec![
                finding_with_expression_in_file(
                    "src/policy.rs",
                    "policy-decl",
                    14,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const INTERNAL_POLICY_LABEL: &str =",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/policy.rs",
                    "policy-literal",
                    15,
                    ExposureClass::StaticUnknown,
                    ProbeFamily::StaticUnknown,
                    "\"internal policy label\";",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/report_config.rs",
                    "report-policy-decl",
                    22,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const REPORT_POLICY_LABEL: &str =",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/report_config.rs",
                    "report-policy-literal",
                    23,
                    ExposureClass::WeaklyExposed,
                    ProbeFamily::StaticUnknown,
                    "\"Policy label\";",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/schema.rs",
                    "schema-decl",
                    31,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const SCHEMA_POLICY_FIELD: &str =",
                    vec![golden],
                ),
                finding_with_expression_in_file(
                    "src/schema.rs",
                    "schema-literal",
                    32,
                    ExposureClass::Exposed,
                    ProbeFamily::StaticUnknown,
                    "\"policy\";",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/config_registry.rs",
                    "opaque-decl",
                    58,
                    ExposureClass::Exposed,
                    ProbeFamily::FieldConstruction,
                    "pub const OPAQUE_CONFIG_LABEL: &str =",
                    vec![],
                ),
                finding_with_expression_in_file(
                    "src/config_registry.rs",
                    "opaque-literal",
                    59,
                    ExposureClass::StaticUnknown,
                    ProbeFamily::StaticUnknown,
                    "\"Opaque label\";",
                    vec![],
                ),
            ],
        };

        let rendered = render(&output);
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("check JSON should parse: {err}"))?;
        let alignment = &value["finding_alignment"];

        assert_eq!(
            alignment["supported_evidence_classes"][1],
            "config_or_policy_constant"
        );
        assert_eq!(alignment["summary"]["canonical_items"], 4);
        assert_eq!(alignment["summary"]["config_policy_constant_total"], 4);
        assert_eq!(alignment["summary"]["config_policy_internal_only"], 1);
        assert_eq!(
            alignment["summary"]["config_policy_actionable_output_observer"],
            1
        );
        assert_eq!(alignment["summary"]["repair_route_coverage"], 1);
        assert_eq!(
            alignment["summary"]["actionable_items_without_repair_route"],
            0
        );
        assert_eq!(alignment["summary"]["verify_command_coverage"], 1);
        assert_eq!(
            alignment["summary"]["actionable_items_without_verify_command"],
            0
        );
        assert_eq!(alignment["summary"]["config_policy_observed"], 1);
        assert_eq!(alignment["summary"]["config_policy_static_limitations"], 1);
        assert_eq!(alignment["items"][0]["gap_state"], "internal_only");
        assert_eq!(
            alignment["items"][0]["config_policy"]["role"],
            "internal_policy_metadata"
        );
        assert_eq!(alignment["items"][1]["gap_state"], "actionable");
        assert_eq!(
            alignment["items"][1]["config_policy"]["target_test_type"],
            "report_render_or_golden"
        );
        assert_eq!(
            alignment["items"][1]["repair_route"]["repair_kind"],
            "output_observer"
        );
        assert_eq!(
            alignment["items"][1]["repair_route"]["target_test_type"],
            "report_render_or_golden"
        );
        assert_eq!(alignment["items"][2]["gap_state"], "already_observed");
        assert!(alignment["items"][2]["repair_route"].is_null());
        assert_eq!(alignment["items"][2]["config_policy"]["observer"], "golden");
        assert_eq!(alignment["items"][3]["gap_state"], "static_limitation");
        assert_eq!(
            alignment["items"][3]["static_limitations"][0]["category"],
            "opaque_config_lookup"
        );
        assert!(alignment["items"][0]["presentation_text"].is_null());
        Ok(())
    }

    #[test]
    fn render_omits_finding_alignment_without_supported_items() -> Result<(), String> {
        let output = sample_output(None);
        let rendered = render(&output);
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("check JSON should parse: {err}"))?;

        assert!(value.get("finding_alignment").is_none());
        Ok(())
    }

    #[test]
    fn finding_json_uses_strongest_related_test_for_oracle_summary() {
        let mut finding = unknown_finding();
        finding.related_tests = vec![
            RelatedTest {
                name: "smoke_test".to_string(),
                file: PathBuf::from("tests/smoke.rs"),
                line: 10,
                oracle: Some("assert!(ok())".to_string()),
                oracle_kind: OracleKind::RelationalCheck,
                oracle_strength: OracleStrength::Weak,
            },
            RelatedTest {
                name: "strict_check".to_string(),
                file: PathBuf::from("tests/strict.rs"),
                line: 21,
                oracle: Some("assert_eq!(value, 42)".to_string()),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            },
        ];

        let mut out = String::new();
        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"oracle_strength\": \"strong\""));
        assert!(out.contains("\"oracle_kind\": \"exact_value\""));
    }

    #[test]
    fn finding_json_formats_observed_value_context_labels() {
        let mut finding = unknown_finding();
        finding.activation.observed_values.push(ValueFact {
            line: 33,
            text: "assert_eq!(actual, expected)".to_string(),
            value: "actual = 10".to_string(),
            context: ValueContext::AssertionArgument,
        });

        let mut out = String::new();
        finding_json(&mut out, &finding, 0);

        assert!(out.contains("observed assertion argument value actual = 10 at line 33"));
    }

    #[test]
    fn finding_json_defaults_oracle_summary_when_related_tests_are_empty() {
        let finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"oracle_kind\": \"unknown\""));
        assert!(out.contains("\"oracle_strength\": \"none\""));
    }

    #[test]
    fn finding_json_limits_evidence_path_related_tests_to_five_entries() {
        let mut finding = unknown_finding();
        finding.related_tests = (0..6)
            .map(|index| RelatedTest {
                name: format!("test_case_{index}"),
                file: PathBuf::from("tests/json_contract.rs"),
                line: 100 + index,
                oracle: Some(format!("assert_eq!(actual, {index});")),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            })
            .collect();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains("test_case_0 uses strong exact value oracle"));
        assert!(out.contains("test_case_4 uses strong exact value oracle"));
        assert!(!out.contains("test_case_5 uses strong exact value oracle"));
    }

    #[test]
    fn finding_json_escapes_special_characters_in_recommended_next_step() {
        let mut finding = unknown_finding();
        finding.recommended_next_step = Some("Verify \"quoted\" step\nthen patch".to_string());
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(
            out.contains("\"recommended_next_step\": \"Verify \\\"quoted\\\" step\\nthen patch\"")
        );
        assert!(
            out.contains("\"suggested_next_action\": \"Verify \\\"quoted\\\" step\\nthen patch\"")
        );
    }

    #[test]
    fn finding_json_emits_empty_next_step_fields_when_recommendation_is_missing() {
        let mut finding = unknown_finding();
        finding.recommended_next_step = None;
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"recommended_next_step\": \"\""));
        assert!(out.contains("\"suggested_next_action\": \"\""));
    }

    #[test]
    fn finding_json_emits_static_limit_kind_only_when_present() {
        let mut finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(!out.contains("\"static_limit_kind\""));

        finding.static_limit_kind = Some(StaticLimitKind::MockedModule);
        out.clear();
        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"static_limit_kind\": \"mocked_module\""));
    }

    #[test]
    fn finding_json_emits_owner_kind_only_when_present() {
        let mut finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(!out.contains("\"owner_kind\""));

        finding.owner_kind = Some(OwnerKind::Function);
        out.clear();
        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"owner_kind\": \"function\""));
    }

    #[test]
    fn finding_json_emits_probe_owner_only_when_present() -> Result<(), String> {
        let mut finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);
        let value: serde_json::Value = serde_json::from_str(&out)
            .map_err(|err| format!("finding JSON should parse: {err}"))?;
        assert!(
            value["probe"].get("owner").is_none(),
            "probe.owner should be omitted when no owner is populated"
        );

        finding.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        out.clear();
        finding_json(&mut out, &finding, 0);
        let value: serde_json::Value = serde_json::from_str(&out)
            .map_err(|err| format!("finding JSON should parse: {err}"))?;

        assert_eq!(value["probe"]["owner"], "python:src/pricing.py::discount");
        Ok(())
    }

    #[test]
    fn finding_json_emits_canonical_gap_identity_when_present() -> Result<(), String> {
        let mut finding = unknown_finding();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "apply_discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);
        let value: serde_json::Value = serde_json::from_str(&out)
            .map_err(|err| format!("finding JSON should parse: {err}"))?;

        assert_eq!(
            value["canonical_gap_id"],
            "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
        );
        assert_eq!(value["canonical_gap_group_size"], 1);
        assert_eq!(value["canonical_gap"]["language"], "python");
        assert_eq!(value["canonical_gap"]["file"], "src/pricing.py");
        assert_eq!(value["canonical_gap"]["owner"], "apply_discount");
        assert_eq!(
            value["canonical_gap"]["behavior_kind"],
            "predicate_boundary"
        );
        assert_eq!(value["canonical_gap"]["probe_kind"], "predicate");
        assert_eq!(
            value["canonical_gap"]["normalized_discriminator"],
            "amount>=threshold"
        );
        Ok(())
    }

    #[test]
    fn finding_json_preserves_language_metadata_order_with_static_limit_kind() {
        let mut finding = unknown_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(OwnerKind::Function);
        finding.static_limit_kind = Some(StaticLimitKind::MockedModule);
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains(
            "\"suggested_next_action\": \"Escalate to real mutation testing.\",\n  \
             \"language\": \"typescript\",\n  \
             \"language_status\": \"preview\",\n  \
             \"owner_kind\": \"function\",\n  \
             \"static_limit_kind\": \"mocked_module\""
        ));
    }

    #[test]
    fn finding_json_projects_typescript_preview_actionability() -> Result<(), String> {
        let mut finding = unknown_finding();
        add_typescript_preview_actionability(&mut finding);
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);
        let value: serde_json::Value = serde_json::from_str(&out)
            .map_err(|err| format!("finding JSON parse failed: {err}"))?;

        let actionability = &value["preview_actionability"];
        assert_eq!(actionability["authority_boundary"], "preview_advisory_only");
        assert_eq!(actionability["repair_packet_ready"], false);
        assert_eq!(actionability["gap_state"], "advisory");
        assert_eq!(
            actionability["actionability_category"],
            "incomplete_repair_packet"
        );
        assert_eq!(
            actionability["missing_actionability_fields"][0],
            "canonical_gap_id"
        );
        assert_eq!(actionability["raw_evidence_refs"][0]["file"], "src/lib.ts");
        assert_eq!(actionability["raw_evidence_refs"][0]["line"], 2);
        assert!(value.get("repair_placement").is_none());
        assert!(value.get("python_repair_card").is_none());
        Ok(())
    }

    #[test]
    fn finding_json_projects_perl_preview_card_json_and_human_scope() -> Result<(), String> {
        let mut finding = unknown_finding();
        add_perl_preview_card_inputs(&mut finding);
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);
        let value: serde_json::Value = serde_json::from_str(&out)
            .map_err(|err| format!("finding JSON parse failed: {err}"))?;

        let card = &value["perl_preview_card"];
        assert_eq!(card["card_version"], "perl_preview_card.v1");
        assert_eq!(card["language"], "perl");
        assert_eq!(card["language_status"], "preview");
        assert_eq!(card["authority_boundary"], "preview_advisory_only");
        assert_eq!(card["surface_scope"], "check_json_and_human");
        assert_eq!(card["public_projection_ready"], true);
        assert_eq!(card["public_repair_packet"], false);
        assert_eq!(card["repair_packet_ready"], false);
        assert_eq!(card["agent_packet_ready"], false);
        assert_eq!(card["gate_candidate"], false);
        assert_eq!(card["badge_candidate"], false);
        assert_eq!(card["ripr_zero_candidate"], false);
        assert_eq!(card["verify"]["command"], "prove t/app.t");
        assert_eq!(card["verify"]["status"], "fact_only_not_delegated");
        assert!(card["receipt"]["command"].is_null());
        assert_eq!(card["receipt"]["status"], "available_not_delegated");
        assert_eq!(card["raw_evidence_refs"][0]["file"], "lib/My/App.pm");
        assert_eq!(card["raw_evidence_refs"][0]["line"], 8);
        assert!(card.get("allowed_edit_surface").is_none());
        assert!(card.get("forbidden_files").is_none());
        assert!(card.get("allowed_edit_boundaries").is_none());
        assert!(card.get("forbidden_edit_boundaries").is_none());
        assert!(card["receipt"].get("argv").is_none());
        assert!(!out.contains("perl_internal_agent_packet"));
        assert!(!out.contains("perl_repair_card"));
        Ok(())
    }

    fn add_typescript_preview_actionability(finding: &mut Finding) {
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(OwnerKind::Function);
        finding.evidence = vec![
            "owner: discountedTotal".to_string(),
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract".to_string(),
            "repair_route: project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available".to_string(),
            "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];
    }

    fn add_perl_preview_card_inputs(finding: &mut Finding) {
        finding.id = "probe:lib_My_App_pm:8:perl_return".to_string();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
                .to_string(),
            language: "perl".to_string(),
            file: "lib/My/App.pm".to_string(),
            owner: "perl:lib/My/App.pm::My::App::discount".to_string(),
            behavior_kind: "return_value".to_string(),
            probe_kind: "exact_return_assertion".to_string(),
            normalized_discriminator: "return_value".to_string(),
        });
        finding.probe = Probe {
            id: ProbeId("probe:lib_My_App_pm:8:perl_return".to_string()),
            location: SourceLocation::new("lib/My/App.pm", 8, 5),
            owner: Some(SymbolId(
                "perl:lib/My/App.pm::My::App::discount".to_string(),
            )),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("return $price".to_string()),
            after: Some("return $discounted".to_string()),
            expression: "return $discounted".to_string(),
            expected_sinks: vec!["return_value".to_string()],
            required_oracles: vec!["exact_return_assertion".to_string()],
        };
        finding.class = ExposureClass::WeaklyExposed;
        finding.evidence = vec![
            "perl_packet_id: perl-preview:gap-return".to_string(),
            "perl_repair_kind: add_exact_return_assertion".to_string(),
            "perl_target_test_shape: Test::More exact_return_assertion".to_string(),
            "perl_suggested_test_location: t/app.t::discount_smoke".to_string(),
            "perl_suggested_assertion: assert the exact returned `return_value` value".to_string(),
            "perl_verify_command: prove t/app.t".to_string(),
            "perl_receipt_command: ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json".to_string(),
            "perl_confidence: medium".to_string(),
            "perl_allowed_edit_boundary: t/app.t".to_string(),
            "perl_forbidden_edit_boundary: lib/My/App.pm, badges/ripr-plus.json".to_string(),
            "perl_stop_if: perl-lsp packet status changes".to_string(),
            "perl_must_not_change: do not edit Perl production code".to_string(),
            "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted".to_string(),
            "raw_evidence_ref: leg=perl_oracle;file=t/app.t;line=7;kind=perl_oracle;source_id=oracle:t/app.t:7:is;owner=perl:lib/My/App.pm::My::App::discount;sample=is(discount(...), 90)".to_string(),
        ];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "return_value".to_string(),
            reason: "Related Perl test reaches the owner but lacks an exact return discriminator"
                .to_string(),
            flow_sink: None,
        }];
        finding.related_tests = vec![RelatedTest {
            name: "discount_smoke".to_string(),
            file: PathBuf::from("t/app.t"),
            line: 7,
            oracle: Some("ok(discount(...))".to_string()),
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
        }];
        finding.recommended_next_step = Some("Add a focused Perl assertion.".to_string());
        finding.language = Some(LanguageId::Perl);
        finding.language_status = Some(LanguageStatus::Preview);
    }

    fn unknown_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:1:static_unknown".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:1:static_unknown".to_string()),
                location: SourceLocation::new("src/lib.rs", 1, 1),
                owner: None,
                family: ProbeFamily::StaticUnknown,
                delta: DeltaKind::Unknown,
                before: None,
                after: None,
                expression: "unknown syntax".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::StaticUnknown,
            ripr: RiprEvidence {
                reach: stage("No stable syntax owner"),
                infect: stage("Changed syntax is not mapped to a probe"),
                propagate: stage("No propagation model is available"),
                reveal: RevealEvidence {
                    observe: stage("No observation model is available"),
                    discriminate: stage("No discriminator model is available"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests: vec![],
            recommended_next_step: Some("Escalate to real mutation testing.".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }

    fn sample_output(base: Option<String>) -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base,
            summary: Summary::default(),
            findings: vec![unknown_finding()],
        }
    }

    fn finding_with_expression(
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
    ) -> Finding {
        finding_with_expression_in_file(
            "src/device_labels.rs",
            id_suffix,
            line,
            class,
            family,
            expression,
            vec![],
        )
    }

    fn finding_with_expression_in_file(
        file: &str,
        id_suffix: &str,
        line: usize,
        class: ExposureClass,
        family: ProbeFamily,
        expression: &str,
        related_tests: Vec<RelatedTest>,
    ) -> Finding {
        let mut finding = unknown_finding();
        let id = format!("probe:src_device_labels_rs:{line}:{id_suffix}");
        finding.id = id.clone();
        finding.probe.id = ProbeId(id);
        finding.probe.location = SourceLocation::new(file, line, 1);
        finding.probe.family = family;
        finding.probe.expression = expression.to_string();
        finding.class = class;
        finding.related_tests = related_tests;
        finding.recommended_next_step = None;
        finding
    }
}
