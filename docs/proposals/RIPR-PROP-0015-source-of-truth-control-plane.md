# RIPR-PROP-0015: Source-of-truth Control Plane

Status: accepted

Owner: repo-infra / source-of-truth

Created: 2026-05-21

Target campaign: Source-of-truth control plane

Linked proposal: n/a

Linked specs:

- [`RIPR-SPEC-0060`: Source-of-truth stack](../specs/RIPR-SPEC-0060-source-of-truth-stack.md)

Linked ADRs:

- None yet. Add an ADR only if the lane changes durable architecture,
  repository authority, package boundaries, or CI enforcement policy.

Linked work items:

- Closed plan:
  [Source-of-truth control plane implementation plan](../../plans/source-of-truth/implementation-plan.md)
- Current-state reconciliation slice: `docs/source-of-truth-current-state`.
- Closeout slice: `docs/source-of-truth-closeout`.
- Completed stack surfaces include doctrine docs, templates, stack spec,
  document artifact ledger, document artifact validator, support-tier claim map,
  support-tier validator, active-goal validation, source-of-truth PR and issue
  templates, advisory Source of Truth CI, graph report, PR-body generator, and
  closeout generator.
- Closeout:
  [Source-of-truth control plane closeout](../handoffs/2026-05-23-source-of-truth-control-plane-closeout.md)

Support-tier impact:

- No tier promotion in the closeout slice.
- The existing `Source-of-truth artifact graph` support-tier row owns the
  product claim to proof-command mapping.

Policy impact:

- `policy/doc-artifacts.toml` records this proposal, the linked spec, the
  implementation plan, and the closeout.
- No CI lane, lint, file, package, no-panic, release, or branch-protection
  policy changed in the closeout slice.

Required evidence:

- `cargo xtask check-doc-artifacts`
- `cargo xtask check-goals`
- `cargo xtask check-support-tiers`
- `cargo xtask repo-contract-report`
- `cargo xtask check-doc-index`
- `cargo xtask markdown-links`
- `cargo xtask check-static-language`
- `cargo xtask check-pr`
- `git diff --check`

Non-goals:

- No blocking CI changes.
- No support-tier promotion.
- No policy-ledger mutation beyond document artifact status and closeout
  registration.
- No package split or public schema change.

Claim boundary:

- This accepted proposal records why the source-of-truth control plane exists
  and which repo-local proof stack landed. The validators prove registered
  artifact integrity, active-goal shape, and support-tier proof-command
  references; they do not prove analyzer correctness, runtime adequacy,
  coverage adequacy, mutation outcomes, release readiness, or blocking CI
  promotion.

Rollback:

- Revert the proposal and index links. No runtime behavior changes.

## Problem

`ripr` already uses proposals, specs, ADRs, plans, active goal manifests,
support tiers, policy ledgers, and closeouts, but those layers are still easier
to apply correctly when the operator already knows the repository history.

That creates avoidable failure modes:

- agents rediscover context from chat instead of repo artifacts;
- PRs cite intent without a linked behavior contract;
- plans carry rationale that belongs in proposals;
- specs absorb work queues or support-tier truth;
- README or release claims can drift away from proof commands;
- CI lanes can cost time without a visible proof obligation;
- policy exceptions can be hard to trace to owners, reasons, and review
  posture;
- closeouts can describe what happened without making the next ready step
  mechanical.

The repo needs a source-of-truth control plane so humans, Codex sessions,
reviewers, and CI can answer from the checkout:

```text
Why does this work exist?
What exact behavior must be true?
What architecture decision governs it?
What PR-sized step comes next?
What proof command validates it?
What product claim may we make after it lands?
What policy ledger changed?
What did the last agent do?
What should the next agent do?
```

## Users and surfaces

Users:

- maintainers selecting the next safe PR-sized slice;
- contributors trying to understand what counts as done;
- reviewers checking whether a PR satisfies its linked contract;
- Codex and other coding agents resuming work from repo artifacts;
- CI maintainers controlling proof cost and enforcement timing;
- release operators checking whether product claims are supported.

