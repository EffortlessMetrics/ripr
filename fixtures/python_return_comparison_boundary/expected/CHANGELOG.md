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

## Pending — python_return_comparison_boundary (3)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless python_return_comparison_boundary --reason "..."`

Updated:
- `expected/human.txt`
