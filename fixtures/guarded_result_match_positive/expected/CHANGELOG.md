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

## Pending — guarded_result_match_positive (3)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_positive (4)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_positive (5)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_positive (6)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless guarded_result_match_positive --reason "..."`

Updated:
- `expected/human.txt`
