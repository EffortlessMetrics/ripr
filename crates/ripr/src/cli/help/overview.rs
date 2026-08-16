/// The default first screen: `ripr`, `ripr --help`, `ripr -h`, `ripr help`.
///
/// This answers "what are you trying to do?" before "what commands exist?"
/// (#1613). The previous overview opened with every policy, ledger, badge, and
/// calibration command — 91 lines, with the quick start buried at line 75 — so
/// a first-time reader had to scroll past the operator surface to find
/// `doctor`. The exhaustive inventory now lives in [`HELP_ALL`] behind
/// `ripr help --all`, and per-command flags behind `ripr help <command>`.
///
/// `help_overview_fits_one_screen` pins the envelope so this cannot silently
/// grow back into a dump.
///
/// The first line is not free prose: "changed Rust code where the nearby tests
/// may not actually catch the behavior" is the canonical description of what
/// `ripr` looks for, listed as such in `docs/TERMINOLOGY.md` and reused in
/// `docs/QUICKSTART.md`. Reword it there first, everywhere, or not at all —
/// `help_runs` in `tests/cli_smoke.rs` holds this line to that vocabulary.
pub(super) const HELP: &str = r#"ripr — find changed Rust code where nearby tests may not actually catch the
       changed behavior.

Usage:
  ripr <command> [options]

Try this first:
  ripr doctor                     Check this workspace can produce evidence.
  ripr check --base origin/main   Analyze the current diff, name the top gap.

The loop is: ripr names one gap -> you add one focused test -> ripr records
whether the gap closed. `ripr.toml` is optional; the zero-config run is the
intended first interface.

What are you trying to do?
  Diagnose setup        ripr doctor
  Inspect one change    ripr check [--base REV | --diff PATH]
                                   [--format human-full | json]
  Guided repo adoption  ripr pilot --root .
  Understand a finding  ripr explain <finding-id>
                        ripr context --at <finding-id>
  Repair one named gap  ripr agent repair --seam-id ID --phase before
                        # edit one focused test
                        ripr agent repair --seam-id ID --phase after
  Compose PR evidence   ripr first-pr --root . --base origin/main --head HEAD
  Work in an editor     ripr lsp --stdio
  Adopt advisory CI     ripr init --ci github

More:
  ripr help <command>    Options for one command.
  ripr help --all        Every command, grouped by area.

ripr is static and advisory. It reads changed code, builds mutation-shaped
probes, and estimates whether tests reach, infect, propagate, and reveal the
changed behavior. It does not run mutants and does not report runtime mutation
outcomes; a real mutation runner confirms later.
"#;

/// The exhaustive command reference: `ripr help --all`.
///
/// Every command the parser accepts must appear here. `help_all_documents_every_
/// public_command` enforces that against the parser's own list, because the
/// previous overview had already drifted behind it — `pr-summary`,
/// `annotations`, `pr-evidence`, and `impacted-evidence` were all reachable and
/// undocumented.
pub(super) const HELP_ALL: &str = r#"ripr — complete command reference.

Task-oriented overview: ripr --help
Options for one command: ripr help <command>

Task map:
  Diagnose setup        ripr doctor
  Inspect one change    ripr check --base origin/main
  Guided repo adoption  ripr pilot --root .
  Repair one named gap  ripr agent repair --seam-id ID --phase before|after
  Compose PR evidence   ripr first-pr --root . --base origin/main --head HEAD
  Adopt advisory CI     ripr init --ci github

Setup:
  ripr doctor
  ripr init [--root PATH] [--ci github] [--dry-run] [--force]
  ripr config validate [--root PATH]
  ripr cache status [--json]
  ripr cache clear [--dry-run] [--force]

Analysis:
  ripr pilot [--root PATH] [--out PATH] [--mode draft] [--max-seams 5] [--timeout-ms 30000]
  ripr check [--base origin/main] [--worktree] [--diff PATH] [--mode draft] [--format FORMAT]
  ripr diff [--root .] [--base origin/main] [--head HEAD] [--mode draft] [--json]
  ripr explain [--base REV|--diff PATH] <finding-id|file:line>
  ripr context [--base REV|--diff PATH] --at <finding-id|file:line>
  ripr rerun --changed-test PATH [--root PATH] [--json] [--out PATH]
  ripr evidence-health [--root PATH] [--out PATH] [--out-md PATH] [--mutation-calibration PATH]
  ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH [--format md|json] [--out PATH]

