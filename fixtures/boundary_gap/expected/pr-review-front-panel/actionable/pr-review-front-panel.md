# RIPR PR Review

Status: advisory

Start here:
- State: actionable
- Source: first_useful_action
- Identity: 67fc764ba37d77bd
- File: src/lib.rs:2
- Repair route: focused_test
- Class: weakly_exposed
- Current evidence strength: Static evidence found related test context, but the current check is weak because the discriminator is missing.
- Missing discriminator: discount_threshold (equality boundary)
- Focused proof intent: Assert the exact discounted_total output at amount == discount_threshold.
- Suggested focused test: add a focused test where amount == discount_threshold and assert the exact discounted_total output.
- Related test: tests/pricing.rs::below_threshold_has_no_discount
- Analysis outcome for the receipt: `ripr check --root <cwd>/fixtures/boundary_gap/input --mode draft --format json > <cwd>/fixtures/boundary_gap/input/target/ripr/workflow/analysis-outcome.json`
- Verify after the test edit: `ripr agent verify --root <cwd>/fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > <cwd>/fixtures/boundary_gap/input/target/ripr/workflow/agent-verify.json`
- Receipt after verify: `ripr agent receipt --root <cwd>/fixtures/boundary_gap/input --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json`
- Receipt: receipt_missing
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

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
- Agent handoff: `ripr agent packet --root <cwd>/fixtures/boundary_gap/input --seam-id 67fc764ba37d77bd --json`
- Verify after the test edit: `ripr agent verify --root <cwd>/fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > <cwd>/fixtures/boundary_gap/input/target/ripr/workflow/agent-verify.json`
- Receipt: receipt_missing

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/actionable/pr-review-front-panel.md
- Repair: fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.md
- Evidence: fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
