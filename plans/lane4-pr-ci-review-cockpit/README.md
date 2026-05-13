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

The current implementation-facing slice is complete:

```text
dogfood(lane4): add PR/CI cockpit gap receipts
```

It adds generated-CI cockpit dogfood receipts without duplicating the shipped
front-panel or packet-index fixture receipts. The next Lane 4 slice is closeout
with language-aware grouping explicitly deferred unless preview-language
evidence is ready.

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
