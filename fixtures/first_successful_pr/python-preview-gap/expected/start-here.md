# RIPR First PR Start Here

Status: advisory
State: actionable

## Start Here

- State: `top_gap`
- Output state: `preview_limited`
- Safe next action: repair one named preview Python gap.
- Top actionable gap: missing boundary assertion
- Changed behavior: `if amount >= threshold:`
- Why this matters: A related Python test reaches this change, but no boundary discriminator was found for the changed behavior.
- Current evidence strength: Static evidence found related Python test context, but the current proof is weak because the discriminator is missing.
- Missing discriminator: amount == threshold
- Focused proof intent: Strengthen the existing related test in `tests/test_pricing.py`: `assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount`.
- Verify command: `pytest tests/test_pricing.py::test_calculate_discount_smoke`
- Receipt command: `ripr outcome --before .ripr/before.json --after .ripr/after.json --format json --out .ripr/receipts/python-threshold.json`
- Receipt path: `target/ripr/receipts/gap-pr-gap-python-app-pricing-py-calculate-discount-predicate-boundary-amount-threshold.targeted-test-outcome.json`
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Evidence boundary:
- Canonical gap: `gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold`
- Language: `python` (preview)
- Static limit: `python_preview`
  - Python repair cards are preview advisory evidence.
- Receipt state: `receipt_missing`

Why this matters:
A related Python test reaches this change, but no boundary discriminator was found for the changed behavior.

Repair:
- Route: `StrengthenExistingTest`
- Target: `tests/test_pricing.py`
- Assertion: `assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount`

Verify command:
`pytest tests/test_pricing.py::test_calculate_discount_smoke`

Receipt command:
`ripr outcome --before .ripr/before.json --after .ripr/after.json --format json --out .ripr/receipts/python-threshold.json`

Agent packet command:
`ripr agent packet --root fixtures/first_successful_pr/python-preview-gap --gap-ledger inputs/reports/gap-decision-ledger.json --gap-id gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold --json > target/ripr/workflow/agent-packet.json`

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
