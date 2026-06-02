# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/first_successful_pr/python-preview-gap/inputs/reports/no-path-check.json`
- after: `fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json`

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
| closed | 0 |
| opened | 0 |
| strengthened | 1 |
| weakened | 0 |
| unchanged | 0 |
| new | 0 |
| removed | 0 |
| changed | 0 |

## Grip Counts

| Class | Before | After |
| --- | ---: | ---: |
| seams_total | 1 | 1 |
| strongly_gripped | 0 | 0 |
| weakly_gripped | 0 | 1 |
| ungripped | 1 | 0 |
| reachable_unrevealed | 0 | 0 |
| activation_unknown | 0 | 0 |
| propagation_unknown | 0 | 0 |
| observation_unknown | 0 | 0 |
| discrimination_unknown | 0 | 0 |
| opaque | 0 | 0 |
| intentional | 0 | 0 |
| suppressed | 0 | 0 |

## Moved

- `gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold` app/pricing.py:2 ungripped -> weakly_gripped (improved; gap improved)
  - grip class moved from ungripped to weakly_gripped
  - reach evidence moved from no to yes
  - observe evidence moved from missing to weak
  - discriminate evidence moved from missing to weak
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

- Gap movement: 0 closed, 0 opened, 1 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.

### What changed?

- Compared before snapshot fixtures/first_successful_pr/python-preview-gap/inputs/reports/no-path-check.json with after snapshot fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- Static seam movement: 1 moved, 0 unchanged, 0 regressed, 0 new, 0 removed.

### What RIPR flagged before?

- ungripped before predicate_boundary at app/pricing.py:2.

### What focused proof changed?

- predicate_boundary at app/pricing.py:2 shows static evidence movement for focused proof outside RIPR: related test count increased by 1.

### What moved after verification?

- 1 improved, 0 changed without ranking higher, 0 regressed, 0 unchanged.
- Gap movement: 0 closed, 0 opened, 1 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.
- predicate_boundary at app/pricing.py:2 moved ungripped -> weakly_gripped (improved).

### What remains weak or unknown?

- predicate_boundary remains weakly_gripped at app/pricing.py:2.

### Reviewer should inspect

- Open the compared artifacts: fixtures/first_successful_pr/python-preview-gap/inputs/reports/no-path-check.json and fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- Inspect the focused test or output proof corresponding to each listed evidence delta.
- Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete.

### Reviewer may believe

- RIPR compared only the listed static snapshots: fixtures/first_successful_pr/python-preview-gap/inputs/reports/no-path-check.json and fixtures/first_successful_pr/python-preview-gap/inputs/reports/before-check.json.
- The listed focused-proof signals are static evidence visible after a test or output proof changed outside RIPR.
- The movement and remaining-weak sections define the static claim boundary for this receipt.

### Reviewer should not believe

- Runtime mutation result.
- Coverage adequacy.
- General correctness.
- Merge approval.
- That RIPR edited source or generated tests.


This report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.
