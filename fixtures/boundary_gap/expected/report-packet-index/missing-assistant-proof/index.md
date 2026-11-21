# RIPR Report Packet Index

Status: warn

Start here:
- PR review front panel: target/ripr/reports/pr-review-front-panel.md

Packet summary:
- Available artifacts: 3
- Missing expected artifacts: 1
- Warnings: 1
- Failures: 0

PR review story:
- first useful action: target/ripr/reports/first-useful-action.md
- review comments: target/ripr/review/comments.md

Repair and agent handoff:
- assistant proof: missing
  - next: `ripr assistant-loop proof --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md`

Missing expected:
- assistant proof: missing_required_input

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
