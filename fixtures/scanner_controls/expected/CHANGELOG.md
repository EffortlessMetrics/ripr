# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0163: new fixture pinning the scanner fail-closed controls (step bound, computed argument, computed next-state arm, bare-identifier arm -> weakly_exposed, no scanner hop)

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (4)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (5)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (6)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (7)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/human.txt`

## Pending — scanner_controls (8)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`