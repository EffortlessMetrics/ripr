# Implementation plans

Implementation plans are durable sequencing guides. They break an accepted
proposal/spec direction into PR-sized work items after the proposal, spec, and
any durable ADRs have established why, what, and constraints.

Plans answer:

- what dependency should land before another slice;
- which artifact or behavior a PR changes;
- what is blocked by or blocks the work item;
- which proof commands are required;
- how to roll the PR back;
- what status or handoff note should survive the session.

Plans do not select a current worker or issue. They also do not own product
rationale, behavior contracts, durable architecture decisions, generated status
truth, support claims, or policy exceptions. Move that content to the linked
proposal, spec, ADR, generated report, support-tier row, or policy ledger.

## Plan shape

A lane plan should include:

```text
# Lane implementation plan

Status:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Live issue / PR:

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

## Execution state

Live execution comes from GitHub issues, pull requests, checks, reviews, and the
local worktree. A plan may describe the intended dependency order and may record
historical status, but it does not make one of its work items active merely by
saying so.

The former `.ripr/goals/active.toml` scheduler and `cargo xtask goals ...`
commands were retired and deleted. Do not recreate them or use historical plan
text as a substitute for the current GitHub/worktree state. When resuming work,
read the controlling issue, current PR/head/checks/reviews, the relevant spec or
ADR, and this plan only for durable sequence/context. See
[`docs/REPO_TRACKING_MODEL.md`](../docs/REPO_TRACKING_MODEL.md).

## Plan Index

These entries are durable plan artifacts. Their own status fields describe the
plan artifact and its historical/declared sequence; GitHub/worktree state decides
what is being executed now. Any retained `Active goal` header pointing to
`.ripr/goals/active.toml` is historical, not a live link or execution instruction.
This includes headers in plans linked below; following a link does not restore
the retired scheduler's authority.

### Proposed Plans

- [Use-case spec layer](use-case-specs/implementation-plan.md)
  (proposed; sequences the RIPR-SPEC-0065 through RIPR-SPEC-0073
  implementation slices after the spec set lands)

### Retained Plans With Historical Execution Headers

- [Python repair routing](python-repair-routing/implementation-plan.md)
  (partially delivered historical work-item ledger; its delivery note and
  linked promotion issue distinguish landed work from the remaining scope;
  the former active-manifest reference is historical)
- [Cross-language evidence router UX](cross-language-evidence-router-ux/implementation-plan.md)
  (retained implementation record; its `Active goal` pointer to the deleted
  manifest is historical and does not select current work)

### Closed or Complete Plans

- [TypeScript preview completion](typescript-preview-completion/implementation-plan.md)
  (closed lane plan; not a support-tier promotion)
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

For docs-only plan/control changes, run at minimum:

```bash
git diff --check
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
```

Run the proof commands listed by the controlling issue and specific work item
before claiming a branch is ready.
