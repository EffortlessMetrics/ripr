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
