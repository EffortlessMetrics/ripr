# Golden Output Changes

## Pending — owner_return_pin_trait_method (1)

Reason:
RIPR-SPEC-0197: initial golden for owner-return pins (#4478)

Command:
`cargo xtask goldens bless owner_return_pin_trait_method --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — owner_return_pin_trait_method (2)

Reason:
RIPR-SPEC-0197 (#4478) acceptance fixture re-rendered in main's #4520 exposure-word format: the trait default method stays pinned exposed through the byte-slice receiver

Command:
`cargo xtask goldens bless owner_return_pin_trait_method --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
