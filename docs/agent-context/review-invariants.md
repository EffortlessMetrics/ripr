# Review Invariants

Use these invariants during automated PR review.

## General

A finding is useful if it identifies a concrete failure mode another agent can fix.

Do not suppress concrete findings because there are many of them. Suppress only duplicates, speculation, or non-actionable comments.

## Review output invariants

Each actionable finding emitted by Droid must include:

* failure mode;
* repo invariant, policy, or edge case violated;
* fix direction (name likely files/functions when useful);
* validation (command, report, fixture, golden, or CI check);
* confidence (High / Medium / Low with justification when not high).

Review output should not optimize for short comments at the expense of repair value. Droid runs consume CI time, model calls, and repo research; each finding should amortize that cost by preserving useful research context in the comment or summary.

Do not discard useful repo research. If Droid inspected specs, policies, CI configuration, prior comments, or in-repo documentation, preserve the relevant result so the next repair agent does not rediscover the same invariant.

## Notification invariants

Automated review output should avoid unnecessary notifications.

* Droid-generated review bodies must not @mention humans, teams, bots, or organizations unless explicitly requested.
* Review comments should be addressed to the next repair agent, not to the PR author.
* Prefer PR-scoped language: `this PR`, `this diff`, `the changed code`.
* Treat platform-generated wrapper mentions as outside repo guidance; do not repeat them in review content.

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
- `automatic_review: true` and `automatic_security_review: true` must be set.
- `review_depth: shallow` unless intentionally changed.
- `cancel-in-progress: false` with per-PR concurrency group.
- `pull_request` types must include `opened`, `synchronize`, `ready_for_review`, `reopened`.
- Same-repo guard (`head.repo.full_name == github.repository`) is required.
- Draft PRs must not be filtered out.
- `MINIMAX_API_KEY` must be job-level env referencing `${{ secrets.MINIMAX_API_KEY }}`.
- Action refs must be immutable 40-character commit SHAs.
- The manual workflow (`droid.yml`) must have trusted actor guards (`OWNER`, `MEMBER`, `COLLABORATOR`).
- These invariants are enforced by `cargo xtask check-droid-review-config`.

## Queueing invariants

For automatic Droid PR review:

- Run on same-repo PRs and every commit.
- Draft PRs are reviewable.
- Do not cancel an active Droid review.
- Keep at most the latest queued review per PR.
- Allow separate PRs to run concurrently.
