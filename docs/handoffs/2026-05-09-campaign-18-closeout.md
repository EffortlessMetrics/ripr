# Handoff: Campaign 18 Closeout

Date: 2026-05-09
Branch / PR: `campaign-ripr-zero-reporting-closeout` / #614
Latest merged PR: #613 `docs: add RIPR Zero reporting workflow` (commit `8838aa7`)

## Current Work Item

`campaign/ripr-zero-reporting-closeout`

Campaign 18 made RIPR Zero reporting operational without changing the default
advisory posture:

```text
reviewed baseline ledger
-> additive baseline metadata
-> baseline debt delta report
-> read-only RIPR Zero status report
-> generated CI first-screen summary
-> user reporting workflow docs
```

The campaign did not change analyzer behavior, recommendation ranking, gate
policy semantics, generated workflow defaults, LSP/editor behavior, mutation
execution, automatic source edits, generated tests, public crate shape, or
release/security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #608 opened Campaign 18 as RIPR Zero Reporting, separate from Campaign 17 adoption mechanics. |
| Reporting contract exists | #609 added RIPR-SPEC-0017, output-schema coverage, capability metadata, and traceability for repo-level RIPR Zero status, baseline metadata health, stale warnings, trends, top debt areas, and repair routing without analyzer identity rewrites or default CI blocking. |
| Baseline metadata is preserved | #610 added additive owner, reason, created, review-after, and source metadata support across baseline create, diff, and shrink-only update while preserving compatibility with Campaign 17 ledgers. |
| Status report exists | #611 added `ripr zero status`, a read-only JSON/Markdown report over baseline debt deltas, reviewed baselines, optional gate decisions, PR guidance, and recommendation calibration evidence. |
| Generated CI surfaces status without owning pass/fail | #612 runs `ripr zero status` when `baseline-debt-delta.json` exists, uploads `ripr-zero-status.{json,md}`, and appends a RIPR Zero summary while leaving `ripr gate evaluate` as the pass/fail authority. |
| Users have a reporting workflow | #613 added `docs/RIPR_ZERO_REPORTING_WORKFLOW.md`, explaining how to read status, age baseline metadata, route repair packets, and interpret progress without treating RIPR 0 as perfect tests or 100 percent coverage. |
| Defaults remain advisory | Campaign docs, generated workflow tests, output contracts, CI docs, and capability metadata keep RIPR Zero status as advisory progress evidence. Gate decisions remain explicit and opt-in. |

## PR Chain

- #608 `campaign: open RIPR Zero reporting`
- #609 `spec: define RIPR Zero reporting surface`
- #610 `baseline: preserve review metadata`
- #611 `report: add RIPR Zero status`
- #612 `ci: surface RIPR Zero status`
- #613 `docs: add RIPR Zero reporting workflow`
- `campaign/ripr-zero-reporting-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 18.
Choose the next campaign explicitly before opening another product lane.

A likely later Lane 4 campaign is a PR evidence ledger or RIPR Zero progress
history: PR-level movement over time, waiver aging, baseline burn-down, and
coverage/grip frontier summaries. That should be opened as a new explicit
campaign rather than folded into Campaign 18 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not treat RIPR Zero status as the pass/fail authority.
- Do not treat RIPR 0 as perfect tests, 100 percent coverage, or runtime
  mutation confirmation.
- Do not hide acknowledged, suppressed, stale, invalid, or missing-input
  entries from summaries.
- Do not auto-adopt new current debt into baselines.
- Do not auto-refresh or rewrite baselines in generated CI.
- Do not move analyzer identity, recommendation ranking, or gate policy
  semantics into reporting closeout work.
- Do not run cargo-mutants or any mutation engine from reporting workflows.
- Do not generate tests, edit source, or call LLM providers from reporting
  surfaces.
