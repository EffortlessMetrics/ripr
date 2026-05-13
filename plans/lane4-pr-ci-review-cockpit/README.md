# Lane 4 PR / CI Review Cockpit Plan

This directory sequences Lane 4 work into scoped PRs. It does not define
behavior truth by itself; behavior contracts stay in specs, durable
architecture decisions stay in ADRs, and policy authority stays in policy and
gate artifacts.

Start with the [Lane 4 tracker](../../docs/lanes/LANE_4_PR_CI_REVIEW.md).

## Documents

- [Implementation plan](implementation-plan.md) - PR-sized sequence for the
  PR/CI review cockpit lane.
- [Generated CI gap map](generated-ci-gap-map.md) - shipped generated-CI
  cockpit surfaces, remaining gaps, and non-gaps.
- [Generated CI baseline audit](generated-ci-baseline-audit.md) - current
  generated workflow commands, summary shape, uploads, missing-artifact
  behavior, and gate boundary.

## Role Model

Use one artifact for one job:

| Artifact | Job |
| --- | --- |
| Proposal | explains why the lane exists, who benefits, alternatives, and success criteria |
| Spec | defines observable behavior, required evidence, acceptance examples, and validation |
| ADR | records durable architecture decisions only |
| Lane tracker | records lane scope, ownership, boundaries, and current operating rule |
| Plan | sequences PR-sized implementation slices and proof commands |
| Active manifest | tells Codex or Droid what to execute now |
| Policy ledger | owns CI, gate, exception, and allowlist authority |
| Closeout | records what shipped, validation, remaining work, and restart context |

Do not duplicate existing specs. Link to
[RIPR-SPEC-0023](../../docs/specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
and
[RIPR-SPEC-0024](../../docs/specs/RIPR-SPEC-0024-report-packet-index.md)
when planning front-panel or packet-index work.

## Current Slice

The first PR is docs-only scaffolding:

```text
docs(lane4): define PR/CI review cockpit source-of-truth model
```

It adds the lane tracker and this plan directory. It does not change generated
CI, report producers, fixtures, schemas, gate policy, default blocking,
language routing, analyzer behavior, editor behavior, source edits, or
generated tests.

## Operating Constraints

- One semantic artifact per PR.
- Specs define behavior, not PR order.
- Plans define PR order, not behavior truth.
- Proposals explain why and alternatives.
- ADRs are only for durable architecture choices.
- Active manifests are machine-readable execution state and must include proof
  commands.
- Rust-default generated CI behavior remains unchanged.
- Preview-language grouping is opt-in and advisory.
- Gate decisions remain the configured pass/fail authority.
- Missing or preview evidence is labeled rather than hidden.
