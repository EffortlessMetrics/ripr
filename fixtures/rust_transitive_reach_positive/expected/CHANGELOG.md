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

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0115: witnessed transitive limitation no longer claims no tests were found

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
rust limitation evidence now surfaces last edge, unresolved edge, analyzer route, and non-claim

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
structured-static-limitation-detail-json

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`

## Pending

Reason:
align transitive-reach limitation depth text with RIPR-SPEC-0114

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
name integration public API reach limitation

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless rust_transitive_reach_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
