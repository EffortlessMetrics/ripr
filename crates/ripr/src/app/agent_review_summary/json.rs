use crate::app::agent_status::AgentStatusCommand;
use serde_json::Value;

use super::types::{
    AgentReviewArtifact, AgentReviewLimits, AgentReviewNextAction, AgentReviewStaticMovement,
    AgentReviewSummaryReport, AgentReviewSurface, AgentReviewTargetSeam, AgentReviewTextSummary,
};

pub(crate) fn render_agent_review_summary_json(
    report: &AgentReviewSummaryReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": report.schema_version,
        "tool": report.tool,
        "status": report.status,
        "root": report.root,
        "target_seam": report.target_seam.as_ref().map(target_seam_json),
        "static_movement": static_movement_json(&report.static_movement),
        "next_command": report.next_command.as_ref().map(agent_status_command_json),
        "surfaces": report.surfaces.iter().map(surface_json).collect::<Vec<_>>(),
        "ci_artifacts": report.ci_artifacts.iter().map(artifact_json).collect::<Vec<_>>(),
        "reviewer_summary": reviewer_summary_json(&report.reviewer_summary),
        "limits": limits_json(&report.limits)
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent review summary JSON: {err}"))
}

fn target_seam_json(seam: &AgentReviewTargetSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id,
        "source": seam.source,
        "file": seam.file,
        "line": seam.line,
        "seam_kind": seam.seam_kind
    })
}

fn static_movement_json(movement: &AgentReviewStaticMovement) -> Value {
    serde_json::json!({
        "state": movement.state,
        "before_class": movement.before_class,
        "after_class": movement.after_class,
        "grip_class": movement.grip_class,
        "evidence_artifact": movement.evidence_artifact,
        "verify_artifact": movement.verify_artifact,
        "summary": movement.summary,
        "next_action": movement.next_action.as_ref().map(next_action_json)
    })
}

fn next_action_json(next_action: &AgentReviewNextAction) -> Value {
    serde_json::json!({
        "kind": next_action.kind,
        "summary": next_action.summary,
        "recommended_action": next_action.recommended_action
    })
}

fn surface_json(surface: &AgentReviewSurface) -> Value {
    serde_json::json!({
        "name": surface.name,
        "label": surface.label,
        "path": surface.path,
        "state": surface.state,
        "status": surface.status,
        "required": surface.required,
        "summary": surface.summary
    })
}

fn artifact_json(artifact: &AgentReviewArtifact) -> Value {
    serde_json::json!({
        "name": artifact.name,
        "path": artifact.path,
        "state": artifact.state
    })
}

fn reviewer_summary_json(summary: &AgentReviewTextSummary) -> Value {
    serde_json::json!({
        "headline": summary.headline,
        "what_changed": summary.what_changed,
        "evidence": summary.evidence,
        "remaining": summary.remaining,
        "reviewer_should_inspect": summary.reviewer_should_inspect
    })
}

fn limits_json(limits: &AgentReviewLimits) -> Value {
    serde_json::json!({
        "static_artifact_relationship": limits.static_artifact_relationship,
        "runtime_mutation_execution": limits.runtime_mutation_execution,
        "automatic_edits": limits.automatic_edits,
        "generated_tests": limits.generated_tests
    })
}

fn agent_status_command_json(command: &AgentStatusCommand) -> Value {
    serde_json::json!({
        "step": command.step,
        "artifact": command.artifact,
        "reason": command.reason,
        "command": command.command
    })
}
