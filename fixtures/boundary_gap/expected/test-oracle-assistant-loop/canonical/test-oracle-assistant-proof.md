# RIPR Test-Oracle Assistant Loop

Status: advisory

Top focused test:
- Seam: src/lib.rs:2
- Owner: src/lib.rs::discounted_total
- Missing discriminator: input that hits the boundary: amount >= discount_threshold
- Suggested test: Add a focused boundary test that exercises amount >= discount_threshold and assert the exact discounted_total output.
- Related test: tests/pricing.rs::below_threshold_has_no_discount
- Assertion shape: assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)
- Verify: ripr agent verify --root fixtures/boundary_gap/input --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json

Movement:
- Before: weakly_gripped
- After: weakly_gripped
- State: unchanged
- Receipt: fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json

Projection:
- PR ledger: fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json
- Coverage/grip frontier: not available
- Gate: not configured

Warnings:
- current replay fixture preserves actual analyzer output: the focused-test snapshot remains weakly_gripped; dogfood receipt work records live movement separately

Limits:
- Static RIPR evidence only.
- Advisory by default.
- No source edits, generated tests, provider calls, or mutation execution.
