use crate::app::CheckOutput;
use crate::config::RiprConfig;
use crate::output::preview_actionability::preview_actionability_for;
use crate::output::python_repair_card::python_repair_card;

/// Render findings as GitHub Actions workflow command annotations.
///
/// Each finding is emitted as one escaped annotation line so newlines and
/// punctuation survive GitHub's workflow-command parser.
pub fn render(output: &CheckOutput) -> String {
    render_with_config(output, &RiprConfig::default())
}

pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let mut out = String::new();
    for finding in &output.findings {
        let Some(annotation_level) = config
            .severity()
            .for_exposure(&finding.class)
            .github_annotation_level()
        else {
            continue;
        };
        let title = format!("ripr {}", finding.class.as_str());
        let mut message = finding
            .recommended_next_step
            .as_deref()
            .unwrap_or("Static RIPR exposure finding")
            .to_string();
        let stop_reasons = finding.effective_stop_reasons();
        if !stop_reasons.is_empty() {
            let reasons = stop_reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(" Stop reason: ");
            message.push_str(&reasons);
        }
        if let Some(gap) = &finding.canonical_gap {
            message.push_str(" Canonical gap: ");
            message.push_str(&gap.id);
        }
        if let Some(actionability) = preview_actionability_for(finding) {
            message.push_str(" Preview actionability: ");
            message.push_str(&actionability.gap_state);
            message.push('/');
            message.push_str(&actionability.actionability_category);
            message.push_str(" (advisory preview; no repair packet).");
        }
        if let Some(card) = python_repair_card(finding) {
            message.push_str(" Python repair card: missing discriminator `");
            message.push_str(&card.missing_discriminator);
            message.push_str("`; add or strengthen `");
            message.push_str(&card.suggested_test_name);
            message.push_str("` in `");
            message.push_str(&card.suggested_test_file);
            message.push_str("`; verify `");
            message.push_str(&card.verify_command);
            message.push_str("` (preview advisory).");
        }
        out.push_str(&format!(
            "::{annotation_level} file={},line={},title={}::{}\n",
            finding.probe.location.file.display(),
            finding.probe.location.line,
            escape_cmd(&title),
            escape_cmd(&message)
        ));
    }
    if output.findings.is_empty() {
        out.push_str("::notice title=ripr::No static mutation exposure findings found\n");
    }
    out
}

