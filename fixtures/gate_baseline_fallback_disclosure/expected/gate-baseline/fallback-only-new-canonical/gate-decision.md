# RIPR Gate Decision

Decision: advisory
Mode: baseline-check
Evaluated: 1
Blocking: 0
Acknowledged: 0
Advisory: 1

## Advisory

- src/pricing.rs:88 weakly_gripped — candidate identity is already present in the explicit baseline
  - Gap state: `actionable`
  - Gap: `gap:newboundary0000002`
  - Seam: `newseam0000000002`
  - Classification: `weakly_gripped`
  - Changed owner: `pricing::discounted_total`
  - Changed behavior: amount >= discount_threshold
  - Why it remains open: amount == discount_threshold
  - Near test: `above_threshold_gets_discount` at `tests/pricing.rs:12`
  - Add: Write one focused Rust test for amount == discount_threshold. Place it in tests/pricing.rs near above_threshold_gets_discount. Do not change production code. Preserve existing fixture style. Verify with ripr agent verify.
  - Verify: `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
  - Receipt: `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id newseam0000000002 --json --out target/ripr/reports/agent-receipt.json`
  - Inspect: `ripr agent brief --root . --seam-id newseam0000000002 --json > target/ripr/workflow/agent-brief.json`
  - Boundary: `static_ripr_evidence_only`

## Warnings

- gate candidate ripr-review-newseam00002 matched baseline evidence by fallback path/line/static_class identity `src/pricing.rs:88:weakly_gripped`; disclosed as baseline_match_kind=legacy_path_line_class — a legacy compatibility match, not a canonical identity match

## Limits

Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.
