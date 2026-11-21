**ripr** gap decision

## Evidence boundary

- Language: python
- Status: preview
- Evidence: syntax-first advisory
- Static limit: missing_import_graph
- Detail: Imported symbol targets were not resolved in syntax-first preview mode.

## Gap state

- canonical gap: `gap:py:pricing:import-error-path`
- state: `actionable`
- repairability: `repairable`

## Why this matters

The related test appears to reach the changed error path, but the test does not
check the specific error kind.

## Repair route

- route: `AddErrorPathAssertion`
- related test: `tests/test_pricing.py::test_rejects_invalid_rate`
- assertion shape: `with pytest.raises(ValueError): calculate_discount(rate)`

## Suggested action

Suggested action: Add one error-kind assertion in the related test, then run
verify and receipt.

## Verify and receipt

- verify: `ripr agent verify --root . --json`
- receipt: `ripr agent receipt --root . --json`

## Limits

- Preview evidence is advisory.
- Static evidence only; no runtime adequacy claim.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
