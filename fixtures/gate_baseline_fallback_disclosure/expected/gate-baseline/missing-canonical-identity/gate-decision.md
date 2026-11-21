# RIPR Gate Decision

Decision: advisory
Mode: baseline-check
Evaluated: 1
Blocking: 0
Acknowledged: 0
Advisory: 1

## Advisory

- src/pricing.rs:88 weakly_gripped — incomplete_repair_route: missing canonical_gap_id, seam_id, verify_command, receipt_command, inspection_command
  - Gap state: `actionable`
  - Classification: `weakly_gripped`
  - Changed owner: `pricing::discounted_total`
  - Changed behavior: amount >= discount_threshold
  - Why it remains open: amount == discount_threshold
  - Near test: `above_threshold_gets_discount` at `tests/pricing.rs:12`
  - Add: Add one focused discriminator test.
  - Boundary: `static_ripr_evidence_only`
  - Repair route limitation: `incomplete_repair_route`
  - Missing route fields: `canonical_gap_id, seam_id, verify_command, receipt_command, inspection_command`
  - Limitation detail: The gate cannot provide a complete bounded repair route from current evidence.

## Warnings

- gate candidate ripr-review-legacy006 matched baseline evidence by fallback path/line/static_class identity `src/pricing.rs:88:weakly_gripped`; disclosed as baseline_match_kind=legacy_path_line_class — a legacy compatibility match, not a canonical identity match

## Limits

Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.
