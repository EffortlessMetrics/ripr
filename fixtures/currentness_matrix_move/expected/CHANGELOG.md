# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0156: initial currentness-matrix corpus fixture (#3282)

Command:
`cargo xtask goldens bless currentness_matrix_move --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0156: corrected hunk header line counts (header/body mismatch only; dispositions unchanged)

Command:
`cargo xtask goldens bless currentness_matrix_move --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless currentness_matrix_move --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — currentness_matrix_move (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless currentness_matrix_move --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — currentness_matrix_move (3)

Reason:
RIPR-SPEC-0001 (#4216 row 5): brace-only and else-only changed lines (`}`, `} else {`) no longer seed static_unknown probes. Only those findings are removed; every remaining finding is byte-identical, and summary/outcome counts drop by the removed count.

Command:
`cargo xtask goldens bless currentness_matrix_move --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
