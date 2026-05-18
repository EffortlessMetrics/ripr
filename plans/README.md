# Plans

Plans are PR queues. They sequence implementation work after the proposal,
spec, and ADR sources of truth define why, what, and durable constraints.

Use a plan when a lane needs multiple PR-sized work items, explicit dependency
ordering, proof commands, or rollback notes. Do not use plans for product
motivation or behavior contracts.

## Role in the source-of-truth stack

```text
Roadmap -> Proposal -> Spec -> ADR -> Plan -> Active goal -> PR -> Proof
```

Plans own:

- work-item order;
- dependencies and blockers;
- per-item production deltas;
- proof commands;
- rollback notes;
- handoff/closeout pointers.

Plans do not own:

- product rationale, which belongs in `docs/proposals/`;
- behavior requirements, which belong in `docs/specs/`;
- durable architecture decisions, which belong in `docs/adr/`;
- live machine-readable execution state, which belongs in `.ripr/goals/active.toml`;
- generated status truth, which belongs in generated receipts or status docs.

## Work item shape

Each work item should include:

```text
## Work item: short-id

Status: ready | active | blocked | completed | superseded
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

Proof commands are required. If a command is advisory, expensive, or unavailable
in a local environment, say so in the work item rather than letting agents guess.

## Current plan directories

- `campaign-27/` — language adapter preview lane notes.
- `editor-first-pr-bridge/` — editor first-PR bridge implementation plan.
- `editor-first-run-usability/` — editor first-run usability implementation
  plan.
- `editor-gap-cockpit/` — editor gap cockpit implementation plan.
- `lane4-pr-ci-review-cockpit/` — PR/CI review cockpit implementation plan and
  generated gap maps.
- `rust-usable-gap-projection/` — Rust usable gap projection implementation
  plan.
