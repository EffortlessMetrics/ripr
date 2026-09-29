# RIPR First PR Start Here

Status: advisory
State: actionable

## Start Here

- State: `top_gap`
- Output state: `actionable_gap`
- Safe next action: repair one named stable Rust gap.
- Top actionable gap: missing boundary assertion
- Changed behavior: `amount >= threshold`
- Why this matters: A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior.
- Current evidence strength: Static evidence found related Rust test context, but the current proof is weak because the discriminator is missing.
- Missing discriminator: Equality-boundary assertion for the changed behavior.
- Focused proof intent: Add a focused boundary assertion in `tests/pricing.rs`: `assert_eq!(discount(100, 100), 90)`.
- Verify after the test edit: `cargo xtask fixtures boundary_gap`
- Receipt after verify: `ripr receipt write --gap gap:rust:pricing:discount:threshold-boundary --verify-command 'cargo xtask fixtures boundary_gap' --status not_run --out target/ripr/receipts/gap-pr-pricing-threshold-boundary.targeted-test-outcome.json`
- Receipt status: the command records `--status not_run` as printed; after the verify command runs, change it to `--status passed` if verify exited 0 or `--status failed` if it did not.
- Receipt path: `target/ripr/receipts/gap-pr-pricing-threshold-boundary.targeted-test-outcome.json`
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Evidence boundary:
- Canonical gap: `gap:rust:pricing:discount:threshold-boundary`
- Language: `rust` (stable)
- Receipt state: `receipt_missing`

Why this matters:
A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior.

Repair:
- Route: `AddBoundaryAssertion`
- Target: `tests/pricing.rs`
- Assertion: `assert_eq!(discount(100, 100), 90)`

Verify after the test edit:
`cargo xtask fixtures boundary_gap`

It runs unchanged in Bash and PowerShell; cmd.exe is not supported.

Receipt after verify:
`ripr receipt write --gap gap:rust:pricing:discount:threshold-boundary --verify-command 'cargo xtask fixtures boundary_gap' --status not_run --out target/ripr/receipts/gap-pr-pricing-threshold-boundary.targeted-test-outcome.json`

It runs unchanged in Bash and PowerShell; cmd.exe is not supported.

Agent packet command:
`ripr agent packet --root <cwd>/fixtures/first_successful_pr/boundary-gap --gap-ledger <cwd>/fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:pricing:threshold-boundary --json > <cwd>/fixtures/first_successful_pr/boundary-gap/target/ripr/workflow/agent-packet.json`

Agent packet command (PowerShell):
`$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr agent packet --root <cwd>/fixtures/first_successful_pr/boundary-gap --gap-ledger <cwd>/fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:pricing:threshold-boundary --json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('<cwd>/fixtures/first_successful_pr/boundary-gap/target/ripr/workflow/agent-packet.json'), $ripr.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false)) } else { throw "ripr exited with code $LASTEXITCODE" }`

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
