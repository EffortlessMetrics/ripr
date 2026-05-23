# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json`
- after: `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json`

## Reviewer Receipt

- What changed: Compared `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json` to `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json`: 0 moved, 1 unchanged, 0 regressed, 0 new, 0 removed.
- What RIPR flagged before: `67fc764ba37d77bd` at src/lib.rs:2 was `weakly_gripped` before verification.
- Focused proof observed: The after snapshot shows static evidence movement such as `new observed value: 100`; any test or output proof was added outside RIPR.
- Static movement: 1 seam(s) stayed in the same static class while rendered evidence changed.
- What remains weak or unknown:
  - 1 seam(s) stayed unchanged after the supplied after snapshot.
- Reviewer should believe:
  - RIPR compared the two supplied static repo-exposure artifacts.
  - 1 seam(s) stayed in the same static class while rendered evidence changed.
  - Evidence deltas describe static movement in rendered artifacts only.
- Reviewer should not believe:
  - RIPR did not edit source or generate tests.
  - This receipt is not runtime confirmation or mutation confirmation.
  - This receipt is not merge approval, gate authority, or coverage completeness.

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
