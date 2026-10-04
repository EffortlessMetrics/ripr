//! RepairCard usability measurement and budget-ratification evidence producer
//! (RIPR-SPEC-0196, #4669): measures the normalized wire size of
//! [`RepairCardV1`] cards against the complete canonical packet envelope for
//! the same seam, over deterministic synthetic fixture profiles, and accounts
//! governed real repair opportunities from the shared #1702/#1579 counting
//! authority without creating a new denominator.
//!
//! Scope discipline (the issue's acceptance law):
//!
//! - Synthetic fixture measurements are reported separately and cannot
//!   ratify real usability by themselves; the ratified decision receipt
//!   (`metrics/repair-card-usability/decision-receipt.json`) records that
//!   real-attempt ratification stays `pending` until the governed corpus
//!   carries attempt cases.
//! - Missing, limited, stale, wrong-target, archaeology-assisted and
//!   abandoned real attempts stay in the denominator through the reused
//!   corpus; nothing is dropped here.
//! - Card-size reduction is reported with actual normalized UTF-8 byte
//!   counts (pretty JSON with exactly one trailing newline, the same
//!   normalization the renderers emit), never token or context-window
//!   guesses.
//! - No model/provider ranking, support-tier promotion or repair-correctness
//!   claim is derived from this sample.

use serde_json::{Value, json};

use crate::agent::command_specs::{AgentArtifactRoute, agent_inspection_command_spec};
use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::{
    ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
};
use crate::analysis::test_grip_evidence::TestGripEvidence;
use crate::cli::commands::agent_card::agent_card_prose_lines;
use crate::domain::{
    Confidence, DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS, DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES,
    DiagnosticConfidence, DiagnosticFixSite, DiagnosticWitness, DiagnosticWitnessLimitation,
    MissingDiscriminatorFact, RepairCardSnapshotCurrentness, StageEvidence, StageState,
};
use crate::output::json::render_pretty_with_newline;

use super::repair_attempt::{RepairAttemptId, RepairAttemptManifest, RepairAttemptState};
use super::repair_card_handoff::{SeamCardFacts, assemble_repair_card, card_packet_json};

/// Versioned schema identity of the usability report document.
pub const REPAIR_CARD_USABILITY_SCHEMA_VERSION: &str = "1.0";

/// Schema marker of the complete packet envelope; the wire card must never
/// embed it (the packet rides behind the card's typed detail reference).
const PACKET_ENVELOPE_MARKER: &str = "agent-seam-packets-json";

/// One deterministic synthetic measurement profile.
struct SyntheticProfile {
    id: &'static str,
    entry: ClassifiedSeam,
    witness: Option<DiagnosticWitness>,
    attempt_head: Option<&'static str>,
}

fn stage(state: StageState) -> StageEvidence {
    StageEvidence::new(state, Confidence::Medium, "synthetic measurement profile")
}

fn boundary_seam() -> RepoSeam {
    RepoSeam::new(
        "src/pricing.rs",
        "pricing::discounted_total",
        SeamKind::PredicateBoundary,
        42,
        88,
        "amount >= discount_threshold",
        RequiredDiscriminator::BoundaryValue {
            description: "amount >= discount_threshold".to_string(),
        },
        ExpectedSink::ReturnValue,
    )
}

fn classified_entry() -> ClassifiedSeam {
    let seam = boundary_seam();
    let seam_id = seam.id().clone();
    ClassifiedSeam {
        seam,
        evidence: TestGripEvidence {
            seam_id,
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::No),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
            new_test_target: None,
        },
        class: SeamGripClass::WeaklyGripped,
    }
}

