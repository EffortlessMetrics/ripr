# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094: ReturnValue probe with no token_match now emits weakly_exposed/observation_unverified (was exposed/1.00 via single-assertion escape hatch); initial golden for the escape-hatch proof fixture

Command:
`cargo xtask goldens bless observation_unverified_return_value --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless observation_unverified_return_value --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless observation_unverified_return_value --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
