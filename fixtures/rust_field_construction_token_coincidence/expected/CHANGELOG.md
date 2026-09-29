# Golden Output Changes

## Pending — rust_field_construction_token_coincidence (1)

Reason:
RIPR-SPEC-0094: initial golden; a same-named field on an unrelated value does not observe the constructed field (#4428)

Command:
`cargo xtask goldens bless rust_field_construction_token_coincidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_field_construction_token_coincidence (2)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless rust_field_construction_token_coincidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_field_construction_token_coincidence (3)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless rust_field_construction_token_coincidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
