use crate::agent::command_specs::{AgentArtifactRoute, agent_regeneration_command_spec};
use crate::agent::loop_commands::{
    WORKFLOW_AGENT_RECEIPT_ARTIFACT, WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT,
    WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT, WORKFLOW_MANIFEST_ARTIFACT, agent_brief_command,
    agent_packet_command, agent_receipt_command, agent_seam_packets_command, agent_start_command,
    agent_verify_command, bound_root, check_analysis_outcome_command, check_repo_exposure_command,
    display_path, workflow_artifact_path,
};
use crate::app::Mode;
use crate::app::agent_status::artifact_required_by_active_loop;
use crate::app::repair_attempt::{RepairAttemptInventoryEntry, inventory_repair_attempts};
use crate::domain::CommandSpec;
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
    /// Whether the active loop mode requires this artifact. Computed with the
    /// same superseded-artifact rule as `ripr agent status`
    /// (`app::agent_status::artifact_required_by_active_loop`), so the
    /// manifest never claims the legacy loop enforces a repository-global
    /// projection the repair-attempt authority has superseded.
    pub(crate) required: bool,
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
    /// FIX #1617: the typed, direct-execution-safe form of the route where a
    /// producer owns one (regeneration routes for the packet/brief steps);
    /// `None` leaves the step legacy-string-only.
    pub(crate) command_spec: Option<CommandSpec>,
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
    // #3999: copy/paste commands bind the selected root once, here; the
    // manifest's `root` field keeps the invocation spelling.
    let commands = workflow_commands(&bound_root(&root_display), mode, &paths, seam_id);
    let repair_attempt_present = repair_attempt_present(root);
    let artifacts = workflow_artifacts(root, &paths, repair_attempt_present);
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
    analysis_outcome: String,
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
            analysis_outcome: workflow_artifact_path_with_default(
                out_dir,
                "analysis-outcome.json",
                WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT,
            ),
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
        analysis_outcome_command_item(root, mode, paths),
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
        command_spec: None,
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
        command_spec: None,
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
        command_spec: None,
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
        command_spec: regeneration_spec_if_root_relative(
            AgentArtifactRoute::Packet,
            root,
            seam_id,
            &paths.agent_packet,
        ),
    }
}

/// FIX (round-1 review): the typed regeneration spec binds root-relative
/// expected writes; an absolute workflow output directory would produce an
/// invalid spec, so those steps stay legacy-string-only instead.
fn regeneration_spec_if_root_relative(
    route: AgentArtifactRoute,
    root: &str,
    seam_id: &str,
    out_path: &str,
) -> Option<CommandSpec> {
    // FIX (round-3 review): the workflow must never advertise a spec that
    // fails its own validation (an absolute or `..`-traversing output path,
    // for instance) — those steps stay legacy-string-only instead.
    let spec = agent_regeneration_command_spec(route, root, seam_id, out_path);
    spec.validate().ok()?;
    Some(spec)
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
        command_spec: regeneration_spec_if_root_relative(
            AgentArtifactRoute::Brief,
            root,
            seam_id,
            &paths.agent_brief,
        ),
    }
}

fn after_snapshot_command(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "after_snapshot".to_string(),
        command_spec: None,
        artifact: paths.after_snapshot.clone(),
        purpose: "Capture static seam evidence after adding one focused test.".to_string(),
        command: check_repo_exposure_command(root, mode.as_str(), &paths.after_snapshot),
    }
}

fn agent_verify_command_item(root: &str, paths: &AgentWorkflowPaths) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_verify".to_string(),
        command_spec: None,
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

fn analysis_outcome_command_item(
    root: &str,
    mode: &Mode,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "analysis_outcome".to_string(),
        command_spec: None,
        artifact: paths.analysis_outcome.clone(),
        purpose:
            "Capture the producer-backed diff completeness outcome after the focused test change."
                .to_string(),
        command: check_analysis_outcome_command(root, mode.as_str(), &paths.analysis_outcome),
    }
}

