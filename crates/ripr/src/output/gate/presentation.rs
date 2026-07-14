use super::exception_policy::ExceptionPolicyReport;
use super::model::{
    CalibrationEvidence, GateDecision, GateDecisionInputs, GateDecisionReport, GatePolicy,
    GateRepairRoute, GateRepairTarget, GateSummary, NewUnsuppressed,
};
use super::{LIMITS_NOTE, SCHEMA_VERSION};
use crate::app::causal_projection::insert_canonical_delta_fields;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn render_gate_decision_json(report: &GateDecisionReport) -> Result<String, String> {
    let mut document = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status,
        "mode": report.mode.as_str(),
        "root": report.root,
        "inputs": inputs_json(&report.inputs),
        "policy": policy_json(&report.policy),
        "summary": summary_json(&report.summary),
        "new_unsuppressed": new_unsuppressed_json(&report.new_unsuppressed),
        "decisions": report.decisions.iter().map(decision_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "config_errors": report.config_errors,
        "limits_note": LIMITS_NOTE,
    });
    // Additive: present only when `--exception-policy` was supplied (#1442),
    // so existing gate-decision consumers and goldens see identical output.
    if let Some(exception_policy) = &report.exception_policy
        && let Some(object) = document.as_object_mut()
    {
        object.insert(
            "exception_policy".to_string(),
            exception_policy_json(exception_policy),
        );
    }
    if let Some(projection) = &report.causal_projection
        && let Some(object) = document.as_object_mut()
    {
        projection.insert_comparison_fields(object);
    }
    serde_json::to_string_pretty(&document)
        .map_err(|err| format!("failed to render gate decision JSON: {err}"))
}

