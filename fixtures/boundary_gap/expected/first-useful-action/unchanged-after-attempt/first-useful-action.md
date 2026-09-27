# RIPR First Useful Action

Status: missing_required_artifact
Audience: agent
Action: generate_missing_artifact

## Next

Regenerate a complete agent receipt before routing.

## One-Screen Recommendation

- Changed behavior: The supplied receipt carries no promotable verify evidence.
- Current evidence strength: `missing_required_artifact`
- Missing discriminator: missing discriminator unavailable
- Focused proof intent: Regenerate a complete agent receipt before routing
- Verify after the test edit: `not_available`
- Receipt after verify: `not_available`
- Artifacts: `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json`, `fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/assistant-proof.json`, `fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt/agent-receipt.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- Receipt movement routes only from a complete analysis outcome.
- The report must not promote receipt movement it cannot validate.

## Check Workflow Status

`ripr agent status --root fixtures/boundary_gap/input --json`

## Fallback

Missing required artifact:
`receipt verify/artifact evidence`

## Limits

- Static evidence only.
- Does not search hidden state.
- Does not change CI blocking.
