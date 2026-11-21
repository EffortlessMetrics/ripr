# Golden Output Changes

## Pending — rust_dep_edge_cross_crate_positive (1)

Reason:
RIPR-SPEC-0172: new corpus fixture pinning the cross-crate dependency-edge admit through the full adapter (issue #3620); initial bless of positive control

Command:
`cargo xtask goldens bless rust_dep_edge_cross_crate_positive --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
