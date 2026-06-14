# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094 Part B: Mode::Warm assertion does NOT confirm Mode::Frozen arm (qualifier-blind hole); initial golden locking the variant-scope fix

Command:
`cargo xtask goldens bless match_arm_type_token_blind --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
