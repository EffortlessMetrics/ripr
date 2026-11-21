# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0163: new fixture pinning the scanner-transition positive path (local-with-call-initializer operand jump, per-row scanner evaluation, boundary equality observed -> exposed)

Command:
`cargo xtask goldens bless scanner_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
