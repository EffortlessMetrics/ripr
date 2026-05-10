# RIPR-SPEC-0025: PR Inline Comment Publisher

Status: proposed

## Problem

`ripr review-comments` already writes bounded PR guidance and generated CI
already emits job summaries plus changed-line check annotations. Inline PR
review comments remain deliberately absent from default workflows because they
create durable review-thread noise when placement, permissions, caps, or
deduplication are wrong.

Some teams will still want an explicit inline-comment mode after they trust the
summary, annotation, and report-packet surfaces. That mode needs a reviewable
contract before any workflow posts comments.

This spec defines a read-only publish plan and the later opt-in publisher
boundary. The plan is the artifact reviewers and workflow authors inspect
before a GitHub API write happens.

## Product Contract

The PR inline comment publisher is an opt-in projection over existing
`ripr review-comments` output. It consumes explicit artifacts and produces a
bounded publish plan before any comment is posted.

The publisher path must:

- consume `target/ripr/review/comments.json` as the guidance source;
- post only from `comments[]`;
- never post `summary_only[]` guidance as inline comments;
- use only the changed-line placement already present on a `comments[]` item;
- cap publishable comments to three by default;
- deduplicate and upsert by `dedupe_key`;
- preserve advisory language and static-evidence limits;
- make disabled, missing-permission, missing-token, fork, and missing-input
  states visible as no-op or blocked plan entries;
- keep generated workflows comment-free by default.

It must not rerun hidden analysis, inspect source to invent placement, change
analyzer behavior, change recommendation ranking, change gate policy, change
LSP/editor behavior, edit source, generate tests, call providers, run mutation
testing, change branch protection, introduce `pull_request_target` defaults, or
make CI blocking by default.

## Behavior

The canonical flow is:

```text
review-comments JSON
-> optional existing RIPR comment metadata
-> explicit comment mode and permission context
-> read-only publish plan
-> optional publisher consumes safe plan operations
```

The plan producer is pure. It must not call GitHub, write PR comments, mutate
source files, or change gate decisions. The later publisher may call GitHub
only when an explicit workflow mode enables inline comments and the permission
context is safe.

## Inputs

The publish-plan producer should accept explicit paths and context:

```text
ripr pr-comments plan \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --existing-comments target/ripr/review/existing-comments.json \
  --mode plan \
  --event-name pull_request \
  --pull-request 123 \
  --head-repo EffortlessMetrics/ripr \
  --base-repo EffortlessMetrics/ripr \
  --out target/ripr/review/comment-publish-plan.json \
  --out-md target/ripr/review/comment-publish-plan.md
```

The exact CLI name may be adjusted during implementation, but the public
behavior must preserve the explicit-input, read-only plan boundary. Generated
CI must not post comments directly from `review-comments` without first
materializing the plan.

Input artifacts and context:

| Input | Required? | Purpose |
| --- | --- | --- |
| `target/ripr/review/comments.json` | yes for useful plans | Source `comments[]`, `summary_only[]`, `suppressed[]`, `dedupe_key`, limits, and advisory text. |
| Existing RIPR comments metadata | optional | Identifies prior RIPR comments by `dedupe_key` for upsert, replace, or delete decisions. |
| Comment mode | yes | `off`, `plan`, or `inline`; default is `off`. |
| Pull request number | required for posting | Identifies the PR thread for later publisher operations. |
| Event name | required for permission decisions | Distinguishes `pull_request`, `workflow_dispatch`, and other contexts. |
| Head and base repository | required for fork checks | Keeps untrusted fork PRs from silently posting comments. |
| Token or permission availability | required for posting | Missing write permission produces visible blocked operations. |

The producer should still render a useful no-op plan when the guidance input is
missing or the mode is `off`.

## Comment Modes

`mode` must be one of:

- `off`: default. No comments are posted. The plan may report that publishing
  is disabled, but generated CI is not required to produce a plan in this mode.
- `plan`: render and upload the read-only publish plan. Do not post comments.
- `inline`: render the publish plan and allow a separate publisher step to
  execute operations marked `safe_to_publish = true`.

Generated workflows must keep `off` as the default. A repository must opt in
explicitly, for example with:

```yaml
env:
  RIPR_COMMENT_MODE: plan # off | plan | inline
```

`inline` mode must not imply gate authority. Gate decisions remain governed by
`ripr gate evaluate` and `gate-decision.{json,md}` when configured.

## Permission Boundary

The default generated workflow must not use `pull_request_target`.

Inline publishing is safe only when all of these are true:

