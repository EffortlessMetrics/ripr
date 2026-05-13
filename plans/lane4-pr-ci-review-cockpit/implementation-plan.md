# Lane 4 PR / CI Review Cockpit Implementation Plan

Status: planning

Lane: 4

Linked lane tracker:
[docs/lanes/LANE_4_PR_CI_REVIEW.md](../../docs/lanes/LANE_4_PR_CI_REVIEW.md)

Existing behavior specs:
[RIPR-SPEC-0023](../../docs/specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
and
[RIPR-SPEC-0024](../../docs/specs/RIPR-SPEC-0024-report-packet-index.md).

Planned proposal path:
`docs/proposals/RIPR-PROP-0002-pr-ci-review-cockpit.md`.

Planned generated-CI workflow spec path:
use the next available `RIPR-SPEC-00NN-generated-pr-ci-review-workflow.md`.
This checkout already uses `RIPR-SPEC-0032` through `RIPR-SPEC-0035`.

## Objective

Turn explicit RIPR PR-time artifacts into a reviewer-first, agent-usable,
advisory PR/CI cockpit: front panel, packet index, generated summary, repair
commands, receipts, language grouping when configured, and clear gate authority
boundaries.

Lane 4 composes explicit artifacts. It must not change analyzer behavior,
mutation execution, source editing, generated tests, editor behavior, policy
semantics, branch protection, or default CI blocking.

## End State

- PR review front panel renders from explicit artifacts only.
- Report packet index groups uploaded artifacts by reviewer use.
- Generated GitHub job summary starts with reviewer-readable next action.
- Missing expected artifacts include regeneration commands.
- Gate decision remains the only configured pass/fail authority.
- Language-aware grouping appears only when `[languages]` declares more than
  Rust.
- Rust-default generated CI output remains unchanged.
- Dogfood receipts cover complete, sparse, blocked, missing-proof,
  unchanged-after-attempt, improved, and preview-language packets.

## Current Baseline

This plan starts from current `main`. It is not a replay of the original
Campaign 24 and Campaign 25 implementation ladders.

Already shipped Lane 4 surfaces:

| Surface | Current evidence |
| --- | --- |
| PR review front panel | [RIPR-SPEC-0023](../../docs/specs/RIPR-SPEC-0023-pr-review-front-panel-report.md), [PR review front panel workflow](../../docs/PR_REVIEW_FRONT_PANEL_WORKFLOW.md), `ripr pr-review front-panel`, generated CI projection, fixture corpus, capability entry, and [Campaign 24 closeout](../../docs/handoffs/2026-05-10-campaign-24-closeout.md) |
| Report packet index | [RIPR-SPEC-0024](../../docs/specs/RIPR-SPEC-0024-report-packet-index.md), [Report packet index workflow](../../docs/REPORT_PACKET_INDEX_WORKFLOW.md), `ripr reports index`, generated CI projection, fixture corpus, dogfood receipts, capability entry, and [Campaign 25 closeout](../../docs/handoffs/2026-05-10-campaign-25-closeout.md) |
| Generated CI wiring for those surfaces | `crates/ripr/src/cli/commands.rs` emits the generated workflow steps, uploads the artifacts, and appends advisory summaries while preserving gate-decision authority |

Future Lane 4 work must audit, compose, document, extend, or close gaps in
those shipped surfaces. It must not create duplicate front-panel or packet-index
producers unless a later spec intentionally changes their public contracts.

## Work Items

### 1. `docs/lane4-source-of-truth`

Goal:
Define the Lane 4 source-of-truth model and PR/CI review cockpit boundaries.

Production delta:
Add `docs/lanes/LANE_4_PR_CI_REVIEW.md`,
`plans/lane4-pr-ci-review-cockpit/README.md`, and this implementation plan.

Non-goals:
No generated CI, report producer, schema, fixture, analyzer, editor, gate,
policy, source-edit, generated-test, provider, or mutation-execution changes.

Acceptance:
The new docs explain what Lane 4 owns, what it consumes, what it does not own,
how proposal/spec/ADR/plan/manifest/policy/handoff artifacts differ, and what
validation gates apply.

Proof commands:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the Lane 4 tracker and plan files plus any index links added for this
slice.

### 2. `docs/lane4-proposal`

Goal:
Add the Lane 4 PR/CI review cockpit proposal.

Production delta:
Add `docs/proposals/RIPR-PROP-0002-pr-ci-review-cockpit.md`.

Non-goals:
No behavior contract edits beyond links to existing and planned specs. No
implementation or generated workflow changes.

Acceptance:
The proposal states the review-compression problem, users and surfaces,
success criteria, alternatives considered, feedback loop, specs to create or
update, non-goals, risks, evidence plan, and exit criteria.

Proof commands:

```bash
rtk cargo xtask check-doc-roles
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the proposal and any index links added for it.

### 3. `docs/lane4-spec-role-alignment`

Goal:
Make the existing front-panel and packet-index specs explicit about their role.

Production delta:
Update RIPR-SPEC-0023 and RIPR-SPEC-0024 with role front-matter and a short
role section that says specs define behavior and acceptance, while this plan
defines PR order.

Non-goals:
No heavy rewrite, schema change, producer change, fixture change, or generated
CI change.

Acceptance:
Both specs retain their behavior contracts and clearly link to the proposal and
plan role boundaries without changing current acceptance semantics.

Proof commands:

```bash
rtk cargo xtask check-spec-format
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Revert the role front-matter and role-section additions.

### 4. `docs/generated-pr-ci-review-workflow-spec`

Goal:
Define the generated PR CI review workflow contract.

Production delta:
Add a new `docs/specs/RIPR-SPEC-00NN-generated-pr-ci-review-workflow.md` using
the next available spec number.

Non-goals:
No workflow implementation, branch-protection change, default blocking change,
hidden analysis rerun, inline comment publishing, source edit, generated test,
or gate semantic change.

Acceptance:
The spec defines generated workflow sections, public command surfaces, artifact
upload contract, job summary contract, advisory/default behavior, gate
authority boundaries, language-aware grouping rules, retry or regeneration
commands, and branch-protection non-goals.

Proof commands:

```bash
rtk cargo xtask check-spec-format
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the generated-CI spec and its spec-index entry.

### 5. `plans/report-packet-index`

Goal:
Add the report packet index implementation plan.

Production delta:
Add `plans/lane4-pr-ci-review-cockpit/report-packet-index.md`.

Non-goals:
No fixture, producer, generated CI, schema, or output change.

Acceptance:
The plan sequences fixture corpus, public `ripr reports index` command,
JSON/Markdown renderer, generated CI integration, dogfood receipt, and closeout
with goal, production delta, non-goals, acceptance, proof commands, and
rollback for each slice.

Proof commands:

```bash
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the report-packet-index plan file.

### 6. `plans/pr-review-front-panel`

Goal:
Add the PR review front panel implementation plan.

Production delta:
Add `plans/lane4-pr-ci-review-cockpit/pr-review-front-panel.md`.

Non-goals:
No fixture, producer, generated CI, schema, or output change.

Acceptance:
The plan sequences explicit artifact input corpus, input status reader,
front-panel selection and fallback states, JSON/Markdown renderer, generated CI
summary projection, and dogfood receipts with goal, production delta,
non-goals, acceptance, proof commands, and rollback for each slice.

Proof commands:

```bash
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the front-panel plan file.

### 7. `goals/lane4-active-manifest`

Goal:
Add a machine-readable Lane 4 manifest without replacing the active Campaign
27 manifest unless Lane 4 becomes the active executor.

Production delta:
Add `.ripr/goals/lanes/lane4-pr-ci-review-cockpit.toml` or another manifest
path supported by the current goals tooling.

Non-goals:
No overwrite of `.ripr/goals/active.toml` while Campaign 27 remains active. No
new goals command behavior unless explicitly selected.

Acceptance:
The manifest includes id, title, status, lane, objective, end_state, work_item
entries, dependencies, stackability where needed, and proof commands.

Proof commands:

```bash
rtk cargo xtask check-campaign
rtk cargo xtask goals status
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the Lane 4 manifest or restore the previous manifest pointer.

### 8. `xtask/check-doc-roles-lane4`

Goal:
Encode the source-of-truth role method into advisory validation.

Production delta:
Extend `cargo xtask check-doc-roles` to cover proposal, spec, ADR, plan, and
goal-manifest role requirements.

Non-goals:
Do not make the check blocking beyond current `check-pr` policy until one
release cycle shows low noise. Do not rewrite old handoffs or plans in this
slice.

Acceptance:
The checker reports missing required role sections and writes a repair-oriented
policy report without changing docs automatically.

Proof commands:

```bash
rtk cargo test -p xtask doc_roles
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Revert the checker extension and keep current proposal/ADR-only validation.

### 9. `audit/pr-review-front-panel-current-state`

Goal:
Record the current PR review front-panel state as a shipped dependency for
future Lane 4 cockpit work.

Production delta:
Update planning, tracker, or closeout docs only if the current shipped
front-panel evidence is stale or hard for agents to discover.

Non-goals:
No duplicate fixture corpus, producer, command, renderer, generated CI step,
schema, or output-contract change. No analyzer, editor, gate, source-edit,
generated-test, provider, or mutation-execution change.

Acceptance:
The Lane 4 plan names the front-panel producer, fixture corpus, generated CI
projection, capability entry, and closeout as current baseline evidence rather
than future TODO work.

Proof commands:

```bash
rtk cargo test -p ripr pr_review_front_panel
rtk cargo xtask check-fixture-contracts
rtk cargo xtask check-output-contracts
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Revert only the audit or planning-doc updates.

### 10. `audit/report-packet-index-current-state`

Goal:
Record the current report packet index state as a shipped dependency for future
Lane 4 cockpit work.

Production delta:
Update planning, tracker, or closeout docs only if the current shipped
packet-index evidence is stale or hard for agents to discover.

Non-goals:
No duplicate fixture corpus, producer, command, renderer, generated CI step,
schema, or output-contract change. No analyzer, editor, gate, source-edit,
generated-test, provider, or mutation-execution change.

Acceptance:
The Lane 4 plan names the packet-index producer, fixture corpus, generated CI
projection, dogfood receipts, capability entry, and closeout as current
baseline evidence rather than future TODO work.

Proof commands:

```bash
rtk cargo test -p ripr report_packet_index
rtk cargo xtask check-fixture-contracts
rtk cargo xtask check-output-contracts
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Revert only the audit or planning-doc updates.

### 11. `docs/generated-ci-cockpit-gap-map`

Goal:
Map the remaining generated-CI cockpit gaps after the shipped front panel and
packet index.

Production delta:
Add or update docs that show how the current generated workflow composes front
panel, packet index, policy artifacts, receipts, and language grouping, and
which remaining behavior belongs in a generated-CI workflow spec.

Non-goals:
No generated workflow implementation change, default blocking change, branch
protection change, inline comment publishing, hidden analysis rerun, source
edit, generated test, provider call, or mutation execution.

Acceptance:
The gap map makes current shipped workflow behavior clear enough that a later
generated-CI spec can define only missing or changed behavior, not duplicate
front-panel or packet-index implementation.

Proof commands:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the generated-CI gap-map doc or revert the related doc updates.

### 12. `ci/language-aware-grouping`

Goal:
Group PR advisory output by language when preview adapters are configured.

Production delta:
Generated CI groups advisory findings by language only when `[languages]`
declares more than Rust.

Non-goals:
Do not start this slice until the preview adapters provide enough Python and
TypeScript evidence. No Rust-default behavior change. No gate authority change.

Acceptance:
`[languages] = ["rust"]` remains byte-for-byte or behavior-equivalent to the
current generated CI summary, while configured preview languages are visibly
labeled preview/advisory.

Proof commands:

```bash
rtk cargo test -p ripr language_aware
rtk cargo xtask fixtures
rtk cargo xtask goldens check
rtk cargo xtask check-workflows
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove language grouping from generated summaries and retain Rust-default
summary behavior.

### 13. `dogfood/lane4-cockpit-gap-receipts`

Goal:
Refresh or add PR/CI review cockpit dogfood receipts for gaps not already
covered by Campaign 24 and Campaign 25.

Production delta:
Add repo-local receipts only for newly identified cockpit gaps, such as
language-preview grouping or cross-surface missing-proof handoff states not
already covered by front-panel or packet-index receipts.

Non-goals:
Do not duplicate existing front-panel or packet-index dogfood cases. No new
report semantics. No analyzer, gate, editor, source-edit, generated-test,
provider, mutation, or default-blocking changes.

Acceptance:
`cargo xtask dogfood` or a lane-specific dogfood command writes checked
receipts for the remaining gap states and links back to the existing Campaign
24 and Campaign 25 receipts for covered states.

Proof commands:

```bash
rtk cargo xtask dogfood
rtk cargo xtask check-output-contracts
rtk cargo xtask check-capabilities
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove only the newly added gap receipts and dogfood wiring.

### 14. `docs/lane4-closeout`

Goal:
Close the PR/CI review cockpit lane with durable proof and restart context.

Production delta:
Add `docs/handoffs/YYYY-MM-DD-lane4-pr-ci-review-cockpit-closeout.md` and
update capability, roadmap, implementation, and lane status surfaces as needed.

Non-goals:
No new behavior in the closeout PR. Do not reopen analyzer, editor, gate,
policy, generated workflow, or preview-language work.

Acceptance:
The closeout states what shipped, what did not change, validation commands,
remaining work, known limits, and next-lane handoff.

Proof commands:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-capabilities
rtk cargo xtask check-traceability
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the closeout and restore the previous lane/capability status.

## Stop Conditions

Stop and write a blocked report instead of broadening the PR if a slice would
require:

- changing analyzer evidence semantics;
- adding or changing public output schemas outside the selected spec;
- changing policy or gate authority;
- changing branch protection or default CI blocking;
- adding dependencies;
- publishing inline PR comments outside the explicit inline-comment lane;
- changing editor or LSP behavior;
- changing release, package, or publish behavior;
- creating source edits or generated tests;
- running provider calls or mutation execution.
