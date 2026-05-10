# RIPR PR Review

Status: advisory

Top issue:
- File: src/lib.rs:2
- Class: weakly_exposed
- Missing discriminator: discount_threshold (equality boundary)
- Suggested focused test: add amount == discount_threshold assertion
- Related test: tests/pricing.rs::below_threshold_has_no_discount

Movement:
- New policy-eligible gaps: 0
- Baseline gaps resolved: 3
- Static movement: improved
- Coverage/grip: flat coverage, improved grip
- Coverage delta: +0.0%
- RIPR unresolved delta: -3

Policy:
- Decision: advisory
- Gate authority: not configured

Repair:
- Receipt: fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/coverage-flat-grip-improved/pr-review-front-panel.md
- Repair: fixtures/boundary_gap/expected/assistant-loop-health/complete-improved/assistant-loop-health.md
- Calibration: fixtures/boundary_gap/expected/pr-review-front-panel/coverage-flat-grip-improved/coverage-grip-frontier.md

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
