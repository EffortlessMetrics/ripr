# The spec/proposal system, fully explained

The system is a **repo source-of-truth stack**. Its central rule is:

> **Do not make every document do every job.**

This guide explains the canonical [Repo Tracking Model](REPO_TRACKING_MODEL.md).
It does not replace that model; use it as the longer operator explanation for
the same layers and boundaries.

Each artifact owns one kind of truth: **why**, **what**, **what decision**,
**how**, **what now**, **what proves it**, and **what changed**.

The end result is a repo where a human, Codex, Droid, Claude, or CI can
answer:

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

## 1. The stack at a glance

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

- A **roadmap** says direction.
- A **proposal** says why an initiative should exist.
- A **spec** says the behavior contract.
- An **ADR** says the durable architecture decision.
- An **implementation plan** says the PR sequence.
- An **active goal manifest** says what Codex is executing now.
- A **support-tier map** says what users may believe.
- A **policy ledger** says the exceptions and obligations.
- A **closeout** says what actually happened.

## 2. Why the system exists

The point is **repo-operational memory**.

Without this system, workers rely on stale chat context, old PR descriptions,
ambiguous README claims, and unverifiable assumptions.

With the system, the repo itself provides the execution graph:

```text
.ripr/goals/active.toml
  -> linked implementation plan
    -> linked spec
      -> linked proposal
        -> linked support-tier and policy proof
```

## 3. Artifact types and ownership

### 3.1 Roadmap

Owns release direction, milestone themes, and high-level sequencing.

### 3.2 Proposal / PRD

Owns why the work exists: problem, value, alternatives, risks, and success
criteria.

### 3.3 Spec

Owns the behavior contract: what must be true, evidence required, and what must
not be claimed.

### 3.4 ADR

Owns durable architecture decisions that future work must respect.

### 3.5 Implementation plan

Owns PR-sized sequencing and proof commands.

### 3.6 Active goal manifest

Owns current machine-readable execution state.

### 3.7 Support tiers

Owns product-claim stability and claim-to-proof-command mapping.

### 3.8 Policy ledgers

Own governed exceptions and obligations (package boundary, CI lanes, lints,
no-panic, file-policy, etc.).

### 3.9 Closeout / handoff

Owns what actually landed, what proof passed, claim changes, and remaining
work.

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

Use stable, repo-scoped IDs like `RIPR-SPEC-0001`.

## 5. Linking model

- roadmap -> proposal
- proposal -> spec + ADR + plan
- spec -> proposal + proof
- ADR -> dependent specs
- plan -> proposal/spec/ADR IDs
- active goal -> plan work items
- PR -> plan/spec/proposal
- closeout -> landed changes and proof

## 6. Status vocabulary

- Proposals/specs/ADRs: `draft`, `proposed`, `accepted`, `implemented`,
  `superseded`, `rejected`
- Plan items: `ready`, `active`, `blocked`, `done`, `superseded`
- Goals: `active`, `paused`, `complete`, `archived`

## 7. Non-duplication rule

Keep one source of truth per fact.

- Support tiers -> `docs/status/SUPPORT_TIERS.md`
- CI lane rules -> `policy/ci-lane-whitelist.toml`
- Non-Rust file exceptions -> `policy/non-rust-allowlist.toml`
- Active Codex work -> `.ripr/goals/active.toml`
- PR order -> `plans/<milestone>/implementation-plan.md`
- Why -> `docs/proposals/`
- Behavior contract -> `docs/specs/`
- Durable decisions -> `docs/adr/`

## 8. Codex operating flow

1. Read `.ripr/goals/active.toml`.
2. Pick next ready `work_item`.
3. Read linked plan item.
4. Read linked spec.
5. Read linked proposal for context.
6. Read linked ADRs if architecture is involved.
7. Make one PR-sized change.
8. Update support tiers or policy ledgers only if claims/policy change.
9. Run listed proof commands.
10. Update goal manifest.
11. Open/review/improve/merge per repo policy.
12. Write closeout notes when the lane completes.

## 9. CI validation concept

Recommended checks include:

- `cargo xtask check-goals`
- `cargo xtask check-doc-index`
- `cargo xtask check-doc-roles`
- `cargo xtask check-traceability`
- `cargo xtask check-capabilities`
- `cargo xtask check-ci-lane-whitelist`
- `cargo xtask check-process-policy`

## 10. PR body shape

Each PR should include links, scope, non-goals, support-tier impact, policy
impact, proof commands, claim boundary, and rollback.

## 11. Operating principles

1. One artifact, one kind of truth.
2. Specs are contracts, not queues.
3. Plans are PR-sized and executable.
4. Claims must be proof-mapped.
5. Exceptions are ledgered (owner/reason/scope/proof/review).
6. Agent state is machine-readable.
7. Do not encode fake repo rules.
8. Verify named commands, crates, workflows, and APIs before relying on them.

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
10. Wire CI (advisory first, then promote selected checks to blocking).

## 13. Short mental model

```text
Proposal = why.
Spec = what.
ADR = durable decision.
Plan = how.
Active goal = what Codex is doing now.
Support tiers = what users may believe.
Policy ledgers = exceptions + proof obligations.
CI = what proved it.
Closeout = what happened.
```

The system works when layers are **linked, validated, and non-duplicative**.
