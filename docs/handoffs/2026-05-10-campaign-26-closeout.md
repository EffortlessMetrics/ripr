# Handoff: Campaign 26 Closeout

Date: 2026-05-10
Branch / PR: `campaign-pr-inline-comment-publisher-closeout` / pending at authoring
Latest merged PR: #714 `dogfood: add PR inline comment publisher receipts` (commit `9b1e9c0`)

## Current Work Item

`campaign/pr-inline-comment-publisher-closeout`

Campaign 26 made durable PR inline comments an explicit opt-in projection over
existing PR guidance:

```text
review-comments JSON and optional existing RIPR comment metadata
-> read-only comment-publish-plan.{json,md}
-> generated CI plan-mode artifacts and summaries
-> inline create/update publishing only when explicitly configured and safe
-> reviewer workflow docs
-> dogfood receipts for representative publish-plan states
```

Inline comments remain disabled by default. Summary-only guidance is never
published inline. The plan is advisory and separate from gate authority.

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow default blocking,
source-edit behavior, generated-test behavior, provider calls, mutation
execution, public crate shape, release posture, branch protection, security
posture, or `pull_request_target` defaults.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #708 opened Campaign 26 as PR Inline Comment Publisher after report-packet indexing, keeping it separate from analyzer, ranking, gate, editor, platform, release, security, and future comment-policy lanes. |
| Publisher contract exists before implementation | #709 added RIPR-SPEC-0025, output-schema coverage, capability metadata, traceability, campaign docs, roadmap, plan, proposal, and changelog updates for an advisory read-only publish plan before any GitHub write. |
| Fixture corpus is pinned before producer work | #710 added `fixtures/boundary_gap/expected/pr-inline-comment-publisher/` with publishable changed-line, summary-only excluded, cap overflow, dedupe/upsert, stale-existing, fork or no-token, and missing-input cases plus fixture checks. |
| Read-only plan producer exists | #711 added `ripr pr-comments plan`, JSON/Markdown rendering, explicit PR guidance and optional existing-comment inputs, operation/skip/block vocabularies, cap/dedupe/permission checks, and CLI/fixture coverage without posting comments. |
| Generated CI opt-in wiring exists | #712 kept `RIPR_COMMENT_MODE=off` by default, added plan-mode artifacts and summaries, captured existing RIPR comment metadata only for opt-in modes, and posted create/update operations only in explicit `inline` mode when the safe plan permits it. |
| Generated CI remains advisory by default | #712 keeps inline comments disabled unless explicitly configured, preserves gate-decision authority, avoids `pull_request_target`, and leaves default generated workflows as job summary, check annotations, and uploaded artifacts. |
| Reader-facing workflow docs exist | #713 added `docs/PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md`, explaining `off`, `plan`, and `inline` modes, publish-plan review, caps, dedupe/upsert, forks, permissions, rollback, and advisory gate boundaries. |
| Dogfood receipts cover publisher states | #714 extended `cargo xtask dogfood` with checked repo-local publish-plan receipts for publishable, summary-only, capped, dedupe/upsert, stale-existing, fork or no-token, and missing-input cases without posting real PR comments. |
| Capability and traceability surfaces are updated | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `.ripr/traceability.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, and `docs/ROADMAP.md` point to the closed Campaign 26 evidence package. |
| Future lane boundary is explicit | No next campaign is opened by this closeout. Future comment policy, PR summary polish, artifact history, policy, editor, analyzer, platform, release, dependency, or MSRV work must be opened explicitly. |

## PR Chain

- #708 `campaign: open PR inline comment publisher`
- #709 `spec: define PR inline comment publisher contract`
- #710 `fixtures: pin PR inline comment publisher corpus`
- #711 `report: add PR inline comment publish plan`
- #712 `ci: add optional PR inline comment publisher`
- #713 `docs: explain PR inline comment publisher workflow`
- #714 `dogfood: add PR inline comment publisher receipts`
- `campaign/pr-inline-comment-publisher-closeout`

## Verification Run

Closeout validation before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 26 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely follow-up lanes should stay separated:

- PR inline comment policy improvements over the existing publish-plan contract;
- comment publisher telemetry or history over existing plan artifacts;
- PR summary polish over existing front-panel, packet-index, and publish-plan
  artifacts;
- analyzer evidence, recommendation ranking, gate policy, editor, platform,
  release, dependency, or MSRV cleanup.

Those should not be folded into Campaign 26 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make comment publishing the pass/fail authority.
- Do not publish inline comments by default.
- Do not publish `summary_only` guidance as inline comments.
- Do not place comments on unchanged or unsafe lines.
- Do not duplicate durable comments across reruns.
- Do not hide disabled, missing-input, fork, missing-token, missing-permission,
  capped, stale-existing, skipped, blocked, or warning states.
- Do not introduce `pull_request_target` defaults or unproven fork behavior.
- Do not claim runtime mutation outcomes, adequacy, correctness, or proof from
  static evidence.
- Do not run cargo-mutants or any mutation engine from inline-comment workflows.
- Do not move analyzer identity, recommendation ranking, gate policy semantics,
  or editor behavior into inline-comment closeout work.
- Do not generate tests, edit source, post free-form review comments, or call
  LLM providers from the inline-comment surface by default.
