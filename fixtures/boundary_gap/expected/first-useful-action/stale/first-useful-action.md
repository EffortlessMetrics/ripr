# RIPR First Useful Action

Status: stale
Audience: developer
Action: refresh_evidence

## Next

Refresh RIPR evidence before acting.

## One-Screen Recommendation

- Changed behavior: The best available seam evidence is stale.
- Current evidence strength: `Static evidence found related test context, but the current check is weak because the discriminator is missing.`
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Focused proof intent: Refresh RIPR evidence before acting
- Verify after the test edit: `not_available`
- Receipt after verify: `not_available`
- Artifacts: `target/ripr/workflow/evidence-context.json`, `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- Stale evidence blocks first-action routing.
- The report must not present stale seam evidence as current.

## Fallback

Refresh RIPR evidence before selecting a focused-test action.

## Limits

- Static evidence only.
- Does not rerun hidden analysis.
- Does not edit source or generate tests.
