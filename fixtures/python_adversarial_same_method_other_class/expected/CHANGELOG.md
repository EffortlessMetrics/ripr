# Golden Output Changes

## Pending

Reason:
false-exposed guard: same-named method on a different class stays weakly_exposed/orthogonal — bare method-name requires owner-class identity (analysis/python-false-exposed-attribute-call-owner-name)

Command:
`cargo xtask goldens bless python_adversarial_same_method_other_class --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
