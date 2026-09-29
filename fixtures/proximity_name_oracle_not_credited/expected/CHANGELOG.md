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
