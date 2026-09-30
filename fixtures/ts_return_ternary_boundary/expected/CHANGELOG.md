# Golden Output Changes

## Pending — ts_return_ternary_boundary (1)

Reason:
RIPR-SPEC-0027: a returned ternary is a predicate probe on its condition, not a return_value probe

Command:
`cargo xtask goldens bless ts_return_ternary_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_return_ternary_boundary (2)

Reason:
RIPR-SPEC-0027: a returned ternary is a predicate probe on its condition, not a return_value probe

Command:
`cargo xtask goldens bless ts_return_ternary_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_return_ternary_boundary (3)

Reason:
RIPR-SPEC-0027: re-bless after main added source_subject to check output; classification unchanged

Command:
`cargo xtask goldens bless ts_return_ternary_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_return_ternary_boundary (4)

Reason:
RIPR-SPEC-0122: plain-word Analysis outcome and State lines (#4777) on a fixture added on main

Command:
`cargo xtask goldens bless ts_return_ternary_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_return_ternary_boundary (5)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless ts_return_ternary_boundary --reason "..."`

Updated:
- `expected/human.txt`
