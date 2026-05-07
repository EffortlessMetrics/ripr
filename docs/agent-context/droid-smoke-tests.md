# Droid Smoke Tests

Run these after changing Droid workflows, Droid review guidance, or Droid model configuration.

For cross-repository rollout discipline, start with
[Droid rollout checklist](droid-rollout.md) and the human
[Roll out Factory Droid review](../how-to/roll-out-droid.md) guide.

Before relying on live smoke tests, run `cargo xtask check-droid-review-config` locally to confirm the automatic review, manual command, and scheduled security-scan workflow invariants still match the checked-in policy.

## Automatic review

1. Open a same-repo draft PR.
2. Confirm Droid Auto Review starts.
3. Confirm the run initializes with `custom:MiniMax-M2.7-0` (quoted in YAML as `"custom:MiniMax-M2.7-0"`).
4. Confirm output is not naked LGTM.
5. Confirm clean review includes:
   - inspected surfaces;
   - checks performed;
   - why no comments;
   - residual risk;
   - validation signal.

## Manual review

Comment:

```text
@droid review
```

Expected:

- trusted actor guard allows the run;
- MiniMax BYOK model is used;
- comments follow `[P0|P1|P2]` and repair-queue format.

## Manual security review

Comment:

```text
@droid security
```

Expected:

- security review runs;
- no unrelated code edits;
- findings include severity and fix direction.

## Full security scan

Implemented by `.github/workflows/droid-security-scan.yml`.

Triggers:
- `workflow_dispatch`
- weekly Monday 08:00 UTC schedule

Expected:
- scan uses `custom:MiniMax-M2.7-0`;
- scan window is 7 days;
- severity threshold is `medium`;
- critical findings block (`security_block_on_critical: true`);
- high findings do not block (`security_block_on_high: false`);
- no secrets are printed in output;
- `show_full_output: false` keeps artifact exposure minimal.

Validate after triggering manually:
- no secrets appear in workflow logs;
- findings include severity and fix direction.

## Artifact hygiene

After changing any Droid workflow, inspect one completed run artifact and confirm:
- generated Factory settings (`~/.factory/settings.local.json`) do not contain expanded secrets — `${MINIMAX_API_KEY}` must remain unexpanded in the heredoc;
- prompt and debug artifacts do not contain unexpected secrets;
- `show_full_output: false` is in effect for all Droid action steps;
- artifact retention and download permissions are appropriate for this repo.

## Rollout smoke sequence

When enabling Droid in a new repository, follow the rollout checklist first.
After merge, use one same-repo PR to confirm automatic review, `@droid review`,
`@droid security`, and manual Droid Security Scan all run with
`custom:MiniMax-M2.7-0`. Inspect one debug artifact before expanding beyond the
pilot batch.
