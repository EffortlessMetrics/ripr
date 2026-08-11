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

Before adding a new spec, ask the repo for the next live ID:

```bash
cargo xtask specs next
```

Spec IDs are source-of-truth identifiers, not generated runtime counts. The
helper only prevents stale numbering assumptions; humans still author the spec
contract.

## Proposed-Spec Lifecycle

`check-spec-format` flags a spec that stays `Status: proposed` for more than
90 days without review (#2708): promote it to `accepted`, re-scope it, or add
evidence justifying the status. Accepted and deprecated specs have been
reviewed and are exempt.

The review-age authority is **repository evidence, not filesystem mtime**
(#3035). Git does not preserve tracked-file mtime across clone/checkout, so an
mtime-based check made an old proposed spec look new exactly where the gate
matters. The age is the committer timestamp of the last commit that changed
the spec file, resolved with the exact path-safe invocation:

```bash
git log -1 --format=%ct -- "<spec-path>"
```

Use the exact spec path (for example
`docs/specs/RIPR-SPEC-0108-evidence-promotion-honesty.md`), never a `*`
wildcard: a wildcard can match multiple spec files and Git would return the
timestamp of a different spec's last commit.

Missing age evidence is fail-closed and visible: an untracked spec, an
unavailable repository, a malformed timestamp, or a commit date in the
future beyond tolerated clock skew produces an explicit `not_proven`
finding, never a silent pass.

## Checks

Run:

```bash
cargo xtask check-spec-format
cargo xtask check-spec-numbering
```

The check verifies required sections, status values, title/filename ID
consistency, and the proposed-spec lifecycle above. The numbering guard
verifies that every spec file appears in `docs/specs/README.md` and that
traceability/capability surfaces do not reference missing spec IDs.
