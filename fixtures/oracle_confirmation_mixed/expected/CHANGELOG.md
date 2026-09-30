# Golden Output Changes

## Pending — oracle_confirmation_mixed (1)

Reason:
RIPR-SPEC-0094 (#4404): unrelated strongest exact oracle must not borrow weaker token confirmation; valid before/head/call-removal variants each pass two tests.

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — oracle_confirmation_mixed (2)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); fixture added by #4421 before #4411 landed, so its golden lacked the block

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — oracle_confirmation_mixed (3)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — oracle_confirmation_mixed (4)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — oracle_confirmation_mixed (5)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/human.txt`