Editor & Agent:
  ripr lsp [--stdio]
  ripr agent repair --root . --seam-id ID --phase before|after
  ripr agent start --root . --seam-id ID [--out target/ripr/workflow]
  ripr agent brief --root . (--diff PATH|--base REV|--files PATHS|--seam-id ID) --json
  ripr agent packet --root . --seam-id ID --json
  ripr agent verify --root . --before before.json --after after.json --json
  ripr agent verify-execute --root . --packet packet.json --result-json result.json --authorize --json
  ripr agent receipt --root . --verify-json agent-verify.json --seam-id ID --json
  ripr agent status --root . [--json]
  ripr agent review-summary --root . [--json]
  ripr swarm queue [--root .] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--language python] [--top 10]
  ripr swarm ingest [--root .] --result target/ripr/workflow/agent-result.json
  ripr plus (--repo-exposure-summary target/ripr/reports/repo-exposure-summary.json|--gap-ledger target/ripr/reports/gap-decision-ledger.json) [--check]

PR & Review:
  ripr outcome --before PATH --after PATH [--format md|json] [--out PATH]
  ripr first-pr [--root .] [--base origin/main] [--head HEAD] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--out-dir target/ripr/reports] [--check]
  ripr start-here [same options as first-pr]
  ripr first-action [--root .] [--pr-guidance target/ripr/review/comments.json] [--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--ledger target/ripr/reports/pr-evidence-ledger.json] [--out target/ripr/reports/first-useful-action.json]
  ripr review-comments --root . --base SHA --head SHA [--out target/ripr/review/comments.json]
  ripr pr-summary [--check] [--baseline <before.json>]
  ripr pr-evidence [--base <rev>] [--head <rev>] [--root <path>] [--check]
  ripr impacted-evidence [--pr-evidence <path>] [--label <label>] [--labels <csv>] [--check]
  ripr annotations [--comments <path>] [--out <path>] [--check]
  ripr pr-ledger record --pr-number 123 --base SHA --head SHA [--gate target/ripr/reports/gate-decision.json] [--baseline-delta target/ripr/reports/baseline-debt-delta.json] [--zero-status target/ripr/reports/ripr-zero-status.json] [--out target/ripr/reports/pr-evidence-ledger.json]
  ripr pr-comments plan --pr-guidance target/ripr/review/comments.json [--existing-comments target/ripr/review/existing-comments.json] [--mode off|plan|inline] [--out target/ripr/review/comment-publish-plan.json]
  ripr pr-review front-panel [--pr-guidance target/ripr/review/comments.json] [--first-action target/ripr/reports/first-useful-action.json] [--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json] [--assistant-health target/ripr/reports/assistant-loop-health.json] [--ledger target/ripr/reports/pr-evidence-ledger.json] [--out target/ripr/reports/pr-review-front-panel.json]
  ripr coverage-grip frontier (--ledger target/ripr/reports/pr-evidence-ledger.json|--baseline-delta target/ripr/reports/baseline-debt-delta.json|--zero-status target/ripr/reports/ripr-zero-status.json) [--coverage target/ripr/reports/coverage-summary.json] [--out target/ripr/reports/coverage-grip-frontier.json]
  ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json [--out target/ripr/reports/test-oracle-assistant-proof.json]
  ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json [--out target/ripr/reports/assistant-loop-health.json]