fn exception_policy_json(report: &ExceptionPolicyReport) -> Value {
    json!({
        "path": report.path,
        "ledger_status": report.ledger_status,
        "ledger_owner": report.ledger_owner,
        "due_review": report.due_review.as_str(),
        "active_count": report.active.len(),
        "active": report.active.iter().map(|entry| json!({
            "id": entry.id,
            "kind": entry.kind,
            "scope": entry.scope,
            "owner": entry.owner,
            "reason": entry.reason,
            "review_after": entry.review_after,
            "expires": entry.expires,
        })).collect::<Vec<_>>(),
        "violations": report.violations.iter().map(|violation| json!({
            "kind": violation.kind,
            "exception_id": violation.exception_id,
            "detail": violation.detail,
            "blocking": violation.blocking,
        })).collect::<Vec<_>>(),
    })
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

    if let Some(exception_policy) = &report.exception_policy {
        out.push_str("## Exception Policy\n\n");
        out.push_str(&format!(
            "Ledger: {} (status: {}, due_review: {})\n",
            md_escape(&exception_policy.path),
            md_escape(&exception_policy.ledger_status),
            exception_policy.due_review.as_str()
        ));
        out.push_str(&format!(
            "Active exceptions: {}\n\n",
            exception_policy.active.len()
        ));
        for entry in &exception_policy.active {
            out.push_str(&format!(
                "- active `{}` ({}) review_after {} expires {}\n",
                md_escape(&entry.id),
                md_escape(&entry.kind),
                md_escape(&entry.review_after),
                md_escape(&entry.expires)
            ));
        }
        for violation in &exception_policy.violations {
            let severity = if violation.blocking {
                "BLOCKING"
            } else {
                "warning"
            };
            out.push_str(&format!(
                "- {severity} {}: {}\n",
                md_escape(&violation.kind),
                md_escape(&violation.detail)
            ));
        }
        out.push('\n');
    }
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
    if let Some(exception_policy) = &inputs.exception_policy
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "exception_policy".to_string(),
            Value::String(exception_policy.clone()),
        );
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
        "gap_state": decision.gap_state,
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
        "repair_route": repair_route_json(&decision.repair_route),
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
    if let Some(delta) = &decision.causal_delta
        && let Some(object) = value.as_object_mut()
    {
        insert_canonical_delta_fields(object, delta);
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

pub(super) fn repair_route_json(route: &GateRepairRoute) -> Value {
    let repair_target = route.repair_target.as_ref().map(|target| match target {
        GateRepairTarget::RelatedTest { name, file, line } => json!({
            "kind": "related_test",
            "name": name,
            "file": file,
            "line": line,
        }),
        GateRepairTarget::ProductionCaller { owner, file, line } => json!({
            "kind": "production_caller",
            "owner": owner,
            "file": file,
            "line": line,
        }),
    });
    let limitation = route.limitation.as_ref().map(|limitation| {
        json!({
            "kind": limitation.kind,
            "missing_fields": limitation.missing_fields,
            "detail": limitation.detail,
        })
    });
    json!({
        "canonical_gap_id": route.canonical_gap_id,
        "seam_id": route.seam_id,
        "classification": route.classification,
        "changed_owner": route.changed_owner,
        "changed_behavior": route.changed_behavior,
        "missing_discriminator": route.missing_discriminator,
        "repair_target": repair_target,
        "test_intent": route.test_intent,
        "verify_command": route.verify_command,
        "receipt_command": route.receipt_command,
        "inspection_command": route.inspection_command,
        "authority_boundary": route.authority_boundary,
        "limitation": limitation,
    })
}

fn calibration_json(evidence: &CalibrationEvidence) -> Value {
    json!({
        "available": evidence.available,
        "outcome": evidence.outcome,
        "confidence_effect": evidence.confidence_effect,
    })
}

fn new_unsuppressed_json(nu: &NewUnsuppressed) -> Value {
    json!({
        "basis": nu.basis,
        "count": nu.count,
        "reason": nu.reason,
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
        if matches!(
            decision.decision.as_str(),
            "blocking" | "acknowledged" | "advisory"
        ) {
            push_optional_code(out, "Gap state", decision.gap_state.as_deref());
            push_optional_code(
                out,
                "Delta attribution",
                decision
                    .delta_attribution
                    .map(|attribution| attribution.as_str()),
            );
            push_repair_route(out, &decision.repair_route);
        }
    }
    out.push('\n');
}

fn push_repair_route(out: &mut String, route: &GateRepairRoute) {
    push_optional_code(out, "Gap", route.canonical_gap_id.as_deref());
    push_optional_code(out, "Seam", route.seam_id.as_deref());
    push_optional_code(out, "Classification", route.classification.as_deref());
    push_optional_code(out, "Changed owner", route.changed_owner.as_deref());
    push_optional_text(out, "Changed behavior", route.changed_behavior.as_deref());
    push_optional_text(
        out,
        "Why it remains open",
        route.missing_discriminator.as_deref(),
    );
    push_repair_target(out, route.repair_target.as_ref());
    push_optional_text(out, "Add", route.test_intent.as_deref());
    push_optional_code(out, "Verify", route.verify_command.as_deref());
    push_optional_code(out, "Receipt", route.receipt_command.as_deref());
    push_optional_code(out, "Inspect", route.inspection_command.as_deref());
    out.push_str(&format!(
        "  - Boundary: `{}`\n",
        md_inline_code(&route.authority_boundary)
    ));
    if let Some(limitation) = &route.limitation {
        out.push_str(&format!(
            "  - Repair route limitation: `{}`\n",
            md_inline_code(limitation.kind)
        ));
        out.push_str(&format!(
            "  - Missing route fields: `{}`\n",
            md_inline_code(&limitation.missing_fields.join(", "))
        ));
        out.push_str(&format!(
            "  - Limitation detail: {}\n",
            md_escape(limitation.detail)
        ));
    }
}

fn push_repair_target(out: &mut String, target: Option<&GateRepairTarget>) {
    match target {
        Some(GateRepairTarget::RelatedTest { name, file, line }) => {
            out.push_str(&format!(
                "  - Near test: `{}` at `{}:{line}`\n",
                md_inline_code(name),
                md_inline_code(file)
            ));
        }
        Some(GateRepairTarget::ProductionCaller { owner, file, line }) => {
            out.push_str(&format!(
                "  - Production caller: `{}`",
                md_inline_code(owner)
            ));
            match (file.as_deref(), line) {
                (Some(file), Some(line)) => {
                    out.push_str(&format!(" at `{}:{line}`", md_inline_code(file)));
                }
                (Some(file), None) => {
                    out.push_str(&format!(" at `{}`", md_inline_code(file)));
                }
                (None, Some(line)) => out.push_str(&format!(" at line `{line}`")),
                (None, None) => {}
            }
            out.push('\n');
        }
        None => {}
    }
}

fn push_optional_code(out: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push_str(&format!("  - {label}: `{}`\n", md_inline_code(value)));
    }
}

fn push_optional_text(out: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push_str(&format!("  - {label}: {}\n", md_escape(value)));
    }
}

fn md_inline_code(value: &str) -> String {
    md_escape(value).replace('`', "\\`")
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_caller_target_markdown_preserves_explicit_location() -> Result<(), String> {
        let mut rendered = String::new();
        let targets = [
            GateRepairTarget::ProductionCaller {
                owner: "foo::dispatch".to_string(),
                file: Some("crates/foo/src/lib.rs".to_string()),
                line: Some(88),
            },
            GateRepairTarget::ProductionCaller {
                owner: "foo::dispatch_file_only".to_string(),
                file: Some("crates/foo/src/lib.rs".to_string()),
                line: None,
            },
            GateRepairTarget::ProductionCaller {
                owner: "foo::dispatch_line_only".to_string(),
                file: None,
                line: Some(89),
            },
            GateRepairTarget::ProductionCaller {
                owner: "foo::dispatch_without_location".to_string(),
                file: None,
                line: None,
            },
        ];

        for target in &targets {
            push_repair_target(&mut rendered, Some(target));
        }
        push_repair_target(&mut rendered, None);

        let expected = concat!(
            "  - Production caller: `foo::dispatch` at `crates/foo/src/lib.rs:88`\n",
            "  - Production caller: `foo::dispatch_file_only` at `crates/foo/src/lib.rs`\n",
            "  - Production caller: `foo::dispatch_line_only` at line `89`\n",
            "  - Production caller: `foo::dispatch_without_location`\n",
        );
        if rendered == expected {
            Ok(())
        } else {
            Err(format!(
                "production caller Markdown mismatch: actual={rendered:?} expected={expected:?}"
            ))
        }
    }
}
