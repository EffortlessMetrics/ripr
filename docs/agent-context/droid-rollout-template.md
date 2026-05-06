# Droid Rollout Template

Use this checklist when copying the `ripr` Factory Droid setup into another
repository. The goal is to preserve the working MiniMax BYOK, fork-safety,
trusted-actor, review-output, and artifact-hygiene posture without adding
repo-specific policy that the target repository does not already use.

## Scope per repository

Start with a small pilot batch. Prefer repositories that already use GitHub
Actions, are not release-critical this week, and have enough normal PR traffic
to validate the bot.

For each target repository, record:

```text
Repo:
Default branch:
Factory Droid GitHub App installed:
Actions enabled:
FACTORY_API_KEY scoped to this repo:
MINIMAX_API_KEY scoped to this repo:
Existing Droid workflows:
Existing workflow allowlist:
Existing AGENTS.md or repo guidance:
Existing validation commands:
Public/fork PR posture:
Security/release sensitivity:
```

## Required secrets and app access

Each target repository needs both secrets available before the workflows merge:

- `FACTORY_API_KEY`
- `MINIMAX_API_KEY`

Prefer organization secrets scoped to selected repositories during rollout. Do
not expose secrets to the whole organization unless every repository is intended
to participate.

The target repository also needs the Factory Droid GitHub App installed and
GitHub Actions enabled.

## Standard workflow set

Copy these lanes when the target repository wants the full bot posture:

- `.github/workflows/droid-review.yml` for automatic same-repo PR review;
- `.github/workflows/droid.yml` for trusted `@droid` commands;
- `.github/workflows/droid-security-scan.yml` for manual and weekly security
  scans.

Keep action references pinned to immutable commit SHAs for workflows using
secrets or write permissions.

## Required safety controls

Verify these controls before merge:

- automatic PR review uses `pull_request`, not `pull_request_target`;
- automatic PR review requires
  `github.event.pull_request.head.repo.full_name == github.repository`;
- manual `@droid` commands require `OWNER`, `MEMBER`, or `COLLABORATOR`;
- all Droid action steps set `show_full_output: false`;
- permissions are explicit and no broader than the target workflow requires;
- active automatic reviews are not canceled, using `cancel-in-progress: false`;
- fork PRs are intentionally skipped for secrets-backed review.

## Standard MiniMax BYOK bridge

Use the runtime Factory settings file, not the Droid Action `settings:` input,
for this MiniMax custom model bridge.

The heredoc delimiter must be quoted so the checked artifact-prone settings file
keeps the API key reference literal:

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

Use the standard model references first:

```yaml
review_depth: shallow
review_model: "custom:MiniMax-M2.7-0"
security_model: "custom:MiniMax-M2.7-0"
```

Do not add `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, or reasoning/depth
provider parameters during baseline rollout.

## Review guidance to copy or adapt

Add lightweight repo-specific guidance even for small repositories. Preserve
these behaviors:

- Droid comments are a repair queue for follow-up agents;
- no naked `LGTM` summaries;
- no arbitrary low comment cap when many concrete findings exist;
- actionable findings include failure mode, invariant, fix direction,
  validation, and confidence;
- clean reviews include an inspection record;
- Droid-generated bodies avoid extra human, team, bot, or organization
  mentions.

For `ripr`, the canonical local guidance lives in:

- `.factory/skills/review-guidelines/SKILL.md`
- `.factory/rules/droid-review.md`
- `docs/agent-context/review-invariants.md`
- `docs/agent-context/droid-smoke-tests.md`

## Policy integration

If the target repository already enforces workflow shell budgets, add or update
entries for each Droid workflow because the BYOK bridge uses shell `run:`
blocks. Do not introduce a full workflow allowlist system only to roll out
Droid unless the target repository already wants that governance.

For repositories with `ripr`-style policy, validate with:

```bash
cargo xtask check-workflows
cargo xtask check-droid-review-config
```

## Smoke test after merge

On a same-repo smoke PR, confirm:

- Droid Auto Review starts;
- Droid initializes with `custom:MiniMax-M2.7-0`;
- the review body follows inspection and repair-queue guidance;
- no extra mentions appear inside the Droid-generated body;
- `@droid review` works for a trusted actor;
- `@droid security` works for a trusted actor;
- Droid Security Scan works from `workflow_dispatch`.

Then inspect one completed Droid debug artifact and confirm:

- `settings.local.json` does not contain an expanded MiniMax secret;
- prompt and debug artifacts do not contain unexpected secrets;
- artifact retention and download permissions match the target repository's
  expectations;
- `show_full_output: false` remains in every Droid action step.

## Rollout discipline

Do not roll out these changes during the baseline phase:

- `review_depth: deep`;
- self-hosted runners;
- `pull_request_target` for secrets-backed review;
- fork PR execution with secrets;
- comment post-processing for platform wrapper mentions;
- global permission reductions that have not first been tested on a pilot repo.
