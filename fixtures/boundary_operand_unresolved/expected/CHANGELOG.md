# Golden Output Changes

## Pending — boundary_operand_unresolved (1)

Reason:
RIPR-SPEC-0158: new fixture pinning the #1429 historical rfind/len_utf8 equality shape to weakly_exposed plus the typed rust_value_propagation_unresolved limitation with no boundary-test prescription

Command:
`cargo xtask goldens bless boundary_operand_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_operand_unresolved (2)

Reason:
RIPR-SPEC-0158: record human-full in updated artifacts for 1429 limitation fixture

Command:
`cargo xtask goldens bless boundary_operand_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — boundary_operand_unresolved (3)

Reason:
RIPR-SPEC-0158: #1724 review corrections to unresolved-operand evidence text (char-literal argument labels, complete try-operator edge)

Command:
`cargo xtask goldens bless boundary_operand_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
