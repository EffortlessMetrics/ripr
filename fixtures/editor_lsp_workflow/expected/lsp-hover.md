# RIPR Editor LSP Workflow Hover

Seam: `67fc764ba37d77bd`
File: `src/lib.rs:2`
Class: `weakly_gripped`
Kind: `predicate_boundary`

## Evidence Path

- reach: yes
- activate: yes
- propagate: yes
- observe: yes
- discriminate: yes

## Missing discriminator

- `discount_threshold (equality boundary)`
- `input that hits the boundary: amount >= discount_threshold`

## Related tests

- `tests/pricing.rs::below_threshold_has_no_discount`
- `tests/pricing.rs::far_above_threshold_discounts`

## Suggested test shape

- File: `tests/pricing.rs`
- Suggested name: `discounted_total_boundary_discriminator`
- Assertion shape: `assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)`

## Handoff, verify, and receipt commands

- Packet: `ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/agent/agent-packet.json`
- Brief: `ripr agent brief --root . --seam-id 67fc764ba37d77bd --json > target/ripr/agent/agent-brief.json`
- After snapshot: `ripr check --root . --base origin/main --mode fast --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json`
- Verify: `ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json`
- Receipt: `ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/agent/agent-receipt.json`

## Status projection

- Matching first-useful-action report: show `ripr: first action`.
- Stale saved-workspace evidence: keep `ripr: stale` visible and tell the user to refresh before acting.
- Wrong-root, malformed, or unsupported report: fail closed without adding diagnostics.

## Limits

- Static evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Does not make policy or gate decisions.
