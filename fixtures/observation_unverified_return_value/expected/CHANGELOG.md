# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094: ReturnValue probe with no token_match now emits weakly_exposed/observation_unverified (was exposed/1.00 via single-assertion escape hatch); initial golden for the escape-hatch proof fixture

Command:
`cargo xtask goldens bless observation_unverified_return_value --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
