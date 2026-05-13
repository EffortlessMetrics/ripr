# Handoff: Lane 4 PR / CI Review Cockpit Closeout

Date: 2026-05-13
Branch / PR: `lane4-pr-ci-review-cockpit-closeout` / pending at authoring
Latest merged PR: #867 `dogfood(lane4): add generated CI cockpit receipts`

## Current Work Item

`docs/lane4-closeout`

Lane 4 made the existing PR-time reports operate as a reviewer-first cockpit:

```text
explicit PR artifacts
-> first useful action
-> PR review front panel
-> report packet index
-> generated CI Start here summary
-> regeneration commands
-> dogfood receipts
-> explicit gate authority boundary
```

The lane composes existing artifacts. It does not create analyzer truth or
make summaries into gates.

## What Shipped

| Surface | Evidence |
| --- | --- |
| Lane source-of-truth model | #849 added [Lane 4 tracker](../lanes/LANE_4_PR_CI_REVIEW.md) and [Lane 4 plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md). |
| Lane proposal | #854 added [RIPR-PROP-0004](../proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md). |
| Role-aligned specs | #855 aligned [RIPR-SPEC-0023](../specs/RIPR-SPEC-0023-pr-review-front-panel-report.md) and [RIPR-SPEC-0024](../specs/RIPR-SPEC-0024-report-packet-index.md) with Lane 4 source-of-truth roles. |
| Generated CI workflow contract | #856 added [RIPR-SPEC-0038](../specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md). |
| Generated CI gap map | #858 added [generated-ci-gap-map.md](../../plans/lane4-pr-ci-review-cockpit/generated-ci-gap-map.md). |
| Lane manifest | #860 added [.ripr/goals/lane4-pr-ci-review-cockpit.toml](../../.ripr/goals/lane4-pr-ci-review-cockpit.toml) without replacing Campaign 27 active execution. |
| Generated CI baseline audit | #862 added [generated-ci-baseline-audit.md](../../plans/lane4-pr-ci-review-cockpit/generated-ci-baseline-audit.md). |
| Reviewer-first generated summary | #864 added the generated CI `Start here` section and regeneration commands for first-useful-action, front-panel, and packet-index surfaces. |
| Generated CI dogfood receipts | #867 added [generated-CI cockpit dogfood receipts](2026-05-13-generated-ci-cockpit-receipts.md) and the `generated_ci_cockpit` dogfood report section. |

## What Did Not Change

- Analyzer classification and recommendation ranking.
- Evidence identity or evidence-record semantics.
- PR review front-panel producer behavior.
- Report packet-index producer behavior.
- LSP or editor behavior.
- Gate policy semantics or branch protection.
- Default CI blocking.
- Inline comment defaults.
- Source edits or generated tests.
- Provider calls.
- Mutation execution.

## Deferred Work

Language-aware grouping did not ship in this lane closeout. It remains
deferred until preview adapters provide enough TypeScript and Python evidence,
or until the lane explicitly defers Python and accepts a narrower grouping
slice. Rust-default generated CI behavior remains unchanged.

The deferred work item remains recorded as:

```text
ci/language-aware-grouping
```

## Verification Run

Closeout validation before opening this PR:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Previously merged behavior receipts in this lane also passed:

```bash
cargo test -p ripr init_generated_github_workflow
cargo test -p xtask dogfood_
cargo xtask dogfood
cargo xtask check-output-contracts
```

## Artifacts

- `target/ripr/reports/dogfood.json`
- `target/ripr/reports/dogfood.md`
- `docs/handoffs/2026-05-13-generated-ci-cockpit-receipts.md`
- `plans/lane4-pr-ci-review-cockpit/generated-ci-gap-map.md`
- `plans/lane4-pr-ci-review-cockpit/generated-ci-baseline-audit.md`
- `.ripr/goals/lane4-pr-ci-review-cockpit.toml`

## Next Lane Handoff

Campaign 28, Spec Graph and Agent Context Packets, can now start without
depending on unfinished Lane 4 doctrine. Use the merged Lane 4 artifacts as
proof that proposal/spec/plan/manifest/receipt/closeout roles are now encoded
for PR/CI review surfaces.

If a future operator resumes Lane 4 instead, the only remaining implementation
slice is language-aware grouping after preview-language readiness is explicit.

## What Not To Do

- Do not rebuild `ripr pr-review front-panel`.
- Do not rebuild `ripr reports index`.
- Do not make generated summaries pass/fail authority.
- Do not change default generated CI blocking.
- Do not promote preview-language evidence without preview labels.
- Do not fold analyzer, editor, gate policy, provider, mutation, release, or
  source-edit behavior into Lane 4 closeout work.