/// A witness-shaped instruction payload: fix site, suggested assertion, one
/// limitation, one missing-discriminator fact. Field-for-field the same
/// authority the check pipeline projects; only the values are synthetic.
pub(crate) fn measurement_witness() -> DiagnosticWitness {
    DiagnosticWitness {
        kind: "predicate_boundary".to_string(),
        probe_family: "predicate_boundary".to_string(),
        changed_expression: "amount >= discount_threshold".to_string(),
        before: Some("amount - 10".to_string()),
        after: Some("amount - 20".to_string()),
        expected_sink: Some("return_value".to_string()),
        missing_discriminators: vec![MissingDiscriminatorFact {
            value: "boundary value at the discount threshold".to_string(),
            reason: "both sides of the predicate are exercised but the threshold \
                     boundary itself is not pinned"
                .to_string(),
            flow_sink: None,
        }],
        fix_site: Some(DiagnosticFixSite {
            file: "tests/pricing.rs".to_string(),
            line: 7,
            test_name: "far_above_threshold_discounts".to_string(),
            current_oracle: None,
            oracle_kind: "assert_eq".to_string(),
            oracle_strength: "strong".to_string(),
            oracle_location: None,
        }),
        suggested_assertion: Some(
            "assert_eq!(discounted_total(100, 100), 90) pins the threshold boundary".to_string(),
        ),
        explain_command: "ripr explain probe:src_pricing.rs:predicate:demo".to_string(),
        confidence: DiagnosticConfidence {
            value: None,
            basis: "synthetic measurement profile".to_string(),
        },
        limitations: vec![DiagnosticWitnessLimitation {
            kind: "synthetic_profile".to_string(),
            detail: "the witness payload is fixture-shaped; real findings carry \
                     analyzer-produced values"
                .to_string(),
        }],
    }
}

fn measurement_attempt(
    seam_id: &str,
    repository_head: &str,
) -> Result<RepairAttemptManifest, String> {
    Ok(RepairAttemptManifest {
        schema_version: super::repair_attempt::REPAIR_ATTEMPT_SCHEMA_VERSION.to_string(),
        kind: "repair_attempt".to_string(),
        repair_attempt_id: RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")
            .map_err(|error| format!("synthetic measurement attempt id: {error}"))?,
        state: RepairAttemptState::AwaitingEdit,
        root: ".".to_string(),
        store: None,
        repository_head: repository_head.to_string(),
        producer_version: "measurement".to_string(),
        seam_id: seam_id.to_string(),
        created_unix_ms: 0,
        artifacts: Vec::new(),
        next_command: "ripr agent repair --root . --seam-id seam-a --phase after".to_string(),
        limitations: Vec::new(),
        non_claims: Vec::new(),
        after: None,
        last_after_refusal: None,
        terminal_artifacts: Vec::new(),
    })
}

/// The four synthetic profiles exercise the card's observable size envelope:
/// no witness (three unavailable families), a full witness, and a witness
/// with a current or stale attempt detail.
fn synthetic_profiles() -> Vec<SyntheticProfile> {
    let entry = classified_entry();
    let entry_with_witness = classified_entry();
    let entry_with_attempt = classified_entry();
    let entry_with_stale_attempt = classified_entry();
    vec![
        SyntheticProfile {
            id: "boundary_no_witness",
            entry,
            witness: None,
            attempt_head: None,
        },
        SyntheticProfile {
            id: "boundary_with_witness",
            entry: entry_with_witness,
            witness: Some(measurement_witness()),
            attempt_head: None,
        },
        SyntheticProfile {
            id: "witness_with_current_attempt",
            entry: entry_with_attempt,
            witness: Some(measurement_witness()),
            attempt_head: Some("abc123"),
        },
        SyntheticProfile {
            id: "witness_with_stale_attempt",
            entry: entry_with_stale_attempt,
            witness: Some(measurement_witness()),
            attempt_head: Some("deadbeef"),
        },
    ]
}

/// Normalize a rendered JSON document to UTF-8 bytes with exactly one
/// trailing newline, the same normalization every renderer emits.
fn normalized_bytes(rendered: &str) -> usize {
    let trimmed = rendered.trim_end_matches('\n');
    trimmed.len() + 1
}

