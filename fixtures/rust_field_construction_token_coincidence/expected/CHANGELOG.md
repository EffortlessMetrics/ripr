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

## Pending — rust_field_construction_token_coincidence (4)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless rust_field_construction_token_coincidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_field_construction_token_coincidence (5)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless rust_field_construction_token_coincidence --reason "..."`

Updated:
- `expected/human.txt`
