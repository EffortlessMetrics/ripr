# Golden Output Changes

## Pending

Reason:
new adversarial false-exposed guard: mock assert_called_once observes the call not the changed return value; medium oracle stays weakly_exposed (never exposed)

Command:
`cargo xtask goldens bless python_adversarial_mock_call_not_value --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
