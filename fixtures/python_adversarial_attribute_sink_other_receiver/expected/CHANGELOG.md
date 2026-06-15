# Golden Output Changes

## Pending

Reason:
pin Cluster A attribute changed-sink false-exposed: different receiver and value must not credit changed_sink_token

Command:
`cargo xtask goldens bless python_adversarial_attribute_sink_other_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
