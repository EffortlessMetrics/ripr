# RIPR First PR Start Here

Status: advisory
State: actionable

## Start Here

- State: `top_gap`
- Output state: `actionable_gap`
- Safe next action: repair one named stable Rust gap.
- Top actionable gap: missing output contract
- Changed behavior: `APPLE_M3_AIR_DEVICE_LABELS_TEXT`
- Why this matters: User-facing output changed, but the gap ledger did not find checked output or golden evidence for the changed text.
- Current evidence strength: Static evidence found changed user-facing output, but no checked output or golden proof is attached.
- Missing discriminator: Checked output or golden proof for the changed text.
- Focused proof intent: Add or update the output proof in `fixtures/device-labels/expected/human.txt` so `golden output contains APPLE_M3_AIR_DEVICE_LABELS_TEXT`.
- Verify command: `cargo xtask goldens check`
- Receipt command: `ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --format json --out target/ripr/receipts/gap-pr-output-device-label.targeted-test-outcome.json`
- Receipt path: `target/ripr/receipts/gap-pr-output-device-label.targeted-test-outcome.json`
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Evidence boundary:
- Canonical gap: `gap:rust:output:device-label`
- Language: `rust` (stable)
- Receipt state: `receipt_missing`

Why this matters:
User-facing output changed, but the gap ledger did not find checked output or golden evidence for the changed text.

Repair:
- Route: `AddOutputGolden`
- Target: `fixtures/device-labels/expected/human.txt`
- Assertion: `golden output contains APPLE_M3_AIR_DEVICE_LABELS_TEXT`

Verify command:
`cargo xtask goldens check`

Receipt command:
`ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --format json --out target/ripr/receipts/gap-pr-output-device-label.targeted-test-outcome.json`

Agent packet command:
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
