# RIPR First PR Start Here

Status: advisory
State: blocked

## Blocked

The gap decision ledger is blocked: read missing.json failed: not found. Refresh the first-run evidence before assigning repair work.

Next:
`ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out inputs/reports/gap-decision-ledger.json --out-md inputs/reports/gap-decision-ledger.md`

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
