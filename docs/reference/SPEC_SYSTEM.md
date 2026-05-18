# Repo source-of-truth system

`ripr` uses a linked source-of-truth stack so humans and agents can find the
right kind of truth without scraping chat history or overloading one document
with every job.

## Stack

```text
Roadmap
  -> Proposal
    -> Spec
      -> ADR where needed
        -> Implementation plan
          -> Active goal
            -> PR
              -> Proof
```

## Artifact roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap | Release direction, milestone framing, and high-level lanes. | Detailed PR order, live status, generated metrics, or proof receipts. |
| Proposal | Why a lane exists, user pain, affected surfaces, alternatives, risks, and non-goals. | Behavior contracts, output schemas, exact PR queue, or generated status. |
| Spec | Required behavior, acceptance examples, proof requirements, test mapping, implementation mapping, and support impact. | Product rationale, PR sequencing, active queue state, or durable architecture decisions. |
| ADR | Durable architecture or operating decisions, context, consequences, and rejected alternatives. | Work queues, current metric state, or implementation task lists. |
| Plan | PR order, work items, dependencies, proof commands, rollback notes, and status handoff. | Product motivation, durable architecture choices, or generated status truth. |
| Active goal | Current machine-readable lane, work items, proof commands, status pointers, and claim boundaries. | Long prose, generated metrics, or durable decisions. |
| Support tiers | Public claims, tier labels, proof pointers, known limitations, and next promotion requirements. | Feature design or implementation plans. |
| Policy ledgers | Exceptions, CI and policy intent, owners, reasons, coverage, and review dates. | Broad architecture or product rationale. |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the linked plan says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable decisions.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public claims require a support-tier row or an equivalent proof pointer.
8. Policy exceptions require an owner, reason, coverage, and review date.

## Required source-of-truth fields

New proposal, spec, ADR, and plan artifacts should declare these fields when
the artifact type supports them. Use `n/a` when a field is intentionally not
applicable.

```text
Status:
Owner:
Created:
Linked proposal:
Linked specs:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

Existing historical artifacts may use older headers. When editing an existing
artifact for material source-of-truth work, prefer moving it toward this field
set instead of adding a second competing header block.

## Agent workflow

Agents must:

1. Read `AGENTS.md` and this file.
2. Read `.ripr/goals/active.toml`.
3. Read the linked implementation plan.
4. Read the linked spec for the selected work item.
5. Read linked ADRs for constraints.
6. Pick exactly one ready work item.
7. Implement only that work item.
8. Run the proof commands listed by the plan or active goal.
9. Update receipts, status, support tiers, or policy ledgers only when the
   selected work item requires it.
10. Stop on missing or contradictory source-of-truth artifacts.

## Stop conditions

Stop and report instead of guessing when:

- the active goal is missing or stale;
- linked files do not exist;
- the selected work item is missing, blocked, or already complete;
- requested work conflicts with an ADR;
- proof commands cannot run;
- generated status is dirty and the plan does not say to regenerate it;
- unrelated staged changes exist;
- a public claim lacks support-tier proof;
- a policy exception lacks owner, reason, coverage, or review date.

## Active goal lifecycle

The active manifest lives at:

```text
.ripr/goals/active.toml
```

Use top-level `status = "active"` for the current execution lane. Use
`status = "paused"` with a reason when no lane is selected. A closed manifest
may remain in place only as a temporary pointer until a successor lane is
selected; closed copies should also be archived under:

```text
.ripr/goals/archive/YYYY-MM-DD-<lane>.toml
```

Do not leave multiple active goals.

## Closeout format

At the end of a lane, write a closeout under the lane plan directory or
`docs/handoffs/`, depending on the plan. A closeout records:

- what shipped;
- proof commands and receipts;
- PRs and CI runs;
- generated status, support-tier, and policy updates;
- what did not ship;
- deferred work;
- claim boundary;
- next lane recommendation.

Closeout records what happened. It does not create new behavior contracts.

## Common failure modes

### Spec becomes a task list

Move PR order to the relevant implementation plan. Keep the spec focused on
behavior, examples, and proof.

### Plan becomes product rationale

Move why-language to the linked proposal. Keep the plan focused on work items,
proof commands, rollback, and handoff state.

### Active goal becomes prose

Keep `active.toml` machine-readable. Put long explanations in proposals, specs,
ADRs, plans, or handoffs and link to them.

### Generated status is hand-edited

Run the generator or checker named by the plan. If no generator exists, record
that as a work item instead of silently editing generated status.

### Support claims drift

Add or update a support-tier row before broadening README, workflow, release,
or public API claims.

### Policy exceptions become silent debt

Every exception needs an owner, reason, coverage, creation date, and review or
expiry date in the relevant `policy/*.toml` ledger.

### Mega PR

Split by semantic artifact or by one implementation work item unless the plan
explicitly requires a combined evidence package.

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

If the repo answers those questions without chat history, the source-of-truth
system is working.
