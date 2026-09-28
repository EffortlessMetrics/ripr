# Golden Output Changes

## Pending — guarded_result_match_sibling_variant (1)

Reason:
RIPR-SPEC-0175: #3731 review fix F2 — sibling-variant guarded pin must not confirm a variant-carrying probe; new fixture pins the non-promotion (both probes weakly_exposed)

Command:
`cargo xtask goldens bless guarded_result_match_sibling_variant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_sibling_variant (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless guarded_result_match_sibling_variant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_sibling_variant (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless guarded_result_match_sibling_variant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
