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
