# Golden Output Changes

## Pending — guarded_result_match_positive (1)

Reason:
RIPR-SPEC-0154: initial golden for the #3709 guarded-Result-match routing positive fixture

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_positive (2)

Reason:
RIPR-SPEC-0154: input_identity-only rebless after stripping the trailing space from the empty context line in diff.patch (repo whitespace gate); classifications unchanged

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
