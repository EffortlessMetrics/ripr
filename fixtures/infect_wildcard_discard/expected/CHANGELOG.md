# Golden Output Changes

## Pending

Reason:
initial golden for fix A: wildcard-discard → infection_unknown

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): confidence/discriminate-message refresh; classification infection_unknown unchanged

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
