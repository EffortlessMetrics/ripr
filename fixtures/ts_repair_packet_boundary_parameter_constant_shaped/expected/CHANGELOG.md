# Golden Output Changes

## Pending — ts_repair_packet_boundary_parameter_constant_shaped (1)

Reason:
RIPR-SPEC-0027: a CONSTANT_CASE owner parameter carries the parameter-pair fact, and the projection reads it as a parameter instead of an unresolved module constant (#4759 review)

Command:
`cargo xtask goldens bless ts_repair_packet_boundary_parameter_constant_shaped --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
