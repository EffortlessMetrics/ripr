# Golden Output Changes

## Pending — python_return_comparison_boundary (1)

Reason:
RIPR-SPEC-0027/0028: a returned relational comparison is a predicate probe on its boundary, not a return_value probe

Command:
`cargo xtask goldens bless python_return_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — python_return_comparison_boundary (2)

Reason:
RIPR-SPEC-0122: plain-word Analysis outcome and State lines (#4777) on a fixture added on main

Command:
`cargo xtask goldens bless python_return_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
