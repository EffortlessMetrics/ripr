# Review Invariants

Use these invariants during automated PR review.

## General

A finding is useful if it identifies a concrete failure mode another agent can fix.

Do not suppress concrete findings because there are many of them. Suppress only duplicates, speculation, or non-actionable comments.

## Workflow invariants

For GitHub Actions:

- Workflows using secrets must not run on untrusted fork PR code.
- Do not use `pull_request_target` unless the workflow is deliberately designed for it.
- Keep `permissions:` minimal and explicit.
- Workflows that call third-party actions with secrets or write permissions should use pinned refs.
- New workflows must be represented in `policy/workflow_allowlist.txt`.
- If a workflow adds shell `run:` blocks, the workflow allowlist budget must reflect them.
- Do not print secrets or generated local settings containing expanded secrets.
- Keep full Droid output disabled unless debugging in a safe private context.

## Droid review invariants

For Droid review workflows:

- Use the MiniMax BYOK model path unless intentionally changing provider.
- Model should be `custom:MiniMax-M2.7-0`.
- Runtime BYOK settings should be written to `~/.factory/settings.local.json`.
- Do not rely on the Droid Action `settings:` input for BYOK custom models unless Factory fixes the path mismatch.
- Keep `${MINIMAX_API_KEY}` literal in checked-in or artifact-prone files.
- Do not set `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL`.
- Keep `show_full_output: false`.

## Queueing invariants

For automatic Droid PR review:

- Run on same-repo PRs and every commit.
- Draft PRs are reviewable.
- Do not cancel an active Droid review.
- Keep at most the latest queued review per PR.
- Allow separate PRs to run concurrently.
