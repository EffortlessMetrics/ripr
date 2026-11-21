use crate::agent::loop_commands::{self, display_path};
use crate::output::pilot::PilotSummaryContext;
use std::path::{Path, PathBuf};

pub(super) struct PilotCommands {
    pub(super) after_snapshot: String,
    pub(super) outcome: String,
    pub(super) retry: String,
}

impl PilotCommands {
    pub(super) fn new(context: PilotSummaryContext<'_>) -> Self {
        let out_dir = context
            .artifacts
            .pilot_summary_json
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let after_path = context
            .artifacts
            .pilot_summary_json
            .parent()
            .map(|dir| dir.join("after.repo-exposure.json"))
            .unwrap_or_else(|| PathBuf::from("after.repo-exposure.json"));
        let after_snapshot = loop_commands::check_repo_exposure_command(
            &display_path(context.root),
            context.mode.as_str(),
            &loop_commands::shell_path(&after_path),
        );
        let outcome = loop_commands::outcome_command(
            &loop_commands::shell_path(&context.artifacts.repo_exposure_json),
            &loop_commands::shell_path(&after_path),
            None,
        );
        let retry_timeout_ms = context.timeout_ms.saturating_mul(4).max(120_000);
        let retry = format!(
            "ripr pilot --root {} --out {} --mode {} --max-seams {} --timeout-ms {}",
            loop_commands::shell_path(context.root),
            loop_commands::shell_path(&out_dir),
            context.mode.as_str(),
            context.max_seams,
            retry_timeout_ms
        );
        Self {
            after_snapshot,
            outcome,
            retry,
        }
    }
}
