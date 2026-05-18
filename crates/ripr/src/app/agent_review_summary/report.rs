use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_AGENT_VERIFY_ARTIFACT, WORKFLOW_MANIFEST_ARTIFACT,
    display_path,
};
use crate::app::agent_status::{self, AgentStatusCommand, AgentStatusReport};
use serde_json::Value;
use std::path::Path;

use super::artifacts::{
    LSP_COCKPIT_ARTIFACT, OPERATOR_COCKPIT_ARTIFACT, REPO_EXPOSURE_ARTIFACT, agent_status_surface,
    ci_artifacts, read_json_surface,
};
use super::receipt::{ReceiptSnapshot, receipt_snapshot};
use super::types::AgentReviewNextAction;
use super::types::{
    AGENT_REVIEW_SUMMARY_SCHEMA_VERSION, AgentReviewLimits, AgentReviewStaticMovement,
    AgentReviewSummaryReport, AgentReviewSurface, AgentReviewTargetSeam, AgentReviewTextSummary,
};
use super::util::string_field;

pub(crate) fn build_agent_review_summary_report(
    root: &Path,
    root_argument: &Path,
) -> AgentReviewSummaryReport {
    let root_display = display_path(root_argument);
    let agent_status = agent_status::build_agent_status_report(root, root_argument);
    let workflow = read_json_surface(
        root,
        "agent_workflow",
        "Agent workflow",
        WORKFLOW_MANIFEST_ARTIFACT,
        false,
    );
    let receipt = read_json_surface(
        root,
        "agent_receipt",
        "Agent receipt",
        WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        true,
    );
    let operator_cockpit = read_json_surface(
        root,
        "operator_cockpit",
        "Operator cockpit",
        OPERATOR_COCKPIT_ARTIFACT,
        false,
    );
    let repo_exposure = read_json_surface(
        root,
        "repo_exposure",
        "Repo exposure",
        REPO_EXPOSURE_ARTIFACT,
        false,
    );
    let lsp_cockpit = read_json_surface(
        root,
        "lsp_cockpit",
        "LSP cockpit",
        LSP_COCKPIT_ARTIFACT,
        false,
    );

    let receipt_snapshot = receipt.value.as_ref().and_then(receipt_snapshot);
    let target_seam = target_seam(
        receipt_snapshot.as_ref(),
        &agent_status,
        workflow.value.as_ref(),
    );
    let static_movement = static_movement(receipt_snapshot.as_ref());
    let next_command = agent_status.missing_commands.first().cloned();
    let mut surfaces = vec![agent_status_surface(&agent_status, &root_display)];
    surfaces.extend([
        workflow.surface,
        receipt.surface,
        operator_cockpit.surface,
        repo_exposure.surface,
        lsp_cockpit.surface,
    ]);
    let ci_artifacts = ci_artifacts(root);
    let status = review_status(&agent_status, &static_movement, &surfaces);
    let reviewer_summary = reviewer_summary(
        &status,
        target_seam.as_ref(),
        &static_movement,
        &next_command,
        &surfaces,
    );

    AgentReviewSummaryReport {
        schema_version: AGENT_REVIEW_SUMMARY_SCHEMA_VERSION.to_string(),
        tool: "ripr".to_string(),
        status,
        root: root_display,
        target_seam,
        static_movement,
        next_command,
        surfaces,
        ci_artifacts,
        reviewer_summary,
        limits: AgentReviewLimits {
            static_artifact_relationship: true,
            runtime_mutation_execution: false,
            automatic_edits: false,
            generated_tests: false,
        },
    }
}

fn target_seam(
    receipt: Option<&ReceiptSnapshot>,
    status: &AgentStatusReport,
    workflow: Option<&Value>,
) -> Option<AgentReviewTargetSeam> {
    if let Some(receipt) = receipt {
        return Some(AgentReviewTargetSeam {
            seam_id: receipt.seam_id.clone(),
            source: "agent_receipt".to_string(),
            file: receipt.file.clone(),
            line: receipt.line,
            seam_kind: receipt.seam_kind.clone(),
        });
    }
    if let Some(seam_id) = workflow
        .and_then(|value| value.get("seam"))
        .and_then(|seam| string_field(seam, "seam_id"))
    {
        return Some(AgentReviewTargetSeam {
            seam_id,
            source: "agent_workflow".to_string(),
            file: workflow
                .and_then(|value| value.get("seam"))
                .and_then(|seam| string_field(seam, "file")),
            line: workflow
                .and_then(|value| value.get("seam"))
                .and_then(|seam| seam.get("line"))
                .and_then(Value::as_u64),
            seam_kind: workflow
                .and_then(|value| value.get("seam"))
                .and_then(|seam| string_field(seam, "seam_kind")),
        });
    }
    status.seam.as_ref().map(|seam| AgentReviewTargetSeam {
        seam_id: seam.seam_id.clone(),
        source: seam.source.clone(),
        file: None,
        line: None,
        seam_kind: None,
    })
}

