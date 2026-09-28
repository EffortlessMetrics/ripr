# Golden Output Changes

## Pending — ts_boundary_named_constant_test (1)

Reason:
RIPR-SPEC-0027 #4104-E3: new fixture pinning UPPER_CASE named-constant resolution (owner-module export const and test-body const) mirroring value_resolution::named_constant strictness

Command:
`cargo xtask goldens bless ts_boundary_named_constant_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_boundary_named_constant_test (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_boundary_named_constant_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_boundary_named_constant_test (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless ts_boundary_named_constant_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_boundary_named_constant_test (4)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless ts_boundary_named_constant_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
