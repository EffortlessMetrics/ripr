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

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless rust_transitive_reach_negative --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