fn static_movement(receipt: Option<&ReceiptSnapshot>) -> AgentReviewStaticMovement {
    let Some(receipt) = receipt else {
        return AgentReviewStaticMovement {
            state: "missing_artifact".to_string(),
            before_class: None,
            after_class: None,
            grip_class: None,
            evidence_artifact: None,
            verify_artifact: None,
            summary: "Agent receipt is missing; static movement is not available.".to_string(),
            next_action: Some(AgentReviewNextAction {
                kind: "missing_artifact".to_string(),
                summary: "Agent receipt is missing.".to_string(),
                recommended_action: "Run the next command listed by agent status.".to_string(),
            }),
        };
    };

    let before = receipt.before_class.as_deref().unwrap_or("unknown");
    let after = receipt.after_class.as_deref().unwrap_or("unknown");
    AgentReviewStaticMovement {
        state: receipt.movement.clone(),
        before_class: receipt.before_class.clone(),
        after_class: receipt.after_class.clone(),
        grip_class: receipt.grip_class.clone(),
        evidence_artifact: Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string()),
        verify_artifact: receipt.verify_artifact.clone(),
        summary: format!(
            "Static movement is {} ({before} -> {after}).",
            receipt.movement
        ),
        next_action: receipt.next_action.clone(),
    }
}

fn review_status(
    agent_status: &AgentStatusReport,
    movement: &AgentReviewStaticMovement,
    surfaces: &[AgentReviewSurface],
) -> String {
    if movement.state == "missing_artifact" {
        return "incomplete".to_string();
    }
    if agent_status.status() != "complete"
        || surfaces
            .iter()
            .any(|surface| surface.state == "invalid_json" || surface.status == "warning")
    {
        return "warning".to_string();
    }
    "ready".to_string()
}

fn reviewer_summary(
    status: &str,
    seam: Option<&AgentReviewTargetSeam>,
    movement: &AgentReviewStaticMovement,
    next_command: &Option<AgentStatusCommand>,
    surfaces: &[AgentReviewSurface],
) -> AgentReviewTextSummary {
    let target = seam
        .map(|seam| seam.seam_id.as_str())
        .unwrap_or("unknown seam");
    let headline = match movement.state.as_str() {
        "missing_artifact" => format!("Review packet is incomplete for {target}."),
        _ => format!("Review packet is {status} for seam {target}."),
    };
    let what_changed = if movement.state == "missing_artifact" {
        "No static before/after movement is available because the agent receipt is missing."
            .to_string()
    } else {
        movement.summary.clone()
    };
    let evidence = movement
        .evidence_artifact
        .as_ref()
        .map(|artifact| {
            let verify = movement
                .verify_artifact
                .as_deref()
                .unwrap_or(WORKFLOW_AGENT_VERIFY_ARTIFACT);
            format!("Review {artifact} with {verify}.")
        })
        .unwrap_or_else(|| "Run agent receipt after verify to create review evidence.".to_string());
    let remaining = movement
        .next_action
        .as_ref()
        .map(|action| action.recommended_action.clone())
        .or_else(|| {
            next_command
                .as_ref()
                .map(|command| format!("Next missing input: {}", command.reason))
        })
        .unwrap_or_else(|| {
            "No next action was recovered from the available artifacts.".to_string()
        });
    let mut reviewer_should_inspect = vec![
        WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string(),
        WORKFLOW_AGENT_VERIFY_ARTIFACT.to_string(),
    ];
    for surface in surfaces {
        if (surface.name == "operator_cockpit" || surface.name == "repo_exposure")
            && let Some(path) = &surface.path
        {
            reviewer_should_inspect.push(path.clone());
        }
    }
    AgentReviewTextSummary {
        headline,
        what_changed,
        evidence,
        remaining,
        reviewer_should_inspect,
    }
}
