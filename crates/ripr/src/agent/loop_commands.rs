use std::path::Path;

pub(crate) const WORKFLOW_BEFORE_SNAPSHOT_FILE: &str = "before.repo-exposure.json";
pub(crate) const WORKFLOW_AFTER_SNAPSHOT_FILE: &str = "after.repo-exposure.json";
pub(crate) const WORKFLOW_AGENT_PACKET_FILE: &str = "agent-packet.json";
pub(crate) const WORKFLOW_AGENT_BRIEF_FILE: &str = "agent-brief.json";
pub(crate) const WORKFLOW_AGENT_VERIFY_FILE: &str = "agent-verify.json";
pub(crate) const WORKFLOW_AGENT_WORKFLOW_JSON_FILE: &str = "agent-workflow.json";
pub(crate) const WORKFLOW_AGENT_WORKFLOW_MARKDOWN_FILE: &str = "agent-workflow.md";
pub(crate) const WORKFLOW_AGENT_STATUS_FILE: &str = "agent-status.json";
pub(crate) const WORKFLOW_AGENT_REVIEW_SUMMARY_FILE: &str = "agent-review-summary.json";

pub(crate) const WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT: &str =
    "target/ripr/workflow/before.repo-exposure.json";
pub(crate) const WORKFLOW_AFTER_SNAPSHOT_ARTIFACT: &str =
    "target/ripr/workflow/after.repo-exposure.json";
pub(crate) const WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT: &str =
    "target/ripr/workflow/agent-seam-packets.json";
pub(crate) const WORKFLOW_AGENT_PACKET_ARTIFACT: &str = "target/ripr/workflow/agent-packet.json";
pub(crate) const WORKFLOW_AGENT_BRIEF_ARTIFACT: &str = "target/ripr/workflow/agent-brief.json";
pub(crate) const WORKFLOW_AGENT_VERIFY_ARTIFACT: &str = "target/ripr/workflow/agent-verify.json";
pub(crate) const WORKFLOW_AGENT_RECEIPT_ARTIFACT: &str = "target/ripr/reports/agent-receipt.json";

pub(crate) const PILOT_BEFORE_SNAPSHOT_ARTIFACT: &str = "target/ripr/pilot/repo-exposure.json";
pub(crate) const PILOT_AFTER_SNAPSHOT_ARTIFACT: &str = "target/ripr/pilot/after.repo-exposure.json";
pub(crate) const EDITOR_AGENT_PACKET_ARTIFACT: &str = "target/ripr/agent/agent-packet.json";
pub(crate) const EDITOR_AGENT_BRIEF_ARTIFACT: &str = "target/ripr/agent/agent-brief.json";
pub(crate) const EDITOR_AGENT_VERIFY_ARTIFACT: &str = "target/ripr/agent/agent-verify.json";
pub(crate) const EDITOR_AGENT_RECEIPT_ARTIFACT: &str = "target/ripr/agent/agent-receipt.json";

pub(crate) const WORKFLOW_AGENT_STATUS_ARTIFACT: &str = "target/ripr/workflow/agent-status.json";
pub(crate) const WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT: &str =
    "target/ripr/workflow/agent-review-summary.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowArtifacts {
    pub(crate) before_snapshot: String,
    pub(crate) after_snapshot: String,
    pub(crate) agent_packet: String,
    pub(crate) agent_brief: String,
    pub(crate) agent_verify: String,
    pub(crate) agent_receipt: String,
    pub(crate) agent_status: String,
    pub(crate) review_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentWorkflowCommands {
    pub(crate) before_snapshot: String,
    pub(crate) agent_packet: String,
    pub(crate) agent_brief: String,
    pub(crate) after_snapshot: String,
    pub(crate) agent_verify: String,
    pub(crate) agent_receipt: String,
    pub(crate) agent_status: String,
    pub(crate) review_summary: String,
}