Surfaces:

- [source-of-truth docs](../source-of-truth/README.md);
- [`docs/proposals/`](./);
- [`docs/specs/`](../specs/);
- [`docs/adr/`](../adr/);
- [`plans/`](../../plans/);
- [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml);
- [`docs/status/SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md);
- [`policy/`](../../policy/);
- [`.github/`](../../.github/);
- [`docs/handoffs/`](../handoffs/);
- `cargo xtask` repo checks and generated reports under `target/ripr/`.

## Success criteria

- The repo has a documented artifact taxonomy: proposal equals why, spec equals
  what, ADR equals durable decision, plan equals PR sequence, active goal equals
  what is executing now, support tiers equal product claim to proof command,
  policy ledgers equal governed exceptions, and closeouts equal what happened.
- A source-of-truth stack spec defines the machine-checkable behavior contract.
- Machine-readable ledgers can register proposals, specs, ADRs, plans, support
  tiers, active goals, policy exceptions, and closeouts without duplicating
  truth across documents.
- Validators can detect missing files, duplicate IDs, unresolved links, invalid
  statuses, unsupported active-goal fields, missing proof commands, and
  unsupported stable claims.
- CI can run the checks advisory first, then promote only the mature core checks
  after burn-in.
- A repo-local report can show active goal, ready work items, accepted
  proposals, accepted specs, ADRs, support-tier impacts, policy impacts,
  missing links, superseded artifacts, and recently completed work.
- A PR body generator can read the active goal and linked plan/spec/proposal
  chain to produce a review-by-contract PR body.
- A closeout generator can archive completed work with proof, claim changes,
  policy changes, remaining work, and the next recommended goal.
- The method remains portable enough for future tokmd productization without
  forcing `ripr` to wait for tokmd.

## Proposed shape

Build the control plane one semantic layer at a time:

1. Define doctrine in `docs/source-of-truth/`.
2. Add concrete templates for each artifact type.
3. Add this proposal to explain why the repo is adopting the control plane.
4. Add `RIPR-SPEC-0060` to define the source-of-truth stack contract.
5. Add or refresh a document artifact ledger under `policy/`.
6. Add `cargo xtask check-doc-artifacts`.
7. Add support-tier claim mapping and `cargo xtask check-support-tiers`.
8. Add active-goal manifest guidance and `cargo xtask check-goals` coverage for
   proof-stack fields.
9. Add PR and issue templates that require proposal/spec/ADR/plan/proof/claim
   boundary/rollback fields.
10. Add advisory CI for source-of-truth checks, then promote only mature checks.
11. Add repo-contract reports, PR body generation, and closeout generation.

Keep the layers distinct:

```text
proposal -> spec -> ADR when needed -> plan -> active goal -> PR -> proof
  -> support-tier or policy update when applicable -> closeout
```

Support-tier truth stays in `docs/status/SUPPORT_TIERS.md`. CI lane truth stays
in CI policy ledgers and workflows. Policy exceptions stay in `policy/`.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep the model as prose only. | Prose helps humans but does not catch missing links, stale active goals, unsupported claims, or policy drift. |
| Put all truth in one master document. | One document cannot safely own why, what, decisions, sequencing, execution state, claims, policy, proof, and closeout without becoming stale or contradictory. |
| Make CI blocking immediately. | New validators need advisory burn-in before they are trusted enough to block unrelated work. |
| Treat README claims as the claim source. | README and release copy are consumer surfaces; support tiers should own claim to proof mapping. |
| Let agents infer the next task from chat. | Chat history is not durable repo state and does not give reviewers a stable contract. |
| Build this only in tokmd first. | `ripr` needs repo-native operating truth now; tokmd productization can follow once the pattern is proven. |

## Specs to create

- `RIPR-SPEC-0060`: Source-of-truth stack contract.

Future specs may split support-tier validation, active-goal validation, graph
reporting, PR body generation, or closeout generation if those contracts grow
beyond the first stack spec.

## Evidence plan

- Docs-first slices define the model and templates before validators.
- The stack spec names artifact fields, valid statuses, link requirements, and
  proof obligations.
- `policy/doc-artifacts.toml` records concrete proposal/spec/ADR/plan artifacts
  as machine-readable rows.
- `cargo xtask check-doc-artifacts` proves ledger parsing, ID uniqueness, file
  existence, kind/path consistency, status vocabulary, link resolution,
  supersession references, and accepted-spec proposal or standalone reasoning.
- `docs/status/SUPPORT_TIERS.md` maps product claims to proof commands.
- `cargo xtask check-support-tiers` proves stable and stabilizing claims carry
  proof commands and do not drift from README or release claims once scanning is
  mature.
- `cargo xtask check-goals` proves active goals and work items reference real
  artifacts, carry proof commands where required, and reject unsupported policy
  fields.
- Advisory CI runs source-of-truth checks before any branch-protection
  promotion.
- Repo-contract reports, PR-body generation, and closeout generation prove the
  graph is useful to humans and agents, not merely valid.

## Risks

- The control plane could become a documentation cleanup instead of an
  operating system. Mitigation: every follow-up slice should add either one
  artifact layer or one validator/report that answers a concrete repo question.
- Validators could become too broad and block unrelated work. Mitigation:
  introduce them advisory first and promote only mature checks.
- Support-tier truth could be duplicated inside specs. Mitigation: specs link
  to support tiers; support tiers own claim/proof mapping.
- CI lane truth could be duplicated inside specs. Mitigation: CI policy ledgers
  and workflow files own lane cost, trigger, and blocking posture.
- Agents could invent fields or merge rules. Mitigation: validators reject
  unsupported active-goal fields and docs require command/path verification.
- The graph could become expensive to maintain. Mitigation: start with a small
  ledger and cheap checks, then generate reports and PR bodies from the same
  data.
- Tokmd productization could overfit to `ripr`. Mitigation: keep repo-local
  paths explicit and isolate reusable proof-stack concepts in later product
  slices.

## Non-goals

- No full mutation engine, coverage dashboard, proof system, second
  rust-analyzer, or generic test generator.
- No package split.
- No public output schema change in this proposal slice.
- No runtime analyzer behavior change.
- No editor/LSP behavior change.
- No new dependency.
- No branch-protection change before advisory burn-in.
- No immediate support-tier promotion.
- No policy exception without a ledger row.
- No invented active-goal fields or merge rules.
- No reliance on chat history as the source of truth.

## Exit criteria

This proposal moved to `accepted` on 2026-05-23 because:

- `RIPR-SPEC-0060` is added and indexed;
- document artifacts are registered in a parseable policy ledger;
- `cargo xtask check-doc-artifacts` validates IDs, paths, statuses, and links;
- active goal validation rejects broken artifact references and unsupported
  fields;
- support-tier validation verifies stable or stabilizing claim proof commands;
- PR and issue templates include proposal/spec/ADR/plan/proof/claim
  boundary/rollback fields;
- source-of-truth CI runs advisory before any blocking promotion;
- repo-contract reports show active goal, ready items, accepted artifacts,
  support-tier impacts, policy impacts, missing links, superseded artifacts,
  and recently completed work;
- PR body and closeout generation can consume the same graph;
- a closeout records what landed, what proof ran, what claims or policies
  changed, what remains, and the next recommended goal;
- the claim boundary remains honest: validators prove repo artifact integrity
  and linked proof obligations, not product correctness beyond the named proof.

The closeout is recorded in
[`docs/handoffs/2026-05-23-source-of-truth-control-plane-closeout.md`](../handoffs/2026-05-23-source-of-truth-control-plane-closeout.md).
