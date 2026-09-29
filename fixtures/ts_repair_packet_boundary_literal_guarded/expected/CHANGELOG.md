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
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy (#4323, main merge)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_literal_guarded --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
