# Golden Output Changes

## Pending — ts_repair_packet_boundary_parameter_constant_shaped (1)

Reason:
RIPR-SPEC-0027: a CONSTANT_CASE owner parameter carries the parameter-pair fact, and the projection reads it as a parameter instead of an unresolved module constant (#4759 review)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_constant_shaped --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_constant_shaped (2)

Reason:
RIPR-SPEC-0085: TypeScript verify commands launch the framework through the package runner (npx --no-install, pnpm exec, yarn, bun run) because node_modules/.bin is not on PATH

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_constant_shaped --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_constant_shaped (3)

Reason:
RIPR-SPEC-0140: issue 5988 populates identity.config_identity from the canonical finding-affecting config fingerprint whenever a ripr.toml is loaded, so fixtures that load one record it (single-field intended flip, formatting-only 1-line drift)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_constant_shaped --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_repair_packet_boundary_parameter_constant_shaped (4)

Reason:
RIPR-SPEC-0140: issue 5988 review repair publishes the fingerprint of the exact loaded ripr.toml text in identity.config_identity, so fixtures that load one record the text fingerprint (single-field intended flip, formatting-only 1-line drift)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_constant_shaped --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
