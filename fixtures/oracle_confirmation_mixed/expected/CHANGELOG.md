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
RIPR-SPEC-0122: J cutoff join regeneration; swarm #4379 drill-in lines and digest why-line text plus swarm unreached static_unknown next step applied to ripr-side fixtures; classes and counts unchanged

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — oracle_confirmation_mixed (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); fixture added by #4421 before #4411 landed, so its golden lacked the block

Command:
`cargo xtask goldens bless oracle_confirmation_mixed --reason "..."`

Updated:
- `expected/human-full.txt`
