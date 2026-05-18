# Implementation plans

Implementation plans are the sequencing layer of the repo source-of-truth stack.
They answer what PR-sized work lands next after the roadmap, proposal, specs,
and ADRs have defined the why, what, and durable constraints.

For the full stack, read [`docs/reference/SPEC_SYSTEM.md`](../docs/reference/SPEC_SYSTEM.md).

## Role

Plans own:

- work-item order;
- dependencies and blockers;
- production delta for each PR-sized slice;
- acceptance criteria;
- proof commands;
- rollback notes;
- handoff or closeout status.

Plans do not own:

- product rationale, which belongs in `docs/proposals/`;
- behavior contracts, which belong in `docs/specs/`;
- durable architecture decisions, which belong in `docs/adr/`;
- generated status truth, which belongs in generated reports or status docs.

## Current plan folders

- [`campaign-27/`](campaign-27/) - Campaign 27 language-adapter preview lane notes.
- [`editor-first-pr-bridge/`](editor-first-pr-bridge/) - editor first-PR bridge implementation plan.
- [`editor-first-run-usability/`](editor-first-run-usability/) - editor first-run usability implementation plan.
- [`editor-gap-cockpit/`](editor-gap-cockpit/) - editor gap cockpit implementation plan.
- [`lane4-pr-ci-review-cockpit/`](lane4-pr-ci-review-cockpit/) - Lane 4 PR / CI review cockpit implementation plan and generated audit notes.
- [`rust-usable-gap-projection/`](rust-usable-gap-projection/) - Rust usable gap projection implementation plan.

## Plan item checklist

Each work item should make these fields easy to find:

- status;
- linked proposal;
- linked spec;
- linked ADR when applicable;
- blockers and blocked-by relationships;
- goal;
- production delta;
- non-goals;
- acceptance;
- proof commands;
- rollback;
- claim boundary or notes.

If a plan starts explaining why a lane exists, move that text to a proposal. If
it starts defining observable behavior in detail, move that text to a spec.
