# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094 Part B: Mode::Warm assertion does NOT confirm Mode::Frozen arm (qualifier-blind hole); initial golden locking the variant-scope fix

Command:
`cargo xtask goldens bless match_arm_type_token_blind --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless match_arm_type_token_blind --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless match_arm_type_token_blind --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless match_arm_type_token_blind --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
