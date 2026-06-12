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
        // RIPR-SPEC-0083: disclose when no analysis scope was provided.
        // This fires only when the caller passed no --diff/--base/--mode, so
        // an empty result here means "nothing was analyzed", not "tests pass".
        if output.no_scope_provided {
            out.push_str(
                "\nNote: no analysis scope was provided — `ripr check` is diff-first. \
Run `ripr check --base origin/main` to analyze your changes, or \
`ripr check --root . --format repo-exposure-md` for a full-repo scan. \
An empty result here does NOT mean your changed behavior is covered.\n",
            );
        }
        render_preview_language_advisories(&mut out, output);
        return out;
    }

    for finding in &output.findings {
        out.push_str(&render_finding_with_config(finding, config));
        out.push('\n');
    }
    render_preview_language_advisories(&mut out, output);
    out
}

/// Emit preview-language advisory notes when preview-language files were
/// in the analyzed scope.
///
/// Called unconditionally; emits nothing when `preview_language_advisories`
/// is empty (pure-Rust scope). See RIPR-SPEC-0082.
///
/// Two wordings per the `enabled` flag:
///
/// - `enabled` — the preview adapter ran; the empty/partial result is advisory
///   and may be incomplete, not Rust-grade clean.
/// - not enabled — the files were detected but not analyzed because the
///   preview adapter is not enabled in `ripr.toml`; the empty result must not
///   be read as clean.
fn render_preview_language_advisories(out: &mut String, output: &CheckOutput) {
    for advisory in &output.preview_language_advisories {
        let language = capitalize_first(&advisory.language);
        if advisory.enabled {
            out.push_str(&format!(
                "\nNote: {} {}(s) analyzed under preview support — preview evidence is advisory and may be incomplete. An empty result here is NOT a clean Rust-grade result.\n",
                advisory.file_count, language,
            ));
        } else {
            out.push_str(&format!(
                "\nNote: this diff contains {} {}(s). The {} adapter is preview and not enabled, so these files were not analyzed — this is NOT a clean Rust-grade result. Enable it in ripr.toml [languages] to analyze them.\n",
                advisory.file_count, language, language,
            ));
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
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
    use crate::analysis::PreviewLanguageAdvisory;
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
            preview_language_advisories: Vec::new(),
            no_scope_provided: false,
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
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: TypeScript cross-language preview is a named limitation until the external oracle path is visible".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_graph_legs: boundary_discriminator:resizable_array_buffer".to_string(),
            "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists".to_string(),
            "evidence_needed_to_promote: bridge calibration and non-preview repair packet contract"
                .to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  Bun cross-language grip:\n"));
        assert!(rendered.contains("    state: rust_ungripped_ts_missing_discriminator\n"));
        assert!(rendered.contains(
            "    Rust seam: src/jsc/Blob.rs owner=Blob::from_js_without_defer_gc boundary=array_buffer.shared || array_buffer.resizable\n"
        ));
        assert!(rendered.contains(
            "    TypeScript evidence: test/js/web/fetch/blob.test.ts verdict=ts_missing_resizable confidence=configured_hint\n"
        ));
        assert!(rendered.contains("    missing discriminators: resizable_array_buffer\n"));
        assert!(
            rendered.contains(
                "    missing graph legs: boundary_discriminator:resizable_array_buffer\n"
            )
        );
        assert!(rendered.contains(
            "    unlock condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists\n"
        ));
        assert!(
            rendered
                .contains("    limitation category: cross_language_oracle_visibility_unresolved\n")
        );
        assert!(rendered.contains("    repair route: analysis/cross-language-oracle-visibility\n"));
        assert!(
            rendered.contains("    action: route_cross_language_oracle_visibility_limitation\n")
        );
        assert!(rendered.contains("    suggested test file: test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains("    placement: rank 1 test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains(
            "    placement reason: existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\n"
        ));
        assert!(rendered.contains("    proof mode: observable_red_green\n"));
        assert!(rendered.contains(
            "    proof mode reason: The missing TypeScript discriminator belongs in an existing bridged stable-byte observer route; future proof should be a system-Bun red/patched-green witness after the discriminator is added.\n"
        ));
        assert!(rendered.contains(
            "    proof execution: runtime=false mutation=false miri=false proof_claim=false\n"
        ));
        assert!(rendered.contains("    advisory packet:\n"));
        assert!(rendered.contains("      version: bun_cross_language_advisory_packet.v1\n"));
        assert!(
            rendered
                .contains("      next action: add_typescript_discriminator_in_suggested_file\n")
        );
        assert!(rendered.contains("      ts test file: test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains(
            "      suggested shape: Add new ArrayBuffer(..., { maxByteLength: ... }) through Blob/view with a stable-byte byte/text/value assertion.\n"
        ));
        assert!(rendered.contains(
            "      stop condition: Stop if placement evidence disappears or the stable-byte assertion requires production-code, public API, or test-framework changes.\n"
        ));
        assert!(rendered.contains(
            "      must not change: Rust production behavior, public API, test framework shape, generated tests, runtime Bun/TypeScript execution, public repair-packet authority\n"
        ));
        assert!(rendered.contains("      public repair packet: false\n"));
        assert!(rendered.contains("      repair packet ready: false\n"));
        assert!(rendered.contains("    authority: preview_advisory_only\n"));
        assert!(rendered.contains("    repair packet ready: false\n"));
    }

    #[test]
    fn render_finding_includes_perl_preview_card_as_advisory_human_surface() {
        let mut finding = unknown_finding();
        add_perl_preview_card_inputs(&mut finding);

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Perl preview card (advisory)\n"));
        assert!(rendered.contains("  card version: perl_preview_card.v1\n"));
        assert!(rendered.contains("  authority: preview_advisory_only (perl/preview)\n"));
        assert!(
            rendered
                .contains("  surface scope: check_json_human_sarif_github_gap_ledger_markdown\n")
        );
        assert!(rendered.contains("  public projection ready: true\n"));
        assert!(rendered.contains("  public repair packet: false\n"));
        assert!(rendered.contains("  repair packet ready: false\n"));
        assert!(rendered.contains("  agent packet ready: false\n"));
        assert!(rendered.contains("  gate candidate: false\n"));
        assert!(rendered.contains("  badge candidate: false\n"));
        assert!(rendered.contains("  RIPR Zero candidate: false\n"));
        assert!(rendered.contains("  packet id: perl-preview:gap-return\n"));
        assert!(rendered.contains(
            "  canonical gap: gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value\n"
        ));
        assert!(rendered.contains("  changed owner: perl:lib/My/App.pm::My::App::discount\n"));
        assert!(rendered.contains("  repair route: add_exact_return_assertion\n"));
        assert!(rendered.contains("  missing discriminator: return_value\n"));
        assert!(rendered.contains("  target test shape: Test::More exact_return_assertion\n"));
        assert!(rendered.contains("  suggested location: t/app.t::discount_smoke\n"));
        assert!(
            rendered.contains(
                "  suggested assertion: assert the exact returned `return_value` value\n"
            )
        );
        assert!(rendered.contains("  verify: prove t/app.t (fact_only_not_delegated)\n"));
        assert!(rendered.contains("  receipt: available_not_delegated\n"));
        assert!(rendered.contains("  raw evidence: perl_change lib/My/App.pm:8 (perl_change)"));
        assert!(rendered.contains("  stop if:\n"));
        assert!(rendered.contains("    - perl-lsp packet status changes\n"));
        assert!(rendered.contains("  must not change:\n"));
        assert!(rendered.contains("    - do not edit Perl production code\n"));
        assert!(!rendered.contains("ripr agent receipt --root"));
        assert!(!rendered.contains("perl_allowed_edit_boundary"));
        assert!(!rendered.contains("perl_forbidden_edit_boundary"));
        assert!(!rendered.contains("allowed edit"));
        assert!(!rendered.contains("forbidden edit"));
        assert!(!rendered.contains("perl_internal_agent_packet"));
        assert!(!rendered.contains("perl_repair_card"));
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
        finding.ripr = RiprEvidence {
            reach: stage(
                StageState::Yes,
                Confidence::Medium,
                "Perl fact packet links the related test to the changed owner",
            ),
            infect: stage(
                StageState::Yes,
                Confidence::Medium,
                "Changed return value reaches the owner result",
            ),
            propagate: stage(
                StageState::Yes,
                Confidence::Medium,
                "Return value can propagate to Test::More assertion",
            ),
            reveal: RevealEvidence {
                observe: stage(
                    StageState::Yes,
                    Confidence::Medium,
                    "Related test reaches the changed owner",
                ),
                discriminate: stage(
                    StageState::Weak,
                    Confidence::Medium,
                    "Exact return discriminator is missing",
                ),
            },
        };
        finding.confidence = 0.8;
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
        finding.missing = vec!["return_value".to_string()];
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

    // RIPR-SPEC-0082 tests: preview-language disclosure honesty
    #[test]
    fn render_emits_preview_disclosure_when_typescript_files_in_scope() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count: 2,
                sample_paths: vec!["src/discount.ts".to_string(), "src/pricing.ts".to_string()],
                enabled: true,
            }],
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("2 Typescript(s) analyzed under preview support"),
            "expected preview disclosure in output; got:\n{rendered}"
        );
        assert!(
            rendered.contains("preview evidence is advisory"),
            "expected advisory note; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_emits_preview_disclosure_when_python_files_in_scope() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "python".to_string(),
                file_count: 3,
                sample_paths: vec!["app/main.py".to_string()],
                enabled: true,
            }],
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("3 Python(s) analyzed under preview support"),
            "expected python preview disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_preview_disclosure_for_pure_rust_scope() {
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("preview support"),
            "pure-Rust scope must not emit preview note; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("NOT a clean Rust-grade result"),
            "pure-Rust scope must not emit honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_preview_disclosure_count_matches_advisory_file_count() {
        // The file_count in the advisory must appear verbatim in the disclosure line.
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count: 7,
                sample_paths: vec!["src/lib.ts".to_string()],
                enabled: true,
            }],
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("7 Typescript(s) analyzed under preview support"),
            "expected file_count=7 in disclosure; got:\n{rendered}"
        );
    }

    #[test]
    fn render_emits_not_enabled_disclosure_for_typescript_files_when_adapter_disabled() {
        // The #1111 default case: TypeScript files in the diff but the adapter
        // is NOT enabled. The empty result must be broken by a disclosure that
        // says the files were not analyzed.
        let output = CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count: 1,
                sample_paths: vec!["src/utils.ts".to_string()],
                enabled: false,
            }],
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("this diff contains 1 Typescript(s)"),
            "expected not-enabled disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("not enabled, so these files were not analyzed"),
            "expected not-analyzed wording; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
        assert!(
            rendered.contains("Enable it in ripr.toml"),
            "expected enable hint; got:\n{rendered}"
        );
        // Must NOT use the enabled wording.
        assert!(
            !rendered.contains("analyzed under preview support"),
            "not-enabled case must not claim analysis ran; got:\n{rendered}"
        );
    }

    #[test]
    fn render_finding_normalizes_backslash_location_path_to_forward_slash() {
        // Proves sections.rs uses display_path: Windows-style .\\src\\pricing.ts
        // must render as src/pricing.ts in the WARNING line.
        let mut finding = sample_finding();
        finding.probe.location = SourceLocation::new(PathBuf::from(r"src\pricing.ts"), 10, 1);

        let rendered = render_finding(&finding);

        assert!(
            rendered.contains("src/pricing.ts:10"),
            "expected forward-slash location path in human output; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r"src\pricing.ts"),
            "backslash path must not appear in human output; got:\n{rendered}"
        );
    }

    #[test]
    fn render_finding_normalizes_backslash_related_test_path_to_forward_slash() {
        // Proves evidence_lines.rs uses display_path: related test file with
        // backslashes must appear as forward-slash in the evidence lines.
        let mut finding = sample_finding();
        finding.related_tests[0].file = PathBuf::from(r"tests\sample.rs");

        let rendered = render_finding(&finding);

        assert!(
            rendered.contains("tests/sample.rs:"),
            "expected forward-slash related-test path in human evidence; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r"tests\sample.rs"),
            "backslash related-test path must not appear in human output; got:\n{rendered}"
        );
    }

    // RIPR-SPEC-0083 tests: no-scope disclosure honesty

    #[test]
    fn render_emits_no_scope_guidance_when_no_scope_provided_and_empty() {
        // The cardinal case: bare `ripr check` produces an empty result.
        // `no_scope_provided: true` must emit the guidance note.
        let output = CheckOutput {
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            no_scope_provided: true,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("no analysis scope was provided"),
            "expected no-scope guidance; got:\n{rendered}"
        );
        assert!(
            rendered.contains("`ripr check --base origin/main`"),
            "expected --base guidance; got:\n{rendered}"
        );
        assert!(
            rendered.contains("does NOT mean your changed behavior is covered"),
            "expected honesty note; got:\n{rendered}"
        );
        // Bug 2 regression guard: the recommended full-repo-scan command must be
        // --format repo-exposure-md, not --mode fast (which is a speed tier).
        assert!(
            rendered.contains("--format repo-exposure-md"),
            "expected --format repo-exposure-md in guidance; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("--mode fast"),
            "guidance must NOT recommend --mode fast as a full-repo scan; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_no_scope_guidance_when_scope_provided_and_empty() {
        // Scope was provided (--diff/--base) but found 0 probes.
        // `no_scope_provided: false` must NOT emit the guidance — the result
        // is honest: that diff really had no probes.
        let output = CheckOutput {
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            no_scope_provided: false,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("no analysis scope was provided"),
            "scope-provided empty result must NOT show no-scope guidance; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("does NOT mean your changed behavior is covered"),
            "scope-provided empty result must NOT show honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_no_scope_guidance_uses_conservative_static_language() {
        // Verify the no-scope disclosure text uses only approved static-language
        // vocabulary. The gate bans mutation-testing runtime terms; we verify
        // the disclosure uses the approved phrasing ("does NOT mean your changed
        // behavior is covered") rather than any runtime claim.
        let output = CheckOutput {
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            no_scope_provided: true,
        };

        let rendered = render(&output);

        // Confirm the actual approved honesty phrase is present.
        assert!(
            rendered.contains("does NOT mean your changed behavior is covered"),
            "expected approved honesty phrase; got:\n{rendered}"
        );
        // The disclosure is a static analysis advisory, not a runtime claim.
        assert!(
            rendered.contains("no analysis scope was provided"),
            "expected scope disclosure lead-in; got:\n{rendered}"
        );
    }

    #[test]
    fn guidance_recommends_format_repo_exposure_md_not_mode_fast() {
        // Bug 2 regression guard: the human guidance string must recommend
        // --format repo-exposure-md for a full-repo scan, NOT --mode fast.
        // --mode is a speed tier on the diff path; it does NOT provide scope.
        let output = CheckOutput {
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            no_scope_provided: true,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("--format repo-exposure-md"),
            "guidance must recommend --format repo-exposure-md for full-repo scan; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("--mode fast"),
            "guidance must NOT recommend --mode fast as a full-repo-scan command; got:\n{rendered}"
        );
    }
}
