# RIPR Report Packet Index

Status: warn

Start here:
- PR review front panel: target/ripr/reports/pr-review-front-panel.md

Packet summary:
- Available artifacts: 5
- Missing expected artifacts: 2
- Warnings: 2
- Failures: 0

PR review story:
- First useful action: target/ripr/reports/first-useful-action.md
- Review guidance: target/ripr/review/comments.md

Repair and agent handoff:
- Assistant proof: target/ripr/reports/test-oracle-assistant-proof.md
- Assistant loop health: target/ripr/reports/assistant-loop-health.md

Validation receipts:
- Agent receipt: missing
  - next: `ripr agent receipt --out target/ripr/reports/agent-receipt.json`
- Check PR: missing
  - next: `cargo xtask check-pr`

Missing expected:
- Agent receipt: not_generated
  - next: `ripr agent receipt --out target/ripr/reports/agent-receipt.json`
- Check PR: not_generated
  - next: `cargo xtask check-pr`

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
