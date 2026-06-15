//! TypeScript preview → complete repair-packet projection (RIPR-SPEC-0087 §PR7).
//!
//! This module implements the **projection** side of the
//! TypeScript-actionability gate: it reads evidence lines from a
//! `preview`-language finding and — when every precondition in §1.2 holds —
//! builds a `GapRecord` that can be fed to the **shared** Rust validator
//! `validate_agent_gap_record_packet` to determine `repair_packet_ready`.
//!
//! Architectural constraints (non-negotiable):
//! - The validator lives in `output::agent_seam_packets`; this module may call
//!   it because both modules are inside `output/`.
//! - Analysis modules (`analysis/**`) must NOT import `crate::output`, so the
//!   projection CANNOT live there.
//! - No parallel TypeScript-specific completeness validator is introduced.
//!   The only flip gate is `validate_agent_gap_record_packet(..) == Ok(())`.

use crate::domain::Finding;
use crate::output::gap_decision_ledger::{
    GapAnchor, GapRecord, GapRepairRoute, ProjectionEligibility,
};
use std::collections::BTreeMap;

/// The authority-boundary string carried by all TypeScript preview packets.
const TS_AUTHORITY_BOUNDARY: &str = "preview_advisory_only";

/// Project a TypeScript preview finding into a `GapRecord` for validator
/// consumption, applying all §1.2 preconditions (G-A through G-F).
///
/// Returns `None` whenever ANY precondition fails — the false case requires
/// no positive proof. Only `Ok(())` from the shared validator leads to a flip.
///
/// # Preconditions checked (fail-closed)
/// - G-A: `actionability_category == "incomplete_repair_packet"`
/// - G-C: non-dynamic oracle with `expected_value_or_variant` present
///   (`typescript_oracle_expected` evidence + no `typescript_limitation:
///   typescript_dynamic_assertion_unresolved`)
/// - G-D: oracle-eligible relation (not `ambiguous_related_test`; verified by G-A)
/// - G-E: non-empty `missing_discriminators` list (a target shape exists)
/// - G-F: no cross-language bridge limitation evidence present
pub(crate) fn typescript_gap_record_for(finding: &Finding) -> Option<GapRecord> {
    // G-A: only the terminal `incomplete_repair_packet` branch is eligible.
    let category = evidence_value(finding, "actionability_category: ")?;
    if category != "incomplete_repair_packet" {
        return None;
    }

    // G-C: non-dynamic oracle — `typescript_oracle_expected` must be present
    // AND no dynamic-assertion limitation may be present.
    let oracle_expected = evidence_value(finding, "typescript_oracle_expected: ")?;
    if oracle_expected.is_empty() {
        return None;
    }
    // Fail-closed: if any dynamic-assertion limitation was emitted, stay preview.
    if finding
        .evidence
        .iter()
        .any(|line| line == "typescript_limitation: typescript_dynamic_assertion_unresolved")
    {
        return None;
    }

    // G-D: oracle-eligible relation is guaranteed by G-A (ambiguous_related_test
    // is a different category and is excluded above). Verify also that there is
    // actually a related test with an oracle-eligible file we can use.
    let related_test = finding
        .related_tests
        .iter()
        .max_by_key(|t| t.oracle_strength.rank())?;
    let test_file = related_test.file.display().to_string().replace('\\', "/");
    if test_file.is_empty() {
        return None;
    }

    // G-E: the finding must have at least one named missing discriminator
    // (the `missing_target_shape` guard in actionability.rs already passed,
    // but we double-check here so the projection is self-contained).
    if finding.activation.missing_discriminators.is_empty() {
        return None;
    }

    // G-F: no unresolved cross-language oracle visibility limitation.
    if finding.evidence.iter().any(|line| {
        line.starts_with("route_cross_language_oracle_visibility_limitation:")
            || line.starts_with("typescript_bun_bridge_verdict:")
            || line
                .starts_with("typescript_limitation: typescript_cross_language_bridge_unresolved")
    }) {
        return None;
    }

    // Derive `canonical_gap_id` from the finding id (§3.2).
    // The finding id is already content-addressed with path normalized \→/ in
    // `fingerprint_probe_id` — we reuse the last two segments (family:fp8)
    // and prepend `gap:typescript:`.
    // F13: normalize before hashing (the finding id already has / separators,
    // but we defensively strip any remaining backslashes from the id string).
    let canonical_gap_id = typescript_canonical_gap_id(&finding.id);

    // Build verify command from evidence (§3.1 — existing producer).
    let verify_command = evidence_value(finding, "typescript_verify_command: ")?;
    if verify_command.is_empty() {
        return None;
    }

    // Build receipt command (§3.2 new producer F6/F7):
    // A fixed `ripr outcome … target/ripr/receipts/<canonical_gap_id>.targeted-test-outcome.json`
    // command — no external provider, no interpolation of free text.
    let receipt_command = typescript_receipt_command(&canonical_gap_id, verify_command);

    // Build repair_route from the finding (§3.1 — test file from related test).
    // The route_kind is derived from the probe family / missing discriminator.
    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()
        .map(|d| d.value.clone());
    let route_kind = typescript_route_kind_for(&finding.probe.family);
    let assertion_shape = evidence_value(finding, "typescript_oracle_observed: ")
        .map(|observed| format!("expect({observed}).toBe({})", oracle_expected));

    let repair_route = GapRepairRoute {
        route_kind: route_kind.to_string(),
        target_file: Some(test_file.clone()),
        related_test: Some(format!("{test_file}::{}", related_test.name)),
        assertion_shape,
        missing_discriminator,
        changed_behavior: None,
        target_line: if related_test.line > 0 {
            Some(related_test.line as u64)
        } else {
            None
        },
        stop_conditions: vec![
            "Stop if the gap record is no longer present or loses agent-packet eligibility."
                .to_string(),
            "Stop if the verification command cannot run from this workspace.".to_string(),
        ],
    };

    // Anchor: probe location + owner from the finding.
    let probe_file = finding
        .probe
        .location
        .file
        .display()
        .to_string()
        .replace('\\', "/");
    let probe_line = finding.probe.location.line as u64;
    let owner_name = finding
        .probe
        .owner
        .as_ref()
        .and_then(|sym| sym.0.rsplit("::").next())
        .or_else(|| evidence_value(finding, "owner: "))
        .map(ToString::to_string);
    let anchor = GapAnchor {
        file: Some(probe_file),
        line: Some(probe_line),
        owner: owner_name,
        dedupe_fingerprint: Some(finding.id.clone()),
    };

    // Projection eligibility: mark agent_packet eligible (validator cond. 1).
    let mut projection_eligibility = BTreeMap::new();
    projection_eligibility.insert(
        "agent_packet".to_string(),
        ProjectionEligibility {
            eligible: true,
            reason: "TypeScript preview complete-contract projection (RIPR-SPEC-0087)".to_string(),
        },
    );

    // Evidence IDs: the finding's own id.
    let evidence_ids = vec![finding.id.clone()];

    Some(GapRecord {
        gap_id: finding.id.clone(),
        canonical_gap_id,
        kind: "typescript_preview_boundary".to_string(),
        language: "typescript".to_string(),
        language_status: "preview".to_string(),
        scope: "diff".to_string(),
        evidence_class: "weakly_exposed".to_string(),
        gap_state: "advisory".to_string(),
        policy_state: "preview".to_string(),
        repairability: "repairable".to_string(),
        repair_route: Some(repair_route),
        static_limit_kind: None,
        static_limit_detail: None,
        static_limits: Vec::new(),
        anchor: Some(anchor),
        evidence_ids,
        projection_eligibility,
        verification_commands: vec![verify_command.to_string()],
        receipt_command: Some(receipt_command),
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: TS_AUTHORITY_BOUNDARY.to_string(),
    })
}

