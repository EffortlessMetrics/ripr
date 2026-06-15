# Golden Output Changes

## Pending

Reason:
new fixture for RANK-1 fix: removed-side probes for fn two now correctly land at line 8 (new-file coordinate), not line 6 (old-side coordinate that maps to fn one territory after +2 net delta from hunk 1)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
