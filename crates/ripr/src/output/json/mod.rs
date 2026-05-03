mod context_packet;
mod formatter;
mod report;

pub use context_packet::render_context_packet;
pub use report::render;

pub(crate) use formatter::{array_field, escape, field, float_field, number_field};

#[cfg(test)]
mod tests {
    use super::{context_packet::render_context_packet, escape, render, report::finding_json};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FlowSinkFact,
        FlowSinkKind, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, Summary, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }

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
}
