# ripr repo exposure report

Schema version: 0.3
Scope: repo

## Summary

| Class | Count |
| --- | --- |
| seams_total | 0 |
| headline_eligible | 0 |
| strongly_gripped | 0 |
| weakly_gripped | 0 |
| ungripped | 0 |
| reachable_unrevealed | 0 |
| activation_unknown | 0 |
| propagation_unknown | 0 |
| observation_unknown | 0 |
| discrimination_unknown | 0 |
| opaque | 0 |
| intentional | 0 |
| suppressed | 0 |

## Limitations

**typescript_diff_first** (ts_file_count: 2)

TypeScript is analyzed diff-first; run 'ripr check --base origin/main' or '--diff <file>' to evaluate changed TypeScript behavior. Full-repo TypeScript exposure is not yet modeled (named limitation).

TypeScript readiness (preview, advisory)

| Signal | Value |
| --- | --- |
| source files | 2 |
| test files | 0 |
| package roots | 1 |
| package confidence | medium |
| runner status | no_tests_detected |
| verify commands | 0 |
| top blocker | typescript_tests_not_detected |

This card is root-level readiness for diff-first TypeScript preview. It does not emit full-repo TypeScript seams, run TypeScript tests, or create gate or badge authority.

No classified seams. The repo seam inventory is empty or no production seams were detected.