/// Derive the content-addressed `gap:typescript:<probe_family>:<fp8>` canonical
/// gap id from the finding's existing content-addressed id (§3.2 / F13).
///
/// The finding id format is `probe:<path>:<family>:<fp8>`. We extract the
/// `<family>` and `<fp8>` segments and build `gap:typescript:<family>:<fp8>`.
/// This avoids introducing a new hash domain and reuses the existing SHA-256 fp8.
pub(crate) fn typescript_canonical_gap_id(finding_id: &str) -> String {
    // finding_id: "probe:src_discount.ts:typescript_preview:1a2b3c4d"
    // Normalize backslashes (defensive, finding ids should already use /).
    let normalized = finding_id.replace('\\', "/");
    // Split on `:` and pick the last two segments as family:fp8.
    let segments: Vec<&str> = normalized.splitn(4, ':').collect();
    if segments.len() == 4 {
        // segments[0]=probe, [1]=path, [2]=family, [3]=fp8
        let family = segments[2];
        let fp8 = segments[3];
        format!("gap:typescript:{family}:{fp8}")
    } else {
        // Fallback: use the whole normalized id as a slug.
        format!("gap:typescript:{normalized}")
    }
}

/// Derive the receipt command for a TypeScript preview finding (§3.2 / F6/F7).
///
/// The command is a fixed `ripr outcome …` invocation that mirrors the Rust
/// receipt shape without any external provider call, curl, or http request.
pub(crate) fn typescript_receipt_command(canonical_gap_id: &str, verify_command: &str) -> String {
    // F7: fixed `ripr outcome` shape only — no external provider or curl.
    // The receipt path uses the canonical_gap_id as a slug (slashes replaced
    // with underscores so the path is a single filename component).
    let slug = canonical_gap_id
        .chars()
        .map(|c| if c == ':' || c == '/' { '_' } else { c })
        .collect::<String>();
    let receipt_path = format!("target/ripr/receipts/{slug}.targeted-test-outcome.json");
    format!(
        "ripr outcome --before <baseline> --after <repair> --verify-cmd \"{verify_command}\" --out {receipt_path}"
    )
}

