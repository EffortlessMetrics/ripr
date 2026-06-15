# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0095 no-false-credit control: src/index.ts re-exports isRawNetworkError from src/other.ts (NOT the changed owner in src/util.ts); single-hop trace resolves to a different source file, so the test correctly stays uncredited (no_static_path, 0 related tests)

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests; new fixtures: typescript_reexport_single_hop (credit), typescript_reexport_no_false_credit (no-false-credit control), typescript_reexport_two_hop_limit (two-hop control); initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
