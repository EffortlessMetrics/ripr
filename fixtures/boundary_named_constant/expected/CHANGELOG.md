# Golden Output Changes

## Pending — boundary_named_constant (1)

Reason:
RIPR-SPEC-0001: new fixture pinning named-constant boundaries: a same-file integer const resolves to its value, a test argument naming the const matches by identity, and an unresolvable const is reported as unknown instead of a missing discriminator

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (2)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (3)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (4)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (5)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (6)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (7)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/human.txt`

## Pending — boundary_named_constant (8)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`