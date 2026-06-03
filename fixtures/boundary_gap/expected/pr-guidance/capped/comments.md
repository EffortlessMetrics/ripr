# RIPR PR Guidance

- root: .
- base: main
- head: HEAD
- mode: draft
- line annotations: 3
- summary-only recommendations: 7
- suppressed recommendations: 2

Advisory static evidence only. RIPR does not edit source, generate tests, run mutation testing, or make CI blocking by default.

## Line Annotations

- `53d21b642e4945bb` @ `src/pricing.rs:10`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 53d21b642e4945bb --json > target/ripr/workflow/agent-brief.json`
- `5b353664321bdea6` @ `src/pricing.rs:20`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 5b353664321bdea6 --json > target/ripr/workflow/agent-brief.json`
- `644b716437604271` @ `src/pricing.rs:30`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 644b716437604271 --json > target/ripr/workflow/agent-brief.json`

## Summary-Only Recommendations

- `6d620c643ca5495c` @ `src/pricing.rs:40`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 6d620c643ca5495c --json > target/ripr/workflow/agent-brief.json`
- `7678476441e9ad27` @ `src/pricing.rs:50`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 7678476441e9ad27 --json > target/ripr/workflow/agent-brief.json`
- `7f21626446d108b2` @ `src/pricing.rs:60`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 7f21626446d108b2 --json > target/ripr/workflow/agent-brief.json`
- `86849d644aa3d7fd` @ `src/pricing.rs:70`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 86849d644aa3d7fd --json > target/ripr/workflow/agent-brief.json`
- `8f9b38644fe8dee8` @ `src/pricing.rs:80`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 8f9b38644fe8dee8 --json > target/ripr/workflow/agent-brief.json`
- `98b15364552d0c53` @ `src/pricing.rs:90`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id 98b15364552d0c53 --json > target/ripr/workflow/agent-brief.json`
- `b74a163aa6812b31` @ `src/pricing.rs:100`: Static evidence names missing discriminator `input that hits the boundary: amount == discount_threshold` for this seam.
  - state: `weakly_gripped`
  - command: `ripr agent brief --root . --seam-id b74a163aa6812b31 --json > target/ripr/workflow/agent-brief.json`

## Suppressed

- `ae33db3aa13cc766`: summary_cap
- `a6d0c03a9d6a2e7b`: summary_cap

