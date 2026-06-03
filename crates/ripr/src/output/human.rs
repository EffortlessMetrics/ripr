use crate::app::CheckOutput;
use crate::config::RiprConfig;
use crate::domain::Finding;

/// Render the complete check report in the human-readable CLI format.
pub fn render(output: &CheckOutput) -> String {
    render_with_config(output, &RiprConfig::default())
}

pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "ripr static RIPR exposure analysis\nmode: {}\nroot: {}\n\n",
        output.mode.as_str(),
        output.root.display()
    ));
    out.push_str(&format!(
        "Summary: {} probe(s), {} exposed, {} weak, {} unrevealed, {} no path, {} unknown\n\n",
        output.summary.probes,
        output.summary.exposed,
        output.summary.weakly_exposed,
        output.summary.reachable_unrevealed,
        output.summary.no_static_path,
        output.summary.static_unknown
            + output.summary.infection_unknown
            + output.summary.propagation_unknown
    ));

    if output.findings.is_empty() {
        out.push_str("No diff-derived mutation exposure probes found.\n");
        return out;
    }

    for finding in &output.findings {
        out.push_str(&render_finding_with_config(finding, config));
        out.push('\n');
    }
    out
}

/// Render one finding section for the human-readable CLI output.
pub fn render_finding(finding: &Finding) -> String {
    render_finding_with_config(finding, &RiprConfig::default())
}

mod evidence_lines;
mod sections;

pub(crate) use sections::render_finding_with_config;

