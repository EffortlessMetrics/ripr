# RIPR PR Review

Status: advisory

Top issue:
- File: src/lib.rs:2
- Class: weakly_exposed
- Missing discriminator: discount_threshold (equality boundary)
- Suggested focused test: add amount == discount_threshold assertion
- Related test: tests/pricing.rs::below_threshold_has_no_discount

Movement:
- New policy-eligible gaps: 1
- Baseline gaps still present: 0
- Baseline gaps resolved: 0
- Static movement: unknown
- Coverage/grip: not available

Policy:
- Decision: advisory
- Gate authority: not configured

Repair:
- Agent handoff: `ripr agent start --root fixtures/boundary_gap/input --seam-id 67fc764ba37d77bd --out target/ripr/workflow`
- Verify: `ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt: missing

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/actionable/pr-review-front-panel.md
- Repair: fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.md
- Evidence: fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
