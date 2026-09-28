use serde_json::Value;

use super::types::AgentReviewNextAction;
use super::util::string_field;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReceiptSnapshot {
    pub(super) seam_id: String,
    pub(super) file: Option<String>,
    pub(super) line: Option<u64>,
    pub(super) seam_kind: Option<String>,
    pub(super) before_class: Option<String>,
    pub(super) after_class: Option<String>,
    pub(super) grip_class: Option<String>,
    pub(super) movement: String,
    pub(super) verify_artifact: Option<String>,
    pub(super) next_action: Option<AgentReviewNextAction>,
}

pub(super) fn receipt_snapshot(value: &Value) -> Option<ReceiptSnapshot> {
    let seam = value.get("seam")?;
    let provenance = value.get("provenance");
    let summary = value.get("summary");
    let seam_id = string_field(seam, "seam_id")
        .or_else(|| provenance.and_then(|provenance| string_field(provenance, "seam_id")))?;
    let movement = string_field(seam, "change")
        .or_else(|| provenance.and_then(|provenance| string_field(provenance, "movement")))
        .unwrap_or_else(|| "unknown".to_string());
    let next_action = summary
        .and_then(|summary| summary.get("next_action"))
        .and_then(next_action);
    Some(ReceiptSnapshot {
        seam_id,
        file: string_field(seam, "file"),
        line: seam.get("line").and_then(Value::as_u64),
        seam_kind: string_field(seam, "seam_kind"),
        before_class: provenance
            .and_then(|provenance| string_field(provenance, "before_class"))
            .or_else(|| string_field(seam, "before")),
        after_class: provenance
            .and_then(|provenance| string_field(provenance, "after_class"))
            .or_else(|| string_field(seam, "after")),
        grip_class: string_field(seam, "grip_class"),
        movement,
        verify_artifact: provenance
            .and_then(|provenance| provenance.get("verify_artifact"))
            .and_then(|artifact| string_field(artifact, "path")),
        next_action,
    })
}

fn next_action(value: &Value) -> Option<AgentReviewNextAction> {
    Some(AgentReviewNextAction {
        kind: string_field(value, "kind")?,
        summary: string_field(value, "summary")?,
        recommended_action: string_field(value, "recommended_action")?,
    })
}
