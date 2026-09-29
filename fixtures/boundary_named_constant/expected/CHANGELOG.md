# Golden Output Changes

## Pending — boundary_named_constant (1)

Reason:
RIPR-SPEC-0001: new fixture pinning named-constant boundaries: a same-file integer const resolves to its value, a test argument naming the const matches by identity, and an unresolvable const is reported as unknown instead of a missing discriminator

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (2)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (3)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (4)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — boundary_named_constant (5)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless boundary_named_constant --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
