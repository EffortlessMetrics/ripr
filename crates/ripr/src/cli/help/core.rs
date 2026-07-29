pub(super) const INIT_HELP: &str = r#"Write an optional repo policy file (ripr.toml) and, with --ci github, a non-blocking advisory workflow.

Usage: ripr init [--root PATH] [--ci github] [--dry-run] [--force]

`ripr init` is optional. It writes the built-in defaults to a repo-local
ripr.toml so teams can commit, review, and tune policy. Missing ripr.toml is
the normal first-run state and uses the same defaults. Running `ripr init` does
not unlock basic CLI, editor, or pilot usefulness.

Options:
  --root PATH      Workspace root where ripr.toml should be written. Defaults to current directory.
  --ci github      Also write .github/workflows/ripr.yml with advisory reports and optional SARIF rendering/upload.
  --dry-run        Show the plan and the file bodies without writing anything.
                   Resolves the same preconditions as the real run, so it
                   fails the same way (existing file without --force, root
                   that is not a directory) instead of reporting success.
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

Environment variables:
  RIPR_PILOT_SEAM_BUDGET   Maximum seams written to pilot artifacts (repo-exposure.json,
                            agent-seam-packets.json). Default: 2000. Set to 0 to disable
                            the budget and write all seams (may produce very large files).
                            When the budget is applied, both artifacts include a
                            limitations[] disclosure naming the env var and a repair route.

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
  --before PATH    Static snapshot before the focused test: repo exposure or check JSON.
  --after PATH     Static snapshot after the focused test: repo exposure or check JSON.
  --format FORMAT  md, markdown, text, or json. Defaults to md.
  --out PATH       Write the rendered receipt to a file instead of stdout.

The outcome receipt is advisory. It compares static repo-exposure snapshots by
seam_id and check-output snapshots by canonical_gap_id, then reports moved,
unchanged, regressed, new, and removed gaps or seams. Its
review receipt summarizes what changed, what RIPR flagged before, which focused
proof signals moved, what remains weak or unknown, and what reviewers should
inspect or avoid inferring. It does not run analysis, edit source, generate
tests, run mutation testing, claim runtime correctness or coverage adequacy,
approve merges, or decide CI policy.

Limitation: the comparison matches seams/findings by id only. The before/after
artifacts do not carry a head SHA, so ripr cannot verify they came from the
same repository or adjacent commits. Ensure the before snapshot is from the
same repo's base and the after snapshot is from the same repo's head before
trusting the movement report.
"#;
pub(super) const CHECK_HELP: &str = r#"Analyze a diff or workspace and emit findings in human, JSON, SARIF, or badge form.

Usage: ripr check [OPTIONS]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --diff PATH              Read a unified diff file instead of running git diff.
  --worktree               Diff the base revision against the live working tree
                           instead of HEAD, including staged and unstaged
                           tracked edits. Cannot be combined with --diff.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          Output format. Defaults to human. Groups:
                             Analysis (diff-scoped):
                               human, human-full, json, github, sarif
                             Badge (diff-scoped, for README status):
                               badge-json, badge-shields,
                               badge-plus-json, badge-plus-shields
                             Badge (repo-scoped, from gap ledger):
                               repo-badge-json, repo-badge-shields,
                               repo-badge-plus-json, repo-badge-plus-shields
                             Repo-scope (full-repo analysis):
                               repo-seams-json, repo-seams-md,
                               repo-exposure-json, repo-exposure-summary-json,
                               repo-exposure-md, repo-sarif
                             Agent (machine-readable repair evidence):
                               agent-seam-packets-json
                           badge-plus-* and repo-badge-plus-* formats read
                           target/ripr/reports/test-efficiency.json when present;
                           missing input renders a neutral "needs test-efficiency"
                           badge and warns on stderr. See docs/BADGE_ADOPTION.md.
                           repo-* and agent-seam-packets-json formats render
                           against the full repo baseline; the non-repo badge-*
                           formats remain diff-scoped.
  --gap-ledger PATH        For repo-badge-* formats only, render badge counts
                           from explicit gap-decision-ledger projection targets
                           instead of seam-native/test-efficiency counts.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.
  --suppression-policy PATH
                           Apply a suppressions TOML (same schema as
                           .ripr/suppressions.toml) to the findings-based
                           formats (human, human-full, json, github). exposure_gap
                           entries select findings by finding_id or by a
                           path glob (with optional static_class narrowing).
                           Suppressed findings stay visible in JSON with
                           suppressed: true; per-class summary counts cover
                           unsuppressed findings only. Relative PATH
                           resolves against --root; a missing or malformed
                           policy fails the run.
  --write-artifact PATH    Write a full-fidelity check artifact (schema
                           ripr-check-artifact-v1) for later
                           `ripr explain --from PATH` /
                           `ripr context --from PATH` reuse. The artifact
                           records the complete finding set plus the input
                           identity (diff bytes hash, root, mode, languages,
                           analysis options, config identity, analyzer
                           version). It is a local, disposable derivative:
                           never a gate, badge, or proof input. Diff-scoped
                           findings runs only, including --worktree runs
                           (the recorded base-to-worktree diff is re-resolved
                           at reuse time; worktree drift fails closed on the
                           diff bytes hash); not supported for repo-scoped
                           formats, --gap-ledger, or managed [perl] producer
                           packet generation (pass --perl-facts PATH
                           explicitly instead).
  --git-timeout SECS       Cooperative deadline in seconds for each git
                           invocation in the diff-load path. A git command
                           that exceeds the deadline is terminated and the
                           error names git_invocation_timeout. 0 disables
                           the deadline. Default: 300 (5 minutes). Also
                           settable via RIPR_GIT_TIMEOUT env var.

