# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0114: pin initial transitive-reach positive fixture golden output (limitation fires: static_limit_kind=rust_transitive_reach_unresolved, class stays no_static_path)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "RIPR-SPEC-0114: initial transitive-reach positive fixture golden"`

Updated:
- `expected/check.json`
- `expected/human.txt`
## Pending

Reason:
initial bless: fixture runner uses mode=fast with relative paths; behavioral correctness verified (no_static_path + rust_transitive_reach_unresolved fires)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
