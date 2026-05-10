# Report Packet Index Proposal

Campaign: 25, Report Packet Index

## Problem

Lane 4 now has the PR review front panel, first useful action, assistant proof
and health, PR evidence ledger, baseline/RIPR Zero reports, gate decisions,
coverage/grip frontier, validation reports, receipts, SARIF, and badge outputs.
The generated CI summary shows the most important PR story, but the uploaded
`ripr-reports` packet is still a collection of directories and files.

Reviewers should not have to know the internal report topology to answer:

```text
Where do I start?
Which report explains the PR story?
Which artifact is the gate authority?
Which packet routes a repair to a human or agent?
Which receipt proves evidence moved?
Which expected artifact is missing, and what command regenerates it?
```

The existing repo-local `cargo xtask reports index` writes
`target/ripr/reports/index.{json,md}`, but it predates the current Lane 4
artifact stack. Campaign 25 should make the index a reviewer-grade artifact
packet map.

## Goal

Make `target/ripr/reports/index.{json,md}` the front door for the uploaded
RIPR report packet.

The index should be advisory and read-only. It should consume explicit existing
artifacts and group them by reviewer use:

```text
Start here
PR review story
Repair and agent handoff
Evidence and movement
Policy and gates
Calibration
Validation receipts
SARIF and badges
```

It should also identify missing expected surfaces and suggest concrete commands
to regenerate them.

## Current Inputs

The campaign should account for these existing surfaces when present:

- `target/ripr/reports/pr-review-front-panel.{json,md}`
- `target/ripr/reports/first-useful-action.{json,md}`
- `target/ripr/reports/test-oracle-assistant-proof.{json,md}`
- `target/ripr/reports/assistant-loop-health.{json,md}`
- `target/ripr/reports/pr-evidence-ledger.{json,md}`
- `target/ripr/reports/baseline-debt-delta.{json,md}`
- `target/ripr/reports/ripr-zero-status.{json,md}`
- `target/ripr/reports/gate-decision.{json,md}`
- `target/ripr/reports/recommendation-calibration.{json,md}`
- `target/ripr/reports/mutation-calibration.{json,md}`
- `target/ripr/reports/coverage-grip-frontier.{json,md}`
- `target/ripr/reports/operator-cockpit.{json,md}`
- `target/ripr/reports/pr-summary.md`
- `target/ripr/reports/critic.{json,md}`
- `target/ripr/receipts/*`
- `target/ripr/review/comments.{json,md}`
- `target/ripr/workflow/*`
- `target/ripr/agent/*`
- `target/ripr/pilot/*`
- `target/ci/*`

## End State

- The index has a stable JSON/Markdown contract.
- The index groups reports by reviewer use rather than filename order.
- The index names the recommended start-here report.
- Missing expected surfaces are visible and include rerun commands.
- Gate decision remains the only optional pass/fail authority.
- Waived, acknowledged, suppressed, baseline, missing-input, stale, warning,
  blocked, and no-action states remain visible.
- Generated CI uploads and summarizes the index only when inputs exist.
- The implementation is fixture-pinned before producer changes.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy changes.
- No LSP/editor behavior changes.
- No provider calls.
- No source edits.
- No generated tests.
- No mutation execution.
- No inline comment publishing.
- No default CI blocking.
- No hidden analysis reruns.
- No artifact discovery that changes upstream report semantics.

## Proposed PR Sequence

1. `spec: define report packet index contract`
2. `fixtures: pin report packet index corpus`
3. `report: update report packet index`
4. `ci: surface report packet index summary`
5. `docs: explain report packet index workflow`
6. `dogfood: add report packet index receipts`
7. `campaign: close report packet index`

## Validation Baseline

Campaign slices should use the scoped commands from `.ripr/goals/active.toml`.
The campaign closeout should rerun:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```
