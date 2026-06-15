# Golden Output Changes

## Pending

Reason:
new fixture: list-literal change observed only at a sibling index is not exposed (#1290 Class A list)

Command:
`cargo xtask goldens bless python_adversarial_list_element_sibling_index --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
