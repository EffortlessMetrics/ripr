# RIPR PR Guidance

- root: .
- base: main
- head: HEAD
- mode: draft
- line annotations: 0
- summary-only recommendations: 1
- suppressed recommendations: 0

Advisory static evidence only. RIPR does not edit source, generate tests, run mutation testing, or make CI blocking by default.

## Line Annotations

- None.

## Summary-Only Recommendations

- `8f7fa8644fd12280` @ `src/pricing.rs:88`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 8f7fa8644fd12280 --json > target/ripr/workflow/agent-brief.json`

## Suppressed

- None.

