# RIPR First Useful Action

Status: unchanged_after_attempt
Audience: agent
Action: revise_focused_test

## Next

Revise the focused test for unchanged static movement.

## Why First

- The supplied receipt records unchanged static movement after a focused-test
  attempt.
- The next safe action is to revise the test rather than request a new
  unrelated seam.

## Where

- File: `tests/pricing.rs`
- Related test: `tests/pricing.rs::below_threshold_has_no_discount`
- Suggested test: `discounted_total_boundary_discriminator`

## Verify

`ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`

## Receipt

`ripr agent receipt --root fixtures/boundary_gap/input --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json`

## Limits

- Static evidence only.
- Does not edit source or generate tests.
- Does not run mutation testing.
