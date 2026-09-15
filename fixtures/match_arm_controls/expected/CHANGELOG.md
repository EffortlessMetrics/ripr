# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0164: computed-value, guard, bare-identifier, and char-scrutinee match variants stay weakly_exposed with zero hop provenance

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
