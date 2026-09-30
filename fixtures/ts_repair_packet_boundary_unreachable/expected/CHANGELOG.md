# Golden Output Changes

## Pending — ts_repair_packet_boundary_unreachable (1)

Reason:
RIPR-SPEC-0087/#4105: new fixture pins that a complete TS repair packet must fail closed and emit a boundary placeholder shape when the observed call input provably cannot reach the named discriminator boundary

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (2)

Reason:
RIPR-SPEC-0087 merge-forward rebless of own fixture ts_repair_packet_boundary_unreachable (#4105): after merging main 106b45ebd, (1) the target-shape placeholder now ends in the shared `expected` placeholder instead of the borrowed literal `.toBe(4)` (main landed the #4105 finding-B placeholder convention in this projection), and (2) the preview-support note wording changed to `1 TypeScript file analyzed` (main renderer wording, formatting only). Boundary placeholder `/* boundary input for user.length == 3 */` and repair_packet_ready:false unchanged.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (3)

Reason:
RIPR-SPEC-0087/#4215: the boundary placeholder shape is now a complete assertion, expect(login(/* boundary input ... */)).toBe(expected); previously it dropped the expect( wrapper. repair_packet_ready:false unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (4)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (5)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (6)

Reason:
RIPR-SPEC-0122: closed-packet safe next action shows the validator's specific cause instead of the generic preview preamble under the line budget (#4216)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (7)

Reason:
RIPR-SPEC-0122 (#4216 final review F3): the closed-packet safe action drops the fixed `is not agent-packet eligible: ` phrase after `validator: `, so the specific cause and its remedy fit the line budget. Only that Safe next action line changes.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (8)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (9)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (10)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (11)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — ts_repair_packet_boundary_unreachable (11)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (12)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/human.txt`
## Pending — ts_repair_packet_boundary_unreachable (11)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_repair_packet_boundary_unreachable (13)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_unreachable --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`