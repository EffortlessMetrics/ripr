use crate::app::agent_workflow::{
    AGENT_WORKFLOW_SCHEMA_VERSION, AgentWorkflowArtifact, AgentWorkflowCommand,
    AgentWorkflowManifest, AgentWorkflowSeam,
};
use serde_json::{Value, json};

/// Shell that every `command` string in this packet is written for.
///
/// The command strings are built with POSIX single-quote escaping and `>`
/// redirection (`agent::loop_commands::shell_arg`), so they are bash source,
/// not shell-neutral text. Naming the shell keeps the packet honest on Windows,
/// where cmd.exe treats `'` as a literal character and PowerShell rejects the
/// `'\''` escape. Emitting an argv form for other shells is out of scope here.
const COMMAND_SHELL: &str = "bash";

pub(crate) fn render_agent_workflow_json(
    manifest: &AgentWorkflowManifest,
) -> Result<String, String> {
    let value = json!({
        "schema_version": AGENT_WORKFLOW_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "ready",
        "command_shell": COMMAND_SHELL,
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
    super::json::render_pretty_with_newline(&value, "agent workflow")
}

pub(crate) fn render_agent_workflow_commands_md(manifest: &AgentWorkflowManifest) -> String {
    markdown::render_commands_document(manifest)
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

mod markdown {
    use super::{AgentWorkflowManifest, command_label};

    pub(super) fn render_commands_document(manifest: &AgentWorkflowManifest) -> String {
        let mut lines = Vec::new();
        push_header(&mut lines);
        push_seam_section(&mut lines, manifest);
        push_commands_section(&mut lines, manifest);
        push_missing_inputs_section(&mut lines, manifest);
        push_boundaries_section(&mut lines);
        lines.join("\n")
    }

    fn push_header(lines: &mut Vec<String>) {
        lines.push("# RIPR Agent Workflow".to_string());
        lines.push(String::new());
        lines.push("This workflow packet is advisory and source-edit-free. It gives a human or agent the static context and commands for one focused test loop.".to_string());
        lines.push(String::new());
        lines.push("Generated commands are bash command lines. They use POSIX single-quote quoting and `>` redirection, so run them from bash — on Windows, Git Bash. cmd.exe and PowerShell do not interpret this quoting the same way and will mis-pass or reject the quoted arguments. WSL bash is not a drop-in substitute: paths here keep their Windows drive-letter prefix, which WSL resolves as a relative path, so running them there requires rewriting each path under `/mnt/` and having ripr available inside WSL.".to_string());
        lines.push(String::new());
    }

    fn push_seam_section(lines: &mut Vec<String>, manifest: &AgentWorkflowManifest) {
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
    }

    fn push_commands_section(lines: &mut Vec<String>, manifest: &AgentWorkflowManifest) {
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
    }

    fn push_missing_inputs_section(lines: &mut Vec<String>, manifest: &AgentWorkflowManifest) {
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
    }

    fn push_boundaries_section(lines: &mut Vec<String>) {
        lines.push("## Boundaries".to_string());
        lines.push(String::new());
        lines.push("- Does not edit source files.".to_string());
        lines.push("- Does not generate tests.".to_string());
        lines.push("- Does not run mutation testing.".to_string());
        lines.push("- Does not call an LLM API.".to_string());
        lines.push("- Does not configure CI blocking.".to_string());
        lines.push(String::new());
    }
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
    fn workflow_markdown_discloses_the_bash_assumption_before_the_first_command_fence()
    -> Result<(), String> {
        let rendered = render_agent_workflow_commands_md(&manifest());

        // A bare `bash` substring proves nothing here: every command block is
        // already fenced as ```bash. The disclosure must be prose that reaches
        // the reader before the first copyable command.
        // Pinned literally rather than shared with the renderer: a constant
        // imported from production would make this test agree with whatever the
        // renderer happens to emit.
        let disclosure = rendered
            .find("Generated commands are bash command lines.")
            .ok_or_else(|| format!("commands.md must disclose the bash assumption: {rendered}"))?;
        let first_fence = rendered
            .find("```bash")
            .ok_or_else(|| "commands.md must still fence commands as bash".to_string())?;
        assert!(
            disclosure < first_fence,
            "bash disclosure at {disclosure} must precede the first command fence at {first_fence}"
        );
        assert!(
            rendered.contains("PowerShell"),
            "disclosure must name the shells that do not accept these commands: {rendered}"
        );
        Ok(())
    }

    /// `display_path` (`agent::loop_commands`) only swaps `\` for `/`, so an
    /// absolute Windows root keeps its drive-letter prefix. Git Bash resolves that; WSL bash
    /// reads it as a path relative to the current directory and the `>`
    /// redirection fails before ripr runs. Recommending "bash on Windows"
    /// without that distinction sends the one affected reader to an environment
    /// where the copied command still breaks, so the caveat is load-bearing and
    /// is pinned here rather than left to review.
    #[test]
    fn workflow_markdown_does_not_offer_wsl_as_an_unqualified_windows_shell() -> Result<(), String>
    {
        let rendered = render_agent_workflow_commands_md(&manifest());

        let wsl = rendered
            .find("WSL")
            .ok_or_else(|| format!("disclosure must address WSL explicitly: {rendered}"))?;
        let git_bash = rendered
            .find("Git Bash")
            .ok_or_else(|| format!("disclosure must name Git Bash: {rendered}"))?;
        assert!(
            git_bash < wsl,
            "Git Bash must be the recommendation the reader meets first, \
             with WSL qualified afterwards: {rendered}"
        );
        assert!(
            rendered.contains("/mnt/"),
            "the WSL caveat must name the translation a reader has to perform, \
             not merely discourage it: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn workflow_json_declares_bash_as_the_command_shell() -> Result<(), String> {
        let rendered = render_agent_workflow_json(&manifest())?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        // Machine consumers never read commands.md, so the manifest must carry
        // the same boundary the Markdown header states.
        assert_eq!(
            value["command_shell"], "bash",
            "workflow manifest must name the shell its command strings assume: {rendered}"
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
