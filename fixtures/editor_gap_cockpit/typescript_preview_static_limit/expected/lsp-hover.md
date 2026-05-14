**ripr** gap decision

## Evidence boundary

- Language: typescript
- Status: preview
- Evidence: syntax-first advisory
- Static limit: mocked_module
- Detail: Mocked module behavior is syntax-first preview evidence; runtime replacement semantics are not modeled.

## Gap state

- canonical gap: `gap:ts:pricing:mocked-discount-client`
- state: `actionable`
- repairability: `repairable`

## Why this matters

The changed call appears reachable from the related test, but the test does not
check the mocked dependency interaction.

## Repair route

- route: `AddInteractionAssertion`
- related test: `src/pricing.test.ts::applies_discount_from_client`
- assertion shape: `expect(discountClient.lookup).toHaveBeenCalledWith(customerId)`

## Suggested action

Suggested action: Add one interaction assertion in the related test, then run
verify and receipt.

## Verify and receipt

- verify: `ripr agent verify --root . --json`
- receipt: `ripr agent receipt --root . --json`

## Limits

- Preview evidence is advisory.
- Static evidence only; no runtime adequacy claim.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
