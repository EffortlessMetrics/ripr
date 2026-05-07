# Roll Out Factory Droid Review

Use this guide when copying `ripr`'s Factory Droid setup to another repository.
The goal is rollout discipline, not new bot behavior: keep the working MiniMax
BYOK bridge, preserve the trust boundaries, add repo-specific review context,
and pilot the setup before broad enablement.

## Rollout Posture

`ripr` is the source template for this setup. Carry forward these defaults for
the first rollout wave:

- MiniMax M2.7 runs through Factory Droid BYOK.
- Automatic PR review, trusted manual `@droid`, and scheduled security scan are
  separate workflows.
- Droid workflows use SHA-pinned third-party actions.
- `show_full_output: false` is explicit in every Droid action step.
- `upload_debug_artifacts: false` is explicit in every Droid action step.
- The approved safe action ref disables raw `$HOME/.factory/**` and
  `droid-prompts/**` artifact upload for normal secrets-backed runs.
- Automatic review is restricted to same-repo PRs so secrets do not run on fork
  code.
- Manual `@droid` commands require a trusted actor: `OWNER`, `MEMBER`, or
  `COLLABORATOR`.
- Droid review comments are repair queues for follow-up agents, not empty
  approval signals.

Do not roll out `review_depth: deep`, `pull_request_target`, self-hosted
runners, fork-secret execution, wrapper-comment post-processing, or global
permission reductions until they have been tested in `ripr`.

Keep the current `ripr` permission shape during baseline rollout:

```yaml
# .github/workflows/droid-review.yml
contents: write
pull-requests: write
issues: write
id-token: write
actions: read

# .github/workflows/droid.yml
contents: read
pull-requests: write
issues: write
id-token: write
actions: read

# .github/workflows/droid-security-scan.yml
contents: write
pull-requests: write
issues: write
id-token: write
actions: read
```

Do not trim `droid-review.yml` to `contents: read` unless a dedicated
permission-test PR proves the Factory Action works with that narrower shape and
is ready to revert. Factory's automated PR review/fix flow may rely on
repository write capability.

## Per-Repo Prerequisites

Confirm these before merging a rollout PR in a target repository:

```text
Repo:
Default branch:
Factory Droid GitHub App installed:
Actions enabled:
FACTORY_API_KEY scoped to this repo:
MINIMAX_API_KEY scoped to this repo:
Existing Droid workflows:
Existing workflow allowlist:
Existing AGENTS.md or repo instructions:
Existing validation commands:
Public/fork PR posture:
Security/release sensitivity:
```

Each participating repository needs both secrets available to GitHub Actions:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Prefer organization secrets scoped to selected pilot repositories. Do not expose
these secrets org-wide unless every repository is intended to participate. The
MiniMax key should be the Token Plan key used by the working `ripr` BYOK setup.

## Files To Copy Or Adapt

Most repositories should receive these workflow lanes:

```text
.github/workflows/droid-review.yml
.github/workflows/droid.yml
.github/workflows/droid-security-scan.yml
```

They should also receive lightweight, repo-aware review guidance. At minimum,
copy or adapt:

```text
.factory/skills/review-guidelines/SKILL.md
.factory/rules/droid-review.md
docs/agent-context/review-invariants.md
docs/agent-context/droid-smoke-tests.md
AGENTS.md
```

Small repositories can use shorter guidance than `ripr`, but preserve these
behaviors:

- Droid comments are a repair queue for agents.
- Do not emit naked `LGTM` or empty approval language.
- Do not impose an arbitrary comment cap.
- Findings include failure mode, why here, fix direction, validation, and
  confidence.
- Clean reviews include an inspection record.
- Droid-generated review bodies do not add extra human, team, bot, or org
  mentions.

## MiniMax BYOK Bridge

Use the runtime Factory settings file, not the Droid Action `settings:` input,
for this MiniMax custom model bridge.

The heredoc delimiter must stay quoted so the API key reference remains literal
in `settings.local.json`:

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

Do not set:

```text
ANTHROPIC_AUTH_TOKEN
ANTHROPIC_BASE_URL
reasoning_effort
```

