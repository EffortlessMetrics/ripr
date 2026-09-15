# RIPR Gate Decision

Decision: advisory
Mode: acknowledgeable
Evaluated: 2
Blocking: 0
Acknowledged: 0
Advisory: 1

## Advisory

- src/pricing.rs:88 weakly_gripped — summary-only recommendation remains visible and advisory
  - Seam: `summary-seam`
  - Classification: `weakly_gripped`
  - Why it remains open: amount == discount_threshold
  - Boundary: `static_ripr_evidence_only`
  - Repair route limitation: `incomplete_repair_route`
  - Missing route fields: `canonical_gap_id, gap_state, changed_owner, changed_behavior, repair_target, test_intent, verify_command, receipt_command, inspection_command`
  - Limitation detail: The gate cannot provide a complete bounded repair route from current evidence.

## Suppressed

- src/pricing.rs:89 weakly_gripped — configured-hidden or suppressed candidate preserved as `severity_off`

## Limits

Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.
