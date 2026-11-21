# RIPR Assistant Loop Health

Status: advisory

Proof packets:
- complete: 1
- partial: 0
- missing required inputs: 0
- missing optional inputs: 0

Evidence movement:
- improved: 0
- unchanged: 0
- regressed: 1
- unknown: 0

Top warnings:
- regressed_movement: 1

Next repair queue:
- `inspect_regression` - src/lib.rs:2 - regressed movement; inspect the changed test and receipt.

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not call providers.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