pub(crate) fn agent_workflow_artifacts(out_dir: &Path) -> AgentWorkflowArtifacts {
    AgentWorkflowArtifacts {
        before_snapshot: workflow_path(out_dir, WORKFLOW_BEFORE_SNAPSHOT_FILE),
        after_snapshot: workflow_path(out_dir, WORKFLOW_AFTER_SNAPSHOT_FILE),
        agent_packet: workflow_path(out_dir, WORKFLOW_AGENT_PACKET_FILE),
        agent_brief: workflow_path(out_dir, WORKFLOW_AGENT_BRIEF_FILE),
        agent_verify: workflow_path(out_dir, WORKFLOW_AGENT_VERIFY_FILE),
        agent_receipt: WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string(),
        agent_status: workflow_path(out_dir, WORKFLOW_AGENT_STATUS_FILE),
        review_summary: workflow_path(out_dir, WORKFLOW_AGENT_REVIEW_SUMMARY_FILE),
    }
}

pub(crate) fn agent_workflow_commands(
    root: &str,
    mode: &str,
    seam_id: &str,
    artifacts: &AgentWorkflowArtifacts,
) -> AgentWorkflowCommands {
    AgentWorkflowCommands {
        before_snapshot: check_repo_exposure_command(root, mode, &artifacts.before_snapshot),
        agent_packet: agent_packet_command(root, seam_id, &artifacts.agent_packet),
        agent_brief: agent_brief_command(root, seam_id, &artifacts.agent_brief),
        after_snapshot: check_repo_exposure_command(root, mode, &artifacts.after_snapshot),
        agent_verify: agent_verify_command(
            root,
            &artifacts.before_snapshot,
            &artifacts.after_snapshot,
            Some(&artifacts.agent_verify),
        ),
        agent_receipt: agent_receipt_command(
            root,
            &artifacts.agent_verify,
            seam_id,
            Some(&artifacts.agent_receipt),
        ),
        agent_status: agent_status_command(root, Some(&artifacts.agent_status)),
        review_summary: agent_review_summary_command(root, Some(&artifacts.review_summary)),
    }
}

