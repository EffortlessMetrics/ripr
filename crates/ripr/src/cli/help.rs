const HELP: &str = r#"ripr — static RIPR mutation exposure analysis for Rust

Usage:
  ripr init [--root PATH] [--dry-run] [--force]
  ripr pilot [--root PATH] [--out PATH] [--mode draft] [--max-seams 5]
  ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]
  ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]
  ripr check [--base origin/main] [--diff PATH] [--mode draft] [--format FORMAT]
  ripr explain [--base REV|--diff PATH] <finding-id|file:line>
  ripr context [--base REV|--diff PATH] --at <finding-id|file:line>
  ripr lsp [--stdio]
  ripr doctor

What it does:
  Reads changed Rust code, creates mutation-like probes, and estimates whether
  tests appear to reach, infect, propagate, and reveal the changed behavior
  through meaningful oracles. It does not run mutants.

Quick start:
  ripr doctor
  ripr pilot
  ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json
  ripr calibrate cargo-mutants --mutants-json target/mutants/outcomes.json --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
  ripr check --diff crates/ripr/examples/sample/example.diff
  ripr check --diff crates/ripr/examples/sample/example.diff --json
  ripr explain --diff crates/ripr/examples/sample/example.diff <finding-id>
"#;

const INIT_HELP: &str = r#"Usage: ripr init [--root PATH] [--dry-run] [--force]

Options:
  --root PATH      Workspace root where ripr.toml should be written. Defaults to current directory.
  --dry-run        Print the generated config without writing.
  --force          Overwrite an existing ripr.toml.

Generated config:
  - uses draft analysis mode and includes unchanged tests
  - shows actionable weak or missing seams with conservative severities
  - hides strongly_gripped, intentional, and suppressed seams
  - records the built-in saved-workspace LSP seam diagnostic default
  - remains advisory and does not configure CI blocking or mutation execution
"#;

const PILOT_HELP: &str = r#"Usage: ripr pilot [--root PATH] [--out PATH] [--mode MODE] [--max-seams N]

Options:
  --root PATH       Workspace root to analyze. Defaults to current directory.
  --out PATH        Output directory for the pilot packet. Defaults to target/ripr/pilot.
  --mode MODE       instant, draft, fast, deep, or ready. Defaults to draft unless ripr.toml sets one.
  --max-seams N     Maximum ranked seams in the pilot summary. Defaults to 5.

Outputs:
  - repo-exposure.json and repo-exposure.md
  - agent-seam-packets.json
  - pilot-summary.json and pilot-summary.md

The pilot packet is advisory. It reports saved-workspace static seam evidence
and points to one next focused test action; it does not run mutation testing,
edit source files, or configure CI policy.
"#;

const OUTCOME_HELP: &str = r#"Usage: ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]

Options:
  --before PATH    Repo-exposure JSON snapshot before the focused test.
  --after PATH     Repo-exposure JSON snapshot after the focused test.
  --format FORMAT  md, markdown, text, or json. Defaults to md.
  --out PATH       Write the rendered receipt to a file instead of stdout.

The outcome receipt is advisory. It compares static repo-exposure snapshots by
seam_id and reports moved, unchanged, regressed, new, and removed seams; it
does not run analysis, mutation testing, or CI policy.
"#;

const CALIBRATE_HELP: &str = r#"Usage: ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]

Options:
  --mutants-json PATH          cargo-mutants JSON file, or directory containing outcomes.json and/or mutants.json.
  --repo-exposure-json PATH    RIPR repo-exposure-json snapshot to join against.
  --format FORMAT             md, markdown, text, or json. Defaults to md.
  --out PATH                  Write the rendered calibration report to a file instead of stdout.

The calibration report is advisory. It imports already-produced runtime
mutation data and joins it to static seam evidence by seam_id first, then by
unambiguous file/line. It does not run mutation testing, alter static
classifications, or configure CI policy.
"#;

const CHECK_HELP: &str = r#"Usage: ripr check [OPTIONS]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --diff PATH              Read a unified diff file instead of running git diff.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          human, json, github, sarif, badge-json, badge-shields,
                           badge-plus-json, badge-plus-shields, repo-badge-json,
                           repo-badge-shields, repo-badge-plus-json,
                           repo-badge-plus-shields, repo-seams-json,
                           repo-seams-md, repo-exposure-json, repo-exposure-md,
                           repo-sarif, agent-seam-packets-json. Defaults to human.
                           badge-plus-* and repo-badge-plus-* formats require
                           target/ripr/reports/test-efficiency.json (run
                           `cargo xtask test-efficiency-report` first).
                           repo-* and agent-seam-packets-json formats render
                           against the full repo baseline; the non-repo badge-*
                           formats remain diff-scoped.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.

