# Golden Output Changes

## Pending

Reason:
new fixture: a changed default value is not exposed when every strong related call overrides the parameter (#1289 trap 45)

Command:
`cargo xtask goldens bless python_adversarial_default_value_overridden --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