1. mode is `inline`;
2. the event context supplies a pull request number;
3. the workflow has pull-request write permission;
4. the PR is same-repository, or a future explicitly documented fork policy is
   enabled and proven safe;
5. the plan operation comes from a `comments[]` item with changed-line
   placement;
6. the operation is within the configured cap;
7. the operation has a `dedupe_key`.

If any required condition is absent, the plan must record a visible no-op or
blocked operation instead of silently succeeding or posting through another
surface.

## Publish Operation Vocabulary

Each candidate operation must have one `operation`:

- `create`: no existing RIPR comment matches the `dedupe_key`; create a new
  inline comment when publishing is enabled and safe.
- `update`: an existing RIPR comment with the same `dedupe_key` exists and the
  body or placement should be refreshed.
- `keep`: an existing RIPR comment with the same `dedupe_key` already matches
  the planned body and placement.
- `delete`: an existing RIPR comment is stale and no longer corresponds to a
  publishable `comments[]` item. Deletion is optional and must be explicit in
  implementation; the plan still records the stale state.
- `skip`: the item is not publishable but is not an error.
- `blocked`: publishing would require unsafe placement, missing permission,
  missing context, unsupported event context, fork trust policy, missing
  `dedupe_key`, or another hard safety condition.

`operation` values are plan semantics, not CI pass/fail outcomes.

## Skip And Block Reasons

`skip_reason` and `blocked_reason` must use bounded vocabulary.

Skip reasons:

- `mode_off`
- `summary_only`
- `suppressed`
- `cap_reached`
- `unchanged_tests`
- `not_publishable`
- `already_current`

Blocked reasons:

- `missing_pr_guidance`
- `malformed_pr_guidance`
- `missing_pull_request`
- `missing_token`
- `missing_write_permission`
- `fork_untrusted`
- `unsafe_event`
- `missing_dedupe_key`
- `missing_changed_line_placement`
- `unsupported_mode`
- `unknown`

`summary_only` guidance should be counted and visible, but never converted into
a line comment.

## Required Evidence

The publish-plan contract is proven only when the implementation can show:

- a `comment-publish-plan.json` report with schema version `0.1`;
- a Markdown sibling suitable for GitHub job summaries and uploaded artifacts;
- explicit input recording for PR guidance, existing-comment metadata, comment
  mode, PR context, repository trust context, and permission state;
- `off`, `plan`, and `inline` mode handling with `off` as the generated-CI
  default;
- operation vocabulary coverage for `create`, `update`, `keep`, `delete`,
  `skip`, and `blocked`;
- summary-only guidance recorded as skipped, never inline-publishable;
- cap behavior that defaults to three publishable comments;
- dedupe/upsert behavior keyed by `dedupe_key`;
- visible blocked states for missing input, malformed input, missing PR
  context, missing token, missing write permission, untrusted fork context,
  unsafe event context, missing dedupe key, and missing changed-line placement;
- generated-CI fixture coverage proving comments remain disabled by default;
- output schema, traceability, capability, and campaign entries that point to
  the behavior.

## JSON Shape

The plan writes:

