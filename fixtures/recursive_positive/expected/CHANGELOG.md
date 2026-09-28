# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0165: bounded recursive helper evaluation flips the caller boundary to exposed with hop provenance

Command:
`cargo xtask goldens bless recursive_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — recursive_positive (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless recursive_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
