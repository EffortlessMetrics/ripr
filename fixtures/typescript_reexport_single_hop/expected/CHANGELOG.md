# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0095 single-hop re-export tracing: test imports isRawNetworkError via src/index.ts which re-exports from src/util.ts (one explicit hop); ripr now credits the test with relation_reason=re_export_chain_followed, class=exposed

Command:
`cargo xtask goldens bless typescript_reexport_single_hop --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests; new fixtures: typescript_reexport_single_hop (credit), typescript_reexport_no_false_credit (no-false-credit control), typescript_reexport_two_hop_limit (two-hop control); initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_single_hop --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
