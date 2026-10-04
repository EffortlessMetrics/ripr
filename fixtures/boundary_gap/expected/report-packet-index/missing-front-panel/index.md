# RIPR Report Packet Index

Status: warn

Next: generate the PR review front panel before using the packet index as the first-screen PR story.

Packet summary:
- Available artifacts: 3
- Missing expected artifacts: 1
- Warnings: 1
- Failures: 0

Start here:
- PR review front panel: missing
  - next: `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md`

PR review story:
- First useful action: target/ripr/reports/first-useful-action.md
- Review guidance: target/ripr/review/comments.md

Repair and agent handoff:
- Assistant proof: target/ripr/reports/test-oracle-assistant-proof.md

Missing expected:
- PR review front panel: not_generated
  - next: `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md`

Limits:
- Advisory report-packet index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
