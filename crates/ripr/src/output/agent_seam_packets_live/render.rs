use super::currentness::{
    BLOCKED_STALE, CURRENT, GapRecordSourceCurrentness, NOT_EVALUATED, QUEUED, STALE,
};
use crate::app::causal_projection::CausalDeltaArtifact;
use crate::output::agent_seam_packets_legacy as legacy;
use crate::output::gap_decision_ledger::GapRecord;
use serde_json::{Value, json};

pub(crate) fn render_agent_gap_record_queue_json_with_currentness(
    root: &str,
    gap_ledger_path: &str,
    records: &[GapRecord],
    language: &str,
    top: usize,
    source_currentness: &GapRecordSourceCurrentness,
) -> Result<String, String> {
    // The typed candidate model is the only assignment authority: validation,
    // ordering, currentness partitioning, and selection all happen here,
    // before any serialization.
    let model = legacy::build_gap_record_queue_model(records, language)?;
    let conflict_counts = legacy::gap_record_queue_conflict_counts(&model.candidates);

    let mut current_total = 0usize;
    let mut stale_total = 0usize;
    let mut not_evaluated_total = 0usize;
    let mut assignable: Vec<(&legacy::GapRecordQueueCandidate, GapRecordSourceCurrentness)> =
        Vec::new();
    let mut blocked: Vec<(&legacy::GapRecordQueueCandidate, GapRecordSourceCurrentness)> =
        Vec::new();
    for candidate in &model.candidates {
        let effective = effective_candidate_currentness(candidate, source_currentness);
        match effective.status.as_str() {
            CURRENT => current_total += 1,
            STALE => stale_total += 1,
            _ => not_evaluated_total += 1,
        }
        if effective.is_assignable() {
            assignable.push((candidate, effective));
        } else {
            blocked.push((candidate, effective));
        }
    }

    // `--top` selects from the live-current assignable frontier only; stale
    // and not_evaluated candidates never consume a bounded assignment slot.
    let packets: Vec<Value> = assignable
        .iter()
        .take(top)
        .enumerate()
        .map(|(index, (candidate, effective))| {
            let mut packet = legacy::gap_record_queue_packet_value(
                candidate,
                &conflict_counts,
                root,
                gap_ledger_path,
                index + 1,
            );
            decorate_packet_currentness(&mut packet, effective)?;
            Ok(packet)
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Blocked candidates stay visible in a separate, bounded, action-free
    // review projection with their typed state, reason, and recovery route.
    let blocked_review_total = blocked.len();
    let blocked_review: Vec<Value> = blocked
        .iter()
        .take(top)
        .map(|(candidate, effective)| blocked_review_entry(candidate, effective))
        .collect();

    let returned = packets.len();
    // Every returned packet is assignable by construction, so the returned
    // assignable count equals the returned count; the unreturned denominator
    // exposes how much assignable frontier `--top` left unrendered.
    let returned_assignable_total = returned;
    let unreturned_assignable_total = assignable.len().saturating_sub(returned_assignable_total);

    let mut envelope = legacy::gap_record_queue_envelope_value(
        root,
        gap_ledger_path,
        &model,
        language,
        top,
        packets,
    );
    let object = envelope
        .as_object_mut()
        .ok_or_else(|| "typed swarm queue renderer must produce an object".to_string())?;

    let summary = object
        .get_mut("summary")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "typed swarm queue is missing summary".to_string())?;
    summary.insert("current_total".to_string(), json!(current_total));
    summary.insert("stale_total".to_string(), json!(stale_total));
    summary.insert(
        "not_evaluated_total".to_string(),
        json!(not_evaluated_total),
    );
    summary.insert("assignable_total".to_string(), json!(assignable.len()));
    summary.insert(
        "returned_assignable_total".to_string(),
        json!(returned_assignable_total),
    );
    summary.insert(
        "unreturned_assignable_total".to_string(),
        json!(unreturned_assignable_total),
    );
    summary.insert(
        "blocked_total".to_string(),
        json!(stale_total + not_evaluated_total),
    );
    summary.insert(
        "blocked_review_total".to_string(),
        json!(blocked_review_total),
    );
    summary.insert(
        "blocked_review_returned".to_string(),
        json!(blocked_review.len()),
    );

    object.insert("source_currentness".to_string(), source_currentness.json());
    object.insert(
        "assignment_policy".to_string(),
        json!({
            "assignable_when": "queue_state=queued and staleness_status=current",
            "not_evaluated_is_assignable": false,
            "source_currentness_owner": "producer_validated_repo_exposure",
        }),
    );
    object.insert("blocked_review".to_string(), Value::Array(blocked_review));
    if assignable.is_empty() && current_total + stale_total + not_evaluated_total > 0 {
        object.insert("status".to_string(), Value::String("blocked".to_string()));
    }
    let must_not_infer = object
        .get_mut("must_not_infer")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "typed swarm queue is missing must_not_infer".to_string())?;
    must_not_infer.push(Value::String(
        "only queue_state=queued with staleness_status=current is assignable; stale and not_evaluated packets are review-only"
            .to_string(),
    ));

    legacy::render_gap_record_queue_envelope_value(envelope)
}

