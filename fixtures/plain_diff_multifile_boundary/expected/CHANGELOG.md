# Golden Output Changes

## Pending

Reason:
new fixture for RANK-2 fix: plain unified diff with two file sections now correctly produces changed_rust_files=2 with probes on src/a.rs and src/b.rs, no phantom path-marker probe

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless plain_diff_multifile_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
