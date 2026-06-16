# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0114: pin initial transitive-reach negative fixture golden output (limitation does NOT fire: no static_limit_kind, bare no_static_path)

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "RIPR-SPEC-0114: initial transitive-reach negative fixture golden"`

Updated:
- `expected/check.json`
- `expected/human.txt`
## Pending

Reason:
initial bless: fixture runner uses mode=fast with relative paths; behavioral correctness verified (bare no_static_path, no static_limit_kind — limitation correctly does NOT fire when no test reaches owner)

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
