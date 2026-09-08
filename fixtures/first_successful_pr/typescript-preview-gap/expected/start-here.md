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
- Verify command: `jest tests/discount.test.ts`
- Receipt command: `ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-command "jest tests/discount.test.ts" --status not_run --out target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json`
- Receipt path: `target/ripr/receipts/gap-pr-gap-typescript-typescript-preview-2396aec1.targeted-test-outcome.json`
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

Verify command:
`jest tests/discount.test.ts`

Verify command (PowerShell):
`jest tests/discount.test.ts`

The first form is written for Bash; cmd.exe is not supported.

Receipt command:
`ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-command "jest tests/discount.test.ts" --status not_run --out target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json`

Receipt command (PowerShell):
`ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-command "jest tests/discount.test.ts" --status not_run --out target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json`

The first form is written for Bash; cmd.exe is not supported.

Agent packet command:
`ripr agent packet --root fixtures/first_successful_pr/typescript-preview-gap --gap-ledger inputs/reports/gap-decision-ledger.json --gap-id gap:pr:gap:typescript:typescript_preview:2396aec1 --json > target/ripr/workflow/agent-packet.json`

Agent packet command (PowerShell):
`$staging = 'target/ripr/workflow/agent-packet.json.ripr-staging'; Remove-Item -LiteralPath $staging -Force -ErrorAction SilentlyContinue; $process = Start-Process -FilePath 'ripr' -ArgumentList @('agent', 'packet', '--root', 'fixtures/first_successful_pr/typescript-preview-gap', '--gap-ledger', 'inputs/reports/gap-decision-ledger.json', '--gap-id', 'gap:pr:gap:typescript:typescript_preview:2396aec1', '--json') -RedirectStandardOutput $staging -NoNewWindow -Wait -PassThru; if ($process.ExitCode -ne 0) { Remove-Item -LiteralPath $staging -Force -ErrorAction SilentlyContinue; throw "ripr exited with code $($process.ExitCode)" }; Move-Item -LiteralPath $staging -Destination 'target/ripr/workflow/agent-packet.json' -Force`

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
