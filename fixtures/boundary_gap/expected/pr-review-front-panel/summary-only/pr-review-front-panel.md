# RIPR PR Review

Status: advisory

Start here:
- State: summary_only
- Source: pr_guidance
- Identity: 67fc764ba37d77bd
- File: src/lib.rs:2
- Repair route: focused_test
- Class: weakly_exposed
- Current evidence strength: weakly_exposed
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Focused proof intent: Add a focused boundary test that exercises amount >= discount_threshold and assert the exact discounted_total output.
- Suggested focused test: add amount >= discount_threshold boundary assertion
- Related test: tests/pricing.rs::below_threshold_has_no_discount
- Verify command: `ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt: receipt_missing
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

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
