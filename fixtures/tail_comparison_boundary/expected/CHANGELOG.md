# Golden Output Changes

## Pending — tail_comparison_boundary (1)

Reason:
RIPR-SPEC-0001: new fixture pinning that a comparison which is the whole tail of a -> bool owner propagates to the returned value, so a wrapper-reached threshold names its missing equality boundary (items == 10) and an on-boundary test is exposed

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (#4002); this fixture's golden was blessed on a base before that change, so --diff envelopes now omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed.

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (4)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (5)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (6)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (7)

Reason:
RIPR-SPEC-0021: emitted related tests keep relation-confidence order so the primary Related test is the strongest relation

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (8)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/human.txt`
