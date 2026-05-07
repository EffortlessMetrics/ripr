//! Shared command templates for the editor, agent, cockpit, and CI proof loop.
//!
//! The public workflow intentionally has two path profiles:
//! - `workflow/*` for local agent status and brief handoff artifacts.
//! - `pilot/agent/*` for editor, CI, and operator cockpit artifacts.

#![allow(
    dead_code,
    reason = "the same template source is included by ripr and xtask, which use different command subsets"
)]

use std::path::Path;

pub(crate) const COMMAND_ROOT: &str = ".";

pub(crate) const WORKFLOW_BEFORE_SNAPSHOT: &str = "target/ripr/workflow/before.repo-exposure.json";
pub(crate) const WORKFLOW_AFTER_SNAPSHOT: &str = "target/ripr/workflow/after.repo-exposure.json";
pub(crate) const WORKFLOW_AGENT_SEAM_PACKETS: &str = "target/ripr/workflow/agent-seam-packets.json";
pub(crate) const WORKFLOW_AGENT_BRIEF: &str = "target/ripr/workflow/agent-brief.json";
pub(crate) const WORKFLOW_AGENT_PACKET: &str = "target/ripr/workflow/agent-packet.json";
pub(crate) const WORKFLOW_AGENT_VERIFY: &str = "target/ripr/workflow/agent-verify.json";
pub(crate) const WORKFLOW_AGENT_RECEIPT: &str = "target/ripr/reports/agent-receipt.json";

pub(crate) const PILOT_DIR: &str = "target/ripr/pilot";
pub(crate) const PILOT_REPO_EXPOSURE: &str = "target/ripr/pilot/repo-exposure.json";
pub(crate) const PILOT_AFTER_SNAPSHOT: &str = "target/ripr/pilot/after.repo-exposure.json";
pub(crate) const EDITOR_AGENT_PACKET: &str = "target/ripr/agent/agent-packet.json";
pub(crate) const EDITOR_AGENT_BRIEF: &str = "target/ripr/agent/agent-brief.json";
pub(crate) const EDITOR_AGENT_VERIFY: &str = "target/ripr/agent/agent-verify.json";
pub(crate) const EDITOR_AGENT_RECEIPT: &str = "target/ripr/agent/agent-receipt.json";
pub(crate) const TARGETED_TEST_OUTCOME: &str = "target/ripr/reports/targeted-test-outcome.json";

pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

pub(crate) fn display_text_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(crate) fn shell_path(path: &Path) -> String {
    shell_arg(&display_path(path))
}

pub(crate) fn shell_text_path(path: &str) -> String {
    shell_arg(&display_text_path(path))
}

pub(crate) fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '\\' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

pub(crate) fn repo_exposure_snapshot_command(root: &Path, mode: &str, output: &str) -> String {
    format!(
        "ripr check --root {} --mode {mode} --format repo-exposure-json > {}",
        shell_path(root),
        shell_text_path(output)
    )
}

pub(crate) fn agent_seam_packets_command(root: &Path, mode: &str, output: &str) -> String {
    format!(
        "ripr check --root {} --mode {mode} --format agent-seam-packets-json > {}",
        shell_path(root),
        shell_text_path(output)
    )
}

pub(crate) fn agent_packet_command(root: &Path, seam_id: &str, output: &str) -> String {
    format!(
        "ripr agent packet --root {} --seam-id {} --json > {}",
        shell_path(root),
        shell_arg(seam_id),
        shell_text_path(output)
    )
}

pub(crate) fn agent_brief_command(root: &Path, seam_id: &str, output: &str) -> String {
    format!(
        "ripr agent brief --root {} --seam-id {} --json > {}",
        shell_path(root),
        shell_arg(seam_id),
        shell_text_path(output)
    )
}

pub(crate) fn agent_verify_command(
    root: &Path,
    before: &str,
    after: &str,
    output: Option<&str>,
) -> String {
    let mut command = format!(
        "ripr agent verify --root {} --before {} --after {} --json",
        shell_path(root),
        shell_text_path(before),
        shell_text_path(after)
    );
    if let Some(output) = output {
        command.push_str(" > ");
        command.push_str(&shell_text_path(output));
    }
    command
}

pub(crate) fn agent_receipt_command(
    root: &Path,
    verify_json: &str,
    seam_id: &str,
    output: &str,
) -> String {
    agent_receipt_command_with_seam_arg(root, verify_json, &shell_arg(seam_id), output)
}

pub(crate) fn agent_receipt_command_with_seam_arg(
    root: &Path,
    verify_json: &str,
    seam_id_arg: &str,
    output: &str,
) -> String {
    format!(
        "ripr agent receipt --root {} --verify-json {} --seam-id {} --json --out {}",
        shell_path(root),
        shell_text_path(verify_json),
        seam_id_arg,
        shell_text_path(output)
    )
}

pub(crate) fn outcome_command(
    before: &str,
    after: &str,
    format: Option<&str>,
    output: Option<&str>,
) -> String {
    let mut command = format!(
        "ripr outcome --before {} --after {}",
        shell_text_path(before),
        shell_text_path(after)
    );
    if let Some(format) = format {
        command.push_str(" --format ");
        command.push_str(format);
    }
    if let Some(output) = output {
        command.push_str(" --out ");
        command.push_str(&shell_text_path(output));
    }
    command
}

pub(crate) fn pilot_command(out: &str) -> String {
    format!("ripr pilot --out {}", shell_text_path(out))
}

pub(crate) fn pilot_retry_command(
    root: &Path,
    out_dir: &Path,
    mode: &str,
    max_seams: usize,
    timeout_ms: u64,
) -> String {
    format!(
        "ripr pilot --root {} --out {} --mode {mode} --max-seams {max_seams} --timeout-ms {timeout_ms}",
        shell_path(root),
        shell_path(out_dir)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_commands_preserve_public_templates() {
        assert_eq!(
            repo_exposure_snapshot_command(Path::new("."), "draft", WORKFLOW_AFTER_SNAPSHOT),
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
        );
        assert_eq!(
            agent_verify_command(
                Path::new("."),
                WORKFLOW_BEFORE_SNAPSHOT,
                WORKFLOW_AFTER_SNAPSHOT,
                Some(WORKFLOW_AGENT_VERIFY),
            ),
            "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json"
        );
    }

    #[test]
    fn editor_agent_commands_preserve_public_templates() {
        assert_eq!(
            agent_packet_command(Path::new("."), "seam-a", EDITOR_AGENT_PACKET),
            "ripr agent packet --root . --seam-id seam-a --json > target/ripr/agent/agent-packet.json"
        );
        assert_eq!(
            outcome_command(
                PILOT_REPO_EXPOSURE,
                PILOT_AFTER_SNAPSHOT,
                Some("json"),
                Some(TARGETED_TEST_OUTCOME),
            ),
            "ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --format json --out target/ripr/reports/targeted-test-outcome.json"
        );
    }

    #[test]
    fn commands_quote_workspace_paths_but_keep_relative_artifacts_plain() {
        assert_eq!(shell_arg("repo root"), "\"repo root\"");
        assert_eq!(
            shell_text_path("target/ripr/workflow"),
            "target/ripr/workflow"
        );
        assert_eq!(
            repo_exposure_snapshot_command(
                Path::new("repo root"),
                "draft",
                WORKFLOW_BEFORE_SNAPSHOT,
            ),
            "ripr check --root \"repo root\" --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );
    }
}
