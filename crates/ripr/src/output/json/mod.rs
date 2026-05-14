mod context_packet;
mod finding_alignment;
mod formatter;
mod report;

pub use context_packet::render_context_packet;
pub(crate) use context_packet::render_context_packet_dto;
pub use report::render;
pub(crate) use report::render_with_config;

pub(crate) use formatter::{array_field, escape, field, float_field, number_field};

#[cfg(test)]
mod tests {
    use super::{context_packet::render_context_packet, render, report::finding_json};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FlowSinkFact,
        FlowSinkKind, LanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind,
        OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind, Summary,
        ValueContext, ValueFact,
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
            alignment["items"][0]["presentation_text"]["target_test_type"],
            "help_output_snapshot"
        );
        assert_eq!(
            alignment["items"][0]["presentation_text"]["suggested_assertion"],
            "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        );
        assert_eq!(alignment["items"][1]["gap_state"], "already_observed");
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

    fn unknown_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:1:static_unknown".to_string(),
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
