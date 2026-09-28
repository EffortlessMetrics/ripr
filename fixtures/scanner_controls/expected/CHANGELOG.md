# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0163: new fixture pinning the scanner fail-closed controls (step bound, computed argument, computed next-state arm, bare-identifier arm -> weakly_exposed, no scanner hop)

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_controls (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless scanner_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
