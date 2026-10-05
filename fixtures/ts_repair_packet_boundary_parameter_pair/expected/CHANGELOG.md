# Golden Output Changes

## Pending — ts_repair_packet_boundary_parameter_pair (1)

Reason:
RIPR-SPEC-0087: judge a parameter-pair boundary through typescript_boundary_parameters evidence and fail closed without it (#4759)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_pair --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_pair (2)

Reason:
absorb main on the merged tree: repair packet schema_version 0.4 to 0.5, the source_subject stamp (#4544), and the RIPR-SPEC-0122 plain-word summary, outcome, state, window and evidence-line wording; the #4759 derived boundary shape is unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_pair --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_pair (3)

Reason:
RIPR-SPEC-0085: TypeScript verify commands launch the framework through the package runner (npx --no-install, pnpm exec, yarn, bun run) because node_modules/.bin is not on PATH

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_pair --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_pair (4)

Reason:
RIPR-SPEC-0140: issue 5988 populates identity.config_identity from the canonical finding-affecting config fingerprint whenever a ripr.toml is loaded, so fixtures that load one record it (single-field intended flip, formatting-only 1-line drift)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_pair --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_pair (5)

Reason:
RIPR-SPEC-0140: issue 5988 review repair publishes the fingerprint of the exact loaded ripr.toml text in identity.config_identity, so fixtures that load one record the text fingerprint (single-field intended flip, formatting-only 1-line drift)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_pair --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
