# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json`
- after: `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json`

## Summary

| Bucket | Count |
| --- | ---: |
| moved | 0 |
| unchanged | 1 |
| regressed | 0 |
| new | 0 |
| removed | 0 |

## Grip Counts

| Class | Before | After |
| --- | ---: | ---: |
| seams_total | 1 | 1 |
| strongly_gripped | 0 | 0 |
| weakly_gripped | 1 | 1 |
| ungripped | 0 | 0 |
| reachable_unrevealed | 0 | 0 |
| activation_unknown | 0 | 0 |
| propagation_unknown | 0 | 0 |
| observation_unknown | 0 | 0 |
| discrimination_unknown | 0 | 0 |
| opaque | 0 | 0 |
| intentional | 0 | 0 |
| suppressed | 0 | 0 |

## Moved

None.

## Unchanged

- `67fc764ba37d77bd` src/lib.rs:2 weakly_gripped -> weakly_gripped (unchanged)
  - new observed value: 100
  - related test count increased by 1

## Regressed

None.

## New

None.

## Removed

None.

This report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.
