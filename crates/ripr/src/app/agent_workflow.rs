use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT,
    WORKFLOW_MANIFEST_ARTIFACT, agent_brief_command, agent_packet_command, agent_receipt_command,
    agent_seam_packets_command, agent_start_command, agent_verify_command,
    check_repo_exposure_command, display_path, workflow_artifact_path,
};
use crate::app::Mode;
use serde_json::Value;
use std::path::Path;

pub(crate) const AGENT_WORKFLOW_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowManifest {
    pub(crate) root: String,
    pub(crate) mode: String,
    pub(crate) out_dir: String,
    pub(crate) seam: AgentWorkflowSeam,
    pub(crate) outputs: AgentWorkflowOutputs,
    pub(crate) artifacts: Vec<AgentWorkflowArtifact>,
    pub(crate) commands: Vec<AgentWorkflowCommand>,
    pub(crate) missing_inputs: Vec<AgentWorkflowCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowSeam {
    pub(crate) seam_id: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u64>,
    pub(crate) seam_kind: Option<String>,
    pub(crate) grip_class: Option<String>,
    pub(crate) why: Option<String>,
    pub(crate) missing_discriminator: Option<String>,
    pub(crate) assertion_shape: Option<String>,
    pub(crate) recommended_test_file: Option<String>,
    pub(crate) recommended_test_name: Option<String>,
    pub(crate) related_test_to_imitate: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowOutputs {
    pub(crate) workflow_manifest: String,
    pub(crate) commands_markdown: String,
    pub(crate) agent_brief: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowArtifact {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) state: AgentWorkflowArtifactState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentWorkflowArtifactState {
    Present,
    Missing,
}

impl AgentWorkflowArtifactState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowCommand {
    pub(crate) step: String,
    pub(crate) artifact: String,
    pub(crate) purpose: String,
    pub(crate) command: String,
}

pub(crate) fn build_agent_workflow_manifest(
    root: &Path,
    root_argument: &Path,
    mode: &Mode,
    out_dir: &Path,
    seam_id: &str,
    agent_brief_json: &str,
) -> Result<AgentWorkflowManifest, String> {
    let root_display = display_path(root_argument);
    let out_display = display_path(out_dir);
    let paths = AgentWorkflowPaths::new(out_dir);
    let seam = workflow_seam_from_brief(agent_brief_json, seam_id)?;
    let commands = workflow_commands(&root_display, mode, &paths, seam_id);
    let artifacts = workflow_artifacts(root, &paths);
    let missing_inputs = commands
        .iter()
        .filter(|command| {
            artifacts
                .iter()
                .find(|artifact| artifact.path == command.artifact)
                .map(|artifact| artifact.state == AgentWorkflowArtifactState::Missing)
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    Ok(AgentWorkflowManifest {
        root: root_display,
        mode: mode.as_str().to_string(),
        out_dir: out_display,
        seam,
        outputs: AgentWorkflowOutputs {
            workflow_manifest: paths.workflow_manifest,
            commands_markdown: paths.commands_markdown,
            agent_brief: paths.agent_brief,
        },
        artifacts,
        commands,
        missing_inputs,
    })
}

struct AgentWorkflowPaths {
    out_dir: String,
    workflow_manifest: String,
    commands_markdown: String,
    before_snapshot: String,
    after_snapshot: String,
    agent_seam_packets: String,
    agent_packet: String,
    agent_brief: String,
    agent_verify: String,
    agent_receipt: String,
}

impl AgentWorkflowPaths {
    fn new(out_dir: &Path) -> Self {
        Self {
            out_dir: display_path(out_dir),
            workflow_manifest: workflow_artifact_path_with_default(
                out_dir,
                "workflow.json",
                WORKFLOW_MANIFEST_ARTIFACT,
            ),
            commands_markdown: workflow_artifact_path_with_default(
                out_dir,
                "commands.md",
                WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT,
            ),
            before_snapshot: workflow_artifact_path(out_dir, "before.repo-exposure.json"),
            after_snapshot: workflow_artifact_path(out_dir, "after.repo-exposure.json"),
            agent_seam_packets: workflow_artifact_path(out_dir, "agent-seam-packets.json"),
            agent_packet: workflow_artifact_path(out_dir, "agent-packet.json"),
            agent_brief: workflow_artifact_path(out_dir, "agent-brief.json"),
            agent_verify: workflow_artifact_path(out_dir, "agent-verify.json"),
            agent_receipt: WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string(),
        }
    }
}

fn workflow_commands(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
    seam_id: &str,
) -> Vec<AgentWorkflowCommand> {
    vec![
        workflow_manifest_command(root, seam_id, paths),
        before_snapshot_command(root, mode, paths),
        agent_seam_packets_command_item(root, mode, paths),
        agent_packet_command_item(root, seam_id, paths),
        agent_brief_command_item(root, seam_id, paths),
        after_snapshot_command(root, mode, paths),
        agent_verify_command_item(root, paths),
        agent_receipt_command_item(root, seam_id, paths),
    ]
}

fn workflow_manifest_command(
    root: &str,
    seam_id: &str,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "workflow_manifest".to_string(),
        artifact: paths.workflow_manifest.clone(),
        purpose: "Regenerate this source-edit-free workflow manifest.".to_string(),
        command: agent_start_command(root, seam_id, &paths.out_dir),
    }
}

fn before_snapshot_command(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "before_snapshot".to_string(),
        artifact: paths.before_snapshot.clone(),
        purpose: "Capture static seam evidence before editing tests.".to_string(),
        command: check_repo_exposure_command(root, mode.as_str(), &paths.before_snapshot),
    }
}

fn agent_seam_packets_command_item(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_seam_packets".to_string(),
        artifact: paths.agent_seam_packets.clone(),
        purpose: "Render the full agent seam packet set for reference.".to_string(),
        command: agent_seam_packets_command(root, mode.as_str(), &paths.agent_seam_packets),
    }
}

fn agent_packet_command_item(
    root: &str,
    seam_id: &str,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_packet".to_string(),
        artifact: paths.agent_packet.clone(),
        purpose: "Expand the selected seam into a bounded agent packet.".to_string(),
        command: agent_packet_command(root, seam_id, &paths.agent_packet),
    }
}

fn agent_brief_command_item(
    root: &str,
    seam_id: &str,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_brief".to_string(),
        artifact: paths.agent_brief.clone(),
        purpose: "Refresh this seam's working-set brief.".to_string(),
        command: agent_brief_command(root, seam_id, &paths.agent_brief),
    }
}

