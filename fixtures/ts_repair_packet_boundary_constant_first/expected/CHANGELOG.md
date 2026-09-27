# Golden Output Changes

## Pending — ts_repair_packet_boundary_constant_first (1)

Reason:
RIPR-SPEC-0087/#4215 review: new fixture pins that a constant written first (DISCOUNT_THRESHOLD <= amount) also fails the TS repair packet closed

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
