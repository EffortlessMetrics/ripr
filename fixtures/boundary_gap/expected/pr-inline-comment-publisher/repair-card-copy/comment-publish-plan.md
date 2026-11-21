# RIPR Inline Comment Publish Plan

Mode: plan
Status: advisory

Summary:
- publishable comments: 1
- skipped: 0
- blocked: 0
- default: inline comments are off

Planned operations:
- create src/pricing.rs:88 `gap:rust:pricing:discount:threshold-boundary`
  - gap: missing boundary assertion
  - changed behavior: `amount == discount_threshold`
  - repair route: add boundary assertion
  - repair: Add an exact assertion for `amount == discount_threshold`.
  - verify: `cargo xtask fixtures boundary_gap`

Limits:
- Advisory inline-comment publish plan only.
- Does not post comments unless explicit inline mode is configured.
- Never publishes summary-only guidance inline.
- Gate decision remains separate pass/fail authority.
