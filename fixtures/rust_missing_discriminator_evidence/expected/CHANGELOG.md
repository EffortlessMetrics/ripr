# Golden Output Changes

## Pending

Reason:
PR #1601 PR1 adds a minimized broad-error producer limitation control and exact-head evidence mappings; preserve current fail-closed output.

Command:
`cargo xtask goldens bless rust_missing_discriminator_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless rust_missing_discriminator_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
