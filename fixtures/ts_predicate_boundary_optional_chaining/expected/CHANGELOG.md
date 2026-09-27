# Golden Output Changes

## Pending — ts_predicate_boundary_optional_chaining (1)

Reason:
RIPR-SPEC-0027 #4104-E2: new fixture pinning optional-chaining (?.), nullish-coalescing (??), and yield-tail predicates as boundary-witnessable through quote-aware operand normalization

Command:
`cargo xtask goldens bless ts_predicate_boundary_optional_chaining --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_optional_chaining (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_predicate_boundary_optional_chaining --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
