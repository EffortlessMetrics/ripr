# RIPR-SPEC-0055: Actionable Evidence Projection

Status: proposed

## Problem

Public RIPR surfaces are most useful when they answer one shared question:

```text
What is the next user-actionable static test-gap repair?
```

Current badge projection can lead with seam-native inventory counts, which are
valuable internally but not equivalent to user-actionable repair work.

## Source-of-truth model

- Raw finding: analyzer evidence, not direct user work.
- Canonical evidence item: countable evidence unit that may be actionable,
  no-action, static-limited, or supporting.
- Actionable canonical gap: user-facing repair unit.
- Public projection: stable surface that renders counts/summaries/packets.
- Internal inventory: seam-native/raw/static-limitation pressure gauges.

## Contract

Every public/user-facing projection should use one actionable unit:

```text
canonical gap
-> repair route
-> related test/repair target
-> verify command
-> receipt command/state
-> advisory/static/preview boundary
```

### Public projection eligibility

A canonical item is public-actionable only when all conditions hold:

- `gap_state = unresolved`
- `actionability = actionable`
- repair route exists
- related test/repair target exists, or explicitly unknown with safe fallback
- verify command exists
- receipt command/state exists or can be emitted
- not suppressed
- not intentional
- not no-action
- not preview-only unless explicitly promoted
- not runtime-only
- not raw seam inventory
- not static limitation without actionability

### Badge contract

- `ripr`: unresolved actionable canonical gaps eligible for public projection.
- `ripr+`: `ripr` count plus actionable test-efficiency repair items when
  projected in the same repair/verify/receipt model.

Seam-native counts remain available as:

```text
internal_inventory.seam_native_count
```

and must not be the default public badge message.

## Required outputs

Native badge summary should expose `basis = canonical_actionable_gap` and a
decomposition of canonical/actionable/internal counts. Shields output remains
the 4-field schema (`schemaVersion`, `label`, `message`, `color`).

`cargo xtask badge-basis` should write:

- `target/ripr/reports/badge-basis.md`
- `target/ripr/reports/badge-basis.json`

including current basis, recommended basis, actionable counts, internal
inventory counts, exclusions, and advisory/preview exclusions.

## Surface alignment requirements

- LSP/editor packets must render one canonical actionable gap with repair,
  target, verify, receipt, and boundary fields, and fail closed on unsafe
  inputs.
- PR/CI evidence must lead with actionable canonical gaps and supporting
  repair/verify/receipt data; raw findings are below-the-fold diagnostics.
- First-pr packet must preserve the same actionable unit and non-claims.
- Public badge must count the same actionable queue.

## Hard boundaries

- No analyzer semantics expansion.
- No new evidence class taxonomy.
- No mutation execution/provider calls/generated tests.
- No source edits or policy promotion as part of this projection contract.
- No manual badge endpoint edits.

