# Generated CI Cockpit Gap Map

Status: planning

Lane: 4

Linked proposal:
[RIPR-PROP-0004: PR / CI Review Cockpit](../../docs/proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md)

Linked specs:

- [RIPR-SPEC-0023: PR Review Front Panel Report](../../docs/specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
- [RIPR-SPEC-0024: Report Packet Index](../../docs/specs/RIPR-SPEC-0024-report-packet-index.md)
- [RIPR-SPEC-0038: Generated PR CI Review Workflow](../../docs/specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md)

## Purpose

This map records what Lane 4 generated CI already does and what remains open.
It prevents future PRs from rebuilding shipped front-panel or packet-index
surfaces when the right next step is audit, composition, language grouping,
receipt coverage, or closeout.

## Already Shipped

| Surface | Evidence |
| --- | --- |
| PR review front panel contract | [RIPR-SPEC-0023](../../docs/specs/RIPR-SPEC-0023-pr-review-front-panel-report.md) |
| PR review front panel producer | `ripr pr-review front-panel` |
| PR review front panel fixtures | `fixtures/boundary_gap/expected/pr-review-front-panel/` |
| PR review front panel workflow docs | [PR review front panel workflow](../../docs/PR_REVIEW_FRONT_PANEL_WORKFLOW.md) |
| PR review front panel closeout | [Campaign 24 closeout](../../docs/handoffs/2026-05-10-campaign-24-closeout.md) |
| Report packet index contract | [RIPR-SPEC-0024](../../docs/specs/RIPR-SPEC-0024-report-packet-index.md) |
| Report packet index producer | `ripr reports index` |
| Report packet index fixtures | `fixtures/boundary_gap/expected/report-packet-index/` |
| Report packet index workflow docs | [Report packet index workflow](../../docs/REPORT_PACKET_INDEX_WORKFLOW.md) |
| Report packet index receipts | [Report packet index receipts](../../docs/handoffs/2026-05-10-report-packet-index-receipts.md) |
| Report packet index closeout | [Campaign 25 closeout](../../docs/handoffs/2026-05-10-campaign-25-closeout.md) |
| Generated CI wiring for both projections | `crates/ripr/src/cli/commands.rs` |

## Current Generated Workflow Shape

Current baseline audit:
[generated-ci-baseline-audit.md](generated-ci-baseline-audit.md).

The generated GitHub workflow currently has the right high-level cockpit
ordering for the shipped surfaces:

```text
Generate advisory RIPR reports
-> render PR review front panel when explicit inputs exist
-> render report packet index when packet inputs exist
-> append advisory Markdown to the job summary
-> upload report artifacts
-> keep gate-decision authority separate
```

Important current behavior:

- `ripr pr-review front-panel` is invoked only after explicit upstream report
  inputs are present.
- `ripr reports index` is invoked over explicit report, review, receipt,
  workflow, agent, pilot, and CI directories.
- Missing front-panel or packet-index inputs are surfaced as warnings or
  no-input messages rather than hidden success.
- Generated CI uploads lower-level artifacts along with the projected cockpit
  surfaces.
- Gate decisions remain separate artifacts; summaries and indexes do not become
  gate authority.

## Remaining Gaps

| Gap | Owner slice | Notes |
| --- | --- | --- |
| Generated CI contract | `docs/generated-pr-ci-review-workflow-spec` | Done by [RIPR-SPEC-0038](../../docs/specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md). |
| Current workflow audit | `audit/generated-ci-cockpit-baseline` | Done by [generated-ci-baseline-audit.md](generated-ci-baseline-audit.md). |
| Reviewer-first summary polish | `ci/generated-summary-cockpit-contract` | Done: the generated job summary includes a `Start here` section. |
| Missing-artifact repair commands | `ci/generated-summary-cockpit-contract` | Done for known first-useful-action, front-panel, and packet-index regeneration commands. |
| Language-aware grouping | `ci/language-aware-grouping` | Wait until preview adapters provide enough TypeScript and Python evidence, or explicitly defer. |
| Generated-CI cockpit receipts | `dogfood/lane4-cockpit-gap-receipts` | Done by [generated-CI cockpit dogfood receipts](../../docs/handoffs/2026-05-13-generated-ci-cockpit-receipts.md); Campaign 24 and 25 receipts remain the fixture-backed source for front-panel and packet-index states. |
| Preview-language packet receipts | `ci/language-aware-grouping` | Deferred with language-aware grouping until preview adapters provide enough TypeScript and Python evidence, or the lane explicitly defers Python. |
| Lane closeout | `docs/lane4-closeout` | Record what shipped, what did not change, validation, known limits, and next-lane handoff. |

## Non-Gaps

The following are not Lane 4 implementation gaps:

- rebuilding `ripr pr-review front-panel`;
- rebuilding `ripr reports index`;
- adding a duplicate cockpit command that competes with those producers;
- changing analyzer classification or recommendation ranking;
- changing LSP/editor routing;
- changing gate policy semantics;
- changing default CI blocking or branch protection;
- publishing inline comments;
- generating tests or editing source;
- calling providers or running mutation testing.

## Next Work Packet

The next Lane 4 work item is:

```text
docs(lane4): close PR/CI review cockpit lane
```

That PR should record the shipped cockpit surfaces, validation, explicit
non-changes, deferred language-aware grouping, and next-lane handoff.

## Validation

Docs-only updates to this map should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```
