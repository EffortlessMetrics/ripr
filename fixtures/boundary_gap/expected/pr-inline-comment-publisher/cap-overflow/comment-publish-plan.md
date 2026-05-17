# RIPR Inline Comment Publish Plan

Mode: plan
Status: advisory

Summary:
- publishable comments: 3
- skipped: 1
- blocked: 0
- default: inline comments are off

Planned operations:
- create src/pricing.rs:10 `ripr:a:src/pricing.rs:10`
  - gap: missing boundary assertion
  - changed behavior: `amount == threshold_a`
  - repair route: add boundary assertion
  - repair: Add one focused boundary assertion for `amount == threshold_a`.
  - verify: `ripr agent verify`
- create src/pricing.rs:20 `ripr:b:src/pricing.rs:20`
  - gap: missing boundary assertion
  - changed behavior: `amount == threshold_b`
  - repair route: add boundary assertion
  - repair: Add one focused boundary assertion for `amount == threshold_b`.
  - verify: `ripr agent verify`
- create src/pricing.rs:30 `ripr:c:src/pricing.rs:30`
  - gap: missing boundary assertion
  - changed behavior: `amount == threshold_c`
  - repair route: add boundary assertion
  - repair: Add one focused boundary assertion for `amount == threshold_c`.
  - verify: `ripr agent verify`

Skipped:
- cap_reached: 1 recommendation was kept out of inline comments

Limits:
- Advisory inline-comment publish plan only.
- Never publishes summary-only guidance inline.
