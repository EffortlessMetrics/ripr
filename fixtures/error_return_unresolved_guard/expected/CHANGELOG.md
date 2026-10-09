# Golden Output Changes

## Pending — error_return_unresolved_guard (1)

Reason:
RIPR-SPEC-0158 #1579: bless precise guard-unresolved limitation output (infection_unknown, typed limitation, no impossible prescription)

Command:
`cargo xtask goldens bless error_return_unresolved_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_return_unresolved_guard (2)

Reason:
RIPR-SPEC-0096: Corrected hunk attributes the actual return line16 to ErrorPath, FieldConstruction and ReturnValue. All retain infection Unknown with unresolved before-constructor evidence, not equivalence or literal-boundary credit. Independent control: Actual fixture test passes; e504 isolated constructor mutant assertion failure; current classifier guarded/literal/unrelated-owner controls.

Command:
`cargo xtask goldens bless error_return_unresolved_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
