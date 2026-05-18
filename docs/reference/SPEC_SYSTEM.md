# Repo source-of-truth system

This repo uses a linked source-of-truth stack so humans and agents can find the
right kind of truth without relying on chat history.

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

The rule is: do not make every document do every job. Separate why, what,
decision, how, what now, and what proves it.

## Artifact roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap | release direction, milestone framing, product strategy | detailed PR queue, live proof receipts |
| Proposal | why, users, affected surfaces, alternatives, risks | behavior contract, PR sequence, generated status |
| Spec | required behavior, acceptance examples, proof requirements | product rationale, PR order, durable architecture decision |
| ADR | durable architecture or operating decision | task list, current metric state, implementation queue |
| Plan | PR sequence, dependencies, proof commands, rollback | product motivation, durable architecture, generated status truth |
| Active goal | current machine-readable lane, objective, work items, claim boundaries | long prose, generated metrics, durable decisions |
| Support tiers | public support claims and proof pointers | feature design, implementation sequencing |
| Policy ledgers | exceptions, CI and policy receipts, review dates | broad architecture or product strategy |

## Source-of-truth map

| Question | Source of truth |
| --- | --- |
| Why are we doing this? | `docs/proposals/` and `docs/ROADMAP.md` |
| What must be true? | `docs/specs/` |
| What architecture decision constrains it? | `docs/adr/` |
| What PR lands next? | `plans/<lane>/implementation-plan.md` and `docs/IMPLEMENTATION_PLAN.md` |
| What is the agent actively executing? | `.ripr/goals/active.toml` |
| What proves the claim? | `docs/status/`, receipts under `target/ripr/`, fixtures, goldens, and CI |
| What exceptions exist? | `policy/` and `.ripr/*allow*` ledgers |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the selected plan item says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable decisions.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public claims require support-tier, status, receipt, fixture, golden, or CI
   proof.
8. Policy exceptions require owner, reason, coverage, and review date when the
   relevant ledger supports them.

## Required metadata

New proposal, spec, ADR, and plan artifacts should declare the metadata needed
for a cold reader to follow the stack. Use `n/a` when a field does not apply.

Common fields:

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

Repo-specific templates live in `docs/templates/` and may use older field names
such as `Target campaign` or `Linked work items`; keep those fields only when
they point to the same source-of-truth role.

## Agent workflow

Agents must:

1. Read `AGENTS.md` or `CLAUDE.md`.
2. Read this document.
3. Read `.ripr/goals/active.toml`.
4. Read the linked implementation plan for the selected work item.
5. Read the linked proposal only for why.
6. Read the linked spec for acceptance and proof.
7. Read linked ADRs for durable constraints.
8. Inspect the current git state.
9. Pick exactly one ready work item.
10. Implement only that item.
11. Run the proof commands.
12. Update plan, status, receipts, or policy ledgers only when the work item
    requires it.
13. Open or update one focused PR.

If an agent cannot identify a ready work item, it should write a handoff or ask
for lane selection instead of inventing work.

## Stop conditions

Stop and report instead of guessing when:

- the active goal is missing, paused without a selected lane, or stale;
- linked proposal, spec, ADR, or plan files do not exist;
- the selected work item is missing proof commands;
- proof commands cannot run and no substitute evidence is named;
- generated status differs from committed status and no generator/checker is
  named;
- unrelated staged files exist;
- requested work conflicts with an ADR;
- a public support claim lacks a proof pointer.

## Active goal lifecycle

The active goal lives at `.ripr/goals/active.toml`.

Use `status = "active"` for the current lane. Use `status = "paused"` only when
there is no selected implementation lane and include a reason. Archive old goals
under `.ripr/goals/archive/YYYY-MM-DD-<lane>.toml` before replacing them.

Do not leave multiple active manifests.

## Closeout format

At the end of a lane, write a closeout under `plans/<lane>/closeout.md` or
`docs/handoffs/YYYY-MM-DD-<lane>-closeout.md` with:

- what shipped;
- proof commands and receipts;
- PRs and CI runs;
- generated status, support-tier, and policy updates;
- what did not ship;
- deferred work;
- claim boundary;
- next lane recommendation.

## Common failure modes

### Spec becomes a task list

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec to
behavior, examples, and proof.

### Plan becomes product rationale

Move why to the proposal; keep the plan to work items, dependencies, proof, and
rollback.

### Active goal becomes prose

Keep TOML machine-readable. Link to docs for long prose and generated tables.

### Generated status is hand-edited

Run the named generator/checker and record the command in the PR proof.

### Support claims drift

Require support/status impact metadata and a proof pointer before broadening a
README or public claim.

### Policy exceptions become silent debt

Every exception must have the fields required by its ledger, including owner,
reason, and coverage.

### Mega PR

Keep to one semantic artifact or one implementation work item unless the plan
explicitly bundles the artifacts.

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
