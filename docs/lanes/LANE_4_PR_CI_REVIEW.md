# Lane 4: PR / CI Review Cockpit

Lane 4 owns RIPR's PR and CI review surfaces. Its job is to make the
pull-request evidence packet readable as one reviewer-first story:

```text
PR diff
-> advisory generated workflow
-> PR review front panel
-> report packet index
-> optional language grouping when configured
-> repair or agent handoff
-> receipt and movement evidence
-> explicit gate authority boundary
```

Lane 4 composes explicit RIPR artifacts. It does not create analyzer evidence,
rerun hidden analysis, decide policy, edit source, generate tests, call
providers, run mutation testing, or change default CI blocking.

## Product Target

A reviewer, maintainer, or coding agent should be able to open one PR summary
or uploaded packet and answer:

- what changed;
- what test gap matters most;
- whether the issue is PR-local risk, baseline debt, waived, suppressed,
  blocked, already improved, or missing evidence;
- what should happen next;
- which artifact supports that next step;
- which command regenerates a missing surface;
- what, if anything, has configured pass/fail authority.

The generated workflow stays cheap and advisory by default. When a configured
gate exists, `gate-decision.{json,md}` remains the pass/fail authority; front
panels, packet indexes, job summaries, and language grouping are projections.

## Source Of Truth Stack

Use one document for one job:

| Layer | Owns | RIPR storage |
| --- | --- | --- |
| Roadmap / campaign | release and lane direction | [Roadmap](../ROADMAP.md), [Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md) |
| Proposal / PRD | why the lane exists, user value, alternatives, success criteria | [RIPR-PROP-0004](../proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md) |
| Specs | behavior contracts and acceptance examples | [RIPR-SPEC-0023](../specs/RIPR-SPEC-0023-pr-review-front-panel-report.md), [RIPR-SPEC-0024](../specs/RIPR-SPEC-0024-report-packet-index.md), later generated-CI specs |
| ADRs | durable architecture decisions | [ADRs](../adr/) |
| Lane tracker | lane-local scope, boundaries, current plan, validation gates | this document |
| Implementation plan | PR-sized sequencing and proof commands | [Lane 4 plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md) |
| Active goal manifest | current Codex/Droid execution state | `.ripr/goals/active.toml` or a lane manifest when Lane 4 is active |
| Policy ledgers | CI, gate, exception, and allowlist truth | `policy/*.toml`, generated workflow policy checks |
| Capability and proof map | public claim to evidence linkage | [Capability matrix](../CAPABILITY_MATRIX.md), `metrics/capabilities.toml` |
| Closeout / handoff | what landed, validation, remaining work, restart context | [Handoffs](../handoffs/) |

Proposal explains why. Specs define what must be true. ADRs record durable
architecture decisions. Plans sequence PRs. Active manifests tell agents what
to do now. Policy ledgers own authority and exceptions. Closeouts record what
happened and what remains.

## Scope

Lane 4 owns these surfaces:

- generated GitHub PR workflow shape;
- generated job-summary language;
- PR review front panel projection;
- report packet index projection;
- artifact upload completeness and navigation;
- missing-surface regeneration commands;
- repair, agent handoff, and receipt links in PR-time artifacts;
- PR-local movement summaries from supplied evidence;
- language-aware grouping in generated CI when preview languages are configured;
- clear separation between advisory summaries and configured gate authority.

Lane 4 consumes:

- PR guidance and review-comment artifacts;
- first-useful-action artifacts;
- test-oracle assistant proof and health artifacts;
- baseline debt delta, RIPR Zero, PR evidence ledger, and receipt artifacts;
- optional gate-decision artifacts;
- optional recommendation, mutation, coverage, and grip calibration artifacts;
- language metadata supplied by analyzer/output lanes.

Lane 4 does not own:

- analyzer classification or recommendation ranking;
- evidence identity or evidence-record semantics;
- mutation execution;
- source edits or generated tests;
- LSP or editor behavior;
- provider calls;
- policy or gate semantics;
- branch protection, required checks, or default CI blocking;
- inline PR comment publishing unless the inline-comment lane explicitly owns
  that slice.

## Current Behavior Specs

Existing Lane 4 behavior contracts:

- [RIPR-SPEC-0023: PR Review Front Panel Report](../specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
  defines the read-only first-screen report over explicit artifacts.
- [RIPR-SPEC-0024: Report Packet Index](../specs/RIPR-SPEC-0024-report-packet-index.md)
  defines the reviewer-first map over the uploaded report packet.

Lane 4 generated-CI composition is governed by
[RIPR-SPEC-0038](../specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md).
This checkout already uses `RIPR-SPEC-0032` through `RIPR-SPEC-0038`, so
future work must not reuse those IDs.

## Plan

The current Lane 4 plan is in
[plans/lane4-pr-ci-review-cockpit](../../plans/lane4-pr-ci-review-cockpit/README.md).
That plan owns PR order. The specs own behavior truth.

The generated-CI cockpit gap map is in
[plans/lane4-pr-ci-review-cockpit/generated-ci-gap-map.md](../../plans/lane4-pr-ci-review-cockpit/generated-ci-gap-map.md).

Lane 4 closeout is recorded in
[2026-05-13-lane4-pr-ci-review-cockpit-closeout.md](../handoffs/2026-05-13-lane4-pr-ci-review-cockpit-closeout.md).

The final slice is docs-only closeout:

```text
docs(lane4): close PR/CI review cockpit lane
```

This slice records shipped surfaces, validation, deferred language-aware
grouping, explicit non-changes, known limits, and next-lane handoff without
changing generated CI, report producers, output contracts, policy, fixtures,
code, or schemas.

## Validation Gates

Docs-only Lane 4 scaffolding should run:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Future behavior or generated-CI changes should add the relevant fixture,
golden, output-contract, generated-workflow, dogfood, and traceability checks
named by the selected work item.

## Cross-Lane Rules

- Lane 4 may summarize analyzer, policy, agent, and language artifacts, but it
  must preserve their authority boundaries.
- Missing artifacts stay visible as missing, stale, malformed, incomplete, or
  configured off. Lane 4 must not convert absence into success.
- Preview-language evidence stays labeled preview and advisory. Rust-default
  generated CI behavior must remain unchanged unless a later policy artifact
  explicitly changes it.
- Generated job summaries and packet indexes may point to gate decisions, but
  they do not become gate decisions.
- If a PR-time surface exposes weak analyzer evidence, fix the evidence source
  in the owning lane and keep the Lane 4 projection change narrow.

## Operating Rule

Before taking a Lane 4 task, confirm it changes PR or CI review composition,
artifact navigation, generated summaries, repair handoff, receipt projection,
or language grouping. If it changes analyzer truth, editor behavior, policy
semantics, release mechanics, dependencies, or source-edit automation, route it
to the owning lane first.
