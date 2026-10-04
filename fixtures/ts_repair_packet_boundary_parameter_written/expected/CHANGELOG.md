# Golden Output Changes

## Pending — ts_repair_packet_boundary_parameter_written (1)

Reason:
RIPR-SPEC-0087: judge a parameter-pair boundary through typescript_boundary_parameters evidence and fail closed without it (#4759)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_written --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_written (2)

Reason:
absorb main on the merged tree: repair packet schema_version 0.4 to 0.5, the source_subject stamp (#4544), and the RIPR-SPEC-0122 plain-word summary, outcome, state, window and evidence-line wording; the written-parameter fail-closed packet is unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_written --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_written (3)

Reason:
RIPR-SPEC-0085: TypeScript verify commands launch the framework through the package runner (npx --no-install, pnpm exec, yarn, bun run) because node_modules/.bin is not on PATH

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_written --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
