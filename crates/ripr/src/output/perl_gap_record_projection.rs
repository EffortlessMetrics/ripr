//! Perl preview → complete repair-packet projection (RIPR-SPEC-0064, ADR 0019).
//!
//! This module implements the **projection** side of the Perl-actionability
//! gate mandated by [ADR 0019](../../docs/adr/0019-language-adapters-reuse-shared-packet-contract.md):
//! it reads `perl_*:` evidence lines from a `preview`-language Perl finding
//! and — when every fail-closed precondition holds — builds a `GapRecord`
//! that can be fed to the **shared** Rust validator
//! `validate_agent_gap_record_packet` to determine `repair_packet_ready`.
//!
//! Architectural constraints (non-negotiable, ADR 0019 §55-86):
//! - The validator lives in `output::agent_seam_packets`; this module may call
//!   it because both modules are inside `output/`.
//! - Analysis modules (`analysis/**`) must NOT import `crate::output`, so the
//!   projection CANNOT live there.
//! - No parallel Perl-specific completeness validator is introduced.
//!   The only flip gate is `validate_agent_gap_record_packet(..) == Ok(())`.
//! - A Perl-local `repair_packet_ready` boolean or bespoke renderer is
//!   forbidden (ADR 0019 §83-86).
//!
//! This projection mirrors `typescript_gap_record_for`
//! (`output/typescript_packet_projection.rs`). It reads the same `perl_*:`
//! evidence keys that `perl_preview_card.rs` reads (so the two stay in
//! lockstep), but projects them into the shared `GapRecord` container rather
//! than the bespoke `PerlPreviewCard` rendering struct.
//!
//! Relationship to the existing `gap_record_from_perl_preview_finding`
//! (`gap_decision_ledger.rs:755`): that function builds a `GapRecord` from
//! JSON check-output (not from a `Finding`) and deliberately marks
//! `agent_packet` eligibility **false** (markdown-advisory only). This
//! projection is the *positive* path — it projects a `Finding` into a
//! `GapRecord` with `agent_packet` eligible, gated by the shared validator.
//! Per the Campaign 31 plan, the bespoke path is decommissioned in PR 16
//! once this shared-validator path is the single authority.
//!
//! Wiring: `preview_actionability.rs` calls `perl_gap_record_for` for
//! `LanguageId::Perl` findings, mirroring how it calls
//! `typescript_gap_record_for` for TypeScript/JavaScript. In the current
//! state, production Perl findings do not carry the `gap_state:` /
//! `actionability_category:` evidence `preview_actionability_for` reads, so
//! the function returns `None` at that gate before reaching the projection;
//! the projection only ever produces `Some(record)` for synthetic test
//! findings. PR 16 lands the real Perl evidence path.

use crate::domain::{ExposureClass, Finding, LanguageId, LanguageStatus};
use crate::output::gap_decision_ledger::{
    GapAnchor, GapRecord, GapRepairRoute, ProjectionEligibility,
};
use std::collections::BTreeMap;

/// The authority-boundary string carried by all Perl preview packets.
///
/// Matches `perl_preview_card.rs` and the TypeScript projection's
/// `TS_AUTHORITY_BOUNDARY` — `preview_advisory_only` is the cross-language
/// constant (ADR 0019 §71-73: render via the shared helpers, do not fork).
const PERL_AUTHORITY_BOUNDARY: &str = "preview_advisory_only";

