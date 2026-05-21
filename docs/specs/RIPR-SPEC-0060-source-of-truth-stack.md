# RIPR-SPEC-0060: Source-of-truth Stack

Status: proposed

Owner: repo-infra / source-of-truth

Created: 2026-05-21

Linked proposal:

- [RIPR-PROP-0015: Source-of-truth Control Plane](../proposals/RIPR-PROP-0015-source-of-truth-control-plane.md)

Linked ADRs:

- None.

Linked plan:

- Planned: `plans/source-of-truth/implementation-plan.md`

Support-tier impact:

- Future support-tier rows should map source-of-truth product claims to proof
  commands. This spec does not promote a claim by itself.

Policy impact:

- Future policy slices should register document artifacts and source-of-truth
  validation posture in policy ledgers. This spec does not mutate a ledger by
  itself.

Required evidence:

- `git diff --check`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`

Claim boundary:

- This spec defines the intended behavior contract for the source-of-truth
  stack. It does not implement the validators, reports, CI wiring, support-tier
  claims, policy ledgers, PR body generator, or closeout generator.

Rollback:

- Revert this spec and index links. No runtime behavior changes.

## Problem

The repo has durable artifacts for why, what, decisions, PR sequencing, current
execution, support claims, policy exceptions, and closeouts. Those artifacts are
useful only when their links are explicit enough for humans, agents, and CI to
verify mechanically.

Without a behavior contract for the stack:

- accepted specs can drift away from their proposals;
- active work items can point at missing plans or missing proof commands;
- support-tier claims can outgrow their proof;
- policy exceptions can lack owner, reason, or review posture;
- closeouts can omit remaining work or claim boundaries;
- agents can invent commands, fields, policies, or merge rules;
- CI can run expensive lanes without a visible proof obligation.

The source-of-truth stack must make the repo answer:

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

## Behavior

The repo maintains a linked source-of-truth stack with distinct artifact roles:

| Layer | Artifact | Behavior |
| --- | --- | --- |
| Direction | Roadmap | Names product direction and milestones. |
| Why | Proposal | Explains problem, users, alternatives, risks, success criteria, and exit criteria. |
| What | Spec | Defines behavior contract, evidence requirements, acceptance examples, and non-goals. |
| Decision | ADR | Records durable architecture or product decisions when needed. |
| Sequence | Implementation plan | Breaks work into PR-sized items with proof commands, rollback, dependencies, and claim boundary. |
| Now | Active goal manifest | Names the currently executing goal and work items in machine-readable form. |
| Claim | Support tiers | Maps user-facing claims to proof commands and maturity tiers. |
| Policy | Ledgers | Records governed exceptions, owners, reasons, review posture, and policy obligations. |
| Proof | CI and commands | Runs the named commands and reports pass, fail, skipped, or advisory state. |
| History | Closeout | Records what landed, what proof ran, what changed, what remains, and the next recommended goal. |

Each artifact may link to adjacent layers, but it must not absorb their truth:

- proposals do not define output schemas or PR order;
- specs do not own support-tier claims, CI lane policy, or campaign queues;
- ADRs do not replace specs or plans;
- plans do not redefine behavior contracts;
- active goal manifests do not invent repo policy;
- support tiers do not redefine specs;
- policy ledgers do not carry product rationale;
- closeouts do not create new behavior contracts.

## Required Evidence

Validators and reports added under this spec should provide cheap mechanical
evidence for the graph:

- document artifact ledger parses;
- artifact IDs are unique;
- artifact files exist;
- artifact IDs appear in their files;
- artifact kind matches its path;
- artifact status is valid for its kind;
- linked proposal, spec, ADR, plan, support-tier, policy, and closeout IDs
  resolve where declared;
- superseded artifacts point to valid replacements;
- accepted specs link to a proposal or record a standalone reason;
- active plans link to at least one proposal or spec;
- active goal manifests parse;
- goal and work-item statuses are valid;
- ready, active, and done work items have proof commands or receipt references;
- blocked work items name blockers;
- active goal work items reference real proposals, specs, and plans;
- unsupported active-goal policy fields are rejected unless the schema defines
  them;
- stable and stabilizing support-tier claims name proof commands;
- README or release stable claims are represented in support tiers once claim
  scanning is mature;
- policy exceptions include owner, reason, scope, and review posture.

Validators should report actionable file paths and fields. Advisory rollout
should precede blocking CI promotion.

## Non-Goals

- No runtime analyzer behavior.
- No mutation execution.
- No coverage dashboard.
- No proof system beyond named repo proof commands.
- No package split.
- No public output schema change in this spec slice.
- No branch-protection promotion before advisory burn-in.
- No support-tier claim promotion without a support-tier row.
- No policy exception without a policy-ledger row.
- No duplicate support-tier truth inside specs.
- No duplicate CI lane truth inside specs.
- No invented active-goal fields or merge rules.
- No reliance on chat history as the source of truth.

## Acceptance Examples

Given an accepted spec with `Linked proposal: RIPR-PROP-0015`, the document
artifact validator resolves `RIPR-PROP-0015` to the proposal file and reports a
clear error if the proposal file or ID is missing.

Given an active goal work item with status `ready`, the goal validator requires
at least one proof command or receipt reference and reports the work item ID if
the proof is missing.

Given a support-tier row that labels a surface `Stable`, the support-tier
validator requires a non-empty proof command and reports the surface name if the
proof is missing.

Given a policy exception row, the policy-ledger validator requires owner,
reason, scope, and review posture before treating the row as valid.

Given a PR that changes only the source-of-truth doctrine docs, the claim
boundary states that no validator, CI behavior, support-tier claim, or policy
ledger has changed.

## Test Mapping

Planned focused tests for `cargo xtask check-doc-artifacts`:

- `rejects_duplicate_artifact_id`;
- `rejects_missing_artifact_file`;
- `rejects_unknown_status`;
- `rejects_missing_linked_proposal`;
- `accepts_superseded_with_replacement`;
- `rejects_kind_path_mismatch`;
- `accepts_standalone_spec_reason`.

Planned focused tests for `cargo xtask check-goals` source-of-truth coverage:

- `rejects_unknown_goal_status`;
- `rejects_unknown_work_item_status`;
- `rejects_ready_item_without_proof`;
- `rejects_blocked_item_without_blocker`;
- `rejects_unknown_referenced_artifact`;
- `rejects_unsupported_policy_field`.

Planned focused tests for `cargo xtask check-support-tiers`:

- `rejects_stable_claim_without_proof`;
- `rejects_empty_proof_command`;
- `accepts_advisory_claim_without_stable_proof`;
- `reports_readme_claim_missing_support_tier_row` once claim scanning is
  mature.

Docs-only PRs that add or amend this spec should run:

```bash
git diff --check
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-doc-index
cargo xtask check-static-language
```

## Implementation Mapping

This spec coordinates these planned source-of-truth slices:

- document artifact ledger in `policy/doc-artifacts.toml`;
- `cargo xtask check-doc-artifacts`;
- support-tier claim map in `docs/status/SUPPORT_TIERS.md`;
- `cargo xtask check-support-tiers`;
- active goal manifest documentation and `cargo xtask check-goals` coverage;
- PR and issue templates under `.github/`;
- advisory source-of-truth CI before blocking promotion;
- `cargo xtask repo-contract-report`;
- `cargo xtask pr-body --work-item <id>`;
- `cargo xtask closeout --goal <goal-id>`.

Existing related surfaces:

- [source-of-truth docs](../source-of-truth/README.md);
- [repo tracking model](../REPO_TRACKING_MODEL.md);
- [spec/proposal system guide](../SPEC_PROPOSAL_SYSTEM.md);
- [Codex goals guide](../CODEX_GOALS.md);
- [support tiers](../status/SUPPORT_TIERS.md);
- [policy ledgers](../../policy/);
- [handoffs](../handoffs/).

## Metrics

Source-of-truth reports should be able to derive:

- total registered artifacts;
- artifacts by kind and status;
- unresolved artifact links;
- accepted specs without proposal or standalone reason;
- active and ready work items;
- ready work items without proof commands;
- blocked work items without blockers;
- stable or stabilizing claims without proof commands;
- policy exceptions missing owner, reason, scope, or review posture;
- superseded artifacts with valid replacements;
- recently completed work items and closeouts.
