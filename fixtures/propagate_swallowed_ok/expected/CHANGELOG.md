# Golden Output Changes

## Pending

Reason:
initial golden for fix B: swallowed ok tail → propagation_unknown

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): confidence 0.87->0.79 refresh; classification propagation_unknown unchanged

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