```text
target/ripr/review/comment-publish-plan.json
target/ripr/review/comment-publish-plan.md
```

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_inline_comment_publish_plan",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-10T12:00:00Z",
  "mode": "plan",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "existing_comments": "target/ripr/review/existing-comments.json",
    "pull_request": 123,
    "event_name": "pull_request",
    "head_repo": "EffortlessMetrics/ripr",
    "base_repo": "EffortlessMetrics/ripr"
  },
  "limits": {
    "max_inline_comments": 3,
    "advisory": true,
    "comments_default": "off"
  },
  "summary": {
    "guidance_comments": 4,
    "summary_only": 1,
    "suppressed": 1,
    "publishable": 3,
    "planned_create": 2,
    "planned_update": 1,
    "planned_keep": 0,
    "planned_delete": 0,
    "skipped": 2,
    "blocked": 0,
    "safe_to_publish": false
  },
  "operations": [
    {
      "operation": "create",
      "safe_to_publish": false,
      "dry_run": true,
      "source_collection": "comments",
      "source_id": "ripr-review-67fc764ba37d77bd",
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88,
        "side": "RIGHT",
        "mode": "exact_seam_line"
      },
      "body": "RIPR advisory: related tests reach this seam but miss `amount == discount_threshold`. Add one focused equality-boundary assertion. Verify with `ripr agent verify`.",
      "existing_comment_id": null,
      "skip_reason": null,
      "blocked_reason": null
    }
  ],
  "skipped": [
    {
      "source_collection": "summary_only",
      "source_id": "ripr-review-summary-1",
      "dedupe_key": "ripr:summary:67fc764ba37d77bd",
      "skip_reason": "summary_only",
      "message": "Summary-only guidance is visible in comments.md but is not eligible for inline publishing."
    }
  ],
  "blocked": [],
  "warnings": [],
  "limits_note": "Advisory inline-comment publish plan only; default workflows do not post comments, summary-only guidance is never published inline, and gate decisions remain separate."
}
```

Field contract:

- `schema_version` is `0.1` until the plan shape changes.
- `kind` is always `pr_inline_comment_publish_plan`.
- `status` is `advisory`; this report is not gate authority.
- `mode` is `off`, `plan`, or `inline`.
- `inputs.pr_guidance` records the explicit review-comments source.
- `inputs.existing_comments` records optional existing-comment metadata when
  supplied.
- `inputs.event_name`, `inputs.head_repo`, and `inputs.base_repo` preserve the
  permission context used for plan decisions.
- `limits.max_inline_comments` defaults to three.
- `limits.comments_default` is `off`.
- `summary.guidance_comments`, `summary.summary_only`, and
  `summary.suppressed` mirror the input collections when available.
- `summary.publishable` counts `comments[]` items eligible under placement,
  cap, permission, and dedupe rules.
- `summary.safe_to_publish` is `true` only when mode is `inline` and every
  planned publishing operation satisfies the permission boundary.
- `operations[]` records candidate comment operations sourced only from
  `comments[]`.
- `operations[].source_collection` must be `comments`.
- `operations[].dedupe_key` is required for `create`, `update`, `keep`, and
  `delete`.
- `operations[].placement` is copied from the source `comments[]` item; the
  plan must not invent placement.
- `operations[].body` must preserve advisory static-evidence language and must
  not claim runtime mutation results.
- `skipped[]` records capped, summary-only, suppressed, disabled, and
  already-current items.
- `blocked[]` records hard safety blockers such as missing permissions,
  untrusted forks, missing PR context, unsafe events, missing dedupe keys, or
  malformed inputs.
- `warnings[]` records malformed optional inputs, stale existing comments,
  unsupported optional metadata, and other non-authoritative context.
- `limits_note` preserves the default-off, advisory, no-summary-only-inline,
  and separate-gate-authority boundaries.

## Markdown Shape

The Markdown sibling should be short enough for a GitHub job summary and clear
enough for review:

```md
# RIPR Inline Comment Publish Plan

Mode: plan
Status: advisory

Summary:
- publishable comments: 3
- skipped: 2
- blocked: 0
- default: inline comments are off

Planned operations:
- create src/pricing.rs:88 `ripr:67fc764ba37d77bd:src/pricing.rs:88`
  - missing discriminator: `amount == discount_threshold`
  - action: add one focused equality-boundary assertion

Skipped:
- summary_only: 1 recommendation remains in `comments.md`
- cap_reached: 1 recommendation was kept out of inline comments

Limits:
- Advisory publish plan only.
- Does not post comments unless explicit inline mode is configured.
- Never publishes summary-only guidance inline.
- Gate decision remains separate pass/fail authority.
```

When the plan cannot safely publish, the Markdown should put the reason first:

```md
# RIPR Inline Comment Publish Plan

Mode: inline
Status: advisory

Blocked:
- missing_write_permission: workflow token cannot write PR comments

Next:
- keep job summary and check annotations as the PR surface, or configure safe
  pull-request write permissions before enabling inline mode.
```

## Existing Comment Metadata

Existing-comment metadata is optional and implementation-defined until the
publisher exists, but the plan should support this stable minimal shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_inline_comment_existing_comments",
  "comments": [
    {
      "comment_id": 987654321,
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "path": "src/pricing.rs",
      "line": 88,
      "side": "RIGHT",
      "body_hash": "sha256:...",
      "outdated": false
    }
  ]
}
```

The plan may use metadata to decide `create`, `update`, `keep`, or `delete`.
It must not require live GitHub reads during read-only planning when metadata
is absent; absent metadata means the plan cannot prove update or keep
operations.

## Generated CI Projection

Generated CI may add a publish-plan step only when explicit configuration
requests it. The default generated workflow must stay:

```text
job summary + check annotations + uploaded artifacts
```

When `RIPR_COMMENT_MODE=plan`, generated CI may:

1. run the plan producer after `ripr review-comments`;
2. upload `comment-publish-plan.{json,md}` with the normal review/report
   artifact packet;