pub(crate) fn check_repo_exposure_command(root: &str, mode: &str, out_path: &str) -> String {
    format!(
        "ripr check --root {} --mode {} --format repo-exposure-json > {}",
        shell_arg(root),
        shell_arg(mode),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_seam_packets_command(root: &str, mode: &str, out_path: &str) -> String {
    format!(
        "ripr check --root {} --mode {} --format agent-seam-packets-json > {}",
        shell_arg(root),
        shell_arg(mode),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_packet_command(root: &str, seam_id: &str, out_path: &str) -> String {
    format!(
        "ripr agent packet --root {} --seam-id {} --json > {}",
        shell_arg(root),
        shell_arg(seam_id),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_brief_command(root: &str, seam_id: &str, out_path: &str) -> String {
    format!(
        "ripr agent brief --root {} --seam-id {} --json > {}",
        shell_arg(root),
        shell_arg(seam_id),
        shell_arg(out_path)
    )
}

pub(crate) fn agent_verify_command(
    root: &str,
    before_path: &str,
    after_path: &str,
    out_path: Option<&str>,
) -> String {
    let command = format!(
        "ripr agent verify --root {} --before {} --after {} --json",
        shell_arg(root),
        shell_arg(before_path),
        shell_arg(after_path)
    );
    append_redirect(command, out_path)
}

pub(crate) fn agent_receipt_command(
    root: &str,
    verify_json: &str,
    seam_id: &str,
    out_path: Option<&str>,
) -> String {
    let command = format!(
        "ripr agent receipt --root {} --verify-json {} --seam-id {} --json",
        shell_arg(root),
        shell_arg(verify_json),
        shell_arg(seam_id)
    );
    match out_path {
        Some(path) => format!("{command} --out {}", shell_arg(path)),
        None => command,
    }
}

pub(crate) fn agent_status_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!("ripr agent status --root {} --json", shell_arg(root)),
        out_path,
    )
}

pub(crate) fn agent_review_summary_command(root: &str, out_path: Option<&str>) -> String {
    append_redirect(
        format!(
            "ripr agent review-summary --root {} --json",
            shell_arg(root)
        ),
        out_path,
    )
}

pub(crate) fn outcome_command(
    before_path: &str,
    after_path: &str,
    out_path: Option<&str>,
) -> String {
    match out_path {
        Some(path) => {
            format!(
                "ripr outcome --before {} --after {} --format json --out {}",
                shell_arg(before_path),
                shell_arg(after_path),
                shell_arg(path)
            )
        }
        None => format!(
            "ripr outcome --before {} --after {}",
            shell_arg(before_path),
            shell_arg(after_path)
        ),
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

pub(crate) fn shell_path(path: &Path) -> String {
    shell_arg(&display_path(path))
}

fn workflow_path(out_dir: &Path, file_name: &str) -> String {
    display_path(&out_dir.join(file_name))
}

pub(crate) fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '\\' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn append_redirect(command: String, out_path: Option<&str>) -> String {
    match out_path {
        Some(path) => format!("{command} > {}", shell_arg(path)),
        None => command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_commands_match_existing_status_templates() {
        assert_eq!(
            check_repo_exposure_command(".", "draft", WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );
        assert_eq!(
            check_repo_exposure_command(".", "draft", WORKFLOW_AFTER_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
        );
        assert_eq!(
            agent_packet_command(".", "seam-a", WORKFLOW_AGENT_PACKET_ARTIFACT),
            "ripr agent packet --root . --seam-id seam-a --json > target/ripr/workflow/agent-packet.json"
        );
        assert_eq!(
            agent_brief_command(".", "seam-a", WORKFLOW_AGENT_BRIEF_ARTIFACT),
            "ripr agent brief --root . --seam-id seam-a --json > target/ripr/workflow/agent-brief.json"
        );
        assert_eq!(
            agent_verify_command(
                ".",
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                Some(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            ),
            "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json"
        );
        assert_eq!(
            agent_receipt_command(
                ".",
                WORKFLOW_AGENT_VERIFY_ARTIFACT,
                "seam-a",
                Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            ),
            "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id seam-a --json --out target/ripr/reports/agent-receipt.json"
        );
    }

    #[test]
    fn editor_commands_match_existing_lsp_templates() {
        assert_eq!(
            agent_packet_command(".", "seam-a", EDITOR_AGENT_PACKET_ARTIFACT),
            "ripr agent packet --root . --seam-id seam-a --json > target/ripr/agent/agent-packet.json"
        );
        assert_eq!(
            check_repo_exposure_command(".", "ready", PILOT_AFTER_SNAPSHOT_ARTIFACT),
            "ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json"
        );
        assert_eq!(
            agent_verify_command(
                ".",
                PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                PILOT_AFTER_SNAPSHOT_ARTIFACT,
                Some(EDITOR_AGENT_VERIFY_ARTIFACT),
            ),
            "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
        );
    }

    #[test]
    fn command_args_quote_spaces_without_touching_plain_tokens() {
        assert_eq!(shell_arg("repo root"), "\"repo root\"");
        assert_eq!(shell_arg("target/ripr/workflow"), "target/ripr/workflow");
        assert_eq!(
            check_repo_exposure_command(
                "repo root",
                "draft",
                "target/ripr/work flow/before.repo-exposure.json"
            ),
            "ripr check --root \"repo root\" --mode draft --format repo-exposure-json > \"target/ripr/work flow/before.repo-exposure.json\""
        );
        assert_eq!(
            agent_seam_packets_command(".", "draft mode", "target/ripr/workflow/packets.json"),
            "ripr check --root . --mode \"draft mode\" --format agent-seam-packets-json > target/ripr/workflow/packets.json"
        );
    }

    #[test]
    fn workflow_artifacts_can_follow_custom_manifest_directory() {
        let artifacts = agent_workflow_artifacts(Path::new("target/ripr/workflow"));

        assert_eq!(artifacts.before_snapshot, WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT);
        assert_eq!(artifacts.agent_packet, WORKFLOW_AGENT_PACKET_ARTIFACT);
        assert_eq!(artifacts.agent_receipt, WORKFLOW_AGENT_RECEIPT_ARTIFACT);
        assert_eq!(artifacts.agent_status, WORKFLOW_AGENT_STATUS_ARTIFACT);
        assert_eq!(
            artifacts.review_summary,
            WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT
        );
    }
}
