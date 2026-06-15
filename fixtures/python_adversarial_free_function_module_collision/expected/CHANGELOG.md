# Golden Output Changes

## Pending

Reason:
pin Cluster B free-function module-identity false-exposed: same-named function from a different module must not credit exposed

Command:
`cargo xtask goldens bless python_adversarial_free_function_module_collision --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
strong exact-value oracle so the module-identity gate is exercised end-to-end (not masked by a weak-oracle early return); changed_sink_token sibling branch now gated too

Command:
`cargo xtask goldens bless python_adversarial_free_function_module_collision --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
