# Handoff: Repo-Ops UX Cockpit Closeout

Date: 2026-05-16
Branch / PR: `repo-ops-ux-cockpit-closeout` / pending at authoring
Latest merged PR: #1053 `automation: suggest command catalog ordering fixes`
(commit `1d31f840`)

## Current Work Item

`campaign/repo-ops-ux-cockpit-closeout`

The repo-ops UX follow-up made the generated-evidence discipline lane usable as
one operating flow instead of a set of separate guardrails. The shipped surface
now gives agents and maintainers a front door for local PR readiness,
repo-level queue state, command mutability, merge readiness, deterministic
repair hints, generated-evidence hygiene, and review receipts.

This is repo-operations work. It does not reopen policy semantics or change
analyzer truth, evidence identity, recommendation ranking, LSP/editor behavior,
generated tests, provider behavior, mutation execution, branch protection,
default CI blocking, baseline adoption, suppression creation, preview-language
promotion, or badge endpoint numbers.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| PR triage is agent-readable | `target/ripr/reports/pr-triage.json` is emitted alongside Markdown; commit `9cf2c039` added the JSON packet. |
| Per-PR merge readiness is agent-readable | #1011 added `target/ripr/reports/gh-pr-status.json` for `cargo xtask gh-pr-status --pr <number>`. |
| Repo-ops packets are discoverable | #1015 indexed command mutability, PR-ready, cockpit, worktree doctor, PR triage, merge readiness, generated-clean, badge policy, critic, receipts, suggested fixes, and check-pr packets through `cargo xtask reports index`. |
| Command mutability cannot silently drift | #1018 added `cargo xtask check-command-catalog`, which compares the help catalog and mutability catalog and writes `target/ripr/reports/command-catalog.md`. |
| Agents have a local PR front door | #1025 added `cargo xtask pr-ready`, writing `target/ripr/reports/pr-ready.{md,json}` from worktree doctor, command catalog, PR summary, critic, receipts, suggested fixes, generated-clean, and badge policy. |
| Maintainers have a repo-level front door | #1035 added `cargo xtask cockpit`, writing `target/ripr/reports/cockpit.{md,json}` from board triage, command catalog, campaign/spec checks, generated-evidence hygiene, and badge policy. |
| Merge watching has policy instead of polling folklore | #1036 added `docs/MERGE_WATCH_POLICY.md` for watcher cadence, branch-refresh decisions, REST fallback, Droid/advisory checks, and worktree merge limits. |
| PR triage produces an action queue | #1047 added queue dispositions such as `merge_candidate`, `needs_rebase`, `close_duplicate`, `needs_fresh_validation`, `needs_owner_decision`, and `do_not_touch_wrong_lane`. |
| Suggested fixes cover deterministic repo hygiene | #1039, #1041, #1044, and #1053 expanded `cargo xtask suggested-fixes` to cover docs index ordering, traceability ordering, capability ordering, and command catalog ordering. |
| Suggested fixes still stop at judgment boundaries | `docs/GENERATED_EVIDENCE.md`, `docs/PR_AUTOMATION.md`, and the suggested-fixes report state that patches must not edit badge values, bless goldens, adopt baselines, create suppressions, change dependency exceptions, bump schemas, alter branch protection, or promote preview evidence. |

## PR Chain

- commit `9cf2c039` `devex: emit PR triage JSON`
- #1011 `devex: emit gh pr status JSON`
- #1015 `reports: index repo-ops packets`
- #1018 `devex: check command catalog coverage`
- #1025 `devex: add pr-ready cockpit`
- #1035 `devex: add repo cockpit`
- #1036 `docs: add merge freshness policy`
- #1039 `automation: expand suggested fixes for docs indexes`
- #1041 `automation: suggest traceability ordering fixes`
- #1044 `automation: suggest capability ordering fixes`
- #1047 `devex: add PR triage queue disposition`
- #1053 `automation: suggest command catalog ordering fixes`
- `campaign/repo-ops-ux-cockpit-closeout`

## Verification Run

Load-bearing validation from the final implementation PR and closeout audit:

```bash
cargo test -p xtask command_catalog
cargo test -p xtask suggested_fixes
cargo xtask check-command-catalog
cargo xtask check-generated-clean
cargo xtask check-pr
cargo xtask pr-ready
cargo xtask cockpit
cargo xtask reports index
cargo xtask suggested-fixes
git diff --check
```

Notes:

- `cargo xtask cockpit` is advisory. In a local checkout it can report
  `actionable` when `target/ripr` exists or GitHub PR triage cannot read live
  board state; that is expected operator guidance, not a product failure.
- `cargo xtask suggested-fixes` may produce a target-local patch on current
  `main`. The patch is generated evidence and must be reviewed before applying;
  it never carries badge values or judgment-required changes.

## Artifacts

Primary front doors:

- `target/ripr/reports/cockpit.md`
- `target/ripr/reports/cockpit.json`
- `target/ripr/reports/pr-ready.md`
- `target/ripr/reports/pr-ready.json`
- `target/ripr/reports/index.md`
- `target/ripr/reports/index.json`

Supporting packets:

- `target/ripr/reports/pr-triage.md`
- `target/ripr/reports/pr-triage.json`
- `target/ripr/reports/gh-pr-status.md`
- `target/ripr/reports/gh-pr-status.json`
- `target/ripr/reports/commands.md`
- `target/ripr/reports/commands.json`
- `target/ripr/reports/command-catalog.md`
- `target/ripr/reports/suggested-fixes.md`
- `target/ripr/reports/suggested-fixes.patch`
- `target/ripr/receipts/`

Committed references:

- `docs/GENERATED_EVIDENCE.md`
- `docs/PR_AUTOMATION.md`
- `docs/MERGE_WATCH_POLICY.md`
- `docs/OUTPUT_SCHEMA.md`
- `docs/IMPLEMENTATION_CAMPAIGNS.md`

## Next Work Item

No ready repo-ops UX cockpit work item remains after this closeout.

Future repo-operations work should open explicitly if maintainers want stronger
release-publish rails, merge-queue integration, credential/secret ownership,
or automatic stale-PR disposition. Those are not implied by the cockpit
closeout.

## What Not To Do

- Do not hand-edit `badges/*.json` in ordinary PRs.
- Do not apply `suggested-fixes.patch` blindly; review the deterministic patch
  first.
- Do not let suggested fixes bless goldens, adopt baselines, create
  suppressions, change dependency exceptions, bump schemas, edit badge values,
  alter branch protection, or promote preview-language evidence.
- Do not make `cargo xtask cockpit` close PRs, update branches, comment, merge,
  or mutate generated endpoint JSON.
- Do not turn `cargo xtask pr-ready` into a replacement for `cargo xtask
  check-pr`.
- Do not reopen Lane 2 policy authority for repo-operations UX polish.