pub(crate) fn render_agent_gap_record_packet_json_with_live_currentness(
    gap_ledger_path: &str,
    record: &GapRecord,
    causal_projection: Option<&CausalDeltaArtifact>,
    source_currentness: &GapRecordSourceCurrentness,
) -> Result<String, String> {
    let legacy_packet = legacy::render_agent_gap_record_packet_json_with_causal(
        gap_ledger_path,
        record,
        causal_projection,
    )?;
    let model = legacy::build_gap_record_queue_model(
        std::slice::from_ref(record),
        record.language.as_str(),
    )?;
    let candidate = model
        .candidates
        .first()
        .ok_or_else(|| "agent packet currentness source did not emit a candidate".to_string())?;
    let effective = effective_candidate_currentness(candidate, source_currentness);

    if effective.is_assignable() {
        let mut envelope = serde_json::from_str::<Value>(&legacy_packet)
            .map_err(|error| format!("parse agent gap packet JSON failed: {error}"))?;
        let object = envelope
            .as_object_mut()
            .ok_or_else(|| "agent gap packet renderer must produce an object".to_string())?;
        let packet = object
            .get_mut("packets")
            .and_then(Value::as_array_mut)
            .and_then(|packets| packets.first_mut())
            .ok_or_else(|| "agent gap packet renderer is missing its packet".to_string())?;
        decorate_packet_currentness(packet, &effective)?;
        object.insert("status".to_string(), Value::String("advisory".to_string()));
        object.insert("source_currentness".to_string(), effective.json());
        let mut rendered = serde_json::to_string_pretty(&envelope)
            .map_err(|error| format!("render current agent gap packet failed: {error}"))?;
        rendered.push('\n');
        return Ok(rendered);
    }

    render_blocked_agent_gap_packet(gap_ledger_path, record, &effective)
}

