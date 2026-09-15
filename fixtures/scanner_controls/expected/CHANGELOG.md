# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0163: new fixture pinning the scanner fail-closed controls (step bound, computed argument, computed next-state arm, bare-identifier arm -> weakly_exposed, no scanner hop)

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
