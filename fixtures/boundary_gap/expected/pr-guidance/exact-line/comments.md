# RIPR PR Guidance

- root: .
- base: main
- head: HEAD
- mode: draft
- line annotations: 1
- summary-only recommendations: 0
- suppressed recommendations: 0
- analysis scope: `working_set`
- run status: `scoped`
- scoped production files: 1/unknown
- classified seams considered: 1
- limitation: `review_comments_working_set_scope_only`; repair_route: `analysis/review-comments-working-set`

Advisory static evidence only. RIPR does not edit source, generate tests, run mutation testing, or make CI blocking by default.

## Line Annotations

- `8f7fa8644fd12280` @ `src/pricing.rs:88`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 8f7fa8644fd12280 --json > target/ripr/workflow/agent-brief.json`

## Summary-Only Recommendations

- None.

## Suppressed

- None.

