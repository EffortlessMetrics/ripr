# ADR 0017: Public Surfaces Project Actionable Canonical Gaps

Status: proposed

Date: 2026-05-19

## Context

RIPR emits multiple evidence layers: raw findings, seam-native inventory,
canonical items, actionable gaps, static limitations, and supporting advisory
signals. Public/user-facing surfaces are easiest to trust when they present one
repair-oriented unit, not mixed counting bases.

## Decision

Public/user-facing RIPR surfaces should project actionable canonical evidence as
the primary unit.

Preferred projection chain:

```text
canonical gap
-> repair route
-> related test / repair target
-> verify command
-> receipt command/state
-> advisory/static/preview boundary
```

Raw findings and seam-native inventory remain supporting/internal diagnostics.

## Consequences

- Public badges must use actionable canonical projection basis.
- First-pr packet, PR/CI lead evidence, and editor/LSP projected actions should
  converge on the same unit.
- Internal inventory pressure is preserved in internal reports and should not be
  presented as default user task counts.
- Unsafe or stale states should fail closed for repair actions.

## Non-goals

- No analyzer behavior expansion.
- No new evidence taxonomy.
- No mutation execution or runtime proof claims.
- No policy promotion or default CI blocking change.
