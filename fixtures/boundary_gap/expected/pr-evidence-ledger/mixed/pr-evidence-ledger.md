# RIPR PR Evidence Ledger

Status: advisory
Gate: acknowledgeable / acknowledged

| Measure | Count |
| --- | ---: |
| New policy-eligible gaps | 1 |
| Existing baseline gaps still present | 2 |
| Baseline gaps resolved | 3 |
| Acknowledged gaps | 1 |
| Suppressed gaps | 1 |
| Blocking candidates | 0 |
| Visible unresolved gaps | 4 |

Top focused test to add:
- src/new.rs:4
  Evidence boundary:
  - Receipt state: receipt_missing
  Missing discriminator: new == 4
  Suggested test: assert_eq!(new(), 4)
  Related test: tests/new.rs::boundary
  Verify: ripr agent verify --json
  Agent: ripr agent start --root . --seam-id new --out target/ripr/workflow

Receipts:
- Gate decision: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/gate-decision.json
- Baseline debt delta: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/baseline-debt-delta.json
- RIPR Zero status: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/ripr-zero-status.json
- Agent receipt: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/agent-receipt.json

Coverage/grip frontier:
- Status: not_available
- Coverage delta: not_available
- RIPR visible unresolved delta: not_available
- Interpretation: coverage input not supplied; coverage and behavioral grip remain separate axes

History:
- Records: 1
- Baseline resolved total: 2
- New policy-eligible total: 0
- Trend: improving

Warnings:
- coverage input not supplied; coverage/grip frontier is not_available

Limits: Read-only advisory PR evidence ledger over existing static RIPR artifacts; gate-decision remains the pass/fail authority.
