# Golden Output Changes

## Pending — match_arm_tuple_derived_return_before_assertion (1)

Reason:
RIPR-SPEC-0093/RIPR-SPEC-0108: #4068 review — new fixture pins the derived tuple-arm return-boundary non-promotion (relation assertion after a direct return stays weakly_exposed)

Command:
`cargo xtask goldens bless match_arm_tuple_derived_return_before_assertion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
