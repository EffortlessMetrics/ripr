# Plans

Plans sequence PR-sized work for an accepted lane. They answer **how this work
lands next**; they do not replace proposals, specs, ADRs, support-tier proof, or
policy ledgers.

Read [`docs/reference/SPEC_SYSTEM.md`](../docs/reference/SPEC_SYSTEM.md) before
adding or reshaping plan artifacts.

## Role

Plans own:

- current factual baseline for the lane;
- work-item order and dependencies;
- acceptance criteria for each PR-sized slice;
- proof commands;
- rollback notes;
- handoff and closeout pointers.

Plans do not own:

- product motivation, which belongs in `docs/proposals/`;
- behavior contracts, which belong in `docs/specs/`;
- durable architecture decisions, which belong in `docs/adr/`;
- generated metrics or status truth, which belongs in generated reports,
  receipts, and status docs.

## Layout

Use a lane directory for multi-PR work:

```text
plans/<lane>/
  README.md
  implementation-plan.md
  closeout.md
```

Small historical plans may keep their existing shape, but new lanes should keep
implementation sequencing under `plans/<lane>/implementation-plan.md` and link
from `.ripr/goals/active.toml`.

## Work item checklist

Each ready work item should name:

- status;
- linked proposal;
- linked spec;
- linked ADR when one constrains the work;
- blockers and blocked-by relationships;
- goal;
- production or docs delta;
- non-goals;
- acceptance criteria;
- proof commands;
- rollback path;
- claim boundary.

Agents should implement exactly one ready work item per PR unless the plan says
otherwise.
