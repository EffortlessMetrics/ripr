# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094: CallDeletion probe with no token_match emits weakly_exposed/observation_unverified; initial golden for the escape-hatch proof fixture

Command:
`cargo xtask goldens bless observation_unverified_call_deletion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