## Workflow Invariants

Use these pinned action refs until there is a deliberate update process:

```yaml
actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd # v5
EffortlessMetrics/droid-action-safe@7c1377ccbacddc95560d1570547a5baa51de01ec # based on Factory-AI/droid-action v5; raw debug artifact upload disabled
```

Do not use `Factory-AI/droid-action` directly for MiniMax BYOK workflows until
upstream exposes a debug-artifact disable input and `ripr`'s checker allowlist
is updated.

Start with this model baseline:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
show_full_output: false
upload_debug_artifacts: false
```

For automatic PR review, keep:

```yaml
on:
  pull_request:
    types: [opened, synchronize, ready_for_review, reopened]

concurrency:
  group: droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}
  cancel-in-progress: false
```

The automatic review job must keep this same-repo guard:

```yaml
github.event.pull_request.head.repo.full_name == github.repository
```

Keep `[skip-review]` as the PR title escape hatch. Draft PRs are intentionally
reviewed, every commit is reviewed, active reviews are not canceled, stale
queued reviews are deduped per PR, and separate PRs can run concurrently.

For manual `@droid`, keep trusted actor guards for `OWNER`, `MEMBER`, and
`COLLABORATOR`. Do not simplify this away; it prevents untrusted comments from
starting secrets-backed jobs.

For scheduled security scan, use manual dispatch plus a weekly schedule:

```yaml
on:
  workflow_dispatch:
  schedule:
    - cron: "0 8 * * 1"

concurrency:
  group: droid-security-scan-${{ github.repository }}
  cancel-in-progress: false
```

Preserve the medium-threshold scheduled scan baseline:

```yaml
security_scan_schedule: true
security_scan_days: 7
security_severity_threshold: medium
security_block_on_critical: true
security_block_on_high: false
```

## Policy Integration

If the target repository has `policy/workflow_allowlist.txt` or a similar
workflow shell-budget policy, add each Droid workflow because the BYOK bridge
uses shell `run:` blocks. `ripr` budgets each Droid workflow at 20 non-empty
shell lines.

If the target repository does not already have workflow policy governance, do
not add a new policy surface only for Droid unless that repository wants the
extra governance.

## Rollout PR Body Checklist

Include this section in each target-repository rollout PR:

```markdown
## Required repo/org secrets

This workflow requires:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Both must be available to this repository before merge.
```

Also state:

- fork PRs are intentionally skipped for secrets-backed Droid review;
- draft PRs are intentionally reviewed;
- `[skip-review]` opts out of automatic review;
- `@droid review` and `@droid security` require a trusted actor;
- `show_full_output: false` controls logs only;
- `upload_debug_artifacts: false` prevents raw Droid debug artifact upload.

## Pilot And Smoke Tests

Start with three to five low-risk repositories that already use GitHub Actions,
have enough PR traffic to test the setup, do not have unusual fork workflows,
and are not release-critical that week.

After merging each pilot rollout:

1. Open or reuse one same-repo PR.
2. Confirm Droid Auto Review starts.
3. Confirm Droid initializes with `custom:MiniMax-M2.7-0`.
4. Confirm review output follows the inspection and repair-queue format.
5. Confirm Droid-generated body text does not add extra mentions.
6. Comment `@droid review` as a trusted actor and confirm it runs.
7. Comment `@droid security` as a trusted actor and confirm it runs.
8. Run Droid Security Scan manually once before relying on the schedule.

Before broad rollout, inspect one successful Droid run's artifact list from
`ripr` or a pilot repository and confirm:

- no raw artifact named `droid-review-debug-<run_id>` was uploaded;
- if sanitized debug artifacts were explicitly enabled in a private manual run,
  the artifact name is `droid-review-debug-sanitized-*`, retention is 1 day,
  and it contains no expanded provider keys, GitHub tokens, authorization
  headers, or raw prompt files;
- MiniMax usage is visible and expected in the provider dashboard.

After the pilot is uneventful, roll out in batches of 10 to 20 repositories.
Avoid one large organization-wide change set; most failures are repo-specific
missing secrets, branch protection, workflow policy, or permission mismatches.
