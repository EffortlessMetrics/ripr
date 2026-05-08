# Handoff: Campaign 14 Closeout

Date: 2026-05-08
Branch / PR: `campaign-recommendation-calibration-closeout` / pending
Latest merged PR: #551 `docs: add recommendation calibration workflow`

## Current Work Item

`campaign/recommendation-calibration-closeout`

Campaign 14 made the PR-time recommendation loop measurable:

```text
PR guidance -> calibration expectations -> outcome receipts -> recommendation
calibration report -> reader workflow
```

The campaign did not add gate evaluation, CI blocking, analyzer behavior, LSP
feature expansion, source edits, generated tests, runtime mutation execution,
telemetry, external services, opaque scores, public crate splits, or default
policy gates.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Recommendation quality came before policy gates | #540 opened Recommendation Calibration after the earlier gate-policy branch was closed unmerged, keeping calibrated gates queued for a later lane. |
| The report contract is specified | #541 added RIPR-SPEC-0013, `docs/OUTPUT_SCHEMA.md` sections for recommendation calibration reports and outcome receipts, traceability, capability metadata, and campaign docs. |
| PR-shaped expectations are fixture-pinned | #543 added `fixtures/boundary_gap/expected/recommendation-calibration/expectations.json` plus the calibration corpus index for useful, noisy, wrong-line, already-covered, summary-only, suppression, generated/migration, macro-heavy, trait/generic, and async/error cases. |
| Review outcomes are local receipts | #545 added outcome receipt fixtures for useful, noisy, wrong-line, already-covered, wrong-target, summary-only-correct, and suppressed-correctly outcomes without telemetry or external services. |
| Precision is reported from existing artifacts | #549 added `cargo xtask recommendation-calibration` and checked `recommendation-calibration.{json,md}` outputs that join PR guidance, expectations, optional receipts, suppression state, placement, latency, and static movement. |
| Users have a dedicated calibration guide | #551 added `docs/RECOMMENDATION_CALIBRATION.md`, covering report use, receipt vocabulary, placement quality, suppression correctness, static movement buckets, reviewer workflow, fixture artifacts, and advisory limits. |

## PR Chain

- #540 `campaign: open recommendation calibration`
- #541 `spec: pin recommendation calibration report`
- #543 `fixtures: add recommendation calibration corpus`
- #545 `review-feedback: add recommendation outcome receipts`
- #549 `report: add recommendation calibration report`
- #551 `docs: add recommendation calibration workflow`
- `campaign/recommendation-calibration-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Choose the next product campaign explicitly before adding new behavior.
Campaign 15, Calibrated Gate Policy, is queued in the docs because gates should
be policy over measured recommendation quality; it is not active yet.

## What Not To Do

- Do not reopen the closed #537 gate-policy attempt.
- Do not add gate evaluation or CI blocking under Campaign 14 maintenance.
- Do not add LSP/editor feature work as part of recommendation calibration.
- Do not add automatic source edits, generated tests, or runtime mutation
  execution.
- Do not turn calibration into telemetry, an external service, or an opaque
  score.
- Do not activate Campaign 15 without an explicit product decision.
