# PR Inline Comment Publisher Proposal

Campaign: 26, PR Inline Comment Publisher

Spec: [RIPR-SPEC-0025](specs/RIPR-SPEC-0025-pr-inline-comment-publisher.md)

## Problem

Lane 4 now has the PR review front panel, first useful action, assistant proof
and health, PR evidence ledger, baseline and RIPR Zero reports, gate decisions,
coverage/grip frontier, report-packet index, validation reports, receipts,
SARIF, and badge outputs. Generated CI also emits changed-line check
annotations from `ripr review-comments`.

Durable PR review comments remain deliberately absent from default workflows.
That is still the right default: persistent review-thread noise is expensive
when placement, ranking, permissions, or dedupe are wrong.

Some teams will still want explicit inline PR comments after they trust the
summary and annotation surface. That requires a focused publisher contract
rather than ad hoc workflow snippets.

## Goal

Make optional inline RIPR PR comments safe and reviewable by introducing a
read-only publish plan before any GitHub mutation.

The publisher path should consume the existing `ripr review-comments` artifact:

```text
target/ripr/review/comments.json
```

It should first render:

```text
target/ripr/review/comment-publish-plan.json
target/ripr/review/comment-publish-plan.md
```

The plan must show what would be posted, updated, skipped, capped, or blocked
by permissions. A later explicit opt-in publisher can consume that plan to
upsert comments.

## Current Inputs

The campaign should account for these existing surfaces when present:

- `target/ripr/review/comments.json`
- `target/ripr/review/comments.md`
- `comments[]` entries with changed-line placement
- `summary_only[]` entries that must never become inline comments
- `suppressed[]` entries that explain cap or suppression decisions
- `dedupe_key` values from PR guidance
- optional existing RIPR comment metadata
- optional pull request number and head/base context from generated CI
- optional workflow comment mode
- optional token or permission availability

## End State

- Inline comments remain disabled by default.
- A read-only publish plan exists before posting behavior.
- The plan posts only from `comments[]`, never from `summary_only[]`.
- The plan targets only changed lines that already have safe placement.
- The default cap is three comments.
- Dedupe keys drive update or replacement of prior RIPR comments.
- Missing token, fork restrictions, missing PR context, or disabled mode produce
  visible no-op states, not hidden failures.
- Any actual publisher is explicit opt-in, advisory, and separate from gate
  pass/fail authority.
- Generated CI defaults stay job summary plus check annotations plus artifacts.

## Comment Modes

The expected mode vocabulary is:

```text
off
plan
inline
```

- `off`: default. No publish plan is generated and no comments are posted.
- `plan`: generate and upload the read-only publish plan, but post nothing.
- `inline`: generate the plan and publish only operations marked safe.

The exact config surface should be specified before implementation. A generated
workflow environment variable such as `RIPR_COMMENT_MODE` is acceptable if the
spec pins the default and permission behavior.

## Safety Rules

- Use only explicit `review-comments` artifacts.
- Do not rerun hidden analysis.
- Do not post `summary_only[]` recommendations.
- Do not invent placement.
- Do not post more than three comments by default.
- Do not duplicate comments across reruns.
- Do not make comments a gate authority.
- Do not use `pull_request_target` by default.
- Do not post on untrusted fork PRs unless a future policy explicitly proves a
  safe permission model.
- Do not change branch protection or repository settings.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy changes.
- No LSP/editor behavior changes.
- No provider calls.
- No source edits.
- No generated tests.
- No mutation execution.
- No default CI blocking.
- No default inline comment publishing.
- No free-form LLM review.
- No broad GitHub review bot behavior.

## Proposed PR Sequence

1. `spec: define PR inline comment publisher contract` - done
2. `fixtures: pin PR inline comment publisher corpus` - done
3. `report: add PR inline comment publish plan` - next
4. `ci: add optional PR inline comment publisher`
5. `docs: explain PR inline comment publisher workflow`
6. `dogfood: add PR inline comment publisher receipts`
7. `campaign: close PR inline comment publisher`

## Validation Baseline

Campaign slices should use the scoped commands from `.ripr/goals/active.toml`.
The campaign closeout should rerun:

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