fn after_snapshot_command(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "after_snapshot".to_string(),
        artifact: paths.after_snapshot.clone(),
        purpose: "Capture static seam evidence after adding one focused test.".to_string(),
        command: check_repo_exposure_command(root, mode.as_str(), &paths.after_snapshot),
    }
}

fn agent_verify_command_item(root: &str, paths: &AgentWorkflowPaths) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_verify".to_string(),
        artifact: paths.agent_verify.clone(),
        purpose: "Compare before and after static evidence for the agent loop.".to_string(),
        command: agent_verify_command(
            root,
            &paths.before_snapshot,
            &paths.after_snapshot,
            Some(&paths.agent_verify),
        ),
    }
}

fn agent_receipt_command_item(
    root: &str,
    seam_id: &str,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_receipt".to_string(),
        artifact: paths.agent_receipt.clone(),
        purpose: "Write a review handoff receipt for the selected seam.".to_string(),
        command: agent_receipt_command(
            root,
            &paths.agent_verify,
            seam_id,
            Some(&paths.agent_receipt),
        ),
    }
}

fn workflow_artifact_path_with_default(
    out_dir: &Path,
    file_name: &str,
    default_path: &str,
) -> String {
    if out_dir == Path::new("target/ripr/workflow") {
        default_path.to_string()
    } else {
        workflow_artifact_path(out_dir, file_name)
    }
}

fn workflow_artifacts(root: &Path, paths: &AgentWorkflowPaths) -> Vec<AgentWorkflowArtifact> {
    [
        ("before_snapshot", "before snapshot", &paths.before_snapshot),
        (
            "agent_seam_packets",
            "agent seam packets",
            &paths.agent_seam_packets,
        ),
        ("agent_packet", "agent packet", &paths.agent_packet),
        ("agent_brief", "agent brief", &paths.agent_brief),
        ("after_snapshot", "after snapshot", &paths.after_snapshot),
        ("agent_verify", "agent verify", &paths.agent_verify),
        ("agent_receipt", "agent receipt", &paths.agent_receipt),
    ]
    .into_iter()
    .map(|(name, label, path)| AgentWorkflowArtifact {
        name: name.to_string(),
        label: label.to_string(),
        path: path.to_string(),
        state: if root.join(path).is_file() {
            AgentWorkflowArtifactState::Present
        } else {
            AgentWorkflowArtifactState::Missing
        },
    })
    .collect()
}

