# RIPR First Useful Action

Status: missing_required_artifact
Audience: agent
Action: generate_missing_artifact

## Next

Generate assistant proof before routing.

## One-Screen Recommendation

- Changed behavior: not named by the selected evidence
- Why: Required joined proof input is missing.
- Current evidence strength: `missing_required_artifact`
- Missing discriminator: missing discriminator unavailable
- Focused proof intent: Generate assistant proof before routing
- Verify after the test edit: `not_available`
- Receipt after verify: `not_available`
- Artifacts: `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json`, `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- Required joined proof input is missing.
- The report must not infer proof state from a raw artifact chain.

## Fallback

Missing required artifact:
`target/ripr/reports/test-oracle-assistant-proof.json`

## Limits

- Static evidence only.
- Does not search hidden state.
- Does not change CI blocking.
