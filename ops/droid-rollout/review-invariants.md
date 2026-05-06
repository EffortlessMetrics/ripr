# Review Invariants — TEMPLATE

> Copy this file into the target repository's agent-context directory and
> tailor the **General** and **Workflow invariants** sections for the target
> repo's domain. The Droid review and security-scan invariants below are
> load-bearing — preserve them verbatim.

## General

A finding is useful if it identifies a concrete failure mode another agent
can fix.

Do not suppress concrete findings because there are many of them. Suppress
only duplicates, speculation, or non-actionable comments.

## Review output invariants

Each actionable finding emitted by Droid must include:

* failure mode;
* repo invariant, policy, or edge case violated;
* fix direction (name likely files/functions when useful);
* validation (command, report, fixture, golden, or CI check);
* confidence (High / Medium / Low with justification when not high).

Review output should not optimize for short comments at the expense of repair
value. Droid runs consume CI time, model calls, and repo research; each
finding should amortize that cost by preserving useful research context in
the comment or summary.

Do not discard useful repo research. If Droid inspected specs, policies, CI
configuration, prior comments, or in-repo documentation, preserve the
relevant result so the next repair agent does not rediscover the same
invariant.

## Notification invariants

Automated review output should avoid unnecessary notifications.

* Droid-generated review bodies must not @mention humans, teams, bots, or
  organizations unless explicitly requested.
* Review comments should be addressed to the next repair agent, not to the
  PR author.
* Prefer PR-scoped language: `this PR`, `this diff`, `the changed code`.
* Treat platform-generated wrapper mentions as outside repo guidance; do not
  repeat them in review content.

## Workflow invariants

For GitHub Actions:

- Workflows using secrets must not run on untrusted fork PR code.
- Do not use `pull_request_target` unless the workflow is deliberately
  designed for it.
- Keep `permissions:` minimal and explicit.
- Workflows that call third-party actions with secrets or write permissions
  should use pinned refs.
- Do not print secrets or generated local settings containing expanded
  secrets.
- Keep full Droid output disabled unless debugging in a safe private context.

## Droid review invariants

For Droid review workflows (`droid-review.yml` and `droid.yml`):

- Use the MiniMax BYOK model path unless intentionally changing provider.
- Model should be `custom:MiniMax-M2.7-0`.
- Runtime BYOK settings should be written to `~/.factory/settings.local.json`.
- Do not rely on the Droid Action `settings:` input for BYOK custom models.
- Keep `${MINIMAX_API_KEY}` literal in checked-in or artifact-prone files.
- Do not set `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL`.
- Keep `show_full_output: false`.
- `automatic_review: true` and `automatic_security_review: true` must be set
  on the auto-review workflow.
- `review_depth: shallow` unless intentionally changed.
- `cancel-in-progress: false` with per-PR concurrency group.
- `pull_request` types must include `opened`, `synchronize`,
  `ready_for_review`, `reopened`.
- Same-repo guard (`head.repo.full_name == github.repository`) is required on
  the auto-review workflow.
- Draft PRs must not be filtered out.
- `MINIMAX_API_KEY` must be job-level env referencing
  `${{ secrets.MINIMAX_API_KEY }}`.
- Action refs must be immutable 40-character commit SHAs.
- The manual workflow (`droid.yml`) must have trusted actor guards (`OWNER`,
  `MEMBER`, `COLLABORATOR`).

## Droid security scan invariants

For `droid-security-scan.yml`:

- Must support `workflow_dispatch`.
- Must run on the documented weekly schedule (`schedule:` + `cron:`) unless
  intentionally changed.
- Must use repo-level concurrency group
  `droid-security-scan-${{ github.repository }}` with
  `cancel-in-progress: false`.
- Must run on `ubuntu-latest`.
- Must use the MiniMax BYOK model path (runtime
  `~/.factory/settings.local.json`, literal `${MINIMAX_API_KEY}`).
- Must set `security_model: "custom:MiniMax-M2.7-0"`.
- Must set `security_scan_schedule: true`.
- Must set `security_scan_days: 7`.
- Must set `security_severity_threshold: medium`.
- Must set `security_block_on_critical: true`.
- Must set `security_block_on_high: false`.
- Must keep `show_full_output: false`.
- Must not use the Droid Action `settings:` input for BYOK.
- Must not set `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL`.
- Must keep action refs pinned to immutable 40-character SHAs.

## Queueing invariants

For automatic Droid PR review:

- Run on same-repo PRs and every commit.
- Draft PRs are reviewable.
- Do not cancel an active Droid review.
- Keep at most the latest queued review per PR.
- Allow separate PRs to run concurrently.
