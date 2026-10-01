# Golden Output Changes

## Pending — rust_adversarial_same_method_other_type (1)

Reason:
RIPR-SPEC-0108: pin #4760 same-method-other-impl false-exposed guard below exposed

Command:
`cargo xtask goldens bless rust_adversarial_same_method_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_adversarial_same_method_other_type (2)

Reason:
RIPR-SPEC-0108: include human-full for #4760 same-method-other-impl class projection

Command:
`cargo xtask goldens bless rust_adversarial_same_method_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_adversarial_same_method_other_type (3)

Reason:
RIPR-SPEC-0108: restore per-assertion related_tests under the 8-row cap for #4760

Command:
`cargo xtask goldens bless rust_adversarial_same_method_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_adversarial_same_method_other_type (4)

Reason:
merge origin/main: RIPR-SPEC-0122 renderer (#4320 windows, #4322 canonical summary tokens, #4324 stage Evidence line) re-renders these #4760 fixtures; diff.patch declared hunk span corrected per #4439 so input_identity updates; classifications and counts unchanged

Command:
`cargo xtask goldens bless rust_adversarial_same_method_other_type --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