fn workflow_seam_from_brief(
    agent_brief_json: &str,
    requested_seam_id: &str,
) -> Result<AgentWorkflowSeam, String> {
    let value: Value = serde_json::from_str(agent_brief_json)
        .map_err(|err| format!("failed to parse generated agent brief JSON: {err}"))?;
    let top_seams = value
        .get("top_seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "generated agent brief JSON is missing top_seams array".to_string())?;
    let seam = top_seams
        .iter()
        .find(|seam| string_field(seam, "seam_id").as_deref() == Some(requested_seam_id))
        .ok_or_else(|| {
            format!("agent start seam_id {requested_seam_id} was not returned by agent brief")
        })?;

    Ok(AgentWorkflowSeam {
        seam_id: requested_seam_id.to_string(),
        file: string_field(seam, "file"),
        line: seam.get("line").and_then(Value::as_u64),
        seam_kind: string_field(seam, "seam_kind"),
        grip_class: string_field(seam, "grip_class"),
        why: seam
            .get("why_now")
            .and_then(|why_now| string_field(why_now, "evidence")),
        missing_discriminator: first_nested_string(seam, "missing_discriminators", "value"),
        assertion_shape: seam
            .get("assertion_shape")
            .and_then(|shape| string_field(shape, "example")),
        recommended_test_file: seam
            .get("recommended_test")
            .and_then(|test| string_field(test, "file")),
        recommended_test_name: seam
            .get("recommended_test")
            .and_then(|test| string_field(test, "name")),
        related_test_to_imitate: seam
            .get("nearest_strong_test_to_imitate")
            .and_then(|test| string_field(test, "name")),
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn first_nested_string(value: &Value, array_key: &str, field: &str) -> Option<String> {
    value
        .get(array_key)
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| string_field(item, field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_workflow_test_dir(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-agent-workflow-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn brief_json() -> &'static str {
        r#"{
  "top_seams": [
    {
      "seam_id": "67fc764ba37d77bd",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "grip_class": "weakly_gripped",
      "why_now": {"evidence": "changed owner function"},
      "missing_discriminators": [{"value": "amount == discount_threshold"}],
      "assertion_shape": {"example": "assert_eq!(...)"},
      "recommended_test": {
        "file": "tests/pricing.rs",
        "name": "discount_threshold_equality_boundary_is_asserted"
      },
      "nearest_strong_test_to_imitate": {
        "name": "applies_discount_above_threshold"
      }
    }
  ]
}"#
    }

    #[test]
    fn workflow_manifest_extracts_seam_and_commands() -> Result<(), String> {
        let root = unique_workflow_test_dir("manifest");
        let out_dir = root.join("target/ripr/workflow");
        std::fs::create_dir_all(&out_dir).map_err(|err| format!("create out dir: {err}"))?;
        std::fs::write(out_dir.join("agent-brief.json"), brief_json())
            .map_err(|err| format!("write brief: {err}"))?;

        let manifest = build_agent_workflow_manifest(
            &root,
            Path::new("."),
            &Mode::Draft,
            Path::new("target/ripr/workflow"),
            "67fc764ba37d77bd",
            brief_json(),
        )?;

        assert_eq!(manifest.seam.file.as_deref(), Some("src/pricing.rs"));
        assert_eq!(
            manifest.seam.missing_discriminator.as_deref(),
            Some("amount == discount_threshold")
        );
        let command_rows = manifest
            .commands
            .iter()
            .map(|command| {
                (
                    command.step.as_str(),
                    command.artifact.as_str(),
                    command.purpose.as_str(),
                    command.command.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            command_rows,
            vec![
                (
                    "workflow_manifest",
                    WORKFLOW_MANIFEST_ARTIFACT,
                    "Regenerate this source-edit-free workflow manifest.",
                    "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow",
                ),
                (
                    "before_snapshot",
                    "target/ripr/workflow/before.repo-exposure.json",
                    "Capture static seam evidence before editing tests.",
                    "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json",
                ),
                (
                    "agent_seam_packets",
                    "target/ripr/workflow/agent-seam-packets.json",
                    "Render the full agent seam packet set for reference.",
                    "ripr check --root . --mode draft --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json",
                ),
                (
                    "agent_packet",
                    "target/ripr/workflow/agent-packet.json",
                    "Expand the selected seam into a bounded agent packet.",
                    "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json",
                ),
                (
                    "agent_brief",
                    "target/ripr/workflow/agent-brief.json",
                    "Refresh this seam's working-set brief.",
                    "ripr agent brief --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-brief.json",
                ),
                (
                    "after_snapshot",
                    "target/ripr/workflow/after.repo-exposure.json",
                    "Capture static seam evidence after adding one focused test.",
                    "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
                ),
                (
                    "agent_verify",
                    "target/ripr/workflow/agent-verify.json",
                    "Compare before and after static evidence for the agent loop.",
                    "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json",
                ),
                (
                    "agent_receipt",
                    WORKFLOW_AGENT_RECEIPT_ARTIFACT,
                    "Write a review handoff receipt for the selected seam.",
                    "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/reports/agent-receipt.json",
                ),
            ]
        );
        assert!(manifest.artifacts.iter().any(|artifact| {
            artifact.name == "agent_brief" && artifact.state == AgentWorkflowArtifactState::Present
        }));
        assert!(
            manifest
                .missing_inputs
                .iter()
                .any(|command| { command.step == "before_snapshot" })
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn workflow_manifest_errors_when_brief_does_not_return_seam() -> Result<(), String> {
        let result = build_agent_workflow_manifest(
            Path::new("."),
            Path::new("."),
            &Mode::Draft,
            Path::new("target/ripr/workflow"),
            "missing",
            brief_json(),
        );
        let err = match result {
            Ok(_) => return Err("workflow manifest should reject missing seam".to_string()),
            Err(err) => err,
        };

        assert!(err.contains("was not returned by agent brief"));
        Ok(())
    }
}
