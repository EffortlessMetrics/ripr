const HELP: &str = r#"ripr — static RIPR mutation exposure analysis for Rust

Usage:
  ripr init [--root PATH] [--ci github] [--dry-run] [--force]
  ripr pilot [--root PATH] [--out PATH] [--mode draft] [--max-seams 5] [--timeout-ms 30000]
  ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]
  ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]
  ripr agent brief --root . (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json
  ripr agent packet --root . --seam-id ID --json
  ripr agent verify --root . --before before.json --after after.json --json
  ripr agent receipt --root . --verify-json agent-verify.json --seam-id ID --json
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
  ripr agent brief --root . --diff change.diff --json
  ripr agent packet --root . --seam-id f3c9e4d21a0b7c88 --json
  ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json
  ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id f3c9e4d21a0b7c88 --json
  ripr check --diff crates/ripr/examples/sample/example.diff
  ripr check --diff crates/ripr/examples/sample/example.diff --json
  ripr explain --diff crates/ripr/examples/sample/example.diff <finding-id>
"#;

const INIT_HELP: &str = r#"Usage: ripr init [--root PATH] [--ci github] [--dry-run] [--force]

Options:
  --root PATH      Workspace root where ripr.toml should be written. Defaults to current directory.
  --ci github      Also write .github/workflows/ripr.yml with advisory reports and optional SARIF rendering/upload.
  --dry-run        Print the generated config without writing.
  --force          Overwrite an existing ripr.toml or generated workflow.

Generated config:
  - uses draft analysis mode and includes unchanged tests
  - shows actionable weak or missing seams with conservative severities
  - hides seams whose configured severity is off
  - records the built-in saved-workspace LSP seam diagnostic default
  - remains advisory and does not configure CI blocking or mutation execution

Generated GitHub workflow:
  - installs ripr and writes a pilot packet plus repo report artifacts
  - uploads report artifacts for review and adds the pilot summary to the job
  - renders and uploads diff/repo SARIF only while RIPR_UPLOAD_SARIF is true
  - uses continue-on-error for advisory RIPR work and upload steps
  - does not enable baseline failure policy by default
"#;

const PILOT_HELP: &str = r#"Usage: ripr pilot [--root PATH] [--out PATH] [--mode MODE] [--max-seams N] [--timeout-ms MS]

Options:
  --root PATH       Workspace root to analyze. Defaults to current directory.
  --out PATH        Output directory for the pilot packet. Defaults to target/ripr/pilot.
  --mode MODE       instant, draft, fast, deep, or ready. Defaults to draft unless ripr.toml sets one.
  --max-seams N     Maximum ranked seams in the pilot summary. Defaults to 5.
  --timeout-ms MS   Maximum analysis budget before writing a partial summary. Defaults to 30000.

Outputs:
  - repo-exposure.json and repo-exposure.md
  - agent-seam-packets.json
  - pilot-summary.json and pilot-summary.md

The pilot packet is advisory. It reports saved-workspace static seam evidence
and points to one next focused test action; it does not run mutation testing,
edit source files, or configure CI policy. If analysis exceeds the timeout,
pilot-summary.json and pilot-summary.md are written with status=partial and an
explicit retry command.
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

const AGENT_HELP: &str = r#"Usage: ripr agent <subcommand>

Subcommands:
  brief      Rank a working-set brief for the agent-active router.
  packet     Expand one visible seam into the existing agent seam packet JSON.
  verify     Compare before/after repo-exposure JSON for agent verification.
  receipt    Summarize one seam from agent verify JSON for review handoff.

Run `ripr agent brief --help`, `ripr agent packet --help`, or
`ripr agent verify --help` for JSON-only agent surfaces. Run
`ripr agent receipt --help` for the verification receipt surface.
"#;

const AGENT_BRIEF_HELP: &str = r#"Usage: ripr agent brief [--root PATH] (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json [--max-seams N]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --diff PATH      Select a diff file and line-level working set.
  --base REV       Derive the working set from a base revision.
  --files PATHS    Comma-separated repo-relative file paths.
  --seam-id ID     Select one visible seam by ID.
  --json           Required until a human brief surface exists.
  --max-seams N    Requested seam cap. Defaults to 3 and cannot exceed 10.

This parser is the first implementation seam for RIPR-SPEC-0010. The brief
router remains advisory and static; it does not run mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;

const AGENT_PACKET_HELP: &str = r#"Usage: ripr agent packet [--root PATH] --seam-id ID --json

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --seam-id ID     Select one visible seam by ID.
  --json           Required until a human packet surface exists.

The packet command expands a seam selected by `ripr agent brief` into the
existing agent-seam-packets-json envelope with one packet. It remains advisory
and static; it does not run mutation testing, generate tests, edit files, change
cache behavior, or touch LSP/MCP surfaces.
"#;

const AGENT_VERIFY_HELP: &str = r#"Usage: ripr agent verify [--root PATH] --before PATH --after PATH --json

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --before PATH    Before `repo-exposure-json` snapshot.
  --after PATH     After `repo-exposure-json` snapshot.
  --json           Required until a human verify surface exists.

The verify command compares two saved static repo-exposure artifacts and emits
an agent-focused before/after summary. Snapshot paths must resolve under
`--root`. The command remains advisory and static; it does not run analysis,
mutation testing, generate tests, edit files, change cache behavior, or touch
LSP/MCP surfaces.
"#;

const AGENT_RECEIPT_HELP: &str = r#"Usage: ripr agent receipt [--root PATH] --verify-json PATH --seam-id ID --json [--test NAME] [--command CMD] [--out PATH]

Options:
  --root PATH         Workspace root. Defaults to current directory.
  --verify-json PATH  JSON emitted by `ripr agent verify`.
  --seam-id ID        Select one seam from the verify JSON.
  --json              Required until a human receipt surface exists.
  --test NAME         Optional focused test added or changed by the agent.
  --command CMD       Optional verification command that was run. Repeatable.
  --out PATH          Write the JSON receipt to a file instead of stdout.

The receipt command narrows a saved agent verify artifact to one seam and adds
handoff metadata for review. The verify JSON path must resolve under `--root`.
It remains advisory and static; it does not run analysis, mutation testing,
generate tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
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

pub(super) fn print_agent_help() {
    println!("{AGENT_HELP}");
}

pub(super) fn print_agent_brief_help() {
    println!("{AGENT_BRIEF_HELP}");
}

pub(super) fn print_agent_packet_help() {
    println!("{AGENT_PACKET_HELP}");
}

pub(super) fn print_agent_verify_help() {
    println!("{AGENT_VERIFY_HELP}");
}

pub(super) fn print_agent_receipt_help() {
    println!("{AGENT_RECEIPT_HELP}");
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
        AGENT_BRIEF_HELP, AGENT_HELP, AGENT_PACKET_HELP, AGENT_RECEIPT_HELP, AGENT_VERIFY_HELP,
        CALIBRATE_HELP, CHECK_HELP, CONTEXT_HELP, DOCTOR_HELP, EXPLAIN_HELP, HELP, INIT_HELP,
        LSP_HELP, OUTCOME_HELP, PILOT_HELP,
    };

    #[test]
    fn top_level_help_mentions_supported_commands() {
        assert!(HELP.contains("ripr init"));
        assert!(HELP.contains("ripr pilot"));
        assert!(HELP.contains("ripr outcome"));
        assert!(HELP.contains("ripr calibrate"));
        assert!(HELP.contains("ripr agent brief"));
        assert!(HELP.contains("ripr agent packet"));
        assert!(HELP.contains("ripr agent verify"));
        assert!(HELP.contains("ripr agent receipt"));
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
        assert!(INIT_HELP.contains("--ci github"));
        assert!(INIT_HELP.contains("--dry-run"));
        assert!(INIT_HELP.contains("--force"));
        assert!(PILOT_HELP.starts_with("Usage: ripr pilot"));
        assert!(PILOT_HELP.contains("pilot-summary.json"));
        assert!(PILOT_HELP.contains("--timeout-ms MS"));
        assert!(OUTCOME_HELP.starts_with("Usage: ripr outcome"));
        assert!(OUTCOME_HELP.contains("--before PATH"));
        assert!(CALIBRATE_HELP.starts_with("Usage: ripr calibrate cargo-mutants"));
        assert!(CALIBRATE_HELP.contains("--mutants-json PATH"));
        assert!(AGENT_HELP.starts_with("Usage: ripr agent"));
        assert!(AGENT_BRIEF_HELP.starts_with("Usage: ripr agent brief"));
        assert!(AGENT_BRIEF_HELP.contains("--max-seams N"));
        assert!(AGENT_BRIEF_HELP.contains("RIPR-SPEC-0010"));
        assert!(AGENT_PACKET_HELP.starts_with("Usage: ripr agent packet"));
        assert!(AGENT_PACKET_HELP.contains("agent-seam-packets-json"));
        assert!(AGENT_VERIFY_HELP.starts_with("Usage: ripr agent verify"));
        assert!(AGENT_VERIFY_HELP.contains("repo-exposure-json"));
        assert!(AGENT_RECEIPT_HELP.starts_with("Usage: ripr agent receipt"));
        assert!(AGENT_RECEIPT_HELP.contains("--verify-json PATH"));
        assert!(EXPLAIN_HELP.starts_with("Usage: ripr explain"));
        assert!(CONTEXT_HELP.starts_with("Usage: ripr context"));
        assert!(DOCTOR_HELP.starts_with("Usage: ripr doctor [--root PATH]"));
        assert!(DOCTOR_HELP.contains("Cargo.toml"));
        assert!(LSP_HELP.contains("--stdio"));
        assert!(LSP_HELP.contains("--version"));
    }
}
