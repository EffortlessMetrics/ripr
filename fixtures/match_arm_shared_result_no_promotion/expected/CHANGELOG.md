# Golden Output Changes

## Pending — match_arm_shared_result_no_promotion (1)

Reason:
RIPR-SPEC-0108: new must-not-promote fixture for match-arm shared-result false exposure

Command:
`cargo xtask goldens bless match_arm_shared_result_no_promotion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_shared_result_no_promotion (2)

Reason:
RIPR-SPEC-0108: refresh human-full golden for match-arm shared-result must-not-promote fixture

Command:
`cargo xtask goldens bless match_arm_shared_result_no_promotion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — match_arm_shared_result_no_promotion (3)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless match_arm_shared_result_no_promotion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — match_arm_shared_result_no_promotion (4)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless match_arm_shared_result_no_promotion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
