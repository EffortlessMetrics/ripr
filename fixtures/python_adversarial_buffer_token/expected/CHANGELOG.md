# Golden Output Changes

## Pending

Reason:
new adversarial false-exposed guard: token coincidence buffer⊂buffered_output stays weakly_exposed/orthogonal end-to-end (#1224 regression guard)

Command:
`cargo xtask goldens bless python_adversarial_buffer_token --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
