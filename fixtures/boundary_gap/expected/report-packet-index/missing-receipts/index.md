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
  - next: `ripr agent status --root .`
- Check PR: missing

Missing expected:
- Agent receipt: not_generated
  - next: `ripr agent status --root .`
- Check PR: not_generated

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
