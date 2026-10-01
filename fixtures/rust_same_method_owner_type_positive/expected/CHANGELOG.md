# Golden Output Changes

## Pending — rust_same_method_owner_type_positive (1)

Reason:
RIPR-SPEC-0108: pin #4760 owner-type receiver identity keeps exposed

Command:
`cargo xtask goldens bless rust_same_method_owner_type_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_same_method_owner_type_positive (2)

Reason:
RIPR-SPEC-0108: include human-full for #4760 owner-type positive control

Command:
`cargo xtask goldens bless rust_same_method_owner_type_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — rust_same_method_owner_type_positive (3)

Reason:
merge origin/main: RIPR-SPEC-0122 renderer (#4320 windows, #4322 canonical summary tokens, #4324 stage Evidence line) re-renders these #4760 fixtures; diff.patch declared hunk span corrected per #4439 so input_identity updates; classifications and counts unchanged

Command:
`cargo xtask goldens bless rust_same_method_owner_type_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
