const HELP: &str = r#"ripr — static RIPR mutation exposure analysis for Rust

Usage:
  ripr init [--root PATH] [--ci github] [--dry-run] [--force]
  ripr pilot [--root PATH] [--out PATH] [--mode draft] [--max-seams 5] [--timeout-ms 30000]
  ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]
  ripr evidence-health [--root PATH] [--out PATH] [--out-md PATH] [--mutation-calibration PATH]
  ripr review-comments --root . --base SHA --head SHA [--out target/ripr/review/comments.json]
  ripr gate evaluate --pr-guidance PATH [--mode visible-only] [--out target/ripr/reports/gate-decision.json]
  ripr baseline create --from target/ripr/reports/gate-decision.json [--out .ripr/gate-baseline.json] [--dry-run] [--force]
  ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json [--out target/ripr/reports/baseline-debt-delta.json] [--out-md target/ripr/reports/baseline-debt-delta.md]
  ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved [--out .ripr/gate-baseline.json]
  ripr zero status --delta target/ripr/reports/baseline-debt-delta.json [--baseline .ripr/gate-baseline.json] [--gate target/ripr/reports/gate-decision.json] [--out target/ripr/reports/ripr-zero-status.json] [--out-md target/ripr/reports/ripr-zero-status.md]
  ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]
  ripr agent start --root . --seam-id ID [--out target/ripr/workflow]
  ripr agent brief --root . (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json
  ripr agent packet --root . --seam-id ID --json
  ripr agent verify --root . --before before.json --after after.json --json
  ripr agent receipt --root . --verify-json agent-verify.json --seam-id ID --json
  ripr agent status --root . [--json]
  ripr agent review-summary --root . [--json]
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
  ripr evidence-health --root .
  ripr review-comments --root . --base origin/main --head HEAD --out target/ripr/review/comments.json
  ripr gate evaluate --pr-guidance target/ripr/review/comments.json --mode visible-only
  ripr baseline create --from target/ripr/reports/gate-decision.json --out .ripr/gate-baseline.json
  ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json
  ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved
  ripr zero status --baseline .ripr/gate-baseline.json --delta target/ripr/reports/baseline-debt-delta.json --gate target/ripr/reports/gate-decision.json
  ripr calibrate cargo-mutants --mutants-json target/mutants/outcomes.json --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
  ripr agent start --root . --seam-id f3c9e4d21a0b7c88
  ripr agent brief --root . --diff change.diff --json
  ripr agent packet --root . --seam-id f3c9e4d21a0b7c88 --json
  ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json
  ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id f3c9e4d21a0b7c88 --json
  ripr agent status --root .
  ripr agent review-summary --root .
  ripr check --diff crates/ripr/examples/sample/example.diff
  ripr check --diff crates/ripr/examples/sample/example.diff --json
  ripr explain --diff crates/ripr/examples/sample/example.diff <finding-id>
"#;

const INIT_HELP: &str = r#"Usage: ripr init [--root PATH] [--ci github] [--dry-run] [--force]

`ripr init` is optional. It writes the built-in defaults to a repo-local
ripr.toml so teams can commit, review, and tune policy. Missing ripr.toml is
the normal first-run state and uses the same defaults. Running `ripr init` does
not unlock basic CLI, editor, or pilot usefulness.

Options:
  --root PATH      Workspace root where ripr.toml should be written. Defaults to current directory.
  --ci github      Also write .github/workflows/ripr.yml with advisory reports and optional SARIF rendering/upload.
  --dry-run        Print the generated config without writing.
  --force          Overwrite an existing ripr.toml or generated workflow.

Generated config:
  - uses draft analysis mode and includes unchanged tests
  - shows actionable weak or missing seams with default severities
  - hides seams whose configured severity is off
  - records the built-in saved-workspace LSP seam diagnostic default
  - remains advisory and does not configure CI blocking or mutation execution

Generated GitHub workflow:
  - installs ripr and writes a pilot packet plus repo report artifacts
  - uploads report artifacts and writes a reviewer-oriented advisory summary
  - surfaces future PR test guidance reports as non-blocking check annotations
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

const EVIDENCE_HEALTH_HELP: &str = r#"Usage: ripr evidence-health [--root PATH] [--out PATH] [--out-md PATH] [--mutation-calibration PATH]

Options:
  --root PATH                    Workspace root to summarize. Defaults to current directory.
  --out PATH                     JSON output path. Defaults to target/ripr/reports/evidence-health.json.
  --out-md PATH                  Markdown output path. Defaults to target/ripr/reports/evidence-health.md.
  --mutation-calibration PATH    Optional imported mutation-calibration JSON for calibration availability counts.

The evidence-health report is an advisory Lane 1 analyzer-health view. It
summarizes seam grip classes, missing discriminators, observed values, related
test confidence, oracle strength, unknown static stages, and optional imported
calibration availability. It does not change analyzer behavior, run mutation
testing, edit source files, configure CI policy, or make gate decisions.
"#;

const REVIEW_COMMENTS_HELP: &str = r#"Usage: ripr review-comments [--root PATH] --base SHA --head SHA [--out PATH]

Options:
  --root PATH    Workspace root. Defaults to current directory.
  --base SHA     Pull-request base revision.
  --head SHA     Pull-request head revision.
  --out PATH     JSON output path. Defaults to target/ripr/review/comments.json.

The review-comments command writes a bounded advisory PR guidance report as
JSON plus a sibling Markdown file. It joins existing static seam evidence with
the changed-line diff and only places line guidance on changed lines. It does
not post to GitHub, edit source, generate tests, run mutation testing, or make
CI blocking by default.
"#;

const GATE_HELP: &str = r#"Usage: ripr gate evaluate --pr-guidance PATH [--mode MODE] [--out PATH] [--out-md PATH]

Options:
  --root PATH                         Workspace root. Defaults to current directory.
  --repo-exposure PATH                Optional repo-exposure JSON input.
  --pr-guidance PATH                  Required PR guidance JSON from `ripr review-comments`.
  --sarif-policy PATH                 Optional SARIF policy JSON input.
  --labels-json PATH                  Optional JSON array or object with labels.
  --label LABEL                       Repeatable current PR label input.
  --agent-verify PATH                 Optional agent verify JSON input.
  --agent-receipt PATH                Optional agent receipt JSON input.
  --recommendation-calibration PATH   Optional recommendation calibration JSON input.
  --mutation-calibration PATH         Optional imported mutation calibration JSON input.
  --baseline PATH                     Explicit baseline for baseline-check or calibrated-gate.
  --mode MODE                         visible-only, acknowledgeable, baseline-check, or calibrated-gate. Defaults to visible-only.
  --acknowledgement-label LABEL       Repeatable acknowledgement label. Defaults to ripr-waive.
  --out PATH                          JSON output path. Defaults to target/ripr/reports/gate-decision.json.
  --out-md PATH                       Markdown output path. Defaults to --out with .md extension.

The gate evaluator is read-only policy over existing RIPR evidence. It writes
JSON and Markdown before returning a non-zero exit for `blocked` or
`config_error` decisions. It does not post comments, edit source, generate
tests, run mutation testing, upload SARIF, mutate GitHub state, or change
generated workflow defaults.
"#;

const BASELINE_HELP: &str = r#"Usage:
  ripr baseline create --from PATH [--out PATH] [--dry-run] [--force]
  ripr baseline diff --baseline PATH --current PATH [--out PATH] [--out-md PATH]
  ripr baseline update --baseline PATH --current PATH --remove-resolved [--out PATH]

Create options:
  --from PATH    Gate-decision JSON from `ripr gate evaluate`.
  --out PATH     Baseline ledger path. Defaults to .ripr/gate-baseline.json.
  --dry-run      Print the baseline ledger JSON without writing.
  --force        Overwrite an existing baseline ledger.

Diff options:
  --baseline PATH    Reviewed baseline ledger. Defaults are supplied by callers.
  --current PATH     Current gate-decision JSON from `ripr gate evaluate`.
  --out PATH         JSON output path. Defaults to target/ripr/reports/baseline-debt-delta.json.
  --out-md PATH      Markdown output path. Defaults to target/ripr/reports/baseline-debt-delta.md.

Update options:
  --baseline PATH       Reviewed baseline ledger to refresh.
  --current PATH        Current gate-decision JSON from `ripr gate evaluate`.
  --remove-resolved     Required shrink-only mode; remove identities absent from current evidence.
  --out PATH            Updated baseline path. Defaults to --baseline.

The baseline create command writes a stable reviewed historical-debt ledger
from existing gate-decision evidence. It includes advisory, acknowledged, and
blocking identities; skips suppressed, configured-off, not-applicable, and
malformed decisions; and refuses to overwrite by default. It does not edit
source, run analysis, run mutation testing, generate tests, change gate policy,
or make CI blocking by default.

The baseline diff command compares a reviewed baseline ledger with current
gate-decision evidence and writes advisory JSON/Markdown debt movement. It
reports still-present, resolved, new policy-eligible, acknowledged, suppressed,
stale, invalid, and missing-input identities. It does not update baselines,
edit source, run analysis, run mutation testing, generate tests, change gate
policy, or make CI blocking by default.

The baseline update command refreshes a reviewed baseline ledger in shrink-only
mode. `--remove-resolved` removes reviewed identities that are absent from the
current gate-decision evidence, preserves malformed or ambiguous entries for
manual review, and never adopts new current debt. Generated CI should not use
this command to rewrite checked-in baselines automatically.
"#;

const ZERO_HELP: &str = r#"Usage: ripr zero status --delta PATH [--baseline PATH] [--gate PATH] [--pr-guidance PATH] [--recommendation-calibration PATH] [--out PATH] [--out-md PATH]

Status options:
  --baseline PATH                       Optional reviewed gate baseline ledger.
  --delta PATH                          Required baseline-debt-delta JSON from `ripr baseline diff`.
  --gate PATH                           Optional gate-decision JSON from `ripr gate evaluate`.
  --pr-guidance PATH                    Optional PR guidance JSON from `ripr review-comments`.
  --recommendation-calibration PATH     Optional recommendation calibration JSON.
  --out PATH                            JSON output path. Defaults to target/ripr/reports/ripr-zero-status.json.
  --out-md PATH                         Markdown output path. Defaults to target/ripr/reports/ripr-zero-status.md.

The RIPR Zero status report is read-only advisory progress evidence over
existing baselines, baseline debt deltas, gate decisions, PR guidance, and
optional calibration artifacts. It reports visible unresolved debt, baseline
movement, metadata health, top debt areas, and bounded repair routes. It does
not run analysis, mutate baselines, edit source, generate tests, call an LLM,
run mutation testing, change gate policy, or make CI blocking by default.
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
  start      Write a source-edit-free workflow manifest for one seam.
  brief      Rank a working-set brief for the agent-active router.
  packet     Expand one visible seam into the existing agent seam packet JSON.
  verify     Compare before/after repo-exposure JSON for agent verification.
  receipt    Summarize one seam from agent verify JSON for review handoff.
  status     Report existing agent-loop artifacts and the next missing command.
  review-summary
             Join agent-loop artifacts into a compact review packet.

Run `ripr agent start --help` for the workflow manifest, `ripr agent brief
--help`, `ripr agent packet --help`, or `ripr agent verify --help` for
JSON-only agent surfaces. Run `ripr agent receipt --help` for the verification
receipt surface, `ripr agent status --help` for the artifact status lens, and
`ripr agent review-summary --help` for the PR-review packet.
"#;

const AGENT_START_HELP: &str = r#"Usage: ripr agent start [--root PATH] --seam-id ID [--out PATH]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --seam-id ID     Select one visible seam by ID.
  --out PATH       Workflow output directory. Defaults to target/ripr/workflow.

The start command writes a source-edit-free workflow packet for one seam:
workflow.json, commands.md, and agent-brief.json. The packet contains artifact
paths and shared command templates for the before snapshot, packet, brief,
after snapshot, verify, and receipt steps. It remains advisory and static; it
does not call an LLM API, run mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
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
handoff metadata for review. The verify JSON path and the before/after snapshot
paths named inside it must resolve under `--root`; receipt provenance hashes
those three artifacts without rerunning analysis. It remains advisory and
static; it does not run analysis, mutation testing, generate tests, edit files,
change cache behavior, or touch LSP/MCP surfaces.
"#;

const AGENT_STATUS_HELP: &str = r#"Usage: ripr agent status [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable status report. Human Markdown is the default.

The status command reads existing agent-loop artifacts under target/ripr only
and reports which before snapshot, after snapshot, brief, packet, verify, and
receipt files are present or missing. It may recover a seam_id from those
artifacts and emits the next command to run for missing inputs. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
"#;

const AGENT_REVIEW_SUMMARY_HELP: &str = r#"Usage: ripr agent review-summary [--root PATH] [--json]

Options:
  --root PATH      Workspace root. Defaults to current directory.
  --json           Emit the machine-readable review summary. Human Markdown is the default.

The review-summary command reads existing agent-loop artifacts and joins agent
status, receipt, workflow, operator cockpit, repo exposure, LSP cockpit when
present, and local CI artifact state into a compact review packet. It remains
advisory and static; it does not run analysis, mutation testing, generate
tests, edit files, change cache behavior, or touch LSP/MCP surfaces.
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

pub(super) fn print_evidence_health_help() {
    println!("{EVIDENCE_HEALTH_HELP}");
}

pub(super) fn print_review_comments_help() {
    println!("{REVIEW_COMMENTS_HELP}");
}

pub(super) fn print_gate_help() {
    println!("{GATE_HELP}");
}

pub(super) fn print_baseline_help() {
    println!("{BASELINE_HELP}");
}

pub(super) fn print_zero_help() {
    println!("{ZERO_HELP}");
}

pub(super) fn print_calibrate_help() {
    println!("{CALIBRATE_HELP}");
}

pub(super) fn print_agent_help() {
    println!("{AGENT_HELP}");
}

pub(super) fn print_agent_start_help() {
    println!("{AGENT_START_HELP}");
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

pub(super) fn print_agent_status_help() {
    println!("{AGENT_STATUS_HELP}");
}

pub(super) fn print_agent_review_summary_help() {
    println!("{AGENT_REVIEW_SUMMARY_HELP}");
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
        AGENT_BRIEF_HELP, AGENT_HELP, AGENT_PACKET_HELP, AGENT_RECEIPT_HELP,
        AGENT_REVIEW_SUMMARY_HELP, AGENT_START_HELP, AGENT_STATUS_HELP, AGENT_VERIFY_HELP,
        BASELINE_HELP, CALIBRATE_HELP, CHECK_HELP, CONTEXT_HELP, DOCTOR_HELP, EVIDENCE_HEALTH_HELP,
        EXPLAIN_HELP, GATE_HELP, HELP, INIT_HELP, LSP_HELP, OUTCOME_HELP, PILOT_HELP,
        REVIEW_COMMENTS_HELP, ZERO_HELP,
    };

    #[test]
    fn top_level_help_mentions_supported_commands() {
        assert!(HELP.contains("ripr init"));
        assert!(HELP.contains("ripr pilot"));
        assert!(HELP.contains("ripr outcome"));
        assert!(HELP.contains("ripr evidence-health"));
        assert!(HELP.contains("ripr review-comments"));
        assert!(HELP.contains("ripr gate evaluate"));
        assert!(HELP.contains("ripr baseline create"));
        assert!(HELP.contains("ripr baseline diff"));
        assert!(HELP.contains("ripr baseline update"));
        assert!(HELP.contains("ripr zero status"));
        assert!(HELP.contains("ripr calibrate"));
        assert!(HELP.contains("ripr agent start"));
        assert!(HELP.contains("ripr agent brief"));
        assert!(HELP.contains("ripr agent packet"));
        assert!(HELP.contains("ripr agent verify"));
        assert!(HELP.contains("ripr agent receipt"));
        assert!(HELP.contains("ripr agent status"));
        assert!(HELP.contains("ripr agent review-summary"));
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
        assert!(EVIDENCE_HEALTH_HELP.starts_with("Usage: ripr evidence-health"));
        assert!(EVIDENCE_HEALTH_HELP.contains("--mutation-calibration PATH"));
        assert!(REVIEW_COMMENTS_HELP.starts_with("Usage: ripr review-comments"));
        assert!(REVIEW_COMMENTS_HELP.contains("target/ripr/review/comments.json"));
        assert!(GATE_HELP.starts_with("Usage: ripr gate evaluate"));
        assert!(GATE_HELP.contains("visible-only"));
        assert!(GATE_HELP.contains("ripr-waive"));
        assert!(BASELINE_HELP.starts_with("Usage:"));
        assert!(BASELINE_HELP.contains("ripr baseline create"));
        assert!(BASELINE_HELP.contains("ripr baseline diff"));
        assert!(BASELINE_HELP.contains("ripr baseline update"));
        assert!(BASELINE_HELP.contains(".ripr/gate-baseline.json"));
        assert!(BASELINE_HELP.contains("baseline-debt-delta.json"));
        assert!(BASELINE_HELP.contains("--remove-resolved"));
        assert!(ZERO_HELP.starts_with("Usage: ripr zero status"));
        assert!(ZERO_HELP.contains("baseline-debt-delta JSON"));
        assert!(ZERO_HELP.contains("RIPR Zero status report"));
        assert!(CALIBRATE_HELP.starts_with("Usage: ripr calibrate cargo-mutants"));
        assert!(CALIBRATE_HELP.contains("--mutants-json PATH"));
        assert!(AGENT_HELP.starts_with("Usage: ripr agent"));
        assert!(AGENT_START_HELP.starts_with("Usage: ripr agent start"));
        assert!(AGENT_START_HELP.contains("workflow.json"));
        assert!(AGENT_BRIEF_HELP.starts_with("Usage: ripr agent brief"));
        assert!(AGENT_BRIEF_HELP.contains("--max-seams N"));
        assert!(AGENT_BRIEF_HELP.contains("RIPR-SPEC-0010"));
        assert!(AGENT_PACKET_HELP.starts_with("Usage: ripr agent packet"));
        assert!(AGENT_PACKET_HELP.contains("agent-seam-packets-json"));
        assert!(AGENT_VERIFY_HELP.starts_with("Usage: ripr agent verify"));
        assert!(AGENT_VERIFY_HELP.contains("repo-exposure-json"));
        assert!(AGENT_RECEIPT_HELP.starts_with("Usage: ripr agent receipt"));
        assert!(AGENT_RECEIPT_HELP.contains("--verify-json PATH"));
        assert!(AGENT_STATUS_HELP.starts_with("Usage: ripr agent status"));
        assert!(AGENT_STATUS_HELP.contains("before snapshot"));
        assert!(AGENT_REVIEW_SUMMARY_HELP.starts_with("Usage: ripr agent review-summary"));
        assert!(AGENT_REVIEW_SUMMARY_HELP.contains("Human Markdown is the default"));
        assert!(EXPLAIN_HELP.starts_with("Usage: ripr explain"));
        assert!(CONTEXT_HELP.starts_with("Usage: ripr context"));
        assert!(DOCTOR_HELP.starts_with("Usage: ripr doctor [--root PATH]"));
        assert!(DOCTOR_HELP.contains("Cargo.toml"));
        assert!(LSP_HELP.contains("--stdio"));
        assert!(LSP_HELP.contains("--version"));
    }
}