3. append the compact publish-plan summary to `$GITHUB_STEP_SUMMARY`;
4. post no comments.

When `RIPR_COMMENT_MODE=inline`, generated CI may execute safe plan operations
only after the plan is written and permission checks are visible in the plan.
Publishing must not change gate pass/fail authority. If publishing fails, the
workflow should keep the lower-level RIPR reports uploaded and should not hide
the publish-plan failure reason.

Generated CI must not add `pull_request_target` by default, publish comments
from forks without an explicit proven policy, post summary-only guidance, or
make comment publishing required for merge.

## Acceptance Examples

- `mode=off` records disabled state or skips planning; no comments are posted.
- `mode=plan` with three line-placeable `comments[]` items renders three
  planned operations and posts nothing.
- More than three `comments[]` items renders at most three publishable
  operations and records the rest as `cap_reached`.
- `summary_only[]` entries are listed as skipped with `summary_only`, not
  converted to line comments.
- A missing `dedupe_key` blocks the item instead of creating a duplicate-prone
  comment.
- Existing comment metadata with the same `dedupe_key` yields `update` or
  `keep` instead of another `create`.
- Stale existing RIPR comments are visible as stale or delete candidates.
- Same-repository `inline` mode with write permission marks safe operations
  publishable.
- Fork PR context without an explicitly proven safe policy records
  `fork_untrusted` and posts nothing.
- Missing token or missing PR write permission records a visible blocker and
  leaves check annotations, job summaries, and artifacts as the PR surface.

## Test Mapping

Follow-up fixtures and tests should cover:

- publishable changed-line comments;
- summary-only exclusion;
- cap overflow;
- missing `dedupe_key`;
- dedupe update and keep behavior;
- stale existing comments;
- disabled mode;
- plan mode;
- inline mode without token;
- inline mode without write permission;
- untrusted fork PR context;
- missing `comments.json`;
- malformed `comments.json`;
- Markdown summary for safe, skipped, blocked, and missing-input cases;
- generated workflow default-off behavior;
- generated workflow plan-mode artifact and summary behavior.

## Implementation Mapping

Follow-up implementation belongs to Campaign 26:

- `fixtures/pr-inline-comment-publisher-corpus` pins the publishable,
  summary-only, capped, dedupe/upsert, stale-existing, fork or no-token, and
  missing-input cases before producer changes.
- `report/pr-inline-comment-publish-plan` adds the read-only plan producer and
  JSON/Markdown renderers without posting to GitHub.
- `ci/pr-inline-comment-publisher` keeps generated CI default-off, adds
  plan-mode artifacts and summary projection, and posts only in explicit inline
  mode with safe permissions.
- `docs/pr-inline-comment-publisher-workflow` explains opt-in use, publish-plan
  interpretation, forks, permissions, caps, dedupe, and advisory boundaries.
- `dogfood/pr-inline-comment-publisher-receipts` records repo-local publish-plan
  receipts without posting real PR comments.
- `campaign/pr-inline-comment-publisher-closeout` records final audit,
  validation, and default-off proof.

No follow-up may change analyzer behavior, recommendation ranking, gate
semantics, LSP/editor behavior, provider behavior, source files, generated
tests, mutation execution, branch protection, `pull_request_target` defaults,
or default CI blocking as part of this campaign.

## Metrics

The plan should make these counts available to later metrics surfaces:

- `pr_inline_comment_publish_plans`;
- `pr_inline_comment_publish_plan_off`;
- `pr_inline_comment_publish_plan_plan_mode`;
- `pr_inline_comment_publish_plan_inline_mode`;
- `pr_inline_comment_publishable`;
- `pr_inline_comment_planned_create`;
- `pr_inline_comment_planned_update`;
- `pr_inline_comment_planned_keep`;
- `pr_inline_comment_planned_delete`;
- `pr_inline_comment_skipped`;
- `pr_inline_comment_blocked`;
- `pr_inline_comment_summary_only_skipped`;
- `pr_inline_comment_cap_reached`;
- `pr_inline_comment_dedupe_matches`;
- `pr_inline_comment_permission_blocked`;
- `pr_inline_comment_fork_blocked`.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No LSP or editor behavior changes.
- No generated workflow changes in this spec PR.
- No default inline comments.
- No default CI blocking.
- No `pull_request_target` default.
- No source edits.
- No generated tests.
- No provider or API calls from the plan producer.
- No mutation execution.
- No implicit source inspection or hidden analysis reruns.
- No free-form LLM review comments.
- No broad GitHub review bot behavior.

## Validation

The spec PR should run:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```
