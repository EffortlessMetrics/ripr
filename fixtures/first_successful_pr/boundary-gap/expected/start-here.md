# RIPR First PR Start Here

Status: advisory
State: actionable

## Top Gap

ripr gap: missing boundary assertion

One-screen recommendation:
- Changed behavior: `amount >= threshold`
- Current evidence strength: actionable repairable static gap in pr_local scope
- Missing discriminator: assert_eq!(discount(100, 100), 90)
- Focused proof intent: Add one focused assertion in `tests/pricing.rs` for `assert_eq!(discount(100, 100), 90)`.
- Verify command: `cargo xtask fixtures boundary_gap`
- Receipt command: `ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --format md --out target/ripr/receipts/gap-pr-pricing-threshold-boundary.md`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

Evidence boundary:
- Canonical gap: `gap:rust:pricing:discount:threshold-boundary`
- Language: `rust` (stable)
- Receipt state: `receipt_missing`

Changed behavior:
`amount >= threshold`

Why this matters:
A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior.

Repair:
- Route: `AddBoundaryAssertion`
- Target: `tests/pricing.rs`
- Assertion: `assert_eq!(discount(100, 100), 90)`

Verify:
`cargo xtask fixtures boundary_gap`

Receipt:
`ripr outcome --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --format md --out target/ripr/receipts/gap-pr-pricing-threshold-boundary.md`

Agent packet:
`ripr agent packet --root fixtures/first_successful_pr/boundary-gap --gap-ledger inputs/reports/gap-decision-ledger.json --gap-id gap:pr:pricing:threshold-boundary --json > target/ripr/workflow/agent-packet.json`

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
