# Active goals

This directory contains machine-readable campaign manifests for `ripr` agents.
The current execution pointer is [`active.toml`](active.toml); closed or
superseded manifests move to [`archive/`](archive/).

Read [`docs/reference/SPEC_SYSTEM.md`](../../docs/reference/SPEC_SYSTEM.md) for
the source-of-truth stack.

## Role

The active goal owns:

- current lane identity;
- active or paused state;
- linked proposal, specs, ADRs, and plan;
- current objective;
- ready/active/blocked/completed work items;
- proof commands;
- claim boundaries;
- status or receipt pointers.

The active goal does not own long design prose, generated metrics, support-tier
truth, or durable architecture decisions. Link to those artifacts instead.

## Agent rules

Agents should:

1. read `active.toml` before choosing work;
2. follow the linked plan and spec;
3. pick exactly one ready work item;
4. run the listed proof commands;
5. stop instead of inventing work when no ready item exists;
6. archive old manifests before activating a replacement lane.

Use `status = "paused"` with a reason when no lane is executable.
