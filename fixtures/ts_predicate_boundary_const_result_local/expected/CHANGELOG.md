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
