# Golden Output Changes

## Pending

Reason:
new adversarial false-exposed guard: token coincidence buffer⊂buffered_output stays weakly_exposed/orthogonal end-to-end (#1224 regression guard)

Command:
`cargo xtask goldens bless python_adversarial_buffer_token --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_buffer_token --reason "..."`

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
