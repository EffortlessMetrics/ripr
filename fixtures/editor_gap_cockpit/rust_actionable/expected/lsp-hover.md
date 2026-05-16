**ripr** gap decision

## Evidence boundary

- Language: rust
- Status: stable
- Action: advisory only

## Gap state

- canonical gap: `gap:rust:pricing:threshold-boundary`
- state: `actionable`
- repairability: `repairable`

## Why this matters

The related test reaches the changed branch but does not check the equality
boundary value.

## Repair route

- route: `AddBoundaryAssertion`
- related test: `tests/pricing.rs::discount_threshold_boundary`
- assertion shape: `assert_eq!(discounted_total(threshold), expected)`

## Verify and receipt

- verify: `ripr agent verify --root . --json`
- receipt: `ripr agent receipt --root . --json`

## Limits

- Static evidence only.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
