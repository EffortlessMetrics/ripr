use super::{Finding, MissingDiscriminatorFact, RelatedTest, ValueFact};

#[derive(Clone, Debug, PartialEq)]
pub struct ContextPacket {
    pub version: &'static str,
    pub tool: &'static str,
    pub canonical_gap_id: Option<String>,
    pub probe: ContextPacketProbe,
    pub ripr: ContextPacketRipr,
    pub related_tests: Vec<RelatedTest>,
    pub observed_values: Vec<ValueFact>,
    pub missing_discriminators: Vec<MissingDiscriminatorFact>,
    pub missing: Vec<String>,
    pub stop_reasons: Vec<String>,
    pub recommended_next_step: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextPacketProbe {
    pub id: String,
    pub family: String,
    pub delta: String,
    pub file: String,
    pub line: usize,
    pub changed_expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextPacketRipr {
    pub reach: String,
    pub infect: String,
    pub propagate: String,
    pub observe: String,
    pub discriminate: String,
}

impl ContextPacket {
    pub fn from_finding(
        finding: &Finding,
        max_related_tests: usize,
        stop_reasons: Vec<String>,
    ) -> Self {
        Self {
            version: "1.0",
            tool: "ripr",
            canonical_gap_id: finding.canonical_gap.as_ref().map(|gap| gap.id.clone()),
            probe: ContextPacketProbe {
                id: finding.probe.id.0.clone(),
                family: finding.probe.family.as_str().to_string(),
                delta: finding.probe.delta.as_str().to_string(),
                file: finding.probe.location.file.display().to_string(),
                line: finding.probe.location.line,
                changed_expression: finding.probe.expression.clone(),
            },
            ripr: ContextPacketRipr {
                reach: finding.ripr.reach.state.as_str().to_string(),
                infect: finding.ripr.infect.state.as_str().to_string(),
                propagate: finding.ripr.propagate.state.as_str().to_string(),
                observe: finding.ripr.reveal.observe.state.as_str().to_string(),
                discriminate: finding.ripr.reveal.discriminate.state.as_str().to_string(),
            },
            related_tests: finding
                .related_tests
                .iter()
                .take(max_related_tests)
                .cloned()
                .collect(),
            observed_values: finding.activation.observed_values.clone(),
            missing_discriminators: finding.activation.missing_discriminators.clone(),
            missing: finding.missing.clone(),
            stop_reasons,
            recommended_next_step: finding.recommended_next_step.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, SymbolId,
        ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn context_packet_from_finding_carries_probe_and_ripr_shape() {
        let packet = ContextPacket::from_finding(
            &sample_finding(),
            5,
            vec!["missing related test".to_string()],
        );

        assert_eq!(packet.version, "1.0");
        assert_eq!(packet.tool, "ripr");
        assert_eq!(packet.canonical_gap_id, None);
        assert_eq!(packet.probe.id, "probe:src_lib_rs:9:predicate");
        assert_eq!(packet.probe.family, "predicate");
        assert_eq!(packet.probe.delta, "control");
        assert_eq!(packet.probe.file, "src/lib.rs");
        assert_eq!(packet.probe.line, 9);
        assert_eq!(packet.probe.changed_expression, "x >= 0");
        assert_eq!(packet.ripr.reach, "yes");
        assert_eq!(packet.ripr.infect, "yes");
        assert_eq!(packet.ripr.propagate, "yes");
        assert_eq!(packet.ripr.observe, "yes");
        assert_eq!(packet.ripr.discriminate, "yes");
        assert_eq!(packet.stop_reasons, vec!["missing related test"]);
    }

    #[test]
    fn context_packet_from_finding_limits_related_tests_and_copies_evidence() {
        let mut finding = sample_finding();
        finding.related_tests = vec![related("t1"), related("t2"), related("t3")];
        finding.activation.observed_values.push(ValueFact {
            line: 11,
            text: "assert_eq!(x, 1)".to_string(),
            value: "1".to_string(),
            context: ValueContext::AssertionArgument,
        });
        finding
            .activation
            .missing_discriminators
            .push(MissingDiscriminatorFact {
                value: "x == 0".to_string(),
                reason: "boundary not asserted".to_string(),
                flow_sink: None,
            });
        finding.missing.push("exact boundary assertion".to_string());
        finding.recommended_next_step = Some("add exact boundary assertion".to_string());

        let packet = ContextPacket::from_finding(&finding, 2, Vec::new());

        assert_eq!(packet.related_tests.len(), 2);
        assert_eq!(packet.related_tests[0].name, "t1");
        assert_eq!(packet.related_tests[1].name, "t2");
        assert_eq!(packet.observed_values, finding.activation.observed_values);
        assert_eq!(
            packet.missing_discriminators,
            finding.activation.missing_discriminators
        );
        assert_eq!(packet.missing, vec!["exact boundary assertion"]);
        assert_eq!(
            packet.recommended_next_step.as_deref(),
            Some("add exact boundary assertion")
        );
    }

    #[test]
    fn context_packet_from_finding_carries_canonical_gap_id() {
        let mut finding = sample_finding();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });

        let packet = ContextPacket::from_finding(&finding, 2, Vec::new());

        assert_eq!(
            packet.canonical_gap_id.as_deref(),
            Some(
                "gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
            )
        );
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:9:predicate".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:9:predicate".to_string()),
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
                    discriminate: stage("Discriminated"),
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
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
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
