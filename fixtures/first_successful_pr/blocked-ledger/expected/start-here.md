# RIPR First PR Start Here

Status: advisory
State: blocked

## Start Here

- State: `blocked_artifact`
- Output state: `missing_artifacts`
- Safe next action: resolve this fail-closed state before assigning repair work.
- Reason: The gap decision ledger is blocked: read missing.json failed: not found. Refresh the first-run evidence before assigning repair work.
- Next command: `ripr check --root <cwd>/fixtures/first_successful_pr/blocked-ledger --mode instant --format repo-exposure-json > <cwd>/fixtures/first_successful_pr/blocked-ledger/target/ripr/reports/repo-exposure.json && ripr reports gap-ledger --root <cwd>/fixtures/first_successful_pr/blocked-ledger --repo-exposure <cwd>/fixtures/first_successful_pr/blocked-ledger/target/ripr/reports/repo-exposure.json --out <cwd>/fixtures/first_successful_pr/blocked-ledger/inputs/reports/gap-decision-ledger.json --out-md <cwd>/fixtures/first_successful_pr/blocked-ledger/inputs/reports/gap-decision-ledger.md`
- Next command (PowerShell 1/2): `$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr check --root <cwd>/fixtures/first_successful_pr/blocked-ledger --mode instant --format repo-exposure-json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('<cwd>/fixtures/first_successful_pr/blocked-ledger/target/ripr/reports/repo-exposure.json'), $ripr.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false)) } else { throw "ripr exited with code $LASTEXITCODE" }`
- Next command (PowerShell 2/2): `ripr reports gap-ledger --root <cwd>/fixtures/first_successful_pr/blocked-ledger --repo-exposure <cwd>/fixtures/first_successful_pr/blocked-ledger/target/ripr/reports/repo-exposure.json --out <cwd>/fixtures/first_successful_pr/blocked-ledger/inputs/reports/gap-decision-ledger.json --out-md <cwd>/fixtures/first_successful_pr/blocked-ledger/inputs/reports/gap-decision-ledger.md`

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
