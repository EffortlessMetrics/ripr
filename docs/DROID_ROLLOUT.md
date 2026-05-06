# Droid Rollout Guide

This guide records the rollout discipline for reusing `ripr`'s Factory Droid
setup in other repositories. The `ripr` workflows are the source template; this
page is the pre-merge and pilot checklist for copying that shape safely.

## Baseline posture

`ripr` uses Factory Droid with MiniMax M2.7 BYOK for three lanes:

- automatic same-repo pull request review;
- trusted manual `@droid` commands;
- manual and scheduled security scanning.

The rollout goal is not to make every repository identical. The goal is to keep
the security and review-output invariants stable while adding repo-specific
agent context.

## Required repository setup

Before merging Droid workflows into a target repository, confirm:

```text
Factory Droid GitHub App installed:
Actions enabled:
FACTORY_API_KEY scoped to this repo:
MINIMAX_API_KEY scoped to this repo:
```

Prefer org-level secrets scoped to selected repositories. Do not expose Droid
secrets to the whole organization unless every repository is intended to run the
workflows.

The workflows expect:

```yaml
env:
  MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}
```

and Droid action inputs of:

```yaml
factory_api_key: ${{ secrets.FACTORY_API_KEY }}
```

Use the MiniMax Token Plan key for `MINIMAX_API_KEY`.

## Files to copy

Most rollout repositories should receive these workflows:

```text
.github/workflows/droid-review.yml
.github/workflows/droid.yml
.github/workflows/droid-security-scan.yml
```

They should also receive lightweight repo-specific guidance, either copied from
`ripr` and shortened or written directly for the target repository:

```text
.factory/skills/review-guidelines/SKILL.md
.factory/rules/droid-review.md
docs/agent-context/review-invariants.md
docs/agent-context/droid-smoke-tests.md
AGENTS.md
```

Small repositories do not need `ripr`'s full product context. They do need to
preserve these review-output rules:

- Droid comments are a repair queue for follow-up agents.
- Do not emit naked `LGTM` or empty approval language.
- Do not set an arbitrary low comment cap.
- Findings name failure mode, why the repo invariant matters here, fix
  direction, validation, and confidence.
- Clean reviews still produce an inspection record.
- Droid-generated review bodies do not add extra `@mentions`.

## Workflow invariants to preserve

Automatic pull request review must keep:

```yaml
on:
  pull_request:
    types: [opened, synchronize, ready_for_review, reopened]

concurrency:
  group: droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}
  cancel-in-progress: false
```

It must also keep the same-repo guard:

```yaml
github.event.pull_request.head.repo.full_name == github.repository
```

Manual `@droid` workflows must keep trusted actor guards for:

```text
OWNER
MEMBER
COLLABORATOR
```

Do not use `pull_request_target` for this rollout. Do not run secrets-backed
Droid jobs on fork PR code.

## Standard model and BYOK bridge

Use MiniMax M2.7 with shallow review depth for the rollout baseline:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
```

Do not roll out `review_depth: deep` organization-wide until it has been tested
in `ripr` with the custom model path.

The working BYOK bridge writes Factory local settings at runtime:

```bash
mkdir -p "$HOME/.factory"
cat > "$HOME/.factory/settings.local.json" <<'JSON'
{
  "customModels": [
    {
      "displayName": "MiniMax-M2.7",
      "model": "MiniMax-M2.7",
      "baseUrl": "https://api.minimax.io/anthropic",
      "apiKey": "${MINIMAX_API_KEY}",
      "provider": "anthropic",
      "maxOutputTokens": 64000,
      "noImageSupport": true,
      "extraArgs": {
        "temperature": 1
      }
    }
  ]
}
JSON
```

The quoted heredoc is intentional. It keeps `${MINIMAX_API_KEY}` literal in the
settings file so debug artifacts do not contain the expanded secret.

Do not use the Droid Action `settings:` input for this MiniMax setup. Do not set
`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, or `reasoning_effort`.

## Action refs and output hygiene

Use the pinned action refs from `ripr` until there is a separate update process:

```yaml
actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd # v5
Factory-AI/droid-action@e3d1f5e7861c36fe4a9c4dca3edec87b964b2bc4 # v5
```

Keep this explicit on every Droid action step:

```yaml
show_full_output: false
```

Pinned SHAs are intentionally static. Updating them should be a deliberate
maintenance change, not an implicit tag movement.

## Policy integration

If the target repository has a workflow allowlist or shell-line budget policy,
add every Droid workflow to that policy because the BYOK bridge uses `run:`
blocks. In `ripr`, each Droid workflow has a `20` non-empty shell-line budget.

Do not invent a workflow allowlist solely for Droid in repositories that do not
already use that governance model.

## Pilot checklist

Use this inventory before patching each repository:

```text
Repo:
Default branch:
Factory App installed:
FACTORY_API_KEY scoped:
MINIMAX_API_KEY scoped:
Existing Droid workflows:
Existing workflow allowlist:
Existing AGENTS.md:
Existing repo validation commands:
Public/fork PR posture:
Security/release sensitivity:
```

After patching, verify:

```text
same-repo guard
trusted actor guard
show_full_output: false
quoted heredoc
literal ${MINIMAX_API_KEY}
no ANTHROPIC_* vars
custom:MiniMax-M2.7-0
action refs pinned to SHA
review_depth: shallow
security threshold high for PR review
security threshold medium for scheduled scan
workflow allowlist updated if applicable
```

Roll out first to three to five low-risk repositories that already use GitHub
Actions, have enough pull request traffic to validate the setup, and are not in
a release-critical window.

## Post-merge smoke tests

For each pilot repository:

1. Open or reuse a same-repo PR and confirm Droid Auto Review starts.
2. Confirm the run initializes with `custom:MiniMax-M2.7-0`.
3. Confirm review output follows the inspection and repair-queue format.
4. Confirm Droid-generated bodies do not add extra `@mentions`.
5. Comment `@droid review` and confirm the trusted manual lane runs.
6. Comment `@droid security` and confirm the manual security lane runs.
7. Run the Droid Security Scan workflow manually once.

For one pilot repository, download a Droid debug artifact and confirm:

- `settings.local.json` does not contain expanded secrets;
- prompt and debug artifacts do not contain unexpected secrets;
- artifact retention and download permissions are acceptable;
- MiniMax usage is visible in the expected usage dashboard.

## Rollout PR body checklist

Include this section in every rollout PR body:

```markdown
## Required repo/org secrets

This workflow requires:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Both must be available to this repository before merge.
```

Also state that fork PRs are intentionally skipped, draft PRs are intentionally
reviewed, and `[skip-review]` in the PR title opts out of automatic review.

## Deferred follow-ups

These are useful follow-ups, but they should not block the first pilot batch:

- Test in `ripr` whether automatic review can use `contents: read` instead of
  `contents: write`.
- Track upstream support for suppressing Factory wrapper `@mentions`.
- Test `review_depth: deep` with MiniMax M2.7 after shallow rollout is stable.
- Create a pinned-action update process.

Do not reduce permissions globally, move to self-hosted runners, or add comment
post-processing until each behavior has been tested in `ripr` first.
