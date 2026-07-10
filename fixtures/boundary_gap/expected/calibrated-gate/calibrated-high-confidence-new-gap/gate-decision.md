# RIPR Gate Decision

Decision: blocked
Mode: calibrated-gate
Evaluated: 1
Blocking: 1
Acknowledged: 0
Advisory: 0

## Blocking

- src/pricing.rs:88 weakly_gripped — new policy-eligible gap has supporting recommendation calibration
  - Gap state: `actionable`
  - Gap: `gap:dedf923a13a00573`
  - Seam: `8f7fa8644fd12280`
  - Classification: `weakly_gripped`
  - Changed owner: `pricing::discounted_total`
  - Changed behavior: amount >= discount_threshold
  - Why it remains open: input that hits the boundary: amount == discount_threshold
  - Near test: `above_threshold_gets_discount` at `tests/pricing.rs:12`
  - Add: Write one focused Rust test for input that hits the boundary: amount == discount_threshold. Place it in tests/pricing.rs near above_threshold_gets_discount. Do not change production code. Preserve existing fixture style. Verify with ripr agent verify.
  - Verify: `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
  - Receipt: `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 8f7fa8644fd12280 --json --out target/ripr/reports/agent-receipt.json`
  - Inspect: `ripr agent brief --root . --seam-id 8f7fa8644fd12280 --json > target/ripr/workflow/agent-brief.json`
  - Boundary: `static_ripr_evidence_only`

## Limits

Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.
