use crate::domain::{Finding, MissingDiscriminatorFact, ValueFact, context_packet::ContextPacket};

use super::{array_field, escape, field, number_field};
use crate::output::json::report::{related_test_json, stop_reason_values};

// Keep the registered packet version literal in this renderer path so the
// output-contract check can verify the JSON surface while the DTO owns values.
const CONTEXT_PACKET_VERSION_CONTRACT: &str = "1.0";

pub fn render_context_packet(finding: &Finding, max_related_tests: usize) -> String {
    let stop_reasons = stop_reason_values(finding);
    let packet = ContextPacket::from_finding(finding, max_related_tests, stop_reasons);
    render_context_packet_dto(&packet)
}

fn render_context_packet_dto(packet: &ContextPacket) -> String {
    let _context_packet_version_contract = CONTEXT_PACKET_VERSION_CONTRACT;
    let mut out = String::new();
    out.push_str("{\n");
    field(&mut out, 1, "version", packet.version, true);
    field(&mut out, 1, "tool", packet.tool, true);
    out.push_str("  \"probe\": {\n");
    field(&mut out, 2, "id", &packet.probe.id, true);
    field(&mut out, 2, "family", &packet.probe.family, true);
    field(&mut out, 2, "delta", &packet.probe.delta, true);
    field(&mut out, 2, "file", &packet.probe.file, true);
    number_field(&mut out, 2, "line", packet.probe.line, true);
    field(
        &mut out,
        2,
        "changed_expression",
        &packet.probe.changed_expression,
        false,
    );
    out.push_str("  },\n");
    out.push_str("  \"ripr\": {\n");
    field(&mut out, 2, "reach", &packet.ripr.reach, true);
    field(&mut out, 2, "infect", &packet.ripr.infect, true);
    field(&mut out, 2, "propagate", &packet.ripr.propagate, true);
    field(&mut out, 2, "observe", &packet.ripr.observe, true);
    field(
        &mut out,
        2,
        "discriminate",
        &packet.ripr.discriminate,
        false,
    );
    out.push_str("  },\n");
    let related_test_count = packet.related_tests.len();
    out.push_str("  \"related_tests\": [\n");
    for (idx, test) in packet.related_tests.iter().enumerate() {
        related_test_json(&mut out, test, 2);
        if idx + 1 != related_test_count {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n");
    value_array(&mut out, 1, "observed_values", &packet.observed_values);
    out.push_str(",\n");
    discriminator_array(
        &mut out,
        1,
        "missing_discriminators",
        &packet.missing_discriminators,
    );
    out.push_str(",\n");
    array_field(&mut out, 1, "missing", &packet.missing, true);
    array_field(&mut out, 1, "stop_reasons", &packet.stop_reasons, true);
    field(
        &mut out,
        1,
        "recommended_next_step",
        packet.recommended_next_step.as_deref().unwrap_or(""),
        false,
    );
    out.push_str("}\n");
    out
}

fn value_array(out: &mut String, indent: usize, name: &str, values: &[ValueFact]) {
    out.push_str(&format!("{}\"{name}\": [", "  ".repeat(indent)));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!(
            "{{\"value\":\"{}\",\"context\":\"{}\",\"line\":{}}}",
            escape(&value.value),
            value.context.as_str(),
            value.line
        ));
    }
    out.push(']');
}

fn discriminator_array(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[MissingDiscriminatorFact],
) {
    out.push_str(&format!("{}\"{name}\": [", "  ".repeat(indent)));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!(
            "{{\"value\":\"{}\",\"reason\":\"{}\"}}",
            escape(&value.value),
            escape(&value.reason)
        ));
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::render_context_packet;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding,
        MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        SymbolId, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn context_packet_limits_related_tests_to_max() {
        let mut finding = sample_finding();
        finding.related_tests = vec![related("t1"), related("t2"), related("t3")];

        let packet = render_context_packet(&finding, 2);

        assert!(packet.contains("\"name\": \"t1\""));
        assert!(packet.contains("\"name\": \"t2\""));
        assert!(!packet.contains("\"name\": \"t3\""));
    }

    #[test]
    fn context_packet_escapes_observed_values_and_discriminator_reasons() {
        let mut finding = sample_finding();
        finding.activation.observed_values.push(ValueFact {
            line: 11,
            text: "assert!(x)".to_string(),
            value: "line \"a\"\nnext".to_string(),
            context: ValueContext::AssertionArgument,
        });
        finding
            .activation
            .missing_discriminators
            .push(MissingDiscriminatorFact {
                value: "v == \"boundary\"".to_string(),
                reason: "No test checks \"boundary\"\ncase".to_string(),
                flow_sink: None,
            });

        let packet = render_context_packet(&finding, 5);

        assert!(packet.contains("line \\\"a\\\"\\nnext"));
        assert!(packet.contains("No test checks \\\"boundary\\\"\\ncase"));
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:9:error_path".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:9:error_path".to_string()),
                location: SourceLocation::new("src/lib.rs", 9, 1),
                owner: Some(SymbolId("crate::sample".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("x > 0".to_string()),
                after: Some("x >= 0".to_string()),
                expression: "x >= 0".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage("Reachable"),
                infect: stage("Possible infection"),
                propagate: stage("Possible propagation"),
                reveal: RevealEvidence {
                    observe: stage("Observed"),
                    discriminate: stage("Weak discriminator"),
                },
            },
            confidence: 0.5,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests: vec![],
            recommended_next_step: None,
        }
    }

    fn related(name: &str) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: PathBuf::from("tests/sample.rs"),
            line: 7,
            oracle: Some("assert_eq!(value, 1)".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Yes, Confidence::Medium, summary)
    }
}
