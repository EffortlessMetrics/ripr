# Golden Output Changes

## Pending

Reason:
new fixture: a changed default value is not exposed when every strong related call overrides the parameter (#1289 trap 45)

Command:
`cargo xtask goldens bless python_adversarial_default_value_overridden --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Populate structured missing_discriminators for the changed-default override downgrade (#1289 trap 45 review fix): the def-header path previously emitted an empty missing_discriminators array, so the repair card and recommended_next_step lost the actionable 'call render without verbose' omission guidance. The downgrade now emits a structured MissingDiscriminatorFact that propagates into the repair card, recommended_next_step, and suggested_assertion.

Command:
`cargo xtask goldens bless python_adversarial_default_value_overridden --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_default_value_overridden --reason "..."`

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
`cargo xtask goldens bless python_adversarial_default_value_overridden --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