Examples:
  ripr check
  ripr check --base HEAD~1
  ripr check --diff crates/ripr/examples/sample/example.diff --format github
  ripr check --mode ready --json
"#;

const EXPLAIN_HELP: &str =
    "Usage: ripr explain [--root PATH] [--base REV|--diff PATH] <finding-id|file:line>";
const CONTEXT_HELP: &str = "Usage: ripr context [--root PATH] [--base REV|--diff PATH] --at <finding-id|file:line> [--max-related-tests N] [--json]";
const DOCTOR_HELP: &str = r#"Usage: ripr doctor [--root PATH]

Checks:
  - root directory exists
  - Cargo.toml is present at the selected root
  - ripr.toml load status and effective defaults are visible
  - git, cargo, and rustc are available
"#;
const LSP_HELP: &str = r#"Usage: ripr lsp [--stdio] [--version]

Options:
  --stdio       Run the language server over stdio LSP framing. This is the default.
  --version     Print the language server version.
"#;

pub(super) fn print_help() {
    println!("{HELP}");
}

pub(super) fn print_check_help() {
    println!("{CHECK_HELP}");
}

pub(super) fn print_init_help() {
    println!("{INIT_HELP}");
}

pub(super) fn print_pilot_help() {
    println!("{PILOT_HELP}");
}

pub(super) fn print_outcome_help() {
    println!("{OUTCOME_HELP}");
}

pub(super) fn print_calibrate_help() {
    println!("{CALIBRATE_HELP}");
}

pub(super) fn print_explain_help() {
    println!("{EXPLAIN_HELP}");
}

pub(super) fn print_context_help() {
    println!("{CONTEXT_HELP}");
}

pub(super) fn print_doctor_help() {
    println!("{DOCTOR_HELP}");
}

pub(super) fn print_lsp_help() {
    println!("{LSP_HELP}");
}

#[cfg(test)]
mod tests {
    use super::{
        CALIBRATE_HELP, CHECK_HELP, CONTEXT_HELP, DOCTOR_HELP, EXPLAIN_HELP, HELP, INIT_HELP,
        LSP_HELP, OUTCOME_HELP, PILOT_HELP,
    };

    #[test]
    fn top_level_help_mentions_supported_commands() {
        assert!(HELP.contains("ripr init"));
        assert!(HELP.contains("ripr pilot"));
        assert!(HELP.contains("ripr outcome"));
        assert!(HELP.contains("ripr calibrate"));
        assert!(HELP.contains("ripr check"));
        assert!(HELP.contains("ripr explain"));
        assert!(HELP.contains("ripr context"));
        assert!(HELP.contains("ripr doctor"));
    }

    #[test]
    fn check_help_mentions_repo_badge_formats_and_examples() {
        assert!(CHECK_HELP.contains("repo-badge-plus-shields"));
        assert!(CHECK_HELP.contains("repo-exposure-json"));
        assert!(CHECK_HELP.contains("agent-seam-packets-json"));
        assert!(CHECK_HELP.contains("repo-sarif"));
        assert!(CHECK_HELP.contains("test-efficiency-report"));
        assert!(CHECK_HELP.contains("--mode ready --json"));
    }

    #[test]
    fn command_specific_help_usage_lines_are_stable() {
        assert!(INIT_HELP.starts_with("Usage: ripr init"));
        assert!(INIT_HELP.contains("--dry-run"));
        assert!(INIT_HELP.contains("--force"));
        assert!(PILOT_HELP.starts_with("Usage: ripr pilot"));
        assert!(PILOT_HELP.contains("pilot-summary.json"));
        assert!(OUTCOME_HELP.starts_with("Usage: ripr outcome"));
        assert!(OUTCOME_HELP.contains("--before PATH"));
        assert!(CALIBRATE_HELP.starts_with("Usage: ripr calibrate cargo-mutants"));
        assert!(CALIBRATE_HELP.contains("--mutants-json PATH"));
        assert!(EXPLAIN_HELP.starts_with("Usage: ripr explain"));
        assert!(CONTEXT_HELP.starts_with("Usage: ripr context"));
        assert!(DOCTOR_HELP.starts_with("Usage: ripr doctor [--root PATH]"));
        assert!(DOCTOR_HELP.contains("Cargo.toml"));
        assert!(LSP_HELP.contains("--stdio"));
        assert!(LSP_HELP.contains("--version"));
    }
}
