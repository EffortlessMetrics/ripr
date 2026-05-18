pub(super) const INIT_HELP: &str = r#"Write an optional repo policy file (ripr.toml) and, with --ci github, a non-blocking advisory workflow.

Usage: ripr init [--root PATH] [--ci github] [--dry-run] [--force]

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
pub(super) const PILOT_HELP: &str = r#"Find the top test gap in this repo and write a packet you can act on.

Usage: ripr pilot [--root PATH] [--out PATH] [--mode MODE] [--max-seams N] [--timeout-ms MS]

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
pub(super) const OUTCOME_HELP: &str = r#"Compare before/after static evidence after adding a focused test.

Usage: ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]

Options:
  --before PATH    Repo-exposure JSON snapshot before the focused test.
  --after PATH     Repo-exposure JSON snapshot after the focused test.
  --format FORMAT  md, markdown, text, or json. Defaults to md.
  --out PATH       Write the rendered receipt to a file instead of stdout.

The outcome receipt is advisory. It compares static repo-exposure snapshots by
seam_id and reports moved, unchanged, regressed, new, and removed seams; it
does not run analysis, mutation testing, or CI policy.
"#;
pub(super) const CHECK_HELP: &str = r#"Analyze a diff or workspace and emit findings in human, JSON, SARIF, or badge form.

Usage: ripr check [OPTIONS]

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
  --gap-ledger PATH        For repo-badge-* formats only, render badge counts
                           from explicit gap-decision-ledger projection targets
                           instead of seam-native/test-efficiency counts.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.

Examples:
  ripr check
  ripr check --base HEAD~1
  ripr check --diff crates/ripr/examples/sample/example.diff --format github
  ripr check --mode ready --json
"#;
pub(super) const EXPLAIN_HELP: &str = r#"Print why ripr flagged a specific change.

Usage: ripr explain [--root PATH] [--base REV|--diff PATH] <finding-id|file:line>
"#;
pub(super) const CONTEXT_HELP: &str = r#"Print the per-change context packet for one finding or location.

Usage: ripr context [--root PATH] [--base REV|--diff PATH] --at <finding-id|file:line> [--max-related-tests N] [--json]
"#;
pub(super) const DOCTOR_HELP: &str = r#"Diagnose the local ripr setup (Rust toolchain, workspace, paths).

Usage: ripr doctor [--root PATH]

Checks:
  - root directory exists
  - Cargo.toml is present at the selected root
  - ripr.toml load status and effective defaults are visible
  - git, cargo, and rustc are available
"#;
pub(super) const LSP_HELP: &str = r#"Start the experimental ripr LSP server over stdio.

Usage: ripr lsp [--stdio] [--version]

Options:
  --stdio       Run the language server over stdio LSP framing. This is the default.
  --version     Print the language server version.
"#;
