use crate::app::agent_workflow::{
    AGENT_WORKFLOW_SCHEMA_VERSION, AgentWorkflowArtifact, AgentWorkflowCommand,
    AgentWorkflowManifest, AgentWorkflowSeam,
};
use serde_json::{Value, json};

pub(crate) fn render_agent_workflow_json(
    manifest: &AgentWorkflowManifest,
) -> Result<String, String> {
    let value = json!({
        "schema_version": AGENT_WORKFLOW_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "ready",
        "root": manifest.root,
        "mode": manifest.mode,
        "out_dir": manifest.out_dir,
        "seam": seam_json(&manifest.seam),
        "outputs": {
            "workflow_manifest": manifest.outputs.workflow_manifest,
            "commands_markdown": manifest.outputs.commands_markdown,
            "agent_brief": manifest.outputs.agent_brief,
        },
        "artifacts": manifest.artifacts.iter().map(artifact_json).collect::<Vec<_>>(),
        "commands": manifest.commands.iter().map(command_json).collect::<Vec<_>>(),
        "missing_inputs": manifest.missing_inputs.iter().map(command_json).collect::<Vec<_>>(),
        "next_command": manifest.missing_inputs.first().map(command_json),
        "boundaries": {
            "source_edits": false,
            "generated_tests": false,
            "runtime_mutation_execution": false,
            "llm_api_calls": false,
            "ci_blocking": false,
        },
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent workflow JSON: {err}"))
}

pub(crate) fn render_agent_workflow_commands_md(manifest: &AgentWorkflowManifest) -> String {
    let mut lines = Vec::new();
    lines.push("# RIPR Agent Workflow".to_string());
    lines.push(String::new());
    lines.push("This workflow packet is advisory and source-edit-free. It gives a human or agent the static context and commands for one focused test loop.".to_string());
    lines.push(String::new());
    lines.push("## Seam".to_string());
    lines.push(String::new());
    lines.push(format!("- Seam ID: `{}`", manifest.seam.seam_id));
    if let (Some(file), Some(line)) = (&manifest.seam.file, manifest.seam.line) {
        lines.push(format!("- Location: `{file}:{line}`"));
    }
    if let Some(kind) = &manifest.seam.seam_kind {
        lines.push(format!("- Kind: `{kind}`"));
    }
    if let Some(class) = &manifest.seam.grip_class {
        lines.push(format!("- Grip class: `{class}`"));
    }
    if let Some(why) = &manifest.seam.why {
        lines.push(format!("- Why now: {why}"));
    }
    if let Some(discriminator) = &manifest.seam.missing_discriminator {
        lines.push(format!("- Missing discriminator: `{discriminator}`"));
    }
    if let Some(assertion) = &manifest.seam.assertion_shape {
        lines.push(format!("- Assertion shape: `{assertion}`"));
    }
    if let Some(file) = &manifest.seam.recommended_test_file {
        lines.push(format!("- Recommended test file: `{file}`"));
    }
    if let Some(test) = &manifest.seam.related_test_to_imitate {
        lines.push(format!("- Imitate: `{test}`"));
    }
    lines.push(String::new());
    lines.push("## Commands".to_string());
    lines.push(String::new());
    for command in &manifest.commands {
        lines.push(format!("### {}", command_label(&command.step)));
        lines.push(String::new());
        lines.push(command.purpose.clone());
        lines.push(String::new());
        lines.push("```bash".to_string());
        lines.push(command.command.clone());
        lines.push("```".to_string());
        lines.push(String::new());
    }
    lines.push("## Missing Inputs".to_string());
    lines.push(String::new());
    if manifest.missing_inputs.is_empty() {
        lines.push("All workflow command artifacts are present.".to_string());
    } else {
        for command in &manifest.missing_inputs {
            lines.push(format!(
                "- `{}` is missing; run `{}`",
                command.artifact, command.command
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Boundaries".to_string());
    lines.push(String::new());
    lines.push("- Does not edit source files.".to_string());
    lines.push("- Does not generate tests.".to_string());
    lines.push("- Does not run mutation testing.".to_string());
    lines.push("- Does not call an LLM API.".to_string());
    lines.push("- Does not configure CI blocking.".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn seam_json(seam: &AgentWorkflowSeam) -> Value {
    json!({
        "seam_id": seam.seam_id,
        "file": seam.file,
        "line": seam.line,
        "seam_kind": seam.seam_kind,
        "grip_class": seam.grip_class,
        "why": seam.why,
        "missing_discriminator": seam.missing_discriminator,
        "assertion_shape": seam.assertion_shape,
        "recommended_test_file": seam.recommended_test_file,
        "recommended_test_name": seam.recommended_test_name,
        "related_test_to_imitate": seam.related_test_to_imitate,
    })
}

fn artifact_json(artifact: &AgentWorkflowArtifact) -> Value {
    json!({
        "name": artifact.name,
        "label": artifact.label,
        "path": artifact.path,
        "required": true,
        "state": artifact.state.as_str(),
    })
}

fn command_json(command: &AgentWorkflowCommand) -> Value {
    json!({
        "step": command.step,
        "artifact": command.artifact,
        "purpose": command.purpose,
        "command": command.command,
    })
}

fn command_label(step: &str) -> String {
    step.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::agent_workflow::{
        AgentWorkflowArtifactState, AgentWorkflowOutputs, AgentWorkflowSeam,
    };

    fn manifest() -> AgentWorkflowManifest {
        AgentWorkflowManifest {
            root: ".".to_string(),
            mode: "draft".to_string(),
            out_dir: "target/ripr/workflow".to_string(),
            seam: AgentWorkflowSeam {
                seam_id: "67fc764ba37d77bd".to_string(),
                file: Some("src/pricing.rs".to_string()),
                line: Some(88),
                seam_kind: Some("predicate_boundary".to_string()),
                grip_class: Some("weakly_gripped".to_string()),
                why: Some("changed owner function".to_string()),
                missing_discriminator: Some("amount == discount_threshold".to_string()),
                assertion_shape: Some("assert_eq!(...)".to_string()),
                recommended_test_file: Some("tests/pricing.rs".to_string()),
                recommended_test_name: Some(
                    "discount_threshold_equality_boundary_is_asserted".to_string(),
                ),
                related_test_to_imitate: Some("applies_discount_above_threshold".to_string()),
            },
            outputs: AgentWorkflowOutputs {
                workflow_manifest: "target/ripr/workflow/workflow.json".to_string(),
                commands_markdown: "target/ripr/workflow/commands.md".to_string(),
                agent_brief: "target/ripr/workflow/agent-brief.json".to_string(),
            },
            artifacts: vec![AgentWorkflowArtifact {
                name: "before_snapshot".to_string(),
                label: "before snapshot".to_string(),
                path: "target/ripr/workflow/before.repo-exposure.json".to_string(),
                state: AgentWorkflowArtifactState::Missing,
            }],
            commands: vec![AgentWorkflowCommand {
                step: "before_snapshot".to_string(),
                artifact: "target/ripr/workflow/before.repo-exposure.json".to_string(),
                purpose: "Capture static seam evidence before editing tests.".to_string(),
                command: "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json".to_string(),
            }],
            missing_inputs: vec![AgentWorkflowCommand {
                step: "before_snapshot".to_string(),
                artifact: "target/ripr/workflow/before.repo-exposure.json".to_string(),
                purpose: "Capture static seam evidence before editing tests.".to_string(),
                command: "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json".to_string(),
            }],
        }
    }

    #[test]
    fn workflow_json_is_structured_and_advisory() -> Result<(), String> {
        let rendered = render_agent_workflow_json(&manifest())?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        assert_eq!(value["schema_version"], AGENT_WORKFLOW_SCHEMA_VERSION);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["seam"]["seam_id"], "67fc764ba37d77bd");
        assert_eq!(value["boundaries"]["source_edits"], false);
        assert_eq!(
            value["next_command"]["command"],
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );
        Ok(())
    }

    #[test]
    fn workflow_markdown_lists_commands_and_boundaries() {
        let rendered = render_agent_workflow_commands_md(&manifest());

        assert!(rendered.contains("# RIPR Agent Workflow"));
        assert!(rendered.contains("Missing discriminator"));
        assert!(rendered.contains("ripr check --root . --mode draft"));
        assert!(rendered.contains("Does not edit source files."));
        assert!(rendered.contains("Does not call an LLM API."));
    }
}
