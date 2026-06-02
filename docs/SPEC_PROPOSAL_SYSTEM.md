# The spec/proposal system, fully explained

This guide explains the canonical [Repo Tracking Model](REPO_TRACKING_MODEL.md).
It does not replace that model; use it as the longer operator explanation for
the same layers and boundaries.

The system is a **repo source-of-truth stack**. Its central rule is:

> **Do not make every document do every job.**

Each artifact owns one kind of truth: **why**, **what**, **what decision**, **how**, **what now**, **what proves it**, and **what changed**.

The end result is a repo where a human, Codex, Droid, Claude, or CI can answer:

```text
Why are we doing this?
What exact behavior must be true?
What architecture decision did we make?
What PR-sized work comes next?
What is the active lane right now?
What proves the claim?
Which support tier changed?
Which policy ledgers changed?
What happened after merge?
```

That is the whole system.

---

## 1. The stack at a glance

The system is organized as a chain:

```text
Roadmap
  -> Proposal / PRD
    -> Specs
      -> ADRs where needed
        -> Implementation plan
          -> Active goal manifest
            -> Issues / PRs
              -> Proof commands
              -> CI lanes
              -> support-tier updates
              -> policy receipts
                -> Closeout / handoff
```

Each layer narrows the previous one.

A **roadmap** says direction. A **proposal** says why. A **spec** says the
behavior contract. An **ADR** says the architecture decision. An
**implementation plan** says PR sequence. An **active goal manifest** says what
is executing now. A **support-tier map** says what users may believe. A
**policy ledger** says exceptions and obligations. A **closeout** says what
actually happened.

---

## 2. Why the system exists

The point is **repo-operational memory**.

Without this system, workers rely on stale chat context, old PR descriptions,
ambiguous README claims, unverified assumptions, hidden CI costs, broad todos,
and hallucinated commands or policies.

With the system, the repo itself provides the execution graph:

```text
.ripr/goals/active.toml
  -> linked implementation plan
    -> linked spec
      -> linked proposal
        -> linked support-tier and policy proof
```

---

## 3. Artifact types

### 3.1 Roadmap

**Owns:** release direction, milestone themes, high-level sequencing.

**Does not own:** acceptance tests, PR order, detailed implementation tasks.

Typical location:

```text
ROADMAP.md
docs/roadmap.md
```

### 3.2 Proposal / PRD

**Owns:** why the work exists (problem, value, alternatives, risks, success).

Typical location:

```text
docs/proposals/
```

Proposal template:

Use [docs/templates/PROPOSAL_TEMPLATE.md](templates/PROPOSAL_TEMPLATE.md).
Do not fork proposal headers or section names in this guide; update the
template when the canonical shape changes.

### 3.3 Spec

**Owns:** what behavior must be true.

Typical location:

```text
docs/specs/
```

Spec template:

Use [docs/templates/SPEC_TEMPLATE.md](templates/SPEC_TEMPLATE.md). The
template is the canonical section inventory checked by repo automation.

### 3.4 ADR

**Owns:** durable architecture decisions.

Typical location:

```text
docs/adr/
```

ADR template:

Use [docs/templates/ADR_TEMPLATE.md](templates/ADR_TEMPLATE.md).

### 3.5 Implementation plan

**Owns:** PR-sized sequencing.

Typical location:

```text
plans/<milestone>/
```

Plan item template:

Use [docs/templates/PLAN_ITEM_TEMPLATE.md](templates/PLAN_ITEM_TEMPLATE.md).

### 3.6 Active goal manifest

**Owns:** what Codex/agent/operator is actively executing now.

Typical location:

```text
.ripr/goals/active.toml
.ripr/goals/archive/
```

### 3.7 Support tiers

**Owns:** product claim -> proof command mapping.

Typical location:

```text
docs/status/SUPPORT_TIERS.md
```

### 3.8 Policy ledgers

**Own:** governed exceptions and obligations.

Typical location:

```text
policy/*.toml
policy/*.txt
```

### 3.9 Closeout / handoff

**Owns:** what actually happened.

Typical locations:

```text
docs/handoffs/
plans/<milestone>/closeout.md
docs/releases/
docs/release/
```

---

## 4. Directory layout

```text
docs/
  proposals/
  specs/
  adr/
  status/
  handoffs/
plans/
.ripr/goals/
policy/
```

Use stable, repo-specific IDs like `RIPR-SPEC-0001`.

---

## 5. How documents link

- roadmap -> proposals
- proposals -> specs + ADRs + plan
- specs -> proposal + proof commands
- ADRs -> dependent specs
- plan -> proposal/spec/ADR IDs
- active goal -> plan work items
- PRs -> plan/spec/proposal
- closeouts -> landed evidence

Recommended shared headers:

```md
Status:
Owner:
Created:
Milestone:
Linked proposal:
Linked specs:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

---

## 6. Status lifecycle

- Proposals/specs/ADRs: `draft`, `proposed`, `accepted`, `implemented`,
  `superseded`, `rejected`
- Plan items: `ready`, `active`, `blocked`, `done`, `superseded`
- Active goals: `active`, `paused`, `complete`, `archived`

---

## 7. What not to duplicate

Single source-of-truth examples:

- Product claim stability -> `docs/status/SUPPORT_TIERS.md`
- CI lane policy -> `policy/ci-lane-whitelist.toml`
- Workspace/package shape -> `policy/workspace_shape.txt`
- File exceptions -> `policy/non-rust-allowlist.toml`
- Active work -> `.ripr/goals/active.toml`
- PR order -> `plans/<milestone>/implementation-plan.md`
- Why -> `docs/proposals/`
- Behavior -> `docs/specs/`
- Durable decisions -> `docs/adr/`

---

## 8. How Codex should use the system

1. Read `.ripr/goals/active.toml`.
2. Pick the next ready `work_item`.
3. Read linked plan item.
4. Read linked spec.
5. Read linked proposal for context.
6. Read linked ADRs if architecture is involved.
7. Make one PR-sized change.
8. Update support tiers/policy ledgers only when claims/policy change.
9. Run listed proof commands.
10. Update goal manifest.
11. Open/review/improve/merge per repo policy.
12. Add closeout notes when lane completes.

---

## 9. How CI enforces the system

Recommended checks:

```text
cargo xtask check-doc-index
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-goals
cargo xtask check-doc-roles
cargo xtask check-ci-lane-whitelist
```

---

## 10. How PRs should look

PR bodies should include summary, links (proposal/spec/ADR/plan/issue), scope,
non-goals, support-tier impact, policy impact, proof commands, claim boundary,
and rollback.

---

## 11. Core operating principles

1. One artifact, one kind of truth.
2. Specs are contracts, not queues.
3. Plans are PR-sized.
4. Claims are proof-mapped.
5. Exceptions are ledgered (owner/reason/scope/proof/review).
6. Agent state is machine-readable.
7. Do not encode fake repo rules.
8. Verify specifics before relying on them.

---

## 12. Minimal rollout order

1. Define docs model.
2. Add doc artifact ledger.
3. Add doc artifact validation.
4. Add active goal manifest.
5. Add goal validation.
6. Add first proposal.
7. Add first spec.
8. Add support tiers.
9. Add package/CI/policy ledgers.
10. Wire CI (advisory then selective blocking).

---

## 13. Simplest mental model

```text
Proposal = why.
Spec = what.
ADR = durable decision.
Plan = how.
Active goal = what Codex is doing now.
Support tiers = what users may believe.
Policy ledgers = what exceptions and proof obligations exist.
CI = what proved it.
Closeout = what happened.
```

The system works when these layers are linked, validated, and non-duplicative.
