use crate::command::CommandCatalogEntry;

const STARTING_POINTS: &[&str] = &[
    "  cargo xtask doctor      # setup and worktree hygiene",
    "  cargo xtask first-pr    # start-here packet with one safe next action",
    "  cargo xtask pr-ready    # local PR readiness packet",
    "  cargo xtask cockpit     # repo maintainer front panel",
    "  cargo xtask check-pr    # review-ready non-release gate",
];

const START_HERE_NOTES: &str = "Start-here language uses the same words for safe next action, missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, preview-limited evidence, verify command, receipt command, and receipt path.";

pub(crate) fn format_top_level_help(commands: &[&str]) -> String {
    let commands = commands.join("\n  ");
    let starting_points = STARTING_POINTS.join("\n");
    format!(
        "xtask commands:\n\n  {commands}\n\nCommon starting points:\n{starting_points}\n\n{START_HERE_NOTES}\n\nRun `cargo xtask help <command>` for mutability, writes, and notes.\nRun `cargo xtask commands` to write the full command catalog report."
    )
}

pub(crate) fn format_help_entries(query: &str, entries: &[CommandCatalogEntry]) -> String {
    let mut lines = vec![format!("xtask help: `{query}`"), String::new()];
    for entry in entries {
        lines.push(format!("Usage: cargo xtask {}", entry.command));
        lines.push(format!("Mutability: {}", entry.mutability));
        lines.push(format!("Writes: {}", entry.writes));
        lines.push(format!("Judgment required: {}", entry.judgment_required));
        lines.push(format!("Notes: {}", entry.notes));
        lines.push(String::new());
    }
    lines.push("Run `cargo xtask help` for the full command list.".to_string());
    lines.join("\n")
}
