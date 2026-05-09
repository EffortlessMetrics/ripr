# RIPR Assistant Loop Health

Status: advisory

Proof packets:
- complete: 0
- partial: 1
- missing required inputs: 0
- missing optional inputs: 2

Evidence movement:
- improved: 0
- unchanged: 0
- regressed: 0
- unknown: 1

Top warnings:
- summary_only_guidance: 1
- stale_input: 1
- missing_receipt: 1
- missing_optional_input: 2

Next repair queue:
- src/lib.rs:2 - summary-only guidance; inspect placement before routing test work.
- src/lib.rs:2 - stale after evidence; refresh before/after evidence.
- src/lib.rs:2 - missing receipt; rerun verify and receipt.

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not call providers.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
