# RIPR First Useful Action

Status: actionable
Audience: developer
Action: write_focused_test

## Next

Start the focused-test repair for this seam.

## One-Screen Recommendation

- Changed behavior: Changed behavior `amount >= discount_threshold` lacks a discriminator for `amount == discount_threshold`; the review card names its repair start.
- Current evidence strength: `Static evidence found related test context, but the current check is weak because the discriminator is missing.`
- Missing discriminator: amount == discount_threshold
- Focused proof intent: assert_eq!(discounted_total(/* boundary input where amount == discount_threshold */), /* expected */)
- Repair start: `ripr agent repair --root . --seam-id 8f7fa8644fd12280 --phase before`
- After the test edit: run the `--attempt ... --phase after` command the before phase prints; it verifies movement and writes the receipt.
- Manual verify without a repair attempt (needs before and after snapshots taken around the test edit): `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Manual receipt without a repair attempt (after the manual verify): `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 8f7fa8644fd12280 --json --out target/ripr/reports/agent-receipt.json`
- Artifacts: `fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json`
- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.

## Why First

- The review card carries a repair start, so the seam passed the repair-packet
  check where the card was produced.
- No assistant proof exists yet, so the repair has not started.
- No waiver, acknowledgement, or suppression applies.

## Where

- File: `tests/pricing.rs`
- Related test: `above_threshold_gets_discount`
- Suggested test: `discounted_total_boundary_discriminator`

## Start Repair

`ripr agent repair --root . --seam-id 8f7fa8644fd12280 --phase before`

After the test edit: run the `--attempt ... --phase after` command the before phase prints; it verifies movement and writes the receipt.

## Manual Verify Without A Repair Attempt

`ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`

## Manual Receipt Without A Repair Attempt

`ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 8f7fa8644fd12280 --json --out target/ripr/reports/agent-receipt.json`

## Limits

- Static evidence only.
- Carries the repair start from the review card; does not re-derive
  eligibility.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Does not make CI blocking by default.
