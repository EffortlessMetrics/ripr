# Golden Output Changes

## Pending — typescript_reexport_barrel_other_name (1)

Reason:
RIPR-SPEC-0095 bounded re-export chains: new barrel negative control; the test imports only formatCents from the barrel, so the changed applyDiscount forwarded by the same barrel stays no_static_path with 0 related tests; initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_barrel_other_name (2)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy (#4323, main merge)
RIPR-SPEC-0122: human lines lead with the plain word the check summary uses; schema value kept beside it

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_barrel_other_name (3)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_barrel_other_name (4)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/human.txt`

## Pending — typescript_reexport_barrel_other_name (5)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless typescript_reexport_barrel_other_name --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`