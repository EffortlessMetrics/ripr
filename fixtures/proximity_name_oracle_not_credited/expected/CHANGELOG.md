# Golden Output Changes

## Pending — proximity_name_oracle_not_credited (1)

Reason:
RIPR-SPEC-0094 / #4486: new fixture; a test related only by name proximity cannot supply the credited oracle when another related test supplies reach

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (2)

Reason:
RIPR-SPEC-0094 / #4486: adds an owner-named test (try_parse_variant_is_distinct) that never calls try_parse; like the token-named test it stays listed but cannot supply the credited oracle

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (3)

Reason:
RIPR-SPEC-0094: re-bless after main's #4520 renamed the human exposure line to lead with the one-word gap name; classification unchanged

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (4)

Reason:
RIPR-SPEC-0021: emitted related tests keep relation-confidence order so the direct caller leads over name-only relations

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (5)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (6)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/human.txt`

## Pending — proximity_name_oracle_not_credited (7)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless proximity_name_oracle_not_credited --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`