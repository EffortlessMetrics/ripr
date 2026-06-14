# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0087 §PR7: complete TS repair packet now actionable

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: actionable TS packet now surfaced via shared renderer (verify+receipt+edit-surface+must_not_change+canonical_gap_id)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 PR8 fix: actionable complete_repair_packet finding no longer emits contradictory blocked-case messaging (why not actionable / evidence needed / only-after-available); preview actionability + card now read why-actionable + repair-action, Next step confirms completeness

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
fix: JSON/SARIF recommended_next_step serialized raw blocked-case string despite repair_packet_ready:true; now uses shared reconcile_next_step fn (RIPR-SPEC-0088 §PR8)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_repair_packet_complete --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