/// Project a Perl preview finding into a `GapRecord` for shared-validator
/// consumption, applying every fail-closed precondition (P-language through
/// P-missing-disc).
///
/// Returns `None` whenever ANY precondition fails — the false case requires
/// no positive proof. Only `Ok(())` from the shared validator leads to a flip.
///
/// # Preconditions checked (fail-closed)
/// - P-language: `finding.language == Perl` and `language_status == Preview`
/// - P-class: `finding.class == WeaklyExposed` (the only class the preview
///   card / projection accepts)
/// - P-dynamic: no dynamic-or-partial-boundary sentinel evidence
///   (`perl_dynamic_boundary: true` / `perl_boundary_status: dynamic` /
///   `perl_packet_status: partial` / `perl_fact_packet_status: partial`)
/// - P-gap: the finding carries a `canonical_gap` (source of the gap id)
/// - P-repair-kind / P-target-shape / P-test-location / P-assertion /
///   P-verify: the corresponding `perl_*:` evidence keys are present
/// - P-stop / P-must-not-change: at least one `perl_stop_if:` and one
///   `perl_must_not_change:` line (mirrors `perl_preview_card.rs:77`)
/// - P-related-test: at least one related test with an oracle-strength rank
///   (mirrors the TS G-D gate)
/// - P-missing-disc: at least one `activation.missing_discriminators` entry
///   (mirrors the TS G-E gate)
pub(crate) fn perl_gap_record_for(finding: &Finding) -> Option<GapRecord> {
    // P-language + P-status: only Perl preview findings are eligible.
    if finding.language != Some(LanguageId::Perl)
        || finding.language_status != Some(LanguageStatus::Preview)
    {
        return None;
    }

    // P-class: the preview projection accepts only WeaklyExposed (mirrors
    // perl_preview_card.rs:60).
    if finding.class != ExposureClass::WeaklyExposed {
        return None;
    }

    // P-dynamic: fail closed on any dynamic/partial-boundary sentinel.
    if has_dynamic_or_partial_boundary(finding) {
        return None;
    }

    // P-gap: the canonical gap is the source of the gap id (Perl findings
    // carry it on the Finding struct; TS re-derives from the probe id).
    let canonical_gap = finding.canonical_gap.as_ref()?;
    let canonical_gap_id = canonical_gap.id.clone();

    // P-repair-kind / P-target-shape / P-test-location / P-assertion /
    // P-verify: every required perl_* evidence key must be present.
    let repair_kind = evidence_value(finding, "perl_repair_kind: ")?;
    let _target_test_shape = evidence_value(finding, "perl_target_test_shape: ")?;
    let suggested_test_location = evidence_value(finding, "perl_suggested_test_location: ")?;
    let suggested_assertion = evidence_value(finding, "perl_suggested_assertion: ")?;
    let verify_command = evidence_value(finding, "perl_verify_command: ")?;

    // P-stop / P-must-not-change: at least one of each (mirrors the preview
    // card's non-empty requirement).
    let stop_conditions = evidence_values(finding, "perl_stop_if: ");
    if stop_conditions.is_empty() {
        return None;
    }
    let must_not_change = evidence_values(finding, "perl_must_not_change: ");
    if must_not_change.is_empty() {
        return None;
    }

    // P-related-test: at least one related test (mirrors TS G-D). We pick the
    // strongest by oracle rank for the target_file / target_line.
    let related_test = finding
        .related_tests
        .iter()
        .max_by_key(|t| t.oracle_strength.rank())?;
    let test_file = related_test.file.display().to_string().replace('\\', "/");
    // Prefer the related-test file for the edit surface; fall back to the
    // file component of the suggested test location.
    let target_file = if test_file.is_empty() {
        suggested_test_location
            .split_once("::")
            .map(|(file, _)| file)
            .unwrap_or(suggested_test_location)
            .to_string()
    } else {
        test_file
    };

    // P-missing-disc: at least one named missing discriminator (mirrors TS G-E).
    if finding.activation.missing_discriminators.is_empty() {
        return None;
    }
    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()
        .map(|d| d.value.clone());

    // Build the receipt command (mirrors the TS receipt shape — no external
    // provider, no curl/http; the shared validator only checks non-empty).
    let receipt_command = perl_receipt_command(&canonical_gap_id, verify_command);

    // route_kind: map the Perl repair kind onto the shared Rust taxonomy.
    // (Mirrors `perl_route_kind` at gap_decision_ledger.rs:1142-1154 — kept
    // private here to avoid cross-module visibility churn; the two must stay
    // in lockstep. If they drift, the validator-parity test catches it.)
    let route_kind = perl_route_kind_for(repair_kind);

    let repair_route = GapRepairRoute {
        route_kind: route_kind.to_string(),
        target_file: Some(target_file.clone()),
        related_test: Some(format!("{target_file}::{}", related_test.name)),
        assertion_shape: Some(suggested_assertion.to_string()),
        missing_discriminator,
        changed_behavior: None,
        target_line: if related_test.line > 0 {
            Some(related_test.line as u64)
        } else {
            None
        },
        inspection_command: None,
        stop_conditions,
    };

    // Anchor: probe location + owner from the canonical gap (Perl fixtures
    // carry the owner on canonical_gap; probe.owner is often None).
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
        .and_then(|sym| sym.0.rsplit("::").next().map(ToString::to_string))
        .or_else(|| {
            canonical_gap
                .owner
                .rsplit("::")
                .next()
                .map(ToString::to_string)
                .or_else(|| Some(canonical_gap.owner.clone()))
        });
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
            reason: "Perl preview complete-contract projection (RIPR-SPEC-0064, ADR 0019)"
                .to_string(),
        },
    );

    Some(GapRecord {
        gap_id: finding.id.clone(),
        canonical_gap_id,
        seam_id: None,
        kind: "perl_preview_boundary".to_string(),
        language: "perl".to_string(),
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
        evidence_ids: vec![finding.id.clone()],
        projection_eligibility,
        verification_commands: vec![verify_command.to_string()],
        receipt_command: Some(receipt_command),
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: PERL_AUTHORITY_BOUNDARY.to_string(),
    })
}

