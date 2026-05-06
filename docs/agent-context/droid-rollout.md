# Droid Rollout Guide

Use this guide when copying `ripr`'s Factory Droid setup into another
repository. The goal is rollout discipline, not new bot behavior: keep the
working MiniMax BYOK bridge, preserve the same trust boundaries, and add enough
repo-specific context that Droid comments remain useful repair packets for
follow-up agents.

## Rollout readiness

Before merging Droid workflows into a target repo, confirm:

```text
Repo:
Default branch:
Factory GitHub App installed:
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

Use org-level secrets scoped to selected repos where possible. Do not expose
`FACTORY_API_KEY` or `MINIMAX_API_KEY` to the whole organization unless every
repo is intended to participate. The MiniMax key should be the Token Plan key
used by the working `ripr` BYOK setup.

## Files to carry forward

Most repos should receive these workflows:

```text
.github/workflows/droid-review.yml
.github/workflows/droid.yml
.github/workflows/droid-security-scan.yml
```

Add lightweight repo guidance alongside the workflows. For small repos this can
be much shorter than `ripr`, but it should preserve these behaviors:

```text
Droid comments are a repair queue for agents.
No naked LGTM.
No arbitrary comment cap.
Findings include failure mode, why here, fix direction, validation, confidence.
Clean reviews produce inspection records.
No extra @mentions in Droid-generated bodies.
```

Useful guidance surfaces to copy or adapt:

```text
.factory/skills/review-guidelines/SKILL.md
.factory/rules/droid-review.md
docs/agent-context/review-invariants.md
docs/agent-context/droid-smoke-tests.md
AGENTS.md
```

## Required workflow invariants

Keep these values identical unless the target repo has an explicit reviewed
reason to diverge:

```text
same-repo PR guard
trusted actor guard for @droid comments
show_full_output: false
quoted heredoc for settings.local.json
literal ${MINIMAX_API_KEY} inside settings.local.json
no ANTHROPIC_AUTH_TOKEN
no ANTHROPIC_BASE_URL
no reasoning_effort
custom:MiniMax-M2.7-0
review_depth: shallow
pinned action refs
```

The working BYOK bridge writes `~/.factory/settings.local.json` at runtime. Do
not switch this MiniMax setup to the Factory Action `settings:` input unless the
Factory path behavior has been revalidated.

The heredoc delimiter must remain quoted:

```bash
cat > "$HOME/.factory/settings.local.json" <<'JSON'
```

and the JSON must keep the API key reference literal:

```json
"apiKey": "${MINIMAX_API_KEY}"
```

This keeps the checked-in workflow and likely debug artifacts from containing an
expanded secret.

## Standard model configuration

Start every rollout repo on the shallow MiniMax baseline:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
```

Do not roll out `review_depth: deep` broadly. Test deep review in `ripr` first,
because Factory may pass model-depth or reasoning parameters that custom models
might not accept.

## Lane-specific guidance

### Automatic PR review

Automatic review should run for same-repo PRs on:

```yaml
on:
  pull_request:
    types: [opened, synchronize, ready_for_review, reopened]
```

Keep per-PR concurrency without canceling active work:

```yaml
concurrency:
  group: droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}
  cancel-in-progress: false
```

This means draft PRs are reviewed intentionally, each commit is reviewed, active
reviews are not canceled, and only stale queued reviews for the same PR are
deduplicated. Keep `[skip-review]` as the title escape hatch.

### Manual `@droid`

Manual commands must be restricted to trusted actors:

```text
OWNER
MEMBER
COLLABORATOR
```

Do not simplify this guard away; manual comments can start secrets-backed jobs.
The manual lane should support repair passes such as:

```text
@droid review
@droid security
@droid security --full
@droid fill
```

### Scheduled security scan

Enable the weekly schedule only after a manual scan succeeds in the target repo.
Use repo-level concurrency:

```yaml
concurrency:
  group: droid-security-scan-${{ github.repository }}
  cancel-in-progress: false
```

Use the medium-threshold scan baseline:

```yaml
security_scan_schedule: true
security_scan_days: 7
security_severity_threshold: medium
security_block_on_critical: true
security_block_on_high: false
```

## Repo-specific policy integration

If the target repo has a workflow allowlist, add entries for each Droid workflow
because the BYOK setup uses shell `run:` blocks. In `ripr`, each Droid workflow
has a 20 non-empty-line budget in `policy/workflow_allowlist.txt`.

If the target repo does not already have this policy, do not invent the policy
only for Droid unless the repo wants that governance.

Each rollout PR should explicitly state the required secrets before merge:

```markdown
## Required repo/org secrets

This workflow requires:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Both must be available to this repository before merge.
```

Also document the expected fork and draft posture:

```text
Fork PRs are intentionally skipped for secrets-backed Droid review.
Draft PRs are intentionally reviewed.
Use [skip-review] in the PR title to opt out.
```

## Pilot rollout

Roll out in batches instead of one organization-wide change.

1. Pick 3 to 5 low-risk repos that already have GitHub Actions, have some PR
   traffic, and are not release-critical that week.
2. After merge, smoke test a same-repo PR:
   - automatic Droid review starts;
   - Droid initializes with `custom:MiniMax-M2.7-0`;
   - review output follows the inspection/repair format;
   - Droid-generated review bodies do not add extra `@mentions`;
   - `@droid review` works;
   - `@droid security` works;
   - the security scan runs by `workflow_dispatch`.
3. Download one debug artifact from a pilot run and confirm:
   - `settings.local.json` does not contain expanded secrets;
   - prompt/debug context does not contain unexpected secrets;
   - artifact retention and download permissions are acceptable;
   - MiniMax usage looks expected.
4. If the pilot is boring, continue in batches of 10 to 20 repos.

## Non-goals for initial rollout

Do not include these in the broad rollout:

```text
review_depth: deep
self-hosted or VPS runners
pull_request_target
secrets-backed Droid jobs for fork PR code
comment post-processing to remove Factory wrapper mentions
global permission reductions that have not been tested in ripr
```
