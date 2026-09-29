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

## Pending — harness_dead_construction_no_exposed_credit (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — harness_dead_construction_no_exposed_credit (5)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — harness_dead_construction_no_exposed_credit (6)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — harness_dead_construction_no_exposed_credit (7)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — harness_dead_construction_no_exposed_credit (8)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless harness_dead_construction_no_exposed_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
