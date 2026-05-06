# Roll Out Factory Droid Review

Use this guide when copying `ripr`'s Factory Droid review setup to another
repository. The goal is rollout discipline, not new bot behavior: keep the
working `ripr` wiring intact, add repo-specific instructions, and pilot the
setup before broad deployment.

## Rollout posture

`ripr` is the template repository for this setup. Carry forward these baseline
properties:

- MiniMax M2.7 runs through Factory Droid BYOK.
- Automatic PR review, trusted manual `@droid`, and scheduled security scan are
  separate workflows.
- Droid workflows use SHA-pinned third-party actions.
- `show_full_output: false` is explicit in every Droid action step.
- Automatic review is restricted to same-repo PRs so secrets are not exposed to
  fork code.
- Manual `@droid` commands require a trusted actor (`OWNER`, `MEMBER`, or
  `COLLABORATOR`).
- Droid review comments are repair queues for follow-up agents, not a naked
  `LGTM` signal.

Do not roll out `review_depth: deep`, `pull_request_target`, self-hosted runner
changes, fork-secret execution, wrapper-comment post-processing, or global
permission reductions until they have been tested in `ripr`.

## Per-repo prerequisites

Confirm these before merging a rollout PR in a target repository:

```text
Repo:
Default branch:
Factory App installed:
FACTORY_API_KEY scoped:
MINIMAX_API_KEY scoped:
Actions enabled:
Existing Droid workflows:
Existing workflow allowlist:
Existing AGENTS.md:
Existing repo validation commands:
Public/fork PR posture:
Security/release sensitivity:
```

Each participating repository needs both secrets available to GitHub Actions:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Prefer org-level secrets scoped to selected repositories. Do not expose these
secrets org-wide unless every repository is intended to participate. The
MiniMax key should be the Token Plan key.

Rollout PRs should include this reminder:

```markdown
## Required repo/org secrets

This workflow requires:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Both must be available to this repository before merge.
```

## Files to copy or adapt

Most repositories should receive these workflow files:

```text
.github/workflows/droid-review.yml
.github/workflows/droid.yml
.github/workflows/droid-security-scan.yml
```

They should also receive lightweight, repo-aware review guidance. At minimum,
carry over or adapt:

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
- Do not emit naked `LGTM` reviews.
- Do not impose an arbitrary comment cap; suppress only duplicates,
  speculation, and non-actionable notes.
- Findings include failure mode, why the changed code violates a repo invariant,
  fix direction, validation, and confidence.
- Clean reviews include an inspection record: changed surfaces, checks
  performed, why no comments were left, residual risk, and validation signal.
- Droid-generated review bodies do not add human, team, bot, or org mentions.

Add a target-repo context section such as:

```markdown
## Droid review focus

- Main product/runtime surfaces
- Security-sensitive files
- Release/publish workflows
- Generated files
- Validation commands
- Known policy files
```

## Required BYOK bridge

Use the checked-in workflow pattern that writes runtime Factory settings to
`$HOME/.factory/settings.local.json`. Do not use the Factory Action `settings:`
input for this MiniMax setup.

The heredoc must stay quoted so the secret reference remains literal in the
settings file and in debug artifacts:

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

## Workflow invariants

Use these pinned action refs until there is a deliberate update process:

```yaml
actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd # v5
Factory-AI/droid-action@e3d1f5e7861c36fe4a9c4dca3edec87b964b2bc4 # v5
```

Use this baseline model configuration:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
security_severity_threshold: high
security_block_on_critical: true
security_block_on_high: false
show_full_output: false
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

Keep the title escape hatch:

```yaml
!contains(github.event.pull_request.title, '[skip-review]')
```

This means draft PRs are intentionally reviewed, every commit is reviewed,
active reviews are not canceled, stale queued reviews are deduped per PR, and
separate PRs can run concurrently.

For manual `@droid`, keep the trusted actor guard for `OWNER`, `MEMBER`, and
`COLLABORATOR`. Do not simplify it away; it prevents untrusted comments from
starting secrets-backed jobs.

For scheduled security scan, use:

```yaml
on:
  workflow_dispatch:
  schedule:
    - cron: "0 8 * * 1"

concurrency:
  group: droid-security-scan-${{ github.repository }}
  cancel-in-progress: false
```

And preserve:

```yaml
security_scan_schedule: true
security_scan_days: 7
security_severity_threshold: medium
security_block_on_critical: true
security_block_on_high: false
```

## Repository policy integration

If the target repository has `policy/workflow_allowlist.txt`, add each Droid
workflow because the BYOK bridge uses shell `run:` blocks. `ripr` budgets `20`
non-empty run lines for each Droid workflow.

If the target repository does not already have workflow policy governance, do
not add a new policy surface only for Droid unless that repository wants the
extra governance.

## Verification checklist

Before merge, inspect the patch for:

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
repo-specific review guidance present
```

After merge, smoke-test the repository:

1. Open or use one same-repo PR.
2. Confirm Droid Auto Review starts.
3. Confirm Droid initializes with `custom:MiniMax-M2.7-0`.
4. Confirm the review body follows the inspection/repair format.
5. Confirm there are no extra mentions inside Droid-generated review content.
6. Comment `@droid review` as a trusted actor and confirm it runs.
7. Comment `@droid security` as a trusted actor and confirm it runs.
8. Run Droid Security Scan manually once.

Before broad rollout, download one successful Droid debug artifact from `ripr` or
one pilot repository and confirm:

- `settings.local.json` does not contain expanded secrets;
- prompt and debug artifacts do not contain unexpected secrets;
- artifact retention and download permissions are acceptable;
- MiniMax usage is visible and expected in the provider dashboard.

## Pilot plan

Start with three to five low-risk repositories that already have GitHub Actions,
have enough PR traffic to test the setup, do not have unusual fork workflows,
and are not release-critical that week.

After the pilot is uneventful, roll out in batches of 10 to 20 repositories.
Avoid one large org-wide change set; most failures are repo-specific missing
secrets, branch protection, workflow policy, or permission mismatches.
