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

## Execution State

The active execution queue is `.ripr/goals/active.toml`. When that manifest
records `status = "closed"` and `no_current_goal = true`, no plan listed here is
automatically active. Select a successor from repo-owned roadmap, proposal,
spec, ADR, issue, or campaign state before starting behavior work.

## Plan Index

These entries are durable plan artifacts. Their own status fields decide whether
they are active, closed, complete, blocked, or historical.

### Proposed Plans

- [Python repair routing](python-repair-routing/implementation-plan.md)
  (proposed, not active until explicitly selected)
- [TypeScript preview completion](typescript-preview-completion/implementation-plan.md)
  (proposed lane plan; preview/advisory boundary preserved)

### Closed or Complete Plans

- [Actionable surface translation](actionable-surface-translation/implementation-plan.md)
  (closed cross-surface translation rail)
- [Editor actionable gap queue](editor-actionable-gap-queue/implementation-plan.md)
- [Editor adoption assurance](editor-adoption-assurance/implementation-plan.md)
- [Editor first-pr bridge](editor-first-pr-bridge/implementation-plan.md)
- [Editor first-run usability](editor-first-run-usability/implementation-plan.md)
- [Editor gap cockpit](editor-gap-cockpit/implementation-plan.md)
- [First Useful PR Loop](first-useful-pr-loop/implementation-plan.md)
- [Lane 1 finding alignment burn-down](lane1-finding-alignment-burndown/implementation-plan.md)
- [Lane 1 value resolution audit fixes](lane1-value-resolution-audit-fixes/implementation-plan.md)
- [Lane 4 PR / CI review cockpit](lane4-pr-ci-review-cockpit/README.md)
- [Rust usable gap projection](rust-usable-gap-projection/README.md)
- [Source-of-truth control plane](source-of-truth/implementation-plan.md)
- [Start-here surface convergence](start-here-surface-convergence/implementation-plan.md)

### Historical Cleanup Rails

- [Adoption integration cleanup](adoption-integration-cleanup/implementation-plan.md)
  (closed historical cleanup rail)
- [Campaign 27](campaign-27/README.md)
  (closed preview-language campaign with historical blocked follow-ups)

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
