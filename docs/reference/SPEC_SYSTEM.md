# Repo source-of-truth system

`ripr` uses a linked source-of-truth stack so people and agents can find the
right truth without replaying chat history or treating every document as a task
list.

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
| Roadmap | release direction, milestones, lanes | detailed PR queue, generated status, proof receipts |
| Proposal | why, users, alternatives, risks, non-goals | behavior contract, PR sequence, generated status |
| Spec | behavior, acceptance, proof, test mapping | product rationale, active queue, durable architecture choices |
| ADR | durable decision, context, consequences | PR task list, current metric state, implementation queue |
| Plan | PR order, work items, proof commands, rollback | product motivation, durable decisions, generated status truth |
| Active goal | current machine-readable lane and work items | long prose, generated metrics, architecture decisions |
| Support tiers | public claim proof and promotion requirements | feature design, product rationale, task queue |
| Policy ledgers | exceptions, owners, coverage, review dates | broad architecture, behavior contracts |

## Current `ripr` locations

| Question | Source of truth |
| --- | --- |
| Why are we doing this lane? | `docs/proposals/` |
| What must be true? | `docs/specs/` |
| What durable decision constrains the work? | `docs/adr/` |
| What PR-sized work lands next? | `plans/<lane>/implementation-plan.md` and campaign docs |
| What should agents execute now? | `.ripr/goals/active.toml` |
| What proves public claims? | `docs/status/`, reports, receipts, fixtures, goldens, CI |
| What exceptions exist? | `policy/*.toml` and `.ripr/*.toml` ledgers |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the selected plan item explicitly says
   otherwise.
3. Proposals explain why; specs define behavior; ADRs record durable choices;
   plans sequence work; active goals tell agents what to execute now.
4. Runtime/code PRs must preserve the repo's spec -> test or fixture -> code ->
   output contract -> metric chain.
5. Generated status, reports, receipts, and metrics are updated by their owning
   tools, not by hand, unless the plan explicitly says otherwise.
6. Public claims require support-tier rows, generated receipts, CI evidence, or
   equivalent proof pointers.
7. Policy exceptions require an owner, reason, coverage, and review or expiry
   metadata in the appropriate policy ledger.
8. Static findings must keep the conservative `ripr` vocabulary from
   `AGENTS.md`; do not claim mutation-testing outcomes from static evidence.

## Required headers

New source-of-truth documents should keep these fields explicit, using `n/a`
when a field does not apply:

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

Existing repo templates may use older field names. Prefer the templates for the
artifact type, then preserve this source-of-truth intent when updating them.

## Agent workflow

Agents must:

1. read repo instructions (`AGENTS.md` and any scoped instruction files);
2. read this source-of-truth system;
3. read `.ripr/goals/active.toml`;
4. choose exactly one ready work item, or stop if none is ready;
5. read the linked implementation plan;
6. read the linked spec for acceptance and proof;
7. read linked ADRs for constraints;
8. inspect git status before editing;
9. implement only the selected item;
10. run the proof commands listed in the plan item, plus any repo gates needed
    for the touched surface;
11. update only required docs, status, receipt, or policy files;
12. commit and prepare one focused PR.

## Stop conditions

Stop and report instead of guessing when:

- the active goal is missing, paused without a selected item, or stale;
- linked specs, ADRs, or plans are missing;
- no ready work item can be identified;
- the requested work conflicts with an ADR or the product contract;
- proof commands cannot run and no substitute evidence is defined;
- generated status is dirty before the work begins;
- unrelated staged changes exist;
- a public claim lacks proof;
- a policy exception is needed but the relevant ledger is not updated.

## Active goal lifecycle

The active manifest lives at:

```text
.ripr/goals/active.toml
```

Use `status = "active"` for an executable lane and `status = "paused"` when no
lane is selected. Archive closed or superseded manifests under:

```text
.ripr/goals/archive/YYYY-MM-DD-<lane>.toml
```

Do not leave multiple current active-goal files.

## Closeout

At the end of a lane, write or update the lane closeout artifact named by the
plan. A closeout should record what shipped, proof commands, receipts, PRs, CI
runs, support-tier and policy updates, deferred work, claim boundaries, and the
recommended next lane.

## Common failure modes

### Spec becomes a task list

Move PR order to the lane implementation plan and keep the spec focused on
behavior, examples, proof, and claim boundaries.

### Plan becomes product rationale

Move why/background text to the proposal and keep the plan focused on work
items, dependencies, proof, and rollback.

### Active goal becomes prose

Keep the active goal TOML machine-readable and link to long-form documents.

### Agent hand-edits generated status

Run the generator or checker named in the plan; if it cannot run, record the
failure instead of editing generated output by hand.

### Support claims drift

Require support-tier impact in source-of-truth artifacts and keep public claims
backed by status rows, receipts, or CI proof.

### Policy exceptions become silent debt

Every exception must live in a policy ledger with owner, reason, coverage, and a
review or expiry date when appropriate.

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

If the repo answers those questions without chat history, the method is working.
