# Handoff: Campaign 22 Closeout

Date: 2026-05-09
Branch / PR: `campaign-first-useful-action-closeout` / pending at authoring
Latest merged PR: #647 `dogfood: add first useful action receipts` (commit `4fdb65a`)

## Current Work Item

`campaign/first-useful-action-closeout`

Campaign 22 compressed the growing RIPR report stack into one advisory next
action without making that action the policy authority:

```text
editor, PR, ledger, proof, receipt, optional gate, coverage/grip, and staleness evidence
-> first-useful-action.{json,md}
-> generated CI summary and artifact projection
-> editor status projection
-> dogfood receipts
```

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, generated workflow defaults, LSP diagnostics, CodeLens, inlay
hints, unsaved-buffer analysis, source-edit behavior, generated-test behavior,
provider calls, mutation execution, public crate shape, release posture, or
security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #633 opened Campaign 22 as First Useful Action, separate from assistant proof reports and future health/trend reporting. |
| Report contract exists before implementation | #636 added RIPR-SPEC-0020, output-schema coverage, capability metadata, traceability, and campaign docs for an advisory first-action report over existing artifacts. |
| Routing corpus is pinned | `fixtures/boundary_gap/expected/first-useful-action/` covers actionable, stale, missing-required-artifact, baseline-only, acknowledged, waived, suppressed, no-actionable-seam, already-improved, and unchanged-after-attempt cases. |
| Read-only producer exists | #639 added `ripr first-action`, JSON/Markdown rendering, explicit artifact input parsing, fixture tests, and CLI smoke coverage without hidden analysis reruns. |
| Generated CI projection exists | #640 and follow-up tightening run `ripr first-action` only when explicit inputs are present, upload `first-useful-action.{json,md}` with the normal report packet, and append an advisory summary without changing pass/fail authority. |
| Editor projection exists | #643 projects an existing `target/ripr/reports/first-useful-action.json` through VS Code status and `ripr: Show Status` without invoking `ripr first-action`, adding diagnostics, or editing source. |
| Reader-facing workflow docs exist | #645 added `docs/FIRST_USEFUL_ACTION_WORKFLOW.md`, explaining GitHub and editor entry points, status meanings, developer/reviewer/agent actions, verification, receipts, fallback states, and advisory limits. |
| Dogfood receipts document routed states | #647 extended `cargo xtask dogfood` with checked repo-local receipts for actionable, baseline-only, stale, missing-required-artifact, unchanged-after-attempt, and no-actionable-seam cases. |
| Capability and traceability surfaces are updated | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `.ripr/traceability.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, and `docs/ROADMAP.md` point to the closed Campaign 22 evidence package. |
| Future lane boundary is explicit | Assistant Loop Health remains a proposed future campaign and is not folded into Campaign 22 closeout. |

## PR Chain

- #633 `campaign: open first useful action`
- #636 `spec: define first useful action report`
- `fixtures/first-useful-action-corpus`
- #639 `report: add first useful action producer`
- #640 `ci: surface first useful action report`
- #643 `lsp: surface first useful action status`
- #645 `docs: add first useful action workflow`
- #647 `dogfood: add first useful action receipts`
- `campaign/first-useful-action-closeout`

## Verification Run

Closeout validation before opening this PR:

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

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 22 after
this closeout.

Choose the next campaign explicitly before opening another product lane. The
likely follow-up is Assistant Loop Health: a read-only report over existing
assistant proof artifacts that measures proof completeness, missing inputs,
static evidence movement, recurring warnings, and bounded repair queues. That
should be opened as a separate campaign rather than folded into First Useful
Action closeout.

Open PR #644, `lsp: harden first useful action status projection`, remains a
separate dirty LSP follow-up branch and should be reconciled independently from
Campaign 22 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make first-action reports the pass/fail authority.
- Do not claim runtime mutation outcomes, adequacy, correctness, or proof from
  static evidence.
- Do not hide stale, missing-input, acknowledged, waived, suppressed,
  already-improved, or no-actionable fallback states.
- Do not run cargo-mutants or any mutation engine from first-action workflows.
- Do not move analyzer identity, recommendation ranking, gate policy
  semantics, or broad editor behavior into closeout work.
- Do not add diagnostics, CodeLens, inlay hints, unsaved-buffer overlays,
  source edits, generated tests, inline comments, or provider calls from the
  first-action surface.
