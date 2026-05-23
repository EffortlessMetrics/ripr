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

## Review Receipt

### What changed?

- Compared before snapshot fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json with after snapshot fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json.
- Static seam movement: 0 moved, 1 unchanged, 0 regressed, 0 new, 0 removed.

### What RIPR flagged before?

- weakly_gripped before predicate_boundary at src/lib.rs:2.

### What focused proof changed?

- predicate_boundary at src/lib.rs:2 shows static evidence movement for focused proof outside RIPR: new observed value: 100; related test count increased by 1.

### What moved after verification?

- 0 improved, 0 changed without ranking higher, 0 regressed, 1 unchanged.
- predicate_boundary at src/lib.rs:2 kept weakly_gripped but evidence changed: new observed value: 100; related test count increased by 1.

### What remains weak or unknown?

- predicate_boundary remains weakly_gripped at src/lib.rs:2.

### Reviewer should inspect

- Open the compared artifacts: fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json and fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json.
- Inspect the focused test or output proof corresponding to each listed evidence delta.
- Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete.

### Reviewer may believe

- RIPR compared only the listed static snapshots: fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json and fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json.
- The listed focused-proof signals are static evidence visible after a test or output proof changed outside RIPR.
- The movement and remaining-weak sections define the static claim boundary for this receipt.

### Reviewer should not believe

- Runtime mutation result.
- Coverage adequacy.
- General correctness.
- Merge approval.
- That RIPR edited source or generated tests.


This report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.
