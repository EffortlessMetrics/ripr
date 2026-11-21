# Golden Output Changes

## Pending — guarded_result_match_swallowed (1)

Reason:
RIPR-SPEC-0154: initial golden for the #3709 swallowed/wildcard non-crediting control fixture

Command:
`cargo xtask goldens bless guarded_result_match_swallowed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_swallowed (2)

Reason:
RIPR-SPEC-0154: re-bless to settle human-full trailing-format drift after the guarded Result match producer landed (#3709)

Command:
`cargo xtask goldens bless guarded_result_match_swallowed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