fn escape_cmd(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(',', "%2C")
        .replace(':', "%3A")
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, Summary,
    };
    use std::path::PathBuf;

    #[test]
    fn render_reports_empty_findings_as_notice() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
        };

        let rendered = render(&output);

        assert_eq!(
            rendered,
            "::notice title=ripr::No static mutation exposure findings found\n"
        );
    }

    #[test]
    fn render_escapes_annotations_and_includes_effective_stop_reason_for_unknowns() {
        let rendered = render(&output_with_unknown_finding());

        assert!(rendered.contains("::notice file=src/lib.rs,line=13,title=ripr static_unknown::"));
        assert!(rendered.contains("Add%3A case%2C with 100%25 coverage%0Athen verify%0Doutcome"));
        assert!(rendered.contains("Stop reason%3A static_probe_unknown"));
    }

    #[test]
    fn render_uses_warning_for_exposed_and_default_message_without_stop_reason() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:21:error_path".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:21:error_path".to_string()),
                    location: SourceLocation::new("src/lib.rs", 21, 1),
                    owner: None,
                    family: ProbeFamily::ErrorPath,
                    delta: DeltaKind::Control,
                    before: Some("ok".to_string()),
                    after: Some("err".to_string()),
                    expression: "result".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::Exposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::Yes, "discriminated"),
                    },
                },
                confidence: 1.0,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: None,
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
            }],
        };

        let rendered = render(&output);

        assert!(rendered.contains("::notice file=src/lib.rs,line=21,title=ripr exposed::"));
        assert!(rendered.contains("Static RIPR exposure finding"));
        assert!(!rendered.contains("Stop reason"));
    }

    #[test]
    fn render_uses_warning_annotation_for_warning_severity_findings() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:34:weak_signal".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:34:weak_signal".to_string()),
                    location: SourceLocation::new("src/lib.rs", 34, 1),
                    owner: None,
                    family: ProbeFamily::Predicate,
                    delta: DeltaKind::Control,
                    before: Some("x > 0".to_string()),
                    after: Some("x >= 0".to_string()),
                    expression: "x > 0".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::WeaklyExposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::No, "not discriminated"),
                    },
                },
                confidence: 0.7,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: Some("Add discriminator assertion".to_string()),
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
            }],
        };

        let rendered = render(&output);

        assert!(rendered.contains("::warning file=src/lib.rs,line=34,title=ripr weakly_exposed::"));
        assert!(rendered.contains("Add discriminator assertion"));
    }

    #[test]
    fn render_includes_canonical_gap_id_when_present() {
        let mut output = output_with_unknown_finding();
        output.findings[0].canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });

        let rendered = render(&output);

        assert!(rendered.contains(
            "Canonical gap%3A gap%3Apython%3Asrc/pricing.py%3Adiscount%3Apredicate_boundary%3Apredicate%3Aamount>=threshold"
        ));
    }

    #[test]
    fn render_includes_preview_actionability_boundary() {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.evidence = vec![
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];

        let rendered = render(&output);

        assert!(rendered.contains(
            "Preview actionability%3A advisory/incomplete_repair_packet (advisory preview; no repair packet)."
        ));
    }

    #[test]
    fn render_includes_python_repair_card_guidance() {
        let rendered = render(&output_with_python_repair_card());

        assert!(
            rendered.contains("Python repair card%3A missing discriminator `amount == threshold`")
        );
        assert!(rendered.contains("add or strengthen `test_calculate_discount_threshold_boundary` in `tests/test_pricing.py`"));
        assert!(rendered.contains(
            "verify `pytest tests/test_pricing.py%3A%3Atest_calculate_discount_threshold_boundary` (preview advisory)."
        ));
    }

    fn output_with_unknown_finding() -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:13:static_unknown".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:13:static_unknown".to_string()),
                    location: SourceLocation::new("src/lib.rs", 13, 1),
                    owner: None,
                    family: ProbeFamily::StaticUnknown,
                    delta: DeltaKind::Unknown,
                    before: None,
                    after: None,
                    expression: "opaque".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::StaticUnknown,
                ripr: RiprEvidence {
                    reach: stage(StageState::Unknown, "reach unknown"),
                    infect: stage(StageState::Unknown, "infection unknown"),
                    propagate: stage(StageState::Unknown, "propagation unknown"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Unknown, "observe unknown"),
                        discriminate: stage(StageState::Unknown, "discriminate unknown"),
                    },
                },
                confidence: 0.2,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests: vec![],
                recommended_next_step: Some(
                    "Add: case, with 100% coverage\nthen verify\routcome".to_string(),
                ),
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
            }],
        }
    }

    fn output_with_python_repair_card() -> CheckOutput {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.id = "probe:src_pricing.py:2:python_preview".to_string();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "calculate_discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });
        finding.probe = Probe {
            id: ProbeId("probe:src_pricing.py:2:python_preview".to_string()),
            location: SourceLocation::new("src/pricing.py", 2, 5),
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: Some("amount > threshold".to_string()),
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec!["return_value".to_string()],
            required_oracles: vec!["exact boundary assertion".to_string()],
        };
        finding.class = ExposureClass::WeaklyExposed;
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "amount == threshold".to_string(),
            reason: "no related test calls the equality boundary".to_string(),
            flow_sink: None,
        }];
        finding.evidence = vec![
            "suggested_test_file: tests/test_pricing.py".to_string(),
            "suggested_test_name: test_calculate_discount_threshold_boundary".to_string(),
            "suggested_test_node_id: tests/test_pricing.py::test_calculate_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command: pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command_confidence: high".to_string(),
        ];
        finding.related_tests = vec![RelatedTest {
            name: "test_calculate_discount_above_threshold".to_string(),
            file: PathBuf::from("tests/test_pricing.py"),
            line: 6,
            oracle: Some("assert result".to_string()),
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
        }];
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.recommended_next_step = Some("Add a Python boundary assertion".to_string());
        output
    }

    fn stage(state: StageState, reason: &str) -> StageEvidence {
        StageEvidence {
            state,
            confidence: Confidence::Low,
            summary: reason.to_string(),
        }
    }
}