/// Map the probe family to a `GapRepairRoute` route_kind (§3.2).
///
/// Routes stay consistent with the Rust taxonomy — no new TS-only route kind.
fn typescript_route_kind_for(family: &crate::domain::ProbeFamily) -> &'static str {
    use crate::domain::ProbeFamily;
    match family {
        ProbeFamily::Predicate => "AddBoundaryAssertion",
        ProbeFamily::ReturnValue => "AddValueAssertion",
        ProbeFamily::ErrorPath => "AddErrorDiscriminator",
        ProbeFamily::FieldConstruction => "AddValueAssertion",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "AddBoundaryAssertion",
        ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => "AddBoundaryAssertion",
    }
}

/// Extract a value from the finding's evidence vec by prefix.
fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────────────────────────────
    // §7.4 Validator-parity test: proves the flip is driven by the shared
    // validator and not a parallel TypeScript-specific path.
    // ──────────────────────────────────────────────────────────────────────

    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, SymbolId,
    };
    use crate::output::agent_seam_packets::{
        gap_record_packet_do_not_do, validate_agent_gap_record_packet,
    };
    use std::path::PathBuf;

    fn complete_finding() -> Finding {
        Finding {
            id: "probe:src_discount.ts:typescript_preview:a1b2c3d4".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_discount.ts:typescript_preview:a1b2c3d4".to_string()),
                location: SourceLocation::new("src/discount.ts", 3, 1),
                owner: Some(SymbolId(
                    "typescript:src/discount.ts::applyDiscount".to_string(),
                )),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("  if (amount >= threshold) {".to_string()),
                expression: "  if (amount >= threshold) {".to_string(),
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
                "raw_evidence_ref: leg=rust_seam;file=src/discount.ts;line=3;kind=typescript_preview_probe;source_id=probe:src_discount.ts:typescript_preview:a1b2c3d4;owner=applyDiscount".to_string(),
                "typescript_package_root: .".to_string(),
                "typescript_workspace_root: .".to_string(),
                "typescript_framework_hint: jest".to_string(),
                "typescript_runner_hint: npm".to_string(),
                "typescript_package_confidence: high".to_string(),
                "typescript_verify_command: jest tests/discount.test.ts".to_string(),
                "typescript_oracle_observed: applyDiscount(100, 100)".to_string(),
                "typescript_oracle_expected: 90".to_string(),
                "typescript_oracle_confidence: high".to_string(),
                "typescript_oracle_evidence_ref: tests/discount.test.ts:5".to_string(),
                "missing_discriminator: amount >= threshold".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount >= threshold".to_string(),
                    reason: "changed TypeScript equality-boundary at line 3 lacks a concrete preview discriminator".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "applyDiscount applies discount".to_string(),
                file: PathBuf::from("tests/discount.test.ts"),
                line: 4,
                oracle_strength: OracleStrength::Weak,
                oracle_kind: OracleKind::ExactValue,
                oracle: Some("expect(...).toBe(...)".to_string()),
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: None,
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

    /// §7.4 Validator-parity test (architectural assertion):
    /// The GapRecord produced by `typescript_gap_record_for` for the complete
    /// fixture MUST pass `validate_agent_gap_record_packet`. Any future fork
    /// of the logic would break this test.
    #[test]
    fn validator_parity_complete_finding_passes_shared_validator() {
        let finding = complete_finding();
        let maybe_record = typescript_gap_record_for(&finding);
        assert!(
            maybe_record.is_some(),
            "complete finding must produce a GapRecord"
        );
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let result = validate_agent_gap_record_packet(&record);
        assert!(
            result.is_ok(),
            "shared validator must accept complete TS GapRecord, got: {:?}",
            result
        );
    }

    /// §7.4: Missing verify command → validator fails (cond. 3), stays preview.
    #[test]
    fn validator_parity_missing_verify_command_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("typescript_verify_command:"));
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing verify command must return None (stay preview)"
        );
    }

    /// §7.4: Missing oracle expected value → G-C fails, returns None.
    #[test]
    fn validator_parity_missing_oracle_expected_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("typescript_oracle_expected:"));
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing oracle expected value must return None (G-C)"
        );
    }

    /// §7.4: Dynamic oracle limitation → G-C fails, returns None.
    #[test]
    fn validator_parity_dynamic_oracle_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .push("typescript_limitation: typescript_dynamic_assertion_unresolved".to_string());
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "dynamic oracle limitation must return None (G-C)"
        );
    }

    /// §7.4: Wrong category → G-A fails, returns None.
    #[test]
    fn validator_parity_wrong_category_returns_none() {
        let mut finding = complete_finding();
        for line in finding.evidence.iter_mut() {
            if line.starts_with("actionability_category: ") {
                *line = "actionability_category: strong_oracle_observed".to_string();
            }
        }
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "non-incomplete-repair-packet category must return None (G-A)"
        );
    }

    /// §7.4: No related tests → projection fails, returns None.
    #[test]
    fn validator_parity_no_related_tests_returns_none() {
        let mut finding = complete_finding();
        finding.related_tests.clear();
        let record = typescript_gap_record_for(&finding);
        assert!(record.is_none(), "no related tests must return None (G-D)");
    }

    /// §7.4: Empty missing_discriminators → G-E fails, returns None.
    #[test]
    fn validator_parity_no_missing_discriminators_returns_none() {
        let mut finding = complete_finding();
        finding.activation.missing_discriminators.clear();
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "empty missing_discriminators must return None (G-E)"
        );
    }

    /// §7.4: Cross-language bridge limitation → G-F fails, returns None.
    #[test]
    fn validator_parity_cross_language_bridge_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .push("typescript_limitation: typescript_cross_language_bridge_unresolved".to_string());
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "cross-language bridge limitation must return None (G-F)"
        );
    }

    #[test]
    fn canonical_gap_id_derives_from_finding_id() {
        let id = "probe:src_discount.ts:typescript_preview:a1b2c3d4";
        let gap_id = typescript_canonical_gap_id(id);
        assert_eq!(gap_id, "gap:typescript:typescript_preview:a1b2c3d4");
    }

    #[test]
    fn canonical_gap_id_normalizes_backslashes() {
        let id = r"probe:src\discount.ts:typescript_preview:a1b2c3d4";
        let gap_id = typescript_canonical_gap_id(id);
        // After backslash normalization, result still starts with gap:typescript:
        assert!(gap_id.starts_with("gap:typescript:"));
    }

    #[test]
    fn receipt_command_is_ripr_outcome_shape() {
        let cmd = typescript_receipt_command(
            "gap:typescript:typescript_preview:a1b2c3d4",
            "jest tests/discount.test.ts",
        );
        assert!(
            cmd.starts_with("ripr outcome "),
            "must start with ripr outcome"
        );
        assert!(
            cmd.contains("target/ripr/receipts/"),
            "must reference receipts path"
        );
        assert!(!cmd.contains("curl"), "F7: must not contain curl");
        assert!(!cmd.contains("http"), "F7: must not contain http");
    }

    /// §3.2 / F14: The shared `gap_record_packet_do_not_do` function must include
    /// the preview-language clause when `language_status == "preview"`.
    /// This proves the TS packet reuses the shared boundary list (not a fork).
    #[test]
    fn must_not_change_includes_preview_clause_via_shared_function() {
        let finding = complete_finding();
        let maybe_record = typescript_gap_record_for(&finding);
        assert!(
            maybe_record.is_some(),
            "complete finding must produce a GapRecord for do_not_do test"
        );
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let do_not_do = gap_record_packet_do_not_do(&record);
        let has_preview_clause = do_not_do
            .iter()
            .any(|s| s.contains("preview-language evidence"));
        assert!(
            has_preview_clause,
            "F14: gap_record_packet_do_not_do must include preview clause for language_status=preview; got: {do_not_do:?}"
        );
    }
}
