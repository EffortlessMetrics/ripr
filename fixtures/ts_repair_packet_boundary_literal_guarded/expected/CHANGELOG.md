# Golden Output Changes

## Pending — ts_repair_packet_boundary_literal_guarded (1)

Reason:
RIPR-SPEC-0087: guarded boundary stays non-delegatable (source challenge on #4429)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_literal_guarded --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_literal_guarded (2)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy (#4323, main merge)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_literal_guarded --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_literal_guarded (3)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_literal_guarded --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_literal_guarded (4)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_literal_guarded --reason "..."`

Updated:
- `expected/human.txt`