fn render_blocked_agent_gap_packet(
    gap_ledger_path: &str,
    record: &GapRecord,
    currentness: &GapRecordSourceCurrentness,
) -> Result<String, String> {
    let gap_id = if record.gap_id.trim().is_empty() {
        record.canonical_gap_id.as_str()
    } else {
        record.gap_id.as_str()
    };
    let canonical_gap_id = if record.canonical_gap_id.trim().is_empty() {
        None
    } else {
        Some(record.canonical_gap_id.as_str())
    };
    let envelope = json!({
        "schema_version": crate::app::AGENT_SEAM_PACKET_SCHEMA_VERSION,
        "scope": "repo",
        "source": "gap_decision_ledger",
        "status": "blocked",
        "inputs": {
            "gap_ledger": gap_ledger_path,
        },
        "source_currentness": currentness.json(),
        "packets_total": 0,
        "packets": [],
        "blocked_candidate": {
            "gap_id": gap_id,
            "canonical_gap_id": canonical_gap_id,
            "language": record.language.as_str(),
            "queue_state": currentness.queue_state.as_str(),
            "staleness_status": currentness.status.as_str(),
            "staleness_reason": currentness.reason.as_str(),
            "refresh_commands": &currentness.refresh_commands,
        },
        "must_not_infer": [
            "blocked_candidate is review-only and carries no edit, verify, receipt, or packet command authority",
            "source_currentness=candidate_current does not establish live packet currentness",
            "stale or not_evaluated currentness must be refreshed before assignment"
        ],
    });
    let mut rendered = serde_json::to_string_pretty(&envelope)
        .map_err(|error| format!("render blocked agent gap packet failed: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
}

/// Resolve the effective per-candidate currentness from the typed candidate
/// model. Assignment authority stays with `status == current` — never
/// `status != stale` — so a `not_evaluated` candidate is stop-and-refresh,
/// never assignable by default.
fn effective_candidate_currentness(
    candidate: &legacy::GapRecordQueueCandidate,
    source_currentness: &GapRecordSourceCurrentness,
) -> GapRecordSourceCurrentness {
    let queue_state = candidate.queue_state.as_str();
    let status = candidate.staleness_status.as_str();

    match (queue_state, status) {
        (BLOCKED_STALE, STALE) => GapRecordSourceCurrentness::stale(
            candidate.staleness_reason.as_str(),
            source_currentness.refresh_commands.clone(),
            source_currentness.source_kind.clone(),
            source_currentness.source_path.clone(),
        ),
        (QUEUED, NOT_EVALUATED) | (QUEUED, CURRENT) => source_currentness.clone(),
        (queue_state, status) => GapRecordSourceCurrentness::not_evaluated(
            format!(
                "packet producer emitted unsupported currentness combination queue_state={queue_state}, staleness_status={status}"
            ),
            source_currentness.refresh_commands.clone(),
            source_currentness.source_kind.clone(),
            source_currentness.source_path.clone(),
        ),
    }
}

/// Build the bounded, action-free review projection for one blocked
/// candidate: identity, queue/currentness state, reason, conflict group, and
/// recovery only — no packet, verify, receipt, suggested-test, or edit-surface
/// authority.
fn blocked_review_entry(
    candidate: &legacy::GapRecordQueueCandidate,
    currentness: &GapRecordSourceCurrentness,
) -> Value {
    json!({
        "source_index": candidate.source_index,
        "gap_id": candidate.gap_id.as_str(),
        "canonical_gap_id": candidate.canonical_gap_id.as_ref(),
        "language": candidate.language.as_str(),
        "queue_state": currentness.queue_state.as_str(),
        "staleness_status": currentness.status.as_str(),
        "staleness_reason": currentness.reason.as_str(),
        "conflict_group": candidate.conflict_group.as_str(),
        "assignment": {
            "eligible": false,
            "reason": currentness.reason.as_str(),
        },
        "refresh_commands": json!(&currentness.refresh_commands),
    })
}

fn decorate_packet_currentness(
    packet: &mut Value,
    currentness: &GapRecordSourceCurrentness,
) -> Result<(), String> {
    let object = packet
        .as_object_mut()
        .ok_or_else(|| "agent packet currentness target must be an object".to_string())?;
    object.insert(
        "queue_state".to_string(),
        Value::String(currentness.queue_state.clone()),
    );
    object.insert(
        "staleness_status".to_string(),
        Value::String(currentness.status.clone()),
    );
    object.insert(
        "staleness_reason".to_string(),
        Value::String(currentness.reason.clone()),
    );
    object.insert("source_currentness".to_string(), currentness.json());
    object.insert(
        "assignment".to_string(),
        json!({
            "eligible": currentness.is_assignable(),
            "reason": if currentness.is_assignable() {
                "producer-validated live-current packet"
            } else {
                currentness.reason.as_str()
            },
        }),
    );
    if !currentness.is_assignable() {
        strip_assignment_authority(object);
        object.insert(
            "refresh_commands".to_string(),
            json!(&currentness.refresh_commands),
        );
    }
    Ok(())
}

fn strip_assignment_authority(object: &mut serde_json::Map<String, Value>) {
    for field in [
        "packet_command_args",
        "verify_command",
        "receipt_command",
        "verification_commands",
        "command_specs",
        "suggested_test_file",
        "suggested_test_name",
        "allowed_edit_surface",
        "allowed_files",
        "forbidden_files",
    ] {
        object.remove(field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_record(receipt: Option<Value>) -> Result<GapRecord, String> {
        serde_json::from_value(json!({
            "gap_id": "gap:python:pricing-boundary",
            "source_currentness": "candidate_current",
            "canonical_gap_id": "gap:python:src/pricing.py:calculate_discount:predicate_boundary",
            "kind": "MissingBoundaryAssertion",
            "language": "python",
            "language_status": "preview",
            "scope": "repo",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "anchor": {
                "file": "src/pricing.py",
                "line": 7,
                "owner": "calculate_discount"
            },
            "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/test_pricing.py",
                "assertion_shape": "assert calculate_discount(100, 100) == 90",
                "changed_behavior": "amount >= threshold"
            },
            "verification_commands": ["pytest tests/test_pricing.py"],
            "receipt_command": "ripr agent receipt --verify-json target/ripr/workflow/verify.json --seam-id gap:python:pricing-boundary --test-changed tests/test_pricing.py",
            "receipt": receipt,
            "projection_eligibility": {
                "agent_packet": {
                    "eligible": true,
                    "reason": "bounded repair route"
                }
            }
        }))
        .map_err(|error| format!("build GapRecord fixture failed: {error}"))
    }

    fn numbered_record(index: usize, receipt: Option<Value>) -> Result<GapRecord, String> {
        serde_json::from_value(json!({
            "gap_id": format!("gap:python:pricing-boundary-{index}"),
            "source_currentness": "candidate_current",
            "canonical_gap_id": format!(
                "gap:python:src/pricing.py:calculate_discount:predicate_boundary:{index}"
            ),
            "kind": "MissingBoundaryAssertion",
            "language": "python",
            "language_status": "preview",
            "scope": "repo",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "anchor": {
                "file": "src/pricing.py",
                "line": 7,
                "owner": "calculate_discount"
            },
            "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/test_pricing.py",
                "assertion_shape": "assert calculate_discount(100, 100) == 90",
                "changed_behavior": "amount >= threshold"
            },
            "verification_commands": ["pytest tests/test_pricing.py"],
            "receipt_command": "ripr agent receipt --verify-json target/ripr/workflow/verify.json --seam-id gap:python:pricing-boundary --test-changed tests/test_pricing.py",
            "receipt": receipt,
            "projection_eligibility": {
                "agent_packet": {
                    "eligible": true,
                    "reason": "bounded repair route"
                }
            }
        }))
        .map_err(|error| format!("build GapRecord fixture failed: {error}"))
    }

    fn current_source() -> GapRecordSourceCurrentness {
        GapRecordSourceCurrentness::current(
            "exact current fixture",
            vec!["refresh source".to_string(), "refresh ledger".to_string()],
            Some("repo_exposure".to_string()),
            Some("target/ripr/reports/repo-exposure.json".to_string()),
        )
    }

    fn unknown_source() -> GapRecordSourceCurrentness {
        GapRecordSourceCurrentness::not_evaluated(
            "legacy ledger has no live snapshot binding",
            vec!["refresh source".to_string(), "refresh ledger".to_string()],
            None,
            None,
        )
    }

    /// Negative control for the assignment authority: a receipt-less record
    /// with a `not_evaluated` source must stay unassignable. If assignment
    /// ever weakened from `status == current` to `status != stale`, this
    /// `not_evaluated` record would be incorrectly assignable and this test
    /// would fail.
    #[test]
    fn candidate_current_alone_remains_non_assignable() -> Result<(), String> {
        let record = valid_record(None)?;
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &[record],
            "python",
            1,
            &unknown_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        assert!(
            value
                .get("packets")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "not_evaluated candidates must never enter the assignable packets: {json}"
        );
        let packet = value
            .get("blocked_review")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing review-only packet: {json}"))?;
        assert_eq!(
            packet.get("queue_state").and_then(Value::as_str),
            Some(super::super::currentness::BLOCKED_NOT_EVALUATED)
        );
        assert_eq!(
            packet.get("staleness_status").and_then(Value::as_str),
            Some(NOT_EVALUATED)
        );
        assert!(packet.get("packet_command_args").is_none());
        assert!(packet.get("verify_command").is_none());
        assert!(
            packet
                .get("refresh_commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| !commands.is_empty()),
            "review-only projection must carry its recovery route: {json}"
        );
        assert_eq!(
            value.get("status").and_then(Value::as_str),
            Some("blocked"),
            "an empty assignable frontier must explain that work cannot be assigned: {json}"
        );
        assert_eq!(
            value
                .get("summary")
                .and_then(|summary| summary.get("assignable_total"))
                .and_then(Value::as_u64),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn exact_current_source_keeps_packet_assignment_surface() -> Result<(), String> {
        let record = valid_record(None)?;
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &[record],
            "python",
            1,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let packet = value
            .get("packets")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing current packet: {json}"))?;
        assert_eq!(
            packet.get("queue_state").and_then(Value::as_str),
            Some(QUEUED)
        );
        assert_eq!(
            packet.get("staleness_status").and_then(Value::as_str),
            Some(CURRENT)
        );
        assert!(packet.get("packet_command_args").is_some());
        assert!(packet.get("verify_command").is_some());
        Ok(())
    }

    #[test]
    fn receipt_stale_overrides_current_source() -> Result<(), String> {
        let record = valid_record(Some(json!({"state": "stale"})))?;
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &[record],
            "python",
            1,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        assert!(
            value
                .get("packets")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "a receipt-stale candidate must not enter assignable packets: {json}"
        );
        let packet = value
            .get("blocked_review")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing stale review packet: {json}"))?;
        assert_eq!(
            packet.get("queue_state").and_then(Value::as_str),
            Some(BLOCKED_STALE)
        );
        assert_eq!(
            packet.get("staleness_status").and_then(Value::as_str),
            Some(STALE)
        );
        assert!(packet.get("packet_command_args").is_none());
        Ok(())
    }

    #[test]
    fn queue_and_single_packet_copy_the_same_non_current_disposition() -> Result<(), String> {
        let record = valid_record(None)?;
        let source = unknown_source();
        let queue = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            std::slice::from_ref(&record),
            "python",
            1,
            &source,
        )?;
        let packet = render_agent_gap_record_packet_json_with_live_currentness(
            "gap-ledger.json",
            &record,
            None,
            &source,
        )?;
        let queue_value = serde_json::from_str::<Value>(&queue)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let packet_value = serde_json::from_str::<Value>(&packet)
            .map_err(|error| format!("packet JSON should parse: {error}"))?;
        let queue_candidate = queue_value
            .get("blocked_review")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing queue candidate: {queue}"))?;
        let blocked_candidate = packet_value
            .get("blocked_candidate")
            .ok_or_else(|| format!("missing blocked candidate: {packet}"))?;
        assert_eq!(
            queue_candidate.get("queue_state").and_then(Value::as_str),
            blocked_candidate.get("queue_state").and_then(Value::as_str)
        );
        assert_eq!(
            queue_candidate
                .get("staleness_status")
                .and_then(Value::as_str),
            blocked_candidate
                .get("staleness_status")
                .and_then(Value::as_str)
        );
        assert!(
            packet_value
                .get("packets")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        );
        for forbidden in [
            "packet_command_args",
            "verify_command",
            "receipt_command",
            "allowed_edit_surface",
            "allowed_files",
        ] {
            assert!(
                !packet.contains(&format!("\"{forbidden}\"")),
                "blocked single packet leaked {forbidden}: {packet}"
            );
        }
        Ok(())
    }

    /// Issue #3504 acceptance: with a stale first record, a live-current
    /// assignable second record, and `--top 1`, `packets[0]` is the second
    /// record with `priority = 1` and its original `source_index`; the stale
    /// first record never consumes the bounded slot.
    #[test]
    fn top_selects_from_assignable_frontier_not_blocked_records() -> Result<(), String> {
        let stale_first = valid_record(Some(json!({"state": "stale"})))?;
        let current_second = numbered_record(1, None)?;
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &[stale_first, current_second],
            "python",
            1,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let packets = value
            .get("packets")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing packets: {json}"))?;
        assert_eq!(packets.len(), 1, "top 1 must return exactly one packet");
        let packet = packets
            .first()
            .ok_or_else(|| format!("missing selected packet: {json}"))?;
        assert_eq!(
            packet.get("gap_id").and_then(Value::as_str),
            Some("gap:python:pricing-boundary-1"),
            "the live-current second record must win the bounded slot: {json}"
        );
        assert_eq!(packet.get("priority").and_then(Value::as_u64), Some(1));
        assert_eq!(
            packet.get("source_index").and_then(Value::as_u64),
            Some(1),
            "the original upstream source_index must be preserved"
        );
        assert_eq!(
            packet.get("queue_state").and_then(Value::as_str),
            Some(QUEUED)
        );
        assert_eq!(
            packet.get("staleness_status").and_then(Value::as_str),
            Some(CURRENT)
        );

        let blocked_review = value
            .get("blocked_review")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing blocked_review: {json}"))?;
        assert_eq!(blocked_review.len(), 1);
        let review = blocked_review
            .first()
            .ok_or_else(|| format!("missing blocked review projection: {json}"))?;
        assert_eq!(
            review.get("gap_id").and_then(Value::as_str),
            Some("gap:python:pricing-boundary"),
            "the stale first record must stay visible as review-only"
        );
        assert_eq!(
            review.get("queue_state").and_then(Value::as_str),
            Some(BLOCKED_STALE)
        );
        assert!(review.get("packet_command_args").is_none());
        assert!(review.get("verify_command").is_none());

        let summary = value
            .get("summary")
            .ok_or_else(|| format!("missing summary: {json}"))?;
        assert_eq!(
            summary.get("assignable_total").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(summary.get("returned").and_then(Value::as_u64), Some(1));
        assert_eq!(
            summary
                .get("unreturned_assignable_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        Ok(())
    }

    /// `--top` selects from the frontier; it does not truncate the filter. A
    /// bound larger than the assignable frontier renders everything and keeps
    /// the honest denominator.
    #[test]
    fn top_larger_than_frontier_renders_everything_with_honest_denominator() -> Result<(), String> {
        let records = [
            numbered_record(0, None)?,
            numbered_record(1, None)?,
            numbered_record(2, Some(json!({"state": "stale"})))?,
        ];
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &records,
            "python",
            10,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let packets = value
            .get("packets")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing packets: {json}"))?;
        assert_eq!(packets.len(), 2, "every assignable candidate renders");
        let priorities: Vec<_> = packets
            .iter()
            .filter_map(|packet| packet.get("priority").and_then(Value::as_u64))
            .collect();
        assert_eq!(priorities, vec![1, 2], "frontier order is preserved");
        let source_indices: Vec<_> = packets
            .iter()
            .filter_map(|packet| packet.get("source_index").and_then(Value::as_u64))
            .collect();
        assert_eq!(source_indices, vec![0, 1], "upstream ledger order wins");
        let summary = value
            .get("summary")
            .ok_or_else(|| format!("missing summary: {json}"))?;
        assert_eq!(
            summary.get("assignable_total").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(summary.get("returned").and_then(Value::as_u64), Some(2));
        assert_eq!(
            summary
                .get("unreturned_assignable_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            summary.get("blocked_review_total").and_then(Value::as_u64),
            Some(1)
        );
        Ok(())
    }

    /// Truncating a larger frontier must keep the denominator: the summary
    /// names how much assignable frontier `--top` left unrendered.
    #[test]
    fn top_truncates_frontier_and_reports_unreturned_denominator() -> Result<(), String> {
        let records = [
            numbered_record(0, None)?,
            numbered_record(1, None)?,
            numbered_record(2, None)?,
        ];
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &records,
            "python",
            2,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let packets = value
            .get("packets")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing packets: {json}"))?;
        assert_eq!(packets.len(), 2);
        let summary = value
            .get("summary")
            .ok_or_else(|| format!("missing summary: {json}"))?;
        assert_eq!(
            summary.get("assignable_total").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(summary.get("returned").and_then(Value::as_u64), Some(2));
        assert_eq!(
            summary
                .get("returned_assignable_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            summary
                .get("unreturned_assignable_total")
                .and_then(Value::as_u64),
            Some(1),
            "the unreturned assignable denominator must stay visible"
        );
        Ok(())
    }

    /// A frontier with only stale and/or not_evaluated candidates renders
    /// `packets = []`, keeps the artifact `blocked`, and explains why work
    /// cannot be assigned with typed reasons and recovery routes.
    #[test]
    fn blocked_only_frontier_renders_empty_packets_with_explanation() -> Result<(), String> {
        let records = [
            valid_record(Some(json!({"state": "stale"})))?,
            numbered_record(1, None)?,
        ];
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &records,
            "python",
            5,
            &unknown_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        assert!(
            value
                .get("packets")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "no assignable candidate means no packets: {json}"
        );
        assert_eq!(value.get("status").and_then(Value::as_str), Some("blocked"));
        let blocked_review = value
            .get("blocked_review")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing blocked_review: {json}"))?;
        assert_eq!(blocked_review.len(), 2);
        for review in blocked_review {
            assert!(
                review
                    .get("staleness_reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| !reason.is_empty()),
                "each review projection explains its block: {json}"
            );
            assert!(
                review
                    .get("refresh_commands")
                    .and_then(Value::as_array)
                    .is_some_and(|commands| !commands.is_empty()),
                "each review projection carries its recovery route: {json}"
            );
            for forbidden in [
                "packet_command_args",
                "verify_command",
                "receipt_command",
                "suggested_test_file",
                "allowed_edit_surface",
                "allowed_files",
            ] {
                assert!(
                    review.get(forbidden).is_none(),
                    "review projection leaked {forbidden}: {json}"
                );
            }
        }
        let summary = value
            .get("summary")
            .ok_or_else(|| format!("missing summary: {json}"))?;
        assert_eq!(
            summary.get("assignable_total").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            summary.get("blocked_total").and_then(Value::as_u64),
            Some(2)
        );
        Ok(())
    }

    /// The review projection is bounded like the assignment frontier, and the
    /// summary always carries the full blocked denominator.
    #[test]
    fn blocked_review_is_bounded_with_full_denominator() -> Result<(), String> {
        let records = [
            numbered_record(0, Some(json!({"state": "stale"})))?,
            numbered_record(1, Some(json!({"state": "stale"})))?,
            numbered_record(2, Some(json!({"state": "stale"})))?,
        ];
        let json = render_agent_gap_record_queue_json_with_currentness(
            ".",
            "gap-ledger.json",
            &records,
            "python",
            2,
            &current_source(),
        )?;
        let value = serde_json::from_str::<Value>(&json)
            .map_err(|error| format!("queue JSON should parse: {error}"))?;
        let blocked_review = value
            .get("blocked_review")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing blocked_review: {json}"))?;
        assert_eq!(blocked_review.len(), 2, "review projections honor --top");
        let summary = value
            .get("summary")
            .ok_or_else(|| format!("missing summary: {json}"))?;
        assert_eq!(
            summary.get("blocked_review_total").and_then(Value::as_u64),
            Some(3),
            "the full blocked denominator must stay visible"
        );
        assert_eq!(
            summary
                .get("blocked_review_returned")
                .and_then(Value::as_u64),
            Some(2)
        );
        Ok(())
    }
}
