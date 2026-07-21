# Golden Output Changes

## Pending

Reason:
new fixture: length-invariant f-string change observed only via len() is not exposed (#1290 Class A 1b)

Command:
`cargo xtask goldens bless python_adversarial_fstring_length_invariant_aggregate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_fstring_length_invariant_aggregate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`
