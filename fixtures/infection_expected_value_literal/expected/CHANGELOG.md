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

## Pending — infection_expected_value_literal (5)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (6)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (7)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (8)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — infection_expected_value_literal (9)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless infection_expected_value_literal --reason "..."`

Updated:
- `expected/human.txt`
