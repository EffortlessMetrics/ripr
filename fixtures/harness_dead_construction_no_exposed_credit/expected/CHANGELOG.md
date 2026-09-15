# Golden Output Changes

## Pending — harness_dead_construction_no_exposed_credit (1)

Reason:
RIPR-SPEC-0173: #3636 honesty-corpus charter fixture — registered libtest-mimic harness whose dead-construction trials reference the changed production functions; expected outputs generated from the current analyzer on main 470b5acb1 with the reachability authority active, pinning all findings below exposed.

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — harness_dead_construction_no_exposed_credit (2)

Reason:
RIPR-SPEC-0173: regenerate after red-under-corruption verification of the #3636 honesty-corpus case; restores the true golden output (all findings no_static_path) for the registered-harness dead-construction fixture.

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — harness_dead_construction_no_exposed_credit (3)

Reason:
RIPR-SPEC-0173: regenerate the #3636 corpus golden after trimming trailing whitespace from diff.patch (git diff --check); re-embeds the input_identity hash of the committed patch bytes.

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
