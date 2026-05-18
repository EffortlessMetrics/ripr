# ADR 0015: Start-Here Surfaces Use Canonical Gap Records

Status: proposed

Date: 2026-05-17

## Context

RIPR now emits and projects many useful artifacts: findings, evidence records,
canonical gaps, gap decision ledgers, repair cards, first-useful-action reports,
first-pr packets, receipts, policy overlays, and editor diagnostics. Users
experience those as one workflow only when the surfaces choose the same unit of
action.

Raw finding counts are useful evidence, but they are not the safest first
screen for a user trying to repair one test-intent gap.

## Decision

Surfaces that present a "start here" or "what next" view should prefer
canonical gap records and repair routes over raw findings or prose-derived
state.

The preferred identity chain is:

```text
canonical_gap_id / gap_id / seam_id / finding_id
-> gap_state
-> repair_route
-> related_test
-> verify_command
-> receipt_command
-> receipt_state
```

Typed fields are authoritative for action safety. Markdown and human text are
display artifacts, not parser inputs for action semantics.

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| Lead with raw finding counts. | Counts help triage but do not tell users where to write the focused test or how to prove movement. |
| Let each surface pick its own action unit. | That preserves local implementation freedom but creates fragmented user workflows. |
| Parse Markdown start-here prose for actions. | Markdown is a human handoff artifact and is too fragile for action safety. |
| Use editor projection as the single source of truth. | The editor consumes artifacts; PR/CI, CLI, policy, and release surfaces need their own typed contracts. |

## Consequences

- PR/CI summaries, CLI front doors, editor projection, first-pr packets, and
  dogfood receipts should converge on the same action unit.
- Raw finding lists remain supporting evidence and debug material.
- Missing, stale, wrong-root, malformed, unsupported, unsafe, disabled, and
  preview-limited states fail closed before stronger repair claims appear.
- Preview-language promotion remains policy-owned and cannot be inferred from
  routing or parser support.
- Future report changes should preserve the spec-test-code-output chain before
  changing first-screen behavior.

## Non-goals

- This ADR does not change analyzer behavior.
- This ADR does not introduce a new output schema by itself.
- This ADR does not make PR/CI summaries gate authorities.
- This ADR does not promote preview-language evidence.
- This ADR does not make the editor create PR/CI reports.
