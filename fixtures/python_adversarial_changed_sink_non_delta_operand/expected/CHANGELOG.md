# Golden Output Changes

## Pending

Reason:
pin #1276 non-delta-operand false-exposed: changed_sink_token must credit only on the delta, not an unchanged operand

Command:
`cargo xtask goldens bless python_adversarial_changed_sink_non_delta_operand --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
