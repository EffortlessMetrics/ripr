# RIPR PR Review

Status: advisory

Top issue:
- File: src/lib.rs:2
- Class: weakly_exposed
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Suggested focused test: add amount >= discount_threshold boundary assertion
- Related test: tests/pricing.rs::below_threshold_has_no_discount

Placement:
- summary-only
- Reason: changed-line placement is unsafe

Movement:
- New policy-eligible gaps: 1
- Baseline gaps still present: 0
- Baseline gaps resolved: 0
- Static movement: unknown
- Coverage/grip: not available

Policy:
- Decision: advisory
- Gate authority: not configured

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/summary-only/pr-review-front-panel.md
- Evidence: fixtures/boundary_gap/expected/pr-review-front-panel/summary-only/comments.md

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
