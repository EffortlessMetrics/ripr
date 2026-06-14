# Golden Output Changes

## Pending

Reason:
document the local-callable-instance limitation (#1172): conservative weakly_exposed pinned; target exposed when relation+oracle-extraction traces local bindings

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive related_tests_total (#1204) + relation_reason/confidence (#1207) fields; fixture added by #1205 after those landed

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
surface smoke oracle via local_binding relation: corrects the misleading 'no direct test' card to 'strengthen existing smoke assertion'; class stays weakly_exposed (analysis/python-local-callable-instance-alignment)

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
