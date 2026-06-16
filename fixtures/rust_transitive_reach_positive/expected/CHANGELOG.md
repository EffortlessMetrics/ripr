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

## Pending

Reason:
RIPR-SPEC-0115: transitive-reach limitation now names the witnessing test (file:line) and entry symbol in evidence; class and static_limit_kind unchanged (message-only drift)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0115: human output now surfaces the transitive-reach witness under a 'Where to look' section (file:line + entry symbol); JSON evidence single-sourced via shared prefix

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#1162 explain enhancement: human output now shows the named static limitation + plain-English meaning (additive 'Static limitation' section, message-only)

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
