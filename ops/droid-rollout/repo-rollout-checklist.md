# Repo Rollout Checklist — TEMPLATE

> Use this checklist when applying the rollout pack to a new
> EffortlessMetrics repository. Copy it into a planning doc or PR description
> for the rollout PR; do **not** commit a per-repo filled-in copy back here.

## Repo metadata

- Target repo:
- Branch protection rules:
- CODEOWNERS coverage:
- Existing CI surface:

## Prerequisites

- [ ] `FACTORY_API_KEY` repo secret is set.
- [ ] `MINIMAX_API_KEY` repo secret is set.
- [ ] Factory Droid GitHub App is installed on the target repo.
- [ ] `[skip-review]` and `@droid` comment conventions documented in target
      repo's CONTRIBUTING or agent-context.

## Workflow installation

- [ ] `ops/droid-rollout/droid-review.yml` copied to
      `.github/workflows/droid-review.yml`.
- [ ] `ops/droid-rollout/droid.yml` copied to `.github/workflows/droid.yml`.
- [ ] `ops/droid-rollout/droid-security-scan.yml` copied to
      `.github/workflows/droid-security-scan.yml`.
- [ ] No edits weakened the same-repo guard, trusted actor guard,
      `show_full_output: false`, pinned SHAs, or BYOK literal-key handling.

## Documentation installation

- [ ] `review-invariants.md` placed in target repo's agent-context.
- [ ] `droid-review-rules.md` placed in target repo's agent-context.
- [ ] `review-guidelines.SKILL.md` placed where the target repo expects
      skills.
- [ ] `droid-smoke-tests.md` placed in target repo's agent-context.
- [ ] **Glossary** and **Repo-specific priorities** sections tailored.

## Smoke tests run

- [ ] Automatic review fires on a same-repo draft PR.
- [ ] Manual `@droid review` works for a trusted actor.
- [ ] Manual `@droid security` works for a trusted actor.
- [ ] Manual `workflow_dispatch` of the security scan completes.
- [ ] Fork PR is **not** auto-reviewed.
- [ ] `[skip-review]` PR is **not** auto-reviewed.

## Artifact hygiene baseline

Record one inspection per rollout. Do **not** paste artifact contents.

- Run ID:
- Artifact name:
- Files inspected (categories only):
- `${MINIMAX_API_KEY}` confirmed literal in generated
  `~/.factory/settings.local.json`: yes / no
- No expanded `FACTORY_API_KEY` or `MINIMAX_API_KEY` in any artifact: yes / no
- `show_full_output: false` confirmed effective in workflow logs: yes / no
- Result: pass / fail
- Residual risk noted:

## Rollback plan

- [ ] If smoke tests or artifact hygiene fail, revert the workflow files in
      the target repo and reopen the rollout when the upstream issue is fixed.
- [ ] Document the failure mode in the rollout PR or in a follow-up issue
      against `EffortlessMetrics/ripr` so the rollout pack itself can be
      improved.

## Sign-off

- Rollout PR:
- Smoke test evidence (run links):
- Artifact hygiene baseline link:
- Approver:
