# Golden Output Changes

## Pending

Reason:
new fixture: length-invariant f-string change observed only via len() is not exposed (#1290 Class A 1b)

Command:
`cargo xtask goldens bless python_adversarial_fstring_length_invariant_aggregate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
