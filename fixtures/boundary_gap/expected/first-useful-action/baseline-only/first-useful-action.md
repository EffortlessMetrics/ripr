# RIPR First Useful Action

Status: baseline_only
Audience: reviewer
Action: acknowledge_baseline

## Next

Leave existing baseline debt outside this PR action.

## One-Screen Recommendation

- Changed behavior: not named by the selected evidence
- Why: The visible debt is baseline-only and not PR-local first-action work.
- Current evidence strength: `Static evidence found related test context, but the current check is weak because the discriminator is missing.`
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Focused proof intent: Leave existing baseline debt outside this PR action
- Verify after the test edit: `not_available`
- Receipt after verify: `not_available`
- Artifacts: `target/ripr/reports/baseline-debt-delta.json`, `target/ripr/reports/pr-evidence-ledger.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- The visible debt is baseline-only.
- No new PR-local actionable seam outranks it.

## Fallback

Track or acknowledge baseline debt separately from PR-local first action.

## Limits

- Static evidence only.
- Does not invent policy.
- Does not make CI blocking by default.
