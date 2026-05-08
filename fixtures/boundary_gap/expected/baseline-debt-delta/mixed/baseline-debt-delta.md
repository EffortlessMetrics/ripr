# RIPR Baseline Debt Delta

Status: advisory
Baseline: fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline.json

| Bucket | Count |
| --- | ---: |
| Still present | 1 |
| Resolved | 1 |
| New policy-eligible | 1 |
| Acknowledged | 1 |
| Suppressed | 1 |
| Stale baseline entry | 0 |
| Invalid baseline entry | 1 |
| Missing current input | 0 |

Top new policy-eligible gaps:
- src/new.rs:4 weakly_gripped
  Missing: new == 4
  Action: add a focused test or acknowledge visibly.

Resolved baseline entries:
- src/gone.rs:2 weakly_gripped

Limits: Advisory baseline debt movement over static RIPR gate evidence; pass/fail remains owned by ripr gate evaluate.
