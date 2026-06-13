# Golden Output Changes

## Pending

Reason:
document the local-callable-instance limitation (#1172): conservative weakly_exposed pinned; target exposed when relation+oracle-extraction traces local bindings

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
