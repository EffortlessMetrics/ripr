# Active goals

The active goal manifest is the machine-readable "what now" source of truth for
agents. It selects the current lane, links the controlling proposal/spec/ADR/plan
artifacts, names work items, and records claim boundaries plus proof commands.

Read `.ripr/goals/active.toml` after `docs/reference/SPEC_SYSTEM.md` and before
choosing work.

## Role in the source-of-truth stack

```text
Roadmap -> Proposal -> Spec -> ADR -> Plan -> Active goal -> PR -> Proof
```

Active goals own:

- current lane identity and status;
- objective and end state;
- linked proposal, specs, ADRs, and plan;
- current work items and their statuses;
- per-item proof commands;
- claim boundaries and status pointers.

Active goals do not own:

- long product rationale;
- behavior contracts;
- architecture decisions;
- generated metric tables;
- support-tier truth.

## Lifecycle

Use exactly one active manifest at `.ripr/goals/active.toml`.

- `status = "active"` means a lane is selected and agents may pick a ready work
  item.
- `status = "paused"` means no lane is selected; include a reason and do not
  invent work.
- `status = "closed"` means the manifest is historical and should be archived
  before the next active lane is selected.

Archive replaced goals under `.ripr/goals/archive/YYYY-MM-DD-<lane>.toml`.

## Work item fields

A work item should identify its plan anchor and proof commands:

```toml
[[work_item]]
id = "work-item-id"
status = "ready"
spec = "docs/specs/RIPR-SPEC-NNNN-contract.md"
adr = "docs/adr/NNNN-decision.md"
plan = "plans/lane/implementation-plan.md#work-item-work-item-id"
claim_boundary = "What this work item may and may not claim."
commands = [
  "cargo test --workspace",
  "git diff --check",
]
```

If the active goal is missing, stale, closed, or lacks a ready work item, stop
and report instead of creating a new lane implicitly.
