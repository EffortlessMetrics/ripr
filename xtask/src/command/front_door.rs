const STARTING_POINTS: &[&str] = &[
    "  cargo xtask doctor      # setup and worktree hygiene",
    "  cargo xtask first-pr    # start-here packet with one safe next action",
    "  cargo xtask pr-ready    # local PR readiness packet",
    "  cargo xtask check-pr    # review-ready non-release gate",
    "  cargo xtask cockpit     # repo maintainer front panel",
];

const START_HERE_NOTES: &str = "Start-here language uses the same words for safe next action, missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, preview-limited evidence, verify command, receipt command, and receipt path.";

pub(crate) fn print() -> Result<(), String> {
    println!("{}", message());
    Ok(())
}

fn message() -> String {
    let starting_points = STARTING_POINTS.join("\n");
    format!(
        "xtask: start here\n\nCommon starting points:\n{starting_points}\n\n{START_HERE_NOTES}\n\nRun `cargo xtask help <command>` for mutability, writes, and notes.\nRun `cargo xtask help --all` for the full command list and [CI] markers.\nRun `cargo xtask commands` to write the full command catalog report."
    )
}
