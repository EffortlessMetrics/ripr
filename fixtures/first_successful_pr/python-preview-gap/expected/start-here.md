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
- Verify after the test edit: `pytest tests/test_pricing.py::test_calculate_discount_smoke`
- Receipt after verify: `ripr outcome --before .ripr/before.json --after .ripr/after.json --format json --out .ripr/receipts/python-threshold.json`
- Receipt path: `.ripr/receipts/python-threshold.json`
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

Verify after the test edit:
`pytest tests/test_pricing.py::test_calculate_discount_smoke`

It runs unchanged in Bash and PowerShell; cmd.exe is not supported.

Receipt after verify:
`ripr outcome --before .ripr/before.json --after .ripr/after.json --format json --out .ripr/receipts/python-threshold.json`

It runs unchanged in Bash and PowerShell; cmd.exe is not supported.

Agent packet command:
`ripr agent packet --root <cwd>/fixtures/first_successful_pr/python-preview-gap --gap-ledger <cwd>/fixtures/first_successful_pr/python-preview-gap/inputs/reports/gap-decision-ledger.json --gap-id 'gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold' --json > <cwd>/fixtures/first_successful_pr/python-preview-gap/target/ripr/workflow/agent-packet.json`

Agent packet command (PowerShell):
`$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr agent packet --root <cwd>/fixtures/first_successful_pr/python-preview-gap --gap-ledger <cwd>/fixtures/first_successful_pr/python-preview-gap/inputs/reports/gap-decision-ledger.json --gap-id 'gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold' --json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('<cwd>/fixtures/first_successful_pr/python-preview-gap/target/ripr/workflow/agent-packet.json'), $ripr.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false)) } else { throw "ripr exited with code $LASTEXITCODE" }`

The first form is written for Bash; cmd.exe is not supported.

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
