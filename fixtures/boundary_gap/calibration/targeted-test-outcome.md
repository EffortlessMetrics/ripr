# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json`
- after: `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json`

## Summary

| Bucket | Count |
| --- | ---: |
| moved | 1 |
| unchanged | 0 |
| regressed | 0 |
| new | 0 |
| removed | 0 |

## Gap Movement

| Movement | Count |
| --- | ---: |
| closed | 1 |
| opened | 0 |
| strengthened | 0 |
| weakened | 0 |
| unchanged | 0 |
| new | 0 |
| removed | 0 |
| changed | 0 |

## Grip Counts

| Class | Before | After |
| --- | ---: | ---: |
| seams_total | 1 | 1 |
| strongly_gripped | 0 | 1 |
| weakly_gripped | 1 | 0 |
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

- `67fc764ba37d77bd` src/lib.rs:2 weakly_gripped -> strongly_gripped (improved; gap closed)
  - grip class moved from weakly_gripped to strongly_gripped
  - missing discriminator no longer reported: discount_threshold (equality boundary) (observed values do not include the equality-boundary case for this predicate)
  - new observed value: 100
  - related test count increased by 1

## Unchanged

None.

## Regressed

None.

## New

None.

## Removed

None.

## Review Receipt

### Gap movement summary

- Gap movement: 1 closed, 0 opened, 0 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.

### What changed?

- Compared before snapshot fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json with after snapshot fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json.
- Static seam movement: 1 moved, 0 unchanged, 0 regressed, 0 new, 0 removed.

### What RIPR flagged before?

- weakly_gripped before predicate_boundary at src/lib.rs:2.

### What focused proof changed?

- predicate_boundary at src/lib.rs:2 shows static evidence movement for focused proof outside RIPR: missing discriminator no longer reported: discount_threshold (equality boundary) (observed values do not include the equality-boundary case for this predicate); new observed value: 100; related test count increased by 1.

### What moved after verification?

- 1 improved, 0 changed without ranking higher, 0 regressed, 0 unchanged.
- Gap movement: 1 closed, 0 opened, 0 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.
- predicate_boundary at src/lib.rs:2 moved weakly_gripped -> strongly_gripped (improved).

### What remains weak or unknown?

- No weak or unknown after-snapshot seams were present in the compared artifacts.

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