#[cfg(test)]
mod tests {
    use super::{render, render_finding};
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        FlowSinkFact, FlowSinkKind, LanguageId, LanguageStatus, MissingDiscriminatorFact,
        OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, Summary, SymbolId, ValueContext,
        ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn render_includes_summary_counts_and_empty_findings_message() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 8,
                exposed: 1,
                weakly_exposed: 2,
                reachable_unrevealed: 1,
                no_static_path: 1,
                static_unknown: 1,
                infection_unknown: 1,
                propagation_unknown: 1,
                ..Summary::default()
            },
            findings: vec![],
        };

        let rendered = render(&output);

        assert!(rendered.contains("mode: draft"));
        assert!(rendered.contains(
            "Summary: 8 probe(s), 1 exposed, 2 weak, 1 unrevealed, 1 no path, 3 unknown"
        ));
        assert!(rendered.contains("No diff-derived mutation exposure probes found."));
    }

    #[test]
    fn render_finding_includes_ripr_evidence_related_tests_gap_and_next_step() {
        let finding = sample_finding();
        let location = finding.probe.location.file.display().to_string();
        let related_path = finding.related_tests[0].file.display().to_string();

        let rendered = render_finding(&finding);

        assert!(rendered.contains(&format!("WARNING {location}:7")));
        assert!(rendered.contains("Changed\n"));
        assert!(rendered.contains("before: if enabled"));
        assert!(rendered.contains("after:  if disabled"));
        assert!(rendered.contains("Probe\n"));
        assert!(rendered.contains("family: predicate"));
        assert!(rendered.contains("Static exposure\n"));
        assert!(rendered.contains("weakly_exposed (warning, confidence 0.70)"));
        assert!(rendered.contains("Evidence\n"));
        assert!(rendered.contains("reach yes: reaches test"));
        assert!(rendered.contains("infection weak: weak mutation"));
        assert!(rendered.contains("propagation unknown: propagation unclear"));
        assert!(rendered.contains("observation yes: observed"));
        assert!(rendered.contains("discriminator no: no discriminator"));
        assert!(rendered.contains("local flow reaches returned value: disabled_result (line 8)"));
        assert!(rendered.contains(&format!(
            "{related_path}:22 test_handles_disabled uses strong exact value oracle: assert_eq!(actual, expected)"
        )));
        assert!(rendered.contains("observed function argument value enabled = false at line 22"));
        assert!(rendered.contains("Weakness\n"));
        assert!(rendered.contains("missing strong oracle"));
        assert!(rendered.contains(
            "missing discriminator enabled == false: related tests do not use the changed value"
        ));
        assert!(rendered.contains("Next step\n"));
        assert!(rendered.contains("Add assertion for disabled path result."));
    }

    #[test]
    fn render_finding_uses_expr_and_fallback_evidence_when_no_before_after() {
        let mut finding = sample_finding();
        finding.probe.before = None;
        finding.probe.after = None;
        finding.flow_sinks.clear();
        finding.related_tests.clear();
        finding.activation.observed_values.clear();
        finding.evidence = vec!["fallback evidence line".to_string()];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("expr:   enabled"));
        assert!(rendered.contains("  - fallback evidence line"));
    }

    #[test]
    fn render_finding_deduplicates_missing_discriminator_value_line() {
        let mut finding = sample_finding();
        finding.missing = vec![
            "Missing discriminator value: enabled == false".to_string(),
            "another gap".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert_eq!(
            rendered
                .matches("missing discriminator enabled == false")
                .count(),
            1
        );
        assert!(rendered.contains("  - another gap"));
    }

    #[test]
    fn render_finding_includes_language_metadata_when_present() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Language\n"));
        assert!(rendered.contains("  language: typescript\n"));
        assert!(rendered.contains("  status: preview\n"));
    }

    #[test]
    fn render_finding_includes_preview_actionability_without_raw_string_spam() {
        let mut finding = unknown_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(crate::domain::OwnerKind::Function);
        finding.evidence = vec![
            "owner: discountedTotal".to_string(),
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];
        finding.missing = vec![
            "TypeScript preview actionability `advisory` / `incomplete_repair_packet`: duplicate summary".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Preview actionability\n"));
        assert!(rendered.contains("  authority: preview_advisory_only\n"));
        assert!(rendered.contains("  gap state: advisory\n"));
        assert!(rendered.contains("  category: incomplete_repair_packet\n"));
        assert!(rendered.contains("  repair packet ready: false\n"));
        assert!(rendered.contains("  raw evidence: src/lib.ts:2 (typescript_preview_probe)"));
        assert!(rendered.contains("  - owner: discountedTotal\n"));
        assert!(!rendered.contains("  - gap_state: advisory\n"));
        assert!(!rendered.contains("duplicate summary"));
    }

    #[test]
    fn render_finding_includes_bun_cross_language_grip() {
        let mut finding = unknown_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(crate::domain::OwnerKind::Function);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript cross-language preview is advisory only".to_string(),
            "repair_route: inspect or add the suggested TypeScript witness".to_string(),
            "evidence_needed_to_promote: bridge calibration and non-preview repair packet contract"
                .to_string(),
            "typescript_bun_ub_bridge_hint: id=bun-blob-array-buffer rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts confidence=configured".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=add_resizable_array_buffer_blob_case suggested_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=add_resizable_array_buffer_blob_case authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable_array_buffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  Bun cross-language grip:\n"));
        assert!(rendered.contains("    state: rust_ungripped_ts_missing_discriminator\n"));
        assert!(rendered.contains(
            "    Rust seam: src/jsc/Blob.rs owner=Blob::from_js_without_defer_gc boundary=array_buffer.shared || array_buffer.resizable\n"
        ));
        assert!(rendered.contains(
            "    TypeScript evidence: test/js/web/fetch/blob.test.ts verdict=ts_missing_resizable confidence=configured\n"
        ));
        assert!(rendered.contains("    missing discriminators: resizable_array_buffer\n"));
        assert!(rendered.contains("    action: add_resizable_array_buffer_blob_case\n"));
        assert!(rendered.contains("    placement: rank 1 test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains("    placement reason: existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable_array_buffer\n"));
        assert!(rendered.contains("    authority: preview_advisory_only\n"));
        assert!(rendered.contains("    repair packet ready: false\n"));
    }

    #[test]
    fn render_finding_omits_language_metadata_when_absent() {
        let rendered = render_finding(&sample_finding());

        assert!(!rendered.contains("Language\n"));
        assert!(!rendered.contains("language:"));
        assert!(!rendered.contains("status:"));
    }

    #[test]
    fn render_finding_omits_rust_default_language_metadata() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::Rust);
        finding.language_status = Some(LanguageStatus::Stable);

        let rendered = render_finding(&finding);

        assert!(!rendered.contains("Language\n"));
        assert!(!rendered.contains("language: rust"));
        assert!(!rendered.contains("status: stable"));
    }

    #[test]
    fn render_finding_includes_probe_owner_when_present() {
        let mut finding = sample_finding();
        finding.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  owner:  python:src/pricing.py::discount\n"));
    }

    #[test]
    fn render_finding_includes_canonical_gap_when_present() {
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

        let rendered = render_finding(&finding);

        assert!(rendered.contains(
            "  canonical gap: gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold\n"
        ));
    }

    #[test]
    fn human_output_includes_effective_stop_reasons_for_unknowns() {
        let output = render_finding(&unknown_finding());

        assert!(output.contains("Stop reasons:"));
        assert!(output.contains("  - static_probe_unknown"));
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:sample.rs:7:predicate".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:sample.rs:7:predicate".to_string()),
                location: SourceLocation::new("src/sample.rs", 7, 3),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("if enabled".to_string()),
                after: Some("if disabled".to_string()),
                expression: "enabled".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes, Confidence::High, "reaches test"),
                infect: stage(StageState::Weak, Confidence::Medium, "weak mutation"),
                propagate: stage(StageState::Unknown, Confidence::Low, "propagation unclear"),
                reveal: RevealEvidence {
                    observe: stage(StageState::Yes, Confidence::High, "observed"),
                    discriminate: stage(StageState::No, Confidence::Medium, "no discriminator"),
                },
            },
            confidence: 0.7,
            evidence: vec![],
            missing: vec!["missing strong oracle".to_string()],
            flow_sinks: vec![FlowSinkFact {
                kind: FlowSinkKind::ReturnValue,
                text: "disabled_result".to_string(),
                line: 8,
                owner: None,
            }],
            activation: ActivationEvidence {
                observed_values: vec![ValueFact {
                    line: 22,
                    text: "sample(false)".to_string(),
                    value: "enabled = false".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "enabled == false".to_string(),
                    reason: "related tests do not use the changed value".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "test_handles_disabled".to_string(),
                file: PathBuf::from("tests/sample.rs"),
                line: 22,
                oracle: Some("assert_eq!(actual, expected)".to_string()),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            }],
            recommended_next_step: Some("Add assertion for disabled path result.".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
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
                reach: unknown_stage("No stable syntax owner"),
                infect: unknown_stage("Changed syntax is not mapped to a probe"),
                propagate: unknown_stage("No propagation model is available"),
                reveal: RevealEvidence {
                    observe: unknown_stage("No observation model is available"),
                    discriminate: unknown_stage("No discriminator model is available"),
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

    fn stage(state: StageState, confidence: Confidence, summary: &str) -> StageEvidence {
        StageEvidence::new(state, confidence, summary)
    }

    fn unknown_stage(summary: &str) -> StageEvidence {
        stage(StageState::Unknown, Confidence::Low, summary)
    }
}
