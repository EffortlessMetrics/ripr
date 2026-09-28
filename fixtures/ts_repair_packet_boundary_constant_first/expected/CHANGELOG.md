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

## Pending — ts_repair_packet_boundary_constant_first (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_first (3)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_first (4)

Reason:
RIPR-SPEC-0122: closed-packet safe next action shows the validator's specific cause instead of the generic preview preamble under the line budget (#4216)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_first (5)

Reason:
RIPR-SPEC-0122 (#4216 final review F3): the closed-packet safe action drops the fixed `is not agent-packet eligible: ` phrase after `validator: `, so the specific cause and its remedy fit the line budget. Only that Safe next action line changes.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_first (6)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_first (7)

Reason:
RIPR-SPEC-0122: digest lines never end inside an open code span

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_first --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
