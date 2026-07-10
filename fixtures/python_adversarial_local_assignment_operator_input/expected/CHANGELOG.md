# Golden Output Changes

## Pending

Reason:
new fixture: local-assignment operator change must not credit an unchanged input operand (#1288)

Command:
`cargo xtask goldens bless python_adversarial_local_assignment_operator_input --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_local_assignment_operator_input --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`
