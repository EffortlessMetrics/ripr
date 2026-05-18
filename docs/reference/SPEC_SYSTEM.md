# Repo source-of-truth system

This repo uses a linked source-of-truth stack so humans and agents can find the
right kind of truth without scraping chat history or treating stale planning
notes as current state.

## Stack

```text
Roadmap
  -> Proposal
    -> Spec
      -> ADR where needed
        -> Implementation plan
          -> Active goal
            -> Issue / PR
              -> Proof
```

## Artifact roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap | release direction, milestones, lane framing | PR queue, live proof receipts |
| Proposal | why, users, alternatives, risks, non-goals | behavior contract, detailed task list |
| Spec | behavior, acceptance, proof, examples | PR order, product rationale |
| ADR | durable architecture or operating decision | implementation queue, current metric state |
| Plan | PR order, work items, proof commands, rollback | product rationale, durable decisions |
| Active goal | current machine-readable lane and work items | generated status, long prose |
| Support tiers | public claim proof and promotion boundary | feature design |
| Policy ledgers | exceptions, owners, coverage, review dates | broad architecture |

## Rules

1. Keep one kind of truth per artifact.
2. Prefer one semantic artifact or one implementation work item per PR unless a
   linked plan explicitly says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable decisions.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public claims require support-tier proof.
8. Policy exceptions require owner, reason, coverage, and review date.

## Source-of-truth map

| Question | Source of truth |
| --- | --- |
| Why are we doing this? | `docs/proposals/` |
| What must be true? | `docs/specs/` |
| What durable decision constrains this? | `docs/adr/` |
| What PR lands next? | `plans/<lane>/implementation-plan.md` |
| What is the agent actively executing? | `.ripr/goals/active.toml` |
| What proves a public claim? | `docs/status/SUPPORT_TIERS.md`, receipts, CI |
| What exceptions exist? | `policy/*.toml` |

## Required headers

Use the repository's existing templates in `docs/templates/` for detailed
formatting. New proposal, spec, ADR, and plan artifacts should declare these
fields when applicable, using `n/a` instead of leaving ambiguous blanks:

- `Status:`
- `Owner:`
- `Created:` or `Date:`
- `Linked proposal:`
- `Linked specs:`
- `Linked ADRs:`
- `Linked plan:`
- `Linked issues:`
- `Linked PRs:`
- `Support-tier impact:`
- `Policy impact:`

Existing artifacts may use older equivalent labels such as `Target campaign`,
`Linked work items`, or `Active goal`; do not churn them only for formatting
unless the selected plan item calls for that migration.

## Agent workflow

Agents must:

1. read `AGENTS.md` and this file;
2. read `.ripr/goals/active.toml`;
3. choose exactly one ready work item when the manifest is active;
4. read the linked plan item;
5. read the linked spec for acceptance;
6. read linked ADRs for constraints;
7. implement only that work item;
8. run the proof commands listed by the plan or manifest;
9. update receipts, status, support tiers, or policy ledgers only when the work
   item requires it;
10. stop on missing or contradictory source-of-truth artifacts.

If the active manifest is closed or paused, agents should not invent the next
campaign. They should report that no active work item is selected unless the
user explicitly asks them to create or revise the rail.

## Stop conditions

Stop and report instead of guessing when:

- the active goal is missing, closed, paused without a selected lane, or stale;
- linked files do not exist;
- a linked spec or plan item is missing;
- requested work conflicts with an ADR;
- proof commands cannot run;
- generated status differs from committed status and no generator is named;
- unrelated staged changes exist;
- a public claim lacks support-tier proof;
- a new policy exception lacks owner, reason, coverage, and review date.

## Active goal lifecycle

### Activate

Use `.ripr/goals/active.toml` for the current machine-readable lane:

```toml
status = "active"
```

### Pause

Use an explicit paused state when no lane is selected:

```toml
status = "paused"
reason = "No selected implementation lane."
```

### Archive

Move the previous active manifest to:

```text
.ripr/goals/archive/YYYY-MM-DD-<lane>.toml
```

Then create the new active manifest. Do not leave multiple active goals.

## Closeout format

At the end of a lane, create or update `plans/<lane>/closeout.md` with:

```text
# Lane closeout: <lane>

Status: completed
Date:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Linked plan:
Active goal archive:

## What shipped

## Proof

## Receipts

## What did not ship

## Deferred work

## Claim boundary

## Next lane recommendation
```

Closeout prevents the next agent from rediscovering old work.

## Common failure modes

### Spec becomes a task list

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec to
behavior, examples, proof, mappings, and claim boundaries.

### Plan becomes product rationale

Move the rationale to `docs/proposals/`; keep the plan to work items,
dependencies, proof commands, rollback, and handoff status.

### Active goal becomes prose

Keep `.ripr/goals/active.toml` machine-readable. Link out to docs instead of
embedding generated tables or long narrative.

### Agent hand-edits generated status

Name the generator or checker in the plan item and run it instead of editing
generated status by hand.

### Support claims drift

Require a support-tier impact statement and a support-tier row or equivalent
proof pointer before broadening public claims.

### Policy exceptions become silent debt

Every exception must have an owner, reason, `covered_by`, and review date or
expiry when temporary.

### Mega PR

Split by semantic artifact or by one implementation work item unless the plan
explicitly accepts a combined evidence package.

## What good looks like

A new contributor or agent can arrive cold and answer:

```text
What are we doing?
Why?
What must be true?
What decision constrains it?
What PR lands next?
What command proves it?
What may we claim?
What must we not claim?
```

If the repo answers those questions without chat history, this system is
working.
