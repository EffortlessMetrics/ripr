# Golden Output Changes

## Pending — ts_return_comparison_boundary (1)

Reason:
RIPR-SPEC-0027/0028: a returned relational comparison is a predicate probe on its boundary, not a return_value probe

Command:
`cargo xtask goldens bless ts_return_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
