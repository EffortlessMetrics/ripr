# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/first_successful_pr/python-preview-gap/inputs/reports/after-check.json`
- after: `fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json`

## Summary

| Bucket | Count |
| --- | ---: |
| moved | 0 |
| unchanged | 0 |
| regressed | 1 |
| new | 0 |
| removed | 0 |

## Gap Movement

| Movement | Count |
| --- | ---: |
| closed | 0 |
| opened | 1 |
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
| strongly_gripped | 1 | 0 |
| weakly_gripped | 0 | 1 |
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

None.

## Regressed

- `gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold` app/pricing.py:2 strongly_gripped -> weakly_gripped (regressed; gap opened)
  - grip class moved from strongly_gripped to weakly_gripped
  - observe evidence moved from yes to weak
  - discriminate evidence moved from yes to weak
  - new missing discriminator reported: amount == threshold (equality boundary not observed)
  - related oracle strength decreased: strong -> unknown

## New

None.

## Removed

None.

## Review Receipt

### Gap movement summary

- Gap movement: 0 closed, 1 opened, 0 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.

### What changed?

- Compared before snapshot fixtures/first_successful_pr/python-preview-gap/inputs/reports/after-check.json with after snapshot fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- Static seam movement: 0 moved, 0 unchanged, 1 regressed, 0 new, 0 removed.

### What RIPR flagged before?

- No before-snapshot weak or unknown seams were present in the compared artifacts.

### What focused proof changed?

- No focused proof signal from a test or output proof outside RIPR was visible in the rendered static snapshots.

### What moved after verification?

- 0 improved, 0 changed without ranking higher, 1 regressed, 0 unchanged.
- Gap movement: 0 closed, 1 opened, 0 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.
- predicate_boundary at app/pricing.py:2 moved strongly_gripped -> weakly_gripped (regressed).

### What remains weak or unknown?

- predicate_boundary remains weakly_gripped at app/pricing.py:2.

### Reviewer should inspect

- Open the compared artifacts: fixtures/first_successful_pr/python-preview-gap/inputs/reports/after-check.json and fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- Inspect the focused test or output proof corresponding to each listed evidence delta.
- Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete.

### Reviewer may believe

- RIPR compared only the listed static snapshots: fixtures/first_successful_pr/python-preview-gap/inputs/reports/after-check.json and fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- No focused-proof signal was visible; this receipt only records before/after static movement.
- The movement and remaining-weak sections define the static claim boundary for this receipt.

### Reviewer should not believe

- Runtime mutation result.
- Coverage adequacy.
- General correctness.
- Merge approval.
- That RIPR edited source or generated tests.


This report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.
