# Golden Output Changes

## Pending

Reason:
new fixture: empty-delta operator change must not credit an unchanged input operand (#1278)

Command:
`cargo xtask goldens bless python_adversarial_operator_delta_input_operand --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
