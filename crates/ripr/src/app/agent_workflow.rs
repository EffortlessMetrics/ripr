use crate::agent::loop_commands::{
    AgentWorkflowArtifacts, AgentWorkflowCommands, WORKFLOW_AGENT_WORKFLOW_JSON_FILE,
    WORKFLOW_AGENT_WORKFLOW_MARKDOWN_FILE, agent_workflow_artifacts, agent_workflow_commands,
    display_path,
};
use crate::app::Mode;
use serde_json::Value;
use std::path::Path;

pub(crate) const AGENT_WORKFLOW_SCHEMA_VERSION: &str = "0.1";
pub(crate) const AGENT_WORKFLOW_JSON_FILE: &str = WORKFLOW_AGENT_WORKFLOW_JSON_FILE;
pub(crate) const AGENT_WORKFLOW_MARKDOWN_FILE: &str = WORKFLOW_AGENT_WORKFLOW_MARKDOWN_FILE;

const WORKFLOW_NOTES: &[&str] = &[
    "This manifest does not edit source files.",
    "Static evidence only; no runtime mutation execution.",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowReport {
    pub(crate) root: String,
    pub(crate) mode: String,
    pub(crate) seam_id: String,
    pub(crate) artifacts: AgentWorkflowArtifacts,
    pub(crate) commands: AgentWorkflowCommands,
    pub(crate) next_step: String,
    pub(crate) notes: Vec<String>,
}

pub(crate) fn build_agent_workflow_report(
    root_argument: &Path,
    mode: &Mode,
    seam_id: &str,
    out_dir: &Path,
) -> AgentWorkflowReport {
    let root = display_path(root_argument);
    let mode = mode.as_str().to_string();
    let artifacts = agent_workflow_artifacts(out_dir);
    let commands = agent_workflow_commands(&root, &mode, seam_id, &artifacts);

    AgentWorkflowReport {
        root,
        mode,
        seam_id: seam_id.to_string(),
        artifacts,
        commands,
        next_step: "before_snapshot".to_string(),
        notes: WORKFLOW_NOTES
            .iter()
            .map(|note| (*note).to_string())
            .collect(),
    }
}

pub(crate) fn render_agent_workflow_json(report: &AgentWorkflowReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": AGENT_WORKFLOW_SCHEMA_VERSION,
        "tool": "ripr",
        "root": report.root,
        "mode": report.mode,
        "seam_id": report.seam_id,
        "artifacts": artifacts_json(&report.artifacts),
        "commands": commands_json(&report.commands),
        "next_step": report.next_step,
        "notes": report.notes
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent workflow JSON: {err}"))
}

pub(crate) fn render_agent_workflow_markdown(report: &AgentWorkflowReport) -> String {
    let mut rendered = String::new();
    rendered.push_str("# RIPR Agent Workflow\n\n");
    rendered.push_str(&format!("Target seam: `{}`\n", report.seam_id));
    rendered.push_str(&format!("Root: `{}`\n", report.root));
    rendered.push_str(&format!("Mode: `{}`\n\n", report.mode));

    rendered.push_str("## Steps\n\n");
    rendered.push_str("1. Capture before snapshot\n");
    rendered.push_str("2. Generate packet\n");
    rendered.push_str("3. Generate brief\n");
    rendered.push_str("4. Write one focused test\n");
    rendered.push_str("5. Capture after snapshot\n");
    rendered.push_str("6. Run verify\n");
    rendered.push_str("7. Emit receipt\n");
    rendered.push_str("8. Run status / review summary\n\n");

    rendered.push_str("## Commands\n\n");
    push_command(
        &mut rendered,
        "Before snapshot",
        &report.commands.before_snapshot,
    );
    push_command(&mut rendered, "Agent packet", &report.commands.agent_packet);
    push_command(&mut rendered, "Agent brief", &report.commands.agent_brief);
    push_command(
        &mut rendered,
        "After snapshot",
        &report.commands.after_snapshot,
    );
    push_command(&mut rendered, "Agent verify", &report.commands.agent_verify);
    push_command(
        &mut rendered,
        "Agent receipt",
        &report.commands.agent_receipt,
    );
    push_command(&mut rendered, "Agent status", &report.commands.agent_status);
    push_command(
        &mut rendered,
        "Review summary",
        &report.commands.review_summary,
    );

    rendered.push_str("## Notes\n\n");
    for note in &report.notes {
        rendered.push_str("- ");
        rendered.push_str(note);
        rendered.push('\n');
    }
    rendered
}

fn artifacts_json(artifacts: &AgentWorkflowArtifacts) -> Value {
    serde_json::json!({
        "before_snapshot": artifacts.before_snapshot,
        "after_snapshot": artifacts.after_snapshot,
        "agent_packet": artifacts.agent_packet,
        "agent_brief": artifacts.agent_brief,
        "agent_verify": artifacts.agent_verify,
        "agent_receipt": artifacts.agent_receipt,
        "agent_status": artifacts.agent_status,
        "review_summary": artifacts.review_summary
    })
}

fn commands_json(commands: &AgentWorkflowCommands) -> Value {
    serde_json::json!({
        "before_snapshot": commands.before_snapshot,
        "agent_packet": commands.agent_packet,
        "agent_brief": commands.agent_brief,
        "after_snapshot": commands.after_snapshot,
        "agent_verify": commands.agent_verify,
        "agent_receipt": commands.agent_receipt,
        "agent_status": commands.agent_status,
        "review_summary": commands.review_summary
    })
}

fn push_command(rendered: &mut String, label: &str, command: &str) {
    rendered.push_str("### ");
    rendered.push_str(label);
    rendered.push_str("\n\n```bash\n");
    rendered.push_str(command);
    rendered.push_str("\n```\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::Path;

    #[test]
    fn agent_workflow_json_pins_artifacts_and_command_sequence() -> Result<(), String> {
        let report = build_agent_workflow_report(
            Path::new("."),
            &Mode::Draft,
            "67fc764ba37d77bd",
            Path::new("target/ripr/workflow"),
        );
        let rendered = render_agent_workflow_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("agent workflow JSON should parse: {err}"))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "ripr");
        assert_eq!(value["root"], ".");
        assert_eq!(value["mode"], "draft");
        assert_eq!(value["seam_id"], "67fc764ba37d77bd");
        assert_eq!(value["next_step"], "before_snapshot");
        assert_eq!(
            value["artifacts"]["agent_packet"],
            "target/ripr/workflow/agent-packet.json"
        );
        assert_eq!(
            value["artifacts"]["agent_receipt"],
            "target/ripr/reports/agent-receipt.json"
        );
        assert_eq!(
            value["commands"]["before_snapshot"],
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );
        assert_eq!(
            value["commands"]["agent_receipt"],
            "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/reports/agent-receipt.json"
        );
        assert!(value["notes"].as_array().is_some_and(|notes| {
            notes
                .iter()
                .any(|note| note == "Static evidence only; no runtime mutation execution.")
        }));
        Ok(())
    }

    #[test]
    fn agent_workflow_markdown_is_short_and_actionable() {
        let report = build_agent_workflow_report(
            Path::new("."),
            &Mode::Draft,
            "67fc764ba37d77bd",
            Path::new("target/ripr/workflow"),
        );
        let rendered = render_agent_workflow_markdown(&report);

        assert!(rendered.starts_with("# RIPR Agent Workflow"));
        assert!(rendered.contains("Target seam: `67fc764ba37d77bd`"));
        assert!(rendered.contains("4. Write one focused test"));
        assert!(rendered.contains("ripr agent verify --root ."));
        assert!(rendered.contains("Static evidence only; no runtime mutation execution."));
        assert!(rendered.lines().count() < 90);
    }

    #[test]
    fn agent_workflow_quotes_roots_with_spaces() {
        let report = build_agent_workflow_report(
            Path::new("repo root"),
            &Mode::Draft,
            "seam-a",
            Path::new("target/ripr/workflow"),
        );

        assert_eq!(
            report.commands.agent_packet,
            "ripr agent packet --root \"repo root\" --seam-id seam-a --json > target/ripr/workflow/agent-packet.json"
        );
    }
}