/// Derive the receipt command for a Perl preview finding.
///
/// Mirrors `typescript_receipt_command`: a fixed `ripr outcome …` shape with
/// no external provider, curl, or http request. The shared validator only
/// checks `receipt_command` is non-empty (`agent_seam_packets.rs:904-911`).
pub(crate) fn perl_receipt_command(canonical_gap_id: &str, verify_command: &str) -> String {
    let slug = canonical_gap_id
        .chars()
        .map(|c| if c == ':' || c == '/' { '_' } else { c })
        .collect::<String>();
    let receipt_path = format!("target/ripr/receipts/{slug}.targeted-test-outcome.json");
    format!(
        "ripr outcome --before <baseline> --after <repair> --verify-cmd \"{verify_command}\" --out {receipt_path}"
    )
}

/// Map the Perl repair kind (from `perl_repair_kind:` evidence) onto the
/// shared Rust `route_kind` taxonomy.
///
/// Kept in lockstep with `perl_route_kind` at
/// `gap_decision_ledger.rs:1142-1154`. The two must agree — if they drift,
/// the validator-parity test (`validator_parity_complete_finding_passes_*`)
/// catches it because a wrong route_kind can fail the shared validator.
fn perl_route_kind_for(repair_kind: &str) -> &'static str {
    match repair_kind {
        "add_exact_return_assertion" | "exact_return_assertion" | "return_value" => {
            "AddExactReturnAssertion"
        }
        "add_predicate_boundary_assertion" | "predicate_boundary" => "AddBoundaryAssertion",
        "add_exception_observer" | "exception_path" | "error_path" => "AddExceptionObserver",
        "add_field_assertion" | "hash_field" | "object_field" | "field" => "AddFieldAssertion",
        "add_output_observer" | "output_log" | "warn_log" => "AddOutputObserver",
        "strengthen_existing_test" | "upgrade_assertion" => "StrengthenExistingTest",
        _ => "AddPerlPreviewAssertion",
    }
}

/// True if any dynamic/partial-boundary sentinel evidence is present.
///
/// Mirrors `perl_preview_card.rs:306-316` so the projection and the preview
/// card agree on what counts as a blocking dynamic boundary.
fn has_dynamic_or_partial_boundary(finding: &Finding) -> bool {
    finding.evidence.iter().any(|entry| {
        matches!(
            entry.as_str(),
            "perl_dynamic_boundary: true"
                | "perl_boundary_status: dynamic"
                | "perl_packet_status: partial"
                | "perl_fact_packet_status: partial"
        )
    })
}

/// Extract a value from the finding's evidence vec by prefix (mirrors the TS
/// helper at `typescript_packet_projection.rs:260-267`).
fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

