# Handoff: Campaign 15 Closeout

Date: 2026-05-08
Branch / PR: `campaign-calibrated-gate-closeout` / pending
Latest merged PR: #566 `docs: add calibrated gate policy guide`

## Current Work Item

`campaign/calibrated-gate-closeout`

Campaign 15 made calibrated gates an explicit, optional policy layer over
existing PR-time evidence:

```text
PR guidance -> recommendation calibration -> gate decision -> optional CI policy
```

The campaign did not add analyzer behavior, LSP feature expansion, runtime
mutation execution, generated tests, source edits, PR comment posting, telemetry,
external services, public crate splits, or default CI blocking.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Gate policy was specified before implementation | #559 added RIPR-SPEC-0014, the output schema contract, traceability, capability metadata, and campaign docs for visible-only, acknowledgeable, baseline-check, and calibrated-gate modes. |
| The evaluator is read-only | #560 added `ripr gate evaluate`, `crates/ripr/src/output/gate.rs`, CLI routing, JSON/Markdown gate decisions, and tests for advisory, acknowledged, blocked, and config-error decisions without posting comments, editing source, or running mutation tests. |
| The decision matrix is fixture-pinned | #561 added `fixtures/boundary_gap/expected/calibrated-gate` cases for visible-only advisory, acknowledged waivers, baseline-check existing gaps, high-confidence blocking, summary/suppression handling, missing inputs, and calibration disagreement. |
| Generated CI remains advisory by default | #563 wired generated GitHub workflows to run the evaluator only when `RIPR_GATE_MODE` is explicitly configured, preserving default advisory behavior; #564 kept gate evidence artifacts uploaded even when a configured gate reports blocking. |
| Users have a dedicated guide | #566 added `docs/CALIBRATED_GATE_POLICY.md`, linked it from CI, PR guidance, recommendation calibration, roadmap, implementation plan, documentation index, and capability metadata, and clarified that waivers are visible acknowledgements rather than silent success. |

## PR Chain

- #554 `campaign: open calibrated gate policy`
- #559 `spec: define calibrated gate policy`
- #560 `gate: add calibrated policy evaluator`
- #561 `fixtures: pin calibrated gate cases`
- #563 `ci: wire optional generated gate decision`
- #564 `ci: preserve gate evidence uploads`
- #566 `docs: add calibrated gate policy guide`
- `campaign/calibrated-gate-closeout`

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

Choose the next product campaign explicitly before adding new behavior. Campaign
15 maintenance should be limited to correctness fixes for the gate policy
surface already specified, evaluated, fixture-pinned, documented, and kept
advisory by default.

## What Not To Do

- Do not make generated CI blocking by default.
- Do not run mutation testing from the gate evaluator.
- Do not treat imported mutation calibration as runtime adequacy.
- Do not hide waived findings; waivers remain visible acknowledged decisions.
- Do not add LSP/editor feature work as part of Campaign 15 maintenance.
- Do not add automatic source edits, generated tests, PR comment posting,
  telemetry, external services, or public crate splits.
