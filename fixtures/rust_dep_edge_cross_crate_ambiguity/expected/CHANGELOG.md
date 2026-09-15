# Golden Output Changes

## Pending — rust_dep_edge_cross_crate_ambiguity (1)

Reason:
RIPR-SPEC-0172: new corpus fixture pinning the wrong-crate oracle non-promotion through the full adapter (issue #3620); initial bless of charter member

Command:
`cargo xtask goldens bless rust_dep_edge_cross_crate_ambiguity --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
