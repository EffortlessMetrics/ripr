# Droid Smoke Tests — TEMPLATE

> Run these after applying the rollout pack to a target repository, and again
> after any change to Droid workflows, review guidance, or model
> configuration.

## Prerequisites

- `FACTORY_API_KEY` and `MINIMAX_API_KEY` repo secrets are set.
- The Factory Droid GitHub App is installed on the target repo.
- The three workflow files have been copied into `.github/workflows/` and
  pushed to the default branch.

## 1. Automatic review

1. Open a same-repo draft PR (a one-line README change is fine).
2. Confirm Droid Auto Review starts within ~1 minute.
3. Confirm the run initializes with `custom:MiniMax-M2.7-0` (quoted in YAML
   as `"custom:MiniMax-M2.7-0"`).
4. Confirm output is **not** a naked LGTM.
5. Confirm a clean review includes:
   - inspected surfaces;
   - checks performed;
   - why no comments;
   - residual risk;
   - validation signal.

## 2. Manual review

Comment on a PR:

```text
@droid review
```

Expected:

- the trusted actor guard allows the run (only `OWNER`, `MEMBER`,
  `COLLABORATOR` triggers it);
- the MiniMax BYOK model is used;
- comments follow `[P0|P1|P2]` and the repair-queue format described in
  `droid-review-rules.md`.

## 3. Manual security review

Comment on a PR:

```text
@droid security
```

Expected:

- a security-focused review runs;
- no unrelated code edits are produced;
- findings include severity and fix direction.

## 4. Full security scan

Implemented by `.github/workflows/droid-security-scan.yml`.

Triggers:

- `workflow_dispatch` (manual);
- weekly Monday 08:00 UTC schedule.

Trigger manually via the Actions tab. Expected:

- the scan uses `custom:MiniMax-M2.7-0`;
- the scan window is 7 days;
- the severity threshold is `medium`;
- critical findings block (`security_block_on_critical: true`);
- high findings do not block (`security_block_on_high: false`);
- no secrets appear in the workflow logs;
- `show_full_output: false` keeps artifact exposure minimal.

## 5. Fork PR behavior

1. Have a contributor open a PR from a fork.
2. Confirm the auto-review workflow **does not run** for the fork PR.
3. Confirm a maintainer can request a review by adding `[droid-review]`-style
   guidance in their PR comment, only after pushing the changes to a
   same-repo branch.

## 6. `[skip-review]` escape hatch

1. Open a same-repo PR with `[skip-review]` in the title.
2. Confirm the auto-review workflow does not run.

## 7. Artifact hygiene

After triggering at least one of (auto-review, manual review, security scan):

1. Download a Droid action debug artifact from a completed run.
2. Confirm `~/.factory/settings.local.json` keeps `${MINIMAX_API_KEY}`
   literal — the expanded MiniMax token must not appear.
3. Search the artifact for `FACTORY_API_KEY`, `MINIMAX_API_KEY`, `apiKey`,
   `Authorization`, and `Bearer`. Confirm no expanded values are present.
4. Confirm `show_full_output: false` is in effect for all Droid action steps
   in the workflow logs.
5. Record the run ID and artifact name in the rollout checklist without
   pasting sensitive content.

If any check above fails, treat the rollout as **incomplete** and roll back
the workflows from the target repo until the issue is fixed upstream.