/// Extract all values matching a prefix (for multi-line keys like
/// `perl_stop_if:` and `perl_must_not_change:`).
fn evidence_values(finding: &Finding, prefix: &str) -> Vec<String> {
    finding
        .evidence
        .iter()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        LanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState,
    };
    use crate::output::agent_seam_packets::{
        gap_record_packet_do_not_do, validate_agent_gap_record_packet,
    };
    use std::path::PathBuf;

    // ──────────────────────────────────────────────────────────────────────
    // Validator-parity tests (mirror typescript_packet_projection.rs:387-498).
    // These prove the Perl projection's flip is driven by the shared
    // validator (ADR 0019 §77-81), not a parallel Perl-specific path.
    // ──────────────────────────────────────────────────────────────────────

    fn complete_perl_finding() -> Finding {
        Finding {
            id: "probe:lib_My_App_pm:8:perl_return".to_string(),
            canonical_gap: Some(FindingCanonicalGap {
                id: "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
                    .to_string(),
                language: "perl".to_string(),
                file: "lib/My/App.pm".to_string(),
                owner: "perl:lib/My/App.pm::My::App::discount".to_string(),
                behavior_kind: "return_value".to_string(),
                probe_kind: "exact_return_assertion".to_string(),
                normalized_discriminator: "return_value".to_string(),
            }),
            probe: Probe {
                id: ProbeId("probe:lib_My_App_pm:8:perl_return".to_string()),
                location: SourceLocation::new("lib/My/App.pm", 8, 5),
                owner: None,
                family: ProbeFamily::ReturnValue,
                delta: DeltaKind::Value,
                before: Some("return $price".to_string()),
                after: Some("return $discounted".to_string()),
                expression: "return $discounted".to_string(),
                expected_sinks: vec!["return_value".to_string()],
                required_oracles: vec!["exact_return_assertion".to_string()],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Perl fact packet links the related test to the changed owner",
                ),
                infect: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Changed return value reaches the owner result",
                ),
                propagate: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Return value can propagate to Test::More assertion",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(
                        StageState::Yes,
                        Confidence::Medium,
                        "Related test reaches the changed owner",
                    ),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "Exact return discriminator is missing",
                    ),
                },
            },
            confidence: 0.8,
            evidence: vec![
                "perl_packet_id: perl-preview:gap-return".to_string(),
                "perl_repair_kind: add_exact_return_assertion".to_string(),
                "perl_target_test_shape: Test::More exact_return_assertion".to_string(),
                "perl_suggested_test_location: t/app.t::discount_smoke".to_string(),
                "perl_suggested_assertion: assert the exact returned `return_value` value"
                    .to_string(),
                "perl_verify_command: prove t/app.t".to_string(),
                "perl_receipt_command: ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json".to_string(),
                "perl_confidence: medium".to_string(),
                "perl_allowed_edit_boundary: t/app.t".to_string(),
                "perl_forbidden_edit_boundary: lib/My/App.pm, badges/ripr-plus.json".to_string(),
                "perl_stop_if: perl-lsp packet status changes".to_string(),
                "perl_stop_if: related test no longer reaches owner".to_string(),
                "perl_must_not_change: do not edit Perl production code".to_string(),
                "perl_must_not_change: do not add suppressions or intent ledger entries"
                    .to_string(),
                "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted".to_string(),
                "raw_evidence_ref: leg=perl_oracle;file=t/app.t;line=7;kind=perl_oracle;source_id=oracle:t/app.t:7:is;owner=perl:lib/My/App.pm::My::App::discount;sample=is(discount(...), 90)".to_string(),
            ],
            missing: vec!["return_value".to_string()],
            flow_sinks: vec![],
            activation: ActivationEvidence {
                observed_values: vec![],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "return_value".to_string(),
                    reason: "Related Perl test reaches the owner but lacks an exact return discriminator".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "discount_smoke".to_string(),
                file: PathBuf::from("t/app.t"),
                line: 7,
                oracle: Some("ok(discount(...))".to_string()),
                oracle_kind: OracleKind::SmokeOnly,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: Some("Add a focused Perl assertion.".to_string()),
            language: Some(LanguageId::Perl),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    /// The GapRecord produced by `perl_gap_record_for` for the complete
    /// fixture MUST pass `validate_agent_gap_record_packet`. Any future fork
    /// of the logic would break this test (ADR 0019 §77-81).
    #[test]
    fn validator_parity_complete_finding_passes_shared_validator() {
        let finding = complete_perl_finding();
        let maybe_record = perl_gap_record_for(&finding);
        assert!(
            maybe_record.is_some(),
            "complete Perl finding must produce a GapRecord"
        );
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let result = validate_agent_gap_record_packet(&record);
        assert!(
            result.is_ok(),
            "shared validator must accept complete Perl GapRecord, got: {:?}",
            result
        );
    }

    /// Missing verify command → projection returns None (P-verify).
    #[test]
    fn validator_parity_missing_verify_command_returns_none() {
        let mut finding = complete_perl_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("perl_verify_command:"));
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing perl_verify_command must return None (P-verify)"
        );
    }

    /// Missing suggested assertion → projection returns None (P-assertion).
    /// (Perl has no `oracle_expected:` key like TS; `perl_suggested_assertion:`
    /// is the closest required assertion evidence — the G-C analog.)
    #[test]
    fn validator_parity_missing_suggested_assertion_returns_none() {
        let mut finding = complete_perl_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("perl_suggested_assertion:"));
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing perl_suggested_assertion must return None (P-assertion)"
        );
    }

    /// Dynamic boundary sentinel → projection returns None (P-dynamic).
    /// (Mirrors TS G-C dynamic-assertion gate; the Perl analog is the
    /// `perl_dynamic_boundary: true` sentinel from `perl_preview_card.rs`.)
    #[test]
    fn validator_parity_dynamic_boundary_returns_none() {
        let mut finding = complete_perl_finding();
        finding
            .evidence
            .push("perl_dynamic_boundary: true".to_string());
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "perl_dynamic_boundary sentinel must return None (P-dynamic)"
        );
    }

    /// Wrong language → projection returns None (P-language).
    /// (Perl has no `actionability_category:` gate like TS; the language check
    /// is the G-A analog for Perl.)
    #[test]
    fn validator_parity_wrong_language_returns_none() {
        let mut finding = complete_perl_finding();
        finding.language = Some(LanguageId::Python);
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "non-Perl language must return None (P-language)"
        );
    }

    /// No related tests → projection returns None (P-related-test, mirrors TS G-D).
    #[test]
    fn validator_parity_no_related_tests_returns_none() {
        let mut finding = complete_perl_finding();
        finding.related_tests.clear();
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "no related tests must return None (P-related-test)"
        );
    }

    /// Empty missing_discriminators → projection returns None (P-missing-disc,
    /// mirrors TS G-E).
    #[test]
    fn validator_parity_no_missing_discriminators_returns_none() {
        let mut finding = complete_perl_finding();
        finding.activation.missing_discriminators.clear();
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "empty missing_discriminators must return None (P-missing-disc)"
        );
    }

    /// Partial packet sentinel → projection returns None (P-dynamic).
    /// (Mirrors TS G-F cross-language-bridge gate; the Perl analog is the
    /// `perl_packet_status: partial` sentinel from `perl_preview_card.rs`.)
    #[test]
    fn validator_parity_partial_packet_returns_none() {
        let mut finding = complete_perl_finding();
        finding
            .evidence
            .push("perl_packet_status: partial".to_string());
        let record = perl_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "perl_packet_status: partial sentinel must return None (P-dynamic)"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // Non-parity tests: pin helper behavior + the ADR 0019 invariants.
    // ──────────────────────────────────────────────────────────────────────

    /// `perl_receipt_command` produces a `ripr outcome` shape with no curl/http
    /// (mirrors the TS receipt invariant).
    #[test]
    fn perl_receipt_command_is_ripr_outcome_shape() {
        let cmd = perl_receipt_command("gap:perl:lib/My/App.pm:discount", "prove t/app.t");
        assert!(
            cmd.starts_with("ripr outcome --before"),
            "receipt must be a ripr outcome command: {cmd}"
        );
        assert!(
            cmd.contains("target/ripr/receipts/"),
            "receipt must write to target/ripr/receipts/: {cmd}"
        );
        assert!(
            !cmd.contains("curl") && !cmd.contains("http"),
            "receipt must not invoke curl/http: {cmd}"
        );
        // Slug replaces ':' and '/' with '_' (but preserves '.' so file
        // extensions like .pm stay readable).
        assert!(
            cmd.contains("gap_perl_lib_My_App.pm_discount"),
            "canonical_gap_id must be slugified in the receipt path (':' and '/' -> '_'): {cmd}"
        );
    }

    /// `gap_record_packet_do_not_do` (shared helper) returns the preview
    /// clause for a Perl GapRecord whose `language_status != "stable"`.
    /// Pins ADR 0019 §73: render via the shared helper, do not fork.
    #[test]
    fn must_not_change_includes_preview_clause_via_shared_function() {
        let finding = complete_perl_finding();
        let maybe_record = perl_gap_record_for(&finding);
        assert!(maybe_record.is_some(), "complete Perl finding must project");
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let do_not_do = gap_record_packet_do_not_do(&record);
        assert!(
            do_not_do.iter().any(|line| line.contains("preview")),
            "shared gap_record_packet_do_not_do must include the preview clause for a Perl GapRecord: {do_not_do:?}"
        );
    }

    /// The complete Perl GapRecord carries the preview authority boundary —
    /// never a stronger boundary (cardinal-sin guard).
    #[test]
    fn complete_finding_carries_preview_authority_boundary() {
        let finding = complete_perl_finding();
        let maybe_record = perl_gap_record_for(&finding);
        assert!(maybe_record.is_some(), "complete Perl finding must project");
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        assert_eq!(
            record.authority_boundary, PERL_AUTHORITY_BOUNDARY,
            "Perl GapRecord authority_boundary must stay preview_advisory_only"
        );
    }
}
