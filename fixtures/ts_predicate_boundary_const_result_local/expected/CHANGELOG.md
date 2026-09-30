# Golden Output Changes

## Pending — ts_predicate_boundary_const_result_local (1)

Reason:
RIPR-SPEC-0027 #4104-E1: new fixture pinning the one-hop const-result local-binding idiom (const result = applyDiscount(100); expect(result).toBe(0.9)) witnessing the changed predicate boundary as exposed

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_const_result_local (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_const_result_local (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_const_result_local (4)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_const_result_local (5)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_const_result_local (6)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless ts_predicate_boundary_const_result_local --reason "..."`

Updated:
- `expected/human.txt`
