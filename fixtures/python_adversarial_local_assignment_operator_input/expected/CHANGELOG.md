# Golden Output Changes

## Pending

Reason:
new fixture: local-assignment operator change must not credit an unchanged input operand (#1288)

Command:
`cargo xtask goldens bless python_adversarial_local_assignment_operator_input --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
