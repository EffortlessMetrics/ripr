use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
    WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT, WORKFLOW_AGENT_STATUS_ARTIFACT,
    WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT, WORKFLOW_AGENT_VERIFY_ARTIFACT,
    WORKFLOW_MANIFEST_ARTIFACT, agent_status_command, display_path,
};
use crate::app::agent_status::{self, AgentStatusCommand, AgentStatusReport};
use serde_json::Value;
use std::path::Path;

pub(crate) const AGENT_REVIEW_SUMMARY_SCHEMA_VERSION: &str = "0.1";

const REPO_EXPOSURE_ARTIFACT: &str = "target/ripr/reports/repo-exposure.json";
const OPERATOR_COCKPIT_ARTIFACT: &str = "target/ripr/reports/operator-cockpit.json";
const OPERATOR_COCKPIT_MARKDOWN_ARTIFACT: &str = "target/ripr/reports/operator-cockpit.md";
const LSP_COCKPIT_ARTIFACT: &str = "target/ripr/reports/lsp-cockpit.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewSummaryReport {
    pub(crate) schema_version: String,
    pub(crate) tool: String,
    pub(crate) status: String,
    pub(crate) root: String,
    pub(crate) target_seam: Option<AgentReviewTargetSeam>,
    pub(crate) static_movement: AgentReviewStaticMovement,
    pub(crate) next_command: Option<AgentStatusCommand>,
    pub(crate) surfaces: Vec<AgentReviewSurface>,
    pub(crate) ci_artifacts: Vec<AgentReviewArtifact>,
    pub(crate) reviewer_summary: AgentReviewTextSummary,
    pub(crate) limits: AgentReviewLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewTargetSeam {
    pub(crate) seam_id: String,
    pub(crate) source: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u64>,
    pub(crate) seam_kind: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewStaticMovement {
    pub(crate) state: String,
    pub(crate) before_class: Option<String>,
    pub(crate) after_class: Option<String>,
    pub(crate) grip_class: Option<String>,
    pub(crate) evidence_artifact: Option<String>,
    pub(crate) verify_artifact: Option<String>,
    pub(crate) summary: String,
    pub(crate) next_action: Option<AgentReviewNextAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewNextAction {
    pub(crate) kind: String,
    pub(crate) summary: String,
    pub(crate) recommended_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewSurface {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) path: Option<String>,
    pub(crate) state: String,
    pub(crate) status: String,
    pub(crate) required: bool,
    pub(crate) summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewArtifact {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewTextSummary {
    pub(crate) headline: String,
    pub(crate) what_changed: String,
    pub(crate) evidence: String,
    pub(crate) remaining: String,
    pub(crate) reviewer_should_inspect: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewLimits {
    pub(crate) static_artifact_relationship: bool,
    pub(crate) runtime_mutation_execution: bool,
    pub(crate) automatic_edits: bool,
    pub(crate) generated_tests: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReceiptSnapshot {
    seam_id: String,
    file: Option<String>,
    line: Option<u64>,
    seam_kind: Option<String>,
    before_class: Option<String>,
    after_class: Option<String>,
    grip_class: Option<String>,
    movement: String,
    verify_artifact: Option<String>,
    remaining_gap: Option<String>,
    next_recommendation: Option<String>,
    next_action: Option<AgentReviewNextAction>,
}

#[derive(Clone, Debug)]
struct ArtifactRead {
    value: Option<Value>,
    surface: AgentReviewSurface,
}

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

pub(crate) fn render_agent_review_summary_markdown(report: &AgentReviewSummaryReport) -> String {
    let mut rendered = String::new();
    rendered.push_str("# RIPR Agent Review Summary\n\n");
    rendered.push_str(&format!("Status: {}\n", report.status));
    match &report.target_seam {
        Some(seam) => rendered.push_str(&format!("Target seam: {}\n", seam.seam_id)),
        None => rendered.push_str("Target seam: unknown\n"),
    }
    rendered.push_str(&format!("Movement: {}\n", report.static_movement.state));
    if let Some(before) = &report.static_movement.before_class {
        let after = report
            .static_movement
            .after_class
            .as_deref()
            .unwrap_or("unknown");
        rendered.push_str(&format!("Static class: {before} -> {after}\n"));
    }
    if let Some(artifact) = &report.static_movement.evidence_artifact {
        rendered.push_str(&format!("Evidence artifact: {artifact}\n"));
    }
    rendered.push('\n');
    rendered.push_str("## Reviewer Focus\n\n");
    rendered.push_str(&format!("{}\n\n", report.reviewer_summary.headline));
    rendered.push_str(&format!(
        "What changed: {}\n",
        report.reviewer_summary.what_changed
    ));
    rendered.push_str(&format!("Evidence: {}\n", report.reviewer_summary.evidence));
    rendered.push_str(&format!(
        "Remaining: {}\n",
        report.reviewer_summary.remaining
    ));
    if !report.reviewer_summary.reviewer_should_inspect.is_empty() {
        rendered.push_str("\nInspect:\n");
        for item in &report.reviewer_summary.reviewer_should_inspect {
            rendered.push_str(&format!("- {item}\n"));
        }
    }
    if let Some(next_command) = &report.next_command {
        rendered.push_str("\nNext command:\n");
        rendered.push_str("```bash\n");
        rendered.push_str(&next_command.command);
        rendered.push_str("\n```\n");
    }
    rendered.push_str("\n## Limits\n\n");
    rendered.push_str("- Static artifact relationship only.\n");
    rendered.push_str("- No runtime mutation execution.\n");
    rendered.push_str("- No automatic source edits.\n");
    rendered.push_str("- No generated tests.\n");
    rendered
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

fn read_json_surface(
    root: &Path,
    name: &'static str,
    label: &'static str,
    path: &'static str,
    required: bool,
) -> ArtifactRead {
    let full_path = root.join(path);
    let missing_state = if required {
        "missing"
    } else {
        "optional_missing"
    };
    let missing_summary = if required {
        format!("{label} artifact is missing.")
    } else {
        format!("{label} artifact is not present.")
    };
    let Ok(text) = std::fs::read_to_string(&full_path) else {
        return ArtifactRead {
            value: None,
            surface: AgentReviewSurface {
                name: name.to_string(),
                label: label.to_string(),
                path: Some(path.to_string()),
                state: missing_state.to_string(),
                status: missing_state.to_string(),
                required,
                summary: missing_summary,
            },
        };
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => {
            let status = string_field(&value, "status").unwrap_or_else(|| "present".to_string());
            let summary = surface_summary(name, &value);
            ArtifactRead {
                value: Some(value),
                surface: AgentReviewSurface {
                    name: name.to_string(),
                    label: label.to_string(),
                    path: Some(path.to_string()),
                    state: "present".to_string(),
                    status,
                    required,
                    summary,
                },
            }
        }
        Err(err) => ArtifactRead {
            value: None,
            surface: AgentReviewSurface {
                name: name.to_string(),
                label: label.to_string(),
                path: Some(path.to_string()),
                state: "invalid_json".to_string(),
                status: "invalid_json".to_string(),
                required,
                summary: format!("{label} artifact could not be parsed as JSON: {err}"),
            },
        },
    }
}

fn agent_status_surface(status: &AgentStatusReport, root_display: &str) -> AgentReviewSurface {
    let present = status
        .artifacts
        .iter()
        .filter(|artifact| artifact.present)
        .count();
    let missing = status.artifacts.len().saturating_sub(present);
    let warnings = status.warnings.len();
    AgentReviewSurface {
        name: "agent_status".to_string(),
        label: "Agent status".to_string(),
        path: Some(WORKFLOW_AGENT_STATUS_ARTIFACT.to_string()),
        state: "computed".to_string(),
        status: status.status().to_string(),
        required: true,
        summary: format!(
            "{present} required artifacts present, {missing} missing, {warnings} warnings. Command: {}",
            agent_status_command(root_display, Some(WORKFLOW_AGENT_STATUS_ARTIFACT))
        ),
    }
}

fn surface_summary(name: &str, value: &Value) -> String {
    match name {
        "agent_workflow" => {
            let seam = value
                .get("seam")
                .and_then(|seam| string_field(seam, "seam_id"))
                .unwrap_or_else(|| "unknown".to_string());
            format!("Workflow targets seam {seam}.")
        }
        "agent_receipt" => receipt_snapshot(value)
            .map(|receipt| {
                format!(
                    "Receipt records {} movement for seam {}.",
                    receipt.movement, receipt.seam_id
                )
            })
            .unwrap_or_else(|| {
                "Receipt is present, but no seam movement was recovered.".to_string()
            }),
        "operator_cockpit" => {
            let status = string_field(value, "status").unwrap_or_else(|| "present".to_string());
            let top_weak = array_len(value, "top_weak_seams").unwrap_or(0);
            let next_commands = array_len(value, "next_commands").unwrap_or(0);
            format!(
                "Operator cockpit status is {status}; {top_weak} top weak seams and {next_commands} next commands are listed."
            )
        }
        "repo_exposure" => {
            let seams = value
                .get("metrics")
                .and_then(|metrics| metrics.get("seams_total"))
                .and_then(Value::as_u64)
                .or_else(|| {
                    value
                        .get("summary")
                        .and_then(|summary| summary.get("total_seams"))
                        .and_then(Value::as_u64)
                })
                .unwrap_or(0);
            let weak = value
                .get("metrics")
                .and_then(|metrics| metrics.get("weakly_gripped"))
                .and_then(Value::as_u64)
                .or_else(|| {
                    value
                        .get("summary")
                        .and_then(|summary| summary.get("weakly_exposed"))
                        .and_then(Value::as_u64)
                })
                .unwrap_or(0);
            format!("Repo exposure artifact lists {seams} seams and {weak} weak seams.")
        }
        "lsp_cockpit" => {
            let status = string_field(value, "status").unwrap_or_else(|| "present".to_string());
            format!("LSP cockpit status is {status}.")
        }
        _ => "Artifact is present.".to_string(),
    }
}

fn receipt_snapshot(value: &Value) -> Option<ReceiptSnapshot> {
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
        remaining_gap: summary.and_then(|summary| string_field(summary, "remaining_gap")),
        next_recommendation: summary
            .and_then(|summary| string_field(summary, "next_recommendation")),
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

fn ci_artifacts(root: &Path) -> Vec<AgentReviewArtifact> {
    [
        ("agent_status", WORKFLOW_AGENT_STATUS_ARTIFACT),
        (
            "agent_status_markdown",
            WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT,
        ),
        ("agent_workflow", WORKFLOW_MANIFEST_ARTIFACT),
        (
            "agent_review_summary",
            WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
        ),
        (
            "agent_review_summary_markdown",
            WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
        ),
        ("agent_receipt", WORKFLOW_AGENT_RECEIPT_ARTIFACT),
        ("operator_cockpit", OPERATOR_COCKPIT_ARTIFACT),
        (
            "operator_cockpit_markdown",
            OPERATOR_COCKPIT_MARKDOWN_ARTIFACT,
        ),
    ]
    .into_iter()
    .map(|(name, path)| AgentReviewArtifact {
        name: name.to_string(),
        path: path.to_string(),
        state: if root.join(path).is_file() {
            "present".to_string()
        } else {
            "missing".to_string()
        },
    })
    .collect()
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

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn array_len(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(Value::as_array).map(Vec::len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::loop_commands::{
        WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_BRIEF_ARTIFACT,
        WORKFLOW_AGENT_PACKET_ARTIFACT, WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_agent_review_summary_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-agent-review-summary-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn fixture_value(relative_path: &str) -> Result<Value, String> {
        let text = std::fs::read_to_string(workspace_root().join(relative_path))
            .map_err(|err| format!("read fixture {relative_path}: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse fixture {relative_path}: {err}"))
    }

    fn write_file(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
        }
        std::fs::write(path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn write_complete_artifacts(root: &Path) -> Result<(), String> {
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            r#"{"changed_seams":[{"seam_id":"seam-a"}],"unchanged_seams":[],"new_gaps":[],"resolved_gaps":[]}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{
  "schema_version": "0.3",
  "tool": "ripr",
  "status": "advisory",
  "provenance": {
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "movement": "improved",
    "verify_artifact": {
      "path": "target/ripr/workflow/agent-verify.json",
      "sha256": "sha256:verify"
    }
  },
  "seam": {
    "seam_id": "seam-a",
    "file": "src/lib.rs",
    "line": 42,
    "seam_kind": "predicate_boundary",
    "before": "weakly_gripped",
    "after": "strongly_gripped",
    "change": "improved",
    "grip_class": "strongly_gripped"
  },
  "summary": {
    "remaining_gap": "No remaining static gap is named by this receipt.",
    "next_recommendation": "Keep the focused test and attach the receipt.",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Keep the focused test and include this receipt in review.",
      "safe_to_merge": false
    }
  }
}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"status":"ready","seam":{"seam_id":"seam-a","file":"src/lib.rs","line":42,"seam_kind":"predicate_boundary"}}"#,
        )?;
        write_file(
            &root.join(REPO_EXPOSURE_ARTIFACT),
            r#"{"status":"ready","metrics":{"seams_total":2,"weakly_gripped":1}}"#,
        )?;
        write_file(
            &root.join(OPERATOR_COCKPIT_ARTIFACT),
            r#"{"status":"ready","top_weak_seams":[{"seam_id":"seam-a"}],"next_commands":[]}"#,
        )?;
        write_file(&root.join(LSP_COCKPIT_ARTIFACT), r#"{"status":"ready"}"#)?;
        Ok(())
    }

    struct ReviewFixtureCase<'a> {
        name: &'a str,
        seam_id: &'a str,
        movement: &'a str,
        before: &'a str,
        after: &'a str,
        grip_class: &'a str,
        action_kind: &'a str,
        action_summary: &'a str,
        action_recommendation: &'a str,
    }

    fn write_review_summary_case_artifacts(
        root: &Path,
        case: &ReviewFixtureCase<'_>,
    ) -> Result<(), String> {
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "changed_seams": [{"seam_id": case.seam_id}],
                "unchanged_seams": [],
                "new_gaps": [],
                "resolved_gaps": []
            }))
            .map_err(|err| format!("render verify fixture: {err}"))?,
        )?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "status": "ready",
                "seam": {
                    "seam_id": case.seam_id,
                    "file": "src/pricing.rs",
                    "line": 42,
                    "seam_kind": "predicate_boundary"
                }
            }))
            .map_err(|err| format!("render workflow fixture: {err}"))?,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": "0.3",
                "tool": "ripr",
                "status": "advisory",
                "provenance": {
                    "before_class": case.before,
                    "after_class": case.after,
                    "movement": case.movement,
                    "verify_artifact": {
                        "path": WORKFLOW_AGENT_VERIFY_ARTIFACT,
                        "sha256": "sha256:verify"
                    }
                },
                "seam": {
                    "seam_id": case.seam_id,
                    "file": "src/pricing.rs",
                    "line": 42,
                    "seam_kind": "predicate_boundary",
                    "before": case.before,
                    "after": case.after,
                    "change": case.movement,
                    "grip_class": case.grip_class
                },
                "summary": {
                    "remaining_gap": "Fixture-controlled static review state.",
                    "next_recommendation": case.action_recommendation,
                    "next_action": {
                        "kind": case.action_kind,
                        "summary": case.action_summary,
                        "recommended_action": case.action_recommendation,
                        "safe_to_merge": false
                    }
                }
            }))
            .map_err(|err| format!("render receipt fixture: {err}"))?,
        )?;
        Ok(())
    }

    fn assert_review_summary_matches_fixture(
        root: &Path,
        root_argument: &Path,
        case_name: &str,
    ) -> Result<(), String> {
        let report = build_agent_review_summary_report(root, root_argument);
        let rendered = render_agent_review_summary_json(&report)?;
        let actual: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse rendered review summary: {err}"))?;
        let fixture_path =
            format!("fixtures/boundary_gap/expected/llm-work-loop/{case_name}/review-summary.json");
        assert_eq!(
            actual,
            fixture_value(&fixture_path)?,
            "{case_name} fixture drifted"
        );
        Ok(())
    }

    #[test]
    fn agent_review_summary_joins_status_receipt_cockpit_repo_and_lsp() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("joined");
        write_complete_artifacts(&root)?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["schema_version"], AGENT_REVIEW_SUMMARY_SCHEMA_VERSION);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["target_seam"]["seam_id"], "seam-a");
        assert_eq!(value["static_movement"]["state"], "improved");
        assert_eq!(
            value["static_movement"]["next_action"]["recommended_action"],
            "Keep the focused test and include this receipt in review."
        );
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "operator_cockpit"
                    && surface["state"] == "present")
        );
        assert!(
            value["ci_artifacts"]
                .as_array()
                .ok_or_else(|| "expected ci artifacts".to_string())?
                .iter()
                .any(|artifact| artifact["name"] == "agent_receipt"
                    && artifact["state"] == "present")
        );
        assert_eq!(value["next_command"], Value::Null);

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_reports_missing_receipt_with_next_command() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("missing-receipt");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["static_movement"]["state"], "missing_artifact");
        assert_eq!(value["next_command"]["step"], "before_snapshot");
        assert_eq!(
            value["static_movement"]["next_action"]["recommended_action"],
            "Run the next command listed by agent status."
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixtures_pin_core_states() -> Result<(), String> {
        let cases = [
            ReviewFixtureCase {
                name: "happy",
                seam_id: "seam-happy",
                movement: "improved",
                before: "weakly_gripped",
                after: "strongly_gripped",
                grip_class: "strongly_gripped",
                action_kind: "improved",
                action_summary: "Static grip improved.",
                action_recommendation: "Keep the focused test and include this receipt in review.",
            },
            ReviewFixtureCase {
                name: "unchanged",
                seam_id: "seam-unchanged",
                movement: "unchanged",
                before: "weakly_gripped",
                after: "weakly_gripped",
                grip_class: "weakly_gripped",
                action_kind: "unchanged",
                action_summary: "Static grip did not improve.",
                action_recommendation: "Add the missing discriminator or stronger assertion named by the packet.",
            },
            ReviewFixtureCase {
                name: "regressed",
                seam_id: "seam-regressed",
                movement: "regressed",
                before: "weakly_gripped",
                after: "ungripped",
                grip_class: "ungripped",
                action_kind: "regressed",
                action_summary: "Static grip regressed.",
                action_recommendation: "Revisit the test or code change before merge.",
            },
        ];

        for case in cases {
            let root = unique_agent_review_summary_test_dir(case.name);
            write_review_summary_case_artifacts(&root, &case)?;
            assert_review_summary_matches_fixture(&root, Path::new("."), case.name)?;
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixture_pins_missing_artifact() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("missing-artifact");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        assert_review_summary_matches_fixture(&root, Path::new("."), "missing-artifact")?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixture_pins_stale_artifact() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("stale-artifact");
        let case = ReviewFixtureCase {
            name: "stale-artifact",
            seam_id: "seam-stale",
            movement: "unchanged",
            before: "weakly_gripped",
            after: "weakly_gripped",
            grip_class: "weakly_gripped",
            action_kind: "unchanged",
            action_summary: "Static grip did not improve.",
            action_recommendation: "Add the missing discriminator or stronger assertion named by the packet.",
        };
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"schema_version":"0.1","tool":"ripr","status":"ready","seam":{"seam_id":"seam-stale","file":"src/pricing.rs","line":42,"seam_kind":"predicate_boundary"}}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"schema_version":"0.3","tool":"ripr","status":"advisory","provenance":{"before_class":"weakly_gripped","after_class":"weakly_gripped","movement":"unchanged","verify_artifact":{"path":"target/ripr/workflow/agent-verify.json","sha256":"sha256:verify"}},"seam":{"seam_id":"seam-stale","file":"src/pricing.rs","line":42,"seam_kind":"predicate_boundary","before":"weakly_gripped","after":"weakly_gripped","change":"unchanged","grip_class":"weakly_gripped"},"summary":{"remaining_gap":"Fixture-controlled static review state.","next_recommendation":"Add the missing discriminator or stronger assertion named by the packet.","next_action":{"kind":"unchanged","summary":"Static grip did not improve.","recommended_action":"Add the missing discriminator or stronger assertion named by the packet.","safe_to_merge":false}}}"#,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(25));
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "changed_seams": [{"seam_id": case.seam_id}],
                "unchanged_seams": [],
                "new_gaps": [],
                "resolved_gaps": []
            }))
            .map_err(|err| format!("render verify fixture: {err}"))?,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(25));
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        assert_review_summary_matches_fixture(&root, Path::new("."), "stale-artifact")?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixtures_pin_path_arguments() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("path-arguments");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        assert_review_summary_matches_fixture(&root, Path::new("repo root"), "path-with-spaces")?;
        assert_review_summary_matches_fixture(
            &root,
            Path::new("repo\\root"),
            "windows-separators",
        )?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_markdown_names_review_focus_and_limits() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("markdown");
        write_complete_artifacts(&root)?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_markdown(&report);

        assert!(rendered.contains("# RIPR Agent Review Summary"));
        assert!(rendered.contains("Target seam: seam-a"));
        assert!(rendered.contains("Movement: improved"));
        assert!(rendered.contains("Static artifact relationship only."));
        assert!(rendered.contains("No runtime mutation execution."));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_warns_for_invalid_optional_surface() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("invalid-surface");
        write_complete_artifacts(&root)?;
        write_file(&root.join(OPERATOR_COCKPIT_ARTIFACT), "{")?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "warning");
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "operator_cockpit"
                    && surface["state"] == "invalid_json"
                    && surface["summary"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("could not be parsed as JSON"))
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_recovers_target_from_workflow() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("workflow-target");
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"status":"ready","seam":{"seam_id":"workflow-seam","file":"src/workflow.rs","line":7,"seam_kind":"branch"}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["target_seam"]["seam_id"], "workflow-seam");
        assert_eq!(value["target_seam"]["source"], "agent_workflow");
        assert_eq!(value["target_seam"]["file"], "src/workflow.rs");
        assert_eq!(value["target_seam"]["line"], 7);
        assert_eq!(value["target_seam"]["seam_kind"], "branch");
        assert_eq!(value["static_movement"]["state"], "missing_artifact");

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_recovers_target_from_status_verify() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("status-target");
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            r#"{"changed_seams":[],"unchanged_seams":[],"new_gaps":[{"seam_id":"verify-seam"}],"resolved_gaps":[]}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["target_seam"]["seam_id"], "verify-seam");
        assert_eq!(value["target_seam"]["source"], "agent_verify");
        assert_eq!(value["next_command"]["step"], "before_snapshot");
        assert!(
            value["next_command"]["command"]
                .as_str()
                .unwrap_or_default()
                .contains("repo-exposure-json")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_treats_lsp_cockpit_as_optional() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("optional-lsp");
        write_complete_artifacts(&root)?;
        std::fs::remove_file(root.join(LSP_COCKPIT_ARTIFACT))
            .map_err(|err| format!("remove lsp cockpit: {err}"))?;
        write_file(
            &root.join(REPO_EXPOSURE_ARTIFACT),
            r#"{"status":"ready","summary":{"total_seams":3,"weakly_exposed":2}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "ready");
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "lsp_cockpit"
                    && surface["state"] == "optional_missing")
        );
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "repo_exposure"
                    && surface["summary"]
                        == "Repo exposure artifact lists 3 seams and 2 weak seams.")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_handles_receipt_without_next_action() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("no-next-action");
        write_complete_artifacts(&root)?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"seam":{"seam_id":"seam-without-next","change":"unchanged"}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "ready");
        assert_eq!(value["target_seam"]["seam_id"], "seam-without-next");
        assert_eq!(value["static_movement"]["state"], "unchanged");
        assert_eq!(value["static_movement"]["before_class"], Value::Null);
        assert_eq!(value["static_movement"]["verify_artifact"], Value::Null);
        assert_eq!(
            value["reviewer_summary"]["remaining"],
            "No next action was recovered from the available artifacts."
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_markdown_includes_next_command_when_incomplete() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("markdown-next-command");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_markdown(&report);

        assert!(rendered.contains("Target seam: unknown"));
        assert!(rendered.contains("Next command:"));
        assert!(rendered.contains("ripr check --root . --mode draft --format repo-exposure-json"));
        assert!(rendered.contains("No generated tests."));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
