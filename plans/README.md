# Implementation plans

Implementation plans are execution queues. They sequence PR-sized work items
for a lane after the proposal, spec, and any durable ADRs have established why,
what, and constraints.

Plans answer:

- what PR lands next;
- which artifact or behavior the PR changes;
- what is blocked by or blocks the work item;
- which proof commands are required;
- how to roll the PR back;
- what status or handoff note should survive the session.

Plans do not own product rationale, behavior contracts, durable architecture
decisions, generated status truth, support claims, or policy exceptions. Move
that content to the linked proposal, spec, ADR, generated report, support-tier
row, or policy ledger.

## Plan shape

A lane plan should include:

```text
# Lane implementation plan

Status:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:

## Current state

## Work item: <short-id>

Status: ready | active | blocked | done | superseded
Linked proposal:
Linked spec:
Linked ADR:
Blocks:
Blocked by:

### Goal
### Production delta
### Non-goals
### Acceptance
### Proof commands
### Rollback
### Notes
```

Use `n/a` when a field does not apply. Keep each work item narrow enough for one
focused PR unless the linked plan explains why a larger evidence package is
safer.

## Current plans

- [Campaign 27](campaign-27/README.md)
- [Editor first-pr bridge](editor-first-pr-bridge/implementation-plan.md)
- [Editor first-run usability](editor-first-run-usability/implementation-plan.md)
- [Editor gap cockpit](editor-gap-cockpit/implementation-plan.md)
- [Lane 4 PR / CI review cockpit](lane4-pr-ci-review-cockpit/README.md)
- [Rust usable gap projection](rust-usable-gap-projection/README.md)

## Validation

For docs-only plan changes, run at minimum:

```bash
git diff --check
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-pr-shape
```

Run the proof commands listed by the specific work item before claiming the
branch is ready.