Policy & Gate:
  ripr gate evaluate --pr-guidance PATH [--mode visible-only] [--out target/ripr/reports/gate-decision.json]
  ripr baseline create --from target/ripr/reports/gate-decision.json [--out .ripr/gate-baseline.json] [--dry-run] [--force]
  ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json [--out target/ripr/reports/baseline-debt-delta.json] [--out-md target/ripr/reports/baseline-debt-delta.md]
  ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved [--out .ripr/gate-baseline.json]
  ripr zero status --delta target/ripr/reports/baseline-debt-delta.json [--baseline .ripr/gate-baseline.json] [--gap-ledger target/ripr/reports/gap-decision-ledger.json] [--gate target/ripr/reports/gate-decision.json] [--out target/ripr/reports/ripr-zero-status.json] [--out-md target/ripr/reports/ripr-zero-status.md]
  ripr policy readiness [--gate-decision target/ripr/reports/gate-decision.json] [--baseline-delta target/ripr/reports/baseline-debt-delta.json] [--out target/ripr/reports/policy-readiness.json] [--out-md target/ripr/reports/policy-readiness.md]
  ripr policy operations --policy-readiness target/ripr/reports/policy-readiness.json [--waiver-aging target/ripr/reports/waiver-aging.json] [--suppression-health target/ripr/reports/suppression-health.json] [--out target/ripr/reports/policy-operations.json] [--out-md target/ripr/reports/policy-operations.md]
  ripr policy history --current target/ripr/reports/policy-operations.json [--history .ripr/policy-history.jsonl] [--commit HEAD] [--pr-number 123] [--out target/ripr/reports/policy-history.json] [--out-md target/ripr/reports/policy-history.md]
  ripr policy promote --to baseline-check --operations target/ripr/reports/policy-operations.json [--history target/ripr/reports/policy-history.json] [--out target/ripr/reports/policy-promotion-baseline-check.json] [--out-md target/ripr/reports/policy-promotion-baseline-check.md]
  ripr policy preview-promote --language typescript --class boundary_gap [--evidence target/ripr/reports/preview-promotion-evidence.json] [--out target/ripr/reports/preview-promotion-typescript-boundary-gap.json] [--out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md]
  ripr policy waiver-aging [--ledger target/ripr/reports/pr-evidence-ledger.json] [--history .ripr/pr-evidence-ledger.jsonl] [--out target/ripr/reports/waiver-aging.json] [--out-md target/ripr/reports/waiver-aging.md]
  ripr policy suppression-health [--root .] [--manifest .ripr/suppressions.toml] [--out target/ripr/reports/suppression-health.json] [--out-md target/ripr/reports/suppression-health.md]

Reports:
  ripr reports index [--reports-dir target/ripr/reports] [--review-dir target/ripr/review] [--out target/ripr/reports/index.json]
  ripr reports gap-ledger --records fixtures/gap-decision-ledger/corpus.json [--out target/ripr/reports/gap-decision-ledger.json]
  ripr reports ts-limitations --check-output <path> [--out target/ripr/reports/ts-limitations.json]
  ripr reports ts-false-actionable --corpus <path> [--out target/ripr/reports/ts-false-actionable.json]
  ripr receipt write --gap <canonical_gap_id> --verify-command "<cmd>" --status <verify_status> [--packet <packet_id>] [--out PATH] [--json]
  ripr receipt check [--path PATH] [--gap <canonical_gap_id>]

What it does:
  Reads changed Rust code, creates mutation-like probes, and estimates whether
  tests appear to reach, infect, propagate, and reveal the changed behavior
  through meaningful oracles. It does not run mutants.

Quick start (one command per group):
  ripr doctor                                             # setup
  ripr check --base origin/main                           # ordinary first value
  ripr agent repair --seam-id ID --phase before           # repair
  ripr first-pr --root . --base origin/main --head HEAD   # PR evidence
  ripr init --ci github                                   # advisory CI
  ripr reports index                                      # reports

Start-here path:
  - `ripr doctor` checks whether the local workspace and config can produce evidence.
  - `ripr check` is the ordinary first-value analysis; `ripr pilot` is the guided repo-adoption workflow.
  - `ripr agent repair` owns the before/edit/after repair transaction; lower-level brief, packet, verify, and receipt commands remain available for control and debugging.
  - `ripr first-pr` and `ripr start-here` compose `target/ripr/reports/start-here.{json,md}` from existing artifacts; they do not run analysis or repair a gap.
  - Safe next action means repair one named gap, regenerate a missing or malformed artifact, or stop on no-action.
  - Missing artifact, stale evidence, wrong root, malformed artifact, and no actionable gap are explicit recovery states.
  - Verify command, receipt command, and receipt path are the static proof rail; receipts are advisory, not runtime adequacy or gate approval.
  - Preview-limited evidence stays syntax-first and advisory, with static limits before repair language.
"#;
