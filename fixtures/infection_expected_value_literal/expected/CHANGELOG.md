# Golden Output Changes

## Pending — infection_expected_value_literal (1)

Reason:
RIPR-SPEC-0001: new fixture pinning that a boundary literal counts as infection evidence only when it is an input of the changed owner (an assertion's expected value is an oracle), with a let-bound owner input as the positive control

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (2)

Reason:
RIPR-SPEC-0001: regenerate own-fixture golden after main's fixture harness stopped injecting the synthetic base field; drift is formatting_only, summary/counts/findings unchanged (1 exposed, 1 weakly_exposed oracle-only control intact)

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (4)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
