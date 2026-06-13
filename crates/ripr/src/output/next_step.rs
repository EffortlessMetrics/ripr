/// Shared reconciliation logic for the `recommended_next_step` /
/// `suggested_next_action` field across all output surfaces.
///
/// The `recommended_next_step` string is produced in the analysis layer, which
/// cannot see the output-layer validator flip (`repair_packet_ready`). When the
/// validator determines the packet IS complete, the raw string still contains
/// the blocked-case tail "no actionable repair packet is emitted until …".
/// Every renderer (human, JSON, SARIF) MUST call `reconcile_next_step` so that
/// all machine surfaces agree — none can serialize the raw field directly.
///
/// RIPR-SPEC-0088 §PR8: the correction happens at render time in the output
/// layer. The analysis layer stays clean.
use crate::domain::Finding;
use crate::output::preview_actionability::preview_actionability_for;

/// Return the reconciled next-step string for `finding`.
///
/// For a finding with `repair_packet_ready: true` the blocked-case tail
/// "; no actionable repair packet is emitted until …" is stripped and replaced
/// with "; the repair packet is complete and delegatable (advisory)."
///
/// For all other findings (incomplete packets or non-TypeScript/JS) the raw
/// `recommended_next_step` value is returned unchanged, preserving the
/// blocked-state disclosure on the output surfaces.
///
/// Returns an empty string when `finding.recommended_next_step` is `None`.
pub(crate) fn reconcile_next_step(finding: &Finding) -> String {
    let step = match finding.recommended_next_step.as_deref() {
        Some(s) => s,
        None => return String::new(),
    };

    let actionable = preview_actionability_for(finding)
        .map(|a| a.repair_packet_ready)
        .unwrap_or(false);

    if !actionable {
        // Blocked or non-preview: surface the raw string unmodified so the
        // real blocked-state disclosure is preserved on all three surfaces.
        return step.to_string();
    }

    // Strip the contradictory "; no actionable repair packet is emitted until
    // …" tail and replace it with an actionable confirmation.
    let head = step
        .split("; no actionable repair packet is emitted until")
        .next()
        .unwrap_or(step)
        .trim_end_matches(['.', ' '])
        .to_string();
    format!("{head}; the repair packet is complete and delegatable (advisory).")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CheckOutput, Mode};
    use crate::config::RiprConfig;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, Summary, SymbolId,
    };
    use crate::output::human::render_with_config as human_render_with_config;
    use crate::output::json::render_with_config as json_render_with_config;
    use crate::output::sarif::render_findings_sarif;
    use crate::output::suppressions::SuppressionEntry;
    use std::path::PathBuf;

    // ── shared test fixture ────────────────────────────────────────────────────

    fn complete_ts_finding() -> Finding {
        Finding {
            id: "probe:src_discount.ts:typescript_preview:2396aec1".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_discount.ts:typescript_preview:2396aec1".to_string()),
                location: SourceLocation::new("src/discount.ts", 2, 1),
                owner: Some(SymbolId(
                    "typescript:src/discount.ts::applyDiscount".to_string(),
                )),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if (amount >= threshold) {".to_string()),
                expression: "if (amount >= threshold) {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::Low, "1 related test"),
                infect: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model infection.",
                ),
                propagate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model propagation.",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "weak oracle"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Low,
                        "weak discriminator",
                    ),
                },
            },
            confidence: 0.4,
            evidence: vec![
                "owner: applyDiscount".to_string(),
                "gap_state: advisory".to_string(),
                "actionability_category: incomplete_repair_packet".to_string(),
                "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract".to_string(),
                "repair_route: project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available".to_string(),
                "evidence_needed_to_promote: canonical gap identity, repair kind, target test shape, related observer, verify command, receipt command, raw evidence refs, and edit constraints".to_string(),
                "raw_evidence_ref: leg=rust_seam;file=src/discount.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_discount.ts:typescript_preview:2396aec1;owner=applyDiscount".to_string(),
                "typescript_package_root: .".to_string(),
                "typescript_workspace_root: .".to_string(),
                "typescript_framework_hint: jest".to_string(),
                "typescript_runner_hint: npm".to_string(),
                "typescript_package_confidence: high".to_string(),
                "typescript_verify_command: jest tests/discount.test.ts".to_string(),
                "typescript_oracle_observed: applyDiscount(100, 100)".to_string(),
                "typescript_oracle_expected: 50".to_string(),
                "typescript_oracle_confidence: high".to_string(),
                "typescript_oracle_evidence_ref: tests/discount.test.ts:3".to_string(),
                "missing_discriminator: amount == threshold".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount == threshold".to_string(),
                    reason:
                        "changed TypeScript equality-boundary at line 2 lacks a concrete preview discriminator"
                            .to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "applyDiscount applies discount when amount meets threshold".to_string(),
                file: PathBuf::from("tests/discount.test.ts"),
                line: 3,
                oracle_strength: OracleStrength::Weak,
                oracle_kind: OracleKind::RelationalCheck,
                oracle: Some("expect(result).toBeGreaterThan(50)".to_string()),
            }],
            recommended_next_step: Some(
                "TypeScript preview advisory: add or strengthen a focused assertion for missing discriminator `amount == threshold`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available.".to_string(),
            ),
            language: Some(LanguageId::TypeScript),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: Some(OwnerKind::Function),
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    fn incomplete_ts_finding() -> Finding {
        let mut f = complete_ts_finding();
        // Remove the typescript_verify_command so the packet is incomplete
        // (the validator rejects it — this is the blocked case).
        f.evidence
            .retain(|l| !l.starts_with("typescript_verify_command:"));
        f.recommended_next_step = Some(
            "TypeScript preview advisory: add or strengthen a focused assertion; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available.".to_string(),
        );
        f
    }

    fn check_output_for(finding: Finding) -> CheckOutput {
        CheckOutput {
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Fast,
            root: PathBuf::from("."),
            base: None,
            summary: Summary::default(),
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            no_scope_provided: false,
        }
    }

    // ── unit: reconcile_next_step ──────────────────────────────────────────────

    /// Complete packet → contradictory tail replaced on ALL three surfaces.
    #[test]
    fn reconcile_next_step_complete_packet_strips_tail() {
        let finding = complete_ts_finding();
        let step = reconcile_next_step(&finding);
        assert!(
            step.contains("the repair packet is complete and delegatable (advisory)"),
            "complete packet must produce actionable step; got: {step}"
        );
        assert!(
            !step.contains("no actionable repair packet is emitted"),
            "complete packet must NOT contain blocked-case tail; got: {step}"
        );
    }

    /// Incomplete packet → blocked-state string preserved.
    #[test]
    fn reconcile_next_step_blocked_packet_preserves_disclosure() {
        let finding = incomplete_ts_finding();
        let step = reconcile_next_step(&finding);
        assert!(
            step.contains("no actionable repair packet is emitted"),
            "blocked packet must preserve blocked-state disclosure; got: {step}"
        );
        assert!(
            !step.contains("the repair packet is complete and delegatable"),
            "blocked packet must NOT say actionable; got: {step}"
        );
    }

    #[test]
    fn reconcile_next_step_none_returns_empty() {
        let mut finding = complete_ts_finding();
        finding.recommended_next_step = None;
        assert_eq!(reconcile_next_step(&finding), "");
    }

    // ── parity: all three surfaces agree ──────────────────────────────────────

    /// PARITY TEST (RIPR-SPEC-0088 §PR8): for a COMPLETE packet, human, JSON,
    /// and SARIF must all produce the SAME reconciled (actionable) next-step
    /// string — none may serialize the raw blocked-case field directly.
    ///
    /// This is the architectural enforcement test: if any renderer forks the
    /// reconciliation logic or serializes the raw field, this test fails.
    #[test]
    fn next_step_parity_complete_packet_all_surfaces_agree() {
        let finding = complete_ts_finding();
        let expected = reconcile_next_step(&finding);
        assert!(
            expected.contains("the repair packet is complete and delegatable (advisory)"),
            "precondition: reconcile_next_step must produce actionable string; got: {expected}"
        );

        let output = check_output_for(finding);
        let config = RiprConfig::default();
        let no_suppressions: Vec<SuppressionEntry> = Vec::new();

        // Human surface
        let human = human_render_with_config(&output, &config);
        assert!(
            human.contains("the repair packet is complete and delegatable (advisory)"),
            "human surface must contain reconciled next-step.\nExpected: {expected}\nHuman output: {human}"
        );
        assert!(
            !human.contains("no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available"),
            "human surface must NOT contain blocked-case tail when packet is complete.\nHuman output: {human}"
        );

        // JSON surface
        let json_out = json_render_with_config(&output, &config);
        assert!(
            json_out.contains("the repair packet is complete and delegatable (advisory)"),
            "JSON surface must contain reconciled next-step.\nExpected: {expected}\nJSON output: {json_out}"
        );
        assert!(
            !json_out.contains("no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available"),
            "JSON surface must NOT contain blocked-case tail when packet is complete.\nJSON output: {json_out}"
        );

        // SARIF surface — parse the JSON and check suggested_next_action
        let sarif_out = render_findings_sarif(&output, &config, &no_suppressions);
        let sarif_parse = serde_json::from_str::<serde_json::Value>(&sarif_out);
        assert!(
            sarif_parse.is_ok(),
            "SARIF output must be valid JSON; got: {sarif_out}"
        );
        let sarif_value = sarif_parse.unwrap_or(serde_json::Value::Null);
        let sarif_result = &sarif_value["runs"][0]["results"][0];
        let sarif_action = sarif_result["properties"]["suggested_next_action"]
            .as_str()
            .unwrap_or("");
        assert_eq!(
            sarif_action, expected,
            "SARIF suggested_next_action must equal reconciled next-step.\nGot: {sarif_action}\nExpected: {expected}"
        );
        assert!(
            !sarif_action.contains("no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available"),
            "SARIF must NOT contain blocked-case tail when packet is complete; got: {sarif_action}"
        );

        // SARIF message.text is ALSO a machine-consumed surface: it embeds the
        // "Next step". It must carry the reconciled tail and NOT the blocked-case
        // string when the packet is complete (the gap that the first fix missed).
        let sarif_message = sarif_result["message"]["text"].as_str().unwrap_or("");
        assert!(
            sarif_message.contains("the repair packet is complete and delegatable (advisory)"),
            "SARIF message.text must contain reconciled next-step.\nExpected tail: {expected}\nMessage: {sarif_message}"
        );
        assert!(
            !sarif_message.contains("no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available"),
            "SARIF message.text must NOT contain blocked-case tail when packet is complete; got: {sarif_message}"
        );

        // Whole-document guard: ZERO occurrences of the blocked-case string
        // anywhere in the rendered SARIF for a complete packet.
        assert_eq!(
            sarif_out
                .matches("no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available")
                .count(),
            0,
            "complete packet must produce ZERO blocked-case strings anywhere in SARIF.\nSARIF: {sarif_out}"
        );
    }

    /// PARITY TEST: for an INCOMPLETE (blocked) packet, human, JSON, and SARIF
    /// must ALL preserve the blocked-state disclosure — the fix must not silence
    /// real blocked-state information on any surface.
    #[test]
    fn next_step_parity_blocked_packet_all_surfaces_disclose() {
        let finding = incomplete_ts_finding();

        let output = check_output_for(finding);
        let config = RiprConfig::default();
        let no_suppressions: Vec<SuppressionEntry> = Vec::new();

        // Human surface
        let human = human_render_with_config(&output, &config);
        assert!(
            human.contains("no actionable repair packet is emitted"),
            "human surface must preserve blocked-case disclosure for incomplete packet.\nHuman output: {human}"
        );

        // JSON surface
        let json_out = json_render_with_config(&output, &config);
        assert!(
            json_out.contains("no actionable repair packet is emitted"),
            "JSON surface must preserve blocked-case disclosure for incomplete packet.\nJSON output: {json_out}"
        );

        // SARIF surface
        let sarif_out = render_findings_sarif(&output, &config, &no_suppressions);
        let sarif_parse = serde_json::from_str::<serde_json::Value>(&sarif_out);
        assert!(
            sarif_parse.is_ok(),
            "SARIF output must be valid JSON; got: {sarif_out}"
        );
        let sarif_value = sarif_parse.unwrap_or(serde_json::Value::Null);
        let sarif_result = &sarif_value["runs"][0]["results"][0];
        let sarif_action = sarif_result["properties"]["suggested_next_action"]
            .as_str()
            .unwrap_or("");
        assert!(
            sarif_action.contains("no actionable repair packet is emitted"),
            "SARIF must preserve blocked-case disclosure for incomplete packet; got: {sarif_action}"
        );

        // SARIF message.text must ALSO preserve the blocked-state disclosure.
        let sarif_message = sarif_result["message"]["text"].as_str().unwrap_or("");
        assert!(
            sarif_message.contains("no actionable repair packet is emitted"),
            "SARIF message.text must preserve blocked-case disclosure for incomplete packet; got: {sarif_message}"
        );
    }
}
