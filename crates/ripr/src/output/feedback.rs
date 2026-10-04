//! JSON rendering for local usefulness-feedback receipts and join reports.

use crate::domain::{
    ActorKind, FEEDBACK_SCHEMA_VERSION, FeedbackJudgment, FeedbackReason, FeedbackReceipt,
    ReferenceState, ResultIdentity, ReviewStatus,
};
use crate::output::json;
use serde_json::Value;

pub(crate) fn render_receipt_document(
    receipt: &FeedbackReceipt,
    status: &str,
) -> Result<String, String> {
    json::render_pretty_with_newline(
        &receipt_value(receipt, status),
        "usefulness-feedback receipt",
    )
}

pub(crate) fn render_join_document(join: &Value) -> Result<String, String> {
    json::render_pretty_with_newline(join, "usefulness-feedback join")
}

fn receipt_value(receipt: &FeedbackReceipt, status: &str) -> Value {
    serde_json::json!({
        "schema_version": FEEDBACK_SCHEMA_VERSION,
        "kind": "usefulness_feedback_receipt",
        "status": status,
        "tool": "ripr",
        "feedback_id": receipt.feedback_id,
        "idempotency_key": receipt.idempotency_key,
        "snapshot_id": receipt.identity.snapshot_id,
        "canonical_item": receipt.identity.canonical_item,
        "route_digest": receipt.identity.route_digest,
        "attempt_id": receipt.identity.attempt_id,
        "receipt_id": receipt.identity.receipt_id,
        "actor_kind": receipt.actor_kind.as_str(),
        "review_status": receipt.review_status.as_str(),
        "review_actor_kind": receipt.review_actor_kind.map(ActorKind::as_str),
        "reason": receipt.reason.as_str(),
        "judgment": receipt.judgment().as_str(),
        "note": receipt.note,
        "reference_state": receipt.reference_state.as_str(),
        "recorded_at": receipt.recorded_at,
        "policy_effects": {
            "diagnostics": "unchanged",
            "classification": "unchanged",
            "baseline": "unchanged",
            "suppressions": "unchanged",
            "gates": "unchanged",
            "gap_closure": "unchanged"
        },
        "must_not_infer": [
            "helpful feedback does not establish correctness",
            "a negative opinion does not automatically establish a false positive",
            "agent feedback is not human-approved until a human review actor is recorded",
            "recording this receipt changes no diagnostic, classification, baseline, suppression, gate, or gap-closure state"
        ]
    })
}

pub(crate) fn rendered_receipt_from_value(value: &Value) -> Result<FeedbackReceipt, String> {
    if value.get("kind").and_then(Value::as_str) != Some("usefulness_feedback_receipt") {
        return Err("feedback receipt kind must be usefulness_feedback_receipt".to_string());
    }
    if value.get("schema_version").and_then(Value::as_str) != Some(FEEDBACK_SCHEMA_VERSION) {
        return Err(format!(
            "feedback receipt schema_version must be {FEEDBACK_SCHEMA_VERSION}"
        ));
    }
    let reason = FeedbackReason::parse(required_str(value, "reason")?)?;
    let judgment = FeedbackJudgment::parse(required_str(value, "judgment")?)?;
    let judgment_override = if reason == FeedbackReason::Other {
        Some(judgment)
    } else {
        None
    };
    let review_actor = optional_str(value, "review_actor_kind")?;
    Ok(FeedbackReceipt {
        feedback_id: required_str(value, "feedback_id")?.to_string(),
        idempotency_key: required_str(value, "idempotency_key")?.to_string(),
        identity: ResultIdentity {
            snapshot_id: required_str(value, "snapshot_id")?.to_string(),
            canonical_item: optional_str(value, "canonical_item")?.map(str::to_string),
            route_digest: optional_str(value, "route_digest")?.map(str::to_string),
            attempt_id: optional_str(value, "attempt_id")?.map(str::to_string),
            receipt_id: optional_str(value, "receipt_id")?.map(str::to_string),
        },
        actor_kind: ActorKind::parse(required_str(value, "actor_kind")?)?,
        review_status: ReviewStatus::parse(required_str(value, "review_status")?)?,
        review_actor_kind: review_actor.map(ActorKind::parse).transpose()?,
        reason,
        judgment_override,
        note: optional_str(value, "note")?.map(str::to_string),
        reference_state: parse_reference_state(required_str(value, "reference_state")?)?,
        recorded_at: required_str(value, "recorded_at")?.to_string(),
    })
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("feedback receipt missing string field `{field}`"))
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Result<Option<&'a str>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(_) => Err(format!(
            "feedback receipt field `{field}` must be a string or null"
        )),
    }
}

fn parse_reference_state(value: &str) -> Result<ReferenceState, String> {
    match value {
        "current" => Ok(ReferenceState::Current),
        "historical" => Ok(ReferenceState::Historical),
        "mismatched" => Ok(ReferenceState::Mismatched),
        other => Err(format!("unknown feedback reference_state {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_receipt() -> FeedbackReceipt {
        FeedbackReceipt {
            feedback_id: "fb-1".to_string(),
            idempotency_key: "key-1".to_string(),
            identity: ResultIdentity {
                snapshot_id: "snap-1".to_string(),
                canonical_item: None,
                route_digest: Some("add_missing_test".to_string()),
                attempt_id: None,
                receipt_id: None,
            },
            actor_kind: ActorKind::Agent,
            review_status: ReviewStatus::Unreviewed,
            review_actor_kind: None,
            reason: FeedbackReason::UsefulLimitation,
            judgment_override: None,
            note: None,
            reference_state: ReferenceState::Current,
            recorded_at: "unix_ms:1".to_string(),
        }
    }

    #[test]
    fn receipt_json_round_trips_and_states_non_effects() -> Result<(), String> {
        let rendered = render_receipt_document(&sample_receipt(), "created")?;
        let value: Value = serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        assert_eq!(value["kind"], "usefulness_feedback_receipt");
        assert_eq!(value["actor_kind"], "agent");
        assert_eq!(value["review_status"], "unreviewed");
        assert_eq!(value["policy_effects"]["suppressions"], "unchanged");
        assert_eq!(value["policy_effects"]["gates"], "unchanged");
        let restored = rendered_receipt_from_value(&value)?;
        assert_eq!(restored.feedback_id, "fb-1");
        assert!(restored.identity.canonical_item.is_none());
        assert_eq!(restored.actor_kind, ActorKind::Agent);
        Ok(())
    }

    #[test]
    fn malformed_kind_fails_closed() -> Result<(), String> {
        let value = serde_json::json!({"kind": "gate-decision", "schema_version": "0.1"});
        let error = rendered_receipt_from_value(&value)
            .err()
            .ok_or_else(|| "wrong kind".to_string())?;
        assert!(error.contains("usefulness_feedback_receipt"));
        Ok(())
    }
}
