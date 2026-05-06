# Droid Rollout Guide

Use this guide when copying `ripr`'s Factory Droid setup to another repository.
The `ripr` workflows are the rollout template, but each target repo still needs
repo-scoped secrets, repo-aware instructions, policy integration, and a small
pilot before broad enablement.

## Rollout posture

The remaining work before broader rollout is rollout discipline, not bot repair.
Keep these defaults for the first wave:

- use MiniMax M2.7 through Factory BYOK;
- keep automatic review, manual `@droid`, and scheduled/manual security scan as
  separate workflows;
- keep Droid action refs pinned to immutable SHAs;
- keep `show_full_output: false` explicit;
- keep `review_depth: shallow` until deeper review is tested in `ripr`;
- keep review output agent-oriented rather than LGTM-oriented.

## Per-repo prerequisites

Confirm these before merging a rollout PR:

```text
Repo:
Default branch:
Factory App installed:
Actions enabled:
FACTORY_API_KEY scoped:
MINIMAX_API_KEY scoped:
Existing Droid workflows:
Existing workflow allowlist:
Existing AGENTS.md:
Existing repo validation commands:
Public/fork PR posture:
Security/release sensitivity:
```

Required secrets:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Prefer org-level secrets scoped only to selected pilot repositories. Do not
expose secrets to the whole org unless every repo is intended to participate.
The MiniMax key should be the Token Plan key.

## Files to copy or adapt

Most rollout repos should receive these workflow files:

```text
.github/workflows/droid-review.yml
.github/workflows/droid.yml
.github/workflows/droid-security-scan.yml
```

Most rollout repos should also receive lightweight repo guidance:

```text
.factory/skills/review-guidelines/SKILL.md
.factory/rules/droid-review.md
docs/agent-context/review-invariants.md
docs/agent-context/droid-smoke-tests.md
AGENTS.md
```

Small repos do not need the full `ripr` guidance. Preserve the key behavior:

- Droid comments are a repair queue for follow-up agents;
- no naked LGTM;
- no arbitrary comment cap;
- findings include failure mode, why here, fix direction, validation, and
  confidence;
- clean reviews produce an inspection record;
- Droid-generated bodies do not add extra `@mentions`.

## Required workflow shape

### MiniMax BYOK bridge

Write Factory settings at runtime with a quoted heredoc:

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

The quoted heredoc is intentional: it keeps `${MINIMAX_API_KEY}` literal in the
settings file instead of writing the expanded secret into artifact-prone local
Factory state.

Do not set:

```text
ANTHROPIC_AUTH_TOKEN
ANTHROPIC_BASE_URL
reasoning_effort
```

Do not use the Factory Action `settings:` input for this MiniMax setup unless
Factory changes the custom-model loading behavior and `ripr` updates its
invariants.

### Standard model settings

Use these values in the first rollout wave:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
show_full_output: false
```

### Automatic PR review

Use `pull_request`, not `pull_request_target`, and keep the same-repo guard so
secrets-backed jobs do not run on untrusted fork code:

```yaml
on:
  pull_request:
    types: [opened, synchronize, ready_for_review, reopened]

concurrency:
  group: droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}
  cancel-in-progress: false
```

Keep `[skip-review]` as a PR title escape hatch. Draft PRs are intentionally
reviewed because Droid is used for early design and rewrite feedback.

### Manual `@droid`

Keep the trusted actor guard for manual commands. Manual commands should run
only for these author associations:

```text
OWNER
MEMBER
COLLABORATOR
```

Do not simplify this away; the guard prevents untrusted comments from starting
secrets-backed jobs.

### Scheduled security scan

Use manual dispatch plus a weekly schedule only after the first manual run is
successful in the target repo:

```yaml
on:
  workflow_dispatch:
  schedule:
    - cron: "0 8 * * 1"

concurrency:
  group: droid-security-scan-${{ github.repository }}
  cancel-in-progress: false
```

Use a medium severity threshold for scheduled scans:

```yaml
security_scan_schedule: true
security_scan_days: 7
security_severity_threshold: medium
security_block_on_critical: true
security_block_on_high: false
```

## Policy integration

If the target repo has `policy/workflow_allowlist.txt`, add each Droid workflow
because the BYOK setup uses shell `run:` blocks. `ripr` budgets each Droid
workflow at 20 non-empty shell lines. Do not invent a workflow allowlist in repos
that do not already use that policy unless the repo explicitly wants that
governance.

## Rollout PR body checklist

Include this in each rollout PR body:

```markdown
## Required repo/org secrets

This workflow requires:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Both must be available to this repository before merge.
```

Also call out:

- fork PRs are intentionally skipped;
- draft PRs are intentionally reviewed;
- `[skip-review]` opts out of automatic review;
- `@droid review` and `@droid security` require a trusted actor;
- `show_full_output: false` limits debug exposure but does not make artifacts
  secret-free.

## Pilot plan

1. Select 3 to 5 low-risk repositories that already use GitHub Actions, have
   enough PR traffic to test, are not release-critical this week, and do not
   have unusual fork workflows.
2. Merge one rollout PR per repo after secrets and app installation are ready.
3. Open or reuse one same-repo PR and confirm:
   - Droid Auto Review starts;
   - Droid initializes with `custom:MiniMax-M2.7-0`;
   - review output follows the inspection/repair format;
   - Droid-generated body text does not add extra `@mentions`;
   - `@droid review` works;
   - `@droid security` works;
   - Droid Security Scan `workflow_dispatch` works.
4. For one pilot repo, download a Droid debug artifact and confirm:
   - `settings.local.json` does not contain expanded secrets;
   - prompt/debug context does not contain unexpected secrets;
   - artifact retention and access are acceptable;
   - MiniMax usage is visible and expected in the usage dashboard.
5. After the pilot is boring, roll out in batches of 10 to 20 repos instead of a
   single org-wide change set.

## Do not roll out yet

Do not roll out these changes broadly until they are separately tested in
`ripr`:

- `review_depth: deep`;
- VPS or self-hosted runners;
- `pull_request_target`;
- fork PRs running secrets-backed Droid jobs;
- comment post-processing to remove Factory wrapper mentions;
- global permission reductions such as changing automatic review from
  `contents: write` to `contents: read`.
