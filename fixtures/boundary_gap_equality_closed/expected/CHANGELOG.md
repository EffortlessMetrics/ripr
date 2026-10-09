# Golden Output Changes

## Pending — boundary_gap_equality_closed (1)

Reason:
RIPR-SPEC-0001: pin the #1429 case-3 closed variant — same canonical probe as boundary_gap moves to exposed once the exact equality assertion observes the boundary

Command:
`cargo xtask goldens bless boundary_gap_equality_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_gap_equality_closed (2)

Reason:
RIPR-SPEC-0001: Same canonical predicate remains Exposed with existing equality row; remove unverified supplied base and refresh schema/count vocabulary. Independent control: Existing exact equality assertion passes; > mutant fails; paired boundary_gap remains weak.

Command:
`cargo xtask goldens bless boundary_gap_equality_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