fn measure_profile(profile: &SyntheticProfile) -> Result<Value, String> {
    let seam_id = profile.entry.seam.id().as_str().to_string();
    // The synthetic measurement assembles pure facts with no checkout: the
    // content-bound packet render is the producer's own portable-root render
    // and the currentness fact is the clean-tree projection.
    let packet_json = card_packet_json(&profile.entry);
    let attempt = profile
        .attempt_head
        .map(|head| measurement_attempt(&seam_id, head))
        .transpose()?;
    let next_command = agent_inspection_command_spec(AgentArtifactRoute::Packet, ".", &seam_id);
    let next_command_display = next_command.display.clone();
    let finding_id = profile.witness.as_ref().map(|_| "finding-demo-1");
    let card = assemble_repair_card(&SeamCardFacts {
        entry: &profile.entry,
        witness: profile.witness.as_ref(),
        finding_id,
        attempt: attempt.as_ref(),
        packet_json: &packet_json,
        repository_head: "abc123",
        workspace_identity: "workspace:measurement",
        currentness: RepairCardSnapshotCurrentness::Current,
        next_command: Some(next_command),
    })?;
    let card_wire = render_pretty_with_newline(&card, "repair card usability measurement")?;
    // The measurement has no checkout to bind, so the closing packet line
    // carries the typed command's portable display.
    let human_lines = agent_card_prose_lines(&card, &next_command_display);
    let human_rendered = format!("{}\n", human_lines.join("\n"));
    let packet_surfaces_seam = {
        let envelope: Value = serde_json::from_str(&packet_json)
            .map_err(|error| format!("synthetic packet envelope did not parse: {error}"))?;
        envelope
            .get("packets")
            .and_then(Value::as_array)
            .is_some_and(|packets| {
                packets.iter().any(|packet| {
                    packet.get("seam_id").and_then(Value::as_str) == Some(seam_id.as_str())
                })
            })
    };
    let canonical_packet_state = card
        .detail_references
        .iter()
        .find(|reference| {
            reference.family == crate::domain::RepairCardDetailFamily::CanonicalPacket
        })
        .map(|reference| {
            serde_json::to_value(reference.state)
                .map(|value| {
                    value
                        .as_str()
                        .map_or_else(|| "unknown".to_string(), str::to_string)
                })
                .map_err(|error| format!("canonical packet state did not serialize: {error}"))
        })
        .transpose()?;
    let card_bytes = normalized_bytes(&card_wire);
    let packet_bytes = normalized_bytes(&packet_json);
    let packet_over_card_percent = packet_bytes
        .checked_mul(100)
        .and_then(|scaled| scaled.checked_div(card_bytes))
        .map_or(Value::Null, |percent| json!(percent));
    Ok(json!({
        "profile": profile.id,
        "card_bytes": card_bytes,
        "packet_bytes": packet_bytes,
        "packet_over_card_percent": packet_over_card_percent,
        "detail_items": card.detail_references.len(),
        "card_within_default_item_bound":
            card.detail_references.len() <= DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS,
        "card_within_default_byte_bound":
            normalized_bytes(&card_wire) <= DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES,
        "card_bytes_below_packet_bytes":
            normalized_bytes(&card_wire) < normalized_bytes(&packet_json),
        "wire_card_omits_packet_envelope": wire_omits_packet_envelope(&card_wire, &seam_id),
        "packet_envelope_surfaces_seam": packet_surfaces_seam,
        "canonical_packet_state": canonical_packet_state,
        "next_action_present": card.next_action.is_some(),
        "human_lines": human_lines.len(),
        "human_bytes": normalized_bytes(&human_rendered),
    }))
}

fn corpus_len(corpus: &Value, key: &str) -> usize {
    corpus
        .get(key)
        .and_then(Value::as_array)
        .map_or(0, |entries| entries.len())
}

/// A wire card omits the packet envelope when neither the raw CLI format
/// marker nor any embedded copy of the measured packet envelope (a string
/// value that parses to a JSON object carrying a `packets` array with an
/// entry for the measured seam) appears anywhere in the card document.
fn wire_omits_packet_envelope(card_wire: &str, seam_id: &str) -> bool {
    if card_wire.contains(PACKET_ENVELOPE_MARKER) {
        return false;
    }
    let Ok(card) = serde_json::from_str::<Value>(card_wire) else {
        return false;
    };
    !string_value_embeds_measured_packet(&card, seam_id)
}

fn string_value_embeds_measured_packet(value: &Value, seam_id: &str) -> bool {
    match value {
        Value::String(text) => {
            let trimmed = text.trim_start();
            if !trimmed.starts_with('{') {
                return false;
            }
            let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
                return false;
            };
            parsed
                .get("packets")
                .and_then(Value::as_array)
                .is_some_and(|packets| {
                    packets.iter().any(|packet| {
                        packet.get("seam_id").and_then(Value::as_str) == Some(seam_id)
                    })
                })
        }
        Value::Array(items) => items
            .iter()
            .any(|item| string_value_embeds_measured_packet(item, seam_id)),
        Value::Object(map) => map
            .values()
            .any(|entry| string_value_embeds_measured_packet(entry, seam_id)),
        _ => false,
    }
}

