# RIPR Assistant Loop Health

Status: incomplete

Proof packets:
- complete: 0
- partial: 0
- missing required inputs: 1
- missing optional inputs: 0

Evidence movement:
- improved: 0
- unchanged: 0
- regressed: 0
- unknown: 1

Top warnings:
- missing_required_input: 1

Next repair queue:
- regenerate proof; supply selected seam and before/after static movement context.

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not call providers.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