Environment variables:
  RIPR_MAX_DIFF_CHANGED_RUST_LINES  Maximum added plus removed Rust diff lines
                                    before check fails closed as
                                    diff_scope_oversized. With --json, stdout
                                    carries a non-consumable limited artifact.
                                    Default: 2000.
  RIPR_MAX_DIFF_INDEX_FILES         Maximum Rust files loaded into the diff
                                    index before check fails closed as
                                    diff_scope_oversized. With --json, stdout
                                    carries a non-consumable limited artifact.
                                    Default: 800.
  RIPR_PARTIAL_DIFF_FILE_BUDGET     Changed-line files analyzed before check
                                    returns a bounded limited_partial_scope
                                    partition with exact selected paths,
                                    lower-bound uninspected counts, and a named
                                    stop reason; gate/baseline/badge/Zero
                                    ineligible. Overrides above the hard guard
                                    are clamped with disclosure. Invalid values
                                    fail closed as partial_budget_invalid.
                                    Default: 200.
  RIPR_PARTIAL_DIFF_LINE_BUDGET     Changed lines analyzed before check returns
                                    limited_partial_scope. The first selected
                                    file is always analyzed, even when it alone
                                    exceeds the budget. Overrides above the
                                    hard guard are clamped with disclosure.
                                    Invalid values fail closed as
                                    partial_budget_invalid. Default: 1000.
  RIPR_GIT_TIMEOUT                  Cooperative deadline in seconds for each git
                                    invocation in the diff-load path. A git command
                                    that exceeds the deadline is terminated and the
                                    error names git_invocation_timeout. 0 disables
                                    the deadline. Default: 300 (5 minutes).

Examples:
  ripr check
  ripr check --base HEAD~1
  ripr check --base HEAD --worktree
  ripr check --diff crates/ripr/examples/sample/example.diff --format github
  ripr check --mode ready --json
  ripr check --base origin/main --json --suppression-policy policy/ripr-suppressions.toml
  ripr check --diff d.patch --write-artifact target/ripr/last-check.json
"#;
pub(super) const DIFF_HELP: &str = r#"Analyze the changed surface first and report full-repo context as an explicit bounded state.

Usage: ripr diff [--root PATH] [--base REV] [--head REV] [--mode MODE] [--format human|json] [--json]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --head REV               Head revision for git diff. Defaults to HEAD.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          human, text, md, markdown, or json. Defaults to human.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.

The diff report emits changed files, changed seams, static evidence for those
seams, a diff-complete runtime status, an explicit full-repo-limited context
status, and a receipt path hint. It does not run mutation testing, edit files,
or turn the full-repo limitation into a success state.
"#;
pub(super) const EXPLAIN_HELP: &str = r#"Print why ripr flagged a specific change.

Usage: ripr explain [--root PATH] [--base REV|--diff PATH] [--from PATH] [--mode MODE] [--no-unchanged-tests] [--perl-facts PATH] [--suppression-policy PATH] <finding-id|file:line>

