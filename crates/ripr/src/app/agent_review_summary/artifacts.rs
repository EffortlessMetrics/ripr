use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
    WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT, WORKFLOW_AGENT_STATUS_ARTIFACT,
    WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT, WORKFLOW_MANIFEST_ARTIFACT, agent_status_command,
};
use crate::app::agent_status::AgentStatusReport;
use serde_json::Value;
use std::path::Path;

use super::receipt::receipt_snapshot;
use super::types::{AgentReviewArtifact, AgentReviewSurface};
use super::util::{array_len, string_field};

pub(super) const REPO_EXPOSURE_ARTIFACT: &str = "target/ripr/reports/repo-exposure.json";
pub(super) const OPERATOR_COCKPIT_ARTIFACT: &str = "target/ripr/reports/operator-cockpit.json";
pub(super) const OPERATOR_COCKPIT_MARKDOWN_ARTIFACT: &str =
    "target/ripr/reports/operator-cockpit.md";
pub(super) const LSP_COCKPIT_ARTIFACT: &str = "target/ripr/reports/lsp-cockpit.json";

#[derive(Clone, Debug)]
pub(super) struct ArtifactRead {
    pub(super) value: Option<Value>,
    pub(super) surface: AgentReviewSurface,
}

pub(super) fn read_json_surface(
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

pub(super) fn agent_status_surface(
    status: &AgentStatusReport,
    root_display: &str,
) -> AgentReviewSurface {
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

pub(super) fn ci_artifacts(root: &Path) -> Vec<AgentReviewArtifact> {
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
