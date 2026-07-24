# Golden Output Changes

## Pending

Reason:
new fixture: empty-delta operator change must not credit an unchanged input operand (#1278)

Command:
`cargo xtask goldens bless python_adversarial_operator_delta_input_operand --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_operator_delta_input_operand --reason "..."`

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

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless python_adversarial_operator_delta_input_operand --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
