# RIPR-PROP-0013: Actionable Evidence Projection

Status: proposed

Owner: Cross-lane; public surface alignment across badge, PR/CI, CLI first-use, and editor projection

Created: 2026-05-19

Target campaign: Actionable Evidence Projection

Linked specs:

- `RIPR-SPEC-0055`: Actionable evidence projection

Linked ADRs:

- `ADR-0017`: Public surfaces project actionable canonical gaps

## Problem

RIPR's product contract is a repair loop: one changed behavior, one actionable
gap, one focused repair route, one verify command, and one receipt that shows
static movement. Public surfaces should project that same unit.

Today, the public badge is generated from seam-native repo inventory
(`ripr+ 24469`, `ripr 24352`), which is useful as internal inventory pressure
but product-misaligned as a public "work queue" signal. It reads like 24k user
tasks even though seam inventory is not the user-facing repair unit.

## Proposal

Define and implement a cross-surface projection contract:

```text
canonical gap
-> repair route
-> related test / repair target
-> verify command
-> receipt command/state
-> advisory/static/preview boundary
```

All public/user-facing RIPR surfaces (badge, first-pr packet, PR/CI lead
evidence, scorecard lead, and editor packet projections) should count/render
the same unresolved actionable canonical queue. Raw findings, seam-native
inventory, and static limitations remain supporting/internal diagnostics.

## Why now

- RIPR already has canonical gap and repair-oriented evidence artifacts.
- Badge optics currently conflict with first-use product wording.
- Cross-surface convergence reduces user confusion and accidental over-claims.

## Non-goals

- No analyzer semantic expansion.
- No new evidence classes.
- No mutation execution, provider/model calls, or generated tests.
- No manual edits to generated badge endpoints.
- No policy promotion or default CI-blocking changes.

## Exit criteria

This proposal moves to `accepted` when the 10-step implementation sequence is
closed and public surfaces consistently project actionable canonical repair
items while seam-native inventory remains available as internal pressure
telemetry.
