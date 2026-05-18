# Active goals

`.ripr/goals/active.toml` is the machine-readable current-work pointer for
agents and repo automation. It is part of the linked source-of-truth stack
described in [`docs/reference/SPEC_SYSTEM.md`](../../docs/reference/SPEC_SYSTEM.md).

## Role

The active goal owns:

- current lane identity;
- objective;
- end-state checklist;
- work-item status;
- proof commands;
- claim boundaries;
- pointers to plans, specs, ADRs, status docs, and archives.

The active goal does not own:

- long-form product rationale;
- behavior contracts;
- durable decisions;
- generated metrics or status tables;
- broad support claims.

Use linked proposals, specs, ADRs, plans, support docs, and policy ledgers for
those truths.

## Agent boot order

1. Read `AGENTS.md`.
2. Read `docs/reference/SPEC_SYSTEM.md`.
3. Read `.ripr/goals/active.toml`.
4. If the goal is active, select exactly one ready work item.
5. Read the linked plan item.
6. Read the linked spec and ADR constraints.
7. Run the proof commands listed for that work item before claiming it is ready.

If the manifest is closed or paused, do not invent the next lane. Report the
state or update the rail only when explicitly asked.

## Archive rule

When replacing the current active goal, archive the old manifest under:

```text
.ripr/goals/archive/YYYY-MM-DD-<lane>.toml
```

Then write the new `.ripr/goals/active.toml`. Do not keep multiple active goal
manifests.
