# Golden Output Changes

## Pending — guarded_result_match_sibling_variant (1)

Reason:
RIPR-SPEC-0175: #3731 review fix F2 — sibling-variant guarded pin must not confirm a variant-carrying probe; new fixture pins the non-promotion (both probes weakly_exposed)

Command:
`cargo xtask goldens bless guarded_result_match_sibling_variant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
