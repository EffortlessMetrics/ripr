# Spec Format

Specs are behavior contracts for humans, tests, tools, and future agents. They
should be consistent enough to parse mechanically.

## Status Values

Allowed statuses:

- `proposed`
- `planned`
- `accepted`
- `deprecated`

A spec status records the document's normative disposition. It does not claim
that runtime behavior is implemented, evidence is current or sufficient,
support is promoted, or work is active. Those states belong to implementation
claims and PR-local slices, traceability and evidence, support authorities, and
live GitHub/worktree state respectively.

The following authorities remain separate:

| Concern | Authority |
| --- | --- |
| Legacy document identity and disposition | This Markdown document and this format contract |
| Requirement lifecycle and ancestry | The v2 dialect defined by #1667 |
| One-PR implementation claim | `ImplementationSliceV1` |
| Evidence and proof currentness | Traceability/evidence edges and exact receipts |
| Support claim | `docs/status/SUPPORT_TIERS.md` |
| Live work | GitHub issues/PRs/checks/reviews and the local worktree |
| Maintenance attention | Advisory reports and content-bound review receipts |

Specs do not expire because time passes or because their files remain
unchanged. If a contract is wrong, rejected, replaced, or retired, update, supersede, or
deprecate it explicitly. Review dates may be useful descriptive context, but
they do not determine spec validity or merge eligibility.

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

Before adding a new spec, ask the repo for the next live ID:

```bash
cargo xtask specs next
```

Spec IDs are source-of-truth identifiers, not generated runtime counts. The
helper only prevents stale numbering assumptions; humans still author the spec
contract.

## Checks

Run:

```bash
cargo xtask check-spec-format
cargo xtask check-spec-numbering
```

The format check is a deterministic validation of the candidate repository
state. It verifies required sections, status values, and title/filename ID
consistency without consulting wall-clock time, filesystem modification time,
or Git history. The numbering guard verifies that every spec file appears in
`docs/specs/README.md` and that traceability/capability surfaces do not
reference missing spec IDs.