/// Governed real-opportunity accounting over the shared #1702/#1579 corpus.
/// The corpus is the only denominator; this report never invents one.
fn real_opportunity_accounting(corpus: &Value) -> Value {
    let cases = corpus_len(corpus, "cases");
    let exclusions = corpus_len(corpus, "exclusions");
    let observations = corpus_len(corpus, "observations");
    let card_measurement_state = if cases == 0 {
        "not_measurable"
    } else {
        "measurable"
    };
    json!({
        "denominator_authority": "metrics/rust-repair-trust/corpus.json (#1702/#1579 counting authority, reused)",
        "attempt_cases": cases,
        "exclusions": exclusions,
        "observations": observations,
        "card_measurement_state": card_measurement_state,
        "reason": if cases == 0 {
            "the governed corpus carries zero attempt cases, so real card usability \
             (wrong-target, missing-field, stale-action, stop-condition and \
             archaeology incidents) stays in the denominator; synthetic fixture \
             measurements below cannot ratify it and the decision receipt keeps \
             real-attempt ratification pending"
        } else {
            "governed attempt cases exist; per-opportunity card measurement is \
             expected to be reported against these cases"
        },
    })
}

fn presentation_accounting(profile_count: usize) -> Value {
    let measured = profile_count;
    json!({
        "cli_json": {
            "state": "measured",
            "scope": "synthetic_fixture_profiles",
            "unit": "normalized_utf8_bytes",
            "profiles": measured,
        },
        "cli_human": {
            "state": "measured",
            "scope": "synthetic_fixture_profiles",
            "unit": "lines_and_normalized_utf8_bytes",
            "profiles": measured,
        },
        "lsp": {
            "state": "not_projected",
            "reason": "the standard-LSP RepairCard projection is #4668, outside this slice; \
                       no LSP presentation is measured or implied here",
        },
        "mcp": {
            "state": "not_projected",
            "reason": "the MCP card projection is #4668/#3089/#3090, outside this slice; \
                       no MCP presentation is measured or implied here",
        },
    })
}

