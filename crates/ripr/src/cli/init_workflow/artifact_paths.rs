use crate::agent::loop_commands;

const ARTIFACT_PATH_REPLACEMENTS: &[(&str, &str)] = &[
    (
        "target/ripr/pilot/repo-exposure.json",
        loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
    ),
    (
        "target/ripr/pilot/after.repo-exposure.json",
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
    ),
    (
        "target/ripr/agent/agent-packet.json",
        loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
    ),
    (
        "target/ripr/agent/agent-brief.json",
        loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
    ),
    (
        "target/ripr/agent/agent-verify.json",
        loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
    ),
    (
        "target/ripr/agent/agent-receipt.json",
        loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
    ),
    (
        "target/ripr/workflow/before.repo-exposure.json",
        loop_commands::WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
    ),
    (
        "target/ripr/workflow/after.repo-exposure.json",
        loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
    ),
    (
        "target/ripr/workflow/workflow.json",
        loop_commands::WORKFLOW_MANIFEST_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-seam-packets.json",
        loop_commands::WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-packet.json",
        loop_commands::WORKFLOW_AGENT_PACKET_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-brief.json",
        loop_commands::WORKFLOW_AGENT_BRIEF_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-verify.json",
        loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
    ),
    (
        "target/ripr/reports/agent-receipt.json",
        loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-status.json",
        loop_commands::WORKFLOW_AGENT_STATUS_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-status.md",
        loop_commands::WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-review-summary.json",
        loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
    ),
    (
        "target/ripr/workflow/agent-review-summary.md",
        loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
    ),
];

pub(super) fn apply_to(workflow: &str) -> String {
    let mut rendered = workflow.to_string();
    for (placeholder, artifact_path) in ARTIFACT_PATH_REPLACEMENTS {
        rendered = rendered.replace(placeholder, artifact_path);
    }
    rendered
}
