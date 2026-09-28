# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0163: new fixture pinning the scanner-transition positive path (local-with-call-initializer operand jump, per-row scanner evaluation, boundary equality observed -> exposed)

Command:
`cargo xtask goldens bless scanner_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — scanner_positive (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless scanner_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
