# RIPR PR Review

Status: blocked

Start here:
- State: actionable
- Source: first_useful_action
- Identity: 67fc764ba37d77bd
- File: src/lib.rs:2
- Repair route: focused_test
- Class: weakly_exposed
- Current evidence strength: Static evidence found related test context, but the current check is weak because the discriminator is missing.
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Focused proof intent: Add a focused boundary test that exercises amount >= discount_threshold and assert the exact discounted_total output.
- Suggested focused test: add amount >= discount_threshold boundary assertion
- Related test: tests/pricing.rs::below_threshold_has_no_discount
- Verify command: `ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt command: `ripr agent receipt --root fixtures/boundary_gap/input --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json`
- Receipt: receipt_missing
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Movement:
- New policy-eligible gaps: 1
- Blocking candidates: 1
- Static movement: unknown
- Coverage/grip: not available

Policy:
- Mode: calibrated-gate
- Decision: blocked
- Gate authority: fixtures/boundary_gap/expected/pr-review-front-panel/blocked/gate-decision.md
- Acknowledgement label: ripr-waive

Repair:
- Agent handoff: `ripr agent start --root fixtures/boundary_gap/input --seam-id 67fc764ba37d77bd --out target/ripr/workflow`
- Verify: `ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt: receipt_missing

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/blocked/pr-review-front-panel.md
- Policy: fixtures/boundary_gap/expected/pr-review-front-panel/blocked/gate-decision.md
- Repair: fixtures/boundary_gap/expected/first-useful-action/actionable/first-useful-action.md

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
