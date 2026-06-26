# Golden Output Changes

## Pending

Reason:
new fixture: dict-literal change observed only at a sibling key is not exposed (#1290)

Command:
`cargo xtask goldens bless python_adversarial_dict_field_sibling_key --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
