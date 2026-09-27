# RIPR First Useful Action

Status: missing_required_artifact
Audience: agent
Action: generate_missing_artifact

## Next

Generate assistant proof before routing.

## One-Screen Recommendation

- Changed behavior: Required joined proof input is missing.
- Current evidence strength: `missing_required_artifact`
- Missing discriminator: missing discriminator unavailable
- Focused proof intent: Generate assistant proof before routing
- Verify after the test edit: `not_available`
- Receipt after verify: `not_available`
- Artifacts: `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json`, `fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/assistant-proof.json`, `fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/agent-receipt.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- Required joined proof input is missing.
- The report must not infer proof state from a raw artifact chain.

## Fallback

Missing required artifact:
`receipt verify/artifact evidence`

## Limits

- Static evidence only.
- Does not search hidden state.
- Does not change CI blocking.
