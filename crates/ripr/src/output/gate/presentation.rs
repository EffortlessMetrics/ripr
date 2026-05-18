use super::model::{
    CalibrationEvidence, GateDecision, GateDecisionInputs, GateDecisionReport, GatePolicy,
    GateSummary,
};
use super::{LIMITS_NOTE, SCHEMA_VERSION};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn render_gate_decision_json(report: &GateDecisionReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status,
        "mode": report.mode.as_str(),
        "root": report.root,
        "inputs": inputs_json(&report.inputs),
        "policy": policy_json(&report.policy),
        "summary": summary_json(&report.summary),
        "decisions": report.decisions.iter().map(decision_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "config_errors": report.config_errors,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render gate decision JSON: {err}"))
}

pub(crate) fn render_gate_decision_markdown(report: &GateDecisionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Gate Decision\n\n");
    out.push_str(&format!("Decision: {}\n", report.status));
    out.push_str(&format!("Mode: {}\n", report.mode.as_str()));
    out.push_str(&format!("Evaluated: {}\n", report.summary.evaluated));
    out.push_str(&format!("Blocking: {}\n", report.summary.blocking));
    out.push_str(&format!("Acknowledged: {}\n", report.summary.acknowledged));
    out.push_str(&format!("Advisory: {}\n\n", report.summary.advisory));

    push_decision_section(&mut out, "Blocking", &report.decisions, "blocking");
    push_decision_section(&mut out, "Acknowledged", &report.decisions, "acknowledged");
    push_decision_section(&mut out, "Advisory", &report.decisions, "advisory");
    push_decision_section(&mut out, "Suppressed", &report.decisions, "suppressed");
    push_decision_section(
        &mut out,
        "Not Applicable",
        &report.decisions,
        "not_applicable",
    );

    if !report.config_errors.is_empty() {
        out.push_str("## Config Errors\n\n");
        for error in &report.config_errors {
            out.push_str(&format!("- {}\n", md_escape(error)));
        }
        out.push('\n');
    }
    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_escape(warning)));
        }
        out.push('\n');
    }
    out.push_str("## Limits\n\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn gate_decision_should_fail(report: &GateDecisionReport) -> bool {
    matches!(report.status.as_str(), "blocked" | "config_error")
}

pub(crate) fn gate_decision_status(report: &GateDecisionReport) -> &str {
    &report.status
}

pub(crate) fn markdown_path_for(out: &Path) -> PathBuf {
    let mut path = out.to_path_buf();
    path.set_extension("md");
    path
}

fn inputs_json(inputs: &GateDecisionInputs) -> Value {
    let mut value = json!({
        "repo_exposure": inputs.repo_exposure,
        "pr_guidance": inputs.pr_guidance,
        "sarif_policy": inputs.sarif_policy,
        "labels_json": inputs.labels_json,
        "labels": inputs.labels,
        "agent_verify": inputs.agent_verify,
        "agent_receipt": inputs.agent_receipt,
        "recommendation_calibration": inputs.recommendation_calibration,
        "mutation_calibration": inputs.mutation_calibration,
        "baseline": inputs.baseline,
    });
    if let Some(gap_ledger) = &inputs.gap_ledger
        && let Some(object) = value.as_object_mut()
    {
        object.insert("gap_ledger".to_string(), Value::String(gap_ledger.clone()));
    }
    value
}

fn policy_json(policy: &GatePolicy) -> Value {
    json!({
        "mode": policy.mode.as_str(),
        "threshold": policy.threshold,
        "acknowledgement_labels": policy.acknowledgement_labels,
        "default_workflow_posture": policy.default_workflow_posture,
    })
}

fn summary_json(summary: &GateSummary) -> Value {
    json!({
        "evaluated": summary.evaluated,
        "blocking": summary.blocking,
        "acknowledged": summary.acknowledged,
        "advisory": summary.advisory,
        "suppressed": summary.suppressed,
        "not_applicable": summary.not_applicable,
        "unknown_confidence": summary.unknown_confidence,
    })
}

fn decision_json(decision: &GateDecision) -> Value {
    let mut value = json!({
        "id": decision.id,
        "source": decision.source,
        "decision": decision.decision,
        "gate_reason": decision.gate_reason,
        "seam_id": decision.seam_id,
        "source_id": decision.source_id,
        "static_class": decision.static_class,
        "severity": decision.severity,
        "placement": {
            "path": decision.placement.path,
            "line": decision.placement.line,
        },
        "policy": {
            "mode": decision.policy.mode.as_str(),
            "threshold": decision.policy.threshold,
            "acknowledgement_label": decision.policy.acknowledgement_label,
            "baseline_identity": decision.policy.baseline_identity,
        },
        "evidence": {
            "missing_discriminator": decision.evidence.missing_discriminator,
            "assertion_shape": decision.evidence.assertion_shape,
            "candidate_values": decision.evidence.candidate_values,
            "recommended_test": decision.evidence.recommended_test,
            "nearby_test_changed": decision.evidence.nearby_test_changed,
            "suppressed": decision.evidence.suppressed,
            "configured_off": decision.evidence.configured_off,
            "recommendation_calibration": calibration_json(&decision.evidence.recommendation_calibration),
            "mutation_calibration": calibration_json(&decision.evidence.mutation_calibration),
        }
    });
    if let Some(repair_route) = &decision.evidence.repair_route
        && let Some(evidence) = value.get_mut("evidence").and_then(Value::as_object_mut)
    {
        evidence.insert("repair_route".to_string(), json!(repair_route));
    }
    if !decision.evidence.verification_commands.is_empty()
        && let Some(evidence) = value.get_mut("evidence").and_then(Value::as_object_mut)
    {
        evidence.insert(
            "verification_commands".to_string(),
            json!(decision.evidence.verification_commands),
        );
    }
    if let Some(canonical_gap_id) = &decision.canonical_gap_id
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "canonical_gap_id".to_string(),
            Value::String(canonical_gap_id.clone()),
        );
    }
    if let Some(gap_id) = &decision.gap_id
        && let Some(object) = value.as_object_mut()
    {
        object.insert("gap_id".to_string(), Value::String(gap_id.clone()));
    }
    if let Some(gap_kind) = &decision.gap_kind
        && let Some(object) = value.as_object_mut()
    {
        object.insert("gap_kind".to_string(), Value::String(gap_kind.clone()));
    }
    value
}

fn calibration_json(evidence: &CalibrationEvidence) -> Value {
    json!({
        "available": evidence.available,
        "outcome": evidence.outcome,
        "confidence_effect": evidence.confidence_effect,
    })
}

fn push_decision_section(
    out: &mut String,
    title: &str,
    decisions: &[GateDecision],
    decision_value: &str,
) {
    let section = decisions
        .iter()
        .filter(|decision| decision.decision == decision_value)
        .collect::<Vec<_>>();
    if section.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for decision in section {
        let path = decision.placement.path.as_deref().unwrap_or("<no path>");
        let line = decision
            .placement
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "?".to_string());
        out.push_str(&format!(
            "- {}:{} {} — {}\n",
            md_escape(path),
            line,
            md_escape(decision.static_class.as_deref().unwrap_or("unknown")),
            md_escape(&decision.gate_reason)
        ));
    }
    out.push('\n');
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}