fn agent_receipt_command_item(
    root: &str,
    seam_id: &str,
    paths: &AgentWorkflowPaths,
) -> AgentWorkflowCommand {
    AgentWorkflowCommand {
        step: "agent_receipt".to_string(),
        command_spec: None,
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

/// Whether a trusted repair attempt is present, mirroring
/// `app::agent_status`: an attempt is present when the inventory lists at
/// least one valid manifest, and an inventory that cannot be read reports no
/// attempt, exactly as status's `required` flags treat it. While an attempt
/// is present the attempt authority supersedes the repository-global
/// workflow projections (docs/REPAIR_ATTEMPT.md, "Durable location" and
/// "Compatibility outputs").
fn repair_attempt_present(root: &Path) -> bool {
    inventory_repair_attempts(root)
        .map(|entries| {
            entries
                .iter()
                .any(|entry| matches!(entry, RepairAttemptInventoryEntry::Valid(_)))
        })
        .unwrap_or(false)
}

fn workflow_artifacts(
    root: &Path,
    paths: &AgentWorkflowPaths,
    repair_attempt_present: bool,
) -> Vec<AgentWorkflowArtifact> {
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
        (
            "analysis_outcome",
            "analysis outcome",
            &paths.analysis_outcome,
        ),
        ("agent_verify", "agent verify", &paths.agent_verify),
        ("agent_receipt", "agent receipt", &paths.agent_receipt),
    ]
    .into_iter()
    .map(|(name, label, path)| AgentWorkflowArtifact {
        name: name.to_string(),
        label: label.to_string(),
        path: path.to_string(),
        required: artifact_required_by_active_loop(name, repair_attempt_present),
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

    /// FIX (round-3 review): a typed regeneration spec is attached only when
    /// it passes its own validation - root-relative writes, no `..`
    /// traversal; escaping outputs keep the step legacy-string-only.
    #[test]
    fn regeneration_specs_require_valid_root_relative_writes() {
        let good = regeneration_spec_if_root_relative(
            AgentArtifactRoute::Packet,
            ".",
            "seam-a",
            "target/ripr/workflow/agent-packet.json",
        );
        assert!(
            good.is_some(),
            "a root-relative write must keep the typed spec"
        );
        assert!(good.as_ref().is_some_and(|spec| spec.validate().is_ok()));

        let escaping = regeneration_spec_if_root_relative(
            AgentArtifactRoute::Packet,
            ".",
            "seam-a",
            "../outside/agent-packet.json",
        );
        assert!(
            escaping.is_none(),
            "a `..`-traversing write must fall back to legacy-string-only"
        );
    }

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
                    command.command.clone(),
                )
            })
            .collect::<Vec<_>>();
        // Issue #3872: redirect targets anchor at the resolved --root, so the
        // expected commands build from the same builders the manifest uses
        // (the anchor math itself is pinned in loop_commands tests).
        let seam_id = "67fc764ba37d77bd";
        assert_eq!(
            command_rows,
            vec![
                (
                    "workflow_manifest",
                    WORKFLOW_MANIFEST_ARTIFACT,
                    "Regenerate this source-edit-free workflow manifest.",
                    agent_start_command(&bound_root("."), seam_id, "target/ripr/workflow"),
                ),
                (
                    "before_snapshot",
                    "target/ripr/workflow/before.repo-exposure.json",
                    "Capture static seam evidence before editing tests.",
                    check_repo_exposure_command(
                        &bound_root("."),
                        "draft",
                        "target/ripr/workflow/before.repo-exposure.json"
                    ),
                ),
                (
                    "agent_seam_packets",
                    "target/ripr/workflow/agent-seam-packets.json",
                    "Render the full agent seam packet set for reference.",
                    agent_seam_packets_command(
                        &bound_root("."),
                        "draft",
                        "target/ripr/workflow/agent-seam-packets.json"
                    ),
                ),
                (
                    "agent_packet",
                    "target/ripr/workflow/agent-packet.json",
                    "Expand the selected seam into a bounded agent packet.",
                    agent_packet_command(
                        &bound_root("."),
                        seam_id,
                        "target/ripr/workflow/agent-packet.json"
                    ),
                ),
                (
                    "agent_brief",
                    "target/ripr/workflow/agent-brief.json",
                    "Refresh this seam's working-set brief.",
                    agent_brief_command(
                        &bound_root("."),
                        seam_id,
                        "target/ripr/workflow/agent-brief.json"
                    ),
                ),
                (
                    "after_snapshot",
                    "target/ripr/workflow/after.repo-exposure.json",
                    "Capture static seam evidence after adding one focused test.",
                    check_repo_exposure_command(
                        &bound_root("."),
                        "draft",
                        "target/ripr/workflow/after.repo-exposure.json"
                    ),
                ),
                (
                    "analysis_outcome",
                    "target/ripr/workflow/analysis-outcome.json",
                    "Capture the producer-backed diff completeness outcome after the focused test change.",
                    check_analysis_outcome_command(
                        &bound_root("."),
                        "draft",
                        "target/ripr/workflow/analysis-outcome.json"
                    ),
                ),
                (
                    "agent_verify",
                    "target/ripr/workflow/agent-verify.json",
                    "Compare before and after static evidence for the agent loop.",
                    agent_verify_command(
                        &bound_root("."),
                        "target/ripr/workflow/before.repo-exposure.json",
                        "target/ripr/workflow/after.repo-exposure.json",
                        Some("target/ripr/workflow/agent-verify.json"),
                    ),
                ),
                (
                    "agent_receipt",
                    WORKFLOW_AGENT_RECEIPT_ARTIFACT,
                    "Write a review handoff receipt for the selected seam.",
                    agent_receipt_command(
                        &bound_root("."),
                        "target/ripr/workflow/agent-verify.json",
                        seam_id,
                        Some("target/ripr/reports/agent-receipt.json"),
                    ),
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

    /// The artifact `required` flags must claim no more than the active loop
    /// mode enforces (docs/LEARNINGS.md, 2026-07-25 false-confidence gates),
    /// with the same superseded-artifact rule `ripr agent status` reports: no
    /// repair attempt means the legacy artifact loop requires every workflow
    /// artifact; a trusted attempt present means the attempt authority
    /// supersedes the repository-global projections (docs/REPAIR_ATTEMPT.md,
    /// "Durable location" and "Compatibility outputs"), so the manifest must
    /// not claim the legacy loop enforces them.
    #[test]
    fn agent_workflow_artifact_required_flags_follow_the_enforced_loop_mode() -> Result<(), String>
    {
        let root = unique_workflow_test_dir("required-flags");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let seam_id = "67fc764ba37d77bd";

        // No repair attempt: the legacy loop is active and requires every
        // workflow artifact, and the manifest JSON must carry that flag.
        let manifest = build_agent_workflow_manifest(
            &root,
            &root,
            &Mode::Draft,
            Path::new("target/ripr/workflow"),
            seam_id,
            brief_json(),
        )?;
        assert!(
            manifest.artifacts.iter().all(|artifact| artifact.required),
            "legacy loop must require every workflow artifact"
        );

        // A trusted repair attempt: the superseded projections are not
        // required; only the workflow-only `agent_seam_packets` stays
        // required because the attempt authority does not supersede it.
        run_git(&root, &["init"])?;
        run_git(
            &root,
            &["config", "user.email", "ripr-test@example.invalid"],
        )?;
        run_git(&root, &["config", "user.name", "RIPR Test"])?;
        write_file(&root.join("README.md"), "# test\n")?;
        run_git(&root, &["add", "."])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        prepare_attempt_fixture(&root, seam_id)?;

        let manifest = build_agent_workflow_manifest(
            &root,
            &root,
            &Mode::Draft,
            Path::new("target/ripr/workflow"),
            seam_id,
            brief_json(),
        )?;
        for artifact in &manifest.artifacts {
            assert_eq!(
                artifact.required,
                artifact_required_by_active_loop(&artifact.name, true),
                "repair loop classification for `{}` must match the status rule",
                artifact.name
            );
        }
        let seam_packets = manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.name == "agent_seam_packets")
            .ok_or_else(|| "manifest must report agent_seam_packets".to_string())?;
        assert!(
            seam_packets.required,
            "agent_seam_packets is not superseded by the repair-attempt authority"
        );
        assert!(
            manifest
                .artifacts
                .iter()
                .filter(|artifact| !artifact.required)
                .count()
                == crate::app::agent_status::REPAIR_ATTEMPT_SUPERSEDED_ARTIFACTS.len(),
            "exactly the superseded projections may be marked not required"
        );

        // The machine-readable contract renders the producer's flags.
        let rendered = crate::output::agent_workflow::render_agent_workflow_json(&manifest)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse workflow JSON: {err}"))?;
        for artifact in value["artifacts"]
            .as_array()
            .ok_or_else(|| "workflow JSON must carry an artifacts array".to_string())?
        {
            let name = artifact["name"]
                .as_str()
                .ok_or_else(|| "artifact must carry a name".to_string())?;
            assert_eq!(
                artifact["required"],
                crate::app::agent_status::artifact_required_by_active_loop(name, true),
                "rendered flag for `{name}` must follow the enforced loop mode"
            );
        }

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// Every workflow artifact must carry an explicit classification for the
    /// repair-attempt loop mode, so a newly added artifact cannot silently
    /// inherit an all-`true` or all-`false` claim.
    #[test]
    fn agent_workflow_every_artifact_has_a_loop_mode_classification() {
        let paths = AgentWorkflowPaths::new(Path::new("target/ripr/workflow"));
        let artifacts = workflow_artifacts(Path::new("."), &paths, true);
        let mut not_required = artifacts
            .iter()
            .filter(|artifact| !artifact.required)
            .map(|artifact| artifact.name.as_str())
            .collect::<Vec<_>>();
        not_required.sort_unstable();
        let mut expected = crate::app::agent_status::REPAIR_ATTEMPT_SUPERSEDED_ARTIFACTS.to_vec();
        expected.sort_unstable();
        assert_eq!(
            not_required, expected,
            "each workflow artifact needs an explicit active-loop classification"
        );
    }

    fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
        crate::testing::fixture_git::fixture_git_ok(root, args)
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

    /// Publishes one real repair attempt the way the before phase does, so
    /// the manifest reads a trusted attempt directory rather than a synthetic
    /// one. Mirrors `agent_status`'s fixture: both surfaces must agree on
    /// what the active loop mode is.
    fn prepare_attempt_fixture(root: &Path, seam_id: &str) -> Result<(), String> {
        use crate::app::repair_attempt::{
            BeforeArtifactSource, BeginRepairAttemptOptions, begin_repair_attempt_with,
            edit_cage_policy_from_packet, write_edit_cage_baseline,
        };
        let workflow = root.join("target/ripr/workflow");
        std::fs::create_dir_all(&workflow)
            .map_err(|err| format!("create {}: {err}", workflow.display()))?;
        let before = workflow.join("before-workflow-honesty.json");
        let packet = workflow.join("packet-workflow-honesty.json");
        let baseline = workflow.join("baseline-workflow-honesty.json");
        write_file(&before, "{}")?;
        let packet_text = serde_json::json!({
            "seam_id": seam_id,
            "allowed_edit_surface": ["tests/target.rs"],
            "forbidden_files": []
        })
        .to_string();
        write_file(&packet, &packet_text)?;
        let policy = edit_cage_policy_from_packet(&packet_text, seam_id)?;
        write_edit_cage_baseline(root, &baseline, &policy)?;
        begin_repair_attempt_with(BeginRepairAttemptOptions {
            root,
            root_argument: root,
            seam_id,
            sources: &[
                BeforeArtifactSource {
                    role: "before_snapshot",
                    path: &before,
                },
                BeforeArtifactSource {
                    role: "agent_packet",
                    path: &packet,
                },
                BeforeArtifactSource {
                    role: "edit_cage_baseline",
                    path: &baseline,
                },
            ],
            expected_repository_head: None,
            next_command_suffix: None,
            store: None,
        })?;
        Ok(())
    }
}
