# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0164: literal match-arm helper transfer flips the caller boundary to exposed with hop provenance

Command:
`cargo xtask goldens bless match_arm_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_positive (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless match_arm_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
