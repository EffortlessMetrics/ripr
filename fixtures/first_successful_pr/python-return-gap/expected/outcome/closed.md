# ripr targeted-test outcome report

Status: advisory

Inputs:
- before: `fixtures/first_successful_pr/python-return-gap/inputs/reports/before-check.json`
- after: `fixtures/first_successful_pr/python-return-gap/inputs/reports/after-check.json`

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

- `gap:python:src/priority.py:is_priority:return_value:return_value:amount>=100` src/priority.py:2 weakly_gripped -> strongly_gripped (improved; gap closed)
  - grip class moved from weakly_gripped to strongly_gripped
  - observe evidence moved from weak to yes
  - discriminate evidence moved from weak to yes
  - missing discriminator no longer reported: return value == amount >= 100 (changed Python returned-value at line 2 lacks a concrete repair discriminator)
  - stronger related oracle visible: smoke -> strong

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

- Compared before snapshot fixtures/first_successful_pr/python-return-gap/inputs/reports/before-check.json with after snapshot fixtures/first_successful_pr/python-return-gap/inputs/reports/after-check.json.
- Static seam movement: 1 moved, 0 unchanged, 0 regressed, 0 new, 0 removed.

### What RIPR flagged before?

- weakly_gripped before return_value at src/priority.py:2.

### What focused proof changed?

- return_value at src/priority.py:2 shows static evidence movement for focused proof outside RIPR: observe evidence moved from weak to yes; discriminate evidence moved from weak to yes; missing discriminator no longer reported: return value == amount >= 100 (changed Python returned-value at line 2 lacks a concrete repair discriminator).

### What moved after verification?

- 1 improved, 0 changed without ranking higher, 0 regressed, 0 unchanged.
- Gap movement: 1 closed, 0 opened, 0 strengthened, 0 weakened, 0 unchanged, 0 new, 0 removed, 0 changed.
- return_value at src/priority.py:2 moved weakly_gripped -> strongly_gripped (improved).

### What remains weak or unknown?

- No weak or unknown after-snapshot seams were present in the compared artifacts.

### Reviewer should inspect

- Open the compared artifacts: fixtures/first_successful_pr/python-return-gap/inputs/reports/before-check.json and fixtures/first_successful_pr/python-return-gap/inputs/reports/after-check.json.
- Inspect the focused test or output proof corresponding to each listed evidence delta.
- Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete.

### Reviewer may believe

- RIPR compared only the listed static snapshots: fixtures/first_successful_pr/python-return-gap/inputs/reports/before-check.json and fixtures/first_successful_pr/python-return-gap/inputs/reports/after-check.json.
- The listed focused-proof signals are static evidence visible after a test or output proof changed outside RIPR.
- The movement and remaining-weak sections define the static claim boundary for this receipt.

### Reviewer should not believe

- Runtime mutation result.
- Coverage adequacy.
- General correctness.
- Merge approval.
- That RIPR edited source or generated tests.


This report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.
