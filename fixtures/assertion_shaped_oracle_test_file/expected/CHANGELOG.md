# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0133: assertion-shaped owner reframes guidance for oracles (new fixture)

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR #2170 review: oracle helper input made internally consistent (asserts absence of foreign separator, exact-equality operand avoids phantom Some() return sink); class and reframed guidance unchanged

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
