# RIPR PR Review

Status: advisory

Start here:
- State: actionable
- Source: first_useful_action
- Identity: 8f7fa8644fd12280
- File: src/pricing.rs:88
- Repair route: focused_test
- Class: weakly_exposed
- Current evidence strength: Static evidence found related test context, but the current check is weak because the discriminator is missing.
- Missing discriminator: amount == discount_threshold
- Focused proof intent: assert_eq!(discounted_total(/* boundary input where amount == discount_threshold */), /* expected */)
- Suggested focused test: add amount == discount_threshold boundary assertion
- Related test: above_threshold_gets_discount
- Repair start: `ripr agent repair --root . --seam-id 8f7fa8644fd12280 --phase before`
- After the test edit: run the `--attempt ... --phase after` command the before phase prints; it verifies movement and writes the receipt.
- Manual verify without a repair attempt (needs before and after snapshots taken around the test edit): `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Manual receipt without a repair attempt (after the manual verify): `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 8f7fa8644fd12280 --json --out target/ripr/reports/agent-receipt.json`
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
- Repair start: `ripr agent repair --root . --seam-id 8f7fa8644fd12280 --phase before`
- After the test edit: run the `--attempt ... --phase after` command the before phase prints; it verifies movement and writes the receipt.
- Manual verify without a repair attempt (needs before and after snapshots taken around the test edit): `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt: receipt_missing

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/repair-start/pr-review-front-panel.md
- Evidence: fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