Options:
  --from PATH  Load findings from a check artifact written by
               `ripr check --write-artifact PATH` instead of re-running the
               analysis. The artifact's recorded diff source is re-resolved
               and its identity is re-verified; any mismatch (diff, root,
               mode, languages, analysis options, config identity, or
               analyzer version) fails closed with a typed error naming the
               mismatched fields. --diff/--base passed alongside --from are
               assertions verified against the recording, not overrides.
  --mode MODE  instant, draft, fast, deep, or ready. Defaults to draft.
               With --from, this feeds the identity recomputation: an
               artifact written with a non-default --mode is consumable only
               when the same mode resolves here (flag or ripr.toml).
  --no-unchanged-tests
               Limit the index to changed Rust files. With --from, an
               artifact written with this flag is consumable only when the
               same setting resolves here (flag or ripr.toml).
  --perl-facts PATH
               Use the explicit Perl facts packet for replay.
  --suppression-policy PATH
               Apply the explicit suppression policy for replay.

Performance:
  Use --from to skip re-analysis when you already ran
  `ripr check --write-artifact PATH`. This avoids re-parsing the diff and
  re-classifying every probe — useful for large diffs and scripted workflows
  that run check, explain, and context in sequence.
"#;
pub(super) const CONTEXT_HELP: &str = r#"Print the per-change context packet for one finding or location.

Usage: ripr context [--root PATH] [--base REV|--diff PATH] [--from PATH] [--mode MODE] [--no-unchanged-tests] [--perl-facts PATH] [--suppression-policy PATH] --at <finding-id|file:line> [--max-related-tests N] [--json]

Options:
  --from PATH  Load findings from a check artifact written by
               `ripr check --write-artifact PATH` instead of re-running the
               analysis (same fail-closed identity gate as explain --from).
               --max-related-tests is a render-time knob honored fresh,
               including beyond the check --json render cap.
  --mode MODE  instant, draft, fast, deep, or ready. Defaults to draft.
               With --from, this feeds the identity recomputation (see
               `ripr explain --help`).
  --no-unchanged-tests
               Limit the index to changed Rust files. With --from, feeds
               the identity recomputation (see `ripr explain --help`).
  --perl-facts PATH
               Use the explicit Perl facts packet for replay.
  --suppression-policy PATH
               Apply the explicit suppression policy for replay.

Performance:
  Use --from to skip re-analysis when you already ran
  `ripr check --write-artifact PATH` (see `ripr explain --help`).
"#;
pub(super) const DOCTOR_HELP: &str = r#"Diagnose the local ripr setup (Rust toolchain, workspace, paths).

Usage: ripr doctor [--root PATH] [--json]

Options:
  --root PATH  Diagnose the selected workspace (defaults to `.`).
  --json       Emit the core checks as stable JSON and use the same exit status.

The JSON report is machine-readable advisory setup evidence. A `fail` status or
non-zero exit means at least one core check failed; it is not a release or gate
decision.

Checks:
  - root directory exists
  - Cargo.toml is present at the selected root
  - ripr.toml load status and effective defaults are visible
  - git, cargo, and rustc are available

First-run diagnosis (printed automatically):
  - Detected languages: shallow file-marker scan; each language shows its
    canonical tier (stable or preview) and [adapter not compiled] when the
    feature is absent. Only languages with concrete markers are listed —
    no overclaiming.
  - Detected test surfaces: per detected language, the first recognizable
    framework marker (cargo test, pytest, jest, vitest, bun). Prints
    "test framework not detected" rather than guessing.
  - Known limitations: static notes on preview coverage, cross-language
    oracle visibility (fail-closed), large-repo scan bounds, and advisory
    nature of preview-language evidence.
  - Recommended first command: ripr check --base origin/main

Start-here next step:
  - after setup is valid, run `ripr first-pr --root . --base origin/main --head HEAD`
    or `ripr start-here --root . --base origin/main --head HEAD`
    or this repo's `cargo xtask first-pr` wrapper
  - open `target/ripr/reports/start-here.md` first when it exists
  - safe next action means repair one named gap, regenerate missing or malformed
    evidence, refresh stale evidence, fix wrong-root setup, or stop on no-action
  - treat missing artifact, stale evidence, wrong root, malformed artifact,
    no actionable gap, and preview-limited evidence as explicit stop or
    regeneration states, not hidden success
  - verify command, receipt command, and receipt path are the static proof rail;
    receipts stay advisory and do not prove runtime adequacy or gate approval
"#;
pub(super) const LSP_HELP: &str = r#"Start the experimental ripr LSP server over stdio.

Usage: ripr lsp [--stdio] [--version]

Options:
  --stdio       Run the language server over stdio LSP framing. This is the default.
  --version     Print the language server version.
"#;