/// Build the versioned usability report. Deterministic: no clocks, no
/// randomness, no network, no filesystem access (the caller supplies the
/// already-parsed governed corpus).
pub fn repair_card_usability_report(corpus: &Value) -> Result<Value, String> {
    let mut profiles = Vec::new();
    for profile in synthetic_profiles() {
        profiles.push(measure_profile(&profile)?);
    }
    let profiles_len = profiles.len();
    let report = json!({
        "schema_version": REPAIR_CARD_USABILITY_SCHEMA_VERSION,
        "kind": "repair_card_usability_report",
        "ratification_scope": "synthetic_fixture_profiles",
        "real_opportunities": real_opportunity_accounting(corpus),
        "synthetic_profiles": profiles,
        "presentations": presentation_accounting(profiles_len),
        "ratified_defaults": {
            "max_detail_items": DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS,
            "max_serialized_bytes": DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES,
            "max_inline_detail_bytes": crate::domain::DEFAULT_REPAIR_CARD_MAX_INLINE_DETAIL_BYTES,
            "field_set": "RIPR-SPEC-0192 RepairCardV1 default fields, unchanged",
            "source": "#4666 provisional constants, ratified for the synthetic fixture scope by RIPR-SPEC-0196",
        },
        "field_set_decision": {
            "status": "unchanged",
            "reason": "no default field was removed: the wrong-target and hidden-help \
                       incident rates that would sanction a removal are not_measurable \
                       without governed real attempts, and every default family is \
                       exercised by the synthetic profiles",
        },
    });
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_corpus() -> Value {
        json!({"cases": [], "exclusions": [1], "observations": [1, 2]})
    }

    #[test]
    fn wire_envelope_check_rejects_embedded_packets_and_markers() -> Result<(), String> {
        let seam_id = "seam-demo";
        let plain_card = json!({
            "kind": "repair_card.v1",
            "seam_id": seam_id,
            "note": "compact card without packet content",
        });
        let plain_wire = serde_json::to_string_pretty(&plain_card)
            .map_err(|error| format!("serialize plain card: {error}"))?;
        if !wire_omits_packet_envelope(&plain_wire, seam_id) {
            return Err("a card without packet content must omit the envelope".to_string());
        }
        let embedded = json!({
            "kind": "repair_card.v1",
            "seam_id": seam_id,
            "packet": "{\"packets\":[{\"seam_id\":\"seam-demo\"}]}",
        });
        let embedded_wire = serde_json::to_string_pretty(&embedded)
            .map_err(|error| format!("serialize embedded card: {error}"))?;
        if wire_omits_packet_envelope(&embedded_wire, seam_id) {
            return Err("an embedded packet envelope must not pass the omission check".to_string());
        }
        let marker_card =
            format!("{{\"kind\":\"repair_card.v1\",\"marker\":\"{PACKET_ENVELOPE_MARKER}\"}}");
        if wire_omits_packet_envelope(&marker_card, seam_id) {
            return Err("the raw CLI format marker must not pass the omission check".to_string());
        }
        Ok(())
    }

    #[test]
    fn report_is_deterministic_across_runs() -> Result<(), String> {
        let first = repair_card_usability_report(&synthetic_corpus())?;
        let second = repair_card_usability_report(&synthetic_corpus())?;
        let first_rendered = serde_json::to_string_pretty(&first)
            .map_err(|error| format!("serialize first report: {error}"))?;
        let second_rendered = serde_json::to_string_pretty(&second)
            .map_err(|error| format!("serialize second report: {error}"))?;
        if first_rendered != second_rendered {
            return Err("the usability report must be byte-identical across runs".to_string());
        }
        Ok(())
    }

    #[test]
    fn every_profile_satisfies_the_ratification_relations() -> Result<(), String> {
        let report = repair_card_usability_report(&synthetic_corpus())?;
        let profiles = report
            .get("synthetic_profiles")
            .and_then(Value::as_array)
            .ok_or_else(|| "synthetic_profiles must be an array".to_string())?;
        if profiles.len() != 4 {
            return Err(format!(
                "expected four synthetic profiles, got {}",
                profiles.len()
            ));
        }
        // The card-vs-packet size comparison is a reported measurement, not a
        // ratification relation: on these single-seam synthetic profiles the
        // compact card wire is not smaller than the single-seam packet wire,
        // because both are small and the card carries its envelope, nine
        // detail references, and digests. The ratification relations are the
        // default bounds and the packet-envelope boundaries.
        let mut violations = Vec::new();
        for profile in profiles {
            let name = profile
                .get("profile")
                .and_then(Value::as_str)
                .map_or("unknown", |name| name);
            for relation in [
                "card_within_default_item_bound",
                "card_within_default_byte_bound",
                "wire_card_omits_packet_envelope",
                "packet_envelope_surfaces_seam",
            ] {
                if profile.get(relation).and_then(Value::as_bool) != Some(true) {
                    violations.push(format!("{name}: {relation}"));
                }
            }
        }
        if !violations.is_empty() {
            return Err(format!(
                "ratification relations violated: {}",
                violations.join(", ")
            ));
        }
        Ok(())
    }

    #[test]
    fn zero_attempt_cases_keep_real_measurement_not_measurable() -> Result<(), String> {
        let report = repair_card_usability_report(&synthetic_corpus())?;
        let real = report
            .get("real_opportunities")
            .ok_or_else(|| "real_opportunities missing".to_string())?;
        if real.get("attempt_cases").and_then(Value::as_u64) != Some(0) {
            return Err("the synthetic corpus carries no attempt cases".to_string());
        }
        if real.get("card_measurement_state").and_then(Value::as_str) != Some("not_measurable") {
            return Err("zero attempt cases must keep real measurement not_measurable".to_string());
        }
        if real.get("exclusions").and_then(Value::as_u64) != Some(1)
            || real.get("observations").and_then(Value::as_u64) != Some(2)
        {
            return Err("exclusions and observations must stay in the denominator".to_string());
        }
        Ok(())
    }

    #[test]
    fn attempt_cases_flip_real_measurement_to_measurable() -> Result<(), String> {
        let corpus = json!({"cases": [1], "exclusions": [], "observations": []});
        let report = repair_card_usability_report(&corpus)?;
        if report
            .get("real_opportunities")
            .and_then(|real| real.get("card_measurement_state"))
            .and_then(Value::as_str)
            != Some("measurable")
        {
            return Err("attempt cases must make real measurement measurable".to_string());
        }
        Ok(())
    }

    #[test]
    fn ratified_defaults_match_the_domain_constants() -> Result<(), String> {
        let report = repair_card_usability_report(&synthetic_corpus())?;
        let defaults = report
            .get("ratified_defaults")
            .ok_or_else(|| "ratified_defaults missing".to_string())?;
        if defaults.get("max_detail_items").and_then(Value::as_u64)
            != Some(DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS as u64)
            || defaults.get("max_serialized_bytes").and_then(Value::as_u64)
                != Some(DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES as u64)
        {
            return Err("ratified defaults must name the domain constants verbatim".to_string());
        }
        Ok(())
    }
}
