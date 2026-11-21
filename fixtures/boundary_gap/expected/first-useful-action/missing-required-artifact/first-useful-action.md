# RIPR First Useful Action

Status: missing_required_artifact
Audience: agent
Action: generate_missing_artifact

## Next

Generate assistant proof before routing.

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
