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

## Pending — guarded_result_match_swallowed (3)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless guarded_result_match_swallowed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_swallowed (4)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless guarded_result_match_swallowed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
