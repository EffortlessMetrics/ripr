# Golden Output Changes

## Pending

Reason:
initial golden for fix A control: named binding stays exposed

Command:
`cargo xtask goldens bless infect_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216 observation_unverified): strengthened A control test to reference MULTIPLIER token so the named-binding probe stays exposed/infect=yes

Command:
`cargo xtask goldens bless infect_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
