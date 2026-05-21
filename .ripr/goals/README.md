# Goal manifests

Goal manifests are the machine-readable execution layer for `ripr` agents. They
tell a cold-start agent what lane is active, which work items exist, what proof
commands are required, and what claim boundaries apply.

The active manifest is:

```text
.ripr/goals/active.toml
```

Archived or focused lane manifests live beside it or under:

```text
.ripr/goals/archive/
```

## Ownership

Goal manifests own:

- current lane identity and status;
- linked proposal, spec, ADR, and plan paths;
- machine-readable objectives and end-state checks;
- work-item IDs, statuses, dependencies, and proof commands;
- status pointers and claim boundaries.

Goal manifests do not own:

- product rationale;
- behavior contracts;
- durable decisions;
- generated metrics or reports;
- support-tier claims;
- policy exceptions.

Move those to the linked proposal, spec, ADR, generated report, support-tier
row, or policy ledger.

## Agent boot order

Agents should read:

1. `AGENTS.md`.
2. `docs/REPO_TRACKING_MODEL.md`.
3. `docs/agent-context/CONTEXT_SYSTEM.md`.
4. `.ripr/goals/active.toml`.
5. The linked implementation plan.
6. The linked spec for the selected work item.
7. Linked ADRs.

Then the agent should pick exactly one ready work item, run the listed proof
commands, and stop if linked artifacts are missing or contradictory.

## Status values

Use these work-item statuses:

- `ready`: available to start.
- `active`: currently being worked.
- `blocked`: blocked by a named dependency or missing artifact.
- `done`: landed and evidence is recorded.
- `completed`: historical synonym used by some older docs; prefer `done` for
  new entries.
- `superseded`: replaced by another work item or lane.

The top-level active manifest uses `active` while work is current and `closed`
only when the campaign has landed and either `successor = "<campaign-id>"`
points at the next selected campaign or `no_current_goal = true` intentionally
marks the repo idle. A closed manifest should also have an archive copy when it
represents completed lane history. The freshness marker prevents agents from
silently continuing from stale execution state.

## Validation

For manifest-only changes, run at minimum:

```bash
git diff --check
cargo xtask goals next
cargo xtask check-campaign
cargo xtask check-doc-index
cargo xtask check-pr-shape
```

Also run any commands listed by the changed work item.
