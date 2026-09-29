# Golden Output Changes

## Pending — ts_repair_packet_boundary_constant_unresolved (1)

Reason:
RIPR-SPEC-0087/#4215: new fixture pins that a TS repair packet fails closed with a boundary placeholder when the discriminator's boundary is an unresolved named constant and no observed argument names it

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (2)

Reason:
RIPR-SPEC-0087/#4215: pin the human-full target shape placeholder for the unresolved named-constant boundary

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (3)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (4)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (5)

Reason:
RIPR-SPEC-0122: closed-packet safe next action shows the validator's specific cause instead of the generic preview preamble under the line budget (#4216)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (6)

Reason:
RIPR-SPEC-0122 (#4216 final review F3): the closed-packet safe action drops the fixed `is not agent-packet eligible: ` phrase after `validator: `, so the specific cause and its remedy fit the line budget. Only that Safe next action line changes.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (7)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (8)

Reason:
RIPR-SPEC-0122: digest lines never end inside an open code span

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (9)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_constant_unresolved (10)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_constant_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
