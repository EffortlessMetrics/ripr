# Golden Output Changes

## Pending — rust_transitive_reach_same_name_other_type (1)

Reason:
RIPR-SPEC-0115: a unit test calling an unrelated type's same-named method no longer outranks the integration witness (#5481)

Command:
`cargo xtask goldens bless rust_transitive_reach_same_name_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_transitive_reach_same_name_other_type (2)

Reason:
RIPR-SPEC-0115: pin the named witness in full human output (#5481)

Command:
`cargo xtask goldens bless rust_transitive_reach_same_name_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
