# Spec Format

Specs are behavior contracts for humans, tests, tools, and future agents. They
should be consistent enough to parse mechanically.

## Status Values

Allowed statuses:

- `proposed`
- `planned`
- `accepted`
- `deprecated`

## Required Sections

Every spec in `docs/specs/RIPR-SPEC-*.md` must include:

- `Status: ...`
- `## Problem`
- `## Behavior`
- `## Required Evidence`
- `## Non-Goals`
- `## Acceptance Examples`
- `## Test Mapping`
- `## Implementation Mapping`
- `## Metrics`

Accepted specs should have concrete acceptance examples and at least one current
or planned test mapping. Planned specs may point at planned tests and planned
modules, but they still need the same sections so agents can reason over the
gap.

## IDs

Spec filenames and titles use stable IDs:

```text
docs/specs/RIPR-SPEC-0004-predicate-boundary-activation.md
# RIPR-SPEC-0004: Predicate Boundary Activation
```

Use these IDs in tests, fixtures, traceability entries, metrics, and PR
summaries when behavior changes.

## Checks

Run:

```bash
cargo xtask check-spec-format
```

The check verifies required sections, status values, and title/filename ID
consistency.
