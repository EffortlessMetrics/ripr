# PR Inline Comment Publisher Workflow

Use inline RIPR comments only when the PR summary, check annotations, and
report-packet artifacts are not visible enough for your review process.

The generated workflow keeps inline comments disabled by default. Inline
comments are an explicit projection over the existing PR guidance artifact:

```text
target/ripr/review/comments.json
```

They do not rerun analysis, change recommendation ranking, decide gate policy,
edit source, generate tests, call providers, run mutation testing, or make CI
blocking by default.

## Choose A Mode

Configure the generated workflow with `RIPR_COMMENT_MODE`:

```text
off
plan
inline
```

`off` is the default. The workflow does not produce a publish plan and does not
call the GitHub comments API.

`plan` is the safe rollout mode. The workflow writes
`target/ripr/review/comment-publish-plan.{json,md}` and adds the Markdown plan
to the job summary. It still posts nothing.

`inline` writes the same plan, then publishes only create or update operations
that the plan marks as safe. The generated publisher keeps deletion and other
stale cleanup operations review-only.

## Roll Out Safely

Start with `plan`:

```yaml
env:
  RIPR_COMMENT_MODE: plan
```

Review several pull requests before enabling `inline`. Check that the plan is
quiet, line placement is correct, summary-only guidance stays out of inline
comments, and duplicate RIPR comments are planned as `keep` or `update` instead
of new threads.

Move to `inline` only after the plan output is boring:

```yaml
env:
  RIPR_COMMENT_MODE: inline
```

Keep `RIPR_COMMENT_MODE=off` for repositories that already have enough
visibility from job summaries, check annotations, SARIF, and uploaded artifacts.

## Read The Publish Plan

Open the job summary section named `PR inline comments`, or inspect:

```text
target/ripr/review/comment-publish-plan.md
target/ripr/review/comment-publish-plan.json
```

The plan answers:

- mode: `off`, `plan`, or `inline`;
- status: whether publishing is disabled, planned, publishable, blocked, or
  warning-only;
- safe to publish: whether any GitHub write should happen;
- counts: publishable, skipped, and blocked items;
- operations: `create`, `update`, `keep`, or `delete`;
- skipped items: useful guidance that should not become a durable inline
  comment;
- blocked items: missing input, missing token, missing write permission,
  untrusted fork, unsafe event, unsupported mode, or unsafe placement.

Use `comment-publish-plan.md` for review. Use the JSON only when debugging the
workflow or feeding another controlled publisher.

## What Can Publish

The publisher can post only from `comments[]` in `comments.json`.

It must never post:

- `summary_only[]` guidance;
- suppressed recommendations;
- recommendations beyond the cap;
- recommendations without a `dedupe_key`;
- recommendations without changed-line placement;
- recommendations from unsafe events or untrusted fork contexts.

The default cap is three publishable comments. If more recommendations exist,
the overflow remains visible in the plan and PR summary instead of becoming
review-thread noise.

## Dedupe And Upsert

Generated CI captures existing RIPR review comments and recognizes them by a
hidden marker:

```text
<!-- ripr:dedupe=... -->
```

The plan uses the matching `dedupe_key` to decide whether to:

- `create` a new RIPR comment;
- `update` an existing RIPR comment;
- `keep` an already-current RIPR comment;
- record stale comments as review-only `delete` operations.

The generated publisher executes safe `create` and `update` operations only.
It does not delete stale comments automatically.

## Forks And Permissions

The generated workflow does not use `pull_request_target`.

Inline publishing is treated as safe only for same-repository pull requests with
a token and pull-request write permission. Fork pull requests, missing tokens,
missing write permission, non-PR events, and unsupported modes produce visible
blocked plan entries instead of posting through another surface.

The generated workflow requests `pull-requests: write` so explicit same-repo
`inline` mode can update review comments. When `RIPR_COMMENT_MODE=off`, the
publisher steps do not run. When `RIPR_COMMENT_MODE=plan`, the workflow renders
the plan but still posts nothing.

## Advisory Boundary

Inline comments are visibility, not policy.

Gate decisions remain separate:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

Do not treat a posted comment as a failure, a waiver, a suppression, or a
mergeability decision. A comment should point to one focused test-oracle gap,
the missing discriminator, the suggested focused test, and the agent or verify
command when available.

## Reviewer Workflow

1. Read the `RIPR PR Review` or advisory summary first.
2. Read `PR inline comments` in the job summary to see whether comments were
   disabled, planned, posted, or blocked.
3. If mode is `plan`, inspect `comment-publish-plan.md` before changing the
   repository setting to `inline`.
4. If mode is `inline`, review RIPR comments as advisory pointers to the same
   evidence in `comments.md` and the report-packet index.
5. Ask for one focused test or use the embedded agent handoff command.
6. Verify movement with the normal RIPR before/after receipt flow.

## When To Stay Off

Keep inline comments off when:

- the job summary already gives reviewers enough signal;
- most guidance is summary-only;
- recommendations are still noisy;
- the repo receives many fork pull requests;
- review threads are already overloaded;
- teams have not reviewed `plan` mode on representative PRs.

Use `plan` when you want visibility into what would post. Use `inline` only
when durable review-thread visibility is worth the noise budget.

## Roll Back

Set the mode back to `off`:

```yaml
env:
  RIPR_COMMENT_MODE: off
```

Existing comments remain in prior PR threads, but future workflow runs will not
create or update RIPR inline comments. Gate decisions, summaries, annotations,
SARIF, and uploaded artifacts continue to work through their existing advisory
surfaces.
