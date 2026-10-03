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
- Verify after the test edit: `(cd -P -- <root> && cargo xtask goldens check)`
- Verify after the test edit (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
- Receipt after verify: `(cd -P -- <root> && ripr receipt write --gap gap:rust:output:device-label --verify-command 'cargo xtask goldens check' --status not_run --out target/ripr/receipts/gap-pr-output-device-label.targeted-test-outcome.json)`
- Receipt after verify (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
- Receipt status: the command records `--status not_run` as printed; after the verify command runs, change it to `--status passed` if verify exited 0 or `--status failed` if it did not.
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

Verify after the test edit: `(cd -P -- <root> && cargo xtask goldens check)`
Verify after the test edit (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
The first form is written for Bash; cmd.exe is not supported.

Receipt after verify: `(cd -P -- <root> && ripr receipt write --gap gap:rust:output:device-label --verify-command 'cargo xtask goldens check' --status not_run --out target/ripr/receipts/gap-pr-output-device-label.targeted-test-outcome.json)`
Receipt after verify (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
The first form is written for Bash; cmd.exe is not supported.

Agent packet command:
`ripr agent packet --root <cwd>/fixtures/first_successful_pr/output-contract-gap --gap-ledger <cwd>/fixtures/first_successful_pr/output-contract-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:output:device-label --json > <cwd>/fixtures/first_successful_pr/output-contract-gap/target/ripr/workflow/agent-packet.json`

Agent packet command (PowerShell):
`$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr agent packet --root <cwd>/fixtures/first_successful_pr/output-contract-gap --gap-ledger <cwd>/fixtures/first_successful_pr/output-contract-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:output:device-label --json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('<cwd>/fixtures/first_successful_pr/output-contract-gap/target/ripr/workflow/agent-packet.json'), $ripr.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false)) } else { throw "ripr exited with code $LASTEXITCODE" }`

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
