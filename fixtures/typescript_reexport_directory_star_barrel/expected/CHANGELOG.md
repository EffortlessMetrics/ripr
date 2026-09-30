# Golden Output Changes

## Pending — typescript_reexport_directory_star_barrel (1)

Reason:
RIPR-SPEC-0095 bounded re-export chains: new unjs/ufo regression fixture; a directory import of a star barrel (import from ../src, src/index.ts export star from ./utils) credits the changed withoutBase via re_export_chain_followed; initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_directory_star_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_directory_star_barrel (2)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity (#4429, merged from main)

Command:
`cargo xtask goldens bless typescript_reexport_directory_star_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_directory_star_barrel (3)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.

Command:
`cargo xtask goldens bless typescript_reexport_directory_star_barrel --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_reexport_directory_star_barrel (4)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless typescript_reexport_directory_star_barrel --reason "..."`

Updated:
- `expected/human.txt`
