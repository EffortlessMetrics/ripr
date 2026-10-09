# RIPR First PR Start Here

Status: advisory
State: actionable

## Start Here

- State: `top_gap`
- Output state: `preview_limited`
- Safe next action: repair one named preview TypeScript gap.
- Top actionable gap: missing boundary assertion
- Changed behavior: `amount >= threshold`
- Why this matters: A related TypeScript test reaches this change, but no boundary discriminator was found for the changed behavior.
- Current evidence strength: Static evidence found related TypeScript test context, but the current proof is weak because the discriminator is missing.
- Missing discriminator: amount == threshold
- Focused proof intent: Add a focused boundary assertion in `tests/discount.test.ts`.
- Verify after the test edit: `(cd -P -- <root> && npx --no-install jest tests/discount.test.ts)`
- Verify after the test edit (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
- Receipt after verify: `(cd -P -- <root> && ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-command "npx --no-install jest tests/discount.test.ts" --status not_run --out target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json)`
- Receipt after verify (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
- Receipt status: the command records `--status not_run` as printed; after the verify command runs, change it to `--status passed` if verify exited 0 or `--status failed` if it did not.
- Receipt path: `target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json`
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Evidence boundary:
- Canonical gap: `gap:typescript:typescript_preview:2396aec1`
- Language: `typescript` (preview)
- Static limit: `typescript_preview`
  - TypeScript repair packets are preview advisory evidence.
- Receipt state: `receipt_missing`

Why this matters:
A related TypeScript test reaches this change, but no boundary discriminator was found for the changed behavior.

Repair:
- Route: `AddBoundaryAssertion`
- Target: `tests/discount.test.ts`

Verify after the test edit: `(cd -P -- <root> && npx --no-install jest tests/discount.test.ts)`
Verify after the test edit (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
The first form is written for Bash; cmd.exe is not supported.

Receipt after verify: `(cd -P -- <root> && ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-command "npx --no-install jest tests/discount.test.ts" --status not_run --out target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json)`
Receipt after verify (PowerShell) unavailable: PowerShell selected-root form is unavailable because generic shell text does not establish native exit-status semantics. Use the Bash form, or inspect the raw command in JSON and run it from the selected repository in PowerShell, checking its result before recording a receipt.
The first form is written for Bash; cmd.exe is not supported.

Agent packet command:
`ripr agent packet --root <cwd>/fixtures/first_successful_pr/typescript-preview-gap --gap-ledger <cwd>/fixtures/first_successful_pr/typescript-preview-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:gap:typescript:typescript_preview:2396aec1 --json > <cwd>/fixtures/first_successful_pr/typescript-preview-gap/target/ripr/workflow/agent-packet.json`

Agent packet command (PowerShell):
`$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr agent packet --root <cwd>/fixtures/first_successful_pr/typescript-preview-gap --gap-ledger <cwd>/fixtures/first_successful_pr/typescript-preview-gap/inputs/reports/gap-decision-ledger.json --gap-id gap:pr:gap:typescript:typescript_preview:2396aec1 --json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('<cwd>/fixtures/first_successful_pr/typescript-preview-gap/target/ripr/workflow/agent-packet.json'), $ripr.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false)) } else { throw "ripr exited with code $LASTEXITCODE" }`

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
