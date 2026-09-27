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
