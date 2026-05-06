# Droid Rollout Guide

Use this guide when copying `ripr`'s Factory Droid setup to another repository.
The goal is to preserve the working MiniMax M2.7 BYOK review lanes while
keeping rollout failures small, observable, and repo-specific.

## Rollout prerequisites

Before merging Droid workflows into a target repository, confirm:

- the Factory Droid GitHub App is installed for the repository;
- GitHub Actions are enabled for the repository;
- `FACTORY_API_KEY` is available to the repository;
- `MINIMAX_API_KEY` is available to the repository;
- the MiniMax key is the intended token-plan key;
- branch protection and required checks are understood;
- public fork PR posture is understood.

Prefer org-level secrets scoped to the selected pilot repositories. Do not make
secrets org-wide by default unless every repository in the org is intended to
run Droid.

## Files to carry forward

Most repositories should start with these workflow lanes:

- `.github/workflows/droid-review.yml` for automatic same-repo PR review;
- `.github/workflows/droid.yml` for trusted `@droid` commands;
- `.github/workflows/droid-security-scan.yml` for manual or scheduled full
  security scans.

Carry forward lightweight repo guidance as well:

- `.factory/skills/review-guidelines/SKILL.md`;
- `.factory/rules/droid-review.md` or equivalent repo-specific review rules;
- `docs/agent-context/review-invariants.md`;
- `docs/agent-context/droid-smoke-tests.md`;
- `AGENTS.md` guidance for the target repository.

For small repositories, the guidance can be shorter than `ripr`'s, but it should
preserve the operating model: Droid output is a repair queue for follow-up
agents, clean reviews need inspection records, and actionable findings need the
failure mode, invariant, fix direction, validation, and confidence.

## Configuration invariants

Keep these settings unchanged for the first rollout batch:

- `review_depth: shallow`;
- `review_model: "custom:MiniMax-M2.7-0"`;
- `security_model: "custom:MiniMax-M2.7-0"`;
- `show_full_output: false`;
- pinned third-party action refs for secrets-backed workflows;
- same-repo guard for automatic PR review;
- trusted actor guard for manual `@droid` commands;
- `pull_request`, not `pull_request_target`, for automatic PR review.

Use the runtime BYOK bridge that writes `~/.factory/settings.local.json` with a
quoted heredoc and a literal `${MINIMAX_API_KEY}` value. Do not replace this
with the Droid Action `settings:` input unless Factory's custom-model path
behavior has been revalidated.

Do not set `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `reasoning_effort`, or
`review_depth: deep` during the initial rollout.

## Workflow policy integration

If the target repository has a workflow allowlist or similar policy, add each
Droid workflow to that policy before merge. The `ripr` workflows use shell
`run:` blocks for the BYOK settings file, so their allowlist entries need enough
budget for those non-empty run lines.

If the target repository does not already have a workflow policy, do not invent
one solely for Droid. Instead, document the workflow lanes, required secrets,
and smoke-test path in the rollout PR.

## Pilot plan

Roll out in small batches:

1. Pick three to five low-risk repositories that already use GitHub Actions.
2. Scope `FACTORY_API_KEY` and `MINIMAX_API_KEY` to only those repositories.
3. Merge the Droid workflow PRs after required secrets are available.
4. Open or reuse one same-repo PR per repository.
5. Confirm automatic Droid review starts and uses `custom:MiniMax-M2.7-0`.
6. Run `@droid review` from a trusted actor.
7. Run `@droid security` from a trusted actor.
8. Trigger the full security scan manually once before relying on the schedule.
9. Inspect one debug artifact for expanded secrets or unexpected prompt context.
10. Check MiniMax usage after the pilot batch.

After the pilot is boring, continue in batches of 10 to 20 repositories. Avoid a
single organization-wide rollout because failures are usually repo-specific:
missing secrets, unusual branch protection, existing workflow policy, or
unexpected permissions.

## Rollout PR body checklist

Include this information in each target-repository PR:

- workflows added and their triggers;
- required secrets: `FACTORY_API_KEY` and `MINIMAX_API_KEY`;
- fork PR behavior: secrets-backed Droid review intentionally skips forks;
- draft PR behavior: draft same-repo PRs are intentionally reviewable;
- `[skip-review]` opt-out behavior for automatic PR review, if copied;
- action ref pinning and `show_full_output: false`;
- validation plan for auto review, manual review, manual security review, and
  full security scan.

## Do not roll out yet

Do not roll out these changes broadly until they have been tested separately in
`ripr` or another pilot repository:

- `review_depth: deep`;
- self-hosted or VPS runner placement;
- `pull_request_target` workflows;
- fork PR review with secrets;
- post-processing to remove Factory wrapper mentions;
- global permission reductions, such as changing automatic review from
  `contents: write` to `contents: read`, unless Factory behavior has been
  validated with that permission set.
