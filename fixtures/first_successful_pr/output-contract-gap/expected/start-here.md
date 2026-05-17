# RIPR First PR Start Here

Status: advisory
State: actionable

## Top Gap

ripr gap: missing output contract

Changed behavior:
`APPLE_M3_AIR_DEVICE_LABELS_TEXT`

Why this matters:
User-facing output changed, but the gap ledger did not find checked output or golden evidence for the changed text.

Repair:
- Route: `AddOutputGolden`
- Target: `fixtures/device-labels/expected/human.txt`
- Assertion: `golden output contains APPLE_M3_AIR_DEVICE_LABELS_TEXT`

Verify:
`cargo xtask goldens check`

Agent packet:
`ripr agent packet --root fixtures/first_successful_pr/output-contract-gap --gap-ledger inputs/reports/gap-decision-ledger.json --gap-id gap:pr:output:device-label --json > target/ripr/workflow/agent-packet.json`

## Artifacts

- Gap decision ledger: `inputs/reports/gap-decision-ledger.json` (present)
- First useful action: `target/ripr/reports/first-useful-action.json` (missing)
- PR repair cards: `target/ripr/review/comments.json` (missing)
- Agent repair packet: `target/ripr/workflow/agent-packet.json` (missing)
- Gate decision: `target/ripr/reports/gate-decision.json` (missing)

## Authority

This packet is advisory. Pass/fail authority remains with explicit gate-decision artifacts when configured.

## Limits

- Composes explicit RIPR artifacts only.
- Does not run hidden analysis.
- Does not edit source or generate tests.
- Does not run mutation testing.
- Does not change CI blocking or gate policy.
