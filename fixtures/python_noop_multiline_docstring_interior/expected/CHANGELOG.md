# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0028 / #1289: AST-backed multi-line docstring interior changes emit no probe.

Command:
`cargo xtask goldens bless python_noop_multiline_docstring_interior --reason "RIPR-SPEC-0028 / #1289: AST-backed multi-line docstring interior changes emit no probe"`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
