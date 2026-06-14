# Golden Output Changes

## Pending

Reason:
initial golden for fix B control: returned call stays weakly_exposed

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): discriminate message now observation_unverified; classification weakly_exposed unchanged

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
