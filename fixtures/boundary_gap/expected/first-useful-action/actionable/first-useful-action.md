# RIPR First Useful Action

Status: actionable
Audience: developer
Action: write_focused_test

## Next

Add equality-boundary discriminator test.

## Why First

- The seam is PR-local.
- The assistant proof report links guidance, handoff, before/after evidence,
  and receipt inputs.
- No waiver, acknowledgement, or suppression applies.

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
- Does not run mutation testing.
- Does not edit source or generate tests.
- Does not make CI blocking by default.
