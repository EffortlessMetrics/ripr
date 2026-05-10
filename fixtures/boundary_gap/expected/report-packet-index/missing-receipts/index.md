# RIPR Report Packet Index

Status: warn

Start here:
- PR review front panel: target/ripr/reports/pr-review-front-panel.md

Packet summary:
- Available artifacts: 3
- Missing expected artifacts: 2
- Warnings: 2
- Failures: 0

PR review story:
- first useful action: target/ripr/reports/first-useful-action.md
- review comments: target/ripr/review/comments.md

Validation receipts:
- agent receipt: missing
  - next: `ripr agent receipt --out target/ripr/reports/agent-receipt.json`
- check PR: missing
  - next: `cargo xtask check-pr`

Missing expected:
- agent receipt: not_generated
- check PR: not_generated

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
