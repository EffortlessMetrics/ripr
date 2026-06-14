# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0095 two-hop limitation: test imports via index.ts->errors.ts->util.ts (two hops); ripr only resolves one hop (fail-closed), so the test honestly stays uncredited (no_static_path, 0 related tests); disclosed limitation

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests; new fixtures: typescript_reexport_single_hop (credit), typescript_reexport_no_false_credit (no-false-credit control), typescript_reexport_two_hop_limit (two-hop control); initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_two_hop_limit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
