use crate::agent::loop_commands::{self, display_path};
use crate::analysis::repair_route::repair_packet_eligibility;
use crate::analysis::{ClassifiedSeam, is_test_surface_path};
use crate::output::agent_seam_packets::recommended_test_for;
use crate::output::pilot::PilotSummaryContext;
use std::path::Path;

/// The documented start of the repair transaction for `entry` (#3906), or
/// `None` when the fail-closed repair-packet flip does not hold. The flip, not
/// route readiness alone, is the authority here: a wrong actionable repair
/// signal is worse than falling back to the snapshot comparison.
///
/// `agent repair` also refuses an edit target outside a test surface, so a
/// seam whose recommended test lives in an inline `#[cfg(test)]` module of a
/// production file gets no repair line: offering one would send the user into
/// a refusal.
///
/// Built here rather than in `agent::loop_commands`, whose file xtask includes
/// into its own tree: a template only pilot calls reads as dead code there.
pub(super) fn repair_start_command(root: &Path, entry: &ClassifiedSeam) -> Option<String> {
    if !repair_packet_eligibility(entry).eligible() {
        return None;
    }
    if !is_test_surface_path(&recommended_test_for(entry).file) {
        return None;
    }
    Some(format!(
        "ripr agent repair --root {} --seam-id {} --phase before",
        loop_commands::shell_arg(&loop_commands::bound_root(&display_path(root))),
        loop_commands::shell_arg(entry.seam.id().as_str()),
    ))
}

pub(super) struct PilotCommands {
    pub(super) after_snapshot: String,
    pub(super) outcome: String,
    pub(super) retry: String,
}

impl PilotCommands {
    pub(super) fn new(context: PilotSummaryContext<'_>) -> Self {
        // `ripr pilot` writes its artifacts relative to the working directory
        // it ran in, not `--root`, so every pilot path a follow-up command
        // names is bound against that directory once, as is the root (#4000):
        // pasted from any directory, the commands read and write the files
        // this run produced.
        let out_dir = loop_commands::bound_root_path(
            context
                .artifacts
                .pilot_summary_json
                .parent()
                .unwrap_or_else(|| Path::new(".")),
        );
        let after_path = out_dir.join("after.repo-exposure.json");
        let command_root = loop_commands::bound_root(&display_path(context.root));
        let after_snapshot = loop_commands::check_repo_exposure_command(
            &command_root,
            context.mode.as_str(),
            &display_path(&after_path),
        );
        let outcome = loop_commands::outcome_command(
            &display_path(&loop_commands::bound_root_path(
                &context.artifacts.repo_exposure_json,
            )),
            &display_path(&after_path),
            None,
        );
        let retry_timeout_ms = context.timeout_ms.saturating_mul(4).max(120_000);
        let retry = format!(
            "ripr pilot --root {} --out {} --mode {} --max-seams {} --timeout-ms {}",
            loop_commands::shell_arg(&command_root),
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
