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
