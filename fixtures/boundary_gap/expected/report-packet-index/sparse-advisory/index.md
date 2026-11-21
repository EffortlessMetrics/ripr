# RIPR Report Packet Index

Status: warn

Start here:
- PR review front panel: target/ripr/reports/pr-review-front-panel.md

Packet summary:
- Available artifacts: 4
- Missing expected artifacts: 2
- Warnings: 2
- Failures: 0

PR review story:
- first useful action: target/ripr/reports/first-useful-action.md
- review comments: target/ripr/review/comments.md

Repair and agent handoff:
- assistant proof: missing
  - next: `ripr assistant-loop proof --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md`

Calibration:
- coverage/grip frontier: configured off

Validation receipts:
- check PR: target/ripr/reports/check-pr.md

Missing expected:
- assistant proof: input_not_available
- coverage/grip frontier: configured_off

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
